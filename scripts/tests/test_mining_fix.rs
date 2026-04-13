use qanto::consensus::Consensus;
use qanto::miner::Miner;
use qanto::qantodag::QantoBlock;

fn main() {
    println!("Testing mining fix with difficulty 0.0001");

    // Create a dummy block with difficulty 0.0001
    let mut block = QantoBlock::new_test_block("test_block_1".to_string());
    block.difficulty = 0.0001;
    block.nonce = 0;

    println!("Block difficulty: {}", block.difficulty);

    // Calculate target from difficulty
    let target = Miner::calculate_target_from_difficulty(block.difficulty);
    println!("Target hash: {}", hex::encode(target));

    // Try a few nonces to see if we can find a valid one quickly
    let mut found_valid = false;
    for nonce in 0..1000000 {
        block.nonce = nonce;
        let pow_hash = block.hash_for_pow();

        if Consensus::is_pow_valid(pow_hash.as_bytes(), block.difficulty) {
            println!("✅ Found valid nonce: {}", nonce);
            println!("Hash: {}", hex::encode(pow_hash.as_bytes()));
            found_valid = true;
            break;
        }

        if nonce % 100000 == 0 {
            println!("Tried {} nonces so far...", nonce);
        }
    }

    if !found_valid {
        println!("❌ No valid nonce found in first 1M attempts");
    }
}
