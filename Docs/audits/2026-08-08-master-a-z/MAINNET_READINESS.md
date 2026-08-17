# Qanto Public-Testnet and Mainnet Readiness Gates

Audit date: 2026-08-08  
Assessment target: effective shared working tree at <code>/Users/cps/qanto</code>  
Public-testnet verdict: **NOT READY**  
Mainnet verdict: **NOT READY; NOT ELIGIBLE FOR MAINNET REVIEW**

## 1. Executive decision

Qanto has useful containment fixes and passing regression suites, but the protocol is not yet safe to expose as a public blockchain. The current blockers are architectural:

- native spenders and validators are not cryptographically bound to their claimed identities;
- the canonical hash admits trailing-zero collisions and mining backends disagree;
- block commitments omit accepted fields;
- competing DAG branches mutate one global state in arrival order;
- block/state persistence is not atomic and reorg rollback does not exist;
- no safe production finality protocol is active;
- live reward validation depends on local floating SAGA/ΩMEGA state;
- genesis, issuance, fee accounting, and the supply cap are contradictory;
- startup sync is nonfunctional, transport limits conflict, and authenticated resumable state sync does not exist; and
- the deployment, performance, contract, and documentation surfaces still overstate what is operational.

The current working tree is appropriate only for controlled local/private engineering networks with no real value and no production-security claims. Calling the same build a “public testnet” would turn known consensus and authorization defects into an internet-facing attack surface.

## 2. Decision rules

### 2.1 Evidence statuses

| Status | Meaning |
|---|---|
| PASS | The exact release candidate meets the gate, reproducible evidence exists, and required independent reviewers signed off |
| PARTIAL | Useful containment or tests exist, but the gate's security invariant is not proven |
| FAIL | A confirmed defect violates the gate |
| BLOCKED | A dependency, approval, implementation, or independent review has not completed |
| NOT APPLICABLE | The feature is provably unreachable, fails closed, is absent from release/deployment paths, and is documented as unavailable |

“Implemented,” “tests pass,” and “we plan to disable it” are not synonyms for PASS.

### 2.2 Release verdict rule

**PUBLIC TESTNET GO** requires all mandatory PTG gates to be PASS on one pinned, immutable release candidate. Optional features may be NOT APPLICABLE only when technical reachability tests and public documentation prove they are disabled.

**MAINNET GO** requires every PTG gate plus every MNG gate to be PASS on the exact mainnet candidate. Mainnet does not inherit exceptions accepted for a disposable test network.

Any open SEV-1, any unresolved consensus divergence, any unauthorized value path, or any untested state/supply migration is an automatic **NO-GO**.

## 3. Source map

This gate document synthesizes, rather than replaces, the evidence in:

| Report | Scope |
|---|---|
| [Consensus, Networking, Transaction, and SAGA Audit](CONSENSUS_NETWORK_TRANSACTION_SAGA.md) | CNS, NET, TX, and SAGA findings |
| [PQC and Tokenomics Audit](PQC_AND_TOKENOMICS.md) | PQC, CRY, POW, ZK, ECO, and monetary documentation findings |
| [Contracts, Performance, and Operations Audit](CONTRACTS_PERFORMANCE_OPERATIONS.md) | SC, PERF, and OPS findings |
| [Specification Delta](SPECIFICATION_DELTA.md) | SPEC findings and implementation-versus-claim matrix |
| [Protocol Change Proposals](PROTOCOL_CHANGE_PROPOSALS.md) | QIP-001 through QIP-007 approval-gated protocol designs |
| [Remediation Roadmap](REMEDIATION_ROADMAP.md) | P0/P1/P2 sequencing, ownership, effort, and exit gates |
| [Threat Model](THREAT_MODEL.md) | protected assets, adversaries, and launch-blocker rule |

## 4. Current positive evidence—and its limit

| Current evidence | Status | Why it does not establish readiness |
|---|---|---|
| SAGA base reward plus fees and exact coinbase distribution are enforced | FIXED/MEASURED | The base reward itself still comes from local floating SAGA state |
| Inbound block ID is recomputed and fixed network difficulty is required | FIXED/MEASURED | The ID omits accepted fields and the underlying hash construction is ambiguous |
| Automatic finality and dynamic sharding are disabled | CONTAINED | This prevents known unsafe paths from running; it does not provide finality or sharding |
| Bridge claims and the SDK mock verifier fail closed | CONTAINED | Bridge functionality remains unimplemented and must not be advertised |
| Peer-supplied snapshot UTXOs are discarded | CONTAINED | Arbitrary UTXO injection is closed, but authenticated state sync remains unavailable |
| Mempool rejects inputless/coinbase transactions | FIXED/MEASURED | Concurrent conflict and capacity admission remain non-atomic |
| CPU QanHash DAG words use explicit little-endian decoding | FIXED/MEASURED | CPU/OpenCL/CUDA/Metal algorithms still diverge |
| Current full Rust Nextest run completed with 207 passed and one skipped across 31 binaries | PASS as regression evidence | Critical protocol invariants lack property/formal/multi-node evidence; some supported build matrices remain broken |
| Hardhat compiles and 38 Faucet/QUSD tests pass; npm audit reports zero advisories | PASS as narrow regression evidence | Eight contracts lack tests; Solidity SAST/fuzzing and enabled-product audits remain absent |
| Docker build context excludes common environment/key/state artifacts; async metrics export no longer blocks Tokio | FIXED/MEASURED | The canonical deployment topology, edge, observability, restore, and clean-host launch still fail readiness |

Passing tests are credits, not a security waiver. The confirmed launch blockers below remain cumulative.

---

# 5. Public-testnet launch gates

## PTG-01 — Approved normative protocol and release scope

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| SPEC-001, SPEC-002, SPEC-004 through SPEC-008, SPEC-018; QIP-001 through QIP-006 remain drafts | Approved byte-exact consensus specification; chosen DAG state-parent semantics; approved consensus mechanism matrix; QIP content hashes and named decision owners; explicit activation and rollback rules; release feature allowlist |

Pass conditions:

1. No consensus behavior is left as “TBD,” inferred from code, or controlled by local configuration.
2. DPoS, PoSe, SAGA, ΩMEGA, sharding, finality, bridge, and smart-contract product claims exactly match the enabled release.
3. Every disabled feature is unreachable through block, transaction, RPC, relayer, startup, or deployment paths.

Independent review: principal protocol reviewer who did not author the implementation.

## PTG-02 — Authenticated account, transaction, validator, and peer identity

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| PQC-001, PQC-002, CNS-001, TX-001, NET-009, SPEC-003; QIP-001 | AddressV2/ValidatorRecordV2 specification; key-to-address and key-to-validator golden vectors; migration manifest; negative impersonation tests; validator epoch/rotation/revocation tests; peer-to-transport identity binding |

Pass conditions:

1. An attacker signing with a valid self-selected key cannot spend another account's UTXO or claim another validator's identity.
2. Validator votes, rewards, finality, and evidence use the same authenticated epoch record.
3. Legacy unbound strings cannot authorize value or consensus.

Independent review: PQC/applied cryptographer plus wallet/identity migration reviewer.

## PTG-03 — Canonical commitments and one portable PoW result

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| CRY-002, POW-001, POW-002, POW-004, CNS-005, CNS-007; QIP-002 | Versioned domain-separated canonical encoding; trailing-zero regression; field-mutation suite; portable CPU reference; cross-platform and accelerator vectors; duplicate-parent/root rejection |

Pass conditions:

1. Every consensus-affecting field is committed before signing/mining.
2. Distinct objects cannot collide because of padding, length, type, role, chain, or version ambiguity.
3. Every enabled mining backend produces byte-identical output; otherwise it remains disabled and external results are CPU-reverified.

Independent review: cryptographer and independent second implementation/conformance team.

## PTG-04 — Deterministic branch state, atomic commit, and reorg recovery

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| CNS-007, CNS-008, CNS-009, TX-005; QIP-004 | Branch-scoped StateV2; committed state root; atomic database batch/WAL proof; deterministic apply/unapply; block-arrival permutation tests; competing-spend/reorg tests; crash injection and full replay equality |

Pass conditions:

1. Identical valid block sets yield identical canonical path, UTXOs, balances, supply, and root regardless of arrival order.
2. A failed or crashed block commit leaves either the complete prior state or complete next state, never a partial transition.
3. Reorgs apply and unapply deterministic deltas and cannot cross a finalized checkpoint.

Independent review: distributed-storage/WAL specialist, consensus engineer, and property-testing reviewer.

## PTG-05 — Safe active finality and validator-set transition

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| CNS-010, CNS-011, SAGA-009, SPEC-002, SPEC-017; QIP-001 and QIP-004 | Approved finality protocol/fault model; signed quorum-certificate vectors; nonzero stake rules; validator-set root/transition; equivocation evidence; formal/model analysis; Byzantine partition/heal tests |

Pass conditions:

1. Conflicting blocks cannot both obtain valid finality under the approved fault assumption.
2. Zero stake, duplicate weight, stale epochs, free-form names, and local anomaly scores cannot create finality or penalties.
3. RPC finality fields correspond to verified certificates, not dormant/manual markers.

Independent review: BFT consensus specialist and formal-methods reviewer.

## PTG-06 — Deterministic economics, coherent genesis, and enforced supply

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| CNS-003, ECO-003 through ECO-007, SPEC-005, SPEC-009; QIP-003 and QIP-006 | Approved genesis choice and atomic-unit allocation manifest; exact integer schedule vectors; separate fee/subsidy accounting; community-budget clamp before commit; restart/reorg/snapshot/full-replay supply reconciliation |

Pass conditions:

1. The sealed 21B and 80/15/5 constants remain internally consistent and are reflected in genesis plus canonical issuance.
2. The network does not combine a full-21B genesis allocation with further subsidy.
3. Fees are transfers, not issuance.
4. No block can commit a subsidy above the exact remaining approved budget.
5. Nodes with different local telemetry compute identical reward and supply.

Independent review: protocol economist and independent monetary-state auditor.

## PTG-07 — SAGA/ΩMEGA consensus isolation

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| SAGA-001, SAGA-002, SAGA-005, SAGA-006, SAGA-008, SAGA-011, SPEC-007; QIP-006 | Call-graph/static gate; multi-node differential tests with different clocks, API traffic, edicts, carbon inputs, random state, and model files; advisory-only RPC semantics |

Pass conditions:

1. SAGA, ΩMEGA, carbon, market, model, and local operator inputs cannot change validity, reward, limits, difficulty, fork choice, validator selection, finality, slashing, or roots.
2. Any future parameter change is a versioned bounded integer transition with delayed canonical activation.
3. Green/AI claims are disabled or described as advisory simulations.

Independent review: consensus determinism reviewer and ML/security reviewer for any enabled advisory service.

## PTG-08 — Propagatable objects, eclipse controls, and authenticated resumable sync

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| NET-003 through NET-009, CNS-006; QIP-005 | One block/message limit; request-response headers/checkpoints; finality/state-root verified chunk manifest; resume/cursor tests; isolated import and atomic activation; event-loop liveness; eclipse/churn simulation |

Pass conditions:

1. Every consensus-valid object at the approved maximum size propagates through gossip/QDS/RPC/proxy; limit-plus-one is rejected consistently.
2. A clean node reaches the same root by fast sync and full replay without trusting peer-supplied UTXOs.
3. Corrupt, conflicting, oversized, slow, or disconnecting peers cannot install false state or stop the swarm event loop.
4. Public nodes enforce outbound anchors, connection/resource caps, duplicate-IP controls, diversity/rotation, and peer scoring under an approved threat model.

Independent review: libp2p/network-security specialist and distributed-storage reviewer.

## PTG-09 — Optional high-risk features fail closed

Current status: **PARTIAL**

| Feature | Blocker mapping | PASS or NOT APPLICABLE evidence |
|---|---|---|
| Bridge/ZK | ZK-001 through ZK-005, CNS-012, SPEC-004, SPEC-018; QIP-007 | Node, SDK, relayer, RPC, docs, and deployment all reject/omit claims; or full QIP-007 implementation and independent bridge audit |
| QUSD | SC-001, SC-008, SPEC-008 | Not deployed/marketed as stablecoin; or complete collateral/oracle/redemption/liquidation design and audit |
| QantoSwap | SC-002, SPEC-008 | Factory/router remain unreachable/reverting and deployment omits them; or complete AMM invariant suite and audit |
| TGE/vesting | SC-004, SC-006, SPEC-008 | No sale/deployment path; or approved allocation/vesting/refund policy and lifecycle audit |
| Dynamic sharding | PERF-007, SPEC-006, SPEC-018 | Disabled and unadvertised; or separately specified/tested/audited protocol |
| GPU mining | POW-001, POW-002, POW-004 | Disabled with CPU verification; or full parity corpus passes |
| Fail-open optimized paths | PERF-003, PERF-005 | Deleted/unreachable with structural test; or uses canonical validation and invariants |

Pass condition: every row is independently PASS or NOT APPLICABLE. One safe code branch is insufficient if another launch path can enable the feature.

Independent review: feature-specific security reviewer and release reachability reviewer.

## PTG-10 — Supported build matrix, critical-path tests, fuzzing, and formal evidence

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| ZK-006, SC-013, SC-014, SPEC-011, PERF-008; formal-verifier placeholder identified by workstream 11 | Pinned clean build; supported default/no-default/all-feature/all-target matrix; format/strict Clippy; Rust and Solidity suites; decoder/consensus fuzzing; property tests; model/formal artifacts; coverage map; no ignored critical failures |

Pass conditions:

1. The release's documented matrix passes from a clean checkout with locked dependencies.
2. Critical paths have adversarial tests: identity, encoding/hash, fork choice, state/reorg, atomic commit, finality, supply, sync, concurrent spends, and optional-feature isolation.
3. The formal-verification claim is removed or backed by reproducible machine-checked/model evidence.
4. Flaky tests are fixed, not retried into green.

Independent review: test-infrastructure owner plus external reviewers who rerun the suite from source.

## PTG-11 — Resource safety and honest performance capacity

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| CNS-013, TX-003, PERF-001 through PERF-010, SPEC-006 | Accepted-and-durable benchmark harness; valid signed fixtures and negative controls; real multi-node network/storage path; scaling curves; sustained/spike/soak results; p50 through p99.9 latency; CPU/RSS/disk/network/queue data; overload recovery |

Pass conditions:

1. No production or reachable helper accepts a transaction because a signature is merely nonempty or validation always returns true.
2. Queues/caps have real bounded semantics and deterministic overload behavior.
3. Peer-controlled arithmetic cannot panic or wrap through validation.
4. Published throughput/latency claims do not exceed reproducible sustained accepted-and-durable results.

The public testnet does not need to hit 10M TPS. It does need to remain correct and recoverable at its declared capacity.

Independent review: performance engineer and protocol-security reviewer.

## PTG-12 — Reproducible hardened deployment and operations

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| OPS-001 through OPS-011, SPEC-010 | One canonical topology; two clean Ubuntu rehearsals; immutable signed image digest, SBOM/provenance; per-node identity/storage; secrets integration; hardened HTTP/gRPC/WSS edge; live metrics/dashboards/alerts; backup/restore and upgrade/rollback drills |

Pass conditions:

1. No Hugging Face or mutable-branch dependency remains in the authoritative launch path.
2. RPC/gRPC/WSS are protected by TLS, method/request/connection limits, rate controls, and audit logging.
3. Prometheus/Grafana panels query real semantics; critical alerts have exercised runbooks.
4. An encrypted/checksummed off-host backup restores on a clean host to the same finalized root and identity.
5. Docker/Kubernetes/Helm assets not selected as canonical are clearly deprecated, not competing instructions.

Independent review: SRE, container supply-chain reviewer, and external deployment operator.

## PTG-13 — Contract and deployment-script release allowlist

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| SC-001 through SC-015, SPEC-008, SPEC-009 | Explicit enabled-contract allowlist; fabricated deployment scripts removed/quarantined; receipt/bytecode/chain-ID/constructor/role verification; tests for every enabled contract; Slither plus second analyzer; invariant/fuzz coverage |

Pass conditions:

1. A deployment command cannot fabricate a receipt, address, or success result.
2. Every enabled contract has an approved product specification, role/governance policy, executable lifecycle tests, and independent review.
3. Incomplete contracts are absent from deployments and public claims.

Independent review: independent Solidity auditor. Base-chain readiness cannot borrow credibility from 38 tests covering only two contracts.

## PTG-14 — Accurate documentation, metadata, and vulnerability operations

Current status: **FAIL**

| Blocker mapping | Required closure evidence |
|---|---|
| SPEC-001 through SPEC-018, SC-015 | Zero unresolved launch-surface specification deltas; canonical chain ID/decimals/reward/endpoints; API finality truth; supported-feature matrix; verified private reporting channel; named response owner; operator/runbook review |

Pass conditions:

1. “Production-grade,” “quantum-resistant,” “final,” “stablecoin,” “AMM,” “bridge,” “AI governance,” “green rewards,” “hyper-sharding,” and performance claims are supported by the exact release.
2. Chain ID is consistently decimal 1234 / hex <code>0x4D2</code>, and decimals are consistently nine.
3. Security reporting reaches a monitored private recipient and escalation is rehearsed.

Independent review: technical writer/reviewer, security response owner, and integration partner.

## PTG-15 — Independent release-candidate audit and public fault rehearsal

Current status: **BLOCKED**

| Blocker mapping | Required closure evidence |
|---|---|
| Cumulative closure of every SEV-1/SEV-2 and QIP-001 through QIP-006 | Independent audit reports scoped to the exact release commit; signed remediation re-tests; multi-node partition/churn/crash/disk-pressure/overload/restore campaign; identical finalized roots and supply after recovery |

Pass conditions:

1. No open SEV-1 or SEV-2 launch blocker.
2. Reviewers are independent of implementation authors and reproduce critical evidence from a clean environment.
3. The final candidate survives a public/adversarial rehearsal without safety divergence.
4. Any accepted residual risk has a named owner, bounded impact, and does not violate a protected invariant.

---

# 6. Mainnet launch gates

Mainnet review starts only after all PTG gates pass and the public testnet produces stable evidence on the protocol intended for mainnet. A redesigned protocol that was not exercised publicly resets the evidence clock.

## Mainnet gate matrix

| Gate | Current status | Required mainnet evidence |
|---|---|---|
| MNG-01 — Public-testnet inheritance | BLOCKED | Every PTG gate PASS on a version compatible with the mainnet candidate; all testnet deviations enumerated and independently reviewed |
| MNG-02 — Protocol and migration freeze | BLOCKED | Final specifications/QIPs, compatibility policy, mainnet genesis/state manifest, deterministic migration/replay tool, activation ceremony, and change freeze |
| MNG-03 — Production validator/finality system | BLOCKED | Diverse independent validator operators; authenticated keys in hardware/offline custody where appropriate; tested validator-set transitions; finality safety/liveness evidence under realistic partitions and churn |
| MNG-04 — Mainnet economics and treasury custody | BLOCKED | Independent genesis allocation/supply attestation; treasury/community/ecosystem/liquidity custody policy; multisig/timelock/role separation; issuance reconciliation and public explorer agreement |
| MNG-05 — Production cryptography and key lifecycle | BLOCKED | PQC/hash/library review; reproducible dependency provenance; key generation/backup/rotation/revocation/destruction ceremonies; no forgeable pseudo-P256 or unimplemented KEM claim in enabled paths |
| MNG-06 — Sustained public-testnet evidence | BLOCKED | Approved observation period without unresolved safety incident; at least one protocol upgrade, validator-set transition, partition recovery, clean-node sync, crash restore, and incident game day |
| MNG-07 — Capacity and safe overload margin | BLOCKED | Sustained measured load at approved safety margin above expected launch demand; bounded queues; p99.9 latency/resource data; DDoS/invalid-input/recovery results |
| MNG-08 — Production infrastructure and disaster recovery | BLOCKED | Dedicated multi-failure-domain topology; immutable signed artifacts; secret manager/HSM policy; monitored SLOs; tested failover, restore, RPO/RTO, capacity, certificate, DNS, and provider-outage procedures |
| MNG-09 — Governance and emergency authority | BLOCKED | Published governance powers, upgrade/rollback constraints, emergency halt boundaries, conflicts/recusal, incident commander/on-call roster, communications authority, and key-compromise playbooks |
| MNG-10 — Supply chain, licensing, and compliance | BLOCKED | Dependency/advisory/license policy PASS; SBOM/provenance/signature verification; repository/package licenses resolved; enabled product/legal/regulatory review completed |
| MNG-11 — Enabled bridge/contracts/products | BLOCKED | Each enabled value-bearing feature has its own complete specification, invariant/fuzz suite, economic analysis, deployment verification, role/custody policy, independent audit, cap, monitoring, and emergency plan |
| MNG-12 — Independent mainnet audit and re-test | BLOCKED | At least cryptography/PQC, consensus/formal, state/storage, economics, P2P/DoS, SRE/supply-chain, and Solidity/bridge reviews as applicable; all high/critical findings closed and re-tested on exact candidate |
| MNG-13 — Launch and rollback rehearsal | BLOCKED | Full dry run from signed binaries and genesis ceremony through validator bootstrap, monitoring, incident injection, halt, restore, communications, and corrective-upgrade decision |

## Mainnet-specific hard stops

Mainnet is an automatic NO-GO if any of the following is true:

- mainnet uses a different consensus, hash, identity, state, finality, economics, or sync rule than the publicly exercised candidate without a new audit/evidence cycle;
- genesis allocation or issued supply cannot be reproduced independently to the atomic unit;
- a single person or online hot key can mint, upgrade, pause, seize, finalize, or replace critical protocol state without a published bounded authority model;
- the validator set is operationally controlled by one organization despite nominally distinct keys;
- rollback assumes deleting finalized history, editing balances, or restoring an unauthenticated snapshot;
- bridge, stablecoin, AMM, sale, or governance contracts are enabled because they “worked on testnet” without their own audit and product-risk gate;
- the release depends on mutable branches/images/tags or unsigned artifacts;
- security response, on-call, and public incident communication have not been rehearsed; or
- economic, security, legal, and operational decision owners have not signed the exact candidate.

---

# 7. Independent review requirements

## 7.1 Required disciplines

| Review discipline | Minimum scope | Blocking output |
|---|---|---|
| Consensus/formal methods | DAG state-parent rule, fork choice, reorg, validator-set transition, finality safety/liveness | Reproducible model/spec, assumptions, counterexample disposition, signed re-test |
| Applied cryptography/PQC | Canonical hash/encoding, ML-DSA identity binding, key lifecycle, QanHash vectors/backends, enabled KEM/P-256 paths | Independent vectors/implementation, library/side-channel risk review, no unresolved critical/high |
| State/storage reliability | Atomic commit, WAL, state root, undo/reorg, snapshot, crash/power-loss recovery | Fault-injection report and exact root/supply recovery evidence |
| Protocol economics | Genesis, allocation, schedule, fee/subsidy separation, supply cap, multi-chain accounting, treasury | Atomic-unit attestation, scenarios, reconciliation, economic-risk sign-off |
| P2P/network security | Wire limits, identity, eclipse/Sybil resistance, sync, decompression/DoS, partitions | Network simulation/load results and clean-node sync equivalence |
| Smart contracts/bridge | Every enabled contract, deployment roles, proof/public inputs, source finality, nullifiers, upgrade/pause/cap policy | Manual audit plus SAST/invariant/fuzz and remediation re-test |
| Performance | Accepted-and-durable throughput, resource saturation, overload safety, recovery | Raw reproducible artifacts; claims bounded by sustained results |
| SRE/supply chain | Images, SBOM/provenance, secrets, edge, monitoring, backups, failover, incident response | Clean deployment/restore/game-day report |

## 7.2 Independence rule

An implementation author may supply evidence but may not be the sole approver of that evidence. Independent reviewers must:

1. receive the exact commit, dependency lockfiles, toolchain, build instructions, threat model, QIP/spec revision, and known-issue list;
2. run critical tests from a clean environment rather than accepting screenshots;
3. disclose scope exclusions and conflicts of interest;
4. classify findings consistently with the audit severity model;
5. verify remediation rather than accepting a code-diff explanation; and
6. sign the exact release candidate or require a delta review after any security-relevant change.

No self-generated percentage score can replace these gates.

## 7.3 Evidence dossier

Every PASS must link to an immutable dossier containing:

- source commit and dirty-tree status;
- toolchain, dependency locks, artifact/image digests, SBOM, and signatures;
- exact commands, environment/topology, raw logs/data, random seeds, and expected roots;
- specification/QIP revision and activation configuration;
- finding-to-fix-to-test traceability;
- migration inputs, output roots, supply totals, and rollback rehearsal;
- independent report and remediation re-test;
- residual-risk owner and expiration/review trigger; and
- documentation and operator-runbook delta.

Evidence from a different commit is informative, not a release pass.

---

# 8. Rollback, recovery, and incident prerequisites

## 8.1 Protocol rollback constraints

1. **Pre-activation rollback:** nodes may return to the last compatible release before a protocol activation/checkpoint.
2. **Post-activation rollback:** nodes must not silently downgrade object, identity, state, economics, or wire versions.
3. **Finalized state:** operators must not delete valid finality certificates or edit finalized balances/supply.
4. **Corrective action:** a consensus-rule defect requires halt, evidence preservation, independent impact analysis, and an explicitly approved versioned corrective fork or testnet reset.
5. **Database recovery:** restore only authenticated snapshots/checkpoints, then deterministically replay and compare state/supply roots.
6. **Optional features:** bridge/contract pause may stop new actions but cannot erase nullifiers, reverse arbitrary balances, or conceal prior effects.

## 8.2 Required incident playbooks and drills

| Incident | Detection prerequisite | Immediate action | Recovery proof required |
|---|---|---|---|
| Consensus/state-root split | Cross-node canonical/finalized root and supply alerts | Stop production/ingress; preserve logs/databases/peer evidence | Root-cause report; deterministic replay; corrective-version or reset approval |
| Conflicting finality certificates | Certificate conflict alarm and evidence collector | Global safety halt; isolate validator keys; public severity-one notice | Formal impact analysis; key/evidence audit; approved recovery fork |
| Unauthorized spend/validator impersonation | Signature/identity mismatch telemetry and fraud proof | Halt affected protocol version; revoke operational access, not ledger ownership | Independent forensic proof and corrected identity activation |
| Supply/genesis mismatch | Independent supply counters and explorer reconciliation | Stop subsidy/block production; preserve state | Atomic-unit reconciliation from genesis and approved migration/reset |
| Corrupt/hostile snapshot | Manifest/chunk/root verification alarms | Reject candidate database/provider; fall back to replay | Full-replay root equality and provider penalty review |
| Database corruption/power loss | Storage checksums, WAL/recovery alerts, root comparison | Fail closed; restore last authenticated checkpoint | Clean restore plus replay to same finalized root/supply |
| Validator/treasury/release-key compromise | Key-use audit logs and anomaly alert | Quorum-approved key isolation/rotation; stop affected authority | Rotation ceremony, scope proof, artifact/state integrity verification |
| Bridge/proof defect | Claim/nullifier/supply alerts | Keep or set bridge fail closed; stop relayer/RPC advertisement | Source/destination reconciliation and independent verifier re-test |
| RPC/P2P DDoS or eclipse | Peer-diversity, queue, latency, error, bandwidth alerts | Enforce quotas/edge controls; protect anchors; degrade noncritical APIs | Connectivity/root convergence and capacity report |
| Malicious/compromised image | Signature/provenance verification and runtime attestation | Quarantine digest; halt rollout; preserve hosts | Rebuilt signed image from clean source, SBOM delta, host/key review |

## 8.3 Operational prerequisites

Before any GO decision:

- incident commander, protocol lead, SRE lead, security lead, communications owner, and decision authority are named;
- 24/7 or explicitly bounded launch coverage matches published response targets;
- private vulnerability reporting is verified end to end;
- status page and stakeholder notification templates exist;
- chain halt, validator isolation, RPC degradation, image rollback, snapshot restore, DNS/certificate failover, and evidence preservation are rehearsed;
- RTO/RPO/SLO targets are approved and measured; and
- no runbook instructs operators to edit balances, suppress errors, trust an unverified snapshot, or delete finalized history.

---

# 9. Executive GO/NO-GO checklist

All boxes are intentionally unchecked for the current tree.

## 9.1 Public testnet

- [ ] PTG-01 approved normative protocol and feature scope PASS
- [ ] PTG-02 authenticated account/validator/peer identity PASS
- [ ] PTG-03 canonical commitments and PoW parity PASS
- [ ] PTG-04 branch state, atomic commit, and reorg PASS
- [ ] PTG-05 active safe finality PASS
- [ ] PTG-06 deterministic economics/genesis/supply PASS
- [ ] PTG-07 SAGA/ΩMEGA isolation PASS
- [ ] PTG-08 propagation, eclipse controls, and SyncV2 PASS
- [ ] PTG-09 every optional high-risk feature PASS or NOT APPLICABLE
- [ ] PTG-10 supported build/test/fuzz/formal gate PASS
- [ ] PTG-11 resource safety and honest capacity PASS
- [ ] PTG-12 deployment, edge, observability, restore, and rollback PASS
- [ ] PTG-13 contract/deployment allowlist PASS
- [ ] PTG-14 documentation/metadata/security operations PASS
- [ ] PTG-15 independent audit and public fault rehearsal PASS
- [ ] Exact release commit, artifact digests, genesis/checkpoint, and evidence dossier recorded
- [ ] Protocol, security, economics, SRE, and independent readiness owners sign GO

**Current public-testnet decision: NO-GO / NOT READY.**

## 9.2 Mainnet

- [ ] Every public-testnet checkbox remains valid for the mainnet-compatible candidate
- [ ] MNG-01 through MNG-13 PASS
- [ ] Mainnet genesis/allocation/supply attestation signed
- [ ] Validator set and key custody meet approved independence/rotation policy
- [ ] Sustained public-testnet observation, upgrade, restore, partition, and incident evidence accepted
- [ ] Mainnet capacity margin, SLO, RTO, RPO, and failure-domain evidence accepted
- [ ] All enabled bridge/contracts/products independently audited and capped
- [ ] Dependency, license, SBOM, provenance, and artifact-signature gates PASS
- [ ] No unresolved critical/high or SEV-1/SEV-2 launch finding
- [ ] Mainnet launch/rollback/communications ceremony rehearsed
- [ ] Governance, protocol, security, economics, operations, legal/compliance as applicable, and independent auditors sign the exact candidate

**Current mainnet decision: NO-GO / NOT READY; mainnet review is premature.**

## 9.3 Required decision record

| Field | Required before GO |
|---|---|
| Release source | Exact clean commit and signed tag |
| Protocol | Approved QIP/spec hashes and activation rule |
| Network | Chain ID, genesis/checkpoint/state root, bootnodes, limits |
| Economics | Allocation manifest, issued/circulating supply, schedule and treasury controls |
| Artifacts | Binary/image/contract bytecode digests, SBOM, provenance, signatures |
| Evidence | Gate dossier index with raw reproducible artifacts |
| Reviews | Named independent reports and remediation re-tests |
| Operations | SLO/RTO/RPO, on-call roster, incident/rollback/restore drill results |
| Residual risk | Explicit owner, bounded impact, acceptance authority, review trigger |
| Signatures | Protocol, security, economics, SRE/release, product/legal as applicable, independent readiness reviewer |

## 10. Final readiness statement

Qanto is not one patch away from launch. The minimum safe path is the P0 reconstruction in <code>REMEDIATION_ROADMAP.md</code>, governed by QIP-001 through QIP-006. QIP-007 and other optional value features stay disabled unless they pass their own complete gates.

The next valid readiness reassessment must examine one pinned candidate and its evidence dossier. It must not infer readiness from worktree changes, passing unit-test counts, simulated throughput, configuration constants, or unverified product claims.
