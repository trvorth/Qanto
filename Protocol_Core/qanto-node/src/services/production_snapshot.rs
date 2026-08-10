//! ProductionSnapshotService — RuntimeService wrapping QantoStorage checkpoint logic.
//!
//! Zero dependencies. Provides lifecycle management for periodic storage checkpoints
//! and graceful shutdown checkpoint.
//!
//! # Design
//!
//! - Runs a periodic checkpoint task (default: every 10 minutes)
//! - On shutdown, creates a final "shutdown checkpoint"
//! - Maintains max_checkpoints (default: 2) by rotating oldest
//! - Reuses QantoStorage::create_checkpoint() — no new persistence logic

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::qanto_storage::QantoStorage;
use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production SnapshotService — wraps QantoStorage checkpoint logic with RuntimeService lifecycle.
pub struct ProductionSnapshotService {
    /// QantoStorage instance for creating checkpoints.
    storage: Arc<QantoStorage>,
    /// Data directory path for checkpoint storage.
    data_dir: PathBuf,
    /// Checkpoint interval in seconds.
    checkpoint_interval_secs: u64,
    /// Maximum number of checkpoints to retain.
    max_checkpoints: usize,
    /// Whether the service is running.
    running: Arc<AtomicBool>,
    /// Task handle for the checkpoint loop.
    task_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
    /// Counter for created checkpoints.
    checkpoint_count: Arc<AtomicU64>,
}

impl ProductionSnapshotService {
    /// Create a new SnapshotService with default settings.
    pub fn new(storage: Arc<QantoStorage>, data_dir: PathBuf) -> Self {
        Self::with_config(storage, data_dir, 600, 2) // 10 minutes, max 2 checkpoints
    }

    /// Create with custom configuration.
    pub fn with_config(
        storage: Arc<QantoStorage>,
        data_dir: PathBuf,
        checkpoint_interval_secs: u64,
        max_checkpoints: usize,
    ) -> Self {
        Self {
            storage,
            data_dir,
            checkpoint_interval_secs,
            max_checkpoints,
            running: Arc::new(AtomicBool::new(false)),
            task_handle: RwLock::new(None),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
            checkpoint_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get the number of checkpoints created.
    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoint_count.load(Ordering::Relaxed)
    }

    /// Check if the service is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Create a checkpoint with the given directory name.
    async fn create_checkpoint(&self, dir_name: &str) -> Result<(), RuntimeError> {
        let checkpoint_dir = self.data_dir.join(dir_name);
        self.storage
            .create_checkpoint(&checkpoint_dir)
            .map_err(|e| RuntimeError::Internal(format!("Checkpoint failed: {}", e)))?;

        self.checkpoint_count.fetch_add(1, Ordering::Relaxed);
        info!("✅ Checkpoint created at {:?}", checkpoint_dir);
        Ok(())
    }
}

#[async_trait]
impl RuntimeService for ProductionSnapshotService {
    fn name(&self) -> &str {
        "snapshot"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> &[&str] {
        &["telemetry"]
    }

    fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy::OnFailure
    }

    async fn initialize(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionSnapshotService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionSnapshotService: Starting periodic checkpoint task");
        self.running.store(true, Ordering::SeqCst);

        let storage = self.storage.clone();
        let data_dir = self.data_dir.clone();
        let checkpoint_interval = Duration::from_secs(self.checkpoint_interval_secs);
        let max_checkpoints = self.max_checkpoints;
        let running = self.running.clone();
        let checkpoint_count = self.checkpoint_count.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(checkpoint_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("SnapshotService: Checkpoint loop cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }

                        // Periodic checkpoint
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let dir_name = format!("checkpoint_{}", timestamp);

                        let checkpoint_dir = data_dir.join(&dir_name);
                        match storage.create_checkpoint(&checkpoint_dir) {
                            Ok(()) => {
                                checkpoint_count.fetch_add(1, Ordering::Relaxed);
                                info!("✅ Periodic checkpoint created at {:?}", checkpoint_dir);
                            }
                            Err(e) => {
                                warn!("⚠️ Periodic checkpoint failed: {}", e);
                            }
                        }

                        // Rotate old checkpoints
                        if let Ok(entries) = std::fs::read_dir(&data_dir) {
                            let mut checkpoint_dirs: Vec<_> = entries
                                .filter_map(|e| e.ok())
                                .filter(|e| {
                                    e.path().is_dir()
                                        && e.file_name()
                                            .to_str()
                                            .map(|n| n.starts_with("checkpoint_") && !n.starts_with("checkpoint_shutdown"))
                                            .unwrap_or(false)
                                })
                                .collect();
                            checkpoint_dirs.sort_by_key(|e| e.file_name());

                            while checkpoint_dirs.len() > max_checkpoints {
                                if let Some(oldest) = checkpoint_dirs.first() {
                                    let oldest_path = oldest.path();
                                    if let Err(e) = std::fs::remove_dir_all(&oldest_path) {
                                        warn!("Failed to remove old checkpoint {:?}: {}", oldest_path, e);
                                    } else {
                                        info!("🗑️ Removed old checkpoint {:?}", oldest_path);
                                    }
                                    checkpoint_dirs.remove(0);
                                }
                            }
                        }
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            info!("SnapshotService: Checkpoint loop exited");
        });

        let mut handle_guard = self.task_handle.write().await;
        *handle_guard = Some(handle);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionSnapshotService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionSnapshotService: Shutdown — creating final checkpoint");

        // Create shutdown checkpoint
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dir_name = format!("checkpoint_shutdown_{}", timestamp);
        let _ = self.create_checkpoint(&dir_name).await;

        // Wait for task to exit
        let mut handle_guard = self.task_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            let _ = handle.await;
        }

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        info!("ProductionSnapshotService: Shutdown complete");
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
