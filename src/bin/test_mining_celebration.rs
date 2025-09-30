//! Test program to demonstrate mining celebration functionality

use qanto::mining_celebration::{celebrate_mining_success, MiningCelebrationParams};
use std::time::Duration;

fn main() {
    println!("Testing Mining Celebration Functionality\n");

    // Test with the 12,500 QNTO reward that was observed
    let params = MiningCelebrationParams {
        block_height: 12345,
        block_hash: "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456".to_string(),
        nonce: 987654321,
        difficulty: 1000.0,
        transactions_count: 5,
        mining_time: Duration::from_secs(45),
        effort: 2500000,
        total_blocks_mined: 1,
        chain_id: 0,
        block_reward: 150000000000, // 150 QNTO in smallest units (150 * 1e9)
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
        transactions_count: 5,
        mining_time: Duration::from_secs(45),
        effort: 2500000,
        total_blocks_mined: 1,
        chain_id: 0,
        block_reward: 150000000000, // 150 QNTO in smallest units (150 * 1e9)
        compact: true,
    };

    celebrate_mining_success(compact_params);

    println!("\nMining celebration test completed successfully!");
}
