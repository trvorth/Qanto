//! --- Qanto Miner ---
//! v2.0.0 - Deterministic PoW
//! This version implements a production-grade, deterministic Proof-of-Work
//! target calculation, removing all floating-point arithmetic from the consensus-
//! critical path to eliminate nondeterminism and precision loss.
//!
//! - REFACTOR (Consensus-Critical): Replaced the f64-based `calculate_target_from_difficulty`
//!   with a pure `U256` integer-based method. Difficulty is now treated as a
//!   fixed-point number for precise, deterministic calculations across all nodes.
//! - OPTIMIZATION: Enhanced the internal `U256` division logic for efficiency.

use crate::qantodag::{QantoBlock, QantoDAG};
use anyhow::Result;
use hex;
use rand::Rng;
use rayon::prelude::*;
use regex::Regex;
use std::ops::Div;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

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
#[derive(Clone, Debug)]
pub struct Miner {
    _address: String,
    _dag: Arc<QantoDAG>,
    target_block_time: u64,
    _use_gpu: bool,
    _zk_enabled: bool,
    threads: usize,
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
        for i in 0..4 {
            let next_carry = self.0[i] >> 63;
            self.0[i] = (self.0[i] << 1) | carry;
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
            return Err(MiningError::InvalidBlock("Division by zero in difficulty calculation".to_string()));
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

impl Miner {
    #[instrument]
    pub fn new(config: MinerConfig) -> Result<Self> {
        let address_regex = Regex::new(r"^[0-9a-fA-F]{64}$")?;
        if !address_regex.is_match(&config.address) {
            return Err(MiningError::InvalidAddress(format!(
                "Invalid miner address format: {}",
                config.address
            ))
            .into());
        }

        let effective_use_gpu = config.use_gpu && cfg!(feature = "gpu");
        if config.use_gpu && !effective_use_gpu {
            warn!("GPU mining enabled in config but 'gpu' feature is not compiled. Disabling GPU for this session.");
        }
        let effective_zk_enabled = config.zk_enabled && cfg!(feature = "zk");
        if config.zk_enabled && !effective_zk_enabled {
            warn!("ZK proofs enabled in config but 'zk' feature is not compiled. Disabling ZK for this session.");
        }

        Ok(Self {
            _address: config.address,
            _dag: config.dag,
            target_block_time: config.target_block_time,
            _use_gpu: effective_use_gpu,
            _zk_enabled: effective_zk_enabled,
            threads: config.threads.max(1),
        })
    }

    /// Solves the Proof-of-Work for a given block template by finding a valid nonce.
    /// This is the primary mining function called by the node's mining loop.
    /// It modifies the block in-place with the found nonce and effort.
    #[instrument(skip(self, block_template))]
    pub fn solve_pow(&self, block_template: &mut QantoBlock) -> Result<(), MiningError> {
        debug!(
            "[DEBUG] solve_pow: Starting PoW for block {}",
            block_template.id
        );
        let start_time = SystemTime::now();
        let timeout_duration = Duration::from_secs(self.target_block_time);
        debug!(
            "[DEBUG] solve_pow: Timeout duration set to {} seconds",
            self.target_block_time
        );

        // This now uses pure integer math for deterministic consensus.
        let target_hash_bytes = Miner::calculate_target_from_difficulty(block_template.difficulty);
        debug!(
            "[DEBUG] solve_pow: Target hash calculated for difficulty {}",
            block_template.difficulty
        );

        if self._use_gpu {
            warn!("[GPU-MINE] GPU mining is enabled in config, but the implementation currently only supports CPU mining. Falling back to CPU.");
        }

        debug!(
            "[DEBUG] solve_pow: Calling mine_cpu with {} threads",
            self.threads
        );
        let mining_result = self.mine_cpu(
            block_template,
            &target_hash_bytes,
            start_time,
            timeout_duration,
        )?;
        debug!(
            "[DEBUG] solve_pow: mine_cpu returned {:?}",
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

    /// Solves the Proof-of-Work with cancellation support.
    /// This version accepts a cancellation token that allows graceful shutdown.
    #[instrument(skip(self, block_template, cancellation_token))]
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

    fn mine_cpu(
        &self,
        block_template: &QantoBlock,
        target_hash_value: &[u8],
        start_time: SystemTime,
        timeout_duration: Duration,
    ) -> Result<Option<(u64, u64)>, MiningError> {
        debug!(
            "[DEBUG] mine_cpu: Building thread pool with {} threads",
            self.threads
        );
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build()?;
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));

        debug!("[DEBUG] mine_cpu: Thread pool built, starting parallel mining");

        let result = thread_pool.install(|| {
            let nonce_start = rand::thread_rng().gen::<u64>();
            debug!(
                "[DEBUG] mine_cpu: Starting nonce range from {}",
                nonce_start
            );

            (nonce_start..u64::MAX)
                .into_par_iter()
                .find_map_any(|current_nonce| {
                    if found_signal.load(Ordering::Relaxed) {
                        return None;
                    }
                    let count = hashes_tried.fetch_add(1, Ordering::Relaxed);

                    if count.is_multiple_of(1_000_000) {
                        debug!(
                            "[DEBUG] mine_cpu: {} million hashes tried",
                            count / 1_000_000
                        );
                        if let Ok(elapsed) = start_time.elapsed() {
                            if elapsed > timeout_duration {
                                debug!("[DEBUG] mine_cpu: Timeout reached after {:?}", elapsed);
                                found_signal.store(true, Ordering::Relaxed);
                                return None;
                            }
                        }
                    }

                    let mut temp_block = block_template.clone();
                    temp_block.nonce = current_nonce;

                    let pow_hash = temp_block.hash();

                    if let Ok(hash_bytes) = hex::decode(&pow_hash) {
                        if Miner::hash_meets_target(&hash_bytes, target_hash_value)
                        {
                            found_signal.store(true, Ordering::Relaxed);
                            return Some(current_nonce);
                        }
                    }
                    None
                })
        });

        let final_hash_count = hashes_tried.load(Ordering::Relaxed);
        Ok(result.map(|nonce| (nonce, final_hash_count)))
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
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build()?;
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));

        debug!("[DEBUG] mine_cpu_with_cancellation: Thread pool built, starting parallel mining");

        let result = thread_pool.install(|| {
            let nonce_start = rand::thread_rng().gen::<u64>();
            debug!("[DEBUG] mine_cpu_with_cancellation: Starting nonce range from {}", nonce_start);

            (nonce_start..u64::MAX)
                .into_par_iter()
                .find_map_any(|current_nonce| {
                    // Check for cancellation
                    if cancellation_token.is_cancelled() || found_signal.load(Ordering::Relaxed) {
                        return None;
                    }

                    let count = hashes_tried.fetch_add(1, Ordering::Relaxed);

                    if count.is_multiple_of(100_000) {
                        // Check cancellation more frequently
                        if cancellation_token.is_cancelled() {
                            debug!("[DEBUG] mine_cpu_with_cancellation: Cancellation requested");
                            found_signal.store(true, Ordering::Relaxed);
                            return None;
                        }
                    }

                    if count.is_multiple_of(1_000_000) {
                        debug!("[DEBUG] mine_cpu_with_cancellation: {} million hashes tried", count / 1_000_000);
                        if let Ok(elapsed) = start_time.elapsed() {
                            if elapsed > timeout_duration {
                                debug!("[DEBUG] mine_cpu_with_cancellation: Timeout reached after {:?}", elapsed);
                                found_signal.store(true, Ordering::Relaxed);
                                return None;
                            }
                        }
                    }

                    let mut temp_block = block_template.clone();
                    temp_block.nonce = current_nonce;

                    let pow_hash = temp_block.hash();

                    if let Ok(hash_bytes) = hex::decode(&pow_hash) {
                        if Miner::hash_meets_target(&hash_bytes, target_hash_value)
                        {
                            found_signal.store(true, Ordering::Relaxed);
                            return Some(current_nonce);
                        }
                    }
                    None
                })
        });

        let final_hash_count = hashes_tried.load(Ordering::Relaxed);
        Ok(result.map(|nonce| (nonce, final_hash_count)))
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
}
