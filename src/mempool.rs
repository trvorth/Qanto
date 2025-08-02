//! --- Qanto Mempool ---
//! v2.5.0 - Production Hardening & Fixes
//! This version corrects a variable ownership bug and continues to harden the
//! mempool's logic for continuous, high-throughput operation by activating
//! the transaction pruning mechanism.

use crate::qantodag::{QantoDAG, UTXO};
use crate::transaction::{Transaction, TransactionError};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{info, instrument, warn};

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
    current_size_bytes: Arc<RwLock<usize>>,
}

impl Mempool {
    #[instrument]
    pub fn new(max_age_secs: u64, max_size_bytes: usize, _max_transactions: usize) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(BTreeMap::new())),
            max_age: Duration::from_secs(max_age_secs),
            max_size_bytes,
            current_size_bytes: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn size(&self) -> usize {
        self.transactions.read().await.len()
    }

    /// Prunes transactions that have exceeded their maximum age from the mempool.
    #[instrument(skip(self))]
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
        let mut current_size = self.current_size_bytes.write().await;

        for id in ids_to_prune {
            if let Some(removed_ptx) = transactions.remove(&id) {
                let tx_size = serde_json::to_vec(&removed_ptx.tx)
                    .unwrap_or_default()
                    .len();
                *current_size = current_size.saturating_sub(tx_size);

                if let Some(ids_at_fee) = priority_queue.get_mut(&removed_ptx.fee_per_byte) {
                    ids_at_fee.retain(|tx_id| tx_id != &id);
                    if ids_at_fee.is_empty() {
                        priority_queue.remove(&removed_ptx.fee_per_byte);
                    }
                }
            }
        }
    }

    #[instrument(skip(self, tx, utxos, dag))]
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
        let mut current_size = self.current_size_bytes.write().await;

        // --- Eviction logic to make space for higher-fee transactions ---
        while *current_size + tx_size > self.max_size_bytes {
            if let Some((&lowest_fee, ids)) = priority_queue.iter_mut().next() {
                if lowest_fee >= fee_per_byte {
                    return Err(MempoolError::MempoolFull);
                }
                if let Some(id_to_evict) = ids.pop() {
                    if let Some(evicted_tx) = transactions.remove(&id_to_evict) {
                        let evicted_size =
                            serde_json::to_vec(&evicted_tx.tx).unwrap_or_default().len();
                        *current_size = current_size.saturating_sub(evicted_size);
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
            *current_size += tx_size;
            // **FIX (E0382):** The info log is placed *before* `tx_id` is moved into the priority queue.
            info!(id=%tx_id, "Added transaction to mempool. Current size: {} bytes", *current_size);
            priority_queue.entry(fee_per_byte).or_default().push(tx_id);
        }
        Ok(())
    }

    pub async fn get_transactions(&self) -> HashMap<String, Transaction> {
        let transactions = self.transactions.read().await;
        transactions
            .iter()
            .map(|(id, p_tx)| (id.clone(), p_tx.tx.clone()))
            .collect()
    }

    pub async fn select_transactions(&self, max_txs: usize) -> Vec<Transaction> {
        let transactions = self.transactions.read().await;
        let priority_queue = self.priority_queue.read().await;
        let mut selected = Vec::with_capacity(max_txs);

        // Iterate from highest fee to lowest
        for ids in priority_queue.values().rev() {
            for id in ids {
                if selected.len() >= max_txs {
                    return selected;
                }
                if let Some(ptx) = transactions.get(id) {
                    selected.push(ptx.tx.clone());
                }
            }
        }
        selected
    }

    #[instrument(skip(self, txs_to_remove))]
    pub async fn remove_transactions(&self, txs_to_remove: &[Transaction]) {
        if txs_to_remove.is_empty() {
            return;
        }

        let mut transactions = self.transactions.write().await;
        let mut priority_queue = self.priority_queue.write().await;
        let mut current_size = self.current_size_bytes.write().await;

        for tx in txs_to_remove {
            if let Some(removed_ptx) = transactions.remove(&tx.id) {
                let tx_size = serde_json::to_vec(&removed_ptx.tx)
                    .unwrap_or_default()
                    .len();
                *current_size = current_size.saturating_sub(tx_size);

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
}
