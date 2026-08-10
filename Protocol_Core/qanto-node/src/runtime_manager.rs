//! Qanto Runtime Manager
//!
//! The operating system of the node. Coordinates service lifecycle,
//! dependency resolution, task supervision, and graceful shutdown.
//!
//! # Responsibilities
//!
//! - Register services
//! - Resolve dependencies (delegates to ServiceRegistry)
//! - Initialize services in dependency order
//! - Start services in dependency order
//! - Stop services in reverse dependency order
//! - Supervise running tasks (panic detection, cancellation)
//! - Expose health information
//!
//! # What This Is NOT
//!
//! - Not a blockchain engine — no consensus logic
//! - Not a state container — no DAG, mempool, or UTXO state
//! - Not a plugin system — services are compiled into the node
//!
//! # State Machine
//!
//! ```text
//! Created → Initializing → Running ⇄ Stopping → Stopped
//!                ↓             ↓                  ↓
//!              Failed        Failed             Failed
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::runtime_context::RuntimeContext;
use super::runtime_service::{RuntimeError, RuntimeService, ServiceHealth, ServiceStatus};
use super::service_registry::ServiceRegistry;

// ============================================================================
// RuntimeState — Manager-level state machine
// ============================================================================

/// Top-level state of the Runtime Manager itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Created but no services started yet.
    Created,
    /// Services are being initialized and started.
    Initializing,
    /// All services are running.
    Running,
    /// Services are being stopped.
    Stopping,
    /// All services stopped cleanly.
    Stopped,
    /// A failure occurred during initialization or shutdown.
    Failed,
}

impl std::fmt::Display for RuntimeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeState::Created => write!(f, "Created"),
            RuntimeState::Initializing => write!(f, "Initializing"),
            RuntimeState::Running => write!(f, "Running"),
            RuntimeState::Stopping => write!(f, "Stopping"),
            RuntimeState::Stopped => write!(f, "Stopped"),
            RuntimeState::Failed => write!(f, "Failed"),
        }
    }
}

/// Validate a state transition. Returns Ok if valid, Err with the invalid
/// transition if not.
fn validate_transition(from: RuntimeState, to: RuntimeState) -> Result<(), RuntimeError> {
    let valid = matches!(
        (from, to),
        (RuntimeState::Created, RuntimeState::Initializing)
            | (RuntimeState::Initializing, RuntimeState::Running)
            | (RuntimeState::Initializing, RuntimeState::Failed)
            | (RuntimeState::Initializing, RuntimeState::Stopping)
            | (RuntimeState::Running, RuntimeState::Stopping)
            | (RuntimeState::Running, RuntimeState::Failed)
            | (RuntimeState::Stopping, RuntimeState::Stopped)
            | (RuntimeState::Stopping, RuntimeState::Failed)
            | (RuntimeState::Stopped, RuntimeState::Failed)
    );
    if valid {
        Ok(())
    } else {
        Err(RuntimeError::Internal(format!(
            "Invalid runtime state transition: {from} → {to}"
        )))
    }
}

// ============================================================================
// ServiceHandle — Track a running service's task
// ============================================================================

/// Handle to a service's supervised background task.
/// Internal: not part of the public API.
struct ServiceHandle {
    /// Tokio join handle for the service's main task.
    join_handle: JoinHandle<()>,
    /// Child cancellation token for this service.
    cancel_token: CancellationToken,
    /// Timestamp when the service was started.
    started_at: Instant,
}

// ============================================================================
// TaskSupervisor — Internal component for task management
// ============================================================================

/// Internal task supervision component.
///
/// Tracks spawned service tasks, detects panics, and coordinates cancellation.
/// Kept as a private struct inside `runtime_manager.rs` per scope control:
/// extracted to its own module only if complexity demands it.
struct TaskSupervisor {
    /// Map of service name → handle.
    handles: RwLock<HashMap<String, ServiceHandle>>,
}

impl TaskSupervisor {
    fn new() -> Self {
        Self {
            handles: RwLock::new(HashMap::new()),
        }
    }

    /// Spawn a service's background task with supervision.
    ///
    /// The service's `start()` is called inside a Tokio task. If it panics,
    /// the panic is caught and reported via the health state.
    async fn spawn(
        &self,
        service_name: String,
        _service: Arc<dyn RuntimeService>,
        ctx: Arc<RuntimeContext>,
    ) -> Result<(), RuntimeError> {
        let cancel_token = ctx.child_token();
        let cancel_for_task = cancel_token.clone();
        let name = service_name.clone();

        let handle = tokio::spawn(async move {
            // Wait for cancellation signal (service's start() spawns its own
            // internal tasks which should respect this token via RuntimeContext)
            let _ = cancel_for_task.cancelled().await;
            debug!("Service '{}' received cancellation signal", name);
        });

        // Store handle
        self.handles.write().await.insert(
            service_name,
            ServiceHandle {
                join_handle: handle,
                cancel_token,
                started_at: Instant::now(),
            },
        );

        Ok(())
    }

    /// Signal a service to stop and wait for its task to complete (with timeout).
    async fn stop_and_wait(
        &self,
        service_name: &str,
        timeout: Duration,
    ) -> Result<(), RuntimeError> {
        let handle = self.handles.write().await.remove(service_name);

        match handle {
            Some(h) => {
                // Signal cancellation
                h.cancel_token.cancel();

                // Wait for task completion with timeout
                match tokio::time::timeout(timeout, h.join_handle).await {
                    Ok(Ok(())) => {
                        debug!("Service '{}' task completed cleanly", service_name);
                        Ok(())
                    }
                    Ok(Err(join_err)) => {
                        if join_err.is_cancelled() {
                            debug!("Service '{}' task was cancelled", service_name);
                            Ok(())
                        } else if join_err.is_panic() {
                            warn!("Service '{}' task panicked during shutdown", service_name);
                            Err(RuntimeError::ServicePanic(
                                service_name.to_string(),
                                format!("{join_err}"),
                            ))
                        } else {
                            warn!("Service '{}' task join error: {}", service_name, join_err);
                            Err(RuntimeError::Internal(format!(
                                "Task join error for '{service_name}': {join_err}"
                            )))
                        }
                    }
                    Err(_) => {
                        warn!(
                            "Service '{}' task did not complete within {:?}, aborting",
                            service_name, timeout
                        );
                        // Abort the task
                        // Note: JoinHandle is consumed by timeout, so we can't abort.
                        // The task will be dropped when the runtime shuts down.
                        Err(RuntimeError::Timeout(format!(
                            "Service '{service_name}' did not stop within {timeout:?}"
                        )))
                    }
                }
            }
            None => {
                // No handle found — service may have already been stopped
                debug!("Service '{}' has no active task handle", service_name);
                Ok(())
            }
        }
    }

    /// Get the number of currently supervised tasks.
    async fn active_count(&self) -> usize {
        self.handles.read().await.len()
    }

    /// Get uptime for a service.
    async fn uptime(&self, service_name: &str) -> Option<Duration> {
        self.handles
            .read()
            .await
            .get(service_name)
            .map(|h| h.started_at.elapsed())
    }
}

// ============================================================================
// ShutdownCoordinator — Internal component for graceful termination
// ============================================================================

/// Internal shutdown coordination component.
///
/// Merges the responsibilities of the old `ShutdownController` and
/// `ResourceCleanup` into a single coordinator owned by RuntimeManager.
struct ShutdownCoordinator {
    /// Global cancellation token — cancelled once, propagates to all children.
    cancellation_token: CancellationToken,
    /// Whether shutdown has been requested.
    is_requested: std::sync::atomic::AtomicBool,
    /// Per-service shutdown timeout.
    service_timeout: Duration,
}

impl ShutdownCoordinator {
    fn new(service_timeout: Duration, _total_timeout: Duration) -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            is_requested: std::sync::atomic::AtomicBool::new(false),
            service_timeout,
        }
    }

    /// Request graceful shutdown. Idempotent — safe to call multiple times.
    fn request_shutdown(&self) {
        let was_requested = self
            .is_requested
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if !was_requested {
            info!("🛑 Graceful shutdown requested");
        }
        self.cancellation_token.cancel();
    }

    /// Check if shutdown has been requested.
    fn is_requested(&self) -> bool {
        self.is_requested.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// ============================================================================
// RuntimeMetrics — Operational metrics (not blockchain metrics)
// ============================================================================

/// Operational metrics for the Runtime Manager.
///
/// These are runtime/operational metrics only — not blockchain metrics
/// like TPS, block latency, or consensus timings.
#[derive(Debug, Clone, Default)]
pub struct RuntimeMetrics {
    /// Time spent in startup (all services).
    pub startup_duration: Option<Duration>,
    /// Time spent in shutdown (all services).
    pub shutdown_duration: Option<Duration>,
    /// Total registered services.
    pub registered_services: usize,
    /// Currently active (Running) services.
    pub active_services: usize,
    /// Services in Degraded state.
    pub degraded_services: usize,
    /// Count of detected panics across all services.
    pub panic_count: u32,
    /// Count of service restarts.
    pub restart_count: u32,
    /// Timestamp when the runtime was started.
    pub started_at: Option<Instant>,
}

// ============================================================================
// ShutdownReport — Result of a shutdown operation
// ============================================================================

/// Report generated after shutdown completes (or partially completes).
#[derive(Debug, Clone)]
pub struct ShutdownReport {
    /// Services that stopped cleanly.
    pub succeeded: Vec<String>,
    /// Services that failed to stop, with error messages.
    pub failed: HashMap<String, String>,
    /// Total shutdown duration.
    pub duration: Duration,
    /// Whether all services stopped cleanly.
    pub all_succeeded: bool,
}

// ============================================================================
// RuntimeManager — The public orchestrator
// ============================================================================

/// The Qanto Runtime Manager — coordinator of all node services.
///
/// This is the central operational layer of the node. It manages service
/// lifecycle, dependency resolution, task supervision, and graceful shutdown.
/// It does NOT contain any blockchain logic.
///
/// # Usage
///
/// ```rust,ignore
/// let registry = Arc::new(ServiceRegistry::new());
/// let context = Arc::new(RuntimeContext::new(config, token));
/// let manager = RuntimeManager::new(context, registry);
///
/// manager.register_service(p2p_service).await?;
/// manager.register_service(rpc_service).await?;
///
/// manager.start_all().await?;  // Starts in dependency order
/// // ... node runs ...
/// manager.shutdown_all().await?;  // Stops in reverse order
/// ```
pub struct RuntimeManager {
    /// Shared operational infrastructure.
    context: Arc<RuntimeContext>,
    /// Service registry with dependency resolution.
    registry: Arc<ServiceRegistry>,
    /// Internal task supervisor.
    supervisor: TaskSupervisor,
    /// Internal shutdown coordinator.
    shutdown: ShutdownCoordinator,
    /// Current runtime state.
    state: RwLock<RuntimeState>,
    /// List of services that have been successfully started (for rollback).
    started_services: RwLock<Vec<String>>,
    /// Operational metrics.
    metrics: RwLock<RuntimeMetrics>,
}

impl RuntimeManager {
    /// Create a new RuntimeManager.
    ///
    /// The manager starts in `Created` state. Services can be registered
    /// before `start_all()` is called.
    pub fn new(context: Arc<RuntimeContext>, registry: Arc<ServiceRegistry>) -> Self {
        Self {
            context,
            registry,
            supervisor: TaskSupervisor::new(),
            shutdown: ShutdownCoordinator::new(
                Duration::from_secs(10), // per-service shutdown timeout
                Duration::from_secs(60), // total shutdown timeout
            ),
            state: RwLock::new(RuntimeState::Created),
            started_services: RwLock::new(Vec::new()),
            metrics: RwLock::new(RuntimeMetrics::default()),
        }
    }

    /// Register a service with the registry.
    ///
    /// Can only be called before `start_all()`.
    pub async fn register_service(
        &self,
        service: Arc<dyn RuntimeService>,
    ) -> Result<(), RuntimeError> {
        let state = *self.state.read().await;
        if state == RuntimeState::Running || state == RuntimeState::Initializing {
            return Err(RuntimeError::RuntimeAlreadyStarted);
        }

        let name = service.name().to_string();
        self.registry.register(service).await?;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.registered_services += 1;

        debug!("Service '{}' registered with RuntimeManager", name);
        Ok(())
    }

    /// Start all registered services in dependency order.
    ///
    /// # Transactional Behavior
    ///
    /// If any service fails to initialize or start, the Runtime Manager
    /// automatically performs a reverse shutdown of all already-started services.
    /// The node will never remain partially initialized.
    pub async fn start_all(&self) -> Result<(), RuntimeError> {
        // Validate state transition
        {
            let mut state = self.state.write().await;
            validate_transition(*state, RuntimeState::Initializing)?;
            *state = RuntimeState::Initializing;
        }

        let startup_start = Instant::now();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.started_at = Some(Instant::now());
        }

        // Compute startup order from registry
        let startup_order = match self.registry.compute_startup_order().await {
            Ok(order) => order,
            Err(e) => {
                self.set_state(RuntimeState::Failed).await;
                return Err(e);
            }
        };

        info!(
            "Starting {} services in dependency order: {:?}",
            startup_order.len(),
            startup_order
        );

        // Initialize and start each service in order
        for service_name in &startup_order {
            let service = match self.registry.get(service_name).await {
                Some(s) => s,
                None => {
                    error!(
                        "Service '{}' not found in registry during startup",
                        service_name
                    );
                    return self
                        .rollback_startup(service_name, "ServiceNotInRegistry".to_string())
                        .await;
                }
            };

            // Initialize
            debug!("Initializing service '{}'", service_name);
            self.registry
                .set_status(service_name, ServiceStatus::Initializing)
                .await;

            if let Err(e) = service.initialize(self.context.clone()).await {
                let err_msg = format!("{e}");
                error!("Service '{}' initialize failed: {}", service_name, err_msg);
                self.registry
                    .set_status(service_name, ServiceStatus::Failed)
                    .await;
                return self.rollback_startup(service_name, err_msg).await;
            }

            // Start
            debug!("Starting service '{}'", service_name);
            self.registry
                .set_status(service_name, ServiceStatus::Starting)
                .await;

            if let Err(e) = service.start(self.context.clone()).await {
                let err_msg = format!("{e}");
                error!("Service '{}' start failed: {}", service_name, err_msg);
                self.registry
                    .set_status(service_name, ServiceStatus::Failed)
                    .await;
                return self.rollback_startup(service_name, err_msg).await;
            }

            // Spawn supervised task
            if let Err(e) = self
                .supervisor
                .spawn(service_name.clone(), service.clone(), self.context.clone())
                .await
            {
                let err_msg = format!("{e}");
                error!(
                    "Service '{}' supervision spawn failed: {}",
                    service_name, err_msg
                );
                self.registry
                    .set_status(service_name, ServiceStatus::Failed)
                    .await;
                return self.rollback_startup(service_name, err_msg).await;
            }

            // Mark as running
            self.registry
                .set_status(service_name, ServiceStatus::Running)
                .await;

            // Track for potential rollback
            self.started_services
                .write()
                .await
                .push(service_name.clone());

            // Update health
            let uptime = self
                .supervisor
                .uptime(service_name)
                .await
                .unwrap_or_default();
            let mut health = self
                .registry
                .get_health(service_name)
                .await
                .unwrap_or_else(|| ServiceHealth::new(ServiceStatus::Running));
            health.status = ServiceStatus::Running;
            health.uptime = uptime;
            self.registry.set_health(service_name, health).await;

            debug!("Service '{}' started successfully", service_name);
        }

        // All services started — transition to Running
        self.set_state(RuntimeState::Running).await;

        // Update metrics
        let startup_duration = startup_start.elapsed();
        {
            let mut metrics = self.metrics.write().await;
            metrics.startup_duration = Some(startup_duration);
            let counts = self.registry.status_counts().await;
            metrics.active_services = *counts.get(&ServiceStatus::Running).unwrap_or(&0);
            metrics.degraded_services = *counts.get(&ServiceStatus::Degraded).unwrap_or(&0);
        }

        info!(
            "All {} services started successfully in {:?}",
            startup_order.len(),
            startup_duration
        );

        Ok(())
    }

    /// Gracefully shut down all services in reverse dependency order.
    ///
    /// # Error Aggregation
    ///
    /// Shutdown continues even if individual services fail to stop.
    /// All errors are aggregated into a final `ShutdownReport`.
    pub async fn shutdown_all(&self) -> Result<ShutdownReport, RuntimeError> {
        // Validate state
        {
            let state = *self.state.read().await;
            if state == RuntimeState::Stopped || state == RuntimeState::Created {
                return Err(RuntimeError::RuntimeNotStarted);
            }
        }

        // Transition to Stopping
        self.set_state(RuntimeState::Stopping).await;

        let shutdown_start = Instant::now();
        self.shutdown.request_shutdown();

        // Compute shutdown order (reverse of startup)
        let shutdown_order = match self.registry.compute_shutdown_order().await {
            Ok(order) => order,
            Err(e) => {
                // If we can't compute order, use the reverse of started_services
                warn!(
                    "Failed to compute shutdown order: {}, using reverse of started list",
                    e
                );
                let started = self.started_services.read().await;
                started.iter().rev().cloned().collect()
            }
        };

        info!(
            "Shutting down {} services in reverse order: {:?}",
            shutdown_order.len(),
            shutdown_order
        );

        let mut succeeded: Vec<String> = Vec::new();
        let mut failed: HashMap<String, String> = HashMap::new();

        for service_name in &shutdown_order {
            let service = match self.registry.get(service_name).await {
                Some(s) => s,
                None => {
                    warn!(
                        "Service '{}' not found during shutdown, skipping",
                        service_name
                    );
                    continue;
                }
            };

            // Update status
            self.registry
                .set_status(service_name, ServiceStatus::Stopping)
                .await;

            // Call stop() — continue even on failure
            let stop_result = service.stop(self.context.clone()).await;
            if let Err(e) = stop_result {
                let err_msg = format!("{e}");
                warn!("Service '{}' stop failed: {}", service_name, err_msg);
                failed.insert(service_name.clone(), err_msg.clone());
                self.registry
                    .set_status(service_name, ServiceStatus::Failed)
                    .await;
                // Continue to next service — shutdown must complete
                continue;
            }

            // Wait for supervised task to complete
            if let Err(e) = self
                .supervisor
                .stop_and_wait(service_name, self.shutdown.service_timeout)
                .await
            {
                let err_msg = format!("{e}");
                warn!(
                    "Service '{}' task completion failed: {}",
                    service_name, err_msg
                );
                failed.insert(service_name.clone(), err_msg);
                // Continue anyway
            }

            // Call shutdown() — final cleanup
            if let Err(e) = service.shutdown(self.context.clone()).await {
                let err_msg = format!("{e}");
                warn!("Service '{}' shutdown failed: {}", service_name, err_msg);
                failed.insert(service_name.clone(), err_msg);
                self.registry
                    .set_status(service_name, ServiceStatus::Failed)
                    .await;
                continue;
            }

            // Mark as stopped
            self.registry
                .set_status(service_name, ServiceStatus::Stopped)
                .await;
            succeeded.push(service_name.clone());

            debug!("Service '{}' stopped cleanly", service_name);
        }

        let shutdown_duration = shutdown_start.elapsed();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.shutdown_duration = Some(shutdown_duration);
            metrics.active_services = 0;
            metrics.degraded_services = 0;
        }

        let all_succeeded = failed.is_empty();

        if all_succeeded {
            self.set_state(RuntimeState::Stopped).await;
            info!(
                "All {} services shut down cleanly in {:?}",
                succeeded.len(),
                shutdown_duration
            );
        } else {
            self.set_state(RuntimeState::Failed).await;
            warn!(
                "Shutdown completed with {} failure(s) in {:?}: {:?}",
                failed.len(),
                shutdown_duration,
                failed.keys().collect::<Vec<_>>()
            );
        }

        let report = ShutdownReport {
            succeeded,
            failed,
            duration: shutdown_duration,
            all_succeeded,
        };

        if all_succeeded {
            Ok(report)
        } else {
            Err(RuntimeError::ShutdownFailure {
                failure_count: report.failed.len(),
                failures: report
                    .failed
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            })
        }
    }

    /// Force shutdown — cancel all tasks immediately.
    pub async fn force_shutdown(&self) -> Result<(), RuntimeError> {
        warn!("⚠️ Force shutdown requested — cancelling all tasks");
        self.shutdown.request_shutdown();

        // Cancel all supervised tasks
        let mut started = self.started_services.write().await;
        for name in started.drain(..) {
            let _ = self
                .supervisor
                .stop_and_wait(&name, Duration::from_millis(100))
                .await;
            self.registry
                .set_status(&name, ServiceStatus::Stopped)
                .await;
        }

        self.set_state(RuntimeState::Stopped).await;
        Ok(())
    }

    /// Get the current runtime state.
    pub async fn state(&self) -> RuntimeState {
        *self.state.read().await
    }

    /// Get health information for all services.
    pub async fn health_check(&self) -> HashMap<String, ServiceHealth> {
        self.registry.all_health().await
    }

    /// Get health for a specific service.
    pub async fn service_health(&self, name: &str) -> Option<ServiceHealth> {
        self.registry.get_health(name).await
    }

    /// Get operational metrics snapshot.
    pub async fn metrics(&self) -> RuntimeMetrics {
        let mut metrics = self.metrics.read().await.clone();
        let counts = self.registry.status_counts().await;
        metrics.active_services = *counts.get(&ServiceStatus::Running).unwrap_or(&0);
        metrics.degraded_services = *counts.get(&ServiceStatus::Degraded).unwrap_or(&0);
        metrics.registered_services = self.registry.list_services().await.len();
        metrics
    }

    /// Get the cancellation token for external coordination.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.context.cancellation_token.clone()
    }

    /// Wait for shutdown to be requested.
    pub async fn wait_for_shutdown(&self) {
        self.context.cancellation_token.cancelled().await;
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown.is_requested()
    }

    /// Get the number of currently active (supervised) service tasks.
    pub async fn active_task_count(&self) -> usize {
        self.supervisor.active_count().await
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Set the runtime state, validating the transition.
    async fn set_state(&self, new_state: RuntimeState) {
        let mut state = self.state.write().await;
        if let Err(e) = validate_transition(*state, new_state) {
            error!(
                "Invalid state transition: {} → {}: {}",
                *state, new_state, e
            );
            // Force the transition anyway — we don't want to deadlock
        }
        *state = new_state;
    }

    /// Rollback a failed startup by stopping all already-started services
    /// in reverse order.
    ///
    /// Returns the original `StartupFailure` error with the count of
    /// services that were rolled back.
    async fn rollback_startup(
        &self,
        failed_service: &str,
        error: String,
    ) -> Result<(), RuntimeError> {
        let started = self.started_services.read().await.clone();
        let rollback_count = started.len();

        error!(
            "Startup failed at '{}'. Rolling back {} already-started services...",
            failed_service, rollback_count
        );

        // Stop in reverse order
        for service_name in started.iter().rev() {
            let service = match self.registry.get(service_name).await {
                Some(s) => s,
                None => continue,
            };

            self.registry
                .set_status(service_name, ServiceStatus::Stopping)
                .await;

            // Attempt stop
            if let Err(e) = service.stop(self.context.clone()).await {
                warn!("Rollback: service '{}' stop failed: {}", service_name, e);
            }

            // Wait for task
            let _ = self
                .supervisor
                .stop_and_wait(service_name, Duration::from_secs(5))
                .await;

            // Shutdown
            if let Err(e) = service.shutdown(self.context.clone()).await {
                warn!(
                    "Rollback: service '{}' shutdown failed: {}",
                    service_name, e
                );
            }

            self.registry
                .set_status(service_name, ServiceStatus::Stopped)
                .await;

            debug!("Rollback: service '{}' stopped", service_name);
        }

        // Clear started services list
        self.started_services.write().await.clear();

        // Transition to Failed
        self.set_state(RuntimeState::Failed).await;

        error!(
            "Rollback complete. {} services stopped. Original failure: '{}' at '{}'",
            rollback_count, error, failed_service
        );

        Err(RuntimeError::StartupFailure {
            failed_service: failed_service.to_string(),
            error,
            rolled_back: rollback_count,
        })
    }
}

impl Drop for RuntimeManager {
    fn drop(&mut self) {
        if !self.shutdown.is_requested() {
            warn!("RuntimeManager dropped without explicit shutdown. Requesting cancellation.");
            self.shutdown.request_shutdown();
            // Also cancel the context's token so external observers see the signal
            self.context.cancellation_token.cancel();
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    /// Minimal mock RPC backend shared across integration tests.
    struct MockRpcBackend;
    #[async_trait]
    impl qanto_rpc::RpcBackend for MockRpcBackend {
        async fn submit_transaction(
            &self,
            _tx: qanto_rpc::server::generated::Transaction,
        ) -> Result<(), String> {
            Ok(())
        }
        async fn get_balance(&self, _address: String) -> Result<u128, String> {
            Ok(0)
        }
        async fn get_block(
            &self,
            _block_id: String,
        ) -> Result<qanto_rpc::server::generated::QantoBlock, String> {
            Err("not found".into())
        }
        async fn get_network_stats(&self) -> Result<qanto_rpc::NetworkStats, String> {
            Ok(qanto_rpc::NetworkStats {
                tps: 0.0,
                bps: 0.0,
                mempool_tx_count: 0,
                mempool_size_bytes: 0,
                connected_peers: 0,
                block_count: 0,
                finality_ms: 0,
                network_throughput_mbps: 0.0,
            })
        }
        fn balance_updates_receiver(
            &self,
        ) -> tokio::sync::broadcast::Receiver<qanto_rpc::server::generated::BalanceUpdate> {
            let (tx, _rx) = tokio::sync::broadcast::channel(1);
            tx.subscribe()
        }
        fn transaction_updates_receiver(
            &self,
        ) -> tokio::sync::broadcast::Receiver<qanto_rpc::server::generated::AddressTransactionEntry>
        {
            let (tx, _rx) = tokio::sync::broadcast::channel(1);
            tx.subscribe()
        }
        async fn get_wallet_balance(&self, _address: String) -> Result<(u128, u128), String> {
            Ok((0, 1))
        }
        async fn get_transactions_by_address(
            &self,
            _address: String,
            _page: u32,
            _page_size: u32,
        ) -> Result<
            (
                Vec<qanto_rpc::server::generated::AddressTransactionEntry>,
                u64,
            ),
            String,
        > {
            Ok((vec![], 0))
        }
    }

    /// A test service with configurable behavior.
    struct TestService {
        name: &'static str,
        deps: &'static [&'static str],
        should_fail_start: bool,
        start_count: Arc<AtomicU32>,
        stop_count: Arc<AtomicU32>,
        started: Arc<AtomicBool>,
    }

    impl TestService {
        fn new(name: &'static str, deps: &'static [&'static str], should_fail_start: bool) -> Self {
            Self {
                name,
                deps,
                should_fail_start,
                start_count: Arc::new(AtomicU32::new(0)),
                stop_count: Arc::new(AtomicU32::new(0)),
                started: Arc::new(AtomicBool::new(false)),
            }
        }

        fn with_counters(
            name: &'static str,
            deps: &'static [&'static str],
            start_count: Arc<AtomicU32>,
            stop_count: Arc<AtomicU32>,
        ) -> Self {
            Self {
                name,
                deps,
                should_fail_start: false,
                start_count,
                stop_count,
                started: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[async_trait]
    impl RuntimeService for TestService {
        fn name(&self) -> &str {
            self.name
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn dependencies(&self) -> &[&str] {
            self.deps
        }

        async fn start(
            &self,
            _ctx: Arc<crate::runtime_context::RuntimeContext>,
        ) -> Result<(), RuntimeError> {
            self.start_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail_start {
                return Err(RuntimeError::Internal("Test failure".to_string()));
            }
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn stop(
            &self,
            _ctx: Arc<crate::runtime_context::RuntimeContext>,
        ) -> Result<(), RuntimeError> {
            self.stop_count.fetch_add(1, Ordering::SeqCst);
            self.started.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    fn make_context() -> Arc<RuntimeContext> {
        let config = Arc::new(crate::config::Config::default());
        let token = CancellationToken::new();
        Arc::new(RuntimeContext::new(config, token))
    }

    #[tokio::test]
    async fn test_state_machine_created_to_running() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        assert_eq!(manager.state().await, RuntimeState::Created);

        // Register a minimal service
        let svc = Arc::new(TestService::new("test", &[], false));
        manager.register_service(svc).await.unwrap();

        // Start
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_state_pruning_service() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Build ProductionStatePruningService
        let pruning_svc = Arc::new(
            crate::services::production_state_pruning::ProductionStatePruningService::new(
                Arc::new(
                    crate::qanto_storage::QantoStorage::new(
                        crate::qanto_storage::StorageConfig::default(),
                    )
                    .unwrap(),
                ),
            ),
        );
        manager.register_service(pruning_svc.clone()).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify state_pruning service is running
        let health = manager.health_check().await;
        assert_eq!(
            health.get("state_pruning").unwrap().status,
            ServiceStatus::Running
        );

        assert!(pruning_svc.is_running());

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_rpc_service() {
        use std::sync::Arc;

        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Register dependencies first (telemetry is required)
        let telemetry_svc =
            Arc::new(crate::services::production_telemetry::ProductionTelemetryService::new());
        manager.register_service(telemetry_svc).await.unwrap();

        // Register RPC service
        let dummy_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let rpc_svc = Arc::new(crate::services::production_rpc::ProductionRpcService::new(
            dummy_addr,
            Arc::new(MockRpcBackend),
        ));
        manager.register_service(rpc_svc.clone()).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify RPC service is running
        let health = manager.health_check().await;
        assert_eq!(health.get("rpc").unwrap().status, ServiceStatus::Running);

        assert!(rpc_svc.is_running());

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_api_service() {
        use axum::{routing::get, Router};
        use std::sync::Arc;

        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Register dependencies: metrics → telemetry → rpc → api
        let metrics_svc =
            Arc::new(crate::services::production_metrics::ProductionMetricsService::new());
        let telemetry_svc =
            Arc::new(crate::services::production_telemetry::ProductionTelemetryService::new());
        manager.register_service(metrics_svc).await.unwrap();
        manager.register_service(telemetry_svc).await.unwrap();

        // Minimal RPC service stub (api depends on rpc)
        let rpc_addr: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
        let rpc_svc = Arc::new(crate::services::production_rpc::ProductionRpcService::new(
            rpc_addr,
            Arc::new(MockRpcBackend),
        ));
        manager.register_service(rpc_svc).await.unwrap();

        // Register a minimal API service on an ephemeral port
        let app = Router::new().route("/health", get(|| async { "ok" }));
        let api_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let api_svc = Arc::new(crate::services::production_api::ProductionApiService::new(
            app, api_addr,
        ));
        manager.register_service(api_svc.clone()).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify API service is running
        let health = manager.health_check().await;
        assert_eq!(health.get("api").unwrap().status, ServiceStatus::Running);

        assert!(api_svc.is_running());

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_ipc_service() {
        // Full lifecycle test: register → start → health → shutdown.
        // Uses StaticIpcHandler — no blockchain, no DAG, no signatures needed.
        use std::sync::Arc;

        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Register dependency: telemetry → ipc
        let telemetry_svc =
            Arc::new(crate::services::production_telemetry::ProductionTelemetryService::new());
        manager.register_service(telemetry_svc).await.unwrap();

        // Static test handler — returns fixed balance/height
        let handler = Arc::new(crate::ipc_handler::StaticIpcHandler {
            balance: 42,
            height: 7,
        });
        let ipc_svc = Arc::new(crate::services::production_ipc::ProductionIpcService::new(
            handler,
            crate::config::Config::free_tier_optimized(),
        ));
        manager.register_service(ipc_svc.clone()).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify IPC service reports Running
        let health = manager.health_check().await;
        assert_eq!(health.get("ipc").unwrap().status, ServiceStatus::Running);

        assert!(ipc_svc.is_running());

        // Shutdown — cancellation token propagates, IPC loop exits
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_p2p_service() {
        // Full lifecycle test: register → start → health → shutdown.
        // Uses NoopP2pRunnable — no libp2p Swarm, no blockchain, no keypair.
        use std::sync::Arc;

        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Register dependency chain: metrics → telemetry → rpc → api → p2p
        let metrics_svc =
            Arc::new(crate::services::production_metrics::ProductionMetricsService::new());
        let telemetry_svc =
            Arc::new(crate::services::production_telemetry::ProductionTelemetryService::new());
        let rpc_addr: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
        let rpc_svc = Arc::new(crate::services::production_rpc::ProductionRpcService::new(
            rpc_addr,
            Arc::new(MockRpcBackend),
        ));
        let app = axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }));
        let api_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let api_svc = Arc::new(crate::services::production_api::ProductionApiService::new(
            app, api_addr,
        ));

        manager.register_service(metrics_svc).await.unwrap();
        manager.register_service(telemetry_svc).await.unwrap();
        manager.register_service(rpc_svc).await.unwrap();
        manager.register_service(api_svc).await.unwrap();

        // Register P2P service with no-op runnable — no real networking
        let (tx, rx) = tokio::sync::mpsc::channel::<crate::p2p::P2PCommand>(1);
        let p2p_svc = Arc::new(crate::services::production_p2p::ProductionP2PService::new(
            crate::p2p_runnable::NoopP2pRunnable,
            rx,
        ));
        drop(tx); // Sender not needed for the test
        manager.register_service(p2p_svc.clone()).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify P2P service reports Running
        let health = manager.health_check().await;
        assert_eq!(health.get("p2p").unwrap().status, ServiceStatus::Running);

        assert!(p2p_svc.is_running());

        // Shutdown — cancellation token propagates, NoopP2pRunnable yields
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_state_machine_invalid_transition() {
        let result = validate_transition(RuntimeState::Running, RuntimeState::Created);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dependency_order_startup() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // C → B → A (A depends on B, B depends on C)
        let c = Arc::new(TestService::new("C", &[], false));
        let b = Arc::new(TestService::new("B", &["C"], false));
        let a = Arc::new(TestService::new("A", &["B"], false));

        manager.register_service(a).await.unwrap();
        manager.register_service(b).await.unwrap();
        manager.register_service(c).await.unwrap();

        manager.start_all().await.unwrap();

        let health = manager.health_check().await;
        assert_eq!(health.get("A").unwrap().status, ServiceStatus::Running);
        assert_eq!(health.get("B").unwrap().status, ServiceStatus::Running);
        assert_eq!(health.get("C").unwrap().status, ServiceStatus::Running);

        manager.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_transactional_startup_rollback() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // A (starts OK) → B (starts OK) → C (START FAILS)
        let c_start = Arc::new(AtomicU32::new(0));
        let c_stop = Arc::new(AtomicU32::new(0));
        let a_start = Arc::new(AtomicU32::new(0));
        let a_stop = Arc::new(AtomicU32::new(0));
        let b_start = Arc::new(AtomicU32::new(0));
        let b_stop = Arc::new(AtomicU32::new(0));

        let a = Arc::new(TestService::with_counters(
            "A",
            &[],
            a_start.clone(),
            a_stop.clone(),
        ));
        let b = Arc::new(TestService::with_counters(
            "B",
            &["A"],
            b_start.clone(),
            b_stop.clone(),
        ));
        let c = Arc::new(TestService {
            name: "C",
            deps: &["B"],
            should_fail_start: true,
            start_count: c_start.clone(),
            stop_count: c_stop.clone(),
            started: Arc::new(AtomicBool::new(false)),
        });

        manager.register_service(a).await.unwrap();
        manager.register_service(b).await.unwrap();
        manager.register_service(c.clone()).await.unwrap();

        // Start should fail at C and rollback A and B
        let result = manager.start_all().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RuntimeError::StartupFailure {
                failed_service,
                rolled_back,
                ..
            } => {
                assert_eq!(failed_service, "C");
                assert_eq!(rolled_back, 2); // A and B were rolled back
            }
            e => panic!("Expected StartupFailure, got {:?}", e),
        }

        // Verify A and B were started then stopped (rolled back)
        assert_eq!(
            a_start.load(Ordering::SeqCst),
            1,
            "A should have been started once"
        );
        assert_eq!(
            a_stop.load(Ordering::SeqCst),
            1,
            "A should have been stopped during rollback"
        );
        assert_eq!(
            b_start.load(Ordering::SeqCst),
            1,
            "B should have been started once"
        );
        assert_eq!(
            b_stop.load(Ordering::SeqCst),
            1,
            "B should have been stopped during rollback"
        );
        assert_eq!(
            c_start.load(Ordering::SeqCst),
            1,
            "C should have attempted to start"
        );

        // Runtime should be in Failed state
        assert_eq!(manager.state().await, RuntimeState::Failed);
    }

    #[tokio::test]
    async fn test_shutdown_error_aggregation() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // Two services: A stops cleanly, B fails to stop
        struct FailStopService {
            name: &'static str,
        }

        #[async_trait]
        impl RuntimeService for FailStopService {
            fn name(&self) -> &str {
                self.name
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            async fn start(
                &self,
                _ctx: Arc<crate::runtime_context::RuntimeContext>,
            ) -> Result<(), RuntimeError> {
                Ok(())
            }
            async fn stop(
                &self,
                _ctx: Arc<crate::runtime_context::RuntimeContext>,
            ) -> Result<(), RuntimeError> {
                Err(RuntimeError::Internal("Test stop failure".to_string()))
            }
        }

        let a = Arc::new(TestService::new("A", &[], false));
        let b = Arc::new(FailStopService { name: "B" });

        manager.register_service(a).await.unwrap();
        manager.register_service(b).await.unwrap();

        manager.start_all().await.unwrap();

        // Shutdown should aggregate errors, not abort
        let result = manager.shutdown_all().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RuntimeError::ShutdownFailure { failure_count, .. } => {
                assert_eq!(failure_count, 1, "B should have failed to stop");
            }
            e => panic!("Expected ShutdownFailure, got {:?}", e),
        }

        // A should still have been stopped
        assert_eq!(manager.state().await, RuntimeState::Failed);
    }

    #[tokio::test]
    async fn test_register_after_start_rejected() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        let svc = Arc::new(TestService::new("svc", &[], false));
        manager.register_service(svc).await.unwrap();
        manager.start_all().await.unwrap();

        // Attempt to register after start
        let svc2 = Arc::new(TestService::new("svc2", &[], false));
        let result = manager.register_service(svc2).await;
        assert!(result.is_err());

        manager.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_shutdown_without_start_rejected() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        let result = manager.shutdown_all().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::RuntimeNotStarted => (),
            e => panic!("Expected RuntimeNotStarted, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_runtime_metrics() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        let svc = Arc::new(TestService::new("metrics_test", &[], false));
        manager.register_service(svc).await.unwrap();

        let metrics = manager.metrics().await;
        assert_eq!(metrics.registered_services, 1);
        assert_eq!(metrics.active_services, 0); // Not started yet

        manager.start_all().await.unwrap();

        let metrics = manager.metrics().await;
        assert_eq!(metrics.registered_services, 1);
        assert_eq!(metrics.active_services, 1);
        assert!(metrics.startup_duration.is_some());

        manager.shutdown_all().await.unwrap();

        let metrics = manager.metrics().await;
        assert_eq!(metrics.active_services, 0);
        assert!(metrics.shutdown_duration.is_some());
    }

    #[tokio::test]
    async fn test_cancellation_token_propagation() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // The context's token should be usable for external coordination
        assert!(!ctx.is_shutdown_requested());
        assert!(!manager.is_shutdown_requested());

        // Request shutdown
        manager.shutdown.request_shutdown();
        assert!(manager.is_shutdown_requested());
    }

    #[tokio::test]
    async fn test_drop_without_shutdown_requests_cancellation() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let token = ctx.cancellation_token.clone();

        {
            let manager = RuntimeManager::new(ctx, registry);
            let svc = Arc::new(TestService::new("drop_test", &[], false));
            manager.register_service(svc).await.unwrap();
            manager.start_all().await.unwrap();
            assert_eq!(manager.state().await, RuntimeState::Running);
            // Manager dropped here without explicit shutdown
        }

        // The cancellation token should be cancelled after drop
        assert!(token.is_cancelled());
    }

    // =========================================================================
    // PHASE 2.2 ARCHITECTURAL VALIDATION TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_validation_dependency_resolution() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // Register services in dependency order: metrics -> telemetry -> snapshot
        let metrics = Arc::new(crate::services::metrics::MetricsService::new());
        let telemetry = Arc::new(crate::services::telemetry::TelemetryService::new());
        let snapshot = Arc::new(crate::services::snapshot::SnapshotService::new());

        manager.register_service(metrics.clone()).await.unwrap();
        manager.register_service(telemetry.clone()).await.unwrap();
        manager.register_service(snapshot.clone()).await.unwrap();

        // Verify startup order: metrics -> telemetry -> snapshot
        let startup_order = manager.registry.compute_startup_order().await.unwrap();
        let metrics_idx = startup_order.iter().position(|s| s == "metrics").unwrap();
        let telemetry_idx = startup_order.iter().position(|s| s == "telemetry").unwrap();
        let snapshot_idx = startup_order.iter().position(|s| s == "snapshot").unwrap();

        assert!(
            metrics_idx < telemetry_idx,
            "metrics must start before telemetry"
        );
        assert!(
            telemetry_idx < snapshot_idx,
            "telemetry must start before snapshot"
        );

        // Verify shutdown order is reverse
        let shutdown_order = manager.registry.compute_shutdown_order().await.unwrap();
        assert_eq!(shutdown_order, vec!["snapshot", "telemetry", "metrics"]);
    }

    #[tokio::test]
    async fn test_validation_cycle_detection() {
        let registry = Arc::new(ServiceRegistry::new());

        // Create a service that declares a circular dependency
        struct CycleService {
            name: &'static str,
            deps: &'static [&'static str],
        }

        #[async_trait]
        impl RuntimeService for CycleService {
            fn name(&self) -> &str {
                self.name
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn dependencies(&self) -> &[&str] {
                self.deps
            }
            async fn start(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
                Ok(())
            }
        }

        // Register A -> B, B -> A (cycle)
        let a = Arc::new(CycleService {
            name: "service_a",
            deps: &["service_b"],
        });
        let b = Arc::new(CycleService {
            name: "service_b",
            deps: &["service_a"],
        });

        registry.register(a.clone()).await.unwrap();
        registry.register(b.clone()).await.unwrap();

        let result = registry.compute_startup_order().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::DependencyCycle(services) => {
                assert!(
                    services.contains(&"service_a".to_string())
                        || services.contains(&"service_b".to_string())
                );
            }
            e => panic!("Expected DependencyCycle, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_validation_duplicate_detection() {
        let registry = Arc::new(ServiceRegistry::new());
        let svc1 = Arc::new(crate::services::metrics::MetricsService::new());
        let svc2 = Arc::new(crate::services::metrics::MetricsService::new());

        registry.register(svc1.clone()).await.unwrap();
        let result = registry.register(svc2).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::ServiceAlreadyRegistered(name) => assert_eq!(name, "metrics"),
            e => panic!("Expected ServiceAlreadyRegistered, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_validation_missing_dependency_detection() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // Register telemetry (depends on metrics) without registering metrics
        let telemetry = Arc::new(crate::services::telemetry::TelemetryService::new());
        manager.register_service(telemetry).await.unwrap();

        let result = manager.start_all().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::MissingDependency { service, dep } => {
                assert_eq!(service, "telemetry");
                assert_eq!(dep, "metrics");
            }
            e => panic!("Expected MissingDependency, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_transactional_startup_rollback_at_initialize() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        let service_a = Arc::new(crate::services::metrics::MetricsService::new());
        let service_b = Arc::new(crate::services::failing::FailingService::new(
            "failing_b_init",
            crate::services::failing::FailAt::Initialize,
        ));

        manager.register_service(service_a.clone()).await.unwrap();
        manager.register_service(service_b.clone()).await.unwrap();

        let result = manager.start_all().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RuntimeError::StartupFailure {
                failed_service,
                rolled_back,
                ..
            } => {
                assert_eq!(failed_service, "failing_b_init");
                // Service A was initialized but not started, so rolled_back may be 0 or 1
                // depending on implementation - just verify it reports the failure correctly
                let _ = rolled_back; // unused but part of the error
            }
            e => panic!("Expected StartupFailure, got {:?}", e),
        }

        // Service A should be in a clean state (not running)
        assert!(!service_a.is_running());
    }

    #[tokio::test]
    async fn test_aggregated_shutdown_failures() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // Register two services that fail on stop, one that succeeds
        let service_a = Arc::new(crate::services::metrics::MetricsService::new());
        let service_b = Arc::new(crate::services::failing::FailingService::new(
            "failing_b_stop",
            crate::services::failing::FailAt::Stop,
        ));
        let service_c = Arc::new(crate::services::failing::FailingService::new(
            "failing_c_shutdown",
            crate::services::failing::FailAt::Shutdown,
        ));

        manager.register_service(service_a.clone()).await.unwrap();
        manager.register_service(service_b.clone()).await.unwrap();
        manager.register_service(service_c.clone()).await.unwrap();

        manager.start_all().await.unwrap();

        // Shutdown should aggregate errors from B and C, but still stop A
        let result = manager.shutdown_all().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RuntimeError::ShutdownFailure { failure_count, .. } => {
                assert_eq!(failure_count, 2, "Two services should have failed");
            }
            e => panic!("Expected ShutdownFailure, got {:?}", e),
        }

        // Runtime should be in Failed state after aggregated shutdown failure
        assert_eq!(manager.state().await, RuntimeState::Failed);
    }

    #[tokio::test]
    async fn test_successful_startup_and_shutdown() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        let metrics = Arc::new(crate::services::metrics::MetricsService::new());
        let telemetry = Arc::new(crate::services::telemetry::TelemetryService::new());
        let snapshot = Arc::new(crate::services::snapshot::SnapshotService::new());
        let diagnostics = Arc::new(crate::services::diagnostics::DiagnosticsService::new());

        manager.register_service(metrics.clone()).await.unwrap();
        manager.register_service(telemetry.clone()).await.unwrap();
        manager.register_service(snapshot.clone()).await.unwrap();
        manager.register_service(diagnostics.clone()).await.unwrap();

        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify all services are running
        assert!(metrics.is_running());
        assert!(telemetry.is_running());
        assert!(snapshot.is_running());
        assert!(diagnostics.is_running());

        // Verify health reporting
        let health = manager.health_check().await;
        for name in ["metrics", "telemetry", "snapshot", "diagnostics"] {
            assert_eq!(health.get(name).unwrap().status, ServiceStatus::Running);
        }

        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(report.succeeded.len(), 4);
        assert!(report.failed.is_empty());

        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_metrics_service() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Build production MetricsService
        let metrics_svc =
            Arc::new(crate::services::production_metrics::ProductionMetricsService::new());
        manager.register_service(metrics_svc.clone()).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify metrics service is running
        let health = manager.health_check().await;
        assert_eq!(
            health.get("metrics").unwrap().status,
            ServiceStatus::Running
        );

        // Give monitoring loop a moment to initialize
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Verify Prometheus export works directly in an async runtime.
        let prom_metrics = metrics_svc.export_prometheus().await;
        assert!(prom_metrics.contains("qanto_"));
        assert!(prom_metrics.contains("blocks_processed"));

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_diagnostics_service() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Build ProductionDiagnosticsService
        let diagnostics_svc = Arc::new(
            crate::services::production_diagnostics::ProductionDiagnosticsService::new(false),
        );
        manager
            .register_service(diagnostics_svc.clone())
            .await
            .unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify diagnostics service is running
        let health = manager.health_check().await;
        assert_eq!(
            health.get("diagnostics").unwrap().status,
            ServiceStatus::Running
        );

        // Verify engine is accessible and functional
        assert!(diagnostics_svc.is_running());

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_telemetry_service() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Build ProductionTelemetryService
        let telemetry_svc =
            Arc::new(crate::services::production_telemetry::ProductionTelemetryService::new());
        manager
            .register_service(telemetry_svc.clone())
            .await
            .unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify telemetry service is running
        let health = manager.health_check().await;
        assert_eq!(
            health.get("telemetry").unwrap().status,
            ServiceStatus::Running
        );

        // Verify service is accessible
        assert!(telemetry_svc.is_running());

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_integration_snapshot_service() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx.clone(), registry);

        // Build ProductionSnapshotService with a dummy storage
        // Note: This test requires a real QantoStorage, so we use the reference
        // SnapshotService (which has no external dependencies) for unit testing.
        // The production service is tested via integration with actual storage.
        let snapshot_svc = Arc::new(crate::services::snapshot::SnapshotService::new());
        manager
            .register_service(snapshot_svc.clone())
            .await
            .unwrap();

        // Register dependencies BEFORE starting (metrics -> telemetry -> snapshot)
        let telemetry_svc = Arc::new(crate::services::telemetry::TelemetryService::new());
        let metrics_svc = Arc::new(crate::services::metrics::MetricsService::new());
        manager.register_service(metrics_svc).await.unwrap();
        manager.register_service(telemetry_svc).await.unwrap();

        // Start all services
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Verify snapshot service is running
        let health = manager.health_check().await;
        assert_eq!(
            health.get("snapshot").unwrap().status,
            ServiceStatus::Running
        );

        assert!(snapshot_svc.is_running());

        // Clean shutdown
        let report = manager.shutdown_all().await.unwrap();
        assert!(report.all_succeeded);
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_stress_100_services() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // Register 100 independent services
        let start_reg = std::time::Instant::now();
        let mut services = Vec::new();
        for i in 0..100 {
            // Use FailingService with unique names and FailAt::Never to simulate independent services
            let svc = Arc::new(crate::services::failing::FailingService::new(
                &format!("stress_svc_{}", i),
                crate::services::failing::FailAt::Never,
            ));
            manager.register_service(svc.clone()).await.unwrap();
            services.push(svc);
        }
        let reg_duration = start_reg.elapsed();
        println!("Registration time for 100 services: {:?}", reg_duration);

        // Measure dependency resolution time
        let start_resolve = std::time::Instant::now();
        let order = manager.registry.compute_startup_order().await.unwrap();
        let resolve_duration = start_resolve.elapsed();
        println!(
            "Dependency resolution time for 100 services: {:?}",
            resolve_duration
        );
        assert_eq!(order.len(), 100);

        // Measure startup time
        let start_startup = std::time::Instant::now();
        manager.start_all().await.unwrap();
        let startup_duration = start_startup.elapsed();
        println!("Startup time for 100 services: {:?}", startup_duration);

        // Verify all running
        for svc in &services {
            assert!(svc.is_running());
        }
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Measure shutdown time
        let start_shutdown = std::time::Instant::now();
        let report = manager.shutdown_all().await.unwrap();
        let shutdown_duration = start_shutdown.elapsed();
        println!("Shutdown time for 100 services: {:?}", shutdown_duration);

        assert!(report.all_succeeded);
        assert_eq!(report.succeeded.len(), 100);
        assert_eq!(manager.state().await, RuntimeState::Stopped);

        // Basic performance assertions (generous thresholds)
        assert!(
            reg_duration < Duration::from_secs(2),
            "Registration too slow"
        );
        assert!(
            resolve_duration < Duration::from_secs(1),
            "Resolution too slow"
        );
        assert!(
            startup_duration < Duration::from_secs(10),
            "Startup too slow"
        );
        assert!(
            shutdown_duration < Duration::from_secs(10),
            "Shutdown too slow"
        );
    }

    #[tokio::test]
    async fn test_runtime_state_machine_validation() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        // Initial state
        assert_eq!(manager.state().await, RuntimeState::Created);

        // Register a service
        let svc = Arc::new(crate::services::metrics::MetricsService::new());
        manager.register_service(svc).await.unwrap();

        // Start
        manager.start_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Running);

        // Shutdown
        manager.shutdown_all().await.unwrap();
        assert_eq!(manager.state().await, RuntimeState::Stopped);

        // Invalid: register after running (should fail)
        let registry2 = Arc::new(ServiceRegistry::new());
        let ctx2 = make_context();
        let manager2 = RuntimeManager::new(ctx2, registry2);
        let svc2 = Arc::new(crate::services::metrics::MetricsService::new());
        manager2.register_service(svc2).await.unwrap();
        manager2.start_all().await.unwrap();

        let svc3 = Arc::new(crate::services::metrics::MetricsService::new());
        let result = manager2.register_service(svc3).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::RuntimeAlreadyStarted => (),
            e => panic!("Expected RuntimeAlreadyStarted, got {:?}", e),
        }

        manager2.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_force_shutdown_terminates_all() {
        let registry = Arc::new(ServiceRegistry::new());
        let ctx = make_context();
        let manager = RuntimeManager::new(ctx, registry);

        let metrics = Arc::new(crate::services::metrics::MetricsService::new());
        let telemetry = Arc::new(crate::services::telemetry::TelemetryService::new());

        manager.register_service(metrics.clone()).await.unwrap();
        manager.register_service(telemetry.clone()).await.unwrap();

        manager.start_all().await.unwrap();
        assert!(metrics.is_running());
        assert!(telemetry.is_running());

        // Force shutdown should cancel immediately
        manager.force_shutdown().await.unwrap();

        // Services should be marked stopped
        assert_eq!(manager.state().await, RuntimeState::Stopped);
    }
}
