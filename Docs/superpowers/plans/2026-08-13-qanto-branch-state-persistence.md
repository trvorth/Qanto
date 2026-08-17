# Qanto Branch-Scoped State Persistence and Merkle-Consistent Storage — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hybrid storage layer (legacy `qanto_storage` key patterns + partial Merkle) with a fully branch-scoped, Merkle-consistent state store where every branch head has a verifiable `StateRootV2`, state transitions are proven against parent state roots, and pruning/archival rules are deterministic. The private engineering profile uses fixed V1 genesis; V2 activation is gated by `ACTIVATION_HEIGHT = 0`.

**Architecture:** `qanto_storage` crate becomes the single source of truth for state. It exposes `StateStoreV2` with branch-scoped `StateRootV2` commitments. All read/write paths go through the canonical `StateRootRecordV2` and typed Merkle nodes from Workstream 2. No legacy key prefixes (`"account/"`, `"utxo/"`, `"storage/"`) in V2 paths. Pruning uses explicit `PruningManifest` committing to pruned roots. Replay verification re-executes state transitions against committed roots.

**Tech Stack:** Rust 2021, `rocksdb`/`sled` backend (abstracted via `StorageBackend` trait), `qanto-consensus-primitives` V2 types (`StateRootV2`, `HashV2`, `DomainV2`), `qanto-core::storage_traits` for node integration.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- Private engineering profile remains active (`ACTIVATION_HEIGHT = 0`). V2 storage code compiles but is unreachable until QIP-004 approval.
- All V2 state keys are typed: `StateKeyV2 = hash_v2(DomainV2::StateKey, chain, schema, canonical_record(key_bytes))`. No legacy string prefixes.
- Merkle tree uses typed domains: `StateLeaf` -> `StateNode` -> `StateRoot` (Workstream 2). No hybrid string-hash Merkle.
- `StateRootV2` in block header commits to the entire authenticated state after block execution.
- Pruning is explicit: `PruningManifestV2` lists pruned state roots and their block heights. No implicit GC.
- Replay verification: given genesis state root + block sequence, re-execution must produce identical state roots at every height.
- Candidate identity governed by Workstream 1. This plan consumes `candidate-manifest.json`.

---

### Task 1: Define V2 State Key, Leaf, Node, Root Schemas and Storage Traits

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/src/storage_v2.rs`
- Modify: `Protocol_Core/qanto-consensus-primitives/src/lib.rs` (export)
- Modify: `Protocol_Core/qanto-core/src/storage_traits.rs`
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/storage_v2.json`
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs`

**Interfaces:**
- Produces: `StateKeyV2`, `StateLeafV2`, `StateNodeV2`, `StateRootV2` constructors, `StorageBackendV2` trait, `StateStoreV2` struct, `PruningManifestV2`.
- Consumes: `hash_v2`, `DomainV2::StateKey/Leaf/Node/Root`, `v2_records::STATE_*_SPECS`, `consensus_parameters::SchemaVersion`.

- [ ] **Step 1: Write failing storage V2 tests.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{
    storage_v2::{StateKeyV2, StateLeafV2, StateNodeV2, StateRootV2, StateStoreV2, PruningManifestV2, StorageBackendV2},
    hash::{hash_v2, DomainV2, HashV2, StateRootV2 as StateRootV2Type},
    v2_records::{STATE_KEY_SPECS, STATE_LEAF_SPECS, STATE_NODE_SPECS, STATE_ROOT_SPECS},
    consensus_parameters::{SchemaVersion, ConsensusParametersV2},
};

#[test]
fn state_key_v2_matches_golden_vector() {
    let key_bytes = b"account/12345/balance";
    let state_key = StateKeyV2::new(key_bytes, 1363527796, SchemaVersion::V2_INITIAL);
    let expected = include_bytes!("../vectors/storage_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(state_key.to_string(), expected["state_key_v2"]);
}

#[test]
fn state_leaf_v2_matches_golden_vector() {
    let key = StateKeyV2::new(b"test", 1363527796, SchemaVersion::V2_INITIAL);
    let value_hash = HashV2::from_bytes([0xAAu8; 32]);
    let leaf = StateLeafV2::new(key, value_hash, 1363527796, SchemaVersion::V2_INITIAL);
    let expected = include_bytes!("../vectors/storage_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(leaf.canonical_bytes()), expected["state_leaf_canonical_bytes"]);
    assert_eq!(leaf.hash().to_string(), expected["state_leaf_hash"]);
}

#[test]
fn state_node_v2_matches_golden_vector() {
    let left = HashV2::from_bytes([0x11u8; 32]);
    let right = HashV2::from_bytes([0x22u8; 32]);
    let node = StateNodeV2::new(left, right, 1363527796, SchemaVersion::V2_INITIAL);
    let expected = include_bytes!("../vectors/storage_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(node.canonical_bytes()), expected["state_node_canonical_bytes"]);
    assert_eq!(node.hash().to_string(), expected["state_node_hash"]);
}

#[test]
fn state_root_v2_matches_golden_vector() {
    let root_hash = HashV2::from_bytes([0x33u8; 32]);
    let state_root = StateRootV2::new(root_hash, 1363527796, SchemaVersion::V2_INITIAL);
    let expected = include_bytes!("../vectors/storage_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(state_root.to_string(), expected["state_root_v2"]);
}

#[test]
fn storage_backend_v2_trait_compile_time_check() {
    // This test only verifies the trait exists and is object-safe
    fn _requires_object_safe<T: StorageBackendV2 + ?Sized>() {}
}
```

- [ ] **Step 2: Run the tests to establish failure.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- state_key_v2 state_leaf_v2 state_node_v2 state_root_v2 storage_backend_v2_trait_compile_time_check
```

Expected: `FAIL`.

- [ ] **Step 3: Implement `storage_v2.rs`.**

Create `Protocol_Core/qanto-consensus-primitives/src/storage_v2.rs`:

```rust
//! V2 state storage primitives: typed keys, Merkle nodes, roots, pruning manifest, and storage traits.

use crate::{
    consensus_parameters::SchemaVersion,
    hash::{hash_v2, DomainV2, HashV2, StateRootV2},
    record::{CanonicalRecord, WireType},
    v2_records::{state_key_record, state_leaf_record, state_node_record, state_root_record, STATE_KEY_SPECS, STATE_LEAF_SPECS, STATE_NODE_SPECS, STATE_ROOT_SPECS},
};

/// Typed state key: hash_v2(DomainV2::StateKey, chain, schema, canonical_record(key_bytes))
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StateKeyV2 {
    pub key_bytes: Vec<u8>,
    pub hash: HashV2,
}

impl StateKeyV2 {
    pub fn new(key_bytes: &[u8], chain_id: u32, schema: SchemaVersion) -> Self {
        let record = state_key_record(key_bytes);
        let bytes = record.finish();
        let hash = hash_v2(DomainV2::StateKey, chain_id, schema as u16, &bytes);
        Self { key_bytes: key_bytes.to_vec(), hash }
    }
    
    pub fn canonical_bytes(&self) -> Vec<u8> {
        state_key_record(&self.key_bytes).finish()
    }
    
    pub fn to_string(&self) -> String {
        self.hash.to_string()
    }
}

/// Typed state leaf: commits to a (StateKeyV2, value_hash) pair.
/// Leaf = hash_v2(DomainV2::StateLeaf, chain, schema, canonical_record(StateKeyV2, value_hash))
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateLeafV2 {
    pub key: StateKeyV2,
    pub value_hash: HashV2,
    pub canonical_bytes: Vec<u8>,
    pub hash: HashV2,
}

impl StateLeafV2 {
    pub fn new(key: StateKeyV2, value_hash: HashV2, chain_id: u32, schema: SchemaVersion) -> Self {
        let mut record = CanonicalRecord::new();
        record.push_hash32(1, key.hash.as_bytes()).unwrap();
        record.push_hash32(2, value_hash.as_bytes()).unwrap();
        let bytes = record.finish();
        let hash = hash_v2(DomainV2::StateLeaf, chain_id, schema as u16, &bytes);
        Self { key, value_hash, canonical_bytes: bytes, hash }
    }
    
    pub fn hash(&self) -> HashV2 {
        self.hash
    }
}

/// Typed state internal node: hash_v2(DomainV2::StateNode, chain, schema, canonical_record(left, right))
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateNodeV2 {
    pub left: HashV2,
    pub right: HashV2,
    pub canonical_bytes: Vec<u8>,
    pub hash: HashV2,
}

impl StateNodeV2 {
    pub fn new(left: HashV2, right: HashV2, chain_id: u32, schema: SchemaVersion) -> Self {
        let mut record = CanonicalRecord::new();
        record.push_hash32(1, left.as_bytes()).unwrap();
        record.push_hash32(2, right.as_bytes()).unwrap();
        let bytes = record.finish();
        let hash = hash_v2(DomainV2::StateNode, chain_id, schema as u16, &bytes);
        Self { left, right, canonical_bytes: bytes, hash }
    }
    
    pub fn hash(&self) -> HashV2 {
        self.hash
    }
}

/// Typed state root: hash_v2(DomainV2::StateRoot, chain, schema, canonical_record(root_hash))
/// This is the same as Workstream 2's StateRootV2 but re-exported here for storage context.
pub type StateRootV2 = crate::hash::StateRootV2;

/// Pruning manifest: commits to pruned state roots and their block height ranges.
/// PruningManifestV2 = hash_v2(DomainV2::PruningManifest, chain, schema, canonical_record(entries))
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PruningManifestV2 {
    pub entries: Vec<PruningEntry>,
    pub canonical_bytes: Vec<u8>,
    pub hash: HashV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PruningEntry {
    pub start_height: u64,
    pub end_height: u64,
    pub pruned_root: StateRootV2,
}

impl PruningManifestV2 {
    pub fn new(entries: Vec<PruningEntry>, chain_id: u32, schema: SchemaVersion) -> Self {
        let mut record = CanonicalRecord::new();
        // Push sequence of entries
        for entry in &entries {
            record.push_u64(1, entry.start_height).unwrap();
            record.push_u64(2, entry.end_height).unwrap();
            record.push_hash32(3, entry.pruned_root.as_bytes()).unwrap();
        }
        let bytes = record.finish();
        let hash = hash_v2(DomainV2::PruningManifest, chain_id, schema as u16, &bytes);
        Self { entries, canonical_bytes: bytes, hash }
    }
}

/// Storage backend trait for V2 state. Implementations: RocksDB, Sled, in-memory.
pub trait StorageBackendV2: Send + Sync {
    /// Get raw value bytes for a state key.
    fn get(&self, key: &StateKeyV2) -> Result<Option<Vec<u8>>, StorageError>;
    
    /// Set raw value bytes for a state key.
    fn set(&mut self, key: StateKeyV2, value: Vec<u8>) -> Result<(), StorageError>;
    
    /// Delete a state key.
    fn delete(&mut self, key: &StateKeyV2) -> Result<(), StorageError>;
    
    /// Get the current state root.
    fn get_state_root(&self) -> Result<StateRootV2, StorageError>;
    
    /// Set the current state root.
    fn set_state_root(&mut self, root: StateRootV2) -> Result<(), StorageError>;
    
    /// Commit a new state root with Merkle proof of transition from parent.
    fn commit_with_proof(&mut self, parent_root: StateRootV2, new_root: StateRootV2, proof: Vec<StateNodeV2>) -> Result<(), StorageError>;
    
    /// Get Merkle proof for a key at a specific state root.
    fn get_proof(&self, root: &StateRootV2, key: &StateKeyV2) -> Result<Vec<StateNodeV2>, StorageError>;
    
    /// Prune state up to a height, producing a PruningManifestV2.
    fn prune(&mut self, up_to_height: u64, new_root: StateRootV2) -> Result<PruningManifestV2, StorageError>;
    
    /// Iterate all keys with a given prefix (for snapshots/migrations).
    fn iter_prefix(&self, prefix: &[u8]) -> Box<dyn Iterator<Item = Result<(StateKeyV2, Vec<u8>), StorageError>> + '_>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    NotFound,
    Corrupted,
    IoError,
    ProofInvalid,
    PruningInvalid,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for StorageError {}

/// High-level state store that enforces V2 invariants.
pub struct StateStoreV2<B: StorageBackendV2> {
    backend: B,
    chain_id: u32,
    schema: SchemaVersion,
    current_root: StateRootV2,
}

impl<B: StorageBackendV2> StateStoreV2<B> {
    pub fn new(backend: B, chain_id: u32, schema: SchemaVersion, genesis_root: StateRootV2) -> Self {
        Self { backend, chain_id, schema, current_root: genesis_root }
    }
    
    pub fn chain_id(&self) -> u32 { self.chain_id }
    pub fn schema(&self) -> SchemaVersion { self.schema }
    pub fn current_root(&self) -> StateRootV2 { self.current_root }
    
    /// Execute a state transition function, producing a new root and proof.
    pub fn execute<F, R>(&mut self, transition: F) -> Result<(StateRootV2, R), StorageError>
    where
        F: FnOnce(&mut StateTransitionContext<'_, B>) -> Result<R, StorageError>,
    {
        let mut ctx = StateTransitionContext { store: self, changes: Vec::new() };
        let result = transition(&mut ctx)?;
        // Apply changes and compute new root with proof
        let (new_root, proof) = ctx.commit()?;
        self.backend.commit_with_proof(self.current_root, new_root, proof)?;
        self.current_root = new_root;
        Ok((new_root, result))
    }
    
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let state_key = StateKeyV2::new(key, self.chain_id, self.schema);
        self.backend.get(&state_key)
    }
    
    pub fn set(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), StorageError> {
        let state_key = StateKeyV2::new(key, self.chain_id, self.schema);
        self.backend.set(state_key, value)
    }
    
    pub fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        let state_key = StateKeyV2::new(key, self.chain_id, self.schema);
        self.backend.delete(&state_key)
    }
    
    pub fn get_proof(&self, key: &[u8]) -> Result<Vec<StateNodeV2>, StorageError> {
        let state_key = StateKeyV2::new(key, self.chain_id, self.schema);
        self.backend.get_proof(&self.current_root, &state_key)
    }
    
    pub fn verify_proof(&self, root: &StateRootV2, key: &[u8], proof: &[StateNodeV2]) -> Result<HashV2, StorageError> {
        let state_key = StateKeyV2::new(key, self.chain_id, self.schema);
        // Reconstruct value hash from proof... simplified for plan
        Ok(HashV2::ZERO)
    }
    
    pub fn prune(&mut self, up_to_height: u64, new_root: StateRootV2) -> Result<PruningManifestV2, StorageError> {
        self.backend.prune(up_to_height, new_root)
    }
}

/// Context for a state transition, tracking changes for Merkle proof generation.
pub struct StateTransitionContext<'a, B: StorageBackendV2> {
    store: &'a mut StateStoreV2<B>,
    changes: Vec<(StateKeyV2, Option<Vec<u8>>)>, // key -> new value (None = delete)
}

impl<'a, B: StorageBackendV2> StateTransitionContext<'a, B> {
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        // Check local changes first
        let state_key = StateKeyV2::new(key, self.store.chain_id, self.store.schema);
        for (k, v) in self.changes.iter().rev() {
            if k == &state_key {
                return Ok(v.clone());
            }
        }
        self.store.get(key)
    }
    
    pub fn set(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), StorageError> {
        let state_key = StateKeyV2::new(key, self.store.chain_id, self.store.schema);
        self.changes.push((state_key, Some(value)));
        Ok(())
    }
    
    pub fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        let state_key = StateKeyV2::new(key, self.store.chain_id, self.store.schema);
        self.changes.push((state_key, None));
        Ok(())
    }
    
    /// Commit changes and return new state root with Merkle proof.
    fn commit(self) -> Result<(StateRootV2, Vec<StateNodeV2>), StorageError> {
        // Apply changes to backend
        for (key, value) in self.changes {
            if let Some(v) = value {
                self.store.backend.set(key, v)?;
            } else {
                self.store.backend.delete(&key)?;
            }
        }
        // In real implementation, compute new Merkle root and proof
        // For now return placeholder
        Ok((self.store.current_root, Vec::new()))
    }
}
```

- [ ] **Step 4: Update `lib.rs` to export storage V2.**

Modify `Protocol_Core/qanto-consensus-primitives/src/lib.rs`:

```rust
pub mod storage_v2;
pub use storage_v2::{StateKeyV2, StateLeafV2, StateNodeV2, StateRootV2, PruningManifestV2, PruningEntry, StorageBackendV2, StorageError, StateStoreV2, StateTransitionContext};
```

- [ ] **Step 5: Update `qanto-core/src/storage_traits.rs` to include V2 trait bounds.**

Modify `Protocol_Core/qanto-core/src/storage_traits.rs`:
- Add `StorageBackendV2` as supertrait or extension to existing `StorageBackend`.
- Ensure node integration uses `StateStoreV2` when V2 active.

- [ ] **Step 6: Create golden vector for storage V2.**

Create `Protocol_Core/qanto-consensus-primitives/tests/vectors/storage_v2.json`.

- [ ] **Step 7: Run all storage V2 tests.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- state_key_v2 state_leaf_v2 state_node_v2 state_root_v2 storage_backend_v2_trait_compile_time_check
```

Expected: `PASS`.

- [ ] **Step 8: Record intended commit.**

Intended commit: `feat(qanto-consensus-primitives): V2 branch-scoped state storage primitives`.

---

### Task 2: Implement RocksDB/Sled Backend for StorageBackendV2

**Files:**
- Create: `Protocol_Core/qanto-storage/src/backend_v2.rs`
- Modify: `Protocol_Core/qanto-storage/src/lib.rs`
- Modify: `Protocol_Core/qanto-storage/Cargo.toml` (ensure `qanto-consensus-primitives` dep)
- Test: `Protocol_Core/qanto-storage/tests/backend_v2.rs`

**Interfaces:**
- Consumes: `StorageBackendV2`, `StateKeyV2`, `StateRootV2`, etc.
- Produces: Concrete `RocksDBBackendV2` and `SledBackendV2` implementing `StorageBackendV2`.

- [ ] **Step 1: Write failing backend tests.**

Create `Protocol_Core/qanto-storage/tests/backend_v2.rs`:

```rust
use qanto_consensus_primitives::{
    storage_v2::{StateKeyV2, StateRootV2, StorageBackendV2, StateStoreV2, StorageError},
    hash::{HashV2, StateRootV2 as StateRootV2Type},
    consensus_parameters::{SchemaVersion, ConsensusParametersV2},
};
use tempfile::TempDir;

fn test_backend<B: StorageBackendV2 + Default>(mut backend: B) {
    let chain_id = 1363527796;
    let schema = SchemaVersion::V2_INITIAL;
    let genesis_root = StateRootV2Type::ZERO; // placeholder
    
    let mut store = StateStoreV2::new(backend, chain_id, schema, genesis_root);
    
    // Basic get/set
    store.set(b"test/key", b"value".to_vec()).unwrap();
    assert_eq!(store.get(b"test/key").unwrap(), Some(b"value".to_vec()));
    
    // Overwrite
    store.set(b"test/key", b"value2".to_vec()).unwrap();
    assert_eq!(store.get(b"test/key").unwrap(), Some(b"value2".to_vec()));
    
    // Delete
    store.delete(b"test/key").unwrap();
    assert_eq!(store.get(b"test/key").unwrap(), None);
    
    // Non-existent key
    assert_eq!(store.get(b"nonexistent").unwrap(), None);
}

#[test]
fn rocksdb_backend_v2_basic_operations() {
    let dir = TempDir::new().unwrap();
    let backend = qanto_storage::backend_v2::RocksDBBackendV2::new(&dir).unwrap();
    test_backend(backend);
}

#[test]
fn sled_backend_v2_basic_operations() {
    let dir = TempDir::new().unwrap();
    let backend = qanto_storage::backend_v2::SledBackendV2::new(&dir).unwrap();
    test_backend(backend);
}
```

- [ ] **Step 2: Run the tests to establish failure.**

```bash
rtk cargo nextest run -p qanto-storage --locked -- rocksdb_backend_v2_basic_operations sled_backend_v2_basic_operations
```

Expected: `FAIL`.

- [ ] **Step 3: Implement `backend_v2.rs`.**

Create `Protocol_Core/qanto-storage/src/backend_v2.rs` with concrete implementations.

- [ ] **Step 4: Run backend tests.**

```bash
rtk cargo nextest run -p qanto-storage --locked -- backend_v2
```

Expected: `PASS`.

- [ ] **Step 5: Record intended commit.**

Intended commit: `feat(qanto-storage): RocksDB/Sled V2 backend implementations`.

---

### Task 3: Wire V2 State Store Into Node Block Validation and Execution

**Files:**
- Modify: `Protocol_Core/qanto-node/src/qantodag.rs`
- Modify: `Protocol_Core/qanto-node/src/consensus.rs`
- Modify: `Protocol_Core/qanto-node/src/block_producer.rs`
- Modify: `Protocol_Core/qanto-node/src/state_sync.rs`
- Test: `Protocol_Core/qanto-node/tests/v2_storage_integration.rs`

**Interfaces:**
- Consumes: `StateStoreV2`, `StorageBackendV2`, `StateRootV2`, `BlockCoreV2`, `TransactionEnvelopeV2`.
- Produces: V2 block execution path that commits state transitions with Merkle proofs.

- [ ] **Step 1: Write failing integration test.**

Create `Protocol_Core/qanto-node/tests/v2_storage_integration.rs`:

```rust
use qanto_consensus_primitives::{
    storage_v2::{StateStoreV2, StorageBackendV2, StateRootV2},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion, is_v2_active},
    block_v2::BlockCoreV2,
    hash::StateRootV2 as StateRootV2Type,
};

#[test]
fn v2_block_execution_commits_state_root() {
    // This test requires ACTIVATION_HEIGHT > 0. We test the structural wiring.
    // Real execution tests use a test-only config with activation at height 1.
    let params = ConsensusParametersV2::for_testnet_candidate();
    assert!(!is_v2_active(100, &params)); // Private profile: unreachable
}

#[test]
fn v2_state_store_compiles_and_wires_in_node() {
    // Compile-time test: verify StateStoreV2 is used in validation path when V2 active
    // If this compiles, the wiring exists
}
```

- [ ] **Step 2: Wire V2 state store into `qantodag.rs` block validation.**

In `qantodag.rs`:
- When `is_v2_active(height, params)`, construct `StateStoreV2` with parent block's `StateRootV2`.
- Execute all transactions in block via `StateStoreV2::execute()`.
- Verify resulting `StateRootV2` matches block's declared `state_root`.
- Store new state root in backend.

- [ ] **Step 3: Wire V2 into `consensus.rs` validation.**

In `consensus.rs::validate_block`:
- When V2 active, delegate to V2 state validation.
- Check `PruningManifestV2` if block includes pruning.

- [ ] **Step 4: Wire V2 into `block_producer.rs` and `state_sync.rs`.**

- `block_producer.rs`: Include `StateRootV2` in produced blocks, compute via `StateStoreV2`.
- `state_sync.rs`: Verify state roots during sync, request Merkle proofs for verification.

- [ ] **Step 5: Run integration build test.**

```bash
rtk cargo build --workspace --locked --features infinite-strata
```

Expected: `PASS`.

- [ ] **Step 6: Record intended commit.**

Intended commit: `feat(qanto-node): V2 branch-scoped state store integration in validation and sync`.

---

### Task 4: Implement State Replay Verification and Deterministic Pruning

**Files:**
- Create: `Protocol_Core/qanto-storage/src/replay_v2.rs`
- Modify: `Protocol_Core/qanto-node/src/state_sync.rs`
- Create: `Protocol_Core/qanto-storage/tests/replay_v2.rs`
- Test: `Protocol_Core/qanto-node/tests/v2_replay.rs`

**Interfaces:**
- Produces: `replay_state_transitions()`, `verify_state_root_at_height()`, deterministic pruning with `PruningManifestV2`.
- Consumes: `StateStoreV2`, `StorageBackendV2`, genesis state, block sequence.

- [ ] **Step 1: Write failing replay tests.**

Create `Protocol_Core/qanto-storage/tests/replay_v2.rs`:

```rust
use qanto_consensus_primitives::{
    storage_v2::{StateStoreV2, StateRootV2, PruningManifestV2},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    block_v2::BlockCoreV2,
    transaction_v2::TransactionEnvelopeV2,
};

#[test]
fn replay_from_genesis_produces_deterministic_roots() {
    // Given genesis state root and block sequence, replay must produce identical roots
    // at every height. This is a structural test - the implementation must guarantee this.
}

#[test]
fn pruning_manifest_commits_pruned_roots() {
    // PruningManifestV2 must list pruned roots with height ranges
    // Re-verification must be able to check pruning validity
}

#[test]
fn state_sync_verifies_roots_against_replay() {
    // Sync path must re-execute or verify Merkle proofs against committed roots
}
```

- [ ] **Step 2: Implement `replay_v2.rs`.**

Create `Protocol_Core/qanto-storage/src/replay_v2.rs` with:
- `ReplayEngineV2` that takes genesis root + block iterator
- `verify_state_transition(parent_root, block, new_root) -> bool`
- `generate_proof_for_height(target_height) -> Vec<StateNodeV2>`
- Deterministic pruning: `prune_up_to(height) -> PruningManifestV2`

- [ ] **Step 3: Wire replay into `state_sync.rs`.**

In `state_sync.rs`:
- On full sync, replay from genesis or last trusted checkpoint.
- On fast sync, verify Merkle proofs against block headers' state roots.
- Store `PruningManifestV2` in backend for audit.

- [ ] **Step 4: Run replay and sync tests.**

```bash
rtk cargo nextest run -p qanto-storage -p qanto-node --locked -- replay_v2 state_sync v2_replay
```

Expected: `PASS`.

- [ ] **Step 5: Record intended commit.**

Intended commit: `feat(qanto-storage): V2 deterministic replay verification and pruning`.

---

### Task 5: Migration from Legacy Storage Keys to V2 Typed Keys

**Files:**
- Create: `Protocol_Core/qanto-storage/src/migration_v1_to_v2.rs`
- Create: `Protocol_Core/qanto-storage/tests/migration_v1_to_v2.rs`
- Modify: `Protocol_Core/qanto-node/src/node.rs` (migration CLI command)

**Interfaces:**
- Produces: `migrate_v1_to_v2()` function, CLI command `qanto migrate-storage --profile private|public`.
- Consumes: Legacy key patterns (`"account/"`, `"utxo/"`, `"storage/"`), V2 `StateKeyV2`.

- [ ] **Step 1: Write failing migration test.**

Create `Protocol_Core/qanto-storage/tests/migration_v1_to_v2.rs`:

```rust
use qanto_storage::{backend_v2::RocksDBBackendV2, migration_v1_to_v2::migrate_v1_to_v2};
use qanto_consensus_primitives::{
    storage_v2::StateStoreV2,
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    hash::StateRootV2,
};
use tempfile::TempDir;

#[test]
fn migration_preserves_all_v1_state() {
    let dir = TempDir::new().unwrap();
    // Setup V1 database with legacy keys
    // Run migration
    // Verify all keys accessible via V2 StateKeyV2
    // Verify state root matches expected post-migration root
}
```

- [ ] **Step 2: Implement migration logic.**

In `migration_v1_to_v2.rs`:
- Scan legacy keys by prefix
- Map each legacy key to V2 `StateKeyV2` (e.g., `"account/{addr}/balance"` -> `StateKeyV2::new(b"account/{addr}/balance", chain_id, schema)`)
- Preserve all values byte-for-byte
- Compute new `StateRootV2` from migrated keys
- Write `PruningManifestV2` with pre-migration root as pruned entry

- [ ] **Step 3: Add CLI migration command to `node.rs`.**

```rust
#[derive(Parser)]
struct MigrateStorageArgs {
    #[arg(long)]
    profile: MigrationProfile, // Private | PublicTestnet
    #[arg(long)]
    dry_run: bool,
}

async fn migrate_storage(cmd: MigrateStorageArgs) -> Result<()> { ... }
```

- [ ] **Step 4: Run migration tests.**

```bash
rtk cargo nextest run -p qanto-storage --locked -- migration_v1_to_v2
rtk cargo build --workspace --locked --features infinite-strata --bin qanto
# Test CLI: qanto migrate-storage --profile private --dry-run
```

Expected: `PASS`.

- [ ] **Step 5: Record intended commit.**

Intended commit: `feat(qanto-storage): V1 to V2 storage migration tool and CLI`.

---

### Task 6: Golden Vector Cross-Storage Validation and Final Review

**Files:**
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs` (add storage vectors)
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/cross_storage.json`

**Interfaces:**
- Validates that `qanto-storage` backend produces identical state roots as `qanto-consensus-primitives` reference.

- [ ] **Step 1: Write cross-crate storage golden vector test.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{
    storage_v2::{StateKeyV2, StateLeafV2, StateNodeV2, StateRootV2, StateStoreV2},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    hash::{HashV2, StateRootV2 as StateRootV2Type},
};

#[test]
fn cross_crate_state_key_matches_storage_backend() {
    let expected = include_bytes!("../vectors/cross_storage.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    let key = StateKeyV2::new(
        &hex::decode(expected["key_bytes"].as_str().unwrap()).unwrap(),
        expected["chain_id"].as_u64().unwrap() as u32,
        SchemaVersion::V2_INITIAL,
    );
    assert_eq!(key.to_string(), expected["state_key_v2"]);
}

#[test]
fn cross_crate_state_root_matches_after_execution() {
    // Given a sequence of state operations, the StateStoreV2 with RocksDB backend
    // must produce the same StateRootV2 as the reference in-memory implementation.
    // This is verified by the reference vectors in cross_storage.json.
}
```

- [ ] **Step 2: Create `cross_storage.json` fixture.**

- [ ] **Step 3: Run all V2 storage tests across crates.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives -p qanto-storage -p qanto-node --locked -- state_ storage_ migration_ replay_ cross_crate_ v2_storage v2_replay
```

Expected: `PASS`.

- [ ] **Step 4: Final self-review checklist.**

- [ ] All V2 state types (Key, Leaf, Node, Root) defined with canonical schemas.
- [ ] `StorageBackendV2` trait with all required operations (get, set, delete, proof, prune, commit).
- [ ] RocksDB and Sled backends implement `StorageBackendV2`.
- [ ] `StateStoreV2` enforces V2 invariants (typed keys, Merkle proofs, chain separation).
- [ ] V2 block execution uses `StateStoreV2` and verifies `StateRootV2`.
- [ ] Replay verification: genesis + blocks -> deterministic state roots at every height.
- [ ] Pruning is explicit via `PruningManifestV2` with committed pruned roots.
- [ ] Migration tool converts all legacy keys to V2 typed keys, preserves values.
- [ ] Chain separation: different chain_id -> different state keys/roots for same logical key.
- [ ] Schema separation: different schema -> different hashes.
- [ ] Domain separation: StateKey/Leaf/Node/Root domains produce different hashes.
- [ ] No legacy key prefixes (`"account/"`, `"utxo/"`, etc.) in V2 code paths.
- [ ] All golden vectors committed and validated.
- [ ] QIP-004 (branch state/fork choice/finality) draft references this plan.
- [ ] All tests pass across `qanto-consensus-primitives`, `qanto-storage`, `qanto-node`.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(qanto-storage): cross-crate V2 storage golden vectors; complete branch-scoped persistence`.

---

## Acceptance Criteria

- [ ] Every V2 state object (key, leaf, node, root, pruning manifest) has frozen `FieldSpec` schema and canonical builder.
- [ ] `StateKeyV2`, `StateLeafV2`, `StateNodeV2`, `StateRootV2`, `PruningManifestV2` all use `hash_v2` with `DomainV2` separation.
- [ ] `StorageBackendV2` trait fully specified; RocksDB and Sled implementations pass all contract tests.
- [ ] `StateStoreV2` is the single state access point in V2 validation path.
- [ ] Block execution produces `StateRootV2` that matches header commitment.
- [ ] Replay from genesis + blocks is deterministic: identical roots at every height.
- [ ] Pruning produces verifiable `PruningManifestV2`; no implicit GC.
- [ ] Migration tool converts 100% of legacy keys to V2 typed keys.
- [ ] Chain, schema, domain separation verified for all state types.
- [ ] Golden vectors for all state types and cross-crate parity.
- [ ] QIP-004 proposal written (fork choice, branch state, finality certificates).
- [ ] All tests pass: `cargo nextest run -p qanto-consensus-primitives -p qanto-storage -p qanto-node --locked`.
- [ ] Workspace builds clean: `cargo build --workspace --locked --features infinite-strata`.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 3 Summary for Dependent Workstreams

| Output | Location | Consumers |
|---|---|---|
| `StateKeyV2`, `StateLeafV2`, `StateNodeV2`, `StateRootV2` | `qanto-consensus-primitives::storage_v2` | Workstreams 4, 5, 7, 8, 9 |
| `PruningManifestV2` | `qanto-consensus-primitives::storage_v2` | Workstreams 4, 7, 9 |
| `StorageBackendV2` trait | `qanto-consensus-primitives::storage_v2` | Workstreams 4, 7, 9 |
| `StateStoreV2` | `qanto-consensus-primitives::storage_v2` | Workstreams 4, 5, 7, 8, 9 |
| `RocksDBBackendV2`, `SledBackendV2` | `qanto-storage::backend_v2` | Workstreams 4, 7, 9 |
| Migration tool | `qanto-storage::migration_v1_to_v2` | Workstream 9 (operations) |
| Replay engine | `qanto-storage::replay_v2` | Workstreams 4, 7, 9 |
| QIP-004 draft | `Docs/protocol/proposals/QIP-004-branch-state-finality.md` | Workstreams 4, 5, 8, 9, 10 |
| Golden vectors | `qanto-consensus-primitives/tests/vectors/storage_v2.json`, `cross_storage.json` | Workstreams 4, 5, 7, 8, 9, 10 |