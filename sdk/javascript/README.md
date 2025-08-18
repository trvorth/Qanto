# Qanto Network JavaScript SDK

A comprehensive TypeScript/JavaScript SDK for interacting with the Qanto Network blockchain.

## Features

- **HTTP API Client**: Full REST API support for blockchain operations
- **WebSocket Client**: Real-time subscriptions for blocks, transactions, and network events
- **GraphQL Client**: Advanced querying capabilities with subscriptions
- **Type Safety**: Full TypeScript support with comprehensive type definitions
- **Multi-Network**: Support for mainnet, testnet, and local development
- **Error Handling**: Robust error handling with custom error types
- **Utilities**: Helper functions for validation, formatting, and blockchain operations

## Installation

```bash
npm install @qanto/sdk
# or
yarn add @qanto/sdk
```

## Quick Start

### Basic Usage

```typescript
import { QantoClient, NETWORKS } from '@qanto/sdk';

// Initialize client
const client = new QantoClient({
  network: NETWORKS.MAINNET,
  // Optional: custom endpoints
  httpEndpoint: 'https://api.qanto.network',
  wsEndpoint: 'wss://ws.qanto.network',
  graphqlEndpoint: 'https://graphql.qanto.network'
});

// Get blockchain info
const info = await client.getNodeInfo();
console.log('Network:', info.network);
console.log('Block Height:', info.blockHeight);

// Get account balance
const balance = await client.getBalance('qanto1abc...');
console.log('Balance:', balance.total);

// Submit transaction
const txHash = await client.submitTransaction('0x...');
console.log('Transaction submitted:', txHash);
```

### WebSocket Subscriptions

```typescript
// Subscribe to new blocks
client.ws.on('block', (block) => {
  console.log('New block:', block.height, block.hash);
});

// Subscribe to transactions
client.ws.on('transaction', (tx) => {
  console.log('New transaction:', tx.hash);
});

// Subscribe to network health
client.ws.on('network_health', (health) => {
  console.log('Network health:', health);
});

// Connect and subscribe
await client.ws.connect();
await client.ws.subscribeToBlocks();
await client.ws.subscribeToTransactions();
```

### GraphQL Queries

```typescript
// Get block with transactions
const block = await client.graphql.getBlock('latest');
console.log('Block transactions:', block.transactions.length);

// Get transaction details
const tx = await client.graphql.getTransaction('0x...');
console.log('Transaction fee:', tx.fee);

// Custom GraphQL query
const result = await client.graphql.query(`
  query {
    blockchainInfo {
      version
      network
      blockHeight
    }
  }
`);
```

## API Reference

### QantoClient

Main client class for interacting with the Qanto Network.

#### Constructor

```typescript
new QantoClient(config: QantoClientConfig)
```

#### Methods

- `getNodeInfo()`: Get blockchain node information
- `getBlock(hashOrHeight)`: Get block by hash or height
- `getTransaction(hash)`: Get transaction by hash
- `getBalance(address)`: Get account balance
- `getUTXOs(address)`: Get unspent transaction outputs
- `submitTransaction(hex)`: Submit a transaction
- `getMempool()`: Get mempool information
- `getAnalyticsDashboard()`: Get analytics data

### WebSocket Client

Real-time event subscriptions.

#### Events

- `connected`: WebSocket connection established
- `disconnected`: WebSocket connection closed
- `error`: Connection or message error
- `block`: New block notification
- `transaction`: New transaction notification
- `network_health`: Network health update
- `analytics`: Analytics dashboard update

#### Methods

- `connect()`: Connect to WebSocket server
- `disconnect()`: Disconnect from WebSocket server
- `subscribeToBlocks()`: Subscribe to block notifications
- `subscribeToTransactions()`: Subscribe to transaction notifications
- `subscribeToNetworkHealth()`: Subscribe to network health updates
- `subscribeToAnalytics()`: Subscribe to analytics updates

### GraphQL Client

Advanced querying with GraphQL.

#### Methods

- `getBlockchainInfo()`: Get blockchain information
- `getBlock(identifier)`: Get block by hash or height
- `getTransaction(hash)`: Get transaction by hash
- `getBalance(address)`: Get account balance
- `getMempoolInfo()`: Get mempool information
- `getNetworkStats()`: Get network statistics
- `submitTransaction(hex)`: Submit transaction via GraphQL
- `query(query, variables)`: Execute custom GraphQL query
- `mutate(mutation, variables)`: Execute GraphQL mutation

## Configuration

### Network Configuration

```typescript
import { NETWORKS, DEFAULT_ENDPOINTS } from '@qanto/sdk';

// Predefined networks
const config = {
  network: NETWORKS.MAINNET, // or TESTNET, LOCAL
};

// Custom endpoints
const customConfig = {
  network: NETWORKS.MAINNET,
  httpEndpoint: 'https://custom-api.example.com',
  wsEndpoint: 'wss://custom-ws.example.com',
  graphqlEndpoint: 'https://custom-graphql.example.com'
};
```

### Error Handling

```typescript
import { QantoError, WebSocketError, GraphQLError } from '@qanto/sdk';

try {
  const balance = await client.getBalance('invalid-address');
} catch (error) {
  if (error instanceof QantoError) {
    console.error('API Error:', error.message);
  } else if (error instanceof WebSocketError) {
    console.error('WebSocket Error:', error.message);
  } else if (error instanceof GraphQLError) {
    console.error('GraphQL Error:', error.message);
  }
}
```

## Utilities

The SDK includes various utility functions:

```typescript
import { 
  isValidQantoAddress,
  isValidTransaction,
  formatAmount,
  parseAmount,
  formatTimestamp,
  retryWithBackoff
} from '@qanto/sdk';

// Validate address
if (isValidQantoAddress('qanto1abc...')) {
  console.log('Valid address');
}

// Format amounts
const formatted = formatAmount(1000000, 6); // "1.000000"
const parsed = parseAmount('1.5', 6); // 1500000

// Retry with exponential backoff
const result = await retryWithBackoff(async () => {
  return await client.getNodeInfo();
}, 3, 1000);
```

## Examples

Check the `examples/` directory for more comprehensive examples:

- `basic-usage.js`: Basic API usage
- `websocket-subscriptions.js`: WebSocket event handling
- `graphql-queries.js`: GraphQL query examples
- `transaction-monitoring.js`: Monitor transactions
- `network-analytics.js`: Network analytics dashboard

## Development

### Building

```bash
npm run build
```

### Testing

```bash
npm test
```

### Linting

```bash
npm run lint
```

## Contributing

Contributions are welcome! Please read our [Contributing Guide](../../CONTRIBUTING.md) for details.

## License

MIT License - see [LICENSE](../../LICENSE) file for details.

## Support

- Documentation: [https://docs.qanto.network](https://docs.qanto.network)
- GitHub Issues: [https://github.com/qanto-network/qanto/issues](https://github.com/qanto-network/qanto/issues)
- Discord: [https://discord.gg/qanto](https://discord.gg/qanto)
- Twitter: [@QantoNetwork](https://twitter.com/QantoNetwork)

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and updates.