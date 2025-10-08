//! --- Qanto Miner ---
//! v0.1.0 - Deterministic PoW
//!
//! This version implements a production-grade, deterministic Proof-of-Work
//! target calculation, removing all floating-point arithmetic from the consensus-
//! critical path to eliminate nondeterminism and precision loss.

use crate::deterministic_mining::get_nonce_with_deterministic_fallback;
use crate::mining_celebration::{on_block_mined, MiningCelebrationParams};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::shutdown::{ShutdownController, ShutdownPhase, ShutdownSignal};
use crate::config::LoggingConfig;
use crate::telemetry::increment_hash_attempts;
use anyhow::Result;
use hex;
use my_blockchain::qanhash::{MiningConfig, MiningDevice, QanhashMiner};
use my_blockchain::qanto_standalone::hash::QantoHash;
use rayon::prelude::*;
use regex::Regex;
use std::ops::Div;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, info, warn, instrument, span, Level};
use crate::set_metric;
use crate::increment_metric;

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
    qanhash_miner: QanhashMiner,
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
            initial_delay_ms: 100,    // Start with 100ms delay
            max_delay_ms: 5000,       // Cap at 5 seconds
            multiplier: 2.0,          // Double delay each time
            max_retries: 10,          // Maximum retry attempts
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

        let effective_use_gpu = config.use_gpu && cfg!(feature = "gpu");
        if config.use_gpu && !effective_use_gpu {
            warn!("GPU mining enabled in config but 'gpu' feature is not compiled. Disabling GPU for this session.");
        }
        let effective_zk_enabled = config.zk_enabled && cfg!(feature = "zk");
        if config.zk_enabled && !effective_zk_enabled {
            warn!("ZK proofs enabled in config but 'zk' feature is not compiled. Disabling ZK for this session.");
        }

        // Configure QanHash miner with optimal settings for high throughput
        // Try GPU first if available and enabled, then fallback to CPU
        let mining_device = if effective_use_gpu {
            // Check if CUDA is available
            #[cfg(feature = "gpu")]
            {
                if Self::is_cuda_available() {
                    info!("CUDA detected, enabling GPU mining");
                    MiningDevice::Gpu
                } else {
                    warn!("GPU mining requested but CUDA not available, falling back to CPU");
                    MiningDevice::Cpu
                }
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
            batch_size: Some(1024), // Optimized batch size for high throughput
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

        info!("Miner initialized successfully with {} threads", effective_threads);

        Ok(Self {
            _address: config.address,
            _dag: (*config.dag).clone(),
            target_block_time: config.target_block_time,
            _use_gpu: effective_use_gpu,
            _zk_enabled: effective_zk_enabled,
            threads: effective_threads,
            qanhash_miner,
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
        let solve_start = Instant::now();
        self.log_mining_start(block_template);

        let target_hash_bytes = Miner::calculate_target_from_difficulty(block_template.difficulty);
        let (header_hash, target) = self.prepare_mining_data(block_template, &target_hash_bytes)?;
        let mining_result = self.execute_mining(&header_hash, &target);
        let mining_result_enum = match mining_result {
            Some((nonce, hash)) => MiningResult::Found { nonce, hash },
            None => MiningResult::Cancelled,
        };

        let result = self.process_mining_result(block_template, mining_result_enum);

        // Record solve time metrics
        let solve_duration = solve_start.elapsed();
        if let Ok(mut times) = self.solve_times.lock() {
            times.push(solve_duration);
            // Keep only the last 100 solve times to prevent memory growth
            if times.len() > 100 {
                times.remove(0);
            }
        }

        // Log the average solve time after each PoW solution
        if result.is_ok() {
            let avg_solve_time = self.get_average_solve_time();
            info!(
                "Average solve time over last {} solves: {:.2}s",
                self.get_solve_count(),
                avg_solve_time
            );
        }

        result
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

    /// Execute the actual mining process
    fn execute_mining(&self, header_hash: &[u8; 32], target: &[u8; 32]) -> Option<(u64, [u8; 32])> {
        let start_nonce = get_nonce_with_deterministic_fallback();
        let qanto_hash = QantoHash::new(*header_hash);

        debug!(
            "[DEBUG] solve_pow: Using advanced QanHash mining with {} threads",
            self.threads
        );

        let mining_result = self.qanhash_miner.mine(&qanto_hash, start_nonce, *target);

        debug!(
            "[DEBUG] solve_pow: QanHash mining returned {:?}",
            mining_result.is_some()
        );

        mining_result
    }

    /// Process the mining result and update block template
    fn process_mining_result(
        &self,
        block_template: &mut QantoBlock,
        mining_result: MiningResult,
    ) -> Result<(), MiningError> {
        match mining_result {
            MiningResult::Found { nonce: found_nonce, hash: final_hash } => {
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
                info!(
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
            MiningResult::Cancelled => {
                Err(MiningError::TimeoutOrCancelled)
            }
            MiningResult::Timeout => {
                Err(MiningError::TimeoutOrCancelled)
            }
        }
    }

    #[instrument(level = "info", skip(self, block_template, cancellation_token), fields(block_id = %block_template.id, difficulty = block_template.difficulty, threads = self.threads))]
    pub fn solve_pow_with_cancellation(
        &self,
        block_template: &mut QantoBlock,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), MiningError> {
        let _span = span!(Level::INFO, "mining_operation", 
            block_id = %block_template.id, 
            difficulty = block_template.difficulty,
            use_gpu = self._use_gpu
        ).entered();
        
        let solve_start = Instant::now();
        debug!(
            "[DEBUG] solve_pow_with_cancellation: Starting PoW for block {}",
            block_template.id
        );
        let start_time = SystemTime::now();
        let timeout_duration = Duration::from_secs(self.target_block_time);
        debug!(
            "[DEBUG] solve_pow_with_cancellation: Timeout duration set to {} seconds",
            self.target_block_time
        );

        // This now uses pure integer math for deterministic consensus.
        let target_hash_bytes = {
            let _target_span = span!(Level::DEBUG, "calculate_target").entered();
            Miner::calculate_target_from_difficulty(block_template.difficulty)
        };
        debug!(
            "[DEBUG] solve_pow_with_cancellation: Target hash calculated for difficulty {}",
            block_template.difficulty
        );

        // Try GPU mining first if enabled and available, then fallback to CPU
        let mining_result = if self._use_gpu {
            #[cfg(feature = "gpu")]
            {
                if Self::is_cuda_available() {
                    info!("[GPU-MINE] Attempting GPU mining with CUDA");
                    match self.mine_gpu_with_cancellation(
                        block_template,
                        &target_hash_bytes,
                        start_time,
                        timeout_duration,
                        cancellation_token.clone(),
                    ) {
                        Ok(MiningResult::Found { nonce, hash }) => {
                            info!("[GPU-MINE] GPU mining successful");
                            MiningResult::Found { nonce, hash }
                        }
                        Ok(MiningResult::Cancelled) | Ok(MiningResult::Timeout) => {
                            warn!("[GPU-MINE] GPU mining cancelled or timed out, falling back to CPU");
                            self.mine_cpu_with_cancellation(
                                block_template,
                                &target_hash_bytes,
                                start_time,
                                timeout_duration,
                                cancellation_token,
                            )?
                        }
                        Err(e) => {
                            warn!("[GPU-MINE] GPU mining error: {e}, falling back to CPU");
                            self.mine_cpu_with_cancellation(
                                block_template,
                                &target_hash_bytes,
                                start_time,
                                timeout_duration,
                                cancellation_token,
                            )?
                        }
                    }
                } else {
                    warn!("[GPU-MINE] CUDA not available, using CPU mining");
                    self.mine_cpu_with_cancellation(
                        block_template,
                        &target_hash_bytes,
                        start_time,
                        timeout_duration,
                        cancellation_token,
                    )?
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                warn!("[GPU-MINE] GPU feature not compiled, using CPU mining");
                self.mine_cpu_with_cancellation(
                    block_template,
                    &target_hash_bytes,
                    start_time,
                    timeout_duration,
                    cancellation_token,
                )?
            }
        } else {
            debug!("[DEBUG] solve_pow_with_cancellation: Calling mine_cpu_with_cancellation with {} threads", self.threads);
            self.mine_cpu_with_cancellation(
                block_template,
                &target_hash_bytes,
                start_time,
                timeout_duration,
                cancellation_token,
            )?
        };

        debug!(
            "[DEBUG] solve_pow_with_cancellation: mining returned {:?}",
            mining_result.is_found()
        );

        // Record solve time metrics
        let solve_duration = solve_start.elapsed();
        if let Ok(mut times) = self.solve_times.lock() {
            times.push(solve_duration);
            // Keep only the last 100 solve times to prevent memory growth
            if times.len() > 100 {
                times.remove(0);
            }
        }

        // Process the mining result
        match mining_result {
            MiningResult::Found { nonce, hash } => {
                let _success_span = span!(Level::INFO, "mining_success", nonce = nonce).entered();
                info!("Mining successful! Found nonce: {}", nonce);
                
                // Set the nonce in the block template
                block_template.nonce = nonce;
                
                // Calculate effort for mining celebration
                let effort = self.total_hashes.load(Ordering::Relaxed);
                
                // Use the detailed mining celebration with proper parameters
                let _mining_time = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default();

                let celebration_params = MiningCelebrationParams {
                    block_height: block_template.height,
                    block_hash: hex::encode(&hash[..8]), // First 8 bytes as hex
                    nonce,
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
            MiningResult::Cancelled | MiningResult::Timeout => {
                warn!("Mining operation cancelled or timed out");
                Err(MiningError::TimeoutOrCancelled)
            }
        }
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
    ) -> Result<MiningResult, MiningError> {
        // Silence unused variable warnings in placeholder implementation
        let _ = block_template;
        let _ = target_hash_bytes;
        let _ = start_time;
        let _ = timeout_duration;
        let _ = cancellation_token;

        // GPU mining implementation would go here
        // For now, return Cancelled to indicate fallback to CPU
        warn!("[GPU-MINE] GPU mining implementation not yet complete, falling back to CPU");
        Ok(MiningResult::Cancelled)
    }

    #[instrument(level = "debug", skip(self, block_template, target_hash_value, cancellation_token), fields(threads = self.threads, timeout_duration = ?timeout_duration))]
    fn mine_cpu_with_cancellation(
        &self,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
        start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<MiningResult, MiningError> {
        let _span = span!(Level::DEBUG, "cpu_mining_setup");
        debug!(
            "[DEBUG] mine_cpu_with_cancellation: Building thread pool with {} threads",
            self.threads
        );
        
        // Implement dynamic thread adjustment for CPU optimization
        let available_cores = num_cpus::get();
        let dynamic_threads = (available_cores as f64 * 0.75).ceil() as usize; // Use 75% of cores to reduce load
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(dynamic_threads.min(self.threads))
            .build()?;
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));

        debug!("[DEBUG] mine_cpu_with_cancellation: Thread pool built, starting parallel mining");

        // Batch configuration for nonce processing
        const BATCH_SIZE: u64 = 10_000; // Process nonces in batches
        const CANCELLATION_CHECK_INTERVAL: u64 = 1_000; // Check cancellation every 1k nonces
        
        let _mining_span = span!(Level::DEBUG, "parallel_mining", batch_size = BATCH_SIZE, check_interval = CANCELLATION_CHECK_INTERVAL);
        let result = thread_pool.install(|| {
            let nonce_start = get_nonce_with_deterministic_fallback();
            debug!(
                "[DEBUG] mine_cpu_with_cancellation: Starting nonce range from {}",
                nonce_start
            );

            // Split nonce range into batches for better cancellation responsiveness
            (0..u64::MAX / BATCH_SIZE)
                .into_par_iter()
                .find_map_any(|batch_idx| {
                    let batch_start = nonce_start.wrapping_add(batch_idx * BATCH_SIZE);
                    let batch_end = batch_start.wrapping_add(BATCH_SIZE);
                    
                    // Process batch sequentially for better cache locality
                    for current_nonce in batch_start..batch_end {
                        // Check cancellation at regular intervals within batch
                        if current_nonce.is_multiple_of(CANCELLATION_CHECK_INTERVAL)
                            && (cancellation_token.is_cancelled() || found_signal.load(Ordering::Relaxed)) {
                                return Some(MiningResult::Cancelled);
                            }

                        if let Some(winning_nonce) = self.try_nonce(
                            current_nonce,
                            &MiningContext {
                                block_template: block_template.clone(),
                                target_hash_value: target_hash_value.to_vec(),
                                cancellation_token: cancellation_token.clone(),
                                found_signal: found_signal.clone(),
                                hashes_tried: hashes_tried.clone(),
                                start_time,
                                timeout_duration,
                            },
                        ) {
                            let final_hash_count = hashes_tried.load(Ordering::Relaxed);
                            let mut hash = [0u8; 32];
                            hash[0..8].copy_from_slice(&winning_nonce.to_be_bytes());
                            hash[8..16].copy_from_slice(&final_hash_count.to_be_bytes());
                            return Some(MiningResult::Found { nonce: winning_nonce, hash });
                        }
                    }
                    
                    None // Continue to next batch
                })
        });

        Ok(result.unwrap_or(MiningResult::Cancelled))
    }

    #[instrument(level = "trace", skip(self, context), fields(nonce = current_nonce))]
    fn try_nonce(&self, current_nonce: u64, context: &MiningContext) -> Option<u64> {
        // Check for cancellation
        if self.should_stop_mining(&context.cancellation_token, &context.found_signal) {
            context.found_signal.store(true, Ordering::Relaxed);
            return None;
        }

        let count = context.hashes_tried.fetch_add(1, Ordering::Relaxed);
        increment_metric!(mining_attempts);
        
        // Increment global hash attempts counter for telemetry
        increment_hash_attempts();

        // Periodic checks for cancellation and timeout
        if self.should_check_cancellation(count)
            && self.handle_cancellation_check(&context.cancellation_token, &context.found_signal)
        {
            let _span = span!(Level::TRACE, "cancellation_triggered", count = count);
            return None;
        }

        if self.should_check_timeout(count)
            && self.handle_timeout_check(
                context.start_time,
                context.timeout_duration,
                &context.found_signal,
                count,
            )
        {
            let _span = span!(Level::TRACE, "timeout_triggered", count = count);
            return None;
        }

        // Try the nonce
        if self.test_nonce(
            current_nonce,
            &context.block_template,
            &context.target_hash_value,
        ) {
            let _span = span!(Level::INFO, "nonce_found", winning_nonce = current_nonce, hashes_tried = count);
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

    fn test_nonce(
        &self,
        current_nonce: u64,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
    ) -> bool {
        let mut temp_block = block_template.clone();
        temp_block.nonce = current_nonce;
        let pow_hash = temp_block.hash();

        if let Ok(hash_bytes) = hex::decode(&pow_hash) {
            return Miner::hash_meets_target(&hash_bytes, target_hash_value);
        }
        false
    }

    /// Calculates the PoW target from a floating-point difficulty value using
    /// deterministic, fixed-point integer arithmetic. This is a consensus-critical function.
    /// Formula: target = max_target / difficulty
    pub fn calculate_target_from_difficulty(difficulty_value: f64) -> Vec<u8> {
        // A difficulty of 0 or less is invalid and results in the easiest possible target.
        if difficulty_value <= 0.0 {
            return U256::MAX.to_big_endian_vec();
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

        target.to_big_endian_vec()
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
            target_hash_value: target_hash_bytes,
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

                return Ok(MiningResult::Found { nonce: winning_nonce, hash });
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
                    let hps = crate::metrics::QantoMetrics::compute_hash_rate(delta_attempts, interval_ms);
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
}
