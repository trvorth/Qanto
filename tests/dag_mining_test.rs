use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::wallet::Wallet;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[inline]
fn new_saga() -> Arc<PalletSaga> {
    Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ))
}

/// Test DAG mining functionality with performance verification
#[tokio::test]
async fn test_dag_mining_performance() {
    let _ = tracing_subscriber::fmt::try_init();

    let temp_dir = std::env::temp_dir().join("qanto_dag_mining_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10,
        compaction_threshold: 1000.0,
        max_open_files: 100,
    };

    let storage = QantoStorage::new(storage_config).unwrap();
    let saga = new_saga();

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 60,
        num_chains: 1,
    };

    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap();

    let wallet = Wallet::new().unwrap();
    let miner_address = wallet.address();

    dag_arc.add_validator(miner_address.clone(), 1000).await;

    let mempool_arc = Arc::new(tokio::sync::RwLock::new(Mempool::new(
        300,
        1024 * 1024,
        1000,
    )));
    let utxos_arc = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

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

    // Test 1: Verify DAG caching works correctly
    info!("Testing DAG caching functionality...");

    let epoch = 0u64; // Test with epoch 0
    let start_time = Instant::now();
    let dag1 = qanto::qanhash::get_qdag_optimized(epoch).await;
    let first_generation_time = start_time.elapsed();

    let start_time = Instant::now();
    let dag2 = qanto::qanhash::get_qdag_optimized(epoch).await;
    let second_access_time = start_time.elapsed();

    // Verify same DAG instance is returned (cached)
    assert!(
        Arc::ptr_eq(&dag1, &dag2),
        "DAG should be cached and return same instance"
    );

    // Verify caching provides performance benefit (avoid brittle 10x requirement)
    assert!(
        second_access_time < first_generation_time / 2,
        "Cached DAG access should be noticeably faster than generation (≥2x). First: {first_generation_time:?}, Second: {second_access_time:?}"
    );

    info!(
        "DAG caching test passed - Generation: {:?}, Cache access: {:?}",
        first_generation_time, second_access_time
    );

    // Test 2: Verify mining with DAG works correctly
    info!("Testing DAG-based mining functionality...");

    let block = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0,
            &Arc::new(miner.clone()),
            None,
        )
        .await
        .unwrap();

    // Test mining with very low difficulty for quick completion
    let start_mining_time = Instant::now();

    // Use the mine_cpu_with_dag_inner function directly to test DAG mining
    let target_hash_value = [0xFF; 32]; // Very easy target
    let threads = 1;
    let cancellation_token = CancellationToken::new();

    // Get the DAG for mining
    let epoch = 0u64;
    let qdag = qanto::qanhash::get_qdag_optimized(epoch).await;

    // Test that mining completes without errors
    let mining_result = qanto::miner::mine_cpu_with_dag_inner(
        block,  // Pass owned value instead of reference
        &target_hash_value,
        threads,
        cancellation_token.clone(),
        qdag.clone(),
    )
    .await;

    let _mining_time = start_mining_time.elapsed();

    // Verify mining result
    match mining_result {
        Ok(qanto::miner::MiningResult::Found { nonce, hash }) => {
            println!("✓ DAG mining successful: nonce={nonce}, hash={hash:?}");
            assert_ne!(hash, [0u8; 32], "Hash should not be zero");
        }
        Ok(qanto::miner::MiningResult::Cancelled) => {
            println!("✗ DAG mining was cancelled");
            panic!("Mining should not be cancelled with easy target");
        }
        Ok(qanto::miner::MiningResult::Timeout) => {
            println!("✗ DAG mining timed out");
            panic!("Mining should not timeout with easy target");
        }
        Err(e) => {
            println!("✗ DAG mining failed: {e:?}");
            panic!("DAG mining should not fail: {e:?}");
        }
    }

    // Test 3: Verify multiple epochs use different DAGs
    info!("Testing multiple epoch DAG generation...");

    let block_index1 = 0u64; // Maps to epoch 0
    let block_index2 = 10_000u64; // Maps to epoch 1 (EPOCH_LENGTH = 10_000)

    let dag_epoch1 = qanto::qanhash::get_qdag_optimized(block_index1).await;
    let dag_epoch2 = qanto::qanhash::get_qdag_optimized(block_index2).await;

    // Different epochs should have different DAG instances or at least different content
    let same_instance = Arc::ptr_eq(&dag_epoch1, &dag_epoch2);
    let same_content = dag_epoch1.len() == dag_epoch2.len()
        && dag_epoch1
            .iter()
            .zip(dag_epoch2.iter())
            .all(|(a, b)| a == b);

    assert!(
        !same_instance || !same_content,
        "Different epochs should have different DAG instances or different content. Same instance: {same_instance}, Same content: {same_content}",
    );

    // But same epoch should return cached DAG
    let dag_epoch1_again = qanto::qanhash::get_qdag_optimized(block_index1).await;
    assert!(
        Arc::ptr_eq(&dag_epoch1, &dag_epoch1_again),
        "Same epoch should return cached DAG instance"
    );

    info!("Multiple epoch DAG test passed");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();

    info!("All DAG mining tests passed successfully!");
}

/// Test DAG memory efficiency and caching behavior
#[tokio::test]
async fn test_dag_memory_efficiency() {
    let _ = tracing_subscriber::fmt::try_init();

    info!("Testing DAG memory efficiency and caching...");

    // Test that accessing many epochs doesn't cause memory explosion
    let mut access_times = Vec::new();

    for epoch in 0..5 {
        let start_time = Instant::now();
        let _dag = qanto::qanhash::get_qdag_optimized(epoch).await;
        let access_time = start_time.elapsed();
        access_times.push(access_time);

        info!("Epoch {} DAG access time: {:?}", epoch, access_time);
    }

    // Verify that later accesses aren't significantly slower (indicating memory pressure)
    let first_access = access_times[0];
    let last_access = access_times[access_times.len() - 1];

    assert!(
        last_access < first_access * 5, // Allow up to 5x slower for last access
        "DAG access times should not degrade significantly. First: {first_access:?}, Last: {last_access:?}",
    );

    // Test cache hit behavior
    let start_time = Instant::now();
    let _dag_cached = qanto::qanhash::get_qdag_optimized(0).await; // Should be cached
    let cached_access_time = start_time.elapsed();

    assert!(
        cached_access_time < first_access / 2, // Cached access should be at least 2x faster
        "Cached DAG access should be faster. Original: {first_access:?}, Cached: {cached_access_time:?}",
    );

    info!("DAG memory efficiency test passed");
}
