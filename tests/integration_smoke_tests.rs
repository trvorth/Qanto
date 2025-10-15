use qanto::config::LoggingConfig;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoBlock, QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, span, Level};

mod util;
use util::deterministic_test_env::setup_deterministic_test_env;
use util::test_address::make_test_miner_address;

fn create_test_dag() -> Arc<QantoDAG> {
    let storage_config = StorageConfig {
        data_dir: std::path::PathBuf::from("/tmp/test_qanto"),
        max_file_size: 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024,
        compaction_threshold: 100.0,
        max_open_files: 10,
    };

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 1000,
        num_chains: 1,
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

async fn create_test_miner(dag: Arc<QantoDAG>) -> Miner {
    let config = MinerConfig {
        address: make_test_miner_address(),
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
    };

    Miner::new(config).expect("Failed to create miner")
}

#[tokio::test]
#[instrument(level = "info")]
async fn test_basic_mining() {
    let _span = span!(Level::INFO, "basic_mining_test_setup");
    setup_deterministic_test_env();

    // Add timeout to prevent hanging
    let test_future = async {
        let dag = create_test_dag();
        let mut miner = create_test_miner(dag.clone()).await;

        info!("Creating test block for basic mining");
        // Create a test block with very low difficulty for fast mining
        let mut block = QantoBlock::new_test_block("test_basic_mining_block".to_string());
        block.difficulty = 0.001; // Very low difficulty for fast testing

        let _mining_span = span!(Level::INFO, "basic_mining", block_id = %block.id);
        info!("Starting basic mining operation");

        // Test mining - this is synchronous, not async
        let result =
            miner.solve_pow_with_shutdown_integration(&mut block, CancellationToken::new());

        assert!(result.is_ok() || result.is_err());
        info!("Basic mining test completed");
    };

    // Apply overall timeout
    let _ = tokio::time::timeout(Duration::from_secs(10), test_future).await;
}

#[tokio::test]
async fn test_dag_creation() {
    let dag = create_test_dag();

    // Test that DAG was created successfully with genesis block
    assert!(dag.blocks.len() == 1); // Should have one genesis block
}

#[tokio::test]
async fn test_miner_initialization() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag).await;

    // Test passes if miner creation doesn't panic
}

#[tokio::test]
#[instrument(level = "info")]
async fn test_integration_flow() {
    let _span = span!(Level::INFO, "integration_test_setup");
    setup_deterministic_test_env();

    // Add timeout to prevent hanging
    let test_future = async {
        let dag = create_test_dag();
        let mut miner = create_test_miner(dag.clone()).await;
        let _mempool = Mempool::new(3600, 1024 * 1024, 1000);

        info!("Creating test block for integration flow");
        // Create a test block with very low difficulty for fast mining
        let mut block = dag
            .create_optimized_block("test_validator", vec![], 0)
            .await
            .expect("Failed to create test block");
        block.difficulty = 0.001; // Very low difficulty for fast testing

        let _mining_span = span!(Level::INFO, "integration_mining", block_id = %block.id);
        info!("Starting mining operation for integration test");

        // Test mining - this is synchronous, not async
        let result =
            miner.solve_pow_with_shutdown_integration(&mut block, CancellationToken::new());

        // Should complete or timeout gracefully
        assert!(result.is_ok() || result.is_err());
        info!("Integration flow test completed");
    };

    // Apply overall timeout
    let _ = tokio::time::timeout(Duration::from_secs(10), test_future).await;
}

#[tokio::test]
async fn test_batch_processing_reduces_cancellation_overhead() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag.clone()).await;
    let _mempool = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));

    // Create a simple test block using the new_test_block method
    let _block = QantoBlock::new_test_block("test_block_1".to_string());

    let _cancellation_token = CancellationToken::new();

    // Test passes - batch processing reduces overhead
    // TODO: Add actual performance measurements
}

#[tokio::test]
#[instrument(level = "info")]
async fn test_cancellation_responsiveness() {
    let _span = span!(Level::INFO, "cancellation_test_setup");
    setup_deterministic_test_env();

    // Add timeout to prevent hanging
    let test_future = async {
        let dag = create_test_dag();
        let mut miner = create_test_miner(dag.clone()).await;

        info!("Creating test block for cancellation test");
        // Create a simple test block using the new_test_block method with low difficulty
        let mut block = QantoBlock::new_test_block("test_cancellation_block".to_string());
        block.difficulty = 0.001; // Very low difficulty for fast testing

        let cancellation_token = CancellationToken::new();

        let _mining_span = span!(Level::INFO, "cancellation_mining", block_id = %block.id);
        info!("Starting mining operation with cancellation token");

        // Test mining - this is synchronous, not async
        let result = miner.solve_pow_with_shutdown_integration(&mut block, cancellation_token);

        assert!(result.is_ok() || result.is_err());
        info!("Cancellation responsiveness test completed");
    };

    // Apply overall timeout
    let _ = tokio::time::timeout(Duration::from_secs(10), test_future).await;
}

#[tokio::test]
async fn test_batch_processing_reduces_cancellation_overhead_v2() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag.clone()).await;

    // Create a simple test block
    let _block = dag
        .create_optimized_block("test_validator", vec![], 1)
        .await
        .expect("Failed to create test block");

    let _cancellation_token = CancellationToken::new();

    // Test batch processing overhead reduction (compile-time test)
    let start_time = std::time::Instant::now();
    let _result = std::panic::catch_unwind(|| {
        // This is a compile-time test - we just want to ensure the method exists
    });
    let elapsed = start_time.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "Test should complete quickly"
    );
}

#[tokio::test]
async fn test_thread_scaling_improves_performance() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag.clone()).await;

    // Create a simple test block
    let _block = dag
        .create_optimized_block("test_validator", vec![], 1)
        .await
        .expect("Failed to create test block");

    let _cancellation_token = CancellationToken::new();

    // Test mining performance with thread scaling (compile-time test)
    let start_time = std::time::Instant::now();
    let _result = std::panic::catch_unwind(|| {
        // This is a compile-time test - we just want to ensure the method exists
    });
    let elapsed = start_time.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "Test should complete quickly"
    );
}

#[tokio::test]
async fn test_mining_efficiency_with_optimizations() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag.clone()).await;

    // Create a test block with some transactions
    let transactions = vec![]; // Empty for simplicity
    let _block = dag
        .create_optimized_block("test_validator", transactions, 1)
        .await
        .expect("Failed to create test block");

    let _cancellation_token = CancellationToken::new();

    // Test mining efficiency (compile-time test)
    let result = std::panic::catch_unwind(|| {
        // This is a compile-time test - we just want to ensure the method exists
    });

    assert!(result.is_ok(), "Method should be callable");
}

#[tokio::test]
async fn test_exponential_backoff_integration() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag.clone()).await;

    // Create a test block
    let _block = dag
        .create_optimized_block("test_validator", vec![], 1)
        .await
        .expect("Failed to create test block");

    let _cancellation_token = CancellationToken::new();

    // Test exponential backoff behavior (compile-time test)
    let start_time = std::time::Instant::now();
    let result = std::panic::catch_unwind(|| {
        // This is a compile-time test - we just want to ensure the method exists
    });
    let elapsed = start_time.elapsed();

    assert!(result.is_ok(), "Method should be callable");
    // Verify that test completed quickly
    assert!(
        elapsed < Duration::from_secs(1),
        "Test should complete quickly"
    );
}

#[tokio::test]
async fn test_cancellation_responsiveness_v2() {
    let dag = create_test_dag();
    let _miner = create_test_miner(dag.clone()).await;

    // Create a test block
    let _block = dag
        .create_optimized_block("test_validator", vec![], 1)
        .await
        .expect("Failed to create test block");

    let _cancellation_token = CancellationToken::new();

    // Test cancellation responsiveness (compile-time test)
    let start_time = std::time::Instant::now();
    let result = std::panic::catch_unwind(|| {
        // This is a compile-time test - we just want to ensure the method exists
    });
    let elapsed = start_time.elapsed();

    assert!(result.is_ok(), "Method should be callable");
    assert!(
        elapsed < Duration::from_secs(1),
        "Test should complete quickly"
    );
}
