use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::time::{interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::block_producer::BlockProducer;
use crate::config::LoggingConfig;
use crate::mempool::Mempool;
use crate::miner::Miner;
use crate::p2p::P2PCommand;
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
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
    mempool_batch_size: usize,

    // Logging configuration
    logging_config: LoggingConfig,
    p2p_broadcast_tx: Option<mpsc::Sender<P2PCommand>>,

    shutdown_token: CancellationToken,
    // Re-evaluation signaling and cancellation
    generation_tx: watch::Sender<u64>,
    generation_rx: watch::Receiver<u64>,
    generation_token: Arc<RwLock<CancellationToken>>, // swapped per generation
    // Debounce miner cancellation to reduce wasted work
    cancel_debounce_ms: u64,
    last_cancel_instant: Arc<RwLock<Instant>>,
    // Canonical tips broadcast channel (fast tips vector for parent selection)
    canonical_tip_tx: watch::Sender<Vec<String>>,
    canonical_tip_rx: watch::Receiver<Vec<String>>,
    // External block notifications to refresh generation/mining on P2P block additions
    external_block_rx: Option<Arc<tokio::sync::Mutex<mpsc::Receiver<()>>>>,
}

/// A candidate block ready for mining
#[derive(Debug, Clone)]
struct MiningCandidate {
    block: QantoBlock,
    #[allow(dead_code)]
    created_at: Instant,
    sequence_id: u64,
    // Snapshot of the generation cancellation token at creation time
    gen_token: CancellationToken,
}

/// A successfully mined block ready for processing
#[derive(Debug, Clone)]
struct MinedBlock {
    block: QantoBlock,
    #[allow(dead_code)]
    mining_duration: Duration,
    sequence_id: u64,
}

impl DecoupledProducer {
    #[allow(clippy::too_many_arguments)]
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
        mempool_batch_size: usize,
        logging_config: LoggingConfig,
        p2p_broadcast_tx: Option<mpsc::Sender<P2PCommand>>,
        shutdown_token: CancellationToken,
        external_block_rx: Option<mpsc::Receiver<()>>,
    ) -> Self {
        let (generation_tx, generation_rx) = watch::channel(0u64);
        let generation_token = Arc::new(RwLock::new(CancellationToken::new()));
        let (canonical_tip_tx, canonical_tip_rx) = watch::channel::<Vec<String>>(Vec::new());
        let external_block_rx = external_block_rx.map(|rx| Arc::new(tokio::sync::Mutex::new(rx)));
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
            mempool_batch_size,
            logging_config,
            p2p_broadcast_tx,
            shutdown_token,
            generation_tx,
            generation_rx,
            generation_token,
            // Debounce cancellation at ~2x block interval, minimum 50ms
            cancel_debounce_ms: block_creation_interval_ms.saturating_mul(2).max(50),
            last_cancel_instant: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(1))),
            canonical_tip_tx,
            canonical_tip_rx,
            external_block_rx,
        }
    }

    /// Main orchestration method that starts all pipeline stages
    pub async fn run(&self) -> Result<(), QantoDAGError> {
        info!(
            "DECOUPLED PRODUCER: Starting high-throughput pipeline - {}ms intervals, {} mining workers",
            self.block_creation_interval_ms, self.mining_workers
        );

        // Seed canonical tips channel with current fast tips for chain 0
        let latest_tips = self.dag.get_fast_tips(0).await.unwrap_or_default();
        let _ = self.canonical_tip_tx.send(latest_tips);

        // Start external block listener if present
        if let Some(mutex_rx) = &self.external_block_rx {
            let mutex_rx = Arc::clone(mutex_rx);
            let generation_tx = self.generation_tx.clone();
            let generation_token_holder = Arc::clone(&self.generation_token);
            let last_cancel_instant_holder = Arc::clone(&self.last_cancel_instant);
            let dag = Arc::clone(&self.dag);
            let canonical_tip_tx = self.canonical_tip_tx.clone();
            let shutdown_token = self.shutdown_token.clone();

            tokio::spawn(async move {
                let mut rx = mutex_rx.lock().await;
                loop {
                    tokio::select! {
                        _ = shutdown_token.cancelled() => {
                            break;
                        }
                        msg = rx.recv() => {
                            if msg.is_none() {
                                break;
                            }
                            debug!("DECOUPLED PRODUCER: Received external block notification. Resetting generation and cancelling stale miners.");

                            // 1. Cancel active mining workers
                            let now = Instant::now();
                            let mut last_cancel = last_cancel_instant_holder.write().await;

                            let mut gt = generation_token_holder.write().await;
                            gt.cancel();
                            *gt = CancellationToken::new();
                            *last_cancel = now;

                            // 2. Bump generation to trigger block creator refresh
                            let current = *generation_tx.borrow();
                            let _ = generation_tx.send(current.wrapping_add(1));

                            // 3. Update canonical tips
                            let new_tips = dag.get_fast_tips(0).await.unwrap_or_default();
                            let _ = canonical_tip_tx.send(new_tips);
                        }
                    }
                }
            });
        }

        // Create communication channels between pipeline stages
        let (candidate_tx, candidate_rx) =
            mpsc::channel::<MiningCandidate>(self.candidate_buffer_size);
        let (mined_tx, mined_rx) = mpsc::channel::<MinedBlock>(self.mined_buffer_size);

        // Start pipeline stages concurrently
        let block_creator_handle = self.start_block_creator(candidate_tx).await;
        let mining_pool_handle = self.start_mining_pool(candidate_rx, mined_tx).await;
        let block_processor_handle = self.start_block_processor(mined_rx).await;

        // Wait for shutdown or any stage to fail
        tokio::select! {
            _ = self.shutdown_token.cancelled() => {
                info!("DECOUPLED PRODUCER: Shutdown signal received");
            },
            result = block_creator_handle => {
                error!("DECOUPLED PRODUCER: Block creator failed: {:?}", result);
            },
            result = mining_pool_handle => {
                error!("DECOUPLED PRODUCER: Mining pool failed: {:?}", result);
            },
            result = block_processor_handle => {
                error!("DECOUPLED PRODUCER: Block processor failed: {:?}", result);
            },
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
        let mut generation_rx = self.generation_rx.clone();
        let _tip_rx = self.canonical_tip_rx.clone();
        let generation_token_holder = Arc::clone(&self.generation_token);

        tokio::spawn(async move {
            let mut creation_interval = interval(Duration::from_millis(interval_ms));
            creation_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let mut sequence_id = 0u64;
            let mut blocks_created = 0u64;
            // Enforce a minimum interval between any candidate creations (including generation-triggered)
            let min_gap = Duration::from_millis(interval_ms);
            let mut last_creation_at = Instant::now();

            info!(
                "BLOCK CREATOR: Starting continuous block creation every {}ms",
                interval_ms
            );

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        info!("BLOCK CREATOR: Shutdown signal received");
                        break;
                    },
                    // Immediate refresh when a new generation is signaled
                    res = generation_rx.changed() => {
                        if res.is_ok() {
                            // Rate-limit generation-triggered candidate creation to respect interval
                            let now = Instant::now();
                            if now.duration_since(last_creation_at) < min_gap {
                                debug!("BLOCK CREATOR: Generation changed, skipping candidate (rate-limited)" );
                                continue;
                            }
                            debug!("BLOCK CREATOR: Generation changed, refreshing candidate");
                            // Validate wallet keypair
                            let (signing_key, public_key) = match wallet.get_keypair() {
                                Ok(keypair) => keypair,
                                Err(e) => {
                                    warn!("BLOCK CREATOR: Failed to get wallet keypair: {}", e);
                                    continue;
                                }
                            };

                            // Query DAG tips dynamically
                            let parents_override = None;
                            // Use deterministic chain selection for tests: chain 0
                            let chain_id = 0u32;
                            let candidate_block = match Self::create_candidate_block(
                                &dag,
                                &mempool,
                                &utxos,
                                &miner,
                                wallet.address().as_str(),
                                signing_key.as_bytes(),
                                public_key.as_bytes(),
                                chain_id,
                                parents_override,
                                None, // difficulty_override
                            ).await {
                                Ok(block) => block,
                                Err(e) => {
                                    warn!("BLOCK CREATOR: Failed to create candidate: {}", e);
                                    continue;
                                }
                            };

                            sequence_id += 1;
                            // Attach generation token snapshot to candidate
                            let gen_token = {
                                let gt = generation_token_holder.read().await;
                                gt.clone()
                            };
                            let candidate = MiningCandidate {
                                block: candidate_block,
                                created_at: Instant::now(),
                                sequence_id,
                                gen_token,
                            };

                            if let Err(e) = candidate_tx.try_send(candidate) {
                                match e {
                                    mpsc::error::TrySendError::Full(_) => {
                                        debug!("BLOCK CREATOR: Mining queue full, skipping candidate #{}", sequence_id);
                                    }
                                    mpsc::error::TrySendError::Closed(_) => {
                                        error!("BLOCK CREATOR: Mining channel closed");
                                        break;
                                    }
                                }
                            } else {
                                blocks_created += 1;
                                debug!("BLOCK CREATOR: Created candidate #{} (total: {})", sequence_id, blocks_created);
                                last_creation_at = now;
                            }
                        } else {
                            warn!("BLOCK CREATOR: Generation watch channel closed");
                            break;
                        }
                    },
                    _ = creation_interval.tick() => {
                        // Inspect mempool for logging/telemetry, but always create a block
                        let tx_count = {
                            let mempool_guard = mempool.read().await;
                            let txs = mempool_guard.len().await;
                            debug!("BLOCK CREATOR: Mempool status - {} transactions", txs);
                            txs
                        };
                        if tx_count == 0 {
                            debug!("BLOCK CREATOR: No user transactions detected — creating heartbeat candidate (coinbase-only)");
                        } else {
                            debug!("BLOCK CREATOR: User transactions present — creating regular candidate");
                        }

                        // Validate wallet keypair
                        let (signing_key, public_key) = match wallet.get_keypair() {
                            Ok(keypair) => {
                                debug!("BLOCK CREATOR: Wallet keypair validated successfully");
                                keypair
                            },
                            Err(e) => {
                                warn!("BLOCK CREATOR: Failed to get wallet keypair: {}", e);
                                continue;
                            }
                        };

                        // Create candidate block
                        debug!("BLOCK CREATOR: Creating candidate block...");
                        // Query DAG tips dynamically
                        let parents_override = None;
                        // Use deterministic chain selection for tests: chain 0
                        let chain_id = 0u32;
                        let candidate_block = match Self::create_candidate_block(
                            &dag,
                            &mempool,
                            &utxos,
                            &miner,
                            wallet.address().as_str(),
                            signing_key.as_bytes(),
                            public_key.as_bytes(),
                            chain_id,
                            parents_override,
                            None, // difficulty_override
                        ).await {
                            Ok(block) => {
                                let is_heartbeat = block.transactions.len() == 1; // coinbase-only
                                if is_heartbeat {
                                    debug!("BLOCK CREATOR: Created HEARTBEAT candidate (coinbase-only)");
                                } else {
                                    debug!("BLOCK CREATOR: Created candidate with {} transactions (incl. coinbase)", block.transactions.len());
                                }
                                block
                            },
                            Err(e) => {
                                warn!("BLOCK CREATOR: Failed to create candidate: {}", e);
                                continue;
                            }
                        };

                        sequence_id += 1;
                        // Attach generation token snapshot to candidate
                        let gen_token = {
                            let gt = generation_token_holder.read().await;
                            gt.clone()
                        };
                        let candidate = MiningCandidate {
                            block: candidate_block,
                            created_at: Instant::now(),
                            sequence_id,
                            gen_token,
                        };

                        // Send to mining pool (non-blocking)
                        match candidate_tx.try_send(candidate) {
                            Ok(()) => {
                                blocks_created += 1;
                                debug!("BLOCK CREATOR: Created candidate #{} (total: {})", sequence_id, blocks_created);
                                last_creation_at = Instant::now();
                            }
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                debug!("BLOCK CREATOR: Mining queue full, skipping candidate #{}", sequence_id);
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                error!("BLOCK CREATOR: Mining channel closed");
                                break;
                            }
                        }
                    }
                }
            }

            info!(
                "BLOCK CREATOR: Stopped after creating {} candidates",
                blocks_created
            );
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
            info!(
                "MINING POOL: Starting {} parallel mining workers",
                worker_count
            );

            // Create work distribution channel
            let (work_tx, work_rx) = mpsc::channel::<MiningCandidate>(worker_count * 4);
            let work_rx = Arc::new(tokio::sync::Mutex::new(work_rx));

            // Start mining workers that actually do parallel work
            let mut worker_handles = Vec::new();

            for worker_id in 0..worker_count {
                let miner_clone = Arc::clone(&miner);
                let mined_tx_clone = mined_tx.clone();
                let work_rx_clone = Arc::clone(&work_rx);
                let shutdown_token_clone = shutdown_token.clone();

                let handle = tokio::spawn(async move {
                    let mut blocks_mined = 0u64;

                    loop {
                        tokio::select! {
                            _ = shutdown_token_clone.cancelled() => {
                                debug!("MINING WORKER {}: Shutdown signal received", worker_id);
                                break;
                            },
                            candidate = async {
                                let mut rx = work_rx_clone.lock().await;
                                rx.recv().await
                            } => {
                                match candidate {
                                    Some(mut candidate) => {
                                        debug!("MINING WORKER {}: Processing candidate #{}", worker_id, candidate.sequence_id);

                                        // Execute mining in parallel
                                        let mining_start = Instant::now();
                                        // Use candidate's generation token snapshot to prevent stale work races
                                        match Self::mine_block(&miner_clone, &mut candidate.block, &candidate.gen_token).await {
                                            Ok(mined_block) => {
                                                let mining_duration = mining_start.elapsed();
                                                blocks_mined += 1;
                                                info!(
                                                    "BLOCK_MINED worker_id={} block_id={} height={} tx_count={} duration_ms={}",
                                                    worker_id,
                                                    mined_block.id,
                                                    mined_block.height,
                                                    mined_block.transactions.len(),
                                                    mining_duration.as_millis()
                                                );

                                                let mined = MinedBlock {
                                                    block: mined_block,
                                                    mining_duration,
                                                    sequence_id: candidate.sequence_id,
                                                };

                                                if let Err(e) = mined_tx_clone.send(mined).await {
                                                    error!("MINING WORKER {}: Failed to send mined block: {}", worker_id, e);
                                                    break;
                                                }

                                                debug!("MINING WORKER {}: Mined block #{} in {:?}", worker_id, candidate.sequence_id, mining_duration);
                                            }
                                            Err(e) => {
                                                let emsg = e.to_string();
                                                if emsg.to_lowercase().contains("cancelled")
                                                    || emsg.to_lowercase().contains("timeout")
                                                    || emsg.to_lowercase().contains("timed out")
                                                {
                                                    debug!(
                                                        "MINING WORKER {}: Candidate #{} cancelled/timeout: {}",
                                                        worker_id, candidate.sequence_id, e
                                                    );
                                                } else {
                                                    warn!(
                                                        "MINING WORKER {}: Failed to mine candidate #{}: {}",
                                                        worker_id, candidate.sequence_id, e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        debug!("MINING WORKER {}: Work channel closed", worker_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    info!(
                        "MINING WORKER {}: Stopped after mining {} blocks",
                        worker_id, blocks_mined
                    );
                });

                worker_handles.push(handle);
            }

            // Distribute incoming candidates to workers
            let mut candidates_received = 0u64;

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        info!("MINING POOL: Shutdown signal received");
                        break;
                    },
                    candidate = candidate_rx.recv() => {
                        match candidate {
                            Some(candidate) => {
                                candidates_received += 1;
                                debug!("MINING POOL: Distributing candidate #{} to workers", candidate.sequence_id);

                                // Send to worker pool (non-blocking)
                                if let Err(e) = work_tx.send(candidate).await {
                                    error!("MINING POOL: Failed to distribute work: {}", e);
                                    break;
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

            // Close work channel to signal workers to stop
            drop(work_tx);

            // Wait for all workers to complete
            for handle in worker_handles {
                let _ = handle.await;
            }

            info!(
                "MINING POOL: Stopped after distributing {} candidates",
                candidates_received
            );
            Ok(())
        })
    }

    /// Stage 3: Mined block processing
    async fn start_block_processor(
        &self,
        mut mined_rx: mpsc::Receiver<MinedBlock>,
    ) -> tokio::task::JoinHandle<Result<(), QantoDAGError>> {
        let dag = Arc::clone(&self.dag);
        let mempool = Arc::clone(&self.mempool);
        let utxos = Arc::clone(&self.utxos);
        let shutdown_token = self.shutdown_token.clone();
        let generation_tx = self.generation_tx.clone();
        let generation_token_holder = Arc::clone(&self.generation_token);
        let last_cancel_instant_holder = Arc::clone(&self.last_cancel_instant);
        let cancel_debounce_ms = self.cancel_debounce_ms;
        let logging_config = self.logging_config.clone();
        let tip_tx = self.canonical_tip_tx.clone();
        let p2p_broadcast_tx = self.p2p_broadcast_tx.clone();
        let mempool_batch_size = self.mempool_batch_size;

        let pending_ttl_ms = self.block_creation_interval_ms.saturating_mul(3).max(500);
        // Use fixed retry interval to reduce busy-looping and stabilize processing
        let retry_ms = 75u64;
        tokio::spawn(async move {
            info!("BLOCK PROCESSOR: Starting async block processing stage");

            let mut blocks_processed = 0u64;
            // Pending children waiting for a missing parent to be committed
            #[derive(Clone)]
            struct PendingChild {
                block: QantoBlock,
                sequence_id: u64,
                queued_at: Instant,
            }
            let mut pending_children: HashMap<String, Vec<PendingChild>> = HashMap::new();
            // Hold pending children for up to ~3 block intervals, minimum 500ms
            let pending_ttl = Duration::from_millis(pending_ttl_ms);
            // Retry moderately often; avoid busy loop — aligned with block interval
            let mut retry_interval = interval(Duration::from_millis(retry_ms));
            retry_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            // Small helper to attempt block add, returning if it was accepted
            #[allow(clippy::too_many_arguments)]
            async fn attempt_add_block(
                dag: &Arc<QantoDAG>,
                mempool: &Arc<RwLock<Mempool>>,
                utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
                block_to_add: QantoBlock,
                p2p_broadcast_tx: &Option<mpsc::Sender<P2PCommand>>,
                generation_tx: &watch::Sender<u64>,
                generation_token_holder: &Arc<RwLock<CancellationToken>>,
                last_cancel_instant_holder: &Arc<RwLock<Instant>>,
                _cancel_debounce_ms: u64,
                canonical_tip_tx: &watch::Sender<Vec<String>>,
            ) -> Result<bool, QantoDAGError> {
                info!(
                    "BLOCK PROCESSOR: Attempting to add block {} with nonce {}",
                    block_to_add.id, block_to_add.nonce
                );
                match dag
                    .add_block(
                        block_to_add.clone(),
                        utxos,
                        Some(mempool),
                        block_to_add.reservation_miner_id.as_deref(),
                    )
                    .await
                {
                    Ok(true) => {
                        // Block accepted — log a clean acceptance line, then preformatted block on its own lines
                        info!("✅ QANTOBLOCK ACCEPTED");
                        let block_str = format!("{}", block_to_add);
                        info!("\n{}", block_str);
                        if let Some(tx) = p2p_broadcast_tx {
                            if let Err(err) = tx
                                .send(P2PCommand::BroadcastBlock(block_to_add.clone()))
                                .await
                            {
                                warn!(
                                    "BLOCK PROCESSOR: Failed to broadcast accepted block {}: {}",
                                    block_to_add.id, err
                                );
                            }
                        }

                        // Remove transactions from mempool after successful block addition
                        {
                            let mempool_guard = mempool.read().await;
                            mempool_guard
                                .remove_transactions(&block_to_add.transactions)
                                .await;
                            info!(
                                "BLOCK PROCESSOR: Removed {} transactions from mempool",
                                block_to_add.transactions.len()
                            );

                            // Sweep mempool for any remaining transactions that reference
                            // UTXOs consumed by this block. This prevents stale transactions
                            // (e.g. re-gossiped from peers) from being re-mined.
                            let mut spent_utxo_ids = HashSet::new();
                            for tx in &block_to_add.transactions {
                                for input in &tx.inputs {
                                    spent_utxo_ids
                                        .insert(format!("{}_{}", input.tx_id, input.output_index));
                                }
                            }
                            let evicted = mempool_guard
                                .evict_transactions_spending_utxos(&spent_utxo_ids)
                                .await;
                            if evicted > 0 {
                                info!(
                                    "BLOCK PROCESSOR: Evicted {} conflicting transactions (spent-UTXO sweep)",
                                    evicted
                                );
                            }
                        }

                        // Release reserved transactions for this miner after success
                        if let Some(miner_id) = block_to_add.reservation_miner_id.as_deref() {
                            let mempool_guard = mempool.read().await;
                            mempool_guard.release_reserved_transactions(miner_id);
                            info!(
                                "BLOCK PROCESSOR: Released reserved transactions for miner {} after success",
                                miner_id
                            );
                        }

                        // Pipeline re-evaluation: bump generation, debounce miner cancellation
                        let current = *generation_tx.borrow();
                        let _ = generation_tx.send(current.wrapping_add(1));
                        let now = Instant::now();
                        let mut last_cancel = last_cancel_instant_holder.write().await;
                        // Force immediate cancellation for miners working on stale tips
                        let mut gt = generation_token_holder.write().await;
                        gt.cancel();
                        *gt = CancellationToken::new();
                        *last_cancel = now;
                        debug!("BLOCK PROCESSOR: Forced miner cancellation after acceptance");

                        // Broadcast updated canonical tips (fast tips for the block's chain)
                        let new_tips = dag
                            .get_fast_tips(block_to_add.chain_id)
                            .await
                            .unwrap_or_default();
                        let _ = canonical_tip_tx.send(new_tips.clone());
                        if !new_tips.is_empty() {
                            let preview = new_tips
                                .iter()
                                .take(2)
                                .map(|t| {
                                    let short = &t[..std::cmp::min(12, t.len())];
                                    short.to_string()
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            info!("📢 Canonical tips updated: [{}]", preview);
                        } else {
                            info!("📢 Canonical tips updated: [none]");
                        }

                        Ok(true)
                    }
                    Ok(false) => {
                        warn!(
                            "BLOCK PROCESSOR: Block {} already exists in DAG",
                            block_to_add.id
                        );
                        // Even if it exists, release reservations to avoid starvation
                        if let Some(miner_id) = block_to_add.reservation_miner_id.as_deref() {
                            let mempool_guard = mempool.read().await;
                            mempool_guard.release_reserved_transactions(miner_id);
                            info!(
                                "BLOCK PROCESSOR: Released reserved transactions for miner {} after rejection",
                                miner_id
                            );
                        }
                        Ok(false)
                    }
                    Err(e) => {
                        // Propagate error upwards for caller to decide how to handle
                        Err(e)
                    }
                }
            }

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        info!("BLOCK PROCESSOR: Shutdown signal received");
                        break;
                    },
                    _ = retry_interval.tick() => {
                        // Periodically retry any pending children whose parents may have been committed
                        // Collect expired children to release reservations outside of the retain() closure
                        let mut expired_children: Vec<(String, PendingChild)> = Vec::new();
                        let now = Instant::now();
                        // Expire old entries and collect ready ones
                        pending_children.retain(|parent_id, children| {
                            let mut keep_parent_key = false;
                            let mut remaining: Vec<PendingChild> = Vec::with_capacity(children.len());
                            for child in children.iter() {
                                if now.duration_since(child.queued_at) > pending_ttl {
                                    // TTL expired: release reservations and drop
                                    warn!(
                                        "BLOCK PROCESSOR: Pending child {} expired waiting for parent {}. Releasing reservations.",
                                        child.block.id, parent_id
                                    );
                                    expired_children.push((parent_id.clone(), child.clone()));
                                    // do not keep
                                } else {
                                    // Keep for future unless we will reattempt below
                                    remaining.push(child.clone());
                                    keep_parent_key = true;
                                }
                            }
                            // Overwrite with remaining entries
                            *children = remaining;
                            keep_parent_key
                        });

                        // Release reservations for expired children outside of the retain() closure
                        for (_parent_id, expired_child) in expired_children {
                            if let Some(miner_id) = expired_child.block.reservation_miner_id.as_deref() {
                                let mempool_guard = mempool.read().await;
                                mempool_guard.release_reserved_transactions(miner_id);
                            }
                        }

                        // If any parent's block now exists, try re-adding its children
                        // We iterate separately to avoid mutable borrowing issues
                        let keys: Vec<String> = pending_children.keys().cloned().collect();
                        for parent_id in keys {
                            // Check if parent exists now in DAG via a quick path: get_fast_tips or blocks map
                            // We use DAG API: parent existence is implied by add_block success; try children directly
                            if let Some(children) = pending_children.remove(&parent_id) {
                                for child in children {
                                    match attempt_add_block(
                                        &dag,
                                        &mempool,
                                        &utxos,
                                        child.block.clone(),
                                        &p2p_broadcast_tx,
                                        &generation_tx,
                                        &generation_token_holder,
                                        &last_cancel_instant_holder,
                                        cancel_debounce_ms,
                                        &tip_tx,
                                    )
                                    .await
                                    {
                                    Ok(true) => {
                                        blocks_processed += 1;
                                        dag.run_periodic_maintenance(mempool_batch_size).await;
                                        debug!(
                                            "BLOCK PROCESSOR: Pending child {} accepted after parent {} committed",
                                                child.block.id, parent_id
                                            );
                                        }
                                        Ok(false) => {
                                            // Already exists or rejected; reservations already handled in helper
                                        }
                                        Err(err) => {
                                            // If still invalid parent, requeue until TTL
                                            match err {
                                                QantoDAGError::InvalidParent(_) => {
                                                    let entry = pending_children
                                                        .entry(parent_id.clone())
                                                        .or_default();
                                                    entry.push(PendingChild {
                                                        block: child.block.clone(),
                                                        sequence_id: child.sequence_id,
                                                        queued_at: child.queued_at,
                                                    });
                                                    debug!(
                                                        "BLOCK PROCESSOR: Still waiting for parent {} for child {}",
                                                        parent_id, child.block.id
                                                    );
                                                }
                                                other => {
                                                    error!(
                                                        "BLOCK PROCESSOR: Failed to add pending child {}: {}",
                                                        child.block.id, other
                                                    );
                                                    // Release reservations on hard failure
                                                    if let Some(miner_id) = child.block.reservation_miner_id.as_deref() {
                                                        let mempool_guard = mempool.read().await;
                                                        mempool_guard.release_reserved_transactions(miner_id);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    mined = mined_rx.recv() => {
                        match mined {
                            Some(mined_block) => {
                                debug!("BLOCK PROCESSOR: Validating and adding mined block #{}", mined_block.sequence_id);

                                // Block celebration display - gated by config flag
                                if logging_config.enable_block_celebrations {
                                    let block_str = format!("{}", mined_block.block);
                                    info!("\n{}", block_str);
                                }

                                // Prepare block: attach optional finality_proof derived from canonical PoW hash
                                let mut block_to_add = mined_block.block.clone();
                                let finality_hex = format!("{}", block_to_add.hash_for_pow());
                                block_to_add.finality_proof = Some(finality_hex);

                                // Validate and add block to DAG with parent-aware pending queue
                                match attempt_add_block(
                                    &dag,
                                    &mempool,
                                    &utxos,
                                    block_to_add.clone(),
                                    &p2p_broadcast_tx,
                                    &generation_tx,
                                    &generation_token_holder,
                                    &last_cancel_instant_holder,
                                    cancel_debounce_ms,
                                    &tip_tx,
                                ).await {
                                    Ok(true) => {
                                        blocks_processed += 1;
                                        dag.run_periodic_maintenance(mempool_batch_size).await;
                                    }
                                    Ok(false) => {
                                        // already exists or rejected; handled inside helper
                                    }
                                    Err(err) => {
                                        match err {
                                            QantoDAGError::InvalidParent(details) => {
                                                // Queue the child under ALL missing parents until they are committed
                                                // Determine which parents are missing right now
                                                let mut queued_any = false;
                                                for parent_id in &block_to_add.parents {
                                                    if dag.get_block(parent_id).await.is_none() {
                                                        let entry = pending_children
                                                            .entry(parent_id.to_string())
                                                            .or_default();
                                                        entry.push(PendingChild {
                                                            block: block_to_add.clone(),
                                                            sequence_id: mined_block.sequence_id,
                                                            queued_at: Instant::now(),
                                                        });
                                                        queued_any = true;
                                                    }
                                                }
                                                if queued_any {
                                                    warn!(
                                                        "BLOCK PROCESSOR: Parent(s) missing for block {} ({}). Queued pending until parents commit.",
                                                        block_to_add.id, details
                                                    );
                                                } else {
                                                    // Fallback: if we couldn't identify missing parents (race), re-attempt later on a generic key
                                                    let entry = pending_children
                                                        .entry("<unknown>".to_string())
                                                        .or_default();
                                                    entry.push(PendingChild {
                                                        block: block_to_add.clone(),
                                                        sequence_id: mined_block.sequence_id,
                                                        queued_at: Instant::now(),
                                                    });
                                                    warn!(
                                                        "BLOCK PROCESSOR: InvalidParent reported but all parents appear present; queued under <unknown> for retry."
                                                    );
                                                }
                                                // Do NOT release reservations yet; expect quick acceptance
                                            }
                                            other => {
                                                let err_str = other.to_string();
                                                error!(
                                                    "BLOCK PROCESSOR: Failed to add block {}: {}",
                                                    block_to_add.id, err_str
                                                );

                                                // If failure is due to spent UTXOs, evict the
                                                // offending transactions from the mempool instead
                                                // of releasing them back for re-mining.
                                                let is_utxo_failure = err_str.contains("UTXO")
                                                    || err_str.contains("transactions failed validation");

                                                if is_utxo_failure {
                                                    let mempool_guard = mempool.read().await;
                                                    // Evict the block's transactions so they cannot
                                                    // be re-included in the next block.
                                                    mempool_guard
                                                        .remove_transactions(&block_to_add.transactions)
                                                        .await;

                                                    // Also sweep for any OTHER transactions in the
                                                    // mempool that reference the same spent UTXOs
                                                    // (e.g. re-gossiped from peers under different tx IDs).
                                                    let mut spent_utxo_ids = HashSet::new();
                                                    for tx in &block_to_add.transactions {
                                                        for input in &tx.inputs {
                                                            spent_utxo_ids.insert(format!("{}_{}", input.tx_id, input.output_index));
                                                        }
                                                    }
                                                    let sweep_evicted = mempool_guard
                                                        .evict_transactions_spending_utxos(&spent_utxo_ids)
                                                        .await;
                                                    warn!(
                                                        "BLOCK PROCESSOR: Evicted {} block txs + {} swept txs from mempool after UTXO validation failure",
                                                        block_to_add.transactions.len(),
                                                        sweep_evicted
                                                    );
                                                    // Also release reservations cleanly
                                                    if let Some(miner_id) = block_to_add.reservation_miner_id.as_deref() {
                                                        mempool_guard.release_reserved_transactions(miner_id);
                                                    }
                                                } else if let Some(miner_id) = block_to_add.reservation_miner_id.as_deref() {
                                                    let mempool_guard = mempool.read().await;
                                                    mempool_guard.release_reserved_transactions(miner_id);
                                                    warn!(
                                                        "BLOCK PROCESSOR: Released reserved transactions for miner {} after failure",
                                                        miner_id
                                                    );
                                                }
                                            }
                                        }
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

            info!(
                "BLOCK PROCESSOR: Stopped after processing {} blocks",
                blocks_processed
            );
            Ok(())
        })
    }

    /// Create a candidate block from mempool transactions
    #[allow(clippy::too_many_arguments)]
    async fn create_candidate_block(
        dag: &Arc<QantoDAG>,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        miner: &Arc<Miner>,
        validator_address: &str,
        signing_key: &[u8],
        public_key: &[u8],
        chain_id: u32,
        parents: Option<Vec<String>>, // Optional explicit parents override
        difficulty_override: Option<u64>,
    ) -> Result<QantoBlock, QantoDAGError> {
        use crate::post_quantum_crypto::{QantoPQPrivateKey, QantoPQPublicKey};

        // Get transactions from mempool
        let _transactions = {
            let mempool_guard = mempool.read().await;
            let _utxos_guard = utxos.read().await;
            let transactions = mempool_guard.get_transactions().await;
            debug!(
                "CREATE_CANDIDATE: Retrieved {} transactions from mempool",
                transactions.len()
            );
            transactions
        };

        // Convert byte slices to QantoPQ keys
        let qr_signing_key = QantoPQPrivateKey::from_bytes(signing_key)
            .map_err(|_| QantoDAGError::Generic("Invalid signing key format".to_string()))?;
        let qr_public_key = QantoPQPublicKey::from_bytes(public_key)
            .map_err(|_| QantoDAGError::Generic("Invalid public key format".to_string()))?;

        debug!(
            "CREATE_CANDIDATE: Calling DAG create_candidate_block on chain {}",
            chain_id
        );
        // Create block candidate
        let candidate_block = dag
            .create_candidate_block(
                &qr_signing_key,
                &qr_public_key,
                validator_address,
                mempool,
                utxos,
                chain_id,
                miner,
                None, // homomorphic_public_key
                parents,
                difficulty_override,
            )
            .await?;

        debug!(
            "CREATE_CANDIDATE: DAG returned candidate block with {} transactions",
            candidate_block.transactions.len()
        );
        Ok(candidate_block)
    }

    /// Mine a block using the miner
    async fn mine_block(
        miner: &Arc<Miner>,
        candidate_block: &mut QantoBlock,
        generation_token: &CancellationToken,
    ) -> Result<QantoBlock, QantoDAGError> {
        let mining_start = Instant::now();

        // Execute mining with cancellation support
        let result = tokio::task::spawn_blocking({
            let miner = miner.clone();
            let mut block = candidate_block.clone();
            let token = generation_token.clone();
            move || {
                match miner.solve_pow_with_cancellation(&mut block, token) {
                    Ok(()) => Ok(block), // Return the mined block with valid nonce
                    Err(e) => Err(e),
                }
            }
        })
        .await;

        match result {
            Ok(Ok(mined_block)) => {
                let mining_duration = mining_start.elapsed();
                debug!(
                    "Mining completed in {:?} with nonce: {}",
                    mining_duration, mined_block.nonce
                );
                Ok(mined_block)
            }
            Ok(Err(e)) => Err(QantoDAGError::Generic(format!("Mining failed: {e}"))),
            Err(e) => Err(QantoDAGError::Generic(format!("Mining task failed: {e}"))),
        }
    }
}

#[async_trait]
impl BlockProducer for DecoupledProducer {
    async fn run(&self) -> Result<(), QantoDAGError> {
        DecoupledProducer::run(self).await
    }
}
