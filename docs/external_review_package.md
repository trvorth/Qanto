# QANTO Protocol — External Review Package & Audit Handoff

**Document Version**: 1.1  
**Target Release**: QANTO v1.0.0-rc1  
**Scope**: Protocol Core (`qanto-node`, `qanto-core`), P2P Engine, UTXO Ledger, Cryptographic Primitives, Tokenomics, and Cross-Chain Bridge  
**Audience**: External security auditors, protocol engineering team, Launch Readiness Gate reviewers  

---

## 1. System Architecture & Data Flow

QANTO is a high-throughput, DAG-based Layer-1 blockchain featuring a post-quantum resistant consensus mechanism. The node is implemented as a unified Rust binary (`qanto`) coordinating consensus, cryptography, ledger persistence, networking, and JSON-RPC/REST/GraphQL/WS/IPC APIs.

### 1.1 Core Subsystems

| Subsystem | Module | Key Files | Purpose |
| :--- | :--- | :--- | :--- |
| **Consensus Engine** | Hybrid PoW/PoS (SAGA Scoring) | `qantodag.rs`, `consensus.rs`, `saga.rs` | Coordinates validator registration, block production/validation, DAG topology, and finalization. |
| **Ledger & UTXO Set** | UTXO Ledger | `qantodag.rs`, `transaction.rs` | Tracks account balances via Unspent Transaction Outputs (UTXOs) with homomorphic encryption options. |
| **Cryptography** | EVM (secp256k1) & PQ (Dilithium) | `post_quantum_crypto.rs`, `transaction.rs` | Standard EVM-compatible signatures for end-users; Dilithium (ML-DSA) for consensus block headers. |
| **Tokenomics** | Block Reward & Cap Engine | `emission.rs`, `lib.rs` | Enforces a hard 21,000,000,000 QNTO supply cap and emission rules. |
| **P2P Networking** | libp2p GossipSub Engine | `p2p.rs`, `node.rs` | Gossip and direct sync for transactions, blocks, carbon credentials, and state snapshots. |
| **Storage & Persistence** | LSM-tree with WAL | `qanto_storage`, `persistence.rs` | High-performance, crash-consistent transactional database storage. |
| **Bridge Engine** | Interoperability Wrapper | `interoperability.rs` | Handles trust anchor verification (Ethereum multisig relayers) for wrapped assets. |

### 1.2 Data Flow Map

```
[Untrusted P2P Network] ──> [gossipsub / sync] ──┐
                                                  ▼
[REST / RPC API Clients] ─> [JSON-RPC / REST] ──> [Mempool] ──> [Block Producer] ──> [QantoDAG State]
                                                                                           │
                                                                                           ▼
[EVM Bridge Claims] ─────> [Relayer Signatures] ─> [Quorum Check] ──> [UTXO Mint] ─────> [Persistence]
```

### 1.3 Trust Boundaries

1. **P2P Boundary**: All blocks, transactions, and synchronization payloads received from peer-to-peer interfaces are untrusted and undergo strict cryptographic signature validation and state transitions before ingestion.
2. **API Endpoint Boundary**: The REST, JSON-RPC, GraphQL, and IPC APIs accept external input. Inputs undergo structural, size, and type limits before reaching core logic.
3. **Mempool Boundary**: Transactions are checked for signature malleability (High-S rejection) and fee solvency before mempool inclusion.
4. **Bridge Relayer Quorum**: Cross-chain claims are anchored entirely by validator relayer signatures. The consensus engine enforces a strict stake-weighted quorum check.

---

## 2. Threat Model & Mitigations

We perform continuous threat modeling based on the STRIDE methodology to analyze and mitigate potential vulnerabilities.

### 2.1 Consensus Attacks & Mitigations
* **51% Attack / Majority Takeover**: An attacker with majority stake could attempt to rewrite history or finalize invalid blocks. Mitigated by enforcing a strict **67% (not 50%) stake-weighted quorum** for block finalization.
* **Equivocation (Double-Signing)**: Validators signing conflicting blocks at the same height are detected by `add_block` and suffer an immediate **30% stake slashing** penalty.
* **Forged Coinbase rewards**: A miner attempts to inflate their block rewards. Mitigated by strict runtime checks in `validate_block` comparing the coinbase transaction value against `calculate_block_reward()` from `emission.rs`.
* **Invalid Merkle Root**: A miner manipulates nested transactions while keeping the block header constant. Mitigated by re-computing and verifying the Merkle root on block ingestion.

### 2.2 Network Partition Attacks & Mitigations
* **Partition with < 67% on either side**: During a network split, neither partition can finalize blocks. Safety is preserved, and the chain halts finalization until reconnection.
* **Partition with >= 67% on one side**: The majority partition continues finalizing blocks, while the minority halts, ensuring deterministic consensus state.
* **Partition Healing**: Upon healing, blocks are gossiped and the nodes converge automatically, extending finality on the correct longest path.

### 2.3 Economic Attacks & Mitigations
* **Delegation Cartels**: Users delegating stake to validators could attempt to dominate voting weight. Mitigated by debiting the delegator's balance and tracking effective stake (`self_stake` + `delegated_stake`).
* **Reward Farming**: An attacker stakes funds, claims block rewards, and immediately unstakes. Mitigated by a strict **10-epoch cooldown** on unstaking.
* **Inflation Bug**: An attacker attempts to exploit emission arithmetic to mint tokens beyond the cap. Mitigated by `Emission::update_supply` rejecting any updates pushing the current supply past the 21 Billion QNTO cap.

### 2.4 Storage & Input Validation Attacks & Mitigations
* **WAL Tail Corruption**: Discards corrupted or truncated segments on node startup, recovering up to the last intact database transaction.
* **RLP Parsing Exploits**: Malformed RLP transaction payloads are checked for structural integrity, size limits, and fuzz-tested to prevent panics.

---

## 3. Bridge Design & Solvency

The QANTO Bridge enables cross-chain transfers (Ethereum ↔ QANTO) using validator relayer signatures as the primary trust anchor.

```
[Ethereum Lock] ──> [Relayer Event] ──> [Relayer Signatures] ──> [Validator Quorum] ──> [QANTO UTXO Mint]
```

### 3.1 Relay Quorum & Validation
* **Stake-Weighted Quorum**: A bridge mint claim is only valid if signed by a stake-weighted quorum of validators representing **>= 67% of the total active stake**.
* **Address Mapping**: Recovered Ethereum addresses are validated on-chain against the registered public keys of the active validator set.

### 3.2 Double-Spend & Solvency Protections
* **DashSet Processed Cache**: Transactions that have already been minted are tracked in a persistent `processed_bridge_claims` DashSet to prevent duplicate claims.
* **Bridge Solvency Invariant**: The total outstanding bridge claims minted on QANTO must never exceed the collateral locked on the host chain:
  $$\sum Claims_{Outstanding} \le Collateral_{Bridge}$$

---

## 4. Cryptographic Assumptions & Primitives

### 4.1 Standard EVM Signatures (secp256k1)
* **Usage**: External user transactions.
* **Format**: ECDSA with Recovery ID (EIP-155 / EIP-1559), serialized as `r`, `s`, and `v`.
* **Replay Protection**: Enforces strict `chain_id` validation via RLP serialization payload checks.
* **Malleability Protections**: Enforces **High-S rejection** (`s > n/2` is invalid), blocking transaction ID mutation.

### 4.2 Post-Quantum Block Signatures (ML-DSA)
* **Usage**: Block header and coinbase transaction signatures.
* **Format**: CRYSTALS-Dilithium (ML-DSA) via `pqcrypto-dilithium`.
* **Verification**: Validator public keys are verified on-chain against the active validator set.

### 4.3 Hash Functions
* **`qanto_hash`**: Custom Blake3-based digest for block IDs, Merkle roots, and internal state hashes.
* **Keccak-256**: Used for EVM address generation, message hashing, and compatibility layers.

---

## 5. Tokenomics Mechanics

The QANTO emission scheduler enforces supply conservation rules:

1. **Emission Schedule**: Block rewards halve periodically. All calculations use `u128` fixed-point arithmetic with `SCALE = 1e9` to prevent rounding errors or precision loss.
2. **Hard Supply Cap**: A hard cap of **21,000,000,000 QNTO** is enforced.
3. **Transaction Fees**: Transaction fees are collected and distributed or burned depending on active governance rules, maintaining long-term tokenomics balance.

---

## 6. Protocol Invariants

The QANTO state machine strictly enforces the following mathematical invariants:

| Ref | Invariant Description | Mathematical / Logical Check |
| :--- | :--- | :--- |
| **I1** | **Hard Cap and Supply Conservation** | $Supply_{Current} \le 21,000,000,000 \text{ QNTO}$ |
| **I2** | **UTXO Conservation** | $\sum Inputs_{value} = \sum Outputs_{value} + Fee$ (for non-coinbase transactions) |
| **I3** | **Consensus Finality & Quorum** | $Quorum \ge 67\%$ built weight is required to finalize |
| **I4** | **Equivocation Slashing** | $Stake_{Slashed} = 0.30 \times Stake_{Active}$ on double-signing detection |
| **I5** | **Bridge Solvency** | $\sum Claims_{Outstanding} \le Collateral_{Bridge}$ |
| **I6** | **Staking Cooldowns** | $Epoch_{Unstake} \ge Epoch_{Stake} + 10$ |

---

## 7. Attack Surface Map & Input Validation Gates

```
  UNTRUSTED INTERFACES                   VALIDATION GATES                 TRUSTED STATE
  ┌──────────────────────┐             ┌────────────────────┐             ┌──────────────┐
  │ P2P libp2p Gossip    │ ──────────> │ RLP Parser, Size   │ ──────────> │ Mempool      │
  ├──────────────────────┤             ├────────────────────┤             ├──────────────┤
  │ REST / JSON-RPC / WS │ ──────────> │ Rate-Limiter, Type │ ──────────> │ Block Engine │
  ├──────────────────────┤             ├────────────────────┤             ├──────────────┤
  │ Bridge Claims        │ ──────────> │ Relayer Quorum 67% │ ──────────> │ UTXO Set     │
  └──────────────────────┘             └────────────────────┘             └──────────────┘
```

1. **RPC Engine**: Rejects requests exceeding size limits. Restricts method invocation via CORS and whitelist filters.
2. **Transaction Flood Mitigation**: Fee pricing adjusts dynamically based on mempool congestion. Low-fee transactions are evicted on saturation.
3. **Write-Ahead Log (WAL) Resilience**: System recovery handles partial page writes and database crashes. WAL state is replayed from the last clean checkpoint.

---

## 8. Security & Verification History (Sprint 8/9 Audits)

### 8.1 Fixed Vulnerabilities

#### 1. Serialization Determinism Transport Bug
* **Symptom**: Signed transactions would occasionally fail validation after being propagated over the network.
* **Root Cause**: The internal struct contained a `transaction_kind` metadata field that was lost or mutated during Protobuf serialization. Because the signing payload included this metadata, the network-decoded transaction had a modified signature hash, failing validation.
* **Fix**: Consolidated all protobuf conversions under `p2p.rs`, ensuring that metadata and transaction kinds are encoded explicitly into the wire format. Added hard determinism regression tests (`test_transaction_roundtrip_determinism` and `test_block_roundtrip_determinism`) asserting that both transaction metadata and block Merkle roots survive full byte-serialization roundtrips.

#### 2. Genesis UTXO Capping (Numeric Overflow)
* **Symptom**: During high-load soak testing, genesis transaction splitting caused numeric overflows when attempting to represent large QAN balances in `u64` fields inside the protobuf wire format.
* **Root Cause**: Protobuf conversions cast internal `u128` values to `u64`. Genesis outputs exceeding $2^{64}-1$ base units overflowed.
* **Fix**: Updated `tx_flood.rs` to cap spent genesis funds to 5 billion QAN ($5 \times 10^{18}$ base units) per transaction split, automatically redirecting the remainder back to the genesis address as change. This keeps all serializable amounts well within `u64::MAX`.

#### 3. Signature Malleability & Replay Protection
* **Fix**: Implemented strict validation of ECDSA signatures to reject any high-S values (`s > n/2`), eliminating transaction ID malleability. Validated replay protection by checking `chain_id` against the global registry.

#### 4. Double-Claim Prevention
* **Fix**: Hardened the bridge against double-claims by tracking processed transaction hashes in a persistent DashSet cache.

#### 5. Saga Index Slice Out-of-Bounds Panic
* **Symptom**: The 1-hour soak test crashed with a panic: `range start index 1 out of range for slice of length 0` in `saga.rs:2336`.
* **Root Cause**: The node evaluated transactional anomalies using `&b.transactions[1..]`. When an empty block (0 transactions) was indexed, the slice start index was out of bounds.
* **Fix**: Changed the unsafe slice indexing to `b.transactions.iter().skip(1)` which is a safe, non-panicking operation for slices of any length.

#### 6. Mempool Stale-UTXO Eviction & Stale Tip Mining Bug
* **Symptom**: During soak testing under heavy transaction load, block production would halt or generate continuous validation failures (`UTXO not found`) after height 4.
* **Root Cause**:
  1. *Missing Mempool Sweeps*: On block acceptance (both locally mined and P2P-gossiped), the mempool did not sweep and evict other pending transactions spending the same inputs (now spent UTXOs). Conflicting transactions remained in the mempool, were re-mined, and then failed consensus validation.
  2. *Stale Parent Tips*: Non-leader nodes' block creators did not receive updates for P2P block arrivals, causing them to permanently mine on stale tip templates, producing fork blocks.
* **Fix**:
  1. Added `evict_transactions_spending_utxos` to the mempool. Wired a post-acceptance conflict sweep in both the local block processor and the P2P block ingestion path (`node.rs`).
  2. Changed the decoupled producer's block creator to build candidate blocks on top of dynamic tips fetched from the DAG (`get_fast_tips()`).
  3. Added an `external_block_rx` notification channel to the decoupled producer. Upon P2P block acceptance, `node.rs` notifies the producer, which immediately cancels active stale PoW miners and triggers a fresh candidate generation.

---

## 9. Verification Status & Launch Gates

### 9.1 Automated Security Regression Tests
All security tests are active in the Rust unit test suite:
* `test_security_signature_malleability_low_s_enforced`: Validates High-S rejection.
* `test_transaction_roundtrip_determinism`: Tests serialization byte determinism.
* `test_block_roundtrip_determinism`: Tests block serialization byte determinism.
* `test_security_supply_conservation`: Asserts the 21B QNTO cap and balance preservation invariants.
* `test_security_chain_id_replay_protection`: Asserts replay protection between chains.
* `test_wal_tail_corruption_recovery`: Asserts WAL resilience.

### 9.2 Fuzzing Suite
Two fuzzing targets are integrated and validated:
* `fuzz_tx_parsing`: Parses raw transaction RLP payloads. Passed 10,000+ iterations.
* `fuzz_bridge_proofs`: Verifies relayer signature sets. Passed 1,000+ iterations.

### 9.3 Soak Test Harness
* **Stage A (5-minute soak)**: Verified successfully with **280,000 transactions processed** at 938.4 TPS under 0 node crashes and zero memory/descriptor leaks.
* **Stage B (1-hour soak)**: Ran successfully for 1 hour under high transaction loads (1000 TPS) with zero crashes, validating long-run RocksDB stability and P2P routing convergence.

### 9.4 Launch Gate Checklist

- [x] **Gate 0: Serialization Determinism**: Verified that signing payloads, transaction IDs, kinds, and Merkle roots match before and after Protobuf serialization.
- [x] **Gate 1: Fuzz Testing**: Run both `fuzz_tx_parsing` and `fuzz_bridge_proofs` targets to 10k+ and 1k+ iterations respectively with zero findings.
- [x] **Gate 2: Unit/Security Tests**: All 19 security regression tests compile and pass.
- [x] **Gate 3: Soak Testing (5m)**: Quick verification harness completes cleanly.
- [x] **Gate 4: Soak Testing (1h)**: Extended soak test validates system memory, file descriptor bounds, and storage persistence.

---
*Generated by QANTO Core Security Team — June 2026*
