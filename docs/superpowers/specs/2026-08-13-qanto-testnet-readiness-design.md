# Qanto Testnet Readiness Design

**Date:** 2026-08-13  
**Program status:** Approved as the controlling implementation direction by the instruction to finish the A-Z mandate.  
**Protocol-policy status:** Consensus, monetary, genesis, cryptographic, governance, compatibility, and public-network choices identified below remain proposals requiring explicit protocol approval before activation.

## Purpose

This specification defines how Qanto will complete the A-Z engineering mandate and reach an evidence-backed testnet-readiness verdict without confusing a green test suite, an internal report, or a controlled private network with permissionless public-network safety.

The program uses two distinct release stages:

1. A resettable, non-value-bearing, controlled **private engineering testnet** for integration, recovery, and operational evidence.
2. A separately versioned **public-testnet candidate** that may launch only after every applicable public gate passes on one exact, reproducible artifact and independent verification is complete.

Private-testnet success does not imply public-testnet readiness. Public-testnet work continues automatically after the private engineering gate unless an action crosses a protocol-policy or external-deployment approval boundary.

## Governing principles

- Asset and supply integrity, consensus safety, determinism, cryptographic correctness, cross-chain authenticity, and permanent-state integrity take priority over performance.
- No P0 or P1 regression is accepted to improve throughput, latency, developer experience, or marketing claims.
- Consensus-visible policy is never changed silently.
- Failed commands, failed tests, and blocked experiments are retained as evidence.
- Every readiness claim identifies its exact commit, lockfiles, toolchain, feature set, configuration, chain ID, genesis, and artifacts.
- Evidence from a different candidate is informative but cannot pass the current candidate.
- Existing source, audit reports, documentation, and marketing are treated as claims until traced through the canonical runtime or reproduced.
- Optional high-risk functionality either passes its own launch gate or is technically unreachable and fails closed.
- No bridge, public deployment, asset movement, DNS change, real-key rotation, release, or publication occurs without explicit authorization.

## Evidence vocabulary

Every material claim uses one evidence label:

- **CONFIRMED** — proven by current code, reproduction, or testing.
- **MEASURED** — produced by a recorded benchmark or experiment.
- **DERIVED** — follows logically or mathematically from verified inputs.
- **ESTIMATED** — calculated from explicit assumptions.
- **HYPOTHESIS** — plausible but not demonstrated.
- **NEEDS EXTERNAL REVIEW** — requires independent specialist validation.

Every gate uses one status:

- **PASS** — all acceptance criteria pass on the exact candidate.
- **PARTIAL** — useful evidence exists but at least one criterion is incomplete.
- **FAIL** — one or more criteria are disproven or failing.
- **BLOCKED** — required evidence cannot currently be produced.
- **NOT APPLICABLE** — excluded with a written rationale and reachability proof.

A passing attack test must also record whether it demonstrates an exploit, verifies containment, or proves remediation. Test process success alone is not security success.

## Delivery approaches considered

### Controlled private network only

This provides rapid multi-node integration evidence with known operators and no assets of value. It cannot establish permissionless safety, eclipse resistance, finality safety, public economic security, or end-to-end post-quantum guarantees.

### Direct public-protocol reconstruction

This avoids a temporary legacy profile but postpones operational feedback and combines too many consensus, storage, network, economic, and deployment changes before integration.

### Staged private-to-public reconstruction — selected

The selected approach first contains the current candidate in a private engineering environment, then reconstructs and independently verifies a versioned public protocol. It yields early operational evidence without representing private-network containment as public-network security.

## Stage model

### Stage 0 — Candidate reconciliation and baseline

Before network operation:

- preserve unrelated working-tree changes;
- identify intentional source changes, generated assets, audit evidence, fuzz corpora, and build output;
- pin the Git commit, `Cargo.lock`, Rust toolchain, enabled features, configuration, chain ID, genesis hash, and consensus-parameter hash;
- record build hosts and environment;
- rerun formatting, compilation, release build, strict lint, unit, integration, attack, documentation, feature, and benchmark-build gates where applicable;
- reconcile historical audit claims against the canonical runtime;
- classify each attack test by security intent;
- generate a candidate manifest and evidence index.

Allowed outcomes are `BASELINE PINNED` or `BASELINE FAILED`. No testnet-readiness verdict is issued at this stage.

### Stage 1 — Controlled private engineering testnet

The network must be:

- resettable;
- non-value-bearing;
- operated by known participants;
- isolated through an operator-controlled access boundary;
- based on a dedicated development chain ID and deterministic genesis;
- free of bridges, sale/TGE functions, QUSD, AMM, and externally represented assets;
- explicit that history, balances, and availability may be reset;
- equipped with emergency halt and full reset procedures;
- documented without decentralization, public-security, or monetary-value claims.

The private candidate may retain explicitly documented fixed-difficulty or legacy behavior only when those limitations are contained and do not threaten real assets. Such behavior remains a blocker for the public candidate.

The only successful verdict at this stage is exactly:

`PRIVATE TESTNET READY`

If safe operation requires unresolved containment or reliability work, use:

`PRIVATE TESTNET READY WITH BLOCKERS`

If even controlled operation cannot preserve state, supply, process stability, or operator safety, use:

`NOT SAFE FOR PRIVATE TESTNET`

### Stage 2 — Versioned public-protocol reconstruction

Implement the public protocol behind explicit version, chain, schema, genesis, and activation boundaries. Stage 1 may continue producing operational evidence while this work proceeds.

No public verdict is issued during incomplete reconstruction.

### Stage 3 — Permissionless public-testnet candidate

Build one reproducible candidate and run all public security, determinism, recovery, performance, economic, operational, and independent-review gates.

Allowed successful verdicts are exactly:

- `PUBLIC TESTNET READY WITH BLOCKERS`
- `PUBLIC TESTNET READY`

P0 consensus, supply, cryptographic, state-integrity, or cross-chain-authenticity findings cannot receive conditional waivers.

## Canonical public release scope

### Included base-chain functionality

- versioned canonical block and transaction encoding;
- ML-DSA-bound account and validator identities;
- transfer and coinbase transactions;
- deterministic PoW production and verification;
- parent-derived difficulty;
- deterministic cumulative-work fork choice;
- branch-scoped state and committed state roots;
- atomic persistence and deterministic reorganization;
- signed certificate finality;
- authenticated and resource-bounded P2P messages;
- bounded mempool, RPC, gossip, and synchronization;
- deterministic issuance, fees, burns, and supply accounting;
- reproducible node deployment, observability, backup, restore, and rollback.

### Excluded unless independently approved and gated

- bridge and IBC claims;
- QUSD;
- AMM and QantoSwap;
- token sale and TGE execution;
- vesting enforcement outside a separately audited allocation mechanism;
- dynamic sharding;
- multi-parent consensus;
- DPoS or PoSe validity paths;
- CUDA, OpenCL, or other heterogeneous consensus mining without bit-exact parity evidence;
- SAGA or ΩMEGA influence over consensus-visible policy;
- EVM paths lacking the complete V2 identity and commitment envelope;
- ZK verifier paths using mock, mutable, or per-process verifier parameters.

Exclusion requires technical unreachability: the feature is absent from the release allowlist, rejected by canonical validation, inaccessible through public RPC, covered by negative reachability tests, and omitted from launch claims. A configuration toggle or alternate transaction path that can reach the feature invalidates containment.

## Versioned protocol boundary

The public candidate uses a new protocol version, public-testnet chain ID, consensus-parameter record, and genesis. V1 behavior is not silently redefined.

### V2 canonical commitments

The existing V2 identity, hash-frame, and canonical-record primitives are foundations, not proof of complete activation. Every consensus-visible object must use frozen schemas and the same implementation in node validation, mining, replay, sync, and tooling.

The following objects receive explicit schemas and domain separation:

- unsigned transaction body;
- signed transaction envelope;
- transaction identifier;
- block header and block identifier;
- PoW preimage and work hash;
- Merkle leaf and internal node;
- state root;
- genesis manifest;
- consensus-parameter record;
- validator vote and finality certificate;
- P2P message envelope;
- snapshot manifest and chunks;
- bridge commitment if a bridge is later introduced.

V2 consensus code must not fall back to JSON, `bincode`, host-endian layout, legacy address derivation, or alternate hash rules.

### Identity and crypto agility

Account, validator, peer, consensus-message, and governance identities commit to:

- role domain;
- identity version;
- network chain ID;
- signature-suite identifier;
- complete public-key length and bytes.

Declared identities are compared canonically and in constant time where appropriate. V2 validation has no “try V2, then legacy” fallback. Signature suite and address versions support later migration through an explicit classical-to-hybrid-to-post-quantum-to-future-PQ lifecycle without weakening current verification.

### Legacy handling

- V1 and V2 carry distinct protocol identifiers.
- V2 nodes reject V1 objects on a V2 chain.
- Migration tools run offline and produce auditable manifests.
- Compatibility cannot bypass V2 identity or signature checks.
- Public V2 starts from a new genesis unless an approved migration proposal proves complete state conversion and supply preservation.

## Canonical block-validation pipeline

All acceptance paths call one canonical rule set:

1. Authenticate and resource-check the network envelope.
2. Decode the declared protocol and schema version.
3. Reject unknown versions and noncanonical encoding.
4. Recompute the block identifier.
5. Resolve the exact parent and its branch state.
6. Derive height, timestamp bounds, difficulty, and validator set from committed ancestors.
7. Validate cumulative work and PoW using the V2 preimage.
8. Validate every transaction signature, chain ID, identity, replay rule, and fee.
9. Execute transactions against a branch-local state overlay.
10. Validate subsidy, fee transfer, burns, and supply transition.
11. Recompute transaction, Merkle, and state commitments.
12. Validate finality references and certificates.
13. Construct forward and undo deltas for state, supply, validators, and indexes.
14. Commit all durable changes atomically.
15. Update deterministic fork choice.
16. Apply or unapply deltas if the canonical tip changes.
17. Gossip only accepted canonical objects.

Unknown features, unavailable proof systems, storage failures, arithmetic failures, mismatched roots, and incomplete parent state fail closed.

## State, fork choice, and finality

### Branch-scoped state

Block validation never mutates one arrival-order-dependent global UTXO, supply, validator, or difficulty state. Each block derives from its declared parent and creates a branch-local result containing or referencing:

- forward state delta;
- undo delta;
- supply delta;
- validator-set delta;
- index delta;
- resulting state root.

### Atomic persistence

Block metadata, canonical indexes, state changes, supply, finality metadata, and undo data are written as one transactional batch. After any crash, either the complete transition is durable or none of it is visible.

Startup verifies the durable tip, state root, supply, and finalized checkpoint before enabling RPC writes, gossip production, or mining.

### Fork choice proposal — requires protocol approval

The proposed launch baseline has exactly one consensus parent per block. Cumulative valid work selects the heaviest non-finalized branch, with canonical block ID as the deterministic equal-work tie-break. Finalized history cannot reorganize. Incomplete multi-parent code remains unreachable.

This is described honestly as a chain with competing branches rather than as implemented multi-parent DAG consensus. Multi-parent consensus requires a separate protocol proposal, state semantics, fork-choice proof, and adversarial verification.

### Finality proposal — requires protocol approval

- A weighted validator set is fixed for each finality epoch.
- Validator IDs bind to V2 ML-DSA keys.
- Votes commit to chain ID, protocol version, epoch, round, height, block ID, validator-set commitment, and vote type.
- A certificate requires more than two-thirds of eligible weight.
- Safety assumes less than one-third Byzantine finality weight.
- Liveness assumes eventual synchrony.
- Validator-set changes activate only through a finalized transition.
- Conflicting signed votes create portable equivocation evidence.
- Slashing or ejection behavior is not activated until separately specified and approved.
- Certificates are persisted and reverified during startup and synchronization.
- A message cannot define its own trusted signer set.

## Difficulty and timing

The current mutable EMA difficulty draft must not be activated as written. A process-global lock, locally supplied solve times, missing persistence, and undefined reorganization behavior can make consensus depend on arrival order or restart history. Its repeated-byte target approximation is not a normative 256-bit quotient.

### Required public design

Difficulty is a pure function:

```text
required_difficulty(parent_header, committed_ancestor_window, consensus_parameters)
    -> exact_next_difficulty
```

It must:

- derive only from committed ancestor headers;
- be independently computable on every branch;
- avoid mutable process-global EMA state;
- avoid local mining attempts and validation wall clock;
- use checked or explicitly saturating wide-integer arithmetic;
- define exact 256-bit target conversion and rounding;
- define minimum, maximum, window, and adjustment bounds;
- include a version and activation boundary;
- produce fixed miner, validator, replay, restart, and backend-parity vectors;
- constrain timestamp manipulation through a specified median-time-past rule and future-drift bound.

The existing 31 ms, 1 second, and 600 second targets are contradictory. No adaptive rule is activated until one timing model is approved.

The timing decision process is:

1. Measure accepted-and-durable propagation and validation across the intended topology.
2. Select a target with a documented margin over measured p99 propagation and validation.
3. Freeze block interval, retarget window, timestamp window, and adjustment bounds in the consensus-parameter record.
4. Commit its hash into genesis.
5. Publish deterministic vectors before activation.

Stage 1 may use a pinned fixed difficulty and makes no adaptive-difficulty security claim.

## Deterministic economics

### Private containment

The private engineering network may use the existing full-prefunded, fee-only legacy model because it is resettable and carries no assets of value. It must not be documented as final tokenomics.

### Public economic proposal — requires protocol approval

Emission V2 remains inactive until QIP-003, a versioned genesis, and migration analysis are approved.

Consensus state separately accounts for:

- genesis allocation;
- cumulative minted subsidy;
- cumulative burned base fees;
- transferred priority fees;
- total supply;
- allocation vesting state.

The supply invariant is:

```text
total_supply_after
  = total_supply_before
  + newly_minted_subsidy
  - burned_base_fees
```

Transferred fees never increase supply. Producer credit is newly minted subsidy plus transferred priority fees.

Before implementation, a protocol proposal freezes:

- maximum supply in atomic units;
- genesis allocations;
- emitted versus prefunded allocations;
- subsidy schedule and terminal behavior;
- any minimum subsidy rule;
- base-fee adjustment and burn;
- priority-fee distribution;
- block resource target;
- vesting cliffs and integer linear-unlock arithmetic;
- validator/miner rewards and penalties;
- reorganization effects;
- rounding direction for every calculation.

All consensus calculations use integer arithmetic. No floating-point value, SAGA output, external API, market price, or local observation changes rewards, issuance, fees, supply, or resource limits.

A generated genesis allocation manifest must sum exactly to the approved supply and be committed by genesis. Replay must reconstruct identical supply at every height.

Economic models cover bootstrap, early network, large ecosystem, and global-scale scenarios using explicit assumptions rather than manufactured prices or adoption figures.

## SAGA and ΩMEGA safety boundary

SAGA and ΩMEGA are advisory analytics only. They cannot directly or indirectly determine:

- balances or state roots;
- block validity or fork choice;
- difficulty or timing;
- subsidy, fees, burns, or supply;
- block-size or gas limits;
- validator eligibility or finality;
- peer bans;
- bridge authorization;
- governance execution.

They may read finalized telemetry, emit recommendations and alerts, or construct nonbinding governance proposals. Turning a recommendation into policy requires a deterministic, versioned, approved upgrade. Negative tests prove no analytics path affects consensus output.

## Transaction and mempool boundary

The canonical transaction pipeline performs cheap, bounded rejection before PQ verification where safe:

1. bounded frame length;
2. supported protocol and schema;
3. canonical encoding;
4. chain ID and transaction-kind allowlist;
5. structural and arithmetic checks;
6. duplicate and replay prechecks;
7. PQ signature and identity validation;
8. UTXO/nonce and fee validation;
9. atomic mempool admission.

The mempool has explicit byte, transaction, per-peer, and per-account limits; TTL; replacement and eviction rules; bounded nonce gaps; duplicate suppression; fee admission; and bounded propagation queues. Alternate RPC or P2P paths cannot bypass these rules.

## P2P, RPC, and synchronization

### Authenticated envelopes

Every consensus-relevant message commits to protocol version, chain ID, message domain, schema, sender identity, complete payload hash, sequence or nonce, and bounded validity period.

### Eclipse and resource controls

The public candidate requires:

- validated-behavior peer scoring;
- per-peer and global message limits;
- bounded pending-request and deduplication maps;
- inbound/outbound connection limits;
- operator, network-prefix, and geography diversity where measurable;
- outbound peer rotation and bootnode diversity;
- request deadlines and maximum object/chunk sizes;
- invalid-message penalties and observable temporary bans;
- bounded decompression and parsing;
- attacker-to-defender amplification measurements.

### RPC, GraphQL, and WebSocket

- Mutating, administrative, debug, faucet, and expensive methods require appropriate authentication and authorization.
- Per-identity and global request limits apply.
- Request size, batching, GraphQL depth/complexity, pagination, subscription count, and historical-query cost are bounded.
- RPC workloads use resource isolation so they cannot starve consensus.
- RPC cannot bypass canonical transaction or mempool validation.
- Administrative interfaces bind privately or use an authenticated operator channel.
- Errors do not reveal secrets.

### Authenticated resumable state sync

A joining node:

1. discovers finalized checkpoints from diverse peers;
2. verifies their finality certificates;
3. obtains a signed, versioned snapshot manifest;
4. downloads bounded hash-addressed chunks;
5. safely resumes interrupted downloads;
6. verifies all chunks and the complete state root;
7. atomically installs the snapshot;
8. replays blocks from the checkpoint;
9. verifies tip, supply, validator state, and state root;
10. serves traffic or mines only after verification.

No single bootstrap or snapshot peer is trusted for the checkpoint, signer set, manifest, or state root.

## Storage, pruning, and recovery

The implementation distinguishes:

- active consensus state;
- block and DAG history;
- historical transactions;
- archival data;
- indexer data;
- ephemeral caches and queues.

Storage acceptance requires:

- versioned key and value schemas;
- atomic write batches;
- explicit WAL and durability policy;
- deterministic indexes;
- bounded caches;
- snapshot and restore verification;
- pruning that cannot remove data required for safe replay, finality, or reorganization;
- archival-node separation;
- corruption detection and fail-closed startup;
- measured write/read amplification and state growth.

Crash tests inject termination before and after every durable transition and prove recovery to either the old or new complete state, never a partial state.

## Testing and verification

### Candidate build gates

The exact approved feature set runs:

```bash
rtk cargo fmt --all -- --check
rtk cargo check --workspace --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo nextest run --workspace --locked
rtk cargo build --release --locked --features "$QANTO_CANDIDATE_FEATURES"
```

`QANTO_CANDIDATE_FEATURES` is the literal comma-separated feature allowlist recorded in the candidate manifest before the release gate. Experimental crates that cannot meet candidate lint/build requirements are removed from the release workspace or explicitly excluded with reachability evidence; failures are not hidden.

### Determinism and properties

Tests prove:

- canonical encodings are unique and round-trip;
- unknown and noncanonical encodings fail;
- identities bind to complete keys, roles, suites, and chain IDs;
- balances do not underflow or overflow;
- UTXO value and supply are conserved according to protocol rules;
- an input or transaction cannot execute twice;
- replay protection holds across chains and versions;
- fees are not minted;
- branch validation is independent of arrival order;
- apply/unapply reorganization restores the original state;
- finalized history cannot reorganize;
- difficulty is identical after restart and replay;
- identical finalized histories produce identical roots on independent processes;
- genesis is deterministic;
- disabled features fail closed.

### Attack-test conversion

Every attack module records threat, vulnerable precondition, expected vulnerable result, expected remediated result, and current interpretation. Exploit-reproduction tests are paired with or converted into regression tests before counting as fixed evidence.

### Fuzzing

Fuzz targets cover transactions, blocks, canonical records, votes, certificates, P2P envelopes, RPC parsing, PQ signatures, snapshots, cross-chain messages, and database decoding. Seeds, corpus, tool version, duration or iteration count, exact candidate, and all crashes are retained.

### Failure injection and chaos

Authorized local environments test termination, sudden kill, restart, disk full, slow disk, corruption, partial writes, latency, packet loss, duplication, partition, clock skew, peer churn, validator loss, malformed blocks, Byzantine behavior, and corrupted snapshots.

Each scenario records whether safety held, whether liveness recovered, and whether operator intervention was required.

### Performance evidence

Throughput means accepted, executed, durably committed, and—where applicable—finalized work. Measurements report separately:

- stateless routing messages per second;
- simple payment transactions per second;
- state-changing transactions per second;
- cross-chain messages per second;
- PQ verifications per second;
- blocks or DAG units per second;
- bytes per second.

Every result records node count, machine specifications, topology, latency, workload, transaction and signature sizes, duration, p50/p95/p99 latency, finality assumptions, CPU, RAM, disk, bandwidth, errors, and environment class. Local results are never relabeled as WAN results.

Long-running tests distinguish burst throughput from sustained throughput. Models at higher loads remain labeled `ESTIMATED` until measured.

### Independent verification

Before a public verdict:

- a reviewer independent of implementation performs a fresh adversarial pass;
- another operator reproduces critical builds and tests from clean source;
- reviewers attempt to falsify every P0 and P1 fix;
- critical findings are fixed and rerun;
- a multi-operator fault rehearsal completes;
- scope, exclusions, assumptions, and residual risks are published accurately.

A self-authored or AI-generated review is not represented as a third-party independent audit.

## Economic and sustainability models

The program models:

- bootstrap with negligible adoption and fees;
- early network growth;
- a large integrated ecosystem;
- global-scale research targets.

Each scenario records users, transaction classes, bytes, validator/miner count, fees, issuance, burns, validator/miner revenue, treasury revenue, commercial-service revenue, storage, bandwidth, and infrastructure cost as explicit assumptions.

Models separately evaluate:

- zero-adoption survival;
- token-price and infrastructure-cost stress;
- validator/miner viability;
- treasury runway;
- early mining concentration and fair-launch risk;
- multidimensional compute, bandwidth, state, priority, and interoperability fees;
- storage bonds, rent, expiry, and archival alternatives;
- spam attacker cost versus aggregate defender cost;
- five-year state and bandwidth sustainability;
- minimum, recommended, high-performance, and archival node profiles;
- toxic MEV and ordering manipulation;
- transparent protocol treasury versus organization revenue;
- GPL-3.0-compatible commercial services requiring legal review.

No token price, revenue, adoption, or capacity value is presented as measured unless an experiment or authoritative external input supports it.

## Operations and release controls

The private network rehearses deterministic genesis, bootstrap, key provisioning, encrypted secret storage, key rotation and revocation, monitoring, backup/restore, snapshots, corruption recovery, node replacement, partitions, emergency halt, rollback, and full reset.

The public candidate additionally requires:

- reproducible minimal container images;
- non-root execution;
- read-only filesystem where practical;
- explicit ports and resource limits;
- signed image digest;
- SBOM and provenance;
- immutable configuration bundle;
- no mutable startup downloads;
- no embedded secrets;
- versioned runbooks;
- vulnerability intake and disclosure process;
- operator upgrade and rollback procedures;
- metrics for consensus, peers, mempool, PQ verification, database, storage, network, issuance, supply, burns, and treasury without secret exposure.

## Activation, migration, and rollback

### Private network

The chain is disposable and makes no continuity promise. Reset is a supported recovery action. Private state is not automatically promoted to public V2.

### Public V2

The candidate uses a new protocol version, chain ID, consensus-parameter hash, and genesis. Rules activate from genesis rather than through an ambiguous mixed-version period.

Any offline balance migration requires:

- source-state commitment;
- conversion-tool version;
- complete allocation manifest;
- ownership-validation rule;
- exact supply proof;
- compatibility analysis;
- independent approvals.

### Rollback

Before public launch, a failed candidate may be replaced because no public continuity promise exists. After launch, nodes do not silently downgrade rules. Emergency response is a documented halt followed by an explicit versioned upgrade or publicly declared replacement chain. Local configuration cannot rewrite finalized or economic history.

## Protocol-change proposal requirements

Before activating a change to consensus, monetary policy, genesis, cryptography, governance, compatibility, chain ID, validator thresholds, slashing, or public behavior, the proposal must contain:

1. current behavior;
2. proven defect;
3. security or economic impact;
4. alternative designs;
5. simulations or measurements;
6. recommended design;
7. compatibility impact;
8. migration strategy;
9. test strategy;
10. rollback strategy.

Each is labeled `REQUIRES PROTOCOL APPROVAL` until approved. Implementation may exist behind an unreachable feature or version boundary but cannot redefine the active protocol.

## Workstream decomposition

The umbrella program is divided into independently reviewable plans:

1. Candidate reconciliation, release containment, and evidence manifest.
2. V2 canonical records, commitments, identities, and activation boundary.
3. Branch state, atomic persistence, fork choice, replay, and reorganization.
4. Validator certificates, finality, equivocation, and set transitions.
5. Economics, genesis, fees, burns, vesting, supply, and SAGA isolation.
6. Parent-derived difficulty, timing, and canonical PoW.
7. Authenticated P2P, bounded mempool/RPC, eclipse controls, and state sync.
8. Optional-feature quarantine and negative reachability tests.
9. Property, fuzz, crash, chaos, performance, and economic evidence.
10. Reproducible operations, recovery drills, independent audit, and verdict.

Each plan must produce a testable deliverable. Overlapping changes to consensus, state, genesis, supply, rewards, cryptographic verification, validators, database consensus schema, bridge verification, chain IDs, or monetary policy are synthesized centrally and implemented sequentially.

## Public-testnet gate set

The public candidate must pass all applicable gates:

1. Approved normative protocol and release scope.
2. Authenticated account, transaction, validator, consensus-message, and peer identity.
3. Canonical commitments and portable PoW.
4. Branch-scoped state, atomic commit, deterministic reorganization, and recovery.
5. Active safe finality and validator transitions.
6. Deterministic economics, genesis, fees, and supply.
7. SAGA and ΩMEGA consensus isolation.
8. Propagatable objects, eclipse controls, authenticated resumable sync, and bounded APIs.
9. Optional high-risk features either independently verified or unreachable.
10. Supported build, lint, test, property, fuzz, and formal/model evidence.
11. Bounded resources, state-growth architecture, spam resistance, and honest capacity evidence.
12. Reproducible hardened deployment, observability, backup, restore, rollback, and drills.
13. Contract and deployment allowlist.
14. Accurate documentation, metadata, claims, and vulnerability operations.
15. Independent audit and multi-operator public fault rehearsal.

## Readiness decision rule

A verdict applies only to one exact candidate and evidence bundle.

Any of the following forces `NOT SAFE FOR PRIVATE TESTNET` or prevents a public-ready verdict according to its scope:

- unauthorized supply creation;
- honest-node consensus or state divergence;
- arrival-order-dependent consensus state;
- partial durable state transition;
- reorganization below finality;
- identity or signature bypass;
- noncanonical commitment path;
- unresolved supply inconsistency;
- enabled mock or unverified bridge;
- unbounded unauthenticated resource consumption that threatens the node;
- deterministic replay mismatch;
- failed clean build or critical regression;
- unsafe recovery;
- missing independent review for a public candidate.

The final A-Z report must contain sections A through AP required by the mandate, the complete scorecard with reasons for every score below 90, explicit answers to all 45 required questions, all implemented changes and regression tests, unresolved risks, protocol proposals, roadmap, and exactly one allowed final verdict.

## Completion criteria

The A-Z mandate is complete only when:

- every reachable audit domain has been examined;
- confirmed P0/P1 implementation defects that can safely be repaired without unapproved policy changes are fixed;
- regressions and protocol invariants are automated;
- reproducible performance, storage, bandwidth, and economic evidence exists or is honestly marked blocked;
- policy proposals are separated from active implementation;
- a fresh independent second-pass audit attempts to falsify the fixes;
- the final evidence-backed readiness report and verdict are issued.

A favorable result is not required. A complete, reproducible, and honest verdict is required.
