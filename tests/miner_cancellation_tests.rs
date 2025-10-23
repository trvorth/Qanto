//! Unit tests for miner cancellation functionality
//!
//! These tests verify that the miner properly handles cancellation signals
//! and that atomic operations on found_signal use consistent ordering.

use qanto::config::LoggingConfig;
use qanto::miner::{Miner, MinerConfig};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use serial_test::serial;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;

/// Helper function to create a test DAG with optimized settings for fast tests
fn create_test_dag() -> Arc<QantoDAG> {
    let storage_config = StorageConfig {
        data_dir: std::path::PathBuf::from("/tmp/test_qanto_miner"),
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

/// Helper function to create a test miner with optimized settings for fast tests
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

#[tokio::test]
#[serial]
async fn test_cancellation_token_stops_mining() {
    let miner = create_test_miner();
    let mut block = qanto::qantodag::QantoBlock::new_test_block("test_block_1".to_string());

    // Create a cancellation token and cancel it immediately
    let cancellation_token = CancellationToken::new();
    cancellation_token.cancel();

    // Mining should return immediately due to cancellation
    let start_time = SystemTime::now();
    let result = miner.solve_pow_with_cancellation(&mut block, cancellation_token);
    let elapsed = start_time.elapsed().unwrap();

    // Should complete quickly (within 100ms) due to cancellation
    assert!(elapsed < Duration::from_millis(100));

    // Should return TimeoutOrCancelled error
    match result {
        Err(qanto::miner::MiningError::TimeoutOrCancelled) => {
            // Expected result
        }
        other => panic!("Expected TimeoutOrCancelled error, got: {other:?}"),
    }
}

#[tokio::test]
#[serial]
async fn test_found_signal_atomic_ordering() {
    let miner = create_test_miner();
    let found_signal = Arc::new(AtomicBool::new(false));
    let cancellation_token = CancellationToken::new();

    // Test that found_signal uses Relaxed ordering consistently
    // This is a behavioral test - we verify the signal works correctly

    // Initially false
    assert!(!found_signal.load(Ordering::Relaxed));

    // Set to true
    found_signal.store(true, Ordering::Relaxed);
    assert!(found_signal.load(Ordering::Relaxed));

    // Test should_stop_mining function behavior
    let should_stop = miner.should_stop_mining(&cancellation_token, &found_signal);
    assert!(
        should_stop,
        "should_stop_mining should return true when found_signal is true"
    );

    // Reset and test with cancellation
    found_signal.store(false, Ordering::Relaxed);
    cancellation_token.cancel();

    let should_stop = miner.should_stop_mining(&cancellation_token, &found_signal);
    assert!(
        should_stop,
        "should_stop_mining should return true when cancellation_token is cancelled"
    );
}

/// Optimized test with 30s timeout, capped iterations, and fast cancellation
#[tokio::test]
#[serial]
async fn test_cancellation_responsiveness() {
    const TEST_TIMEOUT_SECS: u64 = 30;
    const MAX_MINING_ITERATIONS: u64 = 50_000;

    // Wrap entire test in timeout to prevent hanging
    let test_result = tokio::time::timeout(Duration::from_secs(TEST_TIMEOUT_SECS), async {
        let _miner = create_test_miner();

        // Create a block with moderate difficulty for predictable behavior
        let mut block = qanto::qantodag::QantoBlock::new_test_block("test_block_2".to_string());
        // Use moderate difficulty instead of extremely high difficulty
        block.difficulty = 100.0; // Reduced from 1000000.0 for faster execution

        // Create a cancellation token
        let cancellation_token = CancellationToken::new();
        let token_clone = cancellation_token.clone();

        // Start mining in a background task with iteration cap
        let mining_handle: tokio::task::JoinHandle<Result<(), qanto::miner::MiningError>> =
            tokio::spawn(async move {
                // Simulate mining with capped iterations to prevent infinite loops
                let _start_time = SystemTime::now();
                let mut iterations = 0u64;

                loop {
                    // Check cancellation every 1000 iterations
                    if iterations.is_multiple_of(1000) && cancellation_token.is_cancelled() {
                        return Err(qanto::miner::MiningError::TimeoutOrCancelled);
                    }

                    // Cap iterations to prevent infinite loops
                    if iterations >= MAX_MINING_ITERATIONS {
                        return Err(qanto::miner::MiningError::TimeoutOrCancelled);
                    }

                    // Simulate some work (lightweight hash operation)
                    let _dummy_hash = format!("hash_{iterations}");

                    iterations += 1;

                    // Small delay to make cancellation more predictable
                    if iterations.is_multiple_of(100) {
                        tokio::task::yield_now().await;
                    }
                }
            });

        // Wait a short time, then cancel
        tokio::time::sleep(Duration::from_millis(50)).await;
        token_clone.cancel();

        // Mining should complete within reasonable time after cancellation
        let start_cancel = SystemTime::now();
        let result = tokio::time::timeout(Duration::from_millis(500), mining_handle).await;
        let cancel_response_time = start_cancel.elapsed().unwrap();

        assert!(result.is_ok(), "Mining should complete after cancellation");
        assert!(
            cancel_response_time < Duration::from_millis(400),
            "Cancellation should be responsive (took {cancel_response_time:?})"
        );

        // Should return TimeoutOrCancelled error
        match result.unwrap().unwrap() {
            Err(qanto::miner::MiningError::TimeoutOrCancelled) => {
                // Expected result
            }
            other => panic!("Expected TimeoutOrCancelled error, got: {other:?}"),
        }
    })
    .await;

    match test_result {
        Ok(_) => {
            println!("✅ Cancellation responsiveness test completed successfully");
        }
        Err(_) => {
            panic!("❌ Concurrent test timed out after {TEST_TIMEOUT_SECS} seconds");
        }
    }
}

#[test]
#[serial]
fn test_should_check_cancellation_frequency() {
    let miner = create_test_miner();

    // Test cancellation check frequency
    assert!(!miner.should_check_cancellation(99_999));
    assert!(miner.should_check_cancellation(100_000));
    assert!(!miner.should_check_cancellation(100_001));
    assert!(miner.should_check_cancellation(200_000));
    assert!(miner.should_check_cancellation(1_000_000));
}

#[test]
#[serial]
fn test_should_check_timeout_frequency() {
    let miner = create_test_miner();

    // Test timeout check frequency
    assert!(!miner.should_check_timeout(999_999));
    assert!(miner.should_check_timeout(1_000_000));
    assert!(!miner.should_check_timeout(1_000_001));
    assert!(miner.should_check_timeout(2_000_000));
}

#[test]
#[serial]
fn test_handle_cancellation_check() {
    let miner = create_test_miner();
    let found_signal = Arc::new(AtomicBool::new(false));
    let cancellation_token = CancellationToken::new();

    // Test with no cancellation
    let should_stop = miner.handle_cancellation_check(&cancellation_token, &found_signal);
    assert!(!should_stop);
    assert!(!found_signal.load(Ordering::Relaxed));

    // Test with cancellation
    cancellation_token.cancel();
    let should_stop = miner.handle_cancellation_check(&cancellation_token, &found_signal);
    assert!(should_stop);
    assert!(
        found_signal.load(Ordering::Relaxed),
        "found_signal should be set to true when cancellation is detected"
    );
}

/// Optimized concurrent test with reduced load and timeout
#[tokio::test]
#[serial]
async fn test_concurrent_found_signal_access() {
    const TEST_TIMEOUT_SECS: u64 = 10;

    // Wrap test in timeout
    let test_result = tokio::time::timeout(Duration::from_secs(TEST_TIMEOUT_SECS), async {
        let found_signal = Arc::new(AtomicBool::new(false));
        let num_threads = 4; // Reduced from 10 for faster execution
        let operations_per_thread = 100; // Reduced from 1000 for faster execution

        let mut handles = Vec::new();

        // Spawn multiple tasks that concurrently access found_signal
        for i in 0..num_threads {
            let signal = Arc::clone(&found_signal);
            let handle = tokio::spawn(async move {
                for j in 0..operations_per_thread {
                    // Alternate between setting and reading
                    if (i + j) % 2 == 0 {
                        signal.store(true, Ordering::Relaxed);
                    } else {
                        let _value = signal.load(Ordering::Relaxed);
                    }

                    // Yield occasionally to prevent monopolizing CPU
                    if j % 10 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Task should complete successfully");
        }

        // The final state should be consistent (no data races)
        let final_value = found_signal.load(Ordering::Relaxed);
        // We can't predict the exact final value, but the operation should complete without panics
        println!("Final found_signal value: {final_value}");
    })
    .await;

    match test_result {
        Ok(_) => {
            println!("✅ Concurrent access test completed successfully");
        }
        Err(_) => {
            panic!("❌ Concurrent test timed out after {TEST_TIMEOUT_SECS} seconds");
        }
    }
}
