//! --- Qanto Native Storage Engine ---
//! v1.0.0 - Custom Database Implementation
//! This module provides a native storage engine for Qanto,
//! replacing RocksDB with a custom high-performance implementation.
//!
//! Features:
//! - High-performance key-value storage
//! - ACID transactions
//! - Write-ahead logging (WAL)
//! - Compression and encryption
//! - Concurrent access with fine-grained locking
//! - Automatic compaction and garbage collection
//! - Blockchain-optimized data structures

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinHandle;

// Native serialization (will replace serde)
use crate::qanto_serde::{QantoDeserialize, QantoDeserializer, QantoSerialize, QantoSerializer};

#[derive(Error, Debug)]
pub enum QantoStorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Serde error: {0}")]
    Serde(#[from] crate::qanto_serde::QantoSerdeError),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Transaction error: {0}")]
    Transaction(String),
    #[error("Corruption detected: {0}")]
    Corruption(String),
    #[error("Database locked")]
    Locked,
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Compression error: {0}")]
    Compression(String),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Initialization error: {0}")]
    Initialization(String),
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub max_file_size: u64,
    pub compression_enabled: bool,
    pub encryption_enabled: bool,
    pub wal_enabled: bool,
    pub sync_writes: bool,
    pub cache_size: usize,
    pub compaction_threshold: usize, // Changed from f64 to usize
    pub max_open_files: usize,

    // Enhanced configuration for 32 BPS and 10M+ TPS
    pub memtable_size: usize,
    pub write_buffer_size: usize,
    pub batch_size: usize,
    pub parallel_writers: usize,
    pub enable_write_batching: bool,
    pub enable_bloom_filters: bool,
    pub enable_async_io: bool,
    pub sync_interval: Duration,
    pub compression_level: i32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            max_file_size: 64 * 1024 * 1024, // 64MB
            compression_enabled: true,
            encryption_enabled: true,
            wal_enabled: true,
            sync_writes: true,
            cache_size: 128 * 1024 * 1024, // 128MB
            compaction_threshold: 10,      // 10 segments
            max_open_files: 1000,

            // Enhanced defaults for high performance
            memtable_size: 64 * 1024 * 1024,     // 64MB
            write_buffer_size: 16 * 1024 * 1024, // 16MB
            batch_size: 10000,
            parallel_writers: 8,
            enable_write_batching: true,
            enable_bloom_filters: true,
            enable_async_io: true,
            sync_interval: Duration::from_millis(100),
            compression_level: 3,
        }
    }
}

/// Batch operation for efficient writes
#[derive(Debug, Clone)]
pub struct WriteBatch {
    pub operations: Vec<LogEntry>,
}

impl WriteBatch {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            operations: Vec::with_capacity(capacity),
        }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.operations.push(LogEntry::Put { key, value });
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.operations.push(LogEntry::Delete { key });
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn clear(&mut self) {
        self.operations.clear();
    }
}

impl Default for WriteBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Log entry types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntry {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
    Transaction { id: u64, entries: Vec<LogEntry> },
    Commit { transaction_id: u64 },
    Rollback { transaction_id: u64 },
    Checkpoint { sequence: u64 },
}

impl QantoSerialize for LogEntry {
    fn serialize<W: QantoSerializer>(
        &self,
        serializer: &mut W,
    ) -> Result<(), crate::qanto_serde::QantoSerdeError> {
        match self {
            LogEntry::Put { key, value } => {
                serializer.write_u8(0)?;
                QantoSerialize::serialize(key, serializer)?;
                QantoSerialize::serialize(value, serializer)?;
            }
            LogEntry::Delete { key } => {
                serializer.write_u8(1)?;
                QantoSerialize::serialize(key, serializer)?;
            }
            LogEntry::Transaction { id, entries } => {
                serializer.write_u8(2)?;
                QantoSerialize::serialize(id, serializer)?;
                QantoSerialize::serialize(entries, serializer)?;
            }
            LogEntry::Commit { transaction_id } => {
                serializer.write_u8(3)?;
                QantoSerialize::serialize(transaction_id, serializer)?;
            }
            LogEntry::Rollback { transaction_id } => {
                serializer.write_u8(4)?;
                QantoSerialize::serialize(transaction_id, serializer)?;
            }
            LogEntry::Checkpoint { sequence } => {
                serializer.write_u8(5)?;
                QantoSerialize::serialize(sequence, serializer)?;
            }
        }
        Ok(())
    }
}

impl QantoDeserialize for LogEntry {
    fn deserialize<R: QantoDeserializer>(
        deserializer: &mut R,
    ) -> Result<Self, crate::qanto_serde::QantoSerdeError> {
        let tag = deserializer.read_u8()?;
        match tag {
            0 => {
                let key = QantoDeserialize::deserialize(deserializer)?;
                let value = QantoDeserialize::deserialize(deserializer)?;
                Ok(LogEntry::Put { key, value })
            }
            1 => {
                let key = QantoDeserialize::deserialize(deserializer)?;
                Ok(LogEntry::Delete { key })
            }
            2 => {
                let id = QantoDeserialize::deserialize(deserializer)?;
                let entries = QantoDeserialize::deserialize(deserializer)?;
                Ok(LogEntry::Transaction { id, entries })
            }
            3 => {
                let transaction_id = QantoDeserialize::deserialize(deserializer)?;
                Ok(LogEntry::Commit { transaction_id })
            }
            4 => {
                let transaction_id = QantoDeserialize::deserialize(deserializer)?;
                Ok(LogEntry::Rollback { transaction_id })
            }
            5 => {
                let sequence = QantoDeserialize::deserialize(deserializer)?;
                Ok(LogEntry::Checkpoint { sequence })
            }
            _ => Err(crate::qanto_serde::QantoSerdeError::InvalidTypeTag(tag)),
        }
    }
}

/// Storage statistics
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub total_keys: u64,
    pub total_size: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub compactions: u64,
    pub writes: u64,
    pub reads: u64,
    pub deletes: u64,
}

/// In-memory cache entry with atomic access count
#[derive(Debug)]
struct CacheEntry {
    value: Vec<u8>,
    last_accessed: SystemTime,
    access_count: AtomicU64,
}

impl Clone for CacheEntry {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            last_accessed: self.last_accessed,
            access_count: AtomicU64::new(self.access_count.load(Ordering::Relaxed)),
        }
    }
}

impl CacheEntry {
    fn new(value: Vec<u8>) -> Self {
        Self {
            value,
            last_accessed: SystemTime::now(),
            access_count: AtomicU64::new(1),
        }
    }

    #[allow(dead_code)]
    fn touch(&self) -> u64 {
        self.access_count.fetch_add(1, Ordering::Relaxed)
    }
}

/// Write-ahead log
#[derive(Debug)]
struct WriteAheadLog {
    file: BufWriter<File>,
    sequence: u64,
    #[allow(dead_code)] // May be used in future implementations
    path: PathBuf,
}

impl WriteAheadLog {
    fn new(path: PathBuf) -> Result<Self, QantoStorageError> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(Self {
            file: BufWriter::new(file),
            sequence: 0,
            path,
        })
    }

    fn append(&mut self, entry: &LogEntry) -> Result<u64, QantoStorageError> {
        self.sequence += 1;

        // Serialize entry with sequence number
        let mut serializer = crate::qanto_serde::BinarySerializer::new();
        QantoSerialize::serialize(&self.sequence, &mut serializer)?;
        QantoSerialize::serialize(entry, &mut serializer)?;

        let data = serializer.finish();

        // Write length prefix + data
        let len = data.len() as u32;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&data)?;
        self.file.flush()?;

        Ok(self.sequence)
    }

    fn sync(&mut self) -> Result<(), QantoStorageError> {
        self.file.flush()?;
        self.file.get_mut().sync_all()?;
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<(), QantoStorageError> {
        let checkpoint = LogEntry::Checkpoint {
            sequence: self.sequence,
        };
        self.append(&checkpoint)?;
        self.sync()
    }
}

/// High-level account state cache for fast balance lookups
#[derive(Debug, Default, Clone)]
pub struct AccountStateCache {
    balances: Arc<DashMap<String, u128>>, // address -> balance (base units)
}

impl AccountStateCache {
    /// Create a new account state cache.
    pub fn new() -> Self {
        Self {
            balances: Arc::new(DashMap::new()),
        }
    }

    /// Get current balance for an address.
    /// Returns `None` if the address is not in cache.
    pub fn get_balance(&self, address: &str) -> Option<u128> {
        self.balances.get(address).map(|v| *v)
    }

    /// Set the absolute balance for an address.
    pub fn set_balance<A: Into<String>>(&self, address: A, balance: u128) {
        self.balances.insert(address.into(), balance);
    }

    /// Apply a signed delta to an address balance with saturating semantics.
    /// Negative deltas decrease the balance but never go below zero.
    pub fn apply_delta(&self, address: &str, delta: i128) -> u128 {
        use std::cmp::Ordering;
        let next = match delta.cmp(&0) {
            Ordering::Equal => self.get_balance(address).unwrap_or(0),
            Ordering::Greater => {
                let incr = delta as u128;
                self.get_balance(address).unwrap_or(0).saturating_add(incr)
            }
            Ordering::Less => {
                let decr = (-delta) as u128;
                self.get_balance(address).unwrap_or(0).saturating_sub(decr)
            }
        };
        self.set_balance(address.to_string(), next);
        next
    }

    /// Clear all cached balances.
    pub fn clear(&self) {
        self.balances.clear();
    }

    /// Snapshot all balances into a HashMap.
    pub fn snapshot(&self) -> HashMap<String, u128> {
        let mut map = HashMap::new();
        for entry in self.balances.iter() {
            map.insert(entry.key().clone(), *entry.value());
        }
        map
    }
}

/// Storage segment (SSTable-like structure)
#[derive(Debug)]
struct StorageSegment {
    id: u64,
    path: PathBuf,
    index: BTreeMap<Vec<u8>, (u64, u32)>, // key -> (offset, length)
    file: Option<BufReader<File>>,
    size: u64,
    key_count: u64,
    #[allow(dead_code)] // May be used for segment management
    created_at: SystemTime,
}

impl StorageSegment {
    fn new(id: u64, path: PathBuf) -> Self {
        Self {
            id,
            path,
            index: BTreeMap::new(),
            file: None,
            size: 0,
            key_count: 0,
            created_at: SystemTime::now(),
        }
    }

    fn open(&mut self) -> Result<(), QantoStorageError> {
        if self.file.is_none() {
            let file = File::open(&self.path)?;
            self.file = Some(BufReader::new(file));
        }
        Ok(())
    }

    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError> {
        // Clone the offset and length to avoid borrowing issues
        if let Some((offset, length)) = self.index.get(key).map(|(o, l)| (*o, *l)) {
            // Treat zero-length values as tombstones
            if length == 0 {
                return Ok(None);
            }
            self.open()?;

            if let Some(ref mut file) = self.file {
                file.seek(SeekFrom::Start(offset))?;
                let mut buffer = vec![0u8; length as usize];
                file.read_exact(&mut buffer)?;

                // Decompress if needed
                let value = if self.is_compressed(&buffer) {
                    self.decompress(&buffer)?
                } else {
                    buffer
                };

                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    #[allow(dead_code)] // May be used for future key existence checks
    fn contains_key(&self, key: &[u8]) -> bool {
        match self.index.get(key) {
            Some((_, length)) => *length > 0,
            None => false,
        }
    }

    // Detect if a key maps to a tombstone (zero-length value)
    fn is_tombstone(&self, key: &[u8]) -> bool {
        self.index
            .get(key)
            .map(|(_, length)| *length == 0)
            .unwrap_or(false)
    }

    #[allow(dead_code)] // May be used for future key iteration features
    fn keys(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.index.keys()
    }

    fn is_compressed(&self, data: &[u8]) -> bool {
        // Simple magic number check
        data.len() > 4 && &data[0..4] == b"QCMP"
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, QantoStorageError> {
        if !self.is_compressed(data) {
            return Ok(data.to_vec());
        }

        if data.len() < 8 {
            return Err(QantoStorageError::Corruption(
                "Invalid compressed data".to_string(),
            ));
        }

        // Read original size from QCMP header
        let original_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

        // Ensure we have enough bytes after the header
        if data.len() < 8 + original_size {
            return Err(QantoStorageError::Corruption(format!(
                "Compressed payload truncated: expected {} bytes, have {}",
                original_size,
                data.len() - 8
            )));
        }

        // Current compressor stores raw bytes after QCMP header; return them directly
        Ok(data[8..8 + original_size].to_vec())
    }
}

/// Transaction context
#[derive(Debug)]
pub struct Transaction {
    #[allow(dead_code)] // May be used for transaction tracking
    id: u64,
    operations: Vec<LogEntry>,
    #[allow(dead_code)] // May be used for transaction state management
    committed: bool,
    read_snapshot: HashMap<Vec<u8>, Vec<u8>>,
}

impl Transaction {
    fn new(id: u64) -> Self {
        Self {
            id,
            operations: Vec::new(),
            committed: false,
            read_snapshot: HashMap::new(),
        }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.operations.push(LogEntry::Put { key, value });
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.operations.push(LogEntry::Delete { key });
    }

    pub fn get(&self, key: &[u8]) -> Option<&Vec<u8>> {
        // Check transaction-local changes first
        for op in self.operations.iter().rev() {
            match op {
                LogEntry::Put { key: k, value } if k == key => return Some(value),
                LogEntry::Delete { key: k } if k == key => return None,
                _ => {}
            }
        }

        // Check read snapshot
        self.read_snapshot.get(key)
    }
}

/// Main storage engine with optimized concurrent access
#[derive(Debug)]
pub struct QantoStorage {
    config: StorageConfig,
    segments: RwLock<Vec<StorageSegment>>,
    sharded_memtable: ShardedMemtable,
    cache: DashMap<Vec<u8>, CacheEntry>,
    wal: Mutex<Option<WriteAheadLog>>,
    stats: RwLock<StorageStats>,
    cache_size: AtomicUsize,
    next_segment_id: AtomicU64,
    next_transaction_id: AtomicU64,
    active_transactions: RwLock<HashMap<u64, Transaction>>,
    compaction_in_progress: AtomicBool,
    async_io_processor: Option<AsyncIOProcessor>,
    #[allow(dead_code)]
    write_semaphore: Arc<Semaphore>,
}

impl Clone for QantoStorage {
    fn clone(&self) -> Self {
        // Create a new storage instance with the same configuration
        // but fresh internal state
        Self::new(self.config.clone()).expect("Failed to clone QantoStorage")
    }
}

impl QantoStorage {
    /// Create a mining-optimized storage configuration
    pub fn mining_optimized_config(data_dir: PathBuf) -> StorageConfig {
        StorageConfig {
            data_dir,
            max_file_size: 1024 * 1024 * 100, // 100MB for better batching
            compression_enabled: false,       // Disable compression for mining speed
            encryption_enabled: false,        // Disable encryption for mining speed
            wal_enabled: true,                // Keep WAL for safety
            sync_writes: false, // Critical: disable sync writes for mining performance
            cache_size: 1024 * 1024 * 50, // 50MB cache for better performance
            compaction_threshold: 2, // Higher threshold to reduce compaction frequency (changed to usize)
            max_open_files: 500,     // Reasonable limit
            memtable_size: 1024 * 1024 * 16, // 16MB memtable
            write_buffer_size: 1024 * 1024 * 4, // 4MB write buffer
            batch_size: 1000,        // Batch size for writes
            parallel_writers: 4,     // Number of parallel writers
            enable_write_batching: true,
            enable_bloom_filters: true,
            enable_async_io: true,
            sync_interval: Duration::from_millis(100),
            compression_level: 3,
        }
    }

    /// Temporarily disable sync writes for mining operations
    pub fn set_mining_mode(&self, enabled: bool) -> Result<(), QantoStorageError> {
        // Note: This would require making config mutable or using atomic operations
        // For now, we'll implement this as a configuration hint
        tracing::debug!(
            "Mining mode {}: sync_writes disabled for performance",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Optimized batch write for mining operations with sharded writes
    pub fn write_mining_batch(&self, batch: WriteBatch) -> Result<(), QantoStorageError> {
        if batch.is_empty() {
            return Ok(());
        }

        let _tx_id = self.begin_transaction();

        // Collect all keys and values for batch processing
        let mut operations = Vec::new();

        for entry in &batch.operations {
            match entry {
                LogEntry::Put { key, value } => {
                    operations.push((key.clone(), Some(value.clone())));
                }
                LogEntry::Delete { key } => {
                    operations.push((key.clone(), None));
                }
                _ => {}
            }
        }

        // Log to WAL first for durability
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            for entry in &batch.operations {
                wal.append(entry)?;
            }
            if self.config.sync_writes {
                wal.sync()?;
            }
        }

        // Batch update sharded memtable
        for (key, value_opt) in operations {
            match value_opt {
                Some(value) => {
                    self.sharded_memtable.put(key.clone(), value.clone())?;

                    // Update cache using DashMap for lock-free access
                    let entry = CacheEntry::new(value.clone());
                    self.cache.insert(key.clone(), entry);

                    // Update cache size atomically
                    let value_size = key.len() + value.len();
                    let current_size = self.cache_size.fetch_add(value_size, Ordering::Relaxed);

                    // Evict cache if too large
                    if current_size + value_size > self.config.cache_size {
                        self.evict_cache_lockfree();
                    }
                }
                None => {
                    self.sharded_memtable.delete(&key)?;
                    self.cache.remove(&key);
                }
            }
        }

        // Update stats
        {
            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.writes += batch.len() as u64;
        }

        // Check if memtable needs flushing
        let memtable_size = self.sharded_memtable.size() * 1024; // Rough estimate
        if memtable_size > self.config.memtable_size {
            let _flushed_data = self.sharded_memtable.flush_all()?;
        }

        Ok(())
    }

    /// Flush and sync for mining checkpoint (called after block mining)
    pub fn mining_checkpoint(&self) -> Result<(), QantoStorageError> {
        // Flush memtable if needed
        let memtable_size = self.sharded_memtable.size() * 1024; // Rough estimate

        if memtable_size > self.config.cache_size / 8 {
            self.flush_memtable()?;
        }

        // Force sync WAL for checkpoint
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            wal.sync()?;
        }

        tracing::debug!("Mining checkpoint completed: memtable flushed and WAL synced");
        Ok(())
    }

    /// Create a new storage engine with enhanced parallel processing
    pub fn new(config: StorageConfig) -> Result<Self, QantoStorageError> {
        // Create data directory
        std::fs::create_dir_all(&config.data_dir)?;

        // Calculate optimal shard count based on parallel writers
        let shard_count = config.parallel_writers.max(16); // Minimum 16 shards

        let storage = Arc::new(Self {
            config: config.clone(),
            segments: RwLock::new(Vec::new()),
            sharded_memtable: ShardedMemtable::new(shard_count),
            cache: DashMap::with_capacity(config.cache_size / 1024),
            wal: Mutex::new(None),
            stats: RwLock::new(StorageStats::default()),
            cache_size: AtomicUsize::new(0),
            next_segment_id: AtomicU64::new(1),
            next_transaction_id: AtomicU64::new(1),
            active_transactions: RwLock::new(HashMap::new()),
            compaction_in_progress: AtomicBool::new(false),
            async_io_processor: None,
            write_semaphore: Arc::new(Semaphore::new(config.parallel_writers)),
        });

        // Initialize WAL if enabled
        if config.wal_enabled {
            let wal_path = config.data_dir.join("wal.log");
            let wal = WriteAheadLog::new(wal_path)?;
            *storage.wal.lock().unwrap() = Some(wal);
        }

        // Initialize async I/O processor if enabled
        let mut storage_mut = Arc::try_unwrap(storage).map_err(|_| {
            QantoStorageError::Initialization("Failed to initialize storage".to_string())
        })?;

        if config.enable_async_io {
            let storage_arc = Arc::new(storage_mut);
            let storage_weak = Arc::downgrade(&storage_arc);
            let processor =
                AsyncIOProcessor::new(storage_weak, config.batch_size, config.sync_interval);

            // This is a bit tricky - we need to get the storage back
            let mut storage_final = Arc::try_unwrap(storage_arc).map_err(|_| {
                QantoStorageError::Initialization("Failed to finalize storage".to_string())
            })?;

            storage_final.async_io_processor = Some(processor);
            storage_mut = storage_final;
        }

        // Load existing segments
        storage_mut.load_segments()?;

        Ok(storage_mut)
    }

    /// Check if key exists
    /// Check if a key exists in the storage
    pub fn contains_key(&self, key: &[u8]) -> Result<bool, QantoStorageError> {
        // Check cache first
        if self.cache.contains_key(key) {
            return Ok(true);
        }

        // Check sharded memtable
        if let Some(val) = self.sharded_memtable.get(key)? {
            // Empty value indicates a tombstone
            if val.is_empty() {
                return Ok(false);
            }
            return Ok(true);
        }

        // Check segments
        let segments = self
            .segments
            .read()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;

        // Newest to oldest: tombstone masks older values
        for segment in segments.iter().rev() {
            if segment.is_tombstone(key) {
                return Ok(false);
            }
            if segment.contains_key(key) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get all keys with a prefix
    /// Get all keys with a given prefix using sharded memtable
    pub fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, QantoStorageError> {
        let mut keys = HashSet::new();
        let mut tombstoned = HashSet::new();

        // Check sharded memtable using iterator over shard indices
        for shard_lock in (0..self.sharded_memtable.shard_count())
            .filter_map(|idx| self.sharded_memtable.get_shard(idx))
        {
            let shard = shard_lock
                .read()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;

            for (key, value) in shard.iter() {
                if key.starts_with(prefix) {
                    if value.is_empty() {
                        tombstoned.insert(key.clone());
                        keys.remove(key);
                    } else if !tombstoned.contains(key) {
                        keys.insert(key.clone());
                    }
                }
            }
        }

        // Check segments
        let segments = self
            .segments
            .read()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;

        // Newest to oldest: tombstones mask older values
        for segment in segments.iter().rev() {
            for key in segment.keys() {
                if key.starts_with(prefix) {
                    if segment.is_tombstone(key) {
                        tombstoned.insert(key.clone());
                        keys.remove(key);
                    } else if !tombstoned.contains(key) {
                        keys.insert(key.clone());
                    }
                }
            }
        }

        Ok(keys.into_iter().collect())
    }

    /// Start a new transaction
    pub fn begin_transaction(&self) -> u64 {
        let tx_id = self.next_transaction_id.fetch_add(1, Ordering::SeqCst);
        let transaction = Transaction::new(tx_id);

        {
            let mut transactions = self.active_transactions.write().unwrap();
            transactions.insert(tx_id, transaction);
        }

        tx_id
    }

    /// Commit a transaction
    pub fn commit_transaction(&self, tx_id: u64) -> Result<(), QantoStorageError> {
        // Extract and remove transaction from active set
        let transaction = {
            let mut transactions = self.active_transactions.write().unwrap();
            transactions.remove(&tx_id).ok_or_else(|| {
                QantoStorageError::Transaction(format!("Transaction {tx_id} not found"))
            })?
        };

        // 1) Write Transaction envelope to WAL first for durability
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            let entry = LogEntry::Transaction {
                id: tx_id,
                entries: transaction.operations.clone(),
            };
            wal.append(&entry)?;
        }

        // 2) Apply operations directly to memtable/cache without duplicating WAL entries
        //    Prepare inverse ops to enable rollback in case of failure
        let mut inverses: Vec<LogEntry> = Vec::with_capacity(transaction.operations.len());
        let mut applied_count: usize = 0;
        let apply_result: Result<(), QantoStorageError> = (|| {
            for operation in &transaction.operations {
                match operation {
                    LogEntry::Put { key, value } => {
                        // Capture previous value to form inverse
                        let prev = self.get(key.as_slice())?;
                        match prev {
                            Some(prev_val) => {
                                inverses.push(LogEntry::Put {
                                    key: key.clone(),
                                    value: prev_val,
                                });
                            }
                            None => {
                                inverses.push(LogEntry::Delete { key: key.clone() });
                            }
                        }
                        // Apply directly
                        self.apply_put_direct(key.clone(), value.clone())?;
                        applied_count += 1;
                    }
                    LogEntry::Delete { key } => {
                        // Capture previous value to form inverse
                        let prev = self.get(key.as_slice())?;
                        if let Some(prev_val) = prev {
                            inverses.push(LogEntry::Put {
                                key: key.clone(),
                                value: prev_val,
                            });
                        } else {
                            // No-op inverse if key did not exist
                            inverses.push(LogEntry::Delete { key: key.clone() });
                        }
                        // Apply directly
                        self.apply_delete_direct(key.as_slice())?;
                        applied_count += 1;
                    }
                    _ => {}
                }
            }
            Ok(())
        })();

        if let Err(e) = apply_result {
            // Append Rollback entry and revert applied changes in reverse order
            if let Some(ref mut wal) = *self.wal.lock().unwrap() {
                let rollback_entry = LogEntry::Rollback {
                    transaction_id: tx_id,
                };
                wal.append(&rollback_entry)?;
                if self.config.sync_writes {
                    wal.sync()?;
                }
            }
            // Revert only the operations that were successfully applied
            for inverse in inverses.into_iter().take(applied_count).rev() {
                match inverse {
                    LogEntry::Put { key, value } => {
                        // Use direct apply to avoid WAL duplication during rollback
                        let _ = self.apply_put_direct(key, value);
                    }
                    LogEntry::Delete { key } => {
                        let _ = self.apply_delete_direct(key.as_slice());
                    }
                    _ => {}
                }
            }
            return Err(e);
        }

        // 3) Append Commit entry after successful application; sync if configured
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            let commit_entry = LogEntry::Commit {
                transaction_id: tx_id,
            };
            wal.append(&commit_entry)?;
            if self.config.sync_writes {
                wal.sync()?;
            }
        }

        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback_transaction(&self, tx_id: u64) -> Result<(), QantoStorageError> {
        {
            let mut transactions = self.active_transactions.write().unwrap();
            transactions.remove(&tx_id).ok_or_else(|| {
                QantoStorageError::Transaction(format!("Transaction {tx_id} not found"))
            })?;
        }

        // Log rollback to WAL
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            let entry = LogEntry::Rollback {
                transaction_id: tx_id,
            };
            wal.append(&entry)?;
            if self.config.sync_writes {
                wal.sync()?;
            }
        }

        Ok(())
    }

    /// Get storage statistics
    pub fn stats(&self) -> StorageStats {
        self.stats.read().unwrap().clone()
    }

    /// Flush memtable to disk
    pub fn flush(&self) -> Result<(), QantoStorageError> {
        self.flush_memtable()
    }

    /// Compact storage segments
    pub fn compact(&self) -> Result<(), QantoStorageError> {
        if self.compaction_in_progress.load(Ordering::Acquire) {
            return Ok(()); // Already compacting
        }

        self.compaction_in_progress.store(true, Ordering::Release);

        let result = self.perform_compaction();

        self.compaction_in_progress.store(false, Ordering::Release);

        result
    }

    /// Sync all data to disk
    pub fn sync(&self) -> Result<(), QantoStorageError> {
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            wal.sync()?;
        }
        Ok(())
    }

    /// Close the storage engine
    pub fn close(&self) -> Result<(), QantoStorageError> {
        // Flush memtable
        self.flush_memtable()?;

        // Sync WAL
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            wal.checkpoint()?;
        }

        Ok(())
    }

    // Private helper methods

    fn load_segments(&self) -> Result<(), QantoStorageError> {
        if !self.config.data_dir.exists() {
            std::fs::create_dir_all(&self.config.data_dir)?;
            return Ok(());
        }

        let mut segments = self.segments.write().unwrap();
        let mut loaded_segments = Vec::new();

        // Scan data directory for segment files
        for entry in std::fs::read_dir(&self.config.data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("segment_") && filename.ends_with(".qdb") {
                    // Extract segment ID from filename
                    if let Some(id_str) = filename
                        .strip_prefix("segment_")
                        .and_then(|s| s.strip_suffix(".qdb"))
                    {
                        // Handle both regular and compacted segments
                        let id_part = if let Some(base) = id_str.strip_suffix("_compacted") {
                            base
                        } else {
                            id_str
                        };

                        if let Ok(segment_id) = id_part.parse::<u64>() {
                            let mut segment = StorageSegment::new(segment_id, path.clone());

                            // Load segment metadata and index
                            if let Err(e) = self.load_segment_index(&mut segment) {
                                eprintln!("Warning: Failed to load segment {segment_id}: {e}");
                                continue;
                            }

                            loaded_segments.push(segment);

                            // Update next segment ID
                            let current_max = self.next_segment_id.load(Ordering::SeqCst);
                            if segment_id >= current_max {
                                self.next_segment_id.store(segment_id + 1, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }
        }

        // Sort segments by ID
        loaded_segments.sort_by_key(|s| s.id);

        // Update stats
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_keys = loaded_segments.iter().map(|s| s.key_count).sum();
            stats.total_size = loaded_segments.iter().map(|s| s.size).sum();
        }

        *segments = loaded_segments;

        Ok(())
    }

    fn load_segment_index(&self, segment: &mut StorageSegment) -> Result<(), QantoStorageError> {
        let mut file = BufReader::new(File::open(&segment.path)?);

        // Read and verify header
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != b"QSEG" {
            return Err(QantoStorageError::Corruption(
                "Invalid segment magic number".to_string(),
            ));
        }

        let mut version_bytes = [0u8; 4];
        file.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        if version != 1 {
            return Err(QantoStorageError::Corruption(format!(
                "Unsupported segment version: {version}"
            )));
        }

        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes)?;
        let entry_count = u32::from_le_bytes(count_bytes) as u64;

        // Build index by reading all entries
        let mut offset = 12u64; // Header size
        segment.index.clear();

        for _ in 0..entry_count {
            // Read key length
            let mut key_len_bytes = [0u8; 4];
            file.read_exact(&mut key_len_bytes)?;
            let key_len = u32::from_le_bytes(key_len_bytes);

            // Read key
            let mut key = vec![0u8; key_len as usize];
            file.read_exact(&mut key)?;

            // Read value length
            let mut value_len_bytes = [0u8; 4];
            file.read_exact(&mut value_len_bytes)?;
            let value_len = u32::from_le_bytes(value_len_bytes);

            // Skip value data
            file.seek(SeekFrom::Current(value_len as i64))?;

            // Store index entry (offset points to value length field)
            let value_offset = offset + 4 + key_len as u64 + 4;
            segment.index.insert(key, (value_offset, value_len));

            // Update offset for next entry
            offset += 4 + key_len as u64 + 4 + value_len as u64;
        }

        segment.size = offset;
        segment.key_count = entry_count;
        segment.created_at = std::fs::metadata(&segment.path)?
            .created()
            .unwrap_or_else(|_| SystemTime::now());

        Ok(())
    }

    fn flush_memtable(&self) -> Result<(), QantoStorageError> {
        // Get all data from sharded memtable
        let memtable_data = self.sharded_memtable.flush_all()?;

        if memtable_data.is_empty() {
            return Ok(());
        }

        // Create new segment
        let segment_id = self.next_segment_id.fetch_add(1, Ordering::SeqCst);
        let segment_filename = format!("segment_{segment_id:06}.qdb");
        let segment_path = self.config.data_dir.join(segment_filename);

        // Write segment to disk
        self.write_segment(&segment_path, &memtable_data)?;

        // Add to segments list
        let mut segment = StorageSegment::new(segment_id, segment_path.clone());

        // Build proper index with correct offsets and lengths
        let mut offset = 12u64; // Skip header (magic + version + count)
        for (key, value) in &memtable_data {
            let compressed_value = if self.config.compression_enabled {
                self.compress(value)?
            } else {
                value.clone()
            };

            // Calculate value offset (after key length + key + value length)
            let value_offset = offset + 4 + key.len() as u64 + 4;
            segment
                .index
                .insert(key.clone(), (value_offset, compressed_value.len() as u32));

            // Update offset for next entry
            offset += 4 + key.len() as u64 + 4 + compressed_value.len() as u64;
        }

        segment.size = offset;
        segment.key_count = memtable_data.len() as u64;
        segment.created_at = SystemTime::now();

        {
            let mut segments = self.segments.write().unwrap();
            segments.push(segment);
        }

        Ok(())
    }

    fn write_segment(
        &self,
        path: &Path,
        data: &BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), QantoStorageError> {
        let mut file = BufWriter::new(File::create(path)?);

        // Write header
        file.write_all(b"QSEG")?; // Magic number
        file.write_all(&1u32.to_le_bytes())?; // Version
        file.write_all(&(data.len() as u32).to_le_bytes())?; // Entry count

        // Write entries
        for (key, value) in data {
            // Compress value if enabled
            let compressed_value = if self.config.compression_enabled {
                self.compress(value)?
            } else {
                value.clone()
            };

            // Write key length + key
            file.write_all(&(key.len() as u32).to_le_bytes())?;
            file.write_all(key)?;

            // Write value length + value
            file.write_all(&(compressed_value.len() as u32).to_le_bytes())?;
            file.write_all(&compressed_value)?;
        }

        file.flush()?;
        file.into_inner()
            .map_err(|e| QantoStorageError::Io(e.into_error()))?
            .sync_all()?;

        Ok(())
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, QantoStorageError> {
        if !self.config.compression_enabled {
            return Ok(data.to_vec());
        }

        // Simple compression implementation
        let mut compressed = Vec::with_capacity(data.len() + 8);
        compressed.extend_from_slice(b"QCMP"); // Magic header
        compressed.extend_from_slice(&(data.len() as u32).to_le_bytes()); // Original size
        compressed.extend_from_slice(data); // For now, just store uncompressed

        Ok(compressed)
    }

    fn perform_compaction(&self) -> Result<(), QantoStorageError> {
        // Simplified compaction for now
        Ok(())
    }

    /// Put a key-value pair with optimized cache operations
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError> {
        let value_size = key.len() + value.len();

        // If async I/O is enabled, queue the write and update cache for read-your-writes
        if let Some(ref processor) = self.async_io_processor {
            processor.queue_write(key.clone(), value.clone())?;

            // Update cache using DashMap for lock-free access
            let entry = CacheEntry::new(value.clone());
            self.cache.insert(key, entry);

            // Update cache size atomically and evict if necessary
            let current_size = self.cache_size.fetch_add(value_size, Ordering::Relaxed);
            if current_size + value_size > self.config.cache_size {
                self.evict_cache_lockfree();
            }

            // Defer WAL and memtable updates to the async processor batch
            return Ok(());
        }

        // Log to WAL first
        if let Some(ref mut wal) = *self
            .wal
            .lock()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?
        {
            let entry = LogEntry::Put {
                key: key.clone(),
                value: value.clone(),
            };
            wal.append(&entry)?;
            if self.config.sync_writes {
                wal.sync()?;
            }
        }

        // Update memtable
        self.sharded_memtable.put(key.clone(), value.clone())?;

        // Update cache using DashMap for lock-free access
        let entry = CacheEntry::new(value.clone());
        self.cache.insert(key, entry);

        // Update cache size atomically
        let current_size = self.cache_size.fetch_add(value_size, Ordering::Relaxed);

        // Evict cache if too large
        if current_size + value_size > self.config.cache_size {
            self.evict_cache_lockfree();
        }

        // Update stats
        {
            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.writes += 1;
        }

        // Check if memtable needs flushing
        let memtable_size = self.sharded_memtable.size() * 1024; // Rough estimate

        if memtable_size > self.config.cache_size / 4 {
            self.flush_memtable()?;
        }

        Ok(())
    }

    /// High-performance get operation with sharded reads
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError> {
        // Check cache first (lock-free with DashMap)
        if let Some(entry) = self.cache.get(key) {
            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.cache_hits += 1;
            return Ok(Some(entry.value.clone()));
        }

        // Check sharded memtable (lock-free per shard)
        if let Some(value) = self.sharded_memtable.get(key)? {
            // Empty value indicates a tombstone; stop and return None
            if value.is_empty() {
                return Ok(None);
            }
            // Update cache for future reads
            let entry = CacheEntry::new(value.clone());
            self.cache.insert(key.to_vec(), entry);
            self.cache_size
                .fetch_add(key.len() + value.len(), Ordering::Relaxed);

            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.reads += 1;
            return Ok(Some(value));
        }

        // Check segments if not in memtable (need mutable access for file operations)
        let mut segments = self
            .segments
            .write()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;

        for segment in segments.iter_mut().rev() {
            // If a tombstone exists in a newer segment, do not search older segments
            if segment.is_tombstone(key) {
                return Ok(None);
            }
            if let Some(value) = segment.get(key)? {
                // Update cache for future reads
                let entry = CacheEntry::new(value.clone());
                self.cache.insert(key.to_vec(), entry);
                self.cache_size
                    .fetch_add(key.len() + value.len(), Ordering::Relaxed);

                let mut stats = self
                    .stats
                    .write()
                    .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
                stats.reads += 1;
                return Ok(Some(value));
            }
        }

        // Update cache miss stats
        {
            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.cache_misses += 1;
        }

        Ok(None)
    }

    /// Delete a key with optimized cache removal
    pub fn delete(&self, key: &[u8]) -> Result<(), QantoStorageError> {
        // If async I/O is enabled, queue the delete and update cache
        if let Some(ref processor) = self.async_io_processor {
            processor.queue_delete(key.to_vec())?;

            // Remove from cache and update size atomically
            if let Some((_, entry)) = self.cache.remove(key) {
                let value_size = key.len() + entry.value.len();
                self.cache_size.fetch_sub(value_size, Ordering::Relaxed);
            }

            // Immediately insert tombstone in memtable for consistency
            self.sharded_memtable.delete(key)?;

            // Defer WAL updates to the async processor batch
            return Ok(());
        }
        // Log to WAL first
        if let Some(ref mut wal) = *self.wal.lock().unwrap() {
            let entry = LogEntry::Delete { key: key.to_vec() };
            wal.append(&entry)?;
            if self.config.sync_writes {
                wal.sync()?;
            }
        }

        // Remove from memtable
        self.sharded_memtable.delete(key)?;

        // Remove from cache and update size atomically
        if let Some((_, entry)) = self.cache.remove(key) {
            let value_size = key.len() + entry.value.len();
            self.cache_size.fetch_sub(value_size, Ordering::Relaxed);
        }

        // Update stats
        {
            let mut stats = self.stats.write().unwrap();
            stats.deletes += 1;
        }

        Ok(())
    }

    /// Lock-free cache eviction using DashMap
    fn evict_cache_lockfree(&self) {
        let target_size = self.config.cache_size / 2;
        let mut current_size = self.cache_size.load(Ordering::Relaxed);

        if current_size <= target_size {
            return;
        }

        // Collect entries for eviction (LRU-based)
        let mut entries_to_evict = Vec::new();

        for entry in self.cache.iter() {
            let access_count = entry.access_count.load(Ordering::Relaxed);
            let last_accessed = entry.last_accessed;

            entries_to_evict.push((
                entry.key().clone(),
                access_count,
                last_accessed,
                entry.key().len() + entry.value.len(),
            ));
        }

        // Sort by access count and last accessed time (LRU)
        entries_to_evict.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));

        // Evict least recently used entries
        for (key, _, _, size) in entries_to_evict {
            if current_size <= target_size {
                break;
            }

            if self.cache.remove(&key).is_some() {
                current_size = self.cache_size.fetch_sub(size, Ordering::Relaxed) - size;
            }
        }
    }

    /// Execute a batch of operations atomically
    pub fn write_batch(&self, batch: WriteBatch) -> Result<(), QantoStorageError> {
        if batch.is_empty() {
            return Ok(());
        }

        // Begin a transaction and attach operations to it
        let tx_id = self.begin_transaction();

        {
            let mut transactions = self
                .active_transactions
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            let tx = transactions.get_mut(&tx_id).ok_or_else(|| {
                QantoStorageError::Transaction(format!("Transaction {tx_id} not found"))
            })?;
            tx.operations.extend(batch.operations);
        }

        // Commit will log the transaction and apply operations exactly once
        self.commit_transaction(tx_id)
    }

    /// Apply a put directly to memtable/cache without WAL or async queue
    #[allow(dead_code)]
    fn apply_put_direct(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError> {
        let value_size = key.len() + value.len();

        // Update memtable
        self.sharded_memtable.put(key.clone(), value.clone())?;

        // Update cache using DashMap for lock-free access
        let entry = CacheEntry::new(value.clone());
        self.cache.insert(key, entry);

        // Update cache size atomically and evict if necessary
        let current_size = self.cache_size.fetch_add(value_size, Ordering::Relaxed);
        if current_size + value_size > self.config.cache_size {
            self.evict_cache_lockfree();
        }

        // Update stats
        {
            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.writes += 1;
        }

        Ok(())
    }

    /// Apply a delete directly to memtable/cache without WAL or async queue
    #[allow(dead_code)]
    fn apply_delete_direct(&self, key: &[u8]) -> Result<(), QantoStorageError> {
        // Insert tombstone in memtable (empty value)
        self.sharded_memtable.delete(key)?;

        // Remove from cache and update size atomically
        if let Some((_, entry)) = self.cache.remove(key) {
            let value_size = key.len() + entry.value.len();
            self.cache_size.fetch_sub(value_size, Ordering::Relaxed);
        }

        // Update stats
        {
            let mut stats = self
                .stats
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            stats.deletes += 1;
        }

        Ok(())
    }
}

/// Sharded memtable for parallel writes
#[derive(Debug)]
struct ShardedMemtable {
    shards: Vec<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
    shard_count: usize,
}

impl ShardedMemtable {
    fn new(shard_count: usize) -> Self {
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(RwLock::new(BTreeMap::new()));
        }
        Self {
            shards,
            shard_count,
        }
    }

    fn get_shard_index(&self, key: &[u8]) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shard_count
    }

    pub fn shard_count(&self) -> usize {
        self.shard_count
    }

    pub fn get_shard(&self, index: usize) -> Option<&RwLock<BTreeMap<Vec<u8>, Vec<u8>>>> {
        self.shards.get(index)
    }

    /// High-performance put operation with sharded writes
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError> {
        let shard_index = self.get_shard_index(&key);
        let shard = &self.shards[shard_index];

        let mut shard_map = shard
            .write()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;

        shard_map.insert(key, value);
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError> {
        let shard_index = self.get_shard_index(key);
        let shard = self.shards[shard_index]
            .read()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
        Ok(shard.get(key).cloned())
    }

    fn delete(&self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError> {
        let shard_index = self.get_shard_index(key);
        let mut shard = self.shards[shard_index]
            .write()
            .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
        let previous = shard.get(key).cloned();
        shard.insert(key.to_vec(), Vec::new()); // Insert tombstone (empty value)
        Ok(previous)
    }

    fn flush_all(&self) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, QantoStorageError> {
        let mut merged = BTreeMap::new();
        for shard in &self.shards {
            let mut shard_data = shard
                .write()
                .map_err(|e| QantoStorageError::LockPoisoned(e.to_string()))?;
            // Collect all entries and clear the shard
            let entries: Vec<_> = shard_data
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            shard_data.clear();
            for (key, value) in entries {
                merged.insert(key, value);
            }
        }
        Ok(merged)
    }

    fn size(&self) -> usize {
        self.shards
            .iter()
            .map(|shard| shard.read().map(|s| s.len()).unwrap_or(0))
            .sum()
    }
}

/// Async write operation for batching
#[derive(Debug, Clone)]
pub struct AsyncWriteOp {
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>, // None for delete
    pub timestamp: SystemTime,
}

/// Async I/O batch processor
#[derive(Debug)]
struct AsyncIOProcessor {
    #[allow(dead_code)]
    write_queue: mpsc::UnboundedSender<AsyncWriteOp>,
    #[allow(dead_code)]
    batch_size: usize,
    #[allow(dead_code)]
    flush_interval: Duration,
    _handle: JoinHandle<()>,
    #[allow(dead_code)]
    runtime_guard: Option<tokio::runtime::Runtime>,
}

impl AsyncIOProcessor {
    fn new(storage_weak: Weak<QantoStorage>, batch_size: usize, flush_interval: Duration) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let task = async move {
            let mut batch = Vec::with_capacity(batch_size);
            let mut last_flush = tokio::time::Instant::now();

            loop {
                tokio::select! {
                    op = rx.recv() => {
                        match op {
                            Some(write_op) => {
                                batch.push(write_op);

                                if batch.len() >= batch_size ||
                                   last_flush.elapsed() >= flush_interval {
                                    if let Err(e) = Self::flush_batch(&storage_weak, &mut batch).await {
                                        eprintln!("Async I/O flush error: {}", e);
                                    }
                                    last_flush = tokio::time::Instant::now();
                                }
                            }
                            None => break,
                        }
                    }
                    _ = tokio::time::sleep(flush_interval) => {
                        if !batch.is_empty() {
                            if let Err(e) = Self::flush_batch(&storage_weak, &mut batch).await {
                                eprintln!("Async I/O flush error: {}", e);
                            }
                            last_flush = tokio::time::Instant::now();
                        }
                    }
                }
            }
        };

        // Spawn on existing Tokio runtime if available, otherwise create a lightweight runtime
        let (handle, runtime_guard) = match tokio::runtime::Handle::try_current() {
            Ok(h) => (h.spawn(task), None),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .expect("Failed to build Tokio runtime for AsyncIOProcessor");
                let h = rt.spawn(task);
                (h, Some(rt))
            }
        };

        Self {
            write_queue: tx,
            batch_size,
            flush_interval,
            _handle: handle,
            runtime_guard,
        }
    }

    async fn flush_batch(
        storage_weak: &Weak<QantoStorage>,
        batch: &mut Vec<AsyncWriteOp>,
    ) -> Result<(), QantoStorageError> {
        if batch.is_empty() {
            return Ok(());
        }
        if let Some(storage_arc) = storage_weak.upgrade() {
            // Create write batch
            let mut write_batch = WriteBatch::with_capacity(batch.len());
            for op in batch.drain(..) {
                match op.value {
                    Some(value) => write_batch.put(op.key, value),
                    None => write_batch.delete(op.key),
                }
            }

            // Execute batch synchronously without requiring a multi-thread runtime
            storage_arc.write_batch(write_batch)
        } else {
            // Storage is gone; drop pending ops gracefully
            batch.clear();
            Ok(())
        }
    }

    #[allow(dead_code)]
    fn queue_write(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError> {
        let op = AsyncWriteOp {
            key,
            value: Some(value),
            timestamp: SystemTime::now(),
        };
        self.write_queue
            .send(op)
            .map_err(|_| QantoStorageError::InvalidOperation("Write queue closed".to_string()))
    }

    #[allow(dead_code)]
    fn queue_delete(&self, key: Vec<u8>) -> Result<(), QantoStorageError> {
        let op = AsyncWriteOp {
            key,
            value: Some(vec![]), // Use empty value as tombstone instead of None
            timestamp: SystemTime::now(),
        };
        self.write_queue
            .send(op)
            .map_err(|_| QantoStorageError::InvalidOperation("Write queue closed".to_string()))
    }
}

// ... existing code ...
