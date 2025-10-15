use qanto::config::LoggingConfig;
use qanto::elite_mempool::EliteMempool;
use qanto::mempool::Mempool;
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::transaction::{Input, Output, Transaction};
use qanto::types::{HomomorphicEncrypted, QuantumResistantSignature, UTXO};

#[allow(clippy::single_component_path_imports)]
use num_cpus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::info;

// Type alias for complex elite mempool metrics tuple
type EliteMetrics = (
    Option<f64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
);

/// Configuration for performance benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub duration_secs: u64,
    pub num_workers: usize,
    pub transactions_per_second: u64,
    pub transaction_size_bytes: usize,
    pub warmup_duration_secs: u64,
    pub target_block_time: u64,
    pub num_chains: u32,
    pub mempool_max_size: usize,
    pub mempool_size: usize,
    pub data_dir: String,
    pub enable_elite_mempool: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            duration_secs: 60,
            num_workers: num_cpus::get(),
            transactions_per_second: 1000,
            transaction_size_bytes: 250,
            warmup_duration_secs: 10,
            target_block_time: 10,
            num_chains: 1,
            mempool_max_size: 100000,
            mempool_size: 50000,
            data_dir: "./benchmark_data".to_string(),
            enable_elite_mempool: true,
        }
    }
}

/// Performance metrics collected during benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub timestamp: u64,
    pub transactions_per_second: f64,
    pub total_transactions: u64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub disk_io_read_mb: f64,
    pub disk_io_write_mb: f64,
    pub network_rx_mb: f64,
    pub network_tx_mb: f64,
    pub error_count: u64,
    pub success_rate: f64,
    // Elite mempool specific metrics
    pub elite_mempool_tps: Option<f64>,
    pub elite_mempool_processed: Option<u64>,
    pub elite_mempool_validated: Option<u64>,
    pub elite_mempool_rejected: Option<u64>,
    pub elite_mempool_avg_latency_ns: Option<u64>,
}

/// System information collected during benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_cores: usize,
    pub total_memory_gb: f64,
    pub available_memory_gb: f64,
    pub disk_space_gb: f64,
    pub os_info: String,
    pub rust_version: String,
}

/// Complete benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub config: BenchmarkConfig,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub total_duration_secs: f64,
    pub overall_metrics: PerformanceMetrics,
    pub detailed_metrics: Vec<PerformanceMetrics>,
    pub targets_met: HashMap<String, bool>,
    pub recommendations: Vec<String>,
    pub system_info: SystemInfo,
}

/// Main performance benchmark orchestrator
pub struct PerformanceBenchmark {
    config: BenchmarkConfig,
    dag: Arc<QantoDAG>,
    mempool: Arc<RwLock<Mempool>>,
    elite_mempool: Option<Arc<EliteMempool>>,
    metrics_history: Vec<PerformanceMetrics>,
    transaction_counter: Arc<AtomicU64>,
    error_counter: Arc<AtomicU64>,
}

impl PerformanceBenchmark {
    /// Create a new performance benchmark instance
    pub async fn new(config: BenchmarkConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize logging - just use tracing_subscriber directly
        let _ = tracing_subscriber::fmt::try_init();

        // Setup storage
        let storage_config = StorageConfig {
            data_dir: PathBuf::from(&config.data_dir),
            max_file_size: 64 * 1024 * 1024,
            compression_enabled: true,
            encryption_enabled: true,
            wal_enabled: true,
            sync_writes: true,
            cache_size: 128 * 1024 * 1024,
            compaction_threshold: 0.5,
            max_open_files: 1000,
        };
        let storage = Arc::new(QantoStorage::new(storage_config)?);

        let saga_pallet = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));

        // Setup DAG
        let dag_config = QantoDagConfig {
            initial_validator: "benchmark_validator".to_string(),
            target_block_time: 1000,
            num_chains: 1,
        };

        let logging_config = LoggingConfig {
            level: "info".to_string(),
            enable_block_celebrations: false,
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: Some(10),
        };

        let dag = QantoDAG::new(
            dag_config,
            saga_pallet.clone(),
            (*storage).clone(),
            logging_config,
        )?;

        // Initialize mempools
        let mempool = Arc::new(RwLock::new(Mempool::new(
            3600,                              // max_age_secs: 1 hour
            config.mempool_size * 1024 * 1024, // max_size_bytes: convert MB to bytes
            config.mempool_size,               // max_transactions
        )));
        let elite_mempool = if config.enable_elite_mempool {
            Some(Arc::new(EliteMempool::new(
                config.mempool_size,
                config.mempool_size * 250, // max_size_bytes
                4,                         // shard_count
                config.num_workers,        // worker_count
            )?))
        } else {
            None
        };

        Ok(Self {
            config,
            dag,
            mempool,
            elite_mempool,
            metrics_history: Vec::new(),
            transaction_counter: Arc::new(AtomicU64::new(0)),
            error_counter: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Run the complete benchmark
    pub async fn run_benchmark(&mut self) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        info!("Starting performance benchmark");
        let start_time = SystemTime::now();

        // Setup genesis UTXO set for valid transactions
        info!("Setting up genesis UTXO set for benchmark");
        let genesis_utxos = self.setup_genesis_utxos().await?;

        // Start elite mempool if enabled
        if let Some(ref elite_mempool) = self.elite_mempool {
            elite_mempool.start().await?;
            info!("Elite mempool started");
        }

        // Run warmup phase
        info!(
            "Running warmup phase for {} seconds",
            self.config.warmup_duration_secs
        );
        self.run_warmup().await?;

        // Start worker tasks with genesis UTXOs
        let worker_handles = self.start_worker_tasks_with_utxos(&genesis_utxos).await?;

        // Start metrics collection
        let metrics_handle = self.start_metrics_collection().await;

        // Wait for benchmark duration
        info!(
            "Running benchmark for {} seconds",
            self.config.duration_secs
        );
        sleep(Duration::from_secs(self.config.duration_secs)).await;

        // Stop workers and metrics collection
        for handle in worker_handles {
            handle.abort();
        }
        metrics_handle.abort();

        let end_time = SystemTime::now();
        let total_duration = end_time.duration_since(start_time)?.as_secs_f64();

        info!("Benchmark completed in {:.2} seconds", total_duration);

        // Collect final metrics
        let overall_metrics = self.collect_final_metrics().await;
        let system_info = self.collect_system_info().await;
        let targets_met = self.analyze_targets(&overall_metrics);
        let recommendations = self.generate_recommendations(&overall_metrics, &targets_met);

        // Save results
        let result = BenchmarkResult {
            config: self.config.clone(),
            start_time,
            end_time,
            total_duration_secs: total_duration,
            overall_metrics,
            detailed_metrics: self.metrics_history.clone(),
            targets_met,
            recommendations,
            system_info,
        };

        self.save_results(&result).await?;
        Ok(result)
    }

    /// Run warmup phase to stabilize system
    async fn run_warmup(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting warmup phase");
        let warmup_start = Instant::now();
        let warmup_duration = Duration::from_secs(self.config.warmup_duration_secs);

        while warmup_start.elapsed() < warmup_duration {
            // Generate some dummy transactions for warmup
            for _ in 0..100 {
                let tx = Transaction::new_dummy();
                let empty_utxos = HashMap::new();

                // Try adding to standard mempool
                if let Ok(mempool) = self.mempool.try_write() {
                    let _ = mempool
                        .add_transaction(tx.clone(), &empty_utxos, &self.dag)
                        .await;
                }

                // Try adding to elite mempool if available
                if let Some(ref elite_mempool) = self.elite_mempool {
                    let _ = elite_mempool
                        .add_transaction(tx, &empty_utxos, &self.dag)
                        .await;
                }
            }

            sleep(Duration::from_millis(100)).await;
        }

        info!("Warmup phase completed");
        Ok(())
    }

    /// Setup genesis UTXO set for valid transactions
    async fn setup_genesis_utxos(
        &self,
    ) -> Result<HashMap<String, UTXO>, Box<dyn std::error::Error>> {
        let mut genesis_utxos = HashMap::new();

        // Generate 1000 genesis UTXOs with substantial amounts
        for i in 0..1000 {
            let genesis_tx_id = format!("genesis_tx_{i:06}");
            let utxo_key = format!("{genesis_tx_id}_0");

            let utxo = UTXO {
                address: format!("genesis_address_{:06}", i % 100), // 100 different addresses
                amount: 1_000_000_000,                              // 1 billion units per UTXO
                tx_id: genesis_tx_id,
                output_index: 0,
                explorer_link: format!("/explorer/utxo/{utxo_key}"),
            };

            genesis_utxos.insert(utxo_key, utxo);
        }

        info!(
            "Generated {} genesis UTXOs for benchmark",
            genesis_utxos.len()
        );
        Ok(genesis_utxos)
    }

    /// Create a valid transaction that spends from the genesis UTXO set
    fn create_valid_transaction(
        genesis_utxos: &HashMap<String, UTXO>,
        tx_counter: u64,
    ) -> Transaction {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Select a random genesis UTXO to spend from
        let utxo_keys: Vec<_> = genesis_utxos.keys().collect();
        let selected_utxo_key = utxo_keys[tx_counter as usize % utxo_keys.len()];
        let selected_utxo = &genesis_utxos[selected_utxo_key];

        // Generate transaction ID
        let tx_id: [u8; 32] = rng.gen();
        let tx_id_str = hex::encode(tx_id);

        // Create input that spends the selected UTXO
        let input = Input {
            tx_id: selected_utxo.tx_id.clone(),
            output_index: selected_utxo.output_index,
        };

        // Create outputs (send to new addresses)
        let output_amount = selected_utxo.amount / 2; // Split the UTXO
        let change_amount = selected_utxo.amount - output_amount - 1000; // 1000 units fee

        let outputs = vec![
            Output {
                address: format!("recipient_{:06}", tx_counter % 1000),
                amount: output_amount,
                homomorphic_encrypted: HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            },
            Output {
                address: selected_utxo.address.clone(), // Change back to sender
                amount: change_amount,
                homomorphic_encrypted: HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            },
        ];

        Transaction {
            id: tx_id_str,
            sender: selected_utxo.address.clone(),
            receiver: format!("recipient_{:06}", tx_counter % 1000),
            amount: output_amount,
            fee: 1000,
            gas_limit: 50000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: vec![input],
            outputs,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            metadata: HashMap::new(),
            signature: QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            fee_breakdown: None,
        }
    }

    /// Start worker tasks for transaction generation with genesis UTXOs
    async fn start_worker_tasks_with_utxos(
        &self,
        genesis_utxos: &HashMap<String, UTXO>,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>, Box<dyn std::error::Error>> {
        let mut handles = Vec::new();
        let target_interval =
            Duration::from_nanos(1_000_000_000 / self.config.transactions_per_second);

        for worker_id in 0..self.config.num_workers {
            let mempool = self.mempool.clone();
            let elite_mempool = self.elite_mempool.clone();
            let counter = self.transaction_counter.clone();
            let error_counter = self.error_counter.clone();
            let _tx_size = self.config.transaction_size_bytes;
            let dag = self.dag.clone();
            let genesis_utxos_clone = genesis_utxos.clone();

            let handle = tokio::spawn(async move {
                let mut tx_count = 0u64;
                let mut last_tx_time = Instant::now();

                loop {
                    // Rate limiting
                    let elapsed = last_tx_time.elapsed();
                    if elapsed < target_interval {
                        sleep(target_interval - elapsed).await;
                    }
                    last_tx_time = Instant::now();

                    // Create valid transaction using genesis UTXOs
                    let tx = Self::create_valid_transaction(&genesis_utxos_clone, tx_count);

                    // Alternate between standard and elite mempool
                    if worker_id.is_multiple_of(2) {
                        // Use standard mempool with valid UTXOs
                        let tx_clone = tx.clone();
                        let dag_clone = dag.clone();

                        // Clone the mempool Arc to avoid holding the guard across await
                        let mempool_clone = mempool.clone();
                        let result = {
                            let mempool_guard = mempool_clone.write().await;
                            mempool_guard
                                .add_transaction(tx_clone, &genesis_utxos_clone, &dag_clone)
                                .await
                        };

                        if result.is_ok() {
                            counter.fetch_add(1, Ordering::Relaxed);
                        } else {
                            error_counter.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if let Some(ref elite) = elite_mempool {
                        // Use elite mempool with valid UTXOs
                        if elite
                            .add_transaction(tx, &genesis_utxos_clone, &dag)
                            .await
                            .is_ok()
                        {
                            counter.fetch_add(1, Ordering::Relaxed);
                        } else {
                            error_counter.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    tx_count += 1;
                    tokio::task::yield_now().await;
                }
            });

            handles.push(handle);
        }

        info!("Started {} worker tasks with valid UTXO set", handles.len());
        Ok(handles)
    }

    /// Start metrics collection task
    async fn start_metrics_collection(&mut self) -> tokio::task::JoinHandle<()> {
        let _mempool = self.mempool.clone();
        let _elite_mempool = self.elite_mempool.clone();
        let counter = self.transaction_counter.clone();
        let error_counter = self.error_counter.clone();

        tokio::spawn(async move {
            let mut last_count = 0;
            let mut last_time = Instant::now();

            loop {
                sleep(Duration::from_secs(1)).await;

                let current_count = counter.load(Ordering::Relaxed);
                let current_time = Instant::now();
                let elapsed = current_time.duration_since(last_time).as_secs_f64();

                let tps = (current_count - last_count) as f64 / elapsed;
                let errors = error_counter.load(Ordering::Relaxed);
                let success_rate = if current_count > 0 {
                    ((current_count - errors) as f64 / current_count as f64) * 100.0
                } else {
                    0.0
                };

                // Collect elite mempool metrics if available
                let elite_metrics: EliteMetrics = if let Some(ref elite) = _elite_mempool {
                    let metrics = elite.get_metrics();
                    (
                        Some(metrics.get("peak_throughput_tps").copied().unwrap_or(0) as f64),
                        Some(metrics.get("transactions_processed").copied().unwrap_or(0)),
                        Some(metrics.get("transactions_validated").copied().unwrap_or(0)),
                        Some(metrics.get("transactions_rejected").copied().unwrap_or(0)),
                        Some(
                            metrics
                                .get("average_processing_time_ns")
                                .copied()
                                .unwrap_or(0),
                        ),
                    )
                } else {
                    (None, None, None, None, None)
                };

                // Destructure the elite metrics tuple
                let (elite_tps, elite_processed, elite_validated, elite_rejected, elite_latency) =
                    elite_metrics;

                let _metrics = PerformanceMetrics {
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    transactions_per_second: tps,
                    total_transactions: current_count,
                    memory_usage_mb: Self::get_memory_usage(),
                    cpu_usage_percent: Self::get_cpu_usage(),
                    disk_io_read_mb: 0.0,  // Placeholder
                    disk_io_write_mb: 0.0, // Placeholder
                    network_rx_mb: 0.0,    // Placeholder
                    network_tx_mb: 0.0,    // Placeholder
                    error_count: errors,
                    success_rate,
                    elite_mempool_tps: elite_tps,
                    elite_mempool_processed: elite_processed,
                    elite_mempool_validated: elite_validated,
                    elite_mempool_rejected: elite_rejected,
                    elite_mempool_avg_latency_ns: elite_latency,
                };

                info!(
                    "TPS: {:.2}, Total: {}, Errors: {}, Success Rate: {:.2}%",
                    tps, current_count, errors, success_rate
                );

                last_count = current_count;
                last_time = current_time;
            }
        })
    }

    /// Collect final performance metrics
    async fn collect_final_metrics(&self) -> PerformanceMetrics {
        let total_transactions = self.transaction_counter.load(Ordering::Relaxed);
        let total_errors = self.error_counter.load(Ordering::Relaxed);
        let success_rate = if total_transactions > 0 {
            ((total_transactions - total_errors) as f64 / total_transactions as f64) * 100.0
        } else {
            0.0
        };

        // Calculate average TPS over benchmark duration
        let avg_tps = total_transactions as f64 / self.config.duration_secs as f64;

        // Collect elite mempool metrics if available
        let (elite_tps, elite_processed, elite_validated, elite_rejected, elite_latency) =
            if let Some(ref elite_mempool) = self.elite_mempool {
                let metrics = elite_mempool.get_metrics();
                (
                    Some(metrics.get("peak_throughput_tps").copied().unwrap_or(0) as f64),
                    Some(metrics.get("transactions_processed").copied().unwrap_or(0)),
                    Some(metrics.get("transactions_validated").copied().unwrap_or(0)),
                    Some(metrics.get("transactions_rejected").copied().unwrap_or(0)),
                    Some(
                        metrics
                            .get("average_processing_time_ns")
                            .copied()
                            .unwrap_or(0),
                    ),
                )
            } else {
                (None, None, None, None, None)
            };

        PerformanceMetrics {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            transactions_per_second: avg_tps,
            total_transactions,
            memory_usage_mb: Self::get_memory_usage(),
            cpu_usage_percent: Self::get_cpu_usage(),
            disk_io_read_mb: 0.0,
            disk_io_write_mb: 0.0,
            network_rx_mb: 0.0,
            network_tx_mb: 0.0,
            error_count: total_errors,
            success_rate,
            elite_mempool_tps: elite_tps,
            elite_mempool_processed: elite_processed,
            elite_mempool_validated: elite_validated,
            elite_mempool_rejected: elite_rejected,
            elite_mempool_avg_latency_ns: elite_latency,
        }
    }

    /// Get current memory usage in MB
    fn get_memory_usage() -> f64 {
        // Placeholder implementation
        0.0
    }

    /// Get current CPU usage percentage
    fn get_cpu_usage() -> f64 {
        // Placeholder implementation
        0.0
    }

    /// Collect system information
    async fn collect_system_info(&self) -> SystemInfo {
        SystemInfo {
            cpu_cores: num_cpus::get(),
            total_memory_gb: 0.0,     // Placeholder
            available_memory_gb: 0.0, // Placeholder
            disk_space_gb: 0.0,       // Placeholder
            os_info: std::env::consts::OS.to_string(),
            rust_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Analyze if performance targets were met
    fn analyze_targets(&self, metrics: &PerformanceMetrics) -> HashMap<String, bool> {
        let mut targets = HashMap::new();

        targets.insert(
            "target_tps".to_string(),
            metrics.transactions_per_second >= self.config.transactions_per_second as f64,
        );
        targets.insert("success_rate".to_string(), metrics.success_rate >= 95.0);
        targets.insert("low_error_rate".to_string(), metrics.error_count < 100);

        if let Some(elite_tps) = metrics.elite_mempool_tps {
            targets.insert(
                "elite_mempool_performance".to_string(),
                elite_tps >= 50000.0,
            );
        }

        targets
    }

    /// Generate performance recommendations
    fn generate_recommendations(
        &self,
        metrics: &PerformanceMetrics,
        targets: &HashMap<String, bool>,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        if !targets.get("target_tps").unwrap_or(&false) {
            recommendations.push(
                "Consider increasing worker count or optimizing transaction processing".to_string(),
            );
        }

        if !targets.get("success_rate").unwrap_or(&false) {
            recommendations.push(
                "High error rate detected - investigate mempool capacity or validation issues"
                    .to_string(),
            );
        }

        if metrics.memory_usage_mb > 1000.0 {
            recommendations.push("High memory usage - consider tuning cache sizes".to_string());
        }

        if let Some(elite_tps) = metrics.elite_mempool_tps {
            if elite_tps < 50000.0 {
                recommendations.push("Elite mempool underperforming - check SIMD optimization and worker configuration".to_string());
            }
        }

        recommendations
    }

    /// Save benchmark results to file
    async fn save_results(
        &self,
        result: &BenchmarkResult,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let filename = format!(
            "benchmark_results_{}.json",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        );
        let json = serde_json::to_string_pretty(result)?;
        tokio::fs::write(&filename, json).await?;
        info!("Benchmark results saved to {}", filename);
        Ok(())
    }
}

/// Main benchmark execution function
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = BenchmarkConfig::default();
    let mut benchmark = PerformanceBenchmark::new(config).await?;
    let result = benchmark.run_benchmark().await?;

    info!("Benchmark completed successfully!");
    info!(
        "Final TPS: {:.2}",
        result.overall_metrics.transactions_per_second
    );
    info!(
        "Total transactions: {}",
        result.overall_metrics.total_transactions
    );
    info!("Success rate: {:.2}%", result.overall_metrics.success_rate);

    if let Some(elite_tps) = result.overall_metrics.elite_mempool_tps {
        info!("Elite mempool TPS: {:.2}", elite_tps);
    }

    Ok(())
}
