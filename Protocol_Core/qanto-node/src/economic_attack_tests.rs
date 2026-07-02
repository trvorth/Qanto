use crate::performance_optimizations::QantoDAGOptimizations;
use crate::post_quantum_crypto::{
    generate_pq_keypair, pq_sign, QantoPQPrivateKey, QantoPQPublicKey,
};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::transaction::Transaction;
use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use sha3::{Digest, Keccak256};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    // Economic tests use hardcoded test block rewards that don't match
    // the emission schedule. Bypass reward validation since these tests
    // exercise stake concentration/finalization thresholds, not reward correctness.
    dag.bypass_reward_check.store(true, Ordering::Relaxed);
    let arc_dag = Arc::new(dag);
    let weak_self = Arc::downgrade(&arc_dag);
    let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;
    unsafe {
        (*ptr).self_arc = weak_self;
    }
    arc_dag
}

async fn grow_chain_for_val(
    dag: &QantoDAG,
    val_pk: &QantoPQPublicKey,
    val_sk: &QantoPQPrivateKey,
    length: usize,
) -> Vec<String> {
    let utxos = Arc::new(RwLock::new(HashMap::new()));
    let mut added = Vec::new();
    let val_addr = hex::encode(val_pk.as_bytes());
    for i in 1..=length {
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

        let ok = dag
            .add_block(block.clone(), &utxos, None, None)
            .await
            .unwrap();
        if ok {
            added.push(block.id.clone());
            let mut tips_set = dag.tips.entry(0).or_insert_with(HashSet::new);
            tips_set.insert(block.id.clone());
            for parent in &block.parents {
                tips_set.remove(parent);
            }
        }
        let _ = dag.finalize_blocks().await;
    }
    added
}

#[tokio::test]
async fn test_stake_concentration() {
    let (pk_a, sk_a) = generate_pq_keypair(None).unwrap();
    let (pk_b, _sk_b) = generate_pq_keypair(None).unwrap();
    let val_a = hex::encode(pk_a.as_bytes());
    let val_b = hex::encode(pk_b.as_bytes());

    // Case 1: 51% Stake. Validator A alone should NOT be able to finalize blocks (quorum is 67%).
    let dag1 = wrap_and_init_dag(QantoDAG::new_dummy_for_verification());
    dag1.validators.insert(val_a.clone(), 51 * crate::Q_SCALE);
    dag1.validators.insert(val_b.clone(), 49 * crate::Q_SCALE);
    dag1.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let chain1 = grow_chain_for_val(&dag1, &pk_a, &sk_a, 10).await;
    for block_id in &chain1 {
        assert!(
            !dag1.finalized_blocks.contains_key(block_id),
            "51% validator should not be able to finalize block alone"
        );
    }

    // Case 2: 67% Stake. Validator A alone should be able to finalize blocks.
    let dag2 = wrap_and_init_dag(QantoDAG::new_dummy_for_verification());
    dag2.validators.insert(val_a.clone(), 67 * crate::Q_SCALE);
    dag2.validators.insert(val_b.clone(), 33 * crate::Q_SCALE);
    dag2.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let chain2 = grow_chain_for_val(&dag2, &pk_a, &sk_a, 10).await;
    let mut finalized_count = 0;
    for block_id in &chain2 {
        if dag2.finalized_blocks.contains_key(block_id) {
            finalized_count += 1;
        }
    }
    assert!(
        finalized_count > 0,
        "67% validator should be able to finalize blocks alone"
    );

    // Case 3: 90% Stake. Validator A alone should be able to finalize blocks.
    let dag3 = wrap_and_init_dag(QantoDAG::new_dummy_for_verification());
    dag3.validators.insert(val_a.clone(), 90 * crate::Q_SCALE);
    dag3.validators.insert(val_b.clone(), 10 * crate::Q_SCALE);
    dag3.emission
        .write()
        .await
        .update_supply(100 * crate::Q_SCALE)
        .unwrap();

    let chain3 = grow_chain_for_val(&dag3, &pk_a, &sk_a, 10).await;
    let mut finalized_count_90 = 0;
    for block_id in &chain3 {
        if dag3.finalized_blocks.contains_key(block_id) {
            finalized_count_90 += 1;
        }
    }
    assert!(
        finalized_count_90 > 0,
        "90% validator should be able to finalize blocks alone"
    );
}

#[tokio::test]
async fn test_delegation_cartels() {
    let dag = wrap_and_init_dag(QantoDAG::new_dummy_for_verification());

    let val_a = "validator_A".to_string();
    let val_b = "validator_B".to_string();

    dag.validators.insert(val_a.clone(), 100 * crate::Q_SCALE);
    dag.validators.insert(val_b.clone(), 100 * crate::Q_SCALE);

    // Initial effective stake is self-stake
    assert_eq!(dag.get_effective_stake(&val_a), 100 * crate::Q_SCALE);

    // Setup delegator balances
    dag.account_state_cache
        .set_balance("delegator_1", 1000 * crate::Q_SCALE);
    dag.account_state_cache
        .set_balance("delegator_2", 1000 * crate::Q_SCALE);

    // Delegate to Validator A
    let mut tx1 = Transaction::new_dummy();
    tx1.sender = "delegator_1".to_string();
    tx1.amount = 500 * crate::Q_SCALE;
    tx1.transaction_kind = crate::transaction::TransactionKind::Delegate;
    tx1.metadata.insert("validator".to_string(), val_a.clone());

    dag.process_delegate(&tx1).await.unwrap();

    // Verify validator A's effective stake has increased
    assert_eq!(dag.get_effective_stake(&val_a), 600 * crate::Q_SCALE);
    // Verify delegator_1's balance is debited
    assert_eq!(
        dag.account_state_cache.get_balance("delegator_1").unwrap(),
        500 * crate::Q_SCALE
    );

    // Delegate 2nd delegation to Validator A
    let mut tx2 = Transaction::new_dummy();
    tx2.sender = "delegator_2".to_string();
    tx2.amount = 300 * crate::Q_SCALE;
    tx2.transaction_kind = crate::transaction::TransactionKind::Delegate;
    tx2.metadata.insert("validator".to_string(), val_a.clone());

    dag.process_delegate(&tx2).await.unwrap();

    // Verify validator A's effective stake has increased again
    assert_eq!(dag.get_effective_stake(&val_a), 900 * crate::Q_SCALE);
}

#[tokio::test]
async fn test_reward_farming() {
    let dag = wrap_and_init_dag(QantoDAG::new_dummy_for_verification());
    let val_a = "validator_A".to_string();
    dag.validators.insert(val_a.clone(), 1000 * crate::Q_SCALE);

    // Test unstake cooldown epoch checks
    let mut tx = Transaction::new_dummy();
    tx.sender = val_a.clone();
    tx.transaction_kind = crate::transaction::TransactionKind::Unstake;
    tx.metadata.insert(
        "unstake_amount".to_string(),
        (100 * crate::Q_SCALE).to_string(),
    );
    tx.metadata
        .insert("last_stake_epoch".to_string(), "5".to_string());

    // With current epoch = 0, unstaking must fail due to cooldown (needs current >= last + 10 = 15)
    dag.current_epoch.store(0, Ordering::SeqCst);
    let utxos = Arc::new(RwLock::new(HashMap::new()));
    let res = dag.process_unstake(&tx, &utxos).await;
    assert!(res.is_err());
    match res {
        Err(QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("cooldown"));
        }
        other => panic!("Expected cooldown error, got {:?}", other),
    }

    // Advance current epoch to 15, unstaking should now succeed
    dag.current_epoch.store(15, Ordering::SeqCst);
    let res2 = dag.process_unstake(&tx, &utxos).await;
    assert!(
        res2.is_ok(),
        "Unstaking failed after cooldown epoch: {:?}",
        res2
    );

    // Check remaining stake
    let remaining_stake = *dag.validators.get(&val_a).unwrap().value();
    assert_eq!(remaining_stake, 900 * crate::Q_SCALE);
}

#[tokio::test]
async fn test_bridge_drain_attempts() {
    let dag = wrap_and_init_dag(QantoDAG::new_dummy_for_verification());

    // Setup validator relayer key and insert into validators map
    let signing_key = k256::ecdsa::SigningKey::random(&mut rand::thread_rng());
    let pub_key = k256::ecdsa::VerifyingKey::from(&signing_key);
    let binding = pub_key.to_encoded_point(false);
    let pub_bytes = binding.as_bytes();

    let mut pub_hasher = Keccak256::new();
    pub_hasher.update(&pub_bytes[1..]);
    let pub_hash = pub_hasher.finalize();
    let eth_addr = &pub_hash[12..];
    let eth_addr_hex = format!("0x{}", hex::encode(eth_addr));
    let padded_validator = crate::transaction::pad_ethereum_address(&eth_addr_hex);

    dag.validators.insert(padded_validator.clone(), 100);

    // Set bridge locked capacity to 100 QNTO
    *dag.total_bridge_locked.write().await = 100;

    let utxos_arc = Arc::new(RwLock::new(HashMap::new()));

    // Submit claim 1 of 40 QNTO
    let mut tx1 = Transaction::new_dummy();
    tx1.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x111".to_string());
    tx1.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx1.metadata
        .insert("bridge_amount".to_string(), "40".to_string());
    tx1.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
    tx1.metadata
        .insert("bridge_merkle_proof".to_string(), "proof".to_string());
    tx1.metadata
        .insert("bridge_receipt_proof".to_string(), "proof".to_string());
    tx1.metadata
        .insert("bridge_block_header".to_string(), "header".to_string());

    // Sign message hash
    let mut data1 = Vec::new();
    data1.extend_from_slice("Ethereum".as_bytes());
    data1.extend_from_slice("0x111".as_bytes());
    data1.extend_from_slice("40".as_bytes());
    data1.extend_from_slice("0xabc".as_bytes());
    let mut hasher = Keccak256::new();
    hasher.update(&data1);
    let (sig1, rec1) = signing_key.sign_prehash(&hasher.finalize()).unwrap();
    let mut sig1_bytes = sig1.to_bytes().to_vec();
    sig1_bytes.push(rec1.to_byte() + 27);
    tx1.metadata.insert(
        "bridge_relayer_signatures".to_string(),
        hex::encode(sig1_bytes),
    );

    let res1 = dag.process_bridge_claim(&tx1, &utxos_arc).await;
    assert!(res1.is_ok());

    // Submit claim 2 of 60 QNTO (remaining collateral is now 0)
    let mut tx2 = Transaction::new_dummy();
    tx2.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x222".to_string());
    tx2.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx2.metadata
        .insert("bridge_amount".to_string(), "60".to_string());
    tx2.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
    tx2.metadata
        .insert("bridge_merkle_proof".to_string(), "proof".to_string());
    tx2.metadata
        .insert("bridge_receipt_proof".to_string(), "proof".to_string());
    tx2.metadata
        .insert("bridge_block_header".to_string(), "header".to_string());

    let mut data2 = Vec::new();
    data2.extend_from_slice("Ethereum".as_bytes());
    data2.extend_from_slice("0x222".as_bytes());
    data2.extend_from_slice("60".as_bytes());
    data2.extend_from_slice("0xabc".as_bytes());
    let mut hasher = Keccak256::new();
    hasher.update(&data2);
    let (sig2, rec2) = signing_key.sign_prehash(&hasher.finalize()).unwrap();
    let mut sig2_bytes = sig2.to_bytes().to_vec();
    sig2_bytes.push(rec2.to_byte() + 27);
    tx2.metadata.insert(
        "bridge_relayer_signatures".to_string(),
        hex::encode(sig2_bytes),
    );

    let res2 = dag.process_bridge_claim(&tx2, &utxos_arc).await;
    assert!(res2.is_ok());

    // Submit claim 3 of 1 QNTO. This should exceed capacity and fail!
    let mut tx3 = Transaction::new_dummy();
    tx3.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x333".to_string());
    tx3.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx3.metadata
        .insert("bridge_amount".to_string(), "1".to_string());
    tx3.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
    tx3.metadata
        .insert("bridge_merkle_proof".to_string(), "proof".to_string());
    tx3.metadata
        .insert("bridge_receipt_proof".to_string(), "proof".to_string());
    tx3.metadata
        .insert("bridge_block_header".to_string(), "header".to_string());

    let mut data3 = Vec::new();
    data3.extend_from_slice("Ethereum".as_bytes());
    data3.extend_from_slice("0x333".as_bytes());
    data3.extend_from_slice("1".as_bytes());
    data3.extend_from_slice("0xabc".as_bytes());
    let mut hasher = Keccak256::new();
    hasher.update(&data3);
    let (sig3, rec3) = signing_key.sign_prehash(&hasher.finalize()).unwrap();
    let mut sig3_bytes = sig3.to_bytes().to_vec();
    sig3_bytes.push(rec3.to_byte() + 27);
    tx3.metadata.insert(
        "bridge_relayer_signatures".to_string(),
        hex::encode(sig3_bytes),
    );

    let res3 = dag.process_bridge_claim(&tx3, &utxos_arc).await;
    assert!(res3.is_err());
    match res3 {
        Err(QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("exceeds available bridge locked capacity"));
        }
        other => panic!("Expected capacity exceeded error, got {:?}", other),
    }
}
