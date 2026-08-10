//! FailingService — Service that fails at configurable lifecycle stage.
//!
//! Used to validate:
//! - Transactional startup rollback
//! - Aggregated shutdown error reporting

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Configuration for when the service should fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailAt {
    /// Fail during initialize()
    Initialize,
    /// Fail during start()
    Start,
    /// Fail during stop()
    Stop,
    /// Fail during shutdown()
    Shutdown,
    /// Never fail
    Never,
}

/// A service that fails at a configurable lifecycle stage.
pub struct FailingService {
    name: String,
    fail_at: FailAt,
    running: Arc<AtomicBool>,
    health: Arc<RwLock<ServiceHealth>>,
}

impl FailingService {
    pub fn new(name: &str, fail_at: FailAt) -> Self {
        Self {
            name: name.to_string(),
            fail_at,
            running: Arc::new(AtomicBool::new(false)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Default for FailingService {
    fn default() -> Self {
        Self::new("failing", FailAt::Never)
    }
}

#[async_trait]
impl RuntimeService for FailingService {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &[]
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::Never
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("FailingService '{}': initialize", self.name);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;

        if self.fail_at == FailAt::Initialize {
            error!("FailingService '{}': FAILING at initialize", self.name);
            health.status = ServiceStatus::Failed;
            health.error_message = Some("Intentional failure at initialize".to_string());
            return Err(RuntimeError::InitializeFailed(
                self.name.clone(),
                "Intentional failure at initialize".into(),
            ));
        }
        Ok(())
    }

    async fn start(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("FailingService '{}': start", self.name);
        self.running.store(true, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Starting;

        if self.fail_at == FailAt::Start {
            error!("FailingService '{}': FAILING at start", self.name);
            health.status = ServiceStatus::Failed;
            health.error_message = Some("Intentional failure at start".to_string());
            self.running.store(false, Ordering::SeqCst);
            return Err(RuntimeError::StartFailed(
                self.name.clone(),
                "Intentional failure at start".into(),
            ));
        }

        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("FailingService '{}': stop", self.name);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;

        if self.fail_at == FailAt::Stop {
            error!("FailingService '{}': FAILING at stop", self.name);
            health.error_message = Some("Intentional failure at stop".to_string());
            return Err(RuntimeError::StopFailed(
                self.name.clone(),
                "Intentional failure at stop".into(),
            ));
        }

        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("FailingService '{}': shutdown", self.name);

        let mut health = self.health.write().await;

        if self.fail_at == FailAt::Shutdown {
            error!("FailingService '{}': FAILING at shutdown", self.name);
            health.status = ServiceStatus::Failed;
            health.error_message = Some("Intentional failure at shutdown".to_string());
            return Err(RuntimeError::ShutdownFailed(
                self.name.clone(),
                "Intentional failure at shutdown".into(),
            ));
        }

        health.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        self.health.read().await.clone()
    }
}
