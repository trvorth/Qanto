//! Qanto Runtime Service Trait & Core Types
//!
//! This module defines the fundamental abstractions for the Qanto Runtime Manager:
//! the `RuntimeService` trait, service lifecycle statuses, health state,
//! restart policies, and runtime errors.
//!
//! Services are compiled into the node — this is NOT a plugin system.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Lifecycle status of a service within the Runtime Manager.
///
/// State machine:
/// ```text
/// NotStarted → Initializing → Starting → Running ⇄ Degraded
///                  ↓              ↓          ↓
///               Failed         Failed     Stopping → Stopped
///                                             ↓
///                                          Failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceStatus {
    /// Registered but not yet initialized.
    NotStarted,
    /// `initialize()` in progress.
    Initializing,
    /// `start()` in progress.
    Starting,
    /// Fully operational.
    Running,
    /// Running but with issues (e.g., partial peer connectivity).
    Degraded,
    /// `stop()` in progress.
    Stopping,
    /// Cleanly stopped after `shutdown()`.
    Stopped,
    /// Error during initialize/start/stop/shutdown.
    Failed,
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ServiceStatus::NotStarted => "NotStarted",
            ServiceStatus::Initializing => "Initializing",
            ServiceStatus::Starting => "Starting",
            ServiceStatus::Running => "Running",
            ServiceStatus::Degraded => "Degraded",
            ServiceStatus::Stopping => "Stopping",
            ServiceStatus::Stopped => "Stopped",
            ServiceStatus::Failed => "Failed",
        };
        write!(f, "{s}")
    }
}

/// Restart policy for a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestartPolicy {
    /// Never auto-restart. Used for consensus-critical services
    /// (Consensus, MiningEngine, Validation, Cryptography).
    Never,
    /// Restart on failure. Used for non-critical services
    /// (RPC, GraphQL, WebSocket, Metrics, Diagnostic).
    OnFailure,
    /// Always restart on any exit. Used for background maintenance jobs
    /// (Checkpoint, Snapshot, State Pruning).
    Always,
}

/// Operational health of a service at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    /// Current lifecycle status.
    pub status: ServiceStatus,
    /// Timestamp of last health check.
    pub last_check: SystemTime,
    /// Total uptime since last start.
    pub uptime: Duration,
    /// Number of restarts since node boot.
    pub restart_count: u32,
    /// Number of panics detected.
    pub panic_count: u32,
    /// Last error message, if any.
    pub error_message: Option<String>,
}

impl ServiceHealth {
    pub fn new(status: ServiceStatus) -> Self {
        Self {
            status,
            last_check: SystemTime::now(),
            uptime: Duration::ZERO,
            restart_count: 0,
            panic_count: 0,
            error_message: None,
        }
    }
}

/// Errors that can occur during runtime management operations.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// A service with this name is already registered.
    #[error("Service already registered: {0}")]
    ServiceAlreadyRegistered(String),

    /// Requested service not found in registry.
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    /// Service initialization failed.
    #[error("Service '{0}' initialize failed: {1}")]
    InitializeFailed(String, #[source] Box<dyn std::error::Error + Send + Sync>),

    /// Service start failed.
    #[error("Service '{0}' start failed: {1}")]
    StartFailed(String, #[source] Box<dyn std::error::Error + Send + Sync>),

    /// Service stop failed.
    #[error("Service '{0}' stop failed: {1}")]
    StopFailed(String, #[source] Box<dyn std::error::Error + Send + Sync>),

    /// Service shutdown failed.
    #[error("Service '{0}' shutdown failed: {1}")]
    ShutdownFailed(String, #[source] Box<dyn std::error::Error + Send + Sync>),

    /// Service panicked during execution.
    #[error("Service '{0}' panicked: {1}")]
    ServicePanic(String, String),

    /// Dependency cycle detected in service graph.
    #[error("Dependency cycle detected involving: {0:?}")]
    DependencyCycle(Vec<String>),

    /// A dependency is missing from the registry.
    #[error("Missing dependency '{dep}' for service '{service}'")]
    MissingDependency { service: String, dep: String },

    /// Invalid state transition attempted.
    #[error("Invalid state transition for '{service}': from {from} to {to}")]
    InvalidStateTransition {
        service: String,
        from: ServiceStatus,
        to: ServiceStatus,
    },

    /// Operation timed out.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Shutdown already in progress.
    #[error("Shutdown already in progress")]
    ShutdownInProgress,

    /// Runtime manager has already been started.
    #[error("Runtime manager already started")]
    RuntimeAlreadyStarted,

    /// Runtime manager has not been started yet.
    #[error("Runtime manager not started")]
    RuntimeNotStarted,

    /// Startup failed — rolling back already-started services.
    #[error("Startup failed at service '{failed_service}': {error}. Rollback of {rolled_back} services attempted.")]
    StartupFailure {
        failed_service: String,
        error: String,
        rolled_back: usize,
    },

    /// One or more services failed during shutdown.
    #[error("Shutdown completed with {failure_count} failure(s): {failures}")]
    ShutdownFailure {
        failure_count: usize,
        failures: String,
    },

    /// Duplicate service detected.
    #[error("Duplicate service: {0}")]
    DuplicateService(String),

    /// Invalid dependency — dependency does not exist.
    #[error("Invalid dependency '{dep}' for service '{service}'")]
    InvalidDependency { service: String, dep: String },

    /// Internal runtime error.
    #[error("Internal runtime error: {0}")]
    Internal(String),
}

/// The core trait for any service managed by the Qanto Runtime Manager.
///
/// Services implement this trait and are registered with the `ServiceRegistry`.
/// The Runtime Manager calls lifecycle methods in dependency order.
///
/// # Lifecycle
///
/// 1. `initialize(&RuntimeContext)` — one-time setup (DB handles, config validation).
/// 2. `start(&RuntimeContext)` — begin background work, spawn tasks, bind ports.
/// 3. `stop(&RuntimeContext)` — signal cancellation, drain queues, flush state.
/// 4. `shutdown(&RuntimeContext)` — final cleanup, release all resources.
///
/// # Determinism
///
/// This trait does NOT affect consensus determinism. It manages operational
/// lifecycle only. Core engines (Consensus, DAG, Mining) are NOT wrapped as
/// services but owned directly by the RuntimeManager.
#[async_trait]
pub trait RuntimeService: Send + Sync + 'static {
    /// Unique service name used for registration and dependency resolution.
    fn name(&self) -> &str;

    /// Semantic version (e.g., "1.0.0").
    fn version(&self) -> &str;

    /// Optional metadata for introspection (description, author, etc.).
    fn metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Services this service depends on (must start before this service).
    /// Returns static references — these are compile-time declarations.
    fn dependencies(&self) -> &[&str] {
        &[]
    }

    /// Restart policy for this service.
    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::Never
    }

    /// One-time initialization before start.
    /// Use for: DB connection verification, config validation, cache warmup.
    async fn initialize(
        &self,
        _ctx: Arc<super::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Start the service's background work.
    /// Spawn Tokio tasks, bind network ports, begin processing loops.
    /// All spawned tasks should respect the `CancellationToken` from `RuntimeContext`.
    async fn start(
        &self,
        ctx: Arc<super::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError>;

    /// Signal the service to stop accepting new work.
    /// Drain queues, flush buffers, cancel internal loops.
    async fn stop(
        &self,
        _ctx: Arc<super::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Final cleanup — release all resources.
    /// Called after `stop()` completes and all supervised tasks have exited.
    async fn shutdown(
        &self,
        _ctx: Arc<super::runtime_context::RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Report current health status. Default implementation returns `Running`.
    /// Services with meaningful health checks should override this.
    async fn health(&self, _ctx: Arc<super::runtime_context::RuntimeContext>) -> ServiceHealth {
        ServiceHealth::new(ServiceStatus::Running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal service for testing the trait machinery.
    struct TestService {
        name: String,
    }

    impl TestService {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl RuntimeService for TestService {
        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        async fn start(
            &self,
            _ctx: Arc<super::super::runtime_context::RuntimeContext>,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    #[test]
    fn test_service_status_display() {
        assert_eq!(ServiceStatus::Running.to_string(), "Running");
        assert_eq!(ServiceStatus::Failed.to_string(), "Failed");
        assert_eq!(ServiceStatus::NotStarted.to_string(), "NotStarted");
    }

    #[test]
    fn test_service_health_defaults() {
        let health = ServiceHealth::new(ServiceStatus::NotStarted);
        assert_eq!(health.status, ServiceStatus::NotStarted);
        assert_eq!(health.restart_count, 0);
        assert_eq!(health.panic_count, 0);
        assert!(health.error_message.is_none());
    }

    #[test]
    fn test_restart_policy_default_is_never() {
        let svc = TestService::new("test");
        assert_eq!(svc.restart_policy(), RestartPolicy::Never);
    }

    #[test]
    fn test_dependencies_default_is_empty() {
        let svc = TestService::new("test");
        assert!(svc.dependencies().is_empty());
    }
}
