use qanto::mempool::Mempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;
use std::collections::HashMap;

/// Batch insertion should update pending_len_sync and reject duplicates deterministically.
#[tokio::test]
async fn add_transaction_batch_updates_pending_and_rejects_duplicates() {
    qanto::init_test_tracing();

    let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let mempool = Mempool::new(300, 1024 * 1024, 10_000);

    // Prepare batch with a deliberate duplicate.
    let tx1 = Transaction::new_dummy();
    let tx2 = Transaction::new_dummy();
    let tx3 = Transaction::new_dummy();
    let batch = vec![tx1.clone(), tx1.clone(), tx2.clone(), tx3.clone()];

    let (accepted, rejected) = mempool.add_transaction_batch(batch, &utxos, &dag).await;

    // Expect one duplicate rejected and three accepted.
    assert_eq!(
        accepted.len(),
        3,
        "Three unique transactions should be accepted"
    );
    assert_eq!(rejected.len(), 1, "One duplicate should be rejected");
    assert!(
        rejected[0].1.contains("Duplicate"),
        "Rejected reason should indicate duplicate"
    );

    // Pending count via atomic path should match number of accepted txs.
    let pending = mempool.pending_len_sync();
    assert_eq!(
        pending,
        accepted.len(),
        "pending_len_sync should equal accepted count"
    );

    // Cross-validate against async len().
    let len_async = mempool.len().await;
    assert_eq!(
        pending, len_async,
        "pending_len_sync should equal mempool.len()"
    );
}
