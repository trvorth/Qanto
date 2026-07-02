# Debug Session: block-signature-failure
- **Status**: [OPEN]
- **Issue**: Validator mines candidate blocks, but `QantoDAG::add_block` rejects them with `Quantum-resistant signature error: Signature verification failed`, leaving local testnet height at 0.
- **Debug Server**: http://127.0.0.1:7777/event
- **Log File**: .dbg/trae-debug-log-block-signature-failure.ndjson

## Reproduction Steps
1. Build release binary with `cargo build --release -p qanto`.
2. Start local cluster with `QANTO_TESTNET_PREFUND_ADDRESS="<64-hex>" ./Scripts/run_local_testnet.sh`.
3. Wait for node health endpoints to turn healthy and peers to connect.
4. Observe validator logs and run `./Scripts/check_local_testnet_health.sh`.
5. Confirm that mined candidate blocks are rejected and `latest_block_height` remains `0`.

## Hypotheses & Verification
| ID | Hypothesis | Likelihood | Effort | Evidence |
|----|------------|------------|--------|----------|
| A | Block signing payload and verification payload are not canonicalized identically, causing self-signed blocks to fail verification. | High | Medium | Rejected by code-path inspection: `QantoBlock::new()` and `verify_signature()` use the same `serialize_for_signing()` routine. |
| B | The block producer signs with a different key/material than the verifier expects from the block header or validator identity. | High | Medium | Confirmed by source and runtime: `create_candidate_block()` ignored the caller-supplied signing key and hardcoded `QantoPQPrivateKey::new_dummy()` for both reward-calculation and final block signing. |
| C | Block serialization mutates signature-covered fields after signing, invalidating the signature before insertion. | Medium | Medium | Not primary root cause; post-signature mutations (`reward`, `reservation_miner_id`, `cross_chain_references`) are outside the signature payload. |
| D | The quantum-resistant wrapper rejects empty or malformed signature/public-key bytes produced on the block path only. | Medium | Low | Secondary risk only; failure reproduces deterministically even before considering wire transport, and source shows the wrong signing key material on the producer path. |
| E | The DAG insertion path verifies against a different block instance or recomputed hash than the one originally signed. | Medium | Medium | Rejected by runtime pattern: every locally mined height-1 block fails immediately at `block.verify_signature()` before deeper DAG state transitions. |

## Log Evidence
- Health check after 60s: `[OK]` all three nodes healthy and peered, but `[FAIL] rpcnode: latest_block_height must be greater than zero, got 0`.
- Validator runtime log repeatedly shows:
  - `BLOCK PROCESSOR: Attempting to add block ...`
  - `BLOCK PROCESSOR: Failed to add block ...: Quantum-resistant signature error: Signature verification failed`
- Source evidence in `qantodag.rs`:
  - `create_candidate_block(..., qr_signing_key, ...)` previously ignored `qr_signing_key`
  - both `temp_block_for_reward_calc` and final `QantoBlock::new(...)` used `QantoPQPrivateKey::new_dummy()`

## Verification Conclusion
- Root cause identified: the live block producer path signed candidate blocks with a dummy PQ private key instead of the validator's real key material, so `add_block()` rejected every mined block at signature verification and chain height remained at `0`.
- Minimal fix in progress: replace dummy-key signing inside `create_candidate_block()` with the caller-provided validator key and re-run the local testnet.
