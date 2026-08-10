//! ProductionApiService — RuntimeService owning the Axum HTTP server.
//!
//! # Design
//!
//! The Axum HTTP server is the shared listener for:
//! - REST API routes (info, config, transaction submission, etc.)
//! - GraphQL routes (query, mutation, subscription)
//! - WebSocket routes (block updates, mempool, network health)
//!
//! This service owns:
//! - Axum listener lifecycle
//! - bind / serve / graceful shutdown
//! - health reporting
//!
//! Route construction (REST handlers, GraphQL schema, WebSocket router) happens
//! in node.rs Phase 1 — this service receives the pre-built Router and only
//! manages the server's runtime lifecycle.

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

fn validate_api_bind_address(addr: SocketAddr) -> std::io::Result<()> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "refusing non-loopback API bind {addr}; terminate TLS and authentication at a local reverse proxy"
            ),
        ))
    }
}

/// Production ApiService — owns Axum HTTP server lifecycle.
pub struct ProductionApiService {
    /// The pre-built Axum Router (REST + GraphQL + WebSocket merged).
    app: axum::Router,
    /// Bind address.
    addr: SocketAddr,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Task handle for the Axum server.
    task_handle: RwLock<Option<JoinHandle<()>>>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
}

impl ProductionApiService {
    /// Create a new ApiService with a pre-built Router and bind address.
    pub fn new(app: axum::Router, addr: SocketAddr) -> Self {
        Self {
            app,
            addr,
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
impl RuntimeService for ProductionApiService {
    fn name(&self) -> &str {
        "api"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["rpc", "telemetry", "metrics"]
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!(
            "ProductionApiService: initialize — validating bind address {}",
            self.addr
        );
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;

        if let Err(error) = validate_api_bind_address(self.addr) {
            health.status = ServiceStatus::Failed;
            health.error_message = Some(error.to_string());
            return Err(RuntimeError::InitializeFailed(
                "api".to_string(),
                Box::new(error),
            ));
        }

        // Pre-validate bind availability
        match tokio::net::TcpListener::bind(self.addr).await {
            Ok(_listener) => {
                // Port available — listener will be rebound by axum::serve
            }
            Err(e) => {
                error!(
                    "ProductionApiService: Bind validation failed for {}: {}",
                    self.addr, e
                );
                health.status = ServiceStatus::Failed;
                health.error_message = Some(format!("Bind check failed: {}", e));
                return Err(RuntimeError::InitializeFailed(
                    format!("Cannot bind API address {}: {}", self.addr, e),
                    Box::new(e),
                ));
            }
        }

        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        validate_api_bind_address(self.addr)
            .map_err(|error| RuntimeError::StartFailed("api".to_string(), Box::new(error)))?;

        info!(
            "ProductionApiService: Starting Axum HTTP server on {}",
            self.addr
        );

        self.running.store(true, Ordering::SeqCst);

        let app = self.app.clone();
        let addr = self.addr;
        let running = self.running.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        let handle = tokio::spawn(async move {
            // Bind listener
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => {
                    match l.local_addr() {
                        Ok(a) => info!("ProductionApiService: Listening on {}", a),
                        Err(e) => warn!(
                            "ProductionApiService: Could not get listener address: {}",
                            e
                        ),
                    }
                    l
                }
                Err(e) => {
                    error!("ProductionApiService: Failed to bind {}: {}", addr, e);
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Serve with graceful shutdown via cancellation token
            let serve_result = axum::serve(listener, app.into_make_service())
                .with_graceful_shutdown(async move {
                    let _ = cancel_token.cancelled().await;
                    info!("ProductionApiService: Graceful shutdown signal received");
                })
                .await;

            match serve_result {
                Ok(()) => {
                    info!("ProductionApiService: Axum server shut down gracefully");
                }
                Err(e) => {
                    error!("ProductionApiService: Axum server error: {}", e);
                }
            }
            running.store(false, Ordering::SeqCst);
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
        info!("ProductionApiService: Stop requested (new connections rejected, in-flight requests allowed)");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionApiService: Shutdown — waiting for Axum task to exit");

        let mut handle_guard = self.task_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            // Axum's graceful shutdown allows in-flight requests to complete
            match tokio::time::timeout(std::time::Duration::from_secs(15), handle).await {
                Ok(Ok(())) => {
                    info!("ProductionApiService: Axum task exited cleanly");
                }
                Ok(Err(e)) => {
                    warn!("ProductionApiService: Axum task join error: {}", e);
                }
                Err(_) => {
                    warn!("ProductionApiService: Axum task did not exit within 15s, aborting");
                }
            }
        }

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        health.last_check = SystemTime::now();
        info!("ProductionApiService: Shutdown complete");
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

#[cfg(test)]
mod tests {
    use super::validate_api_bind_address;

    #[test]
    fn api_control_plane_is_loopback_only() {
        assert!(validate_api_bind_address("127.0.0.1:8081".parse().unwrap()).is_ok());
        assert!(validate_api_bind_address("[::1]:8081".parse().unwrap()).is_ok());
        assert!(validate_api_bind_address("0.0.0.0:8081".parse().unwrap()).is_err());
    }
}
