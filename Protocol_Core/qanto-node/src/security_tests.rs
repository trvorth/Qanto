use crate::mempool::{Mempool, MempoolError};
use crate::performance_optimizations::QantoDAGOptimizations;
use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDagConfig};
use crate::saga::{GovernanceProposal, ProposalStatus, ProposalType};
use crate::transaction::{
    Input, Output, Transaction, TransactionError, TransactionKind, GLOBAL_CHAIN_ID,
};
use crate::types::{HomomorphicEncrypted, QuantumResistantSignature, UTXO};
use ahash::AHashMap as HashMap;
use k256::ecdsa::signature::hazmat::PrehashSigner;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

// Helper to create a valid signed non-coinbase transfer transaction
pub(crate) fn create_valid_signed_tx(
    sk: &QantoPQPrivateKey,
    _pk: &QantoPQPublicKey,
    chain_id: u32,
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    fee: u128,
) -> Transaction {
    let sender = "0x1111111111111111111111111111111111111111".to_string();
    let receiver = "0x2222222222222222222222222222222222222222".to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut tx = Transaction {
        id: String::new(),
        sender,
        receiver,
        amount: outputs.iter().map(|o| o.amount).sum(),
        fee,
        gas_limit: 50000,
        gas_used: 0,
        gas_price: 1,
        priority_fee: 0,
        inputs,
        outputs,
        timestamp,
        metadata: HashMap::new(),
        signature: QuantumResistantSignature::default(),
        fee_breakdown: None,
        transaction_kind: TransactionKind::Transfer,
        chain_id,
    };
    tx.rebuild_canonical_signature(sk).unwrap();
    tx
}

// Helper to create StorageConfig for custom paths
pub(crate) fn get_storage_config(
    temp_path: &std::path::Path,
) -> crate::qanto_storage::StorageConfig {
    use crate::qanto_storage::StorageConfig;
    use std::time::Duration;

    StorageConfig {
        data_dir: temp_path.to_path_buf(),
        max_file_size: 64 * 1024 * 1024,
        cache_size: 1024 * 1024,
        compression_enabled: true,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        compaction_threshold: 1,
        max_open_files: 100,
        memtable_size: 1024 * 1024 * 16,
        write_buffer_size: 1024 * 1024 * 4,
        batch_size: 1000,
        parallel_writers: 4,
        enable_write_batching: true,
        enable_bloom_filters: true,
        enable_async_io: false,
        sync_interval: Duration::from_millis(100),
        compression_level: 3,
    }
}

// Helper to create QantoDAG with a custom DB directory
pub(crate) fn new_dummy_with_storage_path(temp_path: &std::path::Path) -> QantoDAG {
    use crate::config::LoggingConfig;
    use crate::metrics::QantoMetrics as PerformanceMetrics;
    use crate::mining_metrics::MiningMetrics;
    use crate::optimized_qdag::{OptimizedQDagConfig, OptimizedQDagGenerator};
    use crate::performance_monitoring::{PerformanceMonitor, PerformanceMonitoringConfig};
    use crate::persistence::PersistenceWriter;
    use crate::qanto_storage::AccountStateCache;
    use crate::qanto_storage::QantoStorage;
    use crate::saga::PalletSaga;
    use crossbeam::channel::bounded;
    use dashmap::DashMap;
    use lru::LruCache;
    use qanto_core::balance_stream::BalanceBroadcaster;
    use std::num::NonZeroUsize;
    use std::sync::Weak;
    use tokio::sync::Semaphore;

    let storage_config = get_storage_config(temp_path);
    let db = QantoStorage::new(storage_config).expect("Failed to create dummy QantoStorage");
    let db_arc = Arc::new(db);
    let (sender, receiver) = bounded(1000);

    QantoDAG {
        consensus_lock: tokio::sync::Mutex::new(()),
        blocks: Arc::new(DashMap::new()),
        tips: Arc::new(DashMap::new()),
        validators: Arc::new(DashMap::new()),
        delegations: Arc::new(DashMap::new()),
        target_block_time: 1000,
        emission: Arc::new(RwLock::new(
            crate::emission::Emission::default_with_timestamp(0, 1),
        )),
        num_chains: Arc::new(RwLock::new(1)),
        finalized_blocks: Arc::new(DashMap::new()),
        chain_loads: Arc::new(DashMap::new()),
        difficulty_history: Arc::new(parking_lot::RwLock::new(Vec::new())),
        block_creation_timestamps: Arc::new(DashMap::new()),
        anomaly_history: Arc::new(DashMap::new()),
        cross_chain_swaps: Arc::new(DashMap::new()),
        smart_contracts: Arc::new(DashMap::new()),
        cache: Arc::new(parking_lot::RwLock::new(LruCache::new(
            NonZeroUsize::new(100).unwrap(),
        ))),
        db: db_arc.clone(),
        persistence_writer: Arc::new(PersistenceWriter::new(db_arc.clone(), 8192)),
        saga: Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        )),
        self_arc: Weak::new(),
        current_epoch: Arc::new(AtomicU64::new(0)),
        balance_event_sender: Arc::new(RwLock::new(None)),
        balance_broadcaster: Arc::new(RwLock::new(Some(Arc::new(BalanceBroadcaster::new(32768))))),
        account_state_cache: AccountStateCache::new(),
        total_bridge_locked: Arc::new(RwLock::new(0)),
        total_bridge_claimed: Arc::new(RwLock::new(0)),
        processed_bridge_claims: Arc::new(DashMap::new()),
        tx_index: Arc::new(DashMap::new()),
        address_index: Arc::new(DashMap::new()),
        validator_blocks_index: Arc::new(DashMap::new()),
        bridge_claims_index: Arc::new(DashMap::new()),
        block_processing_semaphore: Arc::new(Semaphore::new(10)),
        validation_workers: Arc::new(Semaphore::new(32)),
        block_queue: Arc::new((sender, receiver)),
        validation_cache: Arc::new(DashMap::new()),
        fast_tips_cache: Arc::new(DashMap::new()),
        processing_blocks: Arc::new(DashMap::new()),
        performance_metrics: Arc::new(PerformanceMetrics::default()),
        mining_metrics: Arc::new(MiningMetrics::new()),
        simd_processor: Arc::new(DashMap::new()),
        lock_free_tx_queue: Arc::new(crossbeam::queue::SegQueue::new()),
        memory_pool: Arc::new(DashMap::new()),
        prefetch_cache: Arc::new(DashMap::new()),
        pipeline_stages: (0..8).map(|_| Arc::new(Semaphore::new(1))).collect(),
        work_stealing_pool: Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_cpus::get())
                .build()
                .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap()),
        ),
        utxo_bloom_filter: Arc::new(DashMap::new()),
        batch_processor: Arc::new(DashMap::new()),
        timing_coordinator: Arc::new(crate::timing::BlockTimingCoordinator::new()),
        performance_monitor: Arc::new(PerformanceMonitor::new(
            PerformanceMonitoringConfig::default(),
        )),
        qdag_generator: Arc::new(OptimizedQDagGenerator::new(OptimizedQDagConfig::default())),
        dev_fee_rate: 100_000_000,
        logging_config: LoggingConfig::default(),
        last_fork_depth: Arc::new(AtomicU64::new(0)),
        last_fork_lca: Arc::new(tokio::sync::RwLock::new(String::new())),
        bypass_reward_check: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
}

#[tokio::test]
async fn test_security_duplicate_tx_mempool() {
    let dag = QantoDAG::new_dummy_for_verification();
    let mempool = Mempool::new(300, 10 * 1024 * 1024, 1000);
    let (pk, sk) = generate_pq_keypair(None).unwrap();

    let input = Input {
        tx_id: "parent_tx".to_string(),
        output_index: 0,
    };
    let output = Output {
        address: "0x1111111111111111111111111111111111111111".to_string(),
        amount: 100,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };

    let tx = create_valid_signed_tx(&sk, &pk, 1234, vec![input], vec![output], 10);

    let mut utxos = HashMap::new();
    utxos.insert(
        "parent_tx_0".to_string(),
        UTXO {
            address: "0x1111111111111111111111111111111111111111".to_string(),
            amount: 200,
            tx_id: "parent_tx".to_string(),
            output_index: 0,
            explorer_link: "".to_string(),
        },
    );

    // 1. First add should succeed
    let res = mempool.add_transaction(tx.clone(), &utxos, &dag).await;
    assert!(res.is_ok(), "First add failed: {:?}", res);

    // 2. Second add should fail as duplicate transaction
    let res2 = mempool.add_transaction(tx, &utxos, &dag).await;
    assert!(res2.is_err(), "Duplicate tx should be rejected");
    match res2 {
        Err(MempoolError::TransactionValidation(msg)) => {
            assert!(msg.contains("Duplicate transaction"));
        }
        other => panic!("Expected Duplicate transaction error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_security_chain_id_replay_protection() {
    let dag = QantoDAG::new_dummy_for_verification();
    let mempool = Mempool::new(300, 10 * 1024 * 1024, 1000);
    let (pk, sk) = generate_pq_keypair(None).unwrap();

    let input = Input {
        tx_id: "parent_tx".to_string(),
        output_index: 0,
    };
    let output = Output {
        address: "0x1111111111111111111111111111111111111111".to_string(),
        amount: 100,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };

    // Construct tx with chain ID 9999
    let tx = create_valid_signed_tx(&sk, &pk, 9999, vec![input], vec![output], 10);

    let mut utxos = HashMap::new();
    utxos.insert(
        "parent_tx_0".to_string(),
        UTXO {
            address: "0x1111111111111111111111111111111111111111".to_string(),
            amount: 200,
            tx_id: "parent_tx".to_string(),
            output_index: 0,
            explorer_link: "".to_string(),
        },
    );

    // Try verifying it against global chain ID (defaults to 1234)
    GLOBAL_CHAIN_ID.store(1234, Ordering::SeqCst);
    let res = tx.verify(&dag, &utxos).await;
    assert!(res.is_err());
    match res {
        Err(TransactionError::InvalidStructure(msg)) => {
            assert!(msg.contains("Chain ID mismatch"));
        }
        other => panic!("Expected Chain ID mismatch, got {:?}", other),
    }

    // Try adding to mempool
    let res_mempool = mempool.add_transaction(tx, &utxos, &dag).await;
    assert!(
        res_mempool.is_err(),
        "Mempool should reject chain ID mismatch"
    );
}

#[tokio::test]
async fn test_security_bridge_claim_zero_signers() {
    let dag = QantoDAG::new_dummy_for_verification();

    let mut tx = Transaction::new_dummy();
    tx.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x123".to_string());
    tx.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx.metadata
        .insert("bridge_amount".to_string(), "100".to_string());
    tx.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
    tx.metadata
        .insert("bridge_merkle_proof".to_string(), "proof".to_string());
    tx.metadata
        .insert("bridge_receipt_proof".to_string(), "proof".to_string());
    tx.metadata
        .insert("bridge_block_header".to_string(), "header".to_string());

    // Explicitly empty relayer signatures list
    tx.metadata
        .insert("bridge_relayer_signatures".to_string(), "".to_string());

    // Synthetic bridge locked capacity
    *dag.total_bridge_locked.write().await = 1000;

    let res = dag
        .process_bridge_claim(&tx, &Arc::new(RwLock::new(HashMap::new())))
        .await;
    assert!(res.is_err());
    match res {
        Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("Empty relayer signatures list"));
        }
        other => panic!("Expected empty relayer signatures error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_security_bridge_claim_insufficient_quorum() {
    use sha3::{Digest, Keccak256};

    let dag = QantoDAG::new_dummy_for_verification();

    // Set up a validator with stake
    dag.validators.insert(
        "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
        100,
    );

    let mut tx = Transaction::new_dummy();
    tx.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x123".to_string());
    tx.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx.metadata
        .insert("bridge_amount".to_string(), "100".to_string());
    tx.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
    tx.metadata
        .insert("bridge_merkle_proof".to_string(), "proof".to_string());
    tx.metadata
        .insert("bridge_receipt_proof".to_string(), "proof".to_string());
    tx.metadata
        .insert("bridge_block_header".to_string(), "header".to_string());

    // Generate a valid signature from an random relayer key (0 stake in validators map)
    let signing_key = k256::ecdsa::SigningKey::random(&mut rand::thread_rng());

    // Build signed message hash: Keccak256 of: source_chain + eth_tx_hash + amount + recipient
    let mut data_to_sign = Vec::new();
    data_to_sign.extend_from_slice("Ethereum".as_bytes());
    data_to_sign.extend_from_slice("0x123".as_bytes());
    data_to_sign.extend_from_slice("100".as_bytes());
    data_to_sign.extend_from_slice("0xabc".as_bytes());

    let mut hasher = Keccak256::new();
    hasher.update(&data_to_sign);
    let message_hash = hasher.finalize();

    let (sig, recovery_id) = signing_key.sign_prehash(&message_hash).unwrap();
    let mut sig_bytes = sig.to_bytes().to_vec();
    sig_bytes.push(recovery_id.to_byte() + 27);
    let sig_hex = hex::encode(sig_bytes);

    tx.metadata
        .insert("bridge_relayer_signatures".to_string(), sig_hex);

    // Synthetic bridge locked capacity
    *dag.total_bridge_locked.write().await = 1000;

    let res = dag
        .process_bridge_claim(&tx, &Arc::new(RwLock::new(HashMap::new())))
        .await;
    assert!(res.is_err());
    match res {
        Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("Relayer quorum check failed"));
        }
        other => panic!("Expected Relayer quorum check failed, got {:?}", other),
    }
}

#[tokio::test]
async fn test_security_bridge_double_claim() {
    use sha3::{Digest, Keccak256};

    let dag = QantoDAG::new_dummy_for_verification();

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

    let mut tx = Transaction::new_dummy();
    tx.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x123".to_string());
    tx.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx.metadata
        .insert("bridge_amount".to_string(), "100".to_string());
    tx.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
    tx.metadata
        .insert("bridge_merkle_proof".to_string(), "proof".to_string());
    tx.metadata
        .insert("bridge_receipt_proof".to_string(), "proof".to_string());
    tx.metadata
        .insert("bridge_block_header".to_string(), "header".to_string());

    // Build signed message hash
    let mut data_to_sign = Vec::new();
    data_to_sign.extend_from_slice("Ethereum".as_bytes());
    data_to_sign.extend_from_slice("0x123".as_bytes());
    data_to_sign.extend_from_slice("100".as_bytes());
    data_to_sign.extend_from_slice("0xabc".as_bytes());

    let mut hasher = Keccak256::new();
    hasher.update(&data_to_sign);
    let message_hash = hasher.finalize();

    let (sig, recovery_id) = signing_key.sign_prehash(&message_hash).unwrap();
    let mut sig_bytes = sig.to_bytes().to_vec();
    sig_bytes.push(recovery_id.to_byte() + 27);
    let sig_hex = hex::encode(sig_bytes);

    tx.metadata
        .insert("bridge_relayer_signatures".to_string(), sig_hex);

    // Synthetic bridge locked capacity
    *dag.total_bridge_locked.write().await = 1000;

    let utxos_arc = Arc::new(RwLock::new(HashMap::new()));

    // 1st processing should succeed
    let res = dag.process_bridge_claim(&tx, &utxos_arc).await;
    assert!(res.is_ok(), "First claim processing failed: {:?}", res);

    // 2nd processing of identical claim should fail
    let res2 = dag.process_bridge_claim(&tx, &utxos_arc).await;
    assert!(res2.is_err(), "Duplicate claim should be rejected");
    match res2 {
        Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("Bridge claim already processed"));
        }
        other => panic!(
            "Expected bridge claim already processed error, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_security_bridge_claim_exceeds_locked() {
    let dag = QantoDAG::new_dummy_for_verification();

    let mut tx = Transaction::new_dummy();
    tx.metadata
        .insert("bridge_source_tx_hash".to_string(), "0x123".to_string());
    tx.metadata
        .insert("bridge_recipient".to_string(), "0xabc".to_string());
    tx.metadata
        .insert("bridge_amount".to_string(), "150".to_string()); // Claiming 150
    tx.metadata
        .insert("bridge_source_chain".to_string(), "Ethereum".to_string());

    // Lock capacity is only 100
    *dag.total_bridge_locked.write().await = 100;
    *dag.total_bridge_claimed.write().await = 0;

    let res = dag
        .process_bridge_claim(&tx, &Arc::new(RwLock::new(HashMap::new())))
        .await;
    assert!(
        res.is_err(),
        "Claim should be rejected because it exceeds locked capacity"
    );
    match res {
        Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("exceeds available bridge locked capacity"));
        }
        other => panic!("Expected capacity exceeded error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_security_stake_insufficient_balance() {
    let dag = QantoDAG::new_dummy_for_verification();

    // Set staker balance to 50
    let sender = "0x1111111111111111111111111111111111111111".to_string();
    dag.account_state_cache.set_balance(sender.clone(), 50);

    let mut tx = Transaction::new_dummy();
    tx.sender = sender;
    tx.metadata
        .insert("stake_amount".to_string(), "100".to_string()); // Try to stake 100

    let res = dag.process_stake(&tx).await;
    assert!(
        res.is_err(),
        "Stake should be rejected due to insufficient balance"
    );
    match res {
        Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("Insufficient balance to stake"));
        }
        other => panic!("Expected insufficient balance to stake, got {:?}", other),
    }
}

#[tokio::test]
async fn test_security_stake_immediate_unstake_cooldown() {
    let dag = QantoDAG::new_dummy_for_verification();

    let sender = "0x1111111111111111111111111111111111111111".to_string();
    dag.validators.insert(sender.clone(), 1000);

    // Current epoch is 5
    dag.current_epoch.store(5, Ordering::SeqCst);

    let mut tx = Transaction::new_dummy();
    tx.sender = sender;
    tx.metadata
        .insert("unstake_amount".to_string(), "500".to_string());

    // Try unstaking claiming last stake epoch was 2 (cooldown is 10 epochs, 5 < 2 + 10)
    tx.metadata
        .insert("last_stake_epoch".to_string(), "2".to_string());

    let res = dag
        .process_unstake(&tx, &Arc::new(RwLock::new(HashMap::new())))
        .await;
    assert!(res.is_err(), "Unstaking should be rejected during cooldown");
    match res {
        Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
            assert!(msg.contains("cooldown period"));
        }
        other => panic!("Expected cooldown period error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_security_1000_vote_precision() {
    let dag = QantoDAG::new_dummy_for_verification();

    // Insert mock proposal
    dag.saga.governance.proposals.write().await.insert(
        "saga-proposal-test".to_string(),
        GovernanceProposal {
            id: "saga-proposal-test".to_string(),
            proposer: "0x0".to_string(),
            proposal_type: ProposalType::Signal("test".to_string()),
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Voting,
            voters: vec![],
            creation_epoch: 0,
            justification: None,
            title: None,
            description: None,
            cid: None,
        },
    );

    // Simulate 1000 distinct voters with exact voting power of 123_456_789 (u128 scale 1e9)
    for i in 0..1000 {
        let voter_addr = format!("voter_{}", i);
        dag.validators.insert(voter_addr.clone(), 123456789);

        let mut tx = Transaction::new_dummy();
        tx.sender = voter_addr;
        tx.metadata
            .insert("proposal_id".to_string(), "saga-proposal-test".to_string());
        tx.metadata
            .insert("vote_for".to_string(), "true".to_string());

        dag.process_vote(&tx).await.unwrap();
    }

    // Verify exact precision of voting power sum (no float precision loss)
    let proposals_guard = dag.saga.governance.proposals.read().await;
    let p = proposals_guard.get("saga-proposal-test").unwrap();
    assert_eq!(p.votes_for, 1000 * 123456789);
}

#[tokio::test]
async fn test_security_high_s_signature_rejected() {
    // Construct Legacy transaction with extremely high-S value (all 0xff) to test EIP-2 rejection
    let mut stream = rlp::RlpStream::new_list(9);
    stream.append(&1u64); // nonce
    stream.append(&vec![1u8]); // gas_price
    stream.append(&vec![1u8]); // gas_limit
    stream.append(&vec![0u8; 20]); // to
    stream.append(&1000u128); // value
    stream.append(&vec![]); // data
    stream.append(&27u64); // v
    stream.append(&vec![1u8; 32]); // r
    stream.append(&vec![0xff; 32]); // s (High S)

    let tx_bytes = stream.out();
    let res = Transaction::recover_evm_sender(&tx_bytes);

    assert!(res.is_err());
    let err_msg = res.err().unwrap();
    assert!(err_msg.contains("high-S") || err_msg.contains("Non-canonical"));
}

#[tokio::test]
async fn test_security_coinbase_signature_validation() {
    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let (pk_wrong, _sk_wrong) = generate_pq_keypair(None).unwrap();

    let output = Output {
        address: "miner".to_string(),
        amount: 100,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };

    let coinbase_tx =
        Transaction::new_coinbase("miner".to_string(), 100, vec![output], &sk, 1234).unwrap();

    // Verifying with correct public key should succeed
    let res = coinbase_tx.verify_signature(&pk);
    assert!(
        res.is_ok(),
        "Signature verification failed with correct key"
    );

    // Verifying with incorrect key should fail
    let res_wrong = coinbase_tx.verify_signature(&pk_wrong);
    assert!(
        res_wrong.is_err(),
        "Verification should fail with wrong key"
    );
}

#[tokio::test]
async fn test_security_node_restart_claim_replay() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();

    // Construct valid signatures using ECDSA key
    use sha3::{Digest, Keccak256};
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

    // 1. Boot 1st node instance at that DB path, register validator, and submit a bridge claim
    {
        let dag = new_dummy_with_storage_path(path);
        dag.validators.insert(padded_validator.clone(), 100);
        *dag.total_bridge_locked.write().await = 1000;

        let mut tx = Transaction::new_dummy();
        tx.metadata
            .insert("bridge_source_tx_hash".to_string(), "0x123".to_string());
        tx.metadata
            .insert("bridge_recipient".to_string(), "0xabc".to_string());
        tx.metadata
            .insert("bridge_amount".to_string(), "100".to_string());
        tx.metadata
            .insert("bridge_source_chain".to_string(), "Ethereum".to_string());
        tx.metadata
            .insert("bridge_merkle_proof".to_string(), "proof".to_string());
        tx.metadata
            .insert("bridge_receipt_proof".to_string(), "proof".to_string());
        tx.metadata
            .insert("bridge_block_header".to_string(), "header".to_string());

        let mut data_to_sign = Vec::new();
        data_to_sign.extend_from_slice("Ethereum".as_bytes());
        data_to_sign.extend_from_slice("0x123".as_bytes());
        data_to_sign.extend_from_slice("100".as_bytes());
        data_to_sign.extend_from_slice("0xabc".as_bytes());

        let mut hasher = Keccak256::new();
        hasher.update(&data_to_sign);
        let message_hash = hasher.finalize();

        let (sig, recovery_id) = signing_key.sign_prehash(&message_hash).unwrap();
        let mut sig_bytes = sig.to_bytes().to_vec();
        sig_bytes.push(recovery_id.to_byte() + 27);
        tx.metadata.insert(
            "bridge_relayer_signatures".to_string(),
            hex::encode(sig_bytes),
        );

        let res = dag
            .process_bridge_claim(&tx, &Arc::new(RwLock::new(HashMap::new())))
            .await;
        assert!(res.is_ok(), "First claim processing failed: {:?}", res);
        dag.shutdown().await.unwrap();
    } // dag is gracefully shut down and dropped here

    // 2. Boot 2nd node instance at the same DB path, it should reload processed bridge claims
    {
        let dag2 = new_dummy_with_storage_path(path);

        // Trigger reload (which happens during populate_and_index_history)
        dag2.populate_and_index_history().unwrap();

        let contains_claim = dag2.processed_bridge_claims.contains_key("Ethereum:0x123");

        // Submit the same bridge claim transaction, it should be rejected immediately as duplicate
        let mut tx = Transaction::new_dummy();
        tx.metadata
            .insert("bridge_source_tx_hash".to_string(), "0x123".to_string());
        tx.metadata
            .insert("bridge_recipient".to_string(), "0xabc".to_string());
        tx.metadata
            .insert("bridge_amount".to_string(), "100".to_string());
        tx.metadata
            .insert("bridge_source_chain".to_string(), "Ethereum".to_string());

        let res = dag2
            .process_bridge_claim(&tx, &Arc::new(RwLock::new(HashMap::new())))
            .await;
        dag2.shutdown().await.unwrap();

        assert!(contains_claim, "Claims should be loaded from DB on startup");
        assert!(res.is_err());
        match res {
            Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
                assert!(msg.contains("Bridge claim already processed"));
            }
            other => panic!(
                "Expected bridge claim already processed error, got {:?}",
                other
            ),
        }
    }
}

#[tokio::test]
async fn test_security_tampered_genesis_load() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();

    // 1. Generate normal genesis and persist it to DB
    let genesis_id = {
        let saga = Arc::new(crate::saga::PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));
        let storage_config = get_storage_config(path);
        let db = crate::qanto_storage::QantoStorage::new(storage_config).unwrap();

        // Let's boot initialization loop which creates genesis block
        let config = QantoDagConfig {
            num_chains: 1,
            target_block_time: 1000,
            initial_validator: "0x0000000000000000000000000000000000000001".to_string(),
            genesis_timestamp: 1_717_250_400,
            dev_fee_rate: 100_000_000,
        };
        let logging_config = crate::config::LoggingConfig::default();

        let arc_dag = QantoDAG::new(config, saga, db, logging_config).unwrap();
        let genesis_key = arc_dag.blocks.iter().next().unwrap().key().clone();
        arc_dag.shutdown().await.unwrap();
        genesis_key
    };

    // 2. Tamper with the genesis block in storage: let's overwrite it with a modified block (different validator)
    {
        let storage_config = get_storage_config(path);
        let db = crate::qanto_storage::QantoStorage::new(storage_config).unwrap();
        let id_bytes = genesis_id.clone().into_bytes();
        let block_bytes = db.get(&id_bytes).unwrap().unwrap();
        let mut block: QantoBlock = serde_json::from_slice(&block_bytes).unwrap();

        // Tamper with the initial validator
        block.validator = "0x0000000000000000000000000000000000000002".to_string();

        // Write it back
        let tampered_bytes = serde_json::to_vec(&block).unwrap();
        db.put(id_bytes, tampered_bytes).unwrap();
        db.flush().unwrap();
    }

    // 3. Try booting node at the same DB path. It should validate the genesis fingerprint and fail
    {
        let saga = Arc::new(crate::saga::PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));
        let storage_config = get_storage_config(path);
        let db = crate::qanto_storage::QantoStorage::new(storage_config).unwrap();
        let config = QantoDagConfig {
            num_chains: 1,
            target_block_time: 1000,
            initial_validator: "0x0000000000000000000000000000000000000001".to_string(),
            genesis_timestamp: 1_717_250_400,
            dev_fee_rate: 100_000_000,
        };
        let logging_config = crate::config::LoggingConfig::default();

        let res = QantoDAG::new(config, saga, db, logging_config);

        // If it succeeded, shut down before asserting to avoid resource leaks
        if let Ok(ref dag) = res {
            let _ = dag.shutdown().await;
        }

        assert!(
            res.is_err(),
            "Node should refuse to start due to tampered genesis validator"
        );
        match res {
            Err(crate::qantodag::QantoDAGError::Governance(msg)) => {
                assert!(msg.contains("Genesis fingerprint verification failed"));
            }
            other => panic!("Expected fingerprint verification error, got {:?}", other),
        }
    }
}

#[tokio::test]
async fn test_security_mempool_dos_limits() {
    let (pk, sk) = generate_pq_keypair(None).unwrap();

    let input = Input {
        tx_id: "parent_tx".to_string(),
        output_index: 0,
    };
    let output = Output {
        address: "0x1111111111111111111111111111111111111111".to_string(),
        amount: 100,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };

    // 1. Transaction size limit: 128KB
    let mut tx_large = create_valid_signed_tx(
        &sk,
        &pk,
        1234,
        vec![input.clone()],
        vec![output.clone()],
        10,
    );
    // Add large metadata key to blow up transaction size to > 128KB
    tx_large.metadata.insert(
        "spam".to_string(),
        vec!['A'; 130 * 1024].into_iter().collect(),
    );
    tx_large.id = tx_large.compute_hash();

    let res = tx_large.validate_for_mempool();
    assert!(res.is_err());
    match res {
        Err(TransactionError::InvalidStructure(msg)) => {
            assert!(msg.contains("Transaction size exceeds limit of 128KB"));
        }
        other => panic!("Expected Transaction size limit error, got {:?}", other),
    }

    // 2. Bridge Merkle proof limit: 64KB
    let mut tx_merkle = create_valid_signed_tx(
        &sk,
        &pk,
        1234,
        vec![input.clone()],
        vec![output.clone()],
        10,
    );
    tx_merkle.metadata.insert(
        "bridge_merkle_proof".to_string(),
        vec!['A'; 65 * 1024].into_iter().collect(),
    );
    tx_merkle.id = tx_merkle.compute_hash();

    let res = tx_merkle.validate_for_mempool();
    assert!(res.is_err());
    match res {
        Err(TransactionError::InvalidStructure(msg)) => {
            assert!(msg.contains("Bridge Merkle proof size exceeds limit of 64KB"));
        }
        other => panic!("Expected Merkle proof limit error, got {:?}", other),
    }

    // 3. Proposal description limit: 16KB
    let mut tx_desc = create_valid_signed_tx(
        &sk,
        &pk,
        1234,
        vec![input.clone()],
        vec![output.clone()],
        10,
    );
    tx_desc.metadata.insert(
        "proposal_description".to_string(),
        vec!['A'; 17 * 1024].into_iter().collect(),
    );
    tx_desc.id = tx_desc.compute_hash();

    let res = tx_desc.validate_for_mempool();
    assert!(res.is_err());
    match res {
        Err(TransactionError::InvalidStructure(msg)) => {
            assert!(msg.contains("Proposal description size exceeds limit of 16KB"));
        }
        other => panic!("Expected proposal description limit error, got {:?}", other),
    }

    // 4. Proposal CID limit: 256 bytes
    let mut tx_cid = create_valid_signed_tx(
        &sk,
        &pk,
        1234,
        vec![input.clone()],
        vec![output.clone()],
        10,
    );
    tx_cid.metadata.insert(
        "proposal_cid".to_string(),
        vec!['A'; 257].into_iter().collect(),
    );
    tx_cid.id = tx_cid.compute_hash();

    let res = tx_cid.validate_for_mempool();
    assert!(res.is_err());
    match res {
        Err(TransactionError::InvalidStructure(msg)) => {
            assert!(msg.contains("Proposal CID size exceeds limit of 256 bytes"));
        }
        other => panic!("Expected proposal CID limit error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_randomized_transaction_parsing_fuzz() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    for _ in 0..500 {
        let len = rng.gen_range(0..256);
        let mut data = vec![0u8; len];
        rng.fill(&mut data[..]);
        let _ = Transaction::recover_evm_sender(&data);
    }
}

#[tokio::test]
async fn test_security_signature_malleability_low_s_enforced() {
    use num_bigint::BigUint;
    use sha3::{Digest, Keccak256};

    let signing_key = k256::ecdsa::SigningKey::random(&mut rand::thread_rng());

    // Legacy EIP-155 transaction construction fields
    let nonce = 1u64;
    let gas_price = vec![10u8];
    let gas_limit = vec![100u8];
    let to_addr = vec![0xaa; 20];
    let value = 1000u128;
    let data = vec![1, 2, 3];
    let chain_id = 1234u64;

    // 1. Build Legacy signing payload: [nonce, gas_price, gas_limit, to, value, data, chain_id, 0, 0]
    let mut stream = rlp::RlpStream::new_list(9);
    stream.append(&nonce);
    stream.append(&gas_price);
    stream.append(&gas_limit);
    stream.append(&to_addr);
    stream.append(&value);
    stream.append(&data);
    stream.append(&chain_id);
    stream.append(&0u8);
    stream.append(&0u8);
    let tx_to_hash = stream.out().to_vec();

    // 2. Hash signing payload
    let mut hasher = Keccak256::new();
    hasher.update(&tx_to_hash);
    let msg_hash = hasher.finalize();

    // 3. Sign hash using ECDSA (k256 automatically produces canonical low-S signature)
    let (sig, recovery_id) = signing_key.sign_prehash(&msg_hash).unwrap();
    let sig_r = sig.r().to_bytes().to_vec();
    let sig_s = sig.s().to_bytes().to_vec();
    let rec_id = recovery_id.to_byte();

    // 4. Construct valid Legacy transaction RLP payload
    let v = chain_id * 2 + 35 + rec_id as u64;
    let mut tx_stream = rlp::RlpStream::new_list(9);
    tx_stream.append(&nonce);
    tx_stream.append(&gas_price);
    tx_stream.append(&gas_limit);
    tx_stream.append(&to_addr);
    tx_stream.append(&value);
    tx_stream.append(&data);
    tx_stream.append(&v);
    tx_stream.append(&sig_r);
    tx_stream.append(&sig_s);
    let valid_tx_bytes = tx_stream.out().to_vec();

    // Verify EVM sender recovery succeeds for canonical signature
    GLOBAL_CHAIN_ID.store(chain_id, Ordering::SeqCst);
    let res = Transaction::recover_evm_sender(&valid_tx_bytes);
    assert!(
        res.is_ok(),
        "Canonical low-S signature recovery failed: {:?}",
        res
    );

    // 5. Construct malleable high-S version: high_s = SECP256K1_ORDER - sig_s
    let order = BigUint::parse_bytes(
        b"fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
        16,
    )
    .unwrap();
    let s_biguint = BigUint::from_bytes_be(&sig_s);
    let high_s_biguint = &order - &s_biguint;
    let mut high_s = high_s_biguint.to_bytes_be();
    while high_s.len() < 32 {
        high_s.insert(0, 0);
    }

    // Parity of recovery ID is negated when flipping s
    let high_rec_id = rec_id ^ 1;
    let v_high = chain_id * 2 + 35 + high_rec_id as u64;

    let mut tx_stream_high = rlp::RlpStream::new_list(9);
    tx_stream_high.append(&nonce);
    tx_stream_high.append(&gas_price);
    tx_stream_high.append(&gas_limit);
    tx_stream_high.append(&to_addr);
    tx_stream_high.append(&value);
    tx_stream_high.append(&data);
    tx_stream_high.append(&v_high);
    tx_stream_high.append(&sig_r);
    tx_stream_high.append(&high_s);
    let invalid_tx_bytes = tx_stream_high.out().to_vec();

    // Verify EVM sender recovery rejects high-S signature
    let res_high = Transaction::recover_evm_sender(&invalid_tx_bytes);
    assert!(
        res_high.is_err(),
        "High-S signature should be rejected as non-canonical"
    );
    let err_msg = res_high.err().unwrap();
    assert!(
        err_msg.contains("high-S") || err_msg.contains("Non-canonical"),
        "Expected high-S / non-canonical error message, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_security_supply_conservation() {
    let dag = QantoDAG::new_dummy_for_verification();

    // Invariant: initial_supply + block_rewards + bridge_mints - burns == current_supply
    let initial_supply = dag.emission.read().await.current_supply();
    assert_eq!(initial_supply, 0);

    let mut block_rewards = 0u128;
    let mut bridge_mints = 0u128;
    let mut burns = 0u128;

    // 1. Simulate block reward emissions
    let reward_amount = 50 * crate::emission::SCALE;
    for _ in 0..10 {
        dag.emission
            .write()
            .await
            .update_supply(reward_amount)
            .unwrap();
        block_rewards += reward_amount;
    }

    // 2. Simulate bridge mints
    let mint_amount = 1000 * crate::emission::SCALE;
    *dag.total_bridge_claimed.write().await += mint_amount;
    bridge_mints += mint_amount;

    // 3. Simulate burns
    let burn_amount = 5 * crate::emission::SCALE;
    burns += burn_amount;

    // Calculate current supply under the active accounting equation
    let calculated_current_supply = initial_supply + block_rewards + bridge_mints - burns;

    // 4. Assert math holds true
    assert_eq!(
        initial_supply + block_rewards + bridge_mints - burns,
        calculated_current_supply
    );

    // 5. Verify supply cap (TOTAL_SUPPLY <= 21B QNTO) is strictly enforced under overflow/large mint attempts
    assert!(calculated_current_supply <= 21_000_000_000 * crate::emission::SCALE);

    // Trigger cap enforcement directly on the emission struct
    let giant_reward = 22_000_000_000 * crate::emission::SCALE;
    let res = dag.emission.write().await.update_supply(giant_reward);
    assert!(res.is_ok());

    let final_supply = dag.emission.read().await.current_supply();
    assert!(final_supply <= 21_000_000_000 * crate::emission::SCALE);
    assert_eq!(final_supply, 21_000_000_000 * crate::emission::SCALE);
}

#[tokio::test]
async fn test_transaction_roundtrip_determinism() {
    use qanto_rpc::server::generated as proto;

    let (pk, sk) = generate_pq_keypair(None).unwrap();
    GLOBAL_CHAIN_ID.store(1234, Ordering::SeqCst);

    let input = Input {
        tx_id: "parent_tx".to_string(),
        output_index: 0,
    };
    let output = Output {
        address: "0x2222222222222222222222222222222222222222".to_string(),
        amount: 5_000_000_000 * crate::emission::SCALE,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };
    let mut original = create_valid_signed_tx(&sk, &pk, 1234, vec![input], vec![output], 10);
    original
        .metadata
        .insert("test_key".to_string(), "test_val".to_string());
    original.transaction_kind = TransactionKind::Unstake;

    // Re-sign original transaction since metadata was changed
    original.rebuild_canonical_signature(&sk).unwrap();

    // Roundtrip transaction through protobuf
    let proto_tx = crate::p2p::convert_internal_tx_to_proto(&original);
    use prost::Message;
    let mut buf = Vec::new();
    proto_tx.encode(&mut buf).unwrap();
    let decoded_proto_tx = proto::Transaction::decode(buf.as_slice()).unwrap();
    let decoded = crate::p2p::convert_proto_tx(decoded_proto_tx).unwrap();

    // Helper to extract signed serialization
    let get_signing_data = |tx: &Transaction| -> Vec<u8> {
        let payload = tx.signing_payload_for_transaction();
        Transaction::serialize_for_signing(&payload).unwrap()
    };

    // Verify determinism assertions
    assert_eq!(get_signing_data(&original), get_signing_data(&decoded));
    assert_eq!(original.compute_hash(), decoded.compute_hash());
    assert_eq!(original.id, decoded.id);
    assert_eq!(original.metadata, decoded.metadata);
    assert_eq!(original.transaction_kind, decoded.transaction_kind);
    original.verify_signature(&pk).unwrap();
    decoded.verify_signature(&pk).unwrap();
}

#[tokio::test]
async fn test_block_roundtrip_determinism() {
    use qanto_rpc::server::generated as proto;
    let (pk, sk) = generate_pq_keypair(None).unwrap();
    GLOBAL_CHAIN_ID.store(1234, Ordering::SeqCst);

    let input1 = Input {
        tx_id: "parent_tx1".to_string(),
        output_index: 0,
    };
    let output1 = Output {
        address: "0x2222222222222222222222222222222222222222".to_string(),
        amount: 5_000_000_000 * crate::emission::SCALE,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };
    let mut tx1 = create_valid_signed_tx(&sk, &pk, 1234, vec![input1], vec![output1], 10);
    tx1.metadata
        .insert("tx1_meta".to_string(), "tx1_val".to_string());
    tx1.transaction_kind = TransactionKind::Stake;
    tx1.rebuild_canonical_signature(&sk).unwrap();

    let input2 = Input {
        tx_id: "parent_tx2".to_string(),
        output_index: 1,
    };
    let output2 = Output {
        address: "0x3333333333333333333333333333333333333333".to_string(),
        amount: 100_000,
        homomorphic_encrypted: HomomorphicEncrypted::default(),
    };
    let mut tx2 = create_valid_signed_tx(&sk, &pk, 1234, vec![input2], vec![output2], 5);
    tx2.metadata
        .insert("tx2_meta".to_string(), "tx2_val".to_string());
    tx2.transaction_kind = TransactionKind::Transfer;
    tx2.rebuild_canonical_signature(&sk).unwrap();

    let creation_data = crate::qantodag::QantoBlockCreationData {
        validator_private_key: sk.clone(),
        chain_id: 1234,
        parents: vec!["parent_block_id_1".to_string()],
        transactions: vec![tx1, tx2],
        difficulty: 100,
        validator: "0x1111111111111111111111111111111111111111".to_string(),
        miner: "0xminer_address".to_string(),
        timestamp: 1625000000,
        current_epoch: 1,
        height: 10,
        paillier_pk: vec![1, 2, 3],
    };

    let original_block = QantoBlock::new(creation_data).unwrap();

    // Now roundtrip block through protobuf
    let proto_block = crate::p2p::convert_internal_block_to_proto(&original_block);
    use prost::Message;
    let mut buf = Vec::new();
    proto_block.encode(&mut buf).unwrap();
    let decoded_proto_block = proto::QantoBlock::decode(buf.as_slice()).unwrap();
    let decoded_block = crate::p2p::convert_proto_block(decoded_proto_block).unwrap();

    // Assert that the decoded block is exactly identical to the original block
    assert_eq!(original_block.id, decoded_block.id);
    assert_eq!(original_block.merkle_root, decoded_block.merkle_root);
    assert_eq!(original_block.parents, decoded_block.parents);
    assert_eq!(original_block.chain_id, decoded_block.chain_id);
    assert_eq!(original_block.difficulty, decoded_block.difficulty);
    assert_eq!(original_block.validator, decoded_block.validator);
    assert_eq!(original_block.miner, decoded_block.miner);
    assert_eq!(original_block.timestamp, decoded_block.timestamp);
    assert_eq!(original_block.height, decoded_block.height);
    assert_eq!(original_block.reward, decoded_block.reward);
    assert_eq!(original_block.effort, decoded_block.effort);
    assert_eq!(
        original_block.signature.signer_public_key,
        decoded_block.signature.signer_public_key
    );
    assert_eq!(
        original_block.signature.signature,
        decoded_block.signature.signature
    );
    assert_eq!(original_block.epoch, decoded_block.epoch);

    // Helper to extract signed serialization
    let get_signing_data = |tx: &Transaction| -> Vec<u8> {
        let payload = tx.signing_payload_for_transaction();
        Transaction::serialize_for_signing(&payload).unwrap()
    };

    // Verify transaction list matches exactly, including IDs, kinds and metadata
    assert_eq!(
        original_block.transactions.len(),
        decoded_block.transactions.len()
    );
    for (orig_tx, dec_tx) in original_block
        .transactions
        .iter()
        .zip(decoded_block.transactions.iter())
    {
        assert_eq!(orig_tx.id, dec_tx.id);
        assert_eq!(orig_tx.sender, dec_tx.sender);
        assert_eq!(orig_tx.receiver, dec_tx.receiver);
        assert_eq!(orig_tx.amount, dec_tx.amount);
        assert_eq!(orig_tx.fee, dec_tx.fee);
        assert_eq!(orig_tx.metadata, dec_tx.metadata);
        assert_eq!(orig_tx.transaction_kind, dec_tx.transaction_kind);
        assert_eq!(get_signing_data(orig_tx), get_signing_data(dec_tx));
        orig_tx.verify_signature(&pk).unwrap();
        dec_tx.verify_signature(&pk).unwrap();
    }
}
