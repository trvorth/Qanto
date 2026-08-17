# Qanto Remediation Roadmap

Audit date: 2026-08-08  
Baseline verdict: **NOT READY FOR PUBLIC TESTNET**  
Planning rule: priorities are evidence gates, not calendar promises

## 1. Executive decision

Qanto cannot reach public-testnet readiness through another round of local patches. The critical path is a versioned protocol reconstruction:

1. freeze unsafe optional features and approve the protocol decisions;
2. establish canonical serialization/hash and authenticated identities;
3. implement branch-scoped atomic state and safe finality;
4. make economics deterministic and remove SAGA from consensus;
5. build authenticated resumable sync on finalized state;
6. prove the result through deterministic replay, adversarial tests, crash recovery, independent review, and a real multi-node soak; and
7. ship one hardened, reproducible deployment rather than several contradictory stacks.

The bridge, QUSD stablecoin claims, QantoSwap, sale/vesting, dynamic sharding, heterogeneous GPU mining, and neural/green reward features are **not on the base-chain launch critical path if they remain technically disabled and are removed from launch claims**. Enabling any one of them adds its own P0 gate.

## 2. Priority definitions

| Priority | Meaning | Release consequence |
|---|---|---|
| P0 | Required before public-testnet readiness can be reconsidered, or required to keep an optional unsafe feature disabled | Any open mandatory P0 item means NOT READY |
| P1 | Required before mainnet or before scaling public-testnet exposure/value | May follow a tightly controlled testnet only when the associated feature is disabled or risk is explicitly bounded |
| P2 | Product expansion, optimization, or research after correctness/security evidence exists | Must not be marketed as implemented before its own gates pass |

Effort ranges below are engineering estimates from the audit findings. They are not delivery dates. External audit queues, protocol-governance decisions, hardware access, and remediation re-review add elapsed time.

## 3. Dependency map

~~~mermaid
flowchart LR
    F["Containment and scope freeze"] --> S["Normative specs and QIP approval"]
    S --> H["QIP-002 canonical encoding, hash, PoW spec"]
    H --> I["QIP-001 bound account and validator identity"]
    H --> ST["QIP-004 state format"]
    I --> ST
    ST --> FIN["QIP-004 certificate finality"]
    ST --> E["QIP-003 deterministic economics"]
    S --> A["QIP-006 SAGA advisory boundary"]
    A --> E
    FIN --> N["QIP-005 authenticated sync"]
    I --> N
    ST --> B["QIP-007 bridge verifier"]
    N --> B
    H --> B
    N --> SOAK["Multi-node fault and soak gate"]
    E --> SOAK
    FIN --> SOAK
    OPS["Canonical hardened deployment"] --> SOAK
    TEST["Independent review and verification"] --> SOAK
~~~

## 4. P0 — Public-testnet reconsideration

### P0.0 Immediate containment and release truthfulness

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Freeze unsafe surfaces | Keep bridge claims/relayer transmission, QUSD stablecoin deployment, QantoSwap, TGE/vesting, dynamic sharding, CUDA/Metal/OpenCL consensus mining, and unverified optimized validators disabled | None | 1-3 days | Rust release engineer, Solidity lead, SRE | Automated startup/integration tests prove each feature fails closed; RPC/docs do not advertise availability |
| Remove fabricated deployment success | Delete or quarantine synthetic receipt/address scripts and replace release paths with chain-ID-pinned receipt verification | None | 3-5 days | Solidity/Hardhat release engineer | No release command can print success without a real confirmed receipt and bytecode match |
| Retract unsupported claims | Correct 10M+/25.447M TPS, 32 BPS, sub-31 ms, hybrid consensus, finality, SAGA-AI, green rewards, bridge, QUSD peg, AMM, and HF endpoint claims | None | 1-3 days | Protocol technical writer, product owner, security reviewer | Specification-delta review has zero launch-surface claims contradicted by implementation |
| Preserve audit baseline | Record commit/worktree provenance, generate dependency/SBOM inventory, back up state/config, and prohibit destructive cleanup | None | 0.5-1 day | Release manager, supply-chain engineer | Reproducible baseline manifest and recoverable backup exist |

Containment does not make the chain ready. It prevents optional defects from expanding the core critical path.

### P0.1 Protocol decision and specification gate

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Approve QIP-001 through QIP-006 | Resolve every open cryptographic identity, hash, DAG-state, finality, economics, wire, and SAGA decision in <code>PROTOCOL_CHANGE_PROPOSALS.md</code> | P0.0 | 2-5 engineering weeks across reviewers | Principal protocol architect, applied cryptographer/PQC reviewer, protocol economist, distributed-systems lead, storage/network leads | Byte-exact normative specs have no TBD consensus behavior; signed approval records identify activation, rollback, tests, and reviewers |
| Decide consensus product scope | Implement DPoS/PoSe/finality as normative mechanisms or remove those claims for the launch version; keep disabled code unreachable | QIP review | 2-5 days for scope decision; implementation belongs below | Protocol governance, consensus architect, product owner | One canonical acceptance path and mechanism matrix is approved |
| Threat/model formalization | Define Byzantine/stake assumptions, network synchrony, state-parent semantics, finality safety/liveness, and economic invariants | QIP-004 decision | 1-3 weeks | Formal methods engineer, consensus researcher, security architect | Model/spec covers partitions, equivocation, reorgs, zero stake, validator-set changes, and crash states |

No implementation should race ahead of this gate. “We will decide the wire format in code review” is how permanent forks are born.

### P0.2 Canonical cryptography, object commitments, and identity

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Canonical Encoding/Hash V2 | Domain tags, length-delimited serialization, full transaction/block commitment, versioning, trailing-zero collision removal | P0.1 QIP-002 approval | 3-8 weeks | Applied cryptographer, Rust protocol engineer, independent implementation reviewer | Cross-language golden vectors; every field mutation changes commitment; no V1/V2 replay/downgrade |
| One QanHash specification | Portable CPU oracle, explicit endian/epoch/DAG rules, hardware vector suite; accelerators disabled until parity | Encoding/hash decision | 3-6 weeks plus hardware validation | PoW/mining engineer, CUDA/Metal/OpenCL specialists, cryptographic reviewer | CPU vectors pass across targets; any enabled backend is byte-identical on full corpus; external GPU result is CPU-reverified |
| Account/validator identity V2 | Key-derived addresses, UTXO-owner binding, validator records/certificates, role/key separation, wallet/genesis migration | Canonical hash/encoding | 2-8 weeks plus migration | PQC engineer, wallet engineer, consensus engineer | Attacker-selected key cannot claim another sender/validator; migration replay conserves exact supply; wallet/node/relayer vectors agree |
| Cryptographic dependency assurance | ML-DSA KATs/malformed-input tests, upstream/library side-channel review, remove fail-open/empty-key paths | Identity implementation | 1-3 weeks plus independent review | PQC specialist, supply-chain security engineer | CI vectors pass; unknown/malformed algorithms fail closed; reviewer signs library-risk disposition |

### P0.3 Canonical state, atomicity, reorg, and finality

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Branch-scoped StateV2 | Approved state-parent model, immutable parent snapshots, state roots, deterministic deltas, unique roots/parents | P0.2 identity and encoding | 6-12+ weeks | Rust storage engineer, DAG/consensus engineer | Block-set permutation tests produce identical path/root; competing spends do not depend on arrival order |
| Atomic block transaction | Atomically commit block, UTXO/balance, supply, indexes, validator state, and undo data | StateV2 design | 3-8 weeks, overlapping StateV2 | Database/WAL engineer, Rust reliability engineer | Crash injection at every boundary yields prior or complete next state, never partial state |
| Canonical reorg | Common-ancestor calculation, deterministic apply/unapply, mempool reconciliation, no reorg across finality | StateV2/atomic commit | 3-6 weeks | Consensus/state engineer, property-testing specialist | Deep competing-branch and restart tests converge on identical state/supply |
| Certificate finality | Signed 2/3-style certificate under approved protocol/fault model, validator-set transitions, equivocation evidence, finalized checkpoint persistence | QIP-001 identity, StateV2, formal model | 6-12+ weeks plus independent review | BFT consensus engineer, formal methods engineer, cryptographer | Model/simulation finds no conflicting finality within assumptions; zero stake never finalizes; partition/heal tests preserve safety |

These ranges overlap, but finality must not be “switched on” while state/reorg correctness is still moving.

### P0.4 Deterministic economics and SAGA boundary

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| SAGA/ΩMEGA containment | Remove local telemetry, floating point, random state, carbon data, edicts, and local threat flags from validation/emission/penalties/limits | QIP-006 approval; may start before StateV2 | 1-3 weeks | Rust consensus engineer, SAGA owner, security reviewer | Nodes with deliberately different local SAGA states accept/reject and compute roots identically |
| Coherent genesis decision | Select full-prefund/no-subsidy or reserved-allocation/community-emission policy; publish exact manifest | QIP-003 economic approval | 1-2 weeks plus review | Protocol economist, treasury/governance owner, cryptographic manifest reviewer | Atomic-unit allocation sum matches sealed constants; current 21B-plus-mining contradiction is gone |
| Deterministic subsidy and cap | Sole integer schedule, separate fees/subsidy, community budget clamp before commit, canonical supply persistence/replay | StateV2, SAGA containment, genesis decision | 2-5 weeks | Protocol economist, Rust monetary-state engineer, property-testing specialist | Boundary/property vectors, restart/reorg/full-replay supply equality, exact final remainder, no cap exceedance |
| Economic independent review | Adversarial schedule, multi-chain issuance, fee, allocation, and market-impact review | Economics implementation | 2-4 reviewer weeks | Independent protocol economist and blockchain auditor | Written acceptance with all modeled assumptions and reconciled supply artifacts |

The sealed 21B and 80/15/5 constants remain unchanged unless a new separately approved monetary proposal says otherwise.

### P0.5 Networking, propagation, and authenticated recovery

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| One propagation envelope | Align consensus, protobuf, gossip, QDS, RPC, proxy, and storage maximum sizes; bind signer to peer/validator identity | QIP-002, QIP-001, QIP-005 approval | 1-3 weeks | Rust/libp2p engineer, protocol serialization engineer | Any consensus-valid object at the limit propagates; limit+1 is rejected consistently |
| Nonblocking connection lifecycle | Timer-driven reconnect, correct multi-connection reference counting, resource/connection caps | None after wire scope | 1-2 weeks | Async Rust/libp2p engineer | Swarm continues polling under maximum backoff and churn; no session accounting drift |
| Eclipse resistance | Protected outbound anchors, subnet/ASN diversity where observable, duplicate-IP limits, rotation/eviction, peer scoring | Network policy approval | 2-4 weeks plus simulation | P2P security engineer, network simulator/SRE | Hostile-seed/churn/partition simulation retains independent honest connectivity under approved assumptions |
| SyncV2 | Header/checkpoint request-response, finality proof, bounded resumable chunks, isolated import, root verification, atomic activation, replay fallback | StateV2 and finality | 4-10 weeks | Distributed storage/network engineer, consensus engineer | Clean node reaches same root as full replay; corrupted/conflicting/oversized/disconnected peers cannot install state or stall recovery |

### P0.6 Core verification and independent security gate

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Build matrix repair | Default, no-default, all-features, all-targets, supported OS/architectures; CUDA remains excluded or fixed explicitly | Stable P0 code | 2-5 days plus backend work | Rust build/CI engineer | Documented supported matrix passes locked builds, format, strict Clippy, unit/doc/integration tests |
| Consensus property/fuzz suite | Block/tx decoders, canonical serialization, DAG permutations, concurrent spends, reorgs, supply, sync, finality | P0 implementations | 3-8 weeks in parallel | Property-testing/fuzzing engineer, protocol SMEs | No critical path has zero coverage; corpora and seeds are archived; sanitizer/fuzzer findings closed |
| Formal verification expansion | Replace placeholder verifier with machine-checked/state-model evidence for core invariants | Final spec and StateV2/finality | 3-8+ weeks | Formal methods engineer | Published model, assumptions, properties, counterexample policy, and reproducible runs |
| Independent audit and re-test | Consensus, PQC/hash, economics, networking, storage, bridge-disabled boundary | Release candidate | 4-8+ reviewer weeks | Independent blockchain security firm/team | All SEV-1/SEV-2 findings closed or explicitly accepted with no launch blocker; remediation re-test signed |

The current full Rust test pass is valuable regression evidence, not proof of protocol correctness.

### P0.7 Smart-contract and release-surface gate

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Base launch contract scope | Deploy only explicitly approved primitives; QUSD, AMM, TGE/vesting remain disabled unless fully specified | P0.0 | 2-5 days | Solidity lead, product owner, security reviewer | Deployment allowlist contains no incomplete value-bearing product |
| Contract tests/SAST | Unit, integration, invariant/fuzz, deployment smoke, coverage threshold, Slither plus second analyzer | Final enabled contract scope | 1-2 weeks | Solidity auditor, fuzz/invariant engineer | All enabled contracts meet threshold; no unresolved high/critical analyzer or manual finding |
| Receipt-verified deployment | Canonical Hardhat deployment, constructor/role checks, bytecode verification, rollback manifest | Contract gate | 3-5 days | Smart-contract release engineer | Fresh local/test chain deployment proves actual addresses, bytecode, roles, chain ID, and receipts |

If QUSD is marketed as a stablecoin or QantoSwap is enabled, add 6-12 and 6-10 engineering weeks respectively plus independent economic/contract audits. They are separate products, not “small contract fixes.”

### P0.8 Reproducible operations and measured system evidence

| Work package | Scope | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| One canonical deployment | Fix Ubuntu bootstrap; choose/consolidate Compose or K8s/Helm; per-node identity/storage; immutable image digest, SBOM, signature | Release candidate | 1-3 weeks | SRE/DevOps, container supply-chain engineer | Two clean Ubuntu 22.04 rehearsals and one upgrade/rollback complete without manual hidden steps |
| Harden public edge | TLS HTTP/gRPC/WSS, rate/request/connection limits, auth where applicable, DDoS isolation, audit logs | Canonical topology | 3-5 days | Network/SRE security engineer | External scan/load test confirms only intended ports/methods and enforced limits |
| Real observability | Export canonical/finalized height, peer, sync, accepted tx/block, queue, persistence, supply metrics; correct dashboards/alerts/runbooks | Stable metric semantics | 2-5 days | Observability engineer, protocol SME | Live PromQL panels resolve; alert drills page the correct runbook; no fabricated dashboard values |
| Backup/restore and crash drill | Quiesced/snapshot-consistent encrypted off-host backup, checksums, identity handling, restore automation, measured RPO/RTO | Canonical storage | 3-5 days plus drill | SRE, storage engineer | Restore to clean host matches finalized state root and identities |
| Honest performance baseline | Canonical signed fixtures, accepted-and-durable counters, 3+ real nodes, scaling curves, raw artifacts, invalid/adversarial load | Correct StateV2/network | 1-2 weeks | Performance engineer, protocol QA, SRE | Reproducible TPS/BPS and p50-p99.9 latency artifact exists; marketing matches measured sustained result |
| Multi-node soak/fault campaign | Partitions, churn, restart, disk pressure/fill, crash, slow peers, overload, recovery, finality and state-root comparison | Every mandatory P0 gate | 1-2 weeks minimum | SRE/chaos engineer, consensus/network/storage owners | No conflicting finalized state; all nodes recover to same root/supply; runbooks and alerts work |

## 5. P0 public-testnet exit gates

The readiness reviewer should issue a new verdict only when every mandatory row below is supported by a reproducible artifact.

| Gate | Required evidence | Fail condition |
|---|---|---|
| G0 — Approved protocol | Final QIP revisions, content hashes, named approvals, no TBD consensus choices | Any protocol behavior decided only by source code |
| G1 — Cryptographic commitments | V2 vectors, independent implementation, identity migration proof, PQC/library review | Any unbound sender/validator, commitment collision, or backend divergence |
| G2 — Deterministic canonical state | Permutation/reorg/full-replay equality, atomic crash recovery, unique genesis/parents | Arrival order or crash changes state/supply |
| G3 — Safe finality | Formal/model evidence, Byzantine partition tests, signed validator-set transitions | Conflicting valid certificates or zero-stake quorum |
| G4 — Deterministic economics | Approved genesis, exact schedule vectors, fee separation, persisted community cap | Full-supply-plus-subsidy, local inputs, or cap not enforced before commit |
| G5 — Network recovery | Common limits, eclipse controls, authenticated resumable sync equal to replay | Valid block cannot propagate, or hostile snapshot can install state |
| G6 — Optional feature isolation | Bridge/QUSD/AMM/TGE/sharding/GPU/SAGA-consensus disabled unless separately passed | Disabled feature can be reached through block, RPC, startup, relayer, or deployment path |
| G7 — Verification | Supported build matrix, strict lint, tests/fuzz/property/formal artifacts, independent re-test | Any open SEV-1/SEV-2 launch blocker |
| G8 — Operations | Immutable signed image, clean-host deployment, hardened edge, live metrics, restore and soak evidence | Hidden manual step, stale HF dependency, unverified restore, or divergent soak state |
| G9 — Honest claims | Specification delta closed; measured performance and feature claims only | Marketing/documentation contradicts the release |

Passing unit tests while any other gate fails does not satisfy readiness.

## 6. P1 — Mainnet and scaled-public-testnet hardening

P1 starts only after the base protocol passes P0 or while independent teams work without destabilizing P0 interfaces.

| Work package | Confirmed debt addressed | Dependencies | Effort | Owner skill profile | Exit gate |
|---|---|---|---:|---|---|
| Atomic mempool reservations | Concurrent conflict/cap check-then-act, ignored count cap, stale heaps | Stable TransactionV2 | 1-3 weeks | Concurrent Rust engineer, property-test engineer | Parallel admission never accepts conflicting inputs or exceeds byte/count caps |
| Replace/remove latent fail-open optimized paths | Always-true/nonempty-signature validation, corrupt optimized queues | Canonical validation API | 1-2 weeks | Performance/Rust engineer, security reviewer | One shared validator; unreachable stubs deleted; negative controls fail |
| Crash-consistency proof under abort | <code>panic = abort</code> skips graceful drain | Atomic StateV2 | 1-2 weeks | Storage reliability engineer | Power-loss/abort campaign restores exact finalized root |
| HAME/P-256 correction | Forgeable pseudo-P256 wrapper | Identity policy | 1-3 weeks plus migration | Applied cryptographer, HAME owner | Standards vectors and key migration pass, or feature is removed |
| ML-KEM transport truth | Advertised keys are discarded and no shared secret is installed | WireV2 | 2-6 weeks or 1-2 days to retract/disable claim | PQ network cryptographer | Real authenticated KEM/TLS integration has vectors/review, or no PQ-KEM claim remains |
| Protocol numeric cleanup | Lossy <code>f64</code> difficulty/effort/carbon wire fields | WireV2 | 1-2 weeks | Serialization engineer | Integer/canonical fields round-trip exactly |
| Governance deployment | ERC20Votes strategy, realistic periods/threshold, timelock roles, lifecycle tests | Contract scope approval | 1-2 weeks | Solidity governance specialist | Full proposal-to-execution and cancel/emergency tests pass |
| Faucet/airdrop authority hardening | Chain guard, claimFor griefing, mutable root/withdraw authority | Enabled contract decision | 2-5 days | Solidity security engineer | Approved role/timelock policy and adversarial tests pass |
| Monitoring/incident maturity | SLOs, symptom alerts, escalation, capacity and security events | P0 observability | 1-2 weeks | SRE/security operations | Game-day evidence meets approved SLO/RTO/RPO |
| Performance optimization after profiling | Bounded queues, worker tuning, real SIMD only where byte-identical | Honest baseline | 2-6 weeks iteratively | Performance engineer, cryptographic implementation reviewer | Each change has before/after raw artifacts and no correctness regression |

## 7. P2 — Optional product and research tracks

These tracks do not block the base public testnet when they are disabled and honestly documented.

| Track | Scope | Minimum effort | Required owner/review | Activation gate |
|---|---|---:|---|---|
| Bridge V2 | QIP-007 source finality, inclusion proof, public inputs, canonical VK/CRS, atomic nullifier, caps | 6-12+ weeks plus independent bridge audit | ZK/light-client cryptographer, cross-chain engineer, economist, external auditors | All QIP-007 gates plus capped monitored testnet; bridge remains off otherwise |
| QUSD stablecoin | Collateral, oracle, health factor, liquidation, redemption, bad debt, emergency controls | 6-12+ engineering weeks plus economic/security audit | DeFi economist, oracle/risk engineer, Solidity auditors | Invariant/economic simulations and independent audits; never launch as a mere mintable token |
| QantoSwap AMM | Pair/reserves, LP token, invariant, fee/TWAP/slippage/reentrancy behavior | 6-10+ engineering weeks plus audit | AMM engineer, formal/invariant tester, Solidity auditor | Differential/invariant/fuzz tests and external audit |
| TGE/vesting | Approved allocations, legal/economic schedule, caps, refund/unsold handling, timelock integration | 2-4 weeks after policy approval | Token-distribution counsel/owner, Solidity engineer/auditor | Policy sign-off and full lifecycle tests |
| Real SAGA ML | Signed models, feature/version contract, poisoning/drift evaluation, non-consensus inference and rollback | 8-16+ weeks | ML security engineer, protocol security reviewer | Measured model artifact; remains advisory |
| Carbon/green incentives | Cryptographic issuers, oracle/geospatial/additionality verification, retirement/replay, disputes | 4-12+ weeks plus external oracle/economic audit | Climate-data/oracle specialist, economist, cryptographer | Separate approved protocol; no consensus impact by default |
| Hyper-sharding | State partitioning, data availability, validator assignment, cross-shard atomic receipts, reorg/finality | 3-6+ engineer-months | Distributed-systems/sharding researchers, formal methods, external audit | Full specification, simulation, adversarial testnet; current stub stays disabled |
| Heterogeneous mining acceleration | Verified CUDA/Metal/OpenCL kernels and benchmarked hardware support | 3-6+ weeks after normative PoW | GPU/kernel engineers, mining reviewer | Byte parity on published corpus and CPU re-verification policy |
| Headline throughput research | Architecture work toward high TPS only after measured bottlenecks | Evidence-driven, not pre-estimated as a promise | Performance/storage/network team | Published sustained accepted-and-durable results; no target declared achieved by configuration math |

## 8. Staffing model and sequencing guidance

The work can run in parallel only at stable interfaces.

| Lane | Core roles | Safe parallel scope | Coordination boundary |
|---|---|---|---|
| Protocol/cryptography | Principal architect, PQC/applied cryptographer, consensus engineer | QIP-002 vectors and QIP-001 identity prototype | Freeze encoding/hash before wallet/state migration |
| State/finality | Storage/WAL engineer, DAG/BFT engineer, formal methods | StateV2 prototype and formal model | Validator identity and state-parent rule must be fixed |
| Economics/SAGA | Protocol economist, monetary-state engineer, SAGA owner | Genesis simulations and advisory-boundary removal | Supply state commits through StateV2 |
| Network/sync | libp2p engineer, distributed storage engineer, P2P security | Reconnect/eclipsing controls before SyncV2 | Authenticated snapshots require finality/root |
| Verification | Property/fuzz engineer, independent auditors, QA/chaos | Build corpora/harnesses while interfaces stabilize | Final acceptance runs only on release candidate |
| Operations | SRE, container supply-chain, observability | Canonical topology, edge, images, dashboards | Final soak waits for protocol release candidate |
| Contracts/products | Solidity/DeFi engineers and auditors | Disabled-feature tests and clean deployment tooling | Optional value products do not gate base chain unless enabled |

Adding people to the same consensus module will not compress the critical path linearly. The highest-leverage parallelism is independent vector implementations, formal modeling, migration tooling, network simulation, and deployment automation.

## 9. Effort envelope

The following is a planning range, not a date commitment:

- Core protocol specification, identity/hash, state/reorg/finality, deterministic economics, and SyncV2: **approximately 30-65+ engineer-weeks**, with major overlap only after interfaces are frozen.
- Verification, formal/model work, independent re-test, deployment hardening, and soak: **approximately 15-35+ additional engineer/reviewer-weeks**, much of it parallel.
- QIP-007 bridge and optional product tracks are excluded from the base estimate and add their own ranges.

A smaller team should expect the elapsed critical path to be dominated by QIP-004 state/finality and independent review. Skipping those gates does not make delivery faster; it only moves the failure into the network.

## 10. Status-control template

Track every work package with this evidence record:

| Field | Required content |
|---|---|
| Work package and owner | Named accountable engineer/reviewer |
| Approved spec/QIP revision | Exact commit/hash |
| Dependency state | Passed gate IDs, not “mostly done” |
| Implementation diff | Reviewable change set |
| Verification artifact | Commands, environment, raw output/results, vectors/seeds |
| Security review | Finding IDs closed and reviewer |
| Migration/rollback rehearsal | Inputs, expected roots, actual result |
| Documentation delta | User/operator claims changed |
| Residual risk | Explicit severity and decision owner |
| Gate result | PASS, FAIL, or BLOCKED; no percentage proxy |

## 11. Readiness rule

The current verdict stays **NOT READY** until every mandatory P0 gate passes on one pinned release candidate and independent reviewers confirm closure. If an optional feature is deferred, it must be unreachable, fail closed, absent from launch/deployment paths, and described as unavailable.

The next readiness review should produce evidence, not optimism:

- identical roots and supply across replay, reorg, restart, snapshot, and partition recovery;
- no unbound identity, hash ambiguity, partial commit, conflicting finality, or local-input consensus path;
- one clean deployable stack with verified restore and observability;
- measured performance claims; and
- no enabled value feature without its own complete security gate.
