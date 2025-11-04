//! Qanto Core Enhancements: Merged Block Display and Real-Time Balances
//!
//! Overview
//! - Implements a merged `Display` for `QantoBlock` combining concise and detailed
//!   formats for high-throughput logging. Optimized for minimal allocations and
//!   deterministic rendering under 32 BPS and 10,000,000+ TPS targets.
//! - Adds `balance_stream` providing sub-second, accurate wallet balance updates
//!   via Tokio broadcast channels and DashMap-backed storage.
//!
//! Performance Targets
//! - Display path: single-line output, truncated identifiers, avoids heap joins.
//! - Balance stream: emits only changed balances; coalesces updates per address.
//!
//! Example Configuration (TOML)
//!
//! ```toml
//! [wallet]
//! # Broadcast channel capacity for wallet balance updates
//! broadcaster_capacity = 65536
//! # Optional: initial addresses to watch (application-specific)
//! watch_addresses = ["addr1", "addr2"]
//! ```
//!
//! Wallet Integration (Rust)
//!
//! ```rust
//! use qanto_core::{BalanceBroadcaster, BalanceUpdate};
//! use tokio::task;
//!
//! let bb = BalanceBroadcaster::new(65_536);
//! let mut rx = bb.subscribe();
//!
//! // Spawn a wallet UI updater
//! task::spawn(async move {
//!     while let Ok(update) = rx.recv().await {
//!         // Update UI immediately with update.address and update.balance
//!     }
//! });
//!
//! // Somewhere in block processing
//! bb.apply_delta("addr1", 1_000); // credit
//! bb.apply_delta("addr1", -250); // debit
//! ```
//!
//! Merged `Display` for `QantoBlock`
//! - Format: `[Chain: <id>, Height: <h>, ID: <short>, Txs: <n>, Parent: <short|none>]`
//! - Truncation: 12 chars + ellipsis (`…`) for long IDs.
//! - Deterministic ordering and content to ensure consistent logs.
//!
//! Backward Compatibility
//! - `QantoBlock` retains `id` and `data`. Additional fields (`chain_id`, `height`,
//!   `timestamp`, `parents`, `transactions_len`) have `serde(default)` to preserve
//!   wire compatibility with older payloads.
//! - `QantoBlock` is re-exported from `qanto_core::qanto_net` via `qanto_core::QantoBlock`.
//!
//! Quality Assurance
//! - Unit tests verify `Display` truncation behavior and parent rendering.
//! - Balance broadcaster correctness tested for immediate delivery and delta math.
//! - Micro-bench tests print throughput metrics for operator review without heavy deps.
//!
//! Migration Guide
//! 1. Imports: use `qanto_core::QantoBlock` where a displayable block is needed.
//! 2. Logging: prefer `format!("{}", block)` rather than custom formatters.
//! 3. Wallet: instantiate `BalanceBroadcaster` and connect your wallet UI to `subscribe()`.
//! 4. Storage/Net: existing `qanto_storage`, `qanto_p2p`, and `qanto_serde` integrations remain
//!    unaffected due to `serde(default)` and re-exports.
//!
//! Benchmarks
//! - See `perf_bench` tests for printed ops/sec metrics in CI or local runs.
//!   Use `cargo test -- --nocapture` to view results.
//!
