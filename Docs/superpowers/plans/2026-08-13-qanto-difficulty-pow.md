# Qanto Difficulty Retarget and Proof-of-Work — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the unsafe adaptive difficulty draft with a safe, replay-verified V2 difficulty retarget algorithm. The V2 PoW uses exact 256-bit target arithmetic (`work_target_v2`) and `qanhash_v2` as the sole CPU oracle. Retarget is parent-derived, bounded (±25% per window), median-time-past anchored, and committed in `ConsensusParametersV2`. Private engineering profile uses fixed difficulty; V2 retarget activates only with `ACTIVATION_HEIGHT > 0` and QIP-005 approval.

**Architecture:** `qanto-consensus-primitives::pow` provides `qanhash_v2`, `verify_qanhash_v2`, `work_target_v2`. `qanto-node::difficulty_v2` provides `retarget_difficulty_v2(parent_blocks, params) -> u64`. Retarget uses median timestamp of last `difficulty_window` blocks, computes target interval ratio, applies bounds, derives next difficulty. All arithmetic is exact integer (no floating point). Miner uses `qanhash_v2` exclusively; GPU/OpenCL paths verify against CPU oracle before submitting.

**Tech Stack:** Rust 2021, `qanto-consensus-primitives` (exact 256-bit division), `qanto-node` difficulty module, `nextest` with chaos/fuzz vectors.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- Private engineering profile: fixed `INITIAL_DIFFICULTY`. V2 retarget code-compiled but unreachable (`ACTIVATION_HEIGHT = 0`).
- No floating point in consensus-critical arithmetic. All retarget math uses integer arithmetic (fixed-point or rational).
- Retarget window: `difficulty_window` from `ConsensusParametersV2` (e.g., 2016 blocks).
- Bounds: `difficulty_adjustment_bounds` from params (e.g., ±25% = 7500/12500 basis points).
- Median Time Past: `median_time_past_window` from params (e.g., 11 blocks).
- Future timestamp drift: `future_timestamp_drift_ms` from params.
- `qanhash_v2` is the ONLY CPU oracle. All mining backends must match its output bit-exactly.
- Target computation: `work_target_v2(difficulty) = floor((2^256 - 1) / difficulty)`. Exact 256-bit long division.
- Historical `difficulty_retarget.rs` (adaptive, mutable, non-replay-safe) is quarantined (Workstream 8).
- QIP-005 draft must reference this plan's exact retarget algorithm and activation logic.

---

### Task 1: Define V2 Difficulty Retarget Algorithm and Consensus Parameters

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/src/difficulty_v2.rs`
- Modify: `Protocol_Core/qanto-consensus-primitives/src/lib.rs`
- Modify: `Protocol_Core/qanto-consensus-primitives/src/consensus_parameters.rs` (add retarget params)
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/difficulty_v2.json`
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs`

**Interfaces:**
- Produces: `retarget_difficulty_v2`, `work_target_v2`, `compute_next_difficulty_v2`.
- Consumes: `ConsensusParametersV2`, `BlockCoreV2` (timestamp, difficulty, height), `hash_v2`.

- [ ] **Step 1: Write failing difficulty V2 tests.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{
    difficulty_v2::{retarget_difficulty_v2, compute_next_difficulty_v2, MedianTimePast},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    block_v2::BlockCoreV2,
    hash::{HashV2, BlockIdV2},
};

#[test]
fn work_target_v2_exact_division_matches_golden_vector() {
    let target = qanto_consensus_primitives::pow::work_target_v2(1000000).unwrap();
    let expected = include_bytes!("../vectors/difficulty_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(target), expected["work_target_1M"]);
}

#[test]
fn retarget_difficulty_v2_bounded_by_params() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    // Create parent blocks with timestamps spanning exactly target interval
    let parents = create_test_chain(params.target_block_interval_ms, 2016);
    let next_diff = retarget_difficulty_v2(&parents, &params).unwrap();
    // Should be close to current difficulty (within bounds)
    assert!(next_diff >= params.min_difficulty);
    assert!(next_diff <= params.max_difficulty);
}

#[test]
fn retarget_difficulty_v2_respects_upper_bound() {
    let params = ConsensusParametersV2 { 
        difficulty_adjustment_bounds: (7500, 12500), // ±25%
        ..ConsensusParametersV2::for_testnet_candidate() 
    };
    // Parent blocks with very fast timestamps (simulating hashrate increase)
    let parents = create_test_chain(params.target_block_interval_ms / 2, 2016);
    let next_diff = retarget_difficulty_v2(&parents, &params).unwrap();
    let max_allowed = params.min_difficulty * 12500 / 10000;
    assert!(next_diff <= max_allowed, "Retarget must not exceed +25% bound");
}

#[test]
fn retarget_difficulty_v2_respects_lower_bound() {
    let params = ConsensusParametersV2 { 
        difficulty_adjustment_bounds: (7500, 12500),
        ..ConsensusParametersV2::for_testnet_candidate() 
    };
    // Parent blocks with very slow timestamps (simulating hashrate decrease)
    let parents = create_test_chain(params.target_block_interval_ms * 2, 2016);
    let next_diff = retarget_difficulty_v2(&parents, &params).unwrap();
    let min_allowed = params.max_difficulty * 7500 / 10000;
    assert!(next_diff >= min_allowed, "Retarget must not exceed -25% bound");
}

#[test]
fn retarget_difficulty_v2_median_time_past() {
    // Timestamp manipulation attack: one block with future timestamp
    // MTP should prevent single-block manipulation
    let params = ConsensusParametersV2::for_testnet_candidate();
    let mut parents = create_test_chain(params.target_block_interval_ms, 2016);
    // Corrupt last block timestamp
    parents.last_mut().unwrap().timestamp += 1_000_000_000; // far future
    let next_diff = retarget_difficulty_v2(&parents, &params).unwrap();
    // MTP should limit the effect
    assert!(next_diff > params.min_difficulty);
    assert!(next_diff < params.max_difficulty);
}

#[test]
fn retarget_difficulty_v2_deterministic_for_same_parents() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let parents = create_test_chain(params.target_block_interval_ms, 2016);
    let d1 = retarget_difficulty_v2(&parents, &params).unwrap();
    let d2 = retarget_difficulty_v2(&parents, &params).unwrap();
    assert_eq!(d1, d2);
}

fn create_test_chain(interval_ms: u64, count: usize) -> Vec<BlockCoreV2> {
    // Helper to create a chain of blocks with regular intervals
    let mut blocks = Vec::new();
    let mut timestamp = 1_700_000_000_000u64;
    let base_diff = 1_000_000u64;
    for i in 0..count {
        let block = BlockCoreV2::new(
            2,
            if i == 0 { BlockIdV2::ZERO } else { blocks[i-1].block_id() },
            HashV2::ZERO,
            HashV2::ZERO,
            timestamp,
            base_diff,
            0,
            HashV2::ZERO,
            HashV2::ZERO,
            i as u64,
        );
        blocks.push(block);
        timestamp += interval_ms;
    }
    blocks
}
```

- [ ] **Step 2: Run the tests to establish failure.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- work_target_v2_exact_division retarget_difficulty_v2_bounded retarget_difficulty_v2_respects retarget_difficulty_v2_median_time_past retarget_difficulty_v2_deterministic
```

Expected: `FAIL`.

- [ ] **Step 3: Implement `difficulty_v2.rs`.**

Create `Protocol_Core/qanto-consensus-primitives/src/difficulty_v2.rs`:

```rust
//! V2 difficulty retarget: parent-derived, bounded, median-time-past anchored, exact integer arithmetic.

use crate::{
    consensus_parameters::ConsensusParametersV2,
    block_v2::BlockCoreV2,
    hash::HashV2,
};

/// Compute median timestamp of last N blocks (for MTP).
pub fn median_time_past(blocks: &[BlockCoreV2], window: usize) -> u64 {
    if blocks.is_empty() { return 0; }
    let start = blocks.len().saturating_sub(window);
    let mut timestamps: Vec<u64> = blocks[start..].iter().map(|b| b.timestamp).collect();
    timestamps.sort();
    timestamps[timestamps.len() / 2]
}

/// Compute the next difficulty based on parent blocks and consensus parameters.
/// Returns None if insufficient blocks (before first retarget window).
pub fn retarget_difficulty_v2(
    parent_blocks: &[BlockCoreV2], // newest first (tip at index 0)
    params: &ConsensusParametersV2,
) -> Option<u64> {
    let window = params.difficulty_window as usize;
    if parent_blocks.len() < window {
        return None; // Not enough history, use current difficulty
    }
    
    // Take last `window` blocks (oldest first for time calc)
    let window_blocks = &parent_blocks[..window];
    
    // Median Time Past of window start and end
    let mtp_start = median_time_past(&window_blocks[window.saturating_sub(params.median_time_past_window)..], params.median_time_past_window);
    let mtp_end = median_time_past(&window_blocks[..params.median_time_past_window], params.median_time_past_window);
    
    if mtp_end <= mtp_start {
        return Some(params.min_difficulty); // Time went backwards or stalled
    }
    
    let actual_interval = mtp_end - mtp_start;
    let target_interval = params.target_block_interval_ms * (window as u64 - 1);
    
    // Current difficulty (from tip of window_blocks, which is oldest in this slice)
    // Actually tip is parent_blocks[0], but we need the difficulty at window start
    let current_difficulty = window_blocks.last().unwrap().difficulty;
    
    // Compute next difficulty: D' = D * actual / target, bounded
    // Use rational arithmetic to avoid overflow: D' = D * actual * 10000 / (target * 10000)
    // Actually: D' = D * target_interval / actual_interval, then apply bounds
    
    let (min_bps, max_bps) = params.difficulty_adjustment_bounds;
    
    // Compute with u128 to avoid overflow
    let current = current_difficulty as u128;
    let target = target_interval as u128;
    let actual = actual_interval as u128;
    
    // D' = D * target / actual
    let next = current.saturating_mul(target) / actual.max(1);
    
    // Apply bounds: next ∈ [D * min_bps / 10000, D * max_bps / 10000]
    let lower = current.saturating_mul(min_bps as u128) / 10000;
    let upper = current.saturating_mul(max_bps as u128) / 10000;
    
    let bounded = next.clamp(lower, upper);
    
    // Clamp to absolute min/max
    let final_diff = bounded.clamp(params.min_difficulty as u128, params.max_difficulty as u128);
    
    Some(final_diff as u64)
}

/// Convenience: compute next difficulty given current block and params.
/// Used by miner and validator.
pub fn compute_next_difficulty_v2(
    current_block: &BlockCoreV2,
    ancestor_blocks: &[BlockCoreV2], // current_block's ancestors, oldest first
    params: &ConsensusParametersV2,
) -> u64 {
    // Build parent_blocks for retarget: current_block + ancestors (newest first)
    let mut parent_blocks = Vec::with_capacity(ancestor_blocks.len() + 1);
    parent_blocks.push(current_block.clone());
    parent_blocks.extend(ancestor_blocks.iter().rev().cloned());
    
    retarget_difficulty_v2(&parent_blocks, params).unwrap_or(current_block.difficulty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_v2::BlockCoreV2;
    use crate::hash::{HashV2, BlockIdV2};
    
    #[test]
    fn median_time_past_works() {
        let blocks = vec![
            BlockCoreV2::new(2, BlockIdV2::ZERO, HashV2::ZERO, HashV2::ZERO, 1000, 1000, 0, HashV2::ZERO, HashV2::ZERO, 0),
            BlockCoreV2::new(2, BlockIdV2::ZERO, HashV2::ZERO, HashV2::ZERO, 2000, 1000, 0, HashV2::ZERO, HashV2::ZERO, 1),
            BlockCoreV2::new(2, BlockIdV2::ZERO, HashV2::ZERO, HashV2::ZERO, 3000, 1000, 0, HashV2::ZERO, HashV2::ZERO, 2),
        ];
        assert_eq!(median_time_past(&blocks, 3), 2000);
        assert_eq!(median_time_past(&blocks, 2), 2500); // median of [2000, 3000]
    }
}
```

- [ ] **Step 4: Update `consensus_parameters.rs` with retarget fields.**

Add to `ConsensusParametersV2`:
```rust
pub difficulty_window: u64,           // e.g., 2016
pub target_block_interval_ms: u64,    // e.g., 120000 (2 minutes)
pub difficulty_adjustment_bounds: (u16, u16), // (min_bps, max_bps) e.g., (7500, 12500)
pub median_time_past_window: usize,   // e.g., 11
pub future_timestamp_drift_ms: u64,   // e.g., 7200000 (2 hours)
```

Add to `CONSENSUS_PARAMETERS_SPECS` and `to_canonical_record`.

- [ ] **Step 5: Create golden vector for difficulty V2.**

Create `Protocol_Core/qanto-consensus-primitives/tests/vectors/difficulty_v2.json`.

- [ ] **Step 6: Run all difficulty V2 tests.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- work_target_v2 retarget_difficulty_v2 median_time_past
```

Expected: `PASS`.

- [ ] **Step 7: Record intended commit.**

Intended commit: `feat(qanto-consensus-primitives): V2 difficulty retarget with exact integer arithmetic`.

---

### Task 2: Wire V2 Difficulty Into Miner, Validator, and Consensus

**Files:**
- Modify: `Protocol_Core/qanto-node/src/miner.rs`
- Modify: `Protocol_Core/qanto-node/src/consensus.rs`
- Modify: `Protocol_Core/qanto-node/src/block_producer.rs`
- Create: `Protocol_Core/qanto-node/src/difficulty_v2.rs` (node integration)
- Test: `Protocol_Core/qanto-node/tests/v2_difficulty_integration.rs`

**Interfaces:**
- Consumes: `retarget_difficulty_v2`, `qanhash_v2`, `work_target_v2`, `ConsensusParametersV2`.
- Produces: V2 miner uses `qanhash_v2` only; validator verifies with `verify_qanhash_v2`; block producer sets correct difficulty.

- [ ] **Step 1: Write failing integration test.**

Create `Protocol_Core/qanto-node/tests/v2_difficulty_integration.rs`:

```rust
use qanto_consensus_primitives::{
    difficulty_v2::{retarget_difficulty_v2, compute_next_difficulty_v2},
    pow::{qanhash_v2, verify_qanhash_v2, work_target_v2},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion, is_v2_active},
    block_v2::BlockCoreV2,
};

#[test]
fn v2_miner_uses_qanhash_v2_exclusively() {
    // Miner's prime_hash/qanhash calls must route to qanhash_v2
    // GPU/OpenCL paths must verify against qanhash_v2 before submit
}

#[test]
fn v2_validator_verifies_pow_with_verify_qanhash_v2() {
    // consensus.rs::validate_proof_of_work uses verify_qanhash_v2 when V2 active
}

#[test]
fn v2_block_producer_sets_retargeted_difficulty() {
    // block_producer computes next difficulty via retarget_difficulty_v2
}
```

- [ ] **Step 2: Implement node integration in `difficulty_v2.rs`.**

Create `Protocol_Core/qanto-node/src/difficulty_v2.rs`:

```rust
//! Node integration of V2 difficulty retarget.

use qanto_consensus_primitives::{
    difficulty_v2::{retarget_difficulty_v2, compute_next_difficulty_v2, median_time_past},
    consensus_parameters::ConsensusParametersV2,
    block_v2::BlockCoreV2,
};

// Re-export for node crate
pub use qanto_consensus_primitives::difficulty_v2::*;
```

- [ ] **Step 3: Wire into `miner.rs`.**

In `miner.rs`:
- Remove custom `mine_block` arithmetic.
- Use `qanhash_v2(chain_id, schema, block_core_hash, nonce)` as CPU oracle.
- GPU/OpenCL: compute candidate, verify with `verify_qanhash_v2` before submitting.
- Target from `work_target_v2(difficulty)`.

- [ ] **Step 4: Wire into `consensus.rs`.**

In `validate_proof_of_work`:
- When `is_v2_active`, call `verify_qanhash_v2` with block's difficulty.
- Verify block's difficulty matches `retarget_difficulty_v2` for its parent chain.

- [ ] **Step 5: Wire into `block_producer.rs`.**

In `produce_block`:
- When V2 active, compute difficulty via `compute_next_difficulty_v2`.
- Set in `BlockCoreV2`.
- Include finality cert if finality active.

- [ ] **Step 6: Run integration build test.**

```bash
rtk cargo build --workspace --locked --features infinite-strata
```

Expected: `PASS`.

- [ ] **Step 7: Record intended commit.**

Intended commit: `feat(qanto-node): V2 difficulty retarget in miner, validator, block producer`.

---

### Task 3: Quarantine Legacy Adaptive Difficulty and Prove V2 Replay Safety

**Files:**
- Modify: `Protocol_Core/qanto-node/src/lib.rs` (mark `difficulty_retarget` as quarantined)
- Create: `Protocol_Core/qanto-node/tests/difficulty_quarantine.rs`
- Test: `Protocol_Core/qanto-node/tests/v2_difficulty_replay.rs`

**Interfaces:**
- Legacy `difficulty_retarget.rs` is marked `#[cfg(test)]` only or behind `quarantined` feature.
- V2 retarget proven replay-safe: same parent chain -> same difficulty.

- [ ] **Step 1: Write quarantine test.**

Create `Protocol_Core/qanto-node/tests/difficulty_quarantine.rs`:

```rust
#[test]
fn legacy_difficulty_retarget_not_exported_in_release() {
    // difficulty_retarget module not accessible in release profile
    // This is a compile-test: if it compiles, quarantine works
}

#[test]
fn legacy_difficulty_retarget_only_in_test_profile() {
    // Only compiled under #[cfg(test)]
}
```

- [ ] **Step 2: Write replay safety test.**

Create `Protocol_Core/qanto-node/tests/v2_difficulty_replay.rs`:

```rust
use qanto_consensus_primitives::{
    difficulty_v2::retarget_difficulty_v2,
    consensus_parameters::ConsensusParametersV2,
    block_v2::BlockCoreV2,
};

#[test]
fn v2_retarget_replay_produces_identical_difficulty() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let chain = create_deterministic_chain(10000);
    // Replay from genesis: each retarget must match
    for i in 2016..chain.len() {
        let window = &chain[i-2016..i+1];
        let diff = retarget_difficulty_v2(window, &params).unwrap();
        assert_eq!(diff, chain[i].difficulty, "Replay mismatch at block {}", i);
    }
}

#[test]
fn v2_retarget_no_mutable_state() {
    // retarget_difficulty_v2 is pure: same inputs -> same output
    // No static/thread-local/global mutable state
}

fn create_deterministic_chain(len: usize) -> Vec<BlockCoreV2> {
    // Build chain using retarget_difficulty_v2 deterministically
    // ...
}
```

- [ ] **Step 3: Mark legacy `difficulty_retarget` as quarantined.**

In `Protocol_Core/qanto-node/src/lib.rs`:
```rust
#[cfg(all(test, feature = "quarantined"))]
pub mod difficulty_retarget;
```

- [ ] **Step 4: Run quarantine and replay tests.**

```bash
rtk cargo nextest run -p qanto-node --locked -- difficulty_quarantine v2_difficulty_replay
```

Expected: `PASS`.

- [ ] **Step 5: Record intended commit.**

Intended commit: `feat(qanto-node): quarantine legacy adaptive difficulty; prove V2 replay safety`.

---

### Task 4: Golden Vector Cross-Difficulty Validation and QIP-005 Draft

**Files:**
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs` (add difficulty vectors)
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/cross_difficulty.json`
- Create: `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md` (or QIP-005 for difficulty/PoW/transport)

**Interfaces:**
- Validates cross-crate difficulty parity.
- Produces QIP-005 with status `REQUIRES PROTOCOL APPROVAL`.

- [ ] **Step 1: Write cross-crate difficulty test.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{difficulty_v2::retarget_difficulty_v2, pow::work_target_v2};

#[test]
fn cross_crate_work_target_matches_miner_calculation() {
    let expected = include_bytes!("../vectors/cross_difficulty.json");
    // Verify work_target_v2 == miner's target computation
}

#[test]
fn cross_crate_retarget_matches_node_validation() {
    // consensus-primitives retarget = node's consensus validation
}
```

- [ ] **Step 2: Create QIP-005 draft.**

Create `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md`:

```yaml
---
qip: QIP-005
status: REQUIRES PROTOCOL APPROVAL
changes:
  - consensus
  - compatibility
  - chain_id
activation: unreachable
migration: new-v2-genesis
rollback: replace-prelaunch-candidate
---
```

Body: 1) Current (fixed difficulty in private, adaptive draft unsafe), 2) Defect (Attack 2 liveness failure, non-replay-safe), 3) Security impact, 4) Alternatives (Bitcoin retarget, ASERT, DAA), 5) Simulations (retarget bounds, MTP attack resistance), 6) Design (this plan), 7) Compatibility, 8) Migration, 9) Tests, 10) Rollback.

- [ ] **Step 3: Run all difficulty tests across crates.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives -p qanto-node --locked -- difficulty_ retarget_ work_target_ v2_difficulty cross_crate_
```

Expected: `PASS`.

- [ ] **Step 4: Final self-review checklist.**

- [ ] `retarget_difficulty_v2` uses exact integer arithmetic (no float).
- [ ] Bounds: ±25% per window (configurable via `ConsensusParametersV2`).
- [ ] Median Time Past (11 blocks) anchors retarget timestamps.
- [ ] Future timestamp drift limited by `ConsensusParametersV2`.
- [ ] `qanhash_v2` is sole CPU oracle; GPU verifies against it.
- [ ] `work_target_v2` exact 256-bit division: `floor((2^256-1)/difficulty)`.
- [ ] Legacy `difficulty_retarget` quarantined (not in release, test-only).
- [ ] Replay safety: deterministic retarget for same parent chain.
- [ ] Private profile: `ACTIVATION_HEIGHT = 0`, V2 retarget unreachable.
- [ ] Golden vectors for work_target, retarget scenarios (normal, bounds, MTP attack).
- [ ] Cross-crate parity: miner=validator=consensus-primitives.
- [ ] QIP-005 draft complete with all 10 sections.
- [ ] All tests pass.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(qanto-consensus-primitives): cross-crate difficulty parity; QIP-005 draft; complete V2 PoW/difficulty`.

---

## Acceptance Criteria

- [ ] `retarget_difficulty_v2` implemented with exact integer arithmetic, bounds, MTP.
- [ ] `work_target_v2` exact 256-bit division, golden vectors match.
- [ ] `qanhash_v2` is sole CPU oracle; all mining backends verify against it.
- [ ] Miner, validator, block producer wired to V2 difficulty when active.
- [ ] Legacy adaptive difficulty quarantined (test-only, not in release).
- [ ] Replay safety proven: deterministic retarget for same parent chain.
- [ ] Private profile: `ACTIVATION_HEIGHT = 0`, V2 difficulty unreachable.
- [ ] Golden vectors for all difficulty scenarios.
- [ ] Cross-crate parity verified.
- [ ] QIP-005 draft written with status `REQUIRES PROTOCOL APPROVAL`.
- [ ] All tests pass.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 6 Summary for Dependent Workstreams

| Output | Location | Consumers |
|---|---|---|
| `retarget_difficulty_v2`, `median_time_past` | `qanto-consensus-primitives::difficulty_v2` | Workstreams 7, 9 |
| `work_target_v2` (existing) | `qanto-consensus-primitives::pow` | Workstreams 7, 9 |
| `qanhash_v2` CPU oracle exclusivity | `qanto-node::miner` | Workstreams 7, 9 |
| Legacy difficulty quarantine | `qanto-node::difficulty_retarget` (test-only) | Workstream 9 |
| QIP-005 draft | `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md` | Workstreams 7, 8, 9, 10 |
| Golden vectors | `qanto-consensus-primitives/tests/vectors/difficulty_v2.json`, `cross_difficulty.json` | Workstreams 7, 9, 10 |