# Qanto Consensus, Networking, Transaction, and SAGA Audit

Audit date: 2026-08-08  
Audit target: effective working tree at `/Users/cps/qanto`  
Scope: mandate workstreams 2, 5, 6, and 7  
Execution mode: read-only source audit plus targeted local tests; this report's author did not modify protocol source  

## Executive verdict

**NOT READY.** The current working tree contains useful containment fixes, but it does not yet define a safe, convergent public-testnet protocol. The launch blockers in this scope are:

1. A block signer is not cryptographically bound to `block.validator`, and a native transaction signer is not bound to `transaction.sender`.
2. The enforced SAGA reward equality uses node-local mutable state and `f64`; ordinary local API activity can change ΩMEGA threat state and therefore the reward another block must claim.
3. Block-size acceptance depends on node-local candidate-production counters.
4. Every accepted DAG branch mutates one global UTXO/balance state immediately. There is no branch-scoped state, canonical-chain application, or rollback on reorganization.
5. The state transition is not atomic: live UTXOs change before persistence enqueue and before the block is inserted, with no rollback on later error.
6. There is no active production finality protocol. The automatic finalizer was correctly disabled, while the remaining callable implementation can finalize conflicting tips and even treats zero total stake as quorum.
7. Duplicate parent IDs are accepted and multiply branch weight; arbitrary additional roots are also accepted.
8. Startup `RequestState` broadcasts the requester's own state instead of requesting a peer checkpoint. Full snapshots and valid blocks can also exceed every propagation transport limit.
9. P2P reconnection sleeps inside the swarm event loop for up to five minutes, and no IP/subnet diversity or peer-rotation policy was found.
10. The standard mempool's conflict and capacity checks are non-atomic; its configured transaction-count limit is ignored.

The current fixes are accurately classified below as `FIXED_IN_WORKTREE`, `CONTAINED_IN_WORKTREE`, or `PRE_EXISTING_FIX`. None was authored by this read-only audit track.

## Evidence labels

- `CONFIRMED`: directly established from current source or an executed test.
- `DERIVED`: follows mechanically from confirmed source behavior.
- `MEASURED`: produced by an executed local test.
- `HYPOTHESIS`: plausible but not reproduced; no launch decision relies solely on one.

---

# Workstream 2 — Consensus Engine Correctness

## 2.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| CNS-001 | SEV-1 | Validator authorization | `Protocol_Core/qanto-node/src/qantodag.rs:623-660`, `:728-750`, `:3010-3012`, `:3150-3168` | Block verification trusts the PQ public key embedded in the block signature. The signed text includes `block.validator`, but no rule derives that validator identity from the key or checks a registered validator certificate. Any PQ key can therefore sign a block labeled as another validator. | CONFIRMED, deferred; unsafe slashing is contained |
| CNS-002 | SEV-1 | SAGA reward equality | `Protocol_Core/qanto-node/src/qantodag.rs:3119-3143`, `:3184-3194`, `:3241-3254` | The known reward-bypass condition is closed: validation now requires `block.reward == saga_base_reward + total_fees`, validates the developer/validator outputs, and requires total coinbase output to equal `block.reward`. | FIXED_IN_WORKTREE; equality is enforced |
| CNS-003 | SEV-1 | Reward determinism | `Protocol_Core/qanto-node/src/saga.rs:3754-3855`; `Protocol_Core/qanto-node/src/qantodag.rs:3138-3143`, `:4421-4428` | The equality in CNS-002 is computed from mutable local credit, ΩMEGA threat state, environmental metrics, predictive premium, active edict, optional ISNM state, and `f64`. Local maintenance cadence increments the epoch and mutates those inputs. Equal blocks can therefore produce different expected rewards on different nodes. | CONFIRMED, launch blocker |
| CNS-004 | SEV-1 | PoW difficulty | `Protocol_Core/qanto-node/src/qantodag.rs:2925-2937`, `:3014-3037`; `Protocol_Core/qanto-node/src/miner.rs:1482-1495` | The earlier self-declared/zero-difficulty acceptance is contained by recomputing the ID and requiring exactly `INITIAL_DIFFICULTY` before PoW verification. `calculate_target_from_difficulty(0)` still returns the easiest target, but an inbound block can no longer reach it with difficulty zero. | FIXED_IN_WORKTREE for the fixed-difficulty network |
| CNS-005 | SEV-1 | Block commitment domain | `Protocol_Core/qanto-node/src/qantodag.rs:623-660`, `:728-750`, `:2900-2903`, `:3092-3100` | Inbound IDs are now recomputed, but the ID/signature payload omits nonce, reward, effort, cross-chain references/swaps, homomorphic data, smart contracts, carbon credentials, epoch, finality proof, and reservation ID. Candidate code mutates reward and cross-chain references after block construction. Several accepted fields therefore remain outside the producer's authenticated commitment. | PARTIALLY_FIXED, launch blocker |
| CNS-006 | SEV-1 | Consensus block-size rule | `Protocol_Core/qanto-node/src/qantodag.rs:2568-2601`, `:2905-2910`, `:2955-3004` | The accepted size limit is derived from `chain_loads`. That counter is incremented when a node creates a local candidate, not when it accepts a block. Nodes with the same DAG but different local mining activity can accept/reject the same 8–32 MiB block differently. | CONFIRMED, launch blocker |
| CNS-007 | SEV-1 | DAG roots and parent multiplicity | `Protocol_Core/qanto-node/src/qantodag.rs:3048-3090`, `:5836-5846`, `:5885-5889`, `:5910-5929` | Validation does not require a unique canonical genesis, does not reject parentless non-genesis blocks, and does not reject duplicate parent IDs. Fork-choice adds a child once per parent-list occurrence and then sums each occurrence, so duplicate parents multiply descendant weight. | CONFIRMED/DERIVED, launch blocker |
| CNS-008 | SEV-1 | Canonical state and reorg | `Protocol_Core/qanto-node/src/qantodag.rs:1820-1830`, `:1847-1849`, `:1910-1925`, `:2072-2091`, `:5906-5989` | `add_block` validates every branch against, then mutates, one live UTXO state before any canonical-path decision. Competing spends are accepted or rejected by arrival order; changing the heaviest path does not rollback the old branch or apply the new branch. | CONFIRMED, launch blocker |
| CNS-009 | SEV-1 | Atomic state commit | `Protocol_Core/qanto-node/src/qantodag.rs:1910-1943`, `:2000-2043`, `:2072-2074` | Live UTXOs are changed before persistence operations and before the block is inserted. Any enqueue, encoding, balance-read, or balance-write failure returns after partial mutation. There is no undo journal or atomic database batch. | CONFIRMED, launch blocker |
| CNS-010 | SEV-1 | Finality semantics | `Protocol_Core/qanto-node/src/qantodag.rs:81`, `:3460-3555`, `:4414-4419` | Automatic finality is now disabled. The remaining public method follows only the first parent for eight blocks, treats zero total stake as quorum, marks the whole window including the tip, can process every conflicting tip independently, and never removes a finalization marker. | CONTAINED_IN_WORKTREE; no active protocol finality |
| CNS-011 | SEV-2 | DPoS/PoSe integration | `Protocol_Core/qanto-node/src/qantodag.rs:4335-4363`; `Protocol_Core/qanto-node/src/consensus.rs:97-193`, `:264-288` | Producer-side validator selection is random and stake weighted, but inbound `is_valid_block` does not enforce selection, registration, minimum stake, votes, or committee certificates. The separate `Consensus` path only warns on PoS failure and has no production caller. No Proof-of-Storage/Execution verification was found. | CONFIRMED specification gap |
| CNS-012 | SEV-2 | Cross-chain reference binding | `Protocol_Core/qanto-node/src/qantodag.rs:3092-3100` | Validation checks only that the referenced block ID exists; it ignores the tuple's declared chain ID and does not authenticate the reference in the block signature/ID. | CONFIRMED, deferred |
| CNS-013 | SEV-2 | Anomaly arithmetic | `Protocol_Core/qanto-node/src/qantodag.rs:3371-3449` | Sums and squared differences use unchecked `u128` arithmetic (`sum`, `diff * diff`, `diff_sq * 10000`) on peer-controlled transaction amounts. The function runs during block validation; debug builds can panic and release builds can wrap. | CONFIRMED, deferred |
| CNS-014 | SEV-3 | Fork-choice selection | `Protocol_Core/qanto-node/src/qantodag.rs:5829-5894`, `:5906-5989`; `Protocol_Core/qanto-node/src/node.rs:1841-1878`, `:1919-1943` | Peer-supplied `effort` was removed from weight, roots/tips/children now use lexical tie-breaks, and sync topological queues are ordered. For an identical well-formed block set the selected path is now deterministic. This does not fix CNS-007 or the arrival-dependent state in CNS-008. | PARTIALLY_FIXED_IN_WORKTREE |

## 2.B Canonical `is_valid_block()` trace

The production node receives blocks through `QantoDAG::add_block`, which calls `QantoDAG::is_valid_block` at `qantodag.rs:1847-1849`. Source search found no production construction or call to `Consensus::validate_block`; `consensus.rs` is not the canonical gate.

| Order | Current check | Exact evidence | Assessment |
|---:|---|---|---|
| 1 | Require non-empty ID; recompute canonical ID; require fixed network difficulty | `qantodag.rs:2920-2937` | ID/difficulty containment implemented |
| 2 | Require at least one transaction; fail closed on bridge claims while verifier unavailable | `qantodag.rs:2938-2953` | Correct containment |
| 3 | Compute a node-local dynamic block-size limit and enforce transaction count/serialized bytes | `qantodag.rs:2955-3004` | Consensus-unsafe because inputs are local |
| 4 | Recompute Merkle root and verify block signature | `qantodag.rs:3006-3012` | Signature key is not bound to validator identity |
| 5 | Compute canonical QanHash PoW and compare with target | `qantodag.rs:3014-3037` | Active; fixed difficulty enforced earlier |
| 6 | Reject timestamps too far in the future | `qantodag.rs:3039-3046` | No median-time-past or old-time bound |
| 7 | Require each parent to exist, share chain ID, and have non-greater timestamp; require height `max(parent)+1` when parents exist | `qantodag.rs:3048-3090` | Missing parent uniqueness and canonical-root rule |
| 8 | Require referenced cross-chain block IDs to exist | `qantodag.rs:3092-3100` | Declared reference chain ID ignored; reference unsigned |
| 9 | Require exactly the first transaction to be inputless/coinbase | `qantodag.rs:3103-3117` | Correct positional rule |
| 10 | Sum fees and recompute SAGA base reward | `qantodag.rs:3119-3143` | Node-local and floating-point consensus input |
| 11 | Verify coinbase signature with the block header's embedded signing key | `qantodag.rs:3150-3169` | Does not establish who owns `block.validator` |
| 12 | Enforce declared reward, developer split, validator output, and total coinbase-output equality | `qantodag.rs:3184-3254` | Known bypass fixed; source reward remains unsafe |
| 13 | Batch-verify non-coinbase signatures and each transaction against the shared UTXO snapshot | `qantodag.rs:3256-3302` | Intra-block duplicate spends are later rejected at `:5762-5774`; branch state remains global |
| 14 | Compute an anomaly score and emit a warning | `qantodag.rs:3304-3319` | Advisory only after slashing containment; arithmetic remains unsafe |

Checks absent from the canonical gate include validator-key identity binding, validator eligibility/selection, DPoS quorum certificates, PoSe resource proofs, finality-proof verification, one canonical genesis, unique parent IDs, a committed state root, deterministic block-size/reward state, and canonical-branch/reorg application.

## 2.C Hybrid mechanism matrix

| Mechanism | Claimed role | Effective implementation | Classification |
|---|---|---|---|
| PoW/QanHash | Permissionless leader election and primary security | Canonical PoW is checked with a fixed accepted difficulty. The heterogeneous QanHash backend equivalence issue is covered by the cryptography report. | PARTIALLY_IMPLEMENTED |
| DPoS | Validator finality helper | Stake records and a random producer selector exist. Inbound blocks do not prove selection or validator ownership; the separate consensus helper only warns and is not on the node path. | SPECIFIED_BUT_UNIMPLEMENTED in canonical acceptance |
| PoSe | Storage/execution-capacity proof | No storage challenge, execution receipt, verifier, resource commitment, or acceptance rule was found. The only PoSe occurrence in consensus code is a comment. | SPECIFIED_BUT_UNIMPLEMENTED |
| SAGA | Telemetry-driven adaptive governor | Local heuristics and edicts are active; the resulting local/float state influences the mandatory reward check. Difficulty is nevertheless fixed, and block size is adjusted by local DAG counters rather than a committed SAGA edict. | PARTIALLY_IMPLEMENTED, consensus-unsafe |
| ΩMEGA | Final security gate | A process-local entropy/action heuristic gates local transaction creation/API admission and changes a global threat flag. `is_valid_block` does not invoke it. The local threat flag does influence SAGA reward. | IMPLEMENTED_BUT_NOT_AS_SPECIFIED; consensus-unsafe side effect |
| DAG fork choice | Deterministic canonical ordering | Lexical ties and difficulty-only weight improve deterministic selection for an identical well-formed set. Duplicate parents amplify weight, roots are not unique, and canonical state is never reconciled. | PARTIALLY_IMPLEMENTED |
| Finality | Irreversible 2/3-stake settlement | Automatic invocation is disabled because the implementation is unsafe. No signed quorum certificate or active finality horizon exists. | SPECIFIED_BUT_UNIMPLEMENTED in production |

## 2.D Fork-choice and finality proof status

### Fork-choice determinism

For an identical, well-formed set of blocks, `get_canonical_path_for_chain` now:

1. selects the lexicographically smallest height-zero root (`qantodag.rs:5910-5923`);
2. assigns each block its fixed difficulty as self-weight and recursively sums descendants (`:5864-5893`);
3. chooses the child with greatest subtree weight and breaks ties by block ID (`:5967-5987`).

That function is deterministic under its assumptions. Protocol convergence is not proven because validation does not enforce those assumptions. A repeated parent ID inserts the same child into the parent-to-children vector repeatedly (`:5841-5845`) and counts the same subtree repeatedly (`:5885-5889`). More importantly, the live UTXO set changes in block arrival order before canonical selection. Two nodes can receive the same competing spends in different orders, retain different accepted block sets, and have different state even if the pure path function later selects the same ID sequence.

### Finality semantics

- **Active condition:** none. `run_periodic_maintenance` no longer calls `finalize_blocks` (`qantodag.rs:4414-4419`). Production call search found only the method and explicit calls in partition/economic tests.
- **Dormant condition:** for each tip, follow the first parent for up to `FINALIZATION_DEPTH = 8`; if eight blocks are collected and unique validator strings on that window represent at least two-thirds of `validators`, mark every collected block finalized (`:3460-3533`). `total_active_stake == 0` is treated as quorum (`:3507-3509`).
- **Reversion:** no removal or rollback of `finalized_blocks` was found. The dormant method can process competing tips independently.
- **Horizon:** there is no active horizon. If the dormant method is invoked, it marks the current tip plus seven ancestors, so the tip is treated as final at depth zero rather than finalizing only an eight-deep ancestor.

## 2.E Implemented fixes in the effective worktree

| Fix | Current evidence | Verification |
|---|---|---|
| Recompute inbound block ID and require fixed difficulty before expensive validation | `qantodag.rs:2920-2937`; tests at `byzantine_tests.rs:26-51` | `cargo test -p qanto --lib byzantine_tests::rejects_ -- --nocapture`: 2 passed |
| Enforce SAGA base reward plus fees and exact coinbase distribution | `qantodag.rs:3119-3254` | Source verified; forged reward regression exists at `byzantine_tests.rs:162-203` |
| Remove automatic stake mutation based on unbound validator names/local anomaly scores | `qantodag.rs:1870-1908` | Focused double-signing regression: 1 passed |
| Disable automatic finality and dynamic sharding | `qantodag.rs:4414-4419` | Source/call-site search verified |
| Remove unsigned `effort` from fork weight and add lexical tie-breaks | `qantodag.rs:5864-5893`, `:5910-5987` | Six `determinism_tests` passed |
| Deterministically order ready children and sync topological queue | `node.rs:1841-1878`, `:1919-1943` | Source verified; no permutation/property test exists |

## 2.F Deferred items

| Priority | Remediation | Estimated effort | Reason/status |
|---|---|---:|---|
| P0 | Version validator identities as a domain-separated commitment to the PQ key and require eligibility/committee certificates on every block. | 3–8 weeks plus migration | Changes validator/address authorization and wire/genesis formats. |
| P0 | Replace SAGA reward validation with a committed integer-only rule or keep SAGA strictly advisory. | 2–5 weeks plus activation plan | Consensus/economic change; coordinate with sealed tokenomics workstream. |
| P0 | Make block-size limits fixed or epoch-committed and deterministic. | 2–5 days | Safe rule must be shared by creation, validation, gossip, and QDS. |
| P0 | Reject duplicate parents, parentless non-genesis blocks, and noncanonical roots; bind every consensus field with typed length-delimited serialization. | 1–3 weeks plus versioning | Block-ID/signature-domain migration. |
| P0 | Implement branch-scoped state, atomic block batches, canonical-path apply/unapply, and reorg tests. | 6–12+ weeks | Architectural reconstruction, not a local patch. |
| P0 | Specify and implement signed 2/3 finality certificates with safety/liveness tests under partitions. | 6–12+ weeks and external review | No safe active finality exists today. |
| P1 | Replace anomaly arithmetic with checked/saturating bounded math and keep it outside consensus. | 1–3 days | Straightforward containment. |

---

# Workstream 5 — Networking and P2P

## 5.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| NET-001 | INFO | Session identity | `Protocol_Core/qanto-node/src/p2p.rs:503-518`, `:855-871`, `:894-910` | Sessions use `(PeerId, ConnectionId)` and close removes the exact connection. The mandate's PeerId-overwrite issue is not present in the effective tree. | PRE_EXISTING_FIX |
| NET-002 | SEV-3 | Multi-connection lifecycle | `Protocol_Core/qanto-node/src/p2p.rs:855-871`, `:894-904` | Every connection adds the peer as explicit, but closing any one connection unconditionally removes the explicit peer even when another session for that PeerId remains. | CONFIRMED residual issue |
| NET-003 | SEV-1 | Startup/state sync | `Protocol_Core/qanto-node/src/node.rs:1329-1333`; `Protocol_Core/qanto-node/src/p2p.rs:1117-1183`, `:1682-1685` | Startup sends `P2PCommand::RequestState`, but the handler collects and broadcasts the requester's own blocks/UTXOs. The `StateRequest` protobuf path is explicitly unsupported. A new node has no actual request/response catch-up protocol. | CONFIRMED, launch blocker |
| NET-004 | SEV-1 | Propagation envelope | `Protocol_Core/qanto-node/src/qantodag.rs:73`, `:2955-3004`; `Protocol_Core/qanto-node/src/p2p.rs:64`, `:699-706`; `Protocol_Core/qanto-node/src/qds.rs:8-10`, `:38-65` | Consensus accepts up to 32 MiB, gossip accepts 2,000,000 bytes, and QDS reads at most 4 MiB. A locally valid block can be impossible to propagate. Full state snapshots become unpublishable even earlier. | CONFIRMED, launch blocker |
| NET-005 | SEV-1 | Snapshot trust | `Protocol_Core/qanto-node/src/p2p.rs:1392-1423`; `Protocol_Core/qanto-node/src/node.rs:957-990` | Peer snapshots carry raw blocks and UTXOs without a finalized checkpoint/state-root proof. Concurrent remediation now discards peer UTXOs and derives state only through sequential block validation. This closes arbitrary UTXO injection but leaves authenticated snapshot sync unavailable. | CONTAINED_IN_WORKTREE; sync remains blocked |
| NET-006 | SEV-2 | Reconnection | `Protocol_Core/qanto-node/src/p2p.rs:789-820`, `:1464-1485`, `:1571-1592` | Backoff and jitter exist, but `check_mesh_peers().await` calls `reconnect_with_backoff`, which sleeps 5–300 seconds inside the only swarm/select event loop. During the sleep the node does not poll swarm events or internal commands. | CONFIRMED |
| NET-007 | SEV-2 | Eclipse resistance | `Protocol_Core/qanto-node/src/p2p.rs:64-65`, `:699-706`, `:1464-1485`; `Protocol_Core/qanto-node/src/config.rs:208-224` | Mesh health requires only one peer. Defaults target four mesh peers/two outbound peers. No IP prefix/ASN diversity, inbound/outbound connection cap, anchor protection, peer rotation, or eviction policy was found. | CONFIRMED, must fix before public exposure |
| NET-008 | SEV-2 | Wire precision and commitments | `Protocol_Core/qanto-node/src/p2p.rs:1796-1836`, `:1930-1952`, `:1966-1981` | Difficulty is encoded as `f64`, effort narrows `u128 -> u64`, and carbon credential integers round-trip through scaled `f64`. Fixed difficulty happens to be exactly representable and effort is now ignored, but unsigned carbon data feeds local SAGA analysis. | CONFIRMED protocol-schema debt |
| NET-009 | SEV-2 | Peer authentication | `Protocol_Core/qanto-node/src/p2p.rs:66`, `:1267-1308`, `:1689-1708` | The fallback HMAC secret is a public hard-coded constant. Envelope signatures prove only possession of the self-declared embedded PQ key; no binding to libp2p PeerId, validator identity, or snapshot authority was found. | CONFIRMED |
| NET-010 | SEV-3 | Fast-sync claim | `Protocol_Core/qanto-node/src/qantodag.rs:107`, `:4468-4495` | `FAST_SYNC_BATCH_SIZE = 5000` is used only to limit validation-cache cleanup. It is not a network sync batch and has no disconnect resume/backoff semantics. | CONFIRMED specification mismatch |
| NET-011 | INFO | Discovery/transports | `Protocol_Core/qanto-node/src/p2p.rs:537-551`, `:575-605`, `:620-658`, `:763-770`, `:826-840` | The canonical server uses static/cached seeds plus Kademlia DHT, DNS, WebSocket/WSS, Noise, Yamux, and periodic Kademlia traffic. No mDNS behaviour is wired in this server. | CONFIRMED |

## 5.B P2P lifecycle assessment

- **Discovery:** configured seeds and cached Kademlia addresses are added at startup; Kademlia bootstrap and closest-peer queries provide discovery. There is no canonical mDNS component.
- **Transport:** DNS + WebSocket with Noise and Yamux is built at `p2p.rs:575-591`. The 30-second Kademlia keepalive is intended for WSS proxy/NAT survival.
- **Sessions:** compound session keys correctly preserve multiple connections, but explicit-gossip peer status is not reference counted.
- **Gossip authentication:** shared run-loop rate limiters and blacklist are persistent (`p2p.rs:773-807`), and the worktree adds 24-hour blacklist expiry. HMAC adds no identity assurance when every deployment shares or falls back to the same secret.
- **Sync:** there is no real state request, checkpoint selection, chunk cursor, resume token, authenticated state root, or peer scoring for bad snapshots.
- **Partition recovery:** exponential delay/jitter exists, but sleeping in the swarm loop makes recovery itself halt networking.

## 5.C Implemented fixes in the effective worktree

| Fix | Evidence | Verification |
|---|---|---|
| Compound session key and exact close removal | `p2p.rs:512`, `:865-866`, `:904` | Pre-existing source fix; no multi-connection regression test |
| Timestamped blacklist with 24-hour expiry | `p2p.rs:74`, `:780-787`, `:1224-1255` | Six focused P2P tests passed; tests cover valid forward, HMAC/signature/rate-limit blacklist, and monetary protobuf parsing |
| Discard unauthenticated snapshot UTXOs | `node.rs:957-990` | Source verified; block-only sync still lacks checkpoint tests |
| Deterministic pending/snapshot block ordering | `node.rs:1841-1878`, `:1919-1943` | Source verified |

## 5.D Deferred items

| Priority | Remediation | Estimated effort | Reason/status |
|---|---|---:|---|
| P0 | Implement explicit state/header request-response with authenticated finalized checkpoints, bounded chunks, cursor/resume, peer penalties, and block/state-root verification. | 4–10 weeks | Current startup sync is nonfunctional. |
| P0 | Define one maximum encoded block size across consensus, protobuf envelope, gossip, and QDS; reject oversized candidates before mining. | 2–5 days plus interoperability test | Current limits contradict each other. |
| P0 | Move reconnect scheduling into timers/state machines so the swarm is always polled. | 2–5 days | Necessary for partition recovery. |
| P1 | Add connection caps, /16-/24/ASN diversity, protected outbound anchors, rotation, duplicate-IP limits, and peer scoring. | 2–4 weeks plus network simulation | Eclipse controls are absent. |
| P1 | Replace lossy protobuf numeric fields and bind the envelope signer to peer/validator identity. | 1–3 weeks plus protocol version | Wire compatibility change. |
| P1 | Reference-count explicit peers by remaining connection count. | 1–2 days | Residual multi-connection correctness. |

---

# Workstream 6 — Transaction Engine, Mempool, and UTXO State

## 6.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| TX-001 | SEV-1 | Native spend authorization | `Protocol_Core/qanto-node/src/transaction.rs:496-548`, `:842-900`, `:903-947`, `:980-1048` | Verification reconstructs the public key from `signature.signer_public_key`, then verifies that key against itself. UTXO ownership checks only compare the UTXO address to the attacker-controlled `sender`; no native rule derives/binds `sender` to the PQ key. | CONFIRMED, launch blocker |
| TX-002 | SEV-2 | Mempool coinbase admission | `Protocol_Core/qanto-node/src/transaction.rs:698-701`, `:1051-1060`, `:1389-1397`; `Protocol_Core/qanto-node/src/mempool.rs:1374-1393` | Because every inputless transaction is classified as coinbase, accepting one from peers could poison candidate construction. Mempool prevalidation now rejects all inputless/coinbase transactions, with a regression test. | FIXED_IN_WORKTREE |
| TX-003 | SEV-2 | Concurrent mempool admission | `Protocol_Core/qanto-node/src/mempool.rs:1295-1322`, `:1395-1445`, `:1462-1514` | `max_transactions` is ignored. Duplicate, conflicting-input, and byte-capacity checks are check-then-act operations across concurrent maps/atomics; two readers can both pass and insert conflicting transactions or oversubscribe the byte cap. | CONFIRMED |
| TX-004 | SEV-3 | Mempool indices | `Protocol_Core/qanto-node/src/mempool.rs:72-92`, `:1516-1538`, `:1774-1806`, `:1815-1873` | Removal/pruning updates the transaction map and BTree priority map but does not remove entries from the global/sharded BinaryHeaps. Selection skips stale map entries, preserving correctness at the cost of unbounded stale heap memory/scan overhead. | CONFIRMED |
| TX-005 | SEV-1 | UTXO fork/reorg semantics | `Protocol_Core/qanto-node/src/qantodag.rs:1820-1830`, `:1910-1925`, `:2072-2091`, `:5676-5785` | A consensus mutex closes concurrent `add_block` double-spend races and the planner rejects duplicate inputs within a block. It does not solve competing branches: the first accepted branch consumes the one global UTXO, and no undo/reorg path exists. | PARTIALLY_HARDENED, launch blocker |
| TX-006 | SEV-2 | Elite mempool | `Protocol_Core/qanto-node/src/node.rs:494-520`, `:1531-1534`; `Protocol_Core/qanto-node/src/elite_mempool.rs:345-399`, `:528-541`, `:554-559` | The node constructs and stores `EliteMempool` but never starts or submits to it. If started, both worker and synchronous paths mark transactions valid without signature/UTXO validation. Priority is only fee-per-byte × size factor; no SAGA ordering or gas-fee integration exists. | SPECIFIED_BUT_UNIMPLEMENTED; dormant |
| TX-007 | SEV-3 | Gas/SAGA fee model | `Protocol_Core/qanto-node/src/gas_fee_model.rs:151-194`; `Protocol_Core/qanto-node/src/saga.rs:3585-3605`; `Protocol_Core/qanto-node/src/transaction.rs:980-1004` | Rich gas and SAGA dynamic-fee calculators exist, but canonical block validation enforces only minimum fee/gas and does not recompute the declared fee from either model. SAGA's dynamic fee ignores caller Karma. | CONFIRMED specification gap |
| TX-008 | INFO | Intra-block conflict detection | `Protocol_Core/qanto-node/src/qantodag.rs:5676-5785` | The commit planner records per-transaction inputs and rejects duplicates within a transaction and across the entire block before applying changes. | CONFIRMED positive control |

## 6.B Transaction and state path

1. Transaction IDs are recomputed during signature verification (`transaction.rs:509-517`). EVM raw transactions additionally recover and compare the EVM sender/receiver/amount (`:519-540`). Native transactions lack the equivalent key-to-address step.
2. Mempool admission performs structural limits, UTXO existence, a scan for pending conflicts, full signature/UTXO verification, byte-cap check, then separate map/queue/heap inserts (`mempool.rs:1374-1543`). The sequence is not atomic across concurrent callers.
3. Block validation verifies each non-coinbase transaction against a shared read snapshot (`qantodag.rs:3256-3302`).
4. `plan_utxo_changes_conflict_free` rechecks existence and detects duplicate spends across the block (`:5676-5785`).
5. `add_block` applies those changes to the one live UTXO map, asynchronously enqueues persistence, updates balances, and only later inserts the block (`:1910-2074`).
6. Fork choice is queried separately and does not control which state transitions are applied. No state rollback method was found.

## 6.C Implemented fixes in the effective worktree

| Fix | Evidence | Verification |
|---|---|---|
| Reject inputless transactions at the mempool boundary | `transaction.rs:1051-1060`, test `:1389-1397` | Focused test: 1 passed |
| Serialize `add_block` validation-through-commit and reject duplicate inputs in the block planner | `qantodag.rs:1827-1830`, `:5676-5785` | Pre-existing hardening; no adversarial concurrent/reorg property test found |
| Evict mempool transactions whose UTXOs were consumed by accepted blocks | `mempool.rs:1808-1873` | Pre-existing control; stale heap indices remain |

## 6.D Deferred items

| Priority | Remediation | Estimated effort | Reason/status |
|---|---|---:|---|
| P0 | Bind native sender/address and every input owner to an authenticated PQ identity. | 3–8 weeks plus wallet/address migration | Authorization and persisted-address change. |
| P0 | Introduce branch-scoped UTXO/state roots and atomic apply/unapply journal for canonical reorgs. | 6–12+ weeks | Consensus architecture change. |
| P1 | Add an atomic input-reservation index and atomic byte/count capacity accounting to the standard mempool. | 1–2 weeks | Prevents conflicting concurrent admission and cap overshoot. |
| P1 | Remove/rebuild stale heap entries on removal, or use one authoritative indexed priority structure. | 3–7 days | Memory/latency hardening. |
| P1 | Either remove the dormant elite path or make it share canonical validation, fee, conflict, capacity, and reservation code. | 1–3 weeks | Current worker validation is a stub. |
| P1 | Specify whether gas/SAGA fees are advisory wallet estimates or consensus rules; do not use local/floating state in consensus. | 1–3 weeks | Current documentation and acceptance differ. |

---

# Workstream 7 — SAGA, ΩMEGA, and Green Mining

## 7.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| SAGA-001 | SEV-1 | Consensus reward inputs | `Protocol_Core/qanto-node/src/saga.rs:3754-3855`; `Protocol_Core/qanto-node/src/omega.rs:329-445` | SAGA reward depends on process-local floating state. ΩMEGA initializes randomly, evolves from local action order/time, and changes a global threat flag; SAGA maps that flag directly into the required block reward. Local API traffic can therefore change consensus acceptance. | CONFIRMED, launch blocker |
| SAGA-002 | SEV-1 | Autonomous edicts | `Protocol_Core/qanto-node/src/qantodag.rs:4421-4428`; `Protocol_Core/qanto-node/src/saga.rs:3860-3890`, `:3892-3950`, `:4266-4337` | Each local maintenance call increments an epoch, classifies threats/congestion from local DAG telemetry, and may create a UUID edict with floating reward/fee multipliers. The reward multiplier is consumed by mandatory validation but the edict is not a committed on-chain transition. | CONFIRMED, launch blocker |
| SAGA-003 | SEV-2 | “AI” inference | `Protocol_Core/qanto-node/src/saga.rs:810-828`, `:1254-1278`, `:1555-1584`, `:2018-2055`, `:2058-2076`, `:3704-3716` | A custom neural-network/training implementation exists, but no production call to `forward_pass` or `evaluate_block_with_saga` was found, block evaluation explicitly removed training-data collection, and no code appends to `training_data_buffer.samples`. Periodic retraining therefore returns for insufficient data. Active attack classification is heuristic. | CONFIRMED, mostly dormant |
| SAGA-004 | SEV-2 | Model integrity | `Protocol_Core/qanto-node/src/saga.rs:2018-2055` | Models can be serialized/deserialized as caller-selected JSON with no signature, version, hash, origin, quorum, or activation rule. No production load call was found, so poisoning is currently dormant; wiring it without an authenticated model-governance protocol would be unsafe. | CONFIRMED dormant risk |
| SAGA-005 | SEV-1 | Carbon credentials | `Protocol_Core/qanto-node/src/saga.rs:3625-3701`, `:4398-4428`; `Protocol_Core/qanto-node/src/node.rs:1022-1029` | “Verification” accepts the string `signed_by_<issuer_id>`, a hard-coded project allowlist, and a hard-coded year. It performs no cryptographic issuer check and ignores geospatial/additionality/issuer-reputation fields. Accepted network credentials change local environmental metrics used by the reward model. | CONFIRMED, launch blocker while SAGA affects reward |
| SAGA-006 | SEV-2 | Embedded carbon claims | `Protocol_Core/qanto-node/src/qantodag.rs:623-660`, `:2915-3319`; `Protocol_Core/qanto-node/src/saga.rs:1403-1425`, `:2445-2463` | Block `carbon_credentials` are outside the block ID/signature and are not verified in `is_valid_block`. SAGA later reads them for environmental/oracle heuristics. A relay can mutate these fields without invalidating the producer signature. | CONFIRMED |
| SAGA-007 | SEV-2 | Karma and green incentives | `Protocol_Core/qanto-node/src/saga.rs:3222-3235`, `:3585-3605`, `:3978-4067`, `:4138-4189` | Karma sources cover governance/SAGA actions, not carbon credentials. Dynamic fee ignores address/Karma, and no canonical validator-selection fee reduction uses Karma. The documented green-mining fee discount is absent. | CONFIRMED specification gap |
| SAGA-008 | SEV-2 | ΩMEGA final gate | `Protocol_Core/qanto-node/src/omega.rs:487-523`; `Protocol_Core/qanto-node/src/transaction.rs:550-605`; `Protocol_Core/qanto-node/src/node.rs:2252-2301`; `Protocol_Core/qanto-node/src/qantodag.rs:2915-3319` | Core ΩMEGA gates local transaction construction/API admission and can be bypassed with `QANTO_OMEGA_DISABLE`. It is not invoked by canonical block validation. `omega_enhanced` is an internally used simulation/orchestrator module, not a node final gate. | CONFIRMED, not implemented as specified |
| SAGA-009 | SEV-2 | Validator penalties | `Protocol_Core/qanto-node/src/qantodag.rs:1870-1908` | Equivocation and anomaly detection previously mutated stake using an unbound validator string/local heuristic. The worktree now rejects equivocation without slashing and makes anomaly scoring advisory. | CONTAINED_IN_WORKTREE |
| SAGA-010 | SEV-3 | Dashboard claims | `Protocol_Core/qanto-node/src/saga.rs:4761-4794` | When no training exists, the dashboard reports hard-coded/default accuracy, prediction confidence, drift, latency, threat level, anomaly score, and security confidence rather than measured inference. | CONFIRMED |
| SAGA-011 | SEV-2 | Adaptive controls vs specification | `Protocol_Core/qanto-node/src/saga.rs:3585-3605`, `:4266-4337`; `Protocol_Core/qanto-node/src/qantodag.rs:1506-1509`, `:2955-2992` | Edicts change reward/fee multipliers only. Difficulty is fixed, and block size comes from local chain-load counters. The documented SAGA block-size/difficulty adaptation is not implemented as a deterministic protocol. | CONFIRMED specification gap |

## 7.B SAGA/ΩMEGA implementation classification

### Telemetry and attack classification

`update_network_state` invokes explicit heuristic detectors for Sybil, spam, centralization, collusion, oracle manipulation, time drift, wash trading, economic attacks, and mempool front-running (`saga.rs:3892-3935`). It sorts risk scores and applies hard-coded thresholds (`:3940-3949`). This is heuristic rules code, not live neural inference.

### Neural component

- A six-layer custom network, forward pass, backpropagation, training loop, and JSON save/load code exist.
- The network is initialized in memory; no repository model artifact or canonical model hash is selected.
- Source search found no production `forward_pass` call.
- Source search found no append/extend into `training_data_buffer.samples`.
- `evaluate_block_with_saga` explicitly says training-data collection was removed and itself has no caller.
- Periodic retraining remains scheduled every ten local epochs but exits when fewer than 32 samples exist.

Result: **the active SAGA security path is heuristic; the neural subsystem is implemented scaffolding but not an operating inference/training control loop.**

### Green mining

- The geospatial consistency and issuer reputation fields exist in the credential schema but are not validated.
- The credential signature is a predictable string, not a cryptographic signature.
- Environmental aggregation uses tonnes × hard-coded project quality and a floating `total_offset / 1000` score.
- No carbon credential awards Karma, and the dynamic-fee API does not accept an address or read Karma.

Result: **green scoring is simulated/untrusted and has no implemented Karma fee reduction.** Because environmental state feeds SAGA's reward premium, it must be removed from consensus immediately or replaced with deterministic authenticated inputs.

### ΩMEGA

ΩMEGA is a local rate/stability heuristic based on random initial identity bytes, `Instant`, action order, Shannon entropy, and floating exponential decay (`omega.rs:329-445`). It can reject local API/constructor transactions and change local threat level. It neither validates incoming gossip transactions at consensus nor gates blocks. Its threat flag must not influence any consensus value.

## 7.C Implemented fixes in the effective worktree

| Fix | Evidence | Verification |
|---|---|---|
| Disable automatic slashing for equivocation attributed through an unbound validator string | `qantodag.rs:1870-1892` | Focused regression passed |
| Make local anomaly scoring advisory rather than stake mutating | `qantodag.rs:1894-1908` | Source verified |
| Disable automatic dynamic sharding, which used local loads/time/random key material | `qantodag.rs:3557-3654`, `:4414-4419` | Source/call-site search verified |

## 7.D Deferred items

| Priority | Remediation | Estimated effort | Reason/status |
|---|---|---:|---|
| P0 | Remove all SAGA/ΩMEGA/local telemetry inputs from block validity and monetary emission; keep them advisory until an epoch-committed integer state machine exists. | 1–3 weeks containment; 8–16+ weeks for protocolized governance | Current reward path can split honest nodes. |
| P0 | Cryptographically authenticate carbon issuers/projects, define replay/retirement semantics, and make geospatial evidence externally verifiable—or disable green rewards. | 4–12+ weeks plus external oracle/audit | Current credentials are forgeable. |
| P1 | Decide whether neural SAGA is a real product feature. If yes, define signed model artifacts, feature schema, deterministic/non-consensus inference boundary, poisoning controls, evaluation, rollback, and governance. | 8–16+ weeks | Current neural path is dormant scaffolding. |
| P1 | Remove hard-coded AI/security dashboard numbers and publish only measured metrics with sample counts/model versions. | 2–5 days | Prevents misleading operational claims. |
| P1 | Specify Karma sources and effects; if fees/selection are consensus relevant, implement with integer, committed state and migration tests. | 2–6 weeks | Documented green fee benefit is absent. |
| P1 | Replace unchecked anomaly math with bounded checked integer operations and add extreme-value property tests. | 1–3 days | Prevents validation DoS. |

---

# Targeted verification and test inventory

Commands were executed through the repository-required `rtk` wrapper against the effective working tree.

| Command | Result | What it establishes |
|---|---|---|
| `cargo test -p qanto --lib determinism_tests -- --nocapture` | PASS: 6 passed, 150 filtered out | Existing witness/order/LCA/GHOST unit tests pass after fork-choice changes. |
| `cargo test -p qanto --lib p2p::tests -- --nocapture` | PASS: 6 passed, 150 filtered out | Gossip forwarding, HMAC/signature/rate-limit blacklist, and protobuf monetary parsing tests pass. |
| `cargo test -p qanto --lib mempool_rejects_inputless_coinbase_like_transactions -- --nocapture` | PASS: 1 passed, 155 filtered out | New external-coinbase containment passes. |
| `cargo test -p qanto --lib byzantine_tests::rejects_ -- --nocapture` | PASS: 2 passed, 154 filtered out | Forged block ID and zero declared difficulty are rejected. |
| `cargo test -p qanto --lib byzantine_tests::test_byzantine_double_signing_rejected_without_unbound_slashing -- --nocapture` | PASS: 1 passed, 155 filtered out | Equivocation is rejected without mutating another validator's stake. |

Relevant existing suites/modules include:

- `Protocol_Core/qanto-node/src/byzantine_tests.rs`: block ID/difficulty, equivocation, forged reward, Merkle root, coinbase signature, and clock drift.
- `Protocol_Core/qanto-node/src/partition_tests.rs`: 60/40, 70/30, 34/33/33, and healing scenarios; these call the currently disabled/public finalizer directly.
- `Protocol_Core/qanto-node/src/economic_attack_tests.rs`: stake concentration, cartel, reward farming, and bridge-drain scenarios.
- `Protocol_Core/qanto-node/src/qantodag.rs:6212-6540`: six determinism/fork unit tests.
- `Protocol_Core/qanto-node/src/p2p.rs:1984-2334`: six message/encoding tests.

Critical zero-coverage gaps in this scope:

1. two nodes receiving the same competing blocks in different orders and proving identical accepted set, canonical path, UTXO root, balances, and finality;
2. duplicate-parent rejection and branch-weight non-amplification;
3. canonical genesis/extra-root rejection;
4. SAGA reward equality across nodes with different local API traffic, maintenance cadence, credentials, and threat history;
5. atomic failure injection at every persistence enqueue/state mutation step;
6. reorg apply/unapply with conflicting spends and mempool reinsertion;
7. concurrent mempool admission of the same ID, same UTXO, and near-capacity transactions;
8. live multi-connection PeerId close behavior;
9. reconnect during a partition while continuing to service swarm events/commands;
10. state sync request, checkpoint authentication, bounded chunk resume, malicious peer, and disconnect recovery;
11. propagation of blocks at every consensus/gossip/QDS size boundary;
12. eclipse simulations with correlated IP prefixes and hostile bootstrap peers.

# Final readiness verdict

## **NOT READY**

Minimum conditions before a public testnet launch in these four workstreams:

1. cryptographically bind native senders and validator identities to their PQ keys;
2. replace local/floating SAGA reward inputs with deterministic committed integer consensus, or remove SAGA/ΩMEGA from validity entirely;
3. make block-size acceptance deterministic and identical to every transport limit;
4. implement atomic branch-scoped state, deterministic canonical apply/unapply, and reorg recovery;
5. reject duplicate parents and all noncanonical roots;
6. implement a real authenticated, resumable sync protocol;
7. keep unsafe finality disabled until signed quorum finality is specified, implemented, and partition-tested;
8. make reconnection nonblocking and deploy minimum eclipse-resistance controls;
9. make standard mempool conflict/capacity admission atomic;
10. pass multi-node adversarial convergence tests covering all conditions above.

The current worktree fixes are worthwhile containment, not evidence of mainnet or public-testnet consensus readiness.
