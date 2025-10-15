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

/// Circuit breaker states for mining resilience
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing fast, not attempting mining
    HalfOpen, // Testing if service has recovered
}

/// Circuit breaker for mining operations
#[derive(Debug, Clone)]
pub struct MiningCircuitBreaker {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub last_failure_time: Option<Instant>,
    pub success_threshold: u32, // Successes needed in half-open to close
    pub half_open_successes: u32,
}

impl Default for MiningCircuitBreaker {
    fn default() -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            failure_threshold: 5, // Open after 5 consecutive failures
            recovery_timeout: Duration::from_secs(30), // Wait 30s before trying again
            last_failure_time: None,
            success_threshold: 2, // Need 2 successes to close from half-open
            half_open_successes: 0,
        }
    }
}

impl MiningCircuitBreaker {
    /// Record a successful operation
    pub fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            }
            CircuitBreakerState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.success_threshold {
                    info!("🔄 Mining circuit breaker: CLOSED (recovered)");
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                    self.half_open_successes = 0;
                    self.last_failure_time = None;
                }
            }
            CircuitBreakerState::Open => {
                // Should not happen, but reset if it does
                warn!("Unexpected success in OPEN state, resetting circuit breaker");
                self.state = CircuitBreakerState::Closed;
                self.failure_count = 0;
                self.last_failure_time = None;
            }
        }
    }

    /// Record a failed operation
    pub fn record_failure(&mut self) {
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    warn!("🚨 Mining circuit breaker: OPEN (too many failures)");
                    self.state = CircuitBreakerState::Open;
                }
            }
            CircuitBreakerState::HalfOpen => {
                warn!("🔄 Mining circuit breaker: OPEN (failed during recovery test)");
                self.state = CircuitBreakerState::Open;
                self.half_open_successes = 0;
                self.failure_count += 1;
            }
            CircuitBreakerState::Open => {
                // Already open, just increment counter
                self.failure_count += 1;
            }
        }
    }

    /// Check if operation should be allowed
    pub fn should_allow_request(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.recovery_timeout {
                        info!("🔄 Mining circuit breaker: HALF-OPEN (testing recovery)");
                        self.state = CircuitBreakerState::HalfOpen;
                        self.half_open_successes = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    // No last failure time, allow request
                    true
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    /// Get current state description
    pub fn get_state_description(&self) -> &'static str {
        match self.state {
            CircuitBreakerState::Closed => "CLOSED",
            CircuitBreakerState::Open => "OPEN",
            CircuitBreakerState::HalfOpen => "HALF-OPEN",
        }
    }
}

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
    /// Enable circuit breaker for mining resilience
    pub enable_circuit_breaker: bool,
    /// Enable graceful degradation under high load
    pub enable_graceful_degradation: bool,
    /// Memory pressure threshold (MB) for graceful degradation
    pub memory_pressure_threshold_mb: u64,
}

impl Default for TestnetMiningConfig {
    fn default() -> Self {
        Self {
            adaptive_difficulty: true,
            target_block_time_ms: 5000, // Adjusted to 5 seconds target block time for testnet, promoting more stable mining cycles while maintaining network responsiveness
            stall_threshold_secs: 3,    // Reduced from 10 to 3 for faster recovery
            difficulty_reduction_factor: 0.7, // Less aggressive reduction (0.7 vs 0.5)
            min_difficulty: 0.0001,     // Lower minimum for easier mining
            max_difficulty: 100.0,      // Reduced maximum to prevent over-difficulty
            enable_retry_logic: true,
            max_retries: 2,     // Reduced from 3 to 2 for faster cycles
            retry_delay_ms: 50, // Reduced from 100 to 50ms
            skip_vdf_in_test: true,
            enable_circuit_breaker: true,
            enable_graceful_degradation: true,
            memory_pressure_threshold_mb: 1024, // 1GB threshold
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
    /// Sliding window of recent mining times (in milliseconds)
    pub recent_block_times: Vec<u64>,
    /// Maximum size of the sliding window
    pub window_size: usize,
    /// Circuit breaker for mining resilience
    pub circuit_breaker: MiningCircuitBreaker,
    /// Graceful degradation state
    pub degraded_mode: bool,
    /// Last memory check time
    pub last_memory_check: Instant,
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
            recent_block_times: Vec::new(),
            window_size: 10, // Track last 10 blocks for sliding window average
            circuit_breaker: MiningCircuitBreaker::default(),
            degraded_mode: false,
            last_memory_check: Instant::now(),
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

    /// Check system health and enable graceful degradation if needed
    async fn check_system_health(&mut self) {
        if !self.config.enable_graceful_degradation {
            return;
        }

        // Check memory pressure every 10 seconds
        if self.state.last_memory_check.elapsed() < Duration::from_secs(10) {
            return;
        }

        self.state.last_memory_check = Instant::now();

        // Simple memory check using system info (placeholder - would need actual system monitoring)
        let memory_usage_mb = self.estimate_memory_usage().await;

        let should_degrade = memory_usage_mb > self.config.memory_pressure_threshold_mb;

        if should_degrade && !self.state.degraded_mode {
            warn!(
                "🐌 Enabling graceful degradation mode (memory pressure: {}MB)",
                memory_usage_mb
            );
            self.state.degraded_mode = true;
            // Reduce difficulty to ease computational load
            self.state.current_difficulty *= 0.5;
        } else if !should_degrade && self.state.degraded_mode {
            info!(
                "🚀 Disabling graceful degradation mode (memory recovered: {}MB)",
                memory_usage_mb
            );
            self.state.degraded_mode = false;
        }
    }

    /// Estimate current memory usage (placeholder implementation)
    async fn estimate_memory_usage(&self) -> u64 {
        // In a real implementation, this would check actual system memory usage
        // For now, return a placeholder value
        512 // MB
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
        info!("🚀 Starting resilient adaptive mining loop for testnet");
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
                    // Check system health and enable graceful degradation if needed
                    self.check_system_health().await;

                    // Check circuit breaker before attempting mining
                    if self.config.enable_circuit_breaker && !self.state.circuit_breaker.should_allow_request() {
                        debug!("⚡ Circuit breaker is OPEN, skipping mining attempt");
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        continue;
                    }

                    // Check if we should mine based on mempool state
                    let should_mine = {
                        let mempool_guard = mempool.read().await;
                        mempool_guard.len().await > 0
                    };

                    if !should_mine {
                        tokio::task::yield_now().await;
                        continue;
                    }

                    // Log circuit breaker state periodically
                    if self.state.total_attempts.is_multiple_of(100) {
                        debug!("🔄 Circuit breaker state: {}, Degraded mode: {}",
                               self.state.circuit_breaker.get_state_description(),
                               self.state.degraded_mode);
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
                                if self.config.enable_circuit_breaker {
                                    self.state.circuit_breaker.record_success();
                                }
                            }
                        }
                        Err(e) => {
                            if e.to_string().contains("cancelled") {
                                info!("Mining cancelled, exiting loop");
                                break;
                            }
                            self.handle_mining_failure(&e).await;
                            if self.config.enable_circuit_breaker {
                                self.state.circuit_breaker.record_failure();
                            }
                        }
                    }
                }
            }
        }

        info!("🏁 Resilient adaptive mining loop completed");
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
            .create_candidate_block_with_difficulty(qanto_dag, wallet, mempool, utxos, miner)
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
        miner: &Arc<Miner>,
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
                miner,
                None, // homomorphic_public_key
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

    /// Process successfully mined block with celebration for first block
    async fn process_mined_block(
        &self,
        qanto_dag: &Arc<QantoDAG>,
        mined_block: QantoBlock,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        mining_duration: Duration,
    ) -> Result<(), QantoDAGError> {
        let block_height = mined_block.height;
        let block_id = mined_block.id.clone();
        let tx_count = mined_block.transactions.len();

        // Special celebration for first block post-genesis
        if block_height == 1 {
            info!("🎊🎉 FIRST BLOCK MINED POST-GENESIS! 🎉🎊");
            info!("🏆 Block #{} - ID: {}", block_height, block_id);
            info!("⚡ Mining Duration: {:?}", mining_duration);
            info!("📦 Transactions: {}", tx_count);
            info!("💎 Difficulty: {:.6}", mined_block.difficulty);
            info!("🚀 Qanto blockchain is now LIVE and mining successfully!");
            info!("🎊🎉 CELEBRATION COMPLETE! 🎉🎊");
        } else {
            info!(
                "🎉 Successfully mined block #{} with {} transactions in {:?}",
                block_height, tx_count, mining_duration
            );
        }

        // Add block to DAG with enhanced retry logic
        const MAX_ADD_RETRIES: u32 = 5; // Increased from 3 to 5
        let mut add_retry_count = 0;

        while add_retry_count < MAX_ADD_RETRIES {
            match qanto_dag.add_block(mined_block.clone(), utxos).await {
                Ok(true) => {
                    info!("✅ Block #{} successfully added to DAG", block_height);
                    break;
                }
                Ok(false) => {
                    add_retry_count += 1;
                    if add_retry_count < MAX_ADD_RETRIES {
                        let delay = Duration::from_millis(100 * (1 << add_retry_count)); // Exponential backoff
                        warn!(
                            "Block #{} was rejected by DAG (attempt {}/{}). Retrying in {:?}",
                            block_height, add_retry_count, MAX_ADD_RETRIES, delay
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        error!(
                            "Block #{} was rejected by DAG after {} attempts",
                            block_height, MAX_ADD_RETRIES
                        );
                        return Err(QantoDAGError::InvalidBlock(
                            "Block rejected after retries".to_string(),
                        ));
                    }
                }
                Err(e) => {
                    add_retry_count += 1;
                    if add_retry_count < MAX_ADD_RETRIES {
                        let delay = Duration::from_millis(100 * (1 << add_retry_count)); // Exponential backoff
                        warn!(
                            "Failed to add block to DAG (attempt {}/{}): {}. Retrying in {:?}",
                            add_retry_count, MAX_ADD_RETRIES, e, delay
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        error!(
                            "Failed to add block to DAG after {} attempts: {}",
                            MAX_ADD_RETRIES, e
                        );
                        return Err(e);
                    }
                }
            }
        }

        // Remove processed transactions from mempool with retry logic
        let mempool_guard = mempool.write().await;
        mempool_guard
            .remove_transactions(&mined_block.transactions)
            .await;
        debug!("✅ Removed {} transactions from mempool", tx_count);

        // Update UTXO set with retry logic
        const MAX_UTXO_RETRIES: u32 = 3;
        let mut utxo_retry_count = 0;

        while utxo_retry_count < MAX_UTXO_RETRIES {
            let mut utxos_guard = utxos.write().await;

            // This is a simplified UTXO update - in a real implementation,
            // you'd need to properly handle inputs/outputs
            let update_result = async {
                for tx in &mined_block.transactions {
                    // Remove spent UTXOs (inputs)
                    for input in &tx.inputs {
                        utxos_guard.remove(&input.tx_id);
                    }

                    // Add new UTXOs (outputs)
                    for (index, output) in tx.outputs.iter().enumerate() {
                        let utxo_id = format!("{}:{}", tx.id, index);
                        let utxo = UTXO {
                            address: output.address.clone(),
                            amount: output.amount,
                            tx_id: tx.id.clone(),
                            output_index: index as u32,
                            explorer_link: format!("https://explorer.qanto.org/tx/{}", tx.id),
                        };
                        utxos_guard.insert(utxo_id, utxo);
                    }
                }
                Ok::<(), String>(())
            }
            .await;

            match update_result {
                Ok(_) => {
                    debug!("✅ Updated UTXO set for block #{}", block_height);
                    break;
                }
                Err(e) => {
                    utxo_retry_count += 1;
                    if utxo_retry_count < MAX_UTXO_RETRIES {
                        warn!(
                            "Failed to update UTXO set (attempt {}/{}): {}. Retrying...",
                            utxo_retry_count, MAX_UTXO_RETRIES, e
                        );
                        drop(utxos_guard); // Release lock before retry
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    } else {
                        warn!(
                            "Failed to update UTXO set after {} attempts: {}",
                            MAX_UTXO_RETRIES, e
                        );
                        // Don't fail the entire operation for UTXO update issues
                        break;
                    }
                }
            }
        }

        // Log mining metrics
        self.metrics.record_success(mining_duration, 0).await;

        // Record mining success for diagnostics
        info!(
            "📊 Mining metrics - Block #{}: {} transactions, {:.2}ms duration, {:.6} difficulty",
            block_height,
            tx_count,
            mining_duration.as_millis(),
            mined_block.difficulty
        );

        Ok(())
    }

    /// Adjust difficulty based on mining performance with Bitcoin-inspired algorithm
    pub async fn adjust_difficulty_if_needed(&mut self) {
        let now = Instant::now();
        let time_since_last_adjustment = now.duration_since(self.state.last_difficulty_adjustment);

        // Cooldown period to prevent oscillations
        if time_since_last_adjustment < Duration::from_secs(2) {
            return;
        }

        let target_time_ms = self.config.target_block_time_ms;
        let stall_threshold = Duration::from_secs(self.config.stall_threshold_secs);
        let time_since_last_block = now.duration_since(self.state.last_block_time);

        // Calculate sliding window average if we have enough history
        let avg_block_time_ms = if self.state.recent_block_times.len() >= 3 {
            self.calculate_sliding_window_average()
        } else {
            // Fallback to simple heuristic for insufficient history
            time_since_last_block.as_millis() as f64
        };

        // Bitcoin-inspired difficulty adjustment with proper clamping
        let difficulty_adjustment_factor = self.calculate_difficulty_adjustment_factor(
            avg_block_time_ms,
            target_time_ms as f64,
            time_since_last_block,
            stall_threshold,
        );

        if (difficulty_adjustment_factor - 1.0).abs() > 0.01 {
            // Only adjust if the change is significant (> 1%)
            let old_difficulty = self.state.current_difficulty;
            self.state.current_difficulty = (old_difficulty * difficulty_adjustment_factor)
                .max(self.config.min_difficulty)
                .min(self.config.max_difficulty);

            self.state.last_difficulty_adjustment = now;

            debug!(
                "Difficulty adjusted from {:.6} to {:.6} (factor: {:.3}, avg_time: {:.1}ms, target: {}ms)",
                old_difficulty,
                self.state.current_difficulty,
                difficulty_adjustment_factor,
                avg_block_time_ms,
                target_time_ms
            );
        }
    }

    /// Calculate sliding window average of recent block times
    fn calculate_sliding_window_average(&self) -> f64 {
        if self.state.recent_block_times.is_empty() {
            return self.config.target_block_time_ms as f64;
        }

        let sum: u64 = self.state.recent_block_times.iter().sum();
        sum as f64 / self.state.recent_block_times.len() as f64
    }

    /// Calculate difficulty adjustment factor using Bitcoin-inspired algorithm
    fn calculate_difficulty_adjustment_factor(
        &self,
        avg_block_time_ms: f64,
        target_time_ms: f64,
        time_since_last_block: Duration,
        stall_threshold: Duration,
    ) -> f64 {
        // Emergency stall protection (immediate difficulty reduction)
        if time_since_last_block > stall_threshold || self.state.consecutive_failures > 3 {
            return self.config.difficulty_reduction_factor;
        }

        // No failures and very fast blocks - increase difficulty
        if self.state.consecutive_failures == 0 && avg_block_time_ms < target_time_ms / 3.0 {
            return 1.05; // Conservative 5% increase
        }

        // Bitcoin-style proportional adjustment with dampening
        let time_ratio = target_time_ms / avg_block_time_ms;

        // Apply dampening to prevent wild swings (limit to ±25% per adjustment)
        let clamped_ratio = time_ratio.clamp(0.75, 1.25);

        // Further smooth the adjustment and return directly
        1.0 + (clamped_ratio - 1.0) * 0.5
    }

    /// Handle successful mining
    async fn handle_successful_mining(&mut self) {
        self.state.successful_blocks += 1;
        self.state.consecutive_failures = 0;
        let now = Instant::now();

        // Record block time in sliding window
        let block_time_ms = now.duration_since(self.state.last_block_time).as_millis() as u64;
        self.state.recent_block_times.push(block_time_ms);

        // Maintain sliding window size
        if self.state.recent_block_times.len() > self.state.window_size {
            self.state.recent_block_times.remove(0);
        }

        self.state.last_block_time = now;

        self.metrics
            .record_success(Duration::from_millis(100), 0)
            .await; // Approximate time, no retries

        info!(
            "🎯 Mining success! Total blocks: {}, Success rate: {:.1}%",
            self.state.successful_blocks,
            self.calculate_success_rate()
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
        let success_rate = self.calculate_success_rate();

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

    /// Calculate success rate with proper f64 tolerance handling
    pub fn calculate_success_rate(&self) -> f64 {
        if self.state.total_attempts == 0 {
            return 0.0;
        }

        // Use f64 arithmetic throughout to avoid precision issues
        let success_rate =
            (self.state.successful_blocks as f64 / self.state.total_attempts as f64) * 100.0;

        // Round to 1 decimal place for consistency
        (success_rate * 10.0).round() / 10.0
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
