//! --- Qanto Mempool ---
//! v2.5.0 - Production Hardening & Fixes
//! This version corrects a variable ownership bug and continues to harden the
//! mempool's logic for continuous, high-throughput operation by activating
//! the transaction pruning mechanism.

use crate::qantodag::QantoDAG;
use crate::transaction::{Transaction, TransactionError};
use crate::types::UTXO;
use dashmap::DashMap;
// use rayon::prelude::*; // Removed unused import
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
}

// A wrapper to make transactions orderable by fee-per-byte and track their age.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrioritizedTransaction {
    tx: Transaction,
    fee_per_byte: u64,
    timestamp: u64,
}

impl Ord for PrioritizedTransaction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.fee_per_byte.cmp(&other.fee_per_byte)
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
    max_age: Duration,
    max_size_bytes: usize,
    current_size_bytes: Arc<AtomicUsize>,
}

/// Optimized mempool implementation using DashMap and BinaryHeap for high performance
#[derive(Debug)]
pub struct OptimizedMempool {
    transactions: DashMap<String, PrioritizedTransaction>,
    priority_queue: Arc<RwLock<BinaryHeap<Reverse<PrioritizedTransaction>>>>,
    max_age: Duration,
    max_size_bytes: usize,
    current_size_bytes: Arc<std::sync::atomic::AtomicUsize>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PriorityEntry {
    fee_per_byte: u64,
    timestamp: u64,
    tx_id: String,
}

impl OptimizedMempool {
    pub fn new(max_age: Duration, max_size_bytes: usize) -> Self {
        Self {
            transactions: DashMap::with_capacity(max_size_bytes / 512), // Estimate avg tx size
            priority_queue: Arc::new(RwLock::new(BinaryHeap::with_capacity(max_size_bytes / 512))),
            max_age,
            max_size_bytes,
            current_size_bytes: Arc::new(AtomicUsize::new(0)),
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
            if !self.evict_lowest_fee_transaction().await {
                return Err(MempoolError::MempoolFull);
            }
        }

        // Add transaction and update size atomically
        if self
            .transactions
            .insert(tx_id.clone(), prioritized_tx.clone())
            .is_none()
        {
            self.current_size_bytes
                .fetch_add(tx_size, Ordering::Relaxed);
        }

        // Add to priority queue with reduced lock contention
        let mut queue = self.priority_queue.write().await;
        queue.push(Reverse(prioritized_tx));

        info!(
            "Added transaction {} with fee_per_byte: {}",
            tx_id, fee_per_byte
        );
        Ok(())
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
    }

    pub fn get_current_size_bytes(&self) -> usize {
        self.current_size_bytes.load(Ordering::Relaxed)
    }

    pub fn get_transaction_count(&self) -> usize {
        self.transactions.len()
    }

    pub async fn get_performance_metrics(&self) -> (usize, usize, f64) {
        let tx_count = self.transactions.len();
        let size_bytes = self
            .current_size_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        let avg_fee_per_byte = if tx_count > 0 {
            let total_fee_per_byte: u64 = self
                .transactions
                .iter()
                .map(|entry| entry.value().fee_per_byte)
                .sum();
            total_fee_per_byte as f64 / tx_count as f64
        } else {
            0.0
        };

        (tx_count, size_bytes, avg_fee_per_byte)
    }
}

impl Mempool {
    pub fn new(max_age_secs: u64, max_size_bytes: usize, _max_transactions: usize) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(BTreeMap::new())),
            max_age: Duration::from_secs(max_age_secs),
            max_size_bytes,
            current_size_bytes: Arc::new(AtomicUsize::new(0)),
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
        tx.verify(dag, utxos).await?;

        if rand::random::<u8>() < 10 {
            // Prune roughly 4% of the time to keep the mempool clean.
            self.prune_old_transactions().await;
        }

        let tx_size = serde_json::to_vec(&tx).unwrap_or_default().len();
        if tx_size == 0 {
            return Ok(());
        }

        let fee_per_byte = (tx.fee * 100).checked_div(tx_size as u64).unwrap_or(0);

        let mut transactions = self.transactions.write().await;
        let mut priority_queue = self.priority_queue.write().await;

        // --- Eviction logic to make space for higher-fee transactions ---
        while self.current_size_bytes.load(Ordering::Relaxed) + tx_size > self.max_size_bytes {
            if let Some((&lowest_fee, ids)) = priority_queue.iter_mut().next() {
                if lowest_fee >= fee_per_byte {
                    return Err(MempoolError::MempoolFull);
                }
                if let Some(id_to_evict) = ids.pop() {
                    if let Some(evicted_tx) = transactions.remove(&id_to_evict) {
                        let evicted_size =
                            serde_json::to_vec(&evicted_tx.tx).unwrap_or_default().len();
                        self.current_size_bytes
                            .fetch_sub(evicted_size, Ordering::Relaxed);
                        warn!(id=%id_to_evict, "Mempool full. Evicting transaction to make space.");
                    }
                }
                if ids.is_empty() {
                    priority_queue.remove(&lowest_fee);
                }
            } else {
                return Err(MempoolError::MempoolFull);
            }
        }

        let tx_id = tx.id.clone();
        let prioritized_tx = PrioritizedTransaction {
            tx,
            fee_per_byte,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        if transactions.insert(tx_id.clone(), prioritized_tx).is_none() {
            self.current_size_bytes
                .fetch_add(tx_size, Ordering::Relaxed);
            // **FIX (E0382):** The info log is placed *before* `tx_id` is moved into the priority queue.
            let current_size = self.current_size_bytes.load(Ordering::Relaxed);
            info!(id=%tx_id, "Added transaction to mempool. Current size: {} bytes", current_size);
            priority_queue.entry(fee_per_byte).or_default().push(tx_id);
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
        let mut tx_vec: Vec<_> = transactions.values().collect();

        // Sort by fee per byte in descending order (highest first)
        tx_vec.sort_unstable_by(|a, b| b.fee_per_byte.cmp(&a.fee_per_byte));

        let mut result = Vec::with_capacity(max_txs.min(tx_vec.len()));
        for ptx in tx_vec.into_iter().take(max_txs) {
            result.push(ptx.tx.clone());
        }
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
                // OPTIMIZATION: Estimate transaction size instead of expensive serialization
                let tx_size = Self::estimate_transaction_size(&removed_ptx.tx);
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
        info!("Removed {} transactions from mempool.", txs_to_remove.len());
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

    /// Fast transaction size estimation without serialization
    fn estimate_transaction_size(tx: &Transaction) -> usize {
        // Base transaction overhead
        let mut size = 64; // Fixed fields: id, fee, timestamp, etc.

        // Estimate input sizes
        size += tx.inputs.len() * 80; // Each input: tx_id (32) + output_index (4) + signature (44)

        // Estimate output sizes
        size += tx.outputs.len() * 40; // Each output: amount (8) + address (32)

        size
    }
}
