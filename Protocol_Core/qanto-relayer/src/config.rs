//! Configuration loading and validation.

use std::env;
use std::time::Duration;

use k256::ecdsa::SigningKey;

use crate::error::{ConfigError, RelayerError};

#[derive(Debug, Clone)]
pub struct RelayerConfig {
    pub eth_rpc_url: String,
    pub qanto_rpc_url: String,
    pub bridge_address: String,
    pub source_chain: String,
    pub relayer_signing_key: SigningKey,
    pub poll_interval: Duration,
    pub confirmations: u64,
    pub start_block: Option<u64>,
    pub state_file: String,
    pub state_retention: Duration,
    pub retry_backoff: Duration,
    pub max_attempts: u32,
    pub max_retries_per_poll: usize,
}

impl RelayerConfig {
    pub fn from_env() -> Result<Self, RelayerError> {
        let eth_rpc_url = env::var("QANTO_RELAYER_ETH_RPC_URL")
            .or_else(|_| env::var("QANTO_RELAYER_ETH_WS_URL"))
            .map_err(|_| ConfigError::MissingEnv("QANTO_RELAYER_ETH_RPC_URL".into()))?;

        let qanto_rpc_url = env::var("QANTO_RELAYER_QANTO_RPC_URL")
            .map_err(|_| ConfigError::MissingEnv("QANTO_RELAYER_QANTO_RPC_URL".into()))?;

        let bridge_address =
            normalize_address(&env::var("QANTO_RELAYER_ETH_BRIDGE_ADDRESS").map_err(|_| {
                ConfigError::MissingEnv("QANTO_RELAYER_ETH_BRIDGE_ADDRESS".into())
            })?)?;

        let source_chain = env::var("QANTO_RELAYER_SOURCE_CHAIN")
            .map_err(|_| ConfigError::MissingEnv("QANTO_RELAYER_SOURCE_CHAIN".into()))?;

        let relayer_signing_key = parse_signing_key(
            &env::var("QANTO_RELAYER_PRIVATE_KEY")
                .map_err(|_| ConfigError::MissingEnv("QANTO_RELAYER_PRIVATE_KEY".into()))?,
        )?;

        let poll_interval_secs = env::var("QANTO_RELAYER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);

        let confirmations = env::var("QANTO_RELAYER_CONFIRMATIONS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1);

        let start_block = env::var("QANTO_RELAYER_START_BLOCK")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());

        let state_file = env::var("QANTO_RELAYER_STATE_FILE")
            .unwrap_or_else(|_| "qanto_relayer_state.json".to_string());

        let state_retention_secs = env::var("QANTO_RELAYER_STATE_RETENTION_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(86_400);

        let retry_backoff_secs = env::var("QANTO_RELAYER_RETRY_BACKOFF_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);

        let max_attempts = env::var("QANTO_RELAYER_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(50);

        let max_retries_per_poll = env::var("QANTO_RELAYER_MAX_RETRIES_PER_POLL")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100);

        Ok(Self {
            eth_rpc_url,
            qanto_rpc_url,
            bridge_address,
            source_chain,
            relayer_signing_key,
            poll_interval: Duration::from_secs(poll_interval_secs),
            confirmations,
            start_block,
            state_file,
            state_retention: Duration::from_secs(state_retention_secs),
            retry_backoff: Duration::from_secs(retry_backoff_secs),
            max_attempts,
            max_retries_per_poll,
        })
    }
}

fn strip_hex_prefix(value: &str) -> &str {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
}

#[allow(dead_code)]
fn normalize_hex(value: &str) -> Result<String, ConfigError> {
    let stripped = strip_hex_prefix(value);
    if stripped.is_empty()
        || !stripped.len().is_multiple_of(2)
        || !stripped.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return Err(ConfigError::InvalidAddress(value.to_string()));
    }
    Ok(format!("0x{}", stripped.to_lowercase()))
}

fn normalize_address(value: &str) -> Result<String, ConfigError> {
    let stripped = strip_hex_prefix(value);
    if stripped.len() != 40 || !stripped.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ConfigError::InvalidAddress(value.to_string()));
    }
    Ok(format!("0x{}", stripped.to_lowercase()))
}

fn parse_signing_key(value: &str) -> Result<SigningKey, ConfigError> {
    let bytes = hex::decode(strip_hex_prefix(value))
        .map_err(|e| ConfigError::InvalidPrivateKey(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(ConfigError::InvalidPrivateKey(format!(
            "relayer private key must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    SigningKey::from_slice(&bytes).map_err(|e| ConfigError::InvalidPrivateKey(e.to_string()))
}

#[allow(dead_code)]
pub fn signing_key_address(signing_key: &SigningKey) -> String {
    use k256::ecdsa::VerifyingKey;
    use sha3::{Digest, Keccak256};

    let verifying_key = VerifyingKey::from(signing_key);
    let binding = verifying_key.to_encoded_point(false);
    let pub_bytes = binding.as_bytes();
    let mut hasher = Keccak256::new();
    hasher.update(&pub_bytes[1..]);
    let digest = hasher.finalize();
    format!("0x{}", hex::encode(&digest[12..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_address() {
        assert_eq!(
            normalize_address("0x1234567890123456789012345678901234567890").unwrap(),
            "0x1234567890123456789012345678901234567890"
        );
        assert_eq!(
            normalize_address("1234567890123456789012345678901234567890").unwrap(),
            "0x1234567890123456789012345678901234567890"
        );
        assert!(normalize_address("0x123").is_err());
    }
}
