use crate::post_quantum_crypto::{generate_pq_keypair, pq_sign};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::{Output, Transaction};
use ahash::AHashMap as HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    // Byzantine tests use hardcoded test block rewards that don't match
    // the emission schedule. Bypass reward validation since these tests
    // exercise equivocation/merkle/signature behavior, not reward correctness.
    // Note: test_byzantine_forged_coinbase_reward tests header-vs-tx mismatch
    // which is a separate check from emission-vs-header validation.
    dag.bypass_reward_check.store(true, Ordering::Relaxed);
    let arc_dag = Arc::new(dag);
    let weak_self = Arc::downgrade(&arc_dag);
    let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;
    unsafe {
        (*ptr).self_arc = weak_self;
    }
    arc_dag
}

#[tokio::test]
async fn test_byzantine_double_signing_slashing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_address = hex::encode(pk.as_bytes());
    let initial_stake = 1_000_000 * crate::Q_SCALE;
    dag.validators
        .insert(validator_address.clone(), initial_stake);
    dag.emission
        .write()
        .await
        .update_supply(initial_stake)
        .unwrap();

    // Create block 1
    let mut block1 = QantoBlock::new_test_block("block_1".to_string());
    block1.validator = validator_address.clone();
    block1.height = 1;
    block1.chain_id = 0;
    block1.reward = block1.transactions[0]
        .outputs
        .iter()
        .map(|o| o.amount)
        .sum();
    block1.merkle_root = QantoBlock::compute_merkle_root(&block1.transactions).unwrap();

    // Sign block 1
    let signing_data1 = crate::qantodag::SigningData {
        parents: &block1.parents,
        transactions: &block1.transactions,
        timestamp: block1.timestamp,
        difficulty: block1.difficulty,
        validator: &block1.validator,
        miner: &block1.miner,
        chain_id: block1.chain_id,
        merkle_root: &block1.merkle_root,
        height: block1.height,
    };
    let pre_sig_data1 = QantoBlock::serialize_for_signing(&signing_data1).unwrap();
    let sig1 = pq_sign(&sk, &pre_sig_data1).unwrap();
    block1.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig1.as_bytes().to_vec(),
    };
    block1.id = block1.hash();

    // Create block 2 (different ID, same height, same validator, same chain)
    let mut block2 = QantoBlock::new_test_block("block_2".to_string());
    block2.validator = validator_address.clone();
    block2.height = 1;
    block2.chain_id = 0;
    block2.transactions[0].outputs[0].address = "other_miner".to_string();
    block2.reward = block2.transactions[0]
        .outputs
        .iter()
        .map(|o| o.amount)
        .sum();
    block2.merkle_root = QantoBlock::compute_merkle_root(&block2.transactions).unwrap();

    // Sign block 2
    let signing_data2 = crate::qantodag::SigningData {
        parents: &block2.parents,
        transactions: &block2.transactions,
        timestamp: block2.timestamp,
        difficulty: block2.difficulty,
        validator: &block2.validator,
        miner: &block2.miner,
        chain_id: block2.chain_id,
        merkle_root: &block2.merkle_root,
        height: block2.height,
    };
    let pre_sig_data2 = QantoBlock::serialize_for_signing(&signing_data2).unwrap();
    let sig2 = pq_sign(&sk, &pre_sig_data2).unwrap();
    block2.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig2.as_bytes().to_vec(),
    };
    block2.id = block2.hash();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // Adding block 1 should succeed
    let res1 = dag.add_block(block1, &utxos, None, None).await;
    assert!(res1.is_ok(), "Failed to add block 1: {:?}", res1);
    assert!(res1.unwrap());

    // Adding block 2 (double-signing equivocation) should fail and slash stake
    let res2 = dag.add_block(block2, &utxos, None, None).await;
    assert!(res2.is_err(), "Expected error for double-signing, got Ok");

    // Check error message
    match res2 {
        Err(QantoDAGError::InvalidBlock(msg)) => {
            assert!(msg.contains("Equivocation") || msg.contains("double-signing"));
        }
        other => panic!("Expected equivocation invalid block error, got {:?}", other),
    }

    // Verify validator stake has been slashed by 30%
    let final_stake = *dag.validators.get(&validator_address).unwrap();
    let expected_stake = initial_stake - (initial_stake * 30 / 100);
    assert_eq!(final_stake, expected_stake);
}

#[tokio::test]
async fn test_byzantine_forged_coinbase_reward() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk, sk) = generate_pq_keypair(None).unwrap();

    let mut block = QantoBlock::new_test_block("forged_coinbase".to_string());
    block.validator = hex::encode(pk.as_bytes());
    // Modify reward in header to mismatch transaction outputs sum
    block.reward = 1000 * crate::Q_SCALE;
    block.transactions[0].outputs[0].amount = 50 * crate::Q_SCALE;
    block.merkle_root = QantoBlock::compute_merkle_root(&block.transactions).unwrap();

    // Sign the block header with correct key to pass block signature check
    let signing_data = crate::qantodag::SigningData {
        parents: &block.parents,
        transactions: &block.transactions,
        timestamp: block.timestamp,
        difficulty: block.difficulty,
        validator: &block.validator,
        miner: &block.miner,
        chain_id: block.chain_id,
        merkle_root: &block.merkle_root,
        height: block.height,
    };
    let pre_sig_data = QantoBlock::serialize_for_signing(&signing_data).unwrap();
    let sig = pq_sign(&sk, &pre_sig_data).unwrap();
    block.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig.as_bytes().to_vec(),
    };
    block.id = block.hash();

    let utxos = Arc::new(RwLock::new(HashMap::new()));
    let res = dag.add_block(block, &utxos, None, None).await;
    assert!(res.is_err());
    match res {
        Err(QantoDAGError::RewardMismatch(_, _)) => {}
        other => panic!("Expected reward mismatch error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_byzantine_invalid_merkle_root() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk, sk) = generate_pq_keypair(None).unwrap();

    let mut block = QantoBlock::new_test_block("bad_merkle".to_string());
    block.validator = hex::encode(pk.as_bytes());
    block.merkle_root = "bad_root_hash_value_123456789".to_string();

    // Sign the block header with correct key to pass block signature check
    let signing_data = crate::qantodag::SigningData {
        parents: &block.parents,
        transactions: &block.transactions,
        timestamp: block.timestamp,
        difficulty: block.difficulty,
        validator: &block.validator,
        miner: &block.miner,
        chain_id: block.chain_id,
        merkle_root: &block.merkle_root,
        height: block.height,
    };
    let pre_sig_data = QantoBlock::serialize_for_signing(&signing_data).unwrap();
    let sig = pq_sign(&sk, &pre_sig_data).unwrap();
    block.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig.as_bytes().to_vec(),
    };
    block.id = block.hash();

    let utxos = Arc::new(RwLock::new(HashMap::new()));
    let res = dag.add_block(block, &utxos, None, None).await;
    assert!(res.is_err());
    match res {
        Err(QantoDAGError::MerkleRootMismatch) => {}
        other => panic!("Expected Merkle root mismatch error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_byzantine_invalid_coinbase_signature() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let (_pk_wrong, sk_wrong) = generate_pq_keypair(None).unwrap();

    let output = Output {
        address: "test_miner".to_string(),
        amount: 50 * crate::Q_SCALE,
        homomorphic_encrypted: crate::types::HomomorphicEncrypted::default(),
    };

    // Create coinbase signed with wrong key
    let bad_coinbase = Transaction::new_coinbase(
        "test_miner".to_string(),
        50 * crate::Q_SCALE,
        vec![output],
        &sk_wrong,
        0,
    )
    .unwrap();

    let mut block = QantoBlock::new_test_block("bad_coinbase_sig".to_string());
    block.transactions = vec![bad_coinbase];
    block.merkle_root = QantoBlock::compute_merkle_root(&block.transactions).unwrap();
    block.validator = hex::encode(pk.as_bytes());

    // Sign the block header with correct key to pass block signature check
    let signing_data = crate::qantodag::SigningData {
        parents: &block.parents,
        transactions: &block.transactions,
        timestamp: block.timestamp,
        difficulty: block.difficulty,
        validator: &block.validator,
        miner: &block.miner,
        chain_id: block.chain_id,
        merkle_root: &block.merkle_root,
        height: block.height,
    };
    let pre_sig_data = QantoBlock::serialize_for_signing(&signing_data).unwrap();
    let sig = pq_sign(&sk, &pre_sig_data).unwrap();
    block.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig.as_bytes().to_vec(),
    };
    block.id = block.hash();

    let utxos = Arc::new(RwLock::new(HashMap::new()));
    let res = dag.add_block(block, &utxos, None, None).await;

    assert!(res.is_err());
}

#[tokio::test]
async fn test_clock_drift_guard_positive_and_negative() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_address = hex::encode(pk.as_bytes());
    dag.validators
        .insert(validator_address.clone(), 1_000_000 * crate::Q_SCALE);
    dag.emission
        .write()
        .await
        .update_supply(1_000_000 * crate::Q_SCALE)
        .unwrap();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // Instantiate Miner correctly
    let logging_config = crate::config::LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false,
        celebration_log_level: "info".to_string(),
        celebration_throttle_per_min: None,
    };
    let miner_address = "a".repeat(64);
    let miner_config = crate::miner::MinerConfig {
        address: miner_address.clone(),
        dag: dag.clone(),
        target_block_time: 3,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: logging_config.clone(),
    };
    let miner = Arc::new(crate::miner::Miner::new(miner_config).unwrap());

    // 1. Positive drift: Parent block timestamp is 15 seconds in the future
    let future_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 15;

    let mut block_future = QantoBlock::new_test_block("block_future".to_string());
    block_future.validator = validator_address.clone();
    block_future.timestamp = future_time;
    block_future.height = 1;
    block_future.chain_id = 0;
    block_future.reward = block_future.transactions[0]
        .outputs
        .iter()
        .map(|o| o.amount)
        .sum();
    block_future.merkle_root = QantoBlock::compute_merkle_root(&block_future.transactions).unwrap();

    // Sign the future block
    let signing_data = crate::qantodag::SigningData {
        parents: &block_future.parents,
        transactions: &block_future.transactions,
        timestamp: block_future.timestamp,
        difficulty: block_future.difficulty,
        validator: &block_future.validator,
        miner: &block_future.miner,
        chain_id: block_future.chain_id,
        merkle_root: &block_future.merkle_root,
        height: block_future.height,
    };
    let pre_sig_data = QantoBlock::serialize_for_signing(&signing_data).unwrap();
    let sig = pq_sign(&sk, &pre_sig_data).unwrap();
    block_future.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig.as_bytes().to_vec(),
    };
    block_future.id = block_future.hash();

    // Add future block to DAG
    let added = dag
        .add_block(block_future.clone(), &utxos, None, None)
        .await
        .unwrap();
    assert!(added);

    // Try to create candidate block (which uses the future block as parent tip)
    // Since parent is now + 15s, and local time is now, this should fail with ClockDrift error.
    let res_pos = dag
        .create_candidate_block(
            &sk,
            &pk,
            &validator_address,
            &Arc::new(RwLock::new(crate::mempool::Mempool::new(
                3600, 10_000_000, 100_000,
            ))),
            &utxos,
            0,
            &miner,
            None,
            None,
            None,
        )
        .await;

    assert!(res_pos.is_err(), "Expected error for positive clock drift");
    match res_pos {
        Err(QantoDAGError::ClockDrift {
            drift_secs,
            threshold,
        }) => {
            assert!((14..=16).contains(&drift_secs));
            assert_eq!(threshold, 10);
        }
        other => panic!("Expected ClockDrift error, got {:?}", other),
    }

    // 2. Historical parent timestamp: Parent block timestamp is 15 seconds in the past.
    // This should remain a valid production path so long as the local clock is not lagging
    // behind the parent timestamp.
    let temp_dir2 = tempfile::tempdir().unwrap();
    let dag2 = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir2.path()));
    dag2.validators
        .insert(validator_address.clone(), 1_000_000 * crate::Q_SCALE);
    dag2.emission
        .write()
        .await
        .update_supply(1_000_000 * crate::Q_SCALE)
        .unwrap();

    let miner_config2 = crate::miner::MinerConfig {
        address: miner_address.clone(),
        dag: dag2.clone(),
        target_block_time: 3,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: logging_config.clone(),
    };
    let miner2 = Arc::new(crate::miner::Miner::new(miner_config2).unwrap());

    // A parent block with timestamp now - 15s
    let past_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 15;

    let mut block_past = QantoBlock::new_test_block("block_past".to_string());
    block_past.validator = validator_address.clone();
    block_past.timestamp = past_time;
    block_past.height = 1;
    block_past.chain_id = 0;
    block_past.reward = block_past.transactions[0]
        .outputs
        .iter()
        .map(|o| o.amount)
        .sum();
    block_past.merkle_root = QantoBlock::compute_merkle_root(&block_past.transactions).unwrap();

    // Sign past block
    let signing_data_past = crate::qantodag::SigningData {
        parents: &block_past.parents,
        transactions: &block_past.transactions,
        timestamp: block_past.timestamp,
        difficulty: block_past.difficulty,
        validator: &block_past.validator,
        miner: &block_past.miner,
        chain_id: block_past.chain_id,
        merkle_root: &block_past.merkle_root,
        height: block_past.height,
    };
    let pre_sig_data_past = QantoBlock::serialize_for_signing(&signing_data_past).unwrap();
    let sig_past = pq_sign(&sk, &pre_sig_data_past).unwrap();
    block_past.signature = crate::types::QuantumResistantSignature {
        signer_public_key: pk.as_bytes().to_vec(),
        signature: sig_past.as_bytes().to_vec(),
    };
    block_past.id = block_past.hash();

    // Add past block to DAG
    let added_past = dag2
        .add_block(block_past.clone(), &utxos, None, None)
        .await
        .unwrap();
    assert!(added_past);

    // Try to create candidate block. Parent timestamp is now - 15s and local clock is now,
    // which is a normal catch-up / restart scenario and should be allowed.
    let res_neg = dag2
        .create_candidate_block(
            &sk,
            &pk,
            &validator_address,
            &Arc::new(RwLock::new(crate::mempool::Mempool::new(
                3600, 10_000_000, 100_000,
            ))),
            &utxos,
            0,
            &miner2,
            None,
            None,
            None,
        )
        .await;

    let candidate = res_neg.expect("Historical parent timestamps should not block production");
    assert_eq!(candidate.height, 2);
    assert!(
        candidate.timestamp >= block_past.timestamp,
        "Candidate timestamp must remain non-decreasing relative to parent"
    );
}
