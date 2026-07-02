# QANTO Codebase Audit Report

> Archival note: this document records the pre-remediation audit state as of 2026-06-30 and intentionally preserves historical misalignment examples. Do not use it as the current runtime port or deployment reference.

Date: 2026-06-30
Scope: `/Volumes/trvorth/qanto`
Audit mode: end-to-end repository inspection, targeted build verification, deployment/config review, documentation/cost-model review

## Executive Summary

Overall status: not production-ready.

The repository contains substantial capability, but the current system state is blocked by a combination of:

1. Core functional defects in the RPC serialization boundary.
2. Silent configuration drift between the Rust runtime schema and deployment manifests.
3. Broken Docker/Kubernetes delivery paths that were not aligned to the current workspace layout.
4. Contradictory infrastructure and cost-model documentation that prevents reliable operational planning.
5. Build reproducibility gaps caused by undocumented native toolchain requirements for post-quantum dependencies.

Immediate remediation was applied to the deployment layer in:

- `DevOps/docker/Dockerfile`
- `DevOps/docker/docker-compose.yml`
- `DevOps/docker/config/config.toml`
- `DevOps/docker/README.md`
- `DevOps/docker/.env.example`
- `DevOps/docker/monitoring/prometheus/prometheus.yml`
- `DevOps/helm/qanto-node/templates/configmap.yaml`
- `DevOps/helm/qanto-node/templates/deployment.yaml`
- `DevOps/helm/qanto-node/templates/service.yaml`
- `DevOps/helm/qanto-node/values.yaml`
- `DevOps/k8s/configmap.yaml`
- `DevOps/k8s/statefulset.yaml`

These changes remove several verified delivery blockers, but they do not by themselves make the core protocol production-ready.

## Methodology

The audit used four evidence streams:

1. Repository inventory across `Protocol_Core`, `DevOps`, `Scripts`, `Frontend`, and `Docs`.
2. Static code inspection of node config loading, RPC serialization, telemetry, wallet boot, and deployment manifests.
3. Targeted Cargo verification:
   - `cargo metadata --format-version 1 --no-deps`
   - `cargo check -p qanto --all-targets`
   - `cargo check -p qanto-rpc`
   - `cargo check -p qanto-zk-sdk`
   - `cargo check -p qanto-relayer`
4. Cross-check of operational docs, launch plans, and cost/infrastructure assumptions.

## Verified Findings

### F1. RPC numeric model is unsafe for canonical QANTO supply

Severity: critical
Category: functional correctness / interoperability / wallet safety

Root cause:

- The protocol kernel uses `u128` for balances and supply.
- The RPC protobuf schema and server serialization use `uint64` for balances, outputs, rewards, and transaction amounts.
- QANTO's declared supply ceiling exceeds `u64::MAX` once represented in 9-decimal base units.

Evidence:

- `Protocol_Core/qanto-node/src/lib.rs`
  - `Q_SCALE = 1_000_000_000`
  - `MAX_TOTAL_SUPPLY = 21_000_000_000 * Q_SCALE`
- `Protocol_Core/qanto-rpc/proto/qanto.proto`
  - `BalanceResponse.balance`, `GetBalanceResponse.base_units`, `Transaction.amount`, `Output.amount`, `QantoBlock.reward`, `UTXO.amount` are all `uint64`
- `Protocol_Core/qanto-rpc/src/server.rs`
  - `base_units: total as u64`
  - `balance: confirmed as u64`

Impact:

- Large balances can truncate at the RPC boundary.
- Wallet and relayer clients can observe corrupted values.
- Cross-service interoperability is unsafe for full-supply or whale-account scenarios.

Required remediation:

- Move all externally serialized monetary fields to string-encoded decimal/base-unit values or split high/low limbs.
- Version the RPC API before rollout.
- Add boundary tests around `u64::MAX`, `MAX_TOTAL_SUPPLY`, and aggregated UTXO balances.

### F2. Build reproducibility is blocked by undocumented native PQ build dependencies

Severity: high
Category: build / onboarding / CI parity

Root cause:

- `qanto-core` depends on `oqs`, which pulls `oqs-sys` and a vendored `liboqs` build.
- The local build failed because `cmake` is required but not documented in the main build path.

Evidence:

- `cargo check -p qanto --all-targets` fails in `oqs-sys`
- Error path: `failed to execute command ... is cmake not installed?`
- `Docs/node-operator-guide.md` and `Docs/guide/testnet-guide.md` mention OpenSSL, RocksDB, and build-essential, but not `cmake` or `liboqs` build prerequisites.

Impact:

- Clean macOS/Linux operator environments cannot reliably build the node.
- "100% validator build success" cannot be achieved with the current prerequisite documentation.

Required remediation:

- Document `cmake` explicitly for all supported OS targets.
- Add a reproducible dependency bootstrap step for PQ-native builds.
- Consider feature-gating OQS for environments that do not need full PQ stacks during local development.

### F3. Config loader accepts silent schema drift

Severity: high
Category: pathway configuration / operability

Root cause:

- `Config` is deserialized with `toml::from_str` without `deny_unknown_fields`.
- The checked-in configs contain unsupported top-level and nested sections that are silently ignored by the runtime.

Evidence:

- `Protocol_Core/qanto-node/src/config.rs`
  - `toml::from_str(&content)` with no strict unknown-field enforcement
- `config.toml` and `Protocol_Core/qanto-node/config.toml` contain unsupported keys/sections such as:
  - top-level `p2p_port`
  - top-level `rpc_port`
  - `[network]`
  - `[consensus]`
  - `[telemetry]`

Impact:

- Operators can believe configuration changes are active when they are not.
- Pathway mapping for ports, bootnodes, and telemetry becomes non-deterministic.
- Production incidents can be caused by "valid-looking but ignored" TOML.

Required remediation:

- Introduce strict config parsing or explicit compatibility adapters.
- Publish one canonical schema and reject unknown fields in production mode.
- Add config round-trip tests for all shipped templates.

### F4. Docker delivery path was broken against the current workspace layout

Severity: critical
Category: deployment / release engineering

Status: remediated in this pass

Root cause:

- Compose referenced `deployment/docker/Dockerfile`, but the repository uses `DevOps/docker/Dockerfile`.
- The Dockerfile assumed a flat root layout with `src/` and `myblockchain/`, which does not match the current workspace under `Protocol_Core/`.
- The entrypoint attempted `generate_wallet --stdin-password`, but that flag does not exist.
- The entrypoint relied on an interactive password prompt model while `qanto start` uses `WALLET_PASSWORD` directly.

Evidence:

- `DevOps/docker/docker-compose.yml`
  - previous `dockerfile: deployment/docker/Dockerfile`
- `DevOps/docker/Dockerfile`
  - previous `COPY src ./src`
  - previous `COPY myblockchain ./myblockchain`
- `Protocol_Core/qanto-node/src/generate_wallet.rs`
  - no `--stdin-password` support
- `Protocol_Core/qanto-node/src/qanto.rs`
  - startup path reads `WALLET_PASSWORD`

Remediation applied:

- Updated Docker build path to `DevOps/docker/Dockerfile`
- Reworked the Dockerfile to build from the actual workspace
- Added `DevOps/docker/config/config.toml`
- Removed the invalid expect-based startup path
- Standardized wallet bootstrap to `WALLET_PASSWORD` / `WALLET_PASSWORD_FILE`
- Fixed Prometheus scrape target to the node's actual HTTP metrics route

Residual risk:

- The image still depends on core protocol code that has not passed a full clean `cargo test --workspace --all-features`.

### F5. Helm chart config did not match the live Rust config schema

Severity: critical
Category: deployment / pathway mapping

Status: remediated in this pass

Root cause:

- The Helm `configmap.yaml` emitted keys like `listen_addresses`, `bind_address`, `[websocket]`, `[metrics]`, and `[storage]` that do not map to the current `Config` struct.
- The readiness probe targeted `/ready`, but the node exposes `/health`.
- Mount paths used `/app/...` while the container runtime uses `/opt/qanto/...`.

Impact:

- Pods could start with silently ignored config.
- Readiness would fail even if the node was otherwise healthy.
- Volume and key paths were inconsistent with the image runtime.

Remediation applied:

- Replaced the Helm config with a `Config`-compatible TOML shape
- Added explicit `CONFIG_PATH`, `WALLET_PATH`, `P2P_IDENTITY`, `PEER_CACHE`, and `WALLET_PASSWORD_FILE`
- Aligned mount paths to `/opt/qanto/...`
- Changed readiness to `/health`
- Simplified service ports to `api`, `rpc`, and `p2p`

### F6. Raw Kubernetes manifests were internally inconsistent and typed incorrectly

Severity: high
Category: deployment / operability

Status: remediated in this pass

Root cause:

- `DevOps/k8s/configmap.yaml` used decimal fractions like `0.8`, `0.7`, and `0.10` where the Rust config expects scaled integer values.
- The stateful set exposed `8333` and `8080` while the current config/runtime model centered on `30303`, `8081`, and `50051`.
- The manifest used a `/ready` endpoint that does not exist.

Impact:

- Config parsing risk and silent behavioral drift.
- Broken probes and mismatched network exposure.
- Key and log persistence paths were fragmented.

Remediation applied:

- Replaced float thresholds with scaled integers
- Aligned ports and probes
- Consolidated keys and logs under the persistent data claim

### F7. Infrastructure and cost documents are contradictory

Severity: high
Category: operational planning / cost transparency

Root cause:

The repository currently contains mutually incompatible operating assumptions:

- `FREE_TESTNET_ARCHITECTURE.md` specifies a `$0/month` model using Hugging Face, Neon, R2, and UptimeRobot.
- `FREE_TESTNET_READINESS_AUDIT.md` concludes that the `$0/month` architecture is structurally unviable.
- `Docs/plans/deployment-plan.md` specifies a Namecheap VPS deployment.
- `Docs/plans/cohort-a-operational-launch-plan.md` assumes five validators across Oracle Cloud, AWS, Hetzner, residential infrastructure, and a Mac Mini.

Impact:

- Operations teams have no single source of truth for cost envelopes.
- Capacity planning and launch governance are ambiguous.
- Compliance verification cannot be signed off while the target deployment model is unsettled.

Required remediation:

- Ratify a single target environment per stage: local dev, private staging, public testnet, production.
- Bind each stage to a cost envelope, SLO target, and approved provider set.
- Remove or archive superseded architecture docs after approval.

### F8. Main operator documentation references missing assets and outdated entrypoints

Severity: medium
Category: documentation / onboarding

Root cause:

- `Docs/node-operator-guide.md` and other docs reference `config.toml.example`, which is not present.
- Main README and RPC docs reference public endpoints, port models, and token assumptions that do not fully align with the current runtime and launch-readiness documents.

Impact:

- New operators cannot follow the documented happy path reliably.
- External integrators may consume incorrect RPC assumptions.

Required remediation:

- Publish one supported example config per delivery surface.
- Review README, RPC docs, and operator docs against the live runtime after the core protocol freezes.

## Pathway Mapping Gaps

The most important unresolved pathway ambiguities are:

1. Wallet boot path:
   - CLI startup, Docker, Helm, and raw Kubernetes previously used different credential flow assumptions.
2. RPC path:
   - gRPC server, HTTP API, JSON-RPC route, and metrics route are distributed across different ports and docs.
3. P2P path:
   - Documentation cites `8000`, `30303`, `30333`, `8333`, and `7860/WSS` across different artifacts.
4. Config source-of-truth:
   - Runtime config schema and deployment-generated TOML were not aligned.
5. Cost source-of-truth:
   - The repo simultaneously describes free-tier, single-VPS, and multi-validator cloud/bare-metal topologies.

## Testing Gaps

The requested "100% functionality validation" is not currently achievable from the checked-in repository state because:

1. Core node builds are blocked on undocumented native dependencies in a clean environment.
2. The RPC numeric boundary is unsafe for canonical monetary ranges.
3. Deployment surfaces were only partially aligned during this pass and still require runtime verification.
4. No unified end-to-end environment definition currently exists for load, interoperability, and cost predictability testing.

## Conclusion

The repository can move toward a production candidate, but only after:

1. Fixing the RPC monetary type boundary.
2. Enforcing a canonical config schema.
3. Ratifying one infrastructure model per environment tier.
4. Rebuilding the validation matrix around clean, reproducible environments.
