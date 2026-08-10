//! TelemetryService — Reference RuntimeService with dependency on metrics.
//!
//! Depends on: metrics
//!
//! Validates that the Runtime Manager correctly orders startup/shutdown
//! for dependent services.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info};

use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// A telemetry service that depends on MetricsService.
pub struct TelemetryService {
    running: Arc<AtomicBool>,
    send_count: Arc<AtomicU64>,
    health: Arc<RwLock<ServiceHealth>>,
}

impl TelemetryService {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            send_count: Arc::new(AtomicU64::new(0)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    pub fn send_count(&self) -> u64 {
        self.send_count.load(Ordering::Relaxed)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for TelemetryService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeService for TelemetryService {
    fn name(&self) -> &str {
        "telemetry"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["metrics"]
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(
        &self,
        _ctx: Arc<crate::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        info!("TelemetryService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(
        &self,
        ctx: Arc<crate::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        info!("TelemetryService: Starting telemetry send task");
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let send_count = self.send_count.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(200));
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("TelemetryService: Send task cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        let count = send_count.fetch_add(1, Ordering::Relaxed);
                        if count.is_multiple_of(25) {
                            debug!("TelemetryService: Sent #{}", count + 1);
                        }
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            info!("TelemetryService: Send task exited");
        });

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(
        &self,
        _ctx: Arc<crate::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        info!("TelemetryService: Stop requested");
        self.running.store(false, Ordering::SeqCst);
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(
        &self,
        _ctx: Arc<crate::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        info!("TelemetryService: Shutdown complete");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn health(&self, _ctx: Arc<crate::runtime_context::RuntimeContext>) -> ServiceHealth {
        self.health.read().await.clone()
    }
}
