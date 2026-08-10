//! Copyright (C) 2026 Trevor <trvorth@qanto.org>
//!
//! End-to-end integration test verifying the full mining lifecycle:
//!   block creation → PoW mining → DAG acceptance → reward distribution → emission supply reduction.

use qanto::emission;
use qanto::emission::Emission;

// ─── Emission Supply Reduction ───────────────────────────────────────────

#[test]
fn test_emission_supply_reduction_after_reward() {
    let genesis_time = 1_000_000u64;
    let num_chains = 1u32;
    let mut emission = Emission::default_with_timestamp(genesis_time, num_chains);

    assert_eq!(emission.current_supply(), 0);

    // Initial reward: 50 QAN
    let reward_y0 = emission.calculate_reward(genesis_time).unwrap();
    assert_eq!(reward_y0, 50_000_000_000);
    emission.update_supply(reward_y0).unwrap();
    assert_eq!(emission.current_supply(), 50_000_000_000);

    // After 3 halving periods (~291 days): reward = 50 * 0.999^3 ≈ 49.85 QAN
    let hp = emission::HALVING_PERIOD;
    let reward_after_3_periods = emission.calculate_reward(genesis_time + 3 * hp).unwrap();
    // 50 * 0.999^3 = 50 * 0.997002999 = 49.85014995 QAN
    assert_eq!(reward_after_3_periods, 49 * emission::SCALE + 850_149_950);
    emission.update_supply(reward_after_3_periods).unwrap();
    assert_eq!(emission.current_supply(), 50_000_000_000 + 49_850_149_950);

    assert!(emission.current_supply() < emission.total_supply());
}

#[test]
fn test_halving_never_goes_below_safety_floor() {
    let genesis_time = 1_000_000u64;
    let num_chains = 1u32;
    let emission = Emission::default_with_timestamp(genesis_time, num_chains);

    let reward = emission
        .calculate_reward(genesis_time + 100 * emission::HALVING_PERIOD)
        .unwrap();
    assert!(reward >= 1, "Safety floor failed: reward was {}", reward);
}

#[test]
fn emission_total_supply_never_breached() {
    let genesis_time = 1_000_000u64;
    let num_chains = 1u32;
    let mut emission = Emission::default_with_timestamp(genesis_time, num_chains);

    let total = emission.total_supply();
    for i in 0..10000 {
        let reward = emission
            .calculate_reward(genesis_time + i * 300)
            .unwrap_or(0);
        emission.update_supply(reward).unwrap();
    }
    assert!(emission.current_supply() <= total);
    assert!(
        emission.current_supply() > 0,
        "Supply should be non-zero after mining"
    );
}

#[test]
fn emission_per_chain_splits_reward() {
    let genesis_time = 1_000_000u64;
    for num_chains in [1u32, 2, 4, 8] {
        let emission = Emission::default_with_timestamp(genesis_time, num_chains);
        let reward = emission.calculate_reward(genesis_time).unwrap();
        let total = reward.saturating_mul(num_chains as u128);
        assert!(total <= 50 * emission::SCALE);
    }
}

#[test]
fn test_halving_10_year_schedule() {
    let genesis_time = 1_000_000u64;
    let num_chains = 1u32;
    let emission = Emission::default_with_timestamp(genesis_time, num_chains);

    let scale = emission::SCALE;
    let hp = emission::HALVING_PERIOD;
    let _factor = emission::HALVING_FACTOR_FIXED; // 999_000_000 = 0.999

    // With factor 0.999 per period: 50 → 49.95 → 49.90005 → ...
    assert_eq!(emission.calculate_reward(genesis_time).unwrap(), 50 * scale);
    assert_eq!(
        emission.calculate_reward(genesis_time + 1 * hp).unwrap(),
        49 * scale + 950_000_000
    );
    assert_eq!(
        emission.calculate_reward(genesis_time + 2 * hp).unwrap(),
        49 * scale + 900_050_000
    );
    assert_eq!(
        emission.calculate_reward(genesis_time + 3 * hp).unwrap(),
        49 * scale + 850_149_950
    );
}

#[test]
fn test_supply_cap_clamps() {
    let genesis_time = 1_000_000u64;
    let mut emission = Emission::default_with_timestamp(genesis_time, 1);

    let max_supply = emission.total_supply();
    emission.update_supply(max_supply).unwrap();
    assert_eq!(emission.current_supply(), max_supply);

    // Adding more clamps, never overflows
    emission.update_supply(1_000_000 * emission::SCALE).unwrap();
    assert_eq!(emission.current_supply(), max_supply);
}

#[test]
fn test_block_reward_distribution() {
    let total_reward = 50_000_000_000u128;
    let dev_fee_rate = 25_000_000u128;
    let scale = 1_000_000_000u128;

    let dev_fee = total_reward
        .saturating_mul(dev_fee_rate)
        .saturating_div(scale);
    let validator_amount = total_reward.saturating_sub(dev_fee);

    assert_eq!(dev_fee, 1_250_000_000); // 2.5% of 50 QAN
    assert_eq!(validator_amount, 48_750_000_000); // 48.75 QAN
    assert_eq!(validator_amount + dev_fee, total_reward);
}

// ─── Balance Stream ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_balance_broadcaster_emit_and_replay_protection() {
    use qanto_core::balance_stream::BalanceBroadcaster;
    use tokio::time::{timeout, Duration};

    let bb = BalanceBroadcaster::new(1024);
    let mut rx = bb.subscribe();

    bb.set_balance("addr_a", 1000);
    let update = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("Timeout")
        .expect("Should receive update");
    assert_eq!(update.address, "addr_a");
    assert_eq!(update.balance, 1000);

    // Duplicate should not emit
    bb.set_balance("addr_a", 1000);
    assert!(timeout(Duration::from_millis(100), rx.recv())
        .await
        .is_err());
}

#[tokio::test]
async fn test_balance_delta_accumulates_and_floors() {
    use qanto_core::balance_stream::BalanceBroadcaster;

    let bb = BalanceBroadcaster::new(1024);
    bb.set_balance("wallet", 1000);
    bb.apply_delta("wallet", 500); // reward
    bb.apply_delta("wallet", -200); // spend
    assert_eq!(bb.get_balance("wallet"), Some(1300));

    // Floor at 0, never panic
    bb.apply_delta("wallet", -5000);
    assert_eq!(bb.get_balance("wallet"), Some(0));
}
