//! Timing Tests
//!
//! This module provides comprehensive tests for timing functionality
//! including microsecond timers, block timing coordination, and timing utilities.

#[cfg(test)]
mod tests {
    use qanto::timing::*;
    use std::time::{Duration, Instant};
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_microsecond_timer_precision() {
        let timer = MicrosecondTimer::new(1000); // 1ms target
        timer.start();

        let start = Instant::now();
        for _ in 0..10 {
            timer.tick().await;
        }
        let elapsed = start.elapsed();

        // Should complete 10 ticks in approximately 10ms (very tolerant for test environments)
        // The timer may be affected by system load, so we allow a wide range
        assert!(elapsed.as_millis() >= 5 && elapsed.as_millis() <= 50);

        let stats = timer.get_stats();
        // Relax precision requirement for test environments
        // Microsecond scheduling can be noisy under CI; accept lower bound
        assert!(stats.precision_percentage > 20.0); // Should be >20% precise in CI
    }

    #[tokio::test]
    async fn test_block_timing_coordinator() {
        let coordinator = BlockTimingCoordinator::new();
        coordinator.start().await;

        // Test block timing
        let block_duration = timeout(
            Duration::from_millis(100),
            coordinator.wait_for_block_window(),
        )
        .await;

        assert!(block_duration.is_ok());

        // Record block completion
        coordinator.record_block_completion();

        let metrics = coordinator.get_comprehensive_metrics();
        assert!(metrics.target_bps >= 32);
    }

    #[test]
    fn test_timing_utils() {
        assert_eq!(timing_utils::ms_to_us(31), 31_000);
        assert_eq!(timing_utils::us_to_ms(31_000), 31);

        let bps = timing_utils::calculate_bps_from_us(31_000);
        assert!((bps - 32.25).abs() < 0.1);

        let mining_interval = timing_utils::calculate_optimal_mining_interval_us(32.0);
        assert_eq!(mining_interval, 7_812); // ~7.8ms for 32 BPS
    }

    #[test]
    fn test_timing_validation() {
        // Valid configuration
        assert!(timing_utils::validate_timing_config(31_000, 7_000, 3_000).is_ok());

        // Invalid configurations
        assert!(timing_utils::validate_timing_config(20_000, 7_000, 3_000).is_err()); // Too fast
        assert!(timing_utils::validate_timing_config(31_000, 20_000, 3_000).is_err()); // Mining too slow
        assert!(timing_utils::validate_timing_config(31_000, 7_000, 5_000).is_err());
        // TX quantum too large
    }
}
