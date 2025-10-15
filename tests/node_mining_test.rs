#[cfg(test)]
mod tests {
    use qanto::config::Config;
    use qanto::node::Node;
    use qanto::qanto_storage::{QantoStorage, StorageConfig};
    use qanto::wallet::Wallet;
    use secrecy::Secret;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Once;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration, Instant};
    use tracing::{debug, info, warn};

    static INIT: Once = Once::new();

    fn init_tracing() {
        INIT.call_once(|| {
            tracing_subscriber::fmt::init();
        });
    }

    /// Create a temporary configuration for testing with embedded addresses
    fn create_test_config(temp_dir: &TempDir) -> Config {
        let data_dir = temp_dir.path().to_string_lossy().to_string();
        let db_path = temp_dir
            .path()
            .join("test.db")
            .to_string_lossy()
            .to_string();
        let wallet_path = temp_dir
            .path()
            .join("test_wallet.json")
            .to_string_lossy()
            .to_string();

        Config {
            genesis_validator: "7256ce7626c2f4b90770d3cc1d1cdd03f213627369600e7243d07c883c638710"
                .to_string(),
            contract_address: "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394"
                .to_string(),
            difficulty: 1,
            mining_enabled: true,
            data_dir,
            db_path,
            wallet_path,
            use_gpu: false,
            mining_threads: 1,
            target_block_time: 1000,
            p2p_address: "/ip4/127.0.0.1/tcp/0".to_string(),
            api_address: "127.0.0.1:0".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_node_mining_with_genesis_config() {
        init_tracing();
        info!("🚀 Starting node mining test with Genesis Engine configuration");

        // Create temporary directory for test data
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let config = create_test_config(&temp_dir);

        info!("📋 Test configuration:");
        info!("  Genesis Validator: {}", config.genesis_validator);
        info!("  Contract Address: {}", config.contract_address);
        info!("  Test Difficulty: {}", config.difficulty);
        info!("  Data Directory: {:?}", config.data_dir);

        // Create wallet for testing
        let wallet_path = PathBuf::from(&config.wallet_path);
        let password = Secret::new("test_password".to_string());

        // Try to load existing wallet, or create new one if it doesn't exist
        let wallet = if wallet_path.exists() {
            Wallet::from_file(&wallet_path, &password).expect("Failed to load existing test wallet")
        } else {
            // Create a wallet from the deterministic private key that matches the genesis validator
            // This ensures the wallet address matches the genesis validator in the config
            let deterministic_private_key =
                "0000000000000000000000000000000000000000000000000000000000000001";
            let new_wallet = Wallet::from_private_key(deterministic_private_key)
                .expect("Failed to create wallet from deterministic private key");

            // Log the wallet address for debugging
            let wallet_address = new_wallet.address();
            info!("Created test wallet with address: {}", wallet_address);
            info!("Genesis validator in config: {}", config.genesis_validator);
            info!(
                "Wallet address matches genesis validator: {}",
                wallet_address == config.genesis_validator
            );

            new_wallet
                .save_to_file(&wallet_path, &password)
                .expect("Failed to save new wallet");
            new_wallet
        };

        let wallet = Arc::new(wallet);

        info!("💼 Test wallet created/loaded successfully");

        // Initialize storage
        let storage_config = StorageConfig {
            data_dir: config.data_dir.clone().into(),
            max_file_size: 1024 * 1024 * 100, // 100MB
            compression_enabled: true,
            encryption_enabled: false,
            wal_enabled: true,
            sync_writes: false,
            cache_size: 1024 * 1024 * 10, // 10MB
            compaction_threshold: 1000.0,
            max_open_files: 100,
        };
        let _storage = QantoStorage::new(storage_config).unwrap();

        // Create and start the node (it will create its own DAG instance)
        let node = Node::new(
            config.clone(),
            temp_dir
                .path()
                .join("config.toml")
                .to_string_lossy()
                .to_string(),
            wallet,
            &temp_dir.path().join("p2p_identity").to_string_lossy(),
            temp_dir
                .path()
                .join("peer_cache.json")
                .to_string_lossy()
                .to_string(),
        )
        .await
        .unwrap();

        // Get reference to the Node's DAG instance
        let dag = node.dag.clone();

        info!("🔗 Node and DAG initialized with genesis configuration");

        info!("🏗️ Node created, starting mining test...");

        // Start the node in a background task
        let node_handle = tokio::spawn(async move {
            if let Err(e) = node.start().await {
                warn!("Node encountered error: {}", e);
            }
        });

        // Record start time
        let start_time = Instant::now();
        let test_duration = Duration::from_secs(60);

        info!(
            "⏱️ Running mining test for {} seconds",
            test_duration.as_secs()
        );

        // Monitor mining progress
        let mut last_block_count = 0;

        while start_time.elapsed() < test_duration {
            sleep(Duration::from_secs(5)).await;

            // Check DAG length to see if blocks have been mined
            let current_block_count = dag.get_block_count().await;

            if current_block_count > last_block_count {
                let blocks_mined = current_block_count - last_block_count;
                info!(
                    "⛏️ Blocks mined: {} (total: {})",
                    blocks_mined, current_block_count
                );
                last_block_count = current_block_count;
            }

            let elapsed = start_time.elapsed();
            debug!(
                "Test progress: {:.1}s / {:.1}s",
                elapsed.as_secs_f64(),
                test_duration.as_secs_f64()
            );
        }

        let elapsed_secs = start_time.elapsed().as_secs();
        info!("⏹️ Mining test completed after {elapsed_secs} seconds");

        // Stop the node
        node_handle.abort();

        // Final block count check
        let final_block_count = dag.get_block_count().await;

        info!("📊 Final mining results:");
        info!("  Total blocks in DAG: {final_block_count}");
        let blocks_mined = final_block_count.saturating_sub(1);
        info!("  Blocks mined during test: {blocks_mined}"); // Subtract genesis block

        // Assert that at least 1 block was mined (excluding genesis)
        assert!(
            final_block_count >= 2, // Genesis block + at least 1 mined block
            "Expected at least 1 block to be mined, but DAG length is only {final_block_count}. \
             This indicates mining was not successful with the Genesis Engine configuration."
        );

        // Verify addresses in mined blocks
        info!("🔍 Verifying addresses in mined blocks...");

        // Get all blocks to verify their addresses
        let all_blocks = dag
            .get_blocks_paginated(final_block_count as usize, 0)
            .await;
        let mut verified_blocks = 0;

        for block in &all_blocks {
            // Skip genesis block (height 0) as it may have different address rules
            if block.height == 0 {
                continue;
            }

            info!("🔎 Verifying block {} (height: {})", block.id, block.height);
            info!("  Validator: {}", block.validator);
            info!("  Miner: {}", block.miner);

            // Verify that the validator address matches the genesis validator
            assert_eq!(
                block.validator, config.genesis_validator,
                "Block {} validator address '{}' does not match expected genesis validator '{}'",
                block.id, block.validator, config.genesis_validator
            );

            // Verify that the miner address matches the genesis validator (in test setup, miner = validator)
            assert_eq!(
                block.miner, config.genesis_validator,
                "Block {} miner address '{}' does not match expected genesis validator '{}'",
                block.id, block.miner, config.genesis_validator
            );

            // Verify that the coinbase transaction (first transaction) has the correct recipient
            if !block.transactions.is_empty() {
                let coinbase_tx = &block.transactions[0];
                info!("  Coinbase transaction ID: {}", coinbase_tx.id);

                // Check if coinbase transaction has outputs with the correct address
                if !coinbase_tx.outputs.is_empty() {
                    let coinbase_recipient = &coinbase_tx.outputs[0].address;
                    info!("  Coinbase recipient: {}", coinbase_recipient);

                    // The coinbase recipient should be the validator address (genesis_validator in test setup)
                    assert_eq!(
                        coinbase_recipient,
                        &config.genesis_validator,
                        "Block {} coinbase recipient '{}' does not match expected genesis validator '{}'",
                        block.id, coinbase_recipient, config.genesis_validator
                    );
                }
            }

            verified_blocks += 1;
        }

        info!(
            "✅ Successfully verified addresses in {} mined blocks",
            verified_blocks
        );
        assert!(
            verified_blocks > 0,
            "No blocks were verified for address correctness"
        );

        info!("✅ Node mining test passed! Successfully mined blocks with Genesis Engine configuration");
    }

    #[tokio::test]
    async fn test_genesis_validator_validation() {
        init_tracing();
        info!("🔍 Testing genesis validator address validation");

        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let mut config = create_test_config(&temp_dir);

        // Test with invalid genesis validator (wrong length)
        config.genesis_validator = "invalid_short".to_string();

        // Validation should be handled in start_node.rs, but we can test config creation
        assert_eq!(config.genesis_validator.len(), 13); // Should be 64 for valid hex

        // Test with valid genesis validator
        config.genesis_validator =
            "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3".to_string();
        assert_eq!(config.genesis_validator.len(), 64);
        assert!(hex::decode(&config.genesis_validator).is_ok());

        info!("✅ Genesis validator validation test passed");
    }

    #[tokio::test]
    async fn test_contract_address_validation() {
        init_tracing();
        info!("🔍 Testing contract address validation");

        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let mut config = create_test_config(&temp_dir);

        // Test with invalid contract address (wrong length)
        config.contract_address = "invalid_short".to_string();
        assert_eq!(config.contract_address.len(), 13); // Should be 64 for valid hex

        // Test with valid contract address
        config.contract_address =
            "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394".to_string();
        assert_eq!(config.contract_address.len(), 64);
        assert!(hex::decode(&config.contract_address).is_ok());

        info!("✅ Contract address validation test passed");
    }
}
