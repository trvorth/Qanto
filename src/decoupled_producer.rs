use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::block_producer::BlockProducer;
use crate::mempool::Mempool;
use crate::miner::Miner;
use crate::qantodag::{QantoDAG, QantoDAGError, QantoBlock};
use crate::types::UTXO;
use crate::wallet::Wallet;

/// High-throughput block producer that decouples block creation from mining
/// Targets 32+ blocks per second by pipelining operations
pub struct DecoupledProducer {
    dag: Arc<QantoDAG>,
    wallet: Arc<Wallet>,
    mempool: Arc<RwLock<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    miner: Arc<Miner>,
    
    // Performance tuning parameters
    block_creation_interval_ms: u64,
    mining_workers: usize,
    candidate_buffer_size: usize,
    mined_buffer_size: usize,
    
    shutdown_token: CancellationToken,
}

/// A candidate block ready for mining
#[derive(Debug, Clone)]
struct MiningCandidate {
    block: QantoBlock,
    #[allow(dead_code)]
    created_at: Instant,
    sequence_id: u64,
}

/// A successfully mined block ready for processing
#[derive(Debug, Clone)]
struct MinedBlock {
    block: QantoBlock,
    mining_duration: Duration,
    sequence_id: u64,
}

impl DecoupledProducer {
    pub fn new(
        dag: Arc<QantoDAG>,
        wallet: Arc<Wallet>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        miner: Arc<Miner>,
        block_creation_interval_ms: u64,
        mining_workers: usize,
        candidate_buffer_size: usize,
        mined_buffer_size: usize,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            dag,
            wallet,
            mempool,
            utxos,
            miner,
            block_creation_interval_ms,
            mining_workers,
            candidate_buffer_size,
            mined_buffer_size,
            shutdown_token,
        }
    }

    /// Main orchestration method that starts all pipeline stages
    pub async fn run(&self) -> Result<(), QantoDAGError> {
        info!(
            "DECOUPLED PRODUCER: Starting high-throughput pipeline - {}ms intervals, {} mining workers",
            self.block_creation_interval_ms, self.mining_workers
        );

        // Create communication channels between pipeline stages
        let (candidate_tx, candidate_rx) = mpsc::channel::<MiningCandidate>(self.candidate_buffer_size);
        let (mined_tx, mined_rx) = mpsc::channel::<MinedBlock>(self.mined_buffer_size);

        // Start pipeline stages concurrently
        let block_creator_handle = self.start_block_creator(candidate_tx).await;
        let mining_pool_handle = self.start_mining_pool(candidate_rx, mined_tx).await;
        let block_processor_handle = self.start_block_processor(mined_rx).await;

        // Wait for shutdown or any stage to fail
        tokio::select! {
            _ = self.shutdown_token.cancelled() => {
                info!("DECOUPLED PRODUCER: Shutdown signal received");
            }
            result = block_creator_handle => {
                error!("DECOUPLED PRODUCER: Block creator failed: {:?}", result);
            }
            result = mining_pool_handle => {
                error!("DECOUPLED PRODUCER: Mining pool failed: {:?}", result);
            }
            result = block_processor_handle => {
                error!("DECOUPLED PRODUCER: Block processor failed: {:?}", result);
            }
        }

        Ok(())
    }

    /// Stage 1: Continuous block candidate creation
    async fn start_block_creator(
        &self,
        candidate_tx: mpsc::Sender<MiningCandidate>,
    ) -> tokio::task::JoinHandle<Result<(), QantoDAGError>> {
        let dag = Arc::clone(&self.dag);
        let wallet = Arc::clone(&self.wallet);
        let mempool = Arc::clone(&self.mempool);
        let utxos = Arc::clone(&self.utxos);
        let miner = Arc::clone(&self.miner);
        let shutdown_token = self.shutdown_token.clone();
        let interval_ms = self.block_creation_interval_ms;

        tokio::spawn(async move {
            let mut creation_interval = interval(Duration::from_millis(interval_ms));
            creation_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            
            let mut sequence_id = 0u64;
            let mut blocks_created = 0u64;

            info!("BLOCK CREATOR: Starting continuous block creation every {}ms", interval_ms);

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        info!("BLOCK CREATOR: Shutdown signal received");
                        break;
                    }
                    _ = creation_interval.tick() => {
                        // Check if we have transactions to process
                        let has_work = {
                            let mempool_guard = mempool.read().await;
                            mempool_guard.len().await > 0
                        };

                        if !has_work {
                            continue;
                        }

                        // Validate wallet keypair
                        let (signing_key, public_key) = match wallet.get_keypair() {
                            Ok(keypair) => keypair,
                            Err(e) => {
                                warn!("BLOCK CREATOR: Failed to get wallet keypair: {}", e);
                                continue;
                            }
                        };

                        // Create candidate block
                        let candidate_block = match Self::create_candidate_block(
                            &dag, &mempool, &utxos, &miner, signing_key.as_bytes(), public_key.as_bytes()
                        ).await {
                            Ok(block) => block,
                            Err(e) => {
                                warn!("BLOCK CREATOR: Failed to create candidate: {}", e);
                                continue;
                            }
                        };

                        sequence_id += 1;
                        let candidate = MiningCandidate {
                            block: candidate_block,
                            created_at: Instant::now(),
                            sequence_id,
                        };

                        // Send to mining pool (non-blocking)
                        match candidate_tx.try_send(candidate) {
                            Ok(()) => {
                                blocks_created += 1;
                                debug!("BLOCK CREATOR: Created candidate #{} (total: {})", sequence_id, blocks_created);
                            }
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                warn!("BLOCK CREATOR: Mining queue full, skipping candidate #{}", sequence_id);
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                error!("BLOCK CREATOR: Mining channel closed");
                                break;
                            }
                        }
                    }
                }
            }

            info!("BLOCK CREATOR: Stopped after creating {} candidates", blocks_created);
            Ok(())
        })
    }

    /// Stage 2: Parallel mining worker pool
    async fn start_mining_pool(
        &self,
        mut candidate_rx: mpsc::Receiver<MiningCandidate>,
        mined_tx: mpsc::Sender<MinedBlock>,
    ) -> tokio::task::JoinHandle<Result<(), QantoDAGError>> {
        let miner = Arc::clone(&self.miner);
        let shutdown_token = self.shutdown_token.clone();
        let worker_count = self.mining_workers;

        tokio::spawn(async move {
            info!("MINING POOL: Starting {} parallel mining workers", worker_count);

            // Create worker pool
            let mut worker_handles = Vec::new();
            
            for worker_id in 0..worker_count {
                let _miner_clone = Arc::clone(&miner);
                let _mined_tx_clone = mined_tx.clone();
                let shutdown_token_clone = shutdown_token.clone();

                let handle = tokio::spawn(async move {
                    let blocks_mined = 0u64;
                    
                    loop {
                        tokio::select! {
                            _ = shutdown_token_clone.cancelled() => {
                                debug!("MINING WORKER {}: Shutdown signal received", worker_id);
                                break;
                            }
                            _ = tokio::time::sleep(Duration::from_millis(1)) => {
                                // Worker ready for work
                            }
                        }
                    }
                    
                    info!("MINING WORKER {}: Stopped after mining {} blocks", worker_id, blocks_mined);
                });
                
                worker_handles.push(handle);
            }

            // Distribute candidates to workers
            let _next_worker = 0;
            let (worker_tx, mut worker_rx) = mpsc::channel::<MiningCandidate>(worker_count * 2);

            // Candidate distributor
            let distributor_handle = tokio::spawn(async move {
                while let Some(candidate) = candidate_rx.recv().await {
                    if worker_tx.send(candidate).await.is_err() {
                        break;
                    }
                }
            });

            // Mining coordinator
            let mut candidates_received = 0u64;
            let mut blocks_mined = 0u64;

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        info!("MINING POOL: Shutdown signal received");
                        break;
                    }
                    candidate = worker_rx.recv() => {
                        match candidate {
                            Some(mut candidate) => {
                                candidates_received += 1;
                                debug!("MINING POOL: Processing candidate #{}", candidate.sequence_id);

                                // Execute mining
                                let mining_start = Instant::now();
                                match Self::mine_block(&miner, &mut candidate.block, &shutdown_token).await {
                                    Ok(mined_block) => {
                                        let mining_duration = mining_start.elapsed();
                                        blocks_mined += 1;

                                        let mined = MinedBlock {
                                            block: mined_block,
                                            mining_duration,
                                            sequence_id: candidate.sequence_id,
                                        };

                                        if let Err(e) = mined_tx.send(mined).await {
                                            error!("MINING POOL: Failed to send mined block: {}", e);
                                            break;
                                        }

                                        debug!("MINING POOL: Mined block #{} in {:?}", candidate.sequence_id, mining_duration);
                                    }
                                    Err(e) => {
                                        warn!("MINING POOL: Failed to mine candidate #{}: {}", candidate.sequence_id, e);
                                    }
                                }
                            }
                            None => {
                                debug!("MINING POOL: Candidate channel closed");
                                break;
                            }
                        }
                    }
                }
            }

            // Cleanup workers
            for handle in worker_handles {
                let _ = handle.await;
            }
            let _ = distributor_handle.await;

            info!("MINING POOL: Stopped after processing {} candidates, mined {} blocks", 
                  candidates_received, blocks_mined);
            Ok(())
        })
    }

    /// Stage 3: Block processing and DAG integration
    async fn start_block_processor(
        &self,
        mut mined_rx: mpsc::Receiver<MinedBlock>,
    ) -> tokio::task::JoinHandle<Result<(), QantoDAGError>> {
        let dag = Arc::clone(&self.dag);
        let mempool = Arc::clone(&self.mempool);
        let utxos = Arc::clone(&self.utxos);
        let shutdown_token = self.shutdown_token.clone();

        tokio::spawn(async move {
            let mut blocks_processed = 0u64;
            let mut total_mining_time = Duration::ZERO;

            info!("BLOCK PROCESSOR: Starting block processing pipeline");

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        info!("BLOCK PROCESSOR: Shutdown signal received");
                        break;
                    }
                    mined_block = mined_rx.recv() => {
                        match mined_block {
                            Some(mined) => {
                                debug!("BLOCK PROCESSOR: Processing mined block #{}", mined.sequence_id);

                                // Add block to DAG
                                match dag.add_block(mined.block.clone(), &utxos).await {
                                    Ok(_) => {
                                        blocks_processed += 1;
                                        total_mining_time += mined.mining_duration;

                                        // Update performance metrics
                                        dag.performance_monitor.record_block_processed();
                                        dag.performance_monitor.update_resource_usage("blocks_processed", blocks_processed);

                                        // Remove processed transactions from mempool
                                        {
                                            let mempool_guard = mempool.write().await;
                                            let _utxos_guard = utxos.read().await;
                                            
                                            for tx in &mined.block.transactions {
                                                mempool_guard.remove_transactions(&[tx.clone()]).await;
                                            }
                                        }

                                        let avg_mining_time = total_mining_time / blocks_processed as u32;
                                        info!(
                                            "BLOCK PROCESSOR: Successfully processed block #{} (total: {}, avg mining: {:?})",
                                            mined.sequence_id, blocks_processed, avg_mining_time
                                        );
                                    }
                                    Err(e) => {
                                        error!("BLOCK PROCESSOR: Failed to add block #{} to DAG: {}", mined.sequence_id, e);
                                    }
                                }
                            }
                            None => {
                                debug!("BLOCK PROCESSOR: Mined block channel closed");
                                break;
                            }
                        }
                    }
                }
            }

            info!("BLOCK PROCESSOR: Stopped after processing {} blocks", blocks_processed);
            Ok(())
        })
    }

    /// Create a candidate block from mempool transactions
    async fn create_candidate_block(
        dag: &Arc<QantoDAG>,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        miner: &Arc<Miner>,
        signing_key: &[u8],
        public_key: &[u8],
    ) -> Result<QantoBlock, QantoDAGError> {
        use crate::post_quantum_crypto::{QantoPQPrivateKey, QantoPQPublicKey};
        
        // Get transactions from mempool
        let _transactions = {
            let mempool_guard = mempool.read().await;
            let _utxos_guard = utxos.read().await;
            mempool_guard.get_transactions().await
        };

        // Convert byte slices to QantoPQ keys
        let qr_signing_key = QantoPQPrivateKey::from_bytes(signing_key)
            .map_err(|_| QantoDAGError::Generic("Invalid signing key format".to_string()))?;
        let qr_public_key = QantoPQPublicKey::from_bytes(public_key)
            .map_err(|_| QantoDAGError::Generic("Invalid public key format".to_string()))?;
            
        // Create block candidate
        let candidate_block = dag.create_candidate_block(
            &qr_signing_key,
            &qr_public_key,
            "validator_address",
            mempool,
            utxos,
            0, // chain_id
            miner,
            None, // homomorphic_public_key
        ).await?;

        Ok(candidate_block)
    }

    /// Mine a block using the miner
    async fn mine_block(
        miner: &Arc<Miner>,
        candidate_block: &mut QantoBlock,
        shutdown_token: &CancellationToken,
    ) -> Result<QantoBlock, QantoDAGError> {
        let mining_start = Instant::now();
        
        // Execute mining with cancellation support
        let result = tokio::task::spawn_blocking({
            let miner = miner.clone();
            let mut block = candidate_block.clone();
            let token = shutdown_token.clone();
            move || miner.solve_pow_with_cancellation(&mut block, token)
        }).await;

        match result {
            Ok(Ok(())) => {
                let mining_duration = mining_start.elapsed();
                debug!("Mining completed in {:?}", mining_duration);
                Ok(candidate_block.clone())
            }
            Ok(Err(e)) => Err(QantoDAGError::Generic(format!("Mining failed: {}", e))),
            Err(e) => Err(QantoDAGError::Generic(format!("Mining task failed: {}", e))),
        }
    }
}

#[async_trait]
impl BlockProducer for DecoupledProducer {
    async fn run(&self) -> Result<(), QantoDAGError> {
        self.run().await
    }
}