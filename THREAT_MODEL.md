# QANTO THREAT MODEL

## Overview
This document models threats against the Qanto protocol based on the architecture discovery phase and attack test reproduction. Updated with findings from 12 attack test modules.

## Threat Actors

### A. Malicious Unauthenticated Internet Client
- **Objective**: Disrupt service, steal information, or gain unauthorized access
- **Attack Path**: RPC endpoints, P2P ports, web interfaces
- **Required Capability**: Network access, basic protocol understanding
- **Expected Payoff**: Service disruption, potential data leakage
- **Detection**: Rate limiting, anomaly detection in request patterns
- **Recovery**: IP blocking, service restart, patching vulnerabilities
- **Attack Tests**: attack_6_rpc_ws_unauthenticated

### B. Malicious P2P Peer
- **Objective**: Eclipse attacks, chain reorganization, consensus disruption
- **Attack Path**: P2P gossip protocol, block/transaction propagation
- **Required Capability**: Ability to run a node, network manipulation
- **Expected Payoff**: Double-spending, censorship, network split
- **Detection**: Peer reputation systems, unexpected message patterns
- **Recovery**: Peer banning, reconnecting to honest nodes, checkpointing
- **Attack Tests**: attack_7_p2p_eclipse_fixed_topics

### C. Byzantine Validator
- **Objective**: Finalize invalid blocks, steal rewards, disrupt consensus
- **Attack Path**: Consensus messages, block proposals, voting
- **Required Capability**: Control of validator keys, stake/resources
- **Expected Payoff**: Illegitimate rewards, chain control, reputation damage
- **Detection**: Invalid block detection, voting pattern analysis
- **Recovery**: Slashing conditions (NOT IMPLEMENTED), validator removal, community governance
- **Attack Tests**: attack_1 (double-spend), attack_3 (reorg), attack_8 (griefing), attack_9 (lock bottleneck)

### D. Economically Rational Spammer
- **Objective**: Fill mempool/blocks with low-fee spam transactions
- **Attack Path**: Transaction submission via RPC or P2P
- **Required Capability**: Ability to create valid transactions, minimal funds
- **Expected Payoff**: Network congestion, increased fees for others
- **Detection**: Mempool monitoring, fee market analysis
- **Recovery**: Dynamic minimum fees, mempool size limits, priority calculation
- **Attack Tests**: attack_10 (blocks_map performance), attack_6

### E. Cross-Chain Relayer Attacker
- **Objective**: Forge or replay cross-chain messages to steal funds
- **Attack Path**: Interoperability module, bridge contracts, relayer node
- **Required Capability**: Understanding of bridge protocols, key material
- **Expected Payoff**: Unauthorized token minting, theft of locked assets
- **Detection**: Proof verification failures, unusual bridge activity
- **Recovery**: Upgrading verifiers, pausing bridges, fraud proofs
- **Attack Tests**: attack_4 (EVM bypass), attack_5 (bridge forged signature)

### F. Quantum-Capable Future Attacker
- **Objective**: Break signature schemes, steal funds, impersonate users
- **Attack Path**: Signature verification, transaction validation
- **Required Capability**: Quantum computer capable of running Shor's/Grover's algorithms
- **Expected Payoff**: Complete compromise of classical signatures
- **Detection**: Not applicable until quantum advantage is demonstrated
- **Recovery**: Migration to post-quantum cryptography, key rotation
- **Attack Tests**: attack_4 (bypasses PQ), attack_12 (omega threat)

### G. Hashrate Attacker (51% Attacker)
- **Objective**: Deep chain reorganization, censorship, double-spend
- **Attack Path**: Mining with >50% network hashrate
- **Required Capability**: >50% of network QanHash power
- **Expected Payoff**: History rewrite, transaction censorship, double-spend
- **Detection**: Hashrate monitoring, reorg depth alerts
- **Recovery**: Checkpointing (NOT IMPLEMENTED), social consensus
- **Attack Tests**: attack_1 (single-parent DAG), attack_2 (liveness)

## Attack Surface Analysis (mapped to 12 Attack Tests)

| # | Attack Test | Component | Vulnerability | Severity | STRIDE |
|---|-------------|-----------|---------------|----------|--------|
| 1 | attack_1_single_parent_dag_safety_violation | QantoDAG | Single-parent DAG enables 51% double-spend; no multi-parent consensus | CRITICAL | Spoofing, Tampering |
| 2 | attack_2_liveness_failure_fixed_difficulty | Miner/Consensus | Fixed difficulty + miner backoff = liveness failure under hashrate drop | HIGH | Denial of Service |
| 3 | attack_3_finality_absence_reorg_any_depth | QantoDAG | No finality gadget = reorg any depth; finalize_blocks() no-op | CRITICAL | Tampering |
| 4 | attack_4_evm_raw_tx_bypass_pq | Transaction/EVM | EVM raw transactions bypass PQ signature verification | HIGH | Spoofing, Elevation |
| 5 | attack_5_bridge_claim_forged_signature | Bridge/IBC | Bridge claims use mock verification; forgeable signatures | CRITICAL | Spoofing, Tampering |
| 6 | attack_6_rpc_ws_unauthenticated | RPC/WS | No auth on RPC/WebSocket endpoints | HIGH | Spoofing, Info Disclosure |
| 7 | attack_7_p2p_eclipse_fixed_topics | P2P/Gossipsub | Fixed topics enable eclipse attacks | HIGH | Denial of Service, Info Disclosure |
| 8 | attack_8_miner_reservation_griefing | Miner | Reservation griefing via difficulty manipulation | MEDIUM | Denial of Service |
| 9 | attack_9_consensus_lock_bottleneck | Consensus | Single mutex bottleneck in validation | MEDIUM | Denial of Service |
| 10 | attack_10_blocks_map_performance | Storage | Unbounded HashMap growth in blocks_map | MEDIUM | Denial of Service |
| 11 | attack_11_saga_f64_nondeterminism | SAGA AI | f64 neural network = nondeterministic governance | HIGH | Tampering, Repudiation |
| 12 | attack_12_omega_threat_manipulation | ΩMEGA | Threat model manipulation via adversarial inputs | MEDIUM | Tampering, Elevation |

## Specific Vulnerability Concerns (Validated by Attack Tests)

### Consensus Safety - CRITICAL
- **Single-parent DAG** (qantodag.rs:3245-3248): Enforces exactly one parent in "containment mode"
- **No multi-parent support**: Branch-scoped state disabled, no GHOST multi-tip support
- **No checkpointing/slashing**: Validators can equivocate without penalty
- **Result**: 51% attacker can produce secret chain and double-spend (attack_1 reproduces this)

### Consensus Liveness - HIGH
- **Fixed difficulty**: `INITIAL_DIFFICULTY = QANTO_SCALE (1e9)` hardcoded everywhere
- **Miner backoff conflict**: Miner divides difficulty by 2 every 3 attempts locally
- **Consensus rejects**: Blocks with difficulty != INITIAL_DIFFICULTY
- **Result**: Under hashrate drop, miner backs off but consensus rejects = permanent stall (attack_2)

### Finality - CRITICAL
- **finalize_blocks()**: Returns error "Finality is disabled until signed validator votes and canonical certificates are implemented"
- **No slashing**: Equivocation not punishable
- **No checkpointing**: No irreversible blocks at any depth
- **Result**: Reorg at ANY depth possible (attack_3 demonstrates 100-block reorg)

### Cryptography - HIGH
- **EVM raw tx bypass**: `TransactionKind::EVMCall` skips PQ signature verification
- **Bridge/IBC mock**: `BRIDGE_PROOF_VERIFIER_AVAILABLE = false`, no real proof verification
- **PQC key rotation**: Manual ML-DSA derivation, no automated policy

### Networking - HIGH
- **Fixed gossipsub topics**: Hardcoded, no topic rotation, enables eclipse
- **No peer scoring**: Basic Kademlia only, no reputation-based peer selection
- **RPC/WS unauthenticated**: No TLS client certs, no API keys, no rate limiting per identity

### Governance/AI - HIGH
- **SAGA f64 nondeterminism**: Neural network uses f64 weights, non-deterministic outputs
- **Not consensus-safe**: Stochastic elements in governance decisions
- **Production hardening removed AI**: But neural network structure remains

### Performance/DOS - MEDIUM
- **blocks_map unbounded**: `DashMap<String, QantoBlock>` grows forever
- **Consensus lock bottleneck**: Single `Mutex<()>` serializes all block validation
- **Miner reservation griefing**: Can reserve mining slots and not use them

## Trust Assumptions (EXPLICIT & UNVALIDATED)

### What Qanto Trusts (Must Be Verified or Mitigated)

1. **Majority of Validators Are Honest**: Assumes <33% Byzantine validators for safety
   - **Status**: NO slashing, NO checkpointing, NO-finality → CANNOT enforce
   - **Risk**: 51% attack practical

2. **P2P Network Eventually Connects Honest Peers**: Relies on gossip for dissemination
   - **Status**: Fixed topics, no eclipse resistance → VIOLATED by attack_7
   - **Risk**: Partition attacks practical

3. **Cryptographic Primitives Are Secure**: Dilithium3, SPHINCS+, Ed25519, P256
   - **Status**: ML-DSA-65 implemented, but EVM raw tx bypasses it
   - **Risk**: Partial bypass (attack_4)

4. **Timestamp Accuracy**: Limited trust in local clocks for consensus timing
   - **Status**: `TEMPORAL_CONSENSUS_WINDOW = 600s`, no VDF or timestamp oracle
   - **Risk**: Time-warp attacks possible

5. **Mempool Fee Market**: Assumes fee market prevents spam effectively
   - **Status**: Fixed difficulty breaks fee market dynamics, `MIN_TX_FEE = 100`
   - **Risk**: Spam practical (attack_10)

6. **SAGA Governance Is Deterministic**: Neural network decisions are reproducible
   - **Status**: f64 weights = NON-deterministic (attack_11)
   - **Risk**: Forks from governance divergence

7. **Bridge Proofs Are Verified**: Cross-chain messages cryptographically verified
   - **Status**: Mock verification only, `BRIDGE_PROOF_VERIFIER_AVAILABLE = false`
   - **Risk**: Asset theft via forged claims (attack_5)

8. **Difficulty Adjustment Works**: Network self-regulates block time
   - **Status**: NO adjustment - `get_required_difficulty()` returns constant
   - **Risk**: Liveness failure under hashrate change (attack_2)

## CVSS 4.0 Scoring (Based on Attack Test Evidence)

| Vulnerability | CVSS 4.0 | Vector |
|---------------|----------|--------|
| Single-parent DAG double-spend | 9.8 CRITICAL | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N |
| No finality / reorg any depth | 9.8 CRITICAL | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N |
| Fixed difficulty liveness failure | 8.6 HIGH | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:N/VI:H/VA:H/SC:N/SI:N/SA:N |
| Bridge claim forge | 9.8 CRITICAL | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N |
| EVM raw tx PQ bypass | 8.8 HIGH | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:N/SC:N/SI:N/SA:N |
| P2P eclipse fixed topics | 8.2 HIGH | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:N/VI:H/VA:H/SC:N/SI:N/SA:N |
| RPC/WS unauthenticated | 7.5 HIGH | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:N/VA:N/SC:N/SI:N/SA:N |
| SAGA f64 nondeterminism | 7.5 HIGH | CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:N/VI:H/VA:N/SC:N/SI:N/SA:N |

## MITRE ATT&CK Mapping

| Technique | Attack Test | Mitigation |
|-----------|-------------|------------|
| T1499.001 (DoS: Network Flood) | attack_6, attack_10 | Rate limiting, resource quotas |
| T1556.002 (Credential Access: Password Spray) | attack_6 (unauth RPC) | mTLS, API keys |
| T1491.001 (Defacement: Blockchain Reorg) | attack_1, attack_3 | Finality gadget, checkpointing |
| T1595 (Active Scanning) | attack_7 (eclipse) | Peer reputation, topic rotation |
| T1565.001 (Data Manipulation: Stored Data) | attack_4, attack_5 | PQ signatures, real bridge proofs |
| T1485 (Data Destruction) | attack_9 (lock bottleneck) | Lock-free data structures |
| T1657 (Impersonation) | attack_4, attack_5 | PQ signature enforcement |

## Recommended Mitigations Priority Matrix

### P0 - Must Fix Before Testnet (Blockers)
1. **Finality gadget** - GHOST/Photon certificate-based finality with validator votes
2. **Slashing for equivocation** - Economic deterrent for Byzantine validators
3. **Adaptive difficulty retarget** - Per-block or per-epoch adjustment with bounds
4. **Bridge proof verification** - Real cryptographic proofs, not mock
5. **EVM tx PQ enforcement** - All tx kinds require PQ signatures

### P1 - Must Fix Before Mainnet
6. **Multi-parent DAG support** - Enable branch-scoped state, GHOST multi-tip
7. **SAGA determinism** - Replace f64 neural net with deterministic decision tree
8. **P2P eclipse resistance** - Peer scoring, topic rotation, diversified connections
9. **RPC/WS authentication** - mTLS, rate limiting per identity
10. **PQC key rotation policy** - Automated rotation, compromise recovery

### P2 - Should Fix
11. **Consensus lock optimization** - Lock-free or sharded validation
12. **blocks_map bounds** - Pruning, archival nodes, state expiry
13. **Miner reservation fairness** - Commit-reveal or VRF-based leader election
14. **Timestamp oracle** - VDF or trusted timestamp service

### P3 - Nice to Have
15. **Formal verification** - Safety/liveness proofs for finality gadget
16. **Chaos testing** - Automated partition, churn, byzantine scenarios
17. **Performance regression CI** - blocks_map, mempool, consensus benchmarks

## Security Testing Gap Analysis

| Test Type | Current | Required | Gap |
|-----------|---------|----------|-----|
| Unit tests | 322 passing | Full coverage | Attack tests cover critical paths only |
| Property tests | None | UTXO conservation, supply cap, no double-spend | GAP |
| Fuzzing | 100 corpus files (tx parsing) | 25M+ executions | GAP |
| Integration tests | None | 50-node network, 30min 1000 TPS | GAP |
| Chaos tests | None | Network partitions, validator churn | GAP |
| Formal verification | None | Safety/liveness proofs | GAP |

## Conclusion

**Verdict: NOT SAFE FOR TESTNET**

The protocol has 4 CRITICAL and 4 HIGH severity vulnerabilities that enable:
- Practical 51% double-spend (single-parent DAG)
- Unbounded chain reorganization (no finality)
- Permanent liveness failure (fixed difficulty + miner backoff)
- Bridge asset theft (mock verification)
- PQ signature bypass (EVM raw tx)

Minimum viable testnet requires P0 fixes. Mainnet requires all P0+P1.