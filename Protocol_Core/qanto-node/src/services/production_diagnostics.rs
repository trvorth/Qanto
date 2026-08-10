//! ProductionDiagnosticsService — RuntimeService wrapping DiagnosticsEngine.
//!
//! Zero dependencies. Provides lifecycle management for node diagnostics
//! (mining attempt recording, strata state tracking, snapshots, issue analysis).
//!
//! # Design
//!
//! DiagnosticsService wraps the existing `DiagnosticsEngine` type and exposes
//! its functionality through the RuntimeManager lifecycle. There is no background
//! monitoring loop — diagnostics are purely event-driven (recorded by Core Engines
//! during mining and strata operations).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::diagnostics::DiagnosticsEngine;
use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production DiagnosticsService — wraps DiagnosticsEngine with RuntimeService lifecycle.
pub struct ProductionDiagnosticsService {
    /// Internal diagnostics engine instance.
    engine: Arc<DiagnosticsEngine>,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
}

impl ProductionDiagnosticsService {
    /// Create a new DiagnosticsService.
    pub fn new(debug_mode: bool) -> Self {
        Self {
            engine: Arc::new(DiagnosticsEngine::new(debug_mode)),
            running: Arc::new(AtomicBool::new(false)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    /// Get a reference to the underlying DiagnosticsEngine.
    pub fn engine_ref(&self) -> Arc<DiagnosticsEngine> {
        self.engine.clone()
    }

    /// Check if the service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for ProductionDiagnosticsService {
    fn default() -> Self {
        Self::new(false)
    }
}

#[async_trait]
impl RuntimeService for ProductionDiagnosticsService {
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
        info!("ProductionDiagnosticsService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionDiagnosticsService: Starting");
        self.running.store(true, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionDiagnosticsService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionDiagnosticsService: Shutdown");
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
