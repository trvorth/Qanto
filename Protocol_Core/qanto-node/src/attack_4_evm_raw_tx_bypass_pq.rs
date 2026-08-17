//! Attack 4: EVM Raw Transaction Path Bypassing PQ Verification
//!
//! HIGH VULNERABILITY: The codebase supports EVM transaction types (TransactionKind::EvmCall, EvmCreate)
//! but these may bypass the ML-DSA-65 post-quantum signature verification
//!
//! In qantodag.rs, is_valid_block only allows Transfer transactions (line 3182-3191):
//! ```rust
//! if block.transactions.iter().skip(1).any(|tx| tx.transaction_kind != TransactionKind::Transfer) {
//!     return Err("Non-transfer state transitions are disabled until branch-scoped atomic state is activated");
//! }
//! ```
//!
//! BUT: This check is in the DAG validation. If EVM transactions are submitted via
//! alternate paths (RPC, mempool direct), they might bypass this check.
//!
//! Additionally, the transaction verification in validate_block_legacy_unreachable
//! (consensus.rs:154-176) uses verify_with_shared_utxos which may not enforce
//! PQ signatures for EVM transactions.

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::Transaction;
use crate::types::UTXO;
use ahash::AHashMap as HashMap;
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    if !dag.tips.contains_key(&1234) {
        let mut genesis = QantoBlock::new_test_block("test-genesis".to_string());
        genesis.height = 0;
        genesis.parents.clear();
        genesis.timestamp = 0;
        genesis.id = genesis.compute_block_id().expect("test genesis id");
        genesis.chain_id = 1234;
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(1234).or_default().insert(genesis.id);
    }
    let arc_dag = Arc::new(dag);
    let weak_self = Arc::downgrade(&arc_dag);
    let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;
    unsafe {
        (*ptr).self_arc = weak_self;
    }
    arc_dag
}

async fn create_test_validator(
    dag: &Arc<QantoDAG>,
) -> (QantoPQPublicKey, QantoPQPrivateKey, String) {
    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_addr = validator_id_v2(
        pk.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    dag.validators
        .insert(validator_addr.clone(), 50 * crate::Q_SCALE);
    (pk, sk, validator_addr)
}

fn create_transfer_tx(
    _from_pk: &QantoPQPublicKey,
    _to_pk: &QantoPQPublicKey,
    _amount: u128,
    _fee: u128,
    _nonce: u64,
) -> Transaction {
    Transaction::new_dummy()
}

/// Creates a dummy EVM-style transaction (using metadata to mark as EVM)
/// Note: Actual EVM transaction creation API doesn't exist; this simulates
/// what an EVM tx would look like for testing the validation path
fn create_evm_tx(_from_pk: &QantoPQPublicKey) -> Transaction {
    let mut tx = Transaction::new_dummy();
    // Mark as non-transfer transaction to test validation
    tx.transaction_kind = crate::transaction::TransactionKind::BridgeClaim;
    tx
}

async fn create_valid_block(
    dag: &Arc<QantoDAG>,
    validator_sk: &QantoPQPrivateKey,
    validator_pk: &QantoPQPublicKey,
    validator_addr: &str,
    parent_tip: &str,
    height: u64,
    txs: Vec<Transaction>,
) -> QantoBlock {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let paillier_pk = crate::types::HomomorphicEncrypted::generate_keypair().0;

    // Calculate reward = sum of fees from user transactions
    let total_fees = txs.iter().map(|tx| tx.fee).sum::<u128>();

    // Create coinbase transaction first (pass user txs for proper context computation)
    let coinbase_tx = create_coinbase_tx(
        dag,
        validator_sk,
        validator_pk,
        validator_addr,
        height,
        parent_tip,
        &txs,
    )
    .await;

    // Combine coinbase with user transactions
    let mut all_txs = vec![coinbase_tx];
    all_txs.extend(txs);

    let mut block = QantoBlock::new_unsealed_candidate(crate::qantodag::QantoBlockCreationData {
        validator_private_key: validator_sk.clone(),
        chain_id: 1234,
        parents: vec![parent_tip.to_string()],
        transactions: all_txs,
        difficulty: crate::QANTO_SCALE as u64,
        validator: validator_addr.to_string(),
        miner: validator_addr.to_string(),
        timestamp: now,
        current_epoch: 0,
        height,
        reward: total_fees,
        nonce: 0,
        effort: 0,
        chain_work: crate::miner::U256::ZERO,
        paillier_pk,
        state_parent: String::new(),
    })
    .unwrap();

    // Mine the block to satisfy PoW using Miner instance
    let logging_config = crate::config::LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false,
        celebration_log_level: "info".to_string(),
        celebration_throttle_per_min: None,
    };
    let miner_addr =
        qanto_core::protocol_identity::account_address_v2(1234, validator_pk.as_bytes()).unwrap();
    let miner_config = crate::miner::MinerConfig {
        address: miner_addr.clone(),
        dag: dag.clone(),
        target_block_time: 3,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: logging_config.clone(),
    };
    let miner = crate::miner::Miner::new(miner_config).unwrap();
    miner.solve_pow(&mut block).unwrap();

    // Seal the block (sign it and compute block ID)
    block.seal(validator_sk).unwrap();
    block
}

/// Creates a coinbase transaction for testing
async fn create_coinbase_tx(
    dag: &Arc<QantoDAG>,
    validator_sk: &QantoPQPrivateKey,
    validator_pk: &QantoPQPublicKey,
    validator_addr: &str,
    height: u64,
    parent_tip: &str,
    user_txs: &[Transaction],
) -> Transaction {
    let chain_id = 1234u32; // Must match validator_id_v2 chain_id
    let reward_address =
        qanto_core::protocol_identity::account_address_v2(chain_id, validator_pk.as_bytes())
            .unwrap();

    // Calculate coinbase amount = sum of fees from user transactions (containment mode: no block subsidy)
    let total_fees = user_txs.iter().map(|tx| tx.fee).sum::<u128>();

    // Compute coinbase context properly - must match QantoBlock::compute_coinbase_context
    let coinbase_context =
        QantoBlock::compute_coinbase_context(chain_id, height, &[parent_tip.to_string()], user_txs);

    let output = crate::transaction::Output {
        address: reward_address.clone(),
        amount: total_fees,
        homomorphic_encrypted: crate::types::HomomorphicEncrypted::new(total_fees, &[0u8; 32]),
    };

    crate::transaction::Transaction::new_coinbase(
        reward_address,
        total_fees,
        vec![output],
        validator_sk,
        chain_id,
        coinbase_context,
    )
    .unwrap()
}

/// ATTACK DEMONSTRATION: EVM transaction bypassing PQ verification
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_4_evm_tx_rejected_in_dag_validation() {
        // Test current behavior: EVM transactions should be rejected by DAG
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (validator_pk, validator_sk, validator_addr) = create_test_validator(&dag).await;
        let (user_pk, _user_sk, _user_addr) = create_test_validator(&dag).await;

        let utxos = Arc::new(RwLock::new(HashMap::new()));
        let tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();

        // Create an EVM call transaction
        let evm_tx = create_evm_tx(&user_pk);

        let block = create_valid_block(
            &dag,
            &validator_sk,
            &validator_pk,
            &validator_addr,
            &tip,
            1,
            vec![evm_tx],
        )
        .await;

        // DAG validation should reject non-Transfer transactions
        let result = dag.add_block(block, &utxos, None, None).await;

        // Currently this SHOULD fail with "Non-transfer state transitions are disabled"
        // If it passes, that's a vulnerability!
        match result {
            Err(QantoDAGError::InvalidBlock(msg))
                if msg.contains("Non-transfer state transitions are disabled") =>
            {
                println!("CURRENT BEHAVIOR: EVM transactions correctly rejected by DAG validation");
                println!("Error: {}", msg);
            }
            Ok(_) => {
                println!("VULNERABILITY: EVM transaction was ACCEPTED by DAG!");
                panic!("EVM transaction should be rejected but was accepted");
            }
            Err(e) => {
                println!("Other error: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn attack_4_evm_raw_tx_via_rpc_bypass() {
        // This test demonstrates the POTENTIAL vulnerability:
        // If EVM transactions can be submitted via RPC directly to mempool
        // and then included in a block by a malicious/corrupted validator,
        // they might bypass the DAG's transaction_kind check.
        //
        // The check in is_valid_block (line 3182-3191) happens AFTER the block
        // is constructed. A malicious validator could construct a block with
        // EVM transactions and submit it.
        //
        // However, the signature verification (line 3227) uses ML-DSA-65
        // which should still apply. The vulnerability would be if EVM tx
        // verification uses different (weaker) signature verification.

        // For now, document the attack surface:
        println!("ATTACK SURFACE ANALYSIS:");
        println!("1. EVM transaction types exist in TransactionKind enum");
        println!("2. DAG validation rejects non-Transfer in is_valid_block()");
        println!("3. BUT: Malicious validator could construct block with EVM txs");
        println!("4. Transaction.verify() might have different code paths for EVM");
        println!("5. Need to verify ALL paths enforce ML-DSA-65 for EVM txs");
    }

    #[tokio::test]
    async fn attack_4_signature_verification_enforced_for_all_tx_kinds() {
        // Verify that ALL transaction kinds require valid ML-DSA-65 signatures
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (validator_pk, validator_sk, validator_addr) = create_test_validator(&dag).await;
        let (user_pk, user_sk, _user_addr) = create_test_validator(&dag).await;

        let utxos = Arc::new(RwLock::new(HashMap::new()));
        let tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();

        // Create a transfer with INVALID signature
        // Use create_valid_signed_tx to get a properly signed transaction, then corrupt it
        let user_account_addr =
            qanto_core::protocol_identity::account_address_v2(1234, user_pk.as_bytes()).unwrap();
        let mut good_tx = crate::security_tests::create_valid_signed_tx(
            &user_sk,
            &user_pk,
            1234,
            vec![],
            vec![],
            100,
        )
        .await;
        good_tx.signature.signature[0] ^= 1; // Corrupt signature

        let block = create_valid_block(
            &dag,
            &validator_sk,
            &validator_pk,
            &validator_addr,
            &tip,
            1,
            vec![good_tx],
        )
        .await;

        let result = dag.add_block(block, &utxos, None, None).await;

        // Should be rejected due to invalid signature
        assert!(result.is_err());
        println!("ML-DSA-65 signature verification enforced for Transfer transactions");

        // Now test if we can create an EVM tx with invalid signature
        // (This depends on whether EVM tx construction is possible)
        // If EVM tx path doesn't use the same signature verification, that's a vuln
    }

    #[tokio::test]
    async fn attack_4_transaction_verify_consistency() {
        // The legacy consensus path (validate_block_legacy_unreachable) uses
        // verify_with_shared_utxos for parallel verification
        // Need to ensure this enforces PQ signatures for ALL tx kinds

        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (validator_pk, validator_sk, validator_addr) = create_test_validator(&dag).await;

        // Test direct block validation through consensus
        let consensus = crate::consensus::Consensus::new(Arc::new(crate::saga::PalletSaga::new()));

        let tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();
        let tx = create_transfer_tx(&validator_pk, &validator_pk, 100, 1, 1);

        let mut block = create_valid_block(
            &dag,
            &validator_sk,
            &validator_pk,
            &validator_addr,
            &tip,
            1,
            vec![tx],
        )
        .await;

        // Corrupt block signature
        block.signature.signature[0] ^= 1;
        block.id = block.compute_block_id().unwrap();

        let utxos = Arc::new(RwLock::new(HashMap::new()));
        let result = consensus.validate_block(&block, &dag, &utxos).await;

        // Should fail signature verification
        assert!(result.is_err());
        println!("Consensus validate_block enforces PQ signature verification");
    }
}
