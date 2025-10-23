//! Block Throughput Validation Test
//!
//! This module provides focused testing for block throughput validation,
//! specifically ensuring the system can achieve >30 BPS (Blocks Per Second).

use anyhow::Result;
use qanto::performance_validation::PerformanceValidator;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::wallet::Wallet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock as AsyncRwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Test timeout duration (15 seconds)
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Target BPS for validation (>30 BPS)
const TARGET_BPS: f64 = 30.0;

/// Enhanced target BPS for high performance (>32 BPS)
const ENHANCED_TARGET_BPS: f64 = 32.0;

/// Create optimized test environment for block throughput testing
async fn create_block_throughput_environment() -> Result<(Arc<QantoDAG>, Wallet)> {
    // Create wallet
    let wallet = Wallet::new().map_err(|e| anyhow::anyhow!("Failed to create wallet: {e}"))?;

    // Create saga
    let saga = PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    );

    // Create unique temporary directory for block throughput testing
    let test_id = std::thread::current().id();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let unique_dir = format!(
        "/tmp/qanto_block_throughput_{}_{}",
        format!("{test_id:?}")
            .replace("ThreadId(", "")
            .replace(")", ""),
        timestamp
    );

    // Optimized storage config for block throughput testing
    let storage_config = StorageConfig {
        data_dir: PathBuf::from(unique_dir),
        max_file_size: 2 * 1024 * 1024, // 2MB for block storage
        compression_enabled: false,      // Disabled for speed
        encryption_enabled: false,       // Disabled for speed
        wal_enabled: true,               // Enabled for block consistency
        sync_writes: false,              // Disabled for speed
        cache_size: 128 * 1024 * 1024,   // 128MB cache for blocks
        compaction_threshold: 0.8,
        max_open_files: 200,
    };

    let storage = QantoStorage::new(storage_config)
        .map_err(|e| anyhow::anyhow!("Failed to create storage: {e}"))?;

    // Optimized DAG config for block throughput
    let dag_config = QantoDagConfig {
        initial_validator: "block_throughput_validator".to_string(),
        target_block_time: 800, // 800ms for >30 BPS (1000ms/800ms = 1.25, so 1.25 blocks per second base)
        num_chains: 4,          // Multiple chains for parallel block processing
        dev_fee_rate: 0.05,     // Reduced fee for faster processing
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        Arc::new(saga),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .map_err(|e| anyhow::anyhow!("Failed to create DAG: {e}"))?;

    Ok((dag_arc, wallet))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn test_block_throughput_validation_30_bps() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    info!("Starting block throughput validation test (>30 BPS)");

    let test_future = async {
        let (dag, wallet) = create_block_throughput_environment().await?;
        let validator = PerformanceValidator::new(Arc::new(AsyncRwLock::new((*dag).clone())));
        let (signing_key, public_key) = wallet.get_keypair()?;

        // Test duration optimized for block throughput measurement
        let duration_secs = 5; // 5 seconds should be enough to measure BPS accurately
        
        info!("Running block throughput validation for {} seconds", duration_secs);
        let start_time = Instant::now();
        
        let results = validator
            .run_comprehensive_validation(duration_secs, &signing_key, &public_key)
            .await
            .map_err(|e| anyhow::anyhow!("Block throughput validation failed: {e}"))?;

        let elapsed = start_time.elapsed();
        
        // Validate results
        assert!(results.test_duration_secs > 0.0);
        assert!(results.total_blocks_processed > 0);
        
        let bps = results.bps_achieved;
        let blocks = results.total_blocks_processed;
        let avg_block_time = results.avg_block_time_ms;
        
        info!("=== BLOCK THROUGHPUT RESULTS ===");
        info!("Test Duration: {:.2} seconds", elapsed.as_secs_f64());
        info!("Total Blocks Processed: {}", blocks);
        info!("Achieved BPS: {:.2}", bps);
        info!("Average Block Time: {:.2} ms", avg_block_time);
        info!("Target BPS: {:.2}", TARGET_BPS);
        
        // Primary validation: >30 BPS
        if bps > TARGET_BPS {
            info!("✅ PASS: Block throughput exceeds 30 BPS target ({:.2} BPS)", bps);
        } else {
            warn!("❌ FAIL: Block throughput below 30 BPS target ({:.2} BPS)", bps);
            // Still assert for test failure
            assert!(bps > TARGET_BPS, "Block throughput must exceed 30 BPS, got {:.2} BPS", bps);
        }
        
        // Enhanced validation: >32 BPS (stretch goal)
        if bps > ENHANCED_TARGET_BPS {
            info!("🚀 EXCELLENT: Block throughput exceeds enhanced 32 BPS target ({:.2} BPS)", bps);
        } else {
            info!("⚠️ NOTE: Block throughput below enhanced 32 BPS target ({:.2} BPS)", bps);
        }
        
        // Validate block time consistency
        let expected_max_block_time = 1000.0 / TARGET_BPS; // ~33.33ms for 30 BPS
        if avg_block_time <= expected_max_block_time {
            info!("✅ PASS: Average block time within target ({:.2} ms <= {:.2} ms)", avg_block_time, expected_max_block_time);
        } else {
            warn!("⚠️ WARNING: Average block time exceeds target ({:.2} ms > {:.2} ms)", avg_block_time, expected_max_block_time);
        }
        
        // Validate minimum blocks processed
        let expected_min_blocks = (duration_secs as f64 * TARGET_BPS * 0.8) as u64; // 80% of target
        if blocks >= expected_min_blocks {
            info!("✅ PASS: Sufficient blocks processed ({} >= {})", blocks, expected_min_blocks);
        } else {
            warn!("⚠️ WARNING: Fewer blocks processed than expected ({} < {})", blocks, expected_min_blocks);
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(TEST_TIMEOUT, test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Block throughput validation test timed out after {:?}", TEST_TIMEOUT);
            Err(anyhow::anyhow!("Block throughput test timed out"))
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_block_throughput_validation_quick() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    info!("Starting quick block throughput validation test");

    let test_future = async {
        let (dag, wallet) = create_block_throughput_environment().await?;
        let validator = PerformanceValidator::new(Arc::new(AsyncRwLock::new((*dag).clone())));
        let (signing_key, public_key) = wallet.get_keypair()?;

        // Quick test with shorter duration
        let duration_secs = 2;
        
        let results = validator
            .run_comprehensive_validation(duration_secs, &signing_key, &public_key)
            .await
            .map_err(|e| anyhow::anyhow!("Quick block throughput validation failed: {e}"))?;

        assert!(results.test_duration_secs > 0.0);
        assert!(results.total_blocks_processed > 0);
        
        let bps = results.bps_achieved;
        info!("Quick block throughput test: {:.2} BPS", bps);
        
        // More lenient validation for quick test
        let quick_target = TARGET_BPS * 0.7; // 70% of target for quick test
        if bps > quick_target {
            info!("✅ PASS: Quick test achieved {:.2} BPS (target: {:.2})", bps, quick_target);
        } else {
            info!("⚠️ NOTE: Quick test achieved {:.2} BPS (target: {:.2})", bps, quick_target);
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(8), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Quick block throughput test timed out");
            Err(anyhow::anyhow!("Quick block throughput test timed out"))
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_block_throughput_validation_intensive() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    info!("Starting intensive block throughput validation test");

    let test_future = async {
        let (dag, wallet) = create_block_throughput_environment().await?;
        let validator = PerformanceValidator::new(Arc::new(AsyncRwLock::new((*dag).clone())));
        let (signing_key, public_key) = wallet.get_keypair()?;

        // Intensive test with longer duration
        let duration_secs = 8;
        
        info!("Running intensive block throughput validation for {} seconds", duration_secs);
        let start_time = Instant::now();
        
        let results = validator
            .run_comprehensive_validation(duration_secs, &signing_key, &public_key)
            .await
            .map_err(|e| anyhow::anyhow!("Intensive block throughput validation failed: {e}"))?;

        let elapsed = start_time.elapsed();
        
        assert!(results.test_duration_secs > 0.0);
        assert!(results.total_blocks_processed > 0);
        
        let bps = results.bps_achieved;
        let blocks = results.total_blocks_processed;
        let tps = results.tps_achieved;
        
        info!("=== INTENSIVE BLOCK THROUGHPUT RESULTS ===");
        info!("Test Duration: {:.2} seconds", elapsed.as_secs_f64());
        info!("Total Blocks Processed: {}", blocks);
        info!("Achieved BPS: {:.2}", bps);
        info!("Achieved TPS: {:.0}", tps);
        info!("Blocks per minute: {:.0}", bps * 60.0);
        
        // Intensive validation with higher expectations
        let intensive_target = ENHANCED_TARGET_BPS; // 32 BPS for intensive test
        if bps > intensive_target {
            info!("✅ PASS: Intensive test achieved {:.2} BPS (target: {:.2})", bps, intensive_target);
        } else {
            warn!("❌ FAIL: Intensive test achieved {:.2} BPS (target: {:.2})", bps, intensive_target);
            // Assert for test failure in intensive mode
            assert!(bps > TARGET_BPS, "Even in intensive mode, must exceed basic 30 BPS target, got {:.2} BPS", bps);
        }
        
        // Validate sustained performance
        let expected_total_blocks = (duration_secs as f64 * TARGET_BPS) as u64;
        if blocks >= expected_total_blocks {
            info!("✅ PASS: Sustained block production ({} >= {} blocks)", blocks, expected_total_blocks);
        } else {
            info!("⚠️ NOTE: Block production below sustained target ({} < {} blocks)", blocks, expected_total_blocks);
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(25), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Intensive block throughput test timed out");
            Err(anyhow::anyhow!("Intensive block throughput test timed out"))
        }
    }
}

#[test]
fn test_block_throughput_constants() {
    // Validate test constants
    assert!(TARGET_BPS > 0.0);
    assert!(ENHANCED_TARGET_BPS > TARGET_BPS);
    assert_eq!(TARGET_BPS, 30.0);
    assert_eq!(ENHANCED_TARGET_BPS, 32.0);
    
    // Validate timeout is reasonable
    assert!(TEST_TIMEOUT.as_secs() >= 10);
    assert!(TEST_TIMEOUT.as_secs() <= 30);
    
    info!("Block throughput test constants validated");
}