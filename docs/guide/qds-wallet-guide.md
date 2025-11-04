# QDS Wallet Quickstart

This guide shows how to query balances from a Qanto node via the Quantum Data Service (QDS) using the wallet CLI.

## Prerequisites

- A running Qanto node built with libp2p and QDS enabled (default in `src/p2p.rs`).
- The node's libp2p multiaddr including its peer ID. Example:

```
/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEu5pQv3oJc3j9bBmQYF5uWfZrKQhQAbCdEfGhIjKlMn
```

You can obtain the peer ID from node logs or by inspecting the node's libp2p identify output.

## Balance Query Options

The `qantowallet` provides multiple balance query methods, each optimized for different scenarios:

### 1. Local-First Balance Query (Fastest)

For the fastest balance lookup with no network access:

```bash
qantowallet balance <ADDRESS_HEX> --local
```

**Benefits:**
- Fastest response time (no network latency)
- Works offline/air-gapped
- Maximum privacy (no network traffic)
- Uses local storage and UTXO scanning

### 2. Smart Balance Query (Recommended)

Intelligent fallback system that tries local first, then network sources:

```bash
qantowallet balance <ADDRESS_HEX>
```

**Fallback Order:**
1. Local storage (fastest)
2. QDS via P2P (if available)
3. gRPC node connection (most reliable)

### 3. QDS-Specific Query

Use the `--qds-multiaddr` flag to query balances directly from a specific peer:

```bash
qantowallet balance <ADDRESS_HEX> --qds-multiaddr \
  "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEu5pQv3oJc3j9bBmQYF5uWfZrKQhQAbCdEfGhIjKlMn"
```

Output example:

```
📊 Balance for ae527b...f9d3: 123.456789 QANTO (123456789000 base units)
```

## Performance Comparison

| Method | Speed | Network Required | Use Case |
|--------|-------|------------------|----------|
| `--local` | Fastest | No | Offline, privacy-focused |
| Default | Fast | Optional | General use, best reliability |
| `--qds-multiaddr` | Medium | Yes | Specific peer targeting |
| `--node-url` | Slower | Yes | Traditional gRPC fallback |

## Notes

- The wallet uses an ephemeral libp2p identity and the QDS request-response protocol name `/qanto/qds/1`.
- A 10-second timeout is applied to QDS requests by default.
- If `--qds-multiaddr` is not provided, the wallet falls back to gRPC (`--node_url`) or WebSocket for `--follow`.
- When `--qds-multiaddr` is provided for the `balance` command, the wallet enters lightweight mode and skips any local P2P discovery/initialization to minimize start-up and resource usage.
- The new local-first approach prioritizes local storage for optimal performance while maintaining network fallbacks for reliability.

## Configuration

While QDS is driven via CLI, node configuration remains in `config.toml`. Example network settings:

```toml
[network]
port = 30333
bootstrap_nodes = [
  "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEu5pQv3oJc3j9bBmQYF5uWfZrKQhQAbCdEfGhIjKlMn"
]
```

Ensure your node listens on the TCP port used by the multiaddr and that QDS is enabled in the node’s P2P behaviour (default in `src/p2p.rs`).

## Troubleshooting

- Invalid multiaddr errors usually indicate a missing `/p2p/<peerid>` component.
- Timeouts can indicate connectivity issues or a node that does not implement QDS.
- Verify the node exposes QDS by checking for `RequestResponse` events in its logs.