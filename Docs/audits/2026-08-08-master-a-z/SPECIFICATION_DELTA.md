# Qanto Specification Delta Report

Audit date: 2026-08-08  
Effective-tree reconciliation: 2026-08-09  
Audit target: effective working tree at `/Users/cps/qanto`  
Mandatory scope: workstream 12 — `OVERVIEW.md`, `README.md`, `SECURITY.md`, `CONTRIBUTING.md`, and `CODE_OF_CONDUCT.md` cross-checked against current source and the workstream 2-10 audit evidence  
Change scope: this report only; no source or user-facing documentation was edited by this audit track

## Executive verdict

**NOT READY.** The root README, security policy, contributor guide, maintained WQNTO/airdrop guides, Hardhat configuration, deployment script, and draft exchange payload now describe the effective pre-release state much more honestly. `OVERVIEW.md`, however, still contradicts its own pre-release warning by presenting inactive DPoS/PoSe finality, bridge minting, PQ transport, SAGA/green-mining controls, hyper-sharding, performance, and contract products as active. Legacy token/operator/RPC metadata also still publishes an incorrect 2.5-QNTO reward and obsolete Hugging Face endpoints. A reader cannot derive one safe integration contract from the documentation set.

The canonical facts that are currently supportable are narrower:

- the Rust virtual workspace and broad subsystem layout exist (`Cargo.toml:9-20`);
- native and wrapped QNTO use nine decimals (`Protocol_Core/qanto-node/src/lib.rs:164-180`; `SmartContracts/contracts/WQNTO.sol:11-15`);
- the sealed 21B/80%/15%/5% constants sum correctly at compile time (`Protocol_Core/qanto-node/src/lib.rs:173-187`);
- default ML-DSA signing and verification invoke real cryptographic code, but keys are not bound to transaction senders or validator identities (`Protocol_Core/qanto-core/src/qanto_native_crypto.rs:533-541`, `:589-614`; `Protocol_Core/qanto-node/src/transaction.rs:496-548`);
- bridge claims, unsafe AMM entry points, automatic finality, and automatic dynamic sharding are currently contained by failing closed or remaining disabled (`Protocol_Core/qanto-node/src/qantodag.rs:2938-2953`, `:4414-4419`; `SmartContracts/contracts/QantoSwapFactory.sol:18-25`; `SmartContracts/contracts/QantoSwapRouter.sol:23-37`).
- the Solidity suite now has a measured baseline of 42.45% line and 43.36% statement coverage with 38/38 tests passing, but critical contracts remain at 0% (`Docs/audits/2026-08-08-master-a-z/CONTRACTS_PERFORMANCE_OPERATIONS.md:161-192`).

Detailed evidence is in:

- `Docs/audits/2026-08-08-master-a-z/CONSENSUS_NETWORK_TRANSACTION_SAGA.md:34-130`, `:134-179`, `:183-224`, `:228-293`;
- `Docs/audits/2026-08-08-master-a-z/PQC_AND_TOKENOMICS.md:30-159`, `:163-341`;
- `Docs/audits/2026-08-08-master-a-z/CONTRACTS_PERFORMANCE_OPERATIONS.md:44-183`, `:187-280`, `:284-417`.

Effective-state precedence: the current reconciliation in this report controls over older audit snapshots. The maintained integration guides, WQNTO deployment tooling, fail-closed bridge status, and measured Solidity coverage were reverified after those snapshots. The underlying absent bridge verifier and low contract coverage remain real.

## Classification method

Every substantive `OVERVIEW.md` claim is assigned one of the mandated classifications:

- `IMPLEMENTED_AS_SPECIFIED` — the cited implementation materially matches the documented behavior.
- `PARTIALLY_IMPLEMENTED` — real code exists, but a material guarantee, integration, or safety property is absent.
- `SPECIFIED_BUT_UNIMPLEMENTED` — the claimed production behavior is absent, dormant, simulated, disabled, or fail-closed.
- `IMPLEMENTED_BUT_UNDOCUMENTED` — effective behavior exists but the primary documents omit it or describe the opposite.

These labels describe specification alignment, not launch readiness. A component can be `IMPLEMENTED_AS_SPECIFIED` and still need independent review.

Path convention in the matrices: unqualified Rust filenames such as `qantodag.rs` resolve under `Protocol_Core/qanto-node/src/`; unqualified Solidity filenames resolve under `SmartContracts/contracts/`. All other paths are repository-relative and were checked in the effective tree.

---

# 12.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| SPEC-001 | INFO | Product maturity disclosure | `README.md:1-29`, `:395`; `OVERVIEW.md:1-5`; `SECURITY.md:5-9`, `:36-40` | The principal maturity surfaces now call Qanto a pre-release research prototype, disclaim public-network/value readiness, and require independent audit. | `FIXED_IN_WORKTREE`; earlier production-grade claim removed |
| SPEC-002 | SEV-1 | Hybrid consensus and finality | `OVERVIEW.md:47-59`, `:68-79`, `:191-214`; `README.md:21-29`, `:179-187`, `:267-274`; `Protocol_Core/qanto-node/src/qantodag.rs:2915-3319`, `:3460-3555`, `:4414-4419`; `Protocol_Core/qanto-node/src/consensus.rs:97-193`, `:264-288` | README now accurately limits the active rule to fixed-difficulty PoW. OVERVIEW still depicts DPoS finality, PoSe, SAGA, and ΩMEGA as validation gates even though inbound acceptance enforces no DPoS committee/quorum, no PoSe verifier exists, and automatic finality is disabled. | `PARTIALLY_IMPLEMENTED`; implementation blocker and internal specification conflict |
| SPEC-003 | SEV-1 | Post-quantum security | `README.md:25-29`, `:69-78`; `OVERVIEW.md:63-65`, `:176-185`; `Protocol_Core/qanto-node/src/transaction.rs:496-548`, `:842-900`; `Protocol_Core/qanto-core/src/qanto_p2p.rs:246-259`, `:482-550` | README now qualifies the PQ direction. OVERVIEW still claims Kyber key exchange and categorical quantum resistance although the supplied ML-DSA key is not bound to the claimed sender/validator and advertised ML-KEM material establishes no PQ session. | `PARTIALLY_IMPLEMENTED`; launch blocker and OVERVIEW overclaim |
| SPEC-004 | SEV-1 | Bridge/ZK pipeline | `OVERVIEW.md:81-85`, `:248-269`, `:311-312`; `README.md:55-65`, `:100-114`; `Docs/bridge-wqnto.md:3-7`, `:19-28`, `:53-66`; `Protocol_Core/qanto-zk-sdk/src/lib.rs:240-250`; `Protocol_Core/qanto-node/src/qantodag.rs:2938-2953`, `:4002-4014` | README and the maintained bridge guide now accurately state that WQNTO is only a same-chain wrapper and bridge claims fail closed. OVERVIEW still depicts a successful lock-proof-mint flow. | `SPECIFIED_BUT_UNIMPLEMENTED`; containment documented, bridge unavailable |
| SPEC-005 | SEV-1 | Monetary specification | `README.md:35-53`; `OVERVIEW.md:138-168`; `SmartContracts/token-info.json:9-17`; `Docs/node-operator-guide.md:1-7`; `Docs/rpc-api.md:1-14`, `:74-82`, `:161-169`; `Protocol_Core/qanto-node/src/emission.rs:6-10`, `:71-129`; `Protocol_Core/qanto-node/src/qantodag.rs:2233-2242`, `:2778-2788`, `:3122-3148`; `Protocol_Core/qanto-node/src/saga.rs:3426-3434`, `:3754-3855`; `config/genesis.json:7-20` | README now accurately marks reward/cap behavior unresolved. OVERVIEW still describes the standalone recurrence as the live long-term incentive, while legacy token/operator/RPC docs still advertise 2.5 QNTO. Live rewards use local floating SAGA state, count fees as issuance, do not enforce the cap on ledger state, and coexist with a genesis allocating the full 21B while the counter starts at zero. | `PARTIALLY_IMPLEMENTED`; economic launch blocker and residual documentation conflict |
| SPEC-006 | SEV-2 | Performance and sharding | `OVERVIEW.md:75-79`, `:112-118`, `:176-185`; `README.md:25-29`, `:140-142`; `Protocol_Core/qanto-node/src/qantodag.rs:68-74`, `:940-960`, `:998-999`, `:1042-1062`, `:4414-4419`, `:4497-4545`; `Protocol_Core/qanto-node/src/sharding.rs:21-90` | README now explicitly rejects unverified TPS/latency claims. OVERVIEW still asserts 10M+, sub-31ms, a bounded 262,144 queue, SIMD signature batching, and linearly scaling hyper-sharding without reproducible evidence; several descriptions are false as wired. | `SPECIFIED_BUT_UNIMPLEMENTED`; OVERVIEW claim removal still required |
| SPEC-007 | SEV-2 | SAGA/ΩMEGA/green mining | `OVERVIEW.md:57-59`, `:68-73`, `:170-185`, `:271-300`; `README.md:21-29`, `:179-187`; `Protocol_Core/qanto-node/src/saga.rs:2018-2055`, `:3625-3716`, `:3892-3950`, `:4266-4337`; `Protocol_Core/qanto-node/src/omega.rs:329-523` | README now describes SAGA as experimental. OVERVIEW still presents live neural governance, verified geospatial carbon scoring, Karma fee/validator effects, and ΩMEGA acceptance. Active behavior is heuristic/local, the green checks are simulated, and ΩMEGA is not the final block gate. | `PARTIALLY_IMPLEMENTED`; consensus-affecting local state is unsafe |
| SPEC-008 | SEV-2 | Smart-contract product claims | `OVERVIEW.md:101-104`, `:120-125`; `README.md:55-65`, `:100-114`; `SmartContracts/contracts/QUSD.sol:9-18`, `:24-38`; `SmartContracts/contracts/QantoTGE.sol:12-19`, `:28-48`; `SmartContracts/contracts/QantoTimelock.sol:6-12`; `SmartContracts/contracts/QantoSwapFactory.sol:18-25`; `SmartContracts/contracts/QantoSwapRouter.sol:23-37` | README now blocks DEX/bridge integration and correctly describes WQNTO. OVERVIEW still calls QUSD collateral-backed, describes vesting and SAGA-integrated governance, and says the suite runs on EVM-compatible shards; those product capabilities are absent or disabled. | `SPECIFIED_BUT_UNIMPLEMENTED`; affected products must remain disabled |
| SPEC-009 | SEV-2 | Residual reward and endpoint metadata | `README.md:35-53`, `:112-136`, `:340-360`; `SmartContracts/token-info.json:7-17`; `SmartContracts/cex-listing-payload.json:3-6`, `:12-25`, `:28-44`, `:57-60`; `Docs/node-operator-guide.md:1-7`; `Docs/rpc-api.md:1-14`, `:74-82`, `:161-169`; `Protocol_Core/qanto-node/src/lib.rs:164-180`; `Protocol_Core/qanto-node/src/config.rs:228-278` | README and the draft CEX payload now use nine decimals, chain ID 1234/`0x4D2`, an unresolved reward, null endpoints, and explicit do-not-integrate status. `token-info.json`, the operator guide, and the RPC guide still publish a 2.5-QNTO reward; `token-info.json` and the RPC guide retain legacy HF endpoints. | `PARTIALLY_IMPLEMENTED`; maintained release surfaces fixed, legacy metadata still unsafe |
| SPEC-010 | SEV-2 | Public deployment topology | `README.md:278-310`, `:340-360`; `PUBLIC_TESTNET_LAUNCH.md:1-19`; `SmartContracts/hardhat.config.js:26-32`; `SmartContracts/cex-listing-payload.json:28-44`; `DevOps/docker/ubuntu-testnet-ignite.sh:4-8`, `:134-153`, `:370-403` | README now labels Docker/Kubernetes/Helm unproven and HF endpoints legacy, Hardhat requires environment-injected RPC/key values, and the CEX payload publishes no endpoint. Dedicated VPS/Docker intent exists, but the preferred Ubuntu bootstrap remains unreproducible and no authoritative live topology has been demonstrated. | `PARTIALLY_IMPLEMENTED`; disclosure fixed, operational launch contract absent |
| SPEC-011 | SEV-2 | Test-suite description and coverage | `OVERVIEW.md:127-132`; `Protocol_Core/qanto-node/src/lib.rs:131-140`; `.config/nextest.toml:1-47`; `SmartContracts/test/contracts.test.js:9-145`, `:149-288`; `Docs/audits/2026-08-08-master-a-z/CONTRACTS_PERFORMANCE_OPERATIONS.md:161-192` | The claimed `Scripts/tests/` directory and named files do not exist. Rust security/partition/economic tests live inside `qanto-node`, and canonical Nextest configuration exists. Hardhat compiles ten contracts and passes 38/38 tests, but measured coverage is only 42.45% lines and 43.36% statements; TGE, Drop, SentinelNFT, and Governor are 0%/0%. The overview still points to a nonexistent suite and presents synthetic performance harnesses as evidence. | `PARTIALLY_IMPLEMENTED`; measured baseline exists, inventory and critical coverage remain inadequate |
| SPEC-012 | INFO | Vulnerability reporting | `README.md:366-372`; `SECURITY.md:11-32`, `:45-51`; `CONTRIBUTING.md:75-77` | README, SECURITY, and CONTRIBUTING now consistently require private reporting, prohibit public disclosure, use response targets, and disclaim a formal bounty. The fallback still does not name a verified private recipient if GitHub private reporting is unavailable. | `FIXED_IN_WORKTREE` for public guidance; operational fallback pending under SPEC-013 |
| SPEC-013 | SEV-3 | Security-policy operations | `SECURITY.md:16-32`, `:36-51` | The revised policy is materially actionable if GitHub private vulnerability reporting is enabled. Remaining gaps are a verified fallback recipient/encrypted channel, named duty owner and emergency escalation, explicit severity taxonomy, safe-harbor scope, and evidence that the stated response targets are staffed. | `PARTIALLY_IMPLEMENTED`; improved in worktree, operational verification pending |
| SPEC-014 | INFO | Contributor guide | `CONTRIBUTING.md:7-39`, `:41-77`; `rust-toolchain.toml:1-4`; `Cargo.toml:9-20`; `.config/nextest.toml:1-47` | The guide now defers to the pinned toolchain, uses locked workspace checks and Nextest, names the Solidity workspace, warns against secret commits, calls `main` an integration branch, gives the full ZK source path, and strengthens proof-test/setup requirements. Node/npm compatibility and required environment variables are still not concretely enumerated, and clean-machine reproducibility is not yet evidenced. | `PARTIALLY_IMPLEMENTED`; major documentation fix in worktree |
| SPEC-015 | INFO | Code-of-conduct enforcement | `CODE_OF_CONDUCT.md:26-38`, `:40-66` | The Contributor Covenant body now provides a private GitHub-advisory reporting path and an abuse-report fallback. It still lacks a conflict-of-interest alternative when repository owners are implicated, an appeal/recusal process, response targets, and clarity on whether security-advisory recipients are the conduct team. | `PARTIALLY_IMPLEMENTED`; reporting route fixed, governance details remain |
| SPEC-016 | SEV-3 | Documentation links and paths | `OVERVIEW.md:94-132`; `README.md:148-176`; repository inventory | OVERVIEW hard-codes `file:///Users/trvorth/...` links, names `qanto-desktop/` instead of the workspace member `qanto-desktop-wallet/src-tauri`, and names nonexistent `Scripts/tests/`. README now uses the correct `Docs/` spelling and desktop-wallet path. | `PARTIALLY_IMPLEMENTED`; README fixed, OVERVIEW inventory stale |
| SPEC-017 | INFO | RPC finality semantics | `README.md:226-274`; `Protocol_Core/qanto-rpc/proto/qanto.proto:29-70`; `Protocol_Core/qanto-node/src/rpc_backend.rs:631-644`; `Protocol_Core/qanto-node/src/qantodag.rs:4414-4419` | README now explicitly says `is_finalized` only reflects the stored finalized set and must not be read as a quorum certificate or irreversible settlement while automatic finality is disabled. | `FIXED_IN_WORKTREE`; protocol finality remains a separate SEV-1 implementation blocker |
| SPEC-018 | INFO | Safety containment documentation | `README.md:21-29`, `:51-57`, `:100-114`, `:267-274`, `:301-302`, `:340-342`; `Docs/bridge-wqnto.md:3-7`, `:53-66`; `Docs/guide/airdrop_guide.md:3-18`, `:20-37`; `OVERVIEW.md:3`, `:309-318` | README and maintained bridge/airdrop guides now disclose research status, disabled finality/bridge/AMM paths, incomplete sharding, unproven deployment, and airdrop non-availability. OVERVIEW's introductory caveat and roadmap acknowledge limitations but its earlier diagrams/product sections still contradict them. | `FIXED_IN_WORKTREE` on maintained integration surfaces; OVERVIEW conflicts retained in SPEC-002 through SPEC-008 |

---

# 12.B `OVERVIEW.md` claim-by-claim matrix

This matrix covers every substantive implementation claim in `OVERVIEW.md`. Headings, table-of-contents entries, and diagram syntax are not treated as independent claims.

| `OVERVIEW.md` claim | Implementation evidence | Classification | Delta |
|---|---|---|---|
| Qanto is a pre-release modular Layer-0 research codebase with Rust, RPC, P2P, crypto, contracts, frontends, and deployment assets (`:1-5`, `:34-66`, `:89-105`) | Eight-member workspace at `Cargo.toml:9-20`; 90+ public node modules at `Protocol_Core/qanto-node/src/lib.rs:3-140`; Solidity/Frontend/DevOps trees exist | `IMPLEMENTED_AS_SPECIFIED` | The pre-release caveat is accurate; absolute local links and later active-capability claims still undermine it |
| PoW provides permissionless leader election and validates QanHash (`:68-70`) | PoW is checked in `qantodag.rs:3014-3037`, but QanHash backends diverge (`myblockchain/src/qanhash.rs:1396-1427`, `kernel.cl:27-123`, `qanhash.cu:80-162`, `qanhash.metal:80-160`) | `PARTIALLY_IMPLEMENTED` | No heterogeneous-backend equivalence contract; custom hash has trailing-zero collisions |
| DPoS is a finality helper with validator header signatures and sub-second settlement (`:71`) | Stake/selection helpers exist (`consensus.rs:97-193`), but inbound acceptance does not enforce committee eligibility or quorum; automatic finality is disabled (`qantodag.rs:4414-4419`) | `SPECIFIED_BUT_UNIMPLEMENTED` | No active signed 2/3 certificate or finality horizon |
| PoSe validates storage/execution capacity (`:72`) | No storage challenge, execution receipt, commitment, or acceptance verifier was found; canonical consensus contains only descriptive references | `SPECIFIED_BUT_UNIMPLEMENTED` | Product claim has no protocol implementation |
| SAGA autonomously monitors telemetry and adjusts block size, fees, and difficulty (`:73`) | Heuristic monitoring/edicts exist (`saga.rs:3892-3950`, `:4266-4337`); difficulty is fixed (`qantodag.rs:2928-2937`); size depends on local counters (`:2955-3004`) | `PARTIALLY_IMPLEMENTED` | Edicts are local/floating and consensus unsafe; documented controls do not match canonical rules |
| Blocks form a multi-parent DAG (`:75-77`; diagram `:216-246`) | Parent vectors and DAG traversal exist (`qantodag.rs:3048-3090`, `:5836-5989`) | `PARTIALLY_IMPLEMENTED` | Duplicate parents and noncanonical roots are not rejected; branch state is global/arrival-dependent |
| Topological ordering guarantees deterministic state transitions (`:78`) | Lexical tie-breaking/topological queues were added (`qantodag.rs:5864-5987`; `node.rs:1841-1943`) | `PARTIALLY_IMPLEMENTED` | Pure ordering improved, but global UTXO mutation/reorg absence prevents deterministic canonical state |
| Hyper-sharding scales linearly toward 10M+ TPS (`:79`) | Standalone `sharding.rs:21-90` is incomplete and overwrites split shards; automatic sharding is disabled at `qantodag.rs:4414-4419` | `SPECIFIED_BUT_UNIMPLEMENTED` | No routing, cross-shard receipts/state roots, validator assignment, atomicity, or benchmark proof |
| Relayer generates a ZK proof and the node validates it before minting (`:81-85`; diagram `:248-269`) | Relayer/SDK types exist, but SDK verification fails closed (`qanto-zk-sdk/src/lib.rs:240-250`) and bridge claims are rejected (`qantodag.rs:2938-2953`) | `SPECIFIED_BUT_UNIMPLEMENTED` | Diagram acknowledges “simulated” proof at `:262` but still depicts successful minting |
| Repository component map is current (`:91-105`) | Most named components exist | `PARTIALLY_IMPLEMENTED` | Links contain another user's absolute path; workspace desktop member is `qanto-desktop-wallet`, and the named `Scripts/tests/` suite is absent |
| CPU, CUDA, and Metal implement one QanHash algorithm (`:106-110`) | All source files exist | `PARTIALLY_IMPLEMENTED` | They compute different functions; CUDA currently fails to compile; no parity vectors/tests exist |
| Shared UTXO references avoid full clones (`:112-115`) | Shared `Arc`/lock-backed state is used in `qantodag.rs:1820-1830` | `IMPLEMENTED_AS_SPECIFIED` | Performance intent is real, but a single global UTXO makes fork/reorg semantics incorrect |
| 256 block workers and 128 validation workers are configured (`:115`) | Constants/semaphores/threadpool are wired at `qantodag.rs:97-111`, `:1042-1069` | `IMPLEMENTED_AS_SPECIFIED` | Counts are not throughput evidence and may oversubscribe documented hosts |
| Lock-free queue has capacity 262,144 (`:116`) | The transaction structure is an unbounded `SegQueue`; the number is only a monitoring threshold (`qantodag.rs:940-960`, `:998-999`, `:4497-4545`) | `SPECIFIED_BUT_UNIMPLEMENTED` | Actual bounded block queue capacity is 512 |
| SIMD batches 32 signatures (`:117`) | Cleanup iterates 32 scalar map entries; alternate validator accepts non-empty signatures (`qantodag.rs:4497-4545`; `performance_optimizations.rs:102-167`) | `SPECIFIED_BUT_UNIMPLEMENTED` | No active SIMD signature verification |
| Elite mempool is SAGA-prioritized (`:118`) | Object is constructed but not started/submitted to; dormant validator marks transactions valid (`node.rs:494-520`, `:1531-1534`; `elite_mempool.rs:345-399`) | `SPECIFIED_BUT_UNIMPLEMENTED` | Priority is fee/size, not canonical SAGA/gas integration |
| WQNTO is a nine-decimal ERC-20 wrapper (`:120-123`) | `WQNTO.sol:7-31` implements deposit/withdraw/ERC-20 and returns nine decimals | `IMPLEMENTED_AS_SPECIFIED` | Withdrawal hardening exists but lacks a dedicated test |
| QUSD is collateral-backed and pegged (`:123`) | `QUSD.sol:9-18` explicitly states it has no peg/collateral/oracle/liquidation/redemption system | `SPECIFIED_BUT_UNIMPLEMENTED` | Overview directly contradicts the contract |
| Governor enables community proposals that influence SAGA (`:124`) | Governor primitive exists (`QantoGovernor.sol:19-25`), but period math is wrong, threshold is zero, and no compatible IVotes token/deployment flow exists | `PARTIALLY_IMPLEMENTED` | No demonstrated SAGA integration or governance lifecycle |
| TGE and Timelock manage vesting (`:125`) | Fixed-rate sale and generic TimelockController exist (`QantoTGE.sol:12-48`; `QantoTimelock.sol:6-12`) | `SPECIFIED_BUT_UNIMPLEMENTED` | No allocation, cliff, release schedule, refund, or binding between them |
| `Scripts/tests/` is a comprehensive mining/DAG/difficulty/performance/SAGA suite (`:127-132`) | Directory is absent; Rust tests instead live in node modules (`qanto-node/src/lib.rs:131-140`). Hardhat's 38 tests measure 42.45% line/43.36% statement coverage, concentrated in Faucet/QUSD (`CONTRACTS_PERFORMANCE_OPERATIONS.md:161-192`) | `SPECIFIED_BUT_UNIMPLEMENTED` | Named files cannot be run from documented paths; measured Solidity coverage does not cover the claimed Rust performance/SAGA suite |
| Nine-decimal supply/allocation constants and compile-time sum are sealed (`:138-158`) | Exact constants/assertion exist at `qanto-node/src/lib.rs:164-187` | `IMPLEMENTED_AS_SPECIFIED` | Does not prove runtime issuance/cap enforcement |
| Integer asymptotic emission uses 8.4M seconds and 0.999 (`:160-168`) | Standalone `Emission::calculate_reward` implements this at `emission.rs:71-109` | `PARTIALLY_IMPLEMENTED` | Canonical candidate/validation paths call SAGA instead; current live schedule is different |
| Carbon credentials are verified with geospatial and issuer-reputation scores; Karma reduces fees/raises DPoS reputation (`:170-174`) | Schema fields exist, but verification uses predictable strings/hard-coded values and ignores those scores (`saga.rs:3625-3701`); Karma fee effect is absent | `SPECIFIED_BUT_UNIMPLEMENTED` | Current green evidence is simulated and unsafe if consensus-relevant |
| Competitive matrix: PQ yes, 10M+, <31ms, neural SAGA, green scoring, zero-gas Karma (`:176-185`) | Underlying fragments exist, but audited evidence rejects each absolute claim | `SPECIFIED_BUT_UNIMPLEMENTED` | Matrix should be removed or rewritten as unverified targets with reproducible artifacts |
| Validation diagram shows optional DPoS, ΩMEGA gate, penalty, and atomic DAG/UTXO update (`:191-214`) | `is_valid_block` has no ΩMEGA call; unsafe slashing was removed; state changes are not atomic (`qantodag.rs:1870-1943`, `:2000-2043`, `:2915-3319`) | `SPECIFIED_BUT_UNIMPLEMENTED` | Diagram is not the effective acceptance path |
| SAGA state machine detects at 0.85, classifies attacks, applies edicts, and retrains (`:271-300`) | Heuristic detectors/thresholds exist; production neural forward/training data flow has no caller/data append (`saga.rs:810-828`, `:2018-2076`, `:3892-3950`) | `PARTIALLY_IMPLEMENTED` | Attack rules are heuristics; retraining/model lifecycle is dormant |
| Roadmap records canonical decimal reconciliation (`:309-310`) | Kernel, WQNTO, README, deployment tooling, bridge guide, airdrop guide, and draft CEX payload all use nine decimals (`qanto-node/src/lib.rs:164-180`; `WQNTO.sol:11-15`; `README.md:39-49`; `Docs/bridge-wqnto.md:9-17`; `Docs/guide/airdrop_guide.md:7-18`; `deploy_wqnto.js:100-121`; `SmartContracts/cex-listing-payload.json:12-15`, `:28-35`) | `IMPLEMENTED_AS_SPECIFIED` | Six-decimal and CEX chain-ID defects are closed on maintained surfaces; legacy reward/endpoint metadata remains a separate issue |
| Roadmap records bridge verification as fail-closed and unimplemented (`:311-312`) | SDK returns a not-implemented error and native validation rejects bridge claims (`qanto-zk-sdk/src/lib.rs:240-250`; `qantodag.rs:2938-2953`) | `IMPLEMENTED_AS_SPECIFIED` | Accurate containment disclosure; a real verifier and launchable bridge remain absent |
| Roadmap records incomplete state-request/snapshot recovery and existing reconnect backoff (`:313-315`) | State request handling is nonfunctional and peer UTXOs are now discarded; backoff exists but sleeps in the swarm loop (`node.rs:1329-1333`; `p2p.rs:789-820`, `:1117-1183`, `:1392-1423`, `:1464-1485`) | `PARTIALLY_IMPLEMENTED` | Diagnosis is materially improved; the action should prioritize authenticated request/response and nonblocking retry scheduling rather than adding backoff that already exists |
| Roadmap says green scoring is simulated (`:316-318`) | Confirmed by `saga.rs:3625-3701` | `IMPLEMENTED_AS_SPECIFIED` | This candid limitation conflicts with earlier affirmative green-security claims |

---

# 12.C `README.md` consistency matrix

| README area | Evidence and delta | Classification |
|---|---|---|
| Identity/design (`README.md:1-29`) | README labels the code a research prototype, limits canonical consensus to fixed-difficulty PoW, and explicitly lists identity, state, finality, issuance, and independent-review gaps. | `IMPLEMENTED_AS_SPECIFIED` |
| Token parameters and emission status (`:35-53`) | Nine decimals, 21B sealed constants, chain ID 1234/`0x4D2`, and the 50-QNTO code bases match source. README correctly says the live SAGA reward/cap/genesis path is unresolved rather than publishing a schedule. | `IMPLEMENTED_AS_SPECIFIED` |
| PQ primitive table (`:69-78`) | README now distinguishes primitive integration from end-to-end authorization and says ML-KEM session establishment and sender/validator binding are incomplete. | `IMPLEMENTED_AS_SPECIFIED` |
| WQNTO semantics and deployment (`:55-65`, `:100-110`) | README correctly calls WQNTO a same-chain wrapper, publishes no address/liquidity claim, and limits deployment to an authorized development network. `deploy_wqnto.js:19-33`, `:52-70`, `:73-123`, `:124-152` loads the artifact, validates the constructor, requires environment authority, checks chain/confirmations/decimals, and writes a mode-0600 receipt. | `IMPLEMENTED_AS_SPECIFIED` |
| CEX integration metadata/endpoints (`:112-136`) | README marks `cex-listing-payload.json` draft and unapproved. The payload itself uses `DRAFT_DO_NOT_SUBMIT`, unresolved reward, 1234/`0x4D2`, null endpoints, no audit claim, and no deployment claim (`cex-listing-payload.json:3-6`, `:12-25`, `:28-44`, `:57-60`). | `IMPLEMENTED_AS_SPECIFIED` |
| Performance status (`:140-142`) | README now states that no reproducible throughput/latency artifact exists and enumerates the evidence required before repeating 10M+/25.447M claims. | `IMPLEMENTED_AS_SPECIFIED` |
| Architecture/consensus (`:146-187`) | The repository map is materially current and README correctly distinguishes active PoW from unenforced DPoS/PoSe and unsafe local SAGA controls. | `IMPLEMENTED_AS_SPECIFIED` |
| Desktop wallet (`:191-222`) | Tauri/React, Argon2id keystore, RPC default, and address-history calls exist (`qanto-desktop-wallet/src-tauri/src/keystore.rs:155`; `app.rs:121`; `tx.rs:479-588`). The “Ed25519 + Dilithium3 ... authorization” language at `README.md:200` remains stronger than the audited sender-binding guarantee, and release packaging was not independently certified. | `PARTIALLY_IMPLEMENTED` |
| Address-history RPC (`:226-274`) | Proto/server/index implementations exist (`qanto-rpc/proto/qanto.proto:10-70`; `qanto-rpc/src/server.rs:239-260`; `qantodag.rs:1554-1571`), and README now explicitly limits `is_finalized` to the stored set rather than a quorum/irreversibility guarantee (`README.md:267-274`). | `IMPLEMENTED_AS_SPECIFIED` |
| Quick start/config (`:278-336`) | README defers to the pinned toolchain, uses locked workspace checks and canonical Nextest, and labels Docker/Kubernetes/Helm paths unproven. The legacy RPC example is explicitly non-authoritative. | `IMPLEMENTED_AS_SPECIFIED` |
| Testnet/HF configuration (`:340-360`) | README explicitly labels the HF topology legacy and structurally unsuitable, while retaining the values only as legacy metadata. It makes no availability or launch-readiness claim. | `IMPLEMENTED_AS_SPECIFIED` |
| Security reporting (`:366-372`) | README routes to SECURITY, prohibits public vulnerability issues, and disclaims a bounty, matching `SECURITY.md:11-32`. | `IMPLEMENTED_AS_SPECIFIED` |
| License (`:387-389`) | Root LICENSE and README use MIT. | `IMPLEMENTED_AS_SPECIFIED` |

---

# 12.D Policy and contributor-document review

## `SECURITY.md`

The effective policy is now materially safer and conditionally actionable:

1. `SECURITY.md:16` directs reporters to GitHub private vulnerability reporting and explicitly prohibits a public issue.
2. `SECURITY.md:21-24` correctly describes acknowledgment, assessment, and update intervals as targets rather than guarantees.
3. `SECURITY.md:31-32` states that no formal bounty or guaranteed reward exists.
4. `SECURITY.md:36-40` distinguishes internal review from independent audit and no longer delegates human triage to SAGA/ΩMEGA.
5. `SECURITY.md:47` correctly says the repository license does not create a disclosure safe harbor.
6. `SECURITY.md:49-51` separates non-sensitive discussion from private vulnerability details.

Remaining operational gaps:

- enablement of GitHub private vulnerability reporting was not verified from repository source;
- when that feature is unavailable, `SECURITY.md:16` says to contact repository owners through project channels but does not identify a specific private/encrypted route;
- no named on-call owner, critical escalation path, severity taxonomy, embargo/coordination policy, or approved safe-harbor scope is published;
- the 48-hour/7-day/weekly targets are reasonable for a staffed project, but staffing and alerting cannot be verified from code.

Classification: `PARTIALLY_IMPLEMENTED`; README reconciliation and the dangerous public-issue instruction are fixed, while operational verification remains open.

## `CONTRIBUTING.md`

The effective guide now covers the pinned toolchain, locked Rust workspace check, canonical Nextest profile, Solidity installation/test, secret hygiene, integration-branch status, exact ZK source path, negative proof testing, setup review, and private security routing (`CONTRIBUTING.md:7-77`). These are substantive fixes.

Remaining limitations:

- Node/npm support is described generically rather than pinned through an explicit version file or documented range (`:15`);
- required variables across the root, Solidity, relayer, frontend, and deployment examples are not enumerated (`:39`);
- formatting/Clippy commands are documented at `:52-55`, but the effective dirty tree's all-target quality gate must be reported from the verification workstream rather than assumed;
- no clean Ubuntu/macOS checkout transcript proves the full contributor path, and frontend packages outside SmartContracts are not covered;
- branch names are documented, but branch-protection/merge enforcement is an external repository setting.

Classification: `PARTIALLY_IMPLEMENTED`, with the major onboarding defects fixed in the effective worktree.

## `CODE_OF_CONDUCT.md`

The document substantially reproduces Contributor Covenant 2.1 and states scope, standards, enforcement responsibilities, and four enforcement levels (`CODE_OF_CONDUCT.md:1-66`). The effective worktree now provides a private GitHub-advisory route and GitHub abuse-report fallback at `:35-38`, fixing the absence of any reporting mechanism.

Remaining governance gaps are a conflict-of-interest alternative when repository/security owners are implicated, a dedicated conduct team or recipient, response targets, recusal/appeal rules, and clarity on retention/confidentiality of conduct reports submitted through the security-advisory system.

Classification: `PARTIALLY_IMPLEMENTED`.

---

# 12.E Implemented documentation/metadata fixes already in the effective tree

This audit track made no edits outside this report. The following corrections were already present in the effective shared worktree and are therefore recorded without claiming sole authorship:

| Fix | Effective evidence | Completeness |
|---|---|---|
| Root maturity, consensus, PQ, emission, bridge/DEX, performance, deployment, finality, and security disclosures are reconciled | `README.md:1-29`, `:35-57`, `:69-78`, `:100-114`, `:140-142`, `:179-187`, `:267-274`, `:278-310`, `:340-372` | README now consistently describes a research prototype and names the live blockers; the desktop hybrid-authorization sentence at `:200` still needs qualification |
| Canonical nine-decimal standard is reconciled across maintained integration surfaces | `README.md:39-49`, `:59-65`; `OVERVIEW.md:309-310`; `Docs/bridge-wqnto.md:9-17`; `Docs/guide/airdrop_guide.md:7-18`; `SmartContracts/cex-listing-payload.json:12-15`, `:28-35`; canonical code at `qanto-node/src/lib.rs:164-180`, `WQNTO.sol:11-15` | Correct on maintained surfaces; the separate reward-path conflict remains |
| Chain ID 1234/`0x4D2` is consistent across maintained deployment/integration surfaces | `README.md:39-49`, `:125-135`, `:314-336`, `:340-360`; `SmartContracts/hardhat.config.js:26-32`; `SmartContracts/cex-listing-payload.json:28-35`; runtime default at `config.rs:237` | CEX decimal-4660 defect fixed; no live endpoint is claimed |
| Airdrop guide now blocks interaction and removes stale campaign claims | `Docs/guide/airdrop_guide.md:1-18`, `:20-37` | No live campaign/address/bridge/swap/finality claim remains; the guide publishes launch and scam-safety gates |
| Hardhat network authority is environment-injected | `SmartContracts/hardhat.config.js:1-15`, `:26-32` | No root `.env` load or hard-coded HF endpoint/key; required values are resolved only for the named network |
| Draft CEX payload fails closed as integration metadata | `SmartContracts/cex-listing-payload.json:3-6`, `:12-25`, `:28-44`, `:45-60` | Reward is unresolved, endpoints are null/empty, audit list is empty, deployment address is TBD, and the payload says not to submit |
| QUSD contract comment explicitly disclaims a peg | `SmartContracts/contracts/QUSD.sol:9-12` | Truthful inline documentation; OVERVIEW still calls it collateral-backed |
| Unsafe AMM and bridge paths contain explicit not-implemented errors, and maintained bridge status docs disclose containment | `QantoSwapFactory.sol:18-25`; `QantoSwapRouter.sol:23-37`; `qanto-zk-sdk/src/lib.rs:240-250`; `qantodag.rs:2938-2953`; `Docs/bridge-wqnto.md:3-7`, `:53-66`; `README.md:55-57`, `:100-114`; `OVERVIEW.md:311-312` | Correct containment in code, README, and maintained guide; OVERVIEW's earlier architecture/sequence still depicts usable functionality |
| WQNTO deployment is native ESM, artifact-based, nine-decimal, and chain-checked | `SmartContracts/deploy_wqnto.js:19-33`, `:52-70`, `:73-123`, `:124-152`; `Docs/bridge-wqnto.md:30-51` | Syntax and missing-RPC fail-closed checks passed; no canonical live deployment address is claimed |
| Solidity coverage is measured rather than unknown | `CONTRACTS_PERFORMANCE_OPERATIONS.md:161-192` | 38/38 passed; 42.45% line and 43.36% statement coverage is a reproducible baseline, not a quality pass |
| Automatic finality and dynamic sharding disablement is documented inline | `qantodag.rs:4414-4419` | Correct implementation comment; primary architecture docs describe both as active |
| Security reporting changed from a public pseudo-confidential issue to GitHub private vulnerability reporting | `SECURITY.md:11-18`, `:49-51` | Core disclosure hazard fixed; private-reporting enablement/fallback still needs operational verification |
| Security promises now use targets, disclaim a bounty, require independent audit, and separate MIT licensing from safe harbor | `SECURITY.md:20-40`, `:45-47`; `README.md:366-372` | README and SECURITY are reconciled; private-channel enablement/fallback ownership remains an operational gate |
| Contributor workflow now uses pinned-toolchain discovery, locked Cargo checks, canonical Nextest, SmartContracts tests, and exact ZK paths | `CONTRIBUTING.md:7-39`, `:52-77` | Major onboarding correction; environment/version matrix and clean-host evidence remain |
| Code of Conduct now names a private report path and fallback | `CODE_OF_CONDUCT.md:35-38` | Basic reporting is actionable; conflict-of-interest/appeal details remain |

Representative already-applied metadata diff:

~~~diff
-| **Chain ID** | `0x1234` (4660 decimal) |
+| **Chain ID** | `1234` (`0x4D2`) |

-| **Chain ID** | `21313` (Hex: `0x5341` - the SAGA neural signature) |
+| **Chain ID** | `1234` (hex: `0x4D2`) |
~~~

Representative already-applied disclosure-policy diff for SPEC-012:

~~~diff
-1. **Contact the Security Team**: Until a dedicated security email is established, submit reports via a GitHub issue marked as "confidential" in the Qanto repository.
+1. **Contact the Security Team Privately**: Submit a private vulnerability report. Do not open a public issue. If private vulnerability reporting is unavailable, request a private channel without including exploit details.

-- **Acknowledgment**: You will receive an initial confirmation within 48 hours.
+- **Acknowledgment target**: We aim to confirm receipt within 48 hours.

-No formal bug bounty program is active during Phase 1. We plan to introduce a rewards system with the testnet launch.
+No formal bug bounty or guaranteed monetary reward is active during Phase 1.

-Reporting vulnerabilities ... will be treated as violations of the MIT License.
+The repository license does not itself define a vulnerability-disclosure safe harbor.
~~~

No diff can honestly be supplied for the unresolved documentation findings because they remain open.

---

# 12.F Deferred items

| Priority | Item | Why deferred in this track | Estimated effort |
|---|---|---|---:|
| P0 | Rewrite OVERVIEW's active finality, full PQ security, live bridge, 10M+/31ms, neural SAGA, verified green mining, QUSD peg, vesting, and EVM-shard claims as disabled, experimental, or unverified | README and maintained integration guides are fixed, but OVERVIEW still contradicts them; corrections are safe documentation changes and should not wait for protocol redesign | 1-2 days plus product/legal approval |
| P0 | Publish one versioned consensus specification covering block commitment, identity, fork choice, state/reorg, finality, wire limits, fees/rewards, genesis, and activation rules | Existing documents are descriptive marketing architecture, not a normative protocol; content requires protocol-owner decisions and external review | 3-6 engineering weeks |
| P0 | Verify the private vulnerability channel | README and SECURITY are reconciled; maintainers must still enable/test GitHub private reporting, designate a verified fallback/duty owner, and add severity/escalation/safe-harbor terms | 0.5-2 days for channel/policy; ongoing staffing |
| P0 | Generate reward, supply, chain-ID, and endpoint metadata from one canonical source | Nine-decimal, chain-ID, README, and CEX surfaces are corrected. `token-info.json`, the operator guide, and RPC guide still publish 2.5 QNTO, and the latter two metadata surfaces retain HF endpoints; documentation cannot silently choose a new consensus policy | 1-3 days after policy decision |
| P0 | Remove or quarantine remaining fabricated/broken deployment instructions and scripts | `deploy_wqnto.js` is repaired; `scripts/deploy_dex.js`, `scripts/deploy_phase11.js`, and the unreproducible infrastructure launch path still need release-owner action | 2-4 days |
| P1 | Rewrite `OVERVIEW.md` links and module/test inventory using repository-relative paths generated from source | Straightforward, but should follow final naming/topology decisions to avoid another stale snapshot | 1-2 days |
| P1 | Finish CONTRIBUTING reproducibility | The major path/toolchain/test corrections are implemented; add Node/npm version policy, environment-variable matrix, frontend checks, and clean-machine validation | 0.5-1 day plus CI verification |
| P1 | Finish Code-of-Conduct governance | Private reporting is now named; add a dedicated recipient, conflict-of-interest alternative, response targets, and recusal/appeal rules | 0.5-1 day |
| P1 | Add documentation contract tests | Link checker, path inventory, JSON chain/reward/decimal consistency, Prometheus-query checks, CLI smoke tests, and claim-to-benchmark artifact validation should run in CI | 2-5 days |
| P1 | Reissue performance and security claims only from immutable evidence artifacts | Requires fixed protocol paths, reproducible multi-node benchmarks, cryptographic KAT/interoperability, analyzers, and independent audit; copy editing cannot manufacture evidence | 4-12+ weeks across other workstreams |

---

# 12.G Release-document conditions

Before any public testnet announcement, the documentation set must satisfy all of these conditions:

1. Clearly label the network as an experimental, non-value-bearing prototype until consensus, authorization, state, bridge, economics, and deployment blockers are closed.
2. State that automatic finality, bridge claims, dynamic sharding, QantoSwap, and QUSD stablecoin functionality are unavailable.
3. Describe ML-DSA as a cryptographic primitive integration, not end-to-end PQ authorization, until sender/validator identity binding and KEM transport are implemented and reviewed.
4. Publish the actual reward/genesis/supply behavior in atomic units and remove the 2.5-QNTO schedule unless it becomes an explicitly approved versioned protocol rule.
5. Remove unsupported performance numbers or attach immutable raw multi-node benchmark artifacts, environment, commit, commands, and acceptance methodology.
6. Publish one canonical chain ID, endpoint topology, deploy procedure, and exchange metadata set.
7. Provide an actionable private security-reporting channel before exposing public RPC/P2P services.
8. Make every repository link and command pass automated clean-checkout validation.

# Final verdict

## **NOT READY**

Workstream 12 remains a launch blocker, but the defect is now concentrated rather than repository-wide. README, SECURITY, CONTRIBUTING, CODE_OF_CONDUCT, the maintained WQNTO/airdrop guides, Hardhat/deployment configuration, and the draft CEX payload are materially reconciled. `OVERVIEW.md` still promises security, economic, consensus, bridge, performance, sharding, green-mining, and contract properties that are absent or disabled, while legacy token/operator/RPC documents still conflict on reward and endpoints. Until those surfaces are made consistent and a versioned normative protocol specification exists, the documentation set is not safe for validator, wallet, bridge, or exchange integration.
