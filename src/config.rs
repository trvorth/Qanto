// src/config.rs

//! --- Qanto Node Configuration ---
//! v2.0.0 - Production & Standalone Ready
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
// EVOLVED: Time is now in MILLISECONDS to support high BPS.
const MIN_TARGET_BLOCK_TIME: u64 = 30; // Minimum 30ms block time (~33 BPS)
const MAX_TARGET_BLOCK_TIME: u64 = 15000; // Maximum 15 seconds
const MAX_PEERS: usize = 128;
const MIN_DIFFICULTY: u64 = 1;
const MAX_DIFFICULTY: u64 = u64::MAX / 2; // More realistic max
const MAX_MINING_THREADS: usize = 256;
const MIN_CHAINS: u32 = 1;
const MAX_CHAINS: u32 = 32;

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
    pub use_gpu: bool,
    pub zk_enabled: bool,
    pub mining_threads: usize,

    // --- Sharding & Scaling ---
    pub num_chains: u32,
    pub mining_chain_id: u32,

    // --- Logging & P2P Internals ---
    pub logging: LoggingConfig,
    pub p2p: P2pConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
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
            heartbeat_interval: 10000, // Increased to reduce CPU usage
            mesh_n_low: 2,             // Reduced for lower resource usage
            mesh_n: 4,                 // Reduced from 8 to 4
            mesh_n_high: 8,            // Reduced from 16 to 8
            mesh_outbound_min: 2,      // Reduced from 4 to 2
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
            num_chains: 1,
            mining_chain_id: 0,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            p2p: P2pConfig::default(),
        }
    }
}

impl Config {
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
            network_id: "qanto-freetier-mainnet".to_string(),
            genesis_validator: "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"
                .to_string(),
            contract_address: "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394"
                .to_string(),
            target_block_time: 2000, // 2 seconds for free-tier stability
            difficulty: 1,           // Minimal difficulty for free-tier
            max_amount: 21_000_000_000,
            use_gpu: false,    // No GPU on free-tier
            zk_enabled: false, // Disable ZK for resource savings
            mining_threads: 1, // Single thread for free-tier
            num_chains: 1,     // Single chain for simplicity
            mining_chain_id: 0,
            logging: LoggingConfig {
                level: "warn".to_string(), // Reduced logging for performance
            },
            p2p: P2pConfig {
                heartbeat_interval: 15000, // Longer intervals for free-tier
                mesh_n_low: 1,
                mesh_n: 2,
                mesh_n_high: 4,
                mesh_outbound_min: 1,
            },
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

        if self.genesis_validator.len() != 64 || hex::decode(&self.genesis_validator).is_err() {
            return Err(ConfigError::Validation(
                "Invalid genesis_validator address format".to_string(),
            ));
        }

        if self.contract_address.len() != 64 || hex::decode(&self.contract_address).is_err() {
            return Err(ConfigError::Validation(
                "Invalid contract_address format".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_default_and_save_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let default_config = Config::default();
        default_config.save(path).unwrap();

        let loaded_config = Config::load(path).unwrap();
        assert_eq!(loaded_config.api_address, default_config.api_address);
        assert_eq!(loaded_config.difficulty, default_config.difficulty);
    }

    #[test]
    fn test_config_validation() {
        // FIX: Initialize with the desired non-default value directly.
        let mut config = Config {
            target_block_time: 5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        config.target_block_time = 60;

        config.difficulty = 0;
        assert!(config.validate().is_err());
        config.difficulty = 100;

        config.mining_threads = 0;
        assert!(config.validate().is_err());
        config.mining_threads = 4;

        config.num_chains = 0;
        assert!(config.validate().is_err());
        config.num_chains = 2;

        config.mining_chain_id = 2;
        assert!(config.validate().is_err());
        config.mining_chain_id = 1;

        assert!(config.validate().is_ok());
    }
}
