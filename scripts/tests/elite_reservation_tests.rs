use std::collections::HashMap;
use std::sync::Arc;

use qanto::config::LoggingConfig;
use qanto::elite_mempool::EliteMempool;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::transaction::Transaction;

fn create_test_dag() -> Arc<QantoDAG> {
    let storage_config = StorageConfig {
        data_dir: std::env::temp_dir().join("qanto_elite_reservation_tests"),
        max_file_size: 64 * 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024 * 1024,
        compaction_threshold: 10,
        max_open_files: 64,
        ..StorageConfig::default()
    };

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 1000,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let logging_config = LoggingConfig::default();

    let saga_pallet = Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ));

    QantoDAG::new(dag_config, saga_pallet, storage, logging_config).expect("Failed to create DAG")
}

#[tokio::test]
async fn concurrent_miners_do_not_select_duplicate_transactions() {
    // Create DAG and EliteMempool
    let dag = create_test_dag();
    let mempool =
        EliteMempool::new(10_000, 64 * 1024 * 1024, 8, 4).expect("Failed to create EliteMempool");

    // Populate mempool with dummy transactions
    let utxos: HashMap<String, qanto::types::UTXO> = HashMap::new();
    for _ in 0..1000 {
        let tx = Transaction::new_dummy();
        mempool
            .add_transaction(tx, &utxos, dag.as_ref())
            .await
            .expect("Failed to add transaction");
    }

    // Concurrently select transactions for two miners
    let m1 = mempool.get_priority_transactions_with_reservation(300, Some("miner1".to_string()));
    let m2 = mempool.get_priority_transactions_with_reservation(300, Some("miner2".to_string()));
    let (t1, t2) = tokio::join!(m1, m2);

    // Ensure no duplicate transaction IDs across miners
    use std::collections::HashSet;
    let set1: HashSet<String> = t1.iter().map(|t| t.id.clone()).collect();
    let set2: HashSet<String> = t2.iter().map(|t| t.id.clone()).collect();
    let intersection: HashSet<String> = set1.intersection(&set2).cloned().collect();

    assert!(
        intersection.is_empty(),
        "Duplicate transactions selected across miners: {:?}",
        intersection
    );
}
