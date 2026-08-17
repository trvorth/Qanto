# Qanto Verification, Evidence, Regression Tests, and Benchmarks — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the complete verification suite that proves the V2 canonical consensus is correct, performant, and economically sound. This includes: golden vector cross-crate validation, property-based testing (proptest), fuzzing (cargo-fuzz), chaos testing, performance benchmarks, economic modeling verification, and evidence bundle generation for each workstream. The output is a `verification-evidence.json` per candidate that auditors can independently verify.

**Architecture:** `qanto-consensus-primitives` is the reference oracle. Every other crate (qanto-node, qanto-core, qanto-storage, qanto-rpc, qanto-p2p) must produce bit-identical canonical bytes and hashes for all V2 types. Verification suite runs in CI for every candidate. Evidence labels: CONFIRMED, MEASURED, DERIVED, ESTIMATED, HYPOTHESIS, NEEDS EXTERNAL REVIEW.

**Tech Stack:** Rust 2021, `nextest`, `proptest`, `cargo-fuzz`, `criterion` for benchmarks, `simulacrum` for chaos, custom evidence bundle generator.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- Private engineering profile: V2 verification code compiles but V2 types unreachable (`ACTIVATION_HEIGHT = 0`).
- Every V2 type has golden vectors in `qanto-consensus-primitives/tests/vectors/*.json`.
- Cross-crate parity is MANDATORY: node = core = consensus-primitives = storage = rpc = p2p.
- Property tests cover: schema encoding/decoding roundtrip, hash domain/chain/schema separation, Merkle proofs, retarget bounds, emission supply cap, vesting linear, fee burn split.
- Fuzz targets: `CanonicalRecord::decode`, `hash_v2`, `qanhash_v2`, `verify_qanhash_v2`, P2P envelope decode, gRPC request decode.
- Chaos tests: network partition, validator churn, difficulty shock, timestamp manipulation, reorg depth.
- Benchmarks: block validation throughput, state root computation, Merkle proof generation/verification, P2P message handling, gRPC latency.
- Economic model verification: emission curve reaches 21B cap, vesting unlocks match schedule, fee burn reduces supply, staking APR floor met.
- All evidence labeled with evidence grade.

---

### Task 1: Golden Vector Cross-Crate Validation Suite

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/tests/golden_cross_crate.rs`
- Modify: `Protocol_Core/qanto-node/tests/golden_cross_crate.rs`
- Modify: `Protocol_Core/qanto-core/tests/golden_cross_crate.rs`
- Modify: `Protocol_Core/qanto-storage/tests/golden_cross_crate.rs`
- Modify: `Protocol_Core/qanto-rpc/tests/golden_cross_crate.rs`
- Script: `scripts/run_golden_validation.sh`

**Interfaces:**
- Consumes: All golden vectors from Workstreams 2–7.
- Produces: `golden_validation_report.json` with PASS/FAIL per vector per crate.

- [ ] **Step 1: Write failing cross-crate golden test.**

Create `Protocol_Core/qanto-consensus-primitives/tests/golden_cross_crate.rs`:

```rust
use qanto_consensus_primitives::{
    hash::{hash_v2, DomainV2, TxIdV2, BlockIdV2, StateRootV2, WorkHashV2, HashV2, MessageIdV2},
    transaction_v2::{TransactionBodyV2, TransactionEnvelopeV2, sign_transaction_body_v2},
    block_v2::{BlockCoreV2, build_tx_merkle_root_v2, compute_pow_work_v2, block_id_v2},
    storage_v2::{StateKeyV2, StateLeafV2, StateNodeV2, StateRootV2 as StorageStateRootV2},
    finality_v2::{FinalityVoteV2, FinalityCertificateV2, ValidatorSetV2, aggregate_votes, verify_certificate},
    net_v2::{P2pEnvelopeV2, MessageType, StateProofV2},
    genesis_v2::{GenesisManifestV2, AllocationRecordV2},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    identity_v2::{account_id_v2, validator_id_v2},
    pow::{qanhash_v2, work_target_v2, verify_qanhash_v2},
};
use qanto_core::qanto_native_crypto::KeyPair;

#[test]
fn golden_consensus_parameters_matches_all_crates() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let record = params.to_canonical_record(SchemaVersion::V2_INITIAL);
    let bytes = record.finish();
    
    // This same bytes must be produced by:
    // - qanto-node genesis init
    // - qanto-rpc GetConsensusParameters
    // - qanto-storage genesis migration
    // We verify the bytes match the golden vector
    let expected = include_bytes!("../../vectors/consensus_parameters.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(&bytes), expected["canonical_bytes"]);
}

// Similar tests for every golden vector type:
// - domain_registry
// - transaction_v2 (body, signing, envelope)
// - block_v2 (core, merkle root, work hash, block id)
// - storage_v2 (key, leaf, node, root, pruning manifest)
// - finality_v2 (vote, certificate, validator set)
// - net_v2 (envelope, state proof)
// - genesis_v2 (manifest)
// - identity_v2 (account id, validator id)
// - difficulty_v2 (work target, retarget scenarios)
```

- [ ] **Step 2: Implement cross-crate test runner script.**

Create `scripts/run_golden_validation.sh`:

```bash
#!/bin/bash
set -e

# Run golden validation for each crate
crates=(
    "qanto-consensus-primitives"
    "qanto-node"
    "qanto-core"
    "qanto-storage"
    "qanto-rpc"
)

for crate in "${crates[@]}"; do
    echo "=== Golden validation for $crate ==="
    cargo nextest run -p "$crate" --locked -- golden_cross_crate \
        --output-file "target/golden_${crate}.json"
done

# Aggregate results
cargo run --bin aggregate_golden_results -- target/golden_*.json > target/golden_validation_report.json
cat target/golden_validation_report.json
```

- [ ] **Step 3: Create aggregate binary.**

Create `Protocol_Core/tools/aggregate_golden_results/src/main.rs` to combine per-crate results.

- [ ] **Step 4: Run golden validation.**

```bash
bash scripts/run_golden_validation.sh
```

Expected: All PASS, report shows 100% parity.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(verification): golden vector cross-crate validation suite`.

---

### Task 2: Property-Based Testing Suite

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/tests/proptest_v2.rs`
- Create: `Protocol_Core/qanto-node/tests/proptest_v2.rs`
- Modify: `Protocol_Core/qanto-consensus-primitives/Cargo.toml` (add proptest dev-dep)

**Interfaces:**
- Produces: Property tests for all V2 canonical invariants.

- [ ] **Step 1: Write failing property tests.**

Create `Protocol_Core/qanto-consensus-primitives/tests/proptest_v2.rs`:

```rust
use proptest::prelude::*;
use qanto_consensus_primitives::{
    record::{CanonicalRecord, DecodedRecord, FieldSpec, WireType},
    hash::{hash_v2, DomainV2, HashV2},
    transaction_v2::TransactionBodyV2,
    block_v2::BlockCoreV2,
    storage_v2::{StateKeyV2, StateLeafV2, StateNodeV2},
    pow::{qanhash_v2, verify_qanhash_v2, work_target_v2},
    consensus_parameters::SchemaVersion,
};

// CanonicalRecord roundtrip: encode -> decode -> encode == original
proptest! {
    #[test]
    fn canonical_record_roundtrip(bytes in any::<Vec<u8>>()) {
        // Generate valid canonical records using strategy
        // This is simplified; real impl uses custom strategies
    }
}

// Hash domain separation: hash_v2(d1) != hash_v2(d2) for d1 != d2
proptest! {
    #[test]
    fn hash_domain_separation(
        payload in any::<Vec<u8>>(),
        d1 in any::<DomainV2>(),
        d2 in any::<DomainV2>(),
    ) {
        prop_assume!(d1 != d2);
        let h1 = hash_v2(d1, 1, 1, &payload);
        let h2 = hash_v2(d2, 1, 1, &payload);
        prop_assert_ne!(h1, h2);
    }
}

// Hash chain separation
proptest! {
    #[test]
    fn hash_chain_separation(
        payload in any::<Vec<u8>>(),
        chain1 in 1u32..1000,
        chain2 in 1001u32..2000,
    ) {
        let h1 = hash_v2(DomainV2::TxBody, chain1, 1, &payload);
        let h2 = hash_v2(DomainV2::TxBody, chain2, 1, &payload);
        prop_assert_ne!(h1, h2);
    }
}

// Hash schema separation
proptest! {
    #[test]
    fn hash_schema_separation(
        payload in any::<Vec<u8>>(),
        schema1 in 1u16..100,
        schema2 in 101u16..200,
    ) {
        let h1 = hash_v2(DomainV2::TxBody, 1, schema1, &payload);
        let h2 = hash_v2(DomainV2::TxBody, 1, schema2, &payload);
        prop_assert_ne!(h1, h2);
    }
}

// qanhash_v2 determinism
proptest! {
    #[test]
    fn qanhash_v2_deterministic(
        core_hash in any::<[u8; 32]>(),
        nonce in any::<u64>(),
    ) {
        let h1 = qanhash_v2(1, 1, HashV2::from_bytes(core_hash), nonce);
        let h2 = qanhash_v2(1, 1, HashV2::from_bytes(core_hash), nonce);
        prop_assert_eq!(h1, h2);
    }
}

// work_target_v2 monotonic in difficulty
proptest! {
    #[test]
    fn work_target_monotonic(diff1 in 1u64..1_000_000, diff2 in 1u64..1_000_000) {
        prop_assume!(diff1 < diff2);
        let t1 = work_target_v2(diff1).unwrap();
        let t2 = work_target_v2(diff2).unwrap();
        // Higher difficulty -> lower target
        prop_assert!(t1 > t2); // Big-endian comparison
    }
}

// Merkle root determinism
proptest! {
    #[test]
    fn merkle_root_deterministic(tx_ids in any::<Vec<HashV2>>()) {
        let r1 = build_tx_merkle_root_v2(&tx_ids);
        let r2 = build_tx_merkle_root_v2(&tx_ids);
        prop_assert_eq!(r1, r2);
    }
}

// Retarget bounds
proptest! {
    #[test]
    fn retarget_within_bounds(
        current_diff in 100_000u64..10_000_000,
        actual_interval in 1_000u64..1_000_000_000,
        target_interval in 1_000u64..1_000_000_000,
    ) {
        let params = test_params_with_bounds(7500, 12500);
        let next = compute_next_difficulty(current_diff, actual_interval, target_interval, &params);
        let lower = current_diff * 7500 / 10000;
        let upper = current_diff * 12500 / 10000;
        prop_assert!(next >= lower && next <= upper);
    }
}

// Emission supply cap
proptest! {
    #[test]
    fn emission_never_exceeds_cap(heights in any::<Vec<u64>>()) {
        let params = test_economic_params();
        let mut total = 0u128;
        for h in heights {
            let r = compute_block_reward_v2(h, total, &params);
            total += r;
            prop_assert!(total <= params.max_total_supply);
        }
    }
}
```

- [ ] **Step 2: Add proptest strategies for V2 types.**

Define `Arbitrary` implementations for `CanonicalRecord`, `HashV2`, `DomainV2`, etc.

- [ ] **Step 3: Run property tests.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- proptest_v2
```

Expected: `PASS` (or finds bugs to fix).

- [ ] **Step 4: Record intended commit.**

Intended commit: `test(verification): property-based tests for V2 canonical invariants`.

---

### Task 3: Fuzzing Suite

**Files:**
- Modify: `Protocol_Core/qanto-node/fuzz/Cargo.toml`
- Create: `Protocol_Core/qanto-node/fuzz/fuzz_targets/`
  - `fuzz_canonical_record_decode.rs`
  - `fuzz_hash_v2.rs`
  - `fuzz_qanhash_v2.rs`
  - `fuzz_p2p_envelope.rs`
  - `fuzz_grpc_request.rs`
- Script: `scripts/run_fuzz.sh`

**Interfaces:**
- Fuzzes all parsers and hash functions with cargo-fuzz.

- [ ] **Step 1: Write fuzz targets.**

Create `Protocol_Core/qanto-node/fuzz/fuzz_targets/fuzz_canonical_record_decode.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use qanto_consensus_primitives::record::DecodedRecord;

fuzz_target!(|data: &[u8]| {
    // CanonicalRecord decode must never panic, must return Err for invalid input
    let _ = DecodedRecord::decode(data);
});
```

Create `fuzz_hash_v2.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use qanto_consensus_primitives::{hash_v2, DomainV2};

fuzz_target!(|data: &[u8]| {
    // hash_v2 must never panic
    let _ = hash_v2(DomainV2::TxBody, 1, 1, data);
    let _ = hash_v2(DomainV2::BlockCore, 1, 1, data);
    // ... all domains
});
```

Create `fuzz_qanhash_v2.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use qanto_consensus_primitives::{qanhash_v2, HashV2};

fuzz_target!(|data: &[u8]| {
    if data.len() >= 40 { // 32 bytes hash + 8 bytes nonce
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&data[..32]);
        let nonce = u64::from_le_bytes(data[32..40].try_into().unwrap_or([0;8]));
        let _ = qanhash_v2(1, 1, HashV2::from_bytes(hash), nonce);
    }
});
```

Create `fuzz_p2p_envelope.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use qanto_consensus_primitives::{net_v2::P2pEnvelopeV2, record::DecodedRecord};

fuzz_target!(|data: &[u8]| {
    let _ = DecodedRecord::decode(data); // Try to decode as P2pEnvelopeV2
});
```

- [ ] **Step 2: Run fuzzing (CI mode: short duration).**

```bash
bash scripts/run_fuzz.sh --ci
# Or locally for longer:
# cargo fuzz run fuzz_canonical_record_decode -- -max_total_time=300
```

Expected: No crashes, no panics. Any findings added to regression tests.

- [ ] **Step 3: Record intended commit.**

Intended commit: `test(verification): fuzzing suite for all V2 parsers and hash functions`.

---

### Task 4: Chaos Testing Suite

**Files:**
- Create: `Protocol_Core/qanto-node/tests/chaos_v2.rs`
- Create: `Protocol_Core/tools/chaos_harness/`

**Interfaces:**
- Simulates network partitions, validator churn, difficulty shocks, timestamp attacks, deep reorgs.

- [ ] **Step 1: Write chaos tests.**

Create `Protocol_Core/qanto-node/tests/chaos_v2.rs`:

```rust
use qanto_consensus_primitives::{
    consensus_parameters::ConsensusParametersV2,
    block_v2::BlockCoreV2,
    difficulty_v2::retarget_difficulty_v2,
};

#[test]
fn chaos_network_partition_no_finality_violation() {
    // Split network into two partitions
    // Each produces blocks
    // When healed, finality certificates must not conflict
    // V2 fork choice + finality must resolve correctly
}

#[test]
fn chaos_validator_churn_maintains_2_3_threshold() {
    // Validators join/leave rapidly
    // ValidatorSetV2 updates at epoch boundaries
    // Finality certificates must still require 2/3 of current set
}

#[test]
fn chaos_difficulty_shock_retarget_bounds_hold() {
    // Simulate sudden hashrate drop (10x) and spike (10x)
    // V2 retarget bounds (±25%) must prevent difficulty runaway
    // Chain must not stall or become too easy
}

#[test]
fn chaos_timestamp_attack_mtp_limits_manipulation() {
    // Attacker produces blocks with future timestamps
    // Median Time Past (11 blocks) must limit effect
    // Difficulty must not drop below bounds
}

#[test]
fn chaos_deep_reorg_finality_prevents_history_rewrite() {
    // Attacker tries to reorg past finalized block
    // Finality certificates must make this impossible
    // Fork choice must not select non-finalized chain over finalized
}
```

- [ ] **Step 2: Implement chaos harness (simulacrum-based).**

Create `Protocol_Core/tools/chaos_harness/` with network simulator.

- [ ] **Step 3: Run chaos tests.**

```bash
rtk cargo nextest run -p qanto-node --locked -- chaos_v2
```

Expected: `PASS` (V2 consensus resilient to all chaos scenarios).

- [ ] **Step 4: Record intended commit.**

Intended commit: `test(verification): chaos testing suite for V2 consensus resilience`.

---

### Task 5: Performance Benchmarks

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/benches/v2_benches.rs`
- Create: `Protocol_Core/qanto-node/benches/v2_benches.rs`
- Script: `scripts/run_benchmarks.sh`

**Interfaces:**
- Measures: block validation throughput, state root computation, Merkle proof gen/verify, P2P message handling, gRPC latency.

- [ ] **Step 1: Write benchmarks.**

Create `Protocol_Core/qanto-consensus-primitives/benches/v2_benches.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use qanto_consensus_primitives::{
    hash::hash_v2,
    transaction_v2::TransactionBodyV2,
    block_v2::{BlockCoreV2, build_tx_merkle_root_v2},
    storage_v2::StateStoreV2,
    pow::{qanhash_v2, verify_qanhash_v2},
    finality_v2::{aggregate_votes, verify_certificate},
};

fn bench_hash_v2(c: &mut Criterion) {
    let payload = vec![0u8; 1024];
    c.bench_function("hash_v2_1kb", |b| b.iter(|| hash_v2(DomainV2::TxBody, 1, 1, black_box(&payload))));
}

fn bench_qanhash_v2(c: &mut Criterion) {
    let core_hash = HashV2::ZERO;
    c.bench_function("qanhash_v2", |b| b.iter(|| qanhash_v2(1, 1, black_box(core_hash), black_box(12345))));
}

fn bench_verify_qanhash_v2(c: &mut Criterion) {
    let work = qanhash_v2(1, 1, HashV2::ZERO, 12345);
    c.bench_function("verify_qanhash_v2", |b| b.iter(|| verify_qanhash_v2(1, 1, HashV2::ZERO, 12345, work, 1_000_000).unwrap()));
}

fn bench_merkle_root(c: &mut Criterion) {
    let tx_ids: Vec<_> = (0..100).map(|i| HashV2::from_bytes([i as u8; 32])).collect();
    c.bench_function("merkle_root_100_txs", |b| b.iter(|| build_tx_merkle_root_v2(black_box(&tx_ids))));
}

fn bench_block_validation(c: &mut Criterion) {
    // Full block validation pipeline
    c.bench_function("block_validation_v2", |b| b.iter(|| {
        // validate_block_v2(...)
    }));
}

criterion_group!(benches, bench_hash_v2, bench_qanhash_v2, bench_verify_qanhash_v2, bench_merkle_root, bench_block_validation);
criterion_main!(benches);
```

- [ ] **Step 2: Run benchmarks.**

```bash
bash scripts/run_benchmarks.sh
# Or: cargo bench -p qanto-consensus-primitives --bench v2_benches
```

Expected: Results saved, baseline established for regression detection.

- [ ] **Step 3: Record intended commit.**

Intended commit: `bench(verification): V2 performance benchmarks for all critical paths`.

---

### Task 6: Economic Model Verification

**Files:**
- Create: `Protocol_Core/qanto-core/tests/economic_verification.rs`
- Script: `scripts/verify_economics.sh`

**Interfaces:**
- Verifies: emission reaches 21B cap, vesting unlocks correctly, fee burn reduces supply, staking APR floor met.

- [ ] **Step 1: Write economic verification tests.**

Create `Protocol_Core/qanto-core/tests/economic_verification.rs`:

```rust
use qanto_consensus_primitives::consensus_parameters::ConsensusParametersV2;
use qanto_core::emission::{compute_block_reward_v2, compute_vested_amount, apply_fee_burn};

#[test]
fn emission_reaches_21b_cap() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let mut total = 0u128;
    for height in 1..10_000_000 {
        let reward = compute_block_reward_v2(height, total, &params);
        total += reward;
        if reward == 0 { break; }
    }
    assert_eq!(total, params.max_total_supply);
}

#[test]
fn vesting_ecosystem_unlocks_on_schedule() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let total_eco = params.ecosystem_allocation;
    let mut unlocked = 0u128;
    
    // Year 1 (cliff)
    for epoch in 0..52 { // 52 weeks
        unlocked = compute_vested_amount(&vesting_schedule, epoch, total_eco);
        assert_eq!(unlocked, 0);
    }
    
    // Year 2-3 (linear)
    for epoch in 52..156 {
        unlocked = compute_vested_amount(&vesting_schedule, epoch, total_eco);
        let expected = total_eco * (epoch - 52) as u128 / 104;
        assert_eq!(unlocked, expected);
    }
    
    // Fully vested
    unlocked = compute_vested_amount(&vesting_schedule, 156, total_eco);
    assert_eq!(unlocked, total_eco);
}

#[test]
fn fee_burn_reduces_effective_supply() {
    let params = ConsensusParametersV2::for_testnet_candidate();
    let mut supply = params.max_total_supply;
    for _ in 0..1_000_000 {
        let (burned, _) = apply_fee_burn(1000, 500, 1_000_000);
        supply = supply.saturating_sub(burned);
    }
    assert!(supply < params.max_total_supply);
}

#[test]
fn staking_apr_floor_met() {
    // Verify that validator rewards from emission reserve meet APR floor
    let params = ConsensusParametersV2::for_testnet_candidate();
    // Calculate annual rewards / total stake >= apr_floor_bps / 10000
}
```

- [ ] **Step 2: Run economic verification.**

```bash
rtk cargo nextest run -p qanto-core --locked -- economic_verification
```

Expected: `PASS`.

- [ ] **Step 3: Record intended commit.**

Intended commit: `test(verification): economic model verification (supply cap, vesting, fee burn, APR floor)`.

---

### Task 7: Evidence Bundle Generator

**Files:**
- Create: `Protocol_Core/tools/evidence_bundle/`
- Create: `Protocol_Core/tools/evidence_bundle/src/main.rs`
- Script: `scripts/generate_evidence.sh`

**Interfaces:**
- Consumes: All test results, golden validation report, benchmarks, economic verification.
- Produces: `evidence-bundle-<candidate-id>.json` with evidence labels per finding.

- [ ] **Step 1: Implement evidence bundle generator.**

Create `Protocol_Core/tools/evidence_bundle/src/main.rs`:

```rust
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub candidate_id: String,
    pub candidate_manifest_sha3_256: String,
    pub generated_at_utc: String,
    pub workstreams: HashMap<String, WorkstreamEvidence>,
    pub overall_assessment: OverallAssessment,
}

#[derive(Serialize, Deserialize)]
pub struct WorkstreamEvidence {
    pub workstream: String,
    pub checks: Vec<CheckEvidence>,
    pub summary: WorkstreamSummary,
}

#[derive(Serialize, Deserialize)]
pub struct CheckEvidence {
    pub check_name: String,
    pub evidence_label: EvidenceLabel,
    pub details: String,
    pub source: String, // test file, benchmark, etc.
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EvidenceLabel {
    Confirmed,
    Measured,
    Derived,
    Estimated,
    Hypothesis,
    NeedsExternalReview,
}

// ... implementation generates bundle from test outputs
```

- [ ] **Step 2: Run evidence generation.**

```bash
bash scripts/generate_evidence.sh
```

Expected: `evidence-bundle-<id>.json` with all workstream evidence.

- [ ] **Step 3: Record intended commit.**

Intended commit: `tool(verification): evidence bundle generator for independent audit`.

---

### Task 8: Final Verification Suite Integration and Report

**Files:**
- Script: `scripts/full_verification.sh`
- Report: `verification-report.md`

**Interfaces:**
- Runs all verification steps, produces final report.

- [ ] **Step 1: Create full verification script.**

Create `scripts/full_verification.sh`:

```bash
#!/bin/bash
set -e

echo "=== Qanto V2 Verification Suite ==="

echo "1. Golden cross-crate validation..."
bash scripts/run_golden_validation.sh

echo "2. Property-based tests..."
cargo nextest run -p qanto-consensus-primitives --locked -- proptest_v2

echo "3. Fuzzing (CI mode)..."
bash scripts/run_fuzz.sh --ci

echo "4. Chaos tests..."
cargo nextest run -p qanto-node --locked -- chaos_v2

echo "5. Benchmarks..."
bash scripts/run_benchmarks.sh

echo "6. Economic verification..."
cargo nextest run -p qanto-core --locked -- economic_verification

echo "7. Evidence bundle..."
bash scripts/generate_evidence.sh

echo "=== Verification Complete ==="
```

- [ ] **Step 2: Run full verification.**

```bash
bash scripts/full_verification.sh
```

Expected: All steps pass, evidence bundle generated.

- [ ] **Step 3: Create verification report.**

Create `verification-report.md` with summary of all evidence labels.

- [ ] **Step 4: Final self-review checklist.**

- [ ] Golden cross-crate validation: 100% parity across all 6 crates for all V2 types.
- [ ] Property tests: cover all canonical invariants (roundtrip, separation, determinism, bounds, cap).
- [ ] Fuzzing: no crashes/panics in any parser or hash function.
- [ ] Chaos tests: V2 consensus resilient to partition, churn, shock, timestamp attack, reorg.
- [ ] Benchmarks: baseline established for all critical paths.
- [ ] Economic verification: supply cap, vesting, fee burn, APR floor all verified.
- [ ] Evidence bundle: generated with evidence labels for every check.
- [ ] Full verification script runs cleanly.
- [ ] All tests pass.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(verification): complete verification suite; evidence bundle; final report`.

---

## Acceptance Criteria

- [ ] Golden vector cross-crate validation passes for all V2 types across all 6 crates.
- [ ] Property-based tests cover all canonical invariants with `proptest`.
- [ ] Fuzzing suite runs without crashes for all parsers and hash functions.
- [ ] Chaos tests prove V2 consensus resilience to 5 attack vectors.
- [ ] Benchmarks established for block validation, Merkle, PoW, finality, state sync, P2P, gRPC.
- [ ] Economic model verified: 21B cap, vesting schedule, fee burn, staking APR floor.
- [ ] Evidence bundle generated with CONFIRMED/MEASURED/DERIVED/ESTIMATED/HYPOTHESIS/NEEDS EXTERNAL REVIEW labels.
- [ ] Full verification script runs end-to-end.
- [ ] All tests pass.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 9 Summary for Dependent Workstreams

| Output | Location | Consumers |
|---|---|---|
| Golden validation suite | `* /tests/golden_cross_crate.rs` | Workstream 10 |
| Property tests | `qanto-consensus-primitives/tests/proptest_v2.rs` | Workstream 10 |
| Fuzz targets | `qanto-node/fuzz/fuzz_targets/` | Workstream 10 |
| Chaos tests | `qanto-node/tests/chaos_v2.rs` | Workstream 10 |
| Benchmarks | `* /benches/v2_benches.rs` | Workstream 10 |
| Economic verification | `qanto-core/tests/economic_verification.rs` | Workstream 10 |
| Evidence bundle generator | `tools/evidence_bundle/` | Workstream 10 |
| Full verification script | `scripts/full_verification.sh` | Workstream 10 |
| Evidence bundle output | `evidence-bundle-<id>.json` | Workstream 10 (final verdict) |