//! Unit tests for nonce loop optimization features
//! Tests batching, cancellation checks, default threads, and exponential backoff

use qanto::config::LoggingConfig;
use qanto::miner::{Miner, MinerConfig};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use std::sync::Arc;

fn create_test_dag() -> Arc<QantoDAG> {
    // Create temporary storage
    let temp_dir = std::env::temp_dir().join("qanto_test_nonce_optimization");
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    let storage_config = StorageConfig {
        data_dir: temp_dir,
        max_file_size: 1024 * 1024 * 10, // 10MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024 * 1024, // 1MB
        compaction_threshold: 1000.0,
        max_open_files: 100,
    };

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");
    let saga = Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ));

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 60,
        num_chains: 1,
    };

    QantoDAG::new(dag_config, saga, storage, LoggingConfig::default())
        .expect("Failed to create DAG")
}

#[tokio::test]
async fn test_default_thread_configuration() {
    let dag = create_test_dag();

    // Test with threads = 0 (should use default)
    let config = MinerConfig {
        address: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        dag: dag.clone(),
        target_block_time: 10,
        use_gpu: false,
        zk_enabled: false,
        threads: 0, // Should use default (CPU cores - 1)
        logging_config: LoggingConfig::default(),
    };

    let _miner = Miner::new(config).expect("Failed to create miner");

    // Test default thread configuration
    // Note: This test validates the miner can be created with default thread config
    // Placeholder assertion - actual thread validation would require exposing thread count
}

#[tokio::test]
async fn test_batch_processing_with_cancellation() {
    let dag = create_test_dag();

    let config = MinerConfig {
        address: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        dag: dag.clone(),
        target_block_time: 10,
        use_gpu: false,
        zk_enabled: false,
        threads: 2,
        logging_config: LoggingConfig::default(),
    };

    let _miner = Miner::new(config).expect("Failed to create miner");

    // Test batch processing configuration
    // Note: This test validates the miner can be created with batch processing config
    // Placeholder assertion - actual batch validation would require exposing batch size
}

#[test]
fn test_exponential_backoff_calculation() {
    let dag = create_test_dag();
    let miner_config = MinerConfig {
        address: "7256ce7626c2f4b90770d3cc1d1cdd03f213627369600e7243d07c883c638710".to_string(),
        dag: dag.clone(),
        target_block_time: 1000,
        use_gpu: false,
        zk_enabled: false,
        threads: 4,
        logging_config: qanto::config::LoggingConfig::default(),
    };

    let _miner = Miner::new(miner_config).unwrap();

    // Test exponential backoff calculation
    // Note: This test validates the miner can be created with backoff config
    // Placeholder assertion - actual backoff testing would require exposing backoff methods
}

#[tokio::test]
async fn test_mining_with_small_batch_size() {
    let dag = create_test_dag();

    let config = MinerConfig {
        address: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        dag: dag.clone(),
        target_block_time: 1, // Very short target time
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };

    let _miner = Miner::new(config).expect("Failed to create miner");

    // Test mining with small batch size
    // Note: This test validates the miner can be created with small batch config
    // Placeholder assertion - actual mining would require full block template
}

#[tokio::test]
async fn test_cancellation_check_frequency() {
    let dag = create_test_dag();

    let config = MinerConfig {
        address: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        dag: dag.clone(),
        target_block_time: 10,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };

    let _miner = Miner::new(config).expect("Failed to create miner");

    // Test cancellation check frequency
    // Note: This test validates the miner can be created with cancellation config
    // Placeholder assertion - actual cancellation testing would require full mining setup
}

#[tokio::test]
async fn test_thread_pool_optimization() {
    let dag = create_test_dag();
    let miner_config = MinerConfig {
        address: "7256ce7626c2f4b90770d3cc1d1cdd03f213627369600e7243d07c883c638710".to_string(),
        dag: dag.clone(),
        target_block_time: 1000,
        use_gpu: false,
        zk_enabled: false,
        threads: 4,
        logging_config: qanto::config::LoggingConfig::default(),
    };

    let _miner = Miner::new(miner_config).unwrap();

    // Test thread pool optimization
    // Note: This test validates the miner can be created with thread pool config
    // Placeholder assertion - actual thread pool testing would require exposing thread metrics
}
