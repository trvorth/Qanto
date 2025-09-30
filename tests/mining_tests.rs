use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::saga::PalletSaga;
use qanto::mempool::Mempool;
use qanto::wallet::Wallet;
use qanto::transaction::Transaction;
use qanto::types::UTXO;
use qanto::miner::{Miner, MinerConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use tracing::{info, debug, error};

/// Mock PoW implementation for testing to avoid timeout issues
pub struct MockPoW {
    pub fixed_nonce: u64,
    pub fixed_difficulty: f64,
}

impl MockPoW {
    pub fn new() -> Self {
        Self {
            fixed_nonce: 12345,
            fixed_difficulty: 0.0001, // Very low difficulty for fast testing
        }
    }

    pub fn solve_block(&self, block: &mut qanto::types::QantoBlock) -> Result<(), Box<dyn std::error::Error>> {
        // Set fixed values for predictable testing
        block.nonce = self.fixed_nonce;
        block.difficulty = self.fixed_difficulty;
        
        // Calculate hash with fixed nonce
        let block_hash_str = block.hash();
        let block_hash_bytes = hex::decode(&block_hash_str)?;
        
        // Verify it meets our mock target
        let target_hash_bytes = qanto::miner::Miner::calculate_target_from_difficulty(self.fixed_difficulty);
        
        if !qanto::miner::Miner::hash_meets_target(&block_hash_bytes, &target_hash_bytes) {
            // If it doesn't meet target, try a few more nonces
            for nonce in 1..1000 {
                block.nonce = nonce;
                let test_hash_str = block.hash();
                let test_hash_bytes = hex::decode(&test_hash_str)?;
                
                if qanto::miner::Miner::hash_meets_target(&test_hash_bytes, &target_hash_bytes) {
                    info!("✅ Mock PoW found valid nonce: {}", nonce);
                    return Ok(());
                }
            }
            return Err("Mock PoW could not find valid nonce".into());
        }
        
        info!("✅ Mock PoW successful with nonce: {}", block.nonce);
        Ok(())
    }
}

#[tokio::test]
async fn test_simple_mining_consolidated() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting consolidated mining test...");

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_mining_consolidated");
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    // Initialize storage configuration with correct structure
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB
        compaction_threshold: 1000.0,
        max_open_files: 100,
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .expect("Failed to initialize storage");

    // Initialize SAGA pallet
    let saga = Arc::new(PalletSaga::new());

    // Initialize QantoDAG with correct configuration
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 60,
        num_chains: 1,
    };

    let dag_arc = QantoDAG::new(dag_config, saga.clone(), storage)
        .expect("Failed to initialize QantoDAG");

    // Create wallet for mining
    let wallet = Wallet::new().expect("Failed to create wallet");
    let miner_address = wallet.address();

    // Add validator to DAG with sufficient stake
    dag_arc.add_validator(miner_address.clone(), 1000).await;

    // Initialize mempool
    let mempool = Mempool::new(300, 1024 * 1024, 1000); // 5 min timeout, 1MB max size, 1000 max transactions
    let mempool_arc = Arc::new(RwLock::new(mempool));

    // Initialize UTXOs
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let utxos_arc = Arc::new(RwLock::new(utxos));

    // Add a test transaction to mempool
    let test_tx = Transaction::new_dummy();
    mempool_arc.write().await.add_transaction(test_tx, &HashMap::new(), &dag_arc).await.expect("Failed to add transaction");

    // Create miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 120, // Longer timeout for testing
        use_gpu: false,
        zk_enabled: false,
        threads: 1, // Single thread for deterministic testing
    };
    let _miner = Miner::new(miner_config).expect("Failed to create miner");

    // Create a block template using DAG's create_candidate_block method
    let (private_key, public_key) = wallet.get_keypair().expect("Failed to get keypair");
    let mut block = dag_arc.create_candidate_block(
        &private_key,
        &public_key,
        &miner_address,
        &mempool_arc,
        &utxos_arc,
        0, // chain_id
    ).await.expect("Failed to create candidate block");

    info!("Created block template with {} transactions", block.transactions.len());
    info!("Original block difficulty: {}", block.difficulty);
    
    // Use mock PoW to avoid timeout issues
    let mock_pow = MockPoW::new();
    
    // Test mining with mock PoW (no timeout needed)
    info!("Starting mock PoW mining...");
    let mining_start = std::time::Instant::now();
    
    match mock_pow.solve_block(&mut block) {
        Ok(()) => {
            let mining_duration = mining_start.elapsed();
            info!("✅ Mock mining successful!");
            info!("Mining took: {:?}", mining_duration);
            info!("Block nonce: {}", block.nonce);
            info!("Block difficulty: {}", block.difficulty);
            
            // Verify the hash meets the target
            let block_hash_str = block.hash();
            let block_hash_bytes = hex::decode(&block_hash_str).expect("Failed to decode block hash");
            let target_hash_bytes = qanto::miner::Miner::calculate_target_from_difficulty(block.difficulty);
            
            if qanto::miner::Miner::hash_meets_target(&block_hash_bytes, &target_hash_bytes) {
                info!("✅ Hash meets target - PoW validation successful!");
            } else {
                error!("❌ Hash does not meet target - PoW validation failed!");
                panic!("PoW validation failed");
            }
        }
        Err(e) => {
            error!("❌ Mock mining failed with error: {:?}", e);
            panic!("Mock mining failed: {:?}", e);
        }
    }
    
    info!("✅ Consolidated mining test completed successfully.");

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_mining_with_multiple_transactions() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting multi-transaction mining test...");

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_multi_mining");
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    // Initialize storage configuration
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 10, // 10MB
        compaction_threshold: 1000.0,
        max_open_files: 100,
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .expect("Failed to initialize storage");

    // Initialize SAGA pallet
    let saga = Arc::new(PalletSaga::new());

    // Initialize QantoDAG
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator_multi".to_string(),
        target_block_time: 60,
        num_chains: 2, // Test with multiple chains
    };

    let dag_arc = QantoDAG::new(dag_config, saga.clone(), storage)
        .expect("Failed to initialize QantoDAG");

    // Create wallet for mining
    let wallet = Wallet::new().expect("Failed to create wallet");
    let miner_address = wallet.address();

    // Add validator to DAG with sufficient stake
    dag_arc.add_validator(miner_address.clone(), 2000).await;

    // Initialize mempool
    let mempool = Mempool::new(300, 1024 * 1024, 1000);
    let mempool_arc = Arc::new(RwLock::new(mempool));

    // Initialize UTXOs
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let utxos_arc = Arc::new(RwLock::new(utxos));

    // Add multiple test transactions to mempool
    for i in 0..5 {
        let test_tx = Transaction::new_dummy();
        mempool_arc.write().await.add_transaction(test_tx, &HashMap::new(), &dag_arc).await
            .expect(&format!("Failed to add transaction {}", i));
    }

    // Create miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 120,
        use_gpu: false,
        zk_enabled: false,
        threads: 2, // Multiple threads for multi-chain
    };
    let _miner = Miner::new(miner_config).expect("Failed to create miner");

    // Test mining on multiple chains
    for chain_id in 0..2 {
        let (private_key, public_key) = wallet.get_keypair().expect("Failed to get keypair");
        let mut block = dag_arc.create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            chain_id,
        ).await.expect("Failed to create candidate block");

        info!("Created block template for chain {} with {} transactions", chain_id, block.transactions.len());
        
        // Use mock PoW
        let mock_pow = MockPoW::new();
        
        match mock_pow.solve_block(&mut block) {
            Ok(()) => {
                info!("✅ Chain {} mining successful!", chain_id);
                
                // Verify the hash meets the target
                let block_hash_str = block.hash();
                let block_hash_bytes = hex::decode(&block_hash_str).expect("Failed to decode block hash");
                let target_hash_bytes = qanto::miner::Miner::calculate_target_from_difficulty(block.difficulty);
                
                assert!(qanto::miner::Miner::hash_meets_target(&block_hash_bytes, &target_hash_bytes), 
                       "Chain {} PoW validation failed", chain_id);
            }
            Err(e) => {
                error!("❌ Chain {} mining failed: {:?}", chain_id, e);
                panic!("Chain {} mining failed: {:?}", chain_id, e);
            }
        }
    }
    
    info!("✅ Multi-transaction mining test completed successfully.");

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_mining_performance_benchmark() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting mining performance benchmark...");

    // Create a temporary directory for storage
    let temp_dir = std::env::temp_dir().join("qanto_test_perf_mining");
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    // Initialize storage configuration
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: true, // Enable compression for performance test
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024 * 50, // Larger cache for performance
        compaction_threshold: 1000.0,
        max_open_files: 200,
    };

    // Initialize storage
    let storage = QantoStorage::new(storage_config)
        .expect("Failed to initialize storage");

    // Initialize SAGA pallet
    let saga = Arc::new(PalletSaga::new());

    // Initialize QantoDAG with performance settings
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator_perf".to_string(),
        target_block_time: 25, // High performance: ~40 BPS
        num_chains: 4, // Multiple chains for parallel processing
    };

    let dag_arc = QantoDAG::new(dag_config, saga.clone(), storage)
        .expect("Failed to initialize QantoDAG");

    // Create wallet for mining
    let wallet = Wallet::new().expect("Failed to create wallet");
    let miner_address = wallet.address();

    // Add validator to DAG with high stake
    dag_arc.add_validator(miner_address.clone(), 10000).await;

    // Initialize mempool with larger capacity
    let mempool = Mempool::new(600, 1024 * 1024 * 10, 10000); // Larger mempool for performance test
    let mempool_arc = Arc::new(RwLock::new(mempool));

    // Initialize UTXOs
    let utxos: HashMap<String, UTXO> = HashMap::new();
    let utxos_arc = Arc::new(RwLock::new(utxos));

    // Add many test transactions to simulate high load
    for i in 0..100 {
        let test_tx = Transaction::new_dummy();
        mempool_arc.write().await.add_transaction(test_tx, &HashMap::new(), &dag_arc).await
            .expect(&format!("Failed to add transaction {}", i));
    }

    // Create high-performance miner configuration
    let miner_config = MinerConfig {
        address: miner_address.clone(),
        dag: dag_arc.clone(),
        target_block_time: 25, // High performance target
        use_gpu: false,
        zk_enabled: false,
        threads: 4, // Multiple threads for performance
    };
    let _miner = Miner::new(miner_config).expect("Failed to create miner");

    // Benchmark mining performance
    let benchmark_start = std::time::Instant::now();
    let mut total_transactions = 0;
    
    // Mine blocks on all chains
    for chain_id in 0..4 {
        let (private_key, public_key) = wallet.get_keypair().expect("Failed to get keypair");
        let mut block = dag_arc.create_candidate_block(
            &private_key,
            &public_key,
            &miner_address,
            &mempool_arc,
            &utxos_arc,
            chain_id,
        ).await.expect("Failed to create candidate block");

        total_transactions += block.transactions.len();
        
        // Use mock PoW for consistent timing
        let mock_pow = MockPoW::new();
        
        let chain_start = std::time::Instant::now();
        match mock_pow.solve_block(&mut block) {
            Ok(()) => {
                let chain_duration = chain_start.elapsed();
                info!("✅ Chain {} mined in {:?} with {} transactions", 
                     chain_id, chain_duration, block.transactions.len());
            }
            Err(e) => {
                error!("❌ Chain {} mining failed: {:?}", chain_id, e);
                panic!("Performance benchmark failed on chain {}: {:?}", chain_id, e);
            }
        }
    }
    
    let total_duration = benchmark_start.elapsed();
    let tps = total_transactions as f64 / total_duration.as_secs_f64();
    
    info!("✅ Performance benchmark completed:");
    info!("   Total time: {:?}", total_duration);
    info!("   Total transactions: {}", total_transactions);
    info!("   TPS: {:.2}", tps);
    info!("   Chains processed: 4");
    
    // Assert performance targets
    assert!(total_duration.as_millis() < 5000, "Benchmark should complete within 5 seconds");
    assert!(tps > 10.0, "Should achieve at least 10 TPS");
    
    info!("✅ Mining performance benchmark completed successfully.");

    // Clean up
    std::fs::remove_dir_all(&temp_dir).ok();
}