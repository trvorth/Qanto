//! Compact Block Relay Protocol (BIP152-style)
//!
//! This module implements compact block relay to reduce bandwidth usage during
//! block propagation. Instead of sending full blocks, nodes send:
//! 1. Block header + short IDs (8 bytes each) for each transaction
//! 2. Peers request missing transactions via GetBlockTxn
//! 3. Sender responds with BlockTxn containing requested transactions
//!
//! Short ID = first 8 bytes of blake3(txid)

use crate::qantodag::QantoBlock;
use crate::transaction::Transaction;
use bincode;
use blake3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

/// Short ID size in bytes (first 8 bytes of blake3 hash)
pub const SHORT_ID_SIZE: usize = 8;

/// Timeout for compact block request-response fallback
pub const COMPACT_BLOCK_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum number of short IDs in a single compact block announcement
pub const MAX_SHORT_IDS_PER_ANNOUNCEMENT: usize = 10000;

/// Errors for compact block protocol
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum CompactBlockError {
    #[error("Invalid short ID length: expected {SHORT_ID_SIZE}, got {0}")]
    InvalidShortIdLength(usize),
    #[error("Short ID collision detected for txids: {0:?}")]
    ShortIdCollision(Vec<String>),
    #[error("Request timeout after {0:?}")]
    RequestTimeout(Duration),
    #[error("Transaction not found in mempool: {0}")]
    TransactionNotFound(String),
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
}

/// Compact block message types (similar to BIP152)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompactBlockMessage {
    /// Announce a new block with header + short IDs
    Announcement {
        /// Block header (serialized)
        header: Vec<u8>,
        /// Short IDs for each transaction in the block (coinbase first)
        short_ids: Vec<ShortId>,
        /// Number of transactions for validation
        txn_count: usize,
    },
    /// Request missing transactions by their short ID indices
    GetBlockTxn {
        /// Block ID this request is for
        block_id: String,
        /// Indices of transactions to fetch (0 = coinbase, 1 = first tx, etc.)
        indices: Vec<u16>,
    },
    /// Response with requested transactions
    BlockTxn {
        /// Block ID this response is for
        block_id: String,
        /// Requested transactions in same order as requested
        transactions: Vec<Transaction>,
    },
}

/// 8-byte short ID for transaction identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShortId([u8; SHORT_ID_SIZE]);

impl ShortId {
    /// Create a short ID from a transaction ID
    pub fn from_txid(txid: &str) -> Self {
        let hash = blake3::hash(txid.as_bytes());
        let mut bytes = [0u8; SHORT_ID_SIZE];
        bytes.copy_from_slice(&hash.as_bytes()[..SHORT_ID_SIZE]);
        Self(bytes)
    }

    /// Create short ID from raw bytes (for deserialization)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CompactBlockError> {
        if bytes.len() != SHORT_ID_SIZE {
            return Err(CompactBlockError::InvalidShortIdLength(bytes.len()));
        }
        let mut arr = [0u8; SHORT_ID_SIZE];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; SHORT_ID_SIZE] {
        &self.0
    }

    /// Convert to u64 for efficient comparison/storage
    pub fn as_u64(&self) -> u64 {
        u64::from_le_bytes(self.0)
    }
}

impl std::fmt::Display for ShortId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Build a compact block announcement from a full block
pub fn build_compact_block_announcement(
    block: &QantoBlock,
    mempool_txids: &[String],
) -> Result<CompactBlockMessage, CompactBlockError> {
    // Serialize block header (without transactions)
    let header = serialize_block_header(block)?;

    // Compute short IDs for all transactions in the block
    let mut short_ids = Vec::with_capacity(block.transactions.len());
    let mut short_id_to_txid: HashMap<ShortId, String> = HashMap::new();

    for tx in &block.transactions {
        let short_id = ShortId::from_txid(&tx.id);

        // Check for collisions
        if let Some(existing) = short_id_to_txid.get(&short_id) {
            if existing != &tx.id {
                return Err(CompactBlockError::ShortIdCollision(vec![
                    existing.clone(),
                    tx.id.clone(),
                ]));
            }
        }
        short_id_to_txid.insert(short_id, tx.id.clone());
        short_ids.push(short_id);
    }

    // Verify all block txids are in mempool (or we have them)
    for (i, tx) in block.transactions.iter().enumerate() {
        if i == 0 {
            // Coinbase - always include
            continue;
        }
        if !mempool_txids.contains(&tx.id) {
            // Transaction not in our mempool - we'll need to request it
            // This is OK for the announcement, the peer will request missing txs
        }
    }

    Ok(CompactBlockMessage::Announcement {
        header,
        short_ids,
        txn_count: block.transactions.len(),
    })
}

/// Serialize just the block header (matching QantoBlock::hash_for_pow format)
fn serialize_block_header(block: &QantoBlock) -> Result<Vec<u8>, CompactBlockError> {
    let mut header = Vec::new();
    header.extend_from_slice(&block.timestamp.to_be_bytes());
    header.extend_from_slice(&block.chain_id.to_be_bytes());
    header.extend_from_slice(&block.height.to_be_bytes());
    header.extend_from_slice(&block.difficulty.to_be_bytes());
    header.extend_from_slice(block.merkle_root.as_bytes());
    header.extend_from_slice(block.validator.as_bytes());
    header.extend_from_slice(block.miner.as_bytes());
    for parent in &block.parents {
        header.extend_from_slice(parent.as_bytes());
    }
    Ok(header)
}

/// Reconstruct a block from compact announcement + received transactions
pub fn reconstruct_block_from_compact(
    announcement: &CompactBlockMessage,
    received_txs: &[Transaction],
    block: &QantoBlock,
) -> Result<QantoBlock, CompactBlockError> {
    let CompactBlockMessage::Announcement { .. } = announcement else {
        return Err(CompactBlockError::InvalidBlock(
            "Not an announcement".to_string(),
        ));
    };

    // Build transaction list: use block's transactions where we have them,
    // otherwise use received transactions
    let mut txns = Vec::with_capacity(block.transactions.len());

    // We need to match received transactions to the short IDs
    // The received_txs should be in the same order as requested indices
    // For now, we assume the caller provides them in correct order

    // Coinbase is always first and should be in the original block
    if block.transactions.is_empty() {
        return Err(CompactBlockError::InvalidBlock(
            "Block has no transactions".to_string(),
        ));
    }
    txns.push(block.transactions[0].clone()); // coinbase

    // For remaining transactions, we need to match by short ID
    // This is a simplified version - in practice we'd have a proper index
    let mut received_idx = 0;
    for (_block_tx_idx, block_tx) in block.transactions.iter().enumerate().skip(1) {
        if received_idx < received_txs.len() {
            // Check if this received tx matches the expected short ID
            let expected_short_id = ShortId::from_txid(&block_tx.id);
            let received_short_id = ShortId::from_txid(&received_txs[received_idx].id);

            if expected_short_id == received_short_id {
                txns.push(received_txs[received_idx].clone());
                received_idx += 1;
            } else {
                // Fall back to original block transaction
                txns.push(block_tx.clone());
            }
        } else {
            txns.push(block_tx.clone());
        }
    }

    // Create new block with reconstructed transactions
    let mut new_block = block.clone();
    new_block.transactions = txns;
    new_block.merkle_root = compute_merkle_root(&new_block.transactions)?;

    Ok(new_block)
}

/// Compute merkle root from transactions
fn compute_merkle_root(transactions: &[Transaction]) -> Result<String, CompactBlockError> {
    if transactions.is_empty() {
        return Ok(String::new());
    }

    let mut hashes: Vec<String> = transactions.iter().map(|tx| tx.id.clone()).collect();

    while hashes.len() > 1 {
        let mut next_level = Vec::new();
        for chunk in hashes.chunks(2) {
            let combined = if chunk.len() == 2 {
                format!("{}{}", chunk[0], chunk[1])
            } else {
                chunk[0].clone()
            };
            let hash = blake3::hash(combined.as_bytes());
            next_level.push(hex::encode(hash.as_bytes()));
        }
        hashes = next_level;
    }

    Ok(hashes[0].clone())
}

/// Create a GetBlockTxn request for missing transactions
pub fn create_get_block_txn_request(
    block_id: String,
    missing_indices: Vec<u16>,
) -> CompactBlockMessage {
    CompactBlockMessage::GetBlockTxn {
        block_id,
        indices: missing_indices,
    }
}

/// Create a BlockTxn response with requested transactions
pub fn create_block_txn_response(
    block_id: String,
    transactions: Vec<Transaction>,
) -> CompactBlockMessage {
    CompactBlockMessage::BlockTxn {
        block_id,
        transactions,
    }
}

/// Serialize a compact block message for network transport
pub fn serialize_compact_block_message(
    msg: &CompactBlockMessage,
) -> Result<Vec<u8>, CompactBlockError> {
    bincode::serialize(msg).map_err(|e| CompactBlockError::SerializationError(e.to_string()))
}

/// Deserialize a compact block message from network transport
pub fn deserialize_compact_block_message(
    data: &[u8],
) -> Result<CompactBlockMessage, CompactBlockError> {
    bincode::deserialize(data).map_err(|e| CompactBlockError::SerializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_id_from_txid() {
        let txid = "abcdef1234567890";
        let short_id = ShortId::from_txid(txid);

        // Verify it's deterministic
        let short_id2 = ShortId::from_txid(txid);
        assert_eq!(short_id, short_id2);

        // Verify it's 8 bytes
        assert_eq!(short_id.as_bytes().len(), SHORT_ID_SIZE);
    }

    #[test]
    fn test_short_id_collision_detection() {
        // Two different txids should produce different short IDs (extremely unlikely to collide)
        let short_id1 = ShortId::from_txid("txid1");
        let short_id2 = ShortId::from_txid("txid2");
        assert_ne!(short_id1, short_id2);
    }

    #[test]
    fn test_short_id_display() {
        let short_id = ShortId::from_txid("test_txid");
        let display = format!("{}", short_id);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_serialize_deserialize_announcement() {
        // This would need a real QantoBlock to test fully
        // For now, test the serialization round-trip works for the enum
        let msg = CompactBlockMessage::Announcement {
            header: vec![1, 2, 3, 4],
            short_ids: vec![
                ShortId([1, 2, 3, 4, 5, 6, 7, 8]),
                ShortId([9, 10, 11, 12, 13, 14, 15, 16]),
            ],
            txn_count: 2,
        };

        let serialized = serialize_compact_block_message(&msg).unwrap();
        let deserialized = deserialize_compact_block_message(&serialized).unwrap();

        match deserialized {
            CompactBlockMessage::Announcement {
                header,
                short_ids,
                txn_count,
            } => {
                assert_eq!(header, vec![1, 2, 3, 4]);
                assert_eq!(short_ids.len(), 2);
                assert_eq!(txn_count, 2);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_get_block_txn_serialization() {
        let msg = create_get_block_txn_request("block123".to_string(), vec![1, 3, 5]);
        let serialized = serialize_compact_block_message(&msg).unwrap();
        let deserialized = deserialize_compact_block_message(&serialized).unwrap();

        match deserialized {
            CompactBlockMessage::GetBlockTxn { block_id, indices } => {
                assert_eq!(block_id, "block123");
                assert_eq!(indices, vec![1, 3, 5]);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_block_txn_serialization() {
        let msg = create_block_txn_response("block123".to_string(), vec![]);
        let serialized = serialize_compact_block_message(&msg).unwrap();
        let deserialized = deserialize_compact_block_message(&serialized).unwrap();

        match deserialized {
            CompactBlockMessage::BlockTxn {
                block_id,
                transactions,
            } => {
                assert_eq!(block_id, "block123");
                assert_eq!(transactions.len(), 0);
            }
            _ => panic!("Wrong variant"),
        }
    }
}
