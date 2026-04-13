use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Mining performance metrics and monitoring
#[derive(Debug, Clone)]
pub struct MiningMetrics {
    // Success metrics
    pub blocks_mined: Arc<AtomicU64>,
    pub total_mining_time: Arc<AtomicU64>, // in milliseconds
    pub successful_attempts: Arc<AtomicU64>,

    // Failure metrics
    pub failed_attempts: Arc<AtomicU64>,
    pub mining_errors: Arc<AtomicU64>,
    pub task_panics: Arc<AtomicU64>,
    pub block_rejections: Arc<AtomicU64>,
    pub mempool_timeouts: Arc<AtomicU64>,

    // Performance metrics
    pub average_mining_time: Arc<AtomicU64>, // in milliseconds
    pub min_mining_time: Arc<AtomicU64>,     // in milliseconds
    pub max_mining_time: Arc<AtomicU64>,     // in milliseconds
    pub retry_count: Arc<AtomicU32>,

    // Operational metrics
    pub last_success_time: Arc<RwLock<Option<Instant>>>,
    pub last_failure_time: Arc<RwLock<Option<Instant>>>,
    pub mining_start_time: Arc<RwLock<Option<Instant>>>,
}

impl Default for MiningMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl MiningMetrics {
    pub fn new() -> Self {
        Self {
            blocks_mined: Arc::new(AtomicU64::new(0)),
            total_mining_time: Arc::new(AtomicU64::new(0)),
            successful_attempts: Arc::new(AtomicU64::new(0)),
            failed_attempts: Arc::new(AtomicU64::new(0)),
            mining_errors: Arc::new(AtomicU64::new(0)),
            task_panics: Arc::new(AtomicU64::new(0)),
            block_rejections: Arc::new(AtomicU64::new(0)),
            mempool_timeouts: Arc::new(AtomicU64::new(0)),
            average_mining_time: Arc::new(AtomicU64::new(0)),
            min_mining_time: Arc::new(AtomicU64::new(u64::MAX)),
            max_mining_time: Arc::new(AtomicU64::new(0)),
            retry_count: Arc::new(AtomicU32::new(0)),
            last_success_time: Arc::new(RwLock::new(None)),
            last_failure_time: Arc::new(RwLock::new(None)),
            mining_start_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Record a successful mining operation
    pub async fn record_success(&self, mining_duration: Duration, retries: u32) {
        let duration_ms = mining_duration.as_millis() as u64;

        self.blocks_mined.fetch_add(1, Ordering::Relaxed);
        self.successful_attempts.fetch_add(1, Ordering::Relaxed);
        self.total_mining_time
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.retry_count.fetch_add(retries, Ordering::Relaxed);

        // Update min/max times
        let current_min = self.min_mining_time.load(Ordering::Relaxed);
        if duration_ms < current_min {
            self.min_mining_time.store(duration_ms, Ordering::Relaxed);
        }

        let current_max = self.max_mining_time.load(Ordering::Relaxed);
        if duration_ms > current_max {
            self.max_mining_time.store(duration_ms, Ordering::Relaxed);
        }

        // Update average
        let total_successes = self.successful_attempts.load(Ordering::Relaxed);
        let total_time = self.total_mining_time.load(Ordering::Relaxed);
        if total_successes > 0 {
            self.average_mining_time
                .store(total_time / total_successes, Ordering::Relaxed);
        }

        *self.last_success_time.write().await = Some(Instant::now());
    }

    /// Record a mining failure
    pub async fn record_failure(&self, failure_type: MiningFailureType) {
        self.failed_attempts.fetch_add(1, Ordering::Relaxed);

        match failure_type {
            MiningFailureType::MiningError => self.mining_errors.fetch_add(1, Ordering::Relaxed),
            MiningFailureType::TaskPanic => self.task_panics.fetch_add(1, Ordering::Relaxed),
            MiningFailureType::BlockRejection => {
                self.block_rejections.fetch_add(1, Ordering::Relaxed)
            }
            MiningFailureType::MempoolTimeout => {
                self.mempool_timeouts.fetch_add(1, Ordering::Relaxed)
            }
        };

        *self.last_failure_time.write().await = Some(Instant::now());
    }

    /// Get current success rate as a percentage
    pub fn get_success_rate(&self) -> f64 {
        let successes = self.successful_attempts.load(Ordering::Relaxed) as f64;
        let total = successes + self.failed_attempts.load(Ordering::Relaxed) as f64;

        if total > 0.0 {
            (successes / total) * 100.0
        } else {
            0.0
        }
    }

    /// Get average mining time in seconds
    pub fn get_average_mining_time_secs(&self) -> f64 {
        let avg_ms = self.average_mining_time.load(Ordering::Relaxed);
        avg_ms as f64 / 1000.0
    }

    /// Get mining rate (blocks per hour)
    pub async fn get_mining_rate(&self) -> f64 {
        let start_time = self.mining_start_time.read().await;
        if let Some(start) = *start_time {
            let elapsed = start.elapsed().as_secs_f64();
            let blocks = self.blocks_mined.load(Ordering::Relaxed) as f64;

            if elapsed > 0.0 {
                (blocks / elapsed) * 3600.0 // blocks per hour
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Start tracking mining session
    pub async fn start_session(&self) {
        *self.mining_start_time.write().await = Some(Instant::now());
    }

    /// Log comprehensive mining statistics
    pub async fn log_statistics(&self) {
        let success_rate = self.get_success_rate();
        let avg_time = self.get_average_mining_time_secs();
        let mining_rate = self.get_mining_rate().await;
        let blocks_mined = self.blocks_mined.load(Ordering::Relaxed);
        let failed_attempts = self.failed_attempts.load(Ordering::Relaxed);
        let mining_errors = self.mining_errors.load(Ordering::Relaxed);
        let task_panics = self.task_panics.load(Ordering::Relaxed);
        let block_rejections = self.block_rejections.load(Ordering::Relaxed);
        let mempool_timeouts = self.mempool_timeouts.load(Ordering::Relaxed);
        let retry_count = self.retry_count.load(Ordering::Relaxed);

        info!(
            "MINING METRICS: Blocks: {}, Success Rate: {:.1}%, Avg Time: {:.2}s, Rate: {:.2} blocks/hr",
            blocks_mined, success_rate, avg_time, mining_rate
        );

        if failed_attempts > 0 {
            info!(
                "MINING FAILURES: Total: {}, Errors: {}, Panics: {}, Rejections: {}, Timeouts: {}, Retries: {}",
                failed_attempts, mining_errors, task_panics, block_rejections, mempool_timeouts, retry_count
            );
        }

        // Warn if success rate is low
        if success_rate < 80.0 && blocks_mined + failed_attempts > 10 {
            warn!(
                "MINING PERFORMANCE WARNING: Success rate is low at {:.1}%. Consider investigating mining issues.",
                success_rate
            );
        }
    }

    /// Get detailed metrics as a structured format
    pub async fn get_detailed_metrics(&self) -> DetailedMiningMetrics {
        DetailedMiningMetrics {
            blocks_mined: self.blocks_mined.load(Ordering::Relaxed),
            successful_attempts: self.successful_attempts.load(Ordering::Relaxed),
            failed_attempts: self.failed_attempts.load(Ordering::Relaxed),
            mining_errors: self.mining_errors.load(Ordering::Relaxed),
            task_panics: self.task_panics.load(Ordering::Relaxed),
            block_rejections: self.block_rejections.load(Ordering::Relaxed),
            mempool_timeouts: self.mempool_timeouts.load(Ordering::Relaxed),
            success_rate: self.get_success_rate(),
            average_mining_time_secs: self.get_average_mining_time_secs(),
            min_mining_time_ms: self.min_mining_time.load(Ordering::Relaxed),
            max_mining_time_ms: self.max_mining_time.load(Ordering::Relaxed),
            mining_rate_per_hour: self.get_mining_rate().await,
            total_retries: self.retry_count.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedMiningMetrics {
    pub blocks_mined: u64,
    pub successful_attempts: u64,
    pub failed_attempts: u64,
    pub mining_errors: u64,
    pub task_panics: u64,
    pub block_rejections: u64,
    pub mempool_timeouts: u64,
    pub success_rate: f64,
    pub average_mining_time_secs: f64,
    pub min_mining_time_ms: u64,
    pub max_mining_time_ms: u64,
    pub mining_rate_per_hour: f64,
    pub total_retries: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum MiningFailureType {
    MiningError,
    TaskPanic,
    BlockRejection,
    MempoolTimeout,
}
