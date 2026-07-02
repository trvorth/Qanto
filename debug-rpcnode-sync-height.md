# Debug Session: rpcnode-sync-height
- **Status**: [OPEN]
- **Issue**: `rpcnode` remains at height `0` in the local 3-node testnet after validator-side consensus fixes; verify whether deterministic genesis resolves sync and identify the next blocker if not.
- **Debug Server**: not started
- **Log File**: .dbg/trae-debug-log-rpcnode-sync-height.ndjson

## Reproduction Steps
1. Build the release node binary after the deterministic genesis patch.
2. Start the local 3-node testnet with `Scripts/run_local_testnet.sh`.
3. Run `Scripts/check_local_testnet_health.sh`.
4. Inspect `logs/testnet/*.log` and `/stats` if `rpcnode` height remains `0`.

## Hypotheses & Verification
| ID | Hypothesis | Likelihood | Effort | Evidence |
|----|------------|------------|--------|----------|
| A | Deterministic genesis already fixes the root cause, and a fresh testnet run will allow `rpcnode` to accept `height=1+` blocks. | High | Low | Rejected: validator mining recovered, but `rpcnode` still reports `latest_block_height = 0`. |
| B | Incoming blocks still arrive out of dependency order, and `node.rs` drops them permanently because there is no orphan retry queue. | High | Med | Confirmed and partially mitigated earlier: orphan retry was needed, but it was not the final blocker because `rpcnode` still remains at `0` after that fix. |
| C | `run_local_testnet.sh` or `check_local_testnet_health.sh` still reports failure because of strict startup timing even when the cluster later converges. | Med | Low | Rejected: current runs show healthy endpoints and connected peers while `rpcnode` still reports `latest_block_height = 0`. |
| D | Canonical P2P direct-delivery is only reaching `bootnode`, and accepted remote blocks are not being relayed onward to `rpcnode`. | High | Low | Confirmed: `validator1.log` shows QDS direct sends and ACKs only for bootnode peer `12D3KooWRPc...`; `bootnode.log` shows `Queued direct block ...` and `BLOCK_ACCEPTED ... via P2P`; `rpcnode.log` shows no corresponding direct-block receipt or DAG acceptance. |

## Log Evidence
- `Scripts/run_local_testnet.sh status` now reports all three nodes healthy with connected peers, while `Scripts/check_local_testnet_health.sh` still fails on `rpcnode: latest_block_height must be greater than zero, got 0`.
- `validator1.log` shows continuous local mining and acceptance (`BLOCK_ACCEPTED`) through height `30+`.
- `validator1.log` also shows direct QDS block delivery to bootnode peer `12D3KooWRPc...` with successful ACKs, for example:
  - `Directly sent block ... at height 1 to peer 12D3KooWRPc... over QDS`
  - `Received QDS ack from 12D3KooWRPc...: accepted=true detail=queued block ...`
- `bootnode.log` confirms those validator blocks are being received and accepted:
  - `Queued direct block ... from peer 12D3KooWLr... for DAG processing`
  - `BLOCK_ACCEPTED ...`
  - `✅ Block ... successfully added to DAG via P2P`
- `rpcnode.log` no longer indicates a peer-connectivity failure, but it still shows no `Queued direct block ...`, no `P2P received BroadcastBlock command ...`, and no `BLOCK_ACCEPTED` for validator-produced remote blocks.
- This narrows the remaining runtime blocker to post-ingest propagation: remote blocks reach bootnode, but are not being relayed onward to `rpcnode`.

## Post-Fix Evidence
- After adding relay-on-accept in `node.rs`, `bootnode.log` shows:
  - `Relayed accepted P2P block ... back through canonical P2P`
- The same rerun shows `rpcnode.log` receiving and accepting the relayed blocks:
  - `Queued direct block ... from peer ...`
  - `BLOCK_ACCEPTED ...`
- Health checks now confirm sync recovery:
  - `block height is growing: 19 -> 31 (delta=12)`
  - `/stats` now converges on the same `latest_block_height` and `latest_block_hash` across `bootnode`, `validator1`, and `rpcnode`
- After wiring finality metric updates to actual DAG finalization and ensuring local mined blocks trigger periodic maintenance, `/stats` now reports non-zero finality on all three nodes:
  - `bootnode {'latest_block_height': 35, 'finality_ms': 85, 'peer_count': 2}`
  - `validator1 {'latest_block_height': 35, 'finality_ms': 83, 'peer_count': 1}`
  - `rpcnode {'latest_block_height': 35, 'finality_ms': 86, 'peer_count': 1}`
- `Scripts/check_local_testnet_health.sh` now passes end-to-end:
  - `Peer connectivity: OK`
  - `Block growth: OK`
  - `Finality metrics: OK`
  - `publish-readiness reports node is ready`

## Verification Conclusion
- The original runtime sync blocker is resolved: `rpcnode` no longer stalls at height `0`.
- The secondary telemetry/finality blocker is also resolved: all three nodes now expose non-zero `finality_ms`, and the local health script passes fully.
- The debug session remains open until user confirmation, but the current local testnet baseline is healthy and publish-ready.
