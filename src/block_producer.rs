use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::mempool::Mempool;
use crate::miner::Miner;

use crate::qantodag::{QantoDAG, QantoDAGError};
use crate::types::UTXO;
use crate::wallet::Wallet;

// Re-export the decoupled producer
pub use crate::decoupled_producer::DecoupledProducer;

use crate::config::LoggingConfig;
use qanto_core::mining_celebration::{
    on_block_mined, LoggingConfig as CoreLoggingConfig, MiningCelebrationParams,
};
use tracing::{debug, error, info, warn};

#[async_trait]
pub trait BlockProducer {
    async fn run(&self) -> Result<(), QantoDAGError>;
}

pub struct SoloProducer {
    dag: Arc<QantoDAG>,
    wallet: Arc<Wallet>,
    mempool: Arc<RwLock<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    miner: Arc<Miner>,
    mining_interval_secs: u64,
    shutdown_token: CancellationToken,
}

impl SoloProducer {
    pub fn new(
        dag: Arc<QantoDAG>,
        wallet: Arc<Wallet>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        miner: Arc<Miner>,
        mining_interval_secs: u64,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            dag,
            wallet,
            mempool,
            utxos,
            miner,
            mining_interval_secs,
            shutdown_token,
        }
    }
}

#[async_trait]
impl BlockProducer for SoloProducer {
    async fn run(&self) -> Result<(), QantoDAGError> {
        info!("SOLO MINER: Starting mining loop");

        let mut mining_state = MiningState::new();
        let timing_coordinator = Arc::clone(&self.dag.timing_coordinator);
        timing_coordinator.start().await;

        // Main mining loop
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown_token.cancelled() => {
                    info!("SOLO MINER: Received shutdown signal, stopping mining loop");
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(self.mining_interval_secs)) => {
                    self.log_heartbeat_if_needed(&mut mining_state, self.mining_interval_secs);

                    if !self.validate_wallet_keypair(&self.wallet).await {
                        warn!("SOLO MINER: Wallet keypair validation failed, skipping mining cycle");
                        continue;
                    }

                    let mut candidate_block = match self.create_mining_candidate(
                        &self.wallet,
                        &self.mempool,
                        &self.utxos,
                        &self.miner,
                    ).await {
                        Ok(block) => block,
                        Err(e) => {
                            warn!("SOLO MINER: Failed to create mining candidate: {:?}", e);
                            continue;
                        }
                    };

                    match self.execute_parallel_mining(
                        &self.miner,
                        &mut candidate_block,
                        &self.shutdown_token,
                    ).await {
                        Ok((mined_block, mining_time)) => {
                            mining_state.blocks_mined += 1;
                            if let Err(e) = self.process_mined_block(
                                mined_block,
                                &self.mempool,
                                &self.utxos,
                                mining_state.blocks_mined,
                                mining_time,
                            ).await {
                                error!("SOLO MINER: Failed to process mined block: {:?}", e);
                            }
                        }
                        Err(shutdown_requested) => {
                            if shutdown_requested {
                                info!("SOLO MINER: Mining interrupted by shutdown signal");
                                break;
                            } else {
                                debug!("SOLO MINER: Mining cycle completed without finding valid block");
                            }
                        }
                    }

                    mining_state.cycles += 1;
                }
            }
        }

        info!("SOLO MINER: Mining loop stopped");

        Ok(())
    }
}

// Helper state for mining progress
#[derive(Debug)]
struct MiningState {
    cycles: u64,
    blocks_mined: u64,
    last_heartbeat: std::time::Instant,
}

impl MiningState {
    fn new() -> Self {
        Self {
            cycles: 0,
            blocks_mined: 0,
            last_heartbeat: std::time::Instant::now(),
        }
    }
}

impl SoloProducer {
    fn log_heartbeat_if_needed(&self, mining_state: &mut MiningState, mining_interval_secs: u64) {
        const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
        if mining_state.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            info!(
                "SOLO MINER HEARTBEAT: Alive - Cycles: {}, Blocks Mined: {}, Next attempt in {} seconds",
                mining_state.cycles, mining_state.blocks_mined, mining_interval_secs
            );
            mining_state.last_heartbeat = std::time::Instant::now();
        }
    }

    async fn validate_wallet_keypair(&self, wallet: &Arc<Wallet>) -> bool {
        match wallet.get_keypair() {
            Ok(_) => true,
            Err(e) => {
                warn!(
                    "SOLO MINER: Could not get keypair, skipping cycle. Error: {}",
                    e
                );
                false
            }
        }
    }

    async fn create_mining_candidate(
        &self,
        wallet: &Arc<Wallet>,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        miner: &Arc<Miner>,
    ) -> Result<crate::qantodag::QantoBlock, QantoDAGError> {
        let miner_address = wallet.address();
        let chain_id_to_mine: u32 = 0;

        info!(
            "SOLO MINER: Creating candidate block for chain {} (rewards to validator: {})",
            chain_id_to_mine, miner_address
        );
        let candidate_creation_start = std::time::Instant::now();

        let (qr_signing_key, qr_public_key) = wallet.get_keypair().map_err(|e| {
            QantoDAGError::WalletError(format!("Failed to get keypair from wallet: {e}"))
        })?;

        let homomorphic_public_key = miner.get_homomorphic_public_key();
        match self
            .dag
            .create_candidate_block(
                &qr_signing_key,
                &qr_public_key,
                &miner_address,
                mempool,
                utxos,
                chain_id_to_mine,
                miner,
                homomorphic_public_key,
            )
            .await
        {
            Ok(block) => {
                info!(
                    "SOLO MINER: Successfully created candidate block {} with {} transactions in {:?}",
                    block.id,
                    block.transactions.len(),
                    candidate_creation_start.elapsed()
                );
                Ok(block)
            }
            Err(e) => {
                warn!(
                    "SOLO MINER: Failed to create candidate block after {:?}: {}. Retrying after delay.",
                    candidate_creation_start.elapsed(),
                    e
                );
                Err(QantoDAGError::Generic(format!(
                    "Failed to create candidate block: {e}"
                )))
            }
        }
    }

    async fn execute_parallel_mining(
        &self,
        miner: &Arc<Miner>,
        candidate_block: &mut crate::qantodag::QantoBlock,
        shutdown_token: &CancellationToken,
    ) -> Result<(crate::qantodag::QantoBlock, std::time::Duration), bool> {
        info!(
            "SOLO MINER: Starting parallel proof-of-work for candidate block {} with difficulty {}",
            candidate_block.id, candidate_block.difficulty
        );

        const PARALLEL_MINERS: usize = 4;
        const NONCE_RANGE_SIZE: u64 = u64::MAX / PARALLEL_MINERS as u64;
        const MINING_TIMEOUT_MS: u64 = 2000;

        let pow_start = std::time::Instant::now();
        let (result_tx, mut result_rx) =
            tokio::sync::mpsc::channel::<crate::qantodag::QantoBlock>(PARALLEL_MINERS);
        let mut mining_tasks = Vec::with_capacity(PARALLEL_MINERS);
        let solution_found = Arc::new(std::sync::atomic::AtomicBool::new(false));

        for miner_id in 0..PARALLEL_MINERS {
            let nonce_start = miner_id as u64 * NONCE_RANGE_SIZE;
            let nonce_end = if miner_id == PARALLEL_MINERS - 1 {
                u64::MAX
            } else {
                ((miner_id + 1) as u64 * NONCE_RANGE_SIZE).saturating_sub(1)
            };

            let miner_clone = miner.clone();
            let shutdown_clone = shutdown_token.clone();
            let mut candidate_clone = candidate_block.clone();
            let result_tx_clone = result_tx.clone();
            let solution_found_clone = solution_found.clone();

            debug!(
                "SOLO MINER: Launching parallel miner {} (nonce range: {} - {})",
                miner_id, nonce_start, nonce_end
            );

            let mining_task = tokio::task::spawn(async move {
                candidate_clone.nonce = nonce_start;
                let mut current_nonce = nonce_start;
                while current_nonce <= nonce_end && !shutdown_clone.is_cancelled() {
                    candidate_clone.nonce = current_nonce;
                    match miner_clone
                        .solve_pow_with_cancellation(&mut candidate_clone, shutdown_clone.clone())
                    {
                        Ok(_) => {
                            if !solution_found_clone.swap(true, std::sync::atomic::Ordering::SeqCst)
                            {
                                info!(
                                    "SOLO MINER: Parallel miner {} found valid nonce: {}",
                                    miner_id, candidate_clone.nonce
                                );
                            }
                            let _ = result_tx_clone.send(candidate_clone).await;
                            return;
                        }
                        Err(_) => {
                            current_nonce = current_nonce.saturating_add(1000);
                        }
                    }
                }
                debug!(
                    "SOLO MINER: Parallel miner {} completed range without finding solution",
                    miner_id
                );
            });

            mining_tasks.push(mining_task);
        }

        drop(result_tx);

        let mining_result = tokio::time::timeout(
            std::time::Duration::from_millis(MINING_TIMEOUT_MS),
            result_rx.recv(),
        )
        .await;
        for task in mining_tasks {
            task.abort();
        }

        match mining_result {
            Ok(Some(mined_block)) => {
                let mining_duration = pow_start.elapsed();
                if !solution_found.load(std::sync::atomic::Ordering::SeqCst) {
                    info!(
                        "SOLO MINER: Parallel mining succeeded in {:?} (nonce: {})",
                        mining_duration, mined_block.nonce
                    );
                }
                *candidate_block = mined_block;
                Ok((candidate_block.clone(), mining_duration))
            }
            Ok(None) => {
                warn!("SOLO MINER: All parallel miners completed without finding solution");
                Err(false)
            }
            Err(_) => {
                warn!(
                    "SOLO MINER: Parallel mining timeout after {}ms - difficulty may be too high",
                    MINING_TIMEOUT_MS
                );
                Err(false)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn process_mined_block(
        &self,
        mined_block: crate::qantodag::QantoBlock,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        total_blocks_mined: u64,
        mining_time: std::time::Duration,
    ) -> Result<(), QantoDAGError> {
        let block_height = mined_block.height;
        let block_id = mined_block.id.clone();
        let tx_count = mined_block.transactions.len();

        let block_hash_hex = mined_block.hash();
        let _total_fees: u64 = mined_block.transactions.iter().map(|tx| tx.fee).sum();

        let current_block_count = self.dag.get_block_count().await;
        info!(
            "📊 Current DAG block count before adding: {}",
            current_block_count
        );

        tokio::task::yield_now().await;

        const MAX_ADD_RETRIES: u32 = 2;
        let mut add_retry_count = 0;

        loop {
            info!(
                "🔄 Attempting to add block {} to DAG (attempt {}/{})",
                block_id,
                add_retry_count + 1,
                MAX_ADD_RETRIES
            );

            match self.dag.add_block(mined_block.clone(), utxos).await {
                Ok(true) => {
                    let new_block_count = self.dag.get_block_count().await;
                    info!(
                        "✅ Block {} successfully added to DAG! Block count: {} -> {}",
                        block_id, current_block_count, new_block_count
                    );

                    on_block_mined(
                        MiningCelebrationParams {
                            block_height,
                            block_hash: mined_block.hash(),
                            nonce: mined_block.nonce,
                            difficulty: mined_block.difficulty,
                            transactions_count: tx_count,
                            mining_time,
                            effort: mined_block.effort,
                            total_blocks_mined,
                            chain_id: mined_block.chain_id,
                            block_reward: mined_block.reward,
                            compact: false,
                        },
                        &to_core_logging(&self.dag.logging_config),
                    );

                    println!("{mined_block}");

                    debug!(
                        "Block Details - Full Hash: {} | Parent: {} | Merkle Root: {} | Chain ID: {} | Epoch: {} | Effort: {}",
                        block_hash_hex,
                        mined_block.parents.first().unwrap_or(&"genesis".to_string()),
                        mined_block.merkle_root,
                        mined_block.chain_id,
                        mined_block.epoch,
                        mined_block.effort
                    );

                    match tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        mempool
                            .read()
                            .await
                            .remove_transactions(&mined_block.transactions),
                    )
                    .await
                    {
                        Ok(_) => {
                            info!("SOLO MINER: Removed {} transactions from mempool", tx_count);
                        }
                        Err(_) => {
                            warn!("SOLO MINER: Timeout removing transactions from mempool, continuing...");
                        }
                    }
                    break;
                }
                Ok(false) => {
                    let final_block_count = self.dag.get_block_count().await;
                    warn!(
                        "⚠️ Block {} already exists or was rejected (attempt {}/{}). Block count: {} -> {}",
                        block_id, add_retry_count + 1, MAX_ADD_RETRIES, current_block_count, final_block_count
                    );

                    if let Some(existing_block) = self.dag.get_block(&block_id).await {
                        info!(
                            "🔍 Block {} already exists in DAG with height {}",
                            existing_block.id, existing_block.height
                        );
                    } else {
                        error!("❌ Block {} was rejected but doesn't exist in DAG! This indicates a validation failure.", block_id);
                    }
                    break;
                }
                Err(e) => {
                    add_retry_count += 1;
                    if add_retry_count >= MAX_ADD_RETRIES {
                        error!(
                            "SOLO MINER: Failed to add block after {} retries: {}",
                            MAX_ADD_RETRIES, e
                        );
                        return Err(e);
                    }

                    warn!(
                        "SOLO MINER: Failed to add block (attempt {}): {}. Retrying...",
                        add_retry_count, e
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }

        Ok(())
    }
}

fn to_core_logging(cfg: &LoggingConfig) -> CoreLoggingConfig {
    CoreLoggingConfig {
        enable_block_celebrations: cfg.enable_block_celebrations,
        celebration_log_level: cfg.celebration_log_level.clone(),
        celebration_throttle_per_min: cfg.celebration_throttle_per_min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio_util::sync::CancellationToken;

    use crate::config::LoggingConfig;
    use crate::mempool::Mempool;
    use crate::miner::{Miner, MinerConfig};
    use crate::qanto_storage::{QantoStorage, StorageConfig};
    use crate::qantodag::{QantoDAG, QantoDagConfig, MAX_TRANSACTIONS_PER_BLOCK};
    use crate::saga::PalletSaga;
    use crate::transaction::Transaction;
    use crate::types::UTXO;
    use crate::wallet::Wallet;

    fn create_test_dag() -> Arc<QantoDAG> {
        let storage_config = StorageConfig {
            data_dir: std::path::PathBuf::from("/tmp/test_qanto_solo_producer_unit"),
            max_file_size: 1024 * 1024,
            compression_enabled: false,
            encryption_enabled: false,
            wal_enabled: false,
            sync_writes: false,
            cache_size: 1024,
            compaction_threshold: 100.0,
            max_open_files: 10,
        };

        let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

        let dag_config = QantoDagConfig {
            initial_validator: "test_validator".to_string(),
            target_block_time: 1000,
            num_chains: 1,
            dev_fee_rate: 0.10,
        };

        let logging_config = LoggingConfig {
            level: "info".to_string(),
            enable_block_celebrations: false,
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: Some(10),
        };

        let saga_pallet = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));

        QantoDAG::new(dag_config, saga_pallet, storage, logging_config)
            .expect("Failed to create DAG")
    }

    #[tokio::test]
    async fn test_create_mining_candidate_populates_transactions() {
        let dag_arc = create_test_dag();
        let wallet_arc = Arc::new(Wallet::new().expect("Failed to create wallet"));
        let miner_address = wallet_arc.address();

        // Satisfy validator preconditions
        dag_arc.add_validator(miner_address.clone(), 1000).await;

        // Initialize mempool and UTXOs
        let mempool_arc = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));
        let utxos_arc: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

        // Seed mempool with deterministic dummy transactions and track their IDs
        let seed_tx_count = 3usize;
        let mut seeded_ids: Vec<String> = Vec::new();
        for _ in 0..seed_tx_count {
            let tx = Transaction::new_dummy();
            seeded_ids.push(tx.id.clone());
            mempool_arc
                .write()
                .await
                .add_transaction(tx, &HashMap::new(), &dag_arc)
                .await
                .expect("Failed to add transaction to mempool");
        }

        // Create miner
        let miner_config = MinerConfig {
            address: miner_address.clone(),
            dag: dag_arc.clone(),
            target_block_time: 1000,
            use_gpu: false,
            zk_enabled: false,
            threads: 1,
            logging_config: LoggingConfig {
                level: "info".to_string(),
                enable_block_celebrations: false,
                celebration_log_level: "info".to_string(),
                celebration_throttle_per_min: Some(10),
            },
        };
        let miner = Arc::new(Miner::new(miner_config).expect("Failed to create miner"));

        // Instantiate SoloProducer (intervals are not used by candidate creation)
        let solo = SoloProducer::new(
            dag_arc.clone(),
            wallet_arc.clone(),
            mempool_arc.clone(),
            utxos_arc.clone(),
            miner.clone(),
            1,
            CancellationToken::new(),
        );

        // Create candidate block via SoloProducer's private method (accessible within module tests)
        let candidate = solo
            .create_mining_candidate(&wallet_arc, &mempool_arc, &utxos_arc, &miner)
            .await
            .expect("Failed to create mining candidate");

        // Assert candidate block structure and that seeded transactions are included
        assert_eq!(candidate.chain_id, 0);
        assert_eq!(candidate.validator, miner_address);
        assert!(!candidate.parents.is_empty());
        assert!(!candidate.transactions.is_empty());
        assert!(candidate.transactions.len() <= MAX_TRANSACTIONS_PER_BLOCK);

        let candidate_ids: std::collections::HashSet<String> = candidate
            .transactions
            .iter()
            .map(|t| t.id.clone())
            .collect();
        for id in seeded_ids {
            assert!(
                candidate_ids.contains(&id),
                "Candidate missing seeded tx {id}"
            );
        }
        // Candidate should include coinbase in addition to seeded transactions
        assert!(candidate.transactions.len() > seed_tx_count);

        // Clean up storage directory
        let _ = std::fs::remove_dir_all("/tmp/test_qanto_solo_producer_unit");
    }
}
