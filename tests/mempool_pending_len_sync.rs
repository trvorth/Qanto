use std::collections::HashMap;

use tokio::time::Duration as TokioDuration;

// Public APIs from the qanto crate
use qanto::mempool::Mempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;

#[tokio::test]
async fn pending_len_sync_initial_zero() {
    let mempool = Mempool::new(5, 1_000_000, 10_000);
    assert_eq!(mempool.pending_len_sync(), 0);
}

#[tokio::test]
async fn pending_len_sync_add_and_remove() {
    let mempool = Mempool::new(60, 5_000_000, 10_000);
    let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
    let utxos: HashMap<String, UTXO> = HashMap::new();

    let t1 = Transaction::new_dummy();
    let t2 = Transaction::new_dummy();
    let t3 = Transaction::new_dummy();

    mempool
        .add_transaction(t1.clone(), &utxos, &dag)
        .await
        .unwrap();
    mempool
        .add_transaction(t2.clone(), &utxos, &dag)
        .await
        .unwrap();
    mempool
        .add_transaction(t3.clone(), &utxos, &dag)
        .await
        .unwrap();

    assert_eq!(mempool.pending_len_sync(), 3);

    mempool.remove_transactions(std::slice::from_ref(&t1)).await;
    assert_eq!(mempool.pending_len_sync(), 2);

    mempool.remove_transactions(&[t2, t3]).await;
    assert_eq!(mempool.pending_len_sync(), 0);
}

#[tokio::test]
async fn pending_len_sync_prune_old_transactions() {
    let mempool = Mempool::new(1, 5_000_000, 10_000);
    let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
    let utxos: HashMap<String, UTXO> = HashMap::new();

    let tx = Transaction::new_dummy();
    mempool
        .add_transaction(tx.clone(), &utxos, &dag)
        .await
        .unwrap();
    assert_eq!(mempool.pending_len_sync(), 1);

    tokio::time::sleep(TokioDuration::from_secs(2)).await;
    mempool.prune_old_transactions().await;

    assert_eq!(mempool.pending_len_sync(), 0);
}
