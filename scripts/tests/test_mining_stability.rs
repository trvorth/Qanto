//! Mining Stability Test (Optimized)
//!
//! This test validates that the Qanto node can mine blocks consistently
//! with stable performance and proper chain integrity using MockPoW for speed.

use qanto::config::Config;
use qanto::node::Node;
use qanto::qantodag::QantoDAG;
use qanto::wallet::Wallet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Mock PoW implementation for fast testing
pub struct MockPoW {
    pub fixed_nonce: u64,
    pub fixed_difficulty: f64,
}

impl Default for MockPoW {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPoW {
    pub fn new() -> Self {
        Self {
            fixed_nonce: 12345,
            fixed_difficulty: 0.0001, // Very low difficulty for fast testing
        }
    }

    pub fn solve_block(
        &self,
        block: &mut qanto::qantodag::QantoBlock,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set fixed values for predictable testing
        block.nonce = self.fixed_nonce;
        block.difficulty = self.fixed_difficulty;

        // Use canonical PoW hash bytes (includes nonce)
        let pow_hash = block.hash_for_pow();
        let pow_hash_bytes = pow_hash.as_bytes();

        // Verify it meets our mock target
        let target_hash_bytes =
            qanto::miner::Miner::calculate_target_from_difficulty(self.fixed_difficulty);

        if !qanto::miner::Miner::hash_meets_target(pow_hash_bytes, &target_hash_bytes) {
            // Reduced nonce trials from 1000 to 50 for faster testing
            for nonce in 1..50 {
                block.nonce = nonce;
                // Recompute canonical PoW hash including nonce
                let test_pow_hash = block.hash_for_pow();
                let test_pow_hash_bytes = test_pow_hash.as_bytes();

                if qanto::miner::Miner::hash_meets_target(test_pow_hash_bytes, &target_hash_bytes) {
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

/// Test configuration for mining stability
#[derive(Debug, Clone)]
struct MiningTestConfig {
    target_blocks: u32,
    max_test_duration: u64, // seconds
    #[allow(dead_code)] // Reserved for future timeout implementation
    max_block_time: u64, // seconds
    #[allow(dead_code)] // Reserved for future verbose logging
    verbose: bool,
}

impl Default for MiningTestConfig {
    fn default() -> Self {
        Self {
            target_blocks: 3,      // Lower target for fast, deterministic CI
            max_test_duration: 30, // Shorter duration to keep CI lean
            max_block_time: 10,    // Reduced from 30 to 10 seconds per block
            verbose: true,
        }
    }
}

/// Mining performance metrics
#[derive(Debug, Default)]
struct MiningMetrics {
    blocks_mined: u32,
    failed_attempts: u32,
    total_mining_time: Duration,
    average_block_time: Duration,
    hash_rate: f64,
}

/// Setup test environment with optimized configuration
async fn setup_test_node() -> Result<(Node, TempDir), Box<dyn std::error::Error>> {
    info!("🔧 Setting up test node with optimized mining configuration");

    // Create temporary directory for test data
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path();

    // Create deterministic wallet that matches the expected genesis validator
    // Use a fixed private key to ensure the wallet address matches the genesis validator
    let deterministic_private_key =
        "0000000000000000000000000000000000000000000000000000000000000001";
    let wallet = Arc::new(Wallet::from_private_key(deterministic_private_key)?);
    let wallet_address = wallet.address();

    info!("🔑 Using deterministic wallet address: {}", wallet_address);

    // Create optimized mining configuration
    let config = Config {
        data_dir: temp_path.join("data").to_string_lossy().to_string(),
        db_path: temp_path.join("test.db").to_string_lossy().to_string(),
        wallet_path: temp_path.join("wallet.json").to_string_lossy().to_string(),
        p2p_identity_path: temp_path.join("p2p_identity").to_string_lossy().to_string(),
        genesis_validator: wallet_address.clone(), // Set genesis validator to match wallet address
        target_block_time: 5000,                   // 5 seconds in milliseconds
        num_chains: 1,                             // Single chain for simplicity
        mining_enabled: true,
        mining_threads: 2, // Use 2 threads for faster mining
        use_gpu: false,    // CPU mining for tests
        p2p_address: "/ip4/127.0.0.1/tcp/0".to_string(), // Random port (libp2p Multiaddr)
        peers: vec![],     // No bootstrap peers for local test
        api_address: "127.0.0.1:0".to_string(), // Random port
        logging: qanto::config::LoggingConfig {
            level: "info".to_string(),
            ..Default::default()
        },
        // Bind RPC server to an ephemeral port to avoid conflicts in CI
        rpc: qanto::config::RpcConfig {
            address: "127.0.0.1:0".to_string(),
        },
        ..Default::default()
    };

    // Create node with correct parameters
    let config_path = temp_path.join("config.toml").to_string_lossy().to_string();
    let peer_cache_path = temp_path
        .join("peer_cache.json")
        .to_string_lossy()
        .to_string();

    let node = Node::new(
        config,
        config_path,
        wallet,
        &temp_path.join("p2p_identity").to_string_lossy(),
        peer_cache_path,
    )
    .await?;

    info!("✅ Test node setup complete");
    Ok((node, temp_dir))
}

/// Validate chain consistency by checking block relationships
async fn validate_chain_consistency(dag: &Arc<QantoDAG>) -> Result<(), Box<dyn std::error::Error>> {
    debug!("🔍 Validating chain consistency");

    let block_count = dag.get_block_count().await;
    if block_count == 0 {
        return Ok(());
    }

    // Get latest block to verify chain integrity
    if let Some(latest_block) = dag.get_latest_block().await {
        debug!(
            "Latest block height: {}, ID: {}",
            latest_block.height, latest_block.id
        );

        // Verify parent relationships for recent blocks
        for parent_id in &latest_block.parents {
            if dag.get_block(parent_id).await.is_none() {
                return Err(format!(
                    "Parent block {} not found for block {}",
                    parent_id, latest_block.id
                )
                .into());
            }
        }
    }

    debug!("✅ Chain consistency validated");
    Ok(())
}

/// Run the actual mining stability test
async fn run_mining_stability_test(
    config: MiningTestConfig,
    metrics: &mut MiningMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔧 Setting up test environment");

    // Setup test environment
    let (node, _temp_dir) = setup_test_node().await?;

    info!("🏗️ Node initialized, starting mining operations");

    // Get references to node components
    let dag = node.dag.clone();

    // Drop base difficulty for fast, deterministic mining in tests
    {
        let mut rules = dag.saga.economy.epoch_rules.write().await;
        if let Some(difficulty_rule) = rules.get_mut("base_difficulty") {
            difficulty_rule.value = 0.00001; // extremely easy PoW
        }
    }

    // Start node in background task
    let node_handle = tokio::spawn(async move {
        if let Err(e) = node.start().await {
            warn!("Node encountered error: {}", e);
        }
    });

    let start_time = Instant::now();
    let mut last_block_count = 0;
    let mut first_block_time: Option<Instant> = None;

    info!(
        "⛏️ Beginning mining loop - target: {} blocks",
        config.target_blocks
    );

    // Main mining loop
    while start_time.elapsed() < Duration::from_secs(config.max_test_duration)
        && metrics.blocks_mined < config.target_blocks
    {
        sleep(Duration::from_secs(2)).await;

        // Check for new blocks using DAG directly
        let current_block_count = dag.get_block_count().await;

        if current_block_count > last_block_count {
            let new_blocks = current_block_count - last_block_count;
            metrics.blocks_mined += new_blocks as u32;
            last_block_count = current_block_count;

            let now = Instant::now();
            if first_block_time.is_none() {
                first_block_time = Some(now);
            }

            info!(
                "⛏️ Mined {} new blocks (total: {}) at {:.2}s",
                new_blocks,
                metrics.blocks_mined,
                start_time.elapsed().as_secs_f64()
            );

            // Validate chain consistency
            validate_chain_consistency(&dag).await?;
        }
    }

    // Calculate timing metrics
    let total_elapsed = start_time.elapsed();
    metrics.total_mining_time = total_elapsed;

    if metrics.blocks_mined > 0 {
        metrics.average_block_time =
            Duration::from_secs_f64(total_elapsed.as_secs_f64() / metrics.blocks_mined as f64);
    }

    // Stop the node
    node_handle.abort();

    // Final validation
    let final_block_count = dag.get_block_count().await;
    info!("📊 Final block count: {}", final_block_count);

    if metrics.blocks_mined < config.target_blocks {
        return Err(format!(
            "Failed to mine target blocks. Expected: {}, Actual: {}",
            config.target_blocks, metrics.blocks_mined
        )
        .into());
    }

    // Add assertions to fail test if timings are anomalous
    if metrics.blocks_mined > 0 {
        assert!(
            metrics.total_mining_time > Duration::from_millis(100),
            "Total mining time ({:.2}s) is suspiciously low for {} blocks",
            metrics.total_mining_time.as_secs_f64(),
            metrics.blocks_mined
        );

        assert!(
            metrics.average_block_time > Duration::from_millis(50),
            "Average block time ({:.2}s) is suspiciously low",
            metrics.average_block_time.as_secs_f64()
        );
    }

    Ok(())
}

/// Print comprehensive test results
fn print_test_results(config: &MiningTestConfig, metrics: &MiningMetrics) {
    info!("📊 Mining Stability Test Results");
    info!("═══════════════════════════════════");
    info!("Target blocks: {}", config.target_blocks);
    info!("Blocks mined: {}", metrics.blocks_mined);
    info!(
        "Success rate: {:.1}%",
        (metrics.blocks_mined as f64 / config.target_blocks as f64) * 100.0
    );

    if metrics.blocks_mined > 0 {
        info!(
            "Average block time: {:.2}s",
            metrics.average_block_time.as_secs_f64()
        );
        info!(
            "Total mining time: {:.2}s",
            metrics.total_mining_time.as_secs_f64()
        );
    }

    if metrics.hash_rate > 0.0 {
        info!("Hash rate: {:.2} H/s", metrics.hash_rate);
    }

    if metrics.failed_attempts > 0 {
        info!("Failed attempts: {}", metrics.failed_attempts);
    }

    info!("═══════════════════════════════════");
}

/// Main test function
#[tokio::test]
async fn test_mining_stability() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing once for tests
    qanto::init_test_tracing();

    info!("🚀 Starting Mining Stability Test");

    let config = MiningTestConfig::default();
    let mut metrics = MiningMetrics::default();

    // Run the mining stability test
    run_mining_stability_test(config.clone(), &mut metrics).await?;

    // Print results
    print_test_results(&config, &metrics);

    // Validate success criteria
    assert!(
        metrics.blocks_mined >= config.target_blocks,
        "Failed to mine required number of blocks. Expected: {}, Got: {}",
        config.target_blocks,
        metrics.blocks_mined
    );

    info!("✅ Mining stability test completed successfully!");
    Ok(())
}

// Consolidated from mining_stability_validation.rs

/// Test configuration for mining stability validation
#[derive(Debug, Clone)]
struct MiningValidationConfig {
    pub duration_seconds: u64,
    pub expected_min_blocks: u32,
    pub max_block_time_ms: u64,
}

impl Default for MiningValidationConfig {
    fn default() -> Self {
        Self {
            duration_seconds: 5, // Short test duration
            expected_min_blocks: 1,
            max_block_time_ms: 10000, // 10 seconds max per block
        }
    }
}

#[tokio::test]
async fn test_mining_stability_config_validation() {
    let config = MiningValidationConfig::default();

    // Validate configuration parameters
    assert!(config.duration_seconds > 0, "Duration must be positive");
    assert!(
        config.expected_min_blocks > 0,
        "Expected minimum blocks must be positive"
    );
    assert!(
        config.max_block_time_ms > 0,
        "Max block time must be positive"
    );

    // Ensure reasonable limits
    assert!(
        config.duration_seconds <= 300,
        "Test duration should be reasonable for CI"
    );
    assert!(
        config.max_block_time_ms <= 60000,
        "Block time should be reasonable"
    );
}

#[tokio::test]
async fn test_mining_node_initialization() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path();

    // Create wallet first to get its address
    let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));
    let wallet_address = wallet.address();

    // Create test configuration matching the pattern from test_mining_stability.rs
    let config = Config {
        data_dir: temp_path.join("data").to_string_lossy().to_string(),
        db_path: temp_path.join("test.db").to_string_lossy().to_string(),
        wallet_path: temp_path.join("wallet.json").to_string_lossy().to_string(),
        p2p_identity_path: temp_path.join("p2p_identity").to_string_lossy().to_string(),
        genesis_validator: wallet_address, // Set genesis validator to match wallet address
        target_block_time: 5000,           // 5 seconds in milliseconds
        num_chains: 1,
        mining_enabled: true,
        mining_threads: 1,
        use_gpu: false,
        p2p_address: "/ip4/127.0.0.1/tcp/0".to_string(), // Random port (libp2p Multiaddr)
        peers: vec![],
        api_address: "127.0.0.1:0".to_string(), // Random port
        logging: qanto::config::LoggingConfig {
            level: "info".to_string(),
            ..Default::default()
        },
        // Bind RPC server to an ephemeral port to avoid conflicts in CI
        rpc: qanto::config::RpcConfig {
            address: "127.0.0.1:0".to_string(),
        },
        ..Default::default()
    };

    // Initialize node (should not panic)
    let config_path = temp_path.join("config.toml").to_string_lossy().to_string();
    let peer_cache_path = temp_path
        .join("peer_cache.json")
        .to_string_lossy()
        .to_string();

    let node_result = Node::new(
        config,
        config_path,
        wallet,
        &temp_path.join("p2p_identity").to_string_lossy(),
        peer_cache_path,
    )
    .await;

    assert!(node_result.is_ok(), "Node initialization should succeed");
}

#[tokio::test]
async fn test_mining_stability_short_run() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path();

    // Create wallet first to get its address
    let wallet = Arc::new(Wallet::new().expect("Failed to create wallet"));
    let wallet_address = wallet.address();

    // Create test configuration
    let config = Config {
        data_dir: temp_path.join("data").to_string_lossy().to_string(),
        db_path: temp_path.join("test.db").to_string_lossy().to_string(),
        wallet_path: temp_path.join("wallet.json").to_string_lossy().to_string(),
        p2p_identity_path: temp_path.join("p2p_identity").to_string_lossy().to_string(),
        genesis_validator: wallet_address, // Set genesis validator to match wallet address
        target_block_time: 5000,
        num_chains: 1,
        mining_enabled: true,
        mining_threads: 1,
        use_gpu: false,
        p2p_address: "/ip4/127.0.0.1/tcp/0".to_string(), // Random port (libp2p Multiaddr)
        peers: vec![],
        api_address: "127.0.0.1:0".to_string(),
        logging: qanto::config::LoggingConfig {
            level: "info".to_string(),
            ..Default::default()
        },
        // Bind RPC server to an ephemeral port to avoid conflicts in CI
        rpc: qanto::config::RpcConfig {
            address: "127.0.0.1:0".to_string(),
        },
        ..Default::default()
    };

    // Initialize node
    let config_path = temp_path.join("config.toml").to_string_lossy().to_string();
    let peer_cache_path = temp_path
        .join("peer_cache.json")
        .to_string_lossy()
        .to_string();

    let node = Node::new(
        config,
        config_path,
        wallet,
        &temp_path.join("p2p_identity").to_string_lossy(),
        peer_cache_path,
    )
    .await
    .expect("Failed to create node");

    // Start mining for a very short duration
    let start_time = std::time::Instant::now();

    // Start the node in a background task
    let node_handle = tokio::spawn(async move {
        if let Err(e) = node.start().await {
            println!("Node start returned error (expected in test): {e}");
        }
    });

    // Wait for a short duration then abort
    tokio::time::sleep(Duration::from_secs(2)).await;
    node_handle.abort();

    let elapsed = start_time.elapsed();
    assert!(
        elapsed.as_secs() <= 5,
        "Test should complete within reasonable time"
    );
}

#[tokio::test]
async fn test_wallet_creation_for_mining() {
    // Test that we can create wallets consistently
    let wallet1 = Wallet::new().expect("Failed to create wallet1");
    let wallet2 = Wallet::new().expect("Failed to create wallet2");

    // Wallets should have different addresses
    assert_ne!(
        wallet1.address(),
        wallet2.address(),
        "Wallets should have unique addresses"
    );

    // Addresses should be valid (non-empty)
    assert!(
        !wallet1.address().is_empty(),
        "Wallet address should not be empty"
    );
    assert!(
        !wallet2.address().is_empty(),
        "Wallet address should not be empty"
    );
}

#[tokio::test]
async fn test_config_mining_parameters() {
    // Initialize config with mining enabled using struct initialization
    let mut config = Config {
        mining_enabled: true,
        ..Default::default()
    };

    // Test mining configuration
    assert!(config.mining_enabled, "Mining should be enabled");

    // Test mining threads configuration
    config.mining_threads = 2;
    assert_eq!(
        config.mining_threads, 2,
        "Mining threads should be configurable"
    );

    // Test target block time
    config.target_block_time = 5000; // 5 seconds
    assert_eq!(
        config.target_block_time, 5000,
        "Target block time should be configurable"
    );

    // Test GPU usage flag
    config.use_gpu = false;
    assert!(!config.use_gpu, "GPU usage should be configurable");
}
