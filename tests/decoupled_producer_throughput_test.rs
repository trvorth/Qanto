use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use qanto::block_producer::DecoupledProducer;
use qanto::config::LoggingConfig;
use qanto::mempool::Mempool;
use qanto::miner::{Miner, MinerConfig};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::transaction::Transaction;
use qanto::types::{HomomorphicEncrypted, UTXO};
use qanto::wallet::Wallet;

const TEST_TIMEOUT_SECS: u64 = 30;
const TARGET_BPS: f64 = 32.0; // Original target
const ENHANCED_TARGET_BPS: f64 = 35.0; // Enhanced target remains
const BLOCK_CREATION_INTERVAL_MS: u64 = 31; // Reverted to production/test default
const MINING_WORKERS: usize = 8; // Keep workers consistent with prior setup

/// Create an optimized test environment for DecoupledProducer throughput testing
async fn create_decoupled_producer_test_environment() -> (
    Arc<QantoDAG>,
    Arc<Wallet>,
    Arc<RwLock<Mempool>>,
    Arc<RwLock<HashMap<String, UTXO>>>,
    Arc<Miner>,
) {
    // Create optimized storage config
    let storage_config = StorageConfig {
        data_dir: PathBuf::from("/tmp/test_decoupled_producer_throughput"),
        max_file_size: 1024 * 1024 * 100, // 100MB
        compression_enabled: false,       // Disable for speed
        encryption_enabled: false,        // Disable for speed
        wal_enabled: true,                // Enable for consistency
        sync_writes: false,               // Disable for speed
        cache_size: 1024 * 1024 * 128,    // 128MB cache
        compaction_threshold: 1000,
        max_open_files: 1000,
        ..StorageConfig::default()
    };

    // Clean up any existing test data
    let _ = std::fs::remove_dir_all(&storage_config.data_dir);

    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    // Create optimized DAG config
    let dag_config = QantoDagConfig {
        initial_validator: "test_validator".to_string(),
        target_block_time: 31, // 31ms for 32 BPS target
        num_chains: 8,         // More chains for better parallelism
        dev_fee_rate: 0.0,     // No fees for testing
    };

    let saga_pallet = Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ));

    let logging_config = LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false, // Disable for performance
        celebration_log_level: "error".to_string(),
        celebration_throttle_per_min: Some(1),
    };

    let dag = QantoDAG::new(dag_config, saga_pallet, storage, logging_config.clone())
        .expect("Failed to create DAG");

    // Force very low PoW difficulty for fast CPU mining in throughput tests.
    // This mirrors the approach used in block_validation_test.rs.
    {
        let mut rules = dag.saga.economy.epoch_rules.write().await;
        if let Some(difficulty_rule) = rules.get_mut("base_difficulty") {
            difficulty_rule.value = 0.000001; // ultra-low difficulty for test speed
        }
        // Ensure generous block rate limit to avoid capping throughput
        if let Some(bpm_rule) = rules.get_mut("max_blocks_per_minute") {
            bpm_rule.value = 64.0 * 60.0; // 64 BPS ceiling
        }
    }

    // Create wallet and add as validator
    let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));
    dag.add_validator(wallet.address(), 1_000_000).await;

    // Create mempool with original capacity
    let mempool = Arc::new(RwLock::new(Mempool::new(
        3600,             // 1 hour timeout
        10 * 1024 * 1024, // 10MB max size (original value)
        10000,            // 10k max transactions
    )));

    // Initialize UTXOs
    let utxos = Arc::new(RwLock::new(HashMap::new()));

    // Create optimized miner config
    let miner_config = MinerConfig {
        address: wallet.address(),
        dag: dag.clone(),
        target_block_time: 31, // Match DAG config for 32 BPS
        use_gpu: false,        // CPU mining for consistency
        zk_enabled: false,     // Disable ZK for speed
        threads: MINING_WORKERS,
        logging_config,
    };

    let miner = Arc::new(Miner::new(miner_config).expect("Failed to create miner"));

    (dag, wallet, mempool, utxos, miner)
}

// Add this import if not already present
use qanto::transaction::TransactionConfig;

// In create_decoupled_producer_test_environment, after creating dag, add sender as validator if needed

/// Populate mempool with valid, signed transactions spending seeded UTXOs
async fn populate_mempool_with_test_transactions(
    mempool: &Arc<RwLock<Mempool>>,
    dag: &Arc<QantoDAG>,
    global_utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    sender_wallet: &Wallet,
    count: usize,
) {
    use qanto::transaction::{Input, Output};
    use tracing::debug;

    let sender_addr = sender_wallet.address();
    let (qr_private_key, _) = sender_wallet
        .get_keypair()
        .expect("Failed to get sender keypair");

    // Create receiver address for spends
    let receiver_wallet = Wallet::new().expect("Failed to create receiver wallet");
    let receiver_addr = receiver_wallet.address();

    // Deterministic homomorphic encryption public key for test outputs
    let (he_public_key, _he_private_key) = HomomorphicEncrypted::generate_keypair();

    // Collect UTXOs for sender
    let utxos_guard = global_utxos.read().await;
    let sender_utxos: Vec<UTXO> = utxos_guard
        .values()
        .filter(|u| u.address == sender_addr)
        .cloned()
        .collect();
    drop(utxos_guard);

    if sender_utxos.len() < count {
        warn!(
            "Not enough UTXOs for {} transactions (available: {})",
            count,
            sender_utxos.len()
        );
        return;
    }

    let mempool_guard = mempool.write().await;
    let tx_timestamps: Arc<RwLock<HashMap<String, u64>>> = Arc::new(RwLock::new(HashMap::new()));

    // Use conservative original fees/gas
    const TX_AMOUNT: u64 = 1_000;
    const TX_FEE: u64 = 10;
    const GAS_LIMIT: u64 = 21_000;
    const GAS_PRICE: u64 = 1;
    const PRIORITY_FEE: u64 = 0;

    for (i, utxo) in sender_utxos.into_iter().take(count).enumerate() {
        // Ensure inputs cover amount + fee
        if utxo.amount < TX_AMOUNT + TX_FEE {
            warn!(
                "Skipping UTXO {} due to insufficient funds: {} < {}",
                format!("{}_{}", utxo.tx_id, utxo.output_index),
                utxo.amount,
                TX_AMOUNT + TX_FEE
            );
            continue;
        }

        let config = TransactionConfig {
            sender: sender_addr.clone(),
            receiver: receiver_addr.clone(),
            amount: TX_AMOUNT,
            fee: TX_FEE,
            gas_limit: GAS_LIMIT,
            gas_price: GAS_PRICE,
            priority_fee: PRIORITY_FEE,
            inputs: vec![Input {
                tx_id: utxo.tx_id.clone(),
                output_index: utxo.output_index,
            }],
            outputs: vec![Output {
                address: receiver_addr.clone(),
                amount: TX_AMOUNT,
                homomorphic_encrypted: HomomorphicEncrypted::new(TX_AMOUNT, &he_public_key),
            }],
            metadata: Some(HashMap::new()),
            tx_timestamps: tx_timestamps.clone(),
        };

        match Transaction::new(config, &qr_private_key).await {
            Ok(mut tx) => {
                tx.id = format!("test_tx_{}", i);
                let utxos_guard = global_utxos.read().await;
                if let Err(e) = mempool_guard.add_transaction(tx, &utxos_guard, dag).await {
                    warn!("Failed to add test transaction {}: {}", i, e);
                } else {
                    debug!("Successfully added test transaction {}", i);
                }
            }
            Err(e) => {
                warn!("Failed to create signed transaction {}: {}", i, e);
            }
        }
    }

    info!("Added test transactions to mempool");
}

/// Seed UTXO set by creating a single valid coinbase-style funding transaction
/// that creates `count` outputs for the sender.
async fn seed_sender_utxos(
    global_utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    sender_wallet: &Wallet,
    count: usize,
    amount_per_output: u64,
) {
    use qanto::transaction::Output;
    use qanto::types::QuantumResistantSignature;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tracing::info;

    let sender_addr = sender_wallet.address();

    // Generate deterministic HE key for outputs
    let (he_public_key, _he_private_key) = HomomorphicEncrypted::generate_keypair();

    // Build outputs for funding transaction
    let mut outputs = Vec::with_capacity(count);
    for _ in 0..count {
        outputs.push(Output {
            address: sender_addr.clone(),
            amount: amount_per_output,
            homomorphic_encrypted: HomomorphicEncrypted::new(amount_per_output, &he_public_key),
        });
    }

    // Create a coinbase-style transaction to mint these outputs (no inputs needed)
    // Construct directly since the coinbase helper is crate-private.
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    let funding_tx = Transaction {
        id: "funding_coinbase_seed_tx".to_string(),
        sender: "0x0000000000000000000000000000000000000000".to_string(), // zero address for coinbase
        receiver: sender_addr.clone(),
        amount: outputs.iter().map(|o| o.amount).sum(),
        fee: 0,
        gas_limit: 0,
        gas_used: 0,
        gas_price: 0,
        priority_fee: 0,
        inputs: vec![],
        outputs,
        timestamp,
        metadata: HashMap::new(),
        signature: QuantumResistantSignature::default(),
        fee_breakdown: None,
    };

    // Insert UTXOs derived from this transaction
    let mut utxos_guard = global_utxos.write().await;
    for output_index in 0..count as u32 {
        let utxo = funding_tx.generate_utxo(output_index);
        let utxo_key = format!("{}_{}", utxo.tx_id, utxo.output_index);
        utxos_guard.insert(utxo_key, utxo);
    }
    drop(utxos_guard);

    info!(
        "Seeded {} coinbase UTXOs for sender {} via funding transaction {}",
        count, sender_addr, funding_tx.id
    );
}

/// Test DecoupledProducer throughput with 32 BPS target
#[tokio::test]
async fn test_decoupled_producer_throughput_32_bps() {
    qanto::init_test_tracing();

    info!("🚀 Starting DecoupledProducer throughput test (32 BPS target)");

    let (dag, wallet, mempool, utxos, miner) = create_decoupled_producer_test_environment().await;

    // Candidate transaction selection now uses MAX_TRANSACTIONS_PER_BLOCK; no override needed.

    // Create sender wallet
    let sender_wallet = Wallet::new().expect("Failed to create sender wallet");
    let sender_addr = sender_wallet.address();
    let (sender_private_key, sender_public_key) = sender_wallet
        .get_keypair()
        .expect("Failed to get sender keypair");

    // Add sender as validator
    dag.add_validator(sender_addr.clone(), 1_000_000).await;

    // Create temporary miner for funding
    let funding_miner_config = MinerConfig {
        address: sender_addr.clone(),
        dag: dag.clone(),
        target_block_time: 31,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };
    let funding_miner =
        Arc::new(Miner::new(funding_miner_config).expect("Failed to create funding miner"));

    // Create empty mempool for coinbase
    let empty_mempool = Arc::new(RwLock::new(Mempool::new(1, 1, 1)));

    // Create candidate funding block (coinbase only)
    let mut funding_block = dag
        .create_candidate_block(
            &sender_private_key,
            &sender_public_key,
            &sender_addr,
            &empty_mempool,
            &utxos,
            0, // chain_id
            &funding_miner,
            None,
            None,
        )
        .await
        .expect("Failed to create funding block");

    // Mine the funding block
    funding_miner
        .solve_pow(&mut funding_block)
        .expect("Failed to mine funding block");
    let mined_funding_block = funding_block;

    // Add to DAG
    dag.add_block(
        mined_funding_block,
        &utxos,
        Some(&empty_mempool),
        Some(&sender_addr),
    )
    .await
    .expect("Failed to add funding block");

    // Verify UTXO created
    let sender_utxos_count = {
        let utxos_guard = utxos.read().await;
        utxos_guard
            .values()
            .filter(|u| u.address == sender_addr)
            .count()
    };
    info!("Sender UTXOs after funding: {}", sender_utxos_count);
    assert!(sender_utxos_count > 0, "No UTXOs created for sender");

    // Seed coinbase-style UTXOs to ensure throughput test has enough inputs
    // Amount per UTXO safely above tx amount+fee (1000 + 10)
    seed_sender_utxos(&utxos, &sender_wallet, 3000, 2_000).await;

    // Populate mempool using real UTXOs
    populate_mempool_with_test_transactions(&mempool, &dag, &utxos, &sender_wallet, 3000).await;

    // Check mempool after population
    let mempool_guard = mempool.read().await;
    let mempool_size = mempool_guard.len().await;
    info!("Mempool populated with {} transactions", mempool_size);
    drop(mempool_guard);

    let shutdown_token = CancellationToken::new();

    // Create DecoupledProducer with optimized settings
    let producer = DecoupledProducer::new(
        dag.clone(),
        wallet,
        mempool,
        utxos,
        miner,
        BLOCK_CREATION_INTERVAL_MS,
        MINING_WORKERS,
        256,                      // Larger candidate buffer for better throughput
        256,                      // Larger mined buffer for better throughput
        LoggingConfig::default(), // Use default logging config for tests
        shutdown_token.clone(),
    );

    // Record initial block count
    let initial_block_count = dag.blocks.len();
    info!("Initial block count: {}", initial_block_count);

    // Start the producer
    let producer_handle = tokio::spawn(async move { producer.run().await });

    // Run for test duration
    let test_start = Instant::now();

    // Check block count periodically
    for i in 1..=TEST_TIMEOUT_SECS {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let current_block_count = dag.blocks.len();
        let blocks_so_far = current_block_count - initial_block_count;
        info!("After {}s: {} blocks produced", i, blocks_so_far);
    }

    // Shutdown the producer
    shutdown_token.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), producer_handle).await;

    let test_duration = test_start.elapsed();
    let final_block_count = dag.blocks.len();
    let blocks_produced = final_block_count - initial_block_count;

    let actual_bps = blocks_produced as f64 / test_duration.as_secs_f64();

    info!("📊 DecoupledProducer Throughput Test Results:");
    info!("  Test Duration: {:.2}s", test_duration.as_secs_f64());
    info!("  Initial Blocks: {}", initial_block_count);
    info!("  Final Blocks: {}", final_block_count);
    info!("  Blocks Produced: {}", blocks_produced);
    info!("  Actual BPS: {:.2}", actual_bps);
    info!("  Target BPS: {:.2}", TARGET_BPS);

    // Persist results for diagnostics even if assertions fail.
    let _ = std::fs::create_dir_all("tests_output");
    let result_str = format!(
        "duration_sec={:.2}\ninitial_blocks={}\nfinal_blocks={}\nblocks_produced={}\nactual_bps={:.2}\ntarget_bps={:.2}\n",
        test_duration.as_secs_f64(),
        initial_block_count,
        final_block_count,
        blocks_produced,
        actual_bps,
        TARGET_BPS
    );
    let _ = std::fs::write("tests_output/decoupled_bps_32.txt", result_str);

    // Clean up test data
    let _ = std::fs::remove_dir_all("/tmp/test_decoupled_producer_throughput");

    // Assertions
    assert!(
        blocks_produced > 0,
        "DecoupledProducer should have produced at least 1 block, got {}",
        blocks_produced
    );

    assert!(
        actual_bps >= TARGET_BPS * 0.9, // Tighter tolerance for 32 BPS target
        "DecoupledProducer BPS {:.2} should be at least 90% of target {:.2}",
        actual_bps,
        TARGET_BPS
    );

    info!("✅ DecoupledProducer throughput test passed!");
}

/// Test DecoupledProducer throughput with enhanced 35 BPS target
#[tokio::test]
async fn test_decoupled_producer_throughput_enhanced() {
    qanto::init_test_tracing();

    info!("🚀 Starting DecoupledProducer enhanced throughput test (35 BPS target)");

    let (dag, wallet, mempool, utxos, miner) = create_decoupled_producer_test_environment().await;

    // Candidate transaction selection now uses MAX_TRANSACTIONS_PER_BLOCK; no override needed.

    // Create sender wallet
    let sender_wallet = Wallet::new().expect("Failed to create sender wallet");
    let sender_addr = sender_wallet.address();
    let (sender_private_key, sender_public_key) = sender_wallet
        .get_keypair()
        .expect("Failed to get sender keypair");

    // Add sender as validator
    dag.add_validator(sender_addr.clone(), 1_000_000).await;

    // Create temporary miner for funding
    let funding_miner_config = MinerConfig {
        address: sender_addr.clone(),
        dag: dag.clone(),
        target_block_time: 31,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: LoggingConfig::default(),
    };
    let funding_miner =
        Arc::new(Miner::new(funding_miner_config).expect("Failed to create funding miner"));

    // Create empty mempool for coinbase
    let empty_mempool = Arc::new(RwLock::new(Mempool::new(1, 1, 1)));

    // Create candidate funding block (coinbase only)
    let mut funding_block = dag
        .create_candidate_block(
            &sender_private_key,
            &sender_public_key,
            &sender_addr,
            &empty_mempool,
            &utxos,
            0, // chain_id
            &funding_miner,
            None,
            None,
        )
        .await
        .expect("Failed to create funding block");

    // Mine the funding block
    funding_miner
        .solve_pow(&mut funding_block)
        .expect("Failed to mine funding block");
    let mined_funding_block = funding_block;

    // Add to DAG
    dag.add_block(
        mined_funding_block,
        &utxos,
        Some(&empty_mempool),
        Some(&sender_addr),
    )
    .await
    .expect("Failed to add funding block");

    // Verify UTXO created
    let sender_utxos_count = {
        let utxos_guard = utxos.read().await;
        utxos_guard
            .values()
            .filter(|u| u.address == sender_addr)
            .count()
    };
    info!("Sender UTXOs after funding: {}", sender_utxos_count);
    assert!(sender_utxos_count > 0, "No UTXOs created for sender");

    // Seed coinbase-style UTXOs to ensure enhanced throughput test has enough inputs
    seed_sender_utxos(&utxos, &sender_wallet, 3000, 2_000).await;

    // Populate mempool using real UTXOs
    populate_mempool_with_test_transactions(&mempool, &dag, &utxos, &sender_wallet, 3000).await;

    let shutdown_token = CancellationToken::new();

    // Create DecoupledProducer with more aggressive settings for enhanced test
    let producer = DecoupledProducer::new(
        dag.clone(),
        wallet,
        mempool,
        utxos,
        miner,
        25,                       // Even faster block creation (25ms = 40 BPS base rate)
        12,                       // More mining workers for enhanced performance
        300,                      // Even larger candidate buffer
        150,                      // Even larger mined buffer
        LoggingConfig::default(), // Use default logging config for tests
        shutdown_token.clone(),
    );

    // Record initial block count
    let initial_block_count = dag.blocks.len();
    info!("Initial block count: {}", initial_block_count);

    // Start the producer
    let producer_handle = tokio::spawn(async move { producer.run().await });

    // Run for test duration
    let test_start = Instant::now();
    tokio::time::sleep(Duration::from_secs(TEST_TIMEOUT_SECS)).await;

    // Shutdown the producer
    shutdown_token.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), producer_handle).await;

    let test_duration = test_start.elapsed();
    let final_block_count = dag.blocks.len();
    let blocks_produced = final_block_count - initial_block_count;

    let actual_bps = blocks_produced as f64 / test_duration.as_secs_f64();

    info!("📊 DecoupledProducer Enhanced Throughput Test Results:");
    info!("  Test Duration: {:.2}s", test_duration.as_secs_f64());
    info!("  Initial Blocks: {}", initial_block_count);
    info!("  Final Blocks: {}", final_block_count);
    info!("  Blocks Produced: {}", blocks_produced);
    info!("  Actual BPS: {:.2}", actual_bps);
    info!("  Target BPS: {:.2}", ENHANCED_TARGET_BPS);

    // Persist results for diagnostics even if assertions fail.
    let _ = std::fs::create_dir_all("tests_output");
    let result_str = format!(
        "duration_sec={:.2}\ninitial_blocks={}\nfinal_blocks={}\nblocks_produced={}\nactual_bps={:.2}\ntarget_bps={:.2}\n",
        test_duration.as_secs_f64(),
        initial_block_count,
        final_block_count,
        blocks_produced,
        actual_bps,
        ENHANCED_TARGET_BPS
    );
    let _ = std::fs::write("tests_output/decoupled_bps_35.txt", result_str);

    // Clean up test data
    let _ = std::fs::remove_dir_all("/tmp/test_decoupled_producer_throughput");

    // Assertions
    assert!(
        blocks_produced > 0,
        "DecoupledProducer should have produced at least 1 block, got {}",
        blocks_produced
    );

    assert!(
        actual_bps >= ENHANCED_TARGET_BPS * 0.85, // Allow 15% tolerance for enhanced test
        "DecoupledProducer enhanced BPS {:.2} should be at least 85% of target {:.2}",
        actual_bps,
        ENHANCED_TARGET_BPS
    );

    info!("✅ DecoupledProducer enhanced throughput test passed!");
}
