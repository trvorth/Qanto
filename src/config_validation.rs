use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Configuration validation errors
#[derive(Error, Debug)]
pub enum ConfigValidationError {
    #[error("Invalid timing parameter: {parameter} = {value}, {reason}")]
    InvalidTiming {
        parameter: String,
        value: u64,
        reason: String,
    },
    #[error("Configuration conflict: {message}")]
    Conflict { message: String },
    #[error("Missing required parameter: {parameter}")]
    MissingParameter { parameter: String },
    #[error("Parameter out of range: {parameter} = {value}, expected range: {min}-{max}")]
    OutOfRange {
        parameter: String,
        value: u64,
        min: u64,
        max: u64,
    },
    #[error("Invalid memory configuration: {message}")]
    InvalidMemory { message: String },
    #[error("Performance constraint violation: {message}")]
    PerformanceConstraint { message: String },
}

/// Timing configuration parameters
#[derive(Debug, Clone)]
pub struct TimingConfig {
    pub target_block_time: u64,
    pub mining_interval_ms: u64,
    pub dummy_tx_interval_ms: u64,
    pub heartbeat_interval: u64,
    pub validation_timeout_ms: u64,
}

/// Memory configuration parameters
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub mempool_max_size_bytes: usize,
    pub mempool_batch_size: usize,
    pub mempool_backpressure_threshold: usize,
    pub memory_soft_limit: usize,
    pub memory_hard_limit: usize,
    pub cache_size: usize,
}

/// Performance configuration parameters
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    pub tx_batch_size: usize,
    pub adaptive_batch_threshold: f64,
    pub dummy_tx_per_cycle: usize,
    pub parallel_validation_batch_size: usize,
    pub block_processing_workers: usize,
    pub transaction_validation_workers: usize,
}

/// Complete configuration validation result
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ConfigValidationError>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
    pub adjusted_config: Option<AdjustedConfig>,
}

/// Configuration with safe adjustments applied
#[derive(Debug, Clone)]
pub struct AdjustedConfig {
    pub timing: TimingConfig,
    pub memory: MemoryConfig,
    pub performance: PerformanceConfig,
    pub adjustments_made: Vec<String>,
}

/// Configuration validator with comprehensive checks
pub struct ConfigValidator {
    strict_mode: bool,
    auto_adjust: bool,
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self {
            strict_mode: false,
            auto_adjust: true,
        }
    }
}

impl ConfigValidator {
    pub fn new(strict_mode: bool, auto_adjust: bool) -> Self {
        Self {
            strict_mode,
            auto_adjust,
        }
    }

    /// Validate complete configuration
    pub fn validate_config(
        &self,
        timing: &TimingConfig,
        memory: &MemoryConfig,
        performance: &PerformanceConfig,
    ) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut recommendations = Vec::new();
        let mut adjustments_made = Vec::new();

        // Validate timing configuration
        self.validate_timing_config(timing, &mut errors, &mut warnings, &mut recommendations);

        // Validate memory configuration
        self.validate_memory_config(memory, &mut errors, &mut warnings, &mut recommendations);

        // Validate performance configuration
        self.validate_performance_config(performance, &mut errors, &mut warnings, &mut recommendations);

        // Cross-validate configurations
        self.cross_validate_configs(timing, memory, performance, &mut errors, &mut warnings, &mut recommendations);

        // Apply auto-adjustments if enabled
        let adjusted_config = if self.auto_adjust && !errors.is_empty() {
            match self.auto_adjust_config(timing, memory, performance, &mut adjustments_made) {
                Ok(config) => Some(config),
                Err(e) => {
                    errors.push(e);
                    None
                }
            }
        } else {
            None
        };

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            recommendations,
            adjusted_config,
        }
    }

    /// Validate timing configuration parameters
    fn validate_timing_config(
        &self,
        config: &TimingConfig,
        errors: &mut Vec<ConfigValidationError>,
        warnings: &mut Vec<String>,
        recommendations: &mut Vec<String>,
    ) {
        // Check for zero durations
        let timing_params = [
            ("target_block_time", config.target_block_time),
            ("mining_interval_ms", config.mining_interval_ms),
            ("dummy_tx_interval_ms", config.dummy_tx_interval_ms),
            ("heartbeat_interval", config.heartbeat_interval),
            ("validation_timeout_ms", config.validation_timeout_ms),
        ];

        for (param, value) in timing_params {
            if value == 0 {
                errors.push(ConfigValidationError::InvalidTiming {
                    parameter: param.to_string(),
                    value,
                    reason: "cannot be zero (would cause panic in tokio::time::interval)".to_string(),
                });
            }
        }

        // Check minimum values
        if config.mining_interval_ms < 1 {
            errors.push(ConfigValidationError::InvalidTiming {
                parameter: "mining_interval_ms".to_string(),
                value: config.mining_interval_ms,
                reason: "must be at least 1ms".to_string(),
            });
        }

        if config.dummy_tx_interval_ms < 1 {
            errors.push(ConfigValidationError::InvalidTiming {
                parameter: "dummy_tx_interval_ms".to_string(),
                value: config.dummy_tx_interval_ms,
                reason: "must be at least 1ms".to_string(),
            });
        }

        // Check for potential precision loss in seconds conversion
        if config.dummy_tx_interval_ms < 1000 {
            warnings.push(format!(
                "dummy_tx_interval_ms ({}) < 1000ms may cause precision loss when converted to seconds",
                config.dummy_tx_interval_ms
            ));
            recommendations.push("Consider using values >= 1000ms for dummy_tx_interval_ms to avoid integer division precision loss".to_string());
        }

        // Check timing relationships
        if config.mining_interval_ms > config.target_block_time {
            warnings.push(format!(
                "mining_interval_ms ({}) > target_block_time ({}), may reduce mining efficiency",
                config.mining_interval_ms, config.target_block_time
            ));
        }

        // Check validation timeout
        if config.validation_timeout_ms > config.target_block_time / 2 {
            warnings.push(format!(
                "validation_timeout_ms ({}) > target_block_time/2 ({}), may cause block production delays",
                config.validation_timeout_ms, config.target_block_time / 2
            ));
        }
    }

    /// Validate memory configuration parameters
    fn validate_memory_config(
        &self,
        config: &MemoryConfig,
        errors: &mut Vec<ConfigValidationError>,
        warnings: &mut Vec<String>,
        recommendations: &mut Vec<String>,
    ) {
        // Check memory limits
        if config.memory_hard_limit <= config.memory_soft_limit {
            errors.push(ConfigValidationError::InvalidMemory {
                message: format!(
                    "memory_hard_limit ({}) must be greater than memory_soft_limit ({})",
                    config.memory_hard_limit, config.memory_soft_limit
                ),
            });
        }

        // Check mempool configuration
        if config.mempool_backpressure_threshold >= config.mempool_max_size_bytes {
            errors.push(ConfigValidationError::InvalidMemory {
                message: format!(
                    "mempool_backpressure_threshold ({}) must be less than mempool_max_size_bytes ({})",
                    config.mempool_backpressure_threshold, config.mempool_max_size_bytes
                ),
            });
        }

        if config.mempool_batch_size > config.mempool_max_size_bytes / 10 {
            warnings.push(format!(
                "mempool_batch_size ({}) > 10% of mempool_max_size_bytes ({}), may cause memory spikes",
                config.mempool_batch_size, config.mempool_max_size_bytes / 10
            ));
        }

        // Check cache size
        if config.cache_size > config.memory_soft_limit / 4 {
            warnings.push(format!(
                "cache_size ({}) > 25% of memory_soft_limit ({}), may cause memory pressure",
                config.cache_size, config.memory_soft_limit / 4
            ));
        }

        // Recommendations
        if config.mempool_max_size_bytes < 1_000_000 {
            recommendations.push("Consider increasing mempool_max_size_bytes for better transaction throughput".to_string());
        }
    }

    /// Validate performance configuration parameters
    fn validate_performance_config(
        &self,
        config: &PerformanceConfig,
        errors: &mut Vec<ConfigValidationError>,
        warnings: &mut Vec<String>,
        recommendations: &mut Vec<String>,
    ) {
        // Check batch sizes
        if config.tx_batch_size == 0 {
            errors.push(ConfigValidationError::InvalidTiming {
                parameter: "tx_batch_size".to_string(),
                value: config.tx_batch_size as u64,
                reason: "cannot be zero".to_string(),
            });
        }

        if config.parallel_validation_batch_size == 0 {
            errors.push(ConfigValidationError::InvalidTiming {
                parameter: "parallel_validation_batch_size".to_string(),
                value: config.parallel_validation_batch_size as u64,
                reason: "cannot be zero".to_string(),
            });
        }

        // Check worker counts
        let cpu_count = num_cpus::get();
        if config.block_processing_workers > cpu_count * 4 {
            warnings.push(format!(
                "block_processing_workers ({}) > 4x CPU count ({}), may cause context switching overhead",
                config.block_processing_workers, cpu_count * 4
            ));
        }

        if config.transaction_validation_workers > cpu_count * 2 {
            warnings.push(format!(
                "transaction_validation_workers ({}) > 2x CPU count ({}), may cause context switching overhead",
                config.transaction_validation_workers, cpu_count * 2
            ));
        }

        // Check adaptive batch threshold
        if config.adaptive_batch_threshold < 0.1 || config.adaptive_batch_threshold > 1.0 {
            errors.push(ConfigValidationError::OutOfRange {
                parameter: "adaptive_batch_threshold".to_string(),
                value: (config.adaptive_batch_threshold * 100.0) as u64,
                min: 10,
                max: 100,
            });
        }

        // Recommendations
        if config.dummy_tx_per_cycle > 1000 {
            recommendations.push("Consider reducing dummy_tx_per_cycle to avoid mempool flooding".to_string());
        }
    }

    /// Cross-validate configurations for consistency
    fn cross_validate_configs(
        &self,
        timing: &TimingConfig,
        memory: &MemoryConfig,
        performance: &PerformanceConfig,
        errors: &mut Vec<ConfigValidationError>,
        warnings: &mut Vec<String>,
        _recommendations: &mut Vec<String>,
    ) {
        // Check if transaction generation rate is sustainable
        let tx_per_second = (performance.dummy_tx_per_cycle as f64) / (timing.dummy_tx_interval_ms as f64 / 1000.0);
        let estimated_tx_size = 500; // bytes per transaction estimate
        let tx_memory_per_second = (tx_per_second * estimated_tx_size as f64) as usize;

        if tx_memory_per_second > memory.mempool_max_size_bytes / 10 {
            warnings.push(format!(
                "High transaction generation rate ({:.1} tx/s) may quickly fill mempool",
                tx_per_second
            ));
        }

        // Check if batch sizes are compatible with memory limits
        let max_batch_memory = performance.tx_batch_size * estimated_tx_size;
        if max_batch_memory > memory.memory_soft_limit / 4 {
            warnings.push(format!(
                "tx_batch_size ({}) may consume significant memory ({} bytes)",
                performance.tx_batch_size, max_batch_memory
            ));
        }

        // Check timing consistency
        if timing.validation_timeout_ms * 2 > timing.target_block_time {
            errors.push(ConfigValidationError::PerformanceConstraint {
                message: format!(
                    "validation_timeout_ms ({}) too high for target_block_time ({})",
                    timing.validation_timeout_ms, timing.target_block_time
                ),
            });
        }
    }

    /// Auto-adjust configuration to fix common issues
    fn auto_adjust_config(
        &self,
        timing: &TimingConfig,
        memory: &MemoryConfig,
        performance: &PerformanceConfig,
        adjustments: &mut Vec<String>,
    ) -> Result<AdjustedConfig, ConfigValidationError> {
        let mut adjusted_timing = timing.clone();
        let mut adjusted_memory = memory.clone();
        let mut adjusted_performance = performance.clone();

        // Fix zero durations
        if adjusted_timing.mining_interval_ms == 0 {
            adjusted_timing.mining_interval_ms = 1000;
            adjustments.push("Set mining_interval_ms to 1000ms (was 0)".to_string());
        }

        if adjusted_timing.dummy_tx_interval_ms == 0 {
            adjusted_timing.dummy_tx_interval_ms = 2000;
            adjustments.push("Set dummy_tx_interval_ms to 2000ms (was 0)".to_string());
        }

        if adjusted_timing.heartbeat_interval == 0 {
            adjusted_timing.heartbeat_interval = 5000;
            adjustments.push("Set heartbeat_interval to 5000ms (was 0)".to_string());
        }

        if adjusted_timing.validation_timeout_ms == 0 {
            adjusted_timing.validation_timeout_ms = 25;
            adjustments.push("Set validation_timeout_ms to 25ms (was 0)".to_string());
        }

        // Fix precision loss issues
        if adjusted_timing.dummy_tx_interval_ms < 1000 && adjusted_timing.dummy_tx_interval_ms > 0 {
            adjusted_timing.dummy_tx_interval_ms = 1000;
            adjustments.push("Increased dummy_tx_interval_ms to 1000ms to avoid precision loss".to_string());
        }

        // Fix memory configuration issues
        if adjusted_memory.memory_hard_limit <= adjusted_memory.memory_soft_limit {
            adjusted_memory.memory_hard_limit = adjusted_memory.memory_soft_limit * 2;
            adjustments.push(format!(
                "Set memory_hard_limit to {} (2x memory_soft_limit)",
                adjusted_memory.memory_hard_limit
            ));
        }

        if adjusted_memory.mempool_backpressure_threshold >= adjusted_memory.mempool_max_size_bytes {
            adjusted_memory.mempool_backpressure_threshold = adjusted_memory.mempool_max_size_bytes * 3 / 4;
            adjustments.push(format!(
                "Set mempool_backpressure_threshold to {} (75% of mempool_max_size_bytes)",
                adjusted_memory.mempool_backpressure_threshold
            ));
        }

        // Fix performance configuration issues
        if adjusted_performance.tx_batch_size == 0 {
            adjusted_performance.tx_batch_size = 100;
            adjustments.push("Set tx_batch_size to 100 (was 0)".to_string());
        }

        if adjusted_performance.parallel_validation_batch_size == 0 {
            adjusted_performance.parallel_validation_batch_size = 1000;
            adjustments.push("Set parallel_validation_batch_size to 1000 (was 0)".to_string());
        }

        if adjusted_performance.adaptive_batch_threshold < 0.1 || adjusted_performance.adaptive_batch_threshold > 1.0 {
            adjusted_performance.adaptive_batch_threshold = 0.8;
            adjustments.push("Set adaptive_batch_threshold to 0.8 (was out of range)".to_string());
        }

        Ok(AdjustedConfig {
            timing: adjusted_timing,
            memory: adjusted_memory,
            performance: adjusted_performance,
            adjustments_made: adjustments.clone(),
        })
    }

    /// Validate configuration from environment variables or CLI args
    pub fn validate_from_env(&self) -> ValidationResult {
        let timing = TimingConfig {
            target_block_time: std::env::var("TARGET_BLOCK_TIME")
                .unwrap_or_else(|_| "31".to_string())
                .parse()
                .unwrap_or(31),
            mining_interval_ms: std::env::var("MINING_INTERVAL_MS")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
            dummy_tx_interval_ms: std::env::var("DUMMY_TX_INTERVAL_MS")
                .unwrap_or_else(|_| "2000".to_string())
                .parse()
                .unwrap_or(2000),
            heartbeat_interval: std::env::var("HEARTBEAT_INTERVAL")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .unwrap_or(5000),
            validation_timeout_ms: std::env::var("VALIDATION_TIMEOUT_MS")
                .unwrap_or_else(|_| "25".to_string())
                .parse()
                .unwrap_or(25),
        };

        let memory = MemoryConfig {
            mempool_max_size_bytes: std::env::var("MEMPOOL_MAX_SIZE_BYTES")
                .unwrap_or_else(|_| "268435456".to_string())
                .parse()
                .unwrap_or(268_435_456),
            mempool_batch_size: std::env::var("MEMPOOL_BATCH_SIZE")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .unwrap_or(10_000),
            mempool_backpressure_threshold: std::env::var("MEMPOOL_BACKPRESSURE_THRESHOLD")
                .unwrap_or_else(|_| "201326592".to_string())
                .parse()
                .unwrap_or(201_326_592),
            memory_soft_limit: std::env::var("MEMORY_SOFT_LIMIT")
                .unwrap_or_else(|_| "1073741824".to_string())
                .parse()
                .unwrap_or(1_073_741_824),
            memory_hard_limit: std::env::var("MEMORY_HARD_LIMIT")
                .unwrap_or_else(|_| "2147483648".to_string())
                .parse()
                .unwrap_or(2_147_483_648),
            cache_size: std::env::var("CACHE_SIZE")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .unwrap_or(10_000),
        };

        let performance = PerformanceConfig {
            tx_batch_size: std::env::var("TX_BATCH_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1_000),
            adaptive_batch_threshold: std::env::var("ADAPTIVE_BATCH_THRESHOLD")
                .unwrap_or_else(|_| "0.8".to_string())
                .parse()
                .unwrap_or(0.8),
            dummy_tx_per_cycle: std::env::var("DUMMY_TX_PER_CYCLE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            parallel_validation_batch_size: std::env::var("PARALLEL_VALIDATION_BATCH_SIZE")
                .unwrap_or_else(|_| "50000".to_string())
                .parse()
                .unwrap_or(50_000),
            block_processing_workers: std::env::var("BLOCK_PROCESSING_WORKERS")
                .unwrap_or_else(|_| "256".to_string())
                .parse()
                .unwrap_or(256),
            transaction_validation_workers: std::env::var("TRANSACTION_VALIDATION_WORKERS")
                .unwrap_or_else(|_| "128".to_string())
                .parse()
                .unwrap_or(128),
        };

        self.validate_config(&timing, &memory, &performance)
    }

    /// Generate configuration report
    pub fn generate_report(&self, result: &ValidationResult) -> String {
        let mut report = String::new();
        
        report.push_str("=== Configuration Validation Report ===\n\n");
        
        if result.is_valid {
            report.push_str("✅ Configuration is VALID\n\n");
        } else {
            report.push_str("❌ Configuration has ERRORS\n\n");
        }

        if !result.errors.is_empty() {
            report.push_str("ERRORS:\n");
            for error in &result.errors {
                report.push_str(&format!("  • {}\n", error));
            }
            report.push('\n');
        }

        if !result.warnings.is_empty() {
            report.push_str("WARNINGS:\n");
            for warning in &result.warnings {
                report.push_str(&format!("  • {}\n", warning));
            }
            report.push('\n');
        }

        if !result.recommendations.is_empty() {
            report.push_str("RECOMMENDATIONS:\n");
            for recommendation in &result.recommendations {
                report.push_str(&format!("  • {}\n", recommendation));
            }
            report.push('\n');
        }

        if let Some(adjusted) = &result.adjusted_config {
            report.push_str("AUTO-ADJUSTMENTS APPLIED:\n");
            for adjustment in &adjusted.adjustments_made {
                report.push_str(&format!("  • {}\n", adjustment));
            }
            report.push('\n');
        }

        report
    }
}