# Qanto Protocol — Complete Architecture Reference

> **Generated:** 2026-08-17  
> **Commit:** 929cc5dd (feature/qanto-testnet-readiness-design)  
> **Graphify Corpus:** 12,417 nodes · 27,880 edges · 653 communities  
> **Status:** Not safe for testnet (per A-Z audit verdict)

---

## 1. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            QANTO NODE (qanto-node)                              │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │  Node        │  │  QantoDAG    │  │  Consensus   │  │  Miner             │  │
│  │  Orchestrator│◄─┤  (DAG Ledger)│◄─┤  Engine      │◄─┤  (PoW Solver)      │  │
│  │  (node.rs)   │  │  (qantodag)  │  │  (consensus) │  │  (miner.rs)        │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └────────┬───────────┘  │
│         │                 │                 │                    │              │
│  ┌──────▼─────────────────▼─────────────────▼────────────────────▼──────────┐  │
│  │                    SHARED STATE & INFRASTRUCTURE                          │  │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────────────────┐  │  │
│  │  │  Mempool   │ │   UTXO     │ │  Storage   │ │  P2P Network          │  │  │
│  │  │  (mempool) │ │   Set      │ │  QantoStorage│ │  (p2p.rs, libp2p)   │  │  │
│  │  └────────────┘ └────────────┘ └────────────┘ └───────────────────────┘  │  │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────────────────┐  │  │
│  │  │  SAGA AI   │ │  Wallet/   │ │  RPC/gRPC/ │ │  Difficulty           │  │  │
│  │  │  (saga.rs) │ │  Keystore  │ │  GraphQL/WS│ │  Retarget (retarget)  │  │  │
│  │  └────────────┘ └────────────┘ └────────────┘ └───────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
           ┌────────────────┐ ┌────────────┐ ┌─────────────┐
           │ qanto-core     │ │qanto-consensus│ │ qanto-rpc   │
           │ (crypto,storage│ │ primitives   │ │ (gRPC API)  │
           │  identities)    │ │ (hash,pow,QIP)│ │             │
           └────────────────┘ └────────────┘ └─────────────┘
```

---

## 2. Node Lifecycle (node.rs)

### Startup Sequence (`Node::new()` → `Node::start()`)

| Phase | Operation | Key Files/Functions |
|-------|-----------|---------------------|
| **1. Config Load** | Load/validate `Config` from TOML, apply CLI overrides | `config.rs:load()`, `Config::validate()` |
| **2. P2P Identity** | Load or generate libp2p Ed25519 keypair (`identity::Keypair`) | `node.rs:276-301` |
| **3. Wallet Validation** | Ensure wallet address matches genesis validator | `node.rs:318-326` |
| **4. SAGA Init** | Initialize `PalletSaga` (with optional ISNM service) | `node.rs:348-360` |
| **5. Storage Open** | Open `QantoStorage` (RocksDB) with `StorageConfig` | `node.rs:362-385` |
| **6. Genesis Load** | Load/persist genesis state from `genesis.json` | `absolute_genesis.rs`, `persistence.rs` |
| **7. UTXO Init** | Load persisted UTXOs or initialize from genesis balances | `node.rs:456-486` |
| **8. QantoDAG Create** | Build `QantoDAG` with genesis config, validators, emission | `qantodag.rs:1291+` |
| **9. Miner Create** | Instantiate `Miner` with Qanhash backend, thread pool | `miner.rs:364-437` |
| **10. Services** | Start IPC, WebSocket, Analytics, ResourceCleanup | `node.rs:500-544` |

### Runtime Task Topology (`Node::start()`)

```
tokio::select! biased;
├── Ctrl+C handler → graceful_shutdown()
├── API Server (Axum) → REST + GraphQL + WebSocket on :8080
├── RPC Server (gRPC) → qanto-rpc on :50051
├── P2P Server (libp2p) → GossipSub + Kademlia + Request-Response
├── Command Processor → processes P2PCommand from p2p channel
├── WebSocket Broadcast → 1s interval, pushes block/mempool/health
├── Checkpoint Task → every 10min, rotation (max 2)
├── Miner Task (if enabled)
│   ├── AdaptiveMiningLoop (adaptive_mining.rs) OR
│   └── DecoupledProducer (decoupled_producer.rs)
└── JoinSet monitor → any task failure triggers full shutdown
```

### Shutdown (`ResourceCleanup`)

- CancellationToken broadcast to all tasks
- 30s timeout for graceful completion
- Task abort on timeout
- Checkpoint persisted on shutdown

---

## 3. QantoDAG — DAG Consensus Engine (qantodag.rs)

### Core Data Structures

```rust
// Block (293KB file, ~5000 lines)
pub struct QantoBlock {
    protocol_version: u16,          // V2 = 2
    chain_id: u32,
    id: String,                     // SHA3-256 hash (canonical)
    parents: Vec<String>,           // Single-parent in V2 (selected-state-parent)
    transactions: Vec<Transaction>,
    difficulty: u64,                // Fixed-point u64 (scale 1e9)
    validator: String,              // PQ-bound validator address (V2)
    miner: String,                  // Must equal validator in V2
    nonce: u64,
    timestamp: u64,
    height: u64,
    reward: u128,
    effort: u128,
    chain_work: U256,               // Cumulative PoW
    merkle_root: String,
    signature: QuantumResistantSignature,
    state_parent: String,           // QIP-004: selected-state-parent
    state_root: String,             // Post-execution state root
    // ... carbon_credentials, cross_chain_refs, etc.
}
```

### Key Algorithms

| Function | Purpose | Location |
|----------|---------|----------|
| `QantoDAG::new()` | Initialize DAG with genesis, storage, validators | L1291 |
| `add_block()` | Validate + insert block, update UTXO, finalize | L2150 |
| `is_valid_block()` | V1 full validation pipeline | L3157 |
| `is_valid_block_v2()` | V2 validation (canonical hashing, PQ sigs) | L3300+ |
| `create_candidate_block()` | Build block template from mempool | L2714 |
| `run_periodic_maintenance()` | Finality, pruning, diff retarget | L5800+ |
| `validate_blocks_parallel()` | Rayon-parallel block validation | L4669 |

### V2 Protocol Changes (QIP-001, QIP-002, QIP-004)

| V1 (Legacy) | V2 (Canonical) |
|-------------|----------------|
| Custom hash ordering | Domain-separated `hash_v2()` |
| ECDSA P-256 "signatures" | ML-DSA-65 (PQ) signatures |
| Multi-parent DAG tips | **Single-parent + state_parent** |
| No canonical block ID | `block_id_v2()` = hash_v2(canonical_envelope) |
| Miner ≠ Validator | **Miner MUST = Validator** |
| Fee-only emission | Versioned emission schedule |
| — | `state_root` after execution |

### Storage Maps (DashMap)

| Map | Key → Value | Purpose |
|-----|-------------|---------|
| `blocks` | `String` → `QantoBlock` | All accepted blocks |
| `blocks_by_height` | `u64` → `Vec<String>` | Height index |
| `finalized_blocks` | `String` → `QantoBlock` | Finalized (immutable) |
| `utxo_set` | `String` → `UTXO` | Mempool validation |
| `validators` | `String` → `u128` | Stake weights |
| `pending_p2p_blocks` | `String` → `PendingP2PBlock` | Out-of-order block queue |

---

## 4. Hybrid Consensus Engine (consensus.rs)

### Validation Pipeline (`Consensus::validate_block()`)

```
validate_block(block, dag, utxos)
├── V2 Active? (height ≥ activation)
│   ├── YES → dag.is_valid_block_v2()  ──► Canonical V2 path
│   └── NO  → dag.is_valid_block()     ──► Legacy V1 path
```

### V1 Validation Rules (Legacy)

| Rule | Check | Severity |
|------|-------|----------|
| **Core Fields** | id, merkle_root, validator non-empty | Reject |
| **Merkle Root** | Matches `compute_merkle_root(transactions)` | Reject |
| **Coinbase** | First tx is coinbase, structure valid | Reject |
| **PoW** | `block.difficulty == required` AND `hash_meets_target()` | Reject |
| **Finality Proof** | If present, matches PoW hash | **Warning only** |
| **Transactions** | All non-coinbase txs `verify()` against UTXOs | Reject |
| **PoS** | Validator stake ≥ threshold | **Warning only** |

### V2 Validation (Canonical)

| Check | Implementation |
|-------|----------------|
| Block ID | `block_id_v2()` matches recomputed canonical hash |
| Signature | ML-DSA-65 verify over canonical envelope |
| PQ Binding | Validator address = `validator_id_v2(validator_pk)` |
| Miner = Validator | `block.miner == block.validator` (enforced) |
| State Parent | Exists, is ancestor, state_root computable |
| Emission | Reward = `Emission::next_block_reward(height)` |
| Transactions | All use `TransactionEnvelopeV2` + PQ signatures |

### Difficulty (`get_required_difficulty()`)

**Current:** Constant `QANTO_SCALE` (1e9 = difficulty 1.0)  
**Retarget:** `difficulty_retarget.rs` — EMA-based, ±25%/epoch (2016 blocks)  
**Miner Local Backoff:** Divides difficulty by 2 every 3 failed attempts (**diverges from consensus** — attack_2)

---

## 5. Transaction Pipeline (transaction.rs)

### UTXO Model

```rust
pub struct Input {
    tx_id: String,      // Previous output txid
    output_index: u32,  // Index in that tx's outputs
}

pub struct Output {
    address: String,                     // 64-char hex (PQ address in V2)
    amount: u128,                        // Base units (1 QAN = 1e9)
    homomorphic_encrypted: HomomorphicEncrypted,
}
```

### Transaction Kinds

| Kind | Purpose | Fee | Coinbase? |
|------|---------|-----|-----------|
| `Coinbase` | Block reward | 0 | Yes |
| `Transfer` | QAN transfer | Tiered | No |
| `Stake` / `Unstake` | Validator stake | Tiered | No |
| `Delegate` | Delegated stake | Tiered | No |
| `Vote` / `Proposal` | Governance | Tiered | No |
| `BridgeLock` / `BridgeClaim` | Cross-chain | Tiered | No |
| `AirdropClaim` | Token claim | Tiered | No |

### Dynamic Fee Calculation

```rust
pub fn calculate_dynamic_fee(amount: u128) -> u128 {
    let rate = if amount < 1_000_000 * Q_SCALE { 0 }      // 0%
                else if amount < 10_000_000 * Q_SCALE { 10_000_000 } // 1%
                else if amount < 100_000_000 * Q_SCALE { 20_000_000 } // 2%
                else { 30_000_000 };                        // 3%
    amount * rate / QANTO_SCALE
}
MIN_TX_FEE = 100 base units
MIN_TX_GAS = 21_000
```

### V2 Transaction Format (QIP-001)

```
TransactionEnvelopeV2 {
    body: TransactionBodyV2 {          // Canonical hash input
        sender: AccountIdV2,          // = hash_v2(pubkey, Domain::Account, chain_id)
        receiver: AccountIdV2,
        amount, fee, gas_limit, ...
        inputs: Vec<Input>,           // Spend TxIdV2 + OutputIndex
        outputs: Vec<Output>,
        nonce: u64,                   // Replay protection
        chain_id: u32,
    },
    signing: TransactionSigningV2 {
        signer_pk: QantoPQPublicKey,  // ML-DSA-65
        signature: QantoPQSignature,  // Over hash_v2(body)
    }
}
```

**Key invariant:** `sender` MUST equal `account_id_v2(signer_pk, chain_id)` — verified in `validate_native_signer_binding()`.

---

## 6. Mining Loop (miner.rs)

### Miner Architecture

```
Miner::solve_pow(block_template)
├── Fast Path (difficulty ≤ 0.01): mine_cpu_fast_path()
└── Standard Path:
    ├── calculate_target_from_difficulty(difficulty) → target_hash_bytes
    ├── prepare_mining_data() → header_hash [u8;32], target [u8;32]
    ├── get_qdag_sync(height) → precomputed QanHash DAG
    ├── QantoHash::new(header_hash)
    ├── Loop:
    │   ├── qanhash_miner.mine_with_dag(&qanto_hash, nonce, target, dag)
    │   │   └── Batched (1024 nonces), SIMD, multi-threaded
    │   ├── On success: update block.nonce, block.effort, celebrate
    │   ├── On timeout (2× target_block_time):
    │   │   ├── difficulty /= 2 (every 3 attempts) ← LOCAL BACKOFF
    │   │   └── Retry with new target
    │   └── Max 10 failed attempts → TimeoutOrCancelled
```

### Qanhash Algorithm

- **Backend:** `my_blockchain::qanhash` (CPU only — GPU disabled)
- **DAG:** Precomputed per epoch (height // 2016), cached globally
- **Target Calculation:** Deterministic integer arithmetic, no floating point
  ```rust
  // target = (2^256 - 1) / difficulty  ≈  [0xFF * (QANTO_SCALE / difficulty); 32]
  ```
- **Nonce Source:** Deterministic fallback (`ChaCha8Rng` from block data) or random

### Mining Modes

| Mode | Trigger | Producer |
|------|---------|----------|
| **Solo Miner** | `mining_enabled=true, adaptive=false` | `DecoupledProducer` (forced) |
| **Adaptive** | `adaptive_mining_enabled=true` | `AdaptiveMiningLoop` (SAGA-informed) |
| **Testnet** | `feature = "performance-test"` | Sub-second intervals |

---

## 7. SAGA AI — Adaptive Controller (saga.rs)

> **⚠️ Classified as "consensus-adjacent telemetry" — NOT consensus-safe (audit finding 7.B)**

### Components

| Module | Purpose | Status |
|--------|---------|--------|
| `PalletSaga` | Main entry point, epoch rules, proposals | Active |
| `SagaDeepNetwork` | 6-layer NN (128→256→512→256→128→64→32) | Training excised |
| `AdaptiveController` | Heuristic parameter adjustment | Active |
| `CarbonImpactPredictor` | CO₂ estimation per tx | Active |
| `AnalyticsDashboard` | Metrics aggregation | Active |
| `GovernanceProposals` | On-chain voting | Active |

### AI Models (Removed for Hardening)

| Model | Purpose | Lines Removed |
|-------|---------|---------------|
| `BehaviorNet` | Validator classification (Malicious/Neutral/Beneficial) | ~400 |
| `CongestionPredictorLSTM` | Mempool congestion forecast | ~300 |
| `CredentialVerifierNet` | Carbon credential verification | ~250 |
| `QueryClassifierNet` | NLU for governance queries | ~200 |
| Training data collection | — | Entire module |

### Active Heuristics

```rust
// Validator behavior (no ML)
fn classify_validator_behavior(m: &ValidMetrics) -> BehaviorClassification {
    if m.performance < 0.2 && m.stake < 50_000 { Malicious }
    else if m.performance > 0.9 && m.stake >= 200_000 && m.has_poco { Beneficial }
    else { Neutral }
}

// Carbon estimation (heuristic)
fn predict_co2_per_tx(tx, congestion) -> f64 {
    let ecu = tx_bytes * 0.1 + base_transfer(10.0) + contract_ops;
    ecu * (1.0 + congestion * 0.75) * 0.005 gCO₂
}
```

### SAGA-Consensus Boundary

| SAGA Output | Consensus Use | Safety |
|-------------|---------------|--------|
| `calculate_dynamic_reward()` | Block reward validation | **Used in V1 only** |
| `epoch_rules` params | Difficulty/fee multipliers | Advisory only |
| Threat level | Mining circuit breaker | **Not consensus** |
| Carbon credentials | Block carbon_credentials | Metadata only |

---

## 8. P2P Networking (p2p.rs)

### Stack: libp2p

| Protocol | Configuration |
|----------|---------------|
| **Transport** | Noise XX + Yamux |
| **Discovery** | Kademlia (MemoryStore) + mDNS (optional) |
| **Gossip** | GossipSub v1.1, fixed topics per network_id |
| **Request-Response** | QDS protocol (`/qanto/qds/1.0.0`) |
| **Message Auth** | HMAC (default secret — **insecure**) + ML-DSA sig |

### GossipSub Topics (Fixed per `network_id`)

| Topic | Payload |
|-------|---------|
| `{net_id}/blocks` | `QantoBlock` |
| `{net_id}/transactions` | `Transaction` |
| `{net_id}/tx_batch` | `Vec<Transaction>` |
| `{net_id}/state_sync` | `(blocks, utxos)` |
| `{net_id}/carbon_cred` | `CarbonOffsetCredential` |

### P2P Server (`P2PServer`)

```
P2PServer::new(config) → Swarm<NodeBehaviour>
.run(rx_commands)  // Event loop
    ├── SwarmEvent::Behaviour(NodeBehaviourEvent)
    │   ├── Gossipsub(Message) → validate → forward to command channel
    │   ├── Kademlia(Discovered peer) → dial
    │   └── QDS(Request) → handle_query → send_response
    └── P2PCommand from command channel → publish to GossipSub
```

### Peer Management

- **Reconnect:** Exponential backoff (5s → 300s, ×1.5, ±25% jitter)
- **Blacklist:** 24h TTL for misbehaving peers
- **Keepalive:** 30s WSS ping
- **Rate Limit:** 50 req/s per peer (governor)

---

## 9. Storage Layer

### QantoStorage (qanto-core/src/qanto_storage.rs)

```
RocksDB Column Families:
├── default           → Block headers, metadata
├── blocks            → Full QantoBlock (bincode)
├── transactions      → Transaction by txid
├── utxos             → UTXO by "txid_index"
├── accounts          → AccountStateCache (balance, nonce)
├── validators        → Validator stake
├── emission          → EmissionStateV2
├── finality          → Finality certificates
├── state_roots       → Block height → state_root
└── checkpoints       → Periodic snapshots
```

### AccountStateCache

```rust
pub struct AccountStateCache {
    balances: RwLock<HashMap<String, u128>>,
    nonces: RwLock<HashMap<String, u64>>,
    // LRU eviction (10,000 entries)
}
```

### Persistence Writer (persistence.rs)

| Key Prefix | Value |
|------------|-------|
| `b_` | Block by ID |
| `h_` | Block IDs by height |
| `t_` | Transaction by ID |
| `u_` | UTXO by key |
| `a_` | Account balance |
| `v_` | Validator stake |
| `e_` | Emission state |
| `tip_` | Current tip(s) |
| `genesis_` | Genesis marker |

---

## 10. RPC Service (qanto-rpc)

### gRPC Endpoints (`qanto-rpc/src/server.rs`)

| Method | Request | Response | Purpose |
|--------|---------|----------|---------|
| `GetBalance` | `GetBalanceRequest {address}` | `GetBalanceResponse {base_units, ...}` | Account balance |
| `GetUtxos` | `GetUtxosRequest {address}` | `GetUtxosResponse {utxos[]}` | Spendable UTXOs |
| `SubmitTx` | `SubmitTxRequest {tx_bytes}` | `SubmitTxResponse {tx_id, accepted}` | Mempool submission |
| `GetBlock` | `GetBlockRequest {height/hash}` | `GetBlockResponse {block}` | Block query |
| `GetNetworkStats` | `GetNetworkStatsRequest` | `GetNetworkStatsResponse` | Health metrics |
| `SubscribeBlocks` | (stream) | `BlockSummary` stream | Real-time blocks |
| `SubscribeTransactions` | `address` | `TransactionUpdate` stream | Address activity |

### REST + GraphQL + WebSocket (node.rs API Server)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/info` | GET | Node version, chain_id, peers |
| `/balance/{addr}` | GET | Balance + UTXOs |
| `/utxos/{addr}` | GET | UTXO list |
| `/transaction` | POST | Submit single tx |
| `/submit-transactions` | POST | Batch submit |
| `/block/{id}` | GET | Block details |
| `/dag` | GET | DAG snapshot |
| `/health` | GET | Readiness probe |
| `/mempool` | GET | Mempool contents |
| `/analytics/dashboard` | GET | SAGA analytics |
| `/metrics` / `/metrics/prometheus` | GET | Prometheus exposition |
| `/rpc` | POST | JSON-RPC 2.0 proxy |
| `/ws` | WS | Real-time events |

---

## 11. Cross-Chain / Interop

### Bridge Architecture (interoperability.rs)

```
Qanto (Source)                    Ethereum/BSC (Target)
┌─────────────────┐              ┌─────────────────┐
│ BridgeLock tx   │── Event ──►  │ Bridge Contract │
│ (amount, dest)  │   (indexed)  │ lock()          │
└─────────────────┘              └────────┬────────┘
                                          │
                    ZK Proof (NOT YET)    │
                    ┌─────────────────────┘
                    ▼
         ┌─────────────────────┐
         │ BridgeClaim tx      │
         │ (merkle proof,      │
         │  nullifier, amount) │
         └─────────────────────┘
```

### QIP-007 Status (Bridge)

| Component | Implemented | Gap |
|-----------|-------------|-----|
| `BridgeLock` tx kind | ✅ | — |
| Event emission | ✅ | — |
| ZK Proof Verifier | ❌ | `BRIDGE_PROOF_VERIFIER_AVAILABLE = false` |
| Nullifier Set | ❌ | Mock only |
| Atomic Claim | ❌ | Fail-open currently |

### IBC / Relayer

- `qanto-relayer` crate exists (minimal)
- `IbcConnection`, `IbcPacket` types in `interoperability.rs`
- No canonical light client or relayer binary

---

## 12. Wallet / Keystore

### qanto-desktop-wallet (Tauri + React)

| Component | File | Purpose |
|-----------|------|---------|
| `keystore.rs` | `src-tauri/src/keystore.rs` | Encrypted wallet storage (SQLite + AES) |
| `tx.rs` | `src-tauri/src/tx.rs` | Transaction building/signing |
| `wallet.ts` | `src/lib/wallet.ts` | Frontend wallet state |

### Wallet Structure (protocol_identity.rs)

```rust
// V2 PQ-bound identity
fn account_address_v2(chain_id, pubkey) -> String
fn validator_id_v2(chain_id, pubkey) -> String
// ML-DSA-65 keypair (1952-byte pubkey, 2528-byte privkey)
```

### Key Derivation

| Path | Algorithm |
|------|-----------|
| Seed → Root | BIP-39 (24 words) + PBKDF2 |
| Root → Account | ML-DSA-65 keypair per account |
| Account → Address | `hash_v2(pubkey, Domain::Account, chain_id)` |

---

## 13. Smart Contracts / ZK

### qanto-zk-sdk

| Circuit | Status | Purpose |
|---------|--------|---------|
| `QanHashZK` | Stub | Prove PoW without revealing nonce |
| `BridgeProof` | Mock | Bridge merkle proof |
| `CarbonCred` | Stub | Prove CO₂ offset validity |

### ZK Integration Points

```rust
// qantodag.rs
const BRIDGE_PROOF_VERIFIER_AVAILABLE: bool = false;

// ZK feature gate
#[cfg(feature = "zk")]
pub mod zkp;  // Full arkworks circuits
#[cfg(not(feature = "zk"))]
pub mod zkp_stub;  // No-op verifier
```

### EVM Compatibility

- `qanto-node/src/evm_*.rs` — EVM transaction execution
- `SmartContract` struct in qantodag (toy interpreter)
- Solidity contracts in `SmartContracts/` (Hardhat)

---

## 14. Attack Test Modules (12 Exploits)

All under `#[cfg(test)]` in `qanto-node/src/`:

| Test File | Vulnerability | Status |
|-----------|---------------|--------|
| `attack_1_single_parent_dag_safety_violation.rs` | Double-spend via parent equivocation | **Fixed** (V2 single-parent) |
| `attack_2_liveness_failure_fixed_difficulty.rs` | Miner backoff ≠ consensus difficulty | **Mitigated** (retarget) |
| `attack_3_finality_absence_reorg_any_depth.rs` | No finality → arbitrary reorg | **Open** (finality disabled) |
| `attack_4_evm_raw_tx_bypass_pq.rs` | EVM tx bypasses PQ sig verify | **Fixed** (enforce V2) |
| `attack_5_bridge_claim_forged_signature.rs` | Bridge claim sig forgery | **Open** (verifier missing) |
| `attack_6_rpc_ws_unauthenticated.rs` | Unauth RPC/WS | **Open** |
| `attack_7_p2p_eclipse_fixed_topics.rs` | Eclipse via fixed topics | **Partial** |
| `attack_8_miner_reservation_griefing.rs` | Miner reservation DoS | **Fixed** (removed) |
| `attack_9_consensus_lock_bottleneck.rs` | Global lock contention | **Fixed** (DashMap) |
| `attack_10_blocks_map_performance.rs` | Blocks Map O(n) growth | **Monitoring** |
| `attack_11_saga_f64_nondeterminism.rs` | f64 in consensus path | **Partial** |
| `attack_12_omega_threat_manipulation.rs` | ΩMEGA telemetry injection | **Isolated** |

---

## 15. Key Configuration (config.rs)

```toml
# config.toml (loaded by Config::load())
[network]
network_id = "qanto-testnet"
chain_id = 1234
genesis_validator = "64-char-hex-address"

[p2p]
address = "/ip4/0.0.0.0/tcp/4001"
peers = ["/ip4/bootnode/tcp/4001/p2p/..."]

[api]
address = "0.0.0.0:8080"

[rpc]
address = "0.0.0.0:50051"

[mining]
enabled = true
adaptive_mining_enabled = false
target_block_time = 600000  # ms
mining_threads = 0          # 0 = auto (cpus-1)
use_gpu = false
zk_enabled = false

[mempool]
capacity = 100000
max_size_bytes = 1073741824  # 1GB
max_age_secs = 3600

[storage]
data_dir = "./data"
block_cache_size_mb = 128
write_buffer_size_mb = 64

[logging]
level = "info"
format = "json"
```

---

## 16. Feature Flags (Cargo.toml)

| Feature | Enables | Default |
|---------|---------|---------|
| `infinite-strata` | `InfiniteStrataNode`, ISNM service | ❌ |
| `zk` | Full arkworks ZK circuits (`zkp.rs`) | ❌ |
| `performance-test` | Low difficulty, fast intervals | ❌ |
| `default` | Core node, no exotic features | ✅ |

---

## 17. Critical Architectural Decisions

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| **Single-parent DAG + state_parent** (QIP-004) | Enables atomic state commits, reorg recovery | Loses multi-parent concurrency |
| **PQ signatures only (ML-DSA-65)** | Quantum resistance | 1952-byte pubkeys, ~2ms verify |
| **Miner = Validator (V2)** | Prevents DPoS stake delegation attacks | No separate mining pools |
| **Fixed block reward (50 QAN)** | Deterministic emission | No fee market dynamics yet |
| **CPU-only Qanhash** | Consensus-deterministic, no GPU trust | Lower hashrate, no ASIC |
| **SAGA isolated from consensus** | Safety — AI nondeterminism | Governance params advisory only |
| **HMAC secret default** | Dev convenience | **CRITICAL** — must rotate for testnet |

---

## 18. Launch Gates (Per Qanto Public Testnet Launch Plan)

| Gate | Requirement | Status |
|------|-------------|--------|
| PTG-01 | Approved normative protocol + release scope | ❌ Specs in flux |
| PTG-02 | Authenticated account/tx/validator/peer identity | ⚠️ V2 auth done, P2P HMAC weak |
| PTG-03 | Canonical commitments + portable PoW | ⚠️ Hash canonical, GPU disabled |
| PTG-04 | Deterministic branch state + atomic commit | ❌ QIP-004 partial |
| PTG-05 | Safe active finality + validator transition | ❌ Finality disabled |
| PTG-06 | Deterministic economics + coergenesis + enforced supply | ⚠️ Emission math done, genesis open |
| PTG-07 | SAGA/ΩMEGA consensus isolation | ✅ Documented, not enforced |

---

## 19. File Map (Key Source Files)

```
Protocol_Core/
├── qanto-node/src/
│   ├── node.rs              # Node orchestrator (191KB)
│   ├── qantodag.rs          # DAG ledger (294KB)
│   ├── consensus.rs         # Hybrid consensus (16KB)
│   ├── transaction.rs       # UTXO tx pipeline (76KB)
│   ├── miner.rs             # PoW mining (81KB)
│   ├── saga.rs              # AI governance (176KB)
│   ├── p2p.rs               # libp2p networking (89KB)
│   ├── config.rs            # Configuration (49KB)
│   ├── difficulty_retarget.rs # EMA retarget (9KB)
│   ├── persistence.rs       # RocksDB keys (18KB)
│   ├── mempool.rs           # Transaction pool (83KB)
│   ├── absolute_genesis.rs  # Genesis loading (42KB)
│   ├── decoupled_producer.rs # High-throughput block prod (56KB)
│   ├── adaptive_mining.rs   # SAGA-informed mining (38KB)
│   ├── wallet.rs            # Wallet CLI (18KB)
│   ├── rpc_backend.rs       # gRPC backend (28KB)
│   ├── websocket_server.rs  # WS server (27KB)
│   ├── graphql_server.rs    # GraphQL API (14KB)
│   ├── types.rs             # Core types (4KB)
│   └── attack_*.rs          # 12 exploit tests (4-19KB each)
├── qanto-core/src/
│   ├── qanto_storage.rs     # RocksDB wrapper
│   ├── protocol_identity.rs # PQ address derivation
│   ├── qanto_native_crypto.rs # ML-DSA, KEM wrappers
│   └── balance_stream.rs    # BalanceBroadcaster
├── qanto-consensus-primitives/src/
│   ├── hash.rs              # hash_v2, domain separation
│   ├── pow.rs               # PoW verify, target calc
│   ├── record.rs            # CanonicalRecord encoding
│   ├── identity_v2.rs       # account_id_v2, validator_id_v2
│   ├── transaction_v2.rs    # TransactionEnvelopeV2
│   └── consensus_parameters.rs # V2 activation heights
└── myblockchain/
    └── qanhash/             # CPU PoW + DAG generation
```

---

## 20. Known Gaps (From Audit)

### P0 — Must Fix Before Any Value Network
1. **Finality disabled** — No checkpointing, 100-block reorgs possible
2. **Bridge verifier missing** — `BRIDGE_PROOF_VERIFIER_AVAILABLE = false`
3. **P2P HMAC default secret** — Production MITM trivial
4. **Custom P-256 "signature"** — Actually hash construction, universally forgeable (V2 fixes, but V1 still reachable)
5. **SAGA f64 nondeterminism** — Consensus-adjacent telemetry uses floats

### P1 — Testnet Hardening
6. `qantodag` blocks_map unbounded growth → OOM risk
7. No authenticated state sync (peer-supplied UTXOs discarded but no replacement)
8. RPC/WS unauthenticated
9. Difficulty retarget not fully wired to consensus (miner local backoff remains)

### P2 — Post-Testnet
10. GPU/Metal mining backends (CPU-oracle KATs required first)
11. ZK bridge verifier (arkworks circuits)
12. SAGA neural components (fully excised — re-implement deterministically if needed)

---

*End of Architecture Reference. Update via `graphify update .` after code changes.*