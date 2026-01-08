# Qanto Consensus: Proof-of-Work Gating Semantics

Version: v0.1.0
Last Updated: 2025-11-12
Related Code: `src/consensus.rs`
Related Whitepaper: `docs/whitepaper/Qanto-whitepaper.md` (Section 3.5.)

## Overview
- Canonical validity of a block’s Proof-of-Work (PoW) is gated strictly by the block’s declared header difficulty (`block.difficulty`).
- SAGA-derived effective difficulty is diagnostic and observational; it does not gate acceptance. This preserves determinism across nodes.
- A strict guard rejects non-positive declared difficulty (`<= 0.0`).
- When declared difficulty diverges from SAGA-effective difficulty, the node emits a `warn` log to aid operators.

## Deterministic Validation
Validation computes a target from the declared difficulty and checks the block’s PoW hash against that target. Only declared difficulty is used for acceptance.

```rust
// Tokio runtime assumed for asynchronous consensus operations.
// Type signature:
// async fn validate_proof_of_work(&self, block: &QantoBlock) -> Result<(), ConsensusError>

let declared = block.difficulty; // must be > 0.0
if declared <= 0.0 {
    tracing::error!(difficulty = declared, "Invalid non-positive declared difficulty");
    return Err(ConsensusError::ProofOfWorkFailed("non-positive difficulty".into()));
}

let pow_hash = block.hash_for_pow();
if !Consensus::is_pow_valid(pow_hash.as_bytes(), declared) {
    return Err(ConsensusError::ProofOfWorkFailed("hash does not meet declared target".into()));
}
```

## SAGA Effective Difficulty (Diagnostics)
- SAGA computes an effective difficulty from miner credit and policy.
- The implementation logs `debug` details and `warn` on divergence:

```
WARN  consensus: Block <id> difficulty differs from SAGA-derivation. Declared: <d>, SAGA-effective: <e>
```

## Boundary Conditions
- Declared difficulty `<= 0.0`: reject with `ConsensusError::ProofOfWorkFailed`.
- Extremely small positive difficulty: valid if PoW meets target; tests ensure reliability.
- Divergence: accepted if PoW meets declared difficulty; warning emitted.

## Testing Strategy
- Unit/integration tests assert rejection for zero/negative difficulty.
- Logging tests capture `warn` divergence and `debug` SAGA computation.
- Property-based tests exercise boundary values (0, negative epsilon, small positive epsilon).

## Cross-References
- Whitepaper Section: “PoW Gating Semantics” in `Qanto-whitepaper.md`.
- Implementation: `src/consensus.rs` (`validate_proof_of_work`, `is_pow_valid`).
- Mining kernels and target calculation: `miner.rs`, `myblockchain/qanhash*.rs`.

## Revision History
- 2025-11-12 (v0.1.0): Documented canonical gating, non-positive guard, and divergence logging.