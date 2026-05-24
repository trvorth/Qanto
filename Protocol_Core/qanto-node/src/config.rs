// src/config.rs

//! --- Qanto Node Configuration ---
//! v0.1.0 - Production & Standalone Ready
//! This module defines the configuration structure for a Qanto node.
//! It uses serde for deserialization from a TOML file and includes
//! robust validation logic to ensure that all configured parameters
//! are sane and within operational limits for a standalone system.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use sysinfo::System;
use thiserror::Error;

// --- Constants for Validation ---
// EVOLVED: Time is now in MILLISECONDS to support high BPS (32+ BPS = <31ms block time)
const MIN_TARGET_BLOCK_TIME: u64 = 25; // Minimum 25ms block time (~40 BPS)
const MAX_TARGET_BLOCK_TIME: u64 = 15000; // Maximum 15 seconds
const MAX_PEERS: usize = 128;
const MIN_DIFFICULTY: u64 = 1;
const MAX_DIFFICULTY: u64 = u64::MAX / 2; // More realistic max
const MAX_MINING_THREADS: usize = 256;
const MIN_CHAINS: u32 = 1;
const MAX_CHAINS: u32 = 32;

// --- Mining-specific validation constants (MICROSECOND PRECISION) ---
const MIN_MINING_INTERVAL_MS: u64 = 1; // Minimum 1ms mining interval for high-performance
const MAX_MINING_INTERVAL_MS: u64 = 3600000; // Maximum 1 hour in milliseconds
const MIN_DUMMY_TX_INTERVAL_MS: u64 = 1; // Minimum 1ms dummy TX interval
const MAX_DUMMY_TX_INTERVAL_MS: u64 = 1000; // Maximum 1 second in milliseconds for high-frequency TX generation
const MIN_DUMMY_TX_PER_CYCLE: u32 = 1; // Minimum 1 dummy TX per cycle
const MAX_DUMMY_TX_PER_CYCLE: u32 = 1000000; // Maximum 1M dummy TX per cycle
const MIN_MEMPOOL_MAX_AGE: u64 = 60; // Minimum 1 minute mempool age
const MAX_MEMPOOL_MAX_AGE: u64 = 86400; // Maximum 24 hours mempool age
const MIN_MEMPOOL_MAX_SIZE: usize = 1024; // Minimum 1KB mempool size
const MAX_MEMPOOL_MAX_SIZE: usize = 16 * 1024 * 1024 * 1024; // Maximum 16GB mempool size (increased for high performance)

// --- Mempool batch processing constants ---
const MIN_MEMPOOL_BATCH_SIZE: usize = 10; // Minimum 10 transactions per batch
const MAX_MEMPOOL_BATCH_SIZE: usize = 1000000; // Maximum 1M transactions per batch (increased for 10M TPS)
const MIN_MEMPOOL_BACKPRESSURE_THRESHOLD: u128 = 500_000_000; // Minimum 50% utilization
const MAX_MEMPOOL_BACKPRESSURE_THRESHOLD: u128 = 950_000_000; // Maximum 95% utilization

// --- Memory limit constants for TX generator backpressure ---
const SOFT_MEMORY_LIMIT: usize = 8 * 1024 * 1024; // 8MB soft limit for backpressure // 8MB soft limit
const HARD_MEMORY_LIMIT: usize = 10 * 1024 * 1024; // 10MB hard limit
const MIN_TX_BATCH_SIZE: usize = 1; // Minimum 1 transaction per batch
const MAX_TX_BATCH_SIZE: usize = 200000; // Maximum 200K transactions per batch (increased for high performance)
const MIN_ADAPTIVE_BATCH_THRESHOLD: u128 = 600_000_000; // Minimum 60% memory utilization for adaptive batching
const MAX_ADAPTIVE_BATCH_THRESHOLD: u128 = 900_000_000; // Maximum 90% memory utilization for adaptive batching

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration from '{path}': {source}")]
    Load {
        path: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("Failed to save configuration to '{path}': {source}")]
    Save {
        path: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("Validation failed: {0}")]
    Validation(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpcConfig {
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    // --- Network Configuration ---
    pub p2p_address: String,
    pub api_address: String,
    pub peers: Vec<String>,
    pub local_full_p2p_address: Option<String>,
    pub network_id: String,

    // --- Blockchain Configuration ---
    pub genesis_validator: String,
    pub contract_address: String,
    pub target_block_time: u64, // Now in milliseconds
    pub difficulty: u64,
    pub max_amount: u64,

    // --- Performance & Hardware ---
    #[serde(alias = "gpu_enabled")]
    pub use_gpu: bool,
    pub zk_enabled: bool,
    pub mining_threads: usize,
    pub mining_enabled: bool,
    pub adaptive_mining_enabled: bool, // Enable adaptive mining with difficulty adjustments

    // --- Block Producer Configuration ---
    pub producer_type: Option<String>, // "solo" or "decoupled" (default: "solo")

    // --- Telemetry Configuration ---
    pub hash_rate_interval_secs: Option<u64>, // Hash rate sampling interval (default: 5)
    pub enable_detailed_telemetry: Option<bool>, // Enable detailed telemetry logging

    // --- Adaptive Difficulty Configuration ---
    pub block_target_ms: Option<u64>, // Target block time in milliseconds
    pub solve_timeout_ms: Option<u64>, // Mining timeout in milliseconds
    pub difficulty_max_adjust_pct: Option<u128>, // Maximum difficulty adjustment percentage
    pub difficulty_smoothing_factor: Option<u128>, // Smoothing factor for difficulty adjustments
    pub difficulty_min: Option<u64>,  // Minimum difficulty value

    // --- Memory & Profiling Configuration ---
    pub db_cache_bytes: Option<usize>, // Database cache size in bytes
    pub mempool_max_bytes: Option<usize>, // Maximum mempool size in bytes

    // --- Dummy Transactions Configuration ---
    pub enable_dummy_tx: Option<bool>, // Enable dummy transaction generation
    pub max_dummy_per_block: Option<u32>, // Maximum dummy transactions per block

    // --- Sharding & Scaling ---
    pub num_chains: u32,
    pub mining_chain_id: u32,

    // Optional mining configuration parameters (MICROSECOND PRECISION)
    pub mining_interval_ms: Option<u64>, // Changed from seconds to milliseconds
    pub dummy_tx_interval_ms: Option<u64>, // Changed from seconds to milliseconds
    pub dummy_tx_per_cycle: Option<u32>,
    pub mempool_max_age_secs: Option<u64>,
    pub mempool_max_size_bytes: Option<usize>,
    pub mempool_max_size: Option<usize>, // Number of transactions (default: 1000)
    pub mempool_batch_size: Option<usize>,
    pub mempool_backpressure_threshold: Option<u128>,

    // --- TX Generator Backpressure Configuration ---
    pub tx_batch_size: Option<usize>,
    pub adaptive_batch_threshold: Option<u128>,
    pub memory_soft_limit: Option<usize>,
    pub memory_hard_limit: Option<usize>,
    /// Optional developer fee rate applied to coinbase rewards (0.10 = 10%)
    pub dev_fee_rate: Option<u128>,

    // --- File & Database Paths ---
    pub data_dir: String,
    pub db_path: String,
    pub wallet_path: String,
    pub p2p_identity_path: String,
    pub log_file_path: Option<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,

    // --- Logging & P2P Internals ---
    pub logging: LoggingConfig,
    pub p2p: P2pConfig,
    pub rpc: RpcConfig,
    #[serde(default)]
    pub mempool: MempoolSectionConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LoggingConfig {
    pub level: String,
    /// Enable block celebration logging (default: false in production, true in dev)
    #[serde(default)]
    pub enable_block_celebrations: bool,
    /// Log level for celebration messages ("debug" or "info")
    #[serde(default = "default_celebration_log_level")]
    pub celebration_log_level: String,
    /// Optional throttle limit for celebration messages per minute
    pub celebration_throttle_per_min: Option<u32>,
}

fn default_celebration_log_level() -> String {
    "info".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolSectionConfig {
    pub parallel_verification_threads: usize,
    pub capacity: usize,
}

impl Default for MempoolSectionConfig {
    fn default() -> Self {
        Self {
            parallel_verification_threads: num_cpus::get(),
            capacity: 1_000_000,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct P2pConfig {
    pub heartbeat_interval: u64, // in milliseconds
    pub mesh_n_low: usize,
    pub mesh_n: usize,
    pub mesh_n_high: usize,
    pub mesh_outbound_min: usize,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: 2000, // 2 seconds - compatible with 1 second block time
            mesh_n_low: 2,            // Reduced for lower resource usage
            mesh_n: 4,                // Reduced from 8 to 4
            mesh_n_high: 8,           // Reduced from 16 to 8
            mesh_outbound_min: 2,     // Reduced from 4 to 2
        }
    }
}

// --- Testnet Defaults ---
impl Default for Config {
    fn default() -> Self {
        Self {
            p2p_address: "/ip4/0.0.0.0/tcp/8008".to_string(),
            api_address: "127.0.0.1:8080".to_string(),
            peers: vec![],
            local_full_p2p_address: None,
            network_id: "qanto-testnet-phoenix".to_string(),
            genesis_validator: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            contract_address: "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394"
                .to_string(),
            target_block_time: 1000, // Evolved to 1 second for higher throughput
            difficulty: 1000,
            max_amount: 21_000_000_000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: Self::get_optimized_thread_count(),
            mining_enabled: false,
            adaptive_mining_enabled: false,

            // --- Producer Configuration ---
            producer_type: Some("solo".to_string()), // Default to solo producer

            // --- Telemetry Configuration ---
            hash_rate_interval_secs: Some(5),
            enable_detailed_telemetry: Some(false),

            // --- Adaptive Difficulty Configuration ---
            block_target_ms: Some(1000),            // Default to 1 second
            solve_timeout_ms: Some(30000),          // Default to 30 seconds
            difficulty_max_adjust_pct: Some(250_000_000),  // Max 25% adjustment (scaled 1e9)
            difficulty_smoothing_factor: Some(100_000_000), // 10% smoothing (scaled 1e9)
            difficulty_min: Some(1),

            // --- Memory & Profiling Configuration ---
            db_cache_bytes: Some(64 * 1024 * 1024), // 64MB default
            mempool_max_bytes: Some(128 * 1024 * 1024), // 128MB default

            // --- Dummy Transactions Configuration ---
            enable_dummy_tx: Some(false),
            max_dummy_per_block: Some(10),

            // --- Sharding & Scaling ---
            num_chains: 1,
            mining_chain_id: 0,
            mining_interval_ms: None, // Use default values if not specified
            dummy_tx_interval_ms: None,
            dummy_tx_per_cycle: None,
            mempool_max_age_secs: None,
            mempool_max_size_bytes: None,
            mempool_max_size: Some(1000), // Default 1000 transactions
            mempool_batch_size: None,
            mempool_backpressure_threshold: None,

            // --- TX Generator Backpressure Configuration ---
            tx_batch_size: None,
            adaptive_batch_threshold: None,
            memory_soft_limit: None,
            memory_hard_limit: None,
            dev_fee_rate: Some(100_000_000), // 10% dev fee (scaled 1e9)

            // --- File & Database Paths ---
            data_dir: "./data".to_string(),
            db_path: "./data/qanto.db".to_string(),
            wallet_path: "wallet.key".to_string(),
            p2p_identity_path: "p2p_identity.key".to_string(),
            log_file_path: Some("./logs/qanto.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,

            // --- Logging & P2P Internals ---
            logging: LoggingConfig {
                level: "error".to_string(), // Minimal logging for maximum performance
                enable_block_celebrations: false,
                celebration_log_level: "info".to_string(),
                celebration_throttle_per_min: None,
            },
            p2p: P2pConfig::default(),
            rpc: RpcConfig {
                address: "127.0.0.1:50051".to_string(),
            },
            mempool: MempoolSectionConfig::default(),
        }
    }
}

impl Config {
    /// Apply CLI path overrides to the configuration
    #[allow(clippy::too_many_arguments)]
    pub fn apply_cli_overrides(
        &mut self,
        wallet_path: Option<String>,
        p2p_identity_path: Option<String>,
        data_dir: Option<String>,
        db_path: Option<String>,
        log_file_path: Option<String>,
        tls_cert_path: Option<String>,
        tls_key_path: Option<String>,
        adaptive_mining_enabled: Option<bool>,
        rpc_port: Option<u16>,
        p2p_port: Option<u16>,
        listen: Option<String>,
        bootnodes: Option<Vec<String>>,
    ) {
        if let Some(wallet) = wallet_path {
            self.wallet_path = wallet;
        }
        if let Some(p2p_identity) = p2p_identity_path {
            self.p2p_identity_path = p2p_identity;
        }
        if let Some(data) = data_dir {
            self.data_dir = data.clone();
            // Update dependent paths if they use the default data directory
            if self.db_path.starts_with("./data/") {
                self.db_path = format!("{data}/qanto.db");
            }
            if let Some(ref log_path) = self.log_file_path {
                if log_path.starts_with("./logs/") {
                    self.log_file_path = Some(format!("{data}/logs/qanto.log"));
                }
            }
        }
        if let Some(db) = db_path {
            self.db_path = db;
        }
        if let Some(log) = log_file_path {
            self.log_file_path = Some(log);
        }
        if let Some(cert) = tls_cert_path {
            self.tls_cert_path = Some(cert);
        }
        if let Some(key) = tls_key_path {
            self.tls_key_path = Some(key);
        }

        if let Some(adaptive_mining) = adaptive_mining_enabled {
            self.adaptive_mining_enabled = adaptive_mining;
        }

        if let Some(port) = rpc_port {
            if let Ok(mut addr) = self.api_address.parse::<SocketAddr>() {
                addr.set_port(port);
                self.api_address = addr.to_string();
            } else if let Some(pos) = self.api_address.rfind(':') {
                self.api_address = format!("{}:{}", &self.api_address[..pos], port);
            } else {
                self.api_address = format!("0.0.0.0:{}", port);
            }
        }

        if let Some(port) = p2p_port {
            if let Some(tcp_pos) = self.p2p_address.find("/tcp/") {
                let start = tcp_pos + 5;
                let mut end = start;
                while end < self.p2p_address.len() && self.p2p_address.as_bytes()[end].is_ascii_digit() {
                    end += 1;
                }
                self.p2p_address = format!("{}{}{}", &self.p2p_address[..start], port, &self.p2p_address[end..]);
            }
        }

        if let Some(listen_addr) = listen {
            if listen_addr.parse::<std::net::IpAddr>().is_ok() {
                let port = p2p_port.unwrap_or(30303);
                self.p2p_address = format!("/ip4/{}/tcp/{}/ws", listen_addr, port);
            } else {
                self.p2p_address = listen_addr;
            }
        }

        if let Some(nodes) = bootnodes {
            self.peers = nodes;
        }
    }

    /// Get default paths based on a base directory
    pub fn get_default_paths(base_dir: &str) -> (String, String, String, String) {
        let data_dir = format!("{base_dir}/data");
        let db_path = format!("{data_dir}/qanto.db");
        let wallet_path = format!("{base_dir}/wallet.key");
        let p2p_identity_path = format!("{base_dir}/p2p_identity.key");
        (data_dir, db_path, wallet_path, p2p_identity_path)
    }

    /// Create a new config with custom base directory
    pub fn with_base_dir(base_dir: &str) -> Self {
        let (data_dir, db_path, wallet_path, p2p_identity_path) = Self::get_default_paths(base_dir);

        Self {
            data_dir,
            db_path,
            wallet_path,
            p2p_identity_path,
            log_file_path: Some(format!("{base_dir}/logs/qanto.log")),
            ..Default::default()
        }
    }

    /// Get optimized thread count based on system resources and free-tier constraints
    fn get_optimized_thread_count() -> usize {
        let cpu_count = num_cpus::get();
        let available_memory_gb = Self::get_available_memory_gb();

        // For free-tier instances (t2.micro/t3.micro), limit threads aggressively
        if available_memory_gb <= 1.0 {
            // t2.micro/t3.micro: 1 GB RAM - use minimal threads
            1.max(cpu_count / 4)
        } else if available_memory_gb <= 2.0 {
            // Small instances: 2 GB RAM - use conservative threading
            2.max(cpu_count / 2)
        } else if available_memory_gb <= 4.0 {
            // Medium instances: 4 GB RAM - use moderate threading
            4.max(cpu_count * 3 / 4)
        } else {
            // Larger instances: use most cores but leave some for system
            cpu_count.max(1)
        }
    }

    /// Get available system memory in GB
    fn get_available_memory_gb() -> f64 {
        let mut sys = System::new_all();
        sys.refresh_memory();
        sys.available_memory() as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    /// Create a free-tier optimized configuration
    pub fn free_tier_optimized() -> Self {
        Self {
            p2p_address: "/ip4/0.0.0.0/tcp/8001".to_string(),
            api_address: "127.0.0.1:8082".to_string(),
            peers: vec![],
            local_full_p2p_address: None,
            network_id: "qanto-testnet".to_string(),
            genesis_validator: "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"
                .to_string(),
            contract_address: "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394"
                .to_string(),
            target_block_time: 2000, // 2 seconds for free-tier stability
            difficulty: 1,           // Minimal difficulty for free-tier
            max_amount: 21_000_000_000,
            use_gpu: false,                 // No GPU on free-tier
            zk_enabled: false,              // Disable ZK for resource savings
            mining_threads: 1,              // Single thread for free-tier
            mining_enabled: false,          // Disabled by default for free-tier
            adaptive_mining_enabled: false, // Disabled for free-tier to save resources

            // --- Producer Configuration ---
            producer_type: Some("solo".to_string()), // Default to solo producer for free-tier

            // --- Telemetry Configuration ---
            hash_rate_interval_secs: Some(10), // Longer interval for free-tier
            enable_detailed_telemetry: Some(false),

            // --- Adaptive Difficulty Configuration ---
            block_target_ms: Some(2000), // 2 seconds for free-tier stability
            solve_timeout_ms: Some(60000), // 1 minute timeout for free-tier
            difficulty_max_adjust_pct: Some(100_000_000), // Conservative 10% adjustment (scaled 1e9)
            difficulty_smoothing_factor: Some(200_000_000), // More smoothing (20%) for stability (scaled 1e9)
            difficulty_min: Some(1),

            // --- Memory & Profiling Configuration ---
            db_cache_bytes: Some(16 * 1024 * 1024), // 16MB for free-tier
            mempool_max_bytes: Some(32 * 1024 * 1024), // 32MB for free-tier

            // --- Dummy Transactions Configuration ---
            enable_dummy_tx: Some(false), // Disabled for free-tier
            max_dummy_per_block: Some(5), // Limited for free-tier

            num_chains: 1, // Single chain for simplicity
            mining_chain_id: 0,
            mining_interval_ms: Some(60000), // Conservative 1-minute intervals for free-tier (converted to ms)
            dummy_tx_interval_ms: Some(30000), // Conservative dummy TX generation (converted to ms)
            dummy_tx_per_cycle: Some(100),   // Limited dummy TX for free-tier
            mempool_max_age_secs: Some(300), // 5-minute mempool age for free-tier
            mempool_max_size_bytes: Some(1024 * 1024), // 1MB mempool for free-tier
            mempool_max_size: Some(500),     // 500 transactions for free-tier
            mempool_batch_size: Some(100),   // Conservative batch size for free-tier
            mempool_backpressure_threshold: Some(800_000_000), // 80% threshold for free-tier

            // --- TX Generator Backpressure Configuration ---
            tx_batch_size: Some(50), // Conservative TX batch size for free-tier
            adaptive_batch_threshold: Some(700_000_000), // 70% memory threshold for free-tier
            memory_soft_limit: Some(SOFT_MEMORY_LIMIT), // 8MB soft limit
            memory_hard_limit: Some(HARD_MEMORY_LIMIT), // 10MB hard limit
            dev_fee_rate: Some(100_000_000), // 10% dev fee (scaled 1e9)

            // --- File & Database Paths ---
            data_dir: "./data".to_string(),
            db_path: "./data/qanto_free.db".to_string(),
            wallet_path: "wallet.key".to_string(),
            p2p_identity_path: "p2p_identity.key".to_string(),
            log_file_path: Some("./logs/qanto_free.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,

            // --- Logging & P2P Internals ---
            logging: LoggingConfig {
                level: "warn".to_string(), // Reduced logging for performance
                enable_block_celebrations: true,
                celebration_log_level: "info".to_string(),
                celebration_throttle_per_min: Some(10),
            },
            p2p: P2pConfig {
                heartbeat_interval: 15000, // Longer intervals for free-tier
                mesh_n_low: 1,
                mesh_n: 2,
                mesh_n_high: 4,
                mesh_outbound_min: 1,
            },
            rpc: RpcConfig {
                address: "127.0.0.1:50051".to_string(),
            },
            mempool: MempoolSectionConfig::default(),
        }
    }

    /// High-performance configuration optimized for 32 BPS and 10M TPS
    pub fn high_performance() -> Self {
        let available_memory_gb = Self::get_available_memory_gb();
        let thread_count = Self::get_optimized_thread_count();

        // Calculate memory limits based on available system memory
        let memory_soft_limit = ((available_memory_gb * 0.4) * 1024.0 * 1024.0 * 1024.0) as usize; // 40% of available memory
        let memory_hard_limit = ((available_memory_gb * 0.6) * 1024.0 * 1024.0 * 1024.0) as usize; // 60% of available memory
        let mempool_size = ((available_memory_gb * 0.3) * 1024.0 * 1024.0 * 1024.0) as usize; // 30% for mempool

        Self {
            p2p_address: "/ip4/0.0.0.0/tcp/8008".to_string(),
            api_address: "127.0.0.1:8080".to_string(),
            peers: vec![],
            local_full_p2p_address: None,
            network_id: "qanto-mainnet-performance".to_string(),
            genesis_validator: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            contract_address: "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394"
                .to_string(),
            target_block_time: 31, // 31ms for 32+ BPS (slightly above minimum for stability)
            difficulty: 1000,
            max_amount: 21_000_000_000,
            use_gpu: true,                  // Enable GPU for high performance
            zk_enabled: true,               // Enable ZK for security
            mining_threads: thread_count,   // Use all available threads
            mining_enabled: true,           // Enable mining for high performance
            adaptive_mining_enabled: false, // Disabled by default, can be enabled via CLI

            // --- Producer Configuration ---
            producer_type: Some("decoupled".to_string()), // Use decoupled producer for high performance

            // --- Telemetry Configuration ---
            hash_rate_interval_secs: Some(1), // Fast telemetry for high performance
            enable_detailed_telemetry: Some(true),

            // --- Adaptive Difficulty Configuration ---
            block_target_ms: Some(31),               // 31ms for 32+ BPS
            solve_timeout_ms: Some(10000),           // 10 second timeout for high performance
            difficulty_max_adjust_pct: Some(500_000_000),   // Aggressive 50% adjustment (scaled 1e9)
            difficulty_smoothing_factor: Some(50_000_000), // Minimal smoothing (5%) for responsiveness (scaled 1e9)
            difficulty_min: Some(1000),

            // --- Memory & Profiling Configuration ---
            db_cache_bytes: Some(512 * 1024 * 1024), // 512MB for high performance
            mempool_max_bytes: Some(mempool_size),   // Dynamic based on available memory

            // --- Dummy Transactions Configuration ---
            enable_dummy_tx: Some(true),    // Enable for stress testing
            max_dummy_per_block: Some(100), // High volume for testing

            num_chains: 4, // Multiple chains for parallel processing
            mining_chain_id: 0,
            mining_interval_ms: Some(1000), // Fast mining intervals (converted to ms)
            dummy_tx_interval_ms: Some(1000), // Fast dummy TX generation for testing (converted to ms)
            dummy_tx_per_cycle: Some(100000), // High dummy TX volume for stress testing
            mempool_max_age_secs: Some(60),   // 1-minute mempool age for high throughput
            mempool_max_size_bytes: Some(mempool_size), // Dynamic mempool size based on available memory
            mempool_max_size: Some(10000),              // 10,000 transactions for high performance
            mempool_batch_size: Some(500000), // Large batch size for 10M TPS (312,500 tx/block * 1.6 buffer)
            mempool_backpressure_threshold: Some(750_000_000), // 75% threshold for high performance

            // --- TX Generator Backpressure Configuration ---
            tx_batch_size: Some(100000), // Large TX batch size for high throughput
            adaptive_batch_threshold: Some(800_000_000), // 80% memory threshold for adaptive batching
            memory_soft_limit: Some(memory_soft_limit),
            memory_hard_limit: Some(memory_hard_limit),
            dev_fee_rate: Some(100_000_000), // 10% dev fee (scaled 1e9)

            // --- File & Database Paths ---
            data_dir: "./data".to_string(),
            db_path: "./data/qanto_performance.db".to_string(),
            wallet_path: "wallet.key".to_string(),
            p2p_identity_path: "p2p_identity.key".to_string(),
            log_file_path: Some("./logs/qanto_performance.log".to_string()),
            tls_cert_path: None,
            tls_key_path: None,

            // --- Logging & P2P Internals ---
            logging: LoggingConfig {
                level: "error".to_string(), // Minimal logging for maximum performance
                enable_block_celebrations: false,
                celebration_log_level: "error".to_string(),
                celebration_throttle_per_min: Some(1),
            },
            p2p: P2pConfig {
                heartbeat_interval: 100, // Very fast heartbeat for rapid consensus (100ms)
                mesh_n_low: 8,
                mesh_n: 16,
                mesh_n_high: 32,
                mesh_outbound_min: 8,
            },
            rpc: RpcConfig {
                address: "127.0.0.1:50051".to_string(),
            },
            mempool: MempoolSectionConfig::default(),
        }
    }

    pub fn load(path: &str) -> Result<Self, ConfigError> {
        if !Path::new(path).exists() {
            let default_config = Config::default();
            default_config
                .save(path)
                .context("Failed to create a default configuration file.")
                .map_err(|source| ConfigError::Save {
                    path: path.to_string(),
                    source,
                })?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)
            .context("Failed to read configuration file.")
            .map_err(|source| ConfigError::Load {
                path: path.to_string(),
                source,
            })?;
        let config: Config = toml::from_str(&content)
            .context("Failed to parse TOML from configuration file.")
            .map_err(|source| ConfigError::Load {
                path: path.to_string(),
                source,
            })?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<(), ConfigError> {
        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize configuration to TOML.")
            .map_err(|source| ConfigError::Save {
                path: path.to_string(),
                source,
            })?;
        fs::write(path, toml_string)
            .context("Failed to write configuration to file.")
            .map_err(|source| ConfigError::Save {
                path: path.to_string(),
                source,
            })?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.api_address.parse::<SocketAddr>().map_err(|_| {
            let mut error_msg = String::with_capacity(32 + self.api_address.len());
            error_msg.push_str("Invalid API address format: '");
            error_msg.push_str(&self.api_address);
            error_msg.push('\'');
            ConfigError::Validation(error_msg)
        })?;

        self.rpc.address.parse::<SocketAddr>().map_err(|_| {
            let mut error_msg = String::with_capacity(33 + self.rpc.address.len());
            error_msg.push_str("Invalid RPC address format: '");
            error_msg.push_str(&self.rpc.address);
            error_msg.push('\'');
            ConfigError::Validation(error_msg)
        })?;

        if !(MIN_TARGET_BLOCK_TIME..=MAX_TARGET_BLOCK_TIME).contains(&self.target_block_time) {
            let mut error_msg = String::with_capacity(64);
            error_msg.push_str("target_block_time (in ms) must be between ");
            error_msg.push_str(&MIN_TARGET_BLOCK_TIME.to_string());
            error_msg.push_str(" and ");
            error_msg.push_str(&MAX_TARGET_BLOCK_TIME.to_string());
            return Err(ConfigError::Validation(error_msg));
        }

        if self.peers.len() > MAX_PEERS {
            let mut error_msg = String::with_capacity(48);
            error_msg.push_str("Number of peers cannot exceed ");
            error_msg.push_str(&MAX_PEERS.to_string());
            return Err(ConfigError::Validation(error_msg));
        }

        if !(MIN_DIFFICULTY..=MAX_DIFFICULTY).contains(&self.difficulty) {
            let mut error_msg = String::with_capacity(64);
            error_msg.push_str("Difficulty must be between ");
            error_msg.push_str(&MIN_DIFFICULTY.to_string());
            error_msg.push_str(" and ");
            error_msg.push_str(&MAX_DIFFICULTY.to_string());
            return Err(ConfigError::Validation(error_msg));
        }

        if self.mining_threads == 0 || self.mining_threads > MAX_MINING_THREADS {
            let mut error_msg = String::with_capacity(48);
            error_msg.push_str("mining_threads must be between 1 and ");
            error_msg.push_str(&MAX_MINING_THREADS.to_string());
            return Err(ConfigError::Validation(error_msg));
        }

        if !(MIN_CHAINS..=MAX_CHAINS).contains(&self.num_chains) {
            let mut error_msg = String::with_capacity(48);
            error_msg.push_str("num_chains must be between ");
            error_msg.push_str(&MIN_CHAINS.to_string());
            error_msg.push_str(" and ");
            error_msg.push_str(&MAX_CHAINS.to_string());
            return Err(ConfigError::Validation(error_msg));
        }

        if self.mining_chain_id >= self.num_chains {
            return Err(ConfigError::Validation(
                "mining_chain_id must be less than num_chains".to_string(),
            ));
        }

        if self.genesis_validator.len() != 64 {
            return Err(ConfigError::Validation(
                "Invalid genesis_validator address format".to_string(),
            ));
        }
        hex::decode(&self.genesis_validator).unwrap_or_else(|_| {
            panic!("Invalid hex in address");
        });

        if self.contract_address.len() != 64 {
            return Err(ConfigError::Validation(
                "Invalid contract_address format".to_string(),
            ));
        }
        hex::decode(&self.contract_address).unwrap_or_else(|_| {
            panic!("Invalid hex in address");
        });

        // Validate mining configuration parameters (MICROSECOND PRECISION)
        if let Some(mining_interval) = self.mining_interval_ms {
            if !(MIN_MINING_INTERVAL_MS..=MAX_MINING_INTERVAL_MS).contains(&mining_interval) {
                return Err(ConfigError::Validation(format!(
                    "mining_interval_ms must be between {MIN_MINING_INTERVAL_MS} and {MAX_MINING_INTERVAL_MS} milliseconds"
                )));
            }
        }

        if let Some(dummy_tx_interval) = self.dummy_tx_interval_ms {
            if !(MIN_DUMMY_TX_INTERVAL_MS..=MAX_DUMMY_TX_INTERVAL_MS).contains(&dummy_tx_interval) {
                return Err(ConfigError::Validation(format!(
                    "dummy_tx_interval_ms must be between {MIN_DUMMY_TX_INTERVAL_MS} and {MAX_DUMMY_TX_INTERVAL_MS} milliseconds"
                )));
            }
        }

        if let Some(dummy_tx_per_cycle) = self.dummy_tx_per_cycle {
            if !(MIN_DUMMY_TX_PER_CYCLE..=MAX_DUMMY_TX_PER_CYCLE).contains(&dummy_tx_per_cycle) {
                return Err(ConfigError::Validation(format!(
                    "dummy_tx_per_cycle must be between {MIN_DUMMY_TX_PER_CYCLE} and {MAX_DUMMY_TX_PER_CYCLE}"
                )));
            }
        }

        if let Some(mempool_max_age) = self.mempool_max_age_secs {
            if !(MIN_MEMPOOL_MAX_AGE..=MAX_MEMPOOL_MAX_AGE).contains(&mempool_max_age) {
                return Err(ConfigError::Validation(format!(
                    "mempool_max_age_secs must be between {MIN_MEMPOOL_MAX_AGE} and {MAX_MEMPOOL_MAX_AGE} seconds"
                )));
            }
        }

        if let Some(mempool_max_size) = self.mempool_max_size_bytes {
            if !(MIN_MEMPOOL_MAX_SIZE..=MAX_MEMPOOL_MAX_SIZE).contains(&mempool_max_size) {
                return Err(ConfigError::Validation(format!(
                    "mempool_max_size_bytes must be between {MIN_MEMPOOL_MAX_SIZE} and {MAX_MEMPOOL_MAX_SIZE} bytes"
                )));
            }
        }

        if let Some(mempool_batch_size) = self.mempool_batch_size {
            if !(MIN_MEMPOOL_BATCH_SIZE..=MAX_MEMPOOL_BATCH_SIZE).contains(&mempool_batch_size) {
                return Err(ConfigError::Validation(format!(
                    "mempool_batch_size must be between {MIN_MEMPOOL_BATCH_SIZE} and {MAX_MEMPOOL_BATCH_SIZE}"
                )));
            }
        }

        if let Some(mempool_backpressure_threshold) = self.mempool_backpressure_threshold {
            if !(MIN_MEMPOOL_BACKPRESSURE_THRESHOLD..=MAX_MEMPOOL_BACKPRESSURE_THRESHOLD)
                .contains(&mempool_backpressure_threshold)
            {
                return Err(ConfigError::Validation(format!(
                    "mempool_backpressure_threshold must be between {MIN_MEMPOOL_BACKPRESSURE_THRESHOLD} and {MAX_MEMPOOL_BACKPRESSURE_THRESHOLD}"
                )));
            }
        }

        // Validate TX Generator Backpressure Configuration
        if let Some(tx_batch_size) = self.tx_batch_size {
            if !(MIN_TX_BATCH_SIZE..=MAX_TX_BATCH_SIZE).contains(&tx_batch_size) {
                return Err(ConfigError::Validation(format!(
                    "tx_batch_size must be between {MIN_TX_BATCH_SIZE} and {MAX_TX_BATCH_SIZE}"
                )));
            }
        }

        // Validate consistency between target_block_time and derived values
        self.validate_target_block_time_consistency()?;

        if let Some(adaptive_batch_threshold) = self.adaptive_batch_threshold {
            if !(MIN_ADAPTIVE_BATCH_THRESHOLD..=MAX_ADAPTIVE_BATCH_THRESHOLD)
                .contains(&adaptive_batch_threshold)
            {
                return Err(ConfigError::Validation(format!(
                    "adaptive_batch_threshold must be between {MIN_ADAPTIVE_BATCH_THRESHOLD} and {MAX_ADAPTIVE_BATCH_THRESHOLD}"
                )));
            }
        }

        if let Some(dev_fee_rate) = self.dev_fee_rate {
            if dev_fee_rate > crate::QANTO_SCALE {
                return Err(ConfigError::Validation(
                    "dev_fee_rate must be between 0 and 1.0 (scaled)".to_string(),
                ));
            }
        }

        if let Some(memory_soft_limit) = self.memory_soft_limit {
            if memory_soft_limit == 0 {
                return Err(ConfigError::Validation(
                    "memory_soft_limit must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(memory_hard_limit) = self.memory_hard_limit {
            if memory_hard_limit == 0 {
                return Err(ConfigError::Validation(
                    "memory_hard_limit must be greater than 0".to_string(),
                ));
            }
        }

        // Validate that hard limit is greater than soft limit
        if let (Some(soft), Some(hard)) = (self.memory_soft_limit, self.memory_hard_limit) {
            if hard <= soft {
                return Err(ConfigError::Validation(
                    "memory_hard_limit must be greater than memory_soft_limit".to_string(),
                ));
            }
        }

        // Validate file and database paths
        if self.data_dir.is_empty() {
            return Err(ConfigError::Validation(
                "data_dir cannot be empty".to_string(),
            ));
        }

        if self.db_path.is_empty() {
            return Err(ConfigError::Validation(
                "db_path cannot be empty".to_string(),
            ));
        }

        if self.wallet_path.is_empty() {
            return Err(ConfigError::Validation(
                "wallet_path cannot be empty".to_string(),
            ));
        }

        if self.p2p_identity_path.is_empty() {
            return Err(ConfigError::Validation(
                "p2p_identity_path cannot be empty".to_string(),
            ));
        }

        // Validate that paths don't contain dangerous characters
        let dangerous_chars = ['<', '>', '|', '&', ';', '`', '$'];
        for path in [
            &self.data_dir,
            &self.db_path,
            &self.wallet_path,
            &self.p2p_identity_path,
        ] {
            if path.chars().any(|c| dangerous_chars.contains(&c)) {
                return Err(ConfigError::Validation(format!(
                    "Path '{path}' contains dangerous characters"
                )));
            }
        }

        if let Some(log_path) = &self.log_file_path {
            if log_path.chars().any(|c| dangerous_chars.contains(&c)) {
                return Err(ConfigError::Validation(format!(
                    "Log file path '{log_path}' contains dangerous characters"
                )));
            }
        }

        Ok(())
    }

    /// Validate consistency between target_block_time and derived values
    fn validate_target_block_time_consistency(&self) -> Result<(), ConfigError> {
        // Validate mining_interval_ms consistency
        if let Some(mining_interval) = self.mining_interval_ms {
            // Mining interval should be at least 10% of target block time
            if mining_interval < self.target_block_time / 10 {
                return Err(ConfigError::Validation(format!(
                    "mining_interval_ms ({}) is too small relative to target_block_time ({}ms). \
                     Mining interval should be at least 10% of target block time ({}ms)",
                    mining_interval,
                    self.target_block_time,
                    self.target_block_time / 10
                )));
            }

            // Mining interval should be at most 10x target block time
            if mining_interval > self.target_block_time * 10 {
                return Err(ConfigError::Validation(format!(
                    "mining_interval_ms ({}) is too large relative to target_block_time ({}ms). \
                     Mining interval should be at most 10x target block time ({}ms)",
                    mining_interval,
                    self.target_block_time,
                    self.target_block_time * 10
                )));
            }
        }

        // Validate dummy_tx_interval_ms consistency
        if let Some(dummy_tx_interval) = self.dummy_tx_interval_ms {
            // Dummy TX interval should be at most 5x target block time
            if dummy_tx_interval > self.target_block_time * 5 {
                return Err(ConfigError::Validation(format!(
                    "dummy_tx_interval_ms ({}) is too large relative to target_block_time ({}ms). \
                     Dummy TX interval should be at most 5x target block time ({}ms)",
                    dummy_tx_interval,
                    self.target_block_time,
                    self.target_block_time * 5
                )));
            }
        }

        // Validate mempool_max_age_secs consistency
        if let Some(mempool_max_age_secs) = self.mempool_max_age_secs {
            let mempool_max_age_ms = mempool_max_age_secs * 1000;
            // Should be at least 10x block time
            if mempool_max_age_ms < self.target_block_time * 10 {
                return Err(ConfigError::Validation(format!(
                    "mempool_max_age_secs ({}s) is too small relative to target_block_time ({}ms). \
                     Mempool max age should be at least 10x target block time ({}ms)",
                    mempool_max_age_secs,
                    self.target_block_time,
                    self.target_block_time * 10
                )));
            }
        }

        // Validate P2P heartbeat interval consistency
        // Heartbeat should be at most 2x target block time
        if self.p2p.heartbeat_interval as u64 > self.target_block_time * 2 {
            return Err(ConfigError::Validation(format!(
                "p2p.heartbeat_interval ({}ms) is too large relative to target_block_time ({}ms). \
                 Heartbeat interval should be at most 2x target block time ({}ms)",
                self.p2p.heartbeat_interval,
                self.target_block_time,
                self.target_block_time * 2
            )));
        }

        Ok(())
    }
}
