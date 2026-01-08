use std::collections::HashMap;
use std::sync::Arc;

use libp2p::{identity, PeerId};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};

use qanto::config::LoggingConfig;
use qanto_core::qanto_native_crypto::qanto_hash;

use qanto::p2p::P2PCommand;
use qanto::post_quantum_crypto::{pq_sign, QantoPQPrivateKey, QantoPQPublicKey};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoBlock, QantoDAG, QantoDAGError, QantoDagConfig, SigningData};
use qanto::saga::PalletSaga;
use qanto::transaction::Transaction;
use qanto::types::QuantumResistantSignature;
use qanto::types::UTXO;
use rand::rngs::OsRng;

fn create_test_dag() -> Arc<QantoDAG> {
    // Use a unique temp directory per test run to avoid RocksDB lock conflicts
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data_dir =
        std::env::temp_dir().join(format!("node_inbound_parent_fetch_test_{}", unique_suffix));
    std::fs::create_dir_all(&data_dir).expect("Failed to create temp storage directory");

    let storage_config = StorageConfig {
        data_dir,
        max_file_size: 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: false,
        sync_writes: false,
        cache_size: 1024,
        compaction_threshold: 100,
        max_open_files: 10,
        ..StorageConfig::default()
    };

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 1000,
        num_chains: 1,
        dev_fee_rate: 0.10,
    };

    let logging_config = LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false,
        celebration_log_level: "info".to_string(),
        celebration_throttle_per_min: Some(10),
    };

    let saga_pallet = Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ));

    QantoDAG::new(dag_config, saga_pallet, storage, logging_config).expect("Failed to create DAG")
}

#[tokio::test]
async fn inbound_block_missing_parent_triggers_requestblock() {
    // Arrange: DAG and channels
    let dag = create_test_dag();
    let utxos: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));

    let (in_tx, mut in_rx) = mpsc::channel::<P2PCommand>(4);
    let (out_tx, mut out_rx) = mpsc::channel::<P2PCommand>(4);

    let source_peer = {
        let kp = identity::Keypair::generate_ed25519();
        PeerId::from(kp.public())
    };

    let mut block = QantoBlock::new_test_block("blk_child_missing_parent".to_string());
    let missing_parent_id = "blk_missing_parent".to_string();
    block.parents = vec![missing_parent_id.clone()];

    // Inject a valid coinbase transaction so validation doesn't fail early on empty txs
    let coinbase_tx = Transaction::new_dummy();
    block.transactions = vec![coinbase_tx.clone()];
    block.merkle_root = QantoBlock::compute_merkle_root(&block.transactions)
        .expect("failed to compute merkle root for test block");
    // Set reward equal to coinbase outputs sum to satisfy reward validation
    let total_coinbase_output: u64 = coinbase_tx.outputs.iter().map(|o| o.amount).sum();
    block.reward = total_coinbase_output;

    // Lower difficulty before signing so signature reflects final header state
    block.difficulty = 0.0001;

    // Ensure block has a valid PQ signature and ID so validation reaches parent checks
    let mut rng = OsRng;
    let private_key: QantoPQPrivateKey = QantoPQPrivateKey::generate(&mut rng);
    let public_key: QantoPQPublicKey = private_key.public_key();
    let signing_data = SigningData {
        chain_id: block.chain_id,
        merkle_root: &block.merkle_root,
        parents: &block.parents,
        transactions: &block.transactions,
        timestamp: block.timestamp,
        difficulty: block.difficulty,
        height: block.height,
        validator: &block.validator,
        miner: &block.miner,
    };
    let pre_signature_data_for_id = QantoBlock::serialize_for_signing(&signing_data)
        .expect("failed to serialize block for signing");
    let new_signature =
        pq_sign(&private_key, &pre_signature_data_for_id).expect("failed to sign block data");
    let new_id = hex::encode(qanto_hash(&pre_signature_data_for_id));
    block.id = new_id;
    block.signature = QuantumResistantSignature {
        signer_public_key: public_key.as_bytes().to_vec(),
        signature: new_signature.as_bytes().to_vec(),
    };
    let mut buf = [0u8; 32];
    primitive_types::U256::MAX.to_big_endian(&mut buf);
    block.target = Some(hex::encode(buf));

    // Solve PoW so block passes PoW validation
    let target_hash_bytes = {
        let t = block.target.clone().expect("header target");
        hex::decode(t).expect("valid target hex")
    };
    let mut found_nonce = false;
    for nonce in 0..100_000u64 {
        block.nonce = nonce;
        let pow_hash = block.hash_for_pow();
        if qanto::miner::Miner::hash_meets_target(pow_hash.as_bytes(), &target_hash_bytes) {
            found_nonce = true;
            break;
        }
    }
    assert!(found_nonce, "failed to find valid nonce for test block PoW");

    // Spawn a minimal command processor harness mirroring node.rs InboundBlock handling
    let dag_clone = dag.clone();
    let utxos_clone = utxos.clone();
    let harness = tokio::spawn(async move {
        while let Some(cmd) = in_rx.recv().await {
            match cmd {
                P2PCommand::InboundBlock { block, source_peer } => {
                    let add_result = dag_clone
                        .add_block(block.clone(), &utxos_clone, None, None)
                        .await;

                    match add_result {
                        Ok(_) => {
                            // Not expected in this test, but ignore
                        }
                        Err(QantoDAGError::InvalidParent(_msg)) => {
                            // Request any missing parents
                            for parent_id in &block.parents {
                                if dag_clone.blocks.get(parent_id).is_none() {
                                    let req = P2PCommand::RequestBlock {
                                        block_id: parent_id.clone(),
                                        peer_id: source_peer,
                                    };
                                    // Forward to out_tx (captures what node.rs would enqueue)
                                    let _ = out_tx.send(req).await;
                                }
                            }
                        }
                        Err(_e) => {
                            // Other errors: ignore for this test
                        }
                    }
                }
                _ => {
                    // Ignore other variants in this harness
                }
            }
        }
    });

    // Act: send the inbound block into the harness
    in_tx
        .send(P2PCommand::InboundBlock { block, source_peer })
        .await
        .expect("failed to send inbound block");

    // Assert: out_rx should receive a RequestBlock for the missing parent and same peer
    let cmd = timeout(Duration::from_secs(3), out_rx.recv())
        .await
        .expect("timeout waiting for RequestBlock")
        .expect("harness channel closed unexpectedly");

    match cmd {
        P2PCommand::RequestBlock { block_id, peer_id } => {
            assert_eq!(block_id, missing_parent_id);
            assert_eq!(peer_id, source_peer);
        }
        other => panic!("unexpected command forwarded: {:?}", other),
    }

    // Cleanup the harness task
    harness.abort();
}
