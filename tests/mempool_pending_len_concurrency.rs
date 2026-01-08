use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinHandle;

use qanto::mempool::Mempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;

/// Concurrently add transactions and ensure the atomic pending count matches.
#[tokio::test]
async fn pending_len_sync_concurrent_adds() {
    let mempool = Mempool::new(60, 50_000_000, 1_000_000);
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());
    let utxos: Arc<HashMap<String, UTXO>> = Arc::new(HashMap::new());

    let total = 800usize;
    let workers = 8usize;
    let per_worker = total / workers;

    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    for _ in 0..workers {
        let mempool_c = mempool.clone();
        let dag_c = dag.clone();
        let utxos_c = utxos.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..per_worker {
                let tx = Transaction::new_dummy();
                let _ = mempool_c.add_transaction(tx, &utxos_c, &dag_c).await;
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(mempool.pending_len_sync(), total);
}

/// Concurrently add then concurrently remove transactions in chunks; verify count reaches zero.
#[tokio::test]
async fn pending_len_sync_concurrent_adds_and_removals() {
    let mempool = Mempool::new(60, 50_000_000, 1_000_000);
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());
    let utxos: Arc<HashMap<String, UTXO>> = Arc::new(HashMap::new());

    let total = 1000usize;
    let workers = 10usize;
    let per_worker = total / workers;

    let mut handles: Vec<JoinHandle<Vec<Transaction>>> = Vec::new();
    for _ in 0..workers {
        let mempool_c = mempool.clone();
        let dag_c = dag.clone();
        let utxos_c = utxos.clone();
        handles.push(tokio::spawn(async move {
            let mut created: Vec<Transaction> = Vec::with_capacity(per_worker);
            for _ in 0..per_worker {
                let tx = Transaction::new_dummy();
                let _ = mempool_c
                    .add_transaction(tx.clone(), &utxos_c, &dag_c)
                    .await;
                created.push(tx);
            }
            created
        }));
    }

    let mut all_txs: Vec<Transaction> = Vec::with_capacity(total);
    for h in handles {
        all_txs.extend(h.await.unwrap());
    }
    assert_eq!(mempool.pending_len_sync(), total);

    // Remove concurrently in chunks of 100
    let chunk_size = 100usize;
    let mut remove_handles: Vec<JoinHandle<()>> = Vec::new();
    for chunk in all_txs.chunks(chunk_size) {
        let mempool_c = mempool.clone();
        let slice_vec: Vec<Transaction> = chunk.to_vec();
        remove_handles.push(tokio::spawn(async move {
            mempool_c.remove_transactions(&slice_vec).await;
        }));
    }

    for h in remove_handles {
        h.await.unwrap();
    }

    assert_eq!(mempool.pending_len_sync(), 0);
    assert_eq!(mempool.len().await, 0);
}
