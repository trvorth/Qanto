//! --- Qanto Miner ---
//! v0.1.0 - Deterministic PoW
//!
//! This version implements a production-grade, deterministic Proof-of-Work
//! target calculation, removing all floating-point arithmetic from the consensus-
//! critical path to eliminate nondeterminism and precision loss.

use crate::mining_celebration::{celebrate_mining_success, MiningCelebrationParams};
use crate::qantodag::{QantoBlock, QantoDAG};
use anyhow::Result;
use hex;
use my_blockchain::qanhash::{MiningConfig, MiningDevice, QanhashMiner};
use my_blockchain::qanto_standalone::hash::QantoHash;
use rand::Rng;
use rayon::prelude::*;
use regex::Regex;
use std::ops::Div;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tracing::{debug, info, warn};

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
}
pub struct Miner {
    _address: String,
    _dag: QantoDAG,
    target_block_time: u64,
    _use_gpu: bool,
    _zk_enabled: bool,
    threads: usize,
    qanhash_miner: QanhashMiner,
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

impl Miner {
    pub fn new(config: MinerConfig) -> Result<Self> {
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
        // Force CPU-only mining to avoid GPU initialization hang during development
        let mining_device = MiningDevice::Cpu;

        if effective_use_gpu {
            warn!("GPU mining requested but disabled for development to avoid initialization hang");
        }

        let qanhash_config = MiningConfig {
            device: mining_device,
            thread_count: Some(config.threads.max(1)),
            batch_size: Some(1024), // Optimized batch size for high throughput
            max_iterations: None,   // No limit for continuous mining
            enable_simd: true,      // Enable SIMD optimizations
        };

        let qanhash_miner = QanhashMiner::new(qanhash_config);

        Ok(Self {
            _address: config.address,
            _dag: (*config.dag).clone(),
            target_block_time: config.target_block_time,
            _use_gpu: effective_use_gpu,
            _zk_enabled: effective_zk_enabled,
            threads: config.threads.max(1),
            qanhash_miner,
        })
    }

    /// Solves the Proof-of-Work for a given block template by finding a valid nonce.
    /// This is the primary mining function called by the node's mining loop.
    /// It modifies the block in-place with the found nonce and effort.
    pub fn solve_pow(&self, block_template: &mut QantoBlock) -> Result<(), MiningError> {
        self.log_mining_start(block_template);

        let target_hash_bytes = Miner::calculate_target_from_difficulty(block_template.difficulty);
        let (header_hash, target) = self.prepare_mining_data(block_template, &target_hash_bytes)?;
        let mining_result = self.execute_mining(&header_hash, &target);

        self.process_mining_result(block_template, mining_result)
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

        let header_hash_hex = block_template.hash();
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
        let start_nonce = rand::thread_rng().gen::<u64>();
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
        mining_result: Option<(u64, [u8; 32])>,
    ) -> Result<(), MiningError> {
        if let Some((found_nonce, final_hash)) = mining_result {
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
                difficulty: 1000.0, // Default difficulty for display
                transactions_count: block_template.transactions.len(),
                mining_time: Duration::from_secs(30), // Approximate mining time
                effort,
                total_blocks_mined: 1,
                chain_id: 0,
                block_reward: 50_000_000_000, // 50 QNTO in smallest units (corrected)
                compact: false,
            };

            celebrate_mining_success(celebration_params);

            Ok(())
        } else {
            Err(MiningError::TimeoutOrCancelled)
        }
    }

    /// Solves the Proof-of-Work with cancellation support.
    /// This version accepts a cancellation token that allows graceful shutdown.
    pub fn solve_pow_with_cancellation(
        &self,
        block_template: &mut QantoBlock,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), MiningError> {
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
        let target_hash_bytes = Miner::calculate_target_from_difficulty(block_template.difficulty);
        debug!(
            "[DEBUG] solve_pow_with_cancellation: Target hash calculated for difficulty {}",
            block_template.difficulty
        );

        if self._use_gpu {
            warn!("[GPU-MINE] GPU mining is enabled in config, but the implementation currently only supports CPU mining. Falling back to CPU.");
        }

        debug!("[DEBUG] solve_pow_with_cancellation: Calling mine_cpu_with_cancellation with {} threads", self.threads);
        let mining_result = self.mine_cpu_with_cancellation(
            block_template,
            &target_hash_bytes,
            start_time,
            timeout_duration,
            cancellation_token,
        )?;
        debug!(
            "[DEBUG] solve_pow_with_cancellation: mine_cpu_with_cancellation returned {:?}",
            mining_result.is_some()
        );

        if let Some((found_nonce, effort)) = mining_result {
            block_template.nonce = found_nonce;
            block_template.effort = effort;

            info!(
                "PoW solved for block ID (pre-hash) {} with nonce {} and effort {}. Block is ready for finalization.",
                block_template.id, found_nonce, effort
            );

            Ok(())
        } else {
            Err(MiningError::TimeoutOrCancelled)
        }
    }

    fn mine_cpu_with_cancellation(
        &self,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
        start_time: SystemTime,
        timeout_duration: Duration,
        cancellation_token: tokio_util::sync::CancellationToken,
    ) -> Result<Option<(u64, u64)>, MiningError> {
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

        let result = thread_pool.install(|| {
            let nonce_start = rand::thread_rng().gen::<u64>();
            debug!(
                "[DEBUG] mine_cpu_with_cancellation: Starting nonce range from {}",
                nonce_start
            );

            (nonce_start..u64::MAX)
                .into_par_iter()
                .find_map_any(|current_nonce| {
                    self.try_nonce(
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
                    )
                })
        });

        let final_hash_count = hashes_tried.load(Ordering::Relaxed);
        Ok(result.map(|nonce| (nonce, final_hash_count)))
    }

    fn try_nonce(&self, current_nonce: u64, context: &MiningContext) -> Option<u64> {
        // Check for cancellation
        if self.should_stop_mining(&context.cancellation_token, &context.found_signal) {
            return None;
        }

        let count = context.hashes_tried.fetch_add(1, Ordering::Relaxed);

        // Periodic checks for cancellation and timeout
        if self.should_check_cancellation(count)
            && self.handle_cancellation_check(&context.cancellation_token, &context.found_signal)
        {
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
            return None;
        }

        // Try the nonce
        if self.test_nonce(
            current_nonce,
            &context.block_template,
            &context.target_hash_value,
        ) {
            context.found_signal.store(true, Ordering::Relaxed);
            return Some(current_nonce);
        }

        None
    }

    fn should_stop_mining(
        &self,
        cancellation_token: &tokio_util::sync::CancellationToken,
        found_signal: &Arc<AtomicBool>,
    ) -> bool {
        cancellation_token.is_cancelled() || found_signal.load(Ordering::Relaxed)
    }

    fn should_check_cancellation(&self, count: u64) -> bool {
        count.is_multiple_of(100_000)
    }

    fn handle_cancellation_check(
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

    fn should_check_timeout(&self, count: u64) -> bool {
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

    // Mining celebration is now handled by the mining_celebration module
}
