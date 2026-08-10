//! Protocol module for Qanto P2P network protocols.
//!
//! This module contains implementations of various network protocols including:
//! - Compact block relay (BIP152-style)
//! - Experimental, bounded header-first sync primitives (not network-enabled)
//! - Latency-aware peer scoring (P1-D)

pub mod compact_block;
pub mod header_sync;
pub mod peer_scoring;

pub use compact_block::{
    build_compact_block_announcement, deserialize_compact_block_message,
    reconstruct_block_from_compact, CompactBlockError, CompactBlockMessage, ShortId,
    COMPACT_BLOCK_REQUEST_TIMEOUT, MAX_SHORT_IDS_PER_ANNOUNCEMENT, SHORT_ID_SIZE,
};

pub use header_sync::{
    BlockHeader, HeaderSyncCodec, HeadersRequest, HeadersResponse, BLOCKS_PER_CHUNK,
    HEADERS_PROTOCOL, HEADERS_REQUEST_TIMEOUT_SECS, MAX_HEADERS_PER_REQUEST,
    MAX_HEADERS_REQUEST_SIZE, MAX_HEADERS_RESPONSE_SIZE, MAX_HEADER_SYNC_SPAN,
    MAX_PARALLEL_DOWNLOAD_PEERS,
};

pub use peer_scoring::{PeerScore, PeerScoring, MAX_RTT_MS, MIN_RTT_MS, STALE_AFTER};
