//! ProductionIpcService — RuntimeService wrapping the IPC server lifecycle.
//!
//! # Design
//!
//! The IPC server provides lightweight state channel access over:
//! - Unix domain socket (preferred on Unix)
//! - TCP fallback (when Unix socket fails or on non-Unix)
//!
//! This service owns:
//! - IPC listener lifecycle (Unix socket or TCP)
//! - graceful shutdown via CancellationToken
//! - health reporting
//!
//! # Testability
//!
//! The service is generic over `H: IpcHandler`. In production, `DagIpcHandler`
//! delegates to `QantoDAG`. In tests, `StaticIpcHandler` returns fixed values,
//! enabling full lifecycle integration tests without blockchain initialization.
//!
//! # What This Does NOT Own
//!
//! - DAG state
//! - Blockchain logic
//! - IPC protocol (delegates to IpcServer::start)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::config::Config;
use crate::ipc_handler::IpcHandler;
use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production IpcService — owns IPC server (Unix socket / TCP) lifecycle.
///
/// Generic over `H: IpcHandler` so tests can inject a static handler without
/// requiring blockchain initialization.
pub struct ProductionIpcService<H: IpcHandler = crate::ipc_handler::DagIpcHandler> {
    /// IPC request handler (production: DagIpcHandler, test: StaticIpcHandler).
    handler: Arc<H>,
    /// Node config (for socket path / TCP port).
    config: Config,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Task handle for the IPC server.
    task_handle: RwLock<Option<JoinHandle<()>>>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
}

impl<H: IpcHandler> ProductionIpcService<H> {
    /// Create a new IpcService.
    pub fn new(handler: Arc<H>, config: Config) -> Self {
        Self {
            handler,
            config,
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
impl<H: IpcHandler> RuntimeService for ProductionIpcService<H> {
    fn name(&self) -> &str {
        "ipc"
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
        info!("ProductionIpcService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionIpcService: Starting IPC server");

        self.running.store(true, Ordering::SeqCst);

        let handler = self.handler.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        let handle = tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    info!("ProductionIpcService: Cancellation received, IPC server shutting down");
                    // Clean up Unix socket on cancellation
                    #[cfg(unix)]
                    {
                        let socket_path = crate::ipc_server::ipc_unix_socket_path(&config);
                        let _ = std::fs::remove_file(&socket_path);
                        info!("ProductionIpcService: Removed IPC socket {}", socket_path.display());
                    }
                }
                _ = crate::ipc_server::IpcServer::start_with_handler(handler, &config) => {
                    warn!("ProductionIpcService: IPC server exited unexpectedly");
                }
            }
            running.store(false, Ordering::SeqCst);
        });

        let mut handle_guard = self.task_handle.write().await;
        *handle_guard = Some(handle);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        health.last_check = SystemTime::now();
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionIpcService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionIpcService: Shutdown");

        let mut handle_guard = self.task_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!("ProductionIpcService: IPC task exited cleanly");
                }
                Ok(Err(e)) => {
                    warn!("ProductionIpcService: IPC task join error: {}", e);
                }
                Err(_) => {
                    warn!("ProductionIpcService: IPC task did not exit within 5s, aborting");
                }
            }
        }

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        health.last_check = SystemTime::now();
        info!("ProductionIpcService: Shutdown complete");
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        let mut health = self.health.read().await.clone();
        if self.running.load(Ordering::Relaxed) {
            health.status = ServiceStatus::Running;
        } else {
            health.status = ServiceStatus::Stopped;
        }
        health.last_check = SystemTime::now();
        health
    }
}
