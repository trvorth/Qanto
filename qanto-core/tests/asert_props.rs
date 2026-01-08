use proptest::prelude::*;
use qanto_core::adaptive_mining::{
    Anchor, AsertDifficulty, AsertDifficultyConfig, DifficultyAlgorithm,
};

fn apply_offset(base_ts: u64, off: i64) -> u64 {
    if off >= 0 {
        base_ts.saturating_add(off as u64)
    } else {
        base_ts.saturating_sub((-off) as u64)
    }
}

fn default_cfg() -> AsertDifficultyConfig {
    AsertDifficultyConfig {
        ideal_block_time_ms: 5000,
        ..AsertDifficultyConfig::default()
    }
}

// Identity: when time delta equals ideal * height delta, target should match anchor.
proptest! {
    #[test]
    fn asert_identity_on_ideal_timing(
        height_base in 100u64..1_000_000,
        height_delta in 0u64..1_000,
        ts in 0u64..1_000_000_000,
        target in 10u64..(u64::MAX / 4 - 10),
    ) {
        let cfg = default_cfg();
        let anchor = Anchor { height: height_base, timestamp: ts, target };
        let current_height = height_base + height_delta;
        let ideal_dt = (cfg.ideal_block_time_ms as u64 / 1000) * height_delta;
        let current_timestamp = ts + ideal_dt;

        let algo = AsertDifficulty::new(cfg);
        let next = algo.next_target(&anchor, current_height, current_timestamp);

        prop_assert_eq!(next, target);
    }
}

// Monotonicity: increasing (time_delta - ideal * height_delta) increases target (subject to clamp).
proptest! {
    #[test]
    fn asert_monotonicity_over_time_offset(
        height_base in 100u64..1_000_000,
        height_delta in 1u64..1_000,
        ts in 0u64..1_000_000_000,
        target in 10u64..(u64::MAX/4 - 10),
        off1 in -3600i64..3600,
        off2 in -3600i64..3600,
    ) {
        let cfg = default_cfg();
        let anchor = Anchor { height: height_base, timestamp: ts, target };
        let current_height = height_base + height_delta;
        let base_ts = ts + (cfg.ideal_block_time_ms as u64 / 1000) * height_delta;

        let (o1, o2) = if off1 <= off2 { (off1, off2) } else { (off2, off1) };
        let t1 = apply_offset(base_ts, o1);
        let t2 = apply_offset(base_ts, o2);

        let algo = AsertDifficulty::new(cfg);
        let n1 = algo.next_target(&anchor, current_height, t1);
        let n2 = algo.next_target(&anchor, current_height, t2);

        // Monotonic non-decreasing with respect to offset.
        prop_assert!(n1 <= n2);
    }
}

// Clamping: extreme positive offset clamps to max_target; extreme negative clamps to min_target.
proptest! {
    #[test]
    fn asert_clamping_extremes(
        height_base in 100u64..1_000_000,
        height_delta in 1u64..1_000,
        ts in 0u64..1_000_000_000,
        pos_off in 600i64..3600,
        neg_off in -3600i64..-600,
    ) {
        let cfg = default_cfg();
        let algo = AsertDifficulty::new(cfg);
        let anchor_hi = Anchor { height: height_base, timestamp: ts, target: cfg.max_target - 1 };
        let anchor_lo = Anchor { height: height_base, timestamp: ts, target: cfg.min_target + 1 };
        let current_height = height_base + height_delta;
        let base_ts = ts + (cfg.ideal_block_time_ms as u64 / 1000) * height_delta;
        let t_pos = apply_offset(base_ts, pos_off);
        let t_neg = apply_offset(base_ts, neg_off);

        let hi = algo.next_target(&anchor_hi, current_height, t_pos);
        let lo = algo.next_target(&anchor_lo, current_height, t_neg);

        prop_assert_eq!(hi, cfg.max_target);
        prop_assert_eq!(lo, cfg.min_target);
    }
}

// Determinism: same inputs yield same output.
proptest! {
    #[test]
    fn asert_deterministic(
        height_base in 100u64..1_000_000,
        height_delta in 0u64..1_000,
        ts in 0u64..1_000_000_000,
        target in 10u64..(u64::MAX/4 - 10),
        off in -3600i64..3600,
    ) {
        let cfg = default_cfg();
        let anchor = Anchor { height: height_base, timestamp: ts, target };
        let current_height = height_base + height_delta;
        let base_ts = ts + (cfg.ideal_block_time_ms as u64 / 1000) * height_delta;
        let t = apply_offset(base_ts, off);

        let algo = AsertDifficulty::new(cfg);
        let n1 = algo.next_target(&anchor, current_height, t);
        let n2 = algo.next_target(&anchor, current_height, t);
        prop_assert_eq!(n1, n2);
    }
}
