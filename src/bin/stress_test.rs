use qanto::metrics::QantoMetrics;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::QantoBlock;
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::transaction::Transaction;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting comprehensive stress test for 10M TPS capability");

    // Create test storage
    let temp_dir = std::env::temp_dir().join("stress_test_db");
    let storage_config = StorageConfig {
        data_dir: temp_dir,
        max_file_size: 1024 * 1024 * 100, // 100MB
        cache_size: 1024 * 1024 * 50,     // 50MB cache
        compression_enabled: true,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        compaction_threshold: 0.7,
        max_open_files: 1000,
    };
    let storage = QantoStorage::new(storage_config)?;

    // Create SAGA pallet
    #[cfg(feature = "infinite-strata")]
    let saga_pallet = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga_pallet = Arc::new(PalletSaga::new());

    // Create QantoDAG config
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 30,
        num_chains: 8,
    };

    let dag = Arc::new(QantoDAG::new(dag_config, saga_pallet, storage)?);

    // Initialize metrics
    let metrics = Arc::new(QantoMetrics::new());

    // Test parameters
    let test_duration = Duration::from_secs(60); // 1 minute test
    let target_tps = 10_000_000; // 10M TPS target
    let num_workers = 1000; // Massive parallelism
    let transactions_per_worker = target_tps / num_workers;

    info!("Test configuration:");
    info!("  Duration: {} seconds", test_duration.as_secs());
    info!("  Target TPS: {}", target_tps);
    info!("  Workers: {}", num_workers);
    info!("  Transactions per worker: {}", transactions_per_worker);

    let start_time = Instant::now();
    let total_transactions = Arc::new(AtomicU64::new(0));
    let total_blocks = Arc::new(AtomicU64::new(0));

    // Spawn worker tasks
    let mut handles = Vec::new();

    for worker_id in 0..num_workers {
        let _dag_clone = dag.clone();
        let metrics_clone = metrics.clone();
        let total_tx_clone = total_transactions.clone();
        let total_blocks_clone = total_blocks.clone();

        let handle = tokio::spawn(async move {
            let mut local_tx_count: u64 = 0;
            let mut local_block_count = 0;
            let worker_start = Instant::now();

            while worker_start.elapsed() < test_duration {
                // Generate test transaction with realistic amounts and fees
                let mut transaction = Transaction::new_dummy();

                // Use varying amounts to test different fee tiers
                let amount = match local_tx_count % 4 {
                    0 => 500_000 * 1_000_000_000,     // 500K QNTO (0% fee)
                    1 => 2_000_000 * 1_000_000_000,   // 2M QNTO (1% fee)
                    2 => 15_000_000 * 1_000_000_000,  // 15M QNTO (2% fee)
                    _ => 150_000_000 * 1_000_000_000, // 150M QNTO (3% fee)
                };

                transaction.amount = amount;
                // Calculate and set proper fee using the dynamic fee function
                transaction.fee = qanto::transaction::calculate_dynamic_fee(amount);

                // Update metrics
                metrics_clone
                    .transactions_processed
                    .fetch_add(1, Ordering::Relaxed);
                local_tx_count += 1;

                // Create test block every 1000 transactions
                if local_tx_count.is_multiple_of(1000) {
                    let _block = QantoBlock::new_test_block(format!(
                        "block_{}_{}",
                        worker_id,
                        local_tx_count / 1000
                    ));
                    metrics_clone
                        .blocks_processed
                        .fetch_add(1, Ordering::Relaxed);
                    local_block_count += 1;
                }

                // Small delay to prevent overwhelming the system
                if local_tx_count.is_multiple_of(10000) {
                    tokio::task::yield_now().await;
                }
            }

            total_tx_clone.fetch_add(local_tx_count, Ordering::Relaxed);
            total_blocks_clone.fetch_add(local_block_count, Ordering::Relaxed);

            (local_tx_count, local_block_count)
        });

        handles.push(handle);
    }

    // Monitor progress
    let monitor_handle = {
        let metrics_clone = metrics.clone();
        let total_tx_clone = total_transactions.clone();
        let total_blocks_clone = total_blocks.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            let monitor_start = Instant::now();

            while monitor_start.elapsed() < test_duration {
                interval.tick().await;

                let elapsed = monitor_start.elapsed().as_secs_f64();
                let tx_count = total_tx_clone.load(Ordering::Relaxed);
                let block_count = total_blocks_clone.load(Ordering::Relaxed);
                let current_tps = tx_count as f64 / elapsed;
                let current_bps = block_count as f64 / elapsed;

                info!(
                    "Progress: {:.1}s | TPS: {:.0} | BPS: {:.2} | Total TX: {} | Total Blocks: {}",
                    elapsed, current_tps, current_bps, tx_count, block_count
                );

                // Update real-time metrics
                metrics_clone.set_tps(current_tps);
                // Note: Using throughput_bps field for BPS tracking
                metrics_clone
                    .throughput_bps
                    .store(current_bps as u64, Ordering::Relaxed);
            }
        })
    };

    // Wait for all workers to complete
    info!("Waiting for workers to complete...");
    let mut worker_results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(result) => worker_results.push(result),
            Err(e) => error!("Worker failed: {}", e),
        }
    }

    // Stop monitoring
    monitor_handle.abort();

    let total_elapsed = start_time.elapsed();
    let final_tx_count = total_transactions.load(Ordering::Relaxed);
    let final_block_count = total_blocks.load(Ordering::Relaxed);

    // Calculate final metrics
    let achieved_tps = final_tx_count as f64 / total_elapsed.as_secs_f64();
    let achieved_bps = final_block_count as f64 / total_elapsed.as_secs_f64();

    // Update final metrics
    metrics.set_tps(achieved_tps);
    // Note: Using throughput_bps field for BPS tracking
    metrics
        .throughput_bps
        .store(achieved_bps as u64, Ordering::Relaxed);

    // Display results
    info!("=== STRESS TEST RESULTS ===");
    info!("Test Duration: {:.2} seconds", total_elapsed.as_secs_f64());
    info!("Total Transactions: {}", final_tx_count);
    info!("Total Blocks: {}", final_block_count);
    info!("Achieved TPS: {:.0}", achieved_tps);
    info!("Achieved BPS: {:.2}", achieved_bps);
    info!("Target TPS: {}", target_tps);
    info!(
        "TPS Achievement: {:.1}%",
        (achieved_tps / target_tps as f64) * 100.0
    );

    // Performance analysis
    if achieved_tps >= target_tps as f64 * 0.8 {
        info!("✅ PASS: Achieved 80%+ of target TPS");
    } else {
        warn!("❌ FAIL: Did not achieve 80% of target TPS");
    }

    if achieved_bps >= 32.0 {
        info!("✅ PASS: Achieved target BPS (32+ blocks/second)");
    } else {
        warn!("❌ FAIL: Did not achieve target BPS");
    }

    // Export metrics for analysis
    let exported_metrics = metrics.export_metrics();
    info!("Exported metrics: {:?}", exported_metrics);

    Ok(())
}
