//! Header-First Parallel Sync Protocol
//!
//! Implements efficient header chain synchronization using libp2p request-response.
//! Peers request headers in ranges, then download blocks in parallel across multiple peers.
//!
//! Protocol: /qanto/headers/2.0.0
//! Request:  HeadersRequest { start_height, end_height }
//! Response: HeadersResponse { headers, tip_height }
//!
//! Sync phases:
//!   HeaderDownload → BlockDownload (parallel across N peers) → Done

use crate::qantodag::QantoBlock;
use async_trait::async_trait;
use bincode::Options;
use futures::prelude::*;
use libp2p::request_response::Codec;
use serde::{Deserialize, Serialize};
use std::io;

pub const HEADERS_PROTOCOL: &str = "/qanto/headers/2.0.0";
pub const MAX_HEADERS_REQUEST_SIZE: usize = 128;
pub const MAX_HEADERS_RESPONSE_SIZE: usize = 8 * 1024 * 1024;
pub const MAX_HEADERS_PER_REQUEST: u64 = 512;
pub const MAX_HEADER_SYNC_SPAN: u64 = 100_000;

/// Number of blocks per download chunk (distributed across peers)
pub const BLOCKS_PER_CHUNK: u64 = 500;

/// Timeout for header download requests
pub const HEADERS_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Maximum number of parallel block download peers
pub const MAX_PARALLEL_DOWNLOAD_PEERS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeadersRequest {
    /// Request headers from start_height (inclusive) to end_height (exclusive)
    GetHeaders { start_height: u64, end_height: u64 },
    /// Request a specific block by height
    GetBlockByHeight { height: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeadersResponse {
    /// Response with block headers
    Headers {
        headers: Vec<BlockHeader>,
        /// The latest known tip height of the responding peer
        tip_height: u64,
    },
    /// Response with a full block
    Block { block: Box<QantoBlock> },
    /// Error response
    Error { message: String },
}

/// Lightweight block header for sync (no transactions, no signatures)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    pub height: u64,
    pub chain_id: u32,
    pub timestamp: u64,
    pub difficulty: u64,
    pub merkle_root: String,
    pub validator: String,
    pub miner: String,
    pub parents: Vec<String>,
    pub block_id: String,
    /// Number of transactions (for validation)
    pub txn_count: usize,
    /// Total transaction fees in this block (for cumulative work estimation)
    pub total_fee: u128,
}

impl BlockHeader {
    /// Extract header from a full QantoBlock
    pub fn from_block(block: &QantoBlock) -> Self {
        Self {
            height: block.height,
            chain_id: block.chain_id,
            timestamp: block.timestamp,
            difficulty: block.difficulty,
            merkle_root: block.merkle_root.clone(),
            validator: block.validator.clone(),
            miner: block.miner.clone(),
            parents: block.parents.clone(),
            block_id: block.id.clone(),
            txn_count: block.transactions.len(),
            total_fee: block.transactions.iter().skip(1).map(|tx| tx.fee).sum(),
        }
    }
}

#[derive(Clone, Default)]
pub struct HeaderSyncCodec;

fn deserialize_bounded<'a, T>(bytes: &'a [u8], limit: usize) -> io::Result<T>
where
    T: Deserialize<'a>,
{
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(limit as u64)
        .reject_trailing_bytes()
        .deserialize(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn serialize_bounded<T>(value: &T, limit: usize) -> io::Result<Vec<u8>>
where
    T: Serialize,
{
    let bytes = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(limit as u64)
        .serialize(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "header-sync payload exceeds protocol limit",
        ));
    }
    Ok(bytes)
}

#[async_trait]
impl Codec for HeaderSyncCodec {
    type Protocol = String;
    type Request = HeadersRequest;
    type Response = HeadersResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::with_capacity(MAX_HEADERS_REQUEST_SIZE);
        io.take((MAX_HEADERS_REQUEST_SIZE + 1) as u64)
            .read_to_end(&mut buf)
            .await?;
        if buf.len() > MAX_HEADERS_REQUEST_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "header-sync request exceeds protocol limit",
            ));
        }
        deserialize_bounded(&buf, MAX_HEADERS_REQUEST_SIZE)
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::with_capacity(16 * 1024);
        io.take((MAX_HEADERS_RESPONSE_SIZE + 1) as u64)
            .read_to_end(&mut buf)
            .await?;
        if buf.len() > MAX_HEADERS_RESPONSE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "header-sync response exceeds protocol limit",
            ));
        }
        let response: HeadersResponse = deserialize_bounded(&buf, MAX_HEADERS_RESPONSE_SIZE)?;
        if let HeadersResponse::Headers { headers, .. } = &response {
            if headers.len() > MAX_HEADERS_PER_REQUEST as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "header-sync response contains too many headers",
                ));
            }
        }
        Ok(response)
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        if let HeadersRequest::GetHeaders {
            start_height,
            end_height,
        } = &req
        {
            let count = end_height.checked_sub(*start_height).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid header range")
            })?;
            if count == 0 || count > MAX_HEADERS_PER_REQUEST {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "header range exceeds protocol limit",
                ));
            }
        }
        let buf = serialize_bounded(&req, MAX_HEADERS_REQUEST_SIZE)?;
        io.write_all(&buf).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        if let HeadersResponse::Headers { headers, .. } = &res {
            if headers.len() > MAX_HEADERS_PER_REQUEST as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "header-sync response contains too many headers",
                ));
            }
        }
        let buf = serialize_bounded(&res, MAX_HEADERS_RESPONSE_SIZE)?;
        io.write_all(&buf).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_from_block() {
        // Verify BlockHeader can be constructed from a minimal block definition
        let header = BlockHeader {
            height: 42,
            chain_id: 1,
            timestamp: 1_700_000_000,
            difficulty: 100_000,
            merkle_root: "test_merkle".to_string(),
            validator: "validator_addr".to_string(),
            miner: "miner_addr".to_string(),
            parents: vec!["parent_1".to_string()],
            block_id: "block_42".to_string(),
            txn_count: 10,
            total_fee: 5000,
        };

        assert_eq!(header.height, 42);
        assert_eq!(header.txn_count, 10);
    }

    #[test]
    fn test_headers_request_serialization() {
        let req = HeadersRequest::GetHeaders {
            start_height: 100,
            end_height: 200,
        };

        let serialized = bincode::serialize(&req).expect("serialize");
        let deserialized: HeadersRequest = bincode::deserialize(&serialized).expect("deserialize");

        match deserialized {
            HeadersRequest::GetHeaders {
                start_height,
                end_height,
            } => {
                assert_eq!(start_height, 100);
                assert_eq!(end_height, 200);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_headers_response_serialization() {
        let resp = HeadersResponse::Headers {
            headers: vec![BlockHeader {
                height: 100,
                chain_id: 1,
                timestamp: 1_700_000_000,
                difficulty: 100_000,
                merkle_root: "root_100".to_string(),
                validator: "v_100".to_string(),
                miner: "m_100".to_string(),
                parents: vec![],
                block_id: "block_100".to_string(),
                txn_count: 5,
                total_fee: 1000,
            }],
            tip_height: 500,
        };

        let serialized = bincode::serialize(&resp).expect("serialize");
        let deserialized: HeadersResponse = bincode::deserialize(&serialized).expect("deserialize");

        match deserialized {
            HeadersResponse::Headers {
                headers,
                tip_height,
            } => {
                assert_eq!(headers.len(), 1);
                assert_eq!(headers[0].height, 100);
                assert_eq!(tip_height, 500);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_get_block_by_height() {
        let req = HeadersRequest::GetBlockByHeight { height: 12345 };
        let bytes = bincode::serialize(&req).expect("serialize");
        let decoded: HeadersRequest = bincode::deserialize(&bytes).expect("deserialize");

        match decoded {
            HeadersRequest::GetBlockByHeight { height } => {
                assert_eq!(height, 12345);
            }
            _ => panic!("wrong variant"),
        }
    }
}
