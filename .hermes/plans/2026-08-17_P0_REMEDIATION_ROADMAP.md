# Qanto P0 Remediation Roadmap — SAFE FOR PRIVATE TESTNET

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Fix all 18 CRITICAL findings from the A-Z audit to achieve **SAFE FOR PRIVATE TESTNET** readiness.

**Architecture:** Systematic remediation across 6 parallel tracks addressing consensus safety, liveness, networking, crypto, storage, and RPC. Each track uses TDD with regression tests.

**Tech Stack:** Rust 1.98+, libp2p 0.56, RocksDB, qanto_native_crypto (ML-DSA-65), tokio, async-trait.

---

## Phase 0: Repository Baseline & Merge Strategy

### Task 0.1: Clean Main Branch Merge
**Objective:** Merge `feature/qanto-testnet-readiness-design` into `main` as baseline

**Files:**
- All modified files in `Protocol_Core/` (31 files from git status)

**Steps:**
1. Commit current uncommitted changes on `main`
2. Merge `feature/qanto-testnet-readiness-design` into `main`
3. Run `cargo test --workspace --locked` to establish baseline
4. Record test count (currently 322 passed, 1 skipped)

**Verification:**
```bash
cd /Users/cps/qanto
git add -A && git commit -m "chore: baseline before P0 remediation"
git merge feature/qanto-testnet-readiness-design --no-ff -m "merge: testnet-readiness design"
cargo test --workspace --locked 2>&1 | tail -20
```

---

## Track A: Consensus Safety & Finality

### Task A.1: Enable Difficulty Retarget in Consensus
**Objective:** Wire `difficulty_retarget.rs` into `is_valid_block()` validation

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs:3228-3235` (lines ~3228-3235)
- Test: `Protocol_Core/qanto-node/src/difficulty_retarget.rs` (existing tests)

**Current Code (lines 3227-3235):**
```rust
// Validate difficulty using adaptive retarget
let state = self.difficulty_state.read().await;
let required_difficulty = get_required_difficulty(&state, block.height);
if block.difficulty as u128 != required_difficulty {
    return Err(QantoDAGError::InvalidBlock(format!(
        "Block difficulty does not match the adaptive network requirement: expected {required_difficulty}, got {}",
        block.difficulty
    )));
}
```

**Issue:** `difficulty_state` field doesn't exist on `QantoDAG` yet.

**Step 1: Add DifficultyState to QantoDAG**
```rust
// In QantoDAG struct definition (around line 200-300)
use crate::difficulty_retarget::DifficultyState;

// Add field:
difficulty_state: Arc<RwLock<DifficultyState>>,
```

**Step 2: Initialize in QantoDAG::new()**
```rust
difficulty_state: Arc::new(RwLock::new(DifficultyState::default())),
```

**Step 3: Call record_solve_time after block acceptance**
```rust
// In commit_block or after successful validation
use crate::difficulty_retarget::record_solve_time;

let solve_time_ms = calculate_solve_time(&block); // implement this
record_solve_time(&mut *self.difficulty_state.write().await, solve_time_ms, block.height);
```

**Step 4: Fix validation to use difficulty_state**
```rust
// Replace the validation block:
let state = self.difficulty_state.read().await;
let required_difficulty = crate::difficulty_retarget::get_required_difficulty(&state, block.height);
if block.difficulty as u128 != required_difficulty { ... }
```

**Step 5: Run tests**
```bash
cargo test --package qanto-node difficulty_retarget -- --nocapture
cargo test --package qanto-node is_valid_block -- --nocapture
```

**Verification:** All difficulty_retarget tests pass + new integration test for dynamic difficulty.

---

### Task A.2: Remove Miner Local Backoff Divergence
**Objective:** Remove miner's local `difficulty /= 2` that conflicts with consensus

**Files:**
- Modify: `Protocol_Core/qanto-node/src/miner.rs` (search for backoff logic)
- Test: Create new test verifying miner respects consensus difficulty

**Search for:**
```bash
grep -n "difficulty.*divide\|backoff.*difficulty\|\/=.*2" Protocol_Core/qanto-node/src/miner.rs
```

**Expected locations:** Around ExponentialBackoffConfig or solve_pow loop.

**Fix:** Remove local difficulty adjustment. Miner should only use `block.difficulty` from DAG.

**Verification:** Attack test `attack_2_liveness_failure_fixed_difficulty` should still pass but now test actual consensus retarget.

---

### Task A.3: Implement Finality Gadget (Validator Votes + Certificates)
**Objective:** Replace `finalize_blocks()` no-op with real finality

**Files:**
- Create: `Protocol_Core/qanto-node/src/finality.rs` (new module)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs:4326-4331`
- Modify: `Protocol_Core/qanto-node/src/lib.rs` (export finality module)

**Design:**
1. **Vote struct:** `FinalityVote { block_id, validator, signature, round }`
2. **Certificate struct:** `FinalityCertificate { block_id, votes: Vec<FinalityVote>, threshold_met: bool }`
3. **Quorum:** 2/3 of active validator stake
4. **Storage:** Persist certificates in RocksDB

**Step 1: Create FinalityVote and Certificate types**
```rust
// finality.rs
use qanto_core::qanto_native_crypto::{QantoPQPublicKey, QantoPQSignature};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityVote {
    pub block_id: String,
    pub validator: String,
    pub signature: QantoPQSignature,
    pub round: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityCertificate {
    pub block_id: String,
    pub votes: Vec<FinalityVote>,
    pub aggregate_stake: u128,
    pub total_stake: u128,
    pub threshold_met: bool,
}
```

**Step 2: Implement vote collection and aggregation**
```rust
pub struct FinalityEngine {
    dag: Arc<QantoDAG>,
    pending_votes: Arc<RwLock<HashMap<String, Vec<FinalityVote>>>>,
    finalized_blocks: Arc<RwLock<HashSet<String>>>,
}

impl FinalityEngine {
    pub async fn collect_vote(&self, vote: FinalityVote) -> Result<(), FinalityError> { ... }
    pub async fn check_finality(&self, block_id: &str) -> Option<FinalityCertificate> { ... }
    pub async fn finalize_block(&self, block_id: &str) -> Result<FinalityCertificate, FinalityError> { ... }
}
```

**Step 3: Wire into QantoDAG.finalize_blocks()**
```rust
pub async fn finalize_blocks(&self) -> Result<(), QantoDAGError> {
    self.finality_engine.check_and_finalize_pending().await
}
```

**Step 4: Add regression test**
```rust
#[tokio::test]
async fn test_finality_gadget_two_thirds_quorum() {
    // Create validators with stake
    // Submit votes for block
    // Verify certificate generated at 2/3 threshold
    // Verify block marked finalized
}
```

**Verification:** `cargo test finality -- --nocapture` passes.

---

### Task A.4: Enable Multi-Parent DAG Consensus
**Objective:** Remove single-parent enforcement (`parents.len() != 1`)

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs:3305-3308`
- Test: `Protocol_Core/qanto-node/src/attack_1_single_parent_dag_safety_violation.rs`

**Current Code (lines 3305-3308):**
```rust
if block.parents.len() != 1 {
    return Err(QantoDAGError::InvalidParent(
        "Containment mode requires exactly one state parent".to_string(),
    ));
}
```

**Fix:** Allow multiple parents with proper DAG validation:
```rust
// Validate each parent exists and is valid
for parent_id in &block.parents {
    if !self.blocks.contains_key(parent_id) {
        return Err(QantoDAGError::InvalidParent(format!(
            "Parent block {} not found", parent_id
        )));
    }
    // Verify parent height < current height
    let parent = self.blocks.get(parent_id).unwrap();
    if parent.height >= block.height {
        return Err(QantoDAGError::InvalidParent(...));
    }
}

// Calculate merge state from all parents (UTXO merge)
// Use GHOST-style or heaviest-subdag fork choice
let heaviest_parent = self.select_heaviest_parent(&block.parents)?;
```

**Verification:** Attack test `attack_1` should now test multi-parent safety, not just demonstrate vulnerability.

---

### Task A.5: Implement Equivocation Slashing
**Objective:** Penalize validators for double-signing

**Files:**
- Create: `Protocol_Core/qanto-node/src/slashing.rs`
- Modify: `Protocol_Core/qanto-node/src/byzantine_tests.rs:103-119`

**Current Test (detects but doesn't punish):**
```rust
#[tokio::test]
async fn test_byzantine_double_signing_rejected_without_unbound_slashing() {
    // Detects double-signing but only warns
}
```

**Design:**
1. Track validator signatures per (height, round)
2. On double-sign: slash stake, remove from validator set, emit evidence
3. Evidence stored in block for other validators to verify

**Verification:** Updated test verifies slashing executes.

---

## Track B: Networking & P2P Security

### Task B.1: Implement Dynamic GossipSub Topics
**Objective:** Replace fixed topics with epoch-based rotating topics

**Files:**
- Modify: `Protocol_Core/qanto-node/src/p2p.rs` (topic subscription logic)
- Test: `Protocol_Core/qanto-node/src/attack_7_p2p_eclipse_fixed_topics.rs`

**Current:** Fixed topics like `"qanto-blocks"`, `"qanto-transactions"`

**Fix:**
```rust
// Generate topic per epoch: "qanto-blocks-epoch-{epoch_number}"
fn get_gossipsub_topic(base: &str, epoch: u64) -> String {
    format!("{}-epoch-{}", base, epoch / EPOCH_LENGTH)
}

// Rotate topics every N blocks
const TOPIC_ROTATION_EPOCHS: u64 = 1000;
```

**Verification:** Attack test updated to verify topic rotation prevents prediction.

---

### Task B.2: Add Peer Diversity Requirements
**Objective:** Enforce minimum peers from different /16 subnets

**Files:**
- Modify: `Protocol_Core/qanto-node/src/p2p.rs` (peer scoring, connection logic)
- Add: `Protocol_Core/qanto-node/src/peer_diversity.rs` (new module)

**Implementation:**
```rust
pub struct PeerDiversityConfig {
    pub min_peers_total: usize,
    pub min_subnets: usize,
    pub max_peers_per_subnet: usize,
    pub require_geographic_diversity: bool,
}

impl PeerManager {
    pub async fn can_accept_peer(&self, peer_addr: &Multiaddr) -> bool {
        let subnet = extract_subnet(peer_addr); // /16
        let current_in_subnet = self.peers_per_subnet.get(subnet).copied().unwrap_or(0);
        current_in_subnet < self.config.max_peers_per_subnet
    }
}
```

**Verification:** Attack test `attack_7_no_peer_diversity_requirements` passes with enforcement.

---

### Task B.3: Add Proactive Peer Scoring
**Objective:** Score peers for diversity, uptime, not just reactive banning

**Files:**
- Modify: `Protocol_Core/qanto-node/src/p2p.rs` (peer scoring)
- Test: `Protocol_Core/qanto-node/src/attack_7_peer_scoring_insufficient.rs`

**Implementation:**
```rust
struct PeerScore {
    diversity_score: f64,      // unique subnet, geo, ASN
    uptime_score: f64,         // hours online
    validity_score: f64,       // valid msgs / total msgs
    latency_score: f64,        // propagation latency
    total: f64,
}

impl PeerManager {
    fn update_score(&mut self, peer_id: PeerId, event: PeerEvent) {
        match event {
            PeerEvent::ValidBlockReceived { latency_ms } => { ... }
            PeerEvent::NewSubnetPeer => { ... }
            PeerEvent::PeerUptime(hours) => { ... }
        }
    }
}
```

---

### Task B.4: Minimum Peer Count Enforcement
**Objective:** Require N peers from M subnets before considering synced

**Files:**
- Modify: `Protocol_Core/qanto-node/src/node.rs` (sync logic)
- Test: `Protocol_Core/qanto-node/src/attack_7_no_minimum_peer_count_enforcement.rs`

---

### Task B.5: Early Message Validation at Ingress
**Objective:** Validate messages before they enter mesh, rate-limit per topic per peer

**Files:**
- Modify: `Protocol_Core/qanto-node/src/p2p.rs` (gossipsub event handler)
- Test: `Protocol_Core/qanto-node/src/attack_7_message_validation_too_late.rs`

---

## Track C: Cryptography Fixes

### Task C.1: Remove/Replace P256 Hash Construction
**Objective:** Replace universally forgeable P256 with real ECDSA or remove EVM path

**Files:**
- Modify: `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:652-726`
- Test: Add test verifying P256 signatures cannot be forged

**Options:**
1. **Remove entirely** - if EVM compatibility not needed for testnet
2. **Replace with real P-256** - use `p256` crate with proper ECDSA

**Recommendation:** Remove for testnet (simpler, safer). Add feature flag `evm-compat` for future.

**Verification:** Fuzz test generates valid sig, attempts forge, must fail.

---

### Task C.2: Implement PQ Key Rotation
**Objective:** Automate ML-DSA key rotation per policy

**Files:**
- Create: `Protocol_Core/qanto-node/src/key_rotation.rs`
- Modify: `Protocol_Core/qanto-node/src/wallet.rs`

**Existing Policy (not enforced):**
```rust
// In wallet.rs or saga.rs - KeyRotationPolicy
pub struct KeyRotationPolicy {
    pub rotation_interval_days: u64,  // 30
    pub max_usage_count: u64,         // 100_000
}
```

**Implementation:**
1. Track key age and usage count per key pair
2. Auto-generate new keypair before expiry
3. Support multiple active keys during transition
4. Update validator registration with new key

---

## Track D: RPC & API Security

### Task D.1: Add RPC Authentication
**Objective:** Require auth for all RPC methods

**Files:**
- Modify: `Protocol_Core/qanto-node/src/rpc_backend.rs`
- Test: `Protocol_Core/qanto-node/src/attack_6_rpc_ws_unauthenticated.rs`

**Implementation:**
```rust
// Add JWT or API key authentication
struct AuthConfig {
    api_keys: HashSet<String>,  // or JWT secret
    rate_limits: RateLimitConfig,
}

impl RpcService {
    async fn authenticate(&self, request: &Request) -> Result<Identity, AuthError> {
        // Check Authorization header or API key
        // Apply rate limits per identity
    }
}
```

**Rate Limiting:**
```rust
use governor::{Quota, RateLimiter};

lazy_static::lazy_static! {
    static ref RPC_RATE_LIMITER: RateLimiter<NotKeyed, InMemoryState, DefaultClock> = 
        RateLimiter::direct(Quota::per_second(nonzero!(100u32)));
}
```

---

### Task D.2: WebSocket Authentication
**Objective:** Authenticate WS connections before subscription

**Files:**
- Modify: `Protocol_Core/qanto-node/src/rpc_backend.rs` (WS handler)
- Test: Same as D.1

---

### Task D.3: Bridge Verification - Implement or Permanently Disable
**Objective:** Either implement real ZK verifier or remove bridge claims entirely

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs:3252-3261` (BRIDGE_PROOF_VERIFIER_AVAILABLE)
- Modify: `Protocol_Core/qanto-zk-sdk/src/lib.rs` (verify_bridge_transaction)

**Current:** `BRIDGE_PROOF_VERIFIER_AVAILABLE = false`, ZK SDK returns error

**Decision for Testnet:** Permanently disable bridge claims, document as future work.

**Fix:**
```rust
// Remove BridgeClaim from TransactionKind enum OR
// Keep but always reject in validation with clear error
if tx.transaction_kind == TransactionKind::BridgeClaim {
    return Err("Bridge claims permanently disabled for testnet");
}
```

**Verification:** Attack test `attack_5_bridge_claim_forged_signature` verifies rejection.

---

## Track E: Storage & State Sustainability

### Task E.1: Implement Blocks Map Eviction / Pruning
**Objective:** Prevent unbounded memory growth in `blocks` DashMap

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (blocks field)
- Create: `Protocol_Core/qanto-node/src/pruning.rs`

**Current:** `blocks: DashMap<String, QantoBlock>` — never evicted

**Fix:**
```rust
// Keep only recent N blocks in memory, older in RocksDB
const IN_MEMORY_BLOCK_LIMIT: usize = 10_000;

impl QantoDAG {
    async fn maybe_evict_old_blocks(&self) {
        if self.blocks.len() > IN_MEMORY_BLOCK_LIMIT {
            let to_evict = self.blocks.len() - IN_MEMORY_BLOCK_LIMIT;
            // Evict oldest by height, ensure persisted to RocksDB first
        }
    }
}
```

**Verification:** Attack test `attack_10_blocks_map_performance` verifies bounded memory.

---

### Task E.2: Add Mempool Per-Peer/Account Limits
**Objective:** Prevent resource exhaustion from single peer/account

**Files:**
- Modify: `Protocol_Core/qanto-node/src/mempool.rs` (EnhancedMempool)
- Test: `Protocol_Core/qanto-node/src/attack_13_mempool_per_peer_limits.rs` (new)

**Implementation:**
```rust
struct MempoolLimits {
    max_total_txs: usize,           // existing
    max_total_bytes: usize,         // existing
    max_per_peer_txs: usize,        // NEW: e.g., 1000
    max_per_peer_bytes: usize,      // NEW: e.g., 1MB
    max_per_account_txs: usize,     // NEW: e.g., 100
    max_per_account_bytes: usize,   // NEW: e.g., 100KB
}

impl Mempool {
    fn check_limits(&self, tx: &Transaction, peer_id: Option<PeerId>) -> Result<(), MempoolError> {
        // Check per-peer, per-account limits before accepting
    }
}
```

---

### Task E.3: Cheap PQ Signature Pre-Filter
**Objective:** Fast rejection of invalid PQ signatures before full verification

**Files:**
- Create: `Protocol_Core/qanto-node/src/pq_preverify.rs`
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs:3286-3288` (verify_signature call)

**Idea:** Check signature format, length, basic structure before expensive ML-DSA verify

```rust
fn quick_pq_preverify(sig: &QuantumResistantSignature, pk: &[u8]) -> Result<(), PQError> {
    // 1. Check algorithm identifier valid
    // 2. Check signature length matches algorithm (ML-DSA-65 = 3309 bytes)
    // 3. Check public key length (1952 bytes)
    // 4. Basic format validation
    // Fast reject in <10μs vs ~1ms full verify
}
```

**Verification:** Attack test `attack_14_invalid_pq_sig_flood` verifies pre-filter drops garbage fast.

---

### Task E.4: State Pruning & Archival Model
**Objective:** Design and implement state pruning for long-term sustainability

**Files:**
- Create: `Protocol_Core/qanto-node/src/state_pruning.rs`
- Modify: `Protocol_Core/qanto-node/src/qanto_storage.rs`

**Design:**
- Active state (last N blocks UTXOs) → in-memory + RocksDB hot
- Historical blocks → compressed in RocksDB cold
- Archival nodes → full history (optional role)
- Prune triggers: height-based, size-based

---

### Task E.5: Snapshot Authenticity Verification
**Objective:** Verify snapshots before accepting state

**Files:**
- Modify: `Protocol_Core/qanto-node/src/snapshot.rs` (ProductionSnapshotService)
- Test: New test for malicious snapshot rejection

**Implementation:**
```rust
struct SnapshotVerification {
    expected_chain_id: u64,
    expected_height: u64,
    expected_state_root: [u8; 32],  // Merkle root of UTXO set
    finality_certificate: FinalityCertificate,  // Proof of finality
}

fn verify_snapshot(snapshot: &Snapshot, expected: &SnapshotVerification) -> Result<(), SnapshotError> {
    // 1. Verify chain ID matches
    // 2. Verify height matches
    // 3. Compute state root from snapshot UTXOs, compare to expected
    // 4. Verify finality certificate (2/3 validator signatures)
    // Only accept if ALL checks pass
}
```

---

## Track F: SAGA Determinism

### Task F.1: Remove or Fix SAGA f64 Non-Determinism
**Objective:** Ensure no f64 in consensus-critical paths

**Files:**
- Modify: `Protocol_Core/qanto-node/src/saga.rs:3754-3828` (calculate_dynamic_reward)
- Modify: `Protocol_Core/qanto-node/src/consensus.rs:272` (validate_block_reward call)
- Test: `Protocol_Core/qanto-node/src/attack_11_saga_f64_nondeterminism.rs`

**Current Issue:** `calculate_dynamic_reward()` uses f64 extensively, marked `#[allow(dead_code)]` but callable.

**Options:**
1. **Remove entirely** - mark as permanently disabled for consensus
2. **Convert to fixed-point** - use integer arithmetic with scaling

**Recommendation for Testnet:** Remove from consensus path, keep for analytics only.

**Fix:**
```rust
// In consensus.rs:272 - validate_block_reward
// Current: calls saga.calculate_dynamic_reward() (dead_code but callable)
// Fix: use simple fee-based reward only (no SAGA)

pub async fn validate_block_reward(&self, block: &QantoBlock) -> Result<(), QantoDAGError> {
    // Containment mode: reward = sum of fees only
    let total_fees = block.transactions.iter().skip(1).map(|tx| tx.fee).sum::<u128>();
    if block.reward != total_fees {
        return Err(QantoDAGError::InvalidBlock(format!(
            "Invalid reward: expected {} (fees only), got {}", total_fees, block.reward
        )));
    }
    Ok(())
}
```

**Verification:** Attack test verifies no f64 in consensus path.

---

## Track G: Validator Set Consensus

### Task G.1: Consensus on Validator Set Membership
**Objective:** Make validator set changes go through consensus, not local mutation

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (validators field)
- Create: `Protocol_Core/qanto-node/src/validator_set.rs`

**Current:** `validators: DashMap<String, u128>` — mutable locally

**Fix:**
1. Validator set changes via special transaction type `ValidatorRegistration`
2. Changes apply at epoch boundary
3. All nodes compute same validator set from finalized history

---

## Test Infrastructure

### Task T.1: Add Integration Test Suite
**Objective:** End-to-end test with multiple nodes

**Files:**
- Create: `Protocol_Core/qanto-node/tests/integration_testnet.rs`

**Scenario:**
1. Start 4 nodes with different keys
2. Connect via P2P
3. Mine blocks, verify finality
4. Test reorg resistance
5. Test difficulty retarget on hashrate drop
6. Test eclipse resistance (simulated)

---

### Task T.2: Chaos Testing Framework
**Objective:** Automated failure injection

**Files:**
- Create: `Protocol_Core/qanto-node/tests/chaos.rs`

**Tests:**
- SIGTERM during block production
- Network partition simulation
- Disk full during write
- Clock skew injection
- Byzantine validator behavior

---

## Execution Order (Dependency Graph)

```
Phase 0: Merge baseline
    │
    ├─→ A.1 Difficulty Retarget (enables A.2)
    │       │
    │       └─→ A.2 Remove Miner Backoff
    │
    ├─→ A.3 Finality Gadget (depends on A.1 for difficulty)
    │
    ├─→ A.4 Multi-Parent DAG (independent)
    │
    ├─→ A.5 Slashing (depends on A.3 for finality evidence)
    │
    ├─→ B.1 Dynamic Topics (independent)
    │
    ├─→ B.2 Peer Diversity (depends on B.1)
    │
    ├─→ B.3 Proactive Scoring (depends on B.2)
    │
    ├─→ B.4 Min Peer Count (depends on B.2)
    │
    ├─→ B.5 Early Validation (independent)
    │
    ├─→ C.1 P256 Removal (independent, high priority)
    │
    ├─→ C.2 Key Rotation (depends on C.1)
    │
    ├─→ D.1 RPC Auth (independent)
    │
    ├─→ D.2 WS Auth (depends on D.1)
    │
    ├─→ D.3 Bridge Disable (independent)
    │
    ├─→ E.1 Blocks Pruning (independent)
    │
    ├─→ E.2 Mempool Limits (independent)
    │
    ├─→ E.3 PQ Pre-filter (independent)
    │
    ├─→ E.4 State Pruning (depends on E.1)
    │
    ├─→ E.5 Snapshot Verify (depends on A.3)
    │
    ├─→ F.1 SAGA Determinism (independent)
    │
    └─→ G.1 Validator Consensus (depends on A.3, A.4)
```

---

## Parallel Execution Plan (8 Agents)

| Agent | Track | Tasks |
|-------|-------|-------|
| 1 | Consensus A | A.1, A.2 |
| 2 | Consensus A | A.3, A.5 |
| 3 | Consensus A | A.4, G.1 |
| 4 | Networking B | B.1, B.2, B.5 |
| 5 | Networking B | B.3, B.4 |
| 6 | Crypto C | C.1, C.2 |
| 7 | RPC D | D.1, D.2, D.3 |
| 8 | Storage E | E.1, E.2, E.3 |
| 9 | Storage E | E.4, E.5 |
| 10 | SAGA F | F.1 |
| 11 | Test T | T.1, T.2 |

---

## Acceptance Criteria for "SAFE FOR PRIVATE TESTNET"

All must pass:

- [ ] `cargo test --workspace --locked` — all 322+ tests pass
- [ ] `cargo test attack_` — all 12 attack tests demonstrate FIXED vulnerabilities
- [ ] Difficulty retarget activates on hashrate drop (integration test)
- [ ] Finality achieved at 2/3 quorum within 3 rounds (integration test)
- [ ] Multi-parent blocks accepted and validated (integration test)
- [ ] Equivocation slashing executes (test)
- [ ] P2P eclipse attack fails (test)
- [ ] P256 forge attempt fails (fuzz test)
- [ ] RPC unauthenticated requests rejected (test)
- [ ] Bridge claims permanently rejected (test)
- [ ] Blocks map memory bounded under load (test)
- [ ] Mempool per-peer limits enforced (test)
- [ ] Invalid PQ signatures fast-rejected (benchmark: <50μs pre-filter vs 1ms full)
- [ ] Snapshot verification rejects tampered state (test)
- [ ] No f64 in consensus path (grep verification)
- [ ] Validator set changes via consensus (test)
- [ ] 4-node testnet runs for 1000 blocks without halt/fork

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Finality gadget complex | High | High | Start with simple vote counting, iterate |
| Multi-parent DAG breaks UTXO merge | Medium | High | Implement UTXO merge carefully, test extensively |
| Difficulty retarget oscillation | Medium | Medium | Conservative bounds (±25%), monitor |
| P2P topic rotation breaks sync | Low | Medium | Gradual rollout, backward compat |
| P256 removal breaks EVM path | Low | Low | Feature flag, disable for testnet |
| SAGA removal affects economics | Low | Low | Containment mode uses fees only |

---

## Estimated Timeline

| Phase | Tasks | Est. Days |
|-------|-------|-----------|
| Phase 0 | Merge, baseline | 1 |
| Track A (Consensus) | A.1-A.5, G.1 | 10-14 |
| Track B (Networking) | B.1-B.5 | 7-10 |
| Track C (Crypto) | C.1-C.2 | 3-5 |
| Track D (RPC) | D.1-D.3 | 3-5 |
| Track E (Storage) | E.1-E.5 | 7-10 |
| Track F (SAGA) | F.1 | 2-3 |
| Integration Tests | T.1-T.2 | 5-7 |
| **Total** | | **38-54 days** |

With 8-11 parallel agents: **~10-14 calendar days** for P0 completion.

---

## Next Steps

1. **Immediate:** Execute Phase 0 merge and verify baseline
2. **Day 1-2:** Spawn agents for Tracks A.1, C.1, D.1 (highest impact, independent)
3. **Day 2-3:** Spawn agents for A.3, B.1, E.1
4. **Day 4+:** Continue per dependency graph
5. **Daily:** Run full test suite, track regression count
6. **Completion:** All acceptance criteria met → declare "SAFE FOR PRIVATE TESTNET"

---

*Plan generated per systematic-debugging and plan skills. Ready for subagent-driven-development execution.*