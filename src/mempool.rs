//! --- Qanto Mempool ---
//! v0.1.0 - Initial Version
//!
//! This version implements a basic mempool with transaction prioritization
//! and UTXO tracking.
use crate::memory_optimization::TransactionArena;
use crate::qantodag::QantoDAG;
use crate::transaction::{Transaction, TransactionError};
use crate::types::UTXO;
use dashmap::DashMap;
// use rayon::prelude::*; // Removed unused import
use parking_lot::RwLock as ParkingRwLock;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum MempoolError {
    #[error("Transaction validation failed: {0}")]
    TransactionValidation(String),
    #[error("Mempool is full")]
    MempoolFull,
    #[error("Transaction error: {0}")]
    Tx(#[from] TransactionError),
    #[error("Timestamp error")]
    TimestampError,
    #[error("Backpressure activated: {0}")]
    BackpressureActive(String),
}

// A wrapper to make transactions orderable by fee-per-byte and track their age.
#[derive(Debug, Clone, PartialEq)]
pub struct PrioritizedTransaction {
    pub tx: Transaction,
    pub fee_per_byte: u64,
    pub timestamp: u64,
}

// Custom Eq implementation that ignores floating-point fields in Transaction
impl Eq for PrioritizedTransaction {}

impl Ord for PrioritizedTransaction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // For eviction priority: lower fee per byte should be evicted first
        // Since we use Reverse in BinaryHeap, we want lower fees to have "higher" priority for eviction
        match self.fee_per_byte.cmp(&other.fee_per_byte) {
            std::cmp::Ordering::Equal => {
                // Secondary: Compare by age (older is better for eviction)
                self.timestamp.cmp(&other.timestamp)
            }
            other => other,
        }
    }
}

impl PartialOrd for PrioritizedTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug)]
pub struct Mempool {
    transactions: Arc<RwLock<HashMap<String, PrioritizedTransaction>>>,
    priority_queue: Arc<RwLock<BTreeMap<u64, Vec<String>>>>,
    priority_heap: Arc<ParkingRwLock<BinaryHeap<PrioritizedTransaction>>>,
    max_age: Duration,
    max_size_bytes: usize,
    current_size_bytes: Arc<AtomicUsize>,
    // Add counters for less verbose logging
    tx_counter: Arc<AtomicUsize>,
    last_log_time: Arc<RwLock<Instant>>,
    log_interval: Duration,
    // Sharding
    shard_count: usize,
    sharded_heaps: Vec<Arc<ParkingRwLock<BinaryHeap<PrioritizedTransaction>>>>,
}

/// Optimized mempool implementation using DashMap and BinaryHeap for high performance
#[derive(Debug)]
pub struct OptimizedMempool {
    transactions: DashMap<String, PrioritizedTransaction>,
    priority_queue: Arc<RwLock<BinaryHeap<Reverse<PrioritizedTransaction>>>>,
    max_age: Duration,
    max_size_bytes: usize,
    current_size_bytes: Arc<std::sync::atomic::AtomicUsize>,
    // Add counters for less verbose logging
    tx_counter: Arc<AtomicUsize>,
    last_log_time: Arc<RwLock<Instant>>,
    log_interval: Duration,
    // Backpressure configuration
    backpressure_threshold: f64, // Percentage of max_size_bytes to trigger backpressure
    backpressure_active: Arc<std::sync::atomic::AtomicBool>,
    rejected_tx_counter: Arc<AtomicUsize>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PriorityEntry {
    fee_per_byte: u64,
    timestamp: u64,
    tx_id: String,
}

/// Enhanced mempool for 1M+ transaction capacity with optimized performance
#[derive(Debug)]
#[allow(dead_code)]
pub struct EnhancedMempool {
    transactions: DashMap<String, PrioritizedTransaction>,
    priority_queue: Arc<RwLock<BinaryHeap<Reverse<PrioritizedTransaction>>>>,
    max_age: Duration,
    max_size_bytes: usize,
    current_size_bytes: Arc<std::sync::atomic::AtomicUsize>,
    max_transactions: usize,
    tx_counter: Arc<AtomicUsize>,
    last_log_time: Arc<RwLock<Instant>>,
    log_interval: Duration,
    backpressure_threshold: f64,
    backpressure_active: Arc<std::sync::atomic::AtomicBool>,
    rejected_tx_counter: Arc<AtomicUsize>,
    // Enhanced fields for 1M+ capacity
    shard_count: usize,
    sharded_queues: Vec<Arc<RwLock<BinaryHeap<Reverse<PrioritizedTransaction>>>>>,
    performance_metrics: Arc<MempoolPerformanceMetrics>,
    // Arena allocator for efficient transaction management
    tx_arena: Arc<TransactionArena>,
}

#[derive(Debug)]
pub struct MempoolPerformanceMetrics {
    pub total_transactions_processed: Arc<AtomicUsize>,
    pub total_transactions_rejected: Arc<AtomicUsize>,
    pub average_processing_time_us: Arc<AtomicUsize>,
    pub peak_memory_usage_bytes: Arc<AtomicUsize>,
    pub cache_hit_ratio: Arc<AtomicUsize>,
}

impl EnhancedMempool {
    pub fn new_enhanced(
        max_age: Duration,
        max_size_bytes: usize,
        max_transactions: usize,
        backpressure_threshold: f64,
        shard_count: usize,
    ) -> Self {
        let sharded_queues: Vec<Arc<RwLock<BinaryHeap<Reverse<PrioritizedTransaction>>>>> = (0
            ..shard_count)
            .map(|_| Arc::new(RwLock::new(BinaryHeap::new())))
            .collect();

        Self {
            transactions: DashMap::new(),
            priority_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            max_age,
            max_size_bytes,
            current_size_bytes: Arc::new(AtomicUsize::new(0)),
            max_transactions,
            tx_counter: Arc::new(AtomicUsize::new(0)),
            last_log_time: Arc::new(RwLock::new(Instant::now())),
            log_interval: Duration::from_secs(30),
            backpressure_threshold,
            backpressure_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            rejected_tx_counter: Arc::new(AtomicUsize::new(0)),
            shard_count,
            sharded_queues,
            performance_metrics: Arc::new(MempoolPerformanceMetrics {
                total_transactions_processed: Arc::new(AtomicUsize::new(0)),
                total_transactions_rejected: Arc::new(AtomicUsize::new(0)),
                average_processing_time_us: Arc::new(AtomicUsize::new(0)),
                peak_memory_usage_bytes: Arc::new(AtomicUsize::new(0)),
                cache_hit_ratio: Arc::new(AtomicUsize::new(0)),
            }),
            tx_arena: Arc::new(TransactionArena::new(2).unwrap()), // 2MB arena for enhanced mempool
        }
    }

    /// Calculate shard index for a transaction
    fn calculate_shard_index(&self, tx_id: &str) -> usize {
        let hash = tx_id
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        (hash as usize) % self.shard_count
    }

    /// Add transaction with enhanced sharding
    pub async fn add_transaction_enhanced(
        &self,
        transaction: Transaction,
    ) -> Result<(), MempoolError> {
        let start_time = Instant::now();
        let tx_id = transaction.id.clone();

        // Check if transaction already exists
        if self.transactions.contains_key(&tx_id) {
            return Ok(());
        }

        // Enhanced transaction size calculation
        let tx_size = self.calculate_enhanced_transaction_size(&transaction);
        let current_size = self.current_size_bytes.load(Ordering::Relaxed);
        let projected_size = current_size + tx_size;
        let capacity_ratio = projected_size as f64 / self.max_size_bytes as f64;

        // Enhanced backpressure with adaptive thresholds
        if capacity_ratio >= self.backpressure_threshold {
            self.backpressure_active.store(true, Ordering::Relaxed);

            let min_fee_threshold = self.calculate_adaptive_fee_threshold().await;
            let fee_per_byte = if tx_size > 0 {
                (transaction.fee * 1000) / (tx_size as u64) // Scale by 1000 for precision
            } else {
                0
            };

            if fee_per_byte < min_fee_threshold {
                self.rejected_tx_counter.fetch_add(1, Ordering::Relaxed);
                self.performance_metrics
                    .total_transactions_rejected
                    .fetch_add(1, Ordering::Relaxed);
                return Err(MempoolError::BackpressureActive(
                    format!("Transaction fee {fee_per_byte}/1000byte below threshold {min_fee_threshold}/1000byte")
                ));
            }
        } else {
            self.backpressure_active.store(false, Ordering::Relaxed);
        }

        let fee_per_byte = if tx_size > 0 {
            (transaction.fee * 1000) / (tx_size as u64)
        } else {
            0
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let prioritized_tx = PrioritizedTransaction {
            tx: transaction,
            fee_per_byte,
            timestamp,
        };

        // Add to main storage
        self.transactions
            .insert(tx_id.clone(), prioritized_tx.clone());
        self.current_size_bytes
            .fetch_add(tx_size, Ordering::Relaxed);
        // Update global metric for mempool size
        crate::set_metric!(
            mempool_size,
            self.current_size_bytes.load(Ordering::Relaxed) as u64
        );
        self.tx_counter.fetch_add(1, Ordering::Relaxed);

        // Add to appropriate shard
        let shard_index = self.calculate_shard_index(&tx_id);
        let mut shard_queue = self.sharded_queues[shard_index].write().await;
        shard_queue.push(Reverse(prioritized_tx.clone()));
        drop(shard_queue);

        // Add to main priority queue
        let mut queue = self.priority_queue.write().await;
        queue.push(Reverse(prioritized_tx));
        drop(queue);

        // Update performance metrics
        let processing_time = start_time.elapsed().as_micros() as usize;
        self.performance_metrics
            .total_transactions_processed
            .fetch_add(1, Ordering::Relaxed);
        self.performance_metrics
            .average_processing_time_us
            .store(processing_time, Ordering::Relaxed);

        let current_memory = self.current_size_bytes.load(Ordering::Relaxed);
        let peak_memory = self
            .performance_metrics
            .peak_memory_usage_bytes
            .load(Ordering::Relaxed);
        if current_memory > peak_memory {
            self.performance_metrics
                .peak_memory_usage_bytes
                .store(current_memory, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Enhanced transaction size calculation
    fn calculate_enhanced_transaction_size(&self, tx: &Transaction) -> usize {
        // More accurate size calculation
        let base_size = std::mem::size_of::<Transaction>();
        let id_size = tx.id.len();
        let sender_size = tx.sender.len();
        let receiver_size = tx.receiver.len();
        let inputs_size = tx.inputs.len() * 64; // Estimated UTXO reference size
        let outputs_size = tx.outputs.len() * 128; // Estimated output size
        let signature_size = 64; // Standard signature size

        base_size
            + id_size
            + sender_size
            + receiver_size
            + inputs_size
            + outputs_size
            + signature_size
    }

    /// Adaptive fee threshold calculation
    async fn calculate_adaptive_fee_threshold(&self) -> u64 {
        let current_size = self.current_size_bytes.load(Ordering::Relaxed);
        let capacity_ratio = current_size as f64 / self.max_size_bytes as f64;

        // Base threshold increases exponentially with capacity usage
        let base_threshold = 100u64; // Base fee per 1000 bytes
        let multiplier = if capacity_ratio > 0.9 {
            10.0
        } else if capacity_ratio > 0.8 {
            5.0
        } else if capacity_ratio > 0.7 {
            2.0
        } else {
            1.0
        };

        (base_threshold as f64 * multiplier) as u64
    }

    /// Get prioritized transactions from shards
    pub async fn get_prioritized_transactions_sharded(&self, max_count: usize) -> Vec<Transaction> {
        let mut all_transactions = Vec::new();
        let per_shard_count = max_count.div_ceil(self.shard_count);

        // Collect from each shard in parallel
        let shard_futures: Vec<_> = (0..self.shard_count)
            .map(|i| async move {
                let queue = self.sharded_queues[i].read().await;
                queue
                    .iter()
                    .take(per_shard_count)
                    .map(|reverse_tx| reverse_tx.0.tx.clone())
                    .collect::<Vec<Transaction>>()
            })
            .collect();

        // Execute all shard queries concurrently
        for shard_txs in futures::future::join_all(shard_futures).await {
            all_transactions.extend(shard_txs);
        }

        // Sort by priority and take top transactions
        all_transactions.into_iter().take(max_count).collect()
    }

    /// Enhanced pruning with parallel processing
    pub async fn prune_old_transactions_enhanced(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let old_txs: Vec<String> = self
            .transactions
            .iter()
            .filter_map(|entry| {
                let (tx_id, prioritized_tx) = entry.pair();
                if now.saturating_sub(prioritized_tx.timestamp) > self.max_age.as_secs() {
                    Some(tx_id.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove old transactions in parallel
        for tx_id in old_txs {
            if let Some((_, prioritized_tx)) = self.transactions.remove(&tx_id) {
                let tx_size = self.calculate_enhanced_transaction_size(&prioritized_tx.tx);
                self.current_size_bytes
                    .fetch_sub(tx_size, Ordering::Relaxed);

                // Remove from appropriate shard
                let shard_index = self.calculate_shard_index(&tx_id);
                let mut shard_queue = self.sharded_queues[shard_index].write().await;
                shard_queue.retain(|reverse_tx| reverse_tx.0.tx.id != tx_id);
            }
        }

        // Rebuild priority queues
        self.rebuild_all_queues().await;
        // Update global metric for mempool size
        crate::set_metric!(
            mempool_size,
            self.current_size_bytes.load(Ordering::Relaxed) as u64
        );
    }

    /// Rebuild all priority queues
    async fn rebuild_all_queues(&self) {
        // Rebuild main queue
        let mut main_queue = self.priority_queue.write().await;
        main_queue.clear();

        // Rebuild shard queues
        for shard_queue in &self.sharded_queues {
            let mut queue = shard_queue.write().await;
            queue.clear();
        }

        // Repopulate queues
        for entry in self.transactions.iter() {
            let (tx_id, prioritized_tx) = entry.pair();

            // Add to main queue
            main_queue.push(Reverse(prioritized_tx.clone()));

            // Add to appropriate shard
            let shard_index = self.calculate_shard_index(tx_id);
            let mut shard_queue = self.sharded_queues[shard_index].write().await;
            shard_queue.push(Reverse(prioritized_tx.clone()));
            drop(shard_queue);
        }
    }

    /// Get enhanced performance metrics
    pub fn get_enhanced_performance_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();

        metrics.insert(
            "total_transactions".to_string(),
            self.transactions.len() as u64,
        );
        metrics.insert(
            "current_size_bytes".to_string(),
            self.current_size_bytes.load(Ordering::Relaxed) as u64,
        );
        metrics.insert("max_size_bytes".to_string(), self.max_size_bytes as u64);
        metrics.insert(
            "backpressure_active".to_string(),
            self.backpressure_active.load(Ordering::Relaxed) as u64,
        );
        metrics.insert(
            "rejected_transactions".to_string(),
            self.rejected_tx_counter.load(Ordering::Relaxed) as u64,
        );
        metrics.insert("shard_count".to_string(), self.shard_count as u64);
        metrics.insert(
            "total_processed".to_string(),
            self.performance_metrics
                .total_transactions_processed
                .load(Ordering::Relaxed) as u64,
        );
        metrics.insert(
            "total_rejected".to_string(),
            self.performance_metrics
                .total_transactions_rejected
                .load(Ordering::Relaxed) as u64,
        );
        metrics.insert(
            "avg_processing_time_us".to_string(),
            self.performance_metrics
                .average_processing_time_us
                .load(Ordering::Relaxed) as u64,
        );
        metrics.insert(
            "peak_memory_bytes".to_string(),
            self.performance_metrics
                .peak_memory_usage_bytes
                .load(Ordering::Relaxed) as u64,
        );

        metrics
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    pub fn get_current_size_bytes(&self) -> usize {
        self.current_size_bytes.load(Ordering::Relaxed)
    }

    pub fn get_transaction_count(&self) -> usize {
        self.transactions.len()
    }
}

impl OptimizedMempool {
    pub fn new(max_age: Duration, max_size_bytes: usize) -> Self {
        Self {
            transactions: DashMap::new(),
            priority_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            max_age,
            max_size_bytes,
            current_size_bytes: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            tx_counter: Arc::new(AtomicUsize::new(0)),
            last_log_time: Arc::new(RwLock::new(Instant::now())),
            log_interval: Duration::from_secs(10), // Log every 10 seconds for high throughput
            backpressure_threshold: 0.75, // Trigger backpressure at 75% capacity for high throughput
            backpressure_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            rejected_tx_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create a new mempool with configurable backpressure threshold
    /// Set backpressure_threshold to 1.0 or higher to effectively disable backpressure
    pub fn new_with_backpressure(
        max_age: Duration,
        max_size_bytes: usize,
        backpressure_threshold: f64,
    ) -> Self {
        Self {
            transactions: DashMap::new(),
            priority_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            max_age,
            max_size_bytes,
            current_size_bytes: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            tx_counter: Arc::new(AtomicUsize::new(0)),
            last_log_time: Arc::new(RwLock::new(Instant::now())),
            log_interval: Duration::from_secs(10),
            backpressure_threshold,
            backpressure_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            rejected_tx_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    pub async fn prune_old_transactions(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let old_txs: Vec<String> = self
            .transactions
            .iter()
            .filter_map(|entry| {
                let (tx_id, prioritized_tx) = entry.pair();
                if now.saturating_sub(prioritized_tx.timestamp) > self.max_age.as_secs() {
                    Some(tx_id.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove old transactions
        for tx_id in old_txs {
            if let Some((_, prioritized_tx)) = self.transactions.remove(&tx_id) {
                let tx_size = serde_json::to_vec(&prioritized_tx.tx)
                    .map(|data| data.len())
                    .unwrap_or(0);
                self.current_size_bytes
                    .fetch_sub(tx_size, std::sync::atomic::Ordering::Relaxed);
                info!("Pruned old transaction: {}", tx_id);
            }
        }

        // Rebuild priority queue after pruning
        self.rebuild_priority_queue().await;
    }

    async fn rebuild_priority_queue(&self) {
        let mut queue = self.priority_queue.write().await;
        queue.clear();

        for entry in self.transactions.iter() {
            let (_, prioritized_tx) = entry.pair();
            queue.push(Reverse(prioritized_tx.clone()));
        }
    }

    pub async fn add_transaction(&self, transaction: Transaction) -> Result<(), MempoolError> {
        let tx_id = transaction.id.clone();

        // Check if transaction already exists
        if self.transactions.contains_key(&tx_id) {
            return Ok(());
        }

        // Pre-calculate transaction size more efficiently
        let tx_size = transaction.id.len()
            + transaction.sender.len()
            + transaction.receiver.len()
            + transaction.inputs.len() * 64
            + transaction.outputs.len() * 128
            + 256; // Estimated overhead

        let current_size = self.current_size_bytes.load(Ordering::Relaxed);
        let projected_size = current_size + tx_size;
        let capacity_ratio = projected_size as f64 / self.max_size_bytes as f64;

        // Implement backpressure mechanism
        if capacity_ratio >= self.backpressure_threshold {
            self.backpressure_active.store(true, Ordering::Relaxed);

            // Calculate minimum fee threshold based on current mempool state
            let min_fee_threshold = self.calculate_dynamic_fee_threshold().await;

            let fee_per_byte = if tx_size > 0 {
                // Use scaled calculation to avoid integer division truncation
                // Scale by 100 to preserve precision for small fees
                (transaction.fee * 100)
                    .checked_div(tx_size as u64)
                    .unwrap_or(0)
            } else {
                0
            };

            // Reject low-fee transactions during backpressure
            if fee_per_byte < min_fee_threshold {
                self.rejected_tx_counter.fetch_add(1, Ordering::Relaxed);
                return Err(MempoolError::BackpressureActive(
                    format!("Transaction fee {fee_per_byte}/byte below threshold {min_fee_threshold}/byte during backpressure")
                ));
            }
        } else {
            self.backpressure_active.store(false, Ordering::Relaxed);
        }

        let fee_per_byte = if tx_size > 0 {
            (transaction.fee * 100)
                .checked_div(tx_size as u64)
                .unwrap_or(0)
        } else {
            0
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let prioritized_tx = PrioritizedTransaction {
            tx: transaction,
            fee_per_byte,
            timestamp,
        };

        // Check if mempool is full and evict if necessary
        while self.current_size_bytes.load(Ordering::Relaxed) + tx_size > self.max_size_bytes {
            warn!(
                "Mempool size limit exceeded: current {} + tx {} > max {}. Attempting eviction.",
                self.current_size_bytes.load(Ordering::Relaxed),
                tx_size,
                self.max_size_bytes
            );
            if !self.evict_lowest_fee_transaction().await {
                warn!("Failed to evict any transaction - mempool is full");
                return Err(MempoolError::MempoolFull);
            }
            warn!(
                "After eviction: current size is now {}",
                self.current_size_bytes.load(Ordering::Relaxed)
            );
        }

        // Add transaction to mempool
        self.transactions
            .insert(tx_id.clone(), prioritized_tx.clone());

        // Update size after successful insertion
        self.current_size_bytes
            .fetch_add(tx_size, Ordering::Relaxed);
        // Update global metric for mempool size
        crate::set_metric!(
            mempool_size,
            self.current_size_bytes.load(Ordering::Relaxed) as u64
        );

        // Add to priority queue with reduced lock contention
        let mut queue = self.priority_queue.write().await;
        queue.push(Reverse(prioritized_tx));

        // Increment transaction counter
        let tx_count = self.tx_counter.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if we should log summary statistics
        let should_log = {
            let mut last_log = self.last_log_time.write().await;
            let now = Instant::now();
            if now.duration_since(*last_log) >= self.log_interval {
                *last_log = now;
                true
            } else {
                false
            }
        };

        if should_log || tx_count.is_multiple_of(1000) {
            let current_size = self.current_size_bytes.load(Ordering::Relaxed);
            let tx_count_in_pool = self.transactions.len();
            let capacity_percentage = (current_size as f64 / self.max_size_bytes as f64) * 100.0;
            let avg_fee_per_byte = if tx_count_in_pool > 0 {
                self.transactions
                    .iter()
                    .map(|entry| entry.value().fee_per_byte)
                    .sum::<u64>() as f64
                    / tx_count_in_pool as f64
            } else {
                0.0
            };

            info!(
                "Mempool summary: {} transactions processed, {} in pool, {:.1}% capacity ({}/{} bytes), avg fee/byte: {:.2}",
                tx_count,
                tx_count_in_pool,
                capacity_percentage,
                current_size,
                self.max_size_bytes,
                avg_fee_per_byte
            );
        } else {
            // Use debug level for individual transactions
            debug!(id=%tx_id, fee_per_byte=%fee_per_byte, "Added transaction to mempool");
        }

        Ok(())
    }

    /// Evicts low priority transactions based on age and fee priority
    pub async fn evict_low_priority(&self, target_bytes: usize) -> usize {
        let mut evicted_count = 0;
        let mut bytes_freed = 0;

        while bytes_freed < target_bytes {
            if !self.evict_lowest_fee_transaction().await {
                break; // No more transactions to evict
            }
            evicted_count += 1;
            bytes_freed = target_bytes.saturating_sub(self.get_current_size_bytes());
        }

        info!(
            "Evicted {} transactions, freed {} bytes",
            evicted_count, bytes_freed
        );
        evicted_count
    }

    async fn evict_lowest_fee_transaction(&self) -> bool {
        let mut queue = self.priority_queue.write().await;

        while let Some(Reverse(lowest_priority_tx)) = queue.pop() {
            let tx_id = lowest_priority_tx.tx.id.clone();

            // Check if transaction still exists (might have been removed)
            if let Some((_, removed_tx)) = self.transactions.remove(&tx_id) {
                let tx_size = serde_json::to_vec(&removed_tx.tx)
                    .map(|data| data.len())
                    .unwrap_or(0);
                self.current_size_bytes
                    .fetch_sub(tx_size, std::sync::atomic::Ordering::Relaxed);
                // Update global metric for mempool size
                crate::set_metric!(
                    mempool_size,
                    self.current_size_bytes
                        .load(std::sync::atomic::Ordering::Relaxed) as u64
                );

                warn!(
                    "Evicted transaction {} with fee_per_byte: {}",
                    tx_id, removed_tx.fee_per_byte
                );
                return true;
            }
        }

        false
    }

    pub fn get_transactions(&self) -> Vec<Transaction> {
        let mut transactions = Vec::with_capacity(self.transactions.len());
        for entry in self.transactions.iter() {
            transactions.push(entry.value().tx.clone());
        }
        transactions
    }

    pub async fn select_transactions(&self, max_count: usize) -> Vec<Transaction> {
        let mut queue = self.priority_queue.write().await;
        let mut selected = Vec::with_capacity(max_count.min(queue.len()));

        // OPTIMIZATION: Use heap's natural ordering instead of sorting
        // BinaryHeap with Reverse gives us highest fee first
        let mut temp_heap = BinaryHeap::new();

        // Extract up to max_count highest priority transactions
        for _ in 0..max_count {
            if let Some(Reverse(prioritized_tx)) = queue.pop() {
                selected.push(prioritized_tx.tx.clone());
                temp_heap.push(Reverse(prioritized_tx));
            } else {
                break;
            }
        }

        // Restore the extracted transactions back to the queue
        while let Some(tx) = temp_heap.pop() {
            queue.push(tx);
        }

        debug!(
            "Selected {} transactions for block creation",
            selected.len()
        );
        selected
    }

    pub async fn remove_transactions(&self, transaction_ids: &[String]) {
        for tx_id in transaction_ids {
            if let Some((_, removed_tx)) = self.transactions.remove(tx_id) {
                let tx_size = serde_json::to_vec(&removed_tx.tx)
                    .map(|data| data.len())
                    .unwrap_or(0);
                self.current_size_bytes
                    .fetch_sub(tx_size, std::sync::atomic::Ordering::Relaxed);
                info!("Removed transaction: {}", tx_id);
            }
        }

        // Rebuild priority queue after removals
        self.rebuild_priority_queue().await;
        // Update global metric for mempool size
        crate::set_metric!(
            mempool_size,
            self.current_size_bytes
                .load(std::sync::atomic::Ordering::Relaxed) as u64
        );
    }

    pub fn get_current_size_bytes(&self) -> usize {
        self.current_size_bytes.load(Ordering::Relaxed)
    }

    pub fn get_transaction_count(&self) -> usize {
        self.transactions.len()
    }

    pub async fn get_performance_metrics(&self) -> (usize, usize, f64) {
        let tx_count = self.get_transaction_count();
        let size_bytes = self.get_current_size_bytes();
        let utilization = if self.max_size_bytes > 0 {
            (size_bytes as f64 / self.max_size_bytes as f64) * 100.0
        } else {
            0.0
        };

        // Log metrics periodically to avoid spam
        let should_log = {
            let mut last_log = self.last_log_time.write().await;
            let now = Instant::now();
            if now.duration_since(*last_log) >= self.log_interval {
                *last_log = now;
                true
            } else {
                false
            }
        };

        if should_log {
            info!(
                "Mempool metrics - Transactions: {}, Size: {} bytes ({:.1}% utilization), Backpressure: {}",
                tx_count, size_bytes, utilization, self.backpressure_active.load(Ordering::Relaxed)
            );
        }

        (tx_count, size_bytes, utilization)
    }

    /// Calculate dynamic fee threshold based on current mempool state
    async fn calculate_dynamic_fee_threshold(&self) -> u64 {
        let current_size = self.current_size_bytes.load(Ordering::Relaxed);
        let capacity_ratio = current_size as f64 / self.max_size_bytes as f64;

        // Base fee threshold (in scaled fee per byte) - adjusted to match 100x scale factor
        let base_threshold = 100; // 1 unit per byte * 100 scale factor to match fee_per_byte calculation

        // Calculate dynamic threshold based on mempool utilization
        let dynamic_multiplier = if capacity_ratio >= 0.95 {
            20.0 // Very high fees when nearly full (increased for 10M TPS)
        } else if capacity_ratio >= 0.90 {
            10.0 // High fees when 90%+ full
        } else if capacity_ratio >= 0.80 {
            5.0 // Medium-high fees when 80%+ full
        } else if capacity_ratio >= 0.70 {
            3.0 // Medium fees when 70%+ full
        } else {
            1.0 // Base fees when below 70%
        };

        // Calculate average fee of current transactions for better threshold
        let avg_fee = if !self.transactions.is_empty() {
            let total_fee: u64 = self
                .transactions
                .iter()
                .map(|entry| entry.value().fee_per_byte)
                .sum();
            total_fee / self.transactions.len() as u64
        } else {
            base_threshold
        };

        // For high-performance mode, use more aggressive fee scaling
        let performance_multiplier = if capacity_ratio >= 0.85 {
            // When approaching capacity, scale fees exponentially
            ((capacity_ratio - 0.85) / 0.15).powf(2.0) * 5.0 + 1.0
        } else {
            1.0
        };

        // Return the higher of dynamic threshold or average fee, with performance scaling
        let threshold = std::cmp::max((base_threshold as f64 * dynamic_multiplier) as u64, avg_fee);

        (threshold as f64 * performance_multiplier) as u64
    }

    /// Get backpressure status and metrics
    pub fn get_backpressure_metrics(&self) -> (bool, usize, f64) {
        let is_active = self.backpressure_active.load(Ordering::Relaxed);
        let rejected_count = self.rejected_tx_counter.load(Ordering::Relaxed);
        let current_size = self.current_size_bytes.load(Ordering::Relaxed);
        let capacity_ratio = current_size as f64 / self.max_size_bytes as f64;

        (is_active, rejected_count, capacity_ratio)
    }

    /// Check if a transaction exists in the mempool
    pub async fn contains_transaction(&self, tx_id: &str) -> bool {
        self.transactions.contains_key(tx_id)
    }

    /// Clear transactions older than 5 minutes
    pub async fn clear_old_txs(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let five_minutes_ago = now.saturating_sub(300); // 5 minutes = 300 seconds

        let old_txs: Vec<String> = self
            .transactions
            .iter()
            .filter_map(|entry| {
                let (tx_id, prioritized_tx) = entry.pair();
                if prioritized_tx.timestamp < five_minutes_ago {
                    Some(tx_id.clone())
                } else {
                    None
                }
            })
            .collect();

        if !old_txs.is_empty() {
            info!(
                "Clearing {} transactions older than 5 minutes",
                old_txs.len()
            );

            // Remove old transactions
            for tx_id in old_txs {
                if let Some((_, prioritized_tx)) = self.transactions.remove(&tx_id) {
                    let tx_size = serde_json::to_vec(&prioritized_tx.tx)
                        .map(|data| data.len())
                        .unwrap_or(0);
                    self.current_size_bytes
                        .fetch_sub(tx_size, std::sync::atomic::Ordering::Relaxed);
                }
            }

            // Rebuild priority queue after clearing
            self.rebuild_priority_queue().await;
        }
    }
}

impl Mempool {
    pub fn new(max_age_secs: u64, max_size_bytes: usize, _max_transactions: usize) -> Self {
        let shard_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let sharded_heaps: Vec<Arc<ParkingRwLock<BinaryHeap<PrioritizedTransaction>>>> = (0
            ..shard_count)
            .map(|_| Arc::new(ParkingRwLock::new(BinaryHeap::new())))
            .collect();

        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(BTreeMap::new())),
            priority_heap: Arc::new(ParkingRwLock::new(BinaryHeap::new())),
            max_age: Duration::from_secs(max_age_secs),
            max_size_bytes,
            current_size_bytes: Arc::new(AtomicUsize::new(0)),
            tx_counter: Arc::new(AtomicUsize::new(0)),
            last_log_time: Arc::new(RwLock::new(Instant::now())),
            log_interval: Duration::from_secs(60), // Log summary every 60 seconds
            shard_count,
            sharded_heaps,
        }
    }

    pub async fn size(&self) -> usize {
        self.transactions.read().await.len()
    }

    /// Prunes transactions that have exceeded their maximum age from the mempool.
    pub async fn prune_old_transactions(&self) {
        let mut transactions = self.transactions.write().await;
        if transactions.is_empty() {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let max_age_secs = self.max_age.as_secs();

        let ids_to_prune: Vec<String> = transactions
            .iter()
            .filter(|(_, p_tx)| now.saturating_sub(p_tx.timestamp) > max_age_secs)
            .map(|(id, _)| id.clone())
            .collect();

        if ids_to_prune.is_empty() {
            return;
        }

        warn!(
            "Pruning {} stale transactions from mempool...",
            ids_to_prune.len()
        );
        let mut priority_queue = self.priority_queue.write().await;

        for id in ids_to_prune {
            if let Some(removed_ptx) = transactions.remove(&id) {
                let tx_size = serde_json::to_vec(&removed_ptx.tx)
                    .unwrap_or_default()
                    .len();
                self.current_size_bytes
                    .fetch_sub(tx_size, Ordering::Relaxed);

                if let Some(ids_at_fee) = priority_queue.get_mut(&removed_ptx.fee_per_byte) {
                    ids_at_fee.retain(|tx_id| tx_id != &id);
                    if ids_at_fee.is_empty() {
                        priority_queue.remove(&removed_ptx.fee_per_byte);
                    }
                }
            }
        }
    }

    pub async fn add_transaction(
        &self,
        tx: Transaction,
        utxos: &HashMap<String, UTXO>,
        dag: &QantoDAG,
    ) -> Result<(), MempoolError> {
        // Lightweight mempool-specific validation with detailed reasons
        if let Err(e) = tx.validate_for_mempool() {
            warn!(
                "Rejected tx {} at mempool pre-check: {}; inputs={}, outputs={}, fee={}, sender={}, receiver={}",
                tx.id,
                e,
                tx.inputs.len(),
                tx.outputs.len(),
                tx.fee,
                tx.sender,
                tx.receiver
            );
            return Err(MempoolError::Tx(e));
        }

        // Fast duplicate check in standard mempool
        {
            let transactions_read = self.transactions.read().await;
            if transactions_read.contains_key(&tx.id) {
                warn!("Rejected tx {}: duplicate transaction id", tx.id);
                return Err(MempoolError::TransactionValidation(
                    "Duplicate transaction".to_string(),
                ));
            }
        }

        // Prefetch UTXO existence before full verification
        for input in &tx.inputs {
            let utxo_id = format!("{}_{}", input.tx_id, input.output_index);
            if !utxos.contains_key(&utxo_id) {
                warn!(
                    "Rejected tx {} at prefetch: missing UTXO {}",
                    tx.id, utxo_id
                );
                return Err(MempoolError::Tx(TransactionError::InvalidStructure(
                    format!("UTXO {utxo_id} not found"),
                )));
            }
        }

        // Fast conflict pre-check against pending mempool inputs (double-spend prevention)
        {
            let tx_inputs: HashSet<String> = tx
                .inputs
                .iter()
                .map(|i| format!("{}_{}", i.tx_id, i.output_index))
                .collect();

            let transactions_guard = self.transactions.read().await;
            for (_, existing) in transactions_guard.iter() {
                for ex_in in &existing.tx.inputs {
                    let key = format!("{}_{}", ex_in.tx_id, ex_in.output_index);
                    if tx_inputs.contains(&key) {
                        warn!(
                            "Rejected tx {}: input {} conflicts with pending tx {}",
                            tx.id, key, existing.tx.id
                        );
                        return Err(MempoolError::TransactionValidation(
                            "UTXO conflict with pending transaction".to_string(),
                        ));
                    }
                }
            }
        }

        // Full verification against UTXOs and DAG
        if let Err(e) = tx.verify(dag, utxos).await {
            warn!(
                "Rejected tx {}: verification failed: {}; inputs={}, outputs={}, fee={}, sender={}, receiver={}",
                tx.id,
                e,
                tx.inputs.len(),
                tx.outputs.len(),
                tx.fee,
                tx.sender,
                tx.receiver
            );
            return Err(MempoolError::Tx(e));
        }

        let tx_id = tx.id.clone();
        let tx_size = Self::calculate_transaction_size(&tx);

        // Check if mempool is full (log with projected capacity details)
        let current_size = self.current_size_bytes.load(Ordering::Relaxed);
        let projected_size = current_size + tx_size;
        if projected_size > self.max_size_bytes {
            let capacity_percentage = (projected_size as f64 / self.max_size_bytes as f64) * 100.0;
            let fee_per_byte_dbg = if tx_size > 0 {
                tx.fee / tx_size as u64
            } else {
                0
            };
            warn!(
                "Rejected tx {}: mempool full ({:.1}% projected, {}/{} bytes). tx_size={}B, fee/byte={}",
                tx_id,
                capacity_percentage,
                projected_size,
                self.max_size_bytes,
                tx_size,
                fee_per_byte_dbg
            );
            return Err(MempoolError::MempoolFull);
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| MempoolError::TimestampError)?
            .as_secs();

        let fee_per_byte = if tx_size > 0 {
            tx.fee / tx_size as u64
        } else {
            0
        };

        let prioritized_tx = PrioritizedTransaction {
            tx: tx.clone(),
            fee_per_byte,
            timestamp,
        };

        // Add to transactions map
        {
            let mut transactions = self.transactions.write().await;
            transactions.insert(tx_id.clone(), prioritized_tx.clone());
        }

        // Add to priority queue (BTreeMap keyed by fee per byte)
        {
            let mut priority_queue = self.priority_queue.write().await;
            priority_queue
                .entry(fee_per_byte)
                .or_insert_with(Vec::new)
                .push(tx_id.clone());
        }

        // Also push into the persistent BinaryHeap for O(1) top and O(log n) updates
        {
            let mut priority_heap = self.priority_heap.write();
            priority_heap.push(prioritized_tx.clone());
        }
        // Push into the appropriate shard heap for parallel selection
        {
            let hash = tx_id
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let shard_index = (hash as usize) % self.shard_count.max(1);
            let mut shard_heap = self.sharded_heaps[shard_index].write();
            shard_heap.push(prioritized_tx);
        }

        // Update size and counter
        self.current_size_bytes
            .fetch_add(tx_size, Ordering::Relaxed);
        let tx_count = self.tx_counter.fetch_add(1, Ordering::Relaxed) + 1;

        // Optimized logging: only log summary periodically or for significant milestones
        let should_log_summary = {
            let mut last_log = self.last_log_time.write().await;
            let now = Instant::now();
            if now.duration_since(*last_log) >= self.log_interval || tx_count.is_multiple_of(1000) {
                *last_log = now;
                true
            } else {
                false
            }
        };

        if should_log_summary {
            let current_size = self.current_size_bytes.load(Ordering::Relaxed);
            let capacity_percentage = (current_size as f64 / self.max_size_bytes as f64) * 100.0;
            let transactions_count = {
                let transactions = self.transactions.read().await;
                transactions.len()
            };
            let avg_fee_per_byte = if transactions_count > 0 {
                let transactions = self.transactions.read().await;
                let total_fee_per_byte: u64 =
                    transactions.values().map(|ptx| ptx.fee_per_byte).sum();
                total_fee_per_byte as f64 / transactions_count as f64
            } else {
                0.0
            };

            info!(
                "Mempool summary: {} transactions processed, {} in pool, {:.1}% capacity ({}/{} bytes), avg fee/byte: {:.2}",
                tx_count,
                transactions_count,
                capacity_percentage,
                current_size,
                self.max_size_bytes,
                avg_fee_per_byte
            );
        } else {
            // Only debug log for individual transactions when not doing summary
            debug!(
                "Added transaction {} to mempool (fee/byte: {})",
                tx_id, fee_per_byte
            );
        }

        Ok(())
    }

    pub async fn get_transactions(&self) -> HashMap<String, Transaction> {
        let transactions = self.transactions.read().await;
        let mut result = HashMap::with_capacity(transactions.len());
        for (id, p_tx) in transactions.iter() {
            result.insert(id.clone(), p_tx.tx.clone());
        }
        result
    }

    pub async fn select_transactions(&self, max_txs: usize) -> Vec<Transaction> {
        let transactions = self.transactions.read().await;

        // Fallback to single heap when sharding is not active
        if self.shard_count <= 1 || self.sharded_heaps.is_empty() {
            let mut heap = self.priority_heap.write();
            let take = max_txs.min(heap.len());
            let mut extracted: Vec<PrioritizedTransaction> = Vec::with_capacity(take);
            let mut result: Vec<Transaction> = Vec::with_capacity(take);

            for _ in 0..take {
                if let Some(ptx) = heap.pop() {
                    if transactions.contains_key(&ptx.tx.id) {
                        result.push(ptx.tx.clone());
                        extracted.push(ptx);
                    }
                } else {
                    break;
                }
            }

            for ptx in extracted {
                heap.push(ptx);
            }

            debug!(
                "Selected {} transactions for block creation (BinaryHeap persistent)",
                result.len()
            );
            return result;
        }

        // Sharded selection: k-way merge across shard heaps
        let take = max_txs;
        let mut result: Vec<Transaction> = Vec::with_capacity(take);

        // Acquire write guards for all shard heaps
        let mut heap_guards: Vec<_> = self.sharded_heaps.iter().map(|h| h.write()).collect();

        // Initialize current tops per shard
        let mut current: Vec<Option<PrioritizedTransaction>> =
            heap_guards.iter_mut().map(|heap| heap.pop()).collect();

        // Track extracted per shard to restore later
        let mut extracted_per_shard: Vec<Vec<PrioritizedTransaction>> =
            vec![Vec::new(); heap_guards.len()];

        for _ in 0..take {
            // Find best among current candidates
            let mut best_idx: Option<usize> = None;
            for (i, opt_ptx) in current.iter().enumerate() {
                if let Some(ptx) = opt_ptx.as_ref() {
                    if let Some(bi) = best_idx {
                        if ptx > current[bi].as_ref().unwrap() {
                            best_idx = Some(i);
                        }
                    } else {
                        best_idx = Some(i);
                    }
                }
            }

            match best_idx {
                Some(i) => {
                    let ptx = current[i].take().unwrap();
                    if transactions.contains_key(&ptx.tx.id) {
                        result.push(ptx.tx.clone());
                        extracted_per_shard[i].push(ptx);
                    }
                    // Load next for this shard
                    current[i] = heap_guards[i].pop();
                }
                None => break,
            }
        }

        // Restore extracted entries
        for (i, extracted) in extracted_per_shard.into_iter().enumerate() {
            for ptx in extracted {
                heap_guards[i].push(ptx);
            }
        }

        debug!(
            "Selected {} transactions for block creation (sharded merge)",
            result.len()
        );
        result
    }

    pub async fn remove_transactions(&self, txs_to_remove: &[Transaction]) {
        if txs_to_remove.is_empty() {
            return;
        }

        let mut transactions = self.transactions.write().await;
        let mut priority_queue = self.priority_queue.write().await;

        for tx in txs_to_remove {
            if let Some(removed_ptx) = transactions.remove(&tx.id) {
                // Use accurate transaction size calculation for consistency
                let tx_size = Self::calculate_transaction_size(&removed_ptx.tx);
                self.current_size_bytes
                    .fetch_sub(tx_size, Ordering::Relaxed);

                if let Some(ids) = priority_queue.get_mut(&removed_ptx.fee_per_byte) {
                    ids.retain(|id| id != &tx.id);
                    if ids.is_empty() {
                        priority_queue.remove(&removed_ptx.fee_per_byte);
                    }
                }
            }
        }

        // Only log removal summary if removing multiple transactions or significant batch
        if txs_to_remove.len() > 1 || txs_to_remove.len() >= 10 {
            info!("Removed {} transactions from mempool.", txs_to_remove.len());
        } else if !txs_to_remove.is_empty() {
            debug!(
                "Removed {} transaction(s) from mempool.",
                txs_to_remove.len()
            );
        }
    }

    /// Get the number of transactions in the mempool
    pub async fn len(&self) -> usize {
        self.transactions.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.transactions.read().await.is_empty()
    }

    /// Get the total fees of all transactions in the mempool
    pub async fn get_total_fees(&self) -> u64 {
        let transactions = self.transactions.read().await;
        transactions.values().map(|ptx| ptx.tx.fee).sum()
    }

    /// Get all pending transactions
    pub async fn get_pending_transactions(&self) -> Vec<Transaction> {
        let transactions = self.transactions.read().await;
        let mut pending = Vec::with_capacity(transactions.len());
        for ptx in transactions.values() {
            pending.push(ptx.tx.clone());
        }
        pending
    }

    /// Accurate transaction size calculation using serialization
    fn calculate_transaction_size(tx: &Transaction) -> usize {
        // Use actual serialization for accurate size calculation
        match serde_json::to_vec(tx) {
            Ok(serialized) => serialized.len(),
            Err(_) => {
                // Fallback to improved estimation if serialization fails
                Self::estimate_transaction_size_improved(tx)
            }
        }
    }

    /// Improved transaction size estimation with more accurate field calculations
    fn estimate_transaction_size_improved(tx: &Transaction) -> usize {
        let mut size = 0;

        // Fixed fields with accurate sizes
        size += 32; // id (String, estimated)
        size += tx.sender.len(); // sender address
        size += tx.receiver.len(); // receiver address
        size += 8; // amount (u64)
        size += 8; // fee (u64)
        size += 8; // timestamp (u64)

        // Inputs: tx_id (String) + output_index (u32)
        for input in &tx.inputs {
            size += input.tx_id.len(); // tx_id string length
            size += 4; // output_index (u32)
        }

        // Outputs: address (String) + amount (u64) + homomorphic_encrypted
        for output in &tx.outputs {
            size += output.address.len(); // address string length
            size += 8; // amount (u64)
                       // HomomorphicEncrypted: ciphertext + public_key (both Vec<u8>)
            size += output.homomorphic_encrypted.ciphertext.len();
            size += output.homomorphic_encrypted.public_key.len();
        }

        // Metadata: HashMap<String, String>
        for (key, value) in &tx.metadata {
            size += key.len();
            size += value.len();
        }

        // QuantumResistantSignature: signer_public_key + signature (both Vec<u8>)
        size += tx.signature.signer_public_key.len();
        size += tx.signature.signature.len();

        // Add overhead for JSON serialization (brackets, quotes, commas, etc.)
        // Estimated at ~20% overhead for JSON format
        size + (size / 5)
    }

    /// Fast transaction size estimation without serialization (legacy method)
    #[allow(dead_code)]
    fn estimate_transaction_size(tx: &Transaction) -> usize {
        // Base transaction overhead
        let mut size = 64; // Fixed fields: id, fee, timestamp, etc.

        // Estimate input sizes
        size += tx.inputs.len() * 80; // Each input: tx_id (32) + output_index (4) + signature (44)

        // Estimate output sizes
        size += tx.outputs.len() * 40; // Each output: amount (8) + address (32)

        size
    }

    /// Get current size of mempool in bytes
    pub fn get_current_size_bytes(&self) -> usize {
        self.current_size_bytes.load(Ordering::Relaxed)
    }

    /// Add a batch of transactions under a single API, returning accepted ids and rejected reasons
    pub async fn add_transaction_batch(
        &self,
        txs: Vec<Transaction>,
        utxos: &HashMap<String, UTXO>,
        dag: &QantoDAG,
    ) -> (Vec<String>, Vec<(String, String)>) {
        if txs.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let start = Instant::now();
        let mut accepted: Vec<String> = Vec::new();
        let mut rejected: Vec<(String, String)> = Vec::new();

        // Shard-aware ordering to reduce lock contention
        let shard_slots = self.shard_count.max(1);
        let mut buckets: Vec<Vec<Transaction>> = vec![Vec::new(); shard_slots];
        for tx in txs {
            let hash = tx
                .id
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let idx = (hash as usize) % shard_slots;
            buckets[idx].push(tx);
        }

        for bucket in buckets.into_iter() {
            for tx in bucket {
                match self.add_transaction(tx.clone(), utxos, dag).await {
                    Ok(_) => accepted.push(tx.id.clone()),
                    Err(e) => rejected.push((tx.id.clone(), e.to_string())),
                }
            }
        }

        let elapsed = start.elapsed();
        debug!(
            "Batch mempool add: accepted={}, rejected={}, elapsed={:?}",
            accepted.len(),
            rejected.len(),
            elapsed
        );

        (accepted, rejected)
    }
}
