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
use thiserror::Error;
use tracing::instrument;

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

    // --- Consensus & DAG Configuration ---
    pub genesis_validator: String,
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
            heartbeat_interval: 5000,
            mesh_n_low: 4,
            mesh_n: 8,
            mesh_n_high: 16,
            mesh_outbound_min: 4,
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
            target_block_time: 1000, // Evolved to 1 second for higher throughput
            difficulty: 1000,
            max_amount: 100_000_000_000,
            use_gpu: false,
            zk_enabled: false,
            mining_threads: num_cpus::get().max(1),
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
    #[instrument]
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

    #[instrument(skip(self))]
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
            ConfigError::Validation(format!(
                "Invalid API address format: '{}'",
                self.api_address
            ))
        })?;

        if !(MIN_TARGET_BLOCK_TIME..=MAX_TARGET_BLOCK_TIME).contains(&self.target_block_time) {
            return Err(ConfigError::Validation(format!(
                "target_block_time (in ms) must be between {MIN_TARGET_BLOCK_TIME} and {MAX_TARGET_BLOCK_TIME}"
            )));
        }

        if self.peers.len() > MAX_PEERS {
            return Err(ConfigError::Validation(format!(
                "Number of peers cannot exceed {MAX_PEERS}"
            )));
        }

        if !(MIN_DIFFICULTY..=MAX_DIFFICULTY).contains(&self.difficulty) {
            return Err(ConfigError::Validation(format!(
                "Difficulty must be between {MIN_DIFFICULTY} and {MAX_DIFFICULTY}"
            )));
        }

        if self.mining_threads == 0 || self.mining_threads > MAX_MINING_THREADS {
            return Err(ConfigError::Validation(format!(
                "mining_threads must be between 1 and {MAX_MINING_THREADS}"
            )));
        }

        if !(MIN_CHAINS..=MAX_CHAINS).contains(&self.num_chains) {
            return Err(ConfigError::Validation(format!(
                "num_chains must be between {MIN_CHAINS} and {MAX_CHAINS}"
            )));
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
        let mut config = Config::default();

        config.target_block_time = 5;
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
