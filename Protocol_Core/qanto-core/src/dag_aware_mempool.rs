//! DAG-Aware Mempool Implementation
//!
//! High-performance mempool with DAG-aware transaction ordering, UTXO reservation,
//! parallel validation, and optimized selection algorithms for 32 BPS and 10M+ TPS.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum DAGMempoolError {
    #[error("Transaction already exists: {0}")]
    TransactionExists(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("UTXO conflict detected: {0}")]
    UTXOConflict(String),
    #[error("Dependency cycle detected")]
    DependencyCycle,
    #[error("Mempool capacity exceeded")]
    CapacityExceeded,
    #[error("Validation timeout")]
    ValidationTimeout,
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Simplified transaction structure for DAG mempool
#[derive(Debug, Clone, PartialEq)]
pub struct DAGTransaction {
    pub id: String,
    pub inputs: Vec<String>,  // Input UTXO IDs
    pub outputs: Vec<String>, // Output UTXO IDs
    pub fee: u64,
    pub size: usize,
    pub timestamp: u64,
    pub dependencies: HashSet<String>, // Transaction dependencies
    pub dag_level: u32,
    pub priority_score: f64,
}

impl DAGTransaction {
    pub fn new(id: String, inputs: Vec<String>, outputs: Vec<String>, fee: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let size = id.len()
            + inputs.iter().map(|i| i.len()).sum::<usize>()
            + outputs.iter().map(|o| o.len()).sum::<usize>();

        Self {
            id,
            inputs,
            outputs,
            fee,
            size,
            timestamp,
            dependencies: HashSet::new(),
            dag_level: 0,
            priority_score: 0.0,
        }
    }

    pub fn calculate_priority(&mut self, current_time: u64) {
        let age_factor = (current_time.saturating_sub(self.timestamp)) as f64 / 3600.0; // Hours
        let fee_rate = self.fee as f64 / self.size as f64;
        let dag_penalty = self.dag_level as f64 * 0.1;

        self.priority_score = fee_rate * (1.0 + age_factor * 0.1) - dag_penalty;
    }
}

/// Transaction dependency graph for DAG analysis
#[derive(Debug, Clone)]
pub struct TransactionDependencyGraph {
    dependencies: HashMap<String, HashSet<String>>,
    dependents: HashMap<String, HashSet<String>>,
    levels: HashMap<String, u32>,
}

impl Default for TransactionDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionDependencyGraph {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            levels: HashMap::new(),
        }
    }

    pub fn add_dependency(&mut self, tx_id: &str, dep_id: &str) -> Result<(), DAGMempoolError> {
        // Check for cycles
        if self.would_create_cycle(tx_id, dep_id) {
            return Err(DAGMempoolError::DependencyCycle);
        }

        self.dependencies
            .entry(tx_id.to_string())
            .or_default()
            .insert(dep_id.to_string());

        self.dependents
            .entry(dep_id.to_string())
            .or_default()
            .insert(tx_id.to_string());

        // Recalculate levels
        self.recalculate_levels();
        Ok(())
    }

    fn would_create_cycle(&self, tx_id: &str, dep_id: &str) -> bool {
        let mut visited = HashSet::new();
        self.has_path(dep_id, tx_id, &mut visited)
    }

    fn has_path(&self, from: &str, to: &str, visited: &mut HashSet<String>) -> bool {
        if from == to {
            return true;
        }

        if visited.contains(from) {
            return false;
        }

        visited.insert(from.to_string());

        // Traverse along dependency edges: from -> each of its dependencies
        if let Some(deps) = self.dependencies.get(from) {
            for dep in deps {
                if self.has_path(dep, to, visited) {
                    return true;
                }
            }
        }

        false
    }

    fn recalculate_levels(&mut self) {
        self.levels.clear();
        let mut queue = VecDeque::new();

        // Build a complete set of transaction IDs present in the graph
        let mut all_txs: HashSet<String> = HashSet::new();
        all_txs.extend(self.dependencies.keys().cloned());
        all_txs.extend(self.dependents.keys().cloned());

        // Find transactions with no dependencies (level 0)
        for tx_id in all_txs.iter() {
            let is_root = self
                .dependencies
                .get(tx_id)
                .is_none_or(|deps| deps.is_empty());
            if is_root {
                self.levels.insert(tx_id.clone(), 0);
                queue.push_back(tx_id.clone());
            }
        }

        // Calculate levels using BFS
        while let Some(tx_id) = queue.pop_front() {
            let current_level = self.levels.get(&tx_id).copied().unwrap_or(0);

            if let Some(dependents) = self.dependents.get(&tx_id) {
                for dependent in dependents {
                    let new_level = current_level + 1;
                    let existing_level = self.levels.get(dependent).copied().unwrap_or(0);

                    if new_level > existing_level {
                        self.levels.insert(dependent.clone(), new_level);
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    pub fn get_level(&self, tx_id: &str) -> u32 {
        self.levels.get(tx_id).copied().unwrap_or(0)
    }

    pub fn remove_transaction(&mut self, tx_id: &str) {
        if let Some(deps) = self.dependencies.remove(tx_id) {
            for dep in deps {
                if let Some(dependents) = self.dependents.get_mut(&dep) {
                    dependents.remove(tx_id);
                }
            }
        }

        if let Some(dependents) = self.dependents.remove(tx_id) {
            for dependent in dependents {
                if let Some(deps) = self.dependencies.get_mut(&dependent) {
                    deps.remove(tx_id);
                }
            }
        }

        self.levels.remove(tx_id);
        self.recalculate_levels();
    }
}

/// High-performance DAG-aware mempool
pub struct DAGAwareMempool {
    // Core storage
    transactions: Arc<RwLock<HashMap<String, DAGTransaction>>>,
    dependency_graph: Arc<RwLock<TransactionDependencyGraph>>,

    // UTXO tracking
    utxo_reservations: Arc<RwLock<HashMap<String, String>>>, // UTXO -> Transaction ID

    // Priority queues by DAG level
    level_queues: Arc<RwLock<BTreeMap<u32, VecDeque<String>>>>,

    // Performance tracking
    total_transactions: AtomicUsize,
    total_size: AtomicU64,
    validation_semaphore: Arc<Semaphore>,

    // Configuration
    max_transactions: usize,
    max_size: u64,
    validation_timeout: Duration,
}

impl DAGAwareMempool {
    pub async fn new(max_transactions: usize, max_size: u64) -> Result<Self, DAGMempoolError> {
        Ok(Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            dependency_graph: Arc::new(RwLock::new(TransactionDependencyGraph::new())),
            utxo_reservations: Arc::new(RwLock::new(HashMap::new())),
            level_queues: Arc::new(RwLock::new(BTreeMap::new())),
            total_transactions: AtomicUsize::new(0),
            total_size: AtomicU64::new(0),
            validation_semaphore: Arc::new(Semaphore::new(32)), // 32 concurrent validations
            max_transactions,
            max_size,
            validation_timeout: Duration::from_millis(100),
        })
    }

    /// Add transaction with DAG-aware validation
    pub async fn add_transaction(&self, mut tx: DAGTransaction) -> Result<(), DAGMempoolError> {
        // Check capacity
        if self.total_transactions.load(Ordering::Relaxed) >= self.max_transactions {
            return Err(DAGMempoolError::CapacityExceeded);
        }

        // Enforce total size limit using configured max_size
        let current_size = self.total_size.load(Ordering::Relaxed);
        if current_size.saturating_add(tx.size as u64) > self.max_size {
            return Err(DAGMempoolError::CapacityExceeded);
        }

        // Acquire validation permit
        let _permit = self.validation_semaphore.acquire().await.unwrap();

        // Validate transaction with timeout
        timeout(self.validation_timeout, self.validate_transaction(&mut tx))
            .await
            .map_err(|_| DAGMempoolError::ValidationTimeout)??;

        // Store tx_id before moving tx
        let tx_id = tx.id.clone();

        // Add to mempool
        {
            let mut transactions = self.transactions.write().await;
            let mut graph = self.dependency_graph.write().await;
            let mut reservations = self.utxo_reservations.write().await;
            let mut queues = self.level_queues.write().await;

            // Check for existing transaction
            if transactions.contains_key(&tx.id) {
                return Err(DAGMempoolError::TransactionExists(tx.id));
            }

            // Reserve UTXOs
            for input in &tx.inputs {
                if let Some(existing_tx) = reservations.get(input) {
                    if existing_tx != &tx.id {
                        return Err(DAGMempoolError::UTXOConflict(input.clone()));
                    }
                }
                reservations.insert(input.clone(), tx.id.clone());
            }

            // Add dependencies and calculate DAG level
            for input in &tx.inputs {
                if let Some(dep_tx_id) = self.find_output_transaction(input, &transactions).await {
                    tx.dependencies.insert(dep_tx_id.clone());
                    graph.add_dependency(&tx.id, &dep_tx_id)?;
                }
            }

            tx.dag_level = graph.get_level(&tx.id);
            tx.calculate_priority(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );

            // Add to level queue
            queues
                .entry(tx.dag_level)
                .or_insert_with(VecDeque::new)
                .push_back(tx.id.clone());

            // Store transaction
            self.total_transactions.fetch_add(1, Ordering::Relaxed);
            self.total_size.fetch_add(tx.size as u64, Ordering::Relaxed);
            transactions.insert(tx_id.clone(), tx);
        }

        debug!("Added transaction to DAG mempool: {}", tx_id);
        Ok(())
    }

    /// Validate transaction (simplified)
    async fn validate_transaction(&self, tx: &mut DAGTransaction) -> Result<(), DAGMempoolError> {
        // Basic validation
        if tx.id.is_empty() {
            return Err(DAGMempoolError::InvalidTransaction(
                "Empty transaction ID".to_string(),
            ));
        }

        if tx.inputs.is_empty() {
            return Err(DAGMempoolError::InvalidTransaction("No inputs".to_string()));
        }

        if tx.outputs.is_empty() {
            return Err(DAGMempoolError::InvalidTransaction(
                "No outputs".to_string(),
            ));
        }

        if tx.fee == 0 {
            return Err(DAGMempoolError::InvalidTransaction("Zero fee".to_string()));
        }

        Ok(())
    }

    /// Find transaction that creates a specific output
    async fn find_output_transaction(
        &self,
        output_id: &str,
        transactions: &HashMap<String, DAGTransaction>,
    ) -> Option<String> {
        for (tx_id, tx) in transactions {
            if tx.outputs.contains(&output_id.to_string()) {
                return Some(tx_id.clone());
            }
        }
        None
    }

    /// Get transactions for block creation (DAG-aware selection)
    pub async fn get_transactions_for_block(
        &self,
        max_count: usize,
        max_size: u64,
    ) -> Vec<DAGTransaction> {
        let mut selected = Vec::new();
        let mut total_size = 0u64;
        let mut selected_ids = HashSet::new();

        let transactions = self.transactions.read().await;
        let queues = self.level_queues.read().await;

        // Process transactions level by level (DAG order)
        for (_level, queue) in queues.iter() {
            for tx_id in queue {
                if selected.len() >= max_count || total_size >= max_size {
                    break;
                }

                if let Some(tx) = transactions.get(tx_id) {
                    // Check if all dependencies are already selected
                    let deps_satisfied =
                        tx.dependencies.iter().all(|dep| selected_ids.contains(dep));

                    if deps_satisfied && total_size + tx.size as u64 <= max_size {
                        selected.push(tx.clone());
                        selected_ids.insert(tx_id.clone());
                        total_size += tx.size as u64;
                    }
                }
            }

            if selected.len() >= max_count || total_size >= max_size {
                break;
            }
        }

        // Sort by priority within the DAG constraints
        selected.sort_by(|a, b| {
            b.priority_score
                .partial_cmp(&a.priority_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        info!(
            "Selected {} transactions for block (total size: {} bytes)",
            selected.len(),
            total_size
        );
        selected
    }

    /// Remove transactions (after block confirmation)
    pub async fn remove_transactions(&self, tx_ids: &[String]) -> Result<(), DAGMempoolError> {
        let mut transactions = self.transactions.write().await;
        let mut graph = self.dependency_graph.write().await;
        let mut reservations = self.utxo_reservations.write().await;
        let mut queues = self.level_queues.write().await;

        for tx_id in tx_ids {
            if let Some(tx) = transactions.remove(tx_id) {
                // Remove UTXO reservations
                for input in &tx.inputs {
                    reservations.remove(input);
                }

                // Remove from dependency graph
                graph.remove_transaction(tx_id);

                // Remove from level queues
                for queue in queues.values_mut() {
                    queue.retain(|id| id != tx_id);
                }

                self.total_transactions.fetch_sub(1, Ordering::Relaxed);
                self.total_size.fetch_sub(tx.size as u64, Ordering::Relaxed);
            }
        }

        debug!("Removed {} transactions from DAG mempool", tx_ids.len());
        Ok(())
    }

    /// Get mempool statistics
    pub async fn get_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();

        stats.insert(
            "total_transactions".to_string(),
            self.total_transactions.load(Ordering::Relaxed) as u64,
        );
        stats.insert(
            "total_size".to_string(),
            self.total_size.load(Ordering::Relaxed),
        );

        let queues = self.level_queues.read().await;
        stats.insert("dag_levels".to_string(), queues.len() as u64);

        let max_level = queues.keys().max().copied().unwrap_or(0);
        stats.insert("max_dag_level".to_string(), max_level as u64);

        stats
    }

    /// Calculate transaction size (simplified)
    pub fn calculate_transaction_size(&self, tx: &DAGTransaction) -> usize {
        tx.size
    }

    /// Prune old transactions
    pub async fn prune_old_transactions(
        &self,
        max_age_secs: u64,
    ) -> Result<usize, DAGMempoolError> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cutoff_time = current_time.saturating_sub(max_age_secs);

        let transactions = self.transactions.read().await;
        let old_tx_ids: Vec<String> = transactions
            .iter()
            .filter(|(_, tx)| tx.timestamp < cutoff_time)
            .map(|(id, _)| id.clone())
            .collect();

        drop(transactions);

        let count = old_tx_ids.len();
        if count > 0 {
            self.remove_transactions(&old_tx_ids).await?;
            info!("Pruned {} old transactions from DAG mempool", count);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dag_mempool_creation() {
        let mempool = DAGAwareMempool::new(1000, 1024 * 1024).await.unwrap();
        let stats = mempool.get_stats().await;
        assert_eq!(stats.get("total_transactions"), Some(&0));
    }

    #[tokio::test]
    async fn test_transaction_addition() {
        let mempool = DAGAwareMempool::new(1000, 1024 * 1024).await.unwrap();

        let tx = DAGTransaction::new(
            "test_tx_1".to_string(),
            vec!["input_1".to_string()],
            vec!["output_1".to_string()],
            100,
        );

        let result = mempool.add_transaction(tx).await;
        assert!(result.is_ok());

        let stats = mempool.get_stats().await;
        assert_eq!(stats.get("total_transactions"), Some(&1));
    }

    #[tokio::test]
    async fn test_dependency_graph() {
        let mut graph = TransactionDependencyGraph::new();

        // Add dependency: tx2 depends on tx1
        let result = graph.add_dependency("tx2", "tx1");
        assert!(result.is_ok());

        assert_eq!(graph.get_level("tx1"), 0);
        assert_eq!(graph.get_level("tx2"), 1);

        // Test cycle detection
        let cycle_result = graph.add_dependency("tx1", "tx2");
        assert!(matches!(
            cycle_result,
            Err(DAGMempoolError::DependencyCycle)
        ));
    }
}
