//! DiagnosticsService — Reference RuntimeService with no dependencies.
//!
//! Independent service for validation of services with zero dependencies.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info};

use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// A diagnostics service with no dependencies.
pub struct DiagnosticsService {
    running: Arc<AtomicBool>,
    // check_count: Arc<AtomicU64>,
    health: Arc<RwLock<ServiceHealth>>,
}

impl DiagnosticsService {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for DiagnosticsService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeService for DiagnosticsService {
    fn name(&self) -> &str {
        "diagnostics"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &[]
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("DiagnosticsService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("DiagnosticsService: Starting diagnostics loop");
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("DiagnosticsService: Diagnostics loop cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        debug!("DiagnosticsService: Health check tick");
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            info!("DiagnosticsService: Diagnostics loop exited");
        });

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("DiagnosticsService: Stop requested");
        self.running.store(false, Ordering::SeqCst);
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("DiagnosticsService: Shutdown complete");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        self.health.read().await.clone()
    }
}
