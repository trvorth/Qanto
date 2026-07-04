# QANTO Remediation And Compliance Plan

Date: 2026-06-30
Related report: `Docs/plans/2026-06-30-codebase-audit-report.md`

## Objective

Move QANTO from the current audited state to a production-candidate state through a phased remediation sequence that addresses correctness, deployment reliability, interoperability, and cost transparency.

## Phase 0: Freeze And Baseline

Goal: establish one auditable baseline before additional feature work.

Actions:

1. Freeze public-facing protocol changes until the critical audit items are triaged.
2. Tag the current audited baseline and capture:
   - `cargo metadata`
   - build logs for Linux x86_64, Linux ARM64, and Apple Silicon
   - config snapshots for Docker, Helm, and raw Kubernetes
3. Publish a single environment matrix:
   - local developer
   - CI
   - private staging
   - public testnet
   - production

Exit criteria:

- Approved baseline tag exists.
- One source-of-truth environment matrix is published.

## Phase 1: Correctness And Serialization

Goal: eliminate protocol-level correctness risks.

Priority work:

1. Replace `u64` monetary fields in `qanto-rpc` with safe external encodings.
2. Add serialization boundary tests for:
   - `u64::MAX`
   - `MAX_TOTAL_SUPPLY`
   - large UTXO aggregation
   - high-fee and cross-chain transfer scenarios
3. Review `qanto-rpc`, `qanto-node`, `qantowallet`, and relayer code for implicit narrowing casts.

Required tests:

- unit tests for protobuf conversion
- compatibility tests for old/new RPC clients
- fuzz/property tests around balance conversion

Exit criteria:

- No unsafe monetary downcasts remain in public RPC payloads.
- Boundary tests pass across all supported targets.

## Phase 2: Configuration Hardening

Goal: remove silent config drift and standardize deployment pathways.

Priority work:

1. Add strict config validation to the runtime:
   - reject unknown fields in production mode
   - explicitly map compatibility aliases where needed
2. Publish one canonical config schema and one canonical example per environment tier.
3. Add automated validation for:
   - root `config.toml`
   - Docker config
   - Helm config
   - raw Kubernetes config

Required tests:

- parse tests for valid configs
- failure tests for unknown keys
- snapshot tests for generated Helm/K8s config output

Exit criteria:

- Unknown or mistyped config keys fail fast.
- All shipped config templates pass automated validation.

## Phase 3: Build And Supply Chain Reproducibility

Goal: make validator/operator builds deterministic.

Priority work:

1. Document native prerequisites for:
   - `cmake`
   - OpenSSL
   - RocksDB toolchain
   - protobuf compiler or vendored equivalent
2. Add CI jobs for:
   - macOS Apple Silicon
   - Ubuntu x86_64
   - Ubuntu ARM64
3. Decide whether PQ-native dependencies are:
   - always-on for all builds
   - optional behind a feature gate for dev/test

Required tests:

- `cargo check --workspace`
- `cargo test --workspace`
- release build smoke tests per target OS/arch

Exit criteria:

- Clean-room builds pass on all target platforms.
- Operator docs match the actual dependency chain.

## Phase 4: Deployment Qualification

Goal: prove that the delivery pipeline matches the runtime.

Priority work:

1. Validate the remediated Docker image by running:
   - compose build
   - wallet generation
   - node startup
   - health and metrics checks
2. Render Helm templates and validate:
   - ports
   - probes
   - volume mounts
   - secrets wiring
3. Validate raw Kubernetes manifests in a staging cluster.

Required tests:

- `docker compose config`
- container startup smoke test
- Helm render and lint
- Kubernetes dry-run apply

Exit criteria:

- Container starts cleanly with encrypted wallet flow.
- Helm and raw K8s produce working pods in staging.

## Phase 5: End-To-End Functional Validation

Goal: validate the mandatory QANTO operating pathways.

### Functional test tracks

1. Core node lifecycle
   - node boot
   - wallet load
   - P2P join
   - block production
   - state persistence

2. Interoperability pathways
   - gRPC RPC
   - HTTP API
   - JSON-RPC routes
   - relayer path
   - balance streaming / wallet queries

3. Governance and telemetry
   - SAGA metrics publication
   - telemetry ingestion
   - alert open/resolve lifecycle
   - dashboard rendering

4. Resilience
   - restart recovery
   - snapshot/restore
   - peer churn
   - degraded dependency behavior

### Performance test tracks

1. Throughput tests
   - single-node baseline
   - multi-node sustained load
   - mempool saturation behavior

2. Latency tests
   - API p50/p95/p99
   - block production interval
   - finality approximation

3. Resource tests
   - CPU and memory ceiling behavior
   - RocksDB growth and compaction behavior
   - FD and connection saturation

### Cost-model validation

1. Measure per-environment costs for:
   - compute
   - storage
   - bandwidth
   - observability
   - backup/snapshot retention
2. Produce monthly and annual envelopes for:
   - local/staging
   - single-node private testnet
   - 5-validator public testnet
   - production baseline

Exit criteria:

- All core pathways have automated or scripted validation.
- Cost envelopes are backed by measured resource consumption, not aspirational estimates.

## Standardized Cost Framework

The repository should stop using one-off provider narratives and instead adopt these environment tiers:

### Tier A: Local Developer

Purpose: feature work, fast feedback

Expected monthly cost:

- direct infra spend: $0
- developer workstation cost: excluded from protocol OPEX

### Tier B: Private Staging

Purpose: CI-adjacent integration and smoke validation

Expected monthly cost:

- 1 small VM + storage + observability: approximately $10-$40

### Tier C: Public Testnet

Purpose: external operator validation and telemetry-backed launch rehearsal

Expected monthly cost:

- 3-5 validators
- persistent storage
- centralized telemetry/observability
- snapshots/backups
- expected envelope: approximately $150-$500, depending on provider mix and data retention

### Tier D: Production Candidate

Purpose: launch-ready pre-testnet or controlled public testnet

Expected monthly cost:

- geographically distributed validators
- redundant observability
- backup/snapshot retention
- incident-response headroom
- expected envelope: approximately $500+ and should be modeled from measured staging usage

## Compliance Verification Checklist

The system can only be marked compliant after all items below are complete:

1. Technical correctness
   - RPC monetary model safe
   - config schema enforced
   - deployment surfaces aligned

2. Operational readiness
   - health, metrics, and logs available
   - restore path tested
   - upgrade drill tested

3. Testing completeness
   - unit, integration, and soak suites passing
   - performance SLOs baselined
   - interoperability paths validated

4. Cost transparency
   - environment tiers approved
   - provider choices approved
   - monthly/annual cost envelopes documented
   - ownership assigned for budget tracking

5. Documentation integrity
   - one canonical operator guide
   - one canonical deployment guide
   - one canonical RPC reference
   - superseded docs archived or marked obsolete

## What Was Completed In This Pass

1. Repaired the Docker delivery assets so they match the current workspace structure.
2. Added a compose-ready config template aligned to the current Rust config schema.
3. Fixed the Prometheus scrape path for the node's HTTP metrics endpoint.
4. Reworked Helm and raw Kubernetes config/manifests to match the live runtime contract more closely.
5. Produced a formal audit report and this phased remediation/compliance plan.

## Remaining Blockers

These items remain open and are the true production blockers:

1. RPC `u64` monetary truncation
2. strict config-schema enforcement
3. clean-room build reproducibility across target platforms
4. full end-to-end load/interoperability test execution
5. approved infrastructure tier and cost model

## Limitations

1. This pass did not rewrite the core monetary RPC schema because that change requires coordinated client/server versioning.
2. This pass did not execute a full staged cluster deployment or public testnet load campaign.
3. The repository worktree already contained extensive unrelated edits, so remediation was intentionally focused on low-collision DevOps surfaces and audit artifacts.
