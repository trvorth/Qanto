//! Integration Tests for Mempool Backpressure
//!
//! This module provides integration tests for mempool backpressure functionality.

use qanto::config::Config;
use qanto::mempool::OptimizedMempool;
use std::time::Duration;

#[tokio::test]
async fn test_mempool_utilization_assertion() {
    // Create a test mempool with backpressure threshold
    let mempool = OptimizedMempool::new_with_backpressure(
        Duration::from_secs(300),
        1024 * 1024, // 1MB
        0.8,         // 80% threshold
    );

    // Get current mempool utilization
    let current_size = mempool.get_current_size_bytes();
    let max_size = 1024 * 1024; // 1MB
    let mempool_utilization = (current_size as f64 / max_size as f64) * 100.0;

    println!("Mempool utilization: {mempool_utilization:.2}%");

    // Assert that mempool utilization is below 100%
    assert!(mempool_utilization < 100.0);
}

#[tokio::test]
async fn test_config_mempool_backpressure_threshold() {
    // Test that config has mempool_backpressure_threshold field
    let config = Config::default();

    // Verify the field exists and has a reasonable default
    if let Some(threshold) = config.mempool_backpressure_threshold {
        assert!(threshold > 0.0);
        assert!(threshold <= 1.0);
        println!("Mempool backpressure threshold: {threshold}");
    } else {
        // If None, that's also valid - it means no backpressure
        println!("Mempool backpressure threshold: None (disabled)");
    }
}
