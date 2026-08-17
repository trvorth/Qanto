//! Qanto Standalone - Core blockchain types and utilities
//!
//! This module provides the core types and utilities for the Qanto blockchain,
//! designed to be portable and self-contained.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

// --- Submodules ---

pub mod collections {
    use dashmap::DashMap;

    /// A thread-safe concurrent hash map
    pub type ConcurrentHashMap<K, V> = DashMap<K, V>;
}

pub mod db {
    use parking_lot::RwLock;
    use std::collections::HashMap;

    /// Key-value store trait
    pub trait KeyValueStore: Send + Sync {
        fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
        fn put(&self, key: &[u8], value: &[u8]);
        fn delete(&self, key: &[u8]);
    }

    /// In-memory key-value store
    #[derive(Debug, Default)]
    pub struct MemoryStore {
        data: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    }

    impl MemoryStore {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl KeyValueStore for MemoryStore {
        fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
            self.data.read().get(key).cloned()
        }

        fn put(&self, key: &[u8], value: &[u8]) {
            self.data.write().insert(key.to_vec(), value.to_vec());
        }

        fn delete(&self, key: &[u8]) {
            self.data.write().remove(key);
        }
    }
}

pub mod hash {
    pub use crate::QantoHash;

    /// Compute Qanto hash
    pub fn qanto_hash(data: &[u8]) -> QantoHash {
        super::qanto_hash(data)
    }

    /// Compute Qanto hash (alias)
    pub fn compute_qanto_hash(data: &[u8]) -> QantoHash {
        super::qanto_hash(data)
    }

    /// Qanto hash type
    pub type Hash = QantoHash;
}

pub mod serde {
    use bincode;
    pub use serde::{Deserialize, Serialize};

    pub fn serialize<T: Serialize>(value: &T) -> Vec<u8> {
        bincode::serialize(value).unwrap_or_default()
    }

    pub fn deserialize<T: for<'a> Deserialize<'a>>(data: &[u8]) -> Option<T> {
        bincode::deserialize(data).ok()
    }

    pub fn serialize_to_vec<T: Serialize>(value: &T) -> Vec<u8> {
        serialize(value)
    }
}

pub mod sync {
    use parking_lot::{Mutex, RwLock};

    pub struct AsyncMutex<T> {
        inner: Mutex<T>,
    }

    impl<T> AsyncMutex<T> {
        pub fn new(value: T) -> Self {
            Self {
                inner: Mutex::new(value),
            }
        }

        pub async fn lock(&self) -> AsyncMutexGuard<'_, T> {
            AsyncMutexGuard {
                guard: self.inner.lock(),
            }
        }
    }

    pub struct AsyncMutexGuard<'a, T> {
        guard: parking_lot::MutexGuard<'a, T>,
    }

    impl<T> std::ops::Deref for AsyncMutexGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.guard
        }
    }

    impl<T> std::ops::DerefMut for AsyncMutexGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.guard
        }
    }

    pub struct AsyncRwLock<T> {
        inner: RwLock<T>,
    }

    impl<T> AsyncRwLock<T> {
        pub fn new(value: T) -> Self {
            Self {
                inner: RwLock::new(value),
            }
        }

        pub async fn read(&self) -> AsyncRwLockReadGuard<'_, T> {
            AsyncRwLockReadGuard {
                guard: self.inner.read(),
            }
        }

        pub async fn write(&self) -> AsyncRwLockWriteGuard<'_, T> {
            AsyncRwLockWriteGuard {
                guard: self.inner.write(),
            }
        }
    }

    pub struct AsyncRwLockReadGuard<'a, T> {
        guard: parking_lot::RwLockReadGuard<'a, T>,
    }

    impl<T> std::ops::Deref for AsyncRwLockReadGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.guard
        }
    }

    pub struct AsyncRwLockWriteGuard<'a, T> {
        guard: parking_lot::RwLockWriteGuard<'a, T>,
    }

    impl<T> std::ops::Deref for AsyncRwLockWriteGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.guard
        }
    }

    impl<T> std::ops::DerefMut for AsyncRwLockWriteGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.guard
        }
    }

    pub type AtomicU64 = std::sync::atomic::AtomicU64;
}

pub mod time {
    use std::time::{SystemTime as StdSystemTime, UNIX_EPOCH};

    pub struct SystemTime;

    impl SystemTime {
        pub fn now_nanos() -> i64 {
            StdSystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as i64
        }
    }

    pub type Timestamp = i64;
}

pub mod parallel {
    /// Thread pool for parallel execution using rayon
    pub use rayon::ThreadPoolBuilder;
}

// --- Main Types ---

/// Qanto hash type
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct QantoHash([u8; 32]);

impl QantoHash {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl AsRef<[u8]> for QantoHash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for QantoHash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl std::fmt::Display for QantoHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Compute Qanto hash
pub fn qanto_hash(data: &[u8]) -> QantoHash {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    QantoHash(bytes)
}

pub type Hash = QantoHash;

// --- Blockchain Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub number: u64,
    pub difficulty: u64,
    pub nonce: u64,
    pub mix_hash: Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub value: u128,
    pub gas_limit: u64,
    pub gas_price: u128,
    pub nonce: u64,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn get_hash(&self) -> Hash {
        qanto_hash(&bincode::serialize(self).unwrap_or_default())
    }

    pub fn hash_hex(&self) -> String {
        format!("0x{}", hex::encode(self.get_hash().as_bytes()))
    }
}

pub struct Blockchain {
    blocks: Vec<Block>,
    difficulty: Arc<parking_lot::RwLock<u64>>,
    is_mining: Arc<parking_lot::RwLock<bool>>,
    hash_rate: Arc<parking_lot::RwLock<u64>>,
}

impl Blockchain {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            difficulty: Arc::new(parking_lot::RwLock::new(1000)),
            is_mining: Arc::new(parking_lot::RwLock::new(false)),
            hash_rate: Arc::new(parking_lot::RwLock::new(0)),
        }
    }

    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }

    pub fn get_block(&self, index: usize) -> Option<&Block> {
        self.blocks.get(index)
    }

    pub fn get_block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_mining(&self) -> bool {
        *self.is_mining.read()
    }

    pub fn get_current_difficulty(&self) -> u64 {
        *self.difficulty.read()
    }

    pub fn get_hash_rate(&self) -> u64 {
        *self.hash_rate.read()
    }

    pub fn set_mining(&self, mining: bool) {
        *self.is_mining.write() = mining;
    }

    pub fn set_difficulty(&self, difficulty: u64) {
        *self.difficulty.write() = difficulty;
    }

    pub fn set_hash_rate(&self, rate: u64) {
        *self.hash_rate.write() = rate;
    }
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

// --- JSON-RPC Helpers ---

pub mod jsonrpc {
    use crate::qanto_standalone::{Block, Blockchain, Hash};

    pub fn get_chain_id() -> u64 {
        1
    }

    pub fn get_block_number(blockchain: &Blockchain) -> u64 {
        blockchain.get_block_count() as u64
    }

    pub fn get_block(blockchain: &Blockchain, number: u64) -> Option<&Block> {
        blockchain.get_block(number as usize)
    }

    pub fn get_block_by_hash<'a>(
        _blockchain: &'a Blockchain,
        _hash: &'a Hash,
    ) -> Option<&'a Block> {
        None // Not implemented
    }

    pub fn get_balance(_blockchain: &Blockchain, _address: &str) -> u128 {
        0 // Not implemented
    }

    pub fn send_raw_transaction(_blockchain: &Blockchain, _tx: &[u8]) -> Result<Hash, String> {
        Err("Not implemented".to_string())
    }
}
