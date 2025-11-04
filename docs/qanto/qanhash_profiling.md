# Qanhash Continuous Profiling

Qanto tracks mining attempts and hash rate in real time.

- Counters: `mining_attempts`, `last_hash_attempts`, `hash_rate_last_sample_ms`.
- Hash rate sampling: Miner samples every ~200ms, computing H/s from attempts delta.
- Telemetry: `telemetry.rs` logs hash rate, total attempts, and mining attempts.

## Viewing Metrics

- Use `metrics::QantoMetrics::format_hash_rate(...)` to format H/s.
- Log output includes current hash rate and attempt counters.
- Export text metrics via the metrics subsystem for dashboards.

## Suggested `config.toml`

```
[logging]
level = "info"
celebration_log_level = "error"
celebration_throttle_per_min = 1

[p2p]
heartbeat_interval = 100
```

## Tips

- Keep sampling non-blocking; use relaxed atomics.
- Avoid double-counting attempts; increment only inside nonce-try paths.
- Use `tracing` spans for mining functions to attribute latencies.