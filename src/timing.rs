// src/timing.rs

//! --- Qanto Microsecond-Level Timing System ---
//! v1.0.0 - High-Performance Timing for 32+ BPS
//!
//! This module provides microsecond-precision timing capabilities for:
//! - 31ms block time targets (32.25+ BPS)
//! - 10M+ TPS transaction processing
//! - Sub-millisecond latency optimization
//! - Continuous mining synchronization

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{interval_at, Interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

/// Microsecond precision timing constants
pub const MICROSECOND_PRECISION: u64 = 1_000; // 1ms = 1000 microseconds
pub const TARGET_BLOCK_TIME_US: u64 = 31_000; // 31ms in microseconds
pub const MIN_BLOCK_TIME_US: u64 = 25_000; // 25ms minimum
pub const MAX_BLOCK_TIME_US: u64 = 50_000; // 50ms maximum for emergency
pub const MINING_TICK_PRECISION_US: u64 = 100; // 100μs mining tick precision
pub const TX_PROCESSING_QUANTUM_US: u64 = 50; // 50μs transaction processing quantum

/// High-precision timing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingMetrics {
    pub block_times_us: Vec<u64>,
    pub mining_intervals_us: Vec<u64>,
    pub tx_processing_times_us: Vec<u64>,
    pub average_block_time_us: u64,
    pub blocks_per_second: f64,
    pub timing_drift_us: i64,
    pub precision_violations: u64,
}

/// Microsecond-precision timer for blockchain operations
#[derive(Debug)]
pub struct MicrosecondTimer {
    #[allow(dead_code)]
    start_time: Instant,
    target_interval_us: AtomicU64,
    actual_intervals_us: Arc<std::sync::Mutex<Vec<u64>>>,
    drift_correction_us: AtomicU64,
    is_running: AtomicBool,
    tick_count: AtomicU64,
}

impl MicrosecondTimer {
    /// Create a new microsecond timer with target interval
    pub fn new(target_interval_us: u64) -> Self {
        Self {
            start_time: Instant::now(),
            target_interval_us: AtomicU64::new(target_interval_us),
            actual_intervals_us: Arc::new(std::sync::Mutex::new(Vec::new())),
            drift_correction_us: AtomicU64::new(0),
            is_running: AtomicBool::new(false),
            tick_count: AtomicU64::new(0),
        }
    }

    /// Start the timer with drift correction
    pub fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);
        debug!(
            "MicrosecondTimer started with target interval: {}μs",
            self.target_interval_us.load(Ordering::Relaxed)
        );
    }

    /// Stop the timer
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        debug!(
            "MicrosecondTimer stopped after {} ticks",
            self.tick_count.load(Ordering::Relaxed)
        );
    }

    /// Get precise tick with drift correction
    pub async fn tick(&self) -> Duration {
        if !self.is_running.load(Ordering::Relaxed) {
            return Duration::from_micros(0);
        }

        let tick_start = Instant::now();
        let target_us = self.target_interval_us.load(Ordering::Relaxed);
        let drift_correction = self.drift_correction_us.load(Ordering::Relaxed);
        let tick_count = self.tick_count.load(Ordering::Relaxed);

        // Apply drift correction
        let adjusted_target_us = if drift_correction > 0 {
            target_us.saturating_sub(drift_correction)
        } else {
            target_us
        };

        // High-precision sleep
        tokio::time::sleep(Duration::from_micros(adjusted_target_us)).await;

        let actual_duration = tick_start.elapsed();
        let actual_us = actual_duration.as_micros() as u64;

        // Record actual interval for drift analysis
        if let Ok(mut intervals) = self.actual_intervals_us.lock() {
            intervals.push(actual_us);
            // Keep only last 1000 intervals for analysis
            if intervals.len() > 1000 {
                intervals.remove(0);
            }
        }

        // Only update drift correction if this wasn't an immediate tick reset
        // This prevents overwriting the immediate tick correction
        if tick_count > 0 {
            // Update drift correction
            let drift = actual_us as i64 - target_us as i64;
            if drift.abs() > 10 {
                // Only correct if drift > 10μs
                self.drift_correction_us.store(
                    (drift / 4).max(0) as u64, // Gradual correction
                    Ordering::Relaxed,
                );
            }
        } else {
            // This was an immediate tick, clear the drift correction for next time
            self.drift_correction_us.store(0, Ordering::Relaxed);
        }

        self.tick_count.fetch_add(1, Ordering::Relaxed);
        actual_duration
    }

    /// Reset timer for immediate tick (used when block is mined)
    pub fn reset_for_immediate_tick(&self) {
        // Set drift correction to the full target interval to make next tick immediate
        let target_us = self.target_interval_us.load(Ordering::Relaxed);
        self.drift_correction_us.store(target_us, Ordering::Relaxed);
        // Reset tick count to indicate this is an immediate reset
        self.tick_count.store(0, Ordering::Relaxed);
        debug!("Timer reset for immediate tick after block mining - next tick will be immediate");
    }

    /// Get timing statistics
    pub fn get_stats(&self) -> TimingStats {
        let intervals = self.actual_intervals_us.lock().unwrap();
        let target = self.target_interval_us.load(Ordering::Relaxed);

        if intervals.is_empty() {
            return TimingStats::default();
        }

        let sum: u64 = intervals.iter().sum();
        let avg = sum / intervals.len() as u64;
        let min = *intervals.iter().min().unwrap();
        let max = *intervals.iter().max().unwrap();

        let variance: f64 = intervals
            .iter()
            .map(|&x| (x as f64 - avg as f64).powi(2))
            .sum::<f64>()
            / intervals.len() as f64;
        let std_dev = variance.sqrt();

        TimingStats {
            target_interval_us: target,
            average_interval_us: avg,
            min_interval_us: min,
            max_interval_us: max,
            std_deviation_us: std_dev,
            drift_us: avg as i64 - target as i64,
            precision_percentage: 100.0 - (std_dev / target as f64 * 100.0),
            tick_count: self.tick_count.load(Ordering::Relaxed),
        }
    }
}

/// Timing statistics structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingStats {
    pub target_interval_us: u64,
    pub average_interval_us: u64,
    pub min_interval_us: u64,
    pub max_interval_us: u64,
    pub std_deviation_us: f64,
    pub drift_us: i64,
    pub precision_percentage: f64,
    pub tick_count: u64,
}

impl Default for TimingStats {
    fn default() -> Self {
        Self {
            target_interval_us: 0,
            average_interval_us: 0,
            min_interval_us: 0,
            max_interval_us: 0,
            std_deviation_us: 0.0,
            drift_us: 0,
            precision_percentage: 0.0,
            tick_count: 0,
        }
    }
}

/// High-performance block timing coordinator
#[derive(Debug)]
pub struct BlockTimingCoordinator {
    block_timer: MicrosecondTimer,
    mining_timer: MicrosecondTimer,
    tx_processing_timer: MicrosecondTimer,
    last_block_time: AtomicU64,
    target_bps: AtomicU64,
    actual_bps: AtomicU64,
}

impl Default for BlockTimingCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockTimingCoordinator {
    /// Create new timing coordinator for 32+ BPS
    pub fn new() -> Self {
        Self {
            block_timer: MicrosecondTimer::new(TARGET_BLOCK_TIME_US),
            mining_timer: MicrosecondTimer::new(MINING_TICK_PRECISION_US),
            tx_processing_timer: MicrosecondTimer::new(TX_PROCESSING_QUANTUM_US),
            last_block_time: AtomicU64::new(0),
            target_bps: AtomicU64::new(32), // 32+ BPS target
            actual_bps: AtomicU64::new(0),
        }
    }

    /// Start all timing systems
    pub async fn start(&self) {
        info!("Starting BlockTimingCoordinator for 32+ BPS performance");
        self.block_timer.start();
        self.mining_timer.start();
        self.tx_processing_timer.start();

        // Start BPS monitoring task
        let coordinator = self.clone();
        tokio::spawn(async move {
            coordinator.monitor_bps().await;
        });
    }

    /// Wait for next block timing window
    pub async fn wait_for_block_window(&self) -> Duration {
        info!("TIMING: Waiting for block window tick...");
        let duration = self.block_timer.tick().await;
        info!(
            "TIMING: Block window tick received! Duration: {:?}",
            duration
        );
        duration
    }

    /// Signal that a block was successfully mined - allows immediate next mining cycle
    pub fn signal_block_mined(&self) {
        info!("TIMING: Block mined signal received - resetting timer for immediate tick");
        // Reset the block timer to allow immediate next cycle
        self.block_timer.reset_for_immediate_tick();
        self.record_block_completion();
        info!("TIMING: Timer reset complete - next tick should be immediate");
    }

    /// Wait for next mining tick
    pub async fn wait_for_mining_tick(&self) -> Duration {
        self.mining_timer.tick().await
    }

    /// Wait for transaction processing quantum
    pub async fn wait_for_tx_quantum(&self) -> Duration {
        self.tx_processing_timer.tick().await
    }

    /// Record block completion time
    pub fn record_block_completion(&self) {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let last_time = self.last_block_time.swap(now_us, Ordering::Relaxed);

        if last_time > 0 {
            let block_interval_us = now_us - last_time;
            let bps = 1_000_000.0 / block_interval_us as f64; // Convert μs to BPS
            self.actual_bps.store(bps as u64, Ordering::Relaxed);

            debug!(
                "Block completed in {}μs, current BPS: {:.2}",
                block_interval_us, bps
            );
        }
    }

    /// Monitor BPS performance
    async fn monitor_bps(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let actual = self.actual_bps.load(Ordering::Relaxed);
            let target = self.target_bps.load(Ordering::Relaxed);

            if actual > 0 {
                let performance_ratio = actual as f64 / target as f64;

                if performance_ratio >= 1.0 {
                    info!(
                        "✅ BPS Performance: {:.2} BPS (target: {} BPS) - {:.1}%",
                        actual,
                        target,
                        performance_ratio * 100.0
                    );
                } else if performance_ratio >= 0.8 {
                    warn!(
                        "⚠️  BPS Performance: {:.2} BPS (target: {} BPS) - {:.1}%",
                        actual,
                        target,
                        performance_ratio * 100.0
                    );
                } else {
                    error!(
                        "❌ BPS Performance: {:.2} BPS (target: {} BPS) - {:.1}%",
                        actual,
                        target,
                        performance_ratio * 100.0
                    );
                }
            }
        }
    }

    /// Get comprehensive timing metrics
    pub fn get_comprehensive_metrics(&self) -> ComprehensiveTimingMetrics {
        ComprehensiveTimingMetrics {
            block_timing: self.block_timer.get_stats(),
            mining_timing: self.mining_timer.get_stats(),
            tx_processing_timing: self.tx_processing_timer.get_stats(),
            target_bps: self.target_bps.load(Ordering::Relaxed),
            actual_bps: self.actual_bps.load(Ordering::Relaxed),
            system_timestamp_us: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
        }
    }
}

impl Clone for BlockTimingCoordinator {
    fn clone(&self) -> Self {
        Self {
            block_timer: MicrosecondTimer::new(
                self.block_timer.target_interval_us.load(Ordering::Relaxed),
            ),
            mining_timer: MicrosecondTimer::new(
                self.mining_timer.target_interval_us.load(Ordering::Relaxed),
            ),
            tx_processing_timer: MicrosecondTimer::new(
                self.tx_processing_timer
                    .target_interval_us
                    .load(Ordering::Relaxed),
            ),
            last_block_time: AtomicU64::new(self.last_block_time.load(Ordering::Relaxed)),
            target_bps: AtomicU64::new(self.target_bps.load(Ordering::Relaxed)),
            actual_bps: AtomicU64::new(self.actual_bps.load(Ordering::Relaxed)),
        }
    }
}

/// Comprehensive timing metrics for all systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveTimingMetrics {
    pub block_timing: TimingStats,
    pub mining_timing: TimingStats,
    pub tx_processing_timing: TimingStats,
    pub target_bps: u64,
    pub actual_bps: u64,
    pub system_timestamp_us: u64,
}

/// Utility functions for timing operations
pub mod timing_utils {
    use super::*;

    /// Convert milliseconds to microseconds
    pub fn ms_to_us(ms: u64) -> u64 {
        ms * 1_000
    }

    /// Convert microseconds to milliseconds
    pub fn us_to_ms(us: u64) -> u64 {
        us / 1_000
    }

    /// Calculate BPS from block time in microseconds
    pub fn calculate_bps_from_us(block_time_us: u64) -> f64 {
        1_000_000.0 / block_time_us as f64
    }

    /// Calculate optimal mining interval for target BPS
    pub fn calculate_optimal_mining_interval_us(target_bps: f64) -> u64 {
        let block_time_us = (1_000_000.0 / target_bps) as u64;
        // Mining interval should be 1/4 of block time for optimal performance
        block_time_us / 4
    }

    /// Validate timing configuration for performance targets
    pub fn validate_timing_config(
        block_time_us: u64,
        mining_interval_us: u64,
        tx_quantum_us: u64,
    ) -> Result<(), String> {
        // Block time validation
        if block_time_us < MIN_BLOCK_TIME_US {
            return Err(format!(
                "Block time {block_time_us}μs is below minimum {MIN_BLOCK_TIME_US}μs"
            ));
        }

        if block_time_us > MAX_BLOCK_TIME_US {
            return Err(format!(
                "Block time {block_time_us}μs exceeds maximum {MAX_BLOCK_TIME_US}μs"
            ));
        }

        // Mining interval validation
        if mining_interval_us > block_time_us / 2 {
            return Err(format!(
                "Mining interval {mining_interval_us}μs is too large for block time {block_time_us}μs"
            ));
        }

        // Transaction quantum validation
        if tx_quantum_us > block_time_us / 10 {
            return Err(format!(
                "Transaction quantum {tx_quantum_us}μs is too large for block time {block_time_us}μs"
            ));
        }

        Ok(())
    }

    /// Get current system time in microseconds since UNIX epoch
    pub fn current_time_us() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Create high-precision tokio interval
    pub fn create_precision_interval(interval_us: u64) -> Interval {
        let duration = Duration::from_micros(interval_us);
        let start = tokio::time::Instant::now() + duration;
        let mut interval = interval_at(start, duration);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval
    }
}
