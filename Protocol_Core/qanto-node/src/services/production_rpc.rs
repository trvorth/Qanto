//! ProductionRpcService — RuntimeService wrapping the gRPC RPC server lifecycle.
//!
//! Wraps `qanto_rpc::start()` with RuntimeService lifecycle management.
//! The service binds a tonic gRPC server on the configured RPC address.
//!
//! # Design
//!
//! - `start()` spawns the gRPC server in a tokio task
//! - `stop()` signals cancellation
//! - `shutdown()` waits for the task to complete
//! - No RPC logic — delegates entirely to `qanto_rpc::start()`
//! - Depends on `telemetry` (telemetry must be running for metrics-backed RPC)

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production RpcService — wraps gRPC RPC server with RuntimeService lifecycle.
pub struct ProductionRpcService {
    /// RPC bind address.
    addr: SocketAddr,
    /// gRPC backend (Arc<dyn RpcBackend>).
    backend: Arc<dyn qanto_rpc::RpcBackend>,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Task handle for the gRPC server.
    task_handle: RwLock<Option<JoinHandle<()>>>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
}

impl ProductionRpcService {
    /// Create a new RpcService.
    pub fn new(addr: SocketAddr, backend: Arc<dyn qanto_rpc::RpcBackend>) -> Self {
        Self {
            addr,
            backend,
            running: Arc::new(AtomicBool::new(false)),
            task_handle: RwLock::new(None),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    /// Check if the service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl RuntimeService for ProductionRpcService {
    fn name(&self) -> &str {
        "rpc"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["telemetry"]
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionRpcService: initialize on {}", self.addr);
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!(
            "ProductionRpcService: Starting gRPC server on {}",
            self.addr
        );

        self.running.store(true, Ordering::SeqCst);

        let addr = self.addr;
        let backend = self.backend.clone();
        let running = self.running.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        let handle = tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    info!("ProductionRpcService: Cancellation received, gRPC server shutting down");
                    running.store(false, Ordering::SeqCst);
                }
                result = qanto_rpc::start(addr, backend) => {
                    match result {
                        Ok(()) => {
                            warn!("ProductionRpcService: gRPC server exited unexpectedly");
                        }
                        Err(e) => {
                            error!("ProductionRpcService: gRPC server failed: {}", e);
                        }
                    }
                    running.store(false, Ordering::SeqCst);
                }
            }
        });

        let mut handle_guard = self.task_handle.write().await;
        *handle_guard = Some(handle);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        health.error_message = Some(format!("Listening on {}", self.addr));
        health.last_check = SystemTime::now();
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionRpcService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionRpcService: Shutdown — waiting for gRPC task to exit");

        let mut handle_guard = self.task_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            // Wait up to 10 seconds for the task to complete
            match tokio::time::timeout(std::time::Duration::from_secs(10), handle).await {
                Ok(Ok(())) => {
                    info!("ProductionRpcService: gRPC task exited cleanly");
                }
                Ok(Err(e)) => {
                    warn!("ProductionRpcService: gRPC task join error: {}", e);
                }
                Err(_) => {
                    warn!("ProductionRpcService: gRPC task did not exit within 10s, aborting");
                }
            }
        }

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        health.last_check = SystemTime::now();
        info!("ProductionRpcService: Shutdown complete");
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        let mut health = self.health.read().await.clone();
        if self.running.load(Ordering::Relaxed) {
            health.status = ServiceStatus::Running;
        } else if health.status != ServiceStatus::Failed {
            health.status = ServiceStatus::Stopped;
        }
        health.last_check = SystemTime::now();
        health
    }
}
