//! ProductionTelemetryService — RuntimeService wrapping telemetry functions.
//!
//! Zero dependencies. Provides lifecycle management for telemetry access
//! (get_live_metrics, get_consensus_health, get_process_metrics, etc.).
//!
//! # Design
//!
//! Telemetry is purely event-driven — no background monitoring loop.
//! All functions read from the shared GLOBAL_METRICS singleton and sysinfo.
//! This service exists solely to provide a structured lifecycle for the
//! telemetry API surface through RuntimeManager.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production TelemetryService — wraps telemetry functions with RuntimeService lifecycle.
pub struct ProductionTelemetryService {
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
}

impl ProductionTelemetryService {
    /// Create a new TelemetryService.
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    /// Check if the service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for ProductionTelemetryService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeService for ProductionTelemetryService {
    fn name(&self) -> &str {
        "telemetry"
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
        info!("ProductionTelemetryService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionTelemetryService: Starting");
        self.running.store(true, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionTelemetryService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionTelemetryService: Shutdown");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        let mut health = self.health.read().await.clone();
        if self.running.load(Ordering::Relaxed) {
            health.status = ServiceStatus::Running;
        } else {
            health.status = ServiceStatus::Stopped;
        }
        health.last_check = std::time::SystemTime::now();
        health
    }
}
