use primitive_types::U256;
use qanto::miner::Miner;
use qanto::qantodag::QantoBlock;

#[tokio::test]
async fn test_mining_with_low_difficulty() {
    println!("Testing mining with easy header target");

    // Create a dummy block with easy header target
    let mut block = QantoBlock::new_test_block("test_block_1".to_string());
    let mut buf = [0u8; 32];
    U256::MAX.to_big_endian(&mut buf);
    block.target = Some(hex::encode(buf));
    block.nonce = 0;

    println!("Header target set to MAX");

    // Try a few nonces to see if we can find a valid one quickly
    let mut found_valid = false;
    for nonce in 0..100000 {
        block.nonce = nonce;
        let pow_hash = block.hash_for_pow();
        let target_bytes = hex::decode(block.target.clone().unwrap()).unwrap();
        if Miner::hash_meets_target(pow_hash.as_bytes(), &target_bytes) {
            println!("✅ Found valid nonce: {}", nonce);
            found_valid = true;
            break;
        }

        if nonce % 10000 == 0 {
            println!("Tried {} nonces so far...", nonce);
        }
    }

    // With easy target, we should find a valid nonce quickly
    assert!(
        found_valid,
        "Should find a valid nonce within 100k attempts for easy target"
    );
}
