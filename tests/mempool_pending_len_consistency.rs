use std::collections::HashMap;
use std::sync::Arc;

use qanto::mempool::Mempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;

/// Ensures that the atomic pending counter stays consistent with the async len()
/// after concurrent inserts.
#[tokio::test]
async fn pending_count_matches_len_after_concurrent_inserts() {
    let max_age_secs: u64 = 3600;
    let max_size_bytes: usize = 256 * 1024 * 1024; // 256 MiB
    let max_transactions: usize = 1_000_000;
    let mempool = Mempool::new(max_age_secs, max_size_bytes, max_transactions);

    let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
    let dag_arc = Arc::new(dag);
    let utxos_arc: Arc<HashMap<String, UTXO>> = Arc::new(HashMap::new());

    let workers = 8usize;
    let per_worker = 1_000usize;

    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let mempool_clone = mempool.clone();
        let dag_clone = dag_arc.clone();
        let utxos_clone = utxos_arc.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..per_worker {
                let tx = Transaction::new_dummy();
                // Coinbases bypass UTXO checks in verification branch; ideal for stress insert.
                mempool_clone
                    .add_transaction(tx, &utxos_clone, &dag_clone)
                    .await
                    .expect("dummy tx admitted to mempool");
            }
        }));
    }

    for h in handles {
        h.await.expect("worker join");
    }

    let len_async = mempool.len().await;
    let pending_sync = mempool.pending_len_sync();
    assert_eq!(
        pending_sync, len_async,
        "pending count must match len() after inserts"
    );
}

/// Ensures consistency after a batch removal of a subset of transactions.
#[tokio::test]
async fn pending_count_matches_len_after_batch_remove() {
    let max_age_secs: u64 = 3600;
    let max_size_bytes: usize = 256 * 1024 * 1024; // 256 MiB
    let max_transactions: usize = 1_000_000;
    let mempool = Mempool::new(max_age_secs, max_size_bytes, max_transactions);

    let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
    let dag_arc = Arc::new(dag);
    let utxos_arc: Arc<HashMap<String, UTXO>> = Arc::new(HashMap::new());

    // Preload with a moderate number of transactions
    for _ in 0..10_000 {
        let tx = Transaction::new_dummy();
        mempool
            .add_transaction(tx, &utxos_arc, &dag_arc)
            .await
            .expect("dummy tx admitted to mempool");
    }

    // Remove half of them
    let current = mempool.get_pending_transactions().await;
    let to_remove = current.into_iter().take(5_000).collect::<Vec<_>>();
    mempool.remove_transactions(&to_remove).await;

    let len_async = mempool.len().await;
    let pending_sync = mempool.pending_len_sync();
    assert_eq!(
        pending_sync, len_async,
        "pending count must match len() after removals"
    );
}
