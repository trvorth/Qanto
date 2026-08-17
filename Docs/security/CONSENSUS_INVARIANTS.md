# Consensus Invariants

**Qanto Layer-0 Blockchain**  
**Branch:** main
**Schema Version:** 2  

## Core Invariants

| # | Invariant | Verified | Test Coverage |
|---|-----------|----------|---------------|
| 1 | Finality requires >= 2/3 of total active stake | FIXED | `quorum_boundaries_are_precise_and_overflow_safe` |
| 2 | Quorum check is overflow-safe (checked arithmetic) | FIXED | `consensus_arithmetic_rejects_u128_boundaries` |
| 3 | Zero total stake returns false in finality path | FIXED | Guard in `finalize_blocks()` |
| 4 | Zero path stake against positive total returns false | FIXED | `checked_stake_quorum(0, 100)` |
| 5 | Duplicate validator stake is not counted twice | DESIGNED | Dedup by validator ID map |
| 6 | Inactive validators excluded from stake sum | DESIGNED | `total_active_stake` calculated from active set |
| 7 | Unknown validators contribute zero stake | DESIGNED | Only registered validators in stake map |
| 8 | Block IDs are deterministic | DESIGNED | Canonical bincode serialization |
| 9 | State roots are deterministic | DESIGNED | Ordered Merkle tree |
| 10 | DAG insertion cannot create cycles | DESIGNED | Parent depth validation |
| 11 | Duplicate block insertion is idempotent | DESIGNED | Hash-indexed block store |
| 12 | Transactions cannot execute twice | DESIGNED | Processed transaction set |
| 13 | Total supply ≤ QANTO_MAX_SUPPLY | DESIGNED | Checked monetary addition |
| 14 | Balances never underflow | DESIGNED | Checked subtraction with overflow rejection |
| 15 | Partition healing converges deterministically | DESIGNED | `partition_tests` suite |
| 16 | Governance fixed-point arithmetic (no f64) | FIXED | `GOVERNANCE_FIXED_POINT_SCALE = 1e9` |
| 17 | Governance legacy f64 → i128 migration | FIXED | `governance_fixed_point_tests` |
| 18 | `ProposalType::UpdateRule` stores i128 not f64 | FIXED | Custom serde with v1/v2 compatibility |

## Finality Quorum

**Rule:** `path_stake / total_active_stake >= 2 / 3`
**Implementation:** `checked_stake_quorum(path_stake, total_active_stake)`

- Returns `Ok(true)` when quorum met
- Returns `Ok(false)` when quorum not met
- Returns `Err(String)` on arithmetic overflow
- Zero total stake is guarded in caller

## Governance Fixed-Point Architecture

**Scale Factor:** `GOVERNANCE_FIXED_POINT_SCALE = 1_000_000_000i128`

**Storage:** All `EpochRule.value` and `ProposalType::UpdateRule` values are stored as `i128` (actual_value × 1e9)

**Migration Decision A:** No persisted governance state exists (in-memory only during testnet runs). Schema upgraded with v1→v2 compatibility in deserialization but no state was persisted.

**Old Format Rejection:** NaN and infinity are rejected. Negative values clamped to ≥ -1000.0. Out-of-range values rejected.

## Accepted Serialization Compatibility

ProposalType and EpochRule implement custom Deserialize that:
1. Accepts v2 (i128) format directly
2. Accepts v1 (f64) format and converts via `from_legacy_f64()`
3. Rejects invalid legacy values
4. Always saves in v2 format