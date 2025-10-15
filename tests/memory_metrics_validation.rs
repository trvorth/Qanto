//! Memory metrics validation and profiling tests for Qanto blockchain node
//!
//! This module provides comprehensive validation for RSS memory tracking,
//! profiling capabilities, and performance testing under various load conditions.

use qanto::metrics::{get_rss_memory_bytes, QantoMetrics};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Configuration for memory profiling tests
#[derive(Debug, Clone)]
pub struct MemoryProfilingConfig {
    /// Duration of profiling test in seconds
    pub test_duration_seconds: u64,
    /// Interval between memory samples in milliseconds
    pub sample_interval_ms: u64,
    /// Memory threshold for warnings in bytes
    pub memory_threshold_bytes: u64,
    /// Enable load testing during profiling
    pub enable_load_testing: bool,
    /// Maximum allowed variance in memory measurements
    pub max_variance_threshold: f64,
}

impl Default for MemoryProfilingConfig {
    fn default() -> Self {
        Self {
            test_duration_seconds: 30,
            sample_interval_ms: 100,
            memory_threshold_bytes: 1024 * 1024 * 1024, // 1GB
            enable_load_testing: true,
            max_variance_threshold: 0.15, // 15% variance allowed
        }
    }
}

/// Results from memory profiling validation
#[derive(Debug, Clone)]
pub struct MemoryProfilingResults {
    /// All memory samples collected during profiling
    pub samples: Vec<MemorySample>,
    /// Maximum RSS observed during test
    pub max_rss_bytes: u64,
    /// Minimum RSS observed during test
    pub min_rss_bytes: u64,
    /// Average RSS during test
    pub avg_rss_bytes: u64,
    /// Variance in memory measurements
    pub variance: f64,
    /// Total test duration
    pub test_duration: Duration,
    /// Number of samples collected
    pub sample_count: u64,
    /// Accuracy percentage compared to direct measurements
    pub accuracy_percentage: f64,
    /// Overall performance score (0.0 - 1.0)
    pub performance_score: f64,
}

/// Individual memory sample
#[derive(Debug, Clone)]
pub struct MemorySample {
    pub timestamp: Instant,
    pub rss_bytes: u64,
    pub heap_bytes: u64,
    pub stack_bytes: u64,
}

/// Memory metrics validator for comprehensive testing
pub struct MemoryMetricsValidator {
    metrics: Arc<QantoMetrics>,
    config: MemoryProfilingConfig,
}

impl MemoryMetricsValidator {
    /// Create new memory metrics validator
    pub fn new(config: MemoryProfilingConfig) -> Self {
        Self {
            metrics: Arc::new(QantoMetrics::new()),
            config,
        }
    }

    /// Run comprehensive memory profiling validation
    pub fn validate_memory_metrics(
        &self,
    ) -> Result<MemoryProfilingResults, Box<dyn std::error::Error>> {
        info!("Starting memory metrics validation");

        // Run basic validation
        let basic_results = self.validate_basic_rss_tracking()?;

        // Run accuracy tests
        let accuracy_results = self.validate_accuracy()?;

        // Combine results
        let mut combined_samples = basic_results.samples.clone();
        combined_samples.extend(accuracy_results.samples.clone());

        let combined_results = MemoryProfilingResults {
            samples: combined_samples.clone(),
            max_rss_bytes: basic_results
                .max_rss_bytes
                .max(accuracy_results.max_rss_bytes),
            min_rss_bytes: basic_results
                .min_rss_bytes
                .min(accuracy_results.min_rss_bytes),
            avg_rss_bytes: (basic_results.avg_rss_bytes + accuracy_results.avg_rss_bytes) / 2,
            variance: Self::calculate_variance(&combined_samples),
            test_duration: basic_results.test_duration + accuracy_results.test_duration,
            sample_count: basic_results.sample_count + accuracy_results.sample_count,
            accuracy_percentage: accuracy_results.accuracy_percentage,
            performance_score: (basic_results.performance_score
                + accuracy_results.performance_score)
                / 2.0,
        };

        info!("Memory metrics validation completed successfully");
        Ok(combined_results)
    }

    /// Validate basic RSS tracking functionality
    fn validate_basic_rss_tracking(
        &self,
    ) -> Result<MemoryProfilingResults, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let test_duration = Duration::from_secs(self.config.test_duration_seconds);
        let sample_interval = Duration::from_millis(self.config.sample_interval_ms);

        let mut samples = Vec::new();
        let mut max_rss = 0u64;
        let mut min_rss = u64::MAX;
        let mut total_rss = 0u64;
        let mut sample_count = 0u64;

        info!(
            "Starting memory metrics validation for {} seconds",
            self.config.test_duration_seconds
        );

        while start_time.elapsed() < test_duration {
            thread::sleep(sample_interval);

            // Update RSS memory in metrics
            self.metrics.update_rss_memory();
            let current_rss = self.metrics.get_rss_bytes();

            // Record sample
            let sample = MemorySample {
                timestamp: Instant::now(),
                rss_bytes: current_rss,
                heap_bytes: 0,  // Not implemented yet
                stack_bytes: 0, // Not implemented yet
            };

            samples.push(sample);
            max_rss = max_rss.max(current_rss);
            min_rss = min_rss.min(current_rss);
            total_rss += current_rss;
            sample_count += 1;

            // Simulate some memory allocation to test tracking
            self.simulate_memory_activity();
        }

        let avg_rss = if sample_count > 0 {
            total_rss / sample_count
        } else {
            0
        };
        let variance = Self::calculate_variance(&samples);
        let performance_score = self.calculate_performance_score(variance, max_rss);

        let results = MemoryProfilingResults {
            samples,
            max_rss_bytes: max_rss,
            min_rss_bytes: min_rss,
            avg_rss_bytes: avg_rss,
            variance,
            test_duration: start_time.elapsed(),
            sample_count,
            accuracy_percentage: 95.0, // Default for basic tracking
            performance_score,
        };

        self.print_profiling_results(&results);
        Ok(results)
    }

    /// Validate accuracy of RSS measurements
    fn validate_accuracy(&self) -> Result<MemoryProfilingResults, Box<dyn std::error::Error>> {
        info!("Validating RSS measurement accuracy");

        let mut samples = Vec::new();
        let mut accuracy_sum = 0.0;
        let mut accuracy_count = 0;

        // Take multiple measurements and compare with direct RSS readings
        for _i in 0..10 {
            // Update metrics
            self.metrics.update_rss_memory();
            let metrics_rss = self.metrics.get_rss_bytes();

            // Get direct measurement
            let direct_rss = get_rss_memory_bytes().unwrap_or(0);

            // Calculate accuracy
            let accuracy = if direct_rss > 0 {
                let diff = (metrics_rss as i64 - direct_rss as i64).abs() as f64;
                let relative_error = diff / direct_rss as f64;
                (1.0 - relative_error) * 100.0
            } else {
                0.0
            };

            accuracy_sum += accuracy;
            accuracy_count += 1;

            let sample = MemorySample {
                timestamp: Instant::now(),
                rss_bytes: metrics_rss,
                heap_bytes: 0,
                stack_bytes: 0,
            };
            samples.push(sample);

            // Small delay between measurements
            thread::sleep(Duration::from_millis(100));
        }

        let avg_accuracy = if accuracy_count > 0 {
            accuracy_sum / accuracy_count as f64
        } else {
            0.0
        };
        let max_rss = samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0);
        let min_rss = samples.iter().map(|s| s.rss_bytes).min().unwrap_or(0);
        let avg_rss = samples.iter().map(|s| s.rss_bytes).sum::<u64>() / samples.len() as u64;

        Ok(MemoryProfilingResults {
            samples: samples.clone(),
            max_rss_bytes: max_rss,
            min_rss_bytes: min_rss,
            avg_rss_bytes: avg_rss,
            variance: Self::calculate_variance(&samples),
            test_duration: Duration::from_secs(1),
            sample_count: accuracy_count as u64,
            accuracy_percentage: avg_accuracy,
            performance_score: avg_accuracy / 100.0,
        })
    }

    /// Test memory metrics under load
    pub fn test_memory_under_load(
        &self,
    ) -> Result<MemoryProfilingResults, Box<dyn std::error::Error>> {
        info!("Validating memory metrics under simulated load");

        // Create memory pressure by allocating and deallocating memory
        let validator_clone1 = MemoryMetricsValidator::new(self.config.clone());
        let validator_clone2 = MemoryMetricsValidator::new(self.config.clone());
        let validator_clone3 = MemoryMetricsValidator::new(self.config.clone());

        // Run load simulation in background threads
        let handle1 = std::thread::spawn(move || validator_clone1.simulate_heavy_memory_load());
        let handle2 = std::thread::spawn(move || validator_clone2.simulate_memory_churn());
        let handle3 = std::thread::spawn(move || validator_clone3.simulate_gradual_growth());

        // Run profiling while under load
        let profiling_result = self.validate_memory_metrics()?;

        // Wait for background tasks to complete
        let _ = handle1.join();
        let _ = handle2.join();
        let _ = handle3.join();

        Ok(profiling_result)
    }

    /// Simulate memory activity for testing
    fn simulate_memory_activity(&self) {
        // Allocate some temporary memory to simulate real usage
        let _temp_data: Vec<u8> = vec![0; 1024 * 1024]; // 1MB allocation

        // Small delay to allow memory tracking
        thread::sleep(Duration::from_millis(1));
    }

    /// Simulate heavy memory load
    fn simulate_heavy_memory_load(&self) {
        let mut allocations = Vec::new();

        // Allocate memory in chunks
        for _ in 0..10 {
            let chunk: Vec<u8> = vec![0; 10 * 1024 * 1024];
            allocations.push(chunk);
            thread::sleep(Duration::from_millis(100));
        }

        // Hold memory for a while
        thread::sleep(Duration::from_secs(2));

        // Deallocate gradually
        while !allocations.is_empty() {
            allocations.pop();
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Simulate memory churn (frequent allocations/deallocations)
    fn simulate_memory_churn(&self) {
        for _ in 0..100 {
            let _temp: Vec<u8> = vec![0; 1024 * 1024]; // 1MB
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Simulate gradual memory growth
    fn simulate_gradual_growth(&self) {
        let mut persistent_data = Vec::new();

        // Gradually increase memory usage
        for i in 0..20 {
            let chunk: Vec<u8> = vec![i as u8; 500 * 1024];
            persistent_data.push(chunk);
            thread::sleep(Duration::from_millis(100));
        }

        // Keep data alive for the duration of the test
        thread::sleep(Duration::from_secs(3));
    }

    /// Calculate variance in memory samples
    fn calculate_variance(samples: &[MemorySample]) -> f64 {
        if samples.len() < 2 {
            return 0.0;
        }

        let mean = samples.iter().map(|s| s.rss_bytes as f64).sum::<f64>() / samples.len() as f64;
        let variance = samples
            .iter()
            .map(|s| {
                let diff = s.rss_bytes as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / samples.len() as f64;

        variance.sqrt() / mean // Coefficient of variation
    }

    /// Calculate performance score based on variance and memory usage
    fn calculate_performance_score(&self, variance: f64, max_rss: u64) -> f64 {
        let variance_score = if variance < self.config.max_variance_threshold {
            1.0 - (variance / self.config.max_variance_threshold)
        } else {
            0.0
        };

        let memory_score = if max_rss < self.config.memory_threshold_bytes {
            1.0
        } else {
            (self.config.memory_threshold_bytes as f64) / (max_rss as f64)
        };

        (variance_score + memory_score) / 2.0
    }

    /// Print profiling results
    fn print_profiling_results(&self, results: &MemoryProfilingResults) {
        info!("=== Memory Profiling Results ===");
        info!("Test Duration: {:?}", results.test_duration);
        info!("Samples Collected: {}", results.sample_count);
        info!("Max RSS: {} MB", results.max_rss_bytes / (1024 * 1024));
        info!("Min RSS: {} MB", results.min_rss_bytes / (1024 * 1024));
        info!("Avg RSS: {} MB", results.avg_rss_bytes / (1024 * 1024));
        info!("Variance: {:.4}", results.variance);
        info!("Accuracy: {:.2}%", results.accuracy_percentage);
        info!("Performance Score: {:.4}", results.performance_score);

        if results.variance > self.config.max_variance_threshold {
            warn!(
                "High variance detected: {:.4} > {:.4}",
                results.variance, self.config.max_variance_threshold
            );
        }

        if results.max_rss_bytes > self.config.memory_threshold_bytes {
            warn!(
                "Memory threshold exceeded: {} MB > {} MB",
                results.max_rss_bytes / (1024 * 1024),
                self.config.memory_threshold_bytes / (1024 * 1024)
            );
        }
    }
}

// Test functions
#[test]
fn test_memory_profiling() {
    let config = MemoryProfilingConfig {
        test_duration_seconds: 5,
        sample_interval_ms: 50,
        ..Default::default()
    };

    let validator = MemoryMetricsValidator::new(config);
    let results = validator
        .validate_memory_metrics()
        .expect("Memory profiling should succeed");

    assert!(results.sample_count > 0, "Should collect memory samples");
    assert!(results.max_rss_bytes > 0, "Should track RSS memory");
    assert!(
        results.performance_score >= 0.0 && results.performance_score <= 1.0,
        "Performance score should be normalized"
    );
}

#[test]
fn test_memory_under_load() {
    let config = MemoryProfilingConfig {
        test_duration_seconds: 3,
        sample_interval_ms: 100,
        enable_load_testing: true,
        ..Default::default()
    };

    let validator = MemoryMetricsValidator::new(config);
    let results = validator
        .test_memory_under_load()
        .expect("Load testing should succeed");

    assert!(
        results.sample_count > 0,
        "Should collect samples under load"
    );
    assert!(
        results.max_rss_bytes > results.min_rss_bytes,
        "Should show memory variation under load"
    );
}
