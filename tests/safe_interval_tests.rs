use qanto::safe_interval::*;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_safe_interval_from_millis() {
    let config = SafeIntervalConfig::default();

    // Test valid interval
    let result = SafeInterval::from_millis(1000, config.clone());
    assert!(result.is_ok());

    // Test zero duration
    let result = SafeInterval::from_millis(0, config.clone());
    assert!(matches!(
        result,
        Err(SafeIntervalError::ZeroDuration { .. })
    ));

    // Test interval actually works
    let mut interval = SafeInterval::from_millis(10, config).unwrap();
    let start = std::time::Instant::now();
    interval.tick().await; // First tick is immediate
    interval.tick().await; // Second tick should wait
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() >= 8); // Allow some tolerance
}

#[tokio::test]
async fn test_safe_interval_from_secs() {
    let config = SafeIntervalConfig::default();

    // Test valid interval
    let result = SafeInterval::from_secs(1, config.clone());
    assert!(result.is_ok());

    // Test zero duration
    let result = SafeInterval::from_secs(0, config);
    assert!(matches!(
        result,
        Err(SafeIntervalError::ZeroDuration { .. })
    ));
}

#[tokio::test]
async fn test_mining_interval_fallback() {
    // Test with zero - should use fallback
    let mut interval = SafeInterval::for_mining(0);
    let _start = std::time::Instant::now();
    interval.tick().await; // First tick is immediate

    // Should not panic and should use fallback duration
    let tick_future = interval.tick();
    let result = timeout(Duration::from_millis(1500), tick_future).await;
    assert!(result.is_ok());
}

#[test]
fn test_validate_timing_config() {
    // Valid configuration
    let result = SafeInterval::validate_timing_config(1000, 2000, 5000);
    assert!(result.is_ok());

    // Invalid configuration with zero
    let result = SafeInterval::validate_timing_config(0, 2000, 5000);
    assert!(matches!(
        result,
        Err(SafeIntervalError::ZeroDuration { .. })
    ));
}

#[test]
fn test_secs_to_millis_safe() {
    // Valid conversion
    let result = SafeInterval::secs_to_millis_safe(5);
    assert_eq!(result.unwrap(), 5000);

    // Test overflow protection
    let result = SafeInterval::secs_to_millis_safe(u64::MAX);
    assert!(result.is_err());
}

#[test]
fn test_millis_to_secs_safe() {
    // Valid conversion
    let result = SafeInterval::millis_to_secs_safe(5000);
    assert_eq!(result, 5);

    // Test rounding down
    let result = SafeInterval::millis_to_secs_safe(5999);
    assert_eq!(result, 5);
}
