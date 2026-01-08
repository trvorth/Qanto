use std::collections::HashMap;
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
use crate::node_keystore::Wallet;
use crate::p2p::P2PCommand;
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::types::UTXO;

/// High-throughput block producer that decouples block creation from mining
/// Targets 32+ blocks per second by pipelining operations
pub struct DecoupledProducer {
    dag: Arc<QantoDAG>,
    wallet: Arc<Wallet>,
    mempool: Arc<RwLock<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    miner: Arc<Miner>,
    p2p_command_sender: mpsc::Sender<P2PCommand>,

    // Performance tuning parameters
    block_creation_interval_ms: u64,
    mining_workers: usize,
    candidate_buffer_size: usize,
    mined_buffer_size: usize,

    // Logging configuration
    logging_config: LoggingConfig,

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

    // Recent accepted block IDs to gate duplicates (LRU + TTL)
    recent_accepted: Arc<RwLock<RecentAccepted>>,
}

/// A candidate block ready for mining
#[derive(Debug, Clone)]
struct MiningCandidate {
    block: QantoBlock,
    #[allow(dead_code)]
    created_at: Instant,
    sequence_id: u64,
    // Generation identifier at the time the candidate was created
    generation_id: u64,
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
    generation_id: u64,
}

/// Tracks recently accepted block IDs to gate duplicates across async boundaries
#[derive(Debug)]
struct RecentAccepted {
    entries: HashMap<String, Instant>,
    max_entries: usize,
    ttl: Duration,
}

impl RecentAccepted {
    fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            ttl,
        }
    }

    fn prune_expired(&mut self, now: Instant) {
        self.entries
            .retain(|_, t| now.duration_since(*t) <= self.ttl);
    }

    fn insert(&mut self, id: String, now: Instant) {
        self.entries.insert(id, now);
        if self.entries.len() > self.max_entries {
            // Drop the oldest entry to enforce max size
            let mut oldest_key: Option<String> = None;
            let mut oldest_time: Option<Instant> = None;
            for (k, t) in self.entries.iter() {
                if oldest_time.is_none_or(|old| *t < old) {
                    oldest_time = Some(*t);
                    oldest_key = Some(k.clone());
                }
            }
            if let Some(k) = oldest_key {
                self.entries.remove(&k);
            }
        }
    }

    fn contains_id(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }
}

impl DecoupledProducer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dag: Arc<QantoDAG>,
        wallet: Arc<Wallet>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        miner: Arc<Miner>,
        p2p_command_sender: mpsc::Sender<P2PCommand>,
        mut block_creation_interval_ms: u64,
        mining_workers: usize,
        mut candidate_buffer_size: usize,
        mut mined_buffer_size: usize,
        logging_config: LoggingConfig,
        shutdown_token: CancellationToken,
    ) -> Self {
        // Tuning for 32 BPS (approx 31.25ms per block)
        // If the configured interval is too slow for 32 BPS, we clamp it.
        if block_creation_interval_ms > 30 {
            warn!(
                "DecoupledProducer: Requested interval {}ms is too slow for 32 BPS target. Clamping to 30ms.",
                block_creation_interval_ms
            );
            block_creation_interval_ms = 30;
        }

        // Ensure buffers are large enough to prevent stalling at 32 BPS
        if candidate_buffer_size < 128 {
            candidate_buffer_size = 128;
        }
        if mined_buffer_size < 128 {
            mined_buffer_size = 128;
        }

        let (generation_tx, generation_rx) = watch::channel(0u64);
        let generation_token = Arc::new(RwLock::new(CancellationToken::new()));
        let (canonical_tip_tx, canonical_tip_rx) = watch::channel::<Vec<String>>(Vec::new());
        // TTL ~ 10x block interval (min 5s) with capacity 8192
        let recent_ttl =
            Duration::from_millis(block_creation_interval_ms.saturating_mul(10).max(5000));
        let recent_accepted = Arc::new(RwLock::new(RecentAccepted::new(8192, recent_ttl)));
        Self {
            dag,
            wallet,
            mempool,
            utxos,
            miner,
            p2p_command_sender,
            block_creation_interval_ms,
            mining_workers,
            candidate_buffer_size,
            mined_buffer_size,
            logging_config,
            shutdown_token,
            generation_tx,
            generation_rx,
            generation_token,
            // Debounce cancellation at ~2x block interval, minimum 50ms
            cancel_debounce_ms: block_creation_interval_ms.saturating_mul(2).max(50),
            last_cancel_instant: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(1))),
            canonical_tip_tx,
            canonical_tip_rx,
            recent_accepted,
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
        let tip_rx = self.canonical_tip_rx.clone();
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

                            // Clone tips to owned value before await to avoid holding watch::Ref across await
                            let current_tips = tip_rx.borrow().clone();
                            let parents_override = if current_tips.is_empty() { None } else { Some(current_tips) };
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
                                generation_id: *generation_rx.borrow(),
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
                        // Clone tips to owned value before await to avoid holding watch::Ref across await
                        let current_tips = tip_rx.borrow().clone();
                        let parents_override = if current_tips.is_empty() { None } else { Some(current_tips) };
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
                            generation_id: *generation_rx.borrow(),
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
        let generation_tx_clone = self.generation_tx.clone();

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
                let generation_tx_worker = generation_tx_clone.clone();

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
                                        // Drop stale work immediately if generation changed or token already cancelled
                                        let current_gen = *generation_tx_worker.borrow();
                                        if candidate.gen_token.is_cancelled() || candidate.generation_id != current_gen {
                                            debug!(
                                                "MINING WORKER {}: Dropping stale candidate #{} (candidate gen {} vs current {})",
                                                worker_id, candidate.sequence_id, candidate.generation_id, current_gen
                                            );
                                            continue;
                                        }
                                        debug!("MINING WORKER {}: Processing candidate #{}", worker_id, candidate.sequence_id);

                                        // Execute mining in parallel
                                        let mining_start = Instant::now();
                                        // Use candidate's generation token snapshot to prevent stale work races
                                        match Self::mine_block(&miner_clone, &mut candidate.block, &candidate.gen_token).await {
                                            Ok(mined_block) => {
                                                println!("DEBUG: Worker {} mined block", worker_id);
                                                let mining_duration = mining_start.elapsed();
                                                blocks_mined += 1;

                                                let mined = MinedBlock {
                                                    block: mined_block,
                                                    mining_duration,
                                                    sequence_id: candidate.sequence_id,
                                                    generation_id: candidate.generation_id,
                                                };

                                                if let Err(e) = mined_tx_clone.send(mined).await {
                                                    error!("MINING WORKER {}: Failed to send mined block: {}", worker_id, e);
                                                    break;
                                                }

                                                debug!("MINING WORKER {}: Mined block #{} in {:?}", worker_id, candidate.sequence_id, mining_duration);
                                            }
                                            Err(e) => {
                                                println!("DEBUG: Worker {} failed to mine: {}", worker_id, e);
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
            let generation_tx_distributor = generation_tx_clone.clone();

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
                                // Skip distribution if candidate is already stale
                                let current_gen = *generation_tx_distributor.borrow();
                                if candidate.gen_token.is_cancelled() || candidate.generation_id != current_gen {
                                    debug!(
                                        "MINING POOL: Dropping stale candidate #{} (candidate gen {} vs current {})",
                                        candidate.sequence_id, candidate.generation_id, current_gen
                                    );
                                    continue;
                                }
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
        let p2p_tx = self.p2p_command_sender.clone();
        let recent_accepted = Arc::clone(&self.recent_accepted);

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
            use std::collections::HashMap as StdHashMap;
            let mut pending_children: StdHashMap<String, Vec<PendingChild>> = StdHashMap::new();
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
                generation_tx: &watch::Sender<u64>,
                generation_token_holder: &Arc<RwLock<CancellationToken>>,
                last_cancel_instant_holder: &Arc<RwLock<Instant>>,
                _cancel_debounce_ms: u64,
                canonical_tip_tx: &watch::Sender<Vec<String>>,
                p2p_command_sender: &mpsc::Sender<P2PCommand>,
                logging_config: &LoggingConfig,
                recent_accepted: &Arc<RwLock<RecentAccepted>>,
            ) -> Result<bool, QantoDAGError> {
                debug!(
                    "BLOCK PROCESSOR: Attempting to add block {} with nonce {}",
                    block_to_add.id, block_to_add.nonce
                );
                match dag
                    .add_block(
                        block_to_add.clone(),
                        utxos,
                        Some(mempool),
                        block_to_add.reservation_snapshot_id.as_deref(),
                    )
                    .await
                {
                    Ok(true) => {
                        // Block accepted — summary at info, heavy display gated by config
                        info!("✅ QANTOBLOCK ACCEPTED: {}", block_to_add.id);
                        if logging_config.enable_block_celebrations {
                            let block_str = format!("{}", block_to_add);
                            info!("\n{}", block_str);
                        }

                        // Remove transactions from mempool after successful block addition
                        {
                            // Avoid outer write-lock during awaits; inner locks handle mutation
                            let mempool_guard = mempool.read().await;
                            mempool_guard
                                .remove_transactions(&block_to_add.transactions)
                                .await;
                            debug!(
                                "BLOCK PROCESSOR: Removed {} transactions from mempool",
                                block_to_add.transactions.len()
                            );
                        }

                        // Release reserved transactions for this snapshot after success
                        if let Some(snapshot_id) = block_to_add.reservation_snapshot_id.as_deref() {
                            let mut mempool_guard = mempool.write().await;
                            mempool_guard.release_reserved_transactions(snapshot_id);
                            debug!(
                                "BLOCK PROCESSOR: Released reserved transactions for snapshot {} after success",
                                snapshot_id
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
                        if logging_config.enable_block_celebrations {
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
                        } else {
                            debug!("Canonical tips updated ({} total)", new_tips.len());
                        }

                        // Broadcast the newly committed block over P2P so peers can sync
                        if let Err(e) = p2p_command_sender
                            .send(P2PCommand::BroadcastBlock(block_to_add.clone()))
                            .await
                        {
                            error!(
                                "BLOCK PROCESSOR: Failed to enqueue P2P BroadcastBlock {}: {}",
                                block_to_add.id, e
                            );
                        } else {
                            debug!(
                                "BLOCK PROCESSOR: Enqueued P2P BroadcastBlock {}",
                                block_to_add.id
                            );
                        }

                        // Record acceptance to gate duplicates across async boundaries
                        {
                            let now = Instant::now();
                            let mut ra = recent_accepted.write().await;
                            ra.prune_expired(now);
                            ra.insert(block_to_add.id.clone(), now);
                        }

                        Ok(true)
                    }
                    Ok(false) => {
                        warn!(
                            "BLOCK PROCESSOR: Block {} already exists in DAG",
                            block_to_add.id
                        );
                        // Even if it exists, release reservations to avoid starvation
                        if let Some(snapshot_id) = block_to_add.reservation_snapshot_id.as_deref() {
                            let mut mempool_guard = mempool.write().await;
                            mempool_guard.release_reserved_transactions(snapshot_id);
                            debug!(
                                "BLOCK PROCESSOR: Released reserved transactions for snapshot {} after rejection",
                                snapshot_id
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
                            if let Some(snapshot_id) = expired_child.block.reservation_snapshot_id.as_deref() {
                                let mut mempool_guard = mempool.write().await;
                                mempool_guard.release_reserved_transactions(snapshot_id);
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
                                        &generation_tx,
                                        &generation_token_holder,
                                    &last_cancel_instant_holder,
                                    cancel_debounce_ms,
                                    &tip_tx,
                                    &p2p_tx,
                                    &logging_config,
                                    &recent_accepted,
                                ).await {
                                    Ok(true) => {
                                        blocks_processed += 1;
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
                                                    if let Some(snapshot_id) = child.block.reservation_snapshot_id.as_deref() {
                                                        let mut mempool_guard = mempool.write().await;
                                                        mempool_guard.release_reserved_transactions(snapshot_id);
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

                                // Drop stale mined blocks from previous generations immediately
                                let current_generation = *generation_tx.borrow();
                                if mined_block.generation_id != current_generation {
                                    // Release any reservations to avoid starvation
                                    if let Some(snapshot_id) = mined_block.block.reservation_snapshot_id.as_deref() {
                                        let mut mempool_guard = mempool.write().await;
                                        mempool_guard.release_reserved_transactions(snapshot_id);
                                    }
                                    debug!(
                                        "BLOCK PROCESSOR: Discarding stale mined block {} from generation {} (current {})",
                                        mined_block.block.id, mined_block.generation_id, current_generation
                                    );
                                    continue;
                                }

                                // Block celebration display - gated by config flag
                                if logging_config.enable_block_celebrations {
                                    let block_str = format!("{}", mined_block.block);
                                    info!("\n{}", block_str);
                                }

                                // Prepare block: attach optional finality_proof derived from canonical PoW hash
                                let mut block_to_add = mined_block.block.clone();
                                let finality_hex = format!("{}", block_to_add.hash_for_pow());
                                block_to_add.finality_proof = Some(finality_hex);

                                // Fast-path duplicate check to avoid spurious warnings and wasted work
                                if dag.get_block(&block_to_add.id).await.is_some() {
                                    if let Some(snapshot_id) = block_to_add.reservation_snapshot_id.as_deref() {
                                        let mut mempool_guard = mempool.write().await;
                                        mempool_guard.release_reserved_transactions(snapshot_id);
                                    }
                                    debug!(
                                        "BLOCK PROCESSOR: Skipping mined block {} — already committed",
                                        block_to_add.id
                                    );
                                    continue;
                                }

                                // Skip if recently accepted (guards races before DAG reflects state)
                                {
                                    let now = Instant::now();
                                    let mut ra = recent_accepted.write().await;
                                    ra.prune_expired(now);
                                    if ra.contains_id(&block_to_add.id) {
                                        if let Some(snapshot_id) = block_to_add.reservation_snapshot_id.as_deref() {
                                            let mut mempool_guard = mempool.write().await;
                                            mempool_guard.release_reserved_transactions(snapshot_id);
                                        }
                                        debug!(
                                            "BLOCK PROCESSOR: Skipping mined block {} — recently accepted",
                                            block_to_add.id
                                        );
                                        continue;
                                    }
                                }

                                // Validate and add block to DAG with parent-aware pending queue
                                println!("DEBUG: Attempting to add block {}", block_to_add.id);
                                match attempt_add_block(
                                    &dag,
                                    &mempool,
                                    &utxos,
                                    block_to_add.clone(),
                                    &generation_tx,
                                    &generation_token_holder,
                                    &last_cancel_instant_holder,
                                    cancel_debounce_ms,
                                    &tip_tx,
                                    &p2p_tx,
                                    &logging_config,
                                    &recent_accepted,
                                ).await {
                                    Ok(true) => {
                                        println!("DEBUG: Block accepted");
                                        let block_str = format!("{}", block_to_add);
                                        info!("\n{}", block_str);
                                        blocks_processed += 1;
                                    }
                                    Ok(false) => {
                                        println!("DEBUG: Block rejected (Ok(false))");
                                        // already exists or rejected; handled inside helper
                                    }
                                    Err(err) => {
                                        println!("DEBUG: Block rejected (Err): {}", err);
                                        match err {
                                            QantoDAGError::InvalidParent(details) => {
                                                // Queue the child under ALL missing parents until they are committed
                                                // Determine which parents are missing right now
                                                let mut queued_any = false;
                                                for parent_id in &block_to_add.parents {
                                                    if dag.get_block(parent_id).await.is_none() {
                                                        let entry = pending_children
                                                            .entry(parent_id.clone())
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
                                                error!(
                                                    "BLOCK PROCESSOR: Failed to add block {}: {}",
                                                    block_to_add.id, other
                                                );
                                                if let Some(snapshot_id) = block_to_add.reservation_snapshot_id.as_deref() {
                                                    let mut mempool_guard = mempool.write().await;
                                                    mempool_guard.release_reserved_transactions(snapshot_id);
                                                    warn!(
                                                        "BLOCK PROCESSOR: Released reserved transactions for snapshot {} after failure",
                                                        snapshot_id
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
            )
            .await?;

        debug!(
            "CREATE_CANDIDATE: DAG returned candidate block with {} transactions",
            candidate_block.transactions.len()
        );
        Ok(candidate_block)
    }

    /// Mine a block using the miner
    #[allow(deprecated)]
    async fn mine_block(
        miner: &Arc<Miner>,
        candidate_block: &mut QantoBlock,
        generation_token: &CancellationToken,
    ) -> Result<QantoBlock, QantoDAGError> {
        let mining_start = Instant::now();

        // Execute mining with cancellation support
        let result = tokio::spawn({
            let miner = miner.clone();
            let mut block = candidate_block.clone();
            let token = generation_token.clone();
            async move {
                match miner.solve_pow_with_cancellation(&mut block, token).await {
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
