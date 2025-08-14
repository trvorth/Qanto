# Layer-0 Blockchain Assessment Rubric

Scale (0–5)
- 0: Critical deficiency/missing
- 1: Major gaps requiring fundamental redesign
- 2: Significant issues needing substantial work
- 3: Acceptable with notable improvements needed
- 4: Good implementation with minor enhancements
- 5: Excellent/production-ready

Default Criterion Weights
- Architecture & Design: 16%
- Cross-Chain Interoperability: 16%
- Security Infrastructure: 20% [Gating minimum: 3.0]
- Performance & Scalability: 16%
- Developer Experience: 12%
- Network Resilience: 12%
- Data & State Management: 8%

Scoring Workflow
1) Score each sub-criterion 0–5 using anchors and record evidence.
2) Multiply by sub-criterion weight; sum for criterion subtotal (0–5).
3) Multiply criterion subtotal by its weight; sum all for overall score.
4) Apply gating: if Security < 3.0, cap maturity at Pre-alpha.
5) Apply Red Flag Override: if triggered, provisionally cap overall at 2.4 until mitigated.

Maturity Bands
- 0.0–1.4: Proto/unsafe
- 1.5–2.4: Pre-alpha
- 2.5–3.4: Beta
- 3.5–4.4: Production-ready with caveats
- 4.5–5.0: Enterprise-grade/robust production

-----------------------------

## 1) Architecture & Design (16%)
Sub-criteria and weights
- Modularity & Separation of Concerns (25%)
- Scalability Patterns & Roadmap (25%)
- Consensus Mechanism & Finality (35%)
- Governance & Upgradeability (15%)

Anchors and evidence
- 0–1: Monolithic, unclear interfaces; consensus ad hoc/undocumented; no upgrade path. Evidence: missing module APIs/specs.
- 2: Partial modularity; consensus selected but unproven at target scale; vague scaling plan. Evidence: draft specs only.
- 3: Clear modules; documented consensus/finality; initial scaling pattern (e.g., shards/rollups/relays) with trade-offs. Evidence: diagrams, reference impls.
- 4: Pluggable modules (consensus/execution/DA); proven finality in prod/testnet; formal specs for critical paths; staged upgrades. Evidence: versioned specs.
- 5: Composable, versioned modules with compatibility guarantees; consensus with safety/liveness proofs or peer-reviewed specs; scalable-by-design. Evidence: formal proofs and conformance tests.

Red flags
- Hidden global state; circular deps; hard-fork-only upgrades; governance centralization w/o sunset; undocumented consensus deviations.

## 2) Cross-Chain Interoperability (16%)
Sub-criteria and weights
- Bridge Security Model (verification trust) (35%)
- Generalized Message Passing (30%)
- Chain Abstraction & Developer UX (20%)
- Finality Handling & Failure Modes (15%)

Anchors and evidence
- 0–1: No interop/custodial; centralized relayers; no verification. Evidence: admin keys, opaque ops.
- 2: Multisig bridges; limited chains; basic replay protection; partial light-client usage. Evidence: key policies, code paths.
- 3: Standardized messaging (IBC-like/SPV); multi-chain; defined timeouts/retries. Evidence: conformance tests, replay protection docs.
- 4: Trust-minimized (light clients, zk/optimistic); cross-domain routing; abstraction APIs (gas/fee/account). Evidence: audited proof systems.
- 5: Audited, production-grade bridges and generalized messaging; unified identity/fee abstraction; robust reorg/replay handling; route discovery/fallback. Evidence: audits, uptime SLAs.

Red flags
- Undisclosed trusted parties; privileged relayers; “oracle finality” w/o economic security; no reorg strategy per chain.

## 3) Security Infrastructure (20%) [Gating minimum: 3.0]
Sub-criteria and weights
- Cryptographic Primitives & Key Management (25%)
- Threat Modeling & Secure Design (25%)
- Verification & Testing Depth (25%)
- Audits, Bounties, and Incident Response (25%)

Anchors and evidence
- 0–1: Weak/outdated crypto; no KMS/HSM; no threat model. Evidence: legacy hashes, single admin key.
- 2: Standard crypto but inconsistent; partial threat analysis; ad-hoc tests; no audit plan. Evidence: missing rotation policy.
- 3: Modern crypto (Ed25519/BLS/secp256k1; Keccak/Poseidon as appropriate); documented threat model (DoS, MEV, equivocation, bridge attacks); fuzzing/property tests; runbooks. Evidence: coverage reports, runbooks.
- 4: Defense-in-depth (rate limits, access control, slashing); HSM/TEE support; SAST/DAST/fuzzing in CI; external reviews scheduled; secrets policy. Evidence: CI logs.
- 5: Multiple independent audits with remediation; formal verification where applicable; active bug bounty; continuous security testing; incident drills and forensics playbooks. Evidence: audit PDFs, timelines.

Red flags
- Unreviewed consensus/bridge changes near launch; privileged reorder/finality; “security via obscurity.”

## 4) Performance & Scalability (16%)
Sub-criteria and weights
- Throughput Architecture & Parallelism (35%)
- Latency & Propagation (25%)
- Resource Efficiency (20%)
- Benchmarking & SLOs (20%)

Anchors and evidence
- 0–1: No perf targets; blocking I/O; serial execution. Evidence: no perf dashboards.
- 2: Prototype throughput; known bottlenecks; inefficient I/O/storage. Evidence: TODOs without roadmap.
- 3: Batching/pipelining; partial parallelism; initial TPS/time-to-finality tests. Evidence: reproducible benchmarks.
- 4: Optimized networking (gossip/QUIC/erasure coding); parallel execution/scheduling; mempool QoS; predictable p95/p99; resource caps. Evidence: flamegraphs, tracked regressions.
- 5: Production SLOs under adversarial load; elastic scaling; congestion control; real-time metrics and alerts. Evidence: public dashboards, geo test results.

Quantitative indicators (tailor to design)
- Sustained TPS and p95/p99 finality at validator sizes 50/100/200 across 3+ regions.
- Block propagation p95 < 1s at 100 validators on commodity hardware.
- CPU/memory/bandwidth caps within targets; state growth < threshold under stress.

Red flags
- Non-reproducible TPS claims; single-region benchmarks; mempool starvation for small tx.

## 5) Developer Experience (12%)
Sub-criteria and weights
- Documentation & Learning Paths (25%)
- SDKs, APIs, and Stability (30%)
- Tooling & Local Environments (25%)
- Deployment & Observability (20%)

Anchors and evidence
- 0–1: Minimal docs; unstable APIs; manual ops only. Evidence: broken quickstarts.
- 2: Basic docs; incomplete SDKs; limited examples; flaky testnet. Evidence: unversioned changes.
- 3: Stable APIs; SDKs (major langs); CLI; usable devnet/testnet; templates. Evidence: example repos.
- 4: Emulators/simulators; scaffolding; CI-ready templates; testnet→mainnet workflows; migration guides; observability hooks. Evidence: demo apps, error catalog.
- 5: One-click local stacks; rich SDKs; package registries; automated deployments; deep telemetry; responsive support/SLA. Evidence: TTFD < 30 min from clean machine.

Red flags
- Undocumented breaking changes; private/internal-only docs; non-deterministic local setup.

## 6) Network Resilience (12%)
Sub-criteria and weights
- Fault Tolerance & Diversity (35%)
- Recovery & Continuity (30%)
- Decentralization & Censorship Resistance (20%)
- Operational Excellence (15%)

Anchors and evidence
- 0–1: Single point of failure; central operators. Evidence: one hosting entity.
- 2: Some redundancy; weak recovery; limited geo diversity. Evidence: single cloud.
- 3: BFT characteristics specified; snapshots/checkpoints; moderate validator distribution; runbooks. Evidence: RTO/RPO targets.
- 4: Diverse operators/regions; anti-eclipse; auto-failover; slashing incentives; chaos testing. Evidence: drill results.
- 5: Proven high availability; DR drills; censorship resistance; SRE on-call; incident retrospectives. Evidence: uptime history, Nakamoto coefficient.

Quantitative indicators
- Validator/operator count and distribution; ASN/cloud/on-prem diversity.
- Nakamoto coefficient and trend.
- Historical downtime, MTTR; RTO ≤ 2h, RPO ≤ 15m.

Red flags
- Governance capture by infra provider; recovery dependent on centralized key.

## 7) Data & State Management (8%)
Sub-criteria and weights
- State Model & Synchronization (30%)
- Storage Backends & Pruning (25%)
- Data Availability (DA) Strategy (25%)
- Proofs & Light Client Friendliness (20%)

Anchors and evidence
- 0–1: Undefined state model; data loss risk. Evidence: no snapshotting.
- 2: Basic KV; monolithic state; full sync only; no compaction. Evidence: unbounded DB growth.
- 3: Clear state model; snapshot/warp sync; pruning modes; indexing. Evidence: sync logs, index schema.
- 4: Modular storage; sharding/partitioning; proof-backed fast sync; efficient pruning; DA integrations. Evidence: DA specs, sampling.
- 5: Succinct state proofs (zk/STARKs where applicable); tiered storage; light-client-friendly commitments; cross-chain state verification. Evidence: proof benchmarks.

Quantitative indicators
- Full node initial sync time on commodity hardware; bandwidth/CPU.
- Light client finality verification cost (bytes/seconds).
- DB size growth per 1M tx; pruning throughput/overhead.

Red flags
- Impractical sync times; DA dependency without redundancy; pruning that risks safety.

-----------------------------

## Evidence Collection Checklist
- Architecture specs, diagrams, module APIs, formal specs/proofs
- Benchmarks (harness, configs), dashboards, profiling artifacts
- Security: threat model, KMS/HSM, tests, audits, bug bounty
- Interop: bridge proofs, reorg/finality handling docs
- DX: docs, SDKs, quickstarts, templates, emulators, CI
- Ops: runbooks, chaos tests, incident history, validator metrics
- Data/state: sync logs, DB metrics, pruning settings, DA proofs

## Validation Steps
- Reproduce benchmarks across 3 regions; record p95/p99.
- Verify audits and remediation by tag/commit; scan for unaudited deltas.
- Run adversarial tests: mempool flood, partitions, relayer failures, equivocation, DA withholding.
- Sync a full node and a light client; measure time, bandwidth, CPU, proof sizes.
- Execute cross-chain message across 2+ domains; test reorg/retry/timeouts.

## Red Flag Override Policy (provisional cap 2.4)
- No external audit on consensus/bridge core pre-mainnet.
- Custodial bridge >20% TVL with opaque governance.
- >40% validators depend on a single operator/cloud.
- Published performance not reproducible within ±20% using provided harness.

## Archetype-specific Weighting (optional)
- Messaging/Interoperability hubs: Cross-Chain 25%, Security 22%, Performance 14%, Architecture 14%, DX 12%, Resilience 9%, Data 4%.
- Data Availability layers: Data 22%, Performance 18%, Security 20%, Architecture 14%, Resilience 12%, DX 8%, Interop 6%.
- Relay/Consensus layers: Architecture 20%, Security 22%, Resilience 16%, Performance 18%, Interop 10%, DX 8%, Data 6%.

## Reviewer Prompts
- Architecture: Module boundaries and stability guarantees? Cross-module invariants?
- Interop: Verification model per route? Reorg/finality mismatch handling?
- Security: Assets/actors in threat model? Which invariants are formally specified/tested?
- Performance: Slowest path at p99 under bursty load? QoS across tx classes?
- DX: Time-to-first-deploy from a clean machine? How are breaking changes handled?
- Resilience: Minimum failures to lose safety/liveness? Evidence for geo/diversity targets?
- Data: Max acceptable sync time? How are DA failures surfaced/handled?

## Scoring Worksheet (fill-in)
Project: ____________________  Date: __________  Reviewer(s): __________

Architecture & Design (16%)
- Modularity ____/5 (x0.25)
- Scalability patterns ____/5 (x0.25)
- Consensus & finality ____/5 (x0.35)
- Governance/upgradeability ____/5 (x0.15)
Subtotal: __.__/5

Cross-Chain Interoperability (16%)
- Bridge security model ____/5 (x0.35)
- Message passing ____/5 (x0.30)
- Chain abstraction ____/5 (x0.20)
- Finality/failure handling ____/5 (x0.15)
Subtotal: __.__/5

Security Infrastructure (20%) [Minimum 3.0]
- Crypto & key mgmt ____/5 (x0.25)
- Threat model ____/5 (x0.25)
- Verification/testing ____/5 (x0.25)
- Audits/bounty/IR ____/5 (x0.25)
Subtotal: __.__/5

Performance & Scalability (16%)
- Throughput/parallelism ____/5 (x0.35)
- Latency/propagation ____/5 (x0.25)
- Resource efficiency ____/5 (x0.20)
- Benchmarks/SLOs ____/5 (x0.20)
Subtotal: __.__/5

Developer Experience (12%)
- Docs/learning ____/5 (x0.25)
- SDKs/APIs ____/5 (x0.30)
- Tooling/local env ____/5 (x0.25)
- Deployment/observability ____/5 (x0.20)
Subtotal: __.__/5

Network Resilience (12%)
- Fault tolerance/diversity ____/5 (x0.35)
- Recovery/continuity ____/5 (x0.30)
- Decentralization/censorship ____/5 (x0.20)
- Ops excellence ____/5 (x0.15)
Subtotal: __.__/5

Data & State Management (8%)
- State/sync ____/5 (x0.30)
- Storage/pruning ____/5 (x0.25)
- DA strategy ____/5 (x0.25)
- Proofs/light client ____/5 (x0.20)
Subtotal: __.__/5

Overall weighted score: __.__/5
Maturity band: __________
Gating check (Security ≥ 3.0): Pass/Fail
Red flags present (cap 2.4): Yes/No

