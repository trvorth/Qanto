# QANTO REPOSITORY BASELINE AUDIT

## Timestamp: 2026-08-17

### Git Repository State
- **Current commit**: 929cc5dd (docs: define Qanto testnet readiness program)
- **Branch**: feature/qanto-testnet-readiness-design
- **Remotes**: 
  - origin: https://github.com/trvorth/qanto.git
  - huggingface: https://huggingface.co/spaces/trvorth/qanto-testnet
- **Modified files**: 51 files modified (Cargo.toml files, consensus primitives, node sources, desktop wallet)
- **Untracked files**: 100+ files including attack tests, docs, configs, infra, patches
- **Recent commits**: 5 commits ahead of main branch

### Toolchain Environment
- **Rust**: rustc 1.98.0-nightly (13f1859f2 2026-06-27)
- **Cargo**: cargo 1.98.0-nightly (a335d47ff 2026-06-26)
- **Toolchain file**: rust-toolchain.toml pins nightly-2026-06-28 with clippy and rustfmt components
- **Node.js**: Available (package.json files in SmartContracts, qanto-desktop-wallet, qanto-dapp)
- **Python**: Not verified in this session

### Workspace Structure (11 Cargo Workspace Members)

| Crate | Path | Role | Type |
|-------|------|------|------|
| **qanto** | Protocol_Core/qanto-node | Main node binary, DAG consensus, P2P, mining, RPC | Binary + Library |
| **qanto-core** | Protocol_Core/qanto-core | Core crypto, networking, serialization, protocol identity, storage | Library |
| **qanto-consensus-primitives** | Protocol_Core/qanto-consensus-primitives | Low-level V2 consensus encoding, hashing, PoW primitives | Library |
| **qanto-rpc** | Protocol_Core/qanto-rpc | gRPC API service (tonic/prost) | Library |
| **qanto-relayer** | Protocol_Core/qanto-relayer | Cross-chain transaction relaying | Binary + Library |
| **qanto-zk-sdk** | Protocol_Core/qanto-zk-sdk | Zero-knowledge proof tools (arkworks) | Library |
| **my-blockchain** | Protocol_Core/myblockchain | Experimental standalone implementation, GPU hashing | Library + Bench |
| **qanto-desktop-wallet** | qanto-desktop-wallet/src-tauri | Tauri desktop wallet (React + Rust) | Binary (Tauri) |
| **qanto-legacy-relayer** | Scripts/tools/relayer | Legacy relayer implementation | Binary |
| **ark-relations** | patches/ark-relations | Patched arkworks dependency (tracing-subscriber 0.3.20) | Patch |
| **libp2p-dns / libp2p-mdns** | patches/ | Patched libp2p dependencies for hickory 0.26 | Patch |

### Binaries & Entry Points (qanto-node)
```
qanto                 # Main node binary (src/bin/qanto.rs)
qantowallet           # Wallet generation tool (src/bin/qantowallet.rs)
generate_wallet       # Wallet generation utility (src/bin/generate_wallet.rs)
import_wallet         # Wallet import utility (src/bin/import_wallet.rs)
monitor               # Monitoring utility (src/bin/monitor.rs)
metrics_server        # Metrics server (src/bin/metrics_server.rs)
performance-benchmark # Performance benchmark (src/performance_benchmark.rs)
genesis_flood         # Genesis flood test (src/bin/genesis_flood.rs)
```

### Feature Flags (qanto-node)
| Feature | Status | Description |
|---------|--------|-------------|
| `default` | `["zk"]` | ZK proofs enabled by default |
| `zk` | ✅ | Full ZK stack (bellman, arkworks, halo2_proofs, pasta_curves) |
| `analyzer` | ✅ | rust-analyzer compatibility dummy |
| `real-zk-verifier` | ✅ | Real ZK verifier |
| `cuda` / `opencl` / `gpu` | ✅ | GPU acceleration (optional) |
| `infinite-strata` | ✅ | Infinite strata feature |
| `performance-test` | ✅ | Performance test utilities |
| `dhat-heap` | ✅ | Heap profiling |
| `ai` | ❌ Removed | AI dependencies removed for production hardening |

### Build Status
```bash
cargo check --workspace --locked        # ✅ PASSED (with warnings only)
cargo build --release --features infinite-strata  # Expected PASSED
cargo fmt --all -- --check              # Need to verify
cargo clippy --workspace --all-targets --locked -- -D warnings  # ❌ FAILED (78 warnings in node, ~19 in consensus primitives)
```

**Key Clippy Warnings**: unused imports, non_camel_case_types (V1_LEGACY, V2_INITIAL), dead_code (TrieNode methods), unused variables, items_after_test_module

### Test Results (384 tests run, 384 passed, 1 skipped)
```bash
cargo nextest run --workspace --locked:
  qanto-core                    47 tests  ✅
  qanto-consensus-primitives    84 tests  ✅ (includes QIP-002 golden vectors)
  qanto-desktop-wallet          8 tests   ✅
  qanto-legacy-relayer          4 tests   ✅
  qanto-zk-sdk                  7 tests   ✅
  qanto-relayer                 4 tests   ✅
  qanto-rpc                     4 tests   ✅
  qanto (qanto-node)            226 tests ✅ (includes 12 attack test modules)
```

### Attack Test Modules (12 total — all compile and pass)
| Module | Description | Tests |
|--------|-------------|-------|
| **attack_1** | Single-parent DAG safety violation (double-spend) | 6 |
| **attack_2** | Liveness failure under fixed difficulty / hashrate drop | 3 |
| **attack_3** | Finality absence - reorg at any depth | 2 |
| **attack_4** | EVM raw transaction bypasses PQ signature verification | 1 |
| **attack_5** | Bridge claim with forged signature | 1 |
| **attack_6** | Unauthenticated RPC/WebSocket access | 1 |
| **attack_7** | P2P eclipse via fixed gossipsub topics | 1 |
| **attack_8** | Miner reservation griefing | 1 |
| **attack_9** | Consensus lock bottleneck | 1 |
| **attack_10** | blocks_map performance / memory growth | 1 |
| **attack_11** | SAGA f64 nondeterminism | 1 |
| **attack_12** | ΩMEGA threat manipulation | 1 |

### Fuzzing Infrastructure
- **Crate**: `qanto-fuzz` (independent workspace at Protocol_Core/qanto-node/fuzz/)
- **Targets**: `fuzz_tx_parsing`, `fuzz_bridge_proofs`
- **Corpus**: 180+ test cases for tx parsing
- **Tool**: cargo-fuzz with libfuzzer-sys

### Benchmarks
- **Location**: Protocol_Core/qanto-node/benches/mining_bench.rs
- **Tool**: criterion (dev-dependency in myblockchain)
- **Benchmarks**: mining performance, display throughput, balance broadcast

### Dependency Overview (Top Transitive)
- **Core**: serde, rand, libc, serde_json, tokio, ahash, tracing, sha3, hex
- **Crypto**: k256, ed25519-dalek, dilithium-rs, ml-kem, pqcrypto-dilithium, argon2, aes-gcm
- **Networking**: libp2p (gossipsub, kad, noise, yamux, dns, websocket), hickory-proto/resolver
- **ZK/Arkworks**: ark-bls12-381, ark-groth16, ark-r1cs-std, ark-relations (patched), halo2_proofs, pasta_curves, bellman
- **P2P**: libp2p 0.56 with full protocol suite
- **Web**: axum, async-graphql, tonic, prost, hyper, reqwest, tower-http
- **Database**: rocksdb (myblockchain), custom storage with WAL (qanto-core)
- **Security patches forced via workspace.dependencies**: quick-xml 0.41.0, quinn-proto 0.11.15, rustls-webpki 0.103.13, ring 0.17.12, hickory-proto 0.26.1, rustls 0.23

### Security Policy (deny.toml)
- **Advisories**: unmaintained = workspace, unsound = all
- **Licenses**: Allowlist of 18 permissive licenses (MIT, Apache-2.0, BSD, ISC, etc.)
- **Bans**: multiple-versions = warn, wildcards = deny
- **Sources**: Only crates.io and tokio-rs/tracing git allowed

### Code Metrics
- **Rust source files**: ~121,423 lines (Protocol_Core only)
- **Crates**: 11 workspace members
- **Test files**: 12 attack test modules + integration tests + fuzz targets
- **Patch files**: 3 (ark-relations, libp2p-dns, libp2p-mdns)

### Known Critical Issues (from attack tests & A-Z audit)

| ID | Issue | Location | Severity |
|----|-------|----------|----------|
| C-01 | **Single-parent DAG** - No multi-parent consensus, 51% double-spend | qantodag.rs:3245-3248 | CRITICAL |
| C-02 | **Fixed difficulty** - INITIAL_DIFFICULTY = 1e9 hardcoded, no retarget | consensus.rs:103,159 | CRITICAL |
| C-03 | **No finality** - finalize_blocks() is no-op, no checkpointing | qantodag.rs | CRITICAL |
| C-04 | **Miner local backoff** conflicts with consensus (difficulty /= 2) | miner.rs | HIGH |
| C-05 | **SAGA AI** - f64 nondeterminism, not consensus-safe | saga.rs | HIGH |
| C-06 | **PQC key rotation** - Manual ML-DSA derivation, no policy | wallet.rs, keystore.rs | MEDIUM |
| C-07 | **EVM raw tx** - Bypasses PQ signature verification | transaction.rs | HIGH |
| C-08 | **Bridge/IBC** - Mock verification only, no real proof validation | relayer/ | HIGH |
| C-09 | **UnAuth RPC/WS** - No authentication on RPC/WS endpoints | rpc_backend.rs, p2p.rs | HIGH |
| C-10 | **P2P eclipse** - Fixed gossipsub topics | p2p.rs | HIGH |
| C-11 | **Lock bottleneck** - Consensus lock contention | qantodag.rs | MEDIUM |
| C-12 | **blocks_map unbounded** - In-memory DashMap grows without bound | qantodag.rs | MEDIUM |

### Documentation Inventory
- **Guides** (Docs/guide/): testnet-guide.md, join-testnet-guide.md, private-engineering-testnet.md, qantowallet-guide.md, wallet-env-password-guide.md, airdrop_guide.md
- **Architecture** (Docs/architecture/): CODEBASE_MAP.md, DUPLICATION_MAP.md
- **Audits** (Docs/audits/): 2026-08-08-master-a-z/ with DEPENDENCY_AUDIT_AND_TESTING.md
- **Security**: SECURITY_REVIEW_REPORT.md, EXTERNAL_AUDIT_REPORT_2026-07-31.md
- **Runbooks** (Docs/runbooks/): Operational procedures
- **Protocol specs** (Docs/protocol/): Protocol specifications
- **Readiness configs** (config/readiness/): Testnet readiness configs

### CI/CD Infrastructure
- **GitHub Actions**: security.yml, solidity.yml workflows
- **Docker**: DevOps/docker/Dockerfile.lightweight
- **AWS Infrastructure**: DevOps/infrastructure/aws-lightweight/
- **Grafana**: grafana/ dashboards

### Configuration Files
- `genesis.json` - Genesis configuration
- `config/readiness/` - Testnet readiness configs
- `.env` files (not tracked) - Runtime configuration
- `deny.toml` - Dependency security policy

---

## Phase 0 Baseline Status: **COMPLETE**

All discovery items documented. Ready for Phase 1 deep architecture analysis.