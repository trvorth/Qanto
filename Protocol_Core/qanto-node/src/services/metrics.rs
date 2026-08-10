//! MetricsService — Reference RuntimeService for architectural validation.
//!
//! Zero dependencies. Spawns a background task that periodically logs a
//! heartbeat metric. Validates that the Runtime Manager can start/stop
//! independent services correctly.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

/// A lightweight metrics service for Runtime Manager validation.
pub struct MetricsService {
    running: Arc<AtomicBool>,
    heartbeat_count: Arc<AtomicU64>,
    health: Arc<RwLock<ServiceHealth>>,
}

impl MetricsService {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            heartbeat_count: Arc::new(AtomicU64::new(0)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    /// Get current heartbeat count for validation.
    pub fn heartbeat_count(&self) -> u64 {
        self.heartbeat_count.load(Ordering::Relaxed)
    }

    /// Check if the service's background task is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for MetricsService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeService for MetricsService {
    fn name(&self) -> &str {
        "metrics"
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
        info!("MetricsService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("MetricsService: Starting heartbeat task");
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let heartbeat_count = self.heartbeat_count.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(100));
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("MetricsService: Heartbeat cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        let count = heartbeat_count.fetch_add(1, Ordering::Relaxed);
                        if count.is_multiple_of(50) {
                            debug!("MetricsService: Heartbeat #{}", count + 1);
                        }
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            info!("MetricsService: Heartbeat task exited");
        });

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("MetricsService: Stop requested");
        self.running.store(false, Ordering::SeqCst);
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("MetricsService: Shutdown complete");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        self.health.read().await.clone()
    }
}
