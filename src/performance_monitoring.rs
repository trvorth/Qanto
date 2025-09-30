//! Performance Monitoring and Adaptive Tuning for Qanto Blockchain
//!
//! This module provides comprehensive performance monitoring, memory optimization,
//! and adaptive tuning capabilities for the Qanto blockchain system.

#[allow(unused_imports)]
use crate::qantodag::QantoDAGError;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Performance monitoring errors
#[derive(Debug, thiserror::Error)]
pub enum PerformanceMonitoringError {
    #[error("Memory threshold exceeded: {current_mb}MB > {threshold_mb}MB")]
    MemoryThresholdExceeded { current_mb: u64, threshold_mb: u64 },

    #[error(
        "Performance degradation detected: {metric} dropped to {current} (threshold: {threshold})"
    )]
    PerformanceDegradation {
        metric: String,
        current: f64,
        threshold: f64,
    },

    #[error("Resource exhaustion: {resource}")]
    ResourceExhaustion { resource: String },

    #[error("Monitoring system error: {0}")]
    MonitoringError(String),
}

/// Performance metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: u64,
    pub memory_usage_mb: u64,
    pub cpu_utilization: f64,
    pub blocks_per_second: f64,
    pub transactions_per_second: f64,
    pub avg_block_time_ms: f64,
    pub avg_tx_processing_time_us: f64,
    pub active_connections: usize,
    pub mempool_size: usize,
    pub utxo_set_size: usize,
    pub cache_hit_ratio: f64,
    pub gc_pressure: f64,
}

/// Adaptive tuning parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveTuningParams {
    pub batch_size: usize,
    pub worker_threads: usize,
    pub cache_size: usize,
    pub gc_threshold_mb: u64,
    pub backpressure_threshold: f64,
    pub mining_difficulty_adjustment: f64,
}

impl Default for AdaptiveTuningParams {
    fn default() -> Self {
        Self {
            batch_size: 10000,
            worker_threads: num_cpus::get(),
            cache_size: 1000,
            gc_threshold_mb: 512,
            backpressure_threshold: 0.8,
            mining_difficulty_adjustment: 1.0,
        }
    }
}

/// Memory optimization strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOptimizationStrategy {
    Conservative,
    Balanced,
    Aggressive,
}

/// Performance monitoring configuration
#[derive(Debug, Clone)]
pub struct PerformanceMonitoringConfig {
    pub monitoring_interval_ms: u64,
    pub memory_threshold_mb: u64,
    pub cpu_threshold: f64,
    pub bps_threshold: f64,
    pub tps_threshold: f64,
    pub history_size: usize,
    pub enable_adaptive_tuning: bool,
    pub memory_optimization_strategy: MemoryOptimizationStrategy,
}

impl Default for PerformanceMonitoringConfig {
    fn default() -> Self {
        Self {
            monitoring_interval_ms: 1000,
            memory_threshold_mb: 1024,
            cpu_threshold: 80.0,
            bps_threshold: 30.0,
            tps_threshold: 1_000_000.0,
            history_size: 1000,
            enable_adaptive_tuning: true,
            memory_optimization_strategy: MemoryOptimizationStrategy::Balanced,
        }
    }
}

/// Performance monitoring and adaptive tuning system
pub struct PerformanceMonitor {
    config: PerformanceMonitoringConfig,
    running: Arc<AtomicBool>,

    // Metrics storage
    current_snapshot: Arc<RwLock<PerformanceSnapshot>>,
    history: Arc<RwLock<VecDeque<PerformanceSnapshot>>>,

    // Adaptive tuning
    tuning_params: Arc<RwLock<AdaptiveTuningParams>>,
    last_tuning: Arc<RwLock<Instant>>,

    // Memory management
    memory_pressure: Arc<AtomicU64>,
    gc_requests: Arc<AtomicU64>,

    // Performance counters
    blocks_processed: Arc<AtomicU64>,
    transactions_processed: Arc<AtomicU64>,
    cache_hits: Arc<AtomicU64>,
    cache_misses: Arc<AtomicU64>,

    // Resource tracking
    active_tasks: Arc<Semaphore>,
    resource_usage: Arc<DashMap<String, u64>>,

    // Optimization hooks
    optimization_hooks: Arc<RwLock<Vec<OptimizationHook>>>,
}

/// Hook for custom optimization strategies
pub type OptimizationHook = Box<
    dyn Fn(
            &PerformanceSnapshot,
            &mut AdaptiveTuningParams,
        ) -> Result<(), PerformanceMonitoringError>
        + Send
        + Sync,
>;

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(config: PerformanceMonitoringConfig) -> Self {
        let initial_snapshot = PerformanceSnapshot {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            memory_usage_mb: 0,
            cpu_utilization: 0.0,
            blocks_per_second: 0.0,
            transactions_per_second: 0.0,
            avg_block_time_ms: 0.0,
            avg_tx_processing_time_us: 0.0,
            active_connections: 0,
            mempool_size: 0,
            utxo_set_size: 0,
            cache_hit_ratio: 0.0,
            gc_pressure: 0.0,
        };

        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            current_snapshot: Arc::new(RwLock::new(initial_snapshot)),
            history: Arc::new(RwLock::new(VecDeque::new())),
            tuning_params: Arc::new(RwLock::new(AdaptiveTuningParams::default())),
            last_tuning: Arc::new(RwLock::new(Instant::now())),
            memory_pressure: Arc::new(AtomicU64::new(0)),
            gc_requests: Arc::new(AtomicU64::new(0)),
            blocks_processed: Arc::new(AtomicU64::new(0)),
            transactions_processed: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            active_tasks: Arc::new(Semaphore::new(10000)),
            resource_usage: Arc::new(DashMap::new()),
            optimization_hooks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start the performance monitoring system
    pub async fn start(&self) -> Result<(), PerformanceMonitoringError> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(PerformanceMonitoringError::MonitoringError(
                "Performance monitor is already running".to_string(),
            ));
        }

        info!("Starting performance monitoring system");

        let monitor = self.clone();
        tokio::spawn(async move {
            monitor.monitoring_loop().await;
        });

        Ok(())
    }

    /// Stop the performance monitoring system
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("Performance monitoring system stopped");
    }

    /// Main monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = interval(Duration::from_millis(self.config.monitoring_interval_ms));

        while self.running.load(Ordering::SeqCst) {
            interval.tick().await;

            if let Err(e) = self.collect_metrics().await {
                error!("Failed to collect performance metrics: {}", e);
                continue;
            }

            if self.config.enable_adaptive_tuning {
                if let Err(e) = self.perform_adaptive_tuning().await {
                    warn!("Adaptive tuning failed: {}", e);
                }
            }

            if let Err(e) = self.check_thresholds().await {
                warn!("Performance threshold check failed: {}", e);
            }

            // Memory optimization
            if let Err(e) = self.optimize_memory().await {
                warn!("Memory optimization failed: {}", e);
            }
        }
    }

    /// Collect current performance metrics
    async fn collect_metrics(&self) -> Result<(), PerformanceMonitoringError> {
        let _permit = self
            .active_tasks
            .acquire()
            .await
            .map_err(|e| PerformanceMonitoringError::MonitoringError(e.to_string()))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Collect system metrics
        let memory_usage_mb = self.get_memory_usage_mb().await;
        let cpu_utilization = self.get_cpu_utilization().await;

        // Collect blockchain metrics
        let _blocks_processed = self.blocks_processed.load(Ordering::Relaxed);
        let _transactions_processed = self.transactions_processed.load(Ordering::Relaxed);

        // Calculate rates from history
        let (bps, tps) = self.calculate_rates().await;

        // Calculate cache hit ratio
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        let cache_hit_ratio = if cache_hits + cache_misses > 0 {
            cache_hits as f64 / (cache_hits + cache_misses) as f64
        } else {
            0.0
        };

        let snapshot = PerformanceSnapshot {
            timestamp,
            memory_usage_mb,
            cpu_utilization,
            blocks_per_second: bps,
            transactions_per_second: tps,
            avg_block_time_ms: if bps > 0.0 { 1000.0 / bps } else { 0.0 },
            avg_tx_processing_time_us: if tps > 0.0 { 1_000_000.0 / tps } else { 0.0 },
            active_connections: self
                .resource_usage
                .get("connections")
                .map(|v| *v as usize)
                .unwrap_or(0),
            mempool_size: self
                .resource_usage
                .get("mempool_size")
                .map(|v| *v as usize)
                .unwrap_or(0),
            utxo_set_size: self
                .resource_usage
                .get("utxo_set_size")
                .map(|v| *v as usize)
                .unwrap_or(0),
            cache_hit_ratio,
            gc_pressure: self.memory_pressure.load(Ordering::Relaxed) as f64 / 100.0,
        };

        // Update current snapshot
        *self.current_snapshot.write().await = snapshot.clone();

        // Add to history
        let mut history = self.history.write().await;
        history.push_back(snapshot);

        // Maintain history size
        while history.len() > self.config.history_size {
            history.pop_front();
        }

        debug!(
            "Performance metrics collected: BPS={:.2}, TPS={:.0}, Memory={}MB",
            bps, tps, memory_usage_mb
        );

        Ok(())
    }

    /// Perform adaptive tuning based on current performance
    async fn perform_adaptive_tuning(&self) -> Result<(), PerformanceMonitoringError> {
        let mut last_tuning = self.last_tuning.write().await;

        // Only tune every 10 seconds to avoid oscillation
        if last_tuning.elapsed() < Duration::from_secs(10) {
            return Ok(());
        }

        let snapshot = self.current_snapshot.read().await.clone();
        let mut params = self.tuning_params.write().await;

        // Apply optimization hooks
        let hooks = self.optimization_hooks.read().await;
        for hook in hooks.iter() {
            hook(&snapshot, &mut params)?;
        }

        // Built-in adaptive tuning logic
        self.apply_adaptive_tuning(&snapshot, &mut params).await?;

        *last_tuning = Instant::now();

        info!(
            "Adaptive tuning applied: batch_size={}, workers={}, cache_size={}",
            params.batch_size, params.worker_threads, params.cache_size
        );

        Ok(())
    }

    /// Apply built-in adaptive tuning strategies
    async fn apply_adaptive_tuning(
        &self,
        snapshot: &PerformanceSnapshot,
        params: &mut AdaptiveTuningParams,
    ) -> Result<(), PerformanceMonitoringError> {
        // Adjust batch size based on TPS performance
        if snapshot.transactions_per_second < self.config.tps_threshold * 0.8 {
            params.batch_size = (params.batch_size * 110 / 100).min(100_000);
        } else if snapshot.transactions_per_second > self.config.tps_threshold * 1.2 {
            params.batch_size = (params.batch_size * 90 / 100).max(1000);
        }

        // Adjust worker threads based on CPU utilization
        if snapshot.cpu_utilization < 60.0 && params.worker_threads < num_cpus::get() * 2 {
            params.worker_threads += 1;
        } else if snapshot.cpu_utilization > 90.0 && params.worker_threads > 1 {
            params.worker_threads -= 1;
        }

        // Adjust cache size based on hit ratio and memory pressure
        if snapshot.cache_hit_ratio < 0.8
            && snapshot.memory_usage_mb < self.config.memory_threshold_mb * 80 / 100
        {
            params.cache_size = (params.cache_size * 110 / 100).min(10_000);
        } else if snapshot.memory_usage_mb > self.config.memory_threshold_mb * 90 / 100 {
            params.cache_size = (params.cache_size * 90 / 100).max(100);
        }

        // Adjust GC threshold based on memory pressure
        if snapshot.gc_pressure > 0.8 {
            params.gc_threshold_mb = (params.gc_threshold_mb * 90 / 100).max(128);
        } else if snapshot.gc_pressure < 0.3 {
            params.gc_threshold_mb = (params.gc_threshold_mb * 110 / 100).min(2048);
        }

        Ok(())
    }

    /// Check performance thresholds and trigger alerts
    async fn check_thresholds(&self) -> Result<(), PerformanceMonitoringError> {
        let snapshot = self.current_snapshot.read().await;

        // Memory threshold check
        if snapshot.memory_usage_mb > self.config.memory_threshold_mb {
            warn!(
                "Memory threshold exceeded: {}MB > {}MB",
                snapshot.memory_usage_mb, self.config.memory_threshold_mb
            );
            self.trigger_memory_cleanup().await?;
        }

        // CPU threshold check
        if snapshot.cpu_utilization > self.config.cpu_threshold {
            warn!(
                "CPU utilization high: {:.1}% > {:.1}%",
                snapshot.cpu_utilization, self.config.cpu_threshold
            );
        }

        // Performance threshold checks
        if snapshot.blocks_per_second < self.config.bps_threshold {
            warn!(
                "BPS below threshold: {:.2} < {:.2}",
                snapshot.blocks_per_second, self.config.bps_threshold
            );
        }

        if snapshot.transactions_per_second < self.config.tps_threshold {
            warn!(
                "TPS below threshold: {:.0} < {:.0}",
                snapshot.transactions_per_second, self.config.tps_threshold
            );
        }

        Ok(())
    }

    /// Optimize memory usage based on current strategy
    async fn optimize_memory(&self) -> Result<(), PerformanceMonitoringError> {
        let snapshot = self.current_snapshot.read().await;
        let memory_usage_percent =
            (snapshot.memory_usage_mb * 100) / self.config.memory_threshold_mb;

        match self.config.memory_optimization_strategy {
            MemoryOptimizationStrategy::Conservative => {
                if memory_usage_percent > 90 {
                    self.trigger_memory_cleanup().await?;
                }
            }
            MemoryOptimizationStrategy::Balanced => {
                if memory_usage_percent > 80 {
                    self.trigger_memory_cleanup().await?;
                }
            }
            MemoryOptimizationStrategy::Aggressive => {
                if memory_usage_percent > 70 {
                    self.trigger_memory_cleanup().await?;
                }
            }
        }

        Ok(())
    }

    /// Trigger memory cleanup operations
    async fn trigger_memory_cleanup(&self) -> Result<(), PerformanceMonitoringError> {
        info!("Triggering memory cleanup operations");

        self.gc_requests.fetch_add(1, Ordering::Relaxed);

        // Force garbage collection (platform-specific)
        #[cfg(target_os = "linux")]
        unsafe {
            libc::malloc_trim(0);
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux platforms, just log the cleanup request
            debug!("Memory cleanup requested (platform-specific cleanup not available)");
        }

        // Clear caches if memory pressure is high
        let memory_pressure = self.memory_pressure.load(Ordering::Relaxed);
        if memory_pressure > 80 {
            // Signal cache cleanup to other components
            self.resource_usage
                .insert("cache_cleanup_requested".to_string(), 1);
        }

        Ok(())
    }

    /// Get current memory usage in MB
    async fn get_memory_usage_mb(&self) -> u64 {
        // Use system-specific memory measurement
        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = tokio::fs::read_to_string("/proc/self/status").await {
                for line in contents.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                return kb / 1024; // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // Use mach system calls for macOS
            use std::mem;

            #[allow(deprecated)]
            unsafe {
                let mut info: libc::mach_task_basic_info = mem::zeroed();
                let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;

                let result = libc::task_info(
                    libc::mach_task_self(),
                    libc::MACH_TASK_BASIC_INFO,
                    &mut info as *mut _ as *mut i32,
                    &mut count,
                );

                if result == libc::KERN_SUCCESS {
                    return (info.resident_size / 1024 / 1024) as u64;
                }
            }
        }

        // Fallback: estimate based on resource tracking
        self.resource_usage
            .get("estimated_memory_mb")
            .map(|v| *v)
            .unwrap_or(0)
    }

    /// Get current CPU utilization percentage
    async fn get_cpu_utilization(&self) -> f64 {
        // Simplified CPU utilization - in production, use proper system monitoring
        self.resource_usage
            .get("cpu_utilization")
            .map(|v| *v as f64)
            .unwrap_or(0.0)
    }

    /// Calculate BPS and TPS rates from history
    async fn calculate_rates(&self) -> (f64, f64) {
        let history = self.history.read().await;

        if history.len() < 2 {
            return (0.0, 0.0);
        }

        let recent = &history[history.len() - 1];
        let previous = &history[history.len() - 2];

        let time_diff = recent.timestamp.saturating_sub(previous.timestamp) as f64;

        if time_diff > 0.0 {
            let bps = (recent.blocks_per_second + previous.blocks_per_second) / 2.0;
            let tps = (recent.transactions_per_second + previous.transactions_per_second) / 2.0;
            (bps, tps)
        } else {
            (0.0, 0.0)
        }
    }

    /// Add a custom optimization hook
    pub async fn add_optimization_hook(&self, hook: OptimizationHook) {
        self.optimization_hooks.write().await.push(hook);
    }

    /// Update resource usage metric
    pub fn update_resource_usage(&self, resource: &str, value: u64) {
        self.resource_usage.insert(resource.to_string(), value);
    }

    /// Record block processing
    pub fn record_block_processed(&self) {
        self.blocks_processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record transaction processing
    pub fn record_transactions_processed(&self, count: u64) {
        self.transactions_processed
            .fetch_add(count, Ordering::Relaxed);
    }

    /// Record cache hit
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache miss
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current performance snapshot
    pub async fn get_current_snapshot(&self) -> PerformanceSnapshot {
        self.current_snapshot.read().await.clone()
    }

    /// Get performance history
    pub async fn get_history(&self) -> Vec<PerformanceSnapshot> {
        self.history.read().await.iter().cloned().collect()
    }

    /// Get current tuning parameters
    pub async fn get_tuning_params(&self) -> AdaptiveTuningParams {
        self.tuning_params.read().await.clone()
    }

    /// Set memory pressure level (0-100)
    pub fn set_memory_pressure(&self, pressure: u64) {
        self.memory_pressure
            .store(pressure.min(100), Ordering::Relaxed);
    }
}

use std::fmt;

impl fmt::Debug for PerformanceMonitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PerformanceMonitor")
            .field("config", &self.config)
            .field("running", &self.running)
            .field("memory_pressure", &self.memory_pressure)
            .field("gc_requests", &self.gc_requests)
            .field("blocks_processed", &self.blocks_processed)
            .field("transactions_processed", &self.transactions_processed)
            .field("cache_hits", &self.cache_hits)
            .field("cache_misses", &self.cache_misses)
            .field(
                "optimization_hooks_count",
                &self
                    .optimization_hooks
                    .try_read()
                    .map(|h| h.len())
                    .unwrap_or(0),
            )
            .finish()
    }
}

impl Clone for PerformanceMonitor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            running: self.running.clone(),
            current_snapshot: self.current_snapshot.clone(),
            history: self.history.clone(),
            tuning_params: self.tuning_params.clone(),
            last_tuning: self.last_tuning.clone(),
            memory_pressure: self.memory_pressure.clone(),
            gc_requests: self.gc_requests.clone(),
            blocks_processed: self.blocks_processed.clone(),
            transactions_processed: self.transactions_processed.clone(),
            cache_hits: self.cache_hits.clone(),
            cache_misses: self.cache_misses.clone(),
            active_tasks: self.active_tasks.clone(),
            resource_usage: self.resource_usage.clone(),
            optimization_hooks: self.optimization_hooks.clone(),
        }
    }
}

/// Memory optimization utilities
pub struct MemoryOptimizer;

impl MemoryOptimizer {
    /// Optimize Vec capacity to reduce memory waste
    pub fn optimize_vec_capacity<T>(vec: &mut Vec<T>) {
        let len = vec.len();
        let capacity = vec.capacity();

        // If capacity is more than 2x the length and we have significant waste
        if capacity > len * 2 && capacity - len > 1000 {
            vec.shrink_to_fit();
        }
    }

    /// Optimize HashMap capacity
    pub fn optimize_hashmap_capacity<K, V>(map: &mut std::collections::HashMap<K, V>)
    where
        K: std::cmp::Eq + std::hash::Hash,
    {
        let len = map.len();
        let capacity = map.capacity();

        if capacity > len * 2 && capacity - len > 1000 {
            map.shrink_to_fit();
        }
    }

    /// Create a pre-sized Vec with optimal capacity
    pub fn create_optimized_vec<T>(estimated_size: usize) -> Vec<T> {
        // Add 25% buffer but cap at reasonable limits
        let optimal_capacity = (estimated_size * 125 / 100).min(1_000_000);
        Vec::with_capacity(optimal_capacity)
    }

    /// Create a pre-sized HashMap with optimal capacity
    pub fn create_optimized_hashmap<K, V>(
        estimated_size: usize,
    ) -> std::collections::HashMap<K, V> {
        let optimal_capacity = (estimated_size * 125 / 100).min(1_000_000);
        std::collections::HashMap::with_capacity(optimal_capacity)
    }
}
