/// <reference path="./types/ws.d.ts" />
import WebSocket from 'ws';
import { EventEmitter } from 'events';
import {
  WebSocketMessage,
  SubscriptionMessage,
  WebSocketError,
  QantoBlock,
  Transaction,
  NetworkHealth,
  AnalyticsDashboardData,
  BalanceUpdate
} from './types';
// NOTE: Backoff helpers available in utils, not used directly here

export interface WebSocketEvents {
  'connected': () => void;
  'disconnected': (code: number, reason: string) => void;
  'error': (error: Error) => void;
  'message': (message: WebSocketMessage) => void;
  'block': (block: QantoBlock) => void;
  'transaction': (transaction: Transaction) => void;
  'network_health': (health: NetworkHealth) => void;
  'analytics': (data: AnalyticsDashboardData) => void;
  'mempool_update': (size: number) => void;
  'security_alert': (alert: any) => void;
  'subscription_confirmed': (type: string) => void;
  'balance_update': (update: BalanceUpdate) => void;
  'balance_subscription_confirmed': (info: { address: string; client_id?: string }) => void;
}

export class QantoWebSocket extends EventEmitter {
  private ws: WebSocket | null = null;
  private url: string;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private isConnecting = false;
  private isManuallyDisconnected = false;
  private subscriptions = new Set<string>();
  private balanceSubscriptions = new Set<string>();
  private heartbeatInterval: NodeJS.Timeout | null = null;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private client: any; // Reference to the main client

  constructor(url: string, client: any) {
    super();
    this.url = url;
    this.client = client;
  }

  /**
   * Connect to the WebSocket server
   */
  async connect(): Promise<void> {
    // NOTE: currently not used; reserve for future client-aware logic
    if (this.client) {
      // no-op read to satisfy TypeScript unused property lint
    }
    if (this.isConnecting || (this.ws && this.ws.readyState === WebSocket.OPEN)) {
      return;
    }

    this.isConnecting = true;
    this.isManuallyDisconnected = false;

    try {
      await this.createConnection();
    } catch (error) {
      this.isConnecting = false;
      throw new WebSocketError(`Failed to connect: ${(error as Error).message}`);
    }
  }

  /**
   * Disconnect from the WebSocket server
   */
  async disconnect(): Promise<void> {
    this.isManuallyDisconnected = true;
    this.stopHeartbeat();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    
    if (this.ws) {
      this.ws.close(1000, 'Manual disconnect');
      this.ws = null;
    }
    
    this.subscriptions.clear();
    this.reconnectAttempts = 0;
  }

  /**
   * Subscribe to a specific event type
   */
  async subscribe(subscriptionType: string, filters?: Record<string, any>): Promise<void> {
    if (!this.isConnected()) {
      await this.connect();
    }

    const message: SubscriptionMessage = {
      action: 'subscribe',
      subscription_type: subscriptionType as any,
      ...(filters && { filters })
    };

    this.send(message);
    this.subscriptions.add(subscriptionType);
  }

  /**
   * Unsubscribe from a specific event type
   */
  async unsubscribe(subscriptionType: string): Promise<void> {
    if (!this.isConnected()) {
      return;
    }

    const message: SubscriptionMessage = {
      action: 'unsubscribe',
      subscription_type: subscriptionType as any
    };

    this.send(message);
    this.subscriptions.delete(subscriptionType);
  }

  /**
   * Subscribe to block notifications
   */
  async subscribeToBlocks(): Promise<void> {
    await this.subscribe('blocks');
  }

  /**
   * Subscribe to transaction confirmations
   */
  async subscribeToTransactions(): Promise<void> {
    await this.subscribe('transactions');
  }

  /**
   * Subscribe to network health updates
   */
  async subscribeToNetworkHealth(): Promise<void> {
    await this.subscribe('network_health');
  }

  /**
   * Subscribe to analytics dashboard updates
   */
  async subscribeToAnalytics(): Promise<void> {
    await this.subscribe('analytics');
  }

  /**
   * Subscribe to security alerts
   */
  async subscribeToSecurityAlerts(): Promise<void> {
    await this.subscribe('security_alerts');
  }

  /**
   * Subscribe to balance updates for a specific address.
   * When finalizedOnly is true, uses structured subscription with filters; otherwise uses alias topic.
   */
  async subscribeToBalances(address: string, options?: { finalizedOnly?: boolean }): Promise<void> {
    if (!this.isConnected()) {
      await this.connect();
    }

    const finalizedOnly = options?.finalizedOnly === true;
    if (finalizedOnly) {
      const msg = {
        type: 'subscribe',
        subscription_type: 'balances',
        filters: { address, finalized_only: 'true' }
      };
      this.send(msg);
    } else {
      const aliasMsg = {
        type: 'subscribe',
        topics: [`wallet_balance:${address}`]
      };
      this.send(aliasMsg);
    }

    this.balanceSubscriptions.add(address);
  }

  /**
   * Check if WebSocket is connected
   */
  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  /**
   * Get current connection state
   */
  getConnectionState(): string {
    if (!this.ws) return 'CLOSED';
    
    switch (this.ws.readyState) {
      case WebSocket.CONNECTING: return 'CONNECTING';
      case WebSocket.OPEN: return 'OPEN';
      case WebSocket.CLOSING: return 'CLOSING';
      case WebSocket.CLOSED: return 'CLOSED';
      default: return 'UNKNOWN';
    }
  }

  /**
   * Get active subscriptions
   */
  getSubscriptions(): string[] {
    return Array.from(this.subscriptions);
  }

  private async createConnection(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.url);
        
        const connectionTimeout = setTimeout(() => {
          if (this.ws) {
            this.ws.terminate();
          }
          reject(new Error('Connection timeout'));
        }, 10000);

        this.ws.on('open', () => {
          clearTimeout(connectionTimeout);
          this.isConnecting = false;
          this.reconnectAttempts = 0;
          this.startHeartbeat();
          this.emit('connected');
          resolve();
        });

        this.ws.on('message', (data: any) => {
          try {
            const message: WebSocketMessage = JSON.parse(data.toString());
            this.handleMessage(message);
          } catch (error) {
            this.emit('error', new WebSocketError(`Failed to parse message: ${(error as Error).message}`));
          }
        });

        this.ws.on('close', (code: number, reason: string) => {
          clearTimeout(connectionTimeout);
          this.isConnecting = false;
          this.stopHeartbeat();
          this.emit('disconnected', code, reason);
          
          if (!this.isManuallyDisconnected && this.reconnectAttempts < this.maxReconnectAttempts) {
            this.scheduleReconnect();
          }
        });

        this.ws.on('error', (error: Error) => {
          clearTimeout(connectionTimeout);
          this.isConnecting = false;
          this.emit('error', new WebSocketError(`WebSocket error: ${error.message}`));
          reject(error);
        });

      } catch (error) {
        this.isConnecting = false;
        reject(error);
      }
    });
  }

  private handleMessage(message: WebSocketMessage): void {
    this.emit('message', message);

    switch (message.type) {
      case 'block_notification': {
        const m: any = message as any;
        this.emit('block', m.block ?? m.data);
        break;
      }
      case 'transaction_confirmation': {
        const m: any = message as any;
        this.emit('transaction', m.transaction ?? m.data);
        break;
      }
      case 'network_health':
        this.emit('network_health', (message as any).data ?? (message as any));
        break;
      case 'analytics_update':
        this.emit('analytics', (message as any).data ?? (message as any));
        break;
      case 'mempool_update': {
        const m: any = message as any;
        this.emit('mempool_update', m.size ?? (m.data?.size ?? m.data));
        break;
      }
      case 'security_alert':
        this.emit('security_alert', (message as any).alert ?? (message as any).data);
        break;
      case 'subscription_confirmed': {
        const m: any = message as any;
        this.emit('subscription_confirmed', m.subscription_type);
        break;
      }
      case 'subscription_confirmation': {
        const m: any = message as any;
        this.emit('subscription_confirmed', m.data?.subscription_type ?? m.subscription_type);
        break;
      }
      case 'balance_subscription_confirmed': {
        const m: any = message as any;
        this.emit('balance_subscription_confirmed', { address: m.address, client_id: m.client_id });
        break;
      }
      case 'balance_snapshot': {
        const m: any = message as any;
        this.emit('balance_update', {
          address: m.address,
          balance: {
            spendable_confirmed: m.total_confirmed,
            immature_coinbase_confirmed: 0,
            unconfirmed_delta: m.total_unconfirmed ?? 0,
            total_confirmed: m.total_confirmed,
          },
          timestamp: m.timestamp,
          finalized: true,
        });
        break;
      }
      case 'balance_delta_update': {
        const m: any = message as any;
        this.emit('balance_update', {
          address: m.address,
          balance: {
            spendable_confirmed: 0,
            immature_coinbase_confirmed: 0,
            unconfirmed_delta: 0,
            total_confirmed: 0,
          },
          timestamp: m.timestamp,
          finalized: m.finalized,
          delta_confirmed: m.delta_confirmed,
          delta_unconfirmed: m.delta_unconfirmed,
        } as any);
        break;
      }
      case 'balance_update': {
        const m: any = message as any;
        const update: BalanceUpdate = {
          address: m.address,
          balance: m.balance,
          timestamp: m.timestamp,
          finalized: m.finalized,
        };
        this.emit('balance_update', update);
        break;
      }
      case 'error': {
        const m: any = message as any;
        const msg = (m.message ?? m.data?.message ?? 'Unknown WebSocket error');
        this.emit('error', new WebSocketError(msg));
        break;
      }
      default:
        console.warn('Unknown message type:', message.type);
    }
  }

  private send(data: any): void {
    if (!this.isConnected()) {
      throw new WebSocketError('WebSocket is not connected');
    }

    try {
      this.ws!.send(JSON.stringify(data));
    } catch (error) {
      throw new WebSocketError(`Failed to send message: ${(error as Error).message}`);
    }
  }

  private async scheduleReconnect(): Promise<void> {
    if (this.isManuallyDisconnected) return;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.reconnectAttempts++;
    const delayMs = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
    if (process.env['QANTO_WS_DEBUG'] === '1') {
      console.log(`Reconnecting in ${delayMs}ms (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);
    }
    await new Promise<void>((resolve) => {
      this.reconnectTimer = setTimeout(async () => {
        try {
          await this.connect();
          for (const subscription of this.subscriptions) {
            await this.subscribe(subscription);
          }
          for (const addr of this.balanceSubscriptions) {
            await this.subscribeToBalances(addr);
          }
        } catch (error) {
          console.error('Reconnection failed:', error);
        } finally {
          resolve();
        }
      }, delayMs);
      (this.reconnectTimer as any)?.unref?.();
    });
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    
    this.heartbeatInterval = setInterval(() => {
      if (this.isConnected()) {
        try {
          this.ws!.ping();
        } catch (error) {
          // eslint-disable-next-line no-console
          console.error('Heartbeat failed:', error);
        }
      }
    }, 30000); // Send ping every 30 seconds
    (this.heartbeatInterval as any)?.unref?.();
  }

  private stopHeartbeat(): void {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }
}
