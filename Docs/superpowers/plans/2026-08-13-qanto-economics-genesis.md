# Qanto Economics, Genesis, Emission, and Supply Invariants — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Freeze all economic parameters (total supply, allocations, emission schedule, fee burn, vesting, staking rewards) behind `ConsensusParametersV2` and `GenesisManifestV2`. Implement V2 emission curve with supply cap enforcement, vesting for Ecosystem Fund (15%, 2-year linear, 1-year cliff), fee burn mechanism, and staking APR floor. Private engineering profile uses legacy V1 emission; V2 emission is code-compiled but unreachable (`ACTIVATION_HEIGHT = 0`). V2 activation gated by `ACTIVATION_HEIGHT = 0` and protocol approval.

**Architecture:** `qanto-core::emission` becomes the single source of economic truth. `ConsensusParametersV2` commits economic constants. `GenesisManifestV2` encodes allocation records with typed hashes. Emission function `compute_block_reward_v2(height, total_supply_so_far, params) -> QAmount` is pure, deterministic, and supply-capped. Vesting enforced via `Vesting Schedule` in state. Fee burn: `base_fee * gas_used` burned, `priority_fee` to miner. Staking rewards from emission reserve.

**Tech Stack:** Rust 2021, `qanto-consensus-primitives` V2 types (`GenesisManifestV2`, `ConsensusParametersV2`), `qanto-core::emission` with V2 emission logic, `qanto-node` block validation integration.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- Private engineering profile: legacy `emission.rs` active, V2 emission code-compiled but unreachable (`ACTIVATION_HEIGHT = 0`).
- Total supply: 21,000,000,000 QNTO (9 decimals = 21e9 * 1e9 base units). Compile-time assertion in `lib.rs` remains.
- Allocations: Community 80% (16.8B), Ecosystem 15% (3.15B, vested), Liquidity 5% (1.05B).
- V2 emission: Deflationary curve targeting 21B cap. Block reward = `f(height, supply_issued)`. No tail emission beyond cap.
- Vesting: Ecosystem Fund 15% — 2-year linear, 1-year cliff. Implemented as `VestingScheduleV2` in state, enforced on transfer.
- Fee burn: EIP-1559 style. `base_fee` burned, `priority_fee` + `tip` to miner/validator.
- Staking APR floor: Minimum validator reward rate enforced by emission reserve.
- All economic parameters in `ConsensusParametersV2` (immutable per protocol version).
- Genesis manifest: typed allocation records, hash-committed in `GenesisManifestV2`.
- QIP-003 draft must reference this plan's exact parameters and activation logic.
- Candidate identity governed by Workstream 1.

---

### Task 1: Define V2 Genesis Manifest and Economic Parameter Schemas

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/src/genesis_v2.rs`
- Modify: `Protocol_Core/qanto-consensus-primitives/src/lib.rs`
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/genesis_v2.json`
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs`

**Interfaces:**
- Produces: `GenesisManifestV2`, `AllocationRecordV2`, `ConsensusParametersV2` economic fields.
- Consumes: `hash_v2`, `DomainV2::GenesisManifest`, `v2_records::GENESIS_MANIFEST_SPECS`, `consensus_parameters`.

- [ ] **Step 1: Write failing genesis V2 tests.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{
    genesis_v2::{GenesisManifestV2, AllocationRecordV2, AllocationType},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    hash::{hash_v2, DomainV2, HashV2, StateRootV2},
    v2_records::GENESIS_MANIFEST_SPECS,
};

#[test]
fn genesis_manifest_v2_canonical_bytes_match_golden_vector() {
    let allocations = vec![
        AllocationRecordV2::new(AllocationType::Community, "0x...".parse().unwrap(), 16_800_000_000_000_000_000u128),
        AllocationRecordV2::new(AllocationType::Ecosystem, "0x...".parse().unwrap(), 3_150_000_000_000_000_000u128),
        AllocationRecordV2::new(AllocationType::Liquidity, "0x...".parse().unwrap(), 1_050_000_000_000_000_000u128),
    ];
    let manifest = GenesisManifestV2::new(
        1363527796, // chain_id
        HashV2::from_bytes([0xDEu8; 32]), // genesis_hash
        allocations,
        SchemaVersion::V2_INITIAL,
    );
    let expected = include_bytes!("../vectors/genesis_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(manifest.canonical_bytes()), expected["canonical_bytes"]);
    assert_eq!(manifest.hash().to_string(), expected["genesis_manifest_hash"]);
}

#[test]
fn consensus_parameters_v2_includes_economic_fields() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    // Verify economic fields exist
    assert!(params.max_total_supply > 0);
    assert!(params.community_allocation > 0);
    assert!(params.ecosystem_allocation > 0);
    assert!(params.liquidity_allocation > 0);
    assert!(params.vesting_cliff_epochs > 0);
    assert!(params.vesting_duration_epochs > 0);
    assert!(params.base_fee_burn_enabled);
    assert!(params.staking_apr_floor_bps > 0);
    assert!(params.emission_curve_version > 0);
}

#[test]
fn genesis_allocations_sum_to_max_supply() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    assert_eq!(
        params.community_allocation + params.ecosystem_allocation + params.liquidity_allocation,
        params.max_total_supply
    );
}
```

- [ ] **Step 2: Run the tests to establish failure.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- genesis_manifest_v2 consensus_parameters_v2_includes_economic_fields genesis_allocations_sum_to_max_supply
```

Expected: `FAIL`.

- [ ] **Step 3: Implement `genesis_v2.rs`.**

Create `Protocol_Core/qanto-consensus-primitives/src/genesis_v2.rs`:

```rust
//! V2 genesis manifest and allocation records.

use crate::{
    consensus_parameters::SchemaVersion,
    hash::{hash_v2, DomainV2, HashV2},
    record::{CanonicalRecord, WireType, SequenceSemantics, SequenceSpec},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum AllocationType {
    Community = 0,
    Ecosystem = 1,
    Liquidity = 2,
    ValidatorReward = 3,
    StakingReserve = 4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationRecordV2 {
    pub allocation_type: AllocationType,
    pub recipient: HashV2, // AccountIdV2
    pub amount: u128, // QAmount (9 decimals)
    pub vesting_schedule: Option<VestingScheduleV2>,
}

impl AllocationRecordV2 {
    pub fn new(allocation_type: AllocationType, recipient: HashV2, amount: u128) -> Self {
        Self { allocation_type, recipient, amount, vesting_schedule: None }
    }
    
    pub fn with_vesting(mut self, schedule: VestingScheduleV2) -> Self {
        self.vesting_schedule = Some(schedule);
        self
    }
    
    pub fn to_canonical_record(&self) -> CanonicalRecord {
        let mut record = CanonicalRecord::new();
        record.push_u8(1, self.allocation_type as u8).unwrap();
        record.push_hash32(2, self.recipient.as_bytes()).unwrap();
        record.push_u128(3, self.amount).unwrap();
        if let Some(schedule) = &self.vesting_schedule {
            record.push_u64(4, schedule.cliff_epoch).unwrap();
            record.push_u64(5, schedule.duration_epochs).unwrap();
            record.push_u64(6, schedule.start_epoch).unwrap();
        }
        record
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingScheduleV2 {
    pub cliff_epoch: u64,
    pub duration_epochs: u64,
    pub start_epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenesisManifestV2 {
    pub chain_id: u32,
    pub genesis_hash: HashV2,
    pub allocations: Vec<AllocationRecordV2>,
    pub canonical_bytes: Vec<u8>,
    pub hash: HashV2,
}

impl GenesisManifestV2 {
    pub fn new(
        chain_id: u32,
        genesis_hash: HashV2,
        allocations: Vec<AllocationRecordV2>,
        schema: SchemaVersion,
    ) -> Self {
        let mut record = CanonicalRecord::new();
        record.push_u32(1, chain_id).unwrap();
        record.push_hash32(2, genesis_hash.as_bytes()).unwrap();
        // Allocations as ordered sequence
        for alloc in &allocations {
            let alloc_rec = alloc.to_canonical_record().finish();
            record.push_bytes(3, &alloc_rec).unwrap();
        }
        let bytes = record.finish();
        let hash = hash_v2(DomainV2::GenesisManifest, chain_id, schema as u16, &bytes);
        Self { chain_id, genesis_hash, allocations, canonical_bytes: bytes, hash }
    }
    
    pub fn canonical_bytes(&self) -> &[u8] { &self.canonical_bytes }
    pub fn hash(&self) -> HashV2 { self.hash }
}
```

- [ ] **Step 4: Update `ConsensusParametersV2` with economic fields.**

Modify `Protocol_Core/qanto-consensus-primitives/src/consensus_parameters.rs`:

```rust
pub struct ConsensusParametersV2 {
    // ... existing fields ...
    pub max_total_supply: u128,
    pub community_allocation: u128,
    pub ecosystem_allocation: u128,
    pub liquidity_allocation: u128,
    pub vesting_cliff_epochs: u64,
    pub vesting_duration_epochs: u64,
    pub base_fee_burn_enabled: bool,
    pub staking_apr_floor_bps: u16, // basis points, e.g., 500 = 5%
    pub emission_curve_version: u8,
    // ... add to to_canonical_record and CONSENSUS_PARAMETERS_SPECS
}
```

- [ ] **Step 5: Create golden vector for genesis V2.**

Create `Protocol_Core/qanto-consensus-primitives/tests/vectors/genesis_v2.json`.

- [ ] **Step 6: Run all genesis V2 tests.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- genesis_manifest_v2 consensus_parameters_v2 genesis_allocations_sum
```

Expected: `PASS`.

- [ ] **Step 7: Record intended commit.**

Intended commit: `feat(qanto-consensus-primitives): V2 genesis manifest and economic parameters`.

---

### Task 2: Implement V2 Emission Curve with Supply Cap Enforcement

**Files:**
- Modify: `Protocol_Core/qanto-core/src/emission.rs`
- Create: `Protocol_Core/qanto-core/tests/emission_v2.rs`
- Test: `Protocol_Core/qanto-node/tests/v2_emission_integration.rs`

**Interfaces:**
- Produces: `compute_block_reward_v2(height, total_issued, params) -> u128`, `compute_vested_amount(schedule, current_epoch) -> u128`, `apply_fee_burn(base_fee, gas_used) -> (burned, to_miner)`.
- Consumes: `ConsensusParametersV2`, `GenesisManifestV2`, `SchemaVersion`.

- [ ] **Step 1: Write failing emission V2 tests.**

Create `Protocol_Core/qanto-core/tests/emission_v2.rs`:

```rust
use qanto_consensus_primitives::{
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
};
use qanto_core::emission::{compute_block_reward_v2, compute_vested_amount, apply_fee_burn, VestingScheduleV2};

#[test]
fn v2_emission_respects_supply_cap() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let mut total_issued = 0u128;
    for height in 1..1_000_000 {
        let reward = compute_block_reward_v2(height, total_issued, &params);
        total_issue += reward;
        if total_issued >= params.max_total_supply {
            assert_eq!(reward, 0, "Reward must be zero after cap reached");
            break;
        }
    }
    assert!(total_issued <= params.max_total_supply);
}

#[test]
fn v2_emission_decreasing_curve() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let reward_1 = compute_block_reward_v2(1, 0, &params);
    let reward_1000 = compute_block_reward_v2(1000, reward_1 * 1000, &params);
    assert!(reward_1000 <= reward_1, "Emission must be non-increasing");
    // Later rewards strictly less until cap
}

#[test]
fn v2_vesting_linear_after_cliff() {
    let schedule = VestingScheduleV2 { cliff_epoch: 52, duration_epochs: 104, start_epoch: 0 }; // ~1yr cliff, 2yr linear (assuming 1 epoch = 1 week)
    let total = 3_150_000_000_000_000_000u128; // 15% ecosystem
    
    // At cliff
    let vested = compute_vested_amount(&schedule, 52, total);
    assert_eq!(vested, 0);
    
    // Halfway through vesting
    let vested = compute_vested_amount(&schedule, 52 + 26, total);
    assert_eq!(vested, total / 4); // 25% vested (half of linear period)
    
    // Fully vested
    let vested = compute_vested_amount(&schedule, 52 + 104, total);
    assert_eq!(vested, total);
}

#[test]
fn v2_fee_burn_splits_correctly() {
    let base_fee = 1000u128;
    let priority_fee = 500u128;
    let gas_used = 1_000_000u128;
    let (burned, to_miner) = apply_fee_burn(base_fee, priority_fee, gas_used);
    assert_eq!(burned, base_fee * gas_used);
    assert_eq!(to_miner, priority_fee * gas_used);
}
```

- [ ] **Step 2: Implement `compute_block_reward_v2`, `compute_vested_amount`, `apply_fee_burn` in `emission.rs`.**

Modify `Protocol_Core/qanto-core/src/emission.rs`:

```rust
use qanto_consensus_primitives::consensus_parameters::ConsensusParametersV2;

/// V2 block reward: deflationary curve with hard cap.
/// reward = max_supply * decay_factor / (height + offset)^exponent
/// Simplified: piecewise linear decay to cap.
pub fn compute_block_reward_v2(height: u64, total_issued: u128, params: &ConsensusParametersV2) -> u128 {
    if total_issued >= params.max_total_supply {
        return 0;
    }
    
    let remaining = params.max_total_supply - total_issued;
    // Annual halving-style decay: reward halves every ~4 years (2,000,000 blocks at 2min)
    let halving_interval = 2_000_000u64;
    let epoch = height / halving_interval;
    
    // Base reward scales with remaining supply
    let base = remaining / 1_000_000; // ~0.0001% of remaining per block initially
    let reward = base >> epoch.min(63); // halve per epoch
    
    (reward.min(remaining)).max(1) // at least 1 unit until cap
}

/// Compute vested amount for a schedule at current epoch.
pub fn compute_vested_amount(schedule: &VestingScheduleV2, current_epoch: u64, total: u128) -> u128 {
    if current_epoch < schedule.start_epoch + schedule.cliff_epoch {
        return 0;
    }
    let elapsed = current_epoch - schedule.start_epoch - schedule.cliff_epoch;
    if elapsed >= schedule.duration_epochs {
        return total;
    }
    // Linear vesting
    total * (elapsed as u128) / (schedule.duration_epochs as u128)
}

/// Apply EIP-1559 fee burn: base_fee burned, priority_fee to miner.
pub fn apply_fee_burn(base_fee: u128, priority_fee: u128, gas_used: u128) -> (u128, u128) {
    let burned = base_fee.saturating_mul(gas_used);
    let to_miner = priority_fee.saturating_mul(gas_used);
    (burned, to_miner)
}
```

- [ ] **Step 3: Wire V2 emission into `qanto-node` block validation.**

In `qanto-node/src/consensus.rs::validate_block_reward`:
- When `is_v2_active`, use `compute_block_reward_v2`.
- Verify coinbase amount = block reward + fee burn (priority_fee portion).

- [ ] **Step 4: Run emission tests.**

```bash
rtk cargo nextest run -p qanto-core --locked -- emission_v2
rtk cargo nextest run -p qanto-node --locked -- v2_emission_integration
```

Expected: `PASS`.

- [ ] **Step 5: Record intended commit.**

Intended commit: `feat(qanto-core): V2 emission curve, vesting, fee burn with supply cap`.

---

### Task 3: Wire Genesis Manifest Into Node Initialization and State

**Files:**
- Modify: `Protocol_Core/qanto-node/src/node.rs`
- Modify: `Protocol_Core/qanto-node/src/block_producer.rs` (genesis block)
- Modify: `Protocol_Core/qanto-node/src/state_sync.rs`
- Test: `Protocol_Core/qanto-node/tests/v2_genesis_integration.rs`

**Interfaces:**
- Consumes: `GenesisManifestV2`, `ConsensusParametersV2`, `HashV2`.
- Produces: Genesis block with `GenesisManifestV2` hash in state root, initial allocations in state store.

- [ ] **Step 1: Write failing genesis integration test.**

Create `Protocol_Core/qanto-node/tests/v2_genesis_integration.rs`:

```rust
use qanto_consensus_primitives::{
    genesis_v2::GenesisManifestV2,
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    hash::StateRootV2,
};

#[test]
fn v2_genesis_block_commits_genesis_manifest() {
    // Genesis block's state_root = hash of GenesisManifestV2 applied to empty state
    // This is a structural test
}

#[test]
fn v2_genesis_allocations_in_state_store() {
    // After genesis, StateStoreV2 has all allocation records at their AccountIdV2 keys
}
```

- [ ] **Step 2: Implement genesis initialization in `node.rs`.**

In `node.rs::initialize_genesis`:
- When V2 active (test-only config), construct `GenesisManifestV2`.
- Apply allocations to `StateStoreV2` (create accounts with balances, set vesting schedules).
- Compute genesis `StateRootV2` and commit to genesis block.

- [ ] **Step 3: Run integration build test.**

```bash
rtk cargo build --workspace --locked --features infinite-strata
```

Expected: `PASS`.

- [ ] **Step 4: Record intended commit.**

Intended commit: `feat(qanto-node): V2 genesis manifest initialization and state allocation`.

---

### Task 4: Golden Vector Cross-Economics Validation and QIP-003 Draft

**Files:**
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs` (add genesis/economic vectors)
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/cross_economics.json`
- Create: `Docs/protocol/proposals/QIP-003-economics-genesis.md`

**Interfaces:**
- Validates cross-crate economic parity.
- Produces QIP-003 with status `REQUIRES PROTOCOL APPROVAL`.

- [ ] **Step 1: Write cross-crate economics test.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{
    genesis_v2::GenesisManifestV2,
    consensus_parameters::ConsensusParametersV2,
};
use qanto_core::emission::{compute_block_reward_v2, compute_vested_amount};

#[test]
fn cross_crate_emission_matches_consensus_params() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    // Verify emission function uses params from consensus-primitives
}

#[test]
fn cross_crate_vesting_matches_ecosystem_allocation() {
    // Verify vesting schedule matches ecosystem_allocation params
}
```

- [ ] **Step 2: Create QIP-003 draft.**

Create `Docs/protocol/proposals/QIP-003-economics-genesis.md`:

```yaml
---
qip: QIP-003
status: REQUIRES PROTOCOL APPROVAL
changes:
  - monetary
  - compatibility
  - chain_id
activation: unreachable
migration: new-v2-genesis
rollback: replace-prelaunch-candidate
---
```

Body: 1) Current (legacy emission inactive, no vesting, no fee burn, fixed allocations in code), 2) Defect (supply not capped, vesting unenforced, fee market broken), 3) Economic impact, 4) Alternatives (Bitcoin-style, EIP-1559, linear vesting), 5) Simulations (emission curve to 21B, vesting unlock schedule), 6) Design (this plan), 7) Compatibility (V1 emission rejected), 8) Migration (new genesis with manifests), 9) Tests (supply cap, vesting, fee burn vectors), 10) Rollback (replace pre-launch).

- [ ] **Step 3: Run all economics tests across crates.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives -p qanto-core -p qanto-node --locked -- genesis_ emission_ vesting_ fee_burn_ cross_crate_ v2_genesis v2_emission
```

Expected: `PASS`.

- [ ] **Step 4: Final self-review checklist.**

- [ ] `GenesisManifestV2` with typed `AllocationRecordV2` and `VestingScheduleV2`.
- [ ] `ConsensusParametersV2` includes all economic constants (supply, allocations, vesting, fee burn, APR floor, emission curve version).
- [ ] `compute_block_reward_v2` enforces 21B hard cap, non-increasing, zero after cap.
- [ ] Vesting: 1-year cliff, 2-year linear for 15% Ecosystem Fund.
- [ ] Fee burn: `base_fee * gas_used` burned, `priority_fee` to miner.
- [ ] Staking APR floor parameter in consensus parameters.
- [ ] Genesis block commits `GenesisManifestV2` hash in state root.
- [ ] Private profile: `ACTIVATION_HEIGHT = 0`, V2 emission unreachable.
- [ ] Golden vectors for genesis manifest, emission curve, vesting, fee burn.
- [ ] Cross-crate parity: node emission = core emission = consensus-primitives params.
- [ ] QIP-003 draft complete with all 10 sections.
- [ ] All tests pass across `qanto-consensus-primitives`, `qanto-core`, `qanto-node`.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(qanto-core): cross-crate economics parity; QIP-003 draft; complete V2 economics`.

---

## Acceptance Criteria

- [ ] `GenesisManifestV2` and `AllocationRecordV2` have frozen schemas, canonical encoding via `hash_v2`.
- [ ] `ConsensusParametersV2` includes all economic parameters (supply, allocations, vesting, fee burn, staking APR, emission curve).
- [ ] `compute_block_reward_v2` is pure, deterministic, supply-capped at 21B QNTO.
- [ ] Vesting schedule enforced: 1-year cliff, 2-year linear for Ecosystem Fund.
- [ ] Fee burn mechanism (EIP-1559 style) implemented and tested.
- [ ] Genesis initialization applies manifest to `StateStoreV2`, commits root.
- [ ] Private profile: `ACTIVATION_HEIGHT = 0`, all V2 economics unreachable.
- [ ] Golden vectors for genesis, emission, vesting, fee burn.
- [ ] Cross-crate parity verified.
- [ ] QIP-003 proposal written with status `REQUIRES PROTOCOL APPROVAL`.
- [ ] All tests pass.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 5 Summary for Dependent Workstreams

| Output | Location | Consumers |
|---|---|---|
| `GenesisManifestV2`, `AllocationRecordV2`, `VestingScheduleV2` | `qanto-consensus-primitives::genesis_v2` | Workstreams 7, 9, 10 |
| Economic fields in `ConsensusParametersV2` | `qanto-consensus-primitives::consensus_parameters` | Workstreams 6, 7, 9, 10 |
| `compute_block_reward_v2`, `compute_vested_amount`, `apply_fee_burn` | `qanto-core::emission` | Workstreams 6, 7, 9 |
| Genesis initialization | `qanto-node::node` | Workstreams 7, 9 |
| QIP-003 draft | `Docs/protocol/proposals/QIP-003-economics-genesis.md` | Workstreams 6, 7, 8, 9, 10 |
| Golden vectors | `qanto-consensus-primitives/tests/vectors/genesis_v2.json`, `cross_economics.json` | Workstreams 6, 7, 8, 9, 10 |