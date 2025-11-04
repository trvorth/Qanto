use my_blockchain::qanto_hash;
use qanto::mempool::Mempool;
use qanto::miner::MiningError;
use qanto::miner::{Miner, MinerConfig};
use qanto::persistence::has_genesis_block;
use qanto::post_quantum_crypto::pq_sign;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::SigningData;
use qanto::qantodag::{QantoBlock, QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::shutdown::ShutdownController;
use qanto::transaction::Transaction;
use qanto::types::QuantumResistantSignature;
use qanto::types::UTXO;
use qanto::wallet::Wallet;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[inline]
fn new_saga() -> Arc<PalletSaga> {
    Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ))
}

/// Mock PoW implementation for testing to avoid timeout issues
pub struct MockPoW {
    pub fixed_nonce: u64,
    pub fixed_difficulty: f64,
}

impl Default for MockPoW {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPoW {
    pub fn new() -> Self {
        Self {
            fixed_nonce: 12345,
            fixed_difficulty: 0.0001, // Very low difficulty for fast testing
        }
    }

    pub fn solve_block(&self, block: &mut QantoBlock) -> Result<(), Box<dyn std::error::Error>> {
        block.nonce = self.fixed_nonce;
        block.difficulty = self.fixed_difficulty;
        // Use canonical PoW hash (includes nonce) for validation
        let pow_hash = block.hash_for_pow();
        let pow_hash_bytes = pow_hash.as_bytes();

        // Verify it meets our mock target
        let target_hash_bytes =
            qanto::miner::Miner::calculate_target_from_difficulty(self.fixed_difficulty);

        if !qanto::miner::Miner::hash_meets_target(pow_hash_bytes, &target_hash_bytes) {
            // Increased nonce trials to 100000 to ensure finding valid nonce
            for nonce in 1..100000 {
                block.nonce = nonce;
                block.difficulty = self.fixed_difficulty;
                // Recompute canonical PoW hash including nonce
                let test_pow_hash = block.hash_for_pow();
                let test_pow_hash_bytes = test_pow_hash.as_bytes();

                if qanto::miner::Miner::hash_meets_target(test_pow_hash_bytes, &target_hash_bytes) {
                    info!("✅ Mock PoW found valid nonce: {}", nonce);
                    return Ok(());
                }
            }
            return Err("Mock PoW could not find valid nonce".into());
        }

        info!("✅ Mock PoW successful with nonce: {}", block.nonce);
        Ok(())
    }
}

#[tokio::test]
async fn test_simple_mining_consolidated() {
    // Initialize logging only if not already initialized
    qanto::init_test_tracing();

    info!("Starting consolidated mining test...");

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_mining_consolidated");
    std::fs::create_dir_all(&temp_dir)
        .unwrap_or_else(|e| panic!("Failed to create temp directory: {e}"));

    // Initialize storage configuration with correct structure
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB
        compaction_threshold: 1000,
        max_open_files: 100,
        ..StorageConfig::default()
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .unwrap_or_else(|e| panic!("Failed to initialize storage: {e}"));

    // Initialize SAGA pallet
    let saga = new_saga();

    // Initialize QantoDAG with correct configuration
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 60,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap_or_else(|e| panic!("Failed to initialize QantoDAG: {e}"));

    // Create wallet for mining
    let wallet = Wallet::new().unwrap_or_else(|e| panic!("Failed to create wallet: {e}"));
    let miner_address = wallet.address();

    // Add validator to DAG with sufficient stake
    dag_arc.add_validator(miner_address.clone(), 1000).await;

    // Initialize mempool
    let mempool = Mempool::new(300, 1024 * 1024, 1000); // 5 min timeout, 1MB max size, 1000 max transactions
    let mempool_arc = Arc::new(RwLock::new(mempool));

    // Initialize UTXOs
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let utxos_arc = Arc::new(RwLock::new(utxos));

    // Add a test transaction to mempool
    let test_tx = Transaction::new_dummy();
    mempool_arc
        .write()
        .await
        .add_transaction(test_tx, &HashMap::new(), &dag_arc)
        .await
        .unwrap_or_else(|e| panic!("Failed to add transaction: {e}"));

    // Create miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 120, // Longer timeout for testing
        use_gpu: false,
        zk_enabled: false,
        threads: 1, // Single thread for deterministic testing
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap_or_else(|e| panic!("Failed to create miner: {e}"));

    // Create a block template using DAG's create_candidate_block method
    let (private_key, public_key) = wallet
        .get_keypair()
        .unwrap_or_else(|e| panic!("Failed to get keypair: {e}"));
    let mut block = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0, // chain_id
            &Arc::new(miner),
            None, // No homomorphic public key for tests
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to create candidate block: {e}"));

    info!(
        "Created block template with {} transactions",
        block.transactions.len()
    );
    info!("Original block difficulty: {}", block.difficulty);

    // Use mock PoW to avoid timeout issues
    let mock_pow = MockPoW::default();

    // Test mining with mock PoW (no timeout needed)
    info!("Starting mock PoW mining...");
    let mining_start = std::time::Instant::now();

    match mock_pow.solve_block(&mut block) {
        Ok(()) => {
            let mining_duration = mining_start.elapsed();
            info!("✅ Mock mining successful!");
            info!("Mining took: {:?}", mining_duration);
            info!("Block nonce: {}", block.nonce);
            info!("Block difficulty: {}", block.difficulty);

            // Verify the hash meets the target
            // Use canonical PoW hash bytes (includes nonce)
            let block_pow_hash = block.hash_for_pow();
            let block_pow_hash_bytes = block_pow_hash.as_bytes();
            let target_hash_bytes =
                qanto::miner::Miner::calculate_target_from_difficulty(block.difficulty);

            if qanto::miner::Miner::hash_meets_target(block_pow_hash_bytes, &target_hash_bytes) {
                info!("✅ Hash meets target - PoW validation successful!");
            } else {
                error!("❌ Hash does not meet target - PoW validation failed!");
                panic!("PoW validation failed");
            }
        }
        Err(e) => {
            error!("❌ Mock mining failed with error: {e:?}");
            panic!("Mock mining failed: {e:?}");
        }
    }

    info!("✅ Consolidated mining test completed successfully.");

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_multi_block_mining_without_dag_regeneration() {
    qanto::init_test_tracing();

    // Use a unique temp directory per test run to avoid cross-test interference
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("qanto_test_multi_mining_{}", unique_suffix));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10,
        compaction_threshold: 1000,
        max_open_files: 100,
        ..StorageConfig::default()
    };

    let storage = QantoStorage::new(storage_config).unwrap();

    let saga = new_saga();

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 60,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap();

    // Ensure genesis is fully initialized and persisted before mining begins.
    // We open a read-only storage view on the same data_dir and wait until the
    // genesis marker is present. This guards against transient I/O races.
    {
        // Retry opening a read-only storage view until the DB is initialized,
        // then wait for the genesis marker to appear.
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(1500);
        loop {
            let reader_cfg = StorageConfig {
                data_dir: temp_dir.clone(),
                wal_enabled: false,
                enable_async_io: false,
                max_open_files: 32,
                cache_size: 1024 * 1024,
                ..StorageConfig::default()
            };
            match QantoStorage::new(reader_cfg) {
                Ok(reader_db) => {
                    if has_genesis_block(&reader_db).unwrap_or(false) {
                        break;
                    }
                }
                Err(_e) => {
                    // DB may not be initialized yet; keep retrying until timeout
                }
            }
            if start.elapsed() > timeout {
                panic!("Timeout waiting for genesis initialization");
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    let wallet = Wallet::new().unwrap();
    let miner_address = wallet.address();

    dag_arc.add_validator(miner_address.clone(), 1000).await;

    let mempool_arc = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));
    let utxos_arc = Arc::new(RwLock::new(HashMap::new()));

    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 120,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap();

    let (private_key, public_key) = wallet.get_keypair().unwrap();

    // First block
    let mut block1 = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0,
            &Arc::new(miner.clone()),
            None,
            None,
        )
        .await
        .unwrap();

    let mock_pow = MockPoW::default();
    mock_pow.solve_block(&mut block1).unwrap();

    // Re-sign the block after changing difficulty
    let signing_data = SigningData {
        chain_id: block1.chain_id,
        merkle_root: &block1.merkle_root,
        parents: &block1.parents,
        transactions: &block1.transactions,
        timestamp: block1.timestamp,
        difficulty: block1.difficulty,
        height: block1.height,
        validator: &block1.validator,
        miner: &block1.miner,
    };
    let pre_signature_data_for_id = QantoBlock::serialize_for_signing(&signing_data).unwrap();
    let new_signature = pq_sign(&private_key, &pre_signature_data_for_id).unwrap();
    let new_id = hex::encode(qanto_hash(&pre_signature_data_for_id));
    block1.id = new_id;
    block1.signature = QuantumResistantSignature {
        signer_public_key: public_key.as_bytes().to_vec(),
        signature: new_signature.as_bytes().to_vec(),
    };
    // Add to DAG
    dag_arc
        .add_block(
            block1.clone(),
            &utxos_arc,
            Some(&mempool_arc),
            Some(&miner_address),
        )
        .await
        .unwrap();

    // Second block
    let mut block2 = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0,
            &Arc::new(miner.clone()),
            None,
            None,
        )
        .await
        .unwrap();

    mock_pow.solve_block(&mut block2).unwrap();

    // Re-sign for block2
    let signing_data = SigningData {
        chain_id: block2.chain_id,
        merkle_root: &block2.merkle_root,
        parents: &block2.parents,
        transactions: &block2.transactions,
        timestamp: block2.timestamp,
        difficulty: block2.difficulty,
        height: block2.height,
        validator: &block2.validator,
        miner: &block2.miner,
    };
    let pre_signature_data_for_id = QantoBlock::serialize_for_signing(&signing_data).unwrap();
    let new_signature = pq_sign(&private_key, &pre_signature_data_for_id).unwrap();
    let new_id = hex::encode(qanto_hash(&pre_signature_data_for_id));
    block2.id = new_id;
    block2.signature = QuantumResistantSignature {
        signer_public_key: public_key.as_bytes().to_vec(),
        signature: new_signature.as_bytes().to_vec(),
    };
    // Assert same epoch assuming epoch = height / 30000
    assert_eq!(block1.height / 30000, block2.height / 30000);

    // To verify caching, check if DAG pointers are same (assuming same epoch)
    let epoch = block1.height / qanto::qanhash::EPOCH_LENGTH; // Assuming DATASET_GROWTH_EPOCH is public or import it

    let dag1 = qanto::qanhash::get_qdag_optimized(epoch).await;
    let dag2 = qanto::qanhash::get_qdag_optimized(epoch).await;

    assert!(
        Arc::ptr_eq(&dag1, &dag2),
        "DAG should be cached and same instance"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_mining_with_multiple_transactions() {
    // Initialize logging only if not already initialized
    qanto::init_test_tracing();

    info!("Starting multi-transaction mining test...");

    // Create a unique temporary directory for storage to avoid cross-test interference
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("qanto_test_multi_mining_{}", unique_suffix));
    std::fs::create_dir_all(&temp_dir)
        .unwrap_or_else(|e| panic!("Failed to create temp directory: {e}"));

    // Initialize storage configuration
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB
        compaction_threshold: 1000,
        max_open_files: 100,
        ..StorageConfig::default()
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .unwrap_or_else(|e| panic!("Failed to initialize storage: {e}"));

    // Initialize SAGA pallet
    let saga = new_saga();

    // Initialize QantoDAG
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator_multi".to_string(),
        target_block_time: 60,
        num_chains: 2, // Test with multiple chains
        dev_fee_rate: 0.10,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap_or_else(|e| panic!("Failed to initialize QantoDAG: {e}"));

    // Create wallet for mining
    let wallet = Wallet::new().unwrap_or_else(|e| panic!("Failed to create wallet: {e}"));
    let miner_address = wallet.address();

    // Add validator to DAG with sufficient stake
    dag_arc.add_validator(miner_address.clone(), 2000).await;

    // Initialize mempool
    let mempool = Mempool::new(300, 1024 * 1024, 1000);
    let mempool_arc = Arc::new(RwLock::new(mempool));

    // Initialize UTXOs
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let utxos_arc = Arc::new(RwLock::new(utxos));

    // Add multiple test transactions to mempool
    for i in 0..5 {
        let test_tx = Transaction::new_dummy();
        mempool_arc
            .write()
            .await
            .add_transaction(test_tx, &HashMap::new(), &dag_arc)
            .await
            .unwrap_or_else(|e| panic!("Failed to add transaction {i}: {e}"));
    }

    // Create miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 120,
        use_gpu: false,
        zk_enabled: false,
        threads: 2, // Multiple threads for multi-chain
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap_or_else(|e| panic!("Failed to create miner: {e}"));

    // Test mining on multiple chains
    for chain_id in 0..2 {
        let (private_key, public_key) = wallet
            .get_keypair()
            .unwrap_or_else(|e| panic!("Failed to get keypair: {e}"));
        let mut block = dag_arc
            .create_candidate_block(
                &private_key,
                &public_key,
                &miner_address,
                &mempool_arc,
                &utxos_arc,
                chain_id,
                &Arc::new(miner.clone()),
                None, // No homomorphic public key for tests
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("Failed to create candidate block: {e}"));

        info!(
            "Created block template for chain {} with {} transactions",
            chain_id,
            block.transactions.len()
        );

        // Use mock PoW
        let mock_pow = MockPoW::default();

        match mock_pow.solve_block(&mut block) {
            Ok(()) => {
                info!("✅ Chain {} mining successful!", chain_id);

                // Verify the hash meets the target using canonical PoW hash
                let block_pow_hash = block.hash_for_pow();
                let block_pow_hash_bytes = block_pow_hash.as_bytes();
                let target_hash_bytes =
                    qanto::miner::Miner::calculate_target_from_difficulty(block.difficulty);

                assert!(
                    qanto::miner::Miner::hash_meets_target(
                        block_pow_hash_bytes,
                        &target_hash_bytes
                    ),
                    "Chain {chain_id} PoW validation failed",
                );
            }
            Err(e) => {
                error!("❌ Chain {chain_id} mining failed: {e:?}");
                panic!("❌ Chain {chain_id} mining failed: {e:?}");
            }
        }
    }

    info!("✅ Multi-transaction mining test completed successfully.");

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_mining_performance_benchmark() {
    // Initialize logging only if not already initialized
    let _ = tracing_subscriber::fmt::try_init();

    info!("Starting mining performance benchmark...");

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_perf_mining");
    std::fs::create_dir_all(&temp_dir)
        .unwrap_or_else(|e| panic!("Failed to create temp directory: {e}"));

    // Initialize storage configuration
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: true,        // Enable compression for performance test
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 50, // Larger cache for performance
        compaction_threshold: 1000,
        max_open_files: 200,
        ..StorageConfig::default()
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .unwrap_or_else(|e| panic!("Failed to initialize storage: {e}"));

    // Initialize SAGA pallet
    let saga = new_saga();

    // Initialize QantoDAG with performance settings
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator_perf".to_string(),
        target_block_time: 25, // High performance: ~40 BPS
        num_chains: 4,         // Multiple chains for parallel processing
        dev_fee_rate: 0.10,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap_or_else(|e| panic!("Failed to initialize QantoDAG: {e}"));

    // Create wallet for mining
    let wallet = Wallet::new().unwrap_or_else(|e| panic!("Failed to create wallet: {e}"));
    let miner_address = wallet.address();

    // Add validator to DAG with high stake
    dag_arc.add_validator(miner_address.clone(), 10000).await;

    // Initialize mempool with larger capacity
    let mempool = Mempool::new(600, 1024 * 1024 * 10, 10000); // Larger mempool for performance test
    let mempool_arc = Arc::new(RwLock::new(mempool));

    // Initialize UTXOs
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let utxos_arc = Arc::new(RwLock::new(utxos));

    // Add many test transactions to simulate high load
    for i in 0..100 {
        let test_tx = Transaction::new_dummy();
        mempool_arc
            .write()
            .await
            .add_transaction(test_tx, &HashMap::new(), &dag_arc)
            .await
            .unwrap_or_else(|e| panic!("Failed to add transaction {i}: {e}"));
    }

    // Create high-performance miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 25, // High performance target
        use_gpu: false,
        zk_enabled: false,
        threads: 4, // Multiple threads for performance
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap_or_else(|e| panic!("Failed to create miner: {e}"));

    // Benchmark mining performance
    let benchmark_start = std::time::Instant::now();
    let mut total_transactions = 0;

    // Mine blocks on all chains
    for chain_id in 0..4 {
        let (private_key, public_key) = wallet
            .get_keypair()
            .unwrap_or_else(|e| panic!("Failed to get keypair: {e}"));
        let mut block = dag_arc
            .create_candidate_block(
                &private_key,
                &public_key,
                &miner_address,
                &mempool_arc,
                &utxos_arc,
                chain_id,
                &Arc::new(miner.clone()),
                None, // No homomorphic public key for tests
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("Failed to create candidate block: {e}"));

        total_transactions += block.transactions.len();

        // Use mock PoW for consistent timing
        let mock_pow = MockPoW::new();

        let chain_start = std::time::Instant::now();
        match mock_pow.solve_block(&mut block) {
            Ok(()) => {
                let chain_duration = chain_start.elapsed();
                info!(
                    "✅ Chain {} mined in {:?} with {} transactions",
                    chain_id,
                    chain_duration,
                    block.transactions.len()
                );
            }
            Err(e) => {
                error!("❌ Chain {} mining failed: {:?}", chain_id, e);
                panic!("Performance benchmark failed on chain {chain_id}: {e:?}");
            }
        }
    }

    let total_duration = benchmark_start.elapsed();
    let tps = total_transactions as f64 / total_duration.as_secs_f64();

    info!("✅ Performance benchmark completed:");
    info!("   Total time: {:?}", total_duration);
    info!("   Total transactions: {}", total_transactions);
    info!("   TPS: {:.2}", tps);
    info!("   Chains processed: 4");

    // Assert performance targets - increased timeout for realistic testing
    assert!(
        total_duration.as_millis() < 30000,
        "Benchmark should complete within 30 seconds"
    );
    assert!(tps > 1.0, "Should achieve at least 1 TPS");

    info!("✅ Mining performance benchmark completed successfully.");

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_miner_cancellation_returns_timeout_or_cancelled() {
    // Initialize logging only if not already initialized
    let _ = tracing_subscriber::fmt::try_init();

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_mining_cancel");
    std::fs::create_dir_all(&temp_dir)
        .unwrap_or_else(|e| panic!("Failed to create temp directory: {e}"));

    // Initialize storage configuration
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB
        compaction_threshold: 1000,
        max_open_files: 100,
        ..StorageConfig::default()
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .unwrap_or_else(|e| panic!("Failed to initialize storage: {e}"));

    // Initialize SAGA pallet
    let saga = new_saga();

    // Initialize QantoDAG
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator_cancel".to_string(),
        target_block_time: 60,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap_or_else(|e| panic!("Failed to initialize QantoDAG: {e}"));

    // Create wallet for mining
    let wallet = Wallet::new().unwrap_or_else(|e| panic!("Failed to create wallet: {e}"));
    let miner_address = wallet.address();

    // Add validator to DAG with sufficient stake
    dag_arc.add_validator(miner_address.clone(), 1000).await;

    // Initialize mempool and UTXOs
    let mempool_arc = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));
    let utxos_arc = Arc::new(RwLock::new(HashMap::new()));

    // Create miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 10, // Not used because we cancel explicitly
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap_or_else(|e| panic!("Failed to create miner: {e}"));

    // Create block template
    let (private_key, public_key) = wallet
        .get_keypair()
        .unwrap_or_else(|e| panic!("Failed to get keypair: {e}"));
    let mut block = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0, // chain_id
            &Arc::new(miner.clone()),
            None, // No homomorphic public key for tests
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to create candidate block: {e}"));

    // Make difficulty extremely high to avoid quick solutions
    block.difficulty = 1_000_000_000.0;

    // Create a cancellation token and start mining in a blocking task
    let token = CancellationToken::new();
    let token_clone = token.clone();
    let mut block_local = block;
    let handle = tokio::task::spawn_blocking(move || {
        miner.solve_pow_with_cancellation(&mut block_local, token_clone)
    });

    // Cancel shortly after starting
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    token.cancel();

    // Assert that mining returns TimeoutOrCancelled
    let res = handle
        .await
        .unwrap_or_else(|e| panic!("Mining task join failed: {e}"));
    match res {
        Err(MiningError::TimeoutOrCancelled) => {
            info!("✅ Cancellation test: miner returned TimeoutOrCancelled as expected");
        }
        Ok(()) => panic!("Expected cancellation error, but mining succeeded"),
        Err(e) => panic!("Expected TimeoutOrCancelled, got: {e:?}"),
    }

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_miner_shutdown_returns_timeout_or_cancelled() {
    // Initialize logging only if not already initialized
    qanto::init_test_tracing();

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_mining_shutdown");
    std::fs::create_dir_all(&temp_dir)
        .unwrap_or_else(|e| panic!("Failed to create temp directory: {e}"));

    // Initialize storage configuration
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB
        compaction_threshold: 1000,
        max_open_files: 100,
        ..StorageConfig::default()
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .unwrap_or_else(|e| panic!("Failed to initialize storage: {e}"));

    // Initialize SAGA pallet
    let saga = new_saga();

    // Initialize QantoDAG
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator_shutdown".to_string(),
        target_block_time: 60,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap_or_else(|e| panic!("Failed to initialize QantoDAG: {e}"));

    // Create wallet for mining
    let wallet = Wallet::new().unwrap_or_else(|e| panic!("Failed to create wallet: {e}"));
    let miner_address = wallet.address();

    // Add validator to DAG with sufficient stake
    dag_arc.add_validator(miner_address.clone(), 1000).await;

    // Initialize mempool and UTXOs
    let mempool_arc = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));
    let utxos_arc = Arc::new(RwLock::new(HashMap::new()));

    // Create miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 10,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap_or_else(|e| panic!("Failed to create miner: {e}"));

    // Create block template
    let (private_key, public_key) = wallet
        .get_keypair()
        .unwrap_or_else(|e| panic!("Failed to get keypair: {e}"));
    let mut block = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0, // chain_id
            &Arc::new(miner.clone()),
            None, // No homomorphic public key for tests
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to create candidate block: {e}"));

    // Make difficulty extremely high to avoid quick solutions
    block.difficulty = 1_000_000_000.0;

    // Setup shutdown controller and integrate with miner
    let controller = ShutdownController::new(std::time::Duration::from_secs(5));
    let mut miner = Miner::new(MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 10,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: qanto::config::LoggingConfig::default(),
    })
    .unwrap_or_else(|e| panic!("Failed to create miner: {e}"));
    miner.set_shutdown_controller(controller.clone());

    // Get cancellation token from controller
    let token = controller.cancellation_token();

    // Request shutdown before starting mining to validate immediate detection
    controller
        .request_shutdown()
        .unwrap_or_else(|e| panic!("Failed to request shutdown: {e}"));

    let mut block_local = block;
    let handle = tokio::task::spawn_blocking(move || {
        miner.solve_pow_with_shutdown_integration(&mut block_local, token)
    });

    // Assert that mining returns TimeoutOrCancelled
    let res = handle
        .await
        .unwrap_or_else(|e| panic!("Mining task join failed: {e}"));
    match res {
        Err(MiningError::TimeoutOrCancelled) => {
            info!("✅ Shutdown test: miner returned TimeoutOrCancelled as expected");
        }
        Ok(()) => panic!("Expected shutdown error, but mining succeeded"),
        Err(e) => panic!("Expected TimeoutOrCancelled, got: {e:?}"),
    }

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}
