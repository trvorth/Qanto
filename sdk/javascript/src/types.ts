// Network types
export const NETWORKS = {
  MAINNET: 'mainnet',
  TESTNET: 'testnet',
  LOCAL: 'local'
} as const;

export type Network = typeof NETWORKS[keyof typeof NETWORKS];

// Default endpoints
export const DEFAULT_ENDPOINTS = {
  [NETWORKS.MAINNET]: {
    http: 'https://api.qanto.org',
    websocket: 'wss://ws.qanto.org',
    graphql: 'https://graphql.qanto.org'
  },
  [NETWORKS.TESTNET]: {
    http: 'https://testnet-api.qanto.org',
    websocket: 'wss://testnet-ws.qanto.org',
    graphql: 'https://testnet-graphql.qanto.org'
  },
  [NETWORKS.LOCAL]: {
    http: 'p2p://discovery',  // Use P2P discovery instead of hardcoded endpoint
    websocket: 'p2p://discovery',  // Use P2P discovery for WebSocket
    graphql: 'p2p://discovery/graphql'  // Use P2P discovery for GraphQL
  }
} as const;

// Core blockchain types
export interface QantoBlock {
  id: string;
  parent_ids: string[];
  timestamp: number;
  nonce: number;
  difficulty: number;
  transactions: Transaction[];
  miner_address: string;
  merkle_root: string;
  signature: string;
  block_hash: string;
  height: number;
  validator_signatures?: ValidatorSignature[];
}

export interface Transaction {
  id: string;
  inputs: TransactionInput[];
  outputs: TransactionOutput[];
  timestamp: number;
  signature: string;
  public_key: string;
  fee: number;
  nonce: number;
}

export interface TransactionInput {
  previous_output_id: string;
  signature: string;
  public_key: string;
}

export interface TransactionOutput {
  id: string;
  address: string;
  amount: number;
}

export interface UTXO {
  id: string;
  address: string;
  amount: number;
  transaction_id: string;
  output_index: number;
  block_id: string;
  timestamp: number;
}

export interface ValidatorSignature {
  validator_address: string;
  signature: string;
  timestamp: number;
}

// Network and node information
export interface NodeInfo {
  version: string;
  network: string;
  node_id: string;
  block_count: number;
  mempool_size: number;
  connected_peers: number;
  sync_status: string;
  uptime: number;
}

export interface NetworkHealth {
  block_count: number;
  mempool_size: number;
  utxo_count: number;
  connected_peers: number;
  sync_status: string;
}

export interface PeerInfo {
  peer_id: string;
  address: string;
  connected_at: number;
  last_seen: number;
  version: string;
}

// Analytics and dashboard data
export interface AnalyticsDashboardData {
  network_health: NetworkHealthMetrics;
  ai_performance: AIPerformanceMetrics;
  security_threats: SecurityThreatMetrics;
  economic_indicators: EconomicIndicatorMetrics;
  environmental_metrics: EnvironmentalMetrics;
  alerts: Alert[];
  historical_data: HistoricalData;
}

export interface NetworkHealthMetrics {
  total_nodes: number;
  active_validators: number;
  network_hashrate: number;
  average_block_time: number;
  transaction_throughput: number;
  network_latency: number;
  sync_percentage: number;
}

export interface AIPerformanceMetrics {
  omega_response_time: number;
  saga_processing_efficiency: number;
  predictive_accuracy: number;
  learning_rate: number;
  model_confidence: number;
}

export interface SecurityThreatMetrics {
  detected_attacks: number;
  blocked_transactions: number;
  suspicious_patterns: number;
  threat_level: 'low' | 'medium' | 'high' | 'critical';
  last_incident: number;
}

export interface EconomicIndicatorMetrics {
  total_supply: number;
  circulating_supply: number;
  market_cap: number;
  transaction_volume_24h: number;
  average_transaction_fee: number;
  staking_ratio: number;
}

export interface EnvironmentalMetrics {
  energy_consumption: number;
  carbon_footprint: number;
  renewable_energy_percentage: number;
  efficiency_score: number;
}

export interface Alert {
  id: string;
  severity: 'info' | 'warning' | 'error' | 'critical';
  category: 'network' | 'security' | 'performance' | 'economic' | 'environmental';
  title: string;
  message: string;
  timestamp: number;
  acknowledged: boolean;
}

export interface HistoricalData {
  time_series: TimeSeriesDataPoint[];
  aggregated_metrics: Record<string, number>;
}

export interface TimeSeriesDataPoint {
  timestamp: number;
  metric_name: string;
  value: number;
  metadata?: Record<string, any>;
}

// WebSocket message types
export interface WebSocketMessage {
  type: 'block_notification' | 'transaction_confirmation' | 'network_health' | 'mempool_update' | 'analytics_update' | 'security_alert' | 'subscription_confirmation' | 'error';
  data: any;
  timestamp: number;
}

export interface SubscriptionMessage {
  action: 'subscribe' | 'unsubscribe';
  subscription_type: 'blocks' | 'transactions' | 'network_health' | 'analytics' | 'security_alerts' | 'economic_indicators';
  filters?: Record<string, any>;
}

// GraphQL types
export interface GraphQLQuery {
  query: string;
  variables?: Record<string, any>;
}

export interface GraphQLResponse<T = any> {
  data?: T;
  errors?: GraphQLError[];
}

export interface GraphQLError {
  message: string;
  locations?: Array<{ line: number; column: number }>;
  path?: Array<string | number>;
  extensions?: Record<string, any>;
}

export class GraphQLError extends Error {
  constructor(message: string, public query?: string, public variables?: any) {
    super(message);
    this.name = 'GraphQLError';
  }
}

// Client configuration
export interface QantoClientConfig {
  httpEndpoint?: string;
  websocketEndpoint?: string;
  graphqlEndpoint?: string;
  network?: 'mainnet' | 'testnet' | 'local';
  timeout?: number;
  retryAttempts?: number;
  retryDelay?: number;
  apiKey?: string;
  userAgent?: string;
}

// API response types
export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  timestamp: number;
}

export interface PaginatedResponse<T = any> {
  items: T[];
  total: number;
  page: number;
  per_page: number;
  has_next: boolean;
  has_prev: boolean;
}

// Wallet and transaction building types
export interface WalletInfo {
  address: string;
  balance: number;
  utxo_count: number;
  transaction_count: number;
}

export interface TransactionBuilder {
  addInput(utxo: UTXO, privateKey: string): TransactionBuilder;
  addOutput(address: string, amount: number): TransactionBuilder;
  setFee(fee: number): TransactionBuilder;
  build(): Transaction;
  sign(privateKey: string): Transaction;
}

// Error types
export class QantoSDKError extends Error {
  constructor(
    message: string,
    public code?: string,
    public statusCode?: number,
    public details?: any
  ) {
    super(message);
    this.name = 'QantoSDKError';
  }
}

export class NetworkError extends QantoSDKError {
  constructor(message: string, statusCode?: number, details?: any) {
    super(message, 'NETWORK_ERROR', statusCode, details);
    this.name = 'NetworkError';
  }
}

export class ValidationError extends QantoSDKError {
  constructor(message: string, details?: any) {
    super(message, 'VALIDATION_ERROR', undefined, details);
    this.name = 'ValidationError';
  }
}

export class WebSocketError extends QantoSDKError {
  constructor(message: string, details?: any) {
    super(message, 'WEBSOCKET_ERROR', undefined, details);
    this.name = 'WebSocketError';
  }
}