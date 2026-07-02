use crate::post_quantum_crypto::{
    generate_pq_keypair, pq_sign, QantoPQPrivateKey, QantoPQPublicKey,
};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::security_tests::new_dummy_with_storage_path;
use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    // Partition tests use hardcoded test block rewards that don't match
    // the emission schedule. Bypass reward validation since these tests
    // exercise consensus/finalization behavior, not reward correctness.
    dag.bypass_reward_check.store(true, Ordering::Relaxed);
    let arc_dag = Arc::new(dag);
    let weak_self = Arc::downgrade(&arc_dag);
    let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;
    unsafe {
        (*ptr).self_arc = weak_self;
    }
    arc_dag
}

// Helper to grow a chain of blocks mined by a list of validators in the DAG
async fn grow_chain(
    dag: &QantoDAG,
    validators: &[(&QantoPQPublicKey, &QantoPQPrivateKey)],
    length: usize,
    utxos: &Arc<RwLock<HashMap<String, crate::types::UTXO>>>,
) -> Vec<String> {
    let mut added_block_ids = Vec::new();
    for i in 1..=length {
        let (val_pk, val_sk) = validators[(i - 1) % validators.len()];
        let val_addr = hex::encode(val_pk.as_bytes());
        // Collect tips
        let tips: Vec<String> = dag
            .tips
            .get(&0)
            .map(|set| set.value().iter().cloned().collect())
            .unwrap_or_default();
        let height = i as u64;

        let mut block = QantoBlock::new_test_block(format!("block_{}_{}", val_addr, i));
        block.validator = val_addr.clone();
        block.height = height;
        block.parents = tips;
        block.chain_id = 0;
        block.reward = block.transactions[0].outputs.iter().map(|o| o.amount).sum();
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
        let sig = pq_sign(val_sk, &pre_sig_data).unwrap();
        block.signature = crate::types::QuantumResistantSignature {
            signer_public_key: val_pk.as_bytes().to_vec(),
            signature: sig.as_bytes().to_vec(),
        };

        block.id = block.hash();

        let added = dag
            .add_block(block.clone(), utxos, None, None)
            .await
            .unwrap();
        if added {
            added_block_ids.push(block.id.clone());
            // Update tips registry manually
            let mut tips_set = dag.tips.entry(0).or_insert_with(HashSet::new);
            tips_set.insert(block.id.clone());

            // Remove parents from tips
            for parent in &block.parents {
                tips_set.remove(parent);
            }
        }

        let _ = dag.finalize_blocks().await;
    }
    added_block_ids
}

#[tokio::test]
async fn test_partition_scenario_a_60_40() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk1, sk1) = generate_pq_keypair(None).unwrap();
    let (pk2, sk2) = generate_pq_keypair(None).unwrap();
    let val1 = hex::encode(pk1.as_bytes());
    let val2 = hex::encode(pk2.as_bytes());

    // Total stake = 100. Scenario A: 60 / 40. Required: 67%
    dag.validators.insert(val1.clone(), 60 * crate::Q_SCALE);
    dag.validators.insert(val2.clone(), 40 * crate::Q_SCALE);
    dag.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // 1. Partition A (60% stake) mines blocks. Neither side should reach quorum.
    let chain_a = grow_chain(&dag, &[(&pk1, &sk1)], 10, &utxos).await;
    assert!(!chain_a.is_empty());

    // Check finalization
    for block_id in &chain_a {
        assert!(
            !dag.finalized_blocks.contains_key(block_id),
            "Partition A (60%) should not finalize block"
        );
    }

    // 2. Partition B (40% stake) mines blocks on a separate DAG instance representing network split
    let temp_dir_b = tempfile::tempdir().unwrap();
    let dag_b = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir_b.path()));
    dag_b.validators.insert(val1.clone(), 60 * crate::Q_SCALE);
    dag_b.validators.insert(val2.clone(), 40 * crate::Q_SCALE);
    dag_b
        .emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();
    let utxos_b = Arc::new(RwLock::new(HashMap::new()));

    let chain_b = grow_chain(&dag_b, &[(&pk2, &sk2)], 10, &utxos_b).await;
    assert!(!chain_b.is_empty());
    for block_id in &chain_b {
        assert!(
            !dag_b.finalized_blocks.contains_key(block_id),
            "Partition B (40%) should not finalize block"
        );
    }
}

#[tokio::test]
async fn test_partition_scenario_b_70_30() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk1, sk1) = generate_pq_keypair(None).unwrap();
    let (pk2, sk2) = generate_pq_keypair(None).unwrap();
    let val1 = hex::encode(pk1.as_bytes());
    let val2 = hex::encode(pk2.as_bytes());

    // Total stake = 100. Scenario B: 70 / 30. Required: 67%
    dag.validators.insert(val1.clone(), 70 * crate::Q_SCALE);
    dag.validators.insert(val2.clone(), 30 * crate::Q_SCALE);
    dag.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // 1. Partition A (70% stake) mines blocks. Since 70 >= 67, it should finalize.
    let chain_a = grow_chain(&dag, &[(&pk1, &sk1)], 10, &utxos).await;
    assert!(!chain_a.is_empty());

    // Check if at least some blocks in the chain are finalized
    let mut finalized_count = 0;
    for block_id in &chain_a {
        if dag.finalized_blocks.contains_key(block_id) {
            finalized_count += 1;
        }
    }
    assert!(
        finalized_count > 0,
        "Partition A (70%) should finalize blocks"
    );

    // 2. Partition B (30% stake) mines blocks. It should not finalize.
    let temp_dir_b = tempfile::tempdir().unwrap();
    let dag_b = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir_b.path()));
    dag_b.validators.insert(val1.clone(), 70 * crate::Q_SCALE);
    dag_b.validators.insert(val2.clone(), 30 * crate::Q_SCALE);
    dag_b
        .emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();
    let utxos_b = Arc::new(RwLock::new(HashMap::new()));

    let chain_b = grow_chain(&dag_b, &[(&pk2, &sk2)], 10, &utxos_b).await;
    assert!(!chain_b.is_empty());
    for block_id in &chain_b {
        assert!(
            !dag_b.finalized_blocks.contains_key(block_id),
            "Partition B (30%) should not finalize block"
        );
    }
}

#[tokio::test]
async fn test_partition_scenario_c_34_33_33() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk1, sk1) = generate_pq_keypair(None).unwrap();
    let (pk2, sk2) = generate_pq_keypair(None).unwrap();
    let (pk3, sk3) = generate_pq_keypair(None).unwrap();
    let val1 = hex::encode(pk1.as_bytes());
    let val2 = hex::encode(pk2.as_bytes());
    let val3 = hex::encode(pk3.as_bytes());

    // Total stake = 100. Scenario C: 34 / 33 / 33. Required: 67%
    dag.validators.insert(val1.clone(), 34 * crate::Q_SCALE);
    dag.validators.insert(val2.clone(), 33 * crate::Q_SCALE);
    dag.validators.insert(val3.clone(), 33 * crate::Q_SCALE);
    dag.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // 1. Partition A (34% stake) mines blocks.
    let chain_a = grow_chain(&dag, &[(&pk1, &sk1)], 10, &utxos).await;
    for block_id in &chain_a {
        assert!(
            !dag.finalized_blocks.contains_key(block_id),
            "Partition A (34%) should not finalize block"
        );
    }

    // 2. Partition B (33% stake) mines blocks on a separate DAG instance
    let temp_dir_b = tempfile::tempdir().unwrap();
    let dag_b = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir_b.path()));
    dag_b.validators.insert(val1.clone(), 34 * crate::Q_SCALE);
    dag_b.validators.insert(val2.clone(), 33 * crate::Q_SCALE);
    dag_b.validators.insert(val3.clone(), 33 * crate::Q_SCALE);
    dag_b
        .emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();
    let utxos_b = Arc::new(RwLock::new(HashMap::new()));

    let chain_b = grow_chain(&dag_b, &[(&pk2, &sk2)], 10, &utxos_b).await;
    for block_id in &chain_b {
        assert!(
            !dag_b.finalized_blocks.contains_key(block_id),
            "Partition B (33%) should not finalize block"
        );
    }

    // 3. Partition C (33% stake) mines blocks on a separate DAG instance
    let temp_dir_c = tempfile::tempdir().unwrap();
    let dag_c = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir_c.path()));
    dag_c.validators.insert(val1.clone(), 34 * crate::Q_SCALE);
    dag_c.validators.insert(val2.clone(), 33 * crate::Q_SCALE);
    dag_c.validators.insert(val3.clone(), 33 * crate::Q_SCALE);
    dag_c
        .emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();
    let utxos_c = Arc::new(RwLock::new(HashMap::new()));

    let chain_c = grow_chain(&dag_c, &[(&pk3, &sk3)], 10, &utxos_c).await;
    for block_id in &chain_c {
        assert!(
            !dag_c.finalized_blocks.contains_key(block_id),
            "Partition C (33%) should not finalize block"
        );
    }
}

#[tokio::test]
async fn test_partition_healing_convergence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    let dag = wrap_and_init_dag(new_dummy_with_storage_path(path));

    let (pk1, sk1) = generate_pq_keypair(None).unwrap();
    let (pk2, sk2) = generate_pq_keypair(None).unwrap();
    let (pk3, sk3) = generate_pq_keypair(None).unwrap();
    let val1 = hex::encode(pk1.as_bytes());
    let val2 = hex::encode(pk2.as_bytes());
    let val3 = hex::encode(pk3.as_bytes());

    // Total stake = 100. Partitions: {val1, val2} (70% stake) vs {val3} (30% stake)
    dag.validators.insert(val1.clone(), 35 * crate::Q_SCALE);
    dag.validators.insert(val2.clone(), 35 * crate::Q_SCALE);
    dag.validators.insert(val3.clone(), 30 * crate::Q_SCALE);
    dag.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // 1. Partition A ({val1, val2}) grows a chain of blocks. It achieves quorum (uses both validators).
    let chain_a = grow_chain(&dag, &[(&pk1, &sk1), (&pk2, &sk2)], 8, &utxos).await;

    // Ensure finality on A
    let _ = dag.finalize_blocks().await;
    let finalized_block_id = &chain_a[0];
    assert!(
        dag.finalized_blocks.contains_key(finalized_block_id),
        "Partition A (70%) should finalize genesis descendant"
    );

    // 2. Partition B ({val3}) grows a divergent chain of blocks (on a separate DAG)
    let temp_dir_b = tempfile::tempdir().unwrap();
    let dag_b = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir_b.path()));
    dag_b.validators.insert(val1.clone(), 35 * crate::Q_SCALE);
    dag_b.validators.insert(val2.clone(), 35 * crate::Q_SCALE);
    dag_b.validators.insert(val3.clone(), 30 * crate::Q_SCALE);
    dag_b
        .emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();
    let utxos_b = Arc::new(RwLock::new(HashMap::new()));

    let chain_b = grow_chain(&dag_b, &[(&pk3, &sk3)], 8, &utxos_b).await;
    assert!(
        !dag_b.finalized_blocks.contains_key(&chain_b[0]),
        "Partition B (30%) should not finalize"
    );

    // 3. Healing: Gossip blocks from Partition A to Partition B
    for block_id in &chain_a {
        let block = dag.blocks.get(block_id).unwrap().clone();
        // Insert into B's DAG
        dag_b.add_block(block, &utxos_b, None, None).await.unwrap();
    }

    // Run finalization on B. It should now finalize the chain of A because it represents 70% stake.
    let _ = dag_b.finalize_blocks().await;
    assert!(
        dag_b.finalized_blocks.contains_key(finalized_block_id),
        "Healed Partition B should converge and finalize A's chain"
    );
}
