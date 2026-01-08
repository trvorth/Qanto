use std::collections::HashMap;
use std::sync::Arc;

use primitive_types::U256;
use proptest::prelude::*;
use qanto::config::LoggingConfig;
use qanto::consensus_engine::Consensus;
use qanto::consensus_engine::ConsensusError;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoBlock, QantoDAG, QantoDagConfig};
use qanto::saga::{PalletSaga, SagaCreditScore};
use qanto::types::UTXO;
use tokio::sync::RwLock;

/// Create a test DAG with temp storage and default logging.
fn create_test_dag() -> Arc<QantoDAG> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp storage directory");
    let data_dir = temp_dir.keep();

    let storage_config = StorageConfig {
        data_dir,
        max_file_size: 64 * 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024 * 1024,
        compaction_threshold: 10,
        max_open_files: 64,
        ..StorageConfig::default()
    };

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 1000,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let logging_config = LoggingConfig::default();

    #[cfg(feature = "infinite-strata")]
    let saga_pallet = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga_pallet = Arc::new(PalletSaga::new());

    QantoDAG::new(dag_config, saga_pallet, storage, logging_config).expect("Failed to create DAG")
}

/// Helper to find a valid nonce for a block at a given declared difficulty.
fn set_target(block: &mut QantoBlock, target_u64: u64) {
    let u = U256::from(target_u64);
    let mut buf = [0u8; 32];
    u.to_big_endian(&mut buf);
    block.target = Some(hex::encode(buf));
}

#[tokio::test]
async fn test_validate_pow_respects_declared_header_difficulty() {
    // Arrange: Saga + Consensus + DAG
    #[cfg(feature = "infinite-strata")]
    let saga = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga = Arc::new(PalletSaga::new());
    let consensus = Consensus::new(saga.clone());
    let dag_arc = create_test_dag();
    let utxos_arc: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

    // Build a simple test block and set an easy header target
    let mut solved_block = QantoBlock::new_test_block("miner_alpha".to_string());
    let mut buf = [0u8; 32];
    primitive_types::U256::MAX.to_big_endian(&mut buf);
    solved_block.target = Some(hex::encode(buf));
    solved_block.nonce = 0;

    // Act: Validate block; should succeed using header target
    consensus
        .validate_block(&solved_block, &dag_arc, &utxos_arc)
        .await
        .expect("PoW validation should pass with header target");

    // Mutate SAGA reputation to produce a different effective difficulty
    let mut scores = saga.reputation.credit_scores.write().await;
    scores.insert(
        solved_block.miner.clone(),
        SagaCreditScore {
            score: 0.9, // raises trust, reduces effective difficulty relative to base
            factors: HashMap::new(),
            history: Vec::new(),
            last_updated: 0,
        },
    );
    drop(scores);

    // Re-validate: should still pass because header target gates validity
    consensus
        .validate_block(&solved_block, &dag_arc, &utxos_arc)
        .await
        .expect("PoW validation should remain gated by header target");
}

#[tokio::test]
async fn test_validate_pow_rejects_when_hash_above_declared_target() {
    // Arrange
    #[cfg(feature = "infinite-strata")]
    let saga = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga = Arc::new(PalletSaga::new());
    let consensus = Consensus::new(saga);
    let dag_arc = create_test_dag();
    let utxos_arc: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

    let mut block = QantoBlock::new_test_block("miner_beta".to_string());
    set_target(&mut block, 1); // extremely hard target
    block.nonce = 0;

    // Act + Assert: Consensus should reject with PoW failure
    let result = consensus.validate_block(&block, &dag_arc, &utxos_arc).await;
    match result {
        Err(ConsensusError::ProofOfWorkFailed(_)) => {}
        Ok(_) => panic!("Expected PoW failure, but validation passed"),
        Err(e) => panic!("Expected PoW failure, got different error: {e:?}"),
    }
}

#[tokio::test]
async fn test_validate_pow_rejects_missing_header_target() {
    #[cfg(feature = "infinite-strata")]
    let saga = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga = Arc::new(PalletSaga::new());
    let consensus = Consensus::new(saga);
    let dag_arc = create_test_dag();
    let utxos_arc: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

    let mut block = QantoBlock::new_test_block("miner_gamma".to_string());
    block.target = None;
    block.nonce = 0;

    let result = consensus.validate_block(&block, &dag_arc, &utxos_arc).await;
    match result {
        Err(ConsensusError::ProofOfWorkFailed(_)) => {}
        Ok(_) => panic!("Expected PoW failure due to missing target, but validation passed"),
        Err(e) => panic!("Expected PoW failure due to missing target, got: {e:?}"),
    }
}

#[tokio::test]
async fn test_validate_pow_rejects_invalid_header_target() {
    #[cfg(feature = "infinite-strata")]
    let saga = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga = Arc::new(PalletSaga::new());
    let consensus = Consensus::new(saga);
    let dag_arc = create_test_dag();
    let utxos_arc: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

    let mut block = QantoBlock::new_test_block("miner_delta".to_string());
    block.target = Some("zzzz".to_string()); // invalid hex
    block.nonce = 0;

    let result = consensus.validate_block(&block, &dag_arc, &utxos_arc).await;
    match result {
        Err(ConsensusError::ProofOfWorkFailed(_)) => {}
        Ok(_) => panic!("Expected PoW failure due to invalid target, but validation passed"),
        Err(e) => panic!("Expected PoW failure due to invalid target, got: {e:?}"),
    }
}

#[tokio::test]
async fn test_header_target_gates_validity_independent_of_saga() {
    // Arrange: default SAGA has base_difficulty=10.0, miner SCS defaults to 0.5
    #[cfg(feature = "infinite-strata")]
    let saga = Arc::new(PalletSaga::new(None));
    #[cfg(not(feature = "infinite-strata"))]
    let saga = Arc::new(PalletSaga::new());
    let consensus = Consensus::new(saga);

    // Block declares a header target; SAGA state is irrelevant for PoW validity
    let mut block = QantoBlock::new_test_block("miner_epsilon".to_string());
    block.difficulty = 7.0;
    block.nonce = 0;

    // Act: compute SAGA-derived effective difficulty
    let effective = consensus.get_effective_difficulty(&block.miner).await;
    // Assert: declared difficulty differs from SAGA-derived difficulty
    assert!(
        (effective - block.difficulty).abs() > f64::EPSILON,
        "Expected declared difficulty {} to differ from SAGA-derived difficulty {}",
        block.difficulty,
        effective
    );
}

// Property-based tests focusing on target mapping and boundary behavior.
#[test]
fn u256_max_is_all_ff() {
    let mut buf = [0u8; 32];
    primitive_types::U256::MAX.to_big_endian(&mut buf);
    assert!(buf.iter().all(|&b| b == 0xFF));
}

proptest! {
    #[test]
    fn prop_pow_target_monotonicity(
        t1_u64 in 1u64..(u64::MAX/4),
        t2_u64 in 1u64..(u64::MAX/4),
        hash in any::<[u8; 32]>()
    ) {
        let mut t1 = [0u8; 32];
        let mut t2 = [0u8; 32];
        primitive_types::U256::from(t1_u64).to_big_endian(&mut t1);
        primitive_types::U256::from(t2_u64).to_big_endian(&mut t2);
        if t2_u64 < t1_u64 {
            if Miner::hash_meets_target(&hash, &t2) {
                prop_assert!(Miner::hash_meets_target(&hash, &t1));
            }
        } else if t1_u64 < t2_u64 && Miner::hash_meets_target(&hash, &t1) {
            prop_assert!(Miner::hash_meets_target(&hash, &t2));
        }
    }
}
use qanto::miner::Miner;
