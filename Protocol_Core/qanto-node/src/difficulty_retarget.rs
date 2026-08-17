//! Adaptive Difficulty Retarget
//!
//! Replaces the fixed INITIAL_DIFFICULTY enforcement with an epoch-based
//! Exponential Moving Average (EMA) retarget bounded by ±25% per epoch.
//!
//! This resolves the liveness failure (attack_2) where miner local backoff
//! (dividing difficulty by 2 every 3 failed attempts) diverged from
//! consensus's hardcoded equality check.

use crate::QANTO_SCALE;
use std::time::Duration;

/// Epoch length in blocks for difficulty retargeting
pub const RETARGET_EPOCH_BLOCKS: u64 = 2016; // ~2 weeks at 10-min blocks

/// Maximum allowed difficulty change per epoch (±25%)
pub const MAX_CHANGE_FACTOR: u64 = 125; // 125% = 25% increase max
pub const MIN_CHANGE_FACTOR: u64 = 75; // 75% = 25% decrease min

/// EMA smoothing factor (alpha = 2 / (N + 1), N=10 samples ≈ 0.18)
/// Using fixed-point arithmetic: alpha_num / alpha_den = 18/100
const EMA_ALPHA_NUM: u64 = 18;
const EMA_ALPHA_DEN: u64 = 100;

/// Target block time in seconds
pub const TARGET_BLOCK_TIME_SECS: u64 = 600; // 10 minutes

/// Minimum difficulty floor (1.0 in Qanto scale)
pub const MIN_DIFFICULTY: u128 = QANTO_SCALE;

/// Maximum difficulty cap (prevents runaway)
pub const MAX_DIFFICULTY: u128 = QANTO_SCALE * 1_000_000;

/// Genesis difficulty (matches INITIAL_DIFFICULTY)
pub const GENESIS_DIFFICULTY: u128 = QANTO_SCALE;

/// Difficulty retarget state persisted per chain
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifficultyState {
    /// Current network difficulty
    pub current_difficulty: u128,
    /// EMA of observed solve times (seconds * 1000 for precision)
    pub ema_solve_time_ms: u64,
    /// Block height of last retarget
    pub last_retarget_height: u64,
    /// Number of samples in current epoch
    pub epoch_sample_count: u64,
    /// Accumulated solve time in current epoch (ms)
    pub epoch_accumulated_ms: u64,
}

impl Default for DifficultyState {
    fn default() -> Self {
        Self {
            current_difficulty: GENESIS_DIFFICULTY,
            ema_solve_time_ms: TARGET_BLOCK_TIME_SECS * 1000,
            last_retarget_height: 0,
            epoch_sample_count: 0,
            epoch_accumulated_ms: 0,
        }
    }
}

/// Calculate the required difficulty for a given block height.
/// Returns the difficulty the block MUST have to be valid.
pub fn get_required_difficulty(state: &DifficultyState, height: u64) -> u128 {
    if height == 0 {
        return GENESIS_DIFFICULTY;
    }
    state.current_difficulty
}

/// Record a block's solve time and update EMA.
/// Call this for each accepted block (after validation).
pub fn record_solve_time(state: &mut DifficultyState, solve_time_ms: u64, height: u64) {
    // Update EMA: new_ema = alpha * sample + (1 - alpha) * old_ema
    // Fixed-point: ema = (alpha_num * sample + (alpha_den - alpha_num) * ema) / alpha_den
    state.ema_solve_time_ms = (EMA_ALPHA_NUM * solve_time_ms
        + (EMA_ALPHA_DEN - EMA_ALPHA_NUM) * state.ema_solve_time_ms)
        / EMA_ALPHA_DEN;

    // Accumulate for epoch retarget
    state.epoch_accumulated_ms += solve_time_ms;
    state.epoch_sample_count += 1;

    // Check if epoch complete
    if height > state.last_retarget_height
        && (height - state.last_retarget_height) >= RETARGET_EPOCH_BLOCKS
    {
        retarget(state, height);
    }
}

/// Perform difficulty retarget at epoch boundary.
/// Bounded by ±25% to prevent oscillation.
fn retarget(state: &mut DifficultyState, height: u64) {
    if state.epoch_sample_count == 0 {
        return;
    }

    // Observed average solve time (ms)
    let observed_avg_ms = state.epoch_accumulated_ms / state.epoch_sample_count;
    let target_ms = TARGET_BLOCK_TIME_SECS * 1000;

    // Calculate raw adjustment factor
    // If blocks are too slow (observed > target), decrease difficulty
    // If blocks are too fast (observed < target), increase difficulty
    let raw_factor_num = target_ms * 100; // basis points for precision
    let raw_factor_den = observed_avg_ms;

    // Clamp to ±25%
    let mut factor_bps = (raw_factor_num * 100) / raw_factor_den; // basis points
    factor_bps = factor_bps.clamp(MIN_CHANGE_FACTOR * 100, MAX_CHANGE_FACTOR * 100); // 7500..12500

    // Apply: new_difficulty = old_difficulty * factor_bps / 10000
    let new_difficulty = state.current_difficulty.saturating_mul(factor_bps as u128) / 10_000;

    // Enforce absolute bounds
    state.current_difficulty = new_difficulty.clamp(MIN_DIFFICULTY, MAX_DIFFICULTY);

    // Reset epoch
    state.last_retarget_height = height;
    state.epoch_sample_count = 0;
    state.epoch_accumulated_ms = 0;
}

/// Validate that a block's difficulty matches the required difficulty for its height.
/// Used in `is_valid_block` consensus check.
pub fn validate_block_difficulty(
    state: &DifficultyState,
    block_difficulty: u64,
    height: u64,
) -> Result<(), String> {
    let required = get_required_difficulty(state, height);
    if block_difficulty as u128 != required {
        return Err(format!(
            "Block difficulty mismatch at height {}: expected {}, got {}",
            height, required, block_difficulty
        ));
    }
    Ok(())
}

/// Convert difficulty to mining target (hash threshold).
/// target = (2^256 - 1) / difficulty, but we use a simplified form for QanHash.
pub fn difficulty_to_target(difficulty: u128) -> [u8; 32] {
    // QanHash uses a 256-bit target. The practical upper bound for difficulty
    // is much smaller than 2^256, so we can compute: target = MAX_TARGET / difficulty
    // where MAX_TARGET = 2^256 - 1 ≈ [0xFF; 32]
    // For implementation simplicity, we use the inverse scaling approach:
    // target_bytes[i] = 0xFF * (QANTO_SCALE / difficulty) capped at 0xFF
    let scale = QANTO_SCALE.saturating_mul(255) / difficulty.max(1);
    let byte_val = scale.clamp(1, 255) as u8;

    // Return a target where all bytes are the scaled value
    // This gives roughly uniform distribution of valid hashes
    [byte_val; 32]
}

/// Legacy constant for backward compatibility during migration
/// DO NOT USE IN NEW CODE — use get_required_difficulty() instead.
pub const INITIAL_DIFFICULTY_LEGACY: u128 = GENESIS_DIFFICULTY;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_difficulty() {
        let state = DifficultyState::default();
        assert_eq!(get_required_difficulty(&state, 0), GENESIS_DIFFICULTY);
    }

    #[test]
    fn test_ema_updates_smoothly() {
        let mut state = DifficultyState::default();
        let initial_ema = state.ema_solve_time_ms;

        // Simulate consistently fast blocks (5 min = 300s = 300_000ms)
        for _ in 0..50 {
            record_solve_time(&mut state, 300_000, 1);
        }

        // EMA should drift toward 300_000 but not equal it immediately
        assert!(state.ema_solve_time_ms < initial_ema);
        assert!(state.ema_solve_time_ms > 300_000);
    }

    #[test]
    fn test_retarget_decreases_difficulty_on_slow_blocks() {
        let mut state = DifficultyState::default();
        state.current_difficulty = QANTO_SCALE * 1000; // Higher starting difficulty
        state.last_retarget_height = 0;
        state.epoch_sample_count = RETARGET_EPOCH_BLOCKS;
        state.epoch_accumulated_ms = 1200 * 1000 * RETARGET_EPOCH_BLOCKS; // 20 min avg

        retarget(&mut state, RETARGET_EPOCH_BLOCKS);

        // Difficulty should decrease (blocks too slow)
        assert!(state.current_difficulty < QANTO_SCALE * 1000);
        // But bounded by 25% decrease
        assert!(state.current_difficulty >= (QANTO_SCALE * 1000 * 75) / 100);
    }

    #[test]
    fn test_retarget_increases_difficulty_on_fast_blocks() {
        let mut state = DifficultyState::default();
        state.current_difficulty = QANTO_SCALE * 1000;
        state.last_retarget_height = 0;
        state.epoch_sample_count = RETARGET_EPOCH_BLOCKS;
        state.epoch_accumulated_ms = 300 * 1000 * RETARGET_EPOCH_BLOCKS; // 5 min avg

        retarget(&mut state, RETARGET_EPOCH_BLOCKS);

        // Difficulty should increase (blocks too fast)
        assert!(state.current_difficulty > QANTO_SCALE * 1000);
        // But bounded by 25% increase
        assert!(state.current_difficulty <= (QANTO_SCALE * 1000 * 125) / 100);
    }

    #[test]
    fn test_bounds_enforced() {
        let mut state = DifficultyState::default();
        state.current_difficulty = MIN_DIFFICULTY - 1; // Below floor

        // Any retarget should clamp to MIN_DIFFICULTY
        state.last_retarget_height = 0;
        state.epoch_sample_count = RETARGET_EPOCH_BLOCKS;
        state.epoch_accumulated_ms = TARGET_BLOCK_TIME_SECS * 1000 * RETARGET_EPOCH_BLOCKS;
        retarget(&mut state, RETARGET_EPOCH_BLOCKS);

        assert_eq!(state.current_difficulty, MIN_DIFFICULTY);
    }

    #[test]
    fn test_validate_block_difficulty() {
        let state = DifficultyState::default();
        assert!(validate_block_difficulty(&state, GENESIS_DIFFICULTY as u64, 0).is_ok());
        assert!(validate_block_difficulty(&state, (GENESIS_DIFFICULTY + 1) as u64, 0).is_err());
    }
}
