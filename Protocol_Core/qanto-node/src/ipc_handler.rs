//! IpcHandler trait — minimal abstraction over the blockchain state needed
//! by the IPC server. Extracted from `IpcServer::start` to enable test doubles.
//!
//! The production implementation delegates to `QantoDAG`. The test double
//! returns fixed values, allowing full lifecycle integration tests without
//! requiring consensus, mining, DAG validation, or quantum signatures.

use async_trait::async_trait;

/// Minimal trait for IPC request handling.
/// Only exposes the two operations IpcServer actually uses.
#[async_trait]
pub trait IpcHandler: Send + Sync + 'static {
    /// Get the balance for an address.
    fn get_balance(&self, address: &str) -> u128;

    /// Get the current block height.
    async fn get_block_count(&self) -> u64;
}

/// Production IpcHandler — delegates to QantoDAG.
pub struct DagIpcHandler {
    pub dag: std::sync::Arc<crate::qantodag::QantoDAG>,
}

#[async_trait]
impl IpcHandler for DagIpcHandler {
    fn get_balance(&self, address: &str) -> u128 {
        self.dag
            .account_state_cache
            .get_balance(address)
            .unwrap_or(0)
    }

    async fn get_block_count(&self) -> u64 {
        self.dag.get_block_count().await
    }
}

/// Test double IpcHandler — returns fixed values. No blockchain required.
pub struct StaticIpcHandler {
    pub balance: u128,
    pub height: u64,
}

#[async_trait]
impl IpcHandler for StaticIpcHandler {
    fn get_balance(&self, _address: &str) -> u128 {
        self.balance
    }

    async fn get_block_count(&self) -> u64 {
        self.height
    }
}
