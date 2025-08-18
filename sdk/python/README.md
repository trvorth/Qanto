# Qanto Python SDK

A comprehensive Python SDK for interacting with the Qanto Network blockchain.

## Features

- **HTTP Client**: Full REST API support with sync/async methods
- **WebSocket Client**: Real-time subscriptions for blocks, transactions, and network events
- **GraphQL Client**: Flexible querying with subscriptions support
- **Type Safety**: Full type hints and Pydantic models
- **Multi-Network Support**: Mainnet, testnet, and custom network configurations
- **Error Handling**: Comprehensive error types and automatic retry logic
- **Rate Limiting**: Built-in rate limiting for API calls
- **Utilities**: Address validation, transaction building, and more

## Installation

```bash
pip install qanto-sdk
```

### Development Installation

```bash
git clone https://github.com/qanto-network/qanto-sdk-python.git
cd qanto-sdk-python
pip install -e .
```

## Quick Start

### Basic Usage

```python
from qanto import QantoClient, Network

# Initialize client
client = QantoClient(network=Network.MAINNET)

# Get node information
node_info = client.get_node_info()
print(f"Node version: {node_info.version}")

# Get latest block
latest_block = client.get_block()
print(f"Latest block height: {latest_block.height}")

# Get balance
balance = client.get_balance("qanto1abc123...")
print(f"Balance: {balance.confirmed} QANTO")

# Submit transaction
tx_data = {
    "inputs": [...],
    "outputs": [...]
}
result = client.submit_transaction(tx_data)
print(f"Transaction hash: {result.transaction_hash}")
```

### Async Usage

```python
import asyncio
from qanto import QantoClient, Network

async def main():
    client = QantoClient(network=Network.MAINNET)
    
    # Get node info asynchronously
    node_info = await client.aget_node_info()
    print(f"Node version: {node_info.version}")
    
    # Get multiple blocks concurrently
    blocks = await asyncio.gather(
        client.aget_block(height=100),
        client.aget_block(height=101),
        client.aget_block(height=102)
    )
    
    for block in blocks:
        print(f"Block {block.height}: {block.hash}")

asyncio.run(main())
```

### WebSocket Subscriptions

```python
import asyncio
from qanto import QantoWebSocket, Network

async def handle_new_block(block):
    print(f"New block: {block['height']} - {block['hash']}")

async def handle_new_transaction(tx):
    print(f"New transaction: {tx['hash']}")

async def main():
    ws = QantoWebSocket(network=Network.MAINNET)
    
    # Set up event handlers
    ws.on('block', handle_new_block)
    ws.on('transaction', handle_new_transaction)
    
    # Connect and subscribe
    await ws.connect()
    await ws.subscribe_to_blocks()
    await ws.subscribe_to_transactions()
    
    # Keep connection alive
    try:
        while True:
            await asyncio.sleep(1)
    except KeyboardInterrupt:
        await ws.disconnect()

asyncio.run(main())
```

### GraphQL Queries

```python
from qanto import QantoGraphQL, Network, DEFAULT_ENDPOINTS

# Initialize GraphQL client
graphql = QantoGraphQL(DEFAULT_ENDPOINTS[Network.MAINNET].graphql)

# Get blockchain info
blockchain_info = graphql.get_blockchain_info()
print(f"Chain height: {blockchain_info['height']}")

# Get block with transactions
block = graphql.get_block(height=1000)
print(f"Block has {len(block.transactions)} transactions")

# Get address balance and UTXOs
balance = graphql.get_balance("qanto1abc123...")
utxos = graphql.get_utxos("qanto1abc123...")
print(f"Balance: {balance['balance']}, UTXOs: {len(utxos)}")

# Subscribe to new blocks (async)
async def subscribe_blocks():
    async for block in graphql.subscribe_to_blocks():
        print(f"New block via GraphQL: {block['height']}")

# Run subscription
import asyncio
asyncio.run(subscribe_blocks())
```

## API Reference

### QantoClient

The main client class for interacting with the Qanto Network.

#### Initialization

```python
from qanto import QantoClient, Network, QantoClientConfig

# Basic initialization
client = QantoClient(network=Network.MAINNET)

# Custom configuration
config = QantoClientConfig(
    timeout=30,
    max_retries=5,
    rate_limit_requests=100,
    rate_limit_period=60
)
client = QantoClient(network=Network.TESTNET, config=config)

# Custom endpoints
client = QantoClient(
    http_endpoint="https://api.custom-qanto.com",
    websocket_endpoint="wss://ws.custom-qanto.com",
    graphql_endpoint="https://graphql.custom-qanto.com"
)
```

#### Methods

**Node Information**
- `get_node_info()` / `aget_node_info()` - Get node information
- `get_network_health()` / `aget_network_health()` - Get network health status

**Blockchain Data**
- `get_block(hash=None, height=None)` / `aget_block()` - Get block by hash or height
- `get_transaction(tx_hash)` / `aget_transaction()` - Get transaction by hash
- `get_balance(address)` / `aget_balance()` - Get address balance
- `get_utxos(address)` / `aget_utxos()` - Get UTXOs for address

**Mempool**
- `get_mempool_info()` / `aget_mempool_info()` - Get mempool information
- `get_mempool_transactions()` / `aget_mempool_transactions()` - Get pending transactions

**Transactions**
- `submit_transaction(transaction)` / `asubmit_transaction()` - Submit transaction
- `estimate_fee(transaction)` / `aestimate_fee()` - Estimate transaction fee

**Analytics**
- `get_analytics_dashboard()` / `aget_analytics_dashboard()` - Get dashboard data

### QantoWebSocket

WebSocket client for real-time subscriptions.

#### Usage

```python
from qanto import QantoWebSocket, Network

ws = QantoWebSocket(network=Network.MAINNET)

# Event handlers
ws.on('connected', lambda: print("Connected"))
ws.on('disconnected', lambda: print("Disconnected"))
ws.on('block', lambda block: print(f"New block: {block['height']}"))
ws.on('transaction', lambda tx: print(f"New tx: {tx['hash']}"))

# Connect and subscribe
await ws.connect()
await ws.subscribe_to_blocks()
await ws.subscribe_to_transactions(address="qanto1abc123...")
await ws.subscribe_to_network_health()

# Disconnect
await ws.disconnect()
```

### QantoGraphQL

GraphQL client for flexible querying.

#### Usage

```python
from qanto import QantoGraphQL

graphql = QantoGraphQL("https://graphql.qanto.network")

# Queries
blockchain_info = graphql.get_blockchain_info()
block = graphql.get_block(height=1000)
transaction = graphql.get_transaction("tx_hash")
balance = graphql.get_balance("address")
utxos = graphql.get_utxos("address")

# Mutations
result = graphql.submit_transaction(tx_data)

# Subscriptions (async)
async for block in graphql.subscribe_to_blocks():
    print(f"New block: {block['height']}")
```

## Configuration

### Networks

```python
from qanto import Network, DEFAULT_ENDPOINTS

# Available networks
Network.MAINNET
Network.TESTNET
Network.DEVNET

# Default endpoints
endpoints = DEFAULT_ENDPOINTS[Network.MAINNET]
print(endpoints.http)      # HTTP API endpoint
print(endpoints.websocket) # WebSocket endpoint
print(endpoints.graphql)   # GraphQL endpoint
```

### Client Configuration

```python
from qanto import QantoClientConfig

config = QantoClientConfig(
    timeout=30,                    # Request timeout in seconds
    max_retries=3,                # Maximum retry attempts
    rate_limit_requests=100,      # Requests per period
    rate_limit_period=60,         # Rate limit period in seconds
    user_agent="MyApp/1.0",       # Custom user agent
    enable_logging=True           # Enable debug logging
)
```

## Error Handling

The SDK provides comprehensive error handling with specific exception types:

```python
from qanto import (
    QantoError, NetworkError, APIError, ValidationError,
    TransactionError, InsufficientFundsError
)

try:
    balance = client.get_balance("invalid_address")
except ValidationError as e:
    print(f"Invalid address: {e}")
except NetworkError as e:
    print(f"Network error: {e}")
except APIError as e:
    print(f"API error: {e.status_code} - {e.message}")
except QantoError as e:
    print(f"General error: {e}")
```

## Utility Functions

```python
from qanto.utils import (
    validate_qanto_address,
    format_amount,
    parse_amount,
    calculate_transaction_fee,
    get_current_timestamp
)

# Address validation
is_valid = validate_qanto_address("qanto1abc123...")

# Amount formatting
formatted = format_amount(1000000)  # "1.000000 QANTO"
parsed = parse_amount("1.5 QANTO")  # 1500000

# Transaction fee calculation
fee = calculate_transaction_fee(tx_size=250, fee_rate=10)

# Timestamp utilities
timestamp = get_current_timestamp()
```

## Development

### Setup

```bash
git clone https://github.com/qanto-network/qanto-sdk-python.git
cd qanto-sdk-python
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -e ".[dev]"
```

### Testing

```bash
# Run tests
pytest

# Run tests with coverage
pytest --cov=qanto

# Run specific test file
pytest tests/test_client.py
```

### Code Quality

```bash
# Format code
black qanto/

# Lint code
flake8 qanto/

# Type checking
mypy qanto/
```

### Building

```bash
# Build package
python -m build

# Install locally
pip install dist/qanto_sdk-*.whl
```

## Examples

See the `examples/` directory for more comprehensive examples:

- `basic_usage.py` - Basic client usage
- `websocket_subscriptions.py` - WebSocket real-time data
- `graphql_queries.py` - GraphQL query examples
- `transaction_building.py` - Building and submitting transactions
- `wallet_integration.py` - Wallet functionality
- `analytics_dashboard.py` - Analytics and monitoring

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

- **Documentation**: [https://docs.qanto.network](https://docs.qanto.network)
- **GitHub Issues**: [https://github.com/qanto-network/qanto-sdk-python/issues](https://github.com/qanto-network/qanto-sdk-python/issues)
- **Discord**: [https://discord.gg/qanto](https://discord.gg/qanto)
- **Email**: support@qanto.network

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a list of changes and version history.