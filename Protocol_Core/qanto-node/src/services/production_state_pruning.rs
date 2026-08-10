//! ProductionStatePruningService — RuntimeService wrapping QantoStorage compaction.
//!
//! Zero dependencies. Provides lifecycle management for periodic state pruning
//! (database compaction) and manual pruning triggers.
//!
//! # Design
//!
//! - Runs periodic compaction task (default: every 30 minutes)
//! - On shutdown, performs final compaction
//! - Reuses QantoStorage::compact() — no new persistence logic
//! - Can be triggered manually via API if needed

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::qanto_storage::QantoStorage;
use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production StatePruningService — wraps QantoStorage compaction with RuntimeService lifecycle.
pub struct ProductionStatePruningService {
    /// QantoStorage instance for compaction.
    storage: Arc<QantoStorage>,
    /// Compaction interval in seconds.
    compaction_interval_secs: u64,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Task handle for the compaction loop.
    task_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
    /// Counter for compactions performed.
    compaction_count: Arc<AtomicU64>,
}

impl ProductionStatePruningService {
    /// Create a new StatePruningService with default settings.
    pub fn new(storage: Arc<QantoStorage>) -> Self {
        Self::with_interval(storage, 1800) // 30 minutes default
    }

    /// Create with custom compaction interval.
    pub fn with_interval(storage: Arc<QantoStorage>, compaction_interval_secs: u64) -> Self {
        Self {
            storage,
            compaction_interval_secs,
            running: Arc::new(AtomicBool::new(false)),
            task_handle: RwLock::new(None),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
            compaction_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get the number of compactions performed.
    pub fn compaction_count(&self) -> u64 {
        self.compaction_count.load(Ordering::Relaxed)
    }

    /// Check if the service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Perform a single compaction cycle.
    async fn perform_compaction(&self) -> Result<(), RuntimeError> {
        info!("StatePruningService: Starting database compaction");
        let start = SystemTime::now();

        self.storage
            .compact()
            .map_err(|e| RuntimeError::Internal(format!("Compaction failed: {}", e)))?;

        let elapsed = start.elapsed().unwrap_or_default();
        self.compaction_count.fetch_add(1, Ordering::Relaxed);
        info!(
            "✅ StatePruningService: Compaction completed in {:?} (total: {})",
            elapsed,
            self.compaction_count()
        );
        Ok(())
    }
}

#[async_trait]
impl RuntimeService for ProductionStatePruningService {
    fn name(&self) -> &str {
        "state_pruning"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &[] // No dependencies — can run independently
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionStatePruningService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionStatePruningService: Starting periodic compaction task");
        self.running.store(true, Ordering::SeqCst);

        let storage = self.storage.clone();
        let compaction_interval = Duration::from_secs(self.compaction_interval_secs);
        let running = self.running.clone();
        let compaction_count = self.compaction_count.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(compaction_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("StatePruningService: Compaction loop cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }

                        info!("StatePruningService: Running scheduled compaction");
                        let start = SystemTime::now();

                        match storage.compact() {
                            Ok(()) => {
                                let elapsed = start.elapsed().unwrap_or_default();
                                compaction_count.fetch_add(1, Ordering::Relaxed);
                                info!(
                                    "✅ StatePruningService: Scheduled compaction completed in {:?} (total: {})",
                                    elapsed,
                                    compaction_count.load(Ordering::Relaxed)
                                );
                            }
                            Err(e) => {
                                warn!("⚠️ StatePruningService: Scheduled compaction failed: {}", e);
                            }
                        }
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            info!("StatePruningService: Compaction loop exited");
        });

        let mut handle_guard = self.task_handle.write().await;
        *handle_guard = Some(handle);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionStatePruningService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionStatePruningService: Shutdown — performing final compaction");

        // Perform final compaction on shutdown
        let _ = self.perform_compaction().await;

        // Wait for task to exit
        let mut handle_guard = self.task_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            let _ = handle.await;
        }

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        info!("ProductionStatePruningService: Shutdown complete");
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
