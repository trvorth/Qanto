export * from './client';
export * from './types';
export * from './websocket';
export * from './graphql';
export * from './utils';

// Re-export the main client as default
export { QantoClient as default } from './client';

// Version information
export const VERSION = '1.0.0';

// Network constants
export const NETWORKS = {
  MAINNET: 'mainnet',
  TESTNET: 'testnet',
  LOCAL: 'local'
} as const;

export type Network = typeof NETWORKS[keyof typeof NETWORKS];

// Default endpoints
export const DEFAULT_ENDPOINTS = {
  [NETWORKS.MAINNET]: {
    http: 'https://api.qanto.network',
    websocket: 'wss://ws.qanto.network',
    graphql: 'https://graphql.qanto.network'
  },
  [NETWORKS.TESTNET]: {
    http: 'https://testnet-api.qanto.network',
    websocket: 'wss://testnet-ws.qanto.network',
    graphql: 'https://testnet-graphql.qanto.network'
  },
  [NETWORKS.LOCAL]: {
    http: 'p2p://discovery',  // Use P2P discovery instead of hardcoded endpoint
    websocket: 'p2p://discovery',  // Use P2P discovery for WebSocket
    graphql: 'p2p://discovery/graphql'  // Use P2P discovery for GraphQL
  }
} as const;