//! Production MetricsService — RuntimeService wrapping the QantoMetrics global singleton.
//!
//! This is the FIRST production service to be migrated into the RuntimeManager.
//! It replaces the reference validation `MetricsService` in `services/metrics.rs`
//! with an operational pre-audit implementation that:
//!
//! - Uses the global `GLOBAL_METRICS` singleton (shared with Core Engines)
//! - Activates monitoring to periodically compute TPS/hash rate
//! - Exposes Prometheus and JSON metrics export methods
//! - Integrates with the RuntimeManager lifecycle
//!
//! # Design
//!
//! MetricsService has zero dependencies and runs a background monitoring loop
//! (via std::thread) that updates real-time TPS, hash rate, and system metrics
//! every second. The API endpoints in `node.rs` continue to read from the
//! global singleton independently of MetricsService's lifecycle.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info};

use crate::metrics::{self, QantoMetrics};
use crate::runtime_context::RuntimeContext;
use crate::runtime_service::{
    RestartPolicy, RuntimeError, RuntimeService, ServiceHealth, ServiceStatus,
};
use async_trait::async_trait;

/// Production MetricsService — wraps the global QantoMetrics singleton
/// with a RuntimeManager-compatible lifecycle.
pub struct ProductionMetricsService {
    /// Handle to the global metrics singleton.
    metrics: Arc<QantoMetrics>,
    /// Whether the monitoring loop is running.
    running: Arc<AtomicBool>,
    /// Handle to the std::thread monitoring loop (if started).
    monitor_handle: RwLock<Option<JoinHandle<()>>>,
    /// Internal health state.
    health: Arc<RwLock<ServiceHealth>>,
}

impl ProductionMetricsService {
    /// Create a new MetricsService using the global singleton.
    pub fn new() -> Self {
        Self {
            metrics: metrics::get_global_metrics(),
            running: Arc::new(AtomicBool::new(false)),
            monitor_handle: RwLock::new(None),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    /// Create with an explicit metrics instance (for isolation or testing).
    pub fn with_metrics(metrics: Arc<QantoMetrics>) -> Self {
        Self {
            metrics,
            running: Arc::new(AtomicBool::new(false)),
            monitor_handle: RwLock::new(None),
            health: Arc::new(RwLock::new(ServiceHealth::new(ServiceStatus::NotStarted))),
        }
    }

    /// Get a reference to the underlying metrics for API handlers.
    pub fn metrics_ref(&self) -> Arc<QantoMetrics> {
        self.metrics.clone()
    }

    /// Export Prometheus-formatted metrics.
    pub async fn export_prometheus(&self) -> String {
        self.metrics.export_prometheus().await
    }

    /// Export metrics as a JSON BTreeMap for deterministic key ordering.
    pub fn export_json(&self) -> std::collections::BTreeMap<String, u128> {
        self.metrics.export_metrics()
    }
}

impl Default for ProductionMetricsService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeService for ProductionMetricsService {
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
        info!("ProductionMetricsService: initialize — global metrics ready");
        let mut health = self.health.write().await;
        health.status = ServiceStatus::Initializing;
        Ok(())
    }

    async fn start(&self, ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionMetricsService: Starting metrics monitoring loop");

        self.running.store(true, Ordering::SeqCst);

        // Start the monitoring loop as async task
        // Updates TPS, hash rate, and system metrics every 1s using tokio::time::interval
        let metrics_clone = self.metrics.clone();
        let running_clone = self.running.clone();
        let cancel_token = ctx.cancellation_token.clone();

        let handle = tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            let mut counter: u64 = 0;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if counter.is_multiple_of(50) && cancel_token.is_cancelled() {
                            debug!("MetricsService: Monitoring loop cancelled");
                            break;
                        }
                        if !running_clone.load(Ordering::Relaxed) {
                            break;
                        }

                        metrics_clone.record_system_metrics();

                        let real_time_tps = metrics_clone.calculate_real_time_tps();
                        metrics_clone.set_tps(real_time_tps);

                        let current_attempts = metrics_clone.mining_attempts.load(Ordering::Relaxed);
                        let previous_attempts = metrics_clone.last_hash_attempts.load(Ordering::Relaxed);
                        metrics_clone
                            .last_hash_attempts
                            .store(current_attempts, Ordering::Relaxed);

                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);
                        let prev_ms = metrics_clone
                            .hash_rate_last_sample_ms
                            .swap(now_ms, Ordering::Relaxed);
                        let interval_ms = if prev_ms == 0 {
                            1000
                        } else {
                            now_ms.saturating_sub(prev_ms)
                        };
                        if interval_ms > 0 && current_attempts >= previous_attempts {
                            let attempts_in_window = current_attempts - previous_attempts;
                            let hps = (attempts_in_window as u128 * 1000) / interval_ms.max(1) as u128;
                            metrics_clone.set_hash_rate_hps(hps);
                        }

                        metrics_clone.touch();

                        // Emit structured JSON log every 60 ticks (1 min)
                        if counter > 0 && counter.is_multiple_of(60) {
                            let summary = metrics_clone.get_performance_summary();
                            tracing::info!(
                                target: "qanto.metrics",
                                tps = summary.get("tps_real_time").copied().unwrap_or(0) as f64 / 1_000_000_000.0,
                                hps = summary.get("network_throughput_mbps").copied().unwrap_or(0) as f64 / 1_000_000_000.0,
                                mempool = summary.get("queue_depth").copied().unwrap_or(0),
                                peers = summary.get("validator_count").copied().unwrap_or(0),
                                uptime_s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                                "QANTO_METRICS_SNAPSHOT",
                            );
                        }

                        if cancel_token.is_cancelled() {
                            debug!("MetricsService: Monitoring loop cancelled (immediate check)");
                            break;
                        }

                        counter += 1;
                    }
                    _ = cancel_token.cancelled() => {
                        debug!("MetricsService: Monitoring loop cancelled via token");
                        break;
                    }
                }
            }
            running_clone.store(false, Ordering::SeqCst);
            info!("MetricsService: Monitoring loop exited");
        });

        let mut handle_guard = self.monitor_handle.write().await;
        *handle_guard = Some(handle);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Running;
        info!("ProductionMetricsService: Running — monitoring loop active");
        Ok(())
    }

    async fn stop(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionMetricsService: Stop requested");
        self.running.store(false, Ordering::SeqCst);

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopping;
        Ok(())
    }

    async fn shutdown(&self, _ctx: Arc<RuntimeContext>) -> Result<(), RuntimeError> {
        info!("ProductionMetricsService: Shutdown");

        let mut handle_guard = self.monitor_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            info!("ProductionMetricsService: Waiting for monitoring task to exit...");
            let _ = handle.await;
            info!("ProductionMetricsService: Monitoring task exited gracefully");
        }

        let mut health = self.health.write().await;
        health.status = ServiceStatus::Stopped;
        info!("ProductionMetricsService: Shutdown complete");
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
