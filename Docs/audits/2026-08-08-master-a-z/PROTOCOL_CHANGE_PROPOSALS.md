# Qanto Protocol Change Proposals

Audit date: 2026-08-08  
Status: **DRAFT — EXPLICIT APPROVAL REQUIRED**  
Applies to: unresolved consensus, cryptographic, economic, networking, SAGA, state, finality, and bridge blockers confirmed in this audit

## 1. Purpose and change-control rule

These proposals convert confirmed findings into reviewable protocol decisions. They are not authorization to change consensus, cryptographic identity, monetary policy, genesis, or bridge authorization.

No proposal below may be implemented or activated until the repository owner or designated protocol-governance authority approves:

1. the normative invariant and byte-level specification;
2. the compatibility and state-migration plan;
3. the activation and emergency-halt rules;
4. the test and independent-review gates; and
5. the explicit unresolved choices listed under “Approval needed.”

The sealed constants at <code>Protocol_Core/qanto-node/src/lib.rs:164-187</code> are inputs to QIP-003 and are **not modified by this document**.

## 2. Proposal index and dependency order

| QIP | Title | Primary confirmed blockers | Depends on | Activation class |
|---|---|---|---|---|
| QIP-001 | PQ-bound accounts, transactions, validators, and peers | PQC-001, PQC-002, CNS-001, TX-001, NET-009 | QIP-002 format/hash decision | Consensus hard fork plus wallet/genesis migration |
| QIP-002 | Canonical domain-separated hashing and one QanHash backend specification | CRY-002, POW-001, POW-002, CNS-005, CNS-007 | None; co-designed with QIP-001 | Consensus hard fork or new public-testnet genesis |
| QIP-003 | Deterministic issuance, fee accounting, supply state, and coherent genesis | ECO-003 through ECO-007, CNS-003 | QIP-004 state model; QIP-006 containment | Monetary-policy activation plus state migration or genesis reset |
| QIP-004 | Branch-scoped state, atomic block commit, canonical reorg, and certificate finality | CNS-007 through CNS-010, TX-005 | QIP-001 and QIP-002 | Consensus/state-format hard fork |
| QIP-005 | Versioned transport envelope and authenticated resumable state sync | NET-003 through NET-009 | QIP-001 identity; QIP-004 state root/finality | Wire-protocol version and coordinated rollout |
| QIP-006 | SAGA/ΩMEGA advisory-only boundary and deterministic governance activation | SAGA-001, SAGA-002, SAGA-005, SAGA-006, SAGA-008, ECO-003 | QIP-003 for monetary rules | Consensus behavior change; safe containment may precede activation |
| QIP-007 | Real bridge proof verification and atomic nullifier state | ZK-001 through ZK-005, CNS-012 | QIP-002, QIP-004, QIP-005 | Bridge protocol version; bridge remains disabled until complete |

Normative terms in these drafts use **MUST**, **MUST NOT**, **SHOULD**, and **MAY** in their ordinary protocol-specification sense.

---

# QIP-001 — PQ-bound accounts, transactions, validators, and peers

Status: DRAFT / APPROVAL REQUIRED  
Security objective: possession of an authenticated key must be necessary and sufficient for every identity-bearing authorization.

## Confirmed problem

Native transaction verification accepts the public key embedded by the signer and never derives the sender or UTXO owner from that key. Block verification similarly accepts the ML-DSA public key embedded in the block signature without binding it to <code>block.validator</code>. P2P envelope keys are not bound to the libp2p PeerId or validator record.

## Proposed invariant

1. Every account address MUST be a versioned, domain-separated commitment to:
   - network/chain identifier;
   - signature-suite identifier;
   - canonical public-key length; and
   - canonical public-key bytes.
2. A native spend MUST prove that the authenticated witness key derives the sender address and every consumed UTXO owner under the same address version.
3. A transaction signature MUST cover the complete QIP-002 canonical transaction payload, chain ID, transaction version, and signature-suite ID.
4. A validator ID MUST be derived from, or explicitly certified to, its consensus key. A block MUST be rejected unless:
   - the signer key maps to the declared validator ID;
   - the validator exists in the epoch validator-set commitment;
   - the key is active for that epoch; and
   - any producer/committee eligibility proof required by the approved consensus design is valid.
5. A peer envelope MUST bind its application signing key to the transport PeerId. Validator-origin messages MUST additionally bind to the active validator record.
6. Legacy free-form sender and validator strings MUST NOT authorize value, rewards, votes, slashing, or finality after activation.
7. Verification MUST reject unknown versions, algorithms, malformed lengths, non-canonical encodings, and empty keys.

## Decisions deliberately left open

- ML-DSA-only identity versus a hybrid classical plus ML-DSA authorization rule.
- Whether an account has one immutable key or a versioned on-chain rotation record.
- Whether validator consensus, finality, transaction, and networking keys are separate certified roles.
- The exact address text encoding and checksum.

These are protocol decisions, not implementation details.

## Compatibility and migration

1. Freeze a byte-exact AddressV2 and ValidatorRecordV2 specification after QIP-002 chooses the hash and canonical serialization.
2. Add read-only legacy decoding before activation, but never treat a legacy unbound sender string as proof of ownership.
3. Produce an auditable migration manifest mapping each legitimate legacy allocation to a V2 address. Each migration entry MUST be authorized by an approved proof or regenerated genesis policy; there is no safe automatic derivation from an unbound legacy sender.
4. Update genesis, UTXO keys, wallet/keystore formats, hardware-wallet interfaces, RPC schemas, indexers, explorers, relayer identities, fixtures, and operator runbooks together.
5. Activate at a new public-testnet genesis or explicit height with a protocol-version bit. Nodes that do not understand V2 MUST stop before activation.
6. Preserve the old state and manifest as immutable audit artifacts.

## Rollback and emergency handling

- Before activation: disable V2 production and return to the last compatible release.
- During migration rehearsal: restore the pre-migration database snapshot and rerun the deterministic manifest.
- After activation: an in-place rollback to unbound identity is forbidden. The only safe responses are emergency halt, a new approved corrective fork, or a testnet reset.
- Key compromise handling requires a separately approved rotation/revocation rule; operator discretion alone MUST NOT rewrite ownership.

## Required tests and evidence

- Golden vectors for every supported key/address/version and malformed-length case.
- Property tests proving that changing key, chain ID, suite ID, role, or payload changes the commitment.
- Negative tests where an attacker signs with a valid self-selected key while claiming another sender/validator.
- UTXO migration replay from genesis with exact pre/post supply equality.
- Cross-implementation vectors for node, wallet, relayer, RPC, and explorer code.
- Validator rotation, expiry, duplicate-key, wrong-epoch, and revoked-key tests.
- ML-DSA known-answer/malformed-input tests and an external side-channel/library review.
- Multi-node activation rehearsal including old-node rejection.

## Principal risks

- Incorrect migration can strand balances or assign them to an attacker.
- Hybrid-policy ambiguity can create downgrade acceptance.
- Address changes break every persisted index and external integration.
- A compromised migration signer or manifest becomes a supply-integrity event.
- Unmaintained PQ dependencies remain a supply-chain risk even after local binding is correct.

## Approval needed

Explicit approval must name:

1. the signature policy: ML-DSA-only or specified hybrid;
2. address and validator record formats;
3. key separation/rotation policy;
4. the authority and proof used for each legacy allocation migration;
5. activation mechanism: new genesis or height; and
6. the independent cryptography reviewer and acceptance criteria.

---

# QIP-002 — Canonical domain-separated hashing and one QanHash backend specification

Status: DRAFT / APPROVAL REQUIRED  
Security objective: no two semantically distinct consensus objects may share a canonical commitment, and every mining backend must return identical bytes.

## Confirmed problem

The current canonical hash zero-pads a partial word without committing the message length or a delimiter, so distinct byte strings such as a value and that value followed by zero bytes can collide by construction. Block ID/signature payloads omit accepted consensus fields. CPU, OpenCL, CUDA, and Metal mining implementations execute different algorithms; CUDA and Metal even disagree on a round constant.

## Proposed invariant

1. Every consensus object MUST use a versioned canonical encoding with:
   - an object-type domain tag;
   - protocol version;
   - chain ID;
   - fixed integer endianness;
   - explicit field tags/order;
   - explicit lengths for variable data;
   - canonical collection ordering where order is semantic; and
   - rejection of duplicate or unknown consensus-critical fields.
2. Transaction ID, block ID, Merkle node/leaf, address, validator ID, state root, finality vote, bridge claim/nullifier, and P2P envelope MUST use distinct domain tags.
3. A BlockV2 commitment MUST cover every field that can affect acceptance, fork choice, state, reward, bridge behavior, finality, or application execution. Candidate construction MUST finish all committed fields before signing or mining.
4. The approved hash primitive/construction MUST commit message length, be collision resistant, have public cross-language vectors, and be implemented without platform-dependent casts or native-endian behavior.
5. The QanHash PoW algorithm MUST have one byte-exact normative specification. A simple portable CPU reference implementation MUST be the correctness oracle.
6. GPU/OpenCL/CUDA/Metal results MUST be CPU-reverified until a backend passes the full conformance suite. A backend that differs on any vector MUST be disabled.
7. Genesis and parent rules MUST reject duplicate parent IDs, parentless non-genesis blocks, and noncanonical roots. A repeated reference MUST never multiply fork weight.

## Decisions deliberately left open

- Standard hash choice and output size.
- Whether QanHash remains the PoW function or is replaced.
- Which currently optional block fields remain consensus fields versus being removed from BlockV2.
- Whether public testnet resets to a clean V2 genesis or uses a height-based transition with dual historical indexing.

## Compatibility and migration

1. Publish a standalone canonical-encoding specification and machine-readable golden-vector corpus before node changes merge.
2. Introduce explicit V2 object and wire discriminants; never infer version from length.
3. Store legacy and V2 object IDs in separate namespaces. Do not silently reinterpret old IDs.
4. For a height activation, the first V2 block MUST commit the approved V1 checkpoint and V2 state root. New genesis is preferred where no state-preservation obligation exists because it removes dual-hash ambiguity.
5. Rebuild or version transaction/block indexes, Merkle stores, wallet caches, explorer records, proof formats, and mining work templates.
6. Keep nonconforming accelerator backends disabled until hardware-specific parity artifacts are published.

## Rollback and emergency handling

- Before activation: miners and nodes may revert to the last V1 release.
- After V2 activation: nodes MUST NOT produce V1 objects. A detected commitment defect requires emergency halt and a newly versioned corrective fork/reset.
- Accelerator rollback is independent: disable the backend and CPU-reverify without changing consensus, provided CPU is the normative algorithm and produces the same approved result.
- Preserve the activation checkpoint, databases, and vector corpus for forensic replay.

## Required tests and evidence

- Golden vectors for empty, one-byte, trailing-zero, boundary-length, maximum-length, Unicode/text rejection where bytes are required, and nested collection cases.
- Explicit regression proving the current trailing-zero collision is absent in V2.
- Mutation tests for every BlockV2 and TransactionV2 field.
- CPU vectors across x86_64, aarch64, little-endian, and an emulated big-endian target where feasible.
- CUDA, Metal, and OpenCL parity across epochs, DAG boundaries, nonces, and invalid-output controls.
- Differential tests between at least two independent canonical-encoding implementations.
- Fork-choice permutation/property tests with duplicate-parent and multi-root adversarial sets.
- Storage/index migration and mixed-version network rehearsal.

## Principal risks

- A serialization mismatch is an immediate consensus split.
- Dual-format acceptance can create replay or downgrade paths.
- Hash migration invalidates IDs, Merkle proofs, PoW work, indexes, and bridge proofs.
- Treating a fast backend as normative before conformance recreates consensus risk.
- A custom hash choice would expand cryptographic review scope materially.

## Approval needed

Explicit approval must name:

1. the standardized hash/construction and output size;
2. the complete domain-tag registry;
3. the BlockV2/TransactionV2 field set and encoding;
4. the normative QanHash algorithm and CPU oracle;
5. whether accelerators remain disabled at launch;
6. activation by new genesis or height; and
7. the independent conformance reviewer.

---

# QIP-003 — Deterministic issuance, fee accounting, supply state, and coherent genesis

Status: DRAFT / ECONOMIC AND CONSENSUS APPROVAL REQUIRED  
Security objective: every honest node derives exactly the same subsidy and total issued supply from canonical state, and issuance can never exceed an approved sealed budget.

## Confirmed problem

The live reward path uses process-local SAGA/ΩMEGA state and floating point rather than the integer emission engine. Supply accounting silently clamps an internal counter after ledger mutation, counts fees as issuance, resets on reconstruction, and is not initialized from the shipped genesis. The shipped genesis allocates the full 21B supply while mining still calculates a subsidy. The standalone engine is capped to total supply rather than the sealed 16.8B community allocation.

## Proposed invariant

1. The sealed constants and their compile-time sum assertion remain unchanged unless separately authorized.
2. Consensus subsidy MUST be computed from canonical integer state only. No floating point, wall-clock-local telemetry, random state, API activity, SAGA credit, ΩMEGA flag, oracle score, or environmental metric may affect acceptance.
3. Transaction fees MUST be accounted as transfers from spenders to approved recipients, never as new issuance.
4. Issued subsidy MUST be a persisted/replayable canonical state value or be exactly derivable from canonical history.
5. Before commit, validation MUST enforce:

~~~text
genesis_issued + cumulative_subsidy + proposed_subsidy
    <= approved_subsidy_budget
total_ledger_supply <= MAX_TOTAL_SUPPLY
~~~

6. The approved mining budget MUST not consume the sealed ecosystem/development or liquidity allocations.
7. Restart, snapshot sync, reorg, and full replay MUST produce the same issued-supply counter and balances.
8. The subsidy included in a block MUST equal the approved integer schedule clamped to the exact remaining budget; a block exceeding the remainder MUST be rejected.

## Coherent genesis choices requiring a decision

| Choice | Genesis interpretation | Mining subsidy |
|---|---|---|
| A — Full-supply prefund | Genesis allocates all 21B across approved recipients | Zero from genesis; mining can receive fees only unless supply is burned under a separately approved rule |
| B — Reserved allocations plus community emission | Genesis allocates only the approved ecosystem/development and liquidity portions, plus any explicitly approved community genesis amount | Remaining community allocation may be emitted under the approved integer schedule |
| C — New allocation policy | A different distribution or schedule | Requires a separate economic proposal; this draft does not authorize it |

The current combination—full 21B genesis plus a fresh reward counter—is invalid under every choice.

## Decisions deliberately left open

- Choice A, B, or a separately specified C.
- Whether schedule periods derive from canonical height/epoch or the current bounded consensus timestamp.
- Exact multi-chain subsidy allocation and treatment of missed slots.
- Fee recipient split and whether any fee burn exists.
- Whether public testnet starts from new genesis or migrates an existing state.

## Compatibility and migration

1. Publish an executable economic specification with atomic-unit vectors and a genesis allocation manifest.
2. Add versioned canonical fields for cumulative subsidy, allocation balances, and schedule period to QIP-004 state.
3. For retained history, replay genesis and every canonical block to produce a reconciliation report. Any unexplained delta blocks activation.
4. A public-testnet reset SHOULD be used if existing state has no preservation requirement. A value-bearing network requires a formally reviewed state migration.
5. Update coinbase format, RPC fields, explorer supply metrics, wallet fee/reward displays, monitoring, and documentation together.
6. SAGA remains advisory under QIP-006; economic parameter changes require explicit on-chain/versioned activation and cannot mutate locally.

## Rollback and emergency handling

- Before activation: revert code/config and preserve the approved genesis candidate.
- During rehearsal: restore the pre-migration snapshot and rerun reconciliation.
- After issuance under the new rule: ordinary rollback is unsafe. Halt block production, preserve the last finalized state, and use an approved corrective fork/reset.
- A cap violation MUST fail closed; operators MUST NOT “repair” supply by editing databases.

## Required tests and evidence

- Exact vectors for the first reward, period boundaries, pre-genesis input, maximum timestamp/height, final partial subsidy, zero remainder, and one-atomic-unit floor.
- Property tests for monotonic non-increasing subsidy and cumulative supply bounds.
- Separate fee and subsidy accounting tests, including zero-fee and maximum-fee blocks.
- Genesis manifest sum proof against the sealed 80/15/5 allocations.
- Restart, snapshot, reorg, crash, and full-replay equality of issued supply and state root.
- Multi-chain/missed-slot vectors for the approved allocation rule.
- Independent protocol-economic review and scenario simulation.
- Negative tests showing different SAGA/ΩMEGA/carbon/local telemetry cannot change block validity.

## Principal risks

- A wrong genesis policy is irreversible once value is distributed.
- Height-versus-time choice materially changes issuance cadence.
- Migration errors can create or strand supply.
- Multi-chain double counting can exhaust the community budget.
- Documentation or explorer metrics can misrepresent circulating versus issued supply even if consensus is correct.

## Approval needed

Explicit approval must name:

1. coherent genesis choice A, B, or separately specified C;
2. schedule clock and exact integer recurrence;
3. community-budget interpretation without changing sealed constants;
4. fee distribution/burn rules;
5. multi-chain allocation;
6. activation/reset strategy; and
7. the independent protocol-economics reviewer.

---

# QIP-004 — Branch-scoped state, atomic block commit, canonical reorg, and certificate finality

Status: DRAFT / APPROVAL REQUIRED  
Security objective: block arrival order, crashes, and competing DAG branches must not change canonical state; finalized histories must not conflict under the approved fault assumption.

## Confirmed problem

Every accepted branch currently validates against and mutates one global UTXO state before canonical selection. There is no rollback or branch-scoped state. UTXOs change before persistence and block insertion, so a later error leaves partial mutation. Duplicate parents and extra roots are accepted. Automatic finality is disabled; the dormant method can finalize conflicting tips and treats zero stake as quorum.

## Proposed invariant

### Branch and state transition

1. Exactly one genesis/checkpoint root is valid for a network version.
2. Every non-genesis block MUST name a unique **state parent** whose committed state root is the pre-state. Additional DAG references, if retained, MUST be unique, authenticated, and MUST NOT implicitly merge mutable state.
3. The approved state-parent rule MUST be deterministic and included in the signed BlockV2 commitment.
4. Transaction validation executes against the state parent's immutable snapshot plus an intra-block overlay.
5. A block MUST commit its resulting state root, issued-supply state, validator-set root, and bridge-nullifier root where enabled.
6. Block bytes, parent links, state delta, indexes, UTXOs/balances, issuance, and finality metadata MUST commit in one atomic database transaction or an equivalent write-ahead protocol with proven recovery.
7. An error before durable commit leaves no visible state change. A crash after durable commit recovers the whole transition exactly once.
8. Fork choice operates on stored valid blocks. Canonical-path change applies and unapplies deterministic deltas from the common ancestor.
9. No reorg may cross the latest valid finalized checkpoint.

### Finality

10. Finality MUST be represented by a domain-separated signed quorum certificate for one block, one chain, one epoch/view, and one validator-set root.
11. Quorum weight MUST be at least the approved threshold of nonzero active stake; zero total stake is never quorum.
12. Honest validators MUST be unable to sign conflicting finality votes without slashable cryptographic evidence bound to QIP-001 identities.
13. A certificate finalizes the certified block and its ancestors only. Competing descendants or tips are not independently final.
14. Validator-set changes MUST have explicit delayed activation and be committed by the prior finalized state.
15. Safety and liveness assumptions under partitions MUST be normative, not comments.

## Critical design decision: DAG state semantics

The current multi-parent structure has no deterministic state-merge specification. Approval must select one:

- **Selected-state-parent model (recommended for tractability):** one authenticated state parent defines execution state; other parents are ordering/availability references only.
- **Deterministic multi-parent merge:** requires a formal conflict-resolution, transaction-order, receipt, and state-root algorithm. This is substantially larger and must be specified before implementation.

## Compatibility and migration

1. Define StateV2 column families/keys, state-root algorithm, delta/undo format, and atomicity boundary.
2. Build an offline replay/migration tool that produces the V2 root from the approved genesis/checkpoint.
3. Introduce BlockV2 state-parent, pre/post-state root, validator-set root, and finality-certificate fields under QIP-002.
4. Do not mix V1 global-state acceptance and V2 branch-scoped acceptance after activation.
5. Seed the first V2 validator set and checkpoint through an approved genesis or migration manifest.
6. Keep automatic finality disabled until certificate verification, persistence, recovery, and partition tests all pass.

## Rollback and emergency handling

- Before activation: disable V2 and restore the last V1 snapshot.
- Migration rehearsal: atomically swap databases only after root equality and full checks; retain the source snapshot.
- After activation: a database bug permits process rollback to the last finalized checkpoint and deterministic replay. A protocol-rule bug requires halt and a new versioned fork/reset.
- Finality certificates cannot be administratively deleted. Conflicting valid certificates trigger safety-failure halt and forensic preservation.

## Required tests and evidence

- Permutation/property tests: identical valid block sets in different arrival orders yield identical canonical path and state root.
- Competing double-spend branches, duplicate parents, extra roots, deep reorgs, and maximum-width DAGs.
- Crash injection at every atomic-commit boundary followed by root and supply comparison.
- Apply/unapply round trips and common-ancestor reorg tests.
- Snapshot-to-full-replay root equality.
- Finality model checking or equivalent formal state-machine analysis for safety and liveness assumptions.
- Byzantine tests for equivocation, invalid/duplicate weight, zero stake, stale epoch, validator-set transition, conflicting tips, and network partitions.
- At least three independent nodes running repeated partition/heal/restart sequences.
- External consensus and storage review.

## Principal risks

- Multi-parent state ambiguity can invalidate the architecture if not resolved first.
- An incorrect undo log or atomic boundary creates silent supply/state corruption.
- A finality bug is worse than no finality because users treat finalized value as irreversible.
- Validator-set migration depends on QIP-001 identity correctness.
- Storage format and root changes can make existing snapshots unusable.

## Approval needed

Explicit approval must name:

1. selected-state-parent or fully specified multi-parent merge;
2. fork-choice weight and tie-break rule;
3. state-root and storage atomicity design;
4. finality protocol family, quorum/fault assumptions, timing model, and validator-set transition;
5. slashing/evidence policy;
6. activation/checkpoint strategy; and
7. independent consensus/formal-review acceptance.

---

# QIP-005 — Versioned transport envelope and authenticated resumable state sync

Status: DRAFT / APPROVAL REQUIRED  
Security objective: every valid consensus object must be propagatable, and a new or recovering node must reconstruct authenticated canonical state without trusting a peer-supplied UTXO map.

## Confirmed problem

Startup “RequestState” broadcasts the requester's own state; protobuf StateRequest is unsupported. Consensus, gossip, and QDS have contradictory 32 MiB, 2 MB, and 4 MiB limits. Snapshot UTXO injection is now contained by discarding peer UTXOs, but no authenticated snapshot protocol remains. Reconnection sleeps inside the swarm event loop. Peer diversity and identity binding are weak.

## Proposed invariant

1. One protocol constant MUST define the maximum canonical encoded BlockV2 size. Candidate construction, consensus validation, protobuf framing, gossip, QDS, RPC, storage, and proxy configuration MUST enforce compatible limits.
2. State sync MUST use explicit request/response messages, never unsolicited full-state broadcast as a substitute.
3. A sync checkpoint MUST identify chain/version, finalized block ID, finality certificate, state root, validator-set root, height/epoch, and manifest hash.
4. Snapshot data MUST be bounded, chunked, hash-addressed, ordered, resumable by cursor, and protected by total-size/item-count/decompression/decode limits.
5. A node MUST verify the finality certificate and manifest before download, verify every chunk, construct state in an isolated database, recompute the root, then atomically activate it.
6. Invalid peers MUST be penalized without allowing one peer to control the checkpoint choice. At least two independent sources SHOULD confirm checkpoint metadata.
7. If authenticated fast sync fails, the node MUST be able to replay blocks from a trusted finalized checkpoint/genesis.
8. Reconnect backoff MUST be timer/state-machine driven while the swarm continues polling.
9. Envelope authentication MUST bind QIP-001 application identity to PeerId and role; a public fallback HMAC MUST NOT provide authorization.
10. Public nodes MUST enforce connection/resource caps, protected outbound anchors, subnet/ASN diversity where observable, rotation/eviction, and duplicate-IP limits.

## Decisions deliberately left open

- Exact maximum block and message/chunk sizes.
- Snapshot data structure and root algorithm, subject to QIP-004.
- Checkpoint discovery/quorum policy.
- Wire codec and compression algorithm.
- Peer scoring weights and diversity targets.

## Compatibility and migration

1. Add explicit protocol negotiation for WireV2 and SyncV2.
2. Keep V1 gossip only below activation; never downgrade a V2 sync request silently.
3. Roll out bootnodes and at least two independent snapshot providers before ordinary nodes.
4. Publish test snapshots and manifests derived from the approved finalized checkpoint.
5. Align reverse proxies, WSS, container ports, rate limits, and observability with protocol limits.
6. Remove the misleading RequestState behavior and unsupported-feature advertising.

## Rollback and emergency handling

- Disable fast sync and fall back to verified block replay.
- Retain the active database until the candidate snapshot root is fully verified; failed imports are discarded.
- A wire rollout may return to V1 only before activation. After activation, incompatible nodes stop rather than downgrade.
- If snapshot authority or checkpoint selection is compromised, freeze advertised manifests and restart from the last independently verified finalized checkpoint.

## Required tests and evidence

- Block/message boundary tests at limit minus one, limit, and limit plus one across every transport.
- Disconnect/reconnect at every chunk, duplicate/out-of-order chunk, corrupted hash, decompression bomb, oversized collection, and slow-peer tests.
- Malicious checkpoint, invalid certificate, conflicting provider, and wrong-chain/version tests.
- Root equality between fast sync and full replay.
- Event-loop liveness while hundreds of peers back off.
- Eclipse simulation with duplicate IP prefixes, churn, poisoned DHT records, and hostile seeds.
- Multi-implementation/wire compatibility tests and a clean-node catch-up drill.
- Measured bandwidth, memory, disk, and recovery-time artifacts.

## Principal risks

- Snapshot verification that relies on the snapshot itself is circular trust.
- Oversized or compressed inputs create remote resource exhaustion.
- A too-small transport limit can make valid consensus objects unpropagatable; a too-large limit amplifies DoS.
- Mixed-version downgrade can partition the network.
- ASN diversity is imperfect behind NAT/proxies and must not become a false security claim.

## Approval needed

Explicit approval must name:

1. canonical block/message/chunk limits;
2. WireV2/SyncV2 schema and codec;
3. checkpoint trust/discovery policy;
4. snapshot/root format;
5. peer diversity/scoring targets;
6. activation and provider rollout; and
7. independent network-security/load-test acceptance.

---

# QIP-006 — SAGA/ΩMEGA advisory-only boundary and deterministic governance activation

Status: DRAFT / APPROVAL REQUIRED  
Security objective: local analytics may inform operators and governance, but can never make honest nodes disagree about block validity or monetary state.

## Confirmed problem

SAGA reward calculation consumes local mutable maps, environmental inputs, edicts, ΩMEGA threat state, random/process-local data, and floating point. Local maintenance creates UUID/floating edicts that affect mandatory reward validation. Carbon “signatures” are forgeable strings. ΩMEGA is a local heuristic, not the canonical final block gate. The neural subsystem is mostly dormant and has no authenticated model lifecycle.

## Proposed invariant

1. SAGA, ΩMEGA, anomaly scoring, carbon credentials, neural inference, market data, local API traffic, wall-clock-local state, and operator inputs MUST NOT affect:
   - block or transaction validity;
   - subsidy, fee accounting, or supply;
   - difficulty or block-size limits;
   - fork choice, validator eligibility, finality, slashing, or state roots.
2. SAGA/ΩMEGA outputs MAY drive dashboards, alerts, peer-local rate limits, candidate recommendations, or governance proposals, provided rejection of an otherwise valid consensus object never depends on those outputs.
3. Consensus parameter changes MUST be explicit versioned integers committed by an approved governance/finality transition with a future activation epoch.
4. Every activated parameter set MUST have a canonical hash, bounded values, deterministic interpretation, and rollback/expiry rule.
5. No locally generated UUID, floating multiplier, model file, or unverified carbon credential may enter consensus state.
6. Carbon/green incentives remain disabled unless a separate approved credential/oracle protocol defines cryptographic issuers, retirement/replay, additionality, geospatial evidence, and dispute handling.
7. Any future ML model remains non-consensus and MUST have signed artifacts, feature/version schema, measured evaluation, poisoning controls, rollback, and explicit operator disclosure.

## Compatibility and migration

1. First containment release removes all SAGA/ΩMEGA reads from canonical validation and emission while preserving read-only telemetry APIs.
2. QIP-003 becomes the sole subsidy rule. QIP-002 fixed limits and QIP-004 validator/finality rules replace local adaptive controls.
3. Mark dashboard fields as heuristic/advisory and remove hard-coded “accuracy/security” values.
4. Version RPC fields so clients can distinguish measured telemetry from canonical protocol parameters.
5. If governance activation is later implemented, add an on-chain ParameterSetV1 with integer bounds and delayed activation.

## Rollback and emergency handling

- Advisory services can be disabled without affecting consensus.
- Before the deterministic boundary activates, revert to the prior release only on an isolated test network; doing so on a shared network can restore nondeterminism.
- After activation, reintroducing SAGA consensus influence requires a new approved QIP and protocol version.
- A bad future parameter set is handled by its approved delayed rollback/supersession rule, not local node overrides.

## Required tests and evidence

- Run identical blocks on nodes with different clocks, API traffic, ΩMEGA seeds, edicts, carbon inputs, model files, and environment variables; acceptance, reward, fork choice, and state root MUST match.
- Static/call-graph gate proving canonical validation has no SAGA/ΩMEGA/floating dependency.
- Deterministic integer ParameterSet vectors and bound/activation tests if governance is added.
- Dashboard provenance tests: every published metric identifies source, sample count, and model/version where applicable.
- Fuzz tests for advisory inputs proving no panic/resource escape into consensus.
- Carbon tests remain excluded from rewards until a separate audited specification exists.

## Principal risks

- Product claims may need to be retracted because the “AI governance/final security gate” is not consensus.
- Local rate limits can still harm one node's availability and require operational testing.
- Reintroducing adaptive consensus too early recreates split risk.
- Signed ML models prevent unauthorized replacement but do not prove robustness or correctness.

## Approval needed

Explicit approval must name:

1. that SAGA/ΩMEGA becomes advisory-only for the next protocol version;
2. the canonical replacement for reward, limits, validator rules, and finality;
3. whether green incentives are disabled or moved to a separate proposal;
4. whether neural SAGA remains a product goal;
5. governance parameter activation/rollback rules, if any; and
6. documentation/marketing changes required to avoid false claims.

---

# QIP-007 — Real bridge proof verification and atomic nullifier state

Status: DRAFT / APPROVAL REQUIRED; BRIDGE MUST REMAIN DISABLED  
Security objective: destination-chain mint/release occurs only for one finalized, authentic, domain-bound source event, exactly once.

## Confirmed problem

The bridge SDK's proof construction is a family of hashes rather than a proof system. The live claim schema omits adequate public inputs. Internal Groth16 uses process-random keys and several weak/empty circuits. The current worktree correctly fails closed in the SDK and rejects bridge claims in block validation, but no real bridge verifier exists.

## Proposed invariant

1. A BridgeClaimV2 MUST commit to:
   - source and destination chain IDs;
   - source block/header identifier and approved finality evidence;
   - receipt/transaction inclusion proof;
   - source bridge contract and event signature;
   - source token, amount, sender, destination recipient, nonce/index;
   - destination asset/action and protocol version; and
   - a domain-separated nullifier.
2. The node MUST independently verify source-header consensus/finality under an explicit trust model. Relayer signatures alone MUST NOT prove the source event.
3. Receipt/log inclusion, contract address, event topics, amount, recipient, and chain domains MUST be public inputs bound by the proof.
4. Verification keys/CRS MUST be canonical, versioned, hash-committed, reproducibly distributed, and governed under an approved setup/update ceremony.
5. Proof verification and nullifier insertion MUST occur in the same QIP-004 atomic state transition. A repeated nullifier is invalid on every branch/canonical replay.
6. Claims MUST have strict byte, field, verification-time, and batch limits.
7. Unknown proof versions, missing public inputs, malformed encodings, unavailable verifier features, and source-finality ambiguity MUST fail closed.
8. Bridge supply/accounting MUST be capped and reconciled separately from native subsidy.
9. Security claims MUST disclose that finality/signature assumptions inherited from external chains may not be post-quantum.

## Decisions deliberately left open

- Supported source chains and their finality models.
- Light-client versus succinct-proof architecture.
- Proof system, curve, trusted-setup model, and upgrade mechanism.
- Confirmation/finality depth, bridge caps, emergency pause authority, and relayer role.
- Mint/burn versus lock/release accounting.

## Compatibility and migration

1. Retain the current fail-closed gate and disable relayer transmission/RPC advertising until all exit gates pass.
2. Introduce BridgeClaimV2; all existing synthetic/mock bundles remain permanently invalid.
3. Publish canonical VK/CRS hashes, source-chain configuration, contract/event registry, and golden claims.
4. Deploy node verifier, relayer, contracts, monitoring, and runbooks in a coordinated versioned release.
5. If any legacy bridge value exists, reconcile it through a separately approved manifest; never infer legitimacy from mock proof format.
6. Activate with conservative per-source caps and staged testnet limits after independent audit.

## Rollback and emergency handling

- Safe default is verifier unavailable and all claims rejected.
- Operators may pause new claims under the approved emergency rule, but cannot delete nullifiers or reverse minted balances administratively.
- A verifier/VK defect requires immediate claim halt, preservation of source/destination evidence, and approved corrective fork or asset-specific recovery.
- Rollback to an old mock verifier is forbidden.

## Required tests and evidence

- Independent valid vectors generated outside the node repository.
- Negative vectors for every public input, wrong chain/contract/event, malformed proof, wrong VK, insufficient finality, source reorg, and duplicate nullifier.
- Branch/reorg/restart tests proving nullifier and bridge supply consistency.
- Proof-parser fuzzing and verification-time/memory DoS benchmarks.
- End-to-end source event to destination state tests with relayer equivocation, delay, duplication, and corruption.
- Trusted-setup/VK reproducibility and governance drill where applicable.
- Independent cryptographic, smart-contract, cross-chain, and economic audit.
- Monitored capped testnet phase before any value-bearing use.

## Principal risks

- Bridge compromise can create unbacked supply even if the base chain is correct.
- External-chain finality and governance become Qanto trust assumptions.
- Trusted setup or VK update authority can become a hidden mint key.
- Proof verification may be computationally exploitable.
- Emergency pause centralization must be explicit and bounded.

## Approval needed

Explicit approval must name:

1. supported source chains and finality trust model;
2. proof/light-client architecture and cryptographic system;
3. public-input schema and nullifier definition;
4. VK/CRS setup and upgrade governance;
5. caps, pause, relayer, and accounting policy;
6. activation stages; and
7. independent bridge-audit acceptance.

---

# 3. Cross-QIP acceptance contract

The QIPs are not independently safe when partially activated. The following combined conditions are mandatory:

1. QIP-002 canonical encoding/hash is frozen before QIP-001 addresses or QIP-004 roots depend on it.
2. QIP-001 identity is active before validator certificates, finality votes, slashing evidence, or authenticated snapshots are trusted.
3. QIP-004 canonical state is active before QIP-003 supply state or QIP-007 nullifiers are considered durable.
4. QIP-006 removes local SAGA/ΩMEGA inputs before QIP-003 deterministic reward activation.
5. QIP-004 finality and QIP-005 authenticated sync are complete before a snapshot can bootstrap authoritative state.
6. QIP-007 remains disabled unless every bridge-specific gate passes; base-chain launch must not implicitly enable it.
7. Any failure in golden vectors, deterministic replay, supply reconciliation, crash recovery, finality safety, or cross-version isolation is a release blocker—not a warning.

## 4. Approval record template

Each approved QIP should append a signed review record containing:

| Field | Required value |
|---|---|
| Approved QIP revision/hash | Exact document commit and content hash |
| Decision owner | Named repository/protocol-governance authority |
| Security reviewers | Named internal and independent reviewers |
| Chosen open decisions | No “TBD” in normative behavior |
| Activation | New genesis or exact protocol/height rule |
| Migration artifact | Manifest/tool version and expected roots |
| Rollback/emergency rule | Exact halt, restore, and corrective-fork authority |
| Test evidence | CI run, vectors, simulations, and external reports |
| Documentation delta | User/operator/integration claims updated |

Until those records exist, every QIP in this file remains a **proposal only**.
