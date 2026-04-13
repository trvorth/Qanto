//! Real-time balance broadcasting utilities for Qanto Core
//!
//! Provides an efficient, low-latency balance update stream designed to support
//! sub-second wallet UI updates while maintaining minimal resource usage.
//!
//! - Uses `tokio::sync::broadcast` for multi-subscriber delivery.
//! - Coalesces per-address updates and emits only changed balances.
//! - Thread-safe storage via `dashmap::DashMap` for high update throughput.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Balance update event emitted to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BalanceUpdate {
    /// Account address (hex-encoded or bech32, depending on configuration).
    pub address: String,
    /// Balance in smallest units.
    pub balance: u128,
    /// Event timestamp in milliseconds since UNIX_EPOCH.
    pub timestamp_ms: u128,
}

/// Balance subscription request for streaming updates over P2P.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BalanceSubscribe {
    /// Account address to subscribe to.
    pub address: String,
    /// Client-generated subscription identifier to correlate responses.
    pub subscription_id: String,
    /// Optional debounce interval in milliseconds for coalescing updates.
    pub debounce_ms: Option<u64>,
}

/// High-throughput balance broadcaster.
///
/// Designed to handle millions of updates per second with minimal contention.
#[derive(Debug)]
pub struct BalanceBroadcaster {
    balances: Arc<DashMap<String, u128>>, // address -> balance
    tx: broadcast::Sender<BalanceUpdate>,
}

impl BalanceBroadcaster {
    /// Create a new broadcaster with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel::<BalanceUpdate>(capacity.max(1024));
        Self {
            balances: Arc::new(DashMap::new()),
            tx,
        }
    }

    /// Subscribe to balance updates.
    pub fn subscribe(&self) -> broadcast::Receiver<BalanceUpdate> {
        self.tx.subscribe()
    }

    /// Set the absolute balance for an address and emit an update if changed.
    pub fn set_balance(&self, address: &str, new_balance: u128) {
        let changed = match self.balances.get(address) {
            Some(curr) => *curr != new_balance,
            None => true,
        };
        if changed {
            self.balances.insert(address.to_string(), new_balance);
            let update = BalanceUpdate {
                address: address.to_string(),
                balance: new_balance,
                timestamp_ms: current_time_ms(),
            };
            // Ignore send errors when no subscribers.
            let _ = self.tx.send(update);
        }
    }

    /// Apply a delta to the balance and emit an update.
    pub fn apply_delta(&self, address: &str, delta: i128) {
        let next = match self.balances.get(address) {
            Some(curr) => {
                let c = *curr as i128 + delta;
                if c < 0 {
                    0
                } else {
                    c as u128
                }
            }
            None => {
                if delta < 0 {
                    0
                } else {
                    delta as u128
                }
            }
        };
        self.set_balance(address, next);
    }

    /// Get the current balance for the address.
    pub fn get_balance(&self, address: &str) -> Option<u128> {
        self.balances.get(address).map(|v| *v)
    }
}

#[inline]
fn current_time_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_immediate_update() {
        let bb = BalanceBroadcaster::new(10_000);
        let mut rx = bb.subscribe();
        bb.set_balance("addr1", 123);

        // Expect sub-second delivery; await the next message.
        use tokio::time::{timeout, Duration};
        let update = timeout(Duration::from_millis(500), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(update.address, "addr1");
        assert_eq!(update.balance, 123);
    }

    #[tokio::test]
    async fn apply_delta_accumulates_correctly() {
        let bb = BalanceBroadcaster::new(1024);
        bb.set_balance("addr2", 1000);
        bb.apply_delta("addr2", 250);
        bb.apply_delta("addr2", -100);
        assert_eq!(bb.get_balance("addr2"), Some(1150));
    }
}
