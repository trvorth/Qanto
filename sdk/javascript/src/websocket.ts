import WebSocket from 'ws';
import { EventEmitter } from 'events';
import {
  WebSocketMessage,
  SubscriptionMessage,
  WebSocketError,
  QantoBlock,
  Transaction,
  NetworkHealth,
  AnalyticsDashboardData
} from './types';
import { delay, retryWithBackoff } from './utils';

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
  private heartbeatInterval: NodeJS.Timeout | null = null;
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

        this.ws.on('message', (data: WebSocket.Data) => {
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
      case 'block_notification':
        this.emit('block', message.data);
        break;
      case 'transaction_confirmation':
        this.emit('transaction', message.data);
        break;
      case 'network_health':
        this.emit('network_health', message.data);
        break;
      case 'analytics_update':
        this.emit('analytics', message.data);
        break;
      case 'mempool_update':
        this.emit('mempool_update', message.data.size || message.data);
        break;
      case 'security_alert':
        this.emit('security_alert', message.data);
        break;
      case 'subscription_confirmation':
        this.emit('subscription_confirmed', message.data.subscription_type);
        break;
      case 'error':
        this.emit('error', new WebSocketError(message.data.message || 'Unknown WebSocket error'));
        break;
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
    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
    
    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);
    
    setTimeout(async () => {
      try {
        await this.connect();
        // Re-establish subscriptions
        for (const subscription of this.subscriptions) {
          await this.subscribe(subscription);
        }
      } catch (error) {
        console.error('Reconnection failed:', error);
      }
    }, delay);
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    
    this.heartbeatInterval = setInterval(() => {
      if (this.isConnected()) {
        try {
          this.ws!.ping();
        } catch (error) {
          console.error('Heartbeat failed:', error);
        }
      }
    }, 30000); // Send ping every 30 seconds
  }

  private stopHeartbeat(): void {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }
}