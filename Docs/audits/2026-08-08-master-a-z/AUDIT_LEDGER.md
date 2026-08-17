# Qanto Master A-Z Audit Ledger

Audit date: 2026-08-08

Final evidence reconciliation: 2026-08-09

Audit target: effective working tree at `/Users/cps/qanto`

Baseline commit: `0478b60368df86ece44b9ad30766a724dfc60d50`

Baseline branch: `main`

Coordinator rule: pre-existing tracked and untracked work is preserved and is not attributed to this audit pass.

Final expanded-mandate verdict: **NOT SAFE FOR PRIVATE TESTNET**

Final twelve-workstream verdict: **NOT READY**

## Evidence labels

- `CONFIRMED`: demonstrated by source inspection, a deterministic check, or a reproduction.
- `MEASURED`: produced by an executed test or benchmark in the recorded environment.
- `DERIVED`: follows mathematically from confirmed inputs.
- `ESTIMATED`: depends on stated assumptions.
- `HYPOTHESIS`: plausible but not yet reproduced.
- `NEEDS_EXTERNAL_REVIEW`: cannot be established solely from this repository and local environment.

## Repository baseline and final evidence

| ID | Command/check | Result | Interpretation |
|---|---|---|---|
| B-001 | `git rev-parse HEAD`, branch, remotes, submodules | PASS | `main` at `0478b603`; remotes are GitHub `origin` and Hugging Face `huggingface`; no submodules were reported. |
| B-002 | Baseline `git status --short --branch` | WARN | Candidate tree was materially dirty before audit work began: 32 tracked paths differed, 8 tracked paths were deleted, and 652 untracked paths existed. Existing work must not be overwritten. |
| B-003 | Rust/Node toolchains | PASS | Rust `1.98.0-nightly` / Cargo `1.98.0-nightly`, pinned by `rust-toolchain.toml` to `nightly-2026-06-28`; Node `v26.5.0`; npm `11.17.0`. |
| B-004 | Workspace metadata | PASS | Cargo resolves eight workspace members and the documented default `zk` feature on `qanto`. |
| B-005 | `cargo fmt --all -- --check` | FAIL | The effective tree is not rustfmt-clean. Failures include new relayer/runtime sources plus crypto, node, wallet, and ZK SDK files. No automatic formatting was applied because these are pre-existing user changes. |
| B-006 | `tools/verify_certik_score.py --self-test` | PASS | Local score verifier weighting, cap, acceptance, and validation self-tests pass. This does not establish that the supplied scorecard is internally consistent or accepted. |
| B-007 | `cargo check --workspace --all-targets --locked` | FAIL | Five compile errors originate in `Protocol_Core/qanto-node/benches/mining_bench.rs`: missing `qanto::protocol`, `BlockTemplate`, `HeaderFirstSync`, and obsolete `cached_header_hash`. Ten warnings were also emitted. |
| B-008 | Tool availability | WARN | `cargo-audit 0.22.2`, `cargo-deny`, Hardhat `3.12.0`, and `cargo-fuzz 0.13.2` were available and used where applicable. Slither, Mythril, Miri, Foundry/Echidna, Docker, Helm, kubectl, ShellCheck, and Terraform were unavailable; none is credited as a pass. |
| B-009 | Build artifacts | INFO | `target/` is ignored and occupies approximately 59 GiB; `SmartContracts/node_modules/` is ignored and occupies approximately 290 MiB. |
| B-010 | Generated/vendored scan | INFO | Checked-in protobuf output exists at `Protocol_Core/qanto-rpc/src/generated/qanto.rs`. Local patched crates live under `patches/`; they require a separate provenance and necessity audit. |
| B-011 | Referenced repository rules | FAIL | `AGENTS.md` names `.agents/rules/SecurityRules.md` and `.agents/rules/DevTestRules.md`, but neither file exists in the effective tree. |
| B-012 | RTK telemetry helper | INFO | `rtk gain` cannot create its history database under the sandboxed application-support directory. This is an audit-tool telemetry limitation, not a Qanto product failure. |
| B-013 | Final `git status --short` snapshot | WARN | The shared candidate tree remained materially dirty at final reconciliation: 60 modified, 9 deleted, and 179 untracked paths (248 status entries). These include pre-existing user work, concurrent remediation, audit evidence, and generated test artifacts; this ledger does not attribute all entries to the audit. |
| B-014 | `cargo check --workspace --locked` | PASS | The default Cargo workspace targets compile. This does not cover all targets or supported feature combinations. |
| B-015 | `cargo nextest run --workspace --locked --no-fail-fast --retries 0` | PASS | 207/207 runnable tests passed across 31 binaries; one additional test was skipped. The run used zero retries. |
| B-016 | Hardhat compile/test and instrumented coverage | WARN | Ten Solidity files compile with solc 0.8.24 and 38/38 tests pass. Instrumented coverage is only 42.45% lines and 43.36% statements; critical contracts remain at 0% coverage. |
| B-017 | `cargo audit --no-fetch` and strict warning mode | WARN | Ordinary audit exits 0 with zero non-ignored known vulnerabilities. Strict warning mode remains red on five warnings: three unmaintained dependencies and two yanked releases. |
| B-018 | `cargo deny check` policy gates | FAIL | Advisories fail on one unsound and three unmaintained dependency conditions plus two yanked warnings. Licenses fail because four first-party packages have no license expression. Bans and sources pass. |
| B-019 | Extended Rust build matrix | FAIL | All-targets fails with five stale benchmark API errors; `qanto --no-default-features` fails with 44 feature-gating errors; `my-blockchain --features cuda` fails with five missing/unresolved integration errors. |
| B-020 | Rust quality gates | FAIL | Strict Clippy is not green, including remaining relayer `manual_is_multiple_of` and RPC `result_large_err` findings after local QanHash lint cleanup. Full rustfmt check still fails broadly across the dirty tree. The touched-file Git diff whitespace check passes. |

## Baseline and mutation boundary

No source code was changed during Phase 0. Subsequent work occurred in a shared, already-dirty tree. The annexes record verified containment and hardening changes as `FIXED_IN_WORKTREE`, `CONTAINED_IN_WORKTREE`, or `PRE_EXISTING_FIX`; those labels describe the effective candidate, not exclusive authorship. No reset, clean, staging, commit, or destructive operation was performed.

## Command log

Commands are executed through the repository-required `rtk` wrapper. Long command output is summarized here; failures remain available in the active task transcript.

| Time/phase | Command family | Status |
|---|---|---|
| Phase 0 | repository identity, remotes, dirty-tree inventory | complete |
| Phase 0 | Cargo metadata, feature and target inventory | complete |
| Phase 0 | formatter and all-target compile baseline | complete with failures B-005 and B-007 |
| Phase 0 | local benchmark-verifier self-test | complete |
| Phase 1 | module/dependency graph, trust boundaries, critical-flow reconstruction | complete |
| Workstreams 1-4 | workspace/dependency/testing, consensus, PQC/QanHash/ZK, tokenomics | complete with launch blockers |
| Workstreams 5-7 | P2P, transaction/UTXO, SAGA/OMEGA | complete with launch blockers |
| Workstreams 8-10 | Solidity, performance, deployment/operations | complete with launch blockers and unavailable external tools |
| Workstreams 11-12 | full test inventory, formal-verifier classification, specification delta | complete with coverage and evidence gaps |
| Final verification | default Cargo check, zero-retry Nextest, Hardhat test/coverage, RustSec, cargo-deny, feature/target matrix, diff check | complete; mixed result recorded in B-014 through B-020 |
| Final readiness review | private-testnet, public-testnet, and mainnet gates | complete; no-go |

## Change log for this audit pass

| Change ID | Path | Purpose | Verification |
|---|---|---|---|
| AZ-001 | `Docs/audits/2026-08-08-master-a-z/` | Persistent evidence ledger, architecture/threat reconstruction, 12-workstream findings, protocol proposals, remediation roadmap, and launch gates | Cross-referenced against the effective tree and executed command matrix |
| AZ-002 | Cargo manifests, lockfile, audit policy, and `deny.toml` | Remove unnecessary overrides, update selected dependencies, and establish stricter dependency/source/license policy | Default workspace check passes; cargo-audit is green for known vulnerabilities; cargo-deny advisory/license gates remain red |
| AZ-003 | Consensus, transaction, networking, QanHash, and bridge paths | Contain reproduced acceptance failures: recomputed block IDs/fixed difficulty, exact reward/coinbase equality, fail-closed bridge/PQ paths, inputless-mempool rejection, deterministic tie ordering, disabled unsafe automatic finality/sharding, explicit little-endian QanHash decoding | Full zero-retry Nextest passes; unresolved protocol-level blockers remain listed in the consensus and PQC annexes |
| AZ-004 | Solidity contracts, Hardhat configuration/tests, and WQNTO deployment tooling | Add fail-closed behavior, deployment validation, decimal/chain-ID consistency, and focused contract tests | 38/38 tests pass; npm audit reports zero advisories; measured coverage remains below release quality at 42.45%/43.36% |
| AZ-005 | User-facing security, contribution, bridge, airdrop, overview, and README material | Remove or qualify unsupported production, bridge, DEX, CEX, performance, PQC, and launch claims | Specification-delta review records remaining discrepancies; documentation does not turn unimplemented protocol features into readiness evidence |
| AZ-006 | Nextest configuration and regression assets | Stabilize the reproduced PQ timing test and preserve focused containment regressions | 207/207 runnable Rust tests pass with one skipped and zero retries; critical multi-node, reorg, backend-parity, and fuzz coverage remains absent |

## Final command matrix

| Gate | Final state | Release interpretation |
|---|---|---|
| Default Rust workspace | **PASS** | Necessary compile gate only |
| Rust Nextest | **PASS: 207/207, 1 skipped** | Useful regression signal; not a consensus proof |
| Hardhat | **PASS: 38/38** | Focused Faucet/QUSD behavior passes |
| Solidity coverage | **42.45% lines / 43.36% statements** | Below a production security threshold; multiple value/authority contracts have no tests |
| Cargo audit | **PASS: 0 non-ignored known vulnerabilities; 5 warnings under strict mode** | Warning debt remains release-relevant |
| Cargo deny | **FAIL: advisories and licenses** | Controlling supply-chain/policy gate is red |
| All-target Rust | **FAIL: 5 errors** | Mining benchmark is stale and cannot compile |
| No-default node | **FAIL: 44 errors** | Fail-closed feature profile is not buildable |
| CUDA mining feature | **FAIL: 5 errors** | GPU backend is not a supported build; source algorithms also diverge |
| Strict Clippy | **FAIL** | Warning-free release quality not established |
| Full rustfmt | **FAIL** | Effective tree is not formatting-clean |
| Solidity SAST/fuzz and infrastructure validation | **UNAVAILABLE/NOT RUN** | Slither/Mythril/Foundry/Echidna and deployment-runtime tools cannot be credited |

## Final decision

The passing default build and unit/integration suites do not neutralize the confirmed ability to authorize native spends with an unbound key, label blocks as another validator, derive consensus rewards and block limits from local mutable state, mutate one global UTXO state in arrival order without atomic reorg support, diverge across QanHash backends, or start without an authenticated state-sync/finality protocol. The bridge is safer only because claims are disabled.

- Expanded mandate: **NOT SAFE FOR PRIVATE TESTNET**.
- Twelve-workstream launch decision: **NOT READY**.
- Public testnet: no-go until every mandatory P0 gate in `MAINNET_READINESS.md` passes with multi-node and adversarial evidence.
- Mainnet: not eligible for review until public-testnet gates, external cryptographic/security review, sustained soak/chaos testing, and approved economic/protocol migrations are complete.
