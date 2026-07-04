# QANTO Protocol — Threat Model

**Version**: 1.0  
**Date**: June 2026  
**Scope**: Protocol Core (`qanto-node`, `qanto-core`), Bridge, Governance, Cryptographic primitives  
**Audience**: External security auditors, protocol reviewers  

---

## 1. System Architecture Overview

QANTO is a DAG-based L1 blockchain with the following core subsystems:

| Subsystem | Module | Key Files |
|-----------|--------|-----------|
| Consensus | Hybrid PoW/PoS with SAGA scoring | `qantodag.rs`, `consensus.rs`, `saga.rs` |
| Cryptography | EVM-compatible secp256k1 + Post-Quantum (Dilithium/Falcon) | `qanto_native_crypto.rs`, `post_quantum_crypto.rs`, `transaction.rs` |
| Bridge | Cross-chain asset transfer (Ethereum ↔ QANTO) | `interoperability.rs`, `qantodag.rs` (bridge claims) |
| Tokenomics | Fixed 21B supply, emission schedule with halving | `emission.rs`, `lib.rs` (constants) |
| Storage | Custom LSM-tree with WAL, segment files | `qanto_storage` crate, `persistence.rs` |
| Networking | libp2p-based P2P with gossip | `p2p.rs`, `node.rs` |
| Governance | Stake-weighted voting, delegation, slashing | `qantodag.rs` (governance handlers) |
| API Surface | REST, GraphQL, WebSocket, IPC, JSON-RPC | `node.rs`, `graphql_server.rs`, `websocket_server.rs`, `ipc_server.rs` |

### Data Flow

```
External TX (EVM/PQ) → Mempool → Block Producer → DAG → Finalization → Persistence
                                                    ↑
Bridge Claim (ETH) → Relayer Signatures → Quorum Check → UTXO Mint
```

---

## 2. Trust Boundaries

### Boundary 1: Network ↔ Node
- **P2P Layer**: All inbound blocks and transactions from peers are untrusted.
- **API Layer**: All HTTP/WS/IPC endpoints are publicly exposed and accept arbitrary input.

### Boundary 2: Mempool ↔ Consensus
- Transactions in the mempool have been syntactically validated but are not yet state-validated.
- Block producers select transactions from the mempool and must re-validate during block construction.

### Boundary 3: Bridge ↔ Native Chain
- Bridge claims originate from external chains (Ethereum). The relayer signature quorum is the sole trust anchor.
- The bridge is collateral-capped: claims exceeding `total_bridge_locked` are rejected.

### Boundary 4: Validator ↔ Validator
- Each validator is assumed to be independently adversarial.
- Consensus safety relies on a 2/3 (67%) stake-weighted quorum for finality.

---

## 3. Cryptographic Primitives

### 3.1 EVM Transaction Signatures (secp256k1)

| Property | Implementation |
|----------|---------------|
| Curve | secp256k1 via `k256` crate |
| Signature format | ECDSA with Recovery ID (EIP-155 / EIP-1559) |
| Replay protection | `chain_id` embedded in RLP signing payload |
| Malleability | **High-S rejection** enforced — signatures with `s > n/2` are rejected |
| Key recovery | `VerifyingKey::recover_from_prehash()` |

**Attack surface**: RLP parsing of raw transaction bytes. Fuzz targets exist at `fuzz/fuzz_targets/fuzz_tx_parsing.rs`.

### 3.2 Post-Quantum Signatures (Block Signing)

| Property | Implementation |
|----------|---------------|
| Algorithm | CRYSTALS-Dilithium (ML-DSA) via `pqcrypto-dilithium` |
| Usage | Block header signing, coinbase transaction signing |
| Key format | `QantoPQPublicKey` / `QantoPQPrivateKey` |

**Attack surface**: PQ key generation and signing correctness. Key sizes are verified on deserialization.

### 3.3 Hash Functions

| Hash | Usage |
|------|-------|
| `qanto_hash` (custom) | Block IDs, Merkle roots, content addressing |
| Keccak-256 | EVM sender recovery, bridge message hashing |
| SHA3 family | Various internal digests |

---

## 4. Threat Categories & Mitigations

### 4.1 Consensus Attacks

| Threat | Description | Mitigation | Tested |
|--------|-------------|------------|--------|
| **51% Attack** | Attacker with majority stake attempts to finalize blocks | 67% quorum requirement (not 50%) | `test_stake_concentration` |
| **Equivocation (Double-Signing)** | Validator signs two conflicting blocks at same height | Equivocation detection in `add_block`; 30% stake slashing | `test_byzantine_double_signing_slashing` |
| **Forged Coinbase** | Validator inflates block reward beyond emission schedule | Reward mismatch check against `calculate_block_reward()` | `test_byzantine_forged_coinbase_reward` |
| **Invalid Merkle Root** | Validator mutates Merkle root to hide tx manipulation | Recomputed and compared in `add_block` | `test_byzantine_invalid_merkle_root` |
| **Long-Range Attack** | Attacker rewrites historical blocks with old keys | Finalized blocks are immutable; checkpoint system | Partial |
| **Nothing-at-Stake** | Validators sign on multiple forks without penalty | Equivocation detection + slashing | `test_byzantine_double_signing_slashing` |

### 4.2 Network Partition Attacks

| Threat | Description | Mitigation | Tested |
|--------|-------------|------------|--------|
| **Partition with < 67% each side** | Network split prevents finality on both sides | Neither side can finalize (safety preserved) | `test_partition_scenario_a_60_40`, `test_partition_scenario_c_34_33_33` |
| **Partition with ≥ 67% one side** | Majority partition continues; minority halts | Correct behavior: majority finalizes, minority waits | `test_partition_scenario_b_70_30` |
| **Partition healing** | Divergent chains must converge on reconnection | Blocks gossiped and accepted; finality extended | `test_partition_healing_convergence` |

### 4.3 Bridge Attacks

| Threat | Description | Mitigation | Tested |
|--------|-------------|------------|--------|
| **Bridge Drain** | Attacker submits claims exceeding collateral | Claim amount checked against `total_bridge_locked` | `test_bridge_drain_attempts` |
| **Forged Relayer Signatures** | Attacker submits claims with invalid relayer sigs | ECDSA recovery + validator set membership check | Yes (integrated) |
| **Double-Claim** | Same source TX hash claimed twice | `processed_bridge_claims` DashSet; persisted across restarts | Yes (Sprint 7) |
| **Quorum Bypass** | Insufficient relayer signatures accepted | Strict stake-weighted quorum enforcement (testnet bypass removed) | Yes (Sprint 7) |

### 4.4 Economic Attacks

| Threat | Description | Mitigation | Tested |
|--------|-------------|------------|--------|
| **Stake Concentration** | Single entity accumulates >67% stake | Governance monitoring; 67% threshold is mathematically sound | `test_stake_concentration` |
| **Delegation Cartel** | Delegators concentrate power to one validator | Delegation properly debits delegator balance; effective stake tracked | `test_delegation_cartels` |
| **Reward Farming** | Stake-then-immediately-unstake to farm rewards | Unstake cooldown enforced (10 epoch minimum) | `test_reward_farming` |
| **Inflation Bug** | Emission schedule produces tokens beyond 21B cap | `Emission::update_supply` rejects if `current_supply + amount > TOTAL_SUPPLY` | Compile-time assertion + runtime check |

### 4.5 Storage & Persistence Attacks

| Threat | Description | Mitigation | Tested |
|--------|-------------|------------|--------|
| **WAL Corruption** | Appended garbage or truncated WAL on crash | WAL replay discards corrupted tail; intact records recovered | `test_wal_tail_corruption_recovery`, `test_wal_mid_record_corruption_recovery` |
| **Segment Corruption** | Tampered `.qdb` segment files | Magic number validation; corrupted segments skipped | `test_segment_file_corruption_detection` |
| **Crash Consistency** | Abrupt termination without shutdown | WAL replay recovers unflushed memtable entries | `test_crash_consistency` |

### 4.6 API / Input Validation Attacks

| Threat | Description | Mitigation |
|--------|-------------|------------|
| **RLP Parsing Exploits** | Malformed EVM transactions crash the node | Length checks, item count validation, fuzz-tested |
| **Oversized Transactions** | Giant payloads exhaust memory | Transaction size limits in mempool admission |
| **Rate Limiting Bypass** | Flood API endpoints | Governor-based rate limiter on all REST endpoints |
| **JSON-RPC Injection** | Malicious method calls | Method whitelist; parameter validation |

---

## 5. Invariants

The following invariants must hold at all times:

1. **Supply Invariant**: `emission.current_supply + all_unmined_rewards == TOTAL_SUPPLY` (21B QNTO)
2. **Finality Invariant**: A finalized block is never reverted or removed from `finalized_blocks`
3. **Quorum Invariant**: No block is finalized unless validators representing ≥ 67% stake have built on its chain
4. **UTXO Conservation**: For every non-coinbase transaction, `sum(inputs) == sum(outputs) + fee`
5. **Equivocation Detection**: If a validator signs two different blocks at the same height on the same chain, the second is rejected and the validator is slashed
6. **Bridge Solvency**: `sum(all_outstanding_claims) <= total_bridge_locked`
7. **Delegation Balance**: `delegator.balance` is debited when delegating; effective stake is self_stake + delegated_stake
8. **Cooldown Enforcement**: Unstaking requires `current_epoch >= stake_epoch + COOLDOWN_EPOCHS`

---

## 6. Known Residual Risks

### 6.1 Upstream Dependency Risks
- **`k256` / `generic-array`**: Deprecation warnings on `as_slice()`. Functionally safe but should track upgrades.
- **`pqcrypto-dilithium`**: NIST standardization is finalized (ML-DSA); library tracks the standard but binding update cadence should be monitored.

### 6.2 Not Yet Addressed
- **MEV / Front-Running**: Transaction ordering within blocks is currently FIFO. No commit-reveal or encrypted mempool.
- **Sybil Resistance at P2P Layer**: Peer connections are not stake-gated. A Sybil attacker could eclipse a node with many low-cost peers.
- **Time Manipulation**: Block timestamps are self-reported by validators. Median timestamp checks are not enforced beyond basic ordering.
- **Smart Contract Vulnerabilities**: If/when EVM execution is enabled, contract-level vulnerabilities become in-scope.

### 6.3 Operational Risks
- **Single-Binary Architecture**: All subsystems run in one process. A panic in one module can crash the entire node.
- **No Formal Verification**: Consensus and finality logic are tested but not formally verified (e.g., TLA+, Coq).

---

## 7. Security Testing Coverage

| Test Suite | File | Tests | Coverage Area |
|-----------|------|-------|---------------|
| Fuzz Testing | `fuzz/fuzz_targets/` | 2 targets | TX parsing, bridge proofs |
| Byzantine Simulation | `byzantine_tests.rs` | 4 tests | Double-signing, forged rewards, invalid Merkle, bad coinbase sig |
| Partition Simulation | `partition_tests.rs` | 4 tests | 60/40, 70/30, 34/33/33 partitions + healing |
| Economic Attack | `economic_attack_tests.rs` | 4 tests | Stake concentration, delegation cartels, reward farming, bridge drain |
| Corruption Recovery | `corruption_tests.rs` | 4 tests | WAL tail, WAL mid-record, segment corruption, crash consistency |
| Security Core | `security_tests.rs` | Multiple | EVM signature recovery, high-S rejection, chain_id replay |
| Soak Testing | `Scripts/soak_test.sh` | 1 harness | Multi-hour resource monitoring under load |

**Total adversarial test coverage**: 16+ dedicated tests across 4 test files + fuzz targets + soak harness.

---

## 8. Recommendations for External Auditors

1. **Start with `qantodag.rs`** (~5300 lines). This is the monolithic state machine containing block validation, finalization, bridge claim processing, governance, and reward calculation. Most critical paths flow through `add_block()`.

2. **Focus areas**:
   - `add_block()` → block validation, equivocation detection, reward calculation
   - `process_bridge_claim()` → relayer signature recovery, quorum check, collateral cap
   - `finalize_blocks()` → stake-weighted quorum calculation, finality determination
   - `process_delegate()` / `process_unstake()` → balance accounting, cooldown enforcement

3. **Verify the 67% quorum math**: Ensure the finality calculation correctly handles rounding, especially with non-uniform stake distributions.

4. **Check emission arithmetic**: All calculations use `u128` fixed-point with `SCALE = 1e9`. Verify no overflow or precision loss in reward calculation and halving logic.

5. **Bridge trust model**: The bridge relies entirely on relayer signature quorum. Verify that the recovered Ethereum addresses correctly map to the validator set and that stake weighting is applied accurately.
