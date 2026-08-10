//! ProductionP2PService — RuntimeService wrapping libp2p P2P networking lifecycle.
//!
//! # Design
//!
//! The P2P networking stack is built around `P2PServer` which owns:
//! - libp2p Swarm with NodeBehaviour (GossipSub, Kademlia, identify, ping, rendezvous)
//! - peer discovery (Kademlia DHT + rendezvous bootstrap)
//! - transport (TCP with noise encryption)
//! - connection management with exponential backoff
//!
//! This service owns:
//! - P2P event loop lifecycle
//! - graceful shutdown via CancellationToken
//! - health reporting
//!
//! # Testability
//!
//! The service is generic over `R: P2pRunnable`. In production, `P2PServerRunnable`
//! delegates to `P2PServer.run()`. In tests, `NoopP2pRunnable` yields to cancellation,
//! enabling full lifecycle integration tests without libp2p Swarm construction.
//!
//! # Architecture Note
//!
//! P2P is intentionally a single operational service. The libp2p Swarm event
//! loop is monolithic — peer discovery, connection management, gossip routing,
//! mesh health, and keepalive all drain from `swarm.select_next_some()`.
//! Splitting into separate ConnectionManagerService or SyncService would require
//! restructuring the Swarm event loop, which is a networking protocol rewrite.
//! The correct boundary is: Phase 1 constructs the P2PServer, Phase 3 runs it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::p2p::P2PCommand;
use crate::p2p_runnable::P2pRunnable;
use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// ProductionP2PService — owns P2P event loop lifecycle.
///
/// Generic over `R: P2pRunnable` so tests can inject a no-op runnable
/// without requiring full P2PServer construction.
pub struct ProductionP2PService<R: P2pRunnable = crate::p2p_runnable::P2PServerRunnable> {
    /// The runnable P2P event loop (production: P2PServerRunnable, test: NoopP2pRunnable).
    runnable: Mutex<Option<R>>,
    /// Command receiver channel (paired with the sender used by other components).
    command_rx: Mutex<Option<mpsc::Receiver<P2PCommand>>>,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Task handle for the P2P event loop.
    task_handle: Mutex<Option<JoinHandle<()>>>,
    /// Internal health state.
    health: Mutex<ServiceHealth>,
}

impl<R: P2pRunnable> ProductionP2PService<R> {
    /// Create a new P2PService with a runnable P2P event loop and command channel.
    pub fn new(runnable: R, command_rx: mpsc::Receiver<P2PCommand>) -> Self {
        Self {
            runnable: Mutex::new(Some(runnable)),
            command_rx: Mutex::new(Some(command_rx)),
            running: Arc::new(AtomicBool::new(false)),
            task_handle: Mutex::new(None),
            health: Mutex::new(ServiceHealth::new(ServiceStatus::NotStarted)),
        }
    }

    /// Check if the service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl<R: P2pRunnable> RuntimeService for ProductionP2PService<R> {
    fn name(&self) -> &str {
        "p2p"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["api"] // P2P depends on API being available for connected-peers endpoint
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionP2PService: initialize");
        let mut health = self.health.lock().unwrap();
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionP2PService: Starting P2P event loop");

        // Take ownership of the runnable and receiver from the Mutex
        let runnable = self.runnable.lock().unwrap().take().ok_or_else(|| {
            RuntimeError::StartFailed(
                "P2P runnable already consumed".into(),
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "runnable was None",
                )),
            )
        })?;

        let command_rx = self.command_rx.lock().unwrap().take().ok_or_else(|| {
            RuntimeError::StartFailed(
                "P2P command receiver already consumed".into(),
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "command_rx was None",
                )),
            )
        })?;

        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        let handle = tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    info!("ProductionP2PService: Cancellation received, P2P shutting down");
                }
                _ = runnable.run(command_rx) => {
                    warn!("ProductionP2PService: P2P event loop exited unexpectedly");
                }
            }
            running.store(false, Ordering::SeqCst);
        });

        {
            let mut handle_guard = self.task_handle.lock().unwrap();
            *handle_guard = Some(handle);
        }

        let mut health = self.health.lock().unwrap();
        health.status = ServiceStatus::Running;
        health.last_check = SystemTime::now();
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionP2PService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.lock().unwrap();
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionP2PService: Shutdown — waiting for P2P event loop");

        let handle = {
            let mut handle_guard = self.task_handle.lock().unwrap();
            handle_guard.take()
        };

        if let Some(handle) = handle {
            match tokio::time::timeout(std::time::Duration::from_secs(10), handle).await {
                Ok(Ok(())) => {
                    info!("ProductionP2PService: P2P task exited cleanly");
                }
                Ok(Err(e)) => {
                    warn!("ProductionP2PService: P2P task join error: {}", e);
                }
                Err(_) => {
                    warn!("ProductionP2PService: P2P task did not exit within 10s, aborting");
                }
            }
        }

        let mut health = self.health.lock().unwrap();
        health.status = ServiceStatus::Stopped;
        health.last_check = SystemTime::now();
        info!("ProductionP2PService: Shutdown complete");
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        let mut health = self.health.lock().unwrap().clone();
        if self.running.load(Ordering::Relaxed) {
            health.status = ServiceStatus::Running;
        } else {
            health.status = ServiceStatus::Stopped;
        }
        health.last_check = SystemTime::now();
        health
    }
}
