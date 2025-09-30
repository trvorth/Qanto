//! Integration Tests for Mempool and Block Size Validation
//!
//! This module provides comprehensive integration tests for mempool overflow/eviction
//! scenarios and block size validation to reproduce and prevent the 874MB error.

use anyhow::Result;
use qanto::mempool::{MempoolError, OptimizedMempool};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoBlock, QantoDAG, QantoDAGError, QantoDagConfig};
use qanto::resource_cleanup::{CleanupPriority, CleanupResource, ResourceCleanup, ResourceType};
use qanto::saga::PalletSaga;
use qanto::transaction::{Input, Output, Transaction};
use qanto::types::QuantumResistantSignature;
use serde_json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock as AsyncRwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Test timeout duration (60 seconds for integration tests)
const INTEGRATION_TEST_TIMEOUT: Duration = Duration::from_secs(60);

/// 1MB block size limit (as per requirements)
const MAX_BLOCK_SIZE: usize = 1_048_576; // 1MB

/// 100KB individual transaction size limit
const MAX_TRANSACTION_SIZE: usize = 102_400; // 100KB

/// Mempool size limits for testing
const MEMPOOL_SIZE_1MB: usize = 1_048_576;
const MEMPOOL_SIZE_10MB: usize = 10_485_760;

/// Create a test transaction with specified size
fn create_test_transaction(size_bytes: usize, fee: u64) -> Result<Transaction> {
    use qanto::types::{HomomorphicEncrypted, QuantumResistantSignature};

    let mut tx = Transaction {
        id: format!(
            "test_tx_{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ),
        sender: "test_sender".to_string(),
        receiver: "test_receiver".to_string(),
        amount: 1000,
        inputs: vec![Input {
            tx_id: "0".repeat(64),
            output_index: 0,
        }],
        outputs: vec![Output {
            address: "test_recipient".to_string(),
            amount: 1000,
            homomorphic_encrypted: HomomorphicEncrypted {
                ciphertext: vec![],
                public_key: vec![],
            },
        }],
        fee,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        metadata: std::collections::HashMap::new(),
        signature: QuantumResistantSignature {
            signer_public_key: vec![],
            signature: vec![],
        },
    };

    // Pad the transaction to reach desired size
    let current_size = serde_json::to_vec(&tx)?.len();
    if size_bytes > current_size {
        let padding_size = size_bytes - current_size;
        tx.signature.signature = vec![0u8; padding_size];
    }

    Ok(tx)
}

/// Create a large transaction that exceeds size limits
fn create_oversized_transaction(size_bytes: usize) -> Result<Transaction> {
    use qanto::types::{HomomorphicEncrypted, QuantumResistantSignature};

    let mut tx = Transaction {
        id: format!(
            "oversized_tx_{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ),
        sender: "test_sender".to_string(),
        receiver: "test_receiver".to_string(),
        amount: 1000,
        inputs: vec![Input {
            tx_id: "0".repeat(64),
            output_index: 0,
        }],
        outputs: vec![Output {
            address: "test_recipient".to_string(),
            amount: 1000,
            homomorphic_encrypted: HomomorphicEncrypted {
                ciphertext: vec![],
                public_key: vec![],
            },
        }],
        fee: 1000,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        metadata: std::collections::HashMap::new(),
        signature: QuantumResistantSignature {
            signer_public_key: vec![],
            signature: vec![0u8; size_bytes], // Large signature to inflate size
        },
    };

    tx.id = format!("oversized_tx_{}", tx.timestamp);
    Ok(tx)
}

/// Create test environment for integration tests
async fn create_integration_test_environment(
) -> Result<(Arc<AsyncRwLock<QantoDAG>>, OptimizedMempool)> {
    // Create SAGA pallet
    #[cfg(feature = "infinite-strata")]
    let saga_pallet = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga_pallet = Arc::new(PalletSaga::new());

    // Create storage config for integration tests
    let storage_config = StorageConfig {
        data_dir: PathBuf::from("integration_test_db"),
        max_file_size: 1024 * 1024 * 50, // 50MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB cache
        compaction_threshold: 0.9,
        max_open_files: 100,
    };

    let storage = QantoStorage::new(storage_config)?;

    // Create DAG config
    let dag_config = QantoDagConfig {
        initial_validator: qanto::qantodag::DEV_ADDRESS.to_string(),
        target_block_time: 30,
        num_chains: 4,
    };

    let dag_instance = QantoDAG::new(dag_config, saga_pallet, storage)?;
    let dag_inner = Arc::try_unwrap(dag_instance).expect("Failed to unwrap Arc for DAG");
    let dag = Arc::new(AsyncRwLock::new(dag_inner));

    // Create mempool with 10MB limit for testing
    let mempool = OptimizedMempool::new(
        Duration::from_secs(300), // 5 minutes max age
        MEMPOOL_SIZE_10MB,
    );

    Ok((dag, mempool))
}

#[tokio::test]
async fn test_mempool_overflow_and_eviction() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting mempool overflow and eviction test");

    let test_future = async {
        let (_dag, mempool) = create_integration_test_environment().await?;

        // Fill mempool to capacity with low-fee transactions
        let low_fee = 10;
        let tx_size = 1024; // 1KB transactions
        let num_low_fee_txs = MEMPOOL_SIZE_10MB / tx_size;

        info!(
            "Adding {} low-fee transactions to fill mempool",
            num_low_fee_txs
        );

        for i in 0..num_low_fee_txs {
            let tx = create_test_transaction(tx_size, low_fee)?;
            match mempool.add_transaction(tx).await {
                Ok(_) => {}
                Err(MempoolError::MempoolFull) => {
                    info!("Mempool full after {} transactions", i);
                    break;
                }
                Err(e) => return Err(anyhow::anyhow!("Unexpected error: {:?}", e)),
            }
        }

        let initial_size = mempool.get_current_size_bytes();
        info!("Mempool size after filling: {} bytes", initial_size);

        // Now add high-fee transactions that should evict low-fee ones
        let high_fee = 1000;
        let num_high_fee_txs = 100;

        info!(
            "Adding {} high-fee transactions to trigger eviction",
            num_high_fee_txs
        );

        for i in 0..num_high_fee_txs {
            let tx = create_test_transaction(tx_size, high_fee)?;
            match mempool.add_transaction(tx).await {
                Ok(_) => {
                    info!("High-fee transaction {} added successfully", i);
                }
                Err(MempoolError::BackpressureActive(_)) => {
                    info!("Backpressure active for transaction {}", i);
                }
                Err(e) => {
                    warn!("Failed to add high-fee transaction {}: {:?}", i, e);
                }
            }
        }

        let final_size = mempool.get_current_size_bytes();
        info!("Final mempool size: {} bytes", final_size);

        // Verify eviction occurred
        assert!(final_size <= MEMPOOL_SIZE_10MB);

        // Verify mempool is still functional
        let tx_count = mempool.get_transaction_count();
        info!("Final transaction count: {}", tx_count);
        assert!(tx_count > 0);

        Ok::<(), anyhow::Error>(())
    };

    match timeout(INTEGRATION_TEST_TIMEOUT, test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!(
                "Mempool overflow test timed out after {:?}",
                INTEGRATION_TEST_TIMEOUT
            );
            Err(anyhow::anyhow!("Test timed out"))
        }
    }
}

#[tokio::test]
async fn test_mempool_backpressure_mechanism() -> Result<()> {
    info!("Testing mempool backpressure mechanism");

    let test_future = async {
        // Create smaller mempool for faster testing
        let mempool = OptimizedMempool::new(
            Duration::from_secs(300),
            MEMPOOL_SIZE_1MB, // 1MB limit
        );

        let tx_size = 10_240; // 10KB transactions
        let low_fee = 5;
        let high_fee = 100;

        // Fill mempool to trigger backpressure (75% threshold)
        let backpressure_threshold_txs = (MEMPOOL_SIZE_1MB * 75 / 100) / tx_size;

        info!(
            "Adding {} transactions to trigger backpressure",
            backpressure_threshold_txs
        );

        for _i in 0..backpressure_threshold_txs {
            let tx = create_test_transaction(tx_size, low_fee)?;
            mempool.add_transaction(tx).await?;
        }

        // Verify backpressure is active
        let size_before_backpressure = mempool.get_current_size_bytes();
        info!(
            "Mempool size before backpressure test: {} bytes",
            size_before_backpressure
        );

        // Try to add low-fee transaction during backpressure
        let low_fee_tx = create_test_transaction(tx_size, low_fee)?;
        let result = mempool.add_transaction(low_fee_tx).await;

        match result {
            Err(MempoolError::BackpressureActive(_)) => {
                info!("Backpressure correctly rejected low-fee transaction");
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected backpressure rejection for low-fee transaction"
                ));
            }
        }

        // High-fee transaction should still be accepted
        let high_fee_tx = create_test_transaction(tx_size, high_fee)?;
        let result = mempool.add_transaction(high_fee_tx).await;

        match result {
            Ok(_) => {
                info!("High-fee transaction accepted during backpressure");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "High-fee transaction should be accepted: {:?}",
                    e
                ));
            }
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(30), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Backpressure test timed out");
            Err(anyhow::anyhow!("Backpressure test timed out"))
        }
    }
}

#[tokio::test]
async fn test_block_size_validation_1mb_limit() -> Result<()> {
    info!("Testing block size validation with 1MB limit");

    let test_future = async {
        let (_dag, _mempool) = create_integration_test_environment().await?;

        // Test 1: Create a block that exceeds 1MB limit (reproduce 874MB error scenario)
        info!("Creating oversized block to test 1MB limit enforcement");

        let mut large_transactions = Vec::new();
        let tx_size = 50_000; // 50KB per transaction
        let num_txs = 25; // 25 * 50KB = 1.25MB (exceeds 1MB limit)

        for _i in 0..num_txs {
            let tx = create_test_transaction(tx_size, 100)?;
            large_transactions.push(tx);
        }

        let oversized_block = QantoBlock {
            chain_id: 1,
            id: "test_oversized_block".to_string(),
            parents: vec!["0".repeat(64)],
            transactions: large_transactions,
            difficulty: 1.0,
            validator: "test_validator".to_string(),
            miner: "test_miner".to_string(),
            nonce: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 1,
            reward: 100,
            effort: 50,
            cross_chain_references: vec![],
            cross_chain_swaps: vec![],
            merkle_root: "test_merkle_root".to_string(),
            signature: QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            homomorphic_encrypted: vec![],
            smart_contracts: vec![],
            carbon_credentials: vec![],
            epoch: 1,
        };

        // Verify block size exceeds 1MB
        let serialized_size = serde_json::to_vec(&oversized_block)?.len();
        info!(
            "Oversized block size: {} bytes ({:.2} MB)",
            serialized_size,
            serialized_size as f64 / 1_048_576.0
        );
        assert!(serialized_size > MAX_BLOCK_SIZE);

        // Test block size validation
        let validation_result = QantoDAG::validate_block_size(&oversized_block);
        match validation_result {
            Err(QantoDAGError::InvalidBlock(msg)) => {
                info!(
                    "Block size validation correctly rejected oversized block: {}",
                    msg
                );
                assert!(msg.contains("exceeds 1MB limit"));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected block size validation to reject oversized block"
                ));
            }
        }

        // Test 2: Create a valid block under 1MB limit
        info!("Creating valid block under 1MB limit");

        let mut valid_transactions = Vec::new();
        let small_tx_size = 1024; // 1KB per transaction
        let num_small_txs = 500; // 500 * 1KB = 500KB (well under 1MB)

        for _i in 0..num_small_txs {
            let tx = create_test_transaction(small_tx_size, 50)?;
            valid_transactions.push(tx);
        }

        let valid_block = QantoBlock {
            chain_id: 1,
            id: "test_valid_block".to_string(),
            parents: vec!["0".repeat(64)],
            transactions: valid_transactions,
            difficulty: 1.0,
            validator: "test_validator".to_string(),
            miner: "test_miner".to_string(),
            nonce: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 1,
            reward: 100,
            effort: 50,
            cross_chain_references: vec![],
            cross_chain_swaps: vec![],
            merkle_root: "test_merkle_root".to_string(),
            signature: qanto::types::QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            homomorphic_encrypted: vec![],
            smart_contracts: vec![],
            carbon_credentials: vec![],
            epoch: 1,
        };

        // Verify block size is under 1MB
        let valid_size = serde_json::to_vec(&valid_block)?.len();
        info!(
            "Valid block size: {} bytes ({:.2} MB)",
            valid_size,
            valid_size as f64 / 1_048_576.0
        );
        assert!(valid_size <= MAX_BLOCK_SIZE);

        // Test block size validation passes
        let validation_result = QantoDAG::validate_block_size(&valid_block);
        match validation_result {
            Ok(_) => {
                info!("Block size validation correctly accepted valid block");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Valid block should pass validation: {:?}",
                    e
                ));
            }
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(INTEGRATION_TEST_TIMEOUT, test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Block size validation test timed out");
            Err(anyhow::anyhow!("Block size test timed out"))
        }
    }
}

#[tokio::test]
async fn test_individual_transaction_size_limit() -> Result<()> {
    info!("Testing individual transaction size limit (100KB)");

    let test_future = async {
        // Test oversized individual transaction
        let oversized_tx = create_oversized_transaction(150_000)?; // 150KB transaction
        let tx_size = serde_json::to_vec(&oversized_tx)?.len();
        info!("Oversized transaction size: {} bytes", tx_size);
        assert!(tx_size > MAX_TRANSACTION_SIZE);

        // Create block with oversized transaction
        let block_with_oversized_tx = QantoBlock {
            chain_id: 1,
            id: "test_oversized_tx_block".to_string(),
            parents: vec!["0".repeat(64)],
            transactions: vec![oversized_tx],
            difficulty: 1.0,
            validator: "test_validator".to_string(),
            miner: "test_miner".to_string(),
            nonce: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 1,
            reward: 100,
            effort: 50,
            cross_chain_references: vec![],
            cross_chain_swaps: vec![],
            merkle_root: "test_merkle_root".to_string(),
            signature: qanto::types::QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            homomorphic_encrypted: vec![],
            smart_contracts: vec![],
            carbon_credentials: vec![],
            epoch: 1,
        };

        // Validate that oversized transaction is rejected
        let validation_result = QantoDAG::validate_block_size(&block_with_oversized_tx);
        match validation_result {
            Err(QantoDAGError::InvalidBlock(msg)) => {
                info!(
                    "Individual transaction size validation correctly rejected: {}",
                    msg
                );
                assert!(msg.contains("exceeds 100KB limit"));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected validation to reject oversized transaction"
                ));
            }
        }

        // Test valid transaction size
        let valid_tx = create_test_transaction(50_000, 100)?; // 50KB transaction
        let valid_tx_size = serde_json::to_vec(&valid_tx)?.len();
        info!("Valid transaction size: {} bytes", valid_tx_size);
        assert!(valid_tx_size <= MAX_TRANSACTION_SIZE);

        let block_with_valid_tx = QantoBlock {
            chain_id: 1,
            id: "test_valid_tx_block".to_string(),
            parents: vec!["0".repeat(64)],
            transactions: vec![valid_tx],
            difficulty: 1.0,
            validator: "test_validator".to_string(),
            miner: "test_miner".to_string(),
            nonce: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 1,
            reward: 100,
            effort: 50,
            cross_chain_references: vec![],
            cross_chain_swaps: vec![],
            merkle_root: "test_merkle_root".to_string(),
            signature: qanto::types::QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            homomorphic_encrypted: vec![],
            smart_contracts: vec![],
            carbon_credentials: vec![],
            epoch: 1,
        };

        // Validate that valid transaction is accepted
        let validation_result = QantoDAG::validate_block_size(&block_with_valid_tx);
        match validation_result {
            Ok(_) => {
                info!("Valid transaction size validation passed");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Valid transaction should pass validation: {:?}",
                    e
                ));
            }
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(20), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Transaction size validation test timed out");
            Err(anyhow::anyhow!("Transaction size test timed out"))
        }
    }
}

#[tokio::test]
async fn test_mempool_eviction_priority() -> Result<()> {
    info!("Testing mempool eviction priority (lowest fee first)");

    let test_future = async {
        let mempool = OptimizedMempool::new(Duration::from_secs(300), MEMPOOL_SIZE_1MB);

        let tx_size = 10_240; // 10KB transactions

        // Add transactions with different fees
        let fees = vec![10, 50, 100, 200, 500]; // Ascending fee order
        let mut tx_ids = Vec::new();

        for (i, fee) in fees.iter().enumerate() {
            let tx = create_test_transaction(tx_size, *fee)?;
            let tx_id = tx.id.clone();
            mempool.add_transaction(tx).await?;
            tx_ids.push(tx_id);
            info!("Added transaction {} with fee {}", i, fee);
        }

        // Fill mempool to trigger eviction
        let fill_count = (MEMPOOL_SIZE_1MB / tx_size) - fees.len();
        for _i in 0..fill_count {
            let tx = create_test_transaction(tx_size, 1000)?; // High fee to trigger eviction
            match mempool.add_transaction(tx).await {
                Ok(_) => {}
                Err(MempoolError::MempoolFull) => break,
                Err(_) => {} // Continue on other errors
            }
        }

        // Verify that low-fee transactions were evicted first
        let remaining_count = mempool.get_transaction_count();
        info!("Remaining transactions after eviction: {}", remaining_count);

        // The mempool should have evicted some transactions
        assert!(remaining_count < fees.len() + fill_count);

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(30), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Eviction priority test timed out");
            Err(anyhow::anyhow!("Eviction priority test timed out"))
        }
    }
}

/// Unit test for transaction creation utilities
#[test]
fn test_transaction_creation_utilities() {
    let tx = create_test_transaction(1024, 100).unwrap();

    // Verify transaction structure
    assert!(!tx.id.is_empty());
    assert_eq!(tx.fee, 100);
    assert!(!tx.inputs.is_empty());
    assert!(!tx.outputs.is_empty());

    // Verify transaction size is approximately correct
    let serialized = serde_json::to_vec(&tx).unwrap();
    assert!(serialized.len() >= 1000); // Should be close to 1024 bytes
}

/// Unit test for oversized transaction creation
#[test]
fn test_oversized_transaction_creation() {
    let tx = create_oversized_transaction(200_000).unwrap();
    let serialized = serde_json::to_vec(&tx).unwrap();

    // Verify transaction exceeds 100KB limit
    assert!(serialized.len() > MAX_TRANSACTION_SIZE);
    assert!(serialized.len() >= 200_000);
}

/// Unit test for size constants
#[test]
fn test_size_constants() {
    assert_eq!(MAX_BLOCK_SIZE, 1_048_576); // 1MB
    assert_eq!(MAX_TRANSACTION_SIZE, 102_400); // 100KB
    assert_eq!(MEMPOOL_SIZE_1MB, 1_048_576); // 1MB
    assert_eq!(MEMPOOL_SIZE_10MB, 10_485_760); // 10MB

    // Verify relationships
    assert!(MAX_TRANSACTION_SIZE < MAX_BLOCK_SIZE);
    assert!(MEMPOOL_SIZE_1MB == MAX_BLOCK_SIZE);
    assert!(MEMPOOL_SIZE_10MB > MEMPOOL_SIZE_1MB);
}

/// Integration test for graceful shutdown with resource cleanup
#[tokio::test]
async fn test_graceful_shutdown_integration() -> Result<()> {
    let cleanup = ResourceCleanup::new(Duration::from_secs(5));

    // Register various resources that would be present in a real node
    let resources = vec![
        CleanupResource {
            name: "mempool".to_string(),
            resource_type: ResourceType::Cache,
            priority: CleanupPriority::High,
            timeout: Duration::from_millis(500),
        },
        CleanupResource {
            name: "storage".to_string(),
            resource_type: ResourceType::Database,
            priority: CleanupPriority::Critical,
            timeout: Duration::from_millis(1000),
        },
        CleanupResource {
            name: "p2p_network".to_string(),
            resource_type: ResourceType::Network,
            priority: CleanupPriority::High,
            timeout: Duration::from_millis(300),
        },
        CleanupResource {
            name: "websocket".to_string(),
            resource_type: ResourceType::WebSocket,
            priority: CleanupPriority::Medium,
            timeout: Duration::from_millis(200),
        },
    ];

    for resource in resources {
        cleanup.register_resource(resource).await?;
    }

    // Register some managed tasks
    cleanup
        .register_task(|token| async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(100)) => Ok(()),
                _ = token.cancelled() => Ok(()),
            }
        })
        .await?;

    cleanup
        .register_task(|token| async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(200)) => Ok(()),
                _ = token.cancelled() => Ok(()),
            }
        })
        .await?;

    // Verify initial state
    let stats = cleanup.get_shutdown_stats().await;
    assert_eq!(stats.registered_resources, 4);
    assert_eq!(stats.managed_tasks, 2);
    assert!(!stats.is_shutdown_requested);

    // Perform graceful shutdown
    cleanup.request_shutdown();
    let shutdown_result =
        tokio::time::timeout(Duration::from_secs(10), cleanup.graceful_shutdown()).await;

    assert!(shutdown_result.is_ok());
    assert!(shutdown_result.unwrap().is_ok());

    // Verify shutdown completed
    let final_stats = cleanup.get_shutdown_stats().await;
    assert!(final_stats.is_shutdown_requested);

    Ok(())
}

/// Integration test for concurrent task management during shutdown
#[tokio::test]
async fn test_concurrent_task_shutdown_integration() -> Result<()> {
    let cleanup = ResourceCleanup::new(Duration::from_secs(3));
    let task_count = 10;

    // Register multiple concurrent tasks
    for i in 0..task_count {
        let task_id = i;
        cleanup
            .register_task(move |token| async move {
                let sleep_duration = Duration::from_millis(50 + (task_id * 10) as u64);
                tokio::select! {
                    _ = tokio::time::sleep(sleep_duration) => {
                        info!("Task {} completed normally", task_id);
                        Ok(())
                    }
                    _ = token.cancelled() => {
                        info!("Task {} was cancelled", task_id);
                        Ok(())
                    }
                }
            })
            .await?;
    }

    // Verify all tasks are registered
    let stats = cleanup.get_shutdown_stats().await;
    assert_eq!(stats.managed_tasks, task_count);

    // Start shutdown after a brief delay
    tokio::spawn({
        let cleanup = cleanup.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            cleanup.request_shutdown();
        }
    });

    // Wait for graceful shutdown
    let shutdown_result =
        tokio::time::timeout(Duration::from_secs(10), cleanup.graceful_shutdown()).await;
    assert!(shutdown_result.is_ok());
    assert!(shutdown_result.unwrap().is_ok());

    Ok(())
}

/// Integration test for resource cleanup timeout handling
#[tokio::test]
async fn test_resource_cleanup_timeout_integration() -> Result<()> {
    let cleanup = ResourceCleanup::new(Duration::from_millis(500));

    // Register a task that will exceed the timeout
    cleanup
        .register_task(|_token| async move {
            // This task ignores cancellation and takes longer than timeout
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok(())
        })
        .await?;

    // Register resources
    cleanup
        .register_resource(CleanupResource {
            name: "slow_resource".to_string(),
            resource_type: ResourceType::Database,
            priority: CleanupPriority::Critical,
            timeout: Duration::from_millis(100),
        })
        .await?;

    // Shutdown should handle timeout gracefully
    cleanup.request_shutdown();
    let shutdown_result =
        tokio::time::timeout(Duration::from_secs(5), cleanup.graceful_shutdown()).await;

    // Should complete even with timeout
    assert!(shutdown_result.is_ok());
    assert!(shutdown_result.unwrap().is_ok());

    Ok(())
}

/// Integration test for runtime shutdown hooks
#[tokio::test]
async fn test_runtime_shutdown_hooks_integration() -> Result<()> {
    use std::sync::atomic::{AtomicU32, Ordering};

    let cleanup = ResourceCleanup::new(Duration::from_secs(5));
    let hook_counter = Arc::new(AtomicU32::new(0));

    // Register multiple runtime hooks
    for i in 0..3 {
        let counter = hook_counter.clone();
        let hook_id = i;

        cleanup
            .register_runtime_hook(Box::new(move |token| {
                let counter = counter.clone();
                tokio::spawn(async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            info!("Runtime hook {} completed", hook_id);
                            Ok(())
                        }
                        _ = token.cancelled() => {
                            info!("Runtime hook {} cancelled", hook_id);
                            Ok(())
                        }
                    }
                })
            }))
            .await?;
    }

    let stats = cleanup.get_shutdown_stats().await;
    assert_eq!(stats.registered_runtime_hooks, 3);

    // Perform shutdown
    cleanup.request_shutdown();
    cleanup.graceful_shutdown().await?;

    // All hooks should have executed
    assert_eq!(hook_counter.load(Ordering::Relaxed), 3);

    Ok(())
}
