#[cfg(test)]
mod adaptive_difficulty_tests {
    use qanto::adaptive_mining::{AdaptiveMiningLoop, TestnetMiningConfig};
    use qanto::mining_metrics::MiningMetrics;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Helper function to create a test mining loop with default configuration
    fn create_test_mining_loop() -> AdaptiveMiningLoop {
        let config = TestnetMiningConfig::default();
        let metrics = Arc::new(MiningMetrics::new());
        AdaptiveMiningLoop::new(config, None, metrics)
    }

    #[tokio::test]
    async fn test_sliding_window_average_calculation() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment (more than 2 seconds ago)
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate a sliding window of fast block times (all under target)
        mining_loop.state.recent_block_times = vec![1000, 1200, 1100, 1300, 1150]; // avg = 1150ms
        mining_loop.state.successful_blocks = 5;
        mining_loop.state.total_attempts = 5;
        mining_loop.state.consecutive_failures = 0;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // With average block time of 1150ms vs target of 1875ms, difficulty should increase
        // The Bitcoin-inspired algorithm should increase difficulty when blocks are consistently fast
        assert!(mining_loop.state.current_difficulty > initial_difficulty);

        // Test success rate calculation with proper f64 tolerance
        let expected_success_rate = 100.0; // 5/5 * 100 = 100%
        let actual_success_rate = mining_loop.calculate_success_rate();
        assert!((actual_success_rate - expected_success_rate).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_sliding_window_with_slow_blocks() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment (more than 5 seconds ago)
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate slow mining (15000ms > 10000ms stall threshold)
        let slow_mining_time = Duration::from_millis(15000);
        mining_loop.state.last_block_time = Instant::now() - slow_mining_time;
        mining_loop.state.successful_blocks = 5;
        mining_loop.state.total_attempts = 10;
        mining_loop.state.consecutive_failures = 3;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should decrease because blocks are slower than stall threshold (15000ms > 10000ms)
        assert!(mining_loop.state.current_difficulty < initial_difficulty);
    }

    #[tokio::test]
    async fn test_clamped_adjustment_limits() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate extremely fast mining to test upper clamping
        mining_loop.state.recent_block_times = vec![100, 150, 120, 180, 130]; // avg = 136ms (very fast)
        mining_loop.state.successful_blocks = 5;
        mining_loop.state.total_attempts = 5;
        mining_loop.state.consecutive_failures = 0;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // With extremely fast average block time (136ms vs target 1875ms), difficulty should increase
        // The Bitcoin-inspired algorithm should clamp the adjustment to prevent extreme changes
        assert!(mining_loop.state.current_difficulty > initial_difficulty);

        // Test that the adjustment is clamped (shouldn't be more than 4x increase)
        let adjustment_ratio = mining_loop.state.current_difficulty / initial_difficulty;
        assert!(
            adjustment_ratio <= 4.0,
            "Difficulty adjustment should be clamped to max 4x increase"
        );

        // Now test lower clamping with extremely slow mining
        mining_loop.state.recent_block_times = vec![10000, 12000, 11000, 13000, 11500]; // avg = 11500ms (very slow)
        mining_loop.state.consecutive_failures = 10; // Many failures
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        let high_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment for slow mining
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should decrease but be clamped (shouldn't be less than 1/4 of original)
        assert!(mining_loop.state.current_difficulty < high_difficulty);
        let reduction_ratio = mining_loop.state.current_difficulty / high_difficulty;
        assert!(
            reduction_ratio >= 0.25,
            "Difficulty reduction should be clamped to max 1/4 decrease"
        );
    }

    #[tokio::test]
    async fn test_smoothing_factor_application() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment (more than 5 seconds ago)
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate stalled mining (15000ms > 10000ms stall threshold)
        let stall_time = Duration::from_millis(15000);
        mining_loop.state.last_block_time = Instant::now() - stall_time;
        mining_loop.state.consecutive_failures = 3;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should be reduced by difficulty_reduction_factor (0.5)
        let expected_difficulty =
            initial_difficulty * mining_loop.config.difficulty_reduction_factor;
        let tolerance = expected_difficulty * 0.01; // 1% tolerance
        assert!((mining_loop.state.current_difficulty - expected_difficulty).abs() < tolerance);
    }

    #[tokio::test]
    async fn test_insufficient_history_fallback() {
        let mut mining_loop = create_test_mining_loop();

        // Set conditions for fallback heuristic (no failures, recent block was fast)
        mining_loop.state.consecutive_failures = 0;
        mining_loop.state.successful_blocks = 5; // Less history
        mining_loop.state.total_attempts = 5;
        mining_loop.state.last_block_time = Instant::now() - Duration::from_millis(1000); // 1 second ago

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // With insufficient history but fast recent block, difficulty might increase slightly
        // or stay the same depending on the fallback logic
        assert!(mining_loop.state.current_difficulty >= initial_difficulty * 0.95);
    }

    #[tokio::test]
    async fn test_minimum_maximum_difficulty_bounds() {
        let mut mining_loop = create_test_mining_loop();

        // Test minimum bound
        mining_loop.state.current_difficulty = mining_loop.config.min_difficulty;

        // Simulate very slow mining to try to decrease difficulty below minimum
        let very_slow_mining_time = Duration::from_millis(30000); // 30 seconds
        mining_loop.state.last_block_time = Instant::now() - very_slow_mining_time;
        mining_loop.state.successful_blocks = 10;
        mining_loop.state.total_attempts = 10;
        mining_loop.state.consecutive_failures = 0;

        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should not go below minimum
        assert!(mining_loop.state.current_difficulty >= mining_loop.config.min_difficulty);

        // Test maximum bound
        mining_loop.state.current_difficulty = mining_loop.config.max_difficulty;

        // Simulate very fast mining to try to increase difficulty above maximum
        let very_fast_mining_time = Duration::from_millis(50); // 50ms
        mining_loop.state.last_block_time = Instant::now() - very_fast_mining_time;
        mining_loop.state.successful_blocks = 20;
        mining_loop.state.total_attempts = 20;
        mining_loop.state.consecutive_failures = 0;

        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should not go above maximum
        assert!(mining_loop.state.current_difficulty <= mining_loop.config.max_difficulty);
    }

    #[tokio::test]
    async fn test_stall_threshold_immediate_reduction() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment (more than 5 seconds ago)
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate stalled mining (exceeds 10 second stall threshold)
        let stall_time = Duration::from_millis(12000); // 12 seconds > 10 second threshold
        mining_loop.state.last_block_time = Instant::now() - stall_time;
        mining_loop.state.consecutive_failures = 2; // Less than 5, so stall threshold is the trigger

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should be reduced due to stall threshold exceeded
        assert!(mining_loop.state.current_difficulty < initial_difficulty);

        // Should be reduced by difficulty_reduction_factor (0.5)
        let expected_difficulty =
            initial_difficulty * mining_loop.config.difficulty_reduction_factor;
        let tolerance = expected_difficulty * 0.01; // 1% tolerance
        assert!((mining_loop.state.current_difficulty - expected_difficulty).abs() < tolerance);
    }

    #[tokio::test]
    async fn test_consecutive_failures_immediate_reduction() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment (more than 5 seconds ago)
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate many consecutive failures (> 5 triggers immediate reduction)
        mining_loop.state.consecutive_failures = 8;
        mining_loop.state.last_block_time = Instant::now() - Duration::from_millis(3000); // Normal time

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should be reduced due to consecutive failures > 5
        assert!(mining_loop.state.current_difficulty < initial_difficulty);

        // Should be reduced by difficulty_reduction_factor (0.5)
        let expected_difficulty =
            initial_difficulty * mining_loop.config.difficulty_reduction_factor;
        let tolerance = expected_difficulty * 0.01; // 1% tolerance
        assert!((mining_loop.state.current_difficulty - expected_difficulty).abs() < tolerance);
    }

    #[tokio::test]
    async fn test_adjustment_cooldown_period() {
        let mut mining_loop = create_test_mining_loop();

        // Set last adjustment to very recent (within cooldown period)
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_millis(500);
        mining_loop.state.successful_blocks = 10;
        mining_loop.state.total_attempts = 10;
        mining_loop.state.consecutive_failures = 0;

        // Simulate fast mining that would normally trigger adjustment
        let fast_mining_time = Duration::from_millis(1000);
        mining_loop.state.last_block_time = Instant::now() - fast_mining_time;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should not change due to cooldown period
        assert_eq!(mining_loop.state.current_difficulty, initial_difficulty);
    }

    #[tokio::test]
    async fn test_mixed_success_failure_history() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate mixed success/failure history with moderate block times
        // Using times slightly faster than target to trigger increase
        mining_loop.state.recent_block_times = vec![1500, 1600, 1400, 1700, 1550]; // avg = 1550ms (faster than 1875ms target)
        mining_loop.state.successful_blocks = 7;
        mining_loop.state.total_attempts = 10; // 7 successes out of 10 attempts = 70%
        mining_loop.state.consecutive_failures = 3;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // With average block time of 1550ms vs target of 1875ms, difficulty should increase
        // The mixed history (70% success rate) should still allow for adjustment
        assert!(mining_loop.state.current_difficulty >= initial_difficulty);

        // Test success rate calculation with proper f64 tolerance
        let expected_success_rate = 70.0; // 7/10 * 100 = 70%
        let actual_success_rate = mining_loop.calculate_success_rate();
        assert!((actual_success_rate - expected_success_rate).abs() < 0.1);
    }

    // Additional edge case tests for comprehensive coverage

    #[tokio::test]
    async fn test_empty_sliding_window_fallback() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Empty sliding window should fallback to target time
        mining_loop.state.recent_block_times = vec![];
        mining_loop.state.successful_blocks = 0;
        mining_loop.state.total_attempts = 0;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // With empty window, the algorithm may still apply default adjustments
        // Allow for small increases due to default behavior
        let adjustment_ratio = mining_loop.state.current_difficulty / initial_difficulty;
        assert!(
            adjustment_ratio <= 1.1,
            "Empty window should not cause large adjustments"
        );
    }

    #[tokio::test]
    async fn test_single_block_time_window() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Single very fast block time
        mining_loop.state.recent_block_times = vec![500]; // Much faster than 1875ms target
        mining_loop.state.successful_blocks = 1;
        mining_loop.state.total_attempts = 1;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Single fast block should increase difficulty
        assert!(mining_loop.state.current_difficulty > initial_difficulty);
    }

    #[tokio::test]
    async fn test_extreme_difficulty_bounds() {
        let mut mining_loop = create_test_mining_loop();

        // Test upper bound
        mining_loop.state.current_difficulty = mining_loop.config.max_difficulty;
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);
        mining_loop.state.recent_block_times = vec![100, 150, 120]; // Extremely fast

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Should not exceed max difficulty
        assert!(mining_loop.state.current_difficulty <= mining_loop.config.max_difficulty);

        // Test lower bound
        mining_loop.state.current_difficulty = mining_loop.config.min_difficulty;
        mining_loop.state.recent_block_times = vec![20000, 25000, 30000]; // Extremely slow
        mining_loop.state.consecutive_failures = 20;
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Should not go below min difficulty
        assert!(mining_loop.state.current_difficulty >= mining_loop.config.min_difficulty);
    }

    #[tokio::test]
    async fn test_success_rate_precision_edge_cases() {
        let mut mining_loop = create_test_mining_loop();

        // Test with large numbers that might cause precision issues
        mining_loop.state.successful_blocks = 999999;
        mining_loop.state.total_attempts = 1000000;

        let success_rate = mining_loop.calculate_success_rate();
        let expected = 100.0; // Rounded to 100.0 due to rounding in calculate_success_rate
        assert!((success_rate - expected).abs() < 0.1);

        // Test with very small success rate
        mining_loop.state.successful_blocks = 1;
        mining_loop.state.total_attempts = 1000000;

        let success_rate = mining_loop.calculate_success_rate();
        let expected = 0.0; // Rounded to 0.0 due to rounding in calculate_success_rate
        assert!((success_rate - expected).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_bitcoin_style_dampening() {
        let mut mining_loop = create_test_mining_loop();

        // Set last_difficulty_adjustment to allow adjustment
        mining_loop.state.last_difficulty_adjustment = Instant::now() - Duration::from_secs(10);

        // Simulate moderately fast mining that should trigger dampened adjustment
        mining_loop.state.recent_block_times = vec![1200, 1300, 1100, 1400, 1250]; // avg = 1250ms
        mining_loop.state.successful_blocks = 5;
        mining_loop.state.total_attempts = 5;

        let initial_difficulty = mining_loop.state.current_difficulty;

        // Trigger difficulty adjustment
        mining_loop.adjust_difficulty_if_needed().await;

        // Difficulty should increase but be dampened (not extreme)
        assert!(mining_loop.state.current_difficulty > initial_difficulty);
        let adjustment_ratio = mining_loop.state.current_difficulty / initial_difficulty;
        assert!(
            adjustment_ratio < 2.0,
            "Adjustment should be dampened, not extreme"
        );
    }
}
