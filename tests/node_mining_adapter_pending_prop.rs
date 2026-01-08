use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use proptest::prelude::*;
use tokio::runtime::Runtime;

use qanto::config::LoggingConfig;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::mining_metrics::MiningMetrics;
use qanto::node_keystore::Wallet;
use qanto::node_mining_adapter::NodeMiningAdapter;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::{QuantumResistantSignature, UTXO};
use qanto_core::adaptive_mining::MiningAdapter;

// Property-based adapter test: random sequence of add/remove/block-apply/prune operations.
proptest! {
    #![proptest_config(ProptestConfig { cases: 6, max_shrink_iters: 0, .. Default::default() })]
    #[test]
    fn adapter_pending_len_property(ops in proptest::collection::vec(0u8..=3, 1..30)) {
        let rt = Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            qanto::init_test_tracing();

            // Lightweight components
            let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
            let dag = std::sync::Arc::new(dag);
            let wallet = std::sync::Arc::new(Wallet::new().expect("wallet"));
            let miner_cfg = MinerConfig {
                address: wallet.address(),
                dag: dag.clone(),
                target_block_time: 5_000,
                use_gpu: false,
                zk_enabled: false,
                threads: 1,
                logging_config: LoggingConfig::default(),
            };
            let miner = std::sync::Arc::new(Miner::new(miner_cfg).expect("miner"));
            let metrics = std::sync::Arc::new(MiningMetrics::new());

            let mempool = std::sync::Arc::new(tokio::sync::RwLock::new(Mempool::new(1, 10_000_000, 100_000)));
            let utxos = std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::<String, UTXO>::new()));

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

            let mut local: Vec<Transaction> = Vec::new();
            let mut prune_calls = 0u8;

            for op in ops {
                match op {
                    // Add
                    0 => {
                        let tx = Transaction::new_dummy();
                        let utx = utxos.read().await;
                        let _ = mempool.write().await.add_transaction(tx.clone(), &utx, &dag).await;
                        local.push(tx);
                    },
                    // Remove one if present
                    1 => {
                        if let Some(tx) = local.pop() {
                            mempool.write().await.remove_transactions(&[tx]).await;
                        }
                    },
                    // Apply a block containing up to 2 of currently-added txs
                    2 => {
                        let take = core::cmp::min(2, local.len());
                        if take > 0 {
                            let txs: Vec<Transaction> = local.drain(local.len() - take..).collect();
                            let merkle_root = qanto::qantodag::QantoBlock::compute_merkle_root(&txs).unwrap_or_default();
                            let block = qanto::qantodag::QantoBlock {
                                chain_id: 0,
                                id: format!("prop_block_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()),
                                parents: vec![],
                                transactions: txs,
                                difficulty: 1.0,
                                target: None,
                                validator: "test_validator".to_string(),
                                miner: "test_miner".to_string(),
                                nonce: 0,
                                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                                height: 1,
                                reward: 0,
                                effort: 0,
                                cross_chain_references: vec![],
                                cross_chain_swaps: vec![],
                                merkle_root,
                                signature: QuantumResistantSignature { signer_public_key: vec![0; 32], signature: vec![0; 64] },
                                homomorphic_encrypted: vec![],
                                smart_contracts: vec![],
                                carbon_credentials: vec![],
                                epoch: 0,
                                finality_proof: None,
                                reservation_snapshot_id: None,
                            };
                            adapter.post_add_block(&block).await.expect("post_add_block");
                        }
                    },
                    // Prune occasionally (limit real sleeps)
                    3 => {
                        if prune_calls < 2 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
                            mempool.read().await.prune_old_transactions().await;
                            prune_calls += 1;
                        }
                    },
                    _ => unreachable!(),
                }

                // Invariants:
                // 1) mempool pending equals mempool.len()
                // 2) adapter pending equals mempool pending
                let len = mempool.read().await.len().await;
                let pending_mp = mempool.read().await.pending_len_sync();
                let pending_adapter = adapter.pending_transactions_len().await;
                assert_eq!(pending_mp, len, "pending_len_sync must match len() after op");
                assert_eq!(pending_adapter, pending_mp, "adapter pending must match mempool pending after op");
            }

            // Final invariants
            let final_len = mempool.read().await.len().await;
            let final_pending_mp = mempool.read().await.pending_len_sync();
            let final_pending_adapter = adapter.pending_transactions_len().await;
            assert_eq!(final_pending_mp, final_len);
            assert_eq!(final_pending_adapter, final_pending_mp);
        });
    }
}
