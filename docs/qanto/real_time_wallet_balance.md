# Real-Time Wallet Balance (Local Storage)

This command queries the local node database for instantaneous wallet balance without network calls.

- Command: `qantowallet balance-rt <address> --db-dir <node_data_dir>`
- Aliases: `qantowallet realtime-balance <address> --db-dir <node_data_dir>`, `qantowallet rt-balance <address> --db-dir <node_data_dir>`
- Source: Reads `balances` and `utxos` from the node’s RocksDB-compatible storage via `QantoStorage`.
- Output: Displays confirmed balance in QANTO with base units.

Example:

```
qantowallet balance-rt ae527b01...d3 --db-dir ./data
qantowallet realtime-balance ae527b01...d3 --db-dir ./data
qantowallet rt-balance ae527b01...d3 --db-dir ./data
```

 Notes:

- Use `qantowallet balance <address> --follow` (alias: `--ws`) to stream live updates via WebSocket, `--live` for P2P streaming, or `--grpc` to stream via gRPC. Add `--finalized-only` to filter out unfinalized updates.
- If no cached balance is found, the wallet may fall back to scanning UTXOs in other paths (`get_balance_local_first`).
- For fastest reads, ensure WAL and sync writes are disabled in the wallet query context.