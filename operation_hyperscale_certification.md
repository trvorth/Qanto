# OPERATION HYPERSCALE: CERTIFICATION REPORT [FAILED]
**Date:** 2026-01-05  
**Executor:** System Architect (AI)  
**Target:** AP Validator (`98.91.191.193`)

## 1. Executive Summary
**CERTIFICATION STATUS: NO-GO**

**CRITICAL BLOCKER:** Target infrastructure is unreachable from this environment. Remote `bench_core` execution and external BPS measurement cannot be performed, so required evidence cannot be captured.

**PERFORMANCE WARNING (LOCAL REFERENCE ONLY):** Local `bench_core` runs on macOS (arm64) do not clear the >1,000,000 TPS threshold, with best observed throughput at **852,268.79 TPS** under the tested parameters. These numbers are not a substitute for target-host validation.

## 2. Targets
- **Internal Throughput:** > 1,000,000 TPS (via `bench_core --json`)
- **Network Block Rate:** ~32.0 BPS (via `qanto_cli check`)

## 3. Infrastructure Audit (From This Environment)
Connectivity checks against the AP validator indicate the target is not reachable over common ports required for certification:

- **SSH (22):** TIMEOUT / unreachable
- **HTTP/RPC (8080):** TIMEOUT / unreachable
- **Alt API (8081):** TIMEOUT / unreachable
- **Consensus/RPC (8332):** TIMEOUT / unreachable
- **gRPC (50051):** TIMEOUT / unreachable

Evidence:
- `ssh -o ConnectTimeout=10 ... user@98.91.191.193` → connect timeout
- `curl http://98.91.191.193:8080/info` → connect timeout
- `nc -vz 98.91.191.193 <port>` → closed for 22/80/443/8080/8081/8332/50051 in this environment

Actionable operator checks:
- Validate AWS Security Group ingress rules for the validator (22, 8080, 8332, 50051 as applicable).
- Validate NACLs and route tables for the subnet.
- Validate host firewall (ufw/iptables) and that services are bound to `0.0.0.0` (not only `127.0.0.1`).
- Validate instance health checks and that the node process is running/listening.

## 4. Configuration Audit (Velocity)
Velocity expectations (as encoded in tooling):
- `target_block_time = 31`
- `mempool_batch_size = 50,000`
- `mempool_max_size_bytes = 8,589,934,592`

Result: **FAIL (not measurable)** — cannot fetch config remotely, and no `--config` file path is available locally for this target.

## 5. Performance Data (Local Baseline — Reference Only)
These results are local-only and are not evidence for the AP validator.

### 5.1 bench_core (Local)
Runs executed:

- Run A (1,000,000 tx):
  - `{"tx_count":1000000,"batch_size":50000,"duration_secs":2.600207,"tps":384584.76575134206,"config_path":null,"config_target_block_time_ms":null,"config_mempool_batch_size":null,"config_mempool_max_size_bytes":null,"velocity_expected":false,"velocity_pass":null}`
- Run B (200,000 tx):
  - `{"tx_count":200000,"batch_size":2000,"duration_secs":0.251595958,"tps":794925.3302392083,"config_path":null,"config_target_block_time_ms":null,"config_mempool_batch_size":null,"config_mempool_max_size_bytes":null,"velocity_expected":false,"velocity_pass":null}`
- Run C (100,000 tx) — best observed:
  - `{"tx_count":100000,"batch_size":1000,"duration_secs":0.117333875,"tps":852268.792793215,"config_path":null,"config_target_block_time_ms":null,"config_mempool_batch_size":null,"config_mempool_max_size_bytes":null,"velocity_expected":false,"velocity_pass":null}`
- Run D (50,000 tx):
  - `{"tx_count":50000,"batch_size":500,"duration_secs":0.065291958,"tps":765791.0948236535,"config_path":null,"config_target_block_time_ms":null,"config_mempool_batch_size":null,"config_mempool_max_size_bytes":null,"velocity_expected":false,"velocity_pass":null}`

Assessment vs target:
- Best observed: **852,268.79 TPS**
- Deficit vs 1,000,000 TPS: **-14.77%**

Interpretation constraints:
- Local results vary materially with tx_count/batch_size/shard_count/worker_count and host scheduling.
- Target certification must be performed on the AP validator host with `--expect-velocity` and `--json`.

### 5.2 qanto_cli (Local External Check)
Correct CLI syntax (node flag is top-level):
- `./target/release/qanto_cli --node http://98.91.191.193:8080 check --seconds 10`

Runs (3): **FAILED**
- Each run: connect timeout to `http://98.91.191.193:8080/info`

Average BPS: **N/A** (no successful samples)

## 6. Patch Summary (Build Enablement)
To unblock building `bench_core` without pulling in the TFHE dependency, TFHE was made optional and feature-gated:
- `tfhe` dependency marked `optional = true` and exposed behind a `tfhe` feature in [Cargo.toml](file:///Users/trevor/qanto/Cargo.toml#L37-L211).
- TFHE imports and TFHE-backed methods in [types.rs](file:///Users/trevor/qanto/src/types.rs#L1-L170) are gated behind `#[cfg(feature = "tfhe")]`.

This change addresses build pressure from TFHE; it does not claim any performance optimization in `bench_core`.

## 7. Go / No-Go
**NO-GO**

Rationale:
- Remote validator is unreachable; required remote JSON evidence and external BPS verification cannot be produced.
- Local `bench_core` reference runs do not clear 1,000,000 TPS under tested parameters.

## 8. Immediate Next Steps
1. Restore reachability to the validator from the certification runner environment (open required ports).
2. On the AP validator, run:
   - `cd /opt/qanto && ./target/release/bench_core --config config/validator-ap.toml --expect-velocity --json`
3. From the certification runner, run (3 samples):
   - `for i in 1 2 3; do ./target/release/qanto_cli --node http://98.91.191.193:8080 check --seconds 10; done`
