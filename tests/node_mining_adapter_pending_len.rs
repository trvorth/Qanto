use qanto::config::LoggingConfig;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::mining_metrics::MiningMetrics;
use qanto::node_keystore::Wallet;
use qanto::node_mining_adapter::NodeMiningAdapter;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;
use qanto_core::adaptive_mining::MiningAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapter should reflect mempool pending count after additions.
#[tokio::test]
async fn pending_len_reflects_additions_via_adapter() {
    qanto::init_test_tracing();

    // Lightweight DAG for verification context (no heavy storage).
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());

    // Minimal wallet/miner setup to satisfy adapter constructor.
    let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));
    let miner_cfg = MinerConfig {
        address: wallet.address(),
        dag: dag.clone(),
        target_block_time: 5_000,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };
    let miner = Arc::new(Miner::new(miner_cfg).expect("Failed to create miner"));
    let metrics = Arc::new(MiningMetrics::new());

    // Mempool + UTXO set.
    let mempool = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 10_000)));
    let utxos = Arc::new(RwLock::new(HashMap::<String, UTXO>::new()));

    // Adapter under test.
    let adapter = NodeMiningAdapter::new(
        dag.clone(),
        wallet.clone(),
        miner.clone(),
        mempool.clone(),
        utxos.clone(),
        metrics.clone(),
        None,
        None,
    );

    // Add N transactions into mempool.
    let n = 5usize;
    for _ in 0..n {
        let tx = Transaction::new_dummy();
        let utxos_guard = utxos.read().await;
        mempool
            .write()
            .await
            .add_transaction(tx, &utxos_guard, &dag)
            .await
            .expect("add_transaction should succeed");
    }

    // Verify adapter sees the same pending count via O(1) path.
    let pending = adapter.pending_transactions_len().await;
    assert_eq!(pending, n, "Adapter pending count should match mempool");
}

/// Adapter should reflect mempool pending count after removals.
#[tokio::test]
async fn pending_len_reflects_removals_via_adapter() {
    qanto::init_test_tracing();

    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());
    let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));
    let miner_cfg = MinerConfig {
        address: wallet.address(),
        dag: dag.clone(),
        target_block_time: 5_000,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };
    let miner = Arc::new(Miner::new(miner_cfg).expect("Failed to create miner"));
    let metrics = Arc::new(MiningMetrics::new());

    let mempool = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 10_000)));
    let utxos = Arc::new(RwLock::new(HashMap::<String, UTXO>::new()));

    let adapter = NodeMiningAdapter::new(
        dag.clone(),
        wallet.clone(),
        miner.clone(),
        mempool.clone(),
        utxos.clone(),
        metrics.clone(),
        None,
        None,
    );

    // Add transactions.
    let mut all: Vec<Transaction> = Vec::new();
    for _ in 0..6 {
        let tx = Transaction::new_dummy();
        let utxos_guard = utxos.read().await;
        mempool
            .write()
            .await
            .add_transaction(tx.clone(), &utxos_guard, &dag)
            .await
            .expect("add_transaction should succeed");
        all.push(tx);
    }

    // Remove some.
    let to_remove = all[..3].to_vec();
    mempool.write().await.remove_transactions(&to_remove).await;

    let pending = adapter.pending_transactions_len().await;
    assert_eq!(pending, 3, "Pending count should reflect removals");
}

/// Adapter should reflect pruning behavior, dropping pending count to zero after age expiration.
#[tokio::test]
async fn pending_len_prunes_to_zero_via_adapter() {
    qanto::init_test_tracing();

    // Set small max_age to force pruning quickly.
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());
    let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));
    let miner_cfg = MinerConfig {
        address: wallet.address(),
        dag: dag.clone(),
        target_block_time: 5_000,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };
    let miner = Arc::new(Miner::new(miner_cfg).expect("Failed to create miner"));
    let metrics = Arc::new(MiningMetrics::new());

    let mempool = Arc::new(RwLock::new(Mempool::new(1, 1024 * 1024, 10_000))); // 1 second max_age
    let utxos = Arc::new(RwLock::new(HashMap::<String, UTXO>::new()));

    let adapter = NodeMiningAdapter::new(
        dag.clone(),
        wallet.clone(),
        miner.clone(),
        mempool.clone(),
        utxos.clone(),
        metrics.clone(),
        None,
        None,
    );

    // Add a few transactions.
    for _ in 0..3 {
        let tx = Transaction::new_dummy();
        let utxos_guard = utxos.read().await;
        mempool
            .write()
            .await
            .add_transaction(tx, &utxos_guard, &dag)
            .await
            .expect("add_transaction should succeed");
    }

    // Wait long enough to exceed max_age and prune.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    mempool.read().await.prune_old_transactions().await;

    let pending = adapter.pending_transactions_len().await;
    assert_eq!(
        pending, 0,
        "Pending count should drop to zero after pruning"
    );
}
