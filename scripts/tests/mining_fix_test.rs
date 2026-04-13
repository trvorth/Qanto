use qanto::consensus::Consensus;
use qanto::miner::Miner;
use qanto::qantodag::QantoBlock;

#[tokio::test]
async fn test_mining_with_low_difficulty() {
    println!("Testing mining fix with difficulty 0.0001");

    // Create a dummy block with difficulty 0.0001
    let mut block = QantoBlock::new_test_block("test_block_1".to_string());
    block.difficulty = 0.0001;
    block.nonce = 0;

    println!("Block difficulty: {}", block.difficulty);

    // Calculate target from difficulty
    let target = Miner::calculate_target_from_difficulty(block.difficulty);
    println!("Target hash: {}", hex::encode(target));

    // The target should not be all f's anymore
    assert_ne!(
        target, [0xff; 32],
        "Target should not be all f's with the fix"
    );

    // Try a few nonces to see if we can find a valid one quickly
    let mut found_valid = false;
    for nonce in 0..100000 {
        block.nonce = nonce;
        let pow_hash = block.hash_for_pow();

        if Consensus::is_pow_valid(pow_hash.as_bytes(), block.difficulty) {
            println!("✅ Found valid nonce: {}", nonce);
            println!("Hash: {}", hex::encode(pow_hash.as_bytes()));
            found_valid = true;
            break;
        }

        if nonce % 10000 == 0 {
            println!("Tried {} nonces so far...", nonce);
        }
    }

    // With difficulty 0.0001, we should find a valid nonce relatively quickly
    assert!(
        found_valid,
        "Should find a valid nonce within 100k attempts for difficulty 0.0001"
    );
}

#[test]
fn test_target_calculation_precision() {
    println!("Testing target calculation precision");

    // Test various small difficulties
    let difficulties = vec![0.0001, 0.001, 0.01, 0.1, 1.0];

    for difficulty in difficulties {
        let target = Miner::calculate_target_from_difficulty(difficulty);
        println!(
            "Difficulty: {} -> Target: {}",
            difficulty,
            hex::encode(target)
        );

        // Target should not be all f's for any reasonable difficulty
        if difficulty < 1.0 {
            assert_ne!(
                target, [0xff; 32],
                "Target should not be all f's for difficulty {}",
                difficulty
            );
        }

        // Target should decrease as difficulty increases
        if difficulty > 0.0001 {
            let higher_target = Miner::calculate_target_from_difficulty(0.0001);
            // Higher difficulty should have lower target (harder to mine)
            assert!(
                target < higher_target,
                "Higher difficulty should have lower target"
            );
        }
    }
}
