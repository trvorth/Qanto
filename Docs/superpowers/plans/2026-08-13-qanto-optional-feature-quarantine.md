# Qanto Optional Feature Quarantine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Systematically quarantine all experimental/optional features that are not part of the V2 canonical consensus. Each quarantined feature is moved behind an explicit feature gate, documented with its status, and verified to be unreachable in the private engineering profile. The quarantine list includes: SAGA (advisory-only), ΩMEGA (simulation-only), heterogeneous mining backends, mutable ZK circuits, EVMCall, Bridge/IBC, QUSD/AMM, dynamic sharding, multi-parent DAG, DPoS/PoSe consensus, sale/minting.

**Architecture:** Create `Protocol_Core/qanto-node/src/quarantine.rs` as the central registry. Each quarantined module is:
1. Moved under `#[cfg(feature = "quarantine-<name>")]`
2. Documented in `QUARANTINE_REGISTRY.md` with: status, reason, activation requirement, test coverage
3. Verified unreachable in `private-engineering` profile (no feature enabled)
4. Attack tests referencing quarantined features are updated or moved to quarantine test suite

**Tech Stack:** Rust 2021, Cargo feature gates, `cargo hack` for feature matrix testing, `nextest`.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- Private engineering profile (`cargo build --profile private-engineering`) must compile with ZERO quarantine features enabled.
- No quarantined code paths in V1 consensus, block validation, transaction processing, or mining.
- Each quarantined feature has a corresponding QIP or explicit "NOT PLANNED" status.
- Attack tests (attack_1–12) that test quarantined vulnerabilities are either:
  - Remediated and moved to regression tests (if fixed in V2 canonical), OR
  - Moved to `quarantine_tests.rs` under `#[cfg(feature = "quarantine-<name>")]` (if testing quarantined behavior)
- QIP-001 through QIP-005 define the canonical boundary; everything else is quarantined.

---

### Quarantine Registry (Source of Truth)

| Feature | Module(s) | Status | Activation Requirement | QIP |
|---------|-----------|--------|------------------------|-----|
| SAGA AI | `saga.rs`, `saga_ai.rs`, `palletsaga.rs` | Advisory-only | Requires QIP for consensus integration | — |
| ΩMEGA | `omega.rs`, `omega_enhanced.rs`, `omega_test` binary | Simulation-only | Requires QIP for consensus integration | — |
| Heterogeneous Mining | `miner.rs` GPU/OpenCL/CUDA paths | Quarantined | Must match `qanhash_v2` bit-exactly | QIP-005 |
| Mutable ZK Circuits | `zkp.rs`, `zk_sequencer.rs` | Quarantined | Requires trusted setup ceremony doc | — |
| EVMCall | `transaction.rs` kind=2, bypass in `verify_signature` | Excluded from private | Requires QIP for EVM integration | — |
| Bridge/IBC | `interoperability.rs`, `quantum_bridge.rs` | Quarantined | Requires QIP for bridge design | — |
| QUSD/AMM | `gas_fee_model.rs`, `qds.rs` | Quarantined | Requires QIP for stablecoin design | — |
| Dynamic Sharding | `sharding.rs`, `infinite_strata_node.rs` | Quarantined | Requires QIP for sharding design | — |
| Multi-parent DAG | `qantodag.rs` multi-parent logic | Incomplete | Requires QIP-004 finality | QIP-004 |
| DPoS/PoSe Consensus | `consensus.rs` PoS/PoSe paths | Inactive | Requires QIP for PoS design | — |
| Sale/Minting | `emission.rs` V2 unreachable, minting logic | Quarantined | Requires QIP-003 economics | QIP-003 |
| Adaptive Difficulty | `difficulty_retarget.rs` | **Quarantined (this plan)** | Replaced by V2 retarget | QIP-005 |
| Legacy V1 Encoding | `transaction.rs` bincode, `qantodag.rs` textual Merkle | Legacy | Replaced by V2 canonical | QIP-002 |

---

### Task 1: Create Quarantine Registry and Feature Gates

**Files:**
- Create: `Protocol_Core/qanto-node/src/quarantine.rs`
- Create: `Protocol_Core/qanto-node/QUARANTINE_REGISTRY.md`
- Modify: `Protocol_Core/qanto-node/Cargo.toml` (add quarantine features)
- Modify: `Protocol_Core/qanto-node/src/lib.rs` (conditional exports)

**Interfaces:**
- Produces: `QUARANTINE_FEATURES` list, compile-time verification of quarantine.

- [ ] **Step 1: Write failing quarantine test.**

Create `Protocol_Core/qanto-node/tests/quarantine_registry.rs`:

```rust
#[test]
fn quarantine_registry_exists_and_complete() {
    // Verify quarantine.rs exports all quarantined modules
    // Verify QUARANTINE_REGISTRY.md has entry for each
}

#[test]
fn private_engineering_profile_excludes_all_quarantine_features() {
    // This is a compile-test: private profile must not enable any quarantine feature
    // Verified by CI: cargo build --profile private-engineering
}

#[test]
fn each_quarantined_module_compiles_only_when_feature_enabled() {
    // For each quarantine feature, verify module only compiles when that feature is on
}
```

- [ ] **Step 2: Create `Cargo.toml` quarantine features.**

Modify `Protocol_Core/qanto-node/Cargo.toml`:

```toml
[features]
default = []
private-engineering = []  # No quarantine features
# Quarantine features (explicitly opt-in)
quarantine-saga = []
quarantine-omega = []
quarantine-heterogeneous-mining = []
quarantine-mutable-zk = []
quarantine-evmcall = []
quarantine-bridge = []
quarantine-qusd-amm = []
quarantine-dynamic-sharding = []
quarantine-multiparent-dag = []
quarantine-dpos-pose = []
quarantine-sale-minting = []
quarantine-adaptive-difficulty = []
quarantine-legacy-v1 = []
# Meta feature for testing all quarantine
quarantine-all = [
    "quarantine-saga", "quarantine-omega", "quarantine-heterogeneous-mining",
    "quarantine-mutable-zk", "quarantine-evmcall", "quarantine-bridge",
    "quarantine-qusd-amm", "quarantine-dynamic-sharding", "quarantine-multiparent-dag",
    "quarantine-dpos-pose", "quarantine-sale-minting", "quarantine-adaptive-difficulty",
    "quarantine-legacy-v1",
]
```

- [ ] **Step 3: Implement `quarantine.rs` as registry.**

Create `Protocol_Core/qanto-node/src/quarantine.rs`:

```rust
//! Central quarantine registry. Each module is conditionally compiled.
//! Private engineering profile compiles with NO quarantine features.

// SAGA - advisory scoring only, not consensus
#[cfg(feature = "quarantine-saga")]
pub mod saga;

// ΩMEGA - threat simulation only, not consensus
#[cfg(feature = "quarantine-omega")]
pub mod omega;
#[cfg(feature = "quarantine-omega")]
pub mod omega_enhanced;

// Heterogeneous mining backends - must match qanhash_v2
#[cfg(feature = "quarantine-heterogeneous-mining")]
pub mod adaptive_mining;
#[cfg(feature = "quarantine-heterogeneous-mining")]
pub mod deterministic_mining;

// Mutable ZK circuits - no trusted setup ceremony
#[cfg(feature = "quarantine-mutable-zk")]
pub mod zkp;
#[cfg(feature = "quarantine-mutable-zk")]
pub mod zk_sequencer;

// EVMCall - bypasses PQ signatures, excluded from private
#[cfg(feature = "quarantine-evmcall")]
pub mod evm_integration; // if exists

// Bridge/IBC - mock verification
#[cfg(feature = "quarantine-bridge")]
pub mod interoperability;
#[cfg(feature = "quarantine-bridge")]
pub mod quantum_bridge;

// QUSD/AMM - unaudited economic design
#[cfg(feature = "quarantine-qusd-amm")]
pub mod gas_fee_model;
#[cfg(feature = "quarantine-qusd-amm")]
pub mod qds;

// Dynamic sharding - incomplete
#[cfg(feature = "quarantine-dynamic-sharding")]
pub mod sharding;
#[cfg(feature = "quarantine-dynamic-sharding")]
pub mod infinite_strata_node;

// Multi-parent DAG - incomplete finality
#[cfg(feature = "quarantine-multiparent-dag")]
pub mod qantodag_multi_parent; // if split

// DPoS/PoSe - not implemented
#[cfg(feature = "quarantine-dpos-pose")]
pub mod consensus_pos;

// Sale/minting - V2 emission unreachable
#[cfg(feature = "quarantine-sale-minting")]
pub mod emission_v2_inactive; // if split

// Adaptive difficulty - UNSAFE, replaced by V2 retarget
#[cfg(feature = "quarantine-adaptive-difficulty")]
pub mod difficulty_retarget;

// Legacy V1 encoding - bincode, textual Merkle
#[cfg(feature = "quarantine-legacy-v1")]
pub mod transaction_legacy;
#[cfg(feature = "quarantine-legacy-v1")]
pub mod qantodag_legacy;

/// Compile-time assertion: private profile has no quarantine features.
#[cfg(all(profile = "private-engineering", any(
    feature = "quarantine-saga",
    feature = "quarantine-omega",
    feature = "quarantine-heterogeneous-mining",
    feature = "quarantine-mutable-zk",
    feature = "quarantine-evmcall",
    feature = "quarantine-bridge",
    feature = "quarantine-qusd-amm",
    feature = "quarantine-dynamic-sharding",
    feature = "quarantine-multiparent-dag",
    feature = "quarantine-dpos-pose",
    feature = "quarantine-sale-minting",
    feature = "quarantine-adaptive-difficulty",
    feature = "quarantine-legacy-v1",
))]
compile_error!("Private engineering profile must not enable any quarantine features!");
```

- [ ] **Step 4: Create `QUARANTINE_REGISTRY.md`.**

Create `Protocol_Core/qanto-node/QUARANTINE_REGISTRY.md`:

```markdown
# Quarantine Registry

Each entry documents a feature that is NOT part of the V2 canonical consensus.
Format: Feature | Module(s) | Status | Reason | Activation Requirement | QIP | Test Coverage
```

List all 14 features from the table above with details.

- [ ] **Step 5: Update `lib.rs` to use quarantine gates.**

Modify `Protocol_Core/qanto-node/src/lib.rs`:
- Wrap existing module exports with `#[cfg(feature = "quarantine-<name>")]`
- Keep V1 legacy path as default (no feature) but mark clearly as legacy
- V2 canonical path is always compiled (no feature gate)

- [ ] **Step 6: Run quarantine registry tests.**

```bash
rtk cargo test -p qanto-node --locked -- quarantine_registry --features quarantine-all
rtk cargo build --profile private-engineering --locked
```

Expected: `PASS` (private profile builds with no quarantine features).

- [ ] **Step 7: Record intended commit.**

Intended commit: `feat(qanto-node): quarantine registry with 14 feature gates and compile-time enforcement`.

---

### Task 2: Relocate Attack Tests to Quarantine or Regression

**Files:**
- Modify: `Protocol_Core/qanto-node/src/lib.rs` (attack test modules)
- Create: `Protocol_Core/qanto-node/src/quarantine_tests.rs`
- Modify: `Protocol_Core/qanto-node/src/attack_tests.rs` (regression tests)

**Interfaces:**
- Attack tests proving V2 fixes → regression tests (always compiled)
- Attack tests testing quarantined behavior → `quarantine_tests.rs` under feature gates

- [ ] **Step 1: Categorize each attack test.**

| Attack Test | Vulnerability | Status | Action |
|-------------|---------------|--------|--------|
| attack_1_single_parent_dag_safety_violation | No finality | Fixed by V2 finality (WS4) | Regression test |
| attack_2_liveness_failure_fixed_difficulty | Fixed difficulty | Fixed by V2 retarget (WS6) | Regression test |
| attack_3_finality_absence_reorg_any_depth | No finality | Fixed by V2 finality (WS4) | Regression test |
| attack_4_evm_raw_tx_bypass_pq | EVMCall bypass | EVMCall quarantined (WS8) | Quarantine test |
| attack_5_bridge_claim_forged_signature | Bridge mock | Bridge quarantined (WS8) | Quarantine test |
| attack_6_rpc_ws_unauthenticated | Unauth RPC | Fixed by V2 auth (WS7) | Regression test |
| attack_7_p2p_eclipse_fixed_topics | P2P eclipse | Fixed by V2 auth (WS7) | Regression test |
| attack_8_miner_reservation_griefing | Miner griefing | Fixed by V2 retarget (WS6) | Regression test |
| attack_9_consensus_lock_bottleneck | Lock contention | Fixed by V2 architecture | Regression test |
| attack_10_blocks_map_performance | Perf | Fixed by V2 storage (WS3) | Regression test |
| attack_11_saga_f64_nondeterminism | SAGA nondeterminism | SAGA quarantined (WS8) | Quarantine test |
| attack_12_omega_threat_manipulation | ΩMEGA simulation | ΩMEGA quarantined (WS8) | Quarantine test |

- [ ] **Step 2: Create `quarantine_tests.rs`.**

Move attack_4, attack_5, attack_11, attack_12 to `quarantine_tests.rs` under appropriate feature gates:

```rust
#[cfg(all(test, feature = "quarantine-evmcall"))]
pub mod attack_4_evm_raw_tx_bypass_pq;

#[cfg(all(test, feature = "quarantine-bridge"))]
pub mod attack_5_bridge_claim_forged_signature;

#[cfg(all(test, feature = "quarantine-saga"))]
pub mod attack_11_saga_f64_nondeterminism;

#[cfg(all(test, feature = "quarantine-omega"))]
pub mod attack_12_omega_threat_manipulation;
```

- [ ] **Step 3: Update `attack_tests.rs` as regression suite.**

Keep attack_1, 2, 3, 6, 7, 8, 9, 10 as regression tests (always compiled, test V2 fixes):

```rust
// These test that V2 canonical consensus FIXES the vulnerability
pub mod attack_1_single_parent_dag_safety_violation; // Tests finality prevents reorg
pub mod attack_2_liveness_failure_fixed_difficulty;  // Tests V2 retarget prevents stall
pub mod attack_3_finality_absence_reorg_any_depth;   // Tests finality certificates
pub mod attack_6_rpc_ws_unauthenticated;             // Tests V2 gRPC auth
pub mod attack_7_p2p_eclipse_fixed_topics;           // Tests V2 P2P auth
pub mod attack_8_miner_reservation_griefing;         // Tests V2 retarget bounds
pub mod attack_9_consensus_lock_bottleneck;          // Tests lock-free V2 paths
pub mod attack_10_blocks_map_performance;            // Tests V2 storage perf
```

- [ ] **Step 4: Run attack test matrix.**

```bash
# Private profile (no quarantine): regression tests only
rtk cargo nextest run -p qanto-node --locked --profile private-engineering -- attack_

# Full quarantine: all attack tests
rtk cargo nextest run -p qanto-node --locked --features quarantine-all -- attack_
```

Expected: All tests pass in their respective profiles.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(qanto-node): relocate attack tests to quarantine/regression per V2 canonical boundary`.

---

### Task 3: Verify V2 Canonical Path Has No Quarantined Dependencies

**Files:**
- Test: `Protocol_Core/qanto-node/tests/canonical_purity.rs`
- Script: `scripts/verify_canonical_purity.sh`

**Interfaces:**
- Produces: Proof that V2 consensus path (validation, mining, sync) does not depend on any quarantined module.

- [ ] **Step 1: Write canonical purity test.**

Create `Protocol_Core/qanto-node/tests/canonical_purity.rs`:

```rust
#[test]
fn v2_consensus_validation_imports_no_quarantine() {
    // Use cargo tree or compile-test to verify:
    // qanto_node::consensus does not depend on any quarantine-* feature
}

#[test]
fn v2_miner_imports_no_quarantine() {
    // qanto_node::miner uses only qanhash_v2, work_target_v2
}

#[test]
fn v2_block_producer_imports_no_quarantine() {
    // Uses only V2 canonical types
}

#[test]
fn v2_state_sync_imports_no_quarantine() {
    // Uses only StateStoreV2, StateProofV2
}

#[test]
fn v2_p2p_imports_no_quarantine() {
    // Uses only P2pEnvelopeV2, MessageType
}
```

- [ ] **Step 2: Implement verification script.**

Create `scripts/verify_canonical_purity.sh`:

```bash
#!/bin/bash
set -e
# Build with private-engineering profile (no quarantine features)
cargo build --profile private-engineering --locked -p qanto-node 2>&1 | grep -E "quarantine|saga|omega|evm|bridge|qusd|sharding|dpos|adaptive" && echo "FAIL: quarantine dependency found" || echo "PASS: no quarantine deps in private profile"

# Verify V2 types are used
cargo tree -p qanto-node --profile private-engineering -i qanto_consensus_primitives | grep -E "v2|canonical"
```

- [ ] **Step 3: Run purity verification.**

```bash
bash scripts/verify_canonical_purity.sh
rtk cargo nextest run -p qanto-node --locked --profile private-engineering -- canonical_purity
```

Expected: `PASS`.

- [ ] **Step 4: Record intended commit.**

Intended commit: `test(qanto-node): verify V2 canonical path purity (no quarantined deps)`.

---

### Task 4: Quarantine Documentation and Final Review

**Files:**
- Create: `Docs/quarantine/QUARANTINE_SUMMARY.md`
- Modify: `Docs/protocol/proposals/QIP-001-identity-crypto.md` (reference quarantine)
- Modify: `Docs/protocol/proposals/QIP-002-v2-protocol-boundary.md` (reference quarantine)
- Modify: `Docs/protocol/proposals/QIP-003-economics-genesis.md` (reference quarantine)
- Modify: `Docs/protocol/proposals/QIP-004-branch-state-finality.md` (reference quarantine)
- Modify: `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md` (reference quarantine)

**Interfaces:**
- Central quarantine documentation for auditors and future activations.

- [ ] **Step 1: Create `QUARANTINE_SUMMARY.md`.**

Create `Docs/quarantine/QUARANTINE_SUMMARY.md`:

```markdown
# Qanto Optional Feature Quarantine Summary

## Purpose
All features not in the V2 canonical consensus are quarantined behind explicit feature gates.
The private engineering testnet runs with ZERO quarantine features enabled.

## Registry
See `Protocol_Core/qanto-node/QUARANTINE_REGISTRY.md` for the complete table.

## Activation Process
To activate a quarantined feature:
1. Write a QIP (if not existing) with full spec, security analysis, test plan
2. Get human approval: REQUIRES PROTOCOL APPROVAL
3. Move feature from quarantine to canonical (new V2 schema version if consensus-visible)
4. Update activation height in ConsensusParametersV2
5. Run full regression + new feature test suite
6. Deploy to private testnet for validation
7. Only then consider public testnet

## Current Status
- 14 features quarantined
- 8 attack tests moved to quarantine test suite
- 8 attack tests remain as V2 regression tests
- Private profile builds clean with no quarantine features
- V2 canonical path verified pure (no quarantined dependencies)
```

- [ ] **Step 2: Update QIPs to reference quarantine.**

Each QIP-001 through QIP-005 should note which quarantined features they supersede or relate to.

- [ ] **Step 3: Run full workspace build in private profile.**

```bash
rtk cargo build --workspace --profile private-engineering --locked
```

Expected: `PASS`.

- [ ] **Step 4: Final self-review checklist.**

- [ ] All 14 quarantined features have feature gates in `Cargo.toml`.
- [ ] `quarantine.rs` conditionally exports each module.
- [ ] `QUARANTINE_REGISTRY.md` documents all 14 with status/reason/QIP.
- [ ] Compile-time assertion: private profile cannot enable quarantine features.
- [ ] Attack tests categorized: 8 regression (V2 fix verified), 4 quarantine (V1 behavior).
- [ ] `quarantine_tests.rs` compiles only with `quarantine-all` feature.
- [ ] `attack_tests.rs` always compiles (regression suite for V2).
- [ ] V2 canonical path (consensus, miner, block_producer, state_sync, p2p) has zero quarantined deps.
- [ ] Purity verification script passes.
- [ ] `QUARANTINE_SUMMARY.md` created for auditors.
- [ ] All QIPs reference quarantine status.
- [ ] Workspace builds in private profile: `cargo build --profile private-engineering --locked`.
- [ ] All tests pass in both profiles.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `docs: quarantine summary; complete optional feature quarantine`.

---

## Acceptance Criteria

- [ ] 14 features quarantined behind explicit Cargo features.
- [ ] Private engineering profile (`--profile private-engineering`) compiles with ZERO quarantine features enabled.
- [ ] Compile-time error if any quarantine feature enabled in private profile.
- [ ] `QUARANTINE_REGISTRY.md` documents all features with status, reason, QIP reference.
- [ ] Attack tests: 8 regression tests (always compiled, test V2 fixes), 4 quarantine tests (only under feature gates).
- [ ] V2 canonical consensus path verified pure: no dependency on any quarantined module.
- [ ] Central quarantine documentation for auditors (`QUARANTINE_SUMMARY.md`).
- [ ] All QIPs reference quarantine status.
- [ ] Workspace builds clean in private profile.
- [ ] All tests pass in both private and quarantine-all profiles.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 8 Summary for Dependent Workstreams

| Output | Location | Consumers |
|---|---|---|
| Quarantine feature gates | `qanto-node/Cargo.toml` | Workstreams 9, 10 |
| `quarantine.rs` registry | `qanto-node/src/quarantine.rs` | Workstreams 9, 10 |
| `QUARANTINE_REGISTRY.md` | `qanto-node/QUARANTINE_REGISTRY.md` | Workstreams 9, 10 |
| Attack test relocation | `qanto-node/src/quarantine_tests.rs`, `attack_tests.rs` | Workstreams 9, 10 |
| Canonical purity verification | `qanto-node/tests/canonical_purity.rs` | Workstreams 9, 10 |
| `QUARANTINE_SUMMARY.md` | `Docs/quarantine/QUARANTINE_SUMMARY.md` | Workstreams 9, 10 |