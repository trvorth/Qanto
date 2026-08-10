//! Port traits (interfaces) for the relayer architecture.

use crate::error::RelayerResult;
use async_trait::async_trait;
use serde::Serialize;

pub mod types;

pub use crate::ports::types::{AssetLockedEvent, BlockHeader, TransactionReceipt};

/// Port: Read-only access to the source EVM chain (bridge events, receipts, blocks).
#[async_trait]
pub trait SourceChainGateway: Send + Sync {
    /// Get the latest block number.
    async fn latest_block_number(&self) -> RelayerResult<u64>;

    /// Fetch AssetLocked events in a block range.
    async fn fetch_locked_events(
        &self,
        bridge_address: &str,
        from_block: u64,
        to_block: u64,
    ) -> RelayerResult<Vec<AssetLockedEvent>>;

    /// Fetch a transaction receipt by hash.
    async fn fetch_receipt(&self, tx_hash: &str) -> RelayerResult<TransactionReceipt>;

    /// Fetch a block header by hash.
    async fn fetch_block_header(&self, block_hash: &str) -> RelayerResult<BlockHeader>;
}

/// Port: Construct a bridge claim request from a validated event.
#[async_trait]
pub trait ClaimFactory: Send + Sync {
    /// Build a signed BridgeClaimRequest ready for dispatch.
    async fn build_claim(
        &self,
        event: &AssetLockedEvent,
        source_chain: &str,
        relayer_signer: &dyn RelayerSigner,
    ) -> RelayerResult<BridgeClaimRequest>;
}

/// Port: Signing capability for the relayer key.
pub trait RelayerSigner: Send + Sync {
    /// Get the relayer's Ethereum address (derived from signing key).
    fn address(&self) -> &str;

    /// Sign a prehash using the relayer's private key.
    fn sign_prehash(&self, prehash: &[u8; 32]) -> RelayerResult<String>;
}

/// The claim request payload to send to Qanto.
#[derive(Debug, Clone, Serialize)]
pub struct BridgeClaimRequest {
    pub payload: Vec<u8>, // Opaque; adapter knows the encoding
}

/// Port: Dispatch a claim to the Qanto chain.
#[async_trait]
pub trait ClaimDispatcher: Send + Sync {
    /// Submit a claim and return the Qanto transaction hash.
    async fn dispatch(&self, claim: &BridgeClaimRequest) -> RelayerResult<String>;
}

/// Port: Persistent delivery state repository.
#[async_trait]
pub trait DeliveryStateRepository: Send + Sync {
    /// Load state from storage.
    async fn load(&mut self) -> RelayerResult<()>;

    /// Save state to storage.
    async fn save(&self) -> RelayerResult<()>;

    /// Check if an event should be skipped (already delivered or invalid).
    fn should_skip(&self, event_key: &str) -> bool;

    /// Check if an event is due for retry.
    fn should_retry(&self, event_key: &str, now_unix_secs: u64, backoff_secs: u64) -> bool;

    /// Mark an attempt (increment counter, update timestamp).
    fn mark_attempt(&mut self, event_key: &str, now_unix_secs: u64);

    /// Record a successful delivery.
    fn mark_delivered(&mut self, event_key: &str, now_unix_secs: u64, claim_tx_hash: String);

    /// Record an invalid (non-retryable) event.
    fn mark_invalid(&mut self, event_key: &str, now_unix_secs: u64, reason: String);

    /// Record a retryable error.
    fn mark_retryable(
        &mut self,
        event_key: &str,
        now_unix_secs: u64,
        max_attempts: u32,
        error: String,
    );

    /// Get all event keys that are pending retry.
    fn pending_retry_keys(&self, now_unix_secs: u64, backoff_secs: u64) -> Vec<String>;

    /// Prune old completed entries. Returns true if any were pruned.
    fn prune_completed(&mut self, now_unix_secs: u64, retention_secs: u64) -> bool;
}
