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
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{interval_at, Instant as TokioInstant, Interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

/// Microsecond precision timing constants
pub const MICROSECOND_PRECISION: u64 = 1_000; // 1ms = 1000 microseconds
pub const TARGET_BLOCK_TIME_US: u64 = 28_000; // 28ms in microseconds (35.7 BPS)
pub const MIN_BLOCK_TIME_US: u64 = 20_000; // 20ms minimum (50 BPS max)
pub const MAX_BLOCK_TIME_US: u64 = 40_000; // 40ms maximum for emergency (25 BPS min)
pub const MINING_TICK_PRECISION_US: u64 = 50; // 50μs mining tick precision (improved)
pub const TX_PROCESSING_QUANTUM_US: u64 = 25; // 25μs transaction processing quantum (improved)

/// High-precision timing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingMetrics {
    pub block_times_us: Vec<u64>,
    pub mining_intervals_us: Vec<u64>,
    pub tx_processing_times_us: Vec<u64>,
    pub average_block_time_us: u64,
    pub blocks_per_second: u64, // Scaled by QANTO_SCALE
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
    is_running: AtomicBool,
    tick_count: AtomicU64,
    // New: High-precision interval timer
    interval_handle: Arc<std::sync::Mutex<Option<Interval>>>,
    last_tick_time: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl MicrosecondTimer {
    /// Create a new microsecond timer with target interval
    pub fn new(target_interval_us: u64) -> Self {
        Self {
            start_time: Instant::now(),
            target_interval_us: AtomicU64::new(target_interval_us),
            actual_intervals_us: Arc::new(std::sync::Mutex::new(Vec::new())),
            is_running: AtomicBool::new(false),
            tick_count: AtomicU64::new(0),
            interval_handle: Arc::new(std::sync::Mutex::new(None)),
            last_tick_time: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Start the timer with high-precision interval
    pub fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);
        let target_us = self.target_interval_us.load(Ordering::Relaxed);

        // Create high-precision interval timer
        let start_time = TokioInstant::now() + Duration::from_micros(target_us);
        let mut interval = interval_at(start_time, Duration::from_micros(target_us));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        if let Ok(mut handle) = self.interval_handle.lock() {
            *handle = Some(interval);
        }

        debug!(
            "MicrosecondTimer started with high-precision interval: {}μs",
            target_us
        );
    }

    /// Stop the timer
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);

        // Clear interval handle
        if let Ok(mut handle) = self.interval_handle.lock() {
            *handle = None;
        }

        debug!(
            "MicrosecondTimer stopped after {} ticks",
            self.tick_count.load(Ordering::Relaxed)
        );
    }

    /// Get precise tick using tokio::time::interval for zero-drift timing
    /// High-precision tick with microsecond accuracy
    pub async fn tick(&self) -> Duration {
        let tick_start = Instant::now();

        // Use high-precision interval timer instead of sleep
        let should_use_interval = {
            let handle = self.interval_handle.lock().unwrap();
            handle.is_some()
        };

        if should_use_interval {
            let mut interval_opt = {
                let mut handle = self.interval_handle.lock().unwrap();
                handle.take()
            };

            if let Some(ref mut interval) = interval_opt {
                interval.tick().await;
                // Put the interval back
                let mut handle = self.interval_handle.lock().unwrap();
                *handle = Some(interval_opt.unwrap());
            }
        } else {
            // Fallback to sleep if interval not initialized
            let target_us = self.target_interval_us.load(Ordering::Relaxed);
            tokio::time::sleep(Duration::from_micros(target_us)).await;
        }

        let actual_duration = tick_start.elapsed();
        let actual_us = actual_duration.as_micros() as u64;

        // Record actual interval for performance monitoring
        if let Ok(mut intervals) = self.actual_intervals_us.lock() {
            intervals.push(actual_us);
            // Keep only last 1000 intervals for analysis
            if intervals.len() > 1000 {
                intervals.remove(0);
            }
        }

        // Update last tick time for drift analysis
        if let Ok(mut last_tick) = self.last_tick_time.lock() {
            *last_tick = Some(tick_start);
        }

        self.tick_count.fetch_add(1, Ordering::Relaxed);
        actual_duration
    }

    /// Reset timer for immediate tick with interval recreation
    pub fn reset_for_immediate_tick(&self) {
        let target_us = self.target_interval_us.load(Ordering::Relaxed);

        // Recreate interval for immediate next tick
        let immediate_start = TokioInstant::now();
        let mut interval = interval_at(immediate_start, Duration::from_micros(target_us));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        if let Ok(mut handle) = self.interval_handle.lock() {
            *handle = Some(interval);
        }

        // Reset tick count to indicate immediate reset
        self.tick_count.store(0, Ordering::Relaxed);
        debug!(
            "Timer reset for immediate tick with interval recreation - next tick will be immediate"
        );
    }

    /// Update target interval dynamically
    pub fn update_target_interval(&self, new_target_us: u64) {
        let old_target = self
            .target_interval_us
            .swap(new_target_us, Ordering::Relaxed);

        // Recreate interval with new target if running
        if self.is_running.load(Ordering::Relaxed) {
            let start_time = TokioInstant::now() + Duration::from_micros(new_target_us);
            let mut interval = interval_at(start_time, Duration::from_micros(new_target_us));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            if let Ok(mut handle) = self.interval_handle.lock() {
                *handle = Some(interval);
            }
        }

        debug!(
            "Timer target interval updated: {}μs -> {}μs",
            old_target, new_target_us
        );
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

        // Integer variance: sum((x - avg)^2) / n
        let variance_scaled: u128 = intervals
            .iter()
            .map(|&x| {
                let diff = x.abs_diff(avg);
                diff as u128 * diff as u128
            })
            .sum::<u128>()
            / intervals.len() as u128;

        // Integer sqrt for std_dev
        // Integer sqrt for std_dev (Newton's method or simple estimation)
        let std_dev = if variance_scaled == 0 {
            0
        } else {
            let mut x = variance_scaled / 2 + 1;
            let mut y = (x + variance_scaled / x) / 2;
            while y < x {
                x = y;
                y = (x + variance_scaled / x) / 2;
            }
            x as u64
        };
        // Wait, I should use a custom isqrt if crate doesn't have it.
        // Actually, let's use the property that std_dev^2 = variance.

        let precision_percentage = if target == 0 {
            0
        } else {
            let mape_scaled = intervals
                .iter()
                .map(|&x| {
                    let diff = (x as i64).abs_diff(target as i64);
                    (diff as u128 * crate::QANTO_SCALE) / target as u128
                })
                .sum::<u128>()
                / intervals.len() as u128;

            // Convert to percentage and clamp to [0, 100]
            if mape_scaled > crate::QANTO_SCALE {
                0
            } else {
                ((crate::QANTO_SCALE - mape_scaled) * 100) / crate::QANTO_SCALE
            }
        };

        TimingStats {
            target_interval_us: target,
            average_interval_us: avg,
            min_interval_us: min,
            max_interval_us: max,
            std_deviation_us: std_dev,
            drift_us: avg as i64 - target as i64,
            precision_percentage: precision_percentage as u64,
            tick_count: self.tick_count.load(Ordering::Relaxed),
        }
    }
}

/// Timing statistics structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimingStats {
    pub target_interval_us: u64,
    pub average_interval_us: u64,
    pub min_interval_us: u64,
    pub max_interval_us: u64,
    pub std_deviation_us: u64,
    pub drift_us: i64,
    pub precision_percentage: u64,
    pub tick_count: u64,
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
    // Adaptive timing fields
    load_factor: AtomicU64, // 0-100 representing system load percentage
    adaptive_interval_us: AtomicU64, // Current adaptive interval
    performance_history: Arc<std::sync::Mutex<VecDeque<u128>>>, // BPS history for trend analysis
    last_adjustment_time: AtomicU64,
    // New: Predictive scheduling fields
    moving_average_window: Arc<std::sync::Mutex<VecDeque<u64>>>, // Block time moving average
    predicted_next_block_time: AtomicU64, // Predicted optimal next block time
    difficulty_adjustment_factor: Arc<std::sync::Mutex<u128>>, // Dynamic difficulty multiplier
    consecutive_fast_blocks: AtomicU64,   // Counter for consecutive fast blocks
    consecutive_slow_blocks: AtomicU64,   // Counter for consecutive slow blocks
}

impl Default for BlockTimingCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockTimingCoordinator {
    /// Create new timing coordinator for 32+ BPS with predictive scheduling
    pub fn new() -> Self {
        Self {
            block_timer: MicrosecondTimer::new(TARGET_BLOCK_TIME_US),
            mining_timer: MicrosecondTimer::new(MINING_TICK_PRECISION_US),
            tx_processing_timer: MicrosecondTimer::new(TX_PROCESSING_QUANTUM_US),
            last_block_time: AtomicU64::new(0),
            target_bps: AtomicU64::new(32), // 32+ BPS target
            actual_bps: AtomicU64::new(0),
            load_factor: AtomicU64::new(50), // Start at 50% load
            adaptive_interval_us: AtomicU64::new(TARGET_BLOCK_TIME_US),
            performance_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(100))),
            last_adjustment_time: AtomicU64::new(0),
            // Initialize predictive scheduling
            moving_average_window: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(20))),
            predicted_next_block_time: AtomicU64::new(TARGET_BLOCK_TIME_US),
            difficulty_adjustment_factor: Arc::new(std::sync::Mutex::new(crate::Q_SCALE)),
            consecutive_fast_blocks: AtomicU64::new(0),
            consecutive_slow_blocks: AtomicU64::new(0),
        }
    }

    /// Start all timing systems with predictive scheduling
    pub async fn start(&self) {
        info!("Starting BlockTimingCoordinator for 32+ BPS with predictive scheduling and adaptive difficulty");
        self.block_timer.start();
        self.mining_timer.start();
        self.tx_processing_timer.start();

        // Start BPS monitoring task
        let coordinator = self.clone();
        tokio::spawn(async move {
            coordinator.monitor_bps().await;
        });

        // Start adaptive timing adjustment task
        let coordinator_adaptive = self.clone();
        tokio::spawn(async move {
            coordinator_adaptive.adaptive_timing_loop().await;
        });

        // Start predictive scheduling task
        let coordinator_predictive = self.clone();
        tokio::spawn(async move {
            coordinator_predictive.predictive_scheduling_loop().await;
        });
    }

    /// Predictive scheduling loop using moving averages
    async fn predictive_scheduling_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(100)); // Update every 100ms
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            self.update_predictive_scheduling().await;
        }
    }
    /// Safe weighted average calculation using integer math
    fn weighted_average(values: &[u64], weights: &[u128]) -> Option<u128> {
        if values.is_empty() || weights.is_empty() || values.len() != weights.len() {
            return None;
        }

        let mut weighted_sum = 0u128;
        let mut weight_sum = 0u128;

        for (&value, &weight) in values.iter().zip(weights.iter()) {
            weighted_sum += value as u128 * weight;
            weight_sum += weight;
        }

        if weight_sum > 0 {
            Some(weighted_sum / weight_sum)
        } else {
            None
        }
    }

    /// Safe trend calculation using integer arithmetic
    fn calculate_safe_trend(recent_values: &[u64], older_values: &[u64]) -> Option<u128> {
        if recent_values.is_empty() || older_values.is_empty() {
            return None;
        }

        let recent_avg =
            recent_values.iter().map(|&x| x as u128).sum::<u128>() / recent_values.len() as u128;
        let older_avg =
            older_values.iter().map(|&x| x as u128).sum::<u128>() / older_values.len() as u128;

        if older_avg > 0 {
            Some((recent_avg * crate::Q_SCALE) / older_avg)
        } else {
            None
        }
    }

    /// Update predictive scheduling based on recent block times
    async fn update_predictive_scheduling(&self) {
        // Compute prediction while holding the lock, but ensure we drop the guard before any await
        let mut has_enough_samples = false;

        if let Ok(window) = self.moving_average_window.lock() {
            if window.len() < 5 {
                // Not enough samples, return early (no await involved while holding the lock)
                return;
            }
            has_enough_samples = true;

            // Calculate weighted moving average using safe integer arithmetic
            let values: Vec<u64> = window.iter().copied().collect();
            let weights: Vec<u128> = (1..=values.len()).map(|i| i as u128).collect();

            if let Some(predicted_time_scaled) = Self::weighted_average(&values, &weights) {
                let predicted_time = predicted_time_scaled as u64;

                // Apply trend analysis for better prediction
                if window.len() >= 10 {
                    let recent_values: Vec<u64> = window.iter().rev().take(5).copied().collect();
                    let older_values: Vec<u64> =
                        window.iter().rev().skip(5).take(5).copied().collect();

                    if let Some(trend_factor) =
                        Self::calculate_safe_trend(&recent_values, &older_values)
                    {
                        let trend_adjusted_time =
                            (predicted_time as u128 * trend_factor) / crate::Q_SCALE;
                        let clamped = (trend_adjusted_time as u64)
                            .clamp(MIN_BLOCK_TIME_US, MAX_BLOCK_TIME_US);
                        self.predicted_next_block_time
                            .store(clamped, Ordering::Relaxed);
                    } else {
                        self.predicted_next_block_time
                            .store(predicted_time, Ordering::Relaxed);
                    }
                } else {
                    self.predicted_next_block_time
                        .store(predicted_time, Ordering::Relaxed);
                }
            }
        }

        // Drop lock before awaiting to keep future Send
        if has_enough_samples {
            // The call below performs .await without holding any MutexGuard
            self.update_difficulty_adjustment().await;
        }
    }

    /// Update difficulty adjustment factor based on block timing patterns
    async fn update_difficulty_adjustment(&self) {
        let target_time = TARGET_BLOCK_TIME_US;
        let predicted_time = self.predicted_next_block_time.load(Ordering::Relaxed);
        let fast_blocks = self.consecutive_fast_blocks.load(Ordering::Relaxed);
        let slow_blocks = self.consecutive_slow_blocks.load(Ordering::Relaxed);

        if let Ok(mut difficulty_factor) = self.difficulty_adjustment_factor.lock() {
            // Adjust difficulty based on consecutive fast/slow blocks
            if fast_blocks >= 3 {
                // Too many fast blocks - increase difficulty (scaled)
                *difficulty_factor = (*difficulty_factor * 105 / 100).min(crate::Q_SCALE * 2);
                self.consecutive_fast_blocks.store(0, Ordering::Relaxed);
                debug!(
                    "Increased difficulty factor to {} units due to {} consecutive fast blocks",
                    *difficulty_factor, fast_blocks
                );
            } else if slow_blocks >= 3 {
                // Too many slow blocks - decrease difficulty (scaled)
                *difficulty_factor = (*difficulty_factor * 95 / 100).max(crate::Q_SCALE / 2);
                self.consecutive_slow_blocks.store(0, Ordering::Relaxed);
                debug!(
                    "Decreased difficulty factor to {} units due to {} consecutive slow blocks",
                    *difficulty_factor, slow_blocks
                );
            }

            // Fine-tune based on prediction vs target
            // prediction_ratio = predicted_time / target_time (scaled by Q_SCALE)
            let prediction_ratio = (predicted_time as u128 * crate::Q_SCALE) / target_time as u128;
            if prediction_ratio > (120 * crate::Q_SCALE / 100) {
                // Predicted time too slow
                *difficulty_factor = (*difficulty_factor * 98) / 100;
            } else if prediction_ratio < (80 * crate::Q_SCALE / 100) {
                // Predicted time too fast
                *difficulty_factor = (*difficulty_factor * 102) / 100;
            }

            *difficulty_factor = difficulty_factor.clamp(crate::Q_SCALE / 2, crate::Q_SCALE * 2);
        }
    }

    /// Get current difficulty adjustment factor for mining
    pub fn get_difficulty_adjustment_factor(&self) -> u128 {
        self.difficulty_adjustment_factor
            .lock()
            .map(|f| *f)
            .unwrap_or(crate::Q_SCALE)
    }

    /// Get predicted next block time for optimization
    pub fn get_predicted_block_time(&self) -> u64 {
        self.predicted_next_block_time.load(Ordering::Relaxed)
    }

    /// Adaptive timing adjustment loop
    async fn adaptive_timing_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(250)); // Adjust every 250ms for faster response
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            self.adjust_timing_based_on_load().await;
        }
    }

    /// Adjust timing intervals based on current system load and performance
    async fn adjust_timing_based_on_load(&self) {
        let current_bps = self.actual_bps.load(Ordering::Relaxed) as u128 * crate::Q_SCALE;
        let target_bps = self.target_bps.load(Ordering::Relaxed) as u128 * crate::Q_SCALE;
        let load_factor = self.load_factor.load(Ordering::Relaxed) as u128 * crate::Q_SCALE / 100;

        // Update performance history
        if let Ok(mut history) = self.performance_history.lock() {
            history.push_back(current_bps);
            if history.len() > 100 {
                history.pop_front();
            }
        }

        // Calculate performance ratio and trend
        let performance_ratio = if target_bps > 0 {
            (current_bps * crate::Q_SCALE) / target_bps
        } else {
            0
        };
        let trend = self.calculate_performance_trend();

        // Determine new adaptive interval based on multiple factors
        let current_interval = self.adaptive_interval_us.load(Ordering::Relaxed);
        let mut new_interval = current_interval;

        // Adaptive logic: 15-35ms range based on load and performance (more aggressive)
        if performance_ratio < (85 * crate::Q_SCALE / 100) {
            // Performance below 85% of target - reduce interval (faster blocks)
            let reduction_factor =
                (92 * crate::Q_SCALE / 100).saturating_sub(load_factor * 12 / 100);
            new_interval = ((current_interval as u128 * reduction_factor) / crate::Q_SCALE) as u64;
            new_interval = new_interval.max(15_000); // Min 15ms (66.7 BPS)
        } else if performance_ratio > (115 * crate::Q_SCALE / 100)
            && trend > (8 * crate::Q_SCALE / 100) as i128
        {
            // Performance above 115% and trending up - can increase interval slightly
            let increase_factor = (103 * crate::Q_SCALE / 100) + (load_factor * 2 / 100);
            new_interval = ((current_interval as u128 * increase_factor) / crate::Q_SCALE) as u64;
            new_interval = new_interval.min(35_000); // Max 35ms (28.6 BPS)
        }

        // Apply load-based fine-tuning (more aggressive)
        if load_factor > 85 * crate::Q_SCALE / 100 {
            // High load - be more conservative
            new_interval = new_interval.max(30_000); // At least 30ms under high load
        } else if load_factor < 25 * crate::Q_SCALE / 100 {
            // Low load - can be very aggressive
            new_interval = new_interval.min(20_000); // At most 20ms under low load (50 BPS)
        }

        // Only adjust if change is significant (>3% or >1ms) - more responsive
        let change_threshold = (current_interval as u128 * 3 / 100).max(1000) as u64;
        if (new_interval as i64 - current_interval as i64).abs() > change_threshold as i64 {
            self.adaptive_interval_us
                .store(new_interval, Ordering::Relaxed);
            self.block_timer
                .target_interval_us
                .store(new_interval, Ordering::Relaxed);

            let now_us = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            self.last_adjustment_time.store(now_us, Ordering::Relaxed);

            debug!(
                "Adaptive timing adjustment: {}μs -> {}μs (load: {} units, BPS: {}, trend: {})",
                current_interval, new_interval, load_factor, current_bps, trend
            );
        }
    }

    /// Calculate performance trend from recent history using integer math
    fn calculate_performance_trend(&self) -> i128 {
        if let Ok(history) = self.performance_history.lock() {
            if history.len() < 10 {
                return 0;
            }

            let recent_avg: u128 = history.iter().rev().take(5).sum::<u128>() / 5;
            let older_avg: u128 = history.iter().rev().skip(5).take(5).sum::<u128>() / 5;

            if older_avg > 0 {
                if recent_avg >= older_avg {
                    ((recent_avg - older_avg) * crate::Q_SCALE / older_avg) as i128
                } else {
                    -(((older_avg - recent_avg) * crate::Q_SCALE / older_avg) as i128)
                }
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Update system load factor (0-100)
    pub fn update_load_factor(&self, load_percentage: u64) {
        let clamped_load = load_percentage.min(100);
        self.load_factor.store(clamped_load, Ordering::Relaxed);
    }

    /// Get current adaptive interval
    pub fn get_adaptive_interval_us(&self) -> u64 {
        self.adaptive_interval_us.load(Ordering::Relaxed)
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
            // BPS scaled by QANTO_SCALE
            let bps = if block_interval_us > 0 {
                (1_000_000 * crate::Q_SCALE) / block_interval_us as u128
            } else {
                0
            };
            self.actual_bps.store(bps as u64, Ordering::Relaxed);

            debug!(
                "Block completed in {}μs, current BPS: {}",
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
                let performance_ratio_scaled = (actual as u128 * crate::Q_SCALE) / target as u128;

                if performance_ratio_scaled >= crate::Q_SCALE {
                    info!(
                        "✅ BPS Performance: {} units (target: {} units) - {}%",
                        actual,
                        target,
                        (performance_ratio_scaled * 100) / crate::Q_SCALE
                    );
                } else if performance_ratio_scaled >= (80 * crate::Q_SCALE / 100) {
                    warn!(
                        "⚠️  BPS Performance: {} units (target: {} units) - {}%",
                        actual,
                        target,
                        (performance_ratio_scaled * 100) / crate::Q_SCALE
                    );
                } else {
                    error!(
                        "❌ BPS Performance: {} units (target: {} units) - {}%",
                        actual,
                        target,
                        (performance_ratio_scaled * 100) / crate::Q_SCALE
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
            load_factor: AtomicU64::new(self.load_factor.load(Ordering::Relaxed)),
            adaptive_interval_us: AtomicU64::new(self.adaptive_interval_us.load(Ordering::Relaxed)),
            performance_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(100))),
            last_adjustment_time: AtomicU64::new(self.last_adjustment_time.load(Ordering::Relaxed)),
            // Clone predictive scheduling fields
            moving_average_window: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(20))),
            predicted_next_block_time: AtomicU64::new(
                self.predicted_next_block_time.load(Ordering::Relaxed),
            ),
            difficulty_adjustment_factor: Arc::new(std::sync::Mutex::new(
                self.difficulty_adjustment_factor
                    .lock()
                    .map(|f| *f)
                    .unwrap_or(crate::Q_SCALE),
            )),
            consecutive_fast_blocks: AtomicU64::new(
                self.consecutive_fast_blocks.load(Ordering::Relaxed),
            ),
            consecutive_slow_blocks: AtomicU64::new(
                self.consecutive_slow_blocks.load(Ordering::Relaxed),
            ),
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
    pub fn calculate_bps_from_us(block_time_us: u64) -> u128 {
        if block_time_us == 0 {
            0
        } else {
            (1_000_000 * crate::Q_SCALE) / block_time_us as u128
        }
    }

    pub fn calculate_optimal_mining_interval_us(target_bps: u128) -> u64 {
        if target_bps == 0 {
            return 0;
        }
        let block_time_us = ((1_000_000 * crate::Q_SCALE) / target_bps) as u64;
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
