//! Qanto Runtime Context
//!
//! Shared operational infrastructure for all services managed by the Runtime Manager.
//! Contains only infrastructure resources — no blockchain state, no consensus state.
//!
//! # Design Principle
//!
//! RuntimeContext is the "hardware abstraction layer" for services. It provides
//! handles to configuration, storage, metrics, networking, and cryptography.
//! Blockchain state (DAG, UTXOs, mempool, consensus) lives in Core Engines,
//! not in RuntimeContext.

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Shared operational infrastructure for all node services.
///
/// All fields are wrapped in `Arc` for shared ownership. Services hold an
/// `Arc<RuntimeContext>` and access these resources during their lifecycle.
///
/// # What belongs here
/// - Configuration
/// - Database / Storage handles
/// - Metrics registry
/// - Cancellation token (for coordinated shutdown)
/// - Cryptographic providers
/// - Network handles
/// - Rate limiting
///
/// # What does NOT belong here
/// - Consensus state (owned by QantoDAG)
/// - Mempool state (owned by Mempool)
/// - UTXO state (owned by QantoDAG)
/// - Mining state (owned by MiningEngine)
/// - Blockchain execution state
#[derive(Clone)]
pub struct RuntimeContext {
    /// Node configuration (loaded from config.toml).
    pub config: Arc<crate::config::Config>,

    /// RocksDB-backed storage layer (shared across DAG, persistence, state sync).
    pub storage: Option<Arc<crate::qanto_storage::QantoStorage>>,

    /// Unified metrics collector.
    pub metrics: Arc<crate::metrics::QantoMetrics>,

    /// Diagnostics engine for node health assessment.
    pub diagnostics: Option<Arc<crate::diagnostics::DiagnosticsEngine>>,

    /// P2P identity keypair (set after P2P initialization).
    /// Using a placeholder PathBuf-based type until integrated.
    pub p2p_identity: Option<Arc<RwLock<Option<libp2p::identity::Keypair>>>>,

    /// P2P swarm handle (set after P2P server starts).
    pub p2p_swarm: Option<Arc<RwLock<Option<P2PSwarmHandle>>>>,

    /// Peer cache for persistent peer discovery.
    pub peer_cache: Arc<RwLock<crate::node::PeerCache>>,

    /// Global cancellation token. Cancelled once, propagates to all child tokens.
    pub cancellation_token: CancellationToken,

    /// Rate limiter for public API endpoints.
    pub rate_limiter: Option<Arc<DirectApiRateLimiter>>,
}

/// Opaque handle to the P2P swarm. Prevents RuntimeContext from coupling to
/// libp2p internal types while allowing services to interact with the swarm
/// through well-defined methods.
#[derive(Clone)]
pub struct P2PSwarmHandle {
    /// Peer ID of the local node.
    pub local_peer_id: libp2p::PeerId,
    /// Whether the swarm is actively running.
    pub is_active: bool,
}

/// Type alias for the API rate limiter to avoid importing governor types
/// directly in RuntimeContext.
type DirectApiRateLimiter = governor::RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::QuantaClock,
>;

impl RuntimeContext {
    /// Create a minimal RuntimeContext with only the required fields.
    /// Callers should populate optional fields after construction as
    /// services and engines initialize.
    pub fn new(config: Arc<crate::config::Config>, cancellation_token: CancellationToken) -> Self {
        Self {
            config,
            storage: None,
            // Use the global metrics singleton — single canonical instance.
            metrics: crate::metrics::get_global_metrics(),
            diagnostics: None,
            p2p_identity: None,
            p2p_swarm: None,
            peer_cache: Arc::new(RwLock::new(crate::node::PeerCache::default())),
            cancellation_token,
            rate_limiter: None,
        }
    }

    /// Create a child cancellation token for a specific service.
    /// The child token is cancelled when the parent is cancelled.
    pub fn child_token(&self) -> CancellationToken {
        self.cancellation_token.child_token()
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

impl std::fmt::Debug for RuntimeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeContext")
            .field("config", &"<Config>")
            .field("storage", &self.storage.as_ref().map(|_| "<QantoStorage>"))
            .field("metrics", &"<QantoMetrics>")
            .field(
                "diagnostics",
                &self.diagnostics.as_ref().map(|_| "<DiagnosticsEngine>"),
            )
            .field(
                "p2p_identity",
                &self.p2p_identity.as_ref().map(|_| "<Keypair>"),
            )
            .field(
                "p2p_swarm",
                &self.p2p_swarm.as_ref().map(|_| "<P2PSwarmHandle>"),
            )
            .field("peer_cache", &"<PeerCache>")
            .field(
                "cancellation_token",
                &self.cancellation_token.is_cancelled(),
            )
            .field("rate_limiter", &"<RateLimiter>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_runtime_context_creation() {
        let config = Arc::new(crate::config::Config::default());
        let token = CancellationToken::new();

        let ctx = RuntimeContext::new(config, token.clone());
        assert!(!ctx.is_shutdown_requested());

        // Verify child token propagation
        let child = ctx.child_token();
        assert!(!child.is_cancelled());

        token.cancel();
        assert!(ctx.is_shutdown_requested());
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_runtime_context_debug() {
        let config = Arc::new(crate::config::Config::default());
        let ctx = RuntimeContext::new(config, CancellationToken::new());
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("RuntimeContext"));
        assert!(debug_str.contains("<Config>"));
    }
}
