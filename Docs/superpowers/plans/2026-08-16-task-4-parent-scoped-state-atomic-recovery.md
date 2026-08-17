# Task 4: Implement Parent-Scoped State and Atomic Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement parent-scoped state management with atomic recovery capabilities in the QantoDAG, enabling branch-scoped execution, deterministic state roots, atomic block commits with WAL durability, and crash recovery with exact-once replay.

**Architecture:** 
- Add `state_parent` and `state_root` fields to `QantoBlock` (V2 protocol)
- Create `StateV2` storage layer with column families for state roots, UTXOs, balances, and undo logs
- Implement `StateTransition` planner that computes deltas against a parent state snapshot
- Add atomic commit path using `WriteBatch` + WAL with undo log for crash recovery
- Implement fork-choice aware reorg handling with apply/unapply from common ancestor
- Add recovery mechanism that replays from last finalized checkpoint

**Tech Stack:** 
- Rust (qanto-node, qanto-core, qanto-consensus-primitives)
- Custom storage engine (QantoStorage with WAL)
- Canonical V2 record encoding (CanonicalRecord, hash_v2, DomainV2)

## Global Constraints

- **V2 Protocol Only:** All new code must use `SchemaVersion::V2_INITIAL` and `DomainV2` domains
- **Atomicity Boundary:** Every state transition (UTXO + balance + block + tip + index) commits in ONE `WriteBatch` with WAL sync
- **Determinism:** State root calculation must be identical across all nodes (sorted address iteration)
- **No Partial Mutation:** In-memory state updates ONLY after durable batch succeeds; crash before = no change
- **Recovery = Replay:** WAL recovery must produce exact same state root as original commit
- **Parent-Scoped:** Validation executes against parent's immutable state snapshot + intra-block overlay
- **Fork Safety:** Reorg applies/unapplies deterministic deltas from LCA; no reorg crosses finalized checkpoint

---

### Task 4.1: Add State Parent and State Root to QantoBlock V2

**Files:**
- Modify: `Protocol_Core/qanto-consensus-primitives/src/v2_records.rs` (add state_root to BLOCK_CORE_SPECS, add state_parent to BLOCK_CORE_SPECS)
- Modify: `Protocol_Core/qanto-consensus-primitives/src/v2_records.rs` (extend `block_core_v2` function)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (QantoBlock struct: add `state_parent: String`, `state_root: String`)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (block_core_v2 call: pass actual state_root and state_parent)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (QantoBlock::compute_block_id: include state_root in canonical record)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (is_valid_block_v2: validate state_parent exists and state_root matches computed)

**Interfaces:**
- Consumes: `HashV2` for state_root, `BlockIdV2` for state_parent
- Produces: BlockCoreV2 record with 11 fields (version, parent_block_id, state_root, transactions_root, timestamp, difficulty, chain_work, validator_id, miner_id, height, state_parent)

- [ ] **Step 1: Write failing test for block_core_v2 with state fields**

```rust
// In Protocol_Core/qanto-consensus-primitives/tests/v2_block_core.rs
#[test]
fn test_block_core_v2_includes_state_parent_and_state_root() {
    let state_root = HashV2::from_bytes([1u8; 32]);
    let state_parent = BlockIdV2::from_bytes([2u8; 32]);
    let record = block_core_v2(
        2, // version
        BlockIdV2::from_bytes([3u8; 32]), // parent_block_id
        state_root,
        HashV2::from_bytes([4u8; 32]), // transactions_root
        1234567890, // timestamp
        1_000_000_000, // difficulty
        1000, // chain_work
        HashV2::from_bytes([5u8; 32]), // validator_id
        HashV2::from_bytes([6u8; 32]), // miner_id
        10, // height
    );
    // Add state_parent as field 11, state_root as field 3 (already there)
    let bytes = record.finish();
    let decoded = DecodedRecord::decode_with_schema(&bytes, BLOCK_CORE_SPECS).unwrap();
    assert_eq!(decoded.field(3).unwrap().as_hash32().unwrap(), state_root.as_bytes());
    assert_eq!(decoded.field(11).unwrap().as_hash32().unwrap(), state_parent.as_bytes());
}
```

- [ ] **Step 2: Run test to verify it fails**
```bash
cd /Users/cps/qanto && cargo test --package qanto-consensus-primitives -- test_block_core_v2_includes_state_parent_and_state_root -- --nocapture
```

- [ ] **Step 3: Modify BLOCK_CORE_SPECS and block_core_v2 in v2_records.rs**

```rust
// BLOCK_CORE_SPECS - add field 11 for state_parent (Hash32)
pub const BLOCK_CORE_SPECS: &[FieldSpec] = &[
    // ... existing fields 1-10 ...
    FieldSpec {
        id: 11,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // state_parent (BlockIdV2)
];

// block_core_v2 function - add state_parent parameter
pub fn block_core_v2(
    version: u8,
    parent_block_id: BlockIdV2,
    state_root: HashV2,
    transactions_root: HashV2,
    timestamp: u64,
    difficulty: u64,
    chain_work: u128,
    validator_id: HashV2,
    miner_id: HashV2,
    height: u64,
    state_parent: BlockIdV2, // NEW PARAMETER
) -> CanonicalRecord {
    // ... existing code ...
    record.push_hash32(11, state_parent.as_bytes()).unwrap(); // NEW FIELD
    record
}
```

- [ ] **Step 4: Run test to verify it passes**

- [ ] **Step 5: Update QantoBlock struct in qantodag.rs**

```rust
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct QantoBlock {
    // ... existing fields ...
    pub state_parent: String,   // BlockId of state parent
    pub state_root: String,     // Computed state root after block execution
    // ...
}
```

- [ ] **Step 6: Update block_core_v2 call in is_valid_block_v2 to pass actual state_parent**

- [ ] **Step 7: Update compute_block_id and signing_data to include state_root**

- [ ] **Step 8: Commit**

---

### Task 4.2: Implement StateV2 Storage Layer with Column Families

**Files:**
- Create: `Protocol_Core/qanto-core/src/state_v2.rs` (new module)
- Modify: `Protocol_Core/qanto-core/src/qanto_storage.rs` (add StateV2 column family support)
- Modify: `Protocol_Core/qanto-core/src/lib.rs` (export state_v2 module)

**Interfaces:**
- Consumes: `WriteBatch`, `QantoStorage`, parent state root
- Produces: `StateTransition` with computed state root, undo log, and delta

- [ ] **Step 1: Create StateV2 module with column family definitions**

```rust
// Protocol_Core/qanto-core/src/state_v2.rs
use crate::qanto_storage::{QantoStorage, WriteBatch, QantoStorageError};
use qanto_consensus_primitives::{hash_v2, HashV2, DomainV2, SchemaVersion};
use std::collections::HashMap;

/// Column family prefixes for StateV2
pub const STATE_ROOT_PREFIX: &[u8] = b"stateroot:";      // key: block_id -> value: state_root (32 bytes)
pub const UNDO_LOG_PREFIX: &[u8] = b"undolog:";          // key: block_id -> value: serialized undo operations
pub const STATE_SNAPSHOT_PREFIX: &[u8] = b"statesnap:";  // key: state_root -> value: serialized state snapshot (for fast lookup)
pub const VALIDATOR_SET_PREFIX: &[u8] = b"valset:";      // key: epoch -> value: validator set root
pub const BRIDGE_NULLIFIER_PREFIX: &[u8] = b"bridgenull:"; // key: nullifier -> value: block_id
pub const ISSUED_SUPPLY_PREFIX: &[u8] = b"issuance:";    // key: block_id -> value: issued supply state

/// Undo operation for atomic rollback
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UndoOp {
    Put { key: Vec<u8>, old_value: Option<Vec<u8>> },  // old_value = None means key didn't exist
    Delete { key: Vec<u8>, old_value: Vec<u8> },
}

/// State transition plan computed against a parent state
#[derive(Clone, Debug)]
pub struct StateTransition {
    pub parent_state_root: HashV2,
    pub new_state_root: HashV2,
    pub balance_deltas: HashMap<String, (u128, u128)>, // address -> (old_balance, new_balance)
    pub utxo_removals: Vec<String>,                     // UTXO IDs spent
    pub utxo_additions: Vec<(String, Vec<u8>)>,         // (UTXO ID, encoded UTXO)
    pub validator_set_changes: Option<Vec<u8>>,         // new validator set root
    pub issued_supply_delta: i128,                      // change in issued supply
    pub bridge_nullifiers: Vec<String>,                 // consumed bridge nullifiers
    pub undo_log: Vec<UndoOp>,                          // for exact-once recovery
    pub block_id: String,
}

/// StateV2 manager for branch-scoped state operations
pub struct StateV2 {
    storage: Arc<QantoStorage>,
}

impl StateV2 {
    pub fn new(storage: Arc<QantoStorage>) -> Self {
        Self { storage }
    }

    /// Compute state transition for a block against its state parent
    pub async fn compute_transition(
        &self,
        block: &QantoBlock,
        parent_state_root: HashV2,
        transactions: &[Transaction],
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        account_cache: &AccountStateCache,
    ) -> Result<StateTransition, QantoDAGError> {
        // TODO: Implement
        unimplemented!()
    }

    /// Apply a state transition atomically (durable + in-memory)
    pub async fn apply_transition(
        &self,
        transition: &StateTransition,
        block: &QantoBlock,
    ) -> Result<(), QantoStorageError> {
        // TODO: Implement
        unimplemented!()
    }

    /// Recover state from WAL - exact replay of durable transitions
    pub fn recover_from_wal(&self) -> Result<(), QantoStorageError> {
        // TODO: Implement
        unimplemented!()
    }

    /// Get state root for a block
    pub fn get_state_root(&self, block_id: &str) -> Result<Option<HashV2>, QantoStorageError> {
        // TODO: Implement
        unimplemented!()
    }

    /// Get undo log for a block (for reorg)
    pub fn get_undo_log(&self, block_id: &str) -> Result<Option<Vec<UndoOp>>, QantoStorageError> {
        // TODO: Implement
        unimplemented!()
    }
}
```

- [ ] **Step 2: Write failing test**

```rust
#[test]
fn test_state_v2_compute_transition() {
    let storage = QantoStorage::new(StorageConfig::default()).unwrap();
    let state_v2 = StateV2::new(Arc::new(storage));
    // Setup parent state, transactions
    // Call compute_transition
    // Verify new_state_root is computed correctly
}
```

- [ ] **Step 3: Implement compute_transition with deterministic state root calculation**

- [ ] **Step 4: Implement apply_transition with WriteBatch + undo log**

- [ ] **Step 5: Implement recover_from_wal**

- [ ] **Step 6: Implement get_state_root and get_undo_log**

- [ ] **Step 7: Run all state_v2 tests**

- [ ] **Step 8: Commit**

---

### Task 4.3: Integrate StateV2 into QantoDAG Block Processing

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (QantoDAG: add `state_v2: Arc<StateV2>`)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (new(): initialize StateV2)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (is_valid_block_v2: use StateV2::compute_transition)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add_block: use StateV2::apply_transition instead of manual WriteBatch)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add_block: remove manual balance/UTXO updates after durable batch)

**Interfaces:**
- Consumes: `StateTransition`, `QantoBlock`, `parent_state_root`
- Produces: Updated in-memory state, durable storage commit

- [ ] **Step 1: Add StateV2 to QantoDAG struct**

```rust
pub struct QantoDAG {
    // ... existing fields ...
    pub state_v2: Arc<StateV2>,  // NEW
}
```

- [ ] **Step 2: Initialize StateV2 in QantoDAG::new()**

```rust
let state_v2 = Arc::new(StateV2::new(db_arc.clone()));
Self {
    // ... existing fields ...
    state_v2,
    // ...
}
```

- [ ] **Step 3: Modify is_valid_block_v2 to compute state transition**

```rust
async fn is_valid_block_v2(&self, block: &QantoBlock, utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>) -> Result<bool, QantoDAGError> {
    // ... existing validation ...
    
    // Get parent state root
    let parent_state_root = if block.height == 0 {
        HashV2::ZERO
    } else {
        self.state_v2.get_state_root(&block.state_parent)?
            .ok_or_else(|| QantoDAGError::InvalidBlock("State parent root not found".to_string()))?
    };
    
    // Compute transition
    let transition = self.state_v2.compute_transition(
        block,
        parent_state_root,
        &block.transactions,
        utxos_arc,
        &self.account_state_cache,
    ).await?;
    
    // Verify computed state root matches block's declared state_root
    let declared_root = HashV2::from_bytes(hex::decode(&block.state_root)?.try_into()?);
    if transition.new_state_root != declared_root {
        return Err(QantoDAGError::InvalidBlock("State root mismatch".to_string()));
    }
    
    Ok(true)
}
```

- [ ] **Step 4: Modify add_block to use apply_transition**

```rust
pub async fn add_block(
    &self,
    block: QantoBlock,
    utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    mempool_arc: Option<&Arc<RwLock<Mempool>>>,
    miner_id: Option<&str>,
) -> Result<bool, QantoDAGError> {
    let _lock = self.consensus_lock.lock().await;
    
    // ... validation ...
    
    // Compute transition (same as validation, but we need the transition for apply)
    let parent_state_root = if block.height == 0 {
        HashV2::ZERO
    } else {
        self.state_v2.get_state_root(&block.state_parent)?
            .ok_or_else(|| QantoDAGError::InvalidBlock("State parent root not found".to_string()))?
    };
    
    let transition = self.state_v2.compute_transition(
        &block,
        parent_state_root,
        &block.transactions,
        utxos_arc,
        &self.account_state_cache,
    ).await?;
    
    // Apply atomically (WAL + in-memory)
    self.state_v2.apply_transition(&transition, &block).await?;
    
    // Update in-memory indexes (no balance/UTXO updates here - done by apply_transition)
    self.blocks.insert(block.id.clone(), block.clone());
    self.index_block_internal(&block);
    // ... update tips, forks, metrics ...
    
    Ok(true)
}
```

- [ ] **Step 5: Remove manual WriteBatch construction from add_block**

- [ ] **Step 6: Run integration tests**

- [ ] **Step 7: Commit**

---

### Task 4.4: Implement Fork-Choice Aware Reorg with Apply/Unapply

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add `reorg_to_chain` method)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add `find_common_ancestor` method)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add `apply_delta` and `unapply_delta` helpers)
- Add: `Protocol_Core/qanto-node/src/fork_choice.rs` (new module for fork choice logic)

**Interfaces:**
- Consumes: old tip, new tip, finalized checkpoint
- Produces: Canonical chain with consistent state root

- [ ] **Step 1: Create fork_choice.rs module**

```rust
// Protocol_Core/qanto-node/src/fork_choice.rs
use crate::qantodag::{QantoDAG, QantoBlock, QantoDAGError};
use qanto_consensus_primitives::HashV2;
use std::collections::HashSet;

/// Fork choice weight strategy: chain work (cumulative PoW)
pub fn calculate_chain_work(dag: &QantoDAG, tip_id: &str) -> Result<U256, QantoDAGError> {
    let mut work = U256::ZERO;
    let mut current = tip_id;
    while let Some(block) = dag.blocks.get(current) {
        work += block.chain_work;
        if block.height == 0 { break; }
        if block.parents.is_empty() { break; }
        current = &block.parents[0]; // state parent
    }
    Ok(work)
}

/// Find LCA of two chain tips following state_parent links
pub fn find_lca(
    dag: &QantoDAG,
    tip_a: &str,
    tip_b: &str,
    finalized_checkpoint: &str,
) -> Result<String, QantoDAGError> {
    // Walk both chains to same height, then step together
    // Stop if we reach finalized_checkpoint
    unimplemented!()
}

/// Apply delta from block to state
pub async fn apply_delta(
    dag: &QantoDAG,
    block: &QantoBlock,
    utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
) -> Result<(), QantoDAGError> {
    unimplemented!()
}

/// Unapply delta from block to state (reverse)
pub async fn unapply_delta(
    dag: &QantoDAG,
    block: &QantoBlock,
    utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
) -> Result<(), QantoDAGError> {
    unimplemented!()
}

/// Reorg from old_tip to new_tip, applying/unapplying from LCA
pub async fn reorg_to_chain(
    dag: &QantoDAG,
    old_tip: &str,
    new_tip: &str,
    finalized_checkpoint: &str,
    utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
) -> Result<(), QantoDAGError> {
    let lca = find_lca(dag, old_tip, new_tip, finalized_checkpoint)?;
    
    // Collect blocks to unapply (from old_tip down to LCA exclusive)
    let mut unapply = Vec::new();
    let mut current = old_tip;
    while current != lca {
        let block = dag.blocks.get(current).ok_or(...)?;
        unapply.push(block.clone());
        if block.height == 0 { break; }
        if block.parents.is_empty() { break; }
        current = block.parents[0].clone(); // state parent
    }
    
    // Collect blocks to apply (from new_tip down to LCA exclusive, then reverse)
    let mut apply = Vec::new();
    current = new_tip;
    while current != lca {
        let block = dag.blocks.get(current).ok_or(...)?;
        apply.push(block.clone());
        if block.height == 0 { break; }
        if block.parents.is_empty() { break; }
        current = block.parents[0].clone();
    }
    apply.reverse();
    
    // Unapply in reverse order (from LCA down to old_tip)
    for block in unapply.iter().rev() {
        unapply_delta(dag, block, utxos_arc).await?;
    }
    
    // Apply in order (from LCA down to new_tip)
    for block in apply {
        apply_delta(dag, &block, utxos_arc).await?;
    }
    
    Ok(())
}
```

- [ ] **Step 2: Write failing test for reorg**

```rust
#[tokio::test]
async fn test_reorg_apply_unapply() {
    // Create DAG with fork
    // Mine blocks on two branches
    // Reorg from branch A to branch B
    // Verify final state root matches replay from genesis
}
```

- [ ] **Step 3: Implement apply_delta using StateV2 undo log**

- [ ] **Step 4: Implement unapply_delta using StateV2 undo log (reverse operations)**

- [ ] **Step 5: Implement find_lca with finalized checkpoint boundary**

- [ ] **Step 6: Implement reorg_to_chain**

- [ ] **Step 7: Run tests**

- [ ] **Step 8: Commit**

---

### Task 4.5: Implement Crash Recovery with Exact-Once Replay

**Files:**
- Modify: `Protocol_Core/qanto-core/src/qanto_storage.rs` (enhance `recover_from_wal`)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add `recover_state` method)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (call recovery on startup if needed)

**Interfaces:**
- Consumes: WAL entries, last finalized checkpoint
- Produces: Consistent state with correct state roots

- [ ] **Step 1: Enhance WAL recovery in qanto_storage.rs**

```rust
// In QantoStorage::recover_from_wal()
fn recover_from_wal(&mut self) -> Result<(), QantoStorageError> {
    // Read WAL sequentially
    // For each Transaction + Commit pair:
    //   Replay Put/Delete operations to memtable
    //   Verify state root matches checkpoint
    // Skip incomplete transactions (no Commit marker)
    // Return recovered state
    unimplemented!()
}
```

- [ ] **Step 2: Add recovery method to QantoDAG**

```rust
pub async fn recover_state(&self) -> Result<(), QantoDAGError> {
    // 1. Find last finalized checkpoint
    let finalized = self.finalized_blocks.iter()
        .filter(|entry| *entry.value())
        .max_by_key(|entry| {
            self.blocks.get(entry.key()).map(|b| b.height).unwrap_or(0)
        });
    
    // 2. Replay from checkpoint using WAL
    // 3. Verify state roots match at each step
    // 4. Rebuild in-memory indexes
    unimplemented!()
}
```

- [ ] **Step 3: Call recovery in QantoDAG::new() if WAL has uncommitted data**

- [ ] **Step 4: Write test for crash recovery**

```rust
#[test]
fn test_crash_recovery_exact_once() {
    // Create DAG, add blocks
    // Simulate crash (drop without close)
    // Reopen DAG
    // Verify state root matches before crash
    // Verify no duplicate application
}
```

- [ ] **Step 5: Run test**

- [ ] **Step 6: Commit**

---

### Task 4.6: Add Block Finality Checkpoint Integration

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (add `finalize_block` method)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (track finalized checkpoint in storage)
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs` (prevent reorg crossing finalized checkpoint)

**Interfaces:**
- Consumes: `FinalityCertificate`, `BlockId`
- Produces: Finalized checkpoint marker

- [ ] **Step 1: Add finalize_block method**

```rust
pub async fn finalize_block(
    &self,
    block_id: &str,
    certificate: &FinalityCertificate,
) -> Result<(), QantoDAGError> {
    // Verify certificate
    // Mark block as finalized
    // Write finalized checkpoint to storage
    // Prune undo logs before checkpoint (optional)
    unimplemented!()
}
```

- [ ] **Step 2: Modify reorg_to_chain to respect finalized checkpoint**

- [ ] **Step 3: Add finalized checkpoint to storage schema**

- [ ] **Step 4: Test finality prevents reorg**

- [ ] **Step 5: Commit**

---

### Task 4.7: Property Tests for State Consistency

**Files:**
- Create: `Protocol_Core/qanto-node/tests/state_consistency.rs`
- Create: `Protocol_Core/qanto-consensus-primitives/tests/state_v2_property.rs`

**Tests:**
- [ ] **Permutation test**: Same blocks in different arrival orders → same canonical path + state root
- [ ] **Competing double-spend branches**: Only one succeeds, state consistent
- [ ] **Duplicate parents / extra roots**: Rejected cleanly
- [ ] **Deep reorgs**: Apply/unapply from LCA produces correct root
- [ ] **Maximum-width DAGs**: Stress test
- [ ] **Crash injection**: At every atomic boundary, recover and verify root + supply
- [ ] **Snapshot-to-full-replay**: Root equality

- [ ] **Step 1: Write permutation test**

```rust
#[tokio::test]
async fn test_state_consistency_permutations() {
    let blocks = generate_valid_blocks(10);
    for _ in 0..100 {
        let mut shuffled = blocks.clone();
        shuffled.shuffle(&mut rand::thread_rng());
        let dag = create_dag();
        let results = dag.process_blocks_optimized(shuffled, &utxos).await?;
        let final_root = dag.account_state_cache.calculate_state_root();
        // All permutations must yield same final state root
        assert_eq!(final_root, EXPECTED_ROOT);
    }
}
```

- [ ] **Step 2: Write crash injection test**

```rust
#[test]
fn test_crash_at_atomic_boundaries() {
    for boundary in 0..NUM_BOUNDARIES {
        // Setup
        // Execute up to boundary
        // Simulate crash (kill process)
        // Recover
        // Verify root == expected
    }
}
```

- [ ] **Step 3: Run all property tests**

- [ ] **Step 4: Commit**

---

### Task 4.8: Integration Tests and End-to-End Verification

**Files:**
- Create: `Protocol_Core/qanto-node/tests/parent_scoped_state.rs`

**Tests:**
- [ ] **Full node sync**: Multiple nodes from genesis → same state roots
- [ ] **Network partition + heal**: State converges
- [ ] **Restart + replay**: Exact state root equality
- [ ] **Multi-chain**: Parallel chains maintain independent state roots

- [ ] **Step 1: Write integration tests**

- [ ] **Step 2: Run with nextest**

- [ ] **Step 3: Verify all workspace tests pass**

- [ ] **Step 4: Commit**

---

### Task 4.9: Cleanup and Documentation

- [ ] **Step 1: Remove deprecated legacy state paths** (V1 global state mutation code paths in add_block)
- [ ] **Step 2: Run cargo fmt --all -- --check**
- [ ] **Step 3: Run cargo clippy --workspace --locked -- -D warnings**
- [ ] **Step 4: Run cargo test --workspace --locked**
- [ ] **Step 5: Update CLAUDE.md with new StateV2 architecture notes**
- [ ] **Step 6: Final commit**

---

## Verification Checklist

After completing all tasks, verify:

- [ ] `cargo check --workspace --locked` → 0 errors
- [ ] `cargo fmt --all -- --check` → clean
- [ ] `cargo clippy --workspace --locked -- -D warnings` → clean
- [ ] `cargo nextest run --workspace --locked` → all tests pass
- [ ] State roots are deterministic across permutations
- [ ] Crash recovery produces exact same state root
- [ ] Reorg apply/unapply from LCA is correct
- [ ] Finality checkpoint prevents reorg across boundary
- [ ] All V2 protocol fields (state_parent, state_root) in blocks
- [ ] Atomic WriteBatch includes: block + tip + UTXOs + balances + state_root + undo_log + issuance + validator_set + bridge_nullifiers

---

**Plan complete and saved to `docs/superpowers/plans/2026-08-16-task-4-parent-scoped-state-atomic-recovery.md`.**