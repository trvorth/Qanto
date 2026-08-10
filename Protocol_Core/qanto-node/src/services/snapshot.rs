//! SnapshotService — Reference RuntimeService with dependency on telemetry.
//!
//! Depends on: telemetry
//!
//! Validates the full dependency chain:
//! metrics -> telemetry -> snapshot

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

/// A snapshot service that depends on TelemetryService.
pub struct SnapshotService {
    running: Arc<AtomicBool>,
    snapshot_count: Arc<AtomicU64>,
    health: Arc<RwLock<ServiceHealth>>,
}

impl SnapshotService {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            snapshot_count: Arc::new(AtomicU64::new(0)),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count.load(Ordering::Relaxed)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for SnapshotService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeService for SnapshotService {
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
        info!("SnapshotService: initialize");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("SnapshotService: Starting snapshot task");
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let snapshot_count = self.snapshot_count.clone();
        let cancel_token = ctx.cancellation_token.child_token();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("SnapshotService: Snapshot task cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        let count = snapshot_count.fetch_add(1, Ordering::Relaxed);
                        debug!("SnapshotService: Created snapshot #{}", count + 1);
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
            info!("SnapshotService: Snapshot task exited");
        });

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("SnapshotService: Stop requested");
        self.running.store(false, Ordering::SeqCst);
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("SnapshotService: Shutdown complete");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        Ok(())
    }

    async fn health(&self, _ctx: Arc<RuntimeContext>) -> ServiceHealth {
        self.health.read().await.clone()
    }
}
