#![allow(async_fn_in_trait)]
#![allow(deprecated)]
//! Adaptive mining, consolidated.
//!
//! This file merges two previously separate modules:
//! - Core ASERT difficulty implementation (header-driven, deterministic)
//! - Node-side adaptive mining loop (circuit breaker, retries, metrics)
//!
//! To avoid crate cycles, the loop is generalized behind a `MiningAdapter`
//! trait. The `qanto` node crate implements this adapter to bridge concrete
//! types (DAG, Wallet, Miner, Mempool, UTXO, metrics, diagnostics).
//!
//! Design goals:
//! - Determinism and correctness first
//! - No direct dependencies on the `qanto` node crate from `qanto-core`
//! - Preserve functionality: difficulty adjustment, retries, circuit breaker,
//!   diagnostics, and metrics via adapter hooks

use async_trait::async_trait;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Anchor data used by ASERT.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Anchor {
    pub height: u64,
    pub timestamp: u64,
    pub target: u64,
}

/// Configuration for ASERT difficulty.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AsertDifficultyConfig {
    /// Ideal block time in milliseconds.
    pub ideal_block_time_ms: i64,
    /// Halflife in milliseconds; time for difficulty error to halve.
    pub halflife_ms: i64,
    /// Minimum target clamp (prevents zero/underflow).
    pub min_target: u64,
    /// Maximum target clamp (prevents overflow/absurdly easy targets).
    pub max_target: u64,
}

impl Default for AsertDifficultyConfig {
    fn default() -> Self {
        Self {
            // 32 BPS = ~31.25ms per block
            ideal_block_time_ms: 31,
            halflife_ms: 1500,
            min_target: 1,
            max_target: u64::MAX / 4,
        }
    }
}

/// Trait for difficulty algorithms.
pub trait DifficultyAlgorithm {
    /// Compute next target based on the anchor and current header data.
    ///
    /// Inputs must be derived from headers only, not wall clock.
    fn next_target(&self, anchor: &Anchor, current_height: u64, current_timestamp: u64) -> u64;
}

/// Modern Difficulty Protocol (v2.0)
///
/// Implements an advanced, adaptive difficulty algorithm optimized for
/// high-throughput DAGs (32 BPS). It uses a modified ASERT algorithm
/// with enhanced damping and non-linear adjustment for rapid stabilization.
///
/// Features:
/// - Drift Correction: Automatically corrects for clock drift
/// - Manipulation Resistance: Clamps extreme timestamp variations
/// - High Precision: Uses 64-bit floating point with deterministic rounding
#[derive(Debug, Clone, Copy)]
pub struct ModernDifficulty {
    pub cfg: AsertDifficultyConfig,
}

/// Backward compatibility alias
pub type AsertDifficulty = ModernDifficulty;

impl ModernDifficulty {
    pub fn new(cfg: AsertDifficultyConfig) -> Self {
        Self { cfg }
    }

    fn clamp_target(&self, target: u128) -> u64 {
        let t = target
            .min(self.cfg.max_target as u128)
            .max(self.cfg.min_target as u128);
        t as u64
    }

    pub fn next_target_u256(
        &self,
        anchor: &Anchor,
        current_height: u64,
        current_timestamp: u64,
    ) -> U256 {
        let t = self.next_target(anchor, current_height, current_timestamp);
        U256::from(t)
    }
}

/// Deprecated: Use AsertDifficultyConfig instead
#[deprecated(since = "0.2.0", note = "Use AsertDifficultyConfig instead")]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct EwmaDifficultyConfig {
    pub ideal_block_time_ms: i64,
    pub window_blocks: usize,
    pub min_target: u64,
    pub max_target: u64,
}

impl Default for EwmaDifficultyConfig {
    fn default() -> Self {
        Self {
            ideal_block_time_ms: 50,
            window_blocks: 30,
            min_target: 1,
            max_target: u64::MAX / 4,
        }
    }
}

/// Deprecated: Use AsertDifficulty instead
#[deprecated(since = "0.2.0", note = "Use AsertDifficulty instead")]
#[derive(Debug, Clone, Copy)]
pub struct EwmaDifficulty {
    pub cfg: EwmaDifficultyConfig,
}

impl EwmaDifficulty {
    pub fn new(cfg: EwmaDifficultyConfig) -> Self {
        Self { cfg }
    }

    fn clamp_target(&self, target: u128) -> u64 {
        target
            .min(self.cfg.max_target as u128)
            .max(self.cfg.min_target as u128) as u64
    }
}

impl DifficultyAlgorithm for EwmaDifficulty {
    fn next_target(&self, anchor: &Anchor, current_height: u64, current_timestamp: u64) -> u64 {
        let height_delta = current_height as i64 - anchor.height as i64;
        let time_delta_ms = ((current_timestamp as i128 - anchor.timestamp as i128) as i64) * 1000;
        let ideal_ms = self.cfg.ideal_block_time_ms;
        let error_ms = time_delta_ms - ideal_ms * height_delta;
        let alpha = 2.0f64 / (self.cfg.window_blocks as f64 + 1.0);
        let exponent =
            (error_ms as f64) * alpha / (self.cfg.window_blocks as f64 * ideal_ms as f64);
        let factor = 2.0f64.powf(exponent);
        let adjusted = (anchor.target as f64) * factor;
        self.clamp_target(adjusted.round() as u128)
    }
}

impl DifficultyAlgorithm for ModernDifficulty {
    fn next_target(&self, anchor: &Anchor, current_height: u64, current_timestamp: u64) -> u64 {
        if current_height < 10 {
            let forced_target = (self.cfg.max_target as u128).saturating_mul(1_000);
            return self.clamp_target(forced_target);
        }

        let height_delta = current_height as i64 - anchor.height as i64;

        if height_delta <= 0 {
            return anchor.target;
        }

        let time_delta_ms = ((current_timestamp as i128 - anchor.timestamp as i128) as i64) * 1000;
        let ideal_ms = self.cfg.ideal_block_time_ms;
        let halflife_ms = self.cfg.halflife_ms;

        let effective_halflife = if time_delta_ms < ideal_ms * height_delta {
            halflife_ms / 2
        } else {
            halflife_ms
        };

        let error_ms = (time_delta_ms - ideal_ms * height_delta) as i128;
        let denom_ms = effective_halflife.max(1) as i128;

        const FP_SHIFT: u32 = 32;
        const FP_ONE: i128 = 1i128 << FP_SHIFT;
        const LN2_FP: i128 = 2_976_040_518;

        let mut x_fp = (error_ms.saturating_mul(FP_ONE)) / denom_ms;
        let min_x_fp = -(8i128 << FP_SHIFT);
        let max_x_fp = 8i128 << FP_SHIFT;
        if x_fp < min_x_fp {
            x_fp = min_x_fp;
        } else if x_fp > max_x_fp {
            x_fp = max_x_fp;
        }

        let mut n = (x_fp >> FP_SHIFT) as i64;
        let mut f_fp = x_fp - ((n as i128) << FP_SHIFT);
        if f_fp < 0 {
            n -= 1;
            f_fp = x_fp - ((n as i128) << FP_SHIFT);
        }

        let y_fp = (f_fp.saturating_mul(LN2_FP)) >> FP_SHIFT;
        let y2_fp = (y_fp.saturating_mul(y_fp)) >> FP_SHIFT;
        let y3_fp = (y2_fp.saturating_mul(y_fp)) >> FP_SHIFT;

        let e_fp = FP_ONE
            .saturating_add(y_fp)
            .saturating_add(y2_fp / 2)
            .saturating_add(y3_fp / 6);

        let factor_fp: i128 = if n >= 0 {
            let shift = n.min(127) as u32;
            e_fp.checked_shl(shift).unwrap_or(i128::MAX)
        } else {
            let shift = (-n).min(127) as u32;
            if shift >= 128u32 {
                0
            } else {
                e_fp >> shift
            }
        };

        let scaled = (anchor.target as u128).saturating_mul(factor_fp.max(0) as u128);
        let adjusted = scaled >> FP_SHIFT;
        self.clamp_target(adjusted)
    }
}

// -----------------------------------------------------------------------------
// Adaptive Mining Config, Circuit Breaker, and State
// -----------------------------------------------------------------------------

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
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(5),
            last_failure_time: None,
            success_threshold: 2,
            half_open_successes: 0,
        }
    }
}

impl MiningCircuitBreaker {
    pub fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::Open => {
                // Move to half-open when first success attempt after timeout
                self.state = CircuitBreakerState::HalfOpen;
                self.half_open_successes = 1;
            }
            CircuitBreakerState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.success_threshold {
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                    self.half_open_successes = 0;
                }
            }
            CircuitBreakerState::Closed => {
                // Normal operation
            }
        }
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitBreakerState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                }
            }
            CircuitBreakerState::HalfOpen => {
                // If failure during half-open, go back to open
                self.state = CircuitBreakerState::Open;
                self.half_open_successes = 0;
            }
            CircuitBreakerState::Open => {
                // Already open, keep failing fast
            }
        }
    }

    pub fn should_allow_request(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Allow probing after timeout
                if let Some(last) = self.last_failure_time {
                    if last.elapsed() >= self.recovery_timeout {
                        self.state = CircuitBreakerState::HalfOpen;
                        self.half_open_successes = 0;
                        return true;
                    }
                }
                false
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    pub fn get_state_description(&self) -> &'static str {
        match self.state {
            CircuitBreakerState::Closed => "closed",
            CircuitBreakerState::Open => "open",
            CircuitBreakerState::HalfOpen => "half-open",
        }
    }
}

/// Mining loop configuration for testnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestnetMiningConfig {
    pub adaptive_difficulty: bool,
    pub target_block_time_ms: u64,
    pub stall_threshold_secs: u64,
    pub difficulty_reduction_factor: f64,
    pub min_difficulty: f64,
    pub max_difficulty: f64,
    pub enable_retry_logic: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub skip_vdf_in_test: bool,
    pub enable_circuit_breaker: bool,
    pub enable_graceful_degradation: bool,
    pub memory_pressure_threshold_mb: u64,
    pub pid_enable: bool,
    pub chain_type: String,
}

impl Default for TestnetMiningConfig {
    fn default() -> Self {
        Self {
            adaptive_difficulty: true,
            target_block_time_ms: 1_875,
            stall_threshold_secs: 10,
            difficulty_reduction_factor: 0.5,
            min_difficulty: 0.0001,
            max_difficulty: 10_000.0,
            enable_retry_logic: true,
            max_retries: 3,
            retry_delay_ms: 100,
            skip_vdf_in_test: true,
            enable_circuit_breaker: true,
            enable_graceful_degradation: true,
            memory_pressure_threshold_mb: 2_048,
            pid_enable: false,
            chain_type: "mainnet".to_string(),
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
            current_difficulty: 0.001,
            last_block_time: Instant::now(),
            consecutive_failures: 0,
            total_attempts: 0,
            successful_blocks: 0,
            last_difficulty_adjustment: Instant::now(),
            mining_start_time: None,
            recent_block_times: Vec::with_capacity(32),
            window_size: 16,
            circuit_breaker: MiningCircuitBreaker::default(),
            degraded_mode: false,
            last_memory_check: Instant::now(),
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

/// Generic diagnostics payload used by the adapter to report attempt metadata.
#[derive(Debug, Clone)]
pub struct MiningAttemptReport {
    pub attempt_id: u64,
    pub difficulty: u64,
    pub target_block_time_ms: u64,
    pub start_time: SystemTime,
    pub nonce: u64,
    pub hash_rate: f64,
    pub success: bool,
    pub error_message: Option<String>,
}

// -----------------------------------------------------------------------------
// Mining Adapter Abstraction
// -----------------------------------------------------------------------------

/// Adapter that bridges the core mining loop with node-specific types.
///
/// The node crate provides an implementation that uses `QantoDAG`, `Wallet`,
/// `Miner`, `Mempool`, `UTXO`, metrics, and diagnostics.
#[async_trait]
pub trait MiningAdapter: Send + Sync + 'static {
    type Block: Clone + Send + Sync + std::fmt::Display + 'static;
    type Error: std::fmt::Display + Send + Sync + 'static;

    /// Return the number of pending transactions currently in the mempool.
    async fn pending_transactions_len(&self) -> usize;

    /// Create a candidate block using the current adapter state and override
    /// difficulty to `difficulty`.
    async fn create_candidate_block(&self, difficulty: f64) -> Result<Self::Block, Self::Error>;

    /// Apply difficulty override to the block instance in-place.
    fn set_block_difficulty(&self, block: &mut Self::Block, difficulty: f64);

    /// Spawn mining for a block with cancellation support.
    async fn mine_block(
        &self,
        block: &mut Self::Block,
        shutdown: &CancellationToken,
    ) -> Result<Self::Block, Self::Error>;

    /// Attempt to add a mined block to the DAG/storage.
    async fn add_block(&self, block: &Self::Block) -> Result<bool, Self::Error>;

    /// Post-addition handling: mempool removals, UTXO updates, metrics logging.
    async fn post_add_block(&self, block: &Self::Block) -> Result<(), Self::Error>;

    /// Record a mining attempt for diagnostics.
    async fn record_attempt(&self, report: MiningAttemptReport);

    /// Record success metrics (duration and retry count).
    async fn record_success_metrics(&self, mining_duration: Duration, retries: u32);

    /// Record a failure for metrics.
    async fn record_failure_metrics(&self, failure_label: &'static str);

    /// Convenience accessors used for logging.
    fn block_height(&self, block: &Self::Block) -> u64;
    fn block_id(&self, block: &Self::Block) -> String;
    fn tx_count(&self, block: &Self::Block) -> usize;

    /// Return the number of connected peers (for bootstrap mode check).
    async fn connected_peers_len(&self) -> usize;

    /// Whether GPU mining is enabled/available.
    fn use_gpu(&self) -> bool {
        false
    }

    /// Number of mining threads to use.
    fn mining_threads(&self) -> usize {
        1
    }

    /// Convert block target to bytes.
    fn get_block_target(&self, block: &Self::Block) -> [u8; 32];

    /// Get block header hash for mining.
    fn get_block_header_hash(&self, block: &Self::Block) -> [u8; 32];

    /// Set nonce and effort on successful mine.
    fn set_block_nonce(&self, block: &mut Self::Block, nonce: u64, effort: u64);

    /// Get the current consensus difficulty (ASERT) from the adapter.
    async fn get_current_difficulty(&self) -> Option<f64> {
        None
    }

    /// Startup mode hint for testnet bootstrap
    /// When enabled, the mining loop will force a very low local difficulty for first cycles
    fn startup_mode(&self) -> bool {
        false
    }
}

// -----------------------------------------------------------------------------
// Adaptive Mining Loop (generic over MiningAdapter)
// -----------------------------------------------------------------------------

/// Enhanced mining loop with adaptive difficulty, retries, and circuit breaker.
pub struct AdaptiveMiningLoop<A: MiningAdapter> {
    pub config: TestnetMiningConfig,
    pub state: AdaptiveMiningState,
    pub adapter: A,
}

impl<A: MiningAdapter> AdaptiveMiningLoop<A> {
    pub fn new(config: TestnetMiningConfig, adapter: A) -> Self {
        Self {
            config,
            state: AdaptiveMiningState::default(),
            adapter,
        }
    }

    async fn check_system_health(&mut self) {
        // Simple memory/health heuristic toggling degraded mode
        let memory_mb = self.estimate_memory_usage().await;
        if self.config.enable_graceful_degradation
            && memory_mb > self.config.memory_pressure_threshold_mb
        {
            if !self.state.degraded_mode {
                warn!(
                    "Memory pressure detected: {}MB > {}MB. Enabling graceful degradation.",
                    memory_mb, self.config.memory_pressure_threshold_mb
                );
                self.state.degraded_mode = true;
            }
        } else if self.state.degraded_mode {
            info!("Memory pressure relieved. Disabling graceful degradation.");
            self.state.degraded_mode = false;
        }
        self.state.last_memory_check = Instant::now();
    }

    /// Placeholder memory estimate; adapter can refine via metrics hooks if needed.
    async fn estimate_memory_usage(&self) -> u64 {
        512 // MB
    }

    /// Main adaptive mining loop with enhanced error handling and retry logic.
    pub async fn run_adaptive_mining_loop(
        &mut self,
        shutdown_token: CancellationToken,
    ) -> Result<(), A::Error> {
        info!("🚀 Starting resilient adaptive mining loop for testnet");
        self.state.mining_start_time = Some(Instant::now());

        loop {
            if shutdown_token.is_cancelled() {
                info!("🛑 Adaptive mining loop received shutdown signal");
                break;
            }

            // Health checks
            self.check_system_health().await;

            // Circuit breaker gate
            if self.config.enable_circuit_breaker
                && !self.state.circuit_breaker.should_allow_request()
            {
                debug!("⚡ Circuit breaker is OPEN, skipping mining attempt");
                tokio::time::sleep(Duration::from_millis(1000)).await;
                continue;
            }

            // Always attempt mining (Heartbeat Protocol)
            // If transactions are empty, we mine an empty block to keep the chain alive.
            // The `create_candidate_block_with_difficulty` handles empty tx selection.

            match self.execute_adaptive_mining_cycle(&shutdown_token).await {
                Ok(true) => {
                    self.state.circuit_breaker.record_success();
                    // Immediate continue for next block
                }
                Ok(false) => {
                    // Cycle skipped or no-op
                    // If mempool is empty and we are in a strict no-empty-block mode (not this case), we might sleep.
                    // But here we want heartbeat mining.
                    // If we are here, it means `execute_adaptive_mining_cycle` returned false, probably due to cancellation.
                }
                Err(_e) => {
                    self.state.circuit_breaker.record_failure();
                    self.adapter
                        .record_failure_metrics("mining_cycle_error")
                        .await;
                    debug!(
                        "Failed mining cycle, circuit breaker state: {}",
                        self.state.circuit_breaker.get_state_description()
                    );
                    // On error, small backoff to avoid hot loop if persistent error
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Ok(())
    }

    /// Execute a single mining cycle with adaptive difficulty and retry logic.
    async fn execute_adaptive_mining_cycle(
        &mut self,
        shutdown_token: &CancellationToken,
    ) -> Result<bool, A::Error> {
        self.state.total_attempts += 1;
        let attempt_start = Instant::now();
        let attempt_id = self.state.total_attempts;

        // Difficulty adjustment
        if self.config.adaptive_difficulty {
            self.adjust_difficulty_if_needed().await;
        }

        info!(
            "⛏️  Mining attempt #{} - Difficulty: {:.6}, Target: {}ms",
            attempt_id, self.state.current_difficulty, self.config.target_block_time_ms
        );

        // Record diagnostics
        self.adapter
            .record_attempt(MiningAttemptReport {
                attempt_id,
                difficulty: self.state.current_difficulty as u64,
                target_block_time_ms: self.config.target_block_time_ms,
                start_time: SystemTime::now(),
                nonce: 0,
                hash_rate: 0.0,
                success: false,
                error_message: None,
            })
            .await;

        // Create candidate block
        let mut candidate_block = self.create_candidate_block_with_difficulty().await?;

        // Mining with retries
        let mining_result = self
            .execute_mining_with_retries(&mut candidate_block, shutdown_token)
            .await;

        match mining_result {
            Ok((mined_block, mining_duration)) => {
                // Success path
                self.handle_successful_mining().await;

                // Process mined block
                self.process_mined_block(mined_block, mining_duration)
                    .await?;

                // Record
                self.adapter
                    .record_success_metrics(mining_duration, 0)
                    .await;

                // End attempt report
                self.adapter
                    .record_attempt(MiningAttemptReport {
                        attempt_id,
                        difficulty: self.state.current_difficulty as u64,
                        target_block_time_ms: self.config.target_block_time_ms,
                        start_time: SystemTime::now() - attempt_start.elapsed(),
                        nonce: 0,
                        hash_rate: 0.0,
                        success: true,
                        error_message: None,
                    })
                    .await;

                Ok(true)
            }
            Err(cancelled) => {
                if cancelled {
                    // Graceful cancellation
                    return Ok(false);
                }

                // Failure path
                self.handle_mining_failure().await;

                self.adapter
                    .record_attempt(MiningAttemptReport {
                        attempt_id,
                        difficulty: self.state.current_difficulty as u64,
                        target_block_time_ms: self.config.target_block_time_ms,
                        start_time: SystemTime::now() - attempt_start.elapsed(),
                        nonce: 0,
                        hash_rate: 0.0,
                        success: false,
                        error_message: Some("Mining failed".to_string()),
                    })
                    .await;
                Ok(false)
            }
        }
    }

    /// Create candidate block with adaptive difficulty.
    async fn create_candidate_block_with_difficulty(&self) -> Result<A::Block, A::Error> {
        let tx_count = self.adapter.pending_transactions_len().await;
        info!(
            "Creating candidate block with {} transactions, difficulty: {:.6}",
            tx_count, self.state.current_difficulty
        );

        let block = self
            .adapter
            .create_candidate_block(self.state.current_difficulty)
            .await?;
        // Difficulty is determined by the adapter (e.g., DAG’s ASERT consensus).
        // Do not override here to preserve deterministic, header-driven difficulty.
        Ok(block)
    }

    /// Execute mining with retry logic and exponential backoff.
    async fn execute_mining_with_retries(
        &self,
        candidate_block: &mut A::Block,
        shutdown_token: &CancellationToken,
    ) -> Result<(A::Block, Duration), bool> {
        let mut retry_count = 0;
        let mut _last_error: Option<String> = None;

        while retry_count <= self.config.max_retries {
            if shutdown_token.is_cancelled() {
                return Err(true);
            }

            match self
                .execute_single_mining_attempt(candidate_block, shutdown_token)
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
                        // Immediate retry for continuous mining pressure
                        debug!(
                            "Mining attempt {} failed/timed out, retrying immediately with fresh nonce",
                            retry_count
                        );
                        // Do not sleep here - we want to keep the GPU/CPU busy
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

    /// Execute a single mining attempt.
    async fn execute_single_mining_attempt(
        &self,
        candidate_block: &mut A::Block,
        shutdown_token: &CancellationToken,
    ) -> Result<(A::Block, Duration), bool> {
        let mining_start = Instant::now();
        debug!(
            "Starting POW mining for block with difficulty {:.6}",
            self.state.current_difficulty
        );

        let mining_timeout = Duration::from_millis(self.config.target_block_time_ms)
            .checked_add(Duration::from_secs(60))
            .unwrap_or(Duration::from_secs(91));

        // Non-blocking mining using tokio::select!
        // This ensures we yield immediately on shutdown (new block) or timeout
        let mining_result = tokio::select! {
            // 1. Shutdown signal (e.g. new block arrived)
            _ = shutdown_token.cancelled() => {
                debug!("Mining cancelled by shutdown signal");
                Err(true) // true = cancelled
            }
            // 2. Timeout
            _ = tokio::time::sleep(mining_timeout) => {
                warn!("POW mining timed out after {:?}", mining_timeout);
                Err(false) // false = failed/timeout
            }
            // 3. Mining task completion
            result = self.adapter.mine_block(candidate_block, shutdown_token) => {
                match result {
                    Ok(block) => Ok(Ok(block)),
                    Err(e) => Ok(Err(e)),
                }
            }
        };

        match mining_result {
            Ok(Ok(mined_block)) => {
                let mining_duration = mining_start.elapsed();
                info!("✅ POW mining completed in {:?}", mining_duration);
                Ok((mined_block, mining_duration))
            }
            Ok(Err(_e)) => {
                debug!("POW mining failed");
                Err(false)
            }
            Err(cancelled) => {
                if cancelled {
                    // Graceful cancellation
                    Err(true)
                } else {
                    // Timeout
                    Err(false)
                }
            }
        }
    }

    /// Process successfully mined block with celebration and DAG insertion.
    async fn process_mined_block(
        &self,
        mined_block: A::Block,
        mining_duration: Duration,
    ) -> Result<(), A::Error> {
        let block_height = self.adapter.block_height(&mined_block);
        let block_id = self.adapter.block_id(&mined_block);
        let tx_count = self.adapter.tx_count(&mined_block);

        // Add block to DAG with enhanced retry logic
        const MAX_ADD_RETRIES: u32 = 5;
        let mut add_retry_count = 0;

        while add_retry_count < MAX_ADD_RETRIES {
            match self.adapter.add_block(&mined_block).await {
                Ok(true) => {
                    info!("✅ Block #{} successfully added to DAG", block_height);

                    // Display success block AFTER it is safely in the DAG/Storage
                    if block_height == 1 {
                        info!("🎊🎉 FIRST BLOCK MINED POST-GENESIS! 🎉🎊");
                        info!("🏆 Block #{} - ID: {}", block_height, block_id);
                        info!("⚡ Mining Duration: {:?}", mining_duration);
                        info!("📦 Transactions: {}", tx_count);
                        info!("🚀 Qanto blockchain is now LIVE and mining successfully!");
                        info!("🎊🎉 CELEBRATION COMPLETE! 🎉🎊");
                    } else {
                        // Explicit call to display block success (via Display trait)
                        info!("\n{}", mined_block);
                    }

                    break;
                }
                Ok(false) => {
                    add_retry_count += 1;
                    if add_retry_count < MAX_ADD_RETRIES {
                        let delay = Duration::from_millis(100 * (1 << add_retry_count));
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
                        return Err(self.adapter.add_block(&mined_block).await.err().unwrap());
                    }
                }
                Err(e) => {
                    add_retry_count += 1;
                    if add_retry_count < MAX_ADD_RETRIES {
                        let delay = Duration::from_millis(100 * (1 << add_retry_count));
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

        // Post-addition handling (mempool/UTXO updates, metrics)
        self.adapter.post_add_block(&mined_block).await?;

        // Log mining metrics
        self.adapter
            .record_success_metrics(mining_duration, 0)
            .await;

        info!(
            "📊 Mining metrics - Block #{}: {} transactions, {:.2}ms duration",
            block_height,
            tx_count,
            mining_duration.as_millis(),
        );

        Ok(())
    }

    /// Adjust difficulty based on mining performance.
    pub async fn adjust_difficulty_if_needed(&mut self) {
        // Startup clamp: force low difficulty in standalone bootstrap mode
        if self.adapter.startup_mode() {
            let forced = 0.001f64;
            if (forced - self.state.current_difficulty).abs() > f64::EPSILON {
                self.state.current_difficulty = forced;
                self.state.last_difficulty_adjustment = Instant::now();
            }
            return;
        }

        if std::env::var("QANTO_DEV_MODE")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            let forced = 0.00001f64;
            if (forced - self.state.current_difficulty).abs() > f64::EPSILON {
                self.state.current_difficulty = forced;
                self.state.last_difficulty_adjustment = Instant::now();
            }
            return;
        }
        // Try to get difficulty from adapter (ASERT)
        if let Some(diff) = self.adapter.get_current_difficulty().await {
            if (diff - self.state.current_difficulty).abs() > f64::EPSILON {
                let old = self.state.current_difficulty;
                self.state.current_difficulty = diff;
                self.state.last_difficulty_adjustment = Instant::now();
                debug!(
                    "Difficulty updated from adapter (ASERT): {:.6} -> {:.6}",
                    old, diff
                );
            }
            return;
        }

        let now = Instant::now();
        let time_since_last_adjustment = now.duration_since(self.state.last_difficulty_adjustment);

        // Cooldown period to prevent oscillations
        if time_since_last_adjustment < Duration::from_secs(2) {
            return;
        }

        let target_time_ms = self.config.target_block_time_ms;
        let stall_threshold = Duration::from_secs(self.config.stall_threshold_secs);
        let time_since_last_block = now.duration_since(self.state.last_block_time);

        // Calculate sliding window average if we have history; fallback when empty
        let avg_block_time_ms = if !self.state.recent_block_times.is_empty() {
            self.calculate_sliding_window_average()
        } else {
            time_since_last_block.as_millis() as f64
        };

        let difficulty_adjustment_factor = if self.config.pid_enable {
            calculate_next_difficulty(&self.state.recent_block_times)
        } else {
            self.calculate_difficulty_adjustment_factor(
                avg_block_time_ms,
                target_time_ms as f64,
                time_since_last_block,
                stall_threshold,
            )
        };

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

    /// Calculate difficulty adjustment factor using a dampened proportional scheme
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

        // Proportional adjustment with dampening
        let time_ratio = target_time_ms / avg_block_time_ms;
        let clamped_ratio = time_ratio.clamp(0.75, 1.25);
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

        info!(
            "🎯 Mining success! Total blocks: {}, Success rate: {:.1}%",
            self.state.successful_blocks,
            self.calculate_success_rate()
        );
    }

    /// Handle mining failure
    async fn handle_mining_failure(&mut self) {
        self.state.consecutive_failures += 1;
        self.adapter.record_failure_metrics("mining_error").await;

        warn!(
            "⚠️  Mining failure #{} (consecutive failures: {})",
            self.state.total_attempts, self.state.consecutive_failures
        );
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

        let success_rate =
            (self.state.successful_blocks as f64 / self.state.total_attempts as f64) * 100.0;
        (success_rate * 10.0).round() / 10.0
    }
}

pub fn calculate_next_difficulty(history: &[u64]) -> f64 {
    let target_ms = 50.0;
    let kp = 0.12;
    let ki = 0.015;
    let kd = 0.08;
    if history.is_empty() {
        return 1.0;
    }
    let avg = (history.iter().sum::<u64>() as f64) / (history.len() as f64);
    let mut integral = 0.0;
    for &t in history {
        integral += target_ms - t as f64;
    }
    let error = target_ms - avg;
    let derivative = if history.len() >= 2 {
        let n = history.len();
        let prev = history[n - 2] as f64;
        let last = history[n - 1] as f64;
        prev - last
    } else {
        0.0
    };
    let output = 1.0
        + kp * (error / target_ms)
        + ki * (integral / (target_ms * history.len() as f64))
        + kd * (derivative / target_ms);
    output.clamp(0.1, 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AsertDifficultyConfig {
        AsertDifficultyConfig {
            ideal_block_time_ms: 5000, // 5 seconds for testing
            halflife_ms: 300000,
            min_target: 1,
            max_target: u64::MAX / 4,
        }
    }

    #[test]
    fn genesis_block_uses_anchor_target() {
        let algo = AsertDifficulty::new(cfg());
        let anchor = Anchor {
            height: 10,
            timestamp: 1_700_000_000,
            target: 100_000,
        };
        // Next block occurs exactly at ideal cadence.
        let next = algo.next_target(
            &anchor,
            11,
            anchor.timestamp + (algo.cfg.ideal_block_time_ms as u64 / 1000),
        );
        assert_eq!(next, anchor.target);
    }

    #[test]
    fn large_time_gap_increases_target() {
        let algo = AsertDifficulty::new(cfg());
        let anchor = Anchor {
            height: 100,
            timestamp: 1_700_000_000,
            target: 80_000,
        };
        // 10 minutes late relative to ideal schedule
        let current_ts = anchor.timestamp + (10 * 60);
        let next = algo.next_target(&anchor, anchor.height + 1, current_ts);
        assert!(next > anchor.target);
    }

    #[test]
    fn rapid_sequence_reduces_target() {
        let algo = AsertDifficulty::new(cfg());
        let anchor = Anchor {
            height: 200,
            timestamp: 1_700_000_000,
            target: 120_000,
        };
        // Arrives 1 second after anchor; faster than ideal 5s
        let current_ts = anchor.timestamp + 1;
        let next = algo.next_target(&anchor, anchor.height + 1, current_ts);
        assert!(next < anchor.target);
    }

    #[test]
    fn clamped_bounds_apply() {
        let cfg = AsertDifficultyConfig {
            ideal_block_time_ms: 5000,
            halflife_ms: 300000,
            min_target: 10,
            max_target: 20,
        };
        let algo = AsertDifficulty::new(cfg);
        let anchor = Anchor {
            height: 0,
            timestamp: 1_700_000_000,
            target: 15,
        };
        let slow = algo.next_target(&anchor, 1, anchor.timestamp + 10_000);
        let fast = algo.next_target(&anchor, anchor.height + 100, anchor.timestamp + 1);
        assert_eq!(slow, 20);
        assert_eq!(fast, 10);
    }
}
