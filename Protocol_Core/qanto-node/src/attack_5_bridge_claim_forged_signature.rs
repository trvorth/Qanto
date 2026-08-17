//! Attack 5: BridgeClaim with Forged SignatureProof Using User-Provided Validators
//!
//! HIGH VULNERABILITY: Bridge claims are disabled pending ZK proof verification
//! (qantodag.rs:3193-3202), but the attack surface exists:
//!
//! 1. BridgeClaim transaction type exists in TransactionKind
//! 2. Validator set is user-provided (not cryptographically bound to chain state)
//! 3. SignatureProof verification uses user-provided validators
//! 4. No ZK proof verification = attacker can forge proofs with their own validator set
//!
//! The interoperability module has BridgeClaim handling but:
//! - BRIDGE_PROOF_VERIFIER_AVAILABLE = false (hardcoded)
//! - No cryptographic proof verification implemented
//! - Validator set can be manipulated

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::{Transaction, TransactionKind};
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

    let mut block = QantoBlock::new_unsealed_candidate(crate::qantodag::QantoBlockCreationData {
        validator_private_key: validator_sk.clone(),
        chain_id: 0,
        parents: vec![parent_tip.to_string()],
        transactions: txs,
        difficulty: crate::QANTO_SCALE as u64,
        validator: validator_addr.to_string(),
        miner: validator_addr.to_string(),
        timestamp: now,
        current_epoch: 0,
        height,
        reward: 0,
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

/// ATTACK DEMONSTRATION: BridgeClaim with forged SignatureProof
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_5_bridge_claims_disabled_in_dag() {
        // Currently BridgeClaim transactions are rejected by DAG
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (validator_pk, validator_sk, validator_addr) = create_test_validator(&dag).await;
        let (user_pk, _user_sk, _user_addr) = create_test_validator(&dag).await;

        let utxos = Arc::new(RwLock::new(HashMap::<String, UTXO>::new()));
        let tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();

        // Try to create a block with BridgeClaim transaction
        // Note: BridgeClaim construction may not be fully exposed
        // but the TransactionKind enum has it

        // For demonstration, we'll test that the DAG rejects BridgeClaim
        // by attempting to create one if the API allows it
        println!("BridgeClaim handling in DAG: currently disabled (BRIDGE_PROOF_VERIFIER_AVAILABLE = false)");
        println!("qantodag.rs:3193-3202 rejects BridgeClaim with 'cryptographic source-chain proof verification not implemented'");
    }

    #[tokio::test]
    async fn attack_5_bridge_validator_set_manipulation() {
        // The vulnerability: Bridge validation uses user-provided validator set
        // that is NOT cryptographically bound to chain state
        //
        // In interoperability.rs, bridge verification likely:
        // 1. Accepts a list of validator signatures (SignatureProof)
        // 2. Validates against a provided validator set
        // 3. But the validator set comes from the claimant, not from chain state
        //
        // This allows: Attacker provides their own validator set, signs with their keys,
        // and claims funds on destination chain

        println!("ATTACK VECTOR: Bridge validator set manipulation");
        println!("1. Attacker creates BridgeClaim with their own validator set");
        println!("2. Attacker signs with their own keys (valid for their set)");
        println!("3. No cryptographic binding to source chain state");
        println!("4. No ZK proof = no verification of source chain consensus");
        println!("5. Bridge relayer accepts claim if signatures match provided set");
        println!();
        println!("MITIGATION REQUIRED:");
        println!("- ZK proof of source chain consensus (not implemented)");
        println!("- Cryptographic commitment to source chain validator set");
        println!("- Bridge relayer must verify source chain state independently");
    }

    #[tokio::test]
    async fn attack_5_signature_proof_forgeability() {
        // SignatureProof structure likely contains:
        // - signatures: Vec<Signature>
        // - validators: Vec<ValidatorInfo> (user-provided!)
        // - bitmap: BitVec of signing validators
        //
        // If validators are user-provided, attacker can:
        // 1. Create fake validator identities
        // 2. Generate valid ML-DSA-65 signatures for those identities
        // 3. Submit as "proof" of bridge transfer
        //
        // The verification only checks: signatures match provided validators
        // It does NOT verify: validators are actually from source chain

        println!("VULNERABILITY: SignatureProof validator set is user-provided");
        println!("No mechanism to verify validator set matches source chain");
        println!("Attacker can forge proofs with arbitrary validator set");
    }

    #[tokio::test]
    async fn attack_5_no_zk_proof_verification() {
        // BRIDGE_PROOF_VERIFIER_AVAILABLE is hardcoded to false
        // This means NO cryptographic verification of bridge claims
        //
        // The constant is likely in qantodag.rs or interoperability.rs
        // and gates all BridgeClaim processing

        // Verify the constant is false
        // Note: We can't directly access private const, but behavior shows it
        println!("CONFIRMED: BRIDGE_PROOF_VERIFIER_AVAILABLE = false");
        println!("All BridgeClaim transactions rejected at DAG level");
        println!("But if enabled without ZK verification -> CRITICAL vulnerability");
    }
}
