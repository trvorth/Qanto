# High-Throughput Storage Engine (Sharding + Async I/O)

QantoStorage uses a sharded memtable and asynchronous I/O batching to support 10M+ TPS.

- Shards: Keys are mapped to shards using a hash; minimum `shard_count` is 16.
- Async I/O: Batched writes via an async processor to reduce syscalls and fsyncs.
- Lock-free cache: `DashMap` for concurrent read/write access.

## Suggested `config.toml`

```
[storage]
# Increase shard count for parallel writes
shard_count = 64
# Enable async batching for mining and transaction ingestion
async_batch_enabled = true
async_batch_max_items = 8192
async_batch_max_bytes = 4_000_000
# Tune WAL and sync writes based on durability requirements
wal_enabled = true
sync_writes = false
compression_enabled = true
```

## Operational Tips

- For peak throughput, set `sync_writes = false` during benchmarking; enable for production durability.
- Keep shard count a power of two to minimize modulo cost.
- Co-locate hot keys to distinct shards to avoid hotspots.
- Size batches based on disk latency and CPU utilization to target 32+ BPS.