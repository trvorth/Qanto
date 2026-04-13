use my_blockchain::qanto_hash;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::post_quantum_crypto::pq_sign;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoBlock, QantoDAG, QantoDagConfig, SigningData};
use qanto::saga::PalletSaga;
use qanto::transaction::Transaction;
use qanto::types::{QuantumResistantSignature, UTXO};
use qanto::wallet::Wallet;
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

/// Minimal PoW helper to avoid heavy mining in tests
struct MockPoW {
    fixed_difficulty: f64,
}
impl Default for MockPoW {
    fn default() -> Self {
        Self {
            fixed_difficulty: 0.0001,
        }
    }
}
impl MockPoW {
    fn solve_block(&self, block: &mut QantoBlock) -> Result<(), Box<dyn std::error::Error>> {
        block.difficulty = self.fixed_difficulty;
        // Try a range of nonces until target is met
        let target_hash_bytes =
            qanto::miner::Miner::calculate_target_from_difficulty(block.difficulty);
        for nonce in 1..200_000u64 {
            block.nonce = nonce;
            let test_pow_hash = block.hash_for_pow();
            if qanto::miner::Miner::hash_meets_target(test_pow_hash.as_bytes(), &target_hash_bytes)
            {
                return Ok(());
            }
        }
        Err("Mock PoW could not find valid nonce in range".into())
    }
}

#[tokio::test]
async fn test_height_parent_selection_and_balance() {
    qanto::init_test_tracing();

    // Storage setup
    let temp_dir = std::env::temp_dir().join("qanto_test_height_balance");
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
        target_block_time: 60,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };
    let dag_arc = QantoDAG::new(
        dag_config,
        saga.clone(),
        storage,
        qanto::config::LoggingConfig::default(),
    )
    .unwrap();

    // Wallet/miner setup
    let wallet = Wallet::new().unwrap();
    let miner_address = wallet.address();
    dag_arc.add_validator(miner_address.clone(), 1000).await;

    let mempool_arc = Arc::new(RwLock::new(Mempool::new(300, 1024 * 1024, 1000)));
    let utxos_arc: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

    // Add one dummy tx so blocks contain > coinbase
    let dummy_tx = Transaction::new_dummy();
    mempool_arc
        .write()
        .await
        .add_transaction(dummy_tx, &HashMap::new(), &dag_arc)
        .await
        .unwrap();

    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 120,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: qanto::config::LoggingConfig::default(),
    };
    let miner = Miner::new(miner_config).unwrap();
    let (private_key, public_key) = wallet.get_keypair().unwrap();

    // Block 1 candidate — should attach to genesis and have height 1
    let mut block1 = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0,
            &Arc::new(miner.clone()),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(block1.height, 1, "First block height must be 1");

    // Mine (mock) and re-sign block1
    let mock_pow = MockPoW::default();
    mock_pow.solve_block(&mut block1).unwrap();
    let signing_data1 = SigningData {
        chain_id: block1.chain_id,
        merkle_root: &block1.merkle_root,
        parents: &block1.parents,
        transactions: &block1.transactions,
        timestamp: block1.timestamp,
        difficulty: block1.difficulty,
        height: block1.height,
        validator: &block1.validator,
        miner: &block1.miner,
    };
    let bytes1 = QantoBlock::serialize_for_signing(&signing_data1).unwrap();
    let sig1 = pq_sign(&private_key, &bytes1).unwrap();
    block1.id = hex::encode(qanto_hash(&bytes1));
    block1.signature = QuantumResistantSignature {
        signer_public_key: public_key.as_bytes().to_vec(),
        signature: sig1.as_bytes().to_vec(),
    };

    dag_arc
        .add_block(
            block1.clone(),
            &utxos_arc,
            Some(&mempool_arc),
            Some(&miner_address),
        )
        .await
        .unwrap();

    // After adding block1, fast tips must reflect block1.id
    let fast_tips_after_b1 = dag_arc.get_fast_tips(0).await.unwrap();
    assert!(
        fast_tips_after_b1.contains(&block1.id),
        "Fast tips should include block1 as tip"
    );

    // Block 2 candidate — must select block1 as parent and set height=2
    let mut block2 = dag_arc
        .create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            0,
            &Arc::new(miner.clone()),
            None,
            None,
        )
        .await
        .unwrap();

    assert!(
        block2.parents.contains(&block1.id),
        "Second block must reference block1 as parent"
    );
    assert_eq!(
        block2.height,
        block1.height + 1,
        "Height must increment by 1"
    );

    // Mine (mock) and re-sign block2
    mock_pow.solve_block(&mut block2).unwrap();
    let signing_data2 = SigningData {
        chain_id: block2.chain_id,
        merkle_root: &block2.merkle_root,
        parents: &block2.parents,
        transactions: &block2.transactions,
        timestamp: block2.timestamp,
        difficulty: block2.difficulty,
        height: block2.height,
        validator: &block2.validator,
        miner: &block2.miner,
    };
    let bytes2 = QantoBlock::serialize_for_signing(&signing_data2).unwrap();
    let sig2 = pq_sign(&private_key, &bytes2).unwrap();
    block2.id = hex::encode(qanto_hash(&bytes2));
    block2.signature = QuantumResistantSignature {
        signer_public_key: public_key.as_bytes().to_vec(),
        signature: sig2.as_bytes().to_vec(),
    };

    dag_arc
        .add_block(
            block2.clone(),
            &utxos_arc,
            Some(&mempool_arc),
            Some(&miner_address),
        )
        .await
        .unwrap();

    // Validate latest tip and height progression again
    let fast_tips_after_b2 = dag_arc.get_fast_tips(0).await.unwrap();
    assert!(
        fast_tips_after_b2.contains(&block2.id),
        "Fast tips should move to block2"
    );

    // Balance verification: sum UTXOs for miner_address equals sum of validator amounts
    let dev_rate = dag_arc.dev_fee_rate;
    let expected_b1 = block1.reward - ((block1.reward as f64) * dev_rate).floor() as u64;
    let expected_b2 = block2.reward - ((block2.reward as f64) * dev_rate).floor() as u64;

    let utxos_read = utxos_arc.read().await;
    let actual_balance: u64 = utxos_read
        .values()
        .filter(|u| u.address == miner_address)
        .map(|u| u.amount)
        .sum();

    assert_eq!(
        actual_balance,
        expected_b1 + expected_b2,
        "Validator balance should equal sum of credited coinbases"
    );

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}
