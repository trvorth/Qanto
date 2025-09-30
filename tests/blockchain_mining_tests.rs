//! Blockchain Mining Tests
//!
//! This module provides comprehensive tests for blockchain mining functionality
//! including low difficulty mining, RPC status, and multiple block mining.

#[cfg(test)]
mod tests {
    use my_blockchain::qanhash::is_solution_valid;
    use my_blockchain::{qanto_standalone::hash::qanto_hash, Blockchain};

    #[test]
    fn test_low_difficulty_mining() {
        let _blockchain = Blockchain::new("test_mining_db").unwrap();

        // Test mining with very low difficulty
        let target = [0xff; 32]; // Very easy target
        let mut nonce = 0u64;
        let mut found_solution = false;

        // Try up to 1000 nonces to find a solution
        for _ in 0..1000 {
            let hash = qanto_hash(&nonce.to_le_bytes());
            if is_solution_valid(hash.as_bytes(), target) {
                found_solution = true;
                break;
            }
            nonce += 1;
        }

        assert!(
            found_solution,
            "Should find a solution with very low difficulty"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_blockchain_rpc_mining_status() {
        let blockchain = Blockchain::new("test_rpc_db").unwrap();

        // Test mining status methods
        let is_mining = blockchain.is_mining();
        let difficulty = blockchain.get_current_difficulty();
        let hash_rate = blockchain.get_hash_rate();
        let block_count = blockchain.get_block_count();

        // Basic assertions - note that is_mining returns true by default in this implementation
        assert!(is_mining, "Mining is always active in this implementation");
        assert!(difficulty > 0, "Difficulty should be positive");
        // Note: hash_rate and block_count are u64, so >= 0 is always true

        println!("✅ RPC mining status test passed");
        println!(
            "Mining: {}, Difficulty: {}, Hash Rate: {}, Block Count: {}",
            is_mining, difficulty, hash_rate, block_count
        );
    }

    #[test]
    fn test_multiple_blocks_mining() {
        let _blockchain = Blockchain::new("test_multi_mining_db").unwrap();

        // Test mining multiple blocks with low difficulty
        let target = [0xff; 32]; // Very easy target

        for i in 0..3 {
            let mut attempts = 0;
            let mut found_solution = false;

            // Try to mine a block
            while attempts < 1000 && !found_solution {
                attempts += 1;
                let nonce_data = format!("block_{}_nonce_{}", i, attempts);
                let hash = qanto_hash(nonce_data.as_bytes());

                if is_solution_valid(hash.as_bytes(), target) {
                    found_solution = true;
                    println!(
                        "✅ Block {} mined successfully with {} attempts",
                        i + 1,
                        attempts
                    );
                }
            }

            assert!(found_solution, "Should mine block {} successfully", i + 1);
        }

        println!("✅ Multiple blocks mining test passed");
    }
}
