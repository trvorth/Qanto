use qanto::config::LoggingConfig;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::mining_metrics::MiningMetrics;
use qanto::node_mining_adapter::NodeMiningAdapter;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::{QuantumResistantSignature, UTXO};
use qanto_core::adaptive_mining::MiningAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Adapter `post_add_block` should remove mined txs from mempool and update pending count deterministically.
#[tokio::test]
async fn post_add_block_removes_txs_and_resets_pending_count() {
    qanto::init_test_tracing();

    // Lightweight DAG and minimal components for adapter.
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());
    let wallet = Arc::new(qanto::node_keystore::Wallet::new().expect("Failed to create wallet"));
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

    // Prepare 3 transactions that will be included in the block, plus 1 extra left in mempool.
    let txs_in_block: Vec<Transaction> = (0..3).map(|_| Transaction::new_dummy()).collect();
    let extra_tx = Transaction::new_dummy();

    // Add all transactions to mempool.
    {
        let utxos_guard = utxos.read().await;
        let mp = mempool.write().await;
        for tx in txs_in_block
            .iter()
            .cloned()
            .chain(std::iter::once(extra_tx.clone()))
        {
            mp.add_transaction(tx, &utxos_guard, &dag)
                .await
                .expect("add_transaction should succeed");
        }
    }

    // Duplicates should be rejected while present in mempool.
    {
        let utxos_guard = utxos.read().await;
        let mp = mempool.write().await;
        let dup_res = mp
            .add_transaction(txs_in_block[0].clone(), &utxos_guard, &dag)
            .await;
        assert!(dup_res.is_err(), "Duplicate insertion should be rejected");
    }

    // Adapter sees 4 pending before block application.
    let pending_before = adapter.pending_transactions_len().await;
    assert_eq!(
        pending_before, 4,
        "Expected 4 pending before post_add_block"
    );

    // Construct a minimal block containing the 3 txs, with a valid merkle root.
    let merkle_root =
        qanto::qantodag::QantoBlock::compute_merkle_root(&txs_in_block).unwrap_or_default();
    let block = qanto::qantodag::QantoBlock {
        chain_id: 0,
        id: "adapter_test_block".to_string(),
        parents: vec![],
        transactions: txs_in_block.clone(),
        difficulty: 1.0,
        target: None,
        validator: "test_validator".to_string(),
        miner: "test_miner".to_string(),
        nonce: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        height: 1,
        reward: 0,
        effort: 0,
        cross_chain_references: vec![],
        cross_chain_swaps: vec![],
        merkle_root,
        signature: QuantumResistantSignature {
            signer_public_key: vec![0; 32],
            signature: vec![0; 64],
        },
        homomorphic_encrypted: vec![],
        smart_contracts: vec![],
        carbon_credentials: vec![],
        epoch: 0,
        finality_proof: None,
        reservation_snapshot_id: None,
    };

    // Apply post-add logic: remove txs and update UTXO set.
    adapter
        .post_add_block(&block)
        .await
        .expect("post_add_block should succeed");

    // Adapter should now report 1 pending (the extra tx remains).
    let pending_after = adapter.pending_transactions_len().await;
    assert_eq!(
        pending_after, 1,
        "Pending should reflect removal of mined txs"
    );

    // Re-adding previously mined txs should now be allowed and count returns to 4.
    {
        let utxos_guard = utxos.read().await;
        let mp = mempool.write().await;
        for tx in txs_in_block.iter().cloned() {
            mp.add_transaction(tx, &utxos_guard, &dag)
                .await
                .expect("re-add after removal should succeed");
        }
    }

    let final_pending = adapter.pending_transactions_len().await;
    assert_eq!(
        final_pending, 4,
        "Pending should be back to 4 after re-adds"
    );
}
