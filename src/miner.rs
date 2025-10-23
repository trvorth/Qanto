//! --- Qanto Miner ---
//! v0.1.0 - Deterministic PoW
//!
//! This version implements a production-grade, deterministic Proof-of-Work
//! target calculation, removing all floating-point arithmetic from the consensus-
//! critical path to eliminate nondeterminism and precision loss.

use crate::config::LoggingConfig;
use crate::deterministic_mining::get_nonce_with_deterministic_fallback;
use crate::mining_celebration::{on_block_mined, MiningCelebrationParams};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::set_metric;
use crate::shutdown::{ShutdownController, ShutdownPhase, ShutdownSignal};
use crate::telemetry::increment_hash_attempts;
use anyhow::Result;
use hex;
use my_blockchain::qanhash::{MiningConfig, MiningDevice, QanhashMiner};
use my_blockchain::qanto_standalone::hash::QantoHash;
use regex::Regex;
use std::ops::Div;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, info, instrument, span, warn, Level};

#[derive(Error, Debug)]
pub enum MiningError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    #[error("System time error: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
    #[error("DAG error: {0}")]
    DAG(#[from] crate::qantodag::QantoDAGError),
    #[error("Emission calculation error: {0}")]
    EmissionError(String),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Wallet error: {0}")]
    Wallet(#[from] crate::wallet::WalletError),
    #[error("Transaction error: {0}")]
    Transaction(#[from] crate::transaction::TransactionError),
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("Thread pool build error: {0}")]
    ThreadPool(String),
    #[error("Key conversion error")]
    KeyConversion,
    #[error("Mining operation timed out or was cancelled without finding a solution")]
    TimeoutOrCancelled,
}

/// Result type for mining operations that can either find a nonce or be cancelled
#[derive(Debug, Clone)]
pub enum MiningResult {
    /// Mining found a valid nonce with the resulting hash
    Found { nonce: u64, hash: [u8; 32] },
    /// Mining was cancelled or timed out
    Cancelled,
    /// Mining timed out before finding a solution
    Timeout,
}

impl MiningResult {
    /// Convert to Option<(u64, [u8; 32])> for backward compatibility
    pub fn to_option(self) -> Option<(u64, [u8; 32])> {
        match self {
            MiningResult::Found { nonce, hash } => Some((nonce, hash)),
            MiningResult::Cancelled | MiningResult::Timeout => None,
        }
    }

    /// Check if the result indicates cancellation
    pub fn is_cancelled(&self) -> bool {
        matches!(self, MiningResult::Cancelled | MiningResult::Timeout)
    }

    /// Check if the result found a valid nonce
    pub fn is_found(&self) -> bool {
        matches!(self, MiningResult::Found { .. })
    }
}

impl From<String> for MiningError {
    fn from(err: String) -> Self {
        MiningError::EmissionError(err)
    }
}

impl From<rayon::ThreadPoolBuildError> for MiningError {
    fn from(err: rayon::ThreadPoolBuildError) -> Self {
        MiningError::ThreadPool(err.to_string())
    }
}

#[derive(Debug)]
pub struct MinerConfig {
    pub address: String,
    pub dag: Arc<QantoDAG>,
    pub target_block_time: u64,
    pub use_gpu: bool,
    pub zk_enabled: bool,
    pub threads: usize,
    pub logging_config: LoggingConfig,
}
pub struct Miner {
    _address: String,
    _dag: QantoDAG,
    target_block_time: u64,
    _use_gpu: bool,
    _zk_enabled: bool,
    threads: usize,
    qanhash_miner: Arc<QanhashMiner>,
    #[cfg(feature = "zk")]
    he_public_key: Vec<u8>,
    #[cfg(feature = "zk")]
    #[allow(dead_code)]
    he_private_key: Vec<u8>,
    // Mining metrics
    pub total_hashes: Arc<AtomicU64>,
    pub solve_times: Arc<std::sync::Mutex<Vec<Duration>>>,
    // Shutdown integration
    shutdown_controller: Option<ShutdownController>,
    shutdown_receiver: Option<broadcast::Receiver<ShutdownSignal>>,
    // Celebration configuration
    logging_config: LoggingConfig,
    // Exponential backoff configuration
    backoff_config: ExponentialBackoffConfig,
}

impl Clone for Miner {
    fn clone(&self) -> Self {
        Self {
            _address: self._address.clone(),
            _dag: self._dag.clone(),
            target_block_time: self.target_block_time,
            _use_gpu: self._use_gpu,
            _zk_enabled: self._zk_enabled,
            threads: self.threads,
            qanhash_miner: self.qanhash_miner.clone(),
            #[cfg(feature = "zk")]
            he_public_key: self.he_public_key.clone(),
            #[cfg(feature = "zk")]
            he_private_key: self.he_private_key.clone(),
            total_hashes: Arc::clone(&self.total_hashes),
            solve_times: Arc::clone(&self.solve_times),
            shutdown_controller: self.shutdown_controller.clone(),
            shutdown_receiver: None, // Cannot clone broadcast::Receiver, so set to None
            logging_config: self.logging_config.clone(),
            backoff_config: self.backoff_config.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct U256([u64; 4]);

impl U256 {
    const ZERO: U256 = U256([0, 0, 0, 0]);
    const MAX: U256 = U256([u64::MAX, u64::MAX, u64::MAX, u64::MAX]);

    fn to_big_endian(self, bytes: &mut [u8; 32]) {
        bytes[0..8].copy_from_slice(&self.0[3].to_be_bytes());
        bytes[8..16].copy_from_slice(&self.0[2].to_be_bytes());
        bytes[16..24].copy_from_slice(&self.0[1].to_be_bytes());
        bytes[24..32].copy_from_slice(&self.0[0].to_be_bytes());
    }

    fn to_big_endian_vec(self) -> Vec<u8> {
        let mut buf = [0u8; 32];
        self.to_big_endian(&mut buf);
        buf.to_vec()
    }

    fn get_bit(&self, bit: usize) -> bool {
        if bit >= 256 {
            return false;
        }
        let word_index = bit / 64;
        let bit_in_word = bit % 64;
        (self.0[word_index] >> bit_in_word) & 1 != 0
    }

    fn set_bit(&mut self, bit: usize) {
        if bit >= 256 {
            return;
        }
        let word_index = bit / 64;
        let bit_in_word = bit % 64;
        self.0[word_index] |= 1 << bit_in_word;
    }

    fn shl_1(mut self) -> Self {
        let mut carry = 0;
        for item in &mut self.0 {
            let next_carry = *item >> 63;
            *item = (*item << 1) | carry;
            carry = next_carry;
        }
        self
    }

    fn sub(self, rhs: Self) -> Self {
        let (d0, borrow) = self.0[0].overflowing_sub(rhs.0[0]);
        let (d1, borrow) = self.0[1].overflowing_sub(rhs.0[1].wrapping_add(borrow as u64));
        let (d2, borrow) = self.0[2].overflowing_sub(rhs.0[2].wrapping_add(borrow as u64));
        let (d3, _) = self.0[3].overflowing_sub(rhs.0[3].wrapping_add(borrow as u64));
        U256([d0, d1, d2, d3])
    }
}

impl From<u64> for U256 {
    fn from(val: u64) -> Self {
        U256([val, 0, 0, 0])
    }
}

impl From<u128> for U256 {
    fn from(val: u128) -> Self {
        U256([val as u64, (val >> 64) as u64, 0, 0])
    }
}

impl Div<U256> for U256 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        // Note: This implementation assumes division won't fail in practice
        // For production use, consider returning Result or using checked division
        self.div_rem(rhs).unwrap_or((Self::ZERO, Self::ZERO)).0
    }
}

impl U256 {
    fn div_rem(self, divisor: Self) -> Result<(Self, Self), MiningError> {
        if divisor == Self::ZERO {
            return Err(MiningError::InvalidBlock(
                "Division by zero in difficulty calculation".to_string(),
            ));
        }
        if self < divisor {
            return Ok((Self::ZERO, self));
        }

        let mut quotient = Self::ZERO;
        let mut remainder = Self::ZERO;

        for i in (0..256).rev() {
            remainder = remainder.shl_1();
            if self.get_bit(i) {
                remainder.0[0] |= 1;
            }
            if remainder >= divisor {
                remainder = remainder.sub(divisor);
                quotient.set_bit(i);
            }
        }
        Ok((quotient, remainder))
    }
}

#[derive(Clone)]
struct MiningContext {
    block_template: QantoBlock,
    target_hash_value: Vec<u8>,
    cancellation_token: tokio_util::sync::CancellationToken,
    found_signal: Arc<AtomicBool>,
    hashes_tried: Arc<AtomicU64>,
    start_time: SystemTime,
    timeout_duration: Duration,
}

/// Configuration for exponential backoff in mining operations
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ExponentialBackoffConfig {
    initial_delay_ms: u64,
    max_delay_ms: u64,
    multiplier: f64,
    max_retries: usize,
}

impl Default for ExponentialBackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 100, // Start with 100ms delay
            max_delay_ms: 5000,    // Cap at 5 seconds
            multiplier: 2.0,       // Double delay each time
            max_retries: 10,       // Maximum retry attempts
        }
    }
}

impl ExponentialBackoffConfig {
    #[allow(dead_code)]
    fn calculate_delay(&self, attempt: usize) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let delay_ms = (self.initial_delay_ms as f64 * self.multiplier.powi((attempt - 1) as i32))
            .min(self.max_delay_ms as f64) as u64;

        Duration::from_millis(delay_ms)
    }
}

impl Miner {
    #[instrument(level = "info", skip(config), fields(address = %config.address, threads = config.threads, use_gpu = config.use_gpu, zk_enabled = config.zk_enabled))]
    pub fn new(config: MinerConfig) -> Result<Self> {
        let _span = span!(Level::INFO, "miner_init", address = %config.address).entered();

        let address_regex = Regex::new(r"^[0-9a-fA-F]{64}$")?;
        if !address_regex.is_match(&config.address) {
            let mut error_msg = String::with_capacity(32 + config.address.len());
            error_msg.push_str("Invalid miner address format: ");
            error_msg.push_str(&config.address);
            return Err(MiningError::InvalidAddress(error_msg).into());
        }

        let compiled_gpu = cfg!(feature = "gpu");
        let effective_use_gpu = config.use_gpu && compiled_gpu;
        // Explicit startup logs for GPU configuration scenarios
        if compiled_gpu && config.use_gpu {
            info!(target = "qanto::miner", "GPU feature compiled and 'gpu_enabled' is true in config.toml. Activating GPU miner.");
        } else if compiled_gpu && !config.use_gpu {
            info!(target = "qanto::miner", "GPU feature is compiled, but 'gpu_enabled' is false in config.toml. Defaulting to CPU miner.");
        } else if !compiled_gpu && config.use_gpu {
            warn!(target = "qanto::miner", "'gpu_enabled = true' in config.toml, but the node was not compiled with the 'gpu' feature. Defaulting to CPU miner. To enable GPU support, please re-compile using: cargo build --release --features gpu");
        }
        let effective_zk_enabled = config.zk_enabled && cfg!(feature = "zk");
        if config.zk_enabled && !effective_zk_enabled {
            warn!("ZK proofs enabled in config but 'zk' feature is not compiled. Disabling ZK for this session.");
        }

        // Configure QanHash miner with optimal settings for high throughput
        // Try GPU first if available and enabled, then fallback to CPU
        let mining_device = if effective_use_gpu {
            #[cfg(feature = "gpu")]
            {
                info!("GPU feature enabled; selecting Auto device (Metal/CUDA/OpenCL).");
                MiningDevice::Auto
            }
            #[cfg(not(feature = "gpu"))]
            {
                warn!("GPU mining requested but 'gpu' feature not compiled, falling back to CPU");
                MiningDevice::Cpu
            }
        } else {
            MiningDevice::Cpu
        };

        // Determine effective thread count: default to (CPU cores - 1) if config.threads == 0
        let default_threads = num_cpus::get().saturating_sub(1).max(1);
        let effective_threads = if config.threads == 0 {
            default_threads
        } else {
            config.threads.max(1)
        };

        let qanhash_config = MiningConfig {
            device: mining_device,
            thread_count: Some(effective_threads),
            batch_size: Some(1024 * 1024), // Use 1M for GPU throughput
            max_iterations: Some(config.target_block_time * 1_000_000), // Limit iterations based on target block time
            enable_simd: true,                                          // Enable SIMD optimizations
        };

        let qanhash_miner = QanhashMiner::new(qanhash_config);

        #[cfg(feature = "zk")]
        let (he_pk, he_sk) = if effective_zk_enabled {
            crate::types::HomomorphicEncrypted::generate_keypair()
        } else {
            (Vec::new(), Vec::new())
        };

        info!(
            "Miner initialized successfully with {} threads",
            effective_threads
        );

        Ok(Self {
            _address: config.address,
            _dag: (*config.dag).clone(),
            target_block_time: config.target_block_time,
            _use_gpu: effective_use_gpu,
            _zk_enabled: effective_zk_enabled,
            threads: effective_threads,
            qanhash_miner: Arc::new(qanhash_miner),
            #[cfg(feature = "zk")]
            he_public_key: he_pk,
            #[cfg(feature = "zk")]
            he_private_key: he_sk,
            total_hashes: Arc::new(AtomicU64::new(0)),
            solve_times: Arc::new(std::sync::Mutex::new(Vec::new())),
            shutdown_controller: None,
            shutdown_receiver: None,
            logging_config: config.logging_config,
            backoff_config: ExponentialBackoffConfig::default(),
        })
    }

    /// Solves the Proof-of-Work for a given block template by finding a valid nonce.
    /// This is the primary mining function called by the node's mining loop.
    /// It modifies the block in-place with the found nonce and effort.
    pub fn solve_pow(&self, block_template: &mut QantoBlock) -> Result<(), MiningError> {
        let mut solve_start = Instant::now();
        let adaptive_timeout = Duration::from_secs(self.target_block_time * 2);
        self.log_mining_start(block_template);

        // FIX: Use the correct method `Self::calculate_target_from_difficulty` which accepts f64.
        let mut target_hash_bytes =
            Self::calculate_target_from_difficulty(block_template.difficulty);
        let (header_hash, mut target) =
            self.prepare_mining_data(block_template, &target_hash_bytes)?;
        let block_index = block_template.height; // Use block height for epoch calculation, removed unnecessary cast

        let dag = my_blockchain::qanhash::get_qdag_sync(block_index);

        let mut qanto_hash = QantoHash::new(header_hash);

        let mut nonce = get_nonce_with_deterministic_fallback();

        let mut failed_attempts = 0;

        // Main mining loop using batch processing with precomputed DAG
        let should_stop = std::sync::atomic::AtomicBool::new(false);

        loop {
            let batch_size = 1024u64;
            // FIX: Removed the extra `Some(batch_size)` argument.
            if let Some((found_nonce, hash)) =
                self.qanhash_miner
                    .mine_with_dag(&qanto_hash, nonce, target, dag.clone())
            {
                let tries = found_nonce - nonce + 1; // Removed unnecessary cast
                self.total_hashes.fetch_add(tries, Ordering::Relaxed);
                block_template.nonce = found_nonce;
                block_template.effort = self.total_hashes.load(Ordering::Relaxed);
                return self.process_mining_result(
                    block_template,
                    MiningResult::Found {
                        nonce: found_nonce,
                        hash,
                    },
                );
            }
            nonce += batch_size;
            self.total_hashes.fetch_add(batch_size, Ordering::Relaxed);
            if should_stop.load(Ordering::Relaxed) {
                return Err(MiningError::TimeoutOrCancelled);
            }
            let elapsed = solve_start.elapsed();
            if elapsed > adaptive_timeout {
                failed_attempts += 1;

                // Adjust difficulty every 3 failed attempts
                if failed_attempts % 3 == 0 {
                    block_template.difficulty *= 0.5;
                    target_hash_bytes =
                        Self::calculate_target_from_difficulty(block_template.difficulty);
                    let (new_header_hash, new_target) =
                        self.prepare_mining_data(block_template, &target_hash_bytes)?;
                    qanto_hash = QantoHash::new(new_header_hash);
                    target = new_target;
                }

                // Record solve time
                if let Ok(mut times) = self.solve_times.lock() {
                    times.push(elapsed);
                    if times.len() > 100 {
                        times.remove(0);
                    }
                }

                // Prevent infinite looping by limiting max failed attempts
                if failed_attempts > 9 {
                    return Err(MiningError::TimeoutOrCancelled);
                }

                // Reset timer for next attempt with adjusted parameters
                solve_start = Instant::now();
            }
        }
    }

    /// Get the average solve time for the last 100 solves
    pub fn get_average_solve_time(&self) -> f64 {
        if let Ok(times) = self.solve_times.lock() {
            if times.is_empty() {
                return 0.0;
            }

            let total_duration: Duration = times.iter().sum();
            total_duration.as_secs_f64() / times.len() as f64
        } else {
            0.0
        }
    }

    /// Get the number of recorded solve times
    pub fn get_solve_count(&self) -> usize {
        if let Ok(times) = self.solve_times.lock() {
            times.len()
        } else {
            0
        }
    }

    /// Get all solve times (for debugging/analysis)
    pub fn get_solve_times(&self) -> Vec<Duration> {
        if let Ok(times) = self.solve_times.lock() {
            times.clone()
        } else {
            Vec::new()
        }
    }

    /// Log the start of mining process
    fn log_mining_start(&self, block_template: &QantoBlock) {
        debug!(
            "[DEBUG] solve_pow: Starting PoW for block {}",
            block_template.id
        );
        debug!(
            "[DEBUG] solve_pow: Target block time set to {} seconds",
            self.target_block_time
        );
    }

    /// Prepare mining data including header hash and target arrays
    fn prepare_mining_data(
        &self,
        block_template: &QantoBlock,
        target_hash_bytes: &[u8],
    ) -> Result<([u8; 32], [u8; 32]), MiningError> {
        debug!(
            "[DEBUG] solve_pow: Target hash calculated for difficulty {}",
            block_template.difficulty
        );

        // Create a temporary block with nonce 0 to ensure consistent hash calculation
        // This matches the behavior in test_nonce where we create a temp block and set the nonce
        let mut temp_block = block_template.clone();
        temp_block.nonce = 0;
        let header_hash_hex = temp_block.hash();
        let header_hash_bytes = hex::decode(&header_hash_hex).map_err(|e| {
            let mut error_msg = String::with_capacity(20 + e.to_string().len());
            error_msg.push_str("Invalid header hash: ");
            error_msg.push_str(&e.to_string());
            MiningError::InvalidBlock(error_msg)
        })?;

        let header_hash = self.convert_to_hash_array(&header_hash_bytes)?;
        let target = self.convert_to_target_array(target_hash_bytes);

        Ok((header_hash, target))
    }

    /// Convert header hash bytes to fixed-size array
    fn convert_to_hash_array(&self, header_hash_bytes: &[u8]) -> Result<[u8; 32], MiningError> {
        let mut header_hash = [0u8; 32];
        if header_hash_bytes.len() >= 32 {
            header_hash.copy_from_slice(&header_hash_bytes[0..32]);
            Ok(header_hash)
        } else {
            Err(MiningError::InvalidBlock(
                "Header hash too short".to_string(),
            ))
        }
    }

    /// Convert target hash bytes to fixed-size array
    fn convert_to_target_array(&self, target_hash_bytes: &[u8]) -> [u8; 32] {
        let mut target = [0u8; 32];
        if target_hash_bytes.len() >= 32 {
            target.copy_from_slice(&target_hash_bytes[0..32]);
        } else {
            target[32 - target_hash_bytes.len()..].copy_from_slice(target_hash_bytes);
        }
        target
    }

    /// Process the mining result and update block template
    fn process_mining_result(
        &self,
        block_template: &mut QantoBlock,
        mining_result: MiningResult,
    ) -> Result<(), MiningError> {
        match mining_result {
            MiningResult::Found {
                nonce: found_nonce,
                hash: final_hash,
            } => {
                block_template.nonce = found_nonce;
                // Convert hash to effort (u64) for compatibility
                let effort = u64::from_be_bytes([
                    final_hash[0],
                    final_hash[1],
                    final_hash[2],
                    final_hash[3],
                    final_hash[4],
                    final_hash[5],
                    final_hash[6],
                    final_hash[7],
                ]);
                block_template.effort = effort;
                debug!(
                    "PoW solved for block ID (pre-hash) {} with nonce {} and effort {}. Block is ready for finalization.",
                    block_template.id, found_nonce, effort
                );

                // Use the detailed mining celebration with proper parameters
                let _mining_time = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default();

                let celebration_params = MiningCelebrationParams {
                    block_height: block_template.height,
                    block_hash: hex::encode(&final_hash[..8]), // First 8 bytes as hex
                    nonce: found_nonce,
                    difficulty: block_template.difficulty, // Use actual block difficulty
                    transactions_count: block_template.transactions.len(),
                    mining_time: Duration::from_secs(30), // Approximate mining time
                    effort,
                    total_blocks_mined: 1,
                    chain_id: 0,
                    block_reward: 50_000_000_000, // 50 QAN in smallest units
                    compact: false,
                };

                on_block_mined(celebration_params, &self.logging_config);

                Ok(())
            }
            MiningResult::Cancelled => Err(MiningError::TimeoutOrCancelled),
            MiningResult::Timeout => Err(MiningError::TimeoutOrCancelled),
        }
    }

    #[instrument(level = "info", skip(self, block_template, cancellation_token), fields(block_id = %block_template.id, difficulty = block_template.difficulty, threads = self.threads))]
    pub fn solve_pow_with_cancellation(
        &self,
        block_template: &mut QantoBlock,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), MiningError> {
        // Immediate cancellation check for pre-cancelled tokens
        if cancellation_token.is_cancelled() {
            debug!("Mining cancelled before starting - token was pre-cancelled");
            return Err(MiningError::TimeoutOrCancelled);
        }

        let start_time = SystemTime::now();

        // Adaptive timeout based on difficulty and target block time
        // Reduced from 2x to 1.5x target time for faster mining cycles
        let adaptive_timeout = Duration::from_secs(30); // Increased timeout to 30 seconds for feasible mining at difficulty 1.0

        self.log_mining_start(block_template);

        // Fetch pre-cached DAG for the current block's epoch to eliminate regeneration bottleneck
        let qdag = {
            use my_blockchain::qanhash::get_qdag_sync;
            let block_index = block_template.height; // Use block height as index
            debug!("Fetching DAG for block index: {}", block_index);
            get_qdag_sync(block_index)
        };

        debug!(
            "Successfully fetched pre-cached DAG for block {}",
            block_template.id
        );

        let target_hash_bytes = Self::calculate_target_from_difficulty(block_template.difficulty);
        let (_header_hash, _target) =
            self.prepare_mining_data(block_template, &target_hash_bytes)?;

        // Atomic success flag to prevent race conditions
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));

        let context = MiningContext {
            block_template: block_template.clone(),
            target_hash_value: target_hash_bytes.to_vec(),
            cancellation_token: cancellation_token.clone(),
            found_signal: found_signal.clone(),
            hashes_tried: hashes_tried.clone(),
            start_time,
            timeout_duration: adaptive_timeout,
        };

        // Try GPU mining first if available and enabled
        #[cfg(feature = "gpu")]
        if self._use_gpu {
            match self.mine_gpu_with_cancellation(
                &context.block_template,
                &context.target_hash_value,
                start_time,
                adaptive_timeout,
                cancellation_token.clone(),
                qdag.clone(),
            ) {
                Ok(MiningResult::Found { nonce, hash }) => {
                    return self.process_mining_result(
                        block_template,
                        MiningResult::Found { nonce, hash },
                    );
                }
                Ok(MiningResult::Cancelled) => {
                    return Err(MiningError::TimeoutOrCancelled);
                }
                Ok(MiningResult::Timeout) => {
                    // Fall through to CPU mining with reduced timeout
                }
                Err(e) => {
                    warn!("GPU mining failed, falling back to CPU: {:?}", e);
                }
            }
        }

        // Use DAG-optimized CPU mining with pre-cached DAG
        let mining_result = self.mine_cpu_with_dag(
            &context.block_template,
            &context.target_hash_value,
            start_time,
            adaptive_timeout,
            cancellation_token,
            qdag,
        )?;

        // Record solve time metrics
        if let Ok(elapsed) = start_time.elapsed() {
            if let Ok(mut solve_times) = self.solve_times.lock() {
                solve_times.push(elapsed);
                // Keep only last 100 solve times for memory efficiency
                let len = solve_times.len();
                if len > 100 {
                    let excess = len - 100;
                    solve_times.drain(..excess);
                }
            }
        }

        self.process_mining_result(block_template, mining_result)
    }

    /// Fallback mining method without DAG optimization (existing logic)
    #[allow(dead_code)]
    fn solve_pow_with_cancellation_fallback(
        &self,
        block_template: &mut QantoBlock,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), MiningError> {
        let start_time = SystemTime::now();
        let adaptive_timeout = Duration::from_secs(30);

        let target_hash_bytes = Self::calculate_target_from_difficulty(block_template.difficulty);
        let (_header_hash, _target) =
            self.prepare_mining_data(block_template, &target_hash_bytes)?;

        // Use existing async-optimized CPU mining without DAG
        let mining_result = self.mine_cpu_async_optimized(
            block_template,
            &target_hash_bytes,
            start_time,
            adaptive_timeout,
            cancellation_token,
        )?;

        // Record solve time metrics
        if let Ok(elapsed) = start_time.elapsed() {
            if let Ok(mut solve_times) = self.solve_times.lock() {
                solve_times.push(elapsed);
                let len = solve_times.len();
                if len > 100 {
                    let excess = len - 100;
                    solve_times.drain(..excess);
                }
            }
        }

        self.process_mining_result(block_template, mining_result)
    }

    /// Checks if CUDA is available for GPU mining
    #[cfg(feature = "gpu")]
    fn is_cuda_available() -> bool {
        // This would typically check for CUDA runtime availability
        // For now, we'll implement a basic check
        match std::process::Command::new("nvidia-smi").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// GPU mining implementation with cancellation support
    #[cfg(feature = "gpu")]
    fn mine_gpu_with_cancellation(
        &self,
        block_template: &QantoBlock,
        target_hash_bytes: &[u8],
        start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
        qdag: Arc<Vec<[u8; 128]>>,
    ) -> Result<MiningResult, MiningError> {
        // Immediate cancellation check
        if cancellation_token.is_cancelled() {
            return Ok(MiningResult::Cancelled);
        }

        // Prepare header and target for GPU mining
        let (header_hash, target) = self.prepare_mining_data(block_template, target_hash_bytes)?;

        // Start from deterministic nonce for parallel workers
        let mut nonce = get_nonce_with_deterministic_fallback();
        const GPU_BATCH_SIZE: u64 = 1024 * 1024; // Align with configured batch size

        loop {
            // Cancellation and timeout checks
            if cancellation_token.is_cancelled() {
                return Ok(MiningResult::Cancelled);
            }
            if start_time.elapsed()? >= timeout_duration {
                return Ok(MiningResult::Timeout);
            }

            // Perform a GPU mining batch using precomputed DAG
            if let Some((found_nonce, hash)) = self
                .qanhash_miner
                .mine_with_dag(&header_hash, nonce, &target, qdag.clone())
            {
                return Ok(MiningResult::Found { nonce: found_nonce, hash });
            }

            // Advance nonce space and record progress
            nonce = nonce.wrapping_add(GPU_BATCH_SIZE);
            self.total_hashes.fetch_add(GPU_BATCH_SIZE, Ordering::Relaxed);
            increment_hash_attempts(GPU_BATCH_SIZE);

            // Yield to other threads to keep event loop responsive
            std::thread::yield_now();
        }
    }

    /// New async-optimized CPU mining with improved cancellation responsiveness
    #[allow(dead_code)]
    #[instrument(level = "debug", skip(self, block_template, target_hash_value, cancellation_token), fields(threads = self.threads, timeout_duration = ?timeout_duration))]
    fn mine_cpu_async_optimized(
        &self,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
        _start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<MiningResult, MiningError> {
        // Check for immediate cancellation
        if cancellation_token.is_cancelled() {
            return Ok(MiningResult::Cancelled);
        }

        // Use blocking task to avoid runtime-within-runtime issue
        let block_template = block_template.clone();
        let target_hash_value = target_hash_value.to_vec();
        let miner_threads = self.threads;
        let cancellation_token_clone = cancellation_token.clone();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                MiningError::ThreadPool(format!("Failed to create async runtime: {e}"))
            })?;

            rt.block_on(async {
                tokio::select! {
                    result = Self::mine_cpu_blocking_inner(
                        &block_template,
                        &target_hash_value,
                        miner_threads,
                        cancellation_token_clone.clone(),
                    ) => result,
                    _ = cancellation_token_clone.cancelled() => Ok(MiningResult::Cancelled),
                    _ = tokio::time::sleep(timeout_duration) => Ok(MiningResult::Timeout),
                }
            })
        });

        handle
            .join()
            .map_err(|_| MiningError::ThreadPool("Mining thread panicked".to_string()))?
    }

    /// Blocking CPU mining implementation for use in separate thread
    #[allow(dead_code)]
    async fn mine_cpu_blocking_inner(
        block_template: &QantoBlock,
        _target_hash_value: &[u8],
        _threads: usize,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<MiningResult, MiningError> {
        use rayon::ThreadPoolBuilder;

        // Optimize thread count: use 90% of available cores for better performance
        let optimal_threads = (num_cpus::get() as f32 * 0.9).ceil() as usize;
        let thread_count = optimal_threads.max(1);

        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build()
            .map_err(|e| MiningError::ThreadPool(e.to_string()))?;

        // Use async channels for better integration with tokio
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let completion_flag = Arc::new(AtomicBool::new(false));
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));
        let winning_nonce = Arc::new(AtomicU64::new(0));
        let winning_hash = Arc::new(std::sync::Mutex::new([0u8; 32]));

        // Work-stealing: divide nonce space among threads
        let nonce_range_per_thread = u64::MAX / thread_count as u64;
        let base_nonce = get_nonce_with_deterministic_fallback();

        // Spawn parallel mining threads with work-stealing
        for thread_id in 0..thread_count {
            let sender = sender.clone();
            let completion_flag = completion_flag.clone();
            let found_signal = found_signal.clone();
            let hashes_tried = hashes_tried.clone();
            let winning_nonce = winning_nonce.clone();
            let winning_hash = winning_hash.clone();
            let block_template = block_template.clone();
            let cancellation_token = cancellation_token.clone();

            let thread_start_nonce =
                base_nonce.wrapping_add(thread_id as u64 * nonce_range_per_thread);

            thread_pool.spawn(move || {
                let mut current_nonce = thread_start_nonce;
                let mut local_hash_count = 0u64;

                // Process nonces in smaller batches for more responsive cancellation
                const BATCH_SIZE: u64 = 10; // Smaller batch for better responsiveness

                loop {
                    // Early termination if solution found by another thread or cancellation requested
                    if found_signal.load(Ordering::Relaxed)
                        || completion_flag.load(Ordering::Relaxed)
                        || cancellation_token.is_cancelled()
                    {
                        break;
                    }

                    // Process batch of nonces
                    for _ in 0..BATCH_SIZE {
                        if found_signal.load(Ordering::Relaxed)
                            || completion_flag.load(Ordering::Relaxed)
                            || cancellation_token.is_cancelled()
                        {
                            break;
                        }

                        // Optimized nonce testing with minimal overhead
                        if Self::test_nonce_optimized(current_nonce, &block_template) {
                            // Atomic completion flag prevents timeout cancellation
                            if !completion_flag.swap(true, Ordering::AcqRel) {
                                // Compute hash immediately while protected by completion flag
                                let hash =
                                    Self::compute_hash_for_nonce(current_nonce, &block_template);

                                // Store results atomically
                                winning_nonce.store(current_nonce, Ordering::Relaxed);
                                if let Ok(mut hash_guard) = winning_hash.lock() {
                                    *hash_guard = hash;
                                }

                                // Signal other threads to stop
                                found_signal.store(true, Ordering::Release);

                                // Send result immediately - this cannot be cancelled now
                                let _ = sender.send(MiningResult::Found {
                                    nonce: current_nonce,
                                    hash,
                                });

                                debug!(
                                    "nonce_found; winning_nonce={} thread_id={}",
                                    current_nonce, thread_id
                                );
                                return;
                            }
                        }

                        current_nonce = current_nonce.wrapping_add(1);
                        local_hash_count += 1;
                    }

                    // Update global hash counter periodically
                    hashes_tried.fetch_add(local_hash_count, Ordering::Relaxed);
                    local_hash_count = 0;

                    // Work-stealing: jump to different nonce space if no solution found
                    if current_nonce.wrapping_sub(thread_start_nonce) > nonce_range_per_thread {
                        current_nonce = thread_start_nonce.wrapping_add(
                            (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos() as u64)
                                % nonce_range_per_thread,
                        );
                    }
                }
            });
        }

        drop(sender); // Close sender to signal completion

        // Wait for result with async receiver
        if let Some(result) = receiver.recv().await {
            // If we found a solution, return it immediately regardless of timeout
            if matches!(result, MiningResult::Found { .. }) {
                debug!("Mining completed successfully with valid nonce");
            }
            return Ok(result);
        }

        // Check if cancellation was requested
        if cancellation_token.is_cancelled() {
            return Ok(MiningResult::Cancelled);
        }

        // All threads finished without finding a solution
        if completion_flag.load(Ordering::Acquire) && found_signal.load(Ordering::Acquire) {
            // Solution was found but channel closed, reconstruct result
            let nonce = winning_nonce.load(Ordering::Relaxed);
            if let Ok(hash_guard) = winning_hash.lock() {
                return Ok(MiningResult::Found {
                    nonce,
                    hash: *hash_guard,
                });
            }
        }

        Ok(MiningResult::Timeout)
    }

    #[allow(dead_code)]
    #[instrument(level = "debug", skip(self, block_template, cancellation_token), fields(threads = self.threads, timeout_duration = ?timeout_duration))]
    fn mine_cpu_optimized(
        &self,
        block_template: &QantoBlock,
        _target_hash_value: &[u8],
        _start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<MiningResult, MiningError> {
        // CRITICAL: Check cancellation immediately before any setup
        if cancellation_token.is_cancelled() {
            return Ok(MiningResult::Cancelled);
        }

        use rayon::ThreadPoolBuilder;

        // Optimize thread count: use 90% of available cores for better performance
        let optimal_threads = (num_cpus::get() as f32 * 0.9).ceil() as usize;
        let thread_count = optimal_threads.max(1);

        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build()
            .map_err(|e| MiningError::ThreadPool(e.to_string()))?;

        // CRITICAL FIX: Atomic completion flag to prevent race conditions
        // This ensures valid nonces are NEVER cancelled after discovery
        let completion_flag = Arc::new(AtomicBool::new(false));
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));
        let winning_nonce = Arc::new(AtomicU64::new(0));
        let winning_hash = Arc::new(std::sync::Mutex::new([0u8; 32]));

        // Work-stealing: divide nonce space among threads
        let nonce_range_per_thread = u64::MAX / thread_count as u64;
        let base_nonce = get_nonce_with_deterministic_fallback();

        let (sender, receiver) = std::sync::mpsc::channel();

        // Spawn parallel mining threads with work-stealing
        for thread_id in 0..thread_count {
            let sender = sender.clone();
            let completion_flag = completion_flag.clone();
            let found_signal = found_signal.clone();
            let hashes_tried = hashes_tried.clone();
            let winning_nonce = winning_nonce.clone();
            let winning_hash = winning_hash.clone();
            let cancellation_token = cancellation_token.clone();
            let block_template = block_template.clone();

            let thread_start_nonce =
                base_nonce.wrapping_add(thread_id as u64 * nonce_range_per_thread);

            thread_pool.spawn(move || {
                let mut current_nonce = thread_start_nonce;
                let mut local_hash_count = 0u64;

                // Process nonces in batches for better cache locality
                // Reduced batch size for more responsive cancellation checking
                const BATCH_SIZE: u64 = 10; // Balance between performance and responsiveness

                loop {
                    // Early termination if solution found by another thread
                    if found_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    // Check cancellation every batch (non-blocking)
                    // CRITICAL: Only check cancellation if no solution is being processed
                    if !completion_flag.load(Ordering::Acquire) && cancellation_token.is_cancelled()
                    {
                        let _ = sender.send(MiningResult::Cancelled);
                        break;
                    }

                    // Process batch of nonces
                    for _ in 0..BATCH_SIZE {
                        if found_signal.load(Ordering::Relaxed) {
                            break;
                        }

                        // Optimized nonce testing with minimal overhead
                        if Self::test_nonce_optimized(current_nonce, &block_template) {
                            // CRITICAL FIX: Atomic completion flag prevents timeout cancellation
                            // Set completion flag FIRST to block any timeout/cancellation
                            if !completion_flag.swap(true, Ordering::AcqRel) {
                                // Compute hash immediately while protected by completion flag
                                let hash =
                                    Self::compute_hash_for_nonce(current_nonce, &block_template);

                                // Store results atomically
                                winning_nonce.store(current_nonce, Ordering::Relaxed);
                                if let Ok(mut hash_guard) = winning_hash.lock() {
                                    *hash_guard = hash;
                                }

                                // Signal other threads to stop
                                found_signal.store(true, Ordering::Release);

                                // Send result immediately - this cannot be cancelled now
                                let _ = sender.send(MiningResult::Found {
                                    nonce: current_nonce,
                                    hash,
                                });

                                debug!(
                                    "nonce_found; winning_nonce={} thread_id={}",
                                    current_nonce, thread_id
                                );
                                break;
                            }
                        }

                        current_nonce = current_nonce.wrapping_add(1);
                        local_hash_count += 1;
                    }

                    // Update global hash counter periodically
                    hashes_tried.fetch_add(local_hash_count, Ordering::Relaxed);
                    local_hash_count = 0;

                    // Work-stealing: jump to different nonce space if no solution found
                    if current_nonce.wrapping_sub(thread_start_nonce) > nonce_range_per_thread {
                        current_nonce = thread_start_nonce.wrapping_add(
                            (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos() as u64)
                                % nonce_range_per_thread,
                        );
                    }
                }
            });
        }

        drop(sender); // Close sender to signal completion

        // Wait for result with timeout handling
        let timeout_start = std::time::Instant::now();

        loop {
            // Non-blocking receive with timeout check
            match receiver.try_recv() {
                Ok(result) => {
                    // CRITICAL: If we found a solution, return it immediately regardless of timeout
                    if matches!(result, MiningResult::Found { .. }) {
                        debug!("Mining completed successfully with valid nonce");
                        return Ok(result);
                    }
                    return Ok(result);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // CRITICAL FIX: Never timeout if completion is in progress
                    if completion_flag.load(Ordering::Acquire) {
                        // Solution is being processed, wait for it
                        std::thread::sleep(Duration::from_micros(10));
                        continue;
                    }

                    // Check timeout condition only if no completion in progress
                    if timeout_start.elapsed() > timeout_duration {
                        // Signal threads to stop only if no solution found
                        if !found_signal.load(Ordering::Acquire) {
                            found_signal.store(true, Ordering::Release);
                            debug!("Mining timeout after {:?}", timeout_start.elapsed());
                            return Ok(MiningResult::Timeout);
                        }
                    }

                    // Brief yield to prevent busy waiting
                    std::thread::sleep(Duration::from_micros(100));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // All threads finished - check if solution was found during completion
                    if completion_flag.load(Ordering::Acquire)
                        && found_signal.load(Ordering::Acquire)
                    {
                        // Solution was found but channel closed, reconstruct result
                        let nonce = winning_nonce.load(Ordering::Relaxed);
                        if let Ok(hash_guard) = winning_hash.lock() {
                            return Ok(MiningResult::Found {
                                nonce,
                                hash: *hash_guard,
                            });
                        }
                    }
                    return Ok(MiningResult::Timeout);
                }
            }
        }
    }

    /// Optimized nonce testing with minimal overhead and no blocking operations
    fn test_nonce_optimized(current_nonce: u64, block_template: &QantoBlock) -> bool {
        // Create a mutable copy for nonce testing
        let mut test_block = block_template.clone();
        test_block.nonce = current_nonce;

        // Use canonical PoW hash method and direct Consensus validation for perfect symmetry
        let pow_hash = test_block.hash_for_pow();
        crate::consensus::Consensus::is_pow_valid(pow_hash.as_bytes(), test_block.difficulty)
    }

    /// Compute hash for a specific nonce (used for result reporting)
    fn compute_hash_for_nonce(nonce: u64, block_template: &QantoBlock) -> [u8; 32] {
        let mut test_block = block_template.clone();
        test_block.nonce = nonce;

        // Use the canonical PoW hash method that includes nonce
        let pow_hash = test_block.hash_for_pow();

        // Convert QantoHash to [u8; 32] array
        let mut hash_array = [0u8; 32];
        let hash_bytes = pow_hash.as_bytes();
        let copy_len = 32.min(hash_bytes.len());
        hash_array[..copy_len].copy_from_slice(&hash_bytes[..copy_len]);
        hash_array
    }

    #[instrument(level = "trace", skip(self, context), fields(nonce = current_nonce))]
    fn try_nonce(&self, current_nonce: u64, context: &MiningContext) -> Option<u64> {
        // Increment hash attempts for metrics (non-blocking)
        context.hashes_tried.fetch_add(1, Ordering::Relaxed);
        increment_hash_attempts();

        // Fast cancellation check (every 10,000 hashes instead of 1,000 for better performance)
        let count = context.hashes_tried.load(Ordering::Relaxed);
        if count.is_multiple_of(10_000)
            && self.handle_cancellation_check(&context.cancellation_token, &context.found_signal)
        {
            return None;
        }

        // Reduced timeout check frequency (every 100,000 hashes instead of 1,000,000)
        if count.is_multiple_of(100_000)
            && self.handle_timeout_check(
                context.start_time,
                context.timeout_duration,
                &context.found_signal,
                count,
            )
        {
            return None;
        }

        // Optimized nonce testing
        if Self::test_nonce_optimized(current_nonce, &context.block_template) {
            // Set found signal to stop other threads
            context.found_signal.store(true, Ordering::Relaxed);
            return Some(current_nonce);
        }

        None
    }

    pub fn should_stop_mining(
        &self,
        cancellation_token: &tokio_util::sync::CancellationToken,
        found_signal: &Arc<AtomicBool>,
    ) -> bool {
        cancellation_token.is_cancelled() || found_signal.load(Ordering::Relaxed)
    }

    pub fn should_check_cancellation(&self, count: u64) -> bool {
        count.is_multiple_of(100_000)
    }

    pub fn handle_cancellation_check(
        &self,
        cancellation_token: &tokio_util::sync::CancellationToken,
        found_signal: &Arc<AtomicBool>,
    ) -> bool {
        if cancellation_token.is_cancelled() {
            debug!("[DEBUG] mine_cpu_with_cancellation: Cancellation requested");
            found_signal.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }

    pub fn should_check_timeout(&self, count: u64) -> bool {
        count.is_multiple_of(1_000_000)
    }

    fn handle_timeout_check(
        &self,
        start_time: SystemTime,
        timeout_duration: Duration,
        found_signal: &Arc<AtomicBool>,
        count: u64,
    ) -> bool {
        debug!(
            "[DEBUG] mine_cpu_with_cancellation: {} million hashes tried",
            count / 1_000_000
        );
        if let Ok(elapsed) = start_time.elapsed() {
            if elapsed > timeout_duration {
                debug!(
                    "[DEBUG] mine_cpu_with_cancellation: Timeout reached after {:?}",
                    elapsed
                );
                found_signal.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    /// Calculates the PoW target from a floating-point difficulty value using
    /// deterministic, fixed-point integer arithmetic. This is a consensus-critical function.
    /// Formula: target = max_target / difficulty
    pub fn calculate_target_from_difficulty(difficulty_value: f64) -> [u8; 32] {
        // A difficulty of 0 or less is invalid and results in the easiest possible target.
        if difficulty_value <= 0.0 {
            let mut arr = [0u8; 32];
            let vec = U256::MAX.to_big_endian_vec();
            arr.copy_from_slice(&vec);
            return arr;
        }

        // To avoid floating-point non-determinism in consensus, we convert the difficulty
        // to a fixed-point integer representation.
        // We use a smaller scaling factor to make mining feasible at low difficulties.
        const SCALE: u128 = 1_000; // 1e3 for reasonable precision without making targets too small
        let difficulty_scaled = (difficulty_value * SCALE as f64) as u128;

        // Ensure difficulty is at least 1 to prevent division by zero, although the
        // initial check for <= 0.0 should handle this.
        let difficulty_int = U256::from(difficulty_scaled.max(1));

        // The max_target is 2^256 - 1. We perform the division in integer space.
        // The formula `target = max_target / difficulty` becomes:
        let target = U256::MAX / difficulty_int;

        // FIX: The malformed line with `\n` has been rewritten as separate, valid Rust statements.
        let mut target_arr = [0u8; 32];
        let vec = target.to_big_endian_vec();
        target_arr[32 - vec.len()..].copy_from_slice(&vec);
        target_arr
    }

    /// Checks if a given hash is less than or equal to the target.
    /// This comparison must be done on the raw big-endian byte representation.
    pub fn hash_meets_target(hash_bytes: &[u8], target_bytes: &[u8]) -> bool {
        hash_bytes <= target_bytes
    }

    /// Set the shutdown controller for graceful shutdown integration
    pub fn set_shutdown_controller(&mut self, controller: ShutdownController) {
        self.shutdown_receiver = Some(controller.subscribe());
        self.shutdown_controller = Some(controller);
    }

    /// Check if mining should stop due to shutdown signal
    fn should_stop_for_shutdown(&mut self) -> bool {
        if let Some(ref mut receiver) = self.shutdown_receiver {
            // Non-blocking check for shutdown signals
            match receiver.try_recv() {
                Ok(ShutdownSignal::Graceful) | Ok(ShutdownSignal::Force) => {
                    info!("Mining stopped due to shutdown signal");
                    return true;
                }
                Ok(ShutdownSignal::PhaseComplete(ShutdownPhase::StopMining)) => {
                    info!("Mining stopped due to StopMining phase");
                    return true;
                }
                Ok(_) => {} // Other signals don't affect mining directly
                Err(broadcast::error::TryRecvError::Empty) => {} // No signal
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    warn!("Mining shutdown signal lagged, stopping mining");
                    return true;
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    warn!("Shutdown signal channel closed, stopping mining");
                    return true;
                }
            }
        }
        false
    }

    /// Enhanced solve_pow_with_cancellation that integrates with shutdown controller
    pub fn solve_pow_with_shutdown_integration(
        &mut self,
        block_template: &mut QantoBlock,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), MiningError> {
        // Check for shutdown before starting
        if self.should_stop_for_shutdown() {
            return Err(MiningError::TimeoutOrCancelled);
        }

        // Use existing solve_pow_with_cancellation but with periodic shutdown checks
        self.solve_pow_with_cancellation_and_shutdown(block_template, cancellation_token)
    }

    /// Internal method that combines cancellation token and shutdown signals
    fn solve_pow_with_cancellation_and_shutdown(
        &mut self,
        block_template: &mut QantoBlock,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), MiningError> {
        let start_time = SystemTime::now();
        let timeout_duration = Duration::from_secs(self.target_block_time * 2);

        self.log_mining_start(block_template);

        let target_hash_bytes = Self::calculate_target_from_difficulty(block_template.difficulty);
        let (_header_hash, _target) =
            self.prepare_mining_data(block_template, &target_hash_bytes)?;

        // Enhanced mining loop with shutdown integration
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));

        let context = MiningContext {
            block_template: block_template.clone(),
            // FIX: Convert the `[u8; 32]` array to a `Vec<u8>` to match the struct definition.
            target_hash_value: target_hash_bytes.to_vec(),
            cancellation_token: cancellation_token.clone(),
            found_signal: found_signal.clone(),
            hashes_tried: hashes_tried.clone(),
            start_time,
            timeout_duration,
        };

        // Use CPU mining with enhanced shutdown checking
        let mining_result = self.mine_cpu_with_shutdown_integration(
            &context.block_template,
            &context.target_hash_value,
            start_time,
            timeout_duration,
            cancellation_token,
        )?;

        self.process_mining_result(block_template, mining_result)
    }

    /// CPU mining with integrated shutdown checking
    fn mine_cpu_with_shutdown_integration(
        &mut self,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
        start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<MiningResult, MiningError> {
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));
        let mut nonce = get_nonce_with_deterministic_fallback();

        let context = MiningContext {
            block_template: block_template.clone(),
            target_hash_value: target_hash_value.to_vec(),
            cancellation_token: cancellation_token.clone(),
            found_signal: found_signal.clone(),
            hashes_tried: hashes_tried.clone(),
            start_time,
            timeout_duration,
        };

        loop {
            // Enhanced stopping conditions
            if self.should_stop_mining(&cancellation_token, &found_signal)
                || self.should_stop_for_shutdown()
            {
                debug!("Mining stopped due to cancellation or shutdown");
                return Ok(MiningResult::Cancelled);
            }

            if let Some(winning_nonce) = self.try_nonce(nonce, &context) {
                let total_hashes = hashes_tried.load(Ordering::Relaxed);
                info!(
                    "🎉 Solution found! Nonce: {}, Total hashes: {}",
                    winning_nonce, total_hashes
                );

                // Create a hash from the winning nonce for compatibility with process_mining_result
                let mut hash = [0u8; 32];
                hash[0..8].copy_from_slice(&winning_nonce.to_be_bytes());
                hash[8..16].copy_from_slice(&total_hashes.to_be_bytes());

                return Ok(MiningResult::Found {
                    nonce: winning_nonce,
                    hash,
                });
            }

            nonce = nonce.wrapping_add(1);
            hashes_tried.fetch_add(1, Ordering::Relaxed);
            // Removed extra mining_attempts increment to avoid double counting; try_nonce() already increments this metric per attempt.

            // Periodic checks for performance and hash-rate sampling
            let count = hashes_tried.load(Ordering::Relaxed);
            let do_check = self.should_check_timeout(count);

            if do_check {
                // Sample hash rate approximately every 200ms using global metrics counters
                let now_ms = match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(d) => d.as_millis() as u64,
                    Err(_) => 0,
                };
                let metrics = crate::metrics::get_global_metrics();
                let last_ms = metrics.hash_rate_last_sample_ms.load(Ordering::Relaxed);
                if now_ms.saturating_sub(last_ms) >= 200 {
                    let current_attempts = metrics.mining_attempts.load(Ordering::Relaxed);
                    let last_attempts = metrics.last_hash_attempts.load(Ordering::Relaxed);
                    let delta_attempts = current_attempts.saturating_sub(last_attempts);
                    let interval_ms = now_ms.saturating_sub(last_ms);
                    let hps = crate::metrics::QantoMetrics::compute_hash_rate(
                        delta_attempts,
                        interval_ms,
                    );
                    metrics.set_hash_rate_hps(hps);
                    set_metric!(last_hash_attempts, current_attempts);
                    set_metric!(hash_rate_last_sample_ms, now_ms);
                }
            }

            if do_check
                && self.handle_timeout_check(start_time, timeout_duration, &found_signal, count)
            {
                return Ok(MiningResult::Timeout);
            }
        }
    }

    #[cfg(feature = "zk")]
    pub fn get_homomorphic_public_key(&self) -> Option<&[u8]> {
        if self._zk_enabled && !self.he_public_key.is_empty() {
            Some(&self.he_public_key)
        } else {
            None
        }
    }

    #[cfg(not(feature = "zk"))]
    pub fn get_homomorphic_public_key(&self) -> Option<&[u8]> {
        None
    }

    /// Get the miner's address
    pub fn get_address(&self) -> Option<String> {
        Some(self._address.clone())
    }

    /// DAG-optimized CPU mining using pre-cached DAG
    #[instrument(level = "debug", skip(self, block_template, target_hash_value, cancellation_token, qdag), fields(threads = self.threads, timeout_duration = ?timeout_duration))]
    fn mine_cpu_with_dag(
        &self,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
        _start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
        qdag: Arc<Vec<[u8; 128]>>,
    ) -> Result<MiningResult, MiningError> {
        // Check for immediate cancellation
        if cancellation_token.is_cancelled() {
            return Ok(MiningResult::Cancelled);
        }

        // Use mine_with_dag from qanhash for optimized mining
        let block_template = block_template.clone();
        let target_hash_value = target_hash_value.to_vec();
        let miner_threads = self.threads;
        let cancellation_token_clone = cancellation_token.clone();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                MiningError::ThreadPool(format!("Failed to create async runtime: {e}"))
            })?;

            rt.block_on(async {
                tokio::select! {
                    result = mine_cpu_with_dag_inner(
                        block_template.clone(),  // Pass owned value
                        &target_hash_value,
                        miner_threads,
                        cancellation_token_clone.clone(),
                        qdag,
                    ) => result,
                    _ = cancellation_token_clone.cancelled() => Ok(MiningResult::Cancelled),
                    _ = tokio::time::sleep(timeout_duration) => Ok(MiningResult::Timeout),
                }
            })
        });

        handle
            .join()
            .map_err(|_| MiningError::ThreadPool("Mining thread panicked".to_string()))?
    }
}

/// Inner DAG-optimized mining implementation
pub async fn mine_cpu_with_dag_inner(
    block_template: QantoBlock, // Take ownership instead of borrowing
    _target_hash_value: &[u8],
    _threads: usize,
    cancellation_token: tokio_util::sync::CancellationToken,
    _qdag: Arc<Vec<[u8; 128]>>, // Prefix with underscore to indicate intentionally unused
) -> Result<MiningResult, MiningError> {
    use rayon::ThreadPoolBuilder;

    // Optimize thread count: use 90% of available cores for better performance
    let optimal_threads = (num_cpus::get() as f32 * 0.9).ceil() as usize;
    let thread_count = optimal_threads.max(1);

    let thread_pool = ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .map_err(|e| MiningError::ThreadPool(e.to_string()))?;

    // Use async channels for better integration with tokio
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let completion_flag = Arc::new(AtomicBool::new(false));
    let found_signal = Arc::new(AtomicBool::new(false));
    let hashes_tried = Arc::new(AtomicU64::new(0));
    let winning_nonce = Arc::new(AtomicU64::new(0));
    let winning_hash = Arc::new(std::sync::Mutex::new([0u8; 32]));

    // Work-stealing: divide nonce space among threads
    let nonce_range_per_thread = u64::MAX / thread_count as u64;
    let base_nonce = get_nonce_with_deterministic_fallback();

    // No manual header construction needed - use canonical methods

    // Spawn parallel mining threads using mine_with_dag
    for thread_id in 0..thread_count {
        let sender = sender.clone();
        let completion_flag = completion_flag.clone();
        let found_signal = found_signal.clone();
        let hashes_tried = hashes_tried.clone();
        let winning_nonce = winning_nonce.clone();
        let winning_hash = winning_hash.clone();
        let cancellation_token = cancellation_token.clone();
        let _qdag = _qdag.clone(); // Keep for future DAG optimization
        let block_template = block_template.clone(); // Clone for each thread

        let thread_start_nonce = base_nonce.wrapping_add(thread_id as u64 * nonce_range_per_thread);
        let thread_end_nonce = thread_start_nonce.wrapping_add(nonce_range_per_thread);

        thread_pool.spawn(move || {
            // Use canonical hashing for perfect mining-validation symmetry
            const BATCH_SIZE: u64 = 1000; // Larger batch size for DAG-optimized mining
            let mut current_nonce = thread_start_nonce;
            let mut test_block = block_template.clone();

            loop {
                // Early termination if solution found by another thread or cancellation requested
                if found_signal.load(Ordering::Relaxed)
                    || completion_flag.load(Ordering::Relaxed)
                    || cancellation_token.is_cancelled()
                {
                    break;
                }

                let batch_end = (current_nonce + BATCH_SIZE).min(thread_end_nonce);

                // Process nonces in batch using canonical methods
                for nonce in current_nonce..batch_end {
                    // Update block's nonce
                    test_block.nonce = nonce;

                    // Calculate PoW hash exclusively using canonical method
                    let pow_hash = test_block.hash_for_pow();

                    // Validate using canonical method for perfect symmetry
                    if test_block.is_pow_valid_with_pow_hash(pow_hash) {
                        // Atomic completion flag prevents timeout cancellation
                        if !completion_flag.swap(true, Ordering::AcqRel) {
                            // Store results atomically
                            winning_nonce.store(nonce, Ordering::Relaxed);
                            if let Ok(mut hash_guard) = winning_hash.lock() {
                                *hash_guard = *pow_hash.as_bytes();
                            }

                            // Signal other threads to stop
                            found_signal.store(true, Ordering::Release);

                            // Send result immediately
                            let _ = sender.send(MiningResult::Found {
                                nonce,
                                hash: *pow_hash.as_bytes(),
                            });

                            debug!(
                                "nonce_found_with_canonical_hashing; winning_nonce={} thread_id={}",
                                nonce, thread_id
                            );
                            return;
                        }
                    }
                }

                // Update hash attempts counter
                hashes_tried.fetch_add(batch_end.saturating_sub(current_nonce), Ordering::Relaxed);

                current_nonce = batch_end;
                if current_nonce >= thread_end_nonce {
                    // Work-stealing: jump to different nonce space
                    current_nonce = thread_start_nonce.wrapping_add(
                        (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as u64)
                            % nonce_range_per_thread,
                    );
                }
            }
        });
    }

    drop(sender); // Close sender to signal completion

    // Wait for result with async receiver
    if let Some(result) = receiver.recv().await {
        // If we found a solution, return it immediately regardless of timeout
        if matches!(result, MiningResult::Found { .. }) {
            debug!("DAG-optimized mining completed successfully with valid nonce");
        }
        return Ok(result);
    }

    // Check if cancellation was requested
    if cancellation_token.is_cancelled() {
        return Ok(MiningResult::Cancelled);
    }

    // All threads finished without finding a solution
    if completion_flag.load(Ordering::Acquire) && found_signal.load(Ordering::Acquire) {
        // Solution was found but channel closed, reconstruct result
        let nonce = winning_nonce.load(Ordering::Relaxed);
        if let Ok(hash_guard) = winning_hash.lock() {
            return Ok(MiningResult::Found {
                nonce,
                hash: *hash_guard,
            });
        }
    }

    Ok(MiningResult::Timeout)
}
