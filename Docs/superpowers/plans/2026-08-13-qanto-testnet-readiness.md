# Qanto Testnet Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Qanto A–Z engineering mandate and issue an evidence-backed private- or public-testnet readiness verdict for one exact, reproducible candidate.

**Architecture:** Work proceeds through ten independently reviewable workstreams. Candidate reconciliation and controlled-private-network containment come first; all public consensus, monetary, genesis, cryptographic, governance, compatibility, timing, and chain-identity changes remain behind explicit V2 activation boundaries and protocol-approval gates. Every gate consumes an immutable candidate manifest and evidence index rather than inheriting claims from older commits or reports.

**Tech Stack:** Rust 2021, pinned `nightly-2026-06-28`, Cargo locked workspace, Tokio, libp2p, ML-DSA-65 Qanto native crypto, SHA3-256 V2 hash frames, `CanonicalRecord`, QantoStorage write batches/WAL, nextest, proptest, cargo-fuzz, local chaos tooling, Docker, JSON/TOML/Markdown evidence.

## Global Constraints

- Work locally. Do not push, publish, release, deploy, alter real DNS, rotate real keys, transfer assets, activate a bridge, or change live infrastructure without explicit authorization.
- Never overwrite unrelated existing work. The starting tree is heavily dirty and must remain attributable file-by-file.
- Never run `git reset --hard`, `git clean -fd`, forced checkout, history rewriting, or an equivalent destructive command without explicit authorization.
- Never modify consensus-visible policy silently. Consensus, monetary policy, genesis, cryptography, governance, compatibility, public chain ID, timing, finality, slashing, and public-network behavior require a written protocol-change proposal, migration analysis, rollback analysis, and explicit protocol approval before activation.
- Private engineering-testnet operation is resettable, non-value-bearing, access-controlled, and uses a dedicated development chain ID and deterministic genesis. It must not be described as permissionless public-testnet readiness.
- Bridge/IBC, QUSD, AMM/QantoSwap, sale/TGE, incomplete EVM identity paths, dynamic sharding, multi-parent consensus, DPoS/PoSe validity, unverified heterogeneous mining, mutable/mock ZK verification, and consensus-affecting SAGA/ΩMEGA remain unreachable unless separately approved and gated.
- Failed commands, failed tests, blocked tools, crashes, and contradictory results are evidence and must be retained.
- Never fabricate tests, TPS, latency, storage, bandwidth, prices, revenues, advisories, attack impact, economic outcomes, cryptographic properties, security guarantees, or independent-review status.
- Every material claim uses exactly one label: `CONFIRMED`, `MEASURED`, `DERIVED`, `ESTIMATED`, `HYPOTHESIS`, or `NEEDS EXTERNAL REVIEW`.
- Every gate uses exactly one status: `PASS`, `PARTIAL`, `FAIL`, `BLOCKED`, or `NOT APPLICABLE`.
- Every readiness result names one exact Git commit/tree, dirty-state manifest, lockfile hashes, toolchain, feature allowlist, configuration hash, chain ID, genesis commitment, consensus-parameter commitment, binary/image hashes, host description, and evidence bundle.
- Prefix shell commands with `rtk`, including every command in a chain. Run `graphify query`, `graphify explain`, or `graphify path` before broad source exploration; run `rtk graphify update .` after source changes.
- Use `$CLAUDE_JOB_DIR/tmp` for temporary artifacts. Durable evidence belongs under `audit-results/readiness/<candidate-id>/`.
- Use TDD for behavior changes: establish the failing or vulnerable case, run it, implement the minimum approved behavior, rerun the focused test, then run the applicable crate/workspace gate.
- Commit only explicitly related files. Every implementation commit ends with `Co-Authored-By: Claude <noreply@anthropic.com>`; do not commit or push unless the user asks.
- A self-authored or AI-generated review is not third-party independent verification.

---

## Plan Suite and Dependency Order

| Order | Workstream plan | Produces | Hard dependency |
|---|---|---|---|
| 1 | `2026-08-13-qanto-candidate-evidence.md` | Exact candidate, private release profile, evidence schemas, current baseline | None |
| 2 | `2026-08-13-qanto-canonical-v2.md` | Frozen V2 schemas, identities, consensus-parameter commitment, activation boundary | Workstream 1; protocol approval before V2 activation |
| 3 | `2026-08-13-qanto-branch-state-persistence.md` | Parent-scoped execution, atomic deltas, deterministic fork choice/reorg/recovery | Workstream 2; fork-choice approval before activation |
| 4 | `2026-08-13-qanto-finality-certificates.md` | Signed votes/certificates, finalized checkpoint, equivocation evidence, set transitions | Workstreams 2–3; finality policy approval before activation |
| 5 | `2026-08-13-qanto-economics-genesis.md` | Deterministic supply/fee/vesting rules and SAGA/ΩMEGA isolation | Workstreams 2–3; QIP-003 and genesis approval before activation |
| 6 | `2026-08-13-qanto-difficulty-pow.md` | Parent-derived difficulty, timestamp rules, exact target/work arithmetic, parity vectors | Workstreams 2–3; timing/retarget approval before activation |
| 7 | `2026-08-13-qanto-network-api-sync.md` | Authenticated bounded transport, canonical admission, eclipse controls, resumable sync | Workstreams 2–6 |
| 8 | `2026-08-13-qanto-optional-feature-quarantine.md` | Build/runtime/RPC/transaction reachability denial for excluded features | Workstreams 1–2; runs in parallel with 3–7 where files do not overlap |
| 9 | `2026-08-13-qanto-verification-evidence.md` | Properties, fuzzing, crash/chaos, benchmarks, storage/bandwidth/economic models | Workstreams 1–8 |
| 10 | `2026-08-13-qanto-operations-audit-verdict.md` | Reproducible operations, drills, independent review packet, A–AP report, scorecard, 45 answers, verdict | Workstreams 1–9 |

Workstreams 3–7 touch overlapping consensus/runtime files and execute sequentially. Workstream 8 may proceed concurrently only for files outside the active consensus workstream; its final reachability suite runs after Workstream 7. Workstreams 9–10 consume an exact release candidate and never backfill evidence from a different candidate.

## Shared Interfaces

The following interfaces are defined once and consumed throughout the suite. Child plans may refine implementation detail, but must not rename or weaken these contracts without updating every dependent plan.

### Candidate manifest

Create `tools/readiness/schema/candidate-manifest.schema.json` and emit `audit-results/readiness/<candidate-id>/candidate-manifest.json`:

```json
{
  "schema_version": 1,
  "candidate_id": "sha256:<canonical-manifest-hash>",
  "captured_at_utc": "RFC3339 supplied by the capture command",
  "git": {
    "head": "40-lowercase-hex",
    "branch": "feature/qanto-testnet-readiness-design",
    "tracked_diff_sha256": "64-lowercase-hex",
    "untracked_inventory_sha256": "64-lowercase-hex",
    "dirty": true
  },
  "inputs": {
    "cargo_lock_sha256": "64-lowercase-hex",
    "rust_toolchain_sha256": "64-lowercase-hex",
    "rustc_verbose": "captured output",
    "cargo_verbose": "captured output",
    "feature_allowlist": [],
    "workspace_members": [],
    "config_sha256": "64-lowercase-hex",
    "chain_id": "u32_from_sha3_256(b\"QANTO\\0PRIVATE-ENGINEERING\\02026-08-13\")",
    "network_id": "qanto-engineering-private-v1",
    "genesis_manifest_sha256": "64-lowercase-hex",
    "genesis_commitment": "64-lowercase-hex",
    "allocation_state_root": "64-lowercase-hex",
    "consensus_parameters_sha256": "64-lowercase-hex"
  },
  "release": {
    "stage": "private-engineering",
    "included_binaries": ["qanto", "qantowallet", "genesis_manifest"],
    "excluded_capabilities": ["bridge", "qusd", "amm", "sale", "dynamic-sharding", "multi-parent", "dpos-validity", "pose-validity", "consensus-saga", "consensus-omega", "evmcall", "mutable-zk-verifier", "heterogeneous-mining"]
  }
}
```

The literal values above are examples of shape, not approved public parameters. The capture tool fills observed fields and rejects missing or malformed commitments; a reviewer approves the resulting manifest before evidence is collected.

### Evidence record

Create `tools/readiness/schema/evidence-record.schema.json`; every command writes one `evidence/<record-id>.json` plus raw stdout/stderr files:

```json
{
  "schema_version": 1,
  "record_id": "baseline-cargo-check",
  "candidate_id": "sha256:<candidate>",
  "claim": "Locked workspace check result",
  "label": "CONFIRMED",
  "gate_status": "PASS",
  "command": ["rtk", "cargo", "check", "--workspace", "--locked"],
  "started_at_utc": "RFC3339",
  "finished_at_utc": "RFC3339",
  "exit_code": 0,
  "host_id": "sha256:<redacted-host-metadata>",
  "stdout_path": "raw/baseline-cargo-check.stdout",
  "stderr_path": "raw/baseline-cargo-check.stderr",
  "artifacts": [],
  "notes": []
}
```

A command with a nonzero exit code uses `FAIL` unless the evidence requirement could not run at all, in which case it uses `BLOCKED`; the wrapper itself returns the underlying exit code after writing the record.

### Attack-test metadata

Create `tools/readiness/attack-tests.toml` with one record per attack module:

```toml
[[attack]]
id = "attack-2-fixed-difficulty-liveness"
module = "attack_2_liveness_failure_fixed_difficulty"
threat = "Hashrate change causes permanent block-production failure"
classification = "exploit-reproduction"
vulnerable_precondition = "Consensus accepts only one fixed difficulty while miner mutates template difficulty"
expected_vulnerable_result = "Reduced-difficulty block is rejected and no canonical adaptive rule restores liveness"
expected_remediated_result = "Every miner and validator derives one identical next difficulty from the parent history"
current_interpretation = "Passing reproduces the vulnerability; it is not remediation evidence"
paired_regression = "difficulty_pow::tests::parent_derived_difficulty_survives_restart_and_reorg"
public_gate = "FAIL"
```

Allowed `classification` values are `exploit-reproduction`, `containment`, `remediation-regression`, and `performance-characterization`.

### Protocol-change status

Create `Docs/protocol/proposals/README.md` and require each proposal to contain this front matter:

```yaml
---
qip: QIP-XXX
status: REQUIRES PROTOCOL APPROVAL
changes:
  - consensus
activation: unreachable
migration: new-v2-genesis
rollback: replace-prelaunch-candidate
---
```

Only a human-approved change may move from `REQUIRES PROTOCOL APPROVAL` to an approved state. Implementation behind an unreachable V2 boundary is permitted; activation is not.

---

### Task 1: Pin the Exact Candidate and Preserve the Starting State

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-candidate-evidence.md`
- Create: `tools/readiness/schema/candidate-manifest.schema.json`
- Create: `tools/readiness/schema/evidence-record.schema.json`
- Create: `tools/readiness/schema/evidence-index.schema.json`
- Create: `tools/readiness/capture-candidate.sh`
- Create: `tools/readiness/run-evidence.sh`
- Create: `tools/readiness/attack-tests.toml`
- Create: `config/readiness/private-engineering.toml`
- Create: `config/readiness/private-engineering-genesis.json`
- Create: `config/readiness/private-release-allowlist.toml`
- Create: `audit-results/readiness/<candidate-id>/candidate-manifest.json` (generated)
- Create: `audit-results/readiness/<candidate-id>/evidence-index.json` (generated)

**Interfaces:**
- Produces: immutable `candidate_id`, manifest, evidence wrapper, private capability allowlist, classified attacks, baseline result set.
- Consumed by: every later task and every readiness claim.

- [ ] **Step 1: Execute the child plan through its candidate-capture tests.**

Run each checkbox in `2026-08-13-qanto-candidate-evidence.md` Tasks 1–4. Do not run a baseline command until manifest validation and dirty-tree attribution tests pass.

- [ ] **Step 2: Freeze the candidate selected for baseline evidence.**

Run:

```bash
rtk bash tools/readiness/capture-candidate.sh \
  --stage private-engineering \
  --config config/readiness/private-engineering.toml \
  --genesis config/readiness/private-engineering-genesis.json \
  --allowlist config/readiness/private-release-allowlist.toml \
  --output audit-results/readiness
```

Expected: prints exactly one `candidate_id=sha256:...` and one durable candidate directory; refuses an all-zero/missing genesis or consensus-parameter commitment.

- [ ] **Step 3: Run and retain the complete current baseline.**

Execute the child plan’s baseline matrix. Expected: every command gets an evidence record regardless of success. Do not repair failures in this step.

- [ ] **Step 4: Issue the Stage 0 status.**

Write `audit-results/readiness/<candidate-id>/stage-0-decision.md` with exactly `BASELINE PINNED` if capture is complete, or `BASELINE FAILED` if any required input cannot be pinned. This is not a testnet verdict.

- [ ] **Step 5: Commit only after explicit request.**

```bash
rtk git add tools/readiness config/readiness audit-results/readiness/<candidate-id>/stage-0-decision.md
rtk git commit -m "chore: pin readiness candidate evidence

Co-Authored-By: Claude <noreply@anthropic.com>"
```

### Task 2: Gate the Controlled Private Engineering Network

**Files:**
- Modify: `config/readiness/private-engineering.toml`
- Modify: `config/readiness/private-release-allowlist.toml`
- Create: `Docs/guide/private-engineering-testnet.md`
- Create: `Docs/runbooks/private-testnet-halt-reset.md`
- Create: `Protocol_Core/qanto-node/tests/private_release_profile.rs`
- Test: `Protocol_Core/qanto-node/tests/private_release_profile.rs`

**Interfaces:**
- Consumes: Workstream 1 manifest and allowlist.
- Produces: resettable non-value-bearing release profile and negative reachability proof.

- [ ] **Step 1: Write failing profile tests from the child plan.**

The tests must prove loopback/private API binding by default; fixed dedicated chain/network identity; deterministic committed genesis; no bridge, QUSD, AMM, sale, EVMCall, dynamic sharding, DPoS/PoSe validity, heterogeneous mining, or SAGA/ΩMEGA consensus path; and explicit fixed-difficulty containment.

- [ ] **Step 2: Run the profile test and capture its failure.**

```bash
rtk cargo nextest run -p qanto --test private_release_profile --locked
```

Expected before containment: `FAIL`, with each reachable excluded capability named.

- [ ] **Step 3: Apply only non-policy containment changes described by Workstream 1.**

Do not change active consensus semantics. Disable release reachability through compile-time allowlists, startup rejection, transaction-kind denial, and private operator binding.

- [ ] **Step 4: Re-run the profile and exact candidate gates.**

```bash
rtk cargo nextest run -p qanto --test private_release_profile --locked
rtk bash tools/readiness/run-evidence.sh --candidate "$QANTO_CANDIDATE_ID" --gate private-profile
```

Expected: all containment assertions pass. A build/test/runtime failure keeps the private gate `FAIL`.

- [ ] **Step 5: Rehearse halt and full reset locally.**

Follow `Docs/runbooks/private-testnet-halt-reset.md` against disposable local data. Record old/new genesis commitments, process-stop confirmation, deleted disposable path, restored node health, and explicit history-loss warning. Never use real keys or non-disposable data.

- [ ] **Step 6: Issue one private-stage verdict.**

Write one of these exact strings to `stage-1-decision.md`: `PRIVATE TESTNET READY`, `PRIVATE TESTNET READY WITH BLOCKERS`, or `NOT SAFE FOR PRIVATE TESTNET`. List blocker evidence IDs below the verdict. Do not deploy or publish.

### Task 3: Freeze V2 Protocol Schemas and Activation Boundaries

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-canonical-v2.md`
- Create: `Protocol_Core/qanto-consensus-primitives/src/consensus_parameters.rs`
- Create: `Protocol_Core/qanto-consensus-primitives/src/v2_records.rs`
- Modify: `Protocol_Core/qanto-consensus-primitives/src/lib.rs`
- Modify: `Protocol_Core/qanto-node/src/transaction.rs`
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs`
- Modify: `Protocol_Core/qanto-core/src/protocol_identity.rs`
- Create: `Docs/protocol/qanto-v2-records.md`
- Create: `Docs/protocol/proposals/QIP-002-v2-protocol-boundary.md`

**Interfaces:**
- Produces: `ProtocolVersion`, `SchemaVersion`, `ConsensusParametersV2`, canonical transaction/block/PoW/Merkle schemas, V2-only identity checks, activation predicate, golden vectors.
- Consumed by: Workstreams 3–8.

- [ ] **Step 1: Execute all schema and golden-vector tests in the child plan.**

Expected: initial runtime-reachability tests fail wherever legacy JSON, `bincode`, host-endian layout, legacy identity, or alternate hash paths remain.

- [ ] **Step 2: Implement shared primitives behind an unreachable V2 chain boundary.**

No V1 behavior changes. No public chain ID is selected or activated.

- [ ] **Step 3: Prove all V2 creation and validation paths use the same bytes.**

Run the child plan’s miner/node/replay/sync/tooling parity tests and golden-vector tests.

- [ ] **Step 4: Stop at the approval boundary.**

QIP-002 remains `REQUIRES PROTOCOL APPROVAL`. Work may continue against test-only V2 fixtures, but no release profile may activate V2 until approved.

### Task 4: Implement Parent-Scoped State and Atomic Recovery

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-branch-state-persistence.md`
- Create: `Protocol_Core/qanto-node/src/consensus/state_transition.rs`
- Create: `Protocol_Core/qanto-node/src/consensus/fork_choice.rs`
- Create: `Protocol_Core/qanto-node/src/consensus/reorg.rs`
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs`
- Modify: `Protocol_Core/qanto-node/src/persistence.rs`
- Modify: `Protocol_Core/qanto-core/src/qanto_storage.rs`
- Create: `Docs/protocol/proposals/QIP-004-branch-state-and-fork-choice.md`

**Interfaces:**
- Consumes: Workstream 2 canonical block/transaction/state-root schemas.
- Produces: `BranchStateView`, `StateDelta`, `UndoDelta`, `ValidatedBlockTransition`, `ChainScore`, atomic commit/recovery and deterministic reorg APIs.

- [ ] **Step 1: Execute child-plan tests for arrival-order independence and apply/unapply identity.**

Expected before implementation: failures demonstrate mutation of shared state, missing undo data, or non-atomic persistence.

- [ ] **Step 2: Implement pure validation and transition construction.**

Every acceptance path must construct a `ValidatedBlockTransition`; no path may mutate canonical state during validation.

- [ ] **Step 3: Implement one atomic durable batch and recovery verifier.**

Inject crashes at each boundary. Recovery must expose either the complete old state or complete new state.

- [ ] **Step 4: Implement deterministic cumulative-work fork choice behind V2.**

Canonical block ID is the exact equal-work tie-break. One consensus parent remains the launch proposal; multi-parent paths remain unreachable.

- [ ] **Step 5: Keep QIP-004 unactivated pending approval.**

Run all tests with V2 test fixtures, but do not change the active chain.

### Task 5: Implement Certificate Finality Behind an Approval Gate

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-finality-certificates.md`
- Create: `Protocol_Core/qanto-consensus-primitives/src/finality.rs`
- Create: `Protocol_Core/qanto-node/src/consensus/finality.rs`
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs`
- Modify: `Protocol_Core/qanto-node/src/persistence.rs`
- Create: `Docs/protocol/proposals/QIP-005-certificate-finality.md`

**Interfaces:**
- Consumes: Workstreams 2–3 identities, block IDs, branch state, persistence.
- Produces: canonical `ValidatorVoteV2`, `FinalityCertificateV2`, `EquivocationEvidenceV2`, `ValidatorSetCommitment`, certificate verification, finalized checkpoint, transition rules.

- [ ] **Step 1: Execute child-plan negative and threshold tests.**

Cover malformed signatures, wrong chain/version/epoch/round/height/block/set, duplicate weight, self-declared signer sets, exactly two-thirds versus greater-than-two-thirds, conflicting certificates, and restart/sync verification.

- [ ] **Step 2: Implement certificate verification and durable finalized checkpoints.**

No slashing or automatic economic penalty is activated. Equivocation evidence is detected and portable.

- [ ] **Step 3: Prove finalized history cannot reorganize.**

Run fork-choice/reorg tests attempting reorganizations at, below, and above the checkpoint.

- [ ] **Step 4: Stop before activation.**

QIP-005 remains `REQUIRES PROTOCOL APPROVAL`; finality safety assumptions require external specialist review.

### Task 6: Implement Deterministic Economics and Genesis Accounting

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-economics-genesis.md`
- Modify: `Protocol_Core/qanto-node/src/emission.rs`
- Modify: `Protocol_Core/qanto-node/src/gas_fee_model.rs`
- Modify: `Protocol_Core/qanto-node/src/absolute_genesis.rs`
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs`
- Modify: `Protocol_Core/qanto-node/src/saga.rs`
- Modify: `Protocol_Core/qanto-node/src/omega.rs`
- Create: `Protocol_Core/qanto-node/src/consensus/economics.rs`
- Create: `Docs/protocol/proposals/QIP-003-emission-fees-vesting.md`

**Interfaces:**
- Consumes: Workstreams 2–3 canonical records and state transitions.
- Produces: `EconomicStateV2`, `EconomicDeltaV2`, deterministic subsidy/fee/burn/vesting functions, supply replay proof, analytics isolation.

- [ ] **Step 1: Preserve private V1 full-prefunded fee-only containment.**

Do not present it as final tokenomics. Add regression tests proving no V2 issuance activates on the private profile.

- [ ] **Step 2: Execute child-plan supply, fee, burn, vesting, reorg, and replay tests.**

Use integer arithmetic and explicit rounding. Every arithmetic error fails closed.

- [ ] **Step 3: Implement V2 economics behind a new-genesis boundary.**

Transferred fees do not increase supply. Enforce `after = before + subsidy - burn`; reorg undo restores every cumulative counter.

- [ ] **Step 4: Prove SAGA/ΩMEGA cannot affect consensus-visible economics.**

Run negative dependency/reachability tests and differential executions with different analytics outputs.

- [ ] **Step 5: Stop before policy activation.**

Maximum supply, allocation, subsidy, terminal behavior, base-fee adjustment, burns, fee distribution, block resource target, vesting, penalties, and all rounding remain unapproved until QIP-003 is approved.

### Task 7: Implement Parent-Derived Difficulty and Canonical PoW

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-difficulty-pow.md`
- Replace/quarantine: `Protocol_Core/qanto-node/src/difficulty_retarget.rs`
- Create: `Protocol_Core/qanto-consensus-primitives/src/difficulty.rs`
- Modify: `Protocol_Core/qanto-node/src/miner.rs`
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs`
- Modify: candidate-producing modules listed in the child plan
- Create: `Docs/protocol/proposals/QIP-006-difficulty-timing-pow.md`

**Interfaces:**
- Consumes: Workstreams 2–3 header schemas, consensus parameters, fork choice.
- Produces: pure `required_difficulty`, exact `target_from_difficulty`, exact `work_from_target`, MTP/future-drift validation, miner/validator parity vectors.

- [ ] **Step 1: Remove process-global difficulty from V2 authority.**

The existing `Arc<RwLock<DifficultyState>>` and locally accumulated solve times may not determine V2 validity.

- [ ] **Step 2: Execute child-plan restart, reorg, boundary, overflow, zero-time, timestamp-manipulation, and backend-parity tests.**

Expected before replacement: tests expose branch/restart dependence and repeated-byte target approximation.

- [ ] **Step 3: Implement pure wide-integer arithmetic behind V2.**

All candidate builders call the same normative primitive. GPU/OpenCL/CUDA remain excluded unless bit-exact parity is proven.

- [ ] **Step 4: Measure before selecting timing.**

The repository’s 31 ms, 1 s, and 600 s values are contradictory. Record propagation/validation p99, then propose target, window, future drift, retarget bounds, and activation in QIP-006.

- [ ] **Step 5: Stop before activation.**

Private Stage 1 remains pinned fixed difficulty. Public adaptive rules remain unreachable until timing and QIP-006 receive approval.

### Task 8: Harden Transport, Admission, APIs, Eclipse Resistance, and Sync

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-network-api-sync.md`
- Create: `Protocol_Core/qanto-consensus-primitives/src/network.rs`
- Modify: `Protocol_Core/qanto-node/src/p2p.rs`
- Modify: `Protocol_Core/qanto-node/src/protocol/peer_scoring.rs`
- Modify: `Protocol_Core/qanto-node/src/mempool.rs`
- Modify: `Protocol_Core/qanto-node/src/node.rs`
- Modify: `Protocol_Core/qanto-rpc/src/server.rs`
- Modify: `Protocol_Core/qanto-node/src/websocket_server.rs`
- Create: `Protocol_Core/qanto-node/src/protocol/state_sync.rs`
- Create: `Docs/protocol/proposals/QIP-007-transport-and-state-sync.md`

**Interfaces:**
- Consumes: Workstreams 2–7 canonical objects, finalized checkpoints, state roots.
- Produces: signed/bounded `NetworkEnvelopeV2`, centralized transaction admission, quotas, peer diversity/scoring, authenticated resumable snapshot sync.

- [ ] **Step 1: Execute child-plan bypass and resource-bound tests.**

Every RPC/P2P/batch path must reach one admission API. Test oversized frames, decompression bombs, queue/map limits, request deadlines, expensive API limits, subscription limits, and consensus starvation.

- [ ] **Step 2: Implement authenticated domain-separated envelopes.**

Messages commit version, chain, domain, schema, sender identity, payload hash, nonce/sequence, and bounded validity.

- [ ] **Step 3: Implement behavior-based peer scoring and diversity controls.**

Measure eclipse and amplification behavior in authorized local simulations; do not claim Internet-scale resistance from unit tests.

- [ ] **Step 4: Implement signed checkpoint/snapshot manifests and resumable bounded chunks.**

Snapshot installation is atomic; the joining node remains non-serving/non-mining until certificate, chunks, state root, supply, validators, and replay pass.

- [ ] **Step 5: Stop at policy/deployment boundaries.**

Transport format and public auth policy remain in QIP-007 pending approval. No public endpoint or external infrastructure is changed.

### Task 9: Prove Optional Features Are Unreachable

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-optional-feature-quarantine.md`
- Create: `Protocol_Core/qanto-node/src/release_profile.rs`
- Modify: `Protocol_Core/qanto-node/src/lib.rs`
- Modify: `Protocol_Core/qanto-node/src/transaction.rs`
- Modify: `Protocol_Core/qanto-node/src/node.rs`
- Modify: `Protocol_Core/qanto-node/Cargo.toml`
- Create: `Protocol_Core/qanto-node/tests/release_reachability.rs`
- Create: `Docs/security/release-capability-matrix.md`

**Interfaces:**
- Consumes: Workstream 1 allowlist and Workstream 2 protocol/version boundary.
- Produces: compile-time/runtime release profile and negative reachability evidence.

- [ ] **Step 1: Execute child-plan source, binary, startup, transaction, RPC, P2P, and documentation reachability tests.**

Each excluded capability must fail at more than a configuration toggle.

- [ ] **Step 2: Implement a typed allowlist and fail-closed startup validation.**

Unknown or contradictory release settings are errors. Excluded transaction kinds and RPC methods return stable unsupported errors before expensive work.

- [ ] **Step 3: Build and inspect only allowlisted artifacts.**

Run locked release builds; record binary symbols/routes/features and prove mock bridge/ZK or experimental consensus paths cannot be reached.

- [ ] **Step 4: Re-run after Workstream 8.**

Any new path introduced by networking or sync invalidates prior reachability evidence until rerun.

### Task 10: Produce Verification, Performance, Storage, and Economic Evidence

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-verification-evidence.md`
- Create/modify: property tests, fuzz targets, crash harness, chaos scenarios, benchmark harness, modeling scripts and schemas named in the child plan
- Create: `audit-results/readiness/<candidate-id>/verification/`

**Interfaces:**
- Consumes: frozen candidate from Workstreams 1–9.
- Produces: reproducible correctness, robustness, capacity, growth, amplification, sustainability, and model evidence.

- [ ] **Step 1: Run deterministic/property suites.**

Cover canonical encoding, identity, replay, UTXO/supply conservation, apply/unapply, arrival-order independence, finality floor, difficulty replay, genesis, and fail-closed features.

- [ ] **Step 2: Run versioned fuzz targets and retain all corpora/crashes.**

Record tool version, seed, duration/iterations, candidate, feature set, and host. A crash is a failed gate until triaged and rerun.

- [ ] **Step 3: Run crash/failure injection and local chaos.**

Cover process kill, disk full/slow/corrupt, partial transition, partitions, latency/loss/duplication, clock skew, churn, validator loss, malformed blocks, Byzantine votes, and corrupted snapshots.

- [ ] **Step 4: Repair the benchmark harness before trusting results.**

The current harness measures submissions and contains fallback counts; it may only report accepted/executed/durable/finalized throughput after cross-checking canonical counters and durable state.

- [ ] **Step 5: Run sustained and resource-bound experiments.**

Report p50/p95/p99, CPU, RAM, disk, bandwidth, errors, topology, workload sizes, duration, and finality assumptions. Keep local and WAN-class evidence distinct.

- [ ] **Step 6: Generate storage, bandwidth, spam, fair-launch, validator/miner, treasury, and commercial-service models.**

Measured inputs remain `MEASURED`; explicit extrapolations are `ESTIMATED`; legal conclusions are `NEEDS EXTERNAL REVIEW`.

### Task 11: Rehearse Operations, Obtain Independent Review, and Issue the Verdict

**Files:**
- Execute: `docs/superpowers/plans/2026-08-13-qanto-operations-audit-verdict.md`
- Create/modify: hardened container, SBOM/provenance scripts, immutable config bundle, observability, backup/restore/rollback/incident runbooks named in the child plan
- Create: `audit-results/readiness/<candidate-id>/independent-review/`
- Create: `Docs/audits/2026-08-13-testnet-readiness/MASTER_A_Z_REPORT.md`
- Create: `Docs/audits/2026-08-13-testnet-readiness/SCORECARD.md`
- Create: `Docs/audits/2026-08-13-testnet-readiness/MANDATORY_ANSWERS.md`
- Create: `Docs/audits/2026-08-13-testnet-readiness/READINESS_DECISION.md`

**Interfaces:**
- Consumes: every workstream’s immutable evidence IDs.
- Produces: reproducible deployment/recovery packet, externally completed review packet, final A–AP report, complete scorecard, all 45 answers, and exactly one allowed verdict.

- [ ] **Step 1: Build the minimal reproducible artifact twice and compare it.**

Record binary/image hashes, SBOM, provenance, non-root/read-only settings, ports, resources, configuration hashes, and absence of embedded secrets or mutable startup downloads.

- [ ] **Step 2: Run operator recovery and fault rehearsals.**

Known local operators rehearse bootstrap, key provisioning, rotation/revocation with disposable keys, backup, restore, snapshot, corruption recovery, replacement, partition, emergency halt, rollback, and reset.

- [ ] **Step 3: Prepare—not fabricate—the independent review packet.**

Include clean-source reproduction steps, candidate manifest, threat model, open findings, P0/P1 falsification checklist, evidence index, and reviewer attestation schema. Mark the gate `BLOCKED` until real independent reviewers and another operator complete it.

- [ ] **Step 4: Reconcile every mandate section A through AP.**

Every claim cites an evidence ID; every unknown is labeled; every historical contradiction is resolved against the exact candidate.

- [ ] **Step 5: Complete every score and mandatory answer.**

Give reasons for each score below 90. Answer all 45 questions directly, including negative answers and blockers.

- [ ] **Step 6: Apply the decision rule and write exactly one verdict.**

Use only the vocabulary allowed by the controlling mandate and stage. A favorable verdict is not required. Missing public independent review prevents `PUBLIC TESTNET READY`.

- [ ] **Step 7: Run final consistency checks.**

```bash
rtk bash tools/readiness/validate-evidence.sh --candidate "$QANTO_CANDIDATE_ID"
rtk bash tools/readiness/validate-report.sh Docs/audits/2026-08-13-testnet-readiness
rtk graphify update .
rtk git diff --check
rtk git status --short
```

Expected: schemas and evidence links validate; report contains A–AP, all 45 answers, one verdict, and no unsupported pass. `git status` may remain dirty, but every change must be attributed.

---

## Program Completion Rule

This plan is complete only when all ten child plans have an evidence-backed terminal status, every reachable audit domain has been examined, safely repairable P0/P1 implementation defects are fixed and regression-tested, protocol-policy proposals remain separate from unapproved activation, blocked experiments are recorded honestly, an independent second pass has actually occurred where required, and the final report issues exactly one evidence-backed verdict.
