use std::collections::HashMap;

use proptest::prelude::*;
use tokio::runtime::Runtime;

use qanto::mempool::Mempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;

// Property-based test: random sequence of add/remove/prune operations.
proptest! {
    #![proptest_config(ProptestConfig { cases: 8, max_shrink_iters: 0, .. Default::default() })]
    #[test]
    fn pending_len_sync_property(ops in proptest::collection::vec(0u8..=2, 1..40)) {
        let rt = Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            let mempool = Mempool::new(1, 10_000_000, 100_000);
            let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
            let utxos: HashMap<String, UTXO> = HashMap::new();

            let mut local: Vec<Transaction> = Vec::new();
            let mut prune_calls = 0u8;

            for op in ops {
                match op {
                    // Add
                    0 => {
                        let tx = Transaction::new_dummy();
                        let _ = mempool.add_transaction(tx.clone(), &utxos, &dag).await;
                        local.push(tx);
                    },
                    // Remove one if present
                    1 => {
                        if let Some(tx) = local.pop() {
                            mempool.remove_transactions(&[tx]).await;
                        }
                    },
                    // Prune occasionally (limit real sleeps)
                    2 => {
                        if prune_calls < 2 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
                            mempool.prune_old_transactions().await;
                            prune_calls += 1;
                        }
                    },
                    _ => unreachable!(),
                }
                // Invariant: atomic pending count equals logical length
                let len = mempool.len().await;
                let pending = mempool.pending_len_sync();
                assert_eq!(pending, len, "pending_len_sync must match len() after op");
            }

            // Final invariant after potential prune
            let final_len = mempool.len().await;
            let final_pending = mempool.pending_len_sync();
            assert_eq!(final_pending, final_len);
        });
    }
}
