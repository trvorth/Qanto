use qanto::config::LoggingConfig;
use qanto::miner::{Miner, MinerConfig, MiningError};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::transaction::Transaction;
use qanto::types::QuantumResistantSignature;
use serial_test::serial;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

// Helper function to create test blocks with configurable difficulty
fn create_test_block_with_difficulty(
    block_id: String,
    difficulty: f64,
) -> qanto::qantodag::QantoBlock {
    let dummy_tx = Transaction::new_dummy();
    let transactions = vec![dummy_tx];
    let merkle_root =
        qanto::qantodag::QantoBlock::compute_merkle_root(&transactions).unwrap_or_default();

    qanto::qantodag::QantoBlock {
        chain_id: 0,
        id: block_id,
        parents: vec![],
        transactions,
        difficulty,
        validator: "test_validator".to_string(),
        miner: "test_miner".to_string(),
        nonce: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        height: 1,
        reward: 0,
        effort: 0,
        cross_chain_references: vec![],
        merkle_root,
        cross_chain_swaps: vec![],
        homomorphic_encrypted: vec![],
        smart_contracts: vec![],
        signature: QuantumResistantSignature {
            signer_public_key: vec![0; 32],
            signature: vec![0; 64],
        },
        carbon_credentials: vec![],
        epoch: 0,
        finality_proof: None,
        reservation_miner_id: None,
    }
}

/// Helper function to create a test DAG with optimized settings for fast tests
fn create_test_dag() -> Arc<QantoDAG> {
    // Use a unique temp directory per test run to avoid interference and reduce I/O overhead
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data_dir = std::env::temp_dir().join(format!("test_qanto_miner_timing_{}", unique_suffix));
    std::fs::create_dir_all(&data_dir).expect("Failed to create temp storage directory");

    let storage_config = StorageConfig {
        data_dir,
        max_file_size: 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024,
        compaction_threshold: 100,
        max_open_files: 10,
        ..StorageConfig::default()
    };

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 1000,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let logging_config = LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false,
        celebration_log_level: "info".to_string(),
        celebration_throttle_per_min: Some(10),
    };

    let saga_pallet = Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ));

    QantoDAG::new(dag_config, saga_pallet, storage, logging_config).expect("Failed to create DAG")
}

fn create_test_miner() -> Miner {
    let dag = create_test_dag();

    let config = MinerConfig {
        address: "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3".to_string(),
        dag,
        target_block_time: 1000,
        use_gpu: false,
        zk_enabled: false,
        threads: 1, // Single thread for predictable test behavior
        logging_config: LoggingConfig {
            level: "info".to_string(),
            enable_block_celebrations: false,
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: Some(10),
        },
    };

    Miner::new(config).expect("Failed to create miner")
}

/// Test that pre-cancelled tokens are handled immediately
#[tokio::test]
#[serial]
async fn test_pre_cancelled_token() {
    let miner = create_test_miner();
    let mut block =
        qanto::qantodag::QantoBlock::new_test_block("test_block_pre_cancel".to_string());

    // Create and immediately cancel the token
    let token = CancellationToken::new();
    token.cancel();

    let start = SystemTime::now();
    let result = miner.solve_pow_with_cancellation(&mut block, token);
    let elapsed = start.elapsed().unwrap();

    // Should return immediately (< 10ms) with cancellation error
    assert!(elapsed < Duration::from_millis(10));
    assert!(matches!(result, Err(MiningError::TimeoutOrCancelled)));
}

/// Test cancellation during mining setup phase
#[tokio::test]
#[serial]
async fn test_cancellation_during_setup() {
    let dag = create_test_dag();
    let miner = Miner::new(MinerConfig {
        address: "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3".to_string(),
        dag,
        target_block_time: 1000,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig {
            level: "info".to_string(),
            enable_block_celebrations: false,
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: Some(10),
        },
    })
    .expect("Failed to create miner");
    let cancellation_token = CancellationToken::new();

    // Create a block with high difficulty to ensure mining takes time
    let block = create_test_block_with_difficulty("test_block_setup".to_string(), 1000000.0);

    // Clone the token for cancellation
    let cancel_token = cancellation_token.clone();

    // Start mining in a separate task
    let mining_task = {
        let mut block_clone = block.clone();
        let token_clone = cancellation_token.clone();
        tokio::spawn(
            async move { miner.solve_pow_with_cancellation(&mut block_clone, token_clone) },
        )
    };

    // Cancel immediately
    cancel_token.cancel();

    // Wait for the mining task to complete with a short timeout
    let result = tokio::time::timeout(Duration::from_secs(5), mining_task).await;

    match result {
        Ok(Ok(mining_result)) => {
            // Mining task completed - should be cancelled
            match mining_result {
                Err(MiningError::TimeoutOrCancelled) => {
                    // Success - cancellation worked
                }
                Ok(_) => panic!("Expected cancellation but mining succeeded"),
                Err(other) => panic!("Expected TimeoutOrCancelled but got: {other:?}"),
            }
        }
        Ok(Err(_)) => panic!("Mining task panicked"),
        Err(_) => {
            // Timeout occurred - this suggests cancellation isn't working properly
            panic!("Mining task timed out - cancellation may not be working");
        }
    }
}

/// Test cancellation during active mining
#[tokio::test]
#[serial]
async fn test_cancellation_during_mining() {
    let miner = Miner::new(MinerConfig {
        address: "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3".to_string(),
        dag: create_test_dag(),
        target_block_time: 60,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig {
            level: "info".to_string(),
            enable_block_celebrations: false,
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: Some(10),
        },
    })
    .expect("Failed to create miner");
    let cancellation_token = CancellationToken::new();

    // Create a block with extremely high difficulty to ensure mining takes a very long time
    let mut block = create_test_block_with_difficulty("test_block_mining".to_string(), 1000000.0);

    // Clone the token for cancellation
    let cancel_token = cancellation_token.clone();

    // Start mining in a separate task
    let mining_task =
        tokio::spawn(
            async move { miner.solve_pow_with_cancellation(&mut block, cancellation_token) },
        );

    // Cancel immediately
    cancel_token.cancel();

    // Wait for the mining task to complete with a short timeout
    let result = tokio::time::timeout(Duration::from_secs(5), mining_task).await;

    match result {
        Ok(Ok(mining_result)) => {
            // Mining task completed - should be cancelled
            match mining_result {
                Err(MiningError::TimeoutOrCancelled) => {
                    // Success - cancellation worked
                }
                Ok(_) => panic!("Expected cancellation but mining succeeded"),
                Err(other) => panic!("Expected TimeoutOrCancelled but got: {other:?}"),
            }
        }
        Ok(Err(_)) => panic!("Mining task panicked"),
        Err(_) => {
            // Timeout occurred - this suggests cancellation isn't working properly
            panic!("Mining task timed out - cancellation may not be working");
        }
    }
}

/// Test multiple rapid cancellations
#[tokio::test]
#[serial]
async fn test_rapid_cancellations() {
    let miner = create_test_miner();

    for i in 0..5 {
        let mut block =
            qanto::qantodag::QantoBlock::new_test_block(format!("test_block_rapid_{i}"));

        let token = CancellationToken::new();
        token.cancel(); // Pre-cancel each one

        let start = SystemTime::now();
        let result = miner.solve_pow_with_cancellation(&mut block, token);
        let elapsed = start.elapsed().unwrap();

        // Each should return immediately
        assert!(elapsed < Duration::from_millis(5));
        assert!(matches!(result, Err(MiningError::TimeoutOrCancelled)));
    }
}

/// Test that cancellation doesn't interfere with successful mining
#[tokio::test]
#[serial]
async fn test_no_cancellation_allows_success() {
    let miner = create_test_miner();
    let mut block = qanto::qantodag::QantoBlock::new_test_block("test_block_success".to_string());

    // Set very low difficulty for quick success
    block.difficulty = 0.5;

    let token = CancellationToken::new();
    // Don't cancel the token

    let start = SystemTime::now();
    let result = miner.solve_pow_with_cancellation(&mut block, token);
    let elapsed = start.elapsed().unwrap();

    // Should succeed within reasonable time with low difficulty; allow some jitter under CI
    assert!(elapsed < Duration::from_secs(12));
    assert!(result.is_ok());
    assert!(block.nonce > 0); // Nonce should be set
}
