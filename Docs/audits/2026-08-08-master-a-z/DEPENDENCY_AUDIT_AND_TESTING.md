# Qanto Dependency Audit and Testing Evidence

Audit date: 2026-08-08  
Audit target: effective working tree at `/Users/cps/qanto`  
Baseline commit: `0478b60368df86ece44b9ad30766a724dfc60d50`  
Scope: mandatory workstreams 1 and 11 only

## Evidence boundary

This report distinguishes source-confirmed facts from executed checks and from tooling that was not available. The repository was materially dirty before this audit; existing user work was preserved. Findings therefore describe the effective working tree, not only the baseline commit.

- `CONFIRMED`: established by source or manifest inspection.
- `MEASURED`: established by a command executed against this working tree.
- `CANDIDATE`: lexical evidence that still needs compiler-semantic or runtime reachability proof.
- `UNAVAILABLE`: the required tool was not installed in the audit environment.

The full generated qanto-node graph is in [MODULE_GRAPH.md](./MODULE_GRAPH.md), with machine-readable [module-graph.json](./module-graph.json) and Graphviz [module-graph.dot](./module-graph.dot).

## Executive conclusion

- The Cargo workspace resolves all **8 declared members**. The Solidity workspace contains all **10 mandated contracts**.
- The final no-retry Rust run is green: **207/207 executed tests passed across 31 binaries; 1 additional test was skipped**. This is a test-run result, not evidence that the protocol is launch-safe.
- Hardhat compiles all 10 contracts and passes **38/38 tests**, but measured coverage is only **42.45% lines / 43.36% statements** and the test source exercises only QantoFaucet and QUSD.
- `cargo audit` reports no known vulnerability after dependency updates, but strict warning mode fails on five warnings. `cargo deny check advisories` remains red on one unsound dependency and three unmaintained dependencies; its stricter result controls the verdict.
- `cargo deny check licenses` fails because four first-party crates have no license expression.
- The ordinary workspace build passes, while the all-target build fails with five errors in the stale mining benchmark.
- The module graph resolves all 112 qanto-node module declarations, but identifies 38 lexical isolation candidates, 33 modules containing stub/mock markers, and two large strongly connected components.
- Known consensus-critical coverage holes remain: zero tests in `my-blockchain`, no CPU/CUDA/Metal/OpenCL parity vectors, no two-node branch-arrival/reorg state proof, no concurrent UTXO double-spend property test, and no real libp2p reconnection-under-partition harness.

# Workstream 1 — Codebase Cartography and Dependency Audit

## 1.A Complete workspace inventories

### Rust virtual workspace

The authoritative member list is `Cargo.toml:9-20`. `cargo metadata --no-deps --format-version 1 --locked` resolved every member.

| # | Workspace path | Package | Primary responsibility | Resolution |
|---:|---|---|---|---|
| 1 | `Protocol_Core/qanto-node` | `qanto 0.1.0` | Main node library, node/wallet/utility binaries, integration tests, mining benchmark | RESOLVED |
| 2 | `Protocol_Core/qanto-core` | `qanto-core 0.1.0` | Native crypto, storage, serialization, network and P2P primitives | RESOLVED |
| 3 | `Protocol_Core/qanto-rpc` | `qanto-rpc 0.1.0` | Protobuf-generated gRPC/RPC server library and build target | RESOLVED |
| 4 | `Protocol_Core/myblockchain` | `my-blockchain 0.1.0` | QanHash CPU/GPU mining library and benchmark | RESOLVED |
| 5 | `Protocol_Core/qanto-zk-sdk` | `qanto-zk-sdk 0.1.0` | Bridge proof verification SDK | RESOLVED |
| 6 | `Protocol_Core/qanto-relayer` | `qanto-relayer 0.1.0` | Cross-chain bridge daemon library and binary | RESOLVED |
| 7 | `Scripts/tools/relayer` | `qanto-legacy-relayer 0.1.0` | Legacy relayer binary | RESOLVED |
| 8 | `qanto-desktop-wallet/src-tauri` | `qanto-desktop-wallet 0.1.0` | Tauri wallet library, binary and build target | RESOLVED |

### Hardhat/Solidity workspace

All mandated files exist under `SmartContracts/contracts/`:

| # | Contract | File | Present |
|---:|---|---|---|
| 1 | WQNTO | `SmartContracts/contracts/WQNTO.sol` | YES |
| 2 | QUSD | `SmartContracts/contracts/QUSD.sol` | YES |
| 3 | QantoGovernor | `SmartContracts/contracts/QantoGovernor.sol` | YES |
| 4 | QantoTGE | `SmartContracts/contracts/QantoTGE.sol` | YES |
| 5 | QantoTimelock | `SmartContracts/contracts/QantoTimelock.sol` | YES |
| 6 | QantoFaucet | `SmartContracts/contracts/QantoFaucet.sol` | YES |
| 7 | QantoDrop | `SmartContracts/contracts/QantoDrop.sol` | YES |
| 8 | QantoSwapFactory | `SmartContracts/contracts/QantoSwapFactory.sol` | YES |
| 9 | QantoSwapRouter | `SmartContracts/contracts/QantoSwapRouter.sol` | YES |
| 10 | SentinelNFT | `SmartContracts/contracts/SentinelNFT.sol` | YES |

`QantoTimelock.sol:6-12` is a thin `TimelockController` constructor wrapper; it has no separately instrumented executable statements in the current coverage output.

## 1.B `[patch.crates-io]` necessity audit

The current manifest retains three overrides at `Cargo.toml:36-42`. Three unnecessary overrides were removed during this pass.

| Override present at baseline | Effective status | Evidence and necessity | CVE / compatibility reason | Upstream disposition |
|---|---|---|---|---|
| `derivative` git `master` | **REMOVED** | The lock now uses registry `derivative 2.2.0` (`Cargo.lock:2286-2294`). The git fork did not eliminate the crate's unmaintained status and was not required for compilation. | No vulnerability fix; RUSTSEC-2024-0388 is an unmaintained warning. | Removal completed. Ultimately remove/upgrade the Arkworks dependency that brings it in. |
| `tracing-subscriber` git tag `0.3.20` | **TOP-LEVEL OVERRIDE REMOVED; ONE GIT COPY REMAINS** | Workspace consumers now use registry 0.3.23 (`Cargo.toml:23-25`; `Cargo.lock:9980-9995`). The local `ark-relations` manifest still pins its compatibility stack to the tracing git tag (`patches/ark-relations/Cargo.toml:56-74`; `Cargo.lock:9966-9977`). | Compatibility, not a CVE fix. | Remove the remaining git stack when the Arkworks patch is upstreamed or Arkworks is upgraded. |
| `ark-relations` local patch | **KEEP PROVISIONALLY** | The source adapts Arkworks 0.5 trace APIs to tracing-subscriber 0.3 (`patches/ark-relations/src/r1cs/trace.rs:8-10`, `:43-63`) and its manifest records the tracing stack (`patches/ark-relations/Cargo.toml:56-74`). | API compatibility. No CVE-specific justification was found. | Upstream the small trace adaptation or migrate to a compatible maintained Arkworks release; then remove the patch. |
| `libp2p-dns` local patch | **KEEP PROVISIONALLY** | The patch raises hickory-resolver from the upstream crate's 0.25.2 requirement to 0.26.1 and adapts resolver APIs (`patches/libp2p-dns/Cargo.toml:54-57`; `patches/libp2p-dns/src/lib.rs:59-115`). | hickory 0.26 compatibility. No CVE-specific justification was found. | Submit upstream or remove after libp2p publishes native hickory 0.26 support. |
| `libp2p-mdns` local patch | **KEEP PROVISIONALLY** | The only material package delta is the hickory-proto 0.26.1 requirement (`patches/libp2p-mdns/Cargo.toml:62-65`). | hickory 0.26 compatibility. No CVE-specific justification was found. | Upstream the version support or remove after a compatible libp2p release. |
| `plist` local patch | **REMOVED FROM RESOLUTION** | The old local directory identifies itself as 1.9.0, while the lock now resolves registry 1.10.0 (`Cargo.lock:6614-6624`) with the required quick-xml dependency. | Compatibility pin was obsolete; no CVE reason remained. | Manifest removal completed. The unused `patches/plist/` directory can be deleted in a reviewed cleanup. |

Supply-chain note: `cargo deny check sources` passes under the current policy, which permits crates.io and the pinned tracing git repository (`deny.toml:53-57`). Passing this policy does not make the remaining local forks maintenance-free.

## 1.C Dependency and policy check results

### Executed matrix

| Check | Current result | Interpretation |
|---|---|---|
| `cargo audit --no-fetch` | **PASS**, exit 0; 1,033 locked dependencies scanned against 1,190 advisories | No non-ignored known vulnerability. Three unmaintained warnings are allowed by default policy. |
| `cargo audit --no-fetch --deny warnings` | **FAIL**, exit 1; five denied warnings | Three unmaintained crates plus two yanked releases make a strict release gate red. |
| `cargo deny check advisories` | **FAIL**, exit 1 | Four errors and two yanked warnings; detailed below. |
| `cargo deny check bans` | **PASS**, exit 0 | Wildcard dependency errors have been removed; policy still reports 59 duplicate-version warnings. |
| `cargo deny check sources` | **PASS**, exit 0 | All sources conform to `deny.toml:53-57`. |
| `cargo deny check licenses` | **FAIL**, exit 4 | Four first-party packages have no license expression. |
| `cargo check --workspace --locked` | **PASS**, exit 0 | Default workspace targets compile. |
| `cargo check --workspace --all-targets --locked` | **FAIL**, exit 101 | Five stale-API errors in `Protocol_Core/qanto-node/benches/mining_bench.rs:20-27`, `:89-111`, `:293`. |

### Advisory inventory and launch classification

This table is the union of the current `cargo audit` and stricter `cargo deny` results. `BLOCK/WARN/INFO` is the workstream-1 advisory classification required by the mandate.

| Advisory / condition | Locked crate | Tool | Classification | Why |
|---|---|---|---|---|
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429) unsound `VariantStrIter` | `glib 0.18.5` (`Cargo.lock:3339-3350`) | cargo-deny | **BLOCK** for Linux desktop release | Known undefined behavior/crash path; upgrade to `glib >=0.20` through the Tauri/GTK dependency chain. |
| [RUSTSEC-2024-0380](https://rustsec.org/advisories/RUSTSEC-2024-0380) replaced by ML-DSA crate | `pqcrypto-dilithium 0.5.0` (`Cargo.lock:6757-6767`) | both | **BLOCK** for mainnet cryptographic maintenance | Directly present in wallet/core dependency paths; replacement is explicitly available. |
| [RUSTSEC-2026-0162](https://rustsec.org/advisories/RUSTSEC-2026-0162) PQClean ecosystem archived | `pqcrypto-traits 0.3.5` (`Cargo.lock:6795-6798`) | cargo-deny | **BLOCK** for mainnet cryptographic maintenance | Direct/transitive wallet dependency with no future upstream security maintenance. |
| [RUSTSEC-2026-0160](https://rustsec.org/advisories/RUSTSEC-2026-0160) SPHINCS+ wrapper unmaintained | `pqcrypto-sphincsplus 0.7.2` (`Cargo.lock:6782-6792`) | cargo-audit | **BLOCK** for mainnet cryptographic maintenance | Used through qanto-core; the upstream PQClean archive removes the expected patch channel. |
| [RUSTSEC-2025-0141](https://rustsec.org/advisories/RUSTSEC-2025-0141) bincode unmaintained | `bincode 1.3.3` (`Cargo.lock:1044-1050`) | cargo-deny | **WARN** | No safe in-place upgrade; a serialization-format migration needs state/wire versioning. |
| [RUSTSEC-2024-0388](https://rustsec.org/advisories/RUSTSEC-2024-0388) derivative unmaintained | `derivative 2.2.0` (`Cargo.lock:2286-2294`) | cargo-audit | **WARN** | Transitive via Arkworks; replace during Arkworks upgrade. |
| Yanked release | `num-bigint 0.4.7` (`Cargo.lock:5848-5855`) | both strict checks | **WARN** | Move to a non-yanked compatible release after Arkworks/ZK compatibility validation. |
| Yanked release | `spin 0.9.8` (`Cargo.lock:8759-8762`) | both strict checks | **WARN** | Transitive dependency; update upstream consumers and prove platform compatibility. |

The ordinary `cargo audit` pass is not the controlling launch gate because `.cargo/audit.toml:21-50` explicitly ignores broad GTK, PQClean, bincode, and other advisories. `deny.toml:16-19` is intentionally stricter for unmaintained workspace dependencies and all unsound advisories.

### Fixed dependency findings

The baseline `crossbeam-epoch 0.9.18` vulnerability (RUSTSEC-2026-0204) is removed; the lock now uses 0.9.20 (`Cargo.lock:1929-1935`). The previously flagged `event-listener` and `lru` versions were also moved to 5.4.2 and 0.16.4 (`Cargo.lock:2807-2814`, `:5204-5210`). These changes are why the current audit has zero non-ignored known vulnerabilities.

### License gate

`cargo deny check licenses` fails on exactly these four first-party crates:

- `Protocol_Core/qanto-rpc/Cargo.toml:1-3`
- `Protocol_Core/qanto-zk-sdk/Cargo.toml:1-3`
- `Protocol_Core/qanto-relayer/Cargo.toml:1-3`
- `qanto-desktop-wallet/src-tauri/Cargo.toml:1-3`

No license was invented during this pass. Repository-level licensing intent is legally inconsistent across available materials, so package metadata requires an explicit owner/legal decision.

## 1.D qanto-node module graph, dead-code candidates, stubs and cycles

### Graph result

| Metric | Result |
|---|---:|
| Public module declarations resolved | 112 |
| Missing module sources | 0 |
| Lexically isolated modules | 38 |
| Modules containing stub/mock markers | 33 |
| Strongly connected components with more than one module | 2 |

The generator performs lexical top-level `crate::module` / `use crate::{...}` analysis. It does not claim compiler-semantic reachability and does not inspect external consumers of public APIs. Accordingly, the 38 entries below are **dead-code candidates**, not proven dead code:

`config_validator`, `formal_verifier`, `hame`, `heritage_archive`, `holo_mesh`, `kernel_boot`, `keygen`, `pipeline`, `pq_crypto`, `storage_mesh`, `a2a_material`, `absolute_singularity`, `config_validation`, `cosmic_consensus`, `entropy_shield`, `eternal_existence`, `final_transcendence`, `futarchy_market`, `global_synthesis`, `inference_mesh`, `infinite_omnipresence`, `mesh_finality`, `mesh_optimizer`, `mesh_recovery`, `neural_arbitrage`, `neural_consensus`, `neural_link`, `neural_mirror`, `quantum_bridge`, `self_sovereign_code`, `sharding`, `swarm_orchestrator`, `truth_engine`, `ubi_credits`, `zk`, `derive_genesis_key`, `zk_sequencer`, `corruption_tests`.

`corruption_tests` is intentionally test-only (`Protocol_Core/qanto-node/src/lib.rs:131-140`). Several other entries are public modules and may be called by binaries or downstream crates, so removal requires compiler-driven reachability and API review.

### Stub-marker candidates

The exact lexical marker set is:

`adaptive_mining(3)`, `analytics_dashboard(1)`, `decentralization(1)`, `formal_verifier(2)`, `graphql_api(1)`, `hame(1)`, `interoperability(5)`, `kernel_boot(2)`, `metrics(5)`, `node(1)`, `omega(11)`, `omega_enhanced(23)`, `performance_validation(4)`, `pq_crypto(1)`, `privacy(2)`, `a2a_material(1)`, `neural_link(1)`, `derive_genesis_key(1)`, `generate_wallet(1)`, `get_address(1)`, `import_wallet(1)`, `metrics_server(1)`, `mock_traits(10)`, `monitor(2)`, `qanto(1)`, `qanto_ai_metrics(1)`, `qantodag(2)`, `x_phyrus(1)`, `infinite_strata_node(1)`, `zkp(6)`, `qantowallet(1)`, `security_tests(1)`, `state_sync(1)`.

Markers are triage signals, not proof that an entire module is a stub. Manual inspection confirms at least:

- `formal_verifier` is synthetic rather than formal verification (`formal_verifier.rs:46-53`, `:57-78`, `:111-119`).
- `pq_crypto` exposes an intentionally unimplemented compatibility verifier, now fail-closed (`pq_crypto.rs:15-38`).
- `metrics_server` has a real server but only a placeholder test (`metrics_server.rs:69-110`).
- `mock_traits` is test/dummy infrastructure by design.

### Circular dependency candidates

The two lexical strongly connected components are:

1. `analytics_dashboard → consensus → decentralization → gas_fee_model → memory_optimization → mempool → miner → omega → performance_monitoring → performance_optimizations → qantodag → saga → saga_ai → telemetry → transaction → websocket_server`
2. `adaptive_mining → block_producer → decoupled_producer → graphql_api → graphql_server → node → p2p → rpc_backend`

Cargo accepts these because they are module-level reference cycles inside a crate, not crate dependency cycles. They still indicate high coupling, wide blast radius, and difficult isolated testing.

## 1.E Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| WS1-001 | SEV-2 | RustSec | `Cargo.lock:1929-1935`, `:2807-2814`, `:5204-5210` | Baseline vulnerable/unsound versions were upgraded; current cargo-audit has zero non-ignored known vulnerabilities. | **FIXED_IN_WORKTREE** |
| WS1-002 | SEV-2 | Desktop dependency | `Cargo.lock:3339-3350` | `glib 0.18.5` is affected by RUSTSEC-2024-0429 unsoundness. | OPEN — release blocker for affected desktop target |
| WS1-003 | SEV-2 | PQ dependency lifecycle | `Cargo.lock:6757-6798` | Dilithium, SPHINCS+ and shared PQClean wrapper dependencies are unmaintained/archived. | OPEN — mainnet maintenance blocker |
| WS1-004 | SEV-2 | License policy | `Protocol_Core/qanto-rpc/Cargo.toml:1-3`; `Protocol_Core/qanto-zk-sdk/Cargo.toml:1-3`; `Protocol_Core/qanto-relayer/Cargo.toml:1-3`; `qanto-desktop-wallet/src-tauri/Cargo.toml:1-3` | Four first-party packages are unlicensed according to cargo-deny. | DEFERRED — owner/legal decision required |
| WS1-005 | SEV-3 | Audit policy | `.cargo/audit.toml:21-50`; `deny.toml:16-19` | Default cargo-audit suppresses advisories that cargo-deny correctly rejects. A release gate that runs only cargo-audit can report a misleading green result. | OPEN — make deny authoritative in CI |
| WS1-006 | SEV-3 | Local forks | `Cargo.toml:36-42`; `patches/ark-relations/Cargo.toml:56-74`; `patches/libp2p-dns/Cargo.toml:54-57`; `patches/libp2p-mdns/Cargo.toml:62-65` | Three compatibility forks remain and need provenance, upstreaming and removal criteria. | PARTIAL — unnecessary overrides removed |
| WS1-007 | SEV-3 | All-target build | `Protocol_Core/qanto-node/benches/mining_bench.rs:20-27`, `:89-111`, `:293` | Mining benchmark imports undeclared/obsolete APIs; all-target compilation fails with five errors. | OPEN |
| WS1-008 | SEV-3 | Module architecture | `Protocol_Core/qanto-node/src/lib.rs:3-140`; `Docs/audits/2026-08-08-master-a-z/MODULE_GRAPH.md:1` | 38 isolation candidates, 33 stub-marker modules, and two large SCCs create significant reachability and coupling debt. | MAPPED; cleanup deferred |
| WS1-009 | INFO | Workspace inventory | `Cargo.toml:9-20`; `SmartContracts/contracts/` | All eight Rust members and all ten mandated Solidity contracts exist. | COMPLETE |

## 1.F Implemented fixes

The dependency-related changes already present in this audit worktree are represented by this exact manifest-level delta:

```diff
-tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }
+tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }

 [patch.crates-io]
-derivative = { git = "https://github.com/mcarton/rust-derivative", branch = "master" }
-tracing-subscriber = { git = "https://github.com/tokio-rs/tracing", tag = "tracing-subscriber-0.3.20" }
 ark-relations = { path = "patches/ark-relations" }
 libp2p-dns = { path = "patches/libp2p-dns" }
 libp2p-mdns = { path = "patches/libp2p-mdns" }
-plist = { path = "patches/plist" }
```

Lockfile remediation:

```diff
-crossbeam-epoch 0.9.18
+crossbeam-epoch 0.9.20
-event-listener 5.4.1
+event-listener 5.4.2
-lru 0.12.5
+lru 0.16.4
-plist 1.9.0 (local path)
+plist 1.10.0 (crates.io registry)
-derivative 2.2.0 (git master)
+derivative 2.2.0 (crates.io registry)
```

Additional controls:

- `deny.toml:1-57` adds explicit advisory, license, wildcard, duplicate and source policy.
- Version metadata was added to first-party path dependencies; `cargo deny check bans` now passes instead of failing wildcard dependency checks.
- Unnecessary derivative, top-level tracing-subscriber and plist overrides were removed while the three source-bearing compatibility patches were retained.

## 1.G Deferred items

| Priority | Item | Why deferred | Estimated effort |
|---|---|---|---:|
| P0 | Upgrade/isolate the Tauri GTK stack so `glib >=0.20` is resolved | Transitive desktop framework migration requires Linux build/runtime validation | 3-7 days |
| P0 | Migrate remaining `pqcrypto-*` dependencies to maintained ML-DSA/SLH-DSA implementations | Changes key/signature formats and requires KATs, interoperability vectors and migration policy | 2-6 weeks plus external crypto review |
| P0 | Assign license expressions to four first-party crates | Legal intent is inconsistent; engineering must not invent a license | 0.5-2 days after owner/legal decision |
| P1 | Replace bincode 1.3.3 and yanked num-bigint/spin paths | Serialization and ZK dependency migrations need versioned compatibility tests | 2-4 weeks |
| P1 | Upstream/remove the three local patches and remaining tracing git stack | Requires upstream PRs or coordinated Arkworks/libp2p upgrades | 1-3 weeks |
| P1 | Repair `mining_bench.rs` and enforce all-target compilation | Benchmark targets reference APIs not exported by the current crate | 1-3 days |
| P1 | Compiler-semantic reachability audit and module decomposition | Public APIs and binary-only callers prevent safe mechanical deletion | 1-2 weeks |

## 1.H Workstream verdict

**NOT READY.** Workstream 1 is blocked by the unsound desktop dependency, unmaintained direct cryptographic dependencies, unresolved first-party licensing, and a failing all-target build. The strict policy is doing its job; the graph is not release-clean yet.

# Workstream 11 — Testing and Formal Verification

## 11.A Test inventory

### Mandated qanto-node test modules

The five mandatory test modules are compiled under `cfg(test)` at `Protocol_Core/qanto-node/src/lib.rs:131-140`:

- `byzantine_tests.rs` — forged IDs/difficulty, invalid Merkle/reward/signature, equivocation and clock drift.
- `corruption_tests.rs` — WAL tail/mid-record corruption, segment corruption and crash consistency.
- `economic_attack_tests.rs` — bridge drain, delegation cartel, reward farming and stake concentration scenarios.
- `partition_tests.rs` — in-memory 60/40, 70/30, 34/33/33 and healing scenarios.
- `security_tests.rs` — parsing, serialization, bridge, staking, signature, genesis and supply assertions.

The partition tests instantiate separate in-process DAGs and manually copy blocks during healing (`partition_tests.rs:25-91`, `:327-358`). They do not start libp2p nodes, disconnect sockets, exercise reconnection timers, or validate peer-session behavior.

### Rust test targets discovered by nextest

`cargo nextest list --workspace --locked --message-format json` discovered 208 test cases; one is ignored, leaving 207 runnable cases.

| Package | Test cases listed | Ignored | Important sources/targets |
|---|---:|---:|---|
| `qanto` | 169 | 0 | 156 library tests plus `e2e_mining_reward` (9), `e2e_pqc_migration` (1), `fast_sync_tests` (1), `fuzz_pqc_math` (1), `interoperability_zk_tests` (1); `pqc_mempool_tests` lists 0 |
| `qanto-core` | 18 | 0 | balance stream, DAG mempool, mining formatting, native crypto, net display, storage recovery |
| `qanto-desktop-wallet` | 4 | 1 | key derivation tests; live airdrop/send/history E2E ignored |
| `qanto-legacy-relayer` | 4 | 0 | packet integrity/security/creation/processing |
| `qanto-relayer` | 5 | 0 | config, error and coordinator type tests |
| `qanto-rpc` | 3 | 0 | monetary string parsing and balance precision |
| `qanto-zk-sdk` | 5 | 0 | fail-closed bridge verifier and numeric parsing |
| `my-blockchain` | **0** | 0 | No executable QanHash unit, KAT or backend parity test |

Separate Rust assets:

- Integration targets: `Protocol_Core/qanto-node/tests/e2e_mining_reward.rs`, `e2e_pqc_migration.rs`, `fast_sync_tests.rs`, `fuzz_pqc_math.rs`, `interoperability_zk_tests.rs`, `pqc_mempool_tests.rs`.
- Benchmarks: `Protocol_Core/qanto-node/benches/mining_bench.rs`, `Protocol_Core/myblockchain/benches/main.rs`.
- Fuzz targets: `Protocol_Core/qanto-node/fuzz/fuzz_targets/fuzz_tx_parsing.rs` and `fuzz_bridge_proofs.rs`, with a checked-in transaction corpus.
- `Protocol_Core/qanto-node/tests/pqc_mempool_tests.rs:1-32` is a Criterion benchmark placed under `tests/`; nextest reports zero tests for that binary.
- `Protocol_Core/qanto-relayer/src/bridge_conformance_tests.rs:196-390` contains nine useful test functions, but `qanto-relayer/src/lib.rs:20-25` never declares that module. It is **not** in the 207-test run. The same library file does not expose the `delivery_state` module imported by that test source.
- The mandated `Scripts/tests/` directory does **not exist** in the effective repository. No mining/DAG/difficulty/throughput/SAGA scripts can be reported from that path.

### Solidity tests

There is one test source: `SmartContracts/test/contracts.test.js`. It contains only:

- `QantoFaucet` tests at `contracts.test.js:7-145`.
- `QUSD` tests at `contracts.test.js:147-288`.

No test suites exist for WQNTO, QantoGovernor, QantoTGE, QantoTimelock, QantoDrop, QantoSwapFactory, QantoSwapRouter or SentinelNFT.

## 11.B Executed test and build results

| Command | Result | Current evidence |
|---|---|---|
| `cargo nextest run --workspace --locked --no-fail-fast --retries 0` | **PASS** | 207 run, 207 passed, 1 skipped, 31 binaries, 13.346 seconds |
| Same nextest run at initial baseline | **FAIL** | 202 run: 201 passed, 1 failed, 1 skipped; the scheduler-sensitive PQ migration timing test failed |
| `cargo nextest show-config test-groups` | **PASS WITH CONFIG GAP** | Config parses, but both `integration` and `mining` groups report “no matches” |
| `cargo check --workspace --locked` | **PASS** | Default workspace targets compile |
| `cargo check --workspace --all-targets --locked` | **FAIL** | Five errors in stale `mining_bench.rs` |
| `npm test` in `SmartContracts/` | **PASS** | 38/38 Mocha tests; 10 Solidity files compiled with solc 0.8.24 |
| `npx hardhat test --coverage` | **PASS, COVERAGE BELOW GATE** | 38/38 tests; 42.45% line and 43.36% statement coverage |

The final Rust run used **zero retries**, so the green result is not retry-masked. The one ignored desktop test is `wallet_live_airdrop_send_history_e2e`, which depends on live external state.

## 11.C Formal verifier classification

`Protocol_Core/qanto-node/src/formal_verifier.rs` is **a placeholder/simulation, not a formal verification framework**:

- `prove_mint_soundness` performs ordinary runtime boolean/overflow checks and returns a constant synthetic proof ID, score, constraint count and timestamp (`formal_verifier.rs:26-54`).
- `run_infinite_fuzzing` loops and adds zero; it generates no inputs, symbolic states, counterexamples or persisted proof artifact (`:57-78`).
- `run_global_intent_burst` loops over a counter while hard-coding state drift to zero (`:81-107`).
- Its “U256” is only a `u128` wrapper; addition can panic in checked/debug builds and wrap in unchecked release builds (`:111-119`).
- The module is lexically isolated and has no test target in the discovered suite.

No SMT solver, model checker, theorem prover, specification language, proof certificate, invariant model, or CI verification step was found. Claims such as “no logical exploits possible” at `:77` are unsupported and must not be used as launch evidence.

## 11.D Measured Solidity coverage

| Contract | Line % | Statement % | Assessment |
|---|---:|---:|---|
| QUSD | 94.12 | 95.65 | Meaningful unit coverage, still no collateral/peg system because none exists |
| QantoFaucet | 89.19 | 88.89 | Meaningful unit coverage |
| QantoSwapFactory | 47.06 | 0.00 | Incidental constructor/declaration coverage; no behavior test |
| WQNTO | 10.00 | 0.00 | Effectively untested |
| QantoSwapRouter | 8.33 | 0.00 | Effectively untested |
| QantoTGE | 0.00 | 0.00 | Untested |
| QantoDrop | 0.00 | 0.00 | Untested |
| SentinelNFT | 0.00 | 0.00 | Untested |
| QantoGovernor | 0.00 | 0.00 | Untested |
| QantoTimelock | n/a | n/a | Thin inherited wrapper; no dedicated behavior test |
| **Total** | **42.45** | **43.36** | Below a production security threshold |

The current coverage artifact was written to `SmartContracts/coverage/html/`. No Rust coverage percentage was generated because `cargo-llvm-cov` is unavailable.

## 11.E Critical-path coverage gaps

| Priority | Required invariant | Existing evidence | Missing proof/test |
|---|---|---|---|
| P0 | CPU, OpenCL, CUDA and Metal produce identical QanHash for every consensus input | `cargo test -p my-blockchain --lib --locked` executes zero tests; known backend divergence is documented in `PQC_AND_TOKENOMICS.md:44-49` | Versioned golden vectors across message lengths, epochs, endian cases and hardware; CPU re-verification of GPU output |
| P0 | Two nodes receiving the same competing blocks reach identical accepted set, canonical path, UTXO root and balances | Pure DAG order/LCA/GHOST unit tests exist, but global state applies by arrival order (`CONSENSUS_NETWORK_TRANSACTION_SAGA.md:47-48`) | Multi-node/property test with permuted arrivals, conflicting spends, canonical apply/unapply and restart |
| P0 | Concurrent validation cannot accept two spends of one UTXO and cannot partially commit state | Add-block serialization and intra-block duplicate checks exist, but no adversarial concurrent/reorg property test (`CONSENSUS_NETWORK_TRANSACTION_SAGA.md:191-205`) | Barrier-controlled concurrent admission/validation, injected persistence failure, undo journal and state-root equality assertions |
| P0 | A disconnected peer reconnects without blocking the swarm and catches up after a partition | In-memory partition tests and one corrupt sync payload test exist; `e2e_pqc_migration.rs:35-37` explicitly says it is not a real libp2p test | Real multi-process/libp2p partition, disconnect mid-batch, backoff/jitter, command responsiveness and catch-up proof |
| P0 | Every externally reachable bridge flow fails closed and remains replay/reorg safe | ZK SDK fail-closed tests run; nine relayer conformance tests exist only in an unwired source file | Wire `bridge_conformance_tests`, fix module visibility, then add real gateway/reorg integration |
| P0 | Governance, timelock, TGE, drop, WQNTO, NFT and AMM fail-closed contracts behave as intended | No dedicated tests; measured 0-10% behavior coverage for most | Unit/property/fuzz tests for access control, reentrancy, schedule boundaries, Merkle replay, slippage and invariant math |
| P1 | Emission fixed-point recurrence is correct at all boundaries | Nine emission tests cover early periods, floor, cap and a 10,000-step loop (`e2e_mining_reward.rs:11-120`) | Pre-genesis timestamp, exact zero/floor transition, multi-chain remainder, extreme timestamp, persisted supply/restart, and the actual SAGA reward path |
| P1 | Fork-choice is deterministic for the full node state transition | Six focused deterministic DAG tests pass | Cross-process hash/state snapshots, randomized DAG permutations, duplicate-parent adversarial cases and reorg state application |

## 11.F Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| WS11-001 | SEV-1 | Mining consensus tests | `Protocol_Core/myblockchain/src/qanhash.rs:1396-1427`; `Protocol_Core/myblockchain/src/qanhash.cu:80-162`; `Protocol_Core/myblockchain/src/qanhash.metal:80-160` | `my-blockchain` executes zero tests and has no CPU/GPU equivalence contract despite known divergent implementations. | OPEN — launch blocker |
| WS11-002 | SEV-1 | State/reorg correctness | `Protocol_Core/qanto-node/src/qantodag.rs:1820-1830`, `:1910-1943`, `:2072-2091`; `Protocol_Core/qanto-node/src/partition_tests.rs:327-358` | No test proves identical state under competing-block arrival permutations, concurrent double-spend attempts, partial-commit failure or canonical reorg. | OPEN — launch blocker |
| WS11-003 | SEV-2 | P2P resilience | `Protocol_Core/qanto-node/tests/e2e_pqc_migration.rs:35-37`; `Protocol_Core/qanto-node/src/partition_tests.rs:25-91` | Existing partition/governor tests are in-process simulations, not real libp2p disconnect/reconnect/catch-up tests. | OPEN |
| WS11-004 | SEV-2 | Solidity coverage | `SmartContracts/test/contracts.test.js:7-288` | 38 tests cover only Faucet/QUSD; total coverage is 42.45% lines / 43.36% statements and eight mandated contracts lack dedicated tests. | OPEN |
| WS11-005 | SEV-3 | Formal verification | `Protocol_Core/qanto-node/src/formal_verifier.rs:26-119` | Constant synthetic results and counter loops are presented as proof/fuzzing; no formal engine exists. | CONFIRMED PLACEHOLDER |
| WS11-006 | SEV-3 | Test discovery | `Protocol_Core/qanto-node/tests/pqc_mempool_tests.rs:1-32`; `Protocol_Core/qanto-relayer/src/lib.rs:20-25`; `Protocol_Core/qanto-relayer/src/bridge_conformance_tests.rs:196-390` | A Criterion target lists zero tests and nine bridge tests are not wired into the crate. | OPEN |
| WS11-007 | SEV-3 | Nextest grouping | `.config/nextest.toml:24-43` | Mining/integration group filters name tests that do not exist, so intended concurrency limits apply to nothing. | OPEN |
| WS11-008 | SEV-3 | Benchmark compile | `Protocol_Core/qanto-node/benches/mining_bench.rs:20-27`, `:89-111`, `:293` | All-target check fails, so benchmark code is not continuously validated. | OPEN |
| WS11-009 | INFO | Rust suite | `.config/nextest.toml:4-22` | Final no-retry workspace run: 207/207 passed; 1 skipped. | VERIFIED CURRENT |
| WS11-010 | INFO | Script inventory | `Scripts/` | The mandated `Scripts/tests/` path is absent. | EXPLICITLY NOT FOUND |

## 11.G Implemented fixes

These fixes are present in the effective audit worktree:

1. The invalid root `.nextest.toml` was replaced by the canonical `.config/nextest.toml`. Current config parsing succeeds (`.config/nextest.toml:1-47`).
2. The flaky migration/governor test now uses one bounded ingress queue, joins both tasks, and applies one 30-second operation timeout instead of a scheduler-dependent per-packet 500-microsecond assertion (`e2e_pqc_migration.rs:35-37`, `:116-155`). It passed in the final no-retry suite.
3. Emission boundary/cap tests were added under `e2e_mining_reward.rs:11-120`; all nine tests pass. These exercise the integer engine, not the separate live SAGA reward path.
4. qanto-node fork-order/LCA/GHOST unit tests and block/transaction serialization determinism tests pass in the current suite.
5. The ZK SDK's compatibility verifier is now explicitly fail-closed and its five tests pass (`qanto-zk-sdk/src/lib.rs:240-250`, `:314-394`).
6. Hardhat was migrated to a working Hardhat 3 test stack; 10 contracts compile and 38 tests pass. Coverage reporting is now executable and exposes the remaining gap instead of relying on test-count optimism.

No consensus protocol, monetary policy or cryptographic format was changed by this dependency/testing report itself.

## 11.H Deferred items

| Priority | Item | Why deferred | Estimated effort |
|---|---|---|---:|
| P0 | QanHash cross-backend KAT/parity suite on CPU, CUDA, Metal and OpenCL | Requires an approved byte-exact algorithm/version and physical GPU runners | 3-6 weeks plus hardware validation |
| P0 | Branch-scoped state/reorg/concurrent double-spend property harness | Current state architecture has no canonical apply/unapply journal to assert against | 6-12+ weeks architecture, then 1-2 weeks testing |
| P0 | Real libp2p partition/reconnect/fast-sync harness | Needs multi-process orchestration, fault injection and a working authenticated sync protocol | 1-3 weeks after sync fixes |
| P0 | Raise critical Solidity contract coverage and add invariants/fuzzing | Eight contracts have no dedicated tests; AMM/governance behavior needs specification first | 2-4 weeks |
| P0 | Wire and repair relayer bridge conformance tests | Test source imports a module that the library does not expose | 1-3 days, then integration expansion |
| P1 | Replace synthetic formal verifier with an actual method or remove claims | Requires formal specification and a chosen framework; external review is mandatory | 4-12+ weeks |
| P1 | Install `cargo-llvm-cov` and enforce Rust coverage in CI | Tool unavailable in this environment; thresholds need ownership-based baselines | 2-5 days |
| P1 | Install/run Miri for unsafe/UB-sensitive code | Miri component is unavailable for the pinned toolchain in this environment | 2-5 days plus triage |
| P1 | Repair nextest group filters and all-target benchmark | Current names no longer match discovered tests; benchmark APIs are stale | 1-3 days |
| P2 | Restore or formally retire `Scripts/tests/` claims | The referenced directory does not exist | 1-3 days after scope decision |

## 11.I Unavailable tooling, explicitly separated

| Tool / evidence | Status | Consequence |
|---|---|---|
| `cargo llvm-cov` | **UNAVAILABLE** — Cargo reports no such command | No measured Rust line/branch coverage percentage. Absence is not treated as zero percent; critical gaps are established by test discovery and source inspection instead. |
| Miri | **UNAVAILABLE** for `nightly-2026-06-28-aarch64-apple-darwin` | No interpreter-based undefined-behavior pass. |
| CUDA hardware/toolchain | **UNAVAILABLE** in this macOS arm64 audit environment | No physical CUDA parity run; source divergence still establishes the blocker. |

Hardhat coverage was available and executed, so its percentages are measured rather than estimated.

## 11.J Workstream verdict

**NOT READY.** A fully green 207-test Rust run and 38-test Solidity run are real improvements, but they do not cover the failure modes that can fork or drain the protocol. Consensus-backend parity, canonical state/reorg behavior, concurrent UTXO safety, real P2P recovery, bridge conformance discovery and most contract behavior remain unproved.
