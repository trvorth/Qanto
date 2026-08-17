use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::security_tests::new_dummy_with_storage_path;
use ahash::AHashMap as HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
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
        let val_addr = qanto_core::protocol_identity::validator_id_v2(
            val_pk.as_array(),
            1234,
            qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
        )
        .to_string();
        // Collect tips
        let tips: Vec<String> = dag
            .tips
            .get(&0)
            .map(|set| set.value().iter().cloned().collect())
            .unwrap_or_default();
        let height = i as u64;

        let mut block = QantoBlock::new_test_block(format!("block_{}_{}", val_addr, i));
        block.height = height;
        block.parents = tips;
        block.chain_id = 0;
        block.bind_test_signer(val_sk, 0).unwrap();

        let added = dag
            .add_block(block.clone(), utxos, None, None)
            .await
            .unwrap();
        if added {
            added_block_ids.push(block.id.clone());
        }
    }
    added_block_ids
}

#[tokio::test]
async fn partitioned_single_tip_views_never_synthesize_finality() {
    let temp_a = tempfile::tempdir().unwrap();
    let temp_b = tempfile::tempdir().unwrap();
    let dag_a = wrap_and_init_dag(new_dummy_with_storage_path(temp_a.path()));
    let dag_b = wrap_and_init_dag(new_dummy_with_storage_path(temp_b.path()));
    let (pk_a, sk_a) = generate_pq_keypair(None).unwrap();
    let (pk_b, sk_b) = generate_pq_keypair(None).unwrap();
    let validator_a = qanto_core::protocol_identity::validator_id_v2(
        pk_a.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    let validator_b = qanto_core::protocol_identity::validator_id_v2(
        pk_b.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();

    dag_a
        .validators
        .insert(validator_a.clone(), 50 * crate::Q_SCALE);
    dag_a
        .validators
        .insert(validator_b.clone(), 50 * crate::Q_SCALE);
    dag_b.validators.insert(validator_a, 50 * crate::Q_SCALE);
    dag_b.validators.insert(validator_b, 50 * crate::Q_SCALE);

    let chain_a = grow_chain(
        &dag_a,
        &[(&pk_a, &sk_a)],
        4,
        &Arc::new(RwLock::new(HashMap::new())),
    )
    .await;
    let chain_b = grow_chain(
        &dag_b,
        &[(&pk_b, &sk_b)],
        4,
        &Arc::new(RwLock::new(HashMap::new())),
    )
    .await;

    assert_eq!(chain_a.len(), 4);
    assert_eq!(chain_b.len(), 4);
    assert!(chain_a
        .iter()
        .all(|id| !dag_a.finalized_blocks.contains_key(id)));
    assert!(chain_b
        .iter()
        .all(|id| !dag_b.finalized_blocks.contains_key(id)));
    assert!(dag_a.finalize_blocks().await.is_err());
    assert!(dag_b.finalize_blocks().await.is_err());
}

#[tokio::test]
async fn partition_healing_rejects_a_non_tip_branch_without_branch_state() {
    let temp_a = tempfile::tempdir().unwrap();
    let temp_b = tempfile::tempdir().unwrap();
    let dag_a = wrap_and_init_dag(new_dummy_with_storage_path(temp_a.path()));
    let dag_b = wrap_and_init_dag(new_dummy_with_storage_path(temp_b.path()));
    let (pk_a, sk_a) = generate_pq_keypair(None).unwrap();
    let (pk_b, sk_b) = generate_pq_keypair(None).unwrap();
    let validator_a = qanto_core::protocol_identity::validator_id_v2(
        pk_a.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    let validator_b = qanto_core::protocol_identity::validator_id_v2(
        pk_b.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    for dag in [&dag_a, &dag_b] {
        dag.validators
            .insert(validator_a.clone(), 50 * crate::Q_SCALE);
        dag.validators
            .insert(validator_b.clone(), 50 * crate::Q_SCALE);
    }

    let utxos_a = Arc::new(RwLock::new(HashMap::new()));
    let utxos_b = Arc::new(RwLock::new(HashMap::new()));
    let chain_a = grow_chain(&dag_a, &[(&pk_a, &sk_a)], 2, &utxos_a).await;
    let chain_b = grow_chain(&dag_b, &[(&pk_b, &sk_b)], 2, &utxos_b).await;
    let tip_before = chain_b.last().unwrap().clone();

    let foreign_block = dag_a.blocks.get(&chain_a[0]).unwrap().clone();
    let error = dag_b
        .add_block(foreign_block, &utxos_b, None, None)
        .await
        .unwrap_err();
    assert!(matches!(error, QantoDAGError::InvalidParent(_)));
    assert_eq!(
        dag_b.tips.get(&0).unwrap().iter().next().unwrap().as_str(),
        tip_before
    );
    assert!(dag_b.finalized_blocks.is_empty());
}
