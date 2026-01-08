use qanto::config::LoggingConfig;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::mining_metrics::MiningMetrics;
use qanto::node_keystore::Wallet;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto_core::adaptive_mining::MiningAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[inline]
fn new_saga() -> Arc<PalletSaga> {
    Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ))
}

/// Ensures NodeMiningAdapter enforces DAG ASERT difficulty, overriding suggestions.
#[tokio::test]
async fn enforces_asert_consensus_difficulty() {
    qanto::init_test_tracing();

    // Set up isolated storage in a temp directory
    let temp_dir = std::env::temp_dir().join("qanto_node_mining_adapter_asert");
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
        target_block_time: 5_000, // ms
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let dag = QantoDAG::new(dag_config, saga.clone(), storage, LoggingConfig::default()).unwrap();

    // Create wallet and register as validator so candidate blocks can be created
    let wallet = Arc::new(Wallet::new().unwrap());
    let miner_address = wallet.address();
    dag.add_validator(miner_address.clone(), 1000).await;

    // Populate DAG with enough blocks to exit "startup mode" (requires > 100 blocks)
    // so that ASERT difficulty enforcement kicks in.
    {
        for i in 1..=101 {
            let mut block = qanto::qantodag::QantoBlock::new_test_block(format!("dummy-{}", i));
            block.height = i;
            block.difficulty = 1.0; // Arbitrary
            dag.blocks.insert(block.id.clone(), block);
        }
        // Update count to reflect inserted blocks
        // (Assuming dag.blocks.len() reflects the map size, which it does)
    }

    // Minimal mempool/UTXO sets
    let mempool = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));
    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // Miner is required by DAG::create_candidate_block API
    let miner_cfg = MinerConfig {
        address: miner_address.clone(),
        dag: dag.clone(),
        target_block_time: 5_000,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };
    let miner = Arc::new(Miner::new(miner_cfg).unwrap());

    // Metrics for adapter hooks
    let metrics = Arc::new(MiningMetrics::new());

    // Construct the adapter
    let adapter = qanto::node_mining_adapter::NodeMiningAdapter::new(
        dag.clone(),
        wallet.clone(),
        miner.clone(),
        mempool.clone(),
        utxos.clone(),
        metrics.clone(),
        None,
        None,
    );

    // Get consensus difficulty from DAG (ASERT-driven)
    let consensus = dag.get_current_difficulty().await;

    // Provide a deliberately mismatched suggested difficulty; adapter must override
    let suggested = consensus * 2.0;
    let candidate: qanto::qantodag::QantoBlock = adapter
        .create_candidate_block(suggested)
        .await
        .expect("candidate block should be created successfully");

    // Verify the adapter enforces consensus difficulty exactly
    let diff = (candidate.difficulty - consensus).abs();
    assert!(diff < 1e-12, "Adapter must set ASERT consensus difficulty. diff={diff} consensus={consensus} candidate={}", candidate.difficulty);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
