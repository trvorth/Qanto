//! Shared types for port interfaces.

use serde::{Deserialize, Serialize};

/// Lightweight representation of an AssetLocked event from the bridge contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetLockedEvent {
    pub user: String,
    pub token: String,
    pub amount: u128,
    pub transaction_hash: String,
    pub block_hash: String,
    pub block_number: u64,
    pub log_index: u64,
}

impl AssetLockedEvent {
    /// Unique key for this event: "tx_hash:log_index"
    pub fn event_key(&self) -> String {
        format!("{}:{}", self.transaction_hash, self.log_index)
    }
}

/// Block header for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub hash: String,
    pub number: u64,
}

/// Transaction receipt with logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub status: String,
    pub logs: Vec<EthLog>,
}

/// Ethereum log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_hash: Option<String>,
    pub block_number: Option<String>,
    pub transaction_hash: Option<String>,
    pub log_index: Option<String>,
    #[serde(default)]
    pub removed: bool,
}
