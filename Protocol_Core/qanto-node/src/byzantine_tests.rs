use crate::post_quantum_crypto::generate_pq_keypair;
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::security_tests::new_dummy_with_storage_path;
use ahash::AHashMap as HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    dag.bypass_reward_check.store(false, Ordering::Relaxed);
    if !dag.tips.contains_key(&0) {
        let mut genesis = QantoBlock::new_test_block("test-genesis".to_string());
        genesis.height = 0;
        genesis.parents.clear();
        genesis.timestamp = 0;
        genesis.id = genesis.compute_block_id().expect("test genesis id");
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(0).or_default().insert(genesis.id);
    }
    let arc_dag = Arc::new(dag);
    // SELF-ARC-001 (Cycle 023): sound self-reference init. Previously this
    // cast Arc::as_ptr (a *const T) to *mut and wrote through it, which is UB
    // by provenance even with no other live handle.
    let _ = arc_dag.self_arc.set(Arc::downgrade(&arc_dag));
    arc_dag
}

#[tokio::test]
async fn rejects_forged_block_id_before_expensive_validation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));
    let block = QantoBlock::new_test_block("attacker-controlled-id".to_string());
    let utxos = Arc::new(RwLock::new(HashMap::new()));

    let error = dag.is_valid_block(&block, &utxos).await.unwrap_err();
    assert!(
        matches!(error, QantoDAGError::InvalidBlock(message) if message.contains("canonical header"))
    );
}

#[tokio::test]
async fn rejects_zero_declared_difficulty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));
    let mut block = QantoBlock::new_test_block(String::new());
    block.difficulty = 0;
    block.id = block.compute_block_id().unwrap();
    let utxos = Arc::new(RwLock::new(HashMap::new()));

    let error = dag.is_valid_block(&block, &utxos).await.unwrap_err();
    assert!(
        matches!(error, QantoDAGError::InvalidBlock(message) if message.contains("network requirement"))
    );
}

#[tokio::test]
async fn test_byzantine_double_signing_rejected_without_unbound_slashing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_address = qanto_core::protocol_identity::validator_id_v2(
        pk.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
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
    let genesis_id = dag.tips.get(&0).unwrap().iter().next().unwrap().clone();
    block1.parents = vec![genesis_id.clone()];
    block1.height = 1;
    block1.chain_id = 0;
    block1.bind_test_signer(&sk, 0).unwrap();

    // Create block 2 (different ID, same height, same validator, same chain)
    let mut block2 = QantoBlock::new_test_block("block_2".to_string());
    block2.parents = vec![genesis_id.clone()];
    block2.height = 1;
    block2.chain_id = 0;
    block2.timestamp = block1.timestamp.saturating_add(1);
    block2.bind_test_signer(&sk, 0).unwrap();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // Adding block 1 should succeed
    let res1 = dag.add_block(block1, &utxos, None, None).await;
    assert!(res1.is_ok(), "Failed to add block 1: {:?}", res1);
    assert!(res1.unwrap());

    // Recreate the validator's signed view at height one. The block remains in
    // history as evidence, while validation sees the same genesis parent.
    dag.tips.get_mut(&0).unwrap().clear();
    dag.tips.get_mut(&0).unwrap().insert(genesis_id);

    // Adding block 2 (double-signing equivocation) should fail. Until validator
    // addresses are cryptographically bound to PQ keys, automatic slashing must
    // remain disabled to prevent an attacker from naming and slashing a victim.
    let res2 = dag.add_block(block2, &utxos, None, None).await;
    assert!(res2.is_err(), "Expected error for double-signing, got Ok");

    // Check error message
    match res2 {
        Err(QantoDAGError::InvalidBlock(msg)) => {
            assert!(msg.contains("Equivocation") || msg.contains("double-signing"));
        }
        other => panic!("Expected equivocation invalid block error, got {:?}", other),
    }

    // Verify unproven attribution cannot mutate validator stake.
    let final_stake = *dag.validators.get(&validator_address).unwrap();
    assert_eq!(final_stake, initial_stake);
}

#[tokio::test]
async fn test_byzantine_forged_coinbase_reward() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (_pk, sk) = generate_pq_keypair(None).unwrap();

    let mut block = QantoBlock::new_test_block("forged_coinbase".to_string());
    block.parents = vec![dag.tips.get(&0).unwrap().iter().next().unwrap().clone()];
    let validator_id = block.bind_test_signer(&sk, 0).unwrap();
    dag.validators.insert(validator_id, 50 * crate::Q_SCALE);
    // Modify reward in header to mismatch transaction outputs sum (this will also invalidate the block ID)
    block.reward = 1000 * crate::Q_SCALE;

    let utxos = Arc::new(RwLock::new(HashMap::new()));
    let res = dag.add_block(block, &utxos, None, None).await;
    assert!(res.is_err());
    match res {
        Err(QantoDAGError::InvalidBlock(msg)) => {
            assert!(msg.contains("Block ID does not match canonical header"));
        }
        other => panic!(
            "Expected InvalidBlock error due to block ID mismatch, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_byzantine_invalid_merkle_root() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (_pk, sk) = generate_pq_keypair(None).unwrap();

    let mut block = QantoBlock::new_test_block("bad_merkle".to_string());
    block.parents = vec![dag.tips.get(&0).unwrap().iter().next().unwrap().clone()];
    let validator_id = block.bind_test_signer(&sk, 0).unwrap();
    dag.validators.insert(validator_id, 50 * crate::Q_SCALE);
    block.merkle_root = "bad_root_hash_value_123456789".to_string();
    block.id = block.compute_block_id().unwrap();

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

    let (_pk, sk) = generate_pq_keypair(None).unwrap();

    let mut block = QantoBlock::new_test_block("bad_coinbase_sig".to_string());
    block.parents = vec![dag.tips.get(&0).unwrap().iter().next().unwrap().clone()];
    let validator_id = block.bind_test_signer(&sk, 0).unwrap();
    dag.validators.insert(validator_id, 50 * crate::Q_SCALE);
    block.transactions[0].signature.signature[0] ^= 1;

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
    let validator_address = qanto_core::protocol_identity::validator_id_v2(
        pk.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
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
    let miner_address =
        qanto_core::protocol_identity::account_address_v2(1234, pk.as_bytes()).unwrap();
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
    block_future.parents = vec![dag.tips.get(&0).unwrap().iter().next().unwrap().clone()];
    block_future.timestamp = future_time;
    block_future.height = 1;
    block_future.chain_id = 0;
    block_future.bind_test_signer(&sk, 0).unwrap();

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
    block_past.parents = vec![dag2.tips.get(&0).unwrap().iter().next().unwrap().clone()];
    block_past.timestamp = past_time;
    block_past.height = 1;
    block_past.chain_id = 0;
    block_past.bind_test_signer(&sk, 0).unwrap();

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
