use std::time::Duration;
use thiserror::Error;
use tokio::time::{interval, Interval};
use tracing::{debug, warn};

/// Minimum allowed interval duration to prevent zero-period panics
pub const MIN_INTERVAL_DURATION_MS: u64 = 1;
pub const MIN_INTERVAL_DURATION_SECS: u64 = 1;

/// Default fallback intervals for various operations
pub const DEFAULT_MINING_INTERVAL_MS: u64 = 1000;
pub const DEFAULT_DUMMY_TX_INTERVAL_MS: u64 = 2000;
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 5000;

#[derive(Error, Debug)]
pub enum SafeIntervalError {
    #[error("Zero duration interval not allowed: {operation}")]
    ZeroDuration { operation: String },
    #[error("Duration too small: {duration_ms}ms, minimum required: {min_ms}ms for {operation}")]
    DurationTooSmall {
        duration_ms: u64,
        min_ms: u64,
        operation: String,
    },
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },
}

/// Configuration for safe interval creation
#[derive(Debug, Clone)]
pub struct SafeIntervalConfig {
    pub operation_name: String,
    pub min_duration_ms: u64,
    pub fallback_duration_ms: u64,
    pub warn_on_fallback: bool,
}

impl Default for SafeIntervalConfig {
    fn default() -> Self {
        Self {
            operation_name: "unknown".to_string(),
            min_duration_ms: MIN_INTERVAL_DURATION_MS,
            fallback_duration_ms: 1000,
            warn_on_fallback: true,
        }
    }
}

/// Safe interval creation utilities
pub struct SafeInterval;

impl SafeInterval {
    /// Create a safe tokio::time::Interval from milliseconds with validation
    pub fn from_millis(
        duration_ms: u64,
        config: SafeIntervalConfig,
    ) -> Result<Interval, SafeIntervalError> {
        if duration_ms == 0 {
            return Err(SafeIntervalError::ZeroDuration {
                operation: config.operation_name,
            });
        }

        if duration_ms < config.min_duration_ms {
            if config.warn_on_fallback {
                warn!(
                    "Interval duration {}ms too small for {}, using fallback {}ms",
                    duration_ms, config.operation_name, config.fallback_duration_ms
                );
            }

            let fallback_duration = Duration::from_millis(config.fallback_duration_ms);
            debug!(
                "Created safe interval for {} with fallback duration: {:?}",
                config.operation_name, fallback_duration
            );
            return Ok(interval(fallback_duration));
        }

        let duration = Duration::from_millis(duration_ms);
        debug!(
            "Created safe interval for {} with duration: {:?}",
            config.operation_name, duration
        );
        Ok(interval(duration))
    }

    /// Create a safe tokio::time::Interval from seconds with validation
    pub fn from_secs(
        duration_secs: u64,
        config: SafeIntervalConfig,
    ) -> Result<Interval, SafeIntervalError> {
        if duration_secs == 0 {
            return Err(SafeIntervalError::ZeroDuration {
                operation: config.operation_name,
            });
        }

        let duration_ms = duration_secs * 1000;
        Self::from_millis(duration_ms, config)
    }

    /// Create a safe interval with automatic fallback for mining operations
    pub fn for_mining(interval_ms: u64) -> Interval {
        let config = SafeIntervalConfig {
            operation_name: "mining".to_string(),
            min_duration_ms: MIN_INTERVAL_DURATION_MS,
            fallback_duration_ms: DEFAULT_MINING_INTERVAL_MS,
            warn_on_fallback: true,
        };

        Self::from_millis(interval_ms, config)
            .unwrap_or_else(|_| interval(Duration::from_millis(DEFAULT_MINING_INTERVAL_MS)))
    }

    /// Create a safe interval with automatic fallback for dummy transactions
    pub fn for_dummy_transactions(interval_ms: u64) -> Interval {
        let config = SafeIntervalConfig {
            operation_name: "dummy_transactions".to_string(),
            min_duration_ms: MIN_INTERVAL_DURATION_MS,
            fallback_duration_ms: DEFAULT_DUMMY_TX_INTERVAL_MS,
            warn_on_fallback: true,
        };

        Self::from_millis(interval_ms, config)
            .unwrap_or_else(|_| interval(Duration::from_millis(DEFAULT_DUMMY_TX_INTERVAL_MS)))
    }

    /// Create a safe interval with automatic fallback for heartbeat operations
    pub fn for_heartbeat(interval_ms: u64) -> Interval {
        let config = SafeIntervalConfig {
            operation_name: "heartbeat".to_string(),
            min_duration_ms: MIN_INTERVAL_DURATION_MS,
            fallback_duration_ms: DEFAULT_HEARTBEAT_INTERVAL_MS,
            warn_on_fallback: true,
        };

        Self::from_millis(interval_ms, config)
            .unwrap_or_else(|_| interval(Duration::from_millis(DEFAULT_HEARTBEAT_INTERVAL_MS)))
    }

    /// Validate timing configuration parameters
    pub fn validate_timing_config(
        mining_interval_ms: u64,
        dummy_tx_interval_ms: u64,
        heartbeat_interval_ms: u64,
    ) -> Result<(), SafeIntervalError> {
        let validations = [
            (mining_interval_ms, "mining_interval_ms"),
            (dummy_tx_interval_ms, "dummy_tx_interval_ms"),
            (heartbeat_interval_ms, "heartbeat_interval_ms"),
        ];

        for (value, name) in validations {
            if value == 0 {
                return Err(SafeIntervalError::ZeroDuration {
                    operation: name.to_string(),
                });
            }
            if value < MIN_INTERVAL_DURATION_MS {
                return Err(SafeIntervalError::DurationTooSmall {
                    duration_ms: value,
                    min_ms: MIN_INTERVAL_DURATION_MS,
                    operation: name.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Convert seconds to milliseconds with overflow protection
    pub fn secs_to_millis_safe(secs: u64) -> Result<u64, SafeIntervalError> {
        secs.checked_mul(1000)
            .ok_or_else(|| SafeIntervalError::InvalidConfiguration {
                message: format!(
                    "Seconds value {secs} would overflow when converted to milliseconds"
                ),
            })
    }

    /// Convert milliseconds to seconds with precision handling
    pub fn millis_to_secs_safe(millis: u64) -> u64 {
        if millis < 1000 {
            // For values less than 1000ms, return 1 second to avoid zero-duration
            1
        } else {
            millis / 1000
        }
    }
}

/// Mining-specific error type for comprehensive error handling
#[derive(Error, Debug)]
pub enum MiningOperationError {
    #[error("Safe interval error: {0}")]
    SafeInterval(#[from] SafeIntervalError),
    #[error("Wallet error: {message}")]
    Wallet { message: String },
    #[error("Mining timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },
    #[error("Resource exhaustion: {resource}")]
    ResourceExhaustion { resource: String },
    #[error("Configuration error: {message}")]
    Configuration { message: String },
    #[error("Shutdown requested")]
    Shutdown,
    #[error("Critical system error: {message}")]
    Critical { message: String },
}

impl From<crate::wallet::WalletError> for MiningOperationError {
    fn from(e: crate::wallet::WalletError) -> Self {
        MiningOperationError::Wallet {
            message: e.to_string(),
        }
    }
}
