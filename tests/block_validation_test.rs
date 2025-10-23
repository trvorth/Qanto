#[cfg(test)]
mod tests {
    use qanto::mempool::Mempool;
    use qanto::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey};
    use qanto::qanto_storage::{QantoStorage, StorageConfig};
    use qanto::qantodag::{QantoDAG, QantoDagConfig};
    use qanto::saga::PalletSaga;
    use rand::rngs::OsRng;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tracing::info;

    fn create_test_keypair() -> (QantoPQPrivateKey, QantoPQPublicKey) {
        let mut rng = OsRng;
        let private_key = QantoPQPrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();
        (private_key, public_key)
    }

    #[tokio::test]
    async fn test_block_validation_with_saga_reward() {
        // Initialize logging
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init()
            .ok();

        info!("🚀 Starting block validation test with SAGA reward calculation");

        // Create storage instances for DAGs
        let storage_config1 = StorageConfig {
            data_dir: PathBuf::from("./test_data_validation1"),
            ..Default::default()
        };
        let _storage1 = QantoStorage::new(storage_config1).expect("Failed to create storage1");

        let storage_config2 = StorageConfig {
            data_dir: PathBuf::from("./test_data_validation2"),
            ..Default::default()
        };
        let _storage2 = QantoStorage::new(storage_config2).expect("Failed to create storage2");

        // Create SAGA instances
        let saga1 = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));
        let saga2 = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));

        // Set low difficulty mode for testing
        {
            let mut rules = saga1.economy.epoch_rules.write().await;
            if let Some(difficulty_rule) = rules.get_mut("base_difficulty") {
                difficulty_rule.value = 0.000001;
            }
        }
        {
            let mut rules = saga2.economy.epoch_rules.write().await;
            if let Some(difficulty_rule) = rules.get_mut("base_difficulty") {
                difficulty_rule.value = 0.000001;
            }
        }

        // Create two separate QantoDAG instances with separate storage
        let config1 = QantoDagConfig {
            initial_validator: "test_validator_1".to_string(),
            target_block_time: 60,
            num_chains: 1,
            dev_fee_rate: 0.10,
        };
        let config2 = QantoDagConfig {
            initial_validator: "test_validator_1".to_string(), // Use same validator as config1
            target_block_time: 60,
            num_chains: 1,
            dev_fee_rate: 0.10,
        };

        let storage1 = QantoStorage::new(StorageConfig {
            data_dir: "test_data1".into(),
            max_file_size: 64 * 1024 * 1024,
            compression_enabled: true,
            encryption_enabled: false,
            wal_enabled: true,
            sync_writes: true,
            cache_size: 1024 * 1024,
            compaction_threshold: 0.5,
            max_open_files: 100,
        })
        .expect("Failed to create storage1");
        let storage2 = QantoStorage::new(StorageConfig {
            data_dir: "test_data2".into(),
            max_file_size: 64 * 1024 * 1024,
            compression_enabled: true,
            encryption_enabled: false,
            wal_enabled: true,
            sync_writes: true,
            cache_size: 1024 * 1024,
            compaction_threshold: 0.5,
            max_open_files: 100,
        })
        .expect("Failed to create storage2");

        // Initialize two separate QantoDAG instances with separate storage
        let dag1 = QantoDAG::new(
            config1,
            saga1.clone(),
            storage1,
            qanto::config::LoggingConfig::default(),
        )
        .expect("Failed to create DAG1");

        let dag2 = QantoDAG::new(
            config2,
            saga2.clone(),
            storage2,
            qanto::config::LoggingConfig::default(),
        )
        .expect("Failed to create DAG2");

        // Add test validator to both DAGs with sufficient stake
        dag1.add_validator("test_validator".to_string(), 1_000_000)
            .await;
        dag2.add_validator("test_validator".to_string(), 1_000_000)
            .await;

        let mempool1 = Arc::new(RwLock::new(Mempool::new(3600, 10 * 1024 * 1024, 10000)));
        let utxos1 = Arc::new(RwLock::new(HashMap::new()));
        let utxos2 = Arc::new(RwLock::new(HashMap::new()));

        // Create a properly signed test block with valid proof-of-work
        let (private_key, public_key) = create_test_keypair();

        // Create block using DAG1's create_candidate_block method
        info!("🔨 Creating candidate block through DAG1");
        let mut block = dag1
            .create_candidate_block(
                &private_key,
                &public_key,
                "test_validator",
                &mempool1,
                &utxos1,
                0, // chain_id
                &Arc::new(
                    qanto::miner::Miner::new(qanto::miner::MinerConfig {
                        address: "7256ce7626c2f4b90770d3cc1d1cdd03f213627369600e7243d07c883c638710"
                            .to_string(),
                        threads: 1,
                        zk_enabled: false,
                        dag: dag1.clone(),
                        target_block_time: 10,
                        use_gpu: false,
                        logging_config: qanto::config::LoggingConfig::default(),
                    })
                    .expect("Failed to create miner"),
                ),
                None, // No homomorphic public key for tests
            )
            .await
            .expect("Failed to create candidate block");

        // Override difficulty for fast PoW solving
        block.difficulty = 0.000001;
        info!(
            "Block created with reward: {} and difficulty: {}",
            block.reward, block.difficulty
        );

        // Solve proof-of-work for the test block
        info!("🔨 Solving PoW for the test block");
        let miner_config = qanto::miner::MinerConfig {
            address: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            dag: dag1.clone(),
            target_block_time: 30,
            use_gpu: false,
            zk_enabled: false,
            threads: 4,
            logging_config: qanto::config::LoggingConfig::default(),
        };
        let miner = qanto::miner::Miner::new(miner_config).expect("Failed to create miner");

        let pow_start_time = std::time::Instant::now();
        let cancellation_token = tokio_util::sync::CancellationToken::new();
        miner
            .solve_pow_with_cancellation(&mut block, cancellation_token)
            .expect("Failed to solve proof-of-work");
        let pow_duration = pow_start_time.elapsed();
        info!("✅ PoW solving completed in {:?}", pow_duration);

        // Calculate rewards BEFORE adding the block to either DAG to ensure fair comparison
        info!("🔍 Calculating SAGA rewards before block addition");
        let total_fees = block.transactions.iter().map(|tx| tx.fee).sum::<u64>();

        let reward1 = saga1
            .calculate_dynamic_reward(&block, &dag1, total_fees)
            .await;
        info!("SAGA1 calculated reward: {:?}", reward1);

        let reward2 = saga2
            .calculate_dynamic_reward(&block, &dag2, total_fees)
            .await;
        info!("SAGA2 calculated reward: {:?}", reward2);

        // Add the block to DAG1 first
        info!("📝 Adding block to DAG1");
        dag1.add_block(block.clone(), &utxos1)
            .await
            .expect("Failed to add block to DAG1");
        info!("✅ Block successfully added to DAG1");

        // Now test if DAG2 can validate the same block
        info!("🔍 Testing block validation on DAG2");

        // Test block validation on DAG2
        let validation_result = dag2.is_valid_block(&block, &utxos2).await;
        match validation_result {
            Ok(true) => {
                info!("✅ Block validation successful on DAG2");

                // Try to add the block to DAG2
                match dag2.add_block(block.clone(), &utxos2).await {
                    Ok(_) => info!("✅ Block successfully added to DAG2"),
                    Err(e) => info!("❌ Failed to add block to DAG2: {}", e),
                }
            }
            Ok(false) => {
                info!("❌ Block validation failed on DAG2 (returned false)");
            }
            Err(e) => {
                info!("❌ Block validation error on DAG2: {}", e);
            }
        }

        // Compare SAGA states between the two instances
        info!("🔍 Comparing SAGA states:");
        let saga1_base_reward = saga1
            .economy
            .epoch_rules
            .read()
            .await
            .get("base_reward")
            .map_or(150_000_000_000.0, |r| r.value);
        let saga2_base_reward = saga2
            .economy
            .epoch_rules
            .read()
            .await
            .get("base_reward")
            .map_or(150_000_000_000.0, |r| r.value);

        info!("SAGA1 base reward: {}", saga1_base_reward);
        info!("SAGA2 base reward: {}", saga2_base_reward);

        // Check if the rewards match
        match (&reward1, &reward2) {
            (Ok(r1), Ok(r2)) => {
                if r1 == r2 {
                    info!(
                        "✅ SAGA reward calculations match between DAG1 and DAG2: {}",
                        r1
                    );
                } else {
                    info!(
                        "❌ SAGA reward calculations differ: DAG1={}, DAG2={}",
                        r1, r2
                    );
                }
            }
            _ => {
                info!(
                    "❌ One or both SAGA reward calculations failed: DAG1={:?}, DAG2={:?}",
                    reward1, reward2
                );
            }
        }

        // Cleanup
        std::fs::remove_dir_all("./test_data_validation1").ok();
        std::fs::remove_dir_all("./test_data_validation2").ok();
    }
}
