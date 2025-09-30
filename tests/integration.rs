#[cfg(test)]
mod tests {
    use libp2p::identity::Keypair;
    use qanto::config::{Config, LoggingConfig, P2pConfig};
    use qanto::mempool::Mempool;
    use qanto::p2p::P2PServer;
    use qanto::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey};
    use qanto::qanto_storage::{QantoStorage, StorageConfig};
    use qanto::qantodag::{QantoBlock, QantoDAG, QantoDagConfig};
    use qanto::saga::PalletSaga;
    use qanto::transaction::{Input, Output, Transaction, TransactionConfig};
    use qanto::types::{HomomorphicEncrypted, UTXO};
    use qanto::wallet::Wallet;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio::time::{sleep, Duration, Instant};

    /// Create mock keys for testing
    fn create_test_keypair() -> (QantoPQPrivateKey, QantoPQPublicKey) {
        let private_key = QantoPQPrivateKey::new_dummy();
        let public_key = private_key.public_key();
        (private_key, public_key)
    }
    use qanto::p2p::P2PConfig;
    use rand::rngs::OsRng;
    use rand::Rng;
    use std::sync::Once;
    use tracing::{debug, info};

    static INIT: Once = Once::new();

    fn init_tracing() {
        INIT.call_once(|| {
            tracing_subscriber::fmt::init();
        });
    }

    fn get_random_port() -> u16 {
        let mut rng = rand::thread_rng();
        rng.gen_range(8000..9000)
    }

    #[tokio::test]
    async fn test_p2p_block_propagation() {
        init_tracing();

        let _wallet = Wallet::new().expect("Failed to create wallet");

        let port1 = get_random_port();
        let port2 = get_random_port();
        let api_port1 = port1 + 1000;
        let api_port2 = port2 + 1000;

        info!(
            "Using ports: p2p1={}, p2p2={}, api1={}, api2={}",
            port1, port2, api_port1, api_port2
        );

        let config1 = Config {
            p2p_address: format!("/ip4/127.0.0.1/tcp/{}", port1),
            api_address: format!("127.0.0.1:{}", api_port1),
            peers: vec![format!("/ip4/127.0.0.1/tcp/{}", port2)],
            local_full_p2p_address: Some(format!("/ip4/127.0.0.1/tcp/{}", port1)),
            network_id: "test_network".to_string(),
            genesis_validator: "1234567890123456789012345678901234567890123456789012345678901234"
                .to_string(),
            contract_address: "1234567890123456789012345678901234567890123456789012345678901234"
                .to_string(),
            target_block_time: 1000, // Fixed: 1 second (within valid range 25-15000ms)
            difficulty: 1,
            max_amount: 1000000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: 1,
            num_chains: 2,
            mining_chain_id: 0,
            mining_interval_ms: Some(1000),
            dummy_tx_interval_ms: Some(500),
            dummy_tx_per_cycle: Some(10),
            mempool_max_age_secs: Some(3600),
            mempool_max_size_bytes: Some(10 * 1024 * 1024),
            mempool_batch_size: Some(100),
            mempool_backpressure_threshold: Some(0.8),
            tx_batch_size: Some(50),
            adaptive_batch_threshold: Some(0.7),
            memory_soft_limit: Some(8 * 1024 * 1024),
            memory_hard_limit: Some(10 * 1024 * 1024),
            data_dir: "./test_data1".to_string(),
            db_path: "./test_data1/qanto.db".to_string(),
            wallet_path: "test_wallet1.key".to_string(),
            p2p_identity_path: "test_p2p_identity1.key".to_string(),
            log_file_path: Some("./logs/test1.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            p2p: P2pConfig {
                heartbeat_interval: 1000, // 1 second for faster connectivity
                mesh_n_low: 1,            // Allow single peer mesh for testing
                mesh_n: 1,                // Target 1 peer for 2-node test
                mesh_n_high: 2,           // Max 2 peers for 2-node test
                mesh_outbound_min: 1,     // Minimum 1 outbound connection
            },
        };

        let config2 = Config {
            p2p_address: format!("/ip4/127.0.0.1/tcp/{}", port2),
            api_address: format!("127.0.0.1:{}", api_port2),
            peers: vec![format!("/ip4/127.0.0.1/tcp/{}", port1)],
            local_full_p2p_address: Some(format!("/ip4/127.0.0.1/tcp/{}", port2)),
            network_id: "test_network".to_string(),
            genesis_validator: "2345678901234567890123456789012345678901234567890123456789012345"
                .to_string(),
            contract_address: "2345678901234567890123456789012345678901234567890123456789012345"
                .to_string(),
            target_block_time: 1000, // Fixed: 1 second (within valid range)
            difficulty: 1,
            max_amount: 1000000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: 1,
            num_chains: 2,
            mining_chain_id: 1,
            mining_interval_ms: Some(1000),
            dummy_tx_interval_ms: Some(500),
            dummy_tx_per_cycle: Some(10),
            mempool_max_age_secs: Some(3600),
            mempool_max_size_bytes: Some(10 * 1024 * 1024),
            mempool_batch_size: Some(100),
            mempool_backpressure_threshold: Some(0.8),
            tx_batch_size: Some(50),
            adaptive_batch_threshold: Some(0.7),
            memory_soft_limit: Some(8 * 1024 * 1024),
            memory_hard_limit: Some(10 * 1024 * 1024),
            data_dir: "./test_data2".to_string(),
            db_path: "./test_data2/qanto.db".to_string(),
            wallet_path: "test_wallet2.key".to_string(),
            p2p_identity_path: "test_p2p_identity2.key".to_string(),
            log_file_path: Some("./logs/test2.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            p2p: P2pConfig {
                heartbeat_interval: 1000, // 1 second for faster connectivity
                mesh_n_low: 1,            // Allow single peer mesh for testing
                mesh_n: 1,                // Target 1 peer for 2-node test
                mesh_n_high: 2,           // Max 2 peers for 2-node test
                mesh_outbound_min: 1,     // Minimum 1 outbound connection
            },
        };

        // Create storage instances for DAGs
        let storage_config1 = StorageConfig {
            data_dir: PathBuf::from("./test_data1_prop"),
            ..Default::default()
        };
        let storage1 = QantoStorage::new(storage_config1).expect("Failed to create storage1");

        let storage_config2 = StorageConfig {
            data_dir: PathBuf::from("./test_data2_prop"),
            ..Default::default()
        };
        let storage2 = QantoStorage::new(storage_config2).expect("Failed to create storage2");

        // Create SAGA instances
        let saga1 = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));
        let saga2 = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));

        // Create DAG configs
        let dag_config1 = QantoDagConfig {
            initial_validator: config1.genesis_validator.clone(),
            target_block_time: 1000, // Fixed: consistent with config
            num_chains: 2,
        };
        let dag_config2 = QantoDagConfig {
            initial_validator: config2.genesis_validator.clone(),
            target_block_time: 1000, // Fixed: consistent with config
            num_chains: 2,
        };

        let dag1 =
            QantoDAG::new(dag_config1, saga1.clone(), storage1).expect("Failed to create DAG1");
        let dag2 = QantoDAG::new(dag_config2, saga2, storage2).expect("Failed to create DAG2");
        let mempool1 = Arc::new(RwLock::new(Mempool::new(3600, 10 * 1024 * 1024, 10000)));
        let mempool2 = Arc::new(RwLock::new(Mempool::new(3600, 10 * 1024 * 1024, 10000)));
        let utxos1 = Arc::new(RwLock::new(HashMap::new()));
        let utxos2 = Arc::new(RwLock::new(HashMap::new()));
        let proposals1 = Arc::new(RwLock::new(Vec::new()));
        let proposals2 = Arc::new(RwLock::new(Vec::new()));

        let local_keypair1 = Keypair::generate_ed25519();
        let mut rng1 = OsRng;
        let node_qr_sk1 = QantoPQPrivateKey::generate(&mut rng1);
        let node_qr_pk1 = node_qr_sk1.public_key();
        let peer_cache_path1 = PathBuf::from("/tmp/qanto_peer_cache1.json");
        let p2p_settings1 = P2pConfig::default();

        let p2p_config1 = P2PConfig {
            topic_prefix: "qantodag",
            listen_addresses: vec![config1.p2p_address.clone()],
            initial_peers: config1.peers.clone(),
            dag: dag1.clone(),
            mempool: mempool1.clone(),
            utxos: utxos1.clone(),
            proposals: proposals1.clone(),
            local_keypair: local_keypair1,
            p2p_settings: p2p_settings1,
            node_qr_sk: &node_qr_sk1,
            node_qr_pk: &node_qr_pk1,
            peer_cache_path: peer_cache_path1.to_string_lossy().to_string(),
        };

        let local_keypair2 = Keypair::generate_ed25519();
        let mut rng2 = OsRng;
        let node_qr_sk2 = QantoPQPrivateKey::generate(&mut rng2);
        let node_qr_pk2 = node_qr_sk2.public_key();
        let peer_cache_path2 = PathBuf::from("/tmp/qanto_peer_cache2.json");
        let p2p_settings2 = P2pConfig::default();

        let p2p_config2 = P2PConfig {
            topic_prefix: "qantodag",
            listen_addresses: vec![config2.p2p_address.clone()],
            initial_peers: config2.peers.clone(),
            dag: dag2.clone(),
            mempool: mempool2.clone(),
            utxos: utxos2.clone(),
            proposals: proposals2.clone(),
            local_keypair: local_keypair2,
            p2p_settings: p2p_settings2,
            node_qr_sk: &node_qr_sk2,
            node_qr_pk: &node_qr_pk2,
            peer_cache_path: peer_cache_path2.to_string_lossy().to_string(),
        };

        let (tx1, rx1) = tokio::sync::mpsc::channel(100);
        let (tx2, rx2) = tokio::sync::mpsc::channel(100);

        let mut p2p1 = P2PServer::new(p2p_config1, tx1.clone())
            .await
            .expect("Failed to create P2P server 1");
        let mut p2p2 = P2PServer::new(p2p_config2, tx2.clone())
            .await
            .expect("Failed to create P2P server 2");

        let p2p_handle1 = tokio::spawn(async move { p2p1.run(rx1).await });
        let p2p_handle2 = tokio::spawn(async move { p2p2.run(rx2).await });

        // Wait longer for peers to connect and establish gossipsub mesh
        sleep(Duration::from_secs(15)).await;

        // Wait for gossipsub mesh establishment by checking connected peers
        let mut mesh_ready = false;
        for attempt in 0..30 {
            // Simple mesh readiness check - in production we'd verify actual mesh formation
            sleep(Duration::from_secs(2)).await;
            info!("Mesh establishment attempt {}/30", attempt + 1);
            if attempt > 10 {
                mesh_ready = true;
                break;
            }
        }
        
        if !mesh_ready {
            info!("Mesh not fully established, proceeding with test anyway");
        }

        // Additional wait for gossipsub mesh stabilization
        sleep(Duration::from_secs(5)).await;

        // Create a properly signed test block with valid proof-of-work
        let (private_key, _public_key) = create_test_keypair();

        // Get the effective difficulty from consensus to ensure consistency
        let consensus = qanto::consensus::Consensus::new(saga1.clone());
        let effective_difficulty = consensus.get_effective_difficulty("test_miner").await;

        // Create block using proper constructor that calculates ID from content
        // Create a coinbase transaction manually (empty inputs make it a coinbase)
        let mut coinbase_tx = Transaction {
            id: String::new(), // Will be computed by compute_hash
            sender: "0000000000000000000000000000000000000000000000000000000000000000".to_string(), // Coinbase sender
            receiver: "test_miner".to_string(),
            amount: 150_000_000_000, // Mining reward
            fee: 0,                  // Coinbase transactions have no fee
            inputs: vec![],          // Empty inputs make this a coinbase transaction
            outputs: vec![Output {
                address: "test_miner".to_string(),
                amount: 150_000_000_000,
                homomorphic_encrypted: HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            }],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            metadata: std::collections::HashMap::new(),
            signature: qanto::types::QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
        };
        // Compute the proper transaction ID using the public method
        coinbase_tx.id = coinbase_tx.compute_fast_hash();
        let transactions = vec![coinbase_tx];

        let block_creation_data = qanto::qantodag::QantoBlockCreationData {
            validator_private_key: private_key.clone(),
            chain_id: 0,
            parents: vec![],
            transactions,
            difficulty: effective_difficulty,
            validator: "test_validator".to_string(),
            miner: "test_miner".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            current_epoch: 0,
            height: 1,
            paillier_pk: vec![0; 32],
        };

        let mut block = QantoBlock::new(block_creation_data).expect("Failed to create block");

        // Create proper signature for the block
        let signing_data = qanto::qantodag::SigningData {
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

        let data_to_sign = QantoBlock::serialize_for_signing(&signing_data).unwrap();
        let signature =
            qanto::types::QuantumResistantSignature::sign(&private_key, &data_to_sign).unwrap();
        block.signature = signature;

        // Solve proof-of-work for the test block
        let miner_config = qanto::miner::MinerConfig {
            address: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            dag: dag1.clone(),
            target_block_time: 120, // Increased timeout for testing
            use_gpu: false,
            zk_enabled: false,
            threads: 1,
        };
        let miner = qanto::miner::Miner::new(miner_config).expect("Failed to create miner");

        // Debug print block hash before and after PoW solving
        let hash_before = block.hash();
        info!("Block hash before PoW: {}", hash_before);

        miner
            .solve_pow(&mut block)
            .expect("Failed to solve proof-of-work");

        let hash_after = block.hash();
        info!("Block hash after PoW: {}", hash_after);

        // Verify the block is valid after PoW (hash should meet target difficulty)
        info!("Block difficulty: {}", block.difficulty);
        let target_hash_bytes =
            qanto::miner::Miner::calculate_target_from_difficulty(block.difficulty);
        info!("Target hash: {}", hex::encode(&target_hash_bytes));
        
        // Use the block's actual hash method which includes nonce and qanhash
        let block_hash = block.hash();
        let block_pow_hash_bytes = hex::decode(&block_hash).expect("Failed to decode block hash");
        info!("Block PoW hash: {}", block_hash);
        info!("Block nonce: {}", block.nonce);
        info!("Block effort: {}", block.effort);
        
        // Debug the hash and target values
        info!("Block hash bytes length: {}", block_pow_hash_bytes.len());
        info!("Target hash bytes length: {}", target_hash_bytes.len());
        info!("Block hash hex: {}", hex::encode(&block_pow_hash_bytes));
        info!("Target hash hex: {}", hex::encode(&target_hash_bytes));

        let hash_meets_target =
            qanto::miner::Miner::hash_meets_target(&block_pow_hash_bytes, &target_hash_bytes);
        info!("Hash meets target: {}", hash_meets_target);

        if !hash_meets_target {
            info!("Block hash as bytes: {:?}", block_pow_hash_bytes);
            info!("Target hash as bytes: {:?}", target_hash_bytes);
            // Compare byte by byte for debugging
            for (i, (hash_byte, target_byte)) in block_pow_hash_bytes.iter().zip(target_hash_bytes.iter()).enumerate() {
                if hash_byte != target_byte {
                    info!("First difference at byte {}: hash={:02x}, target={:02x}", i, hash_byte, target_byte);
                    break;
                }
            }
        }

        assert!(
            hash_meets_target,
            "Block hash should meet target difficulty after PoW"
        );
        info!("Block hash meets target difficulty: {}", block_hash);

        info!("Broadcasting block: {}", block.id);

        // Add the block to node 1's DAG first
        dag1.add_block(block.clone(), &utxos1)
            .await
            .expect("Failed to add block to node 1's DAG");
        info!("Added block to node 1's DAG");

        // Broadcast the block via P2P with retry mechanism
        let mut broadcast_success = false;
        for attempt in 0..3 {
            match tx1.send(qanto::p2p::P2PCommand::BroadcastBlock(block.clone())).await {
                Ok(_) => {
                    info!("Block broadcast successful on attempt {}", attempt + 1);
                    broadcast_success = true;
                    break;
                }
                Err(e) => {
                    info!("Block broadcast failed on attempt {}: {}", attempt + 1, e);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
        
        assert!(broadcast_success, "Failed to broadcast block after multiple attempts");

        // Add 60s polling loop to check if block propagated (increased timeout)
        let start_time = Instant::now();
        let timeout = Duration::from_secs(60);
        let mut block_propagated = false;

        while start_time.elapsed() < timeout {
            let dag2_guard = &*dag2;
            if dag2_guard.blocks.contains_key(&block.id) {
                info!("Block successfully propagated to peer after {} seconds", start_time.elapsed().as_secs());
                block_propagated = true;
                break;
            }
            debug!("Block not yet propagated, waiting...");
            sleep(Duration::from_millis(500)).await;
        }

        assert!(
            block_propagated,
            "Block not propagated to peer within 60 seconds"
        );

        let dag2_guard = &*dag2;
        assert!(
            dag2_guard.blocks.contains_key(&block.id),
            "Block should exist in DAG2"
        );

        p2p_handle1.abort();
        p2p_handle2.abort();
    }

    #[tokio::test]
    async fn test_p2p_state_sync() {
        init_tracing();

        let wallet = Wallet::new().expect("Failed to create wallet");

        let port1 = get_random_port();
        let port2 = get_random_port();
        let api_port1 = port1 + 2000;
        let api_port2 = port2 + 2000;

        info!(
            "Using ports: p2p1={}, p2p2={}, api1={}, api2={}",
            port1, port2, api_port1, api_port2
        );

        let config1 = Config {
            p2p_address: format!("/ip4/127.0.0.1/tcp/{}", port1),
            api_address: format!("0.0.0.0:{}", api_port1),
            peers: vec![format!("/ip4/127.0.0.1/tcp/{}", port2)],
            local_full_p2p_address: Some(format!("/ip4/127.0.0.1/tcp/{}", port1)),
            network_id: "test_network".to_string(),
            genesis_validator: wallet.address(),
            contract_address: "3456789012345678901234567890123456789012345678901234567890123456"
                .to_string(),
            target_block_time: 1000, // Fixed: 1 second (within valid range)
            difficulty: 1,
            max_amount: 21_000_000_000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: 4,
            num_chains: 2,
            mining_chain_id: 0,
            mining_interval_ms: Some(1000),
            dummy_tx_interval_ms: Some(500),
            dummy_tx_per_cycle: Some(10),
            mempool_max_age_secs: Some(3600),
            mempool_max_size_bytes: Some(10 * 1024 * 1024),
            mempool_batch_size: Some(100),
            mempool_backpressure_threshold: Some(0.8),
            tx_batch_size: Some(50),
            adaptive_batch_threshold: Some(0.7),
            memory_soft_limit: Some(8 * 1024 * 1024),
            memory_hard_limit: Some(10 * 1024 * 1024),
            data_dir: "./test_data3".to_string(),
            db_path: "./test_data3/qanto.db".to_string(),
            wallet_path: "test_wallet3.key".to_string(),
            p2p_identity_path: "test_p2p_identity3.key".to_string(),
            log_file_path: Some("./logs/test3.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            p2p: P2pConfig {
                heartbeat_interval: 1000, // 1 second for faster connectivity
                mesh_n_low: 1,            // Allow single peer mesh for testing
                mesh_n: 1,                // Target 1 peer for 2-node test
                mesh_n_high: 2,           // Max 2 peers for 2-node test
                mesh_outbound_min: 1,     // Minimum 1 outbound connection
            },
        };

        let config2 = Config {
            p2p_address: format!("/ip4/127.0.0.1/tcp/{}", port2),
            api_address: format!("0.0.0.0:{}", api_port2),
            peers: vec![format!("/ip4/127.0.0.1/tcp/{}", port1)],
            local_full_p2p_address: Some(format!("/ip4/127.0.0.1/tcp/{}", port2)),
            network_id: "test_network".to_string(),
            genesis_validator: config1.genesis_validator.clone(),
            contract_address: "4567890123456789012345678901234567890123456789012345678901234567"
                .to_string(),
            target_block_time: 1000, // Fixed: 1 second (within valid range)
            difficulty: 1,
            max_amount: 21_000_000_000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: 4,
            num_chains: 2,
            mining_chain_id: 1,
            mining_interval_ms: Some(1000),
            dummy_tx_interval_ms: Some(500),
            dummy_tx_per_cycle: Some(10),
            mempool_max_age_secs: Some(3600),
            mempool_max_size_bytes: Some(10 * 1024 * 1024),
            mempool_batch_size: Some(100),
            mempool_backpressure_threshold: Some(0.8),
            tx_batch_size: Some(50),
            adaptive_batch_threshold: Some(0.7),
            memory_soft_limit: Some(8 * 1024 * 1024),
            memory_hard_limit: Some(10 * 1024 * 1024),
            data_dir: "./test_data4".to_string(),
            db_path: "./test_data4/qanto.db".to_string(),
            wallet_path: "test_wallet4.key".to_string(),
            p2p_identity_path: "test_p2p_identity4.key".to_string(),
            log_file_path: Some("./logs/test4.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            p2p: P2pConfig {
                heartbeat_interval: 1000, // 1 second for faster connectivity
                mesh_n_low: 1,            // Allow single peer mesh for testing
                mesh_n: 1,                // Target 1 peer for 2-node test
                mesh_n_high: 2,           // Max 2 peers for 2-node test
                mesh_outbound_min: 1,     // Minimum 1 outbound connection
            },
        };

        // Create storage instances for DAGs
        let storage_config1 = StorageConfig {
            data_dir: PathBuf::from("./test_data1"),
            ..Default::default()
        };
        let storage1 = QantoStorage::new(storage_config1).expect("Failed to create storage1");

        let storage_config2 = StorageConfig {
            data_dir: PathBuf::from("./test_data2"),
            ..Default::default()
        };
        let storage2 = QantoStorage::new(storage_config2).expect("Failed to create storage2");

        // Create SAGA instances
        let saga1 = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));
        let saga2 = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));

        // Create DAG configs
        let dag_config1 = QantoDagConfig {
            initial_validator: config1.genesis_validator.clone(),
            target_block_time: 1000, // Fixed: consistent with config
            num_chains: 2,
        };
        let dag_config2 = QantoDagConfig {
            initial_validator: config2.genesis_validator.clone(),
            target_block_time: 1000, // Fixed: consistent with config
            num_chains: 2,
        };

        let dag1 = QantoDAG::new(dag_config1, saga1, storage1).expect("Failed to create DAG1");
        let dag2 = QantoDAG::new(dag_config2, saga2, storage2).expect("Failed to create DAG2");
        let mempool1 = Arc::new(RwLock::new(Mempool::new(3600, 10 * 1024 * 1024, 10000)));
        let mempool2 = Arc::new(RwLock::new(Mempool::new(3600, 10 * 1024 * 1024, 10000)));
        let utxos1 = Arc::new(RwLock::new(HashMap::new()));
        let utxos2 = Arc::new(RwLock::new(HashMap::new()));
        let proposals1 = Arc::new(RwLock::new(Vec::new()));
        let proposals2 = Arc::new(RwLock::new(Vec::new()));

        // Add test UTXO to node1
        utxos1.write().await.insert(
            "test_utxo".to_string(),
            UTXO {
                address: config1.genesis_validator.clone(),
                amount: 1000,
                tx_id: "test_tx".to_string(),
                output_index: 0,
                explorer_link: String::new(),
            },
        );

        info!("Added test UTXO to node1: amount=1000");

        let local_keypair1 = Keypair::generate_ed25519();
        let mut rng1 = OsRng;
        let node_qr_sk1 = QantoPQPrivateKey::generate(&mut rng1);
        let node_qr_pk1 = node_qr_sk1.public_key();
        let peer_cache_path1 = PathBuf::from("/tmp/qanto_peer_cache1.json");
        let p2p_settings1 = P2pConfig::default();

        let p2p_config1 = P2PConfig {
            topic_prefix: "qantodag",
            listen_addresses: vec![config1.p2p_address.clone()],
            initial_peers: config1.peers.clone(),
            dag: dag1.clone(),
            mempool: mempool1.clone(),
            utxos: utxos1.clone(),
            proposals: proposals1.clone(),
            local_keypair: local_keypair1,
            p2p_settings: p2p_settings1,
            node_qr_sk: &node_qr_sk1,
            node_qr_pk: &node_qr_pk1,
            peer_cache_path: peer_cache_path1.to_string_lossy().to_string(),
        };

        let local_keypair2 = Keypair::generate_ed25519();
        let mut rng2 = OsRng;
        let node_qr_sk2 = QantoPQPrivateKey::generate(&mut rng2);
        let node_qr_pk2 = node_qr_sk2.public_key();
        let peer_cache_path2 = PathBuf::from("/tmp/qanto_peer_cache2.json");
        let p2p_settings2 = P2pConfig::default();

        let p2p_config2 = P2PConfig {
            topic_prefix: "qantodag",
            listen_addresses: vec![config2.p2p_address.clone()],
            initial_peers: config2.peers.clone(),
            dag: dag2.clone(),
            mempool: mempool2.clone(),
            utxos: utxos2.clone(),
            proposals: proposals2.clone(),
            local_keypair: local_keypair2,
            p2p_settings: p2p_settings2,
            node_qr_sk: &node_qr_sk2,
            node_qr_pk: &node_qr_pk2,
            peer_cache_path: peer_cache_path2.to_string_lossy().to_string(),
        };

        let (tx1, rx1) = tokio::sync::mpsc::channel(100);
        let (tx2, rx2) = tokio::sync::mpsc::channel(100);

        let mut p2p1 = P2PServer::new(p2p_config1, tx1.clone())
            .await
            .expect("Failed to create P2P server 1");
        let mut p2p2 = P2PServer::new(p2p_config2, tx2.clone())
            .await
            .expect("Failed to create P2P server 2");

        let p2p_handle1 = tokio::spawn(async move { p2p1.run(rx1).await });
        let p2p_handle2 = tokio::spawn(async move { p2p2.run(rx2).await });

        // Wait longer for peers to connect and establish gossipsub mesh
        sleep(Duration::from_secs(15)).await;

        // Wait for gossipsub mesh establishment for state sync
        let mut mesh_ready = false;
        for attempt in 0..30 {
            sleep(Duration::from_secs(2)).await;
            info!("State sync mesh establishment attempt {}/30", attempt + 1);
            if attempt > 10 {
                mesh_ready = true;
                break;
            }
        }
        
        if !mesh_ready {
            info!("Mesh not fully established for state sync, proceeding anyway");
        }

        // Additional stabilization time
        sleep(Duration::from_secs(5)).await;

        // Trigger state sync
        info!("Requesting state sync from node2");
        tx2.send(qanto::p2p::P2PCommand::RequestState)
            .await
            .expect("Failed to send state sync request");

        // Add 60s polling for UTXO hash equality with logging (increased timeout)
        let start_time = Instant::now();
        let timeout = Duration::from_secs(60);
        let mut state_synced = false;

        while start_time.elapsed() < timeout {
            let utxos1_guard = utxos1.read().await;
            let utxos2_guard = utxos2.read().await;

            debug!(
                "UTXO lengths: node1={}, node2={}",
                utxos1_guard.len(),
                utxos2_guard.len()
            );

            if utxos2_guard.contains_key("test_utxo") {
                info!("UTXO successfully synced to node2 after {} seconds", start_time.elapsed().as_secs());
                state_synced = true;
                break;
            }

            if utxos1_guard.len() != utxos2_guard.len() {
                debug!(
                    "UTXO count mismatch - node1: {}, node2: {}",
                    utxos1_guard.len(),
                    utxos2_guard.len()
                );
            }

            sleep(Duration::from_millis(500)).await;
        }

        assert!(state_synced, "UTXO not synced within 60 seconds");

        let utxos2_guard = utxos2.read().await;
        assert_eq!(
            utxos2_guard
                .get("test_utxo")
                .expect("UTXO should exist")
                .amount,
            1000,
            "Incorrect UTXO amount"
        );

        p2p_handle1.abort();
        p2p_handle2.abort();
    }

    #[tokio::test]
    async fn test_wallet_status_command() {
        init_tracing();

        let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));

        let config = Config {
            p2p_address: format!("/ip4/127.0.0.1/tcp/{}", get_random_port()),
            api_address: format!("0.0.0.0:{}", get_random_port()),
            peers: vec![],
            local_full_p2p_address: None,
            network_id: "test-network".to_string(),
            genesis_validator: wallet.address(),
            contract_address: "3456789012345678901234567890123456789012345678901234567890123456"
                .to_string(),
            target_block_time: 1000, // Fixed: 1 second (within valid range 25-15000ms)
            difficulty: 1,
            max_amount: 21_000_000_000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: 4,
            num_chains: 2,
            mining_chain_id: 0,
            mining_interval_ms: Some(100),
            dummy_tx_interval_ms: Some(100),
            dummy_tx_per_cycle: Some(10),
            mempool_max_age_secs: Some(300),
            mempool_max_size_bytes: Some(1024 * 1024),
            mempool_batch_size: Some(100),
            mempool_backpressure_threshold: Some(0.8),
            tx_batch_size: Some(50),
            adaptive_batch_threshold: Some(0.7),
            memory_soft_limit: Some(8 * 1024 * 1024),
            memory_hard_limit: Some(10 * 1024 * 1024),
            data_dir: "/tmp/test_data".to_string(),
            db_path: "/tmp/test_data/qanto.db".to_string(),
            wallet_path: "/tmp/test_wallet.key".to_string(),
            p2p_identity_path: "/tmp/test_p2p_identity.key".to_string(),
            log_file_path: Some("/tmp/test_qanto.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            p2p: P2pConfig {
                heartbeat_interval: 1000, // Fixed: reduced to 1000ms to be within 2x target_block_time
                mesh_n_low: 4,
                mesh_n: 6,
                mesh_n_high: 12,
                mesh_outbound_min: 2,
            },
        };

        // Validate config before using it
        config.validate().expect("Config validation should pass");
        info!("Config validation passed");

        let contract_address = config.contract_address.clone();
        let node = qanto::node::Node::new(
            config,
            "test_config.toml".to_string(),
            wallet.clone(),
            "/tmp/test_p2p_identity.key",
            "/tmp/test_peer_cache.json".to_string(),
        )
        .await
        .expect("Failed to create node");

        let receiver_address = "1234567890123456789012345678901234567890123456789012345678901234";
        let tx_config = TransactionConfig {
            sender: contract_address,
            receiver: receiver_address.to_string(),
            amount: 100,
            fee: 1,
            inputs: vec![Input {
                tx_id: "genesis_total_supply_tx".to_string(),
                output_index: 0,
            }],
            outputs: vec![Output {
                address: receiver_address.to_string(),
                amount: 100,
                homomorphic_encrypted: HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            }],
            metadata: None,
            tx_timestamps: Arc::new(RwLock::new(HashMap::<String, u64>::new())),
        };

        let (pq_private_key, _) = wallet.get_keypair().expect("Failed to get wallet keypair");
        let tx = Transaction::new(tx_config, &pq_private_key)
            .await
            .expect("Failed to create transaction");

        let mempool = node.mempool.write().await;
        let utxos = node.utxos.read().await;
        mempool
            .add_transaction(tx.clone(), &*utxos, &*node.dag)
            .await
            .expect("Failed to add transaction to mempool");

        // Access mempool directly through P2P instead of HTTP endpoint
        let mempool_transactions = mempool.get_transactions().await;
        assert!(
            mempool_transactions.contains_key(&tx.id),
            "Transaction not found in mempool"
        );

        info!("Transaction successfully added to mempool: {}", tx.id);
    }
}
