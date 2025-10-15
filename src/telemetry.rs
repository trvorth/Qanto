//! Telemetry module for Qanto blockchain
//! Provides centralized telemetry collection and reporting with configurable intervals.

use crate::metrics::{get_global_metrics, QantoMetrics};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use tracing::{debug, info};

/// Global static counter for hash attempts - incremented per hash attempt
pub static HASH_ATTEMPTS: AtomicU64 = AtomicU64::new(0);

/// Telemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Interval for hash rate sampling in seconds
    pub hash_rate_interval_secs: u64,
    /// Enable detailed telemetry logging
    pub enable_detailed_logging: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            hash_rate_interval_secs: 5,
            enable_detailed_logging: false,
        }
    }
}

/// Telemetry manager for coordinating metrics collection
pub struct TelemetryManager {
    config: TelemetryConfig,
    last_hash_attempts: AtomicU64,
    last_sample_time: AtomicU64,
}

impl TelemetryManager {
    pub fn new(config: TelemetryConfig) -> Self {
        Self {
            config,
            last_hash_attempts: AtomicU64::new(0),
            last_sample_time: AtomicU64::new(0),
        }
    }

    /// Start the telemetry collection task
    pub fn start_telemetry_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(&self);
        tokio::spawn(async move {
            manager.run_telemetry_loop().await;
        })
    }

    /// Main telemetry collection loop
    async fn run_telemetry_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.hash_rate_interval_secs));

        info!(
            "Starting telemetry collection with {}s hash rate sampling interval",
            self.config.hash_rate_interval_secs
        );

        loop {
            interval.tick().await;
            self.collect_hash_rate_metrics();

            if self.config.enable_detailed_logging {
                self.log_detailed_metrics();
            }
        }
    }

    /// Collect and update hash rate metrics
    fn collect_hash_rate_metrics(&self) {
        let current_time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let current_attempts = HASH_ATTEMPTS.load(Ordering::Relaxed);
        let last_attempts = self.last_hash_attempts.load(Ordering::Relaxed);
        let last_time_ms = self.last_sample_time.load(Ordering::Relaxed);

        // Calculate hash rate if we have previous data
        if last_time_ms > 0 {
            let attempts_delta = current_attempts.saturating_sub(last_attempts);
            let time_delta_ms = current_time_ms.saturating_sub(last_time_ms);

            if time_delta_ms > 0 {
                let hash_rate_hps = QantoMetrics::compute_hash_rate(attempts_delta, time_delta_ms);

                // Update global metrics
                let metrics = get_global_metrics();
                metrics.set_hash_rate_hps(hash_rate_hps);

                // Export as qanto_hash_rate_hs metric
                debug!(
                    "Hash rate updated: {} H/s ({} attempts in {}ms)",
                    QantoMetrics::format_hash_rate(hash_rate_hps),
                    attempts_delta,
                    time_delta_ms
                );
            }
        }

        // Update tracking values
        self.last_hash_attempts
            .store(current_attempts, Ordering::Relaxed);
        self.last_sample_time
            .store(current_time_ms, Ordering::Relaxed);
    }

    /// Log detailed telemetry metrics
    fn log_detailed_metrics(&self) {
        let metrics = get_global_metrics();
        let hash_rate = metrics.get_hash_rate_hps();
        let total_attempts = HASH_ATTEMPTS.load(Ordering::Relaxed);

        info!(
            "Telemetry: Hash Rate: {}, Total Attempts: {}, Mining Attempts: {}",
            QantoMetrics::format_hash_rate(hash_rate),
            total_attempts,
            metrics.mining_attempts.load(Ordering::Relaxed)
        );
    }

    /// Get current hash rate in H/s
    pub fn get_current_hash_rate(&self) -> f64 {
        get_global_metrics().get_hash_rate_hps()
    }

    /// Get total hash attempts
    pub fn get_total_hash_attempts(&self) -> u64 {
        HASH_ATTEMPTS.load(Ordering::Relaxed)
    }
}

/// Increment the global hash attempts counter
#[inline]
pub fn increment_hash_attempts() {
    HASH_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

/// Get current hash attempts count
#[inline]
pub fn get_hash_attempts() -> u64 {
    HASH_ATTEMPTS.load(Ordering::Relaxed)
}

/// Convert hash attempts delta and time interval to hash rate
pub fn calculate_hash_rate(attempts_delta: u64, interval_ms: u64) -> f64 {
    QantoMetrics::compute_hash_rate(attempts_delta, interval_ms)
}

/// Format hash rate with appropriate units (H/s, KH/s, MH/s, etc.)
pub fn format_hash_rate(hash_rate_hps: f64) -> String {
    QantoMetrics::format_hash_rate(hash_rate_hps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[test]
    fn test_hash_rate_calculation() {
        // Test basic hash rate calculation
        let attempts_delta = 1000;
        let interval_ms = 1000; // 1 second
        let hash_rate = calculate_hash_rate(attempts_delta, interval_ms);
        assert_eq!(hash_rate, 1000.0); // 1000 H/s

        // Test with different intervals
        let hash_rate_500ms = calculate_hash_rate(500, 500);
        assert_eq!(hash_rate_500ms, 1000.0); // Still 1000 H/s

        // Test zero interval handling
        let hash_rate_zero = calculate_hash_rate(1000, 0);
        assert_eq!(hash_rate_zero, 0.0);
    }

    #[test]
    fn test_hash_rate_formatting() {
        // Test basic formatting
        assert_eq!(QantoMetrics::format_hash_rate(500.0), "500.00 H/s");
        assert_eq!(QantoMetrics::format_hash_rate(1500.0), "1.50 kH/s");
        assert_eq!(QantoMetrics::format_hash_rate(2_500_000.0), "2.50 MH/s");
        assert_eq!(QantoMetrics::format_hash_rate(3_500_000_000.0), "3.50 GH/s");

        // Test edge cases
        assert_eq!(QantoMetrics::format_hash_rate(0.0), "0.00 H/s");
        assert_eq!(QantoMetrics::format_hash_rate(999.0), "999.00 H/s");
        assert_eq!(QantoMetrics::format_hash_rate(1000.0), "1.00 kH/s");
    }

    #[test]
    fn test_hash_attempts_counter() {
        // Reset counter for test
        HASH_ATTEMPTS.store(0, Ordering::Relaxed);

        // Test increment
        increment_hash_attempts();
        assert_eq!(get_hash_attempts(), 1);

        // Test multiple increments
        for _ in 0..100 {
            increment_hash_attempts();
        }
        assert_eq!(get_hash_attempts(), 101);
    }

    #[test]
    fn test_edge_cases() {
        // Test negative values
        assert_eq!(QantoMetrics::format_hash_rate(-100.0), "0.00 H/s");

        // Test infinity and NaN
        assert_eq!(QantoMetrics::format_hash_rate(f64::INFINITY), "0.00 H/s");
        assert_eq!(
            QantoMetrics::format_hash_rate(f64::NEG_INFINITY),
            "0.00 H/s"
        );
        assert_eq!(QantoMetrics::format_hash_rate(f64::NAN), "0.00 H/s");

        // Test very large values
        assert_eq!(QantoMetrics::format_hash_rate(1e15), "1.00 PH/s");
        assert_eq!(QantoMetrics::format_hash_rate(1e18), "1000.00 PH/s");
    }

    #[test]
    fn test_precision_and_rounding() {
        // Test precision with decimal values
        assert_eq!(QantoMetrics::format_hash_rate(1234.567), "1.23 kH/s");
        assert_eq!(QantoMetrics::format_hash_rate(999.999), "1000.00 H/s");
        assert_eq!(QantoMetrics::format_hash_rate(1000.001), "1.00 kH/s");

        // Test rounding behavior
        assert_eq!(QantoMetrics::format_hash_rate(1234.994), "1.23 kH/s");
        assert_eq!(QantoMetrics::format_hash_rate(1234.995), "1.23 kH/s");
    }

    #[tokio::test]
    async fn test_telemetry_manager_creation() {
        let config = TelemetryConfig {
            hash_rate_interval_secs: 1,
            enable_detailed_logging: true,
        };

        let manager = TelemetryManager::new(config.clone());
        assert_eq!(manager.config.hash_rate_interval_secs, 1);
        assert!(manager.config.enable_detailed_logging);
        assert_eq!(manager.last_hash_attempts.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_hash_rate_metrics_collection() {
        // Reset counter for test
        HASH_ATTEMPTS.store(0, Ordering::Relaxed);

        let config = TelemetryConfig {
            hash_rate_interval_secs: 1,
            enable_detailed_logging: false,
        };

        let manager = TelemetryManager::new(config);

        // Simulate some hash attempts
        for _ in 0..1000 {
            increment_hash_attempts();
        }

        // Wait a bit to ensure time difference
        sleep(Duration::from_millis(10)).await;

        // Collect metrics (this should update internal state)
        manager.collect_hash_rate_metrics();

        // Verify the counter was read
        assert_eq!(get_hash_attempts(), 1000);

        // Add more attempts and collect again
        for _ in 0..500 {
            increment_hash_attempts();
        }

        sleep(Duration::from_millis(10)).await;
        manager.collect_hash_rate_metrics();

        assert_eq!(get_hash_attempts(), 1500);
    }

    #[test]
    fn test_concurrent_hash_attempts() {
        // Reset counter for test
        HASH_ATTEMPTS.store(0, Ordering::Relaxed);

        // Simulate concurrent hash attempts
        let handles: Vec<_> = (0..5)
            .map(|_| {
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        increment_hash_attempts();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Check that all attempts were recorded (allowing for some variance due to global state)
        let final_count = get_hash_attempts();
        assert!(
            final_count >= 5000,
            "Expected at least 5000 attempts, got {final_count}"
        );
    }

    #[test]
    fn test_conversion_accuracy() {
        // Test that our conversion matches expected values exactly
        struct TestCase {
            attempts: u64,
            interval_ms: u64,
            expected_hps: f64,
        }

        let test_cases = vec![
            TestCase {
                attempts: 1000,
                interval_ms: 1000,
                expected_hps: 1000.0,
            },
            TestCase {
                attempts: 500,
                interval_ms: 500,
                expected_hps: 1000.0,
            },
            TestCase {
                attempts: 2000,
                interval_ms: 1000,
                expected_hps: 2000.0,
            },
            TestCase {
                attempts: 1,
                interval_ms: 1,
                expected_hps: 1000.0,
            },
            TestCase {
                attempts: 5000,
                interval_ms: 5000,
                expected_hps: 1000.0,
            },
        ];

        for case in test_cases {
            let result = calculate_hash_rate(case.attempts, case.interval_ms);
            assert_eq!(
                result, case.expected_hps,
                "Failed for attempts={}, interval_ms={}",
                case.attempts, case.interval_ms
            );
        }
    }
}
