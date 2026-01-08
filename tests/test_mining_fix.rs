use qanto::miner::Miner;
use qanto::qantodag::QantoBlock;

fn main() {
    println!("Testing mining fix with difficulty 0.0001");

    // Create a dummy block with easy header target
    let mut block = QantoBlock::new_test_block("test_block_1".to_string());
    let mut buf = [0u8; 32];
    primitive_types::U256::MAX.to_big_endian(&mut buf);
    block.target = Some(hex::encode(buf));
    block.nonce = 0;

    let target = hex::decode(block.target.clone().unwrap()).unwrap();
    println!("Target hash: {}", hex::encode(&target));

    // Try a few nonces to see if we can find a valid one quickly
    let mut found_valid = false;
    for nonce in 0..1000000 {
        block.nonce = nonce;
        let pow_hash = block.hash_for_pow();
        if Miner::hash_meets_target(pow_hash.as_bytes(), &target) {
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
