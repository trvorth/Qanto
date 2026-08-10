//! Copyright (C) 2026 Trevor <trvorth@qanto.org>
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program. If not, see <https://gnu.org>.

//! P2pRunnable trait — minimal abstraction over the P2P event loop.
//!
//! Enables `ProductionP2PService` to be tested with a lightweight runnable
//! without requiring full P2PServer construction (libp2p Swarm, blockchain
//! dependencies, keypairs, etc.).
//!
//! # Design
//!
//! - Production: `P2PServerRunnable` wraps `P2PServer.run()`
//! - Test: `NoopP2pRunnable` immediately exits on cancellation
//!
//! The trait is intentionally minimal — it only abstracts the `run()` call.
//! All construction (Swarm, behaviours, transport) stays in node.rs Phase 1.

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::p2p::{P2PCommand, P2PServer};

/// Minimal trait for the P2P event loop.
/// Only abstracts `run()` — construction stays in node.rs Phase 1.
#[async_trait]
pub trait P2pRunnable: Send + 'static {
    /// Run the P2P event loop with the given command receiver.
    async fn run(self, rx: mpsc::Receiver<P2PCommand>);
}

/// Production runnable — delegates to P2PServer.run().
pub struct P2PServerRunnable {
    pub server: P2PServer,
}

#[async_trait]
impl P2pRunnable for P2PServerRunnable {
    async fn run(self, rx: mpsc::Receiver<P2PCommand>) {
        let mut server = self.server;
        match server.run(rx).await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("P2PServer.run() error: {}", e);
            }
        }
    }
}

/// Test double — immediately yields to cancellation.
/// Used for lifecycle integration tests without libp2p or blockchain setup.
pub struct NoopP2pRunnable;

#[async_trait]
impl P2pRunnable for NoopP2pRunnable {
    async fn run(self, _rx: mpsc::Receiver<P2PCommand>) {
        // No-op: the test's CancellationToken will race against this
        // and win, triggering clean shutdown.
        std::future::pending::<()>().await;
    }
}
