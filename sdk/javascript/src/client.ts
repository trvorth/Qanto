import axios, { AxiosInstance, AxiosRequestConfig } from 'axios';
import { EventEmitter } from 'eventemitter3';
import { QantoWebSocket } from './websocket';
import { QantoGraphQL } from './graphql';
import {
  QantoClientConfig,
  QantoBlock,
  Transaction,
  UTXO,
  NodeInfo,
  NetworkHealth,
  AnalyticsDashboardData,
  ApiResponse,
  PaginatedResponse,
  WalletInfo,
  QantoSDKError,
  NetworkError,
  ValidationError,
  Network,
  DEFAULT_ENDPOINTS
} from './types';
import { validateAddress, validateTransaction } from './utils';

export class QantoClient extends EventEmitter {
  private httpClient: AxiosInstance;
  private config: Required<QantoClientConfig>;
  public websocket: QantoWebSocket;
  public graphql: QantoGraphQL;

  constructor(config: QantoClientConfig = {}) {
    super();
    
    // Set default configuration
    this.config = {
      network: config.network || 'local',
      timeout: config.timeout || 30000,
      retryAttempts: config.retryAttempts || 3,
      retryDelay: config.retryDelay || 1000,
      apiKey: config.apiKey || '',
      userAgent: config.userAgent || `@qanto/sdk/1.0.0`,
      ...config
    };

    // Set endpoints based on network if not provided
    const networkEndpoints = DEFAULT_ENDPOINTS[this.config.network as Network];
    this.config.httpEndpoint = this.config.httpEndpoint || networkEndpoints.http;
    this.config.websocketEndpoint = this.config.websocketEndpoint || networkEndpoints.websocket;
    this.config.graphqlEndpoint = this.config.graphqlEndpoint || networkEndpoints.graphql;

    // Initialize HTTP client
    this.httpClient = axios.create({
      baseURL: this.config.httpEndpoint,
      timeout: this.config.timeout,
      headers: {
        'Content-Type': 'application/json',
        'User-Agent': this.config.userAgent,
        ...(this.config.apiKey && { 'Authorization': `Bearer ${this.config.apiKey}` })
      }
    });

    // Add response interceptor for error handling
    this.httpClient.interceptors.response.use(
      (response) => response,
      (error) => {
        if (error.response) {
          throw new NetworkError(
            error.response.data?.message || error.message,
            error.response.status,
            error.response.data
          );
        } else if (error.request) {
          throw new NetworkError('Network request failed', undefined, error);
        } else {
          throw new QantoSDKError(error.message);
        }
      }
    );

    // Initialize WebSocket and GraphQL clients
    this.websocket = new QantoWebSocket(this.config.websocketEndpoint, this);
    this.graphql = new QantoGraphQL(this.config.graphqlEndpoint, this);
  }

  // Node information methods
  async getNodeInfo(): Promise<NodeInfo> {
    const response = await this.httpClient.get<NodeInfo>('/info');
    return response.data;
  }

  async getNetworkHealth(): Promise<NetworkHealth> {
    const response = await this.httpClient.get<NetworkHealth>('/health');
    return response.data;
  }

  async getConnectedPeers(): Promise<string[]> {
    const response = await this.httpClient.get<string[]>('/p2p_getConnectedPeers');
    return response.data;
  }

  // Block methods
  async getBlock(blockId: string): Promise<QantoBlock> {
    if (!blockId || typeof blockId !== 'string') {
      throw new ValidationError('Block ID must be a non-empty string');
    }
    
    const response = await this.httpClient.get<QantoBlock>(`/block/${blockId}`);
    return response.data;
  }

  async getBlocks(page: number = 1, perPage: number = 20): Promise<PaginatedResponse<QantoBlock>> {
    const response = await this.httpClient.get<string[]>('/blocks');
    const blockIds = response.data;
    
    // Simple pagination logic
    const startIndex = (page - 1) * perPage;
    const endIndex = startIndex + perPage;
    const paginatedIds = blockIds.slice(startIndex, endIndex);
    
    // Fetch block details
    const blocks = await Promise.all(
      paginatedIds.map(id => this.getBlock(id))
    );
    
    return {
      items: blocks,
      total: blockIds.length,
      page,
      per_page: perPage,
      has_next: endIndex < blockIds.length,
      has_prev: page > 1
    };
  }

  async getLatestBlocks(count: number = 10): Promise<QantoBlock[]> {
    const response = await this.httpClient.get<string[]>('/blocks');
    const latestIds = response.data.slice(-count);
    
    return Promise.all(latestIds.map(id => this.getBlock(id)));
  }

  // Transaction methods
  async submitTransaction(transaction: Transaction): Promise<string> {
    if (!validateTransaction(transaction)) {
      throw new ValidationError('Invalid transaction format');
    }
    
    const response = await this.httpClient.post<string>('/transaction', transaction);
    return response.data;
  }

  async getMempool(): Promise<Record<string, Transaction>> {
    const response = await this.httpClient.get<Record<string, Transaction>>('/mempool');
    return response.data;
  }

  // Wallet and UTXO methods
  async getBalance(address: string): Promise<number> {
    if (!validateAddress(address)) {
      throw new ValidationError('Invalid address format');
    }
    
    const response = await this.httpClient.get<number>(`/balance/${address}`);
    return response.data;
  }

  async getUTXOs(address: string): Promise<Record<string, UTXO>> {
    if (!validateAddress(address)) {
      throw new ValidationError('Invalid address format');
    }
    
    const response = await this.httpClient.get<Record<string, UTXO>>(`/utxos/${address}`);
    return response.data;
  }

  async getWalletInfo(address: string): Promise<WalletInfo> {
    if (!validateAddress(address)) {
      throw new ValidationError('Invalid address format');
    }
    
    const [balance, utxos] = await Promise.all([
      this.getBalance(address),
      this.getUTXOs(address)
    ]);
    
    return {
      address,
      balance,
      utxo_count: Object.keys(utxos).length,
      transaction_count: 0 // This would need to be implemented in the API
    };
  }

  // Analytics methods
  async getAnalyticsDashboard(): Promise<AnalyticsDashboardData> {
    const response = await this.httpClient.get<AnalyticsDashboardData>('/analytics/dashboard');
    return response.data;
  }

  // DAG methods
  async getDAGInfo(): Promise<any> {
    const response = await this.httpClient.get('/dag');
    return response.data;
  }

  async getPublishReadiness(): Promise<any> {
    const response = await this.httpClient.get('/publish-readiness');
    return response.data;
  }

  // Utility methods
  async ping(): Promise<boolean> {
    try {
      await this.httpClient.get('/health');
      return true;
    } catch {
      return false;
    }
  }

  async waitForConnection(timeout: number = 10000): Promise<boolean> {
    const startTime = Date.now();
    
    while (Date.now() - startTime < timeout) {
      if (await this.ping()) {
        return true;
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    
    return false;
  }

  // Configuration methods
  getConfig(): Readonly<Required<QantoClientConfig>> {
    return { ...this.config };
  }

  updateConfig(newConfig: Partial<QantoClientConfig>): void {
    this.config = { ...this.config, ...newConfig };
    
    // Update HTTP client if endpoint changed
    if (newConfig.httpEndpoint) {
      this.httpClient.defaults.baseURL = newConfig.httpEndpoint;
    }
    
    // Update timeout if changed
    if (newConfig.timeout) {
      this.httpClient.defaults.timeout = newConfig.timeout;
    }
    
    // Update headers if API key changed
    if (newConfig.apiKey !== undefined) {
      if (newConfig.apiKey) {
        this.httpClient.defaults.headers['Authorization'] = `Bearer ${newConfig.apiKey}`;
      } else {
        delete this.httpClient.defaults.headers['Authorization'];
      }
    }
  }

  // Cleanup method
  async disconnect(): Promise<void> {
    await this.websocket.disconnect();
    this.removeAllListeners();
  }

  // Private helper method for retrying requests
  private async retryRequest<T>(
    requestFn: () => Promise<T>,
    attempts: number = this.config.retryAttempts
  ): Promise<T> {
    let lastError: Error;
    
    for (let i = 0; i < attempts; i++) {
      try {
        return await requestFn();
      } catch (error) {
        lastError = error as Error;
        
        if (i < attempts - 1) {
          await new Promise(resolve => 
            setTimeout(resolve, this.config.retryDelay * Math.pow(2, i))
          );
        }
      }
    }
    
    throw lastError!;
  }
}