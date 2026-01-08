//! Consensus logic and difficulty adjustment algorithms.

mod difficulty;
pub use difficulty::*;

/// Calculates the next difficulty using the ASERT algorithm.
///
/// # Arguments
///
/// * `parent_difficulty` - The difficulty of the parent block.
/// * `parent_timestamp` - The timestamp of the parent block (in seconds or milliseconds, must match anchor).
/// * `parent_height` - The height of the parent block.
/// * `anchor_timestamp` - The timestamp of the anchor block.
/// * `anchor_height` - The height of the anchor block.
///
/// # Returns
///
/// The calculated difficulty for the next block.
pub fn calculate_next_difficulty(
    parent_difficulty: f64,
    parent_timestamp: u64,
    parent_height: u64,
    anchor_timestamp: u64,
    anchor_height: u64,
) -> f64 {
    const TARGET_BLOCK_TIME: f64 = 31.25; // 32 BPS (31.25ms)
    const HALFLIFE: f64 = 86400000.0; // 1 day in milliseconds

    let height_diff = (parent_height - anchor_height) as f64;
    let time_diff = (parent_timestamp - anchor_timestamp) as f64;

    // ASERT Formula: D_next = D_anchor * e^((time_diff - ideal_time) / tau)
    let exponent = (time_diff - (height_diff * TARGET_BLOCK_TIME)) / HALFLIFE;
    parent_difficulty * exponent.exp()
}

/// Calculates the next target using ASERT-DAA and maps to 256-bit target bytes.
#[allow(clippy::too_many_arguments)]
pub fn calculate_next_target(
    anchor_target: u64,
    anchor_timestamp_ms: u64,
    anchor_height: u64,
    current_timestamp_ms: u64,
    current_height: u64,
    tau_ms: i64,
    min_target: u64,
    max_target: u64,
) -> [u8; 32] {
    let height_delta = current_height as i64 - anchor_height as i64;
    let time_delta_ms = current_timestamp_ms as i64 - anchor_timestamp_ms as i64;
    const IDEAL_MS: i64 = 31; // 32 BPS (~31ms)

    let exponent = ((time_delta_ms - IDEAL_MS * height_delta) as f64) / (tau_ms as f64);
    let factor = 2.0_f64.powf(exponent);

    let mut next = (anchor_target as f64 * factor).round() as u128;
    next = next.min(max_target as u128).max(min_target as u128);

    let next_u64 = next as u64;
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&next_u64.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::calculate_next_target;
    #[test]
    fn asert_burst_and_lull_clamped() {
        let min = 1u64;
        let max = u64::MAX - 1;
        let anchor_t = 1000u64;
        let anchor_ts = 1_000u64;
        let anchor_h = 100u64;
        // Burst: time << ideal
        let next_burst =
            calculate_next_target(anchor_t, anchor_ts, anchor_h, 1_010, 101, 500, min, max);
        // Lull: time >> ideal
        let next_lull =
            calculate_next_target(anchor_t, anchor_ts, anchor_h, 2_000, 101, 500, min, max);
        assert_ne!(next_burst, next_lull);
    }
}
