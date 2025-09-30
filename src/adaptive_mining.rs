use crate::diagnostics::{DiagnosticsEngine, MiningAttemptParams, StrataStateParams};
use crate::mempool::Mempool;
use crate::miner::Miner;
use crate::mining_metrics::{MiningFailureType, MiningMetrics};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::types::UTXO;
use crate::wallet::Wallet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Testnet mining configuration with adaptive features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestnetMiningConfig {
    /// Enable adaptive difficulty adjustment
    pub adaptive_difficulty: bool,
    /// Target block time in milliseconds for testnet
    pub target_block_time_ms: u64,
    /// Maximum time to wait before reducing difficulty (in seconds)
    pub stall_threshold_secs: u64,
    /// Difficulty reduction factor when stalled (0.5 = 50% reduction)
    pub difficulty_reduction_factor: f64,
    /// Minimum difficulty floor for testnet
    pub min_difficulty: f64,
    /// Maximum difficulty ceiling for testnet
    pub max_difficulty: f64,
    /// Enable retry logic for failed mining attempts
    pub enable_retry_logic: bool,
    /// Maximum number of retries per mining attempt
    pub max_retries: u32,
    /// Base delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Skip VDF computation in test mode
    pub skip_vdf_in_test: bool,
}

impl Default for TestnetMiningConfig {
    fn default() -> Self {
        Self {
            adaptive_difficulty: true,
            target_block_time_ms: 5000, // 5 seconds for testnet
            stall_threshold_secs: 10,
            difficulty_reduction_factor: 0.5,
            min_difficulty: 0.001,
            max_difficulty: 1000.0,
            enable_retry_logic: true,
            max_retries: 3,
            retry_delay_ms: 100,
            skip_vdf_in_test: true,
        }
    }
}

/// Adaptive mining state tracking
#[derive(Debug, Clone)]
pub struct AdaptiveMiningState {
    pub current_difficulty: f64,
    pub last_block_time: Instant,
    pub consecutive_failures: u32,
    pub total_attempts: u64,
    pub successful_blocks: u64,
    pub last_difficulty_adjustment: Instant,
    pub mining_start_time: Option<Instant>,
}

impl Default for AdaptiveMiningState {
    fn default() -> Self {
        Self {
            current_difficulty: 1.0,
            last_block_time: Instant::now(),
            consecutive_failures: 0,
            total_attempts: 0,
            successful_blocks: 0,
            last_difficulty_adjustment: Instant::now(),
            mining_start_time: None,
        }
    }
}

/// Enhanced mining loop with adaptive difficulty and retry logic
pub struct AdaptiveMiningLoop {
    pub config: TestnetMiningConfig,
    pub state: AdaptiveMiningState,
    pub diagnostics: Option<Arc<DiagnosticsEngine>>,
    pub metrics: Arc<MiningMetrics>,
}

impl AdaptiveMiningLoop {
    pub fn new(
        config: TestnetMiningConfig,
        diagnostics: Option<Arc<DiagnosticsEngine>>,
        metrics: Arc<MiningMetrics>,
    ) -> Self {
        Self {
            config,
            state: AdaptiveMiningState::default(),
            diagnostics,
            metrics,
        }
    }

    /// Main adaptive mining loop with enhanced error handling and retry logic
    pub async fn run_adaptive_mining_loop(
        &mut self,
        qanto_dag: Arc<QantoDAG>,
        wallet: Arc<Wallet>,
        miner: Arc<Miner>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        shutdown_token: CancellationToken,
    ) -> Result<(), QantoDAGError> {
        info!("🚀 Starting adaptive mining loop for testnet");
        self.state.mining_start_time = Some(Instant::now());

        // Initialize metrics session
        self.metrics.start_session().await;

        loop {
            tokio::select! {
                biased;
                _ = shutdown_token.cancelled() => {
                    info!("🛑 Adaptive mining loop received shutdown signal");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    // Check if we should mine based on mempool state
                    let should_mine = {
                        let mempool_guard = mempool.read().await;
                        mempool_guard.len().await > 0
                    };

                    if !should_mine {
                        tokio::task::yield_now().await;
                        continue;
                    }

                    // Execute mining cycle with adaptive features
                    match self.execute_adaptive_mining_cycle(
                        &qanto_dag,
                        &wallet,
                        &miner,
                        &mempool,
                        &utxos,
                        &shutdown_token,
                    ).await {
                        Ok(block_mined) => {
                            if block_mined {
                                self.handle_successful_mining().await;
                            }
                        }
                        Err(e) => {
                            if e.to_string().contains("cancelled") {
                                info!("Mining cancelled, exiting loop");
                                break;
                            }
                            self.handle_mining_failure(&e).await;
                        }
                    }
                }
            }
        }

        info!("🏁 Adaptive mining loop completed");
        self.metrics.log_statistics().await;
        Ok(())
    }

    /// Execute a single mining cycle with adaptive difficulty and retry logic
    async fn execute_adaptive_mining_cycle(
        &mut self,
        qanto_dag: &Arc<QantoDAG>,
        wallet: &Arc<Wallet>,
        miner: &Arc<Miner>,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        shutdown_token: &CancellationToken,
    ) -> Result<bool, QantoDAGError> {
        self.state.total_attempts += 1;
        let attempt_start = Instant::now();
        let attempt_id = self.state.total_attempts;

        // Check if we need to adjust difficulty
        if self.config.adaptive_difficulty {
            self.adjust_difficulty_if_needed().await;
        }

        info!(
            "⛏️  Mining attempt #{} - Difficulty: {:.6}, Target: {}ms",
            attempt_id, self.state.current_difficulty, self.config.target_block_time_ms
        );

        // Record diagnostics if enabled
        if let Some(diagnostics) = &self.diagnostics {
            diagnostics
                .record_strata_state(StrataStateParams {
                    layer_count: 1,
                    current_layer: 0,
                    vdf_iterations: self.state.current_difficulty as u64,
                    vdf_difficulty: self.state.current_difficulty as u64,
                    proof_verification_time_ms: 0,
                    quantum_state_valid: true,
                    threshold_signatures_count: self.state.consecutive_failures,
                })
                .await
                .ok();
        }

        // Create candidate block with current difficulty
        let mut candidate_block = match self
            .create_candidate_block_with_difficulty(qanto_dag, wallet, mempool, utxos)
            .await
        {
            Ok(block) => block,
            Err(e) => {
                warn!("Failed to create candidate block: {}", e);
                self.record_mining_attempt(attempt_id, attempt_start, false, Some(e.to_string()))
                    .await;
                return Err(e);
            }
        };

        // Execute mining with retry logic
        let mining_result = if self.config.enable_retry_logic {
            self.execute_mining_with_retries(miner, &mut candidate_block, shutdown_token)
                .await
        } else {
            self.execute_single_mining_attempt(miner, &mut candidate_block, shutdown_token)
                .await
        };

        match mining_result {
            Ok((mined_block, mining_duration)) => {
                // Process the successfully mined block
                match self
                    .process_mined_block(qanto_dag, mined_block, mempool, utxos, mining_duration)
                    .await
                {
                    Ok(_) => {
                        self.record_mining_attempt(attempt_id, attempt_start, true, None)
                            .await;
                        Ok(true)
                    }
                    Err(e) => {
                        warn!("Failed to process mined block: {}", e);
                        self.record_mining_attempt(
                            attempt_id,
                            attempt_start,
                            false,
                            Some(e.to_string()),
                        )
                        .await;
                        Err(e)
                    }
                }
            }
            Err(cancelled) => {
                if cancelled {
                    return Err(QantoDAGError::Generic("Mining cancelled".to_string()));
                }
                self.record_mining_attempt(
                    attempt_id,
                    attempt_start,
                    false,
                    Some("Mining failed".to_string()),
                )
                .await;
                Ok(false)
            }
        }
    }

    /// Create candidate block with adaptive difficulty
    async fn create_candidate_block_with_difficulty(
        &self,
        qanto_dag: &Arc<QantoDAG>,
        wallet: &Arc<Wallet>,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<QantoBlock, QantoDAGError> {
        let miner_address = wallet.address();
        let chain_id: u32 = 0;

        // Get transactions from mempool
        let transactions = {
            let mempool_guard = mempool.read().await;
            mempool_guard.get_pending_transactions().await
        };

        info!(
            "Creating candidate block with {} transactions, difficulty: {:.6}",
            transactions.len(),
            self.state.current_difficulty
        );

        // Create block with adaptive difficulty
        let mut candidate_block = qanto_dag
            .create_candidate_block(
                &qanto_dag.get_private_key().await?,
                &qanto_dag.get_private_key().await?.public_key(),
                &miner_address,
                mempool,
                utxos,
                chain_id,
            )
            .await?;

        // Override difficulty with adaptive value
        candidate_block.difficulty = self.state.current_difficulty;

        Ok(candidate_block)
    }

    /// Execute mining with retry logic and exponential backoff
    async fn execute_mining_with_retries(
        &self,
        miner: &Arc<Miner>,
        candidate_block: &mut QantoBlock,
        shutdown_token: &CancellationToken,
    ) -> Result<(QantoBlock, Duration), bool> {
        let mut retry_count = 0;
        let mut _last_error = None;

        while retry_count <= self.config.max_retries {
            if shutdown_token.is_cancelled() {
                return Err(true);
            }

            match self
                .execute_single_mining_attempt(miner, candidate_block, shutdown_token)
                .await
            {
                Ok(result) => return Ok(result),
                Err(cancelled) => {
                    if cancelled {
                        return Err(true);
                    }

                    _last_error = Some("Mining attempt failed".to_string());
                    retry_count += 1;

                    if retry_count <= self.config.max_retries {
                        let delay = self.config.retry_delay_ms * (1 << (retry_count - 1)); // Exponential backoff
                        debug!(
                            "Mining attempt {} failed, retrying in {}ms",
                            retry_count, delay
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        warn!(
            "All mining attempts failed after {} retries",
            self.config.max_retries
        );
        Err(false)
    }

    /// Execute a single mining attempt
    async fn execute_single_mining_attempt(
        &self,
        miner: &Arc<Miner>,
        candidate_block: &mut QantoBlock,
        shutdown_token: &CancellationToken,
    ) -> Result<(QantoBlock, Duration), bool> {
        let mining_start = Instant::now();

        debug!(
            "Starting POW mining for block with difficulty {:.6}",
            candidate_block.difficulty
        );

        let miner_clone = miner.clone();
        let shutdown_clone = shutdown_token.clone();
        let mut candidate_clone = candidate_block.clone();

        // Execute mining in blocking task with timeout
        let mining_timeout = Duration::from_secs(self.config.stall_threshold_secs);

        let mining_result = tokio::time::timeout(
            mining_timeout,
            tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    miner_clone
                        .solve_pow_with_cancellation(&mut candidate_clone, shutdown_clone)
                        .map(|_| candidate_clone)
                }))
            }),
        )
        .await;

        match mining_result {
            Ok(Ok(Ok(Ok(mined_block)))) => {
                let mining_duration = mining_start.elapsed();
                info!("✅ POW mining completed in {:?}", mining_duration);
                Ok((mined_block, mining_duration))
            }
            Ok(Ok(Ok(Err(_)))) => {
                debug!("POW mining failed");
                Err(false)
            }
            Ok(Ok(Err(_))) => {
                warn!("POW mining panicked");
                Err(false)
            }
            Ok(Err(_)) => {
                debug!("POW mining task failed to spawn");
                Err(false)
            }
            Err(_) => {
                warn!("POW mining timed out after {:?}", mining_timeout);
                Err(false)
            }
        }
    }

    /// Process successfully mined block
    async fn process_mined_block(
        &self,
        qanto_dag: &Arc<QantoDAG>,
        mined_block: QantoBlock,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        mining_duration: Duration,
    ) -> Result<(), QantoDAGError> {
        let block_height = mined_block.height;
        let _block_id = mined_block.id.clone();
        let tx_count = mined_block.transactions.len();

        info!(
            "🎉 Successfully mined block #{} with {} transactions in {:?}",
            block_height, tx_count, mining_duration
        );

        // Add block to DAG with retry logic
        const MAX_ADD_RETRIES: u32 = 3;
        let mut add_retry_count = 0;

        loop {
            match qanto_dag.add_block(mined_block.clone(), utxos).await {
                Ok(true) => {
                    info!("✅ Block #{} successfully added to DAG", block_height);

                    // Remove processed transactions from mempool
                    {
                        let mempool_guard = mempool.write().await;
                        let processed_transactions = &mined_block.transactions;
                        mempool_guard
                            .remove_transactions(processed_transactions)
                            .await;
                    }

                    return Ok(());
                }
                Ok(false) => {
                    warn!("Block #{} was rejected by DAG", block_height);
                    return Err(QantoDAGError::InvalidBlock("Block rejected".to_string()));
                }
                Err(e) => {
                    add_retry_count += 1;
                    if add_retry_count >= MAX_ADD_RETRIES {
                        error!(
                            "Failed to add block after {} retries: {}",
                            MAX_ADD_RETRIES, e
                        );
                        return Err(e);
                    }
                    warn!("Failed to add block (attempt {}): {}", add_retry_count, e);
                    tokio::time::sleep(Duration::from_millis(100 * add_retry_count as u64)).await;
                }
            }
        }
    }

    /// Adjust difficulty based on mining performance
    async fn adjust_difficulty_if_needed(&mut self) {
        let now = Instant::now();
        let time_since_last_block = now.duration_since(self.state.last_block_time);
        let time_since_last_adjustment = now.duration_since(self.state.last_difficulty_adjustment);

        // Only adjust difficulty if enough time has passed
        if time_since_last_adjustment < Duration::from_secs(5) {
            return;
        }

        let target_time = Duration::from_millis(self.config.target_block_time_ms);
        let stall_threshold = Duration::from_secs(self.config.stall_threshold_secs);

        let should_reduce_difficulty =
            time_since_last_block > stall_threshold || self.state.consecutive_failures > 5;

        if should_reduce_difficulty {
            let old_difficulty = self.state.current_difficulty;
            self.state.current_difficulty *= self.config.difficulty_reduction_factor;
            self.state.current_difficulty = self
                .state
                .current_difficulty
                .max(self.config.min_difficulty);

            if self.state.current_difficulty != old_difficulty {
                info!(
                    "📉 Reducing difficulty: {:.6} -> {:.6} (stalled for {:?})",
                    old_difficulty, self.state.current_difficulty, time_since_last_block
                );
                self.state.last_difficulty_adjustment = now;
            }
        } else if self.state.consecutive_failures == 0 && time_since_last_block < target_time / 2 {
            // Increase difficulty if blocks are coming too fast
            let old_difficulty = self.state.current_difficulty;
            self.state.current_difficulty *= 1.1; // 10% increase
            self.state.current_difficulty = self
                .state
                .current_difficulty
                .min(self.config.max_difficulty);

            if self.state.current_difficulty != old_difficulty {
                info!(
                    "📈 Increasing difficulty: {:.6} -> {:.6} (blocks too fast)",
                    old_difficulty, self.state.current_difficulty
                );
                self.state.last_difficulty_adjustment = now;
            }
        }
    }

    /// Handle successful mining
    async fn handle_successful_mining(&mut self) {
        self.state.successful_blocks += 1;
        self.state.consecutive_failures = 0;
        self.state.last_block_time = Instant::now();

        self.metrics
            .record_success(Duration::from_millis(100), 0)
            .await; // Approximate time, no retries

        info!(
            "🎯 Mining success! Total blocks: {}, Success rate: {:.1}%",
            self.state.successful_blocks,
            (self.state.successful_blocks as f64 / self.state.total_attempts as f64) * 100.0
        );
    }

    /// Handle mining failure
    async fn handle_mining_failure(&mut self, error: &QantoDAGError) {
        self.state.consecutive_failures += 1;
        self.metrics
            .record_failure(MiningFailureType::MiningError)
            .await;

        warn!(
            "⚠️  Mining failure #{}: {} (consecutive failures: {})",
            self.state.total_attempts, error, self.state.consecutive_failures
        );
    }

    /// Record mining attempt for diagnostics
    async fn record_mining_attempt(
        &self,
        attempt_id: u64,
        start_time: Instant,
        success: bool,
        error_message: Option<String>,
    ) {
        if let Some(diagnostics) = &self.diagnostics {
            let system_start = SystemTime::now() - start_time.elapsed();
            diagnostics
                .record_mining_attempt(MiningAttemptParams {
                    attempt_id,
                    difficulty: self.state.current_difficulty as u64,
                    target_block_time: self.config.target_block_time_ms,
                    start_time: system_start,
                    nonce: 0,         // nonce - would need to be passed from mining result
                    hash_rate: 100.0, // hash_rate - approximate
                    success,
                    error_message,
                })
                .await
                .ok();
        }
    }

    /// Get current mining statistics
    pub fn get_mining_stats(&self) -> AdaptiveMiningStats {
        let success_rate = if self.state.total_attempts > 0 {
            (self.state.successful_blocks as f64 / self.state.total_attempts as f64) * 100.0
        } else {
            0.0
        };

        let uptime = self
            .state
            .mining_start_time
            .map(|start| start.elapsed())
            .unwrap_or_default();

        AdaptiveMiningStats {
            total_attempts: self.state.total_attempts,
            successful_blocks: self.state.successful_blocks,
            consecutive_failures: self.state.consecutive_failures,
            current_difficulty: self.state.current_difficulty,
            success_rate,
            uptime_secs: uptime.as_secs(),
        }
    }
}

/// Mining statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveMiningStats {
    pub total_attempts: u64,
    pub successful_blocks: u64,
    pub consecutive_failures: u32,
    pub current_difficulty: f64,
    pub success_rate: f64,
    pub uptime_secs: u64,
}
