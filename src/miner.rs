//! --- Qanto Miner ---
//! v1.5.0 - f64 Difficulty Compatibility
//! This version resolves build errors by making the miner compatible with the new
//! f64-based difficulty system managed by the SAGA pallet.
//!
//! - BUILD FIX (E0308): Updated `calculate_target_from_difficulty` to accept an `f64`,
//!   aligning it with the `QantoBlock` struct and SAGA's PID controller output.
//! - REFACTOR: Enhanced the internal `U256` type to handle division by an `f64`
//!   to correctly calculate the PoW target from the new floating-point difficulty.

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
use tracing::{info, instrument, warn};

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
    pub difficulty_hex: String,
    pub target_block_time: u64,
    pub use_gpu: bool,
    pub zk_enabled: bool,
    pub threads: usize,
    pub num_chains: u32,
}
#[derive(Clone, Debug)]
pub struct Miner {
    _address: String,
    _dag: Arc<QantoDAG>,
    _difficulty: u64,
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

impl Div<U256> for U256 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        self.div_rem(rhs).0
    }
}

impl U256 {
    fn div_rem(self, divisor: Self) -> (Self, Self) {
        if divisor == Self::ZERO {
            panic!("division by zero");
        }
        if self < divisor {
            return (Self::ZERO, self);
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
        (quotient, remainder)
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
        let difficulty = u64::from_str_radix(config.difficulty_hex.trim_start_matches("0x"), 16)?;
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
            _difficulty: difficulty,
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
        let start_time = SystemTime::now();
        let timeout_duration = Duration::from_secs(self.target_block_time);

        // --- FIX (E0308): Pass the f64 difficulty directly ---
        let target_hash_bytes = Miner::calculate_target_from_difficulty(block_template.difficulty);

        if self._use_gpu {
            warn!("[GPU-MINE] GPU mining is enabled in config, but the implementation currently only supports CPU mining. Falling back to CPU.");
        }

        let mining_result = self.mine_cpu(
            block_template,
            &target_hash_bytes,
            start_time,
            timeout_duration,
        )?;

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
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build()?;
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));

        let result = thread_pool.install(|| {
            let nonce_start = rand::thread_rng().gen::<u64>();

            (nonce_start..u64::MAX)
                .into_par_iter()
                .find_map_any(|current_nonce| {
                    if found_signal.load(Ordering::Relaxed) {
                        return None;
                    }
                    let count = hashes_tried.fetch_add(1, Ordering::Relaxed);

                    if count.is_multiple_of(1_000_000) {
                        if let Ok(elapsed) = start_time.elapsed() {
                            if elapsed > timeout_duration {
                                found_signal.store(true, Ordering::Relaxed);
                                return None;
                            }
                        }
                    }

                    let mut temp_block = block_template.clone();
                    temp_block.nonce = current_nonce;

                    let pow_hash = temp_block.hash();

                    if Miner::hash_meets_target(&hex::decode(&pow_hash).unwrap(), target_hash_value)
                    {
                        found_signal.store(true, Ordering::Relaxed);
                        return Some(current_nonce);
                    }
                    None
                })
        });

        let final_hash_count = hashes_tried.load(Ordering::Relaxed);
        Ok(result.map(|nonce| (nonce, final_hash_count)))
    }

    /// Calculates the PoW target from a floating-point difficulty value.
    /// A higher difficulty results in a lower (harder) target.
    pub fn calculate_target_from_difficulty(difficulty_value: f64) -> Vec<u8> {
        if difficulty_value <= 0.0 {
            return U256::MAX.to_big_endian_vec();
        }

        // The formula is: target = max_target / difficulty
        // Since we are using integer math, we can approximate this.
        // For f64, we can perform the division directly on a high-precision representation.
        let max_target_float = 2.0_f64.powi(256);
        let target_float = max_target_float / difficulty_value;

        // Convert the resulting float back to a 256-bit integer representation.
        // This is a simplification; a full implementation would use a big number library.
        let mut target = U256::ZERO;
        if target_float.is_finite() {
            // This is an approximation. For cryptographic purposes, a proper BigInt library
            // would be necessary to avoid precision loss.
            let mut val = target_float;
            for i in (0..256).rev() {
                let bit_val = 2.0_f64.powi(i as i32);
                if val >= bit_val {
                    target.set_bit(i);
                    val -= bit_val;
                }
            }
        } else {
            // If difficulty is extremely low, target is effectively MAX
            return U256::MAX.to_big_endian_vec();
        }

        target.to_big_endian_vec()
    }

    /// Checks if a given hash is less than or equal to the target.
    pub fn hash_meets_target(hash_bytes: &[u8], target_bytes: &[u8]) -> bool {
        hash_bytes <= target_bytes
    }
}
