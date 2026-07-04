# Network Layer Optimization (zstd Compression)

The P2P stack enforces zstd-frame compression for blocks and transactions.

- `NetworkConfig.enable_compression = true` ensures compression for all payloads.
- `compression_level` tunes CPU vs bandwidth; start with `5–7` for balanced performance.
- `SetReconciliationGossip` compresses payloads if they exceed the `compression_threshold`.

## Suggested `config.toml`

```
[p2p]
# Always compress block/tx payloads
enable_compression = true
compression_level = 6
compression_threshold_bytes = 1024
# Gossip parameters tuned for 32 BPS
mesh_n_low = 8
mesh_n = 16
mesh_n_high = 32
mesh_outbound_min = 8
heartbeat_interval = 100
```

## Notes

- Compression reduces bandwidth significantly at 32+ BPS; adjust level based on CPU availability.
- Monitor `network_bytes_sent`/`received` metrics to validate compression efficacy.