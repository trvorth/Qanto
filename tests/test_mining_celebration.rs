//! Test program to demonstrate mining celebration functionality

use qanto::mining_celebration::{celebrate_mining_success, MiningCelebrationParams};
use std::time::Duration;

#[test]
fn test_mining_celebration_display() {
    println!("Testing Mining Celebration Functionality\n");

    // Test with the 50 QAN reward and high transaction count for TPS demonstration
    let params = MiningCelebrationParams {
        block_height: 12345,
        block_hash: "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456".to_string(),
        nonce: 987654321,
        difficulty: 1000.0,
        transactions_count: 312500, // Target transactions per block for 10M+ TPS
        mining_time: Duration::from_millis(31), // 31ms for ~32 BPS
        effort: 2500000,
        total_blocks_mined: 1,
        chain_id: 0,
        block_reward: 50_000_000_000, // 50 QAN in smallest units (50 * 1e9)
        compact: false,
    };

    // Display full celebration
    celebrate_mining_success(params);

    println!();
    println!("{}", "=".repeat(50));
    println!("Now testing compact celebration:");
    println!();

    // Test compact version
    let compact_params = MiningCelebrationParams {
        block_height: 12345,
        block_hash: "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456".to_string(),
        nonce: 987654321,
        difficulty: 1000.0,
        transactions_count: 312500, // Target transactions per block for 10M+ TPS
        mining_time: Duration::from_millis(31), // 31ms for ~32 BPS
        effort: 2500000,
        total_blocks_mined: 1,
        chain_id: 0,
        block_reward: 50_000_000_000, // 50 QAN in smallest units (50 * 1e9)
        compact: true,
    };

    celebrate_mining_success(compact_params);

    println!("\nMining celebration test completed successfully!");
}
