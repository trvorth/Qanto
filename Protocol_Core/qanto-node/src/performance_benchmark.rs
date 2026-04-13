use qanto::config::LoggingConfig;
use qanto::elite_mempool::{EliteMempool, EliteMempoolError};
use qanto::mempool::{Mempool, MempoolError};
use qanto::qanto_storage::{QantoStorage, StorageConfig};
use qanto::qantodag::{QantoDAG, QantoDagConfig};
use qanto::saga::PalletSaga;
use qanto::transaction::{Input, Output, Transaction, TransactionError};
use qanto::types::{HomomorphicEncrypted, QuantumResistantSignature, UTXO};
use sysinfo::{get_current_pid, ProcessRefreshKind, System};

#[allow(clippy::single_component_path_imports)]
use num_cpus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::info;

static PROCESS_SYS: OnceLock<Mutex<System>> = OnceLock::new();

// Type alias for complex elite mempool metrics tuple
#[allow(dead_code)]
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
    // New: rejection breakdowns for standard and elite mempools
    pub std_rejection_breakdown: Option<HashMap<String, u64>>,
    pub elite_rejection_breakdown: Option<HashMap<String, u64>>,
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
    metrics_history: Arc<RwLock<Vec<PerformanceMetrics>>>,
    transaction_counter: Arc<AtomicU64>,
    error_counter: Arc<AtomicU64>,
    // New: rejection counters for detailed breakdowns
    std_reject_backpressure: Arc<AtomicU64>,
    std_reject_full: Arc<AtomicU64>,
    std_reject_validation: Arc<AtomicU64>,
    std_reject_timestamp: Arc<AtomicU64>,
    std_reject_tx: Arc<AtomicU64>,
    elite_reject_validation: Arc<AtomicU64>,
    elite_reject_capacity: Arc<AtomicU64>,
    elite_reject_worker_pool: Arc<AtomicU64>,
    elite_reject_channel: Arc<AtomicU64>,
    elite_reject_tx: Arc<AtomicU64>,
    // Granular TransactionError counters (standard)
    std_tx_omega_rejection: Arc<AtomicU64>,
    std_tx_invalid_address: Arc<AtomicU64>,
    std_tx_signature_verification: Arc<AtomicU64>,
    std_tx_insufficient_funds: Arc<AtomicU64>,
    std_tx_homomorphic_error: Arc<AtomicU64>,
    std_tx_rate_limit: Arc<AtomicU64>,
    std_tx_anomaly_detected: Arc<AtomicU64>,
    std_tx_serialization_error: Arc<AtomicU64>,
    std_tx_timestamp_error_via_tx: Arc<AtomicU64>,
    std_tx_wallet_error: Arc<AtomicU64>,
    std_tx_invalid_metadata: Arc<AtomicU64>,
    std_tx_pqcrypto_error: Arc<AtomicU64>,
    std_tx_gas_fee_error: Arc<AtomicU64>,
    std_tx_other: Arc<AtomicU64>,
    // Granular InvalidStructure classifiers (standard)
    std_tx_is_utxo_not_found: Arc<AtomicU64>,
    std_tx_is_duplicate_input: Arc<AtomicU64>,
    std_tx_is_input_output_mismatch: Arc<AtomicU64>,
    std_tx_is_sender_mismatch: Arc<AtomicU64>,
    std_tx_is_coinbase_fee_nonzero: Arc<AtomicU64>,
    std_tx_is_coinbase_output_zero: Arc<AtomicU64>,
    std_tx_is_empty_tx_id: Arc<AtomicU64>,
    std_tx_is_empty_in_out: Arc<AtomicU64>,
    std_tx_is_fee_required: Arc<AtomicU64>,
    std_tx_is_output_zero: Arc<AtomicU64>,
    // Granular TransactionError counters (elite)
    elite_tx_omega_rejection: Arc<AtomicU64>,
    elite_tx_invalid_address: Arc<AtomicU64>,
    elite_tx_signature_verification: Arc<AtomicU64>,
    elite_tx_insufficient_funds: Arc<AtomicU64>,
    elite_tx_homomorphic_error: Arc<AtomicU64>,
    elite_tx_rate_limit: Arc<AtomicU64>,
    elite_tx_anomaly_detected: Arc<AtomicU64>,
    elite_tx_serialization_error: Arc<AtomicU64>,
    elite_tx_timestamp_error_via_tx: Arc<AtomicU64>,
    elite_tx_wallet_error: Arc<AtomicU64>,
    elite_tx_invalid_metadata: Arc<AtomicU64>,
    elite_tx_pqcrypto_error: Arc<AtomicU64>,
    elite_tx_gas_fee_error: Arc<AtomicU64>,
    elite_tx_other: Arc<AtomicU64>,
    // Granular InvalidStructure classifiers (elite)
    elite_tx_is_utxo_not_found: Arc<AtomicU64>,
    elite_tx_is_duplicate_input: Arc<AtomicU64>,
    elite_tx_is_input_output_mismatch: Arc<AtomicU64>,
    elite_tx_is_sender_mismatch: Arc<AtomicU64>,
    elite_tx_is_coinbase_fee_nonzero: Arc<AtomicU64>,
    elite_tx_is_coinbase_output_zero: Arc<AtomicU64>,
    elite_tx_is_empty_tx_id: Arc<AtomicU64>,
    elite_tx_is_empty_in_out: Arc<AtomicU64>,
    elite_tx_is_fee_required: Arc<AtomicU64>,
    elite_tx_is_output_zero: Arc<AtomicU64>,
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
            compaction_threshold: 10, // number of segments before compaction
            max_open_files: 1000,
            // Sensible defaults aligned with StorageConfig::default()
            memtable_size: 64 * 1024 * 1024,
            write_buffer_size: 16 * 1024 * 1024,
            batch_size: 10000,
            parallel_writers: 8,
            enable_write_batching: true,
            enable_bloom_filters: true,
            enable_async_io: true,
            sync_interval: Duration::from_millis(100),
            compression_level: 3,
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
            dev_fee_rate: 0.10,
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
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            transaction_counter: Arc::new(AtomicU64::new(0)),
            error_counter: Arc::new(AtomicU64::new(0)),
            std_reject_backpressure: Arc::new(AtomicU64::new(0)),
            std_reject_full: Arc::new(AtomicU64::new(0)),
            std_reject_validation: Arc::new(AtomicU64::new(0)),
            std_reject_timestamp: Arc::new(AtomicU64::new(0)),
            std_reject_tx: Arc::new(AtomicU64::new(0)),
            elite_reject_validation: Arc::new(AtomicU64::new(0)),
            elite_reject_capacity: Arc::new(AtomicU64::new(0)),
            elite_reject_worker_pool: Arc::new(AtomicU64::new(0)),
            elite_reject_channel: Arc::new(AtomicU64::new(0)),
            elite_reject_tx: Arc::new(AtomicU64::new(0)),
            // Init granular TransactionError counters (standard)
            std_tx_omega_rejection: Arc::new(AtomicU64::new(0)),
            std_tx_invalid_address: Arc::new(AtomicU64::new(0)),
            std_tx_signature_verification: Arc::new(AtomicU64::new(0)),
            std_tx_insufficient_funds: Arc::new(AtomicU64::new(0)),
            std_tx_homomorphic_error: Arc::new(AtomicU64::new(0)),
            std_tx_rate_limit: Arc::new(AtomicU64::new(0)),
            std_tx_anomaly_detected: Arc::new(AtomicU64::new(0)),
            std_tx_serialization_error: Arc::new(AtomicU64::new(0)),
            std_tx_timestamp_error_via_tx: Arc::new(AtomicU64::new(0)),
            std_tx_wallet_error: Arc::new(AtomicU64::new(0)),
            std_tx_invalid_metadata: Arc::new(AtomicU64::new(0)),
            std_tx_pqcrypto_error: Arc::new(AtomicU64::new(0)),
            std_tx_gas_fee_error: Arc::new(AtomicU64::new(0)),
            std_tx_other: Arc::new(AtomicU64::new(0)),
            // Init granular InvalidStructure classifiers (standard)
            std_tx_is_utxo_not_found: Arc::new(AtomicU64::new(0)),
            std_tx_is_duplicate_input: Arc::new(AtomicU64::new(0)),
            std_tx_is_input_output_mismatch: Arc::new(AtomicU64::new(0)),
            std_tx_is_sender_mismatch: Arc::new(AtomicU64::new(0)),
            std_tx_is_coinbase_fee_nonzero: Arc::new(AtomicU64::new(0)),
            std_tx_is_coinbase_output_zero: Arc::new(AtomicU64::new(0)),
            std_tx_is_empty_tx_id: Arc::new(AtomicU64::new(0)),
            std_tx_is_empty_in_out: Arc::new(AtomicU64::new(0)),
            std_tx_is_fee_required: Arc::new(AtomicU64::new(0)),
            std_tx_is_output_zero: Arc::new(AtomicU64::new(0)),
            // Init granular TransactionError counters (elite)
            elite_tx_omega_rejection: Arc::new(AtomicU64::new(0)),
            elite_tx_invalid_address: Arc::new(AtomicU64::new(0)),
            elite_tx_signature_verification: Arc::new(AtomicU64::new(0)),
            elite_tx_insufficient_funds: Arc::new(AtomicU64::new(0)),
            elite_tx_homomorphic_error: Arc::new(AtomicU64::new(0)),
            elite_tx_rate_limit: Arc::new(AtomicU64::new(0)),
            elite_tx_anomaly_detected: Arc::new(AtomicU64::new(0)),
            elite_tx_serialization_error: Arc::new(AtomicU64::new(0)),
            elite_tx_timestamp_error_via_tx: Arc::new(AtomicU64::new(0)),
            elite_tx_wallet_error: Arc::new(AtomicU64::new(0)),
            elite_tx_invalid_metadata: Arc::new(AtomicU64::new(0)),
            elite_tx_pqcrypto_error: Arc::new(AtomicU64::new(0)),
            elite_tx_gas_fee_error: Arc::new(AtomicU64::new(0)),
            elite_tx_other: Arc::new(AtomicU64::new(0)),
            // Init granular InvalidStructure classifiers (elite)
            elite_tx_is_utxo_not_found: Arc::new(AtomicU64::new(0)),
            elite_tx_is_duplicate_input: Arc::new(AtomicU64::new(0)),
            elite_tx_is_input_output_mismatch: Arc::new(AtomicU64::new(0)),
            elite_tx_is_sender_mismatch: Arc::new(AtomicU64::new(0)),
            elite_tx_is_coinbase_fee_nonzero: Arc::new(AtomicU64::new(0)),
            elite_tx_is_coinbase_output_zero: Arc::new(AtomicU64::new(0)),
            elite_tx_is_empty_tx_id: Arc::new(AtomicU64::new(0)),
            elite_tx_is_empty_in_out: Arc::new(AtomicU64::new(0)),
            elite_tx_is_fee_required: Arc::new(AtomicU64::new(0)),
            elite_tx_is_output_zero: Arc::new(AtomicU64::new(0)),
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
            detailed_metrics: self.metrics_history.read().await.clone(),
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
    fn create_valid_transaction_from_utxo(selected_utxo: &UTXO, tx_counter: u64) -> Transaction {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let tx_id: [u8; 32] = rng.gen();
        let tx_id_str = hex::encode(tx_id);

        let amount = selected_utxo.amount;
        let fee = 1_000; // small fixed fee for benchmark stability
        let send_amount = amount.saturating_sub(fee) / 2;
        let change_amount = amount.saturating_sub(fee).saturating_sub(send_amount);

        let input = Input {
            tx_id: selected_utxo.tx_id.clone(),
            output_index: selected_utxo.output_index,
        };

        let receiver_address = format!("receiver_{tx_counter}");

        let outputs = vec![
            Output {
                address: receiver_address,
                amount: send_amount,
                homomorphic_encrypted: HomomorphicEncrypted::default(),
            },
            Output {
                address: selected_utxo.address.clone(),
                amount: change_amount,
                homomorphic_encrypted: HomomorphicEncrypted::default(),
            },
        ];

        Transaction {
            id: tx_id_str,
            sender: selected_utxo.address.clone(),
            receiver: outputs[0].address.clone(),
            amount: send_amount,
            fee,
            gas_limit: 21_000,
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
            signature: QuantumResistantSignature::default(),
            fee_breakdown: None,
        }
    }

    // Helper classifiers for TransactionError::InvalidStructure messages
    fn is_utxo_not_found(msg: &str) -> bool {
        // supports "UTXO not found: ..." and "UTXO ... not found"
        let m = msg.to_ascii_lowercase();
        (m.contains("utxo") && m.contains("not found")) || m.contains("utxo not found:")
    }

    fn is_duplicate_input(msg: &str) -> bool {
        msg.contains("Duplicate input detected")
    }

    fn is_input_output_mismatch(msg: &str) -> bool {
        msg.contains("Input/output value mismatch")
    }

    fn is_sender_mismatch(msg: &str) -> bool {
        let m = msg.to_ascii_lowercase();
        m.contains("does not belong to sender") || m.contains("does not match sender")
    }

    fn is_coinbase_fee_nonzero(msg: &str) -> bool {
        msg.contains("Coinbase fee must be 0")
    }

    fn is_coinbase_output_zero(msg: &str) -> bool {
        msg.contains("Coinbase output cannot be zero")
    }

    fn is_empty_tx_id(msg: &str) -> bool {
        msg.contains("Transaction ID cannot be empty")
    }

    fn is_empty_in_out(msg: &str) -> bool {
        msg.contains("Transaction cannot have both empty inputs and outputs")
    }

    fn is_fee_required(msg: &str) -> bool {
        msg.contains("Non-coinbase transaction must have a fee")
    }

    fn is_output_zero(msg: &str) -> bool {
        msg.contains("Output amount cannot be zero")
    }

    /// Start worker tasks for transaction generation with genesis UTXOs
    async fn start_worker_tasks_with_utxos(
        &self,
        genesis_utxos: &HashMap<String, UTXO>,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>, Box<dyn std::error::Error>> {
        let num_workers = self.config.num_workers.max(1);
        let total_tps = self.config.transactions_per_second.max(1);
        let per_worker_tps = (total_tps / num_workers as u64).max(1);
        let target_interval = Duration::from_nanos(1_000_000_000u64 / per_worker_tps);

        // Snapshot and partition genesis UTXOs into disjoint per-worker sets
        let genesis_utxos_snapshot = genesis_utxos.clone();
        let mut worker_sets: Vec<HashMap<String, UTXO>> =
            (0..num_workers).map(|_| HashMap::new()).collect();
        for (i, (k, v)) in genesis_utxos_snapshot.into_iter().enumerate() {
            worker_sets[i % num_workers].insert(k, v);
        }

        let worker_local_utxos: Vec<Arc<RwLock<HashMap<String, UTXO>>>> = worker_sets
            .into_iter()
            .map(|m| Arc::new(RwLock::new(m)))
            .collect();

        let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::with_capacity(num_workers);
        let mempool_arc = self.mempool.clone();
        let elite_arc = self.elite_mempool.clone();
        let dag_arc = self.dag.clone();

        let tx_counter_arc = self.transaction_counter.clone();
        let err_counter_arc = self.error_counter.clone();

        // Standard mempool rejection counters
        let std_bp = self.std_reject_backpressure.clone();
        let std_full = self.std_reject_full.clone();
        let std_val = self.std_reject_validation.clone();
        let std_ts = self.std_reject_timestamp.clone();
        let std_tx = self.std_reject_tx.clone();

        // Elite mempool rejection counters
        let elite_val = self.elite_reject_validation.clone();
        let elite_cap = self.elite_reject_capacity.clone();
        let elite_wp = self.elite_reject_worker_pool.clone();
        let elite_ch = self.elite_reject_channel.clone();
        let elite_tx = self.elite_reject_tx.clone();

        // Clone granular counters for worker loop classification
        let std_tx_omega = self.std_tx_omega_rejection.clone();
        let std_tx_invalid_addr = self.std_tx_invalid_address.clone();
        let std_tx_sig = self.std_tx_signature_verification.clone();
        let std_tx_insuf = self.std_tx_insufficient_funds.clone();
        let std_tx_homo = self.std_tx_homomorphic_error.clone();
        let std_tx_rate = self.std_tx_rate_limit.clone();
        let std_tx_anom = self.std_tx_anomaly_detected.clone();
        let std_tx_ser = self.std_tx_serialization_error.clone();
        let std_tx_ts_tx = self.std_tx_timestamp_error_via_tx.clone();
        let std_tx_wallet = self.std_tx_wallet_error.clone();
        let std_tx_meta = self.std_tx_invalid_metadata.clone();
        let std_tx_pq = self.std_tx_pqcrypto_error.clone();
        let std_tx_gas = self.std_tx_gas_fee_error.clone();
        let std_is_utxo_nf = self.std_tx_is_utxo_not_found.clone();
        let std_is_dup_in = self.std_tx_is_duplicate_input.clone();
        let std_is_io_mm = self.std_tx_is_input_output_mismatch.clone();
        let std_is_sender_mm = self.std_tx_is_sender_mismatch.clone();
        let std_is_cb_fee = self.std_tx_is_coinbase_fee_nonzero.clone();
        let std_is_cb_out0 = self.std_tx_is_coinbase_output_zero.clone();
        let std_is_empty_id = self.std_tx_is_empty_tx_id.clone();
        let std_is_empty_io = self.std_tx_is_empty_in_out.clone();
        let std_is_fee_req = self.std_tx_is_fee_required.clone();
        let std_is_out0 = self.std_tx_is_output_zero.clone();
        let std_tx_other_c = self.std_tx_other.clone();

        let elite_tx_omega = self.elite_tx_omega_rejection.clone();
        let elite_tx_invalid_addr = self.elite_tx_invalid_address.clone();
        let elite_tx_sig = self.elite_tx_signature_verification.clone();
        let elite_tx_insuf = self.elite_tx_insufficient_funds.clone();
        let elite_tx_homo = self.elite_tx_homomorphic_error.clone();
        let elite_tx_rate = self.elite_tx_rate_limit.clone();
        let elite_tx_anom = self.elite_tx_anomaly_detected.clone();
        let elite_tx_ser = self.elite_tx_serialization_error.clone();
        let elite_tx_ts_tx = self.elite_tx_timestamp_error_via_tx.clone();
        let elite_tx_wallet = self.elite_tx_wallet_error.clone();
        let elite_tx_meta = self.elite_tx_invalid_metadata.clone();
        let elite_tx_pq = self.elite_tx_pqcrypto_error.clone();
        let elite_tx_gas = self.elite_tx_gas_fee_error.clone();
        let elite_is_utxo_nf = self.elite_tx_is_utxo_not_found.clone();
        let elite_is_dup_in = self.elite_tx_is_duplicate_input.clone();
        let elite_is_io_mm = self.elite_tx_is_input_output_mismatch.clone();
        let elite_is_sender_mm = self.elite_tx_is_sender_mismatch.clone();
        let elite_is_cb_fee = self.elite_tx_is_coinbase_fee_nonzero.clone();
        let elite_is_cb_out0 = self.elite_tx_is_coinbase_output_zero.clone();
        let elite_is_empty_id = self.elite_tx_is_empty_tx_id.clone();
        let elite_is_empty_io = self.elite_tx_is_empty_in_out.clone();
        let elite_is_fee_req = self.elite_tx_is_fee_required.clone();
        let elite_is_out0 = self.elite_tx_is_output_zero.clone();
        let elite_tx_other_c = self.elite_tx_other.clone();

        for (worker_id, local_utxos) in worker_local_utxos.iter().enumerate().take(num_workers) {
            let local_utxos = local_utxos.clone();
            let mempool_clone = mempool_arc.clone();
            let elite_clone = elite_arc.clone();
            let dag_clone = dag_arc.clone();

            let tx_counter = tx_counter_arc.clone();
            let err_counter = err_counter_arc.clone();
            let std_bp_c = std_bp.clone();
            let std_full_c = std_full.clone();
            let std_val_c = std_val.clone();
            let std_ts_c = std_ts.clone();
            let std_tx_c = std_tx.clone();

            let elite_val_c = elite_val.clone();
            let elite_cap_c = elite_cap.clone();
            let elite_wp_c = elite_wp.clone();
            let elite_ch_c = elite_ch.clone();
            let elite_tx_c = elite_tx.clone();

            // Per-worker clones of granular counters for closure capture
            let std_tx_omega = std_tx_omega.clone();
            let std_tx_invalid_addr = std_tx_invalid_addr.clone();
            let std_tx_sig = std_tx_sig.clone();
            let std_tx_insuf = std_tx_insuf.clone();
            let std_tx_homo = std_tx_homo.clone();
            let std_tx_rate = std_tx_rate.clone();
            let std_tx_anom = std_tx_anom.clone();
            let std_tx_ser = std_tx_ser.clone();
            let std_tx_ts_tx = std_tx_ts_tx.clone();
            let std_tx_wallet = std_tx_wallet.clone();
            let std_tx_meta = std_tx_meta.clone();
            let std_tx_pq = std_tx_pq.clone();
            let std_tx_gas = std_tx_gas.clone();
            let std_is_utxo_nf = std_is_utxo_nf.clone();
            let std_is_dup_in = std_is_dup_in.clone();
            let std_is_io_mm = std_is_io_mm.clone();
            let std_is_sender_mm = std_is_sender_mm.clone();
            let std_is_cb_fee = std_is_cb_fee.clone();
            let std_is_cb_out0 = std_is_cb_out0.clone();
            let std_is_empty_id = std_is_empty_id.clone();
            let std_is_empty_io = std_is_empty_io.clone();
            let std_is_fee_req = std_is_fee_req.clone();
            let std_is_out0 = std_is_out0.clone();
            let std_tx_other_c = std_tx_other_c.clone();

            let elite_tx_omega = elite_tx_omega.clone();
            let elite_tx_invalid_addr = elite_tx_invalid_addr.clone();
            let elite_tx_sig = elite_tx_sig.clone();
            let elite_tx_insuf = elite_tx_insuf.clone();
            let elite_tx_homo = elite_tx_homo.clone();
            let elite_tx_rate = elite_tx_rate.clone();
            let elite_tx_anom = elite_tx_anom.clone();
            let elite_tx_ser = elite_tx_ser.clone();
            let elite_tx_ts_tx = elite_tx_ts_tx.clone();
            let elite_tx_wallet = elite_tx_wallet.clone();
            let elite_tx_meta = elite_tx_meta.clone();
            let elite_tx_pq = elite_tx_pq.clone();
            let elite_tx_gas = elite_tx_gas.clone();
            let elite_is_utxo_nf = elite_is_utxo_nf.clone();
            let elite_is_dup_in = elite_is_dup_in.clone();
            let elite_is_io_mm = elite_is_io_mm.clone();
            let elite_is_sender_mm = elite_is_sender_mm.clone();
            let elite_is_cb_fee = elite_is_cb_fee.clone();
            let elite_is_cb_out0 = elite_is_cb_out0.clone();
            let elite_is_empty_id = elite_is_empty_id.clone();
            let elite_is_empty_io = elite_is_empty_io.clone();
            let elite_is_fee_req = elite_is_fee_req.clone();
            let elite_is_out0 = elite_is_out0.clone();
            let elite_tx_other_c = elite_tx_other_c.clone();

            let handle = tokio::spawn(async move {
                let mut tx_count: u64 = 0;
                loop {
                    // Select a UTXO from this worker's local set
                    let maybe_selection: Option<(String, UTXO)> = {
                        let guard = local_utxos.read().await;
                        if guard.is_empty() {
                            None
                        } else {
                            let keys: Vec<String> = guard.keys().cloned().collect();
                            let idx = (tx_count as usize) % keys.len();
                            let key = keys[idx].clone();
                            let utxo = guard.get(&key).expect("key exists").clone();
                            Some((key, utxo))
                        }
                    };

                    if maybe_selection.is_none() {
                        tokio::task::yield_now().await;
                        sleep(target_interval).await;
                        continue;
                    }
                    let (selected_key, selected_utxo) = maybe_selection.unwrap();

                    // Create transaction from selected local UTXO
                    let tx = Self::create_valid_transaction_from_utxo(&selected_utxo, tx_count);

                    // Provide a read view of local UTXOs for verification
                    let local_view = local_utxos.read().await;

                    // Route to standard or elite mempool
                    if worker_id.is_multiple_of(2) || elite_clone.is_none() {
                        let mempool_guard = mempool_clone.write().await;
                        match mempool_guard
                            .add_transaction(tx.clone(), &local_view, &dag_clone)
                            .await
                        {
                            Ok(_) => {
                                tx_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                // Consume spent UTXO and add change
                                drop(local_view);
                                let mut w = local_utxos.write().await;
                                w.remove(&selected_key);
                                let change_utxo = tx.generate_utxo(1);
                                let change_key =
                                    format!("{}_{}", change_utxo.tx_id, change_utxo.output_index);
                                w.insert(change_key, change_utxo);
                            }
                            Err(e) => {
                                err_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                match e {
                                    MempoolError::BackpressureActive(_) => {
                                        std_bp_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    MempoolError::MempoolFull => {
                                        std_full_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    MempoolError::TimestampError => {
                                        std_ts_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    MempoolError::TransactionValidation(_) => {
                                        std_val_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    MempoolError::Tx(te) => {
                                        std_tx_c.fetch_add(1, Ordering::Relaxed);
                                        match te {
                                            TransactionError::OmegaRejection => {
                                                std_tx_omega.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InvalidAddress => {
                                                std_tx_invalid_addr.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::QuantumSignatureVerification => {
                                                std_tx_sig.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InsufficientFunds => {
                                                std_tx_insuf.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InvalidStructure(msg) => {
                                                if Self::is_utxo_not_found(&msg) {
                                                    std_is_utxo_nf.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_duplicate_input(&msg) {
                                                    std_is_dup_in.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_input_output_mismatch(&msg) {
                                                    std_is_io_mm.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_sender_mismatch(&msg) {
                                                    std_is_sender_mm
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_coinbase_fee_nonzero(&msg) {
                                                    std_is_cb_fee.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_coinbase_output_zero(&msg) {
                                                    std_is_cb_out0.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_empty_tx_id(&msg) {
                                                    std_is_empty_id.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_empty_in_out(&msg) {
                                                    std_is_empty_io.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_fee_required(&msg) {
                                                    std_is_fee_req.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_output_zero(&msg) {
                                                    std_is_out0.fetch_add(1, Ordering::Relaxed);
                                                } else {
                                                    std_tx_other_c.fetch_add(1, Ordering::Relaxed);
                                                }
                                            }
                                            TransactionError::HomomorphicError(_) => {
                                                std_tx_homo.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::RateLimitExceeded => {
                                                std_tx_rate.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::AnomalyDetected(_) => {
                                                std_tx_anom.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::Serialization(_) => {
                                                std_tx_ser.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::TimestampError => {
                                                std_tx_ts_tx.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::Wallet(_) => {
                                                std_tx_wallet.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InvalidMetadata(_) => {
                                                std_tx_meta.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::PqCrypto(_) => {
                                                std_tx_pq.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::GasFee(_) => {
                                                std_tx_gas.fetch_add(1, Ordering::Relaxed);
                                            }
                                        }
                                    }
                                }
                                // On failure, do not consume the UTXO; leave it for retry
                                drop(local_view);
                            }
                        }
                    } else if let Some(elite) = elite_clone.as_ref() {
                        match elite
                            .add_transaction(tx.clone(), &local_view, &dag_clone)
                            .await
                        {
                            Ok(_) => {
                                tx_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                // Consume spent UTXO and add change
                                drop(local_view);
                                let mut w = local_utxos.write().await;
                                w.remove(&selected_key);
                                let change_utxo = tx.generate_utxo(1);
                                let change_key =
                                    format!("{}_{}", change_utxo.tx_id, change_utxo.output_index);
                                w.insert(change_key, change_utxo);
                            }
                            Err(e) => {
                                err_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                match e {
                                    EliteMempoolError::ValidationFailed(_) => {
                                        elite_val_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    EliteMempoolError::CapacityExceeded => {
                                        elite_cap_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    EliteMempoolError::WorkerPoolError(_) => {
                                        elite_wp_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    EliteMempoolError::ChannelError(_) => {
                                        elite_ch_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    EliteMempoolError::TransactionError(te) => {
                                        elite_tx_c
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        match te {
                                            TransactionError::OmegaRejection => {
                                                elite_tx_omega.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InvalidAddress => {
                                                elite_tx_invalid_addr
                                                    .fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::QuantumSignatureVerification => {
                                                elite_tx_sig.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InsufficientFunds => {
                                                elite_tx_insuf.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InvalidStructure(msg) => {
                                                if Self::is_utxo_not_found(&msg) {
                                                    elite_is_utxo_nf
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_duplicate_input(&msg) {
                                                    elite_is_dup_in.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_input_output_mismatch(&msg) {
                                                    elite_is_io_mm.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_sender_mismatch(&msg) {
                                                    elite_is_sender_mm
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_coinbase_fee_nonzero(&msg) {
                                                    elite_is_cb_fee.fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_coinbase_output_zero(&msg) {
                                                    elite_is_cb_out0
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_empty_tx_id(&msg) {
                                                    elite_is_empty_id
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_empty_in_out(&msg) {
                                                    elite_is_empty_io
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_fee_required(&msg) {
                                                    elite_is_fee_req
                                                        .fetch_add(1, Ordering::Relaxed);
                                                } else if Self::is_output_zero(&msg) {
                                                    elite_is_out0.fetch_add(1, Ordering::Relaxed);
                                                } else {
                                                    elite_tx_other_c
                                                        .fetch_add(1, Ordering::Relaxed);
                                                }
                                            }
                                            TransactionError::HomomorphicError(_) => {
                                                elite_tx_homo.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::RateLimitExceeded => {
                                                elite_tx_rate.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::AnomalyDetected(_) => {
                                                elite_tx_anom.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::Serialization(_) => {
                                                elite_tx_ser.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::TimestampError => {
                                                elite_tx_ts_tx.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::Wallet(_) => {
                                                elite_tx_wallet.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::InvalidMetadata(_) => {
                                                elite_tx_meta.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::PqCrypto(_) => {
                                                elite_tx_pq.fetch_add(1, Ordering::Relaxed);
                                            }
                                            TransactionError::GasFee(_) => {
                                                elite_tx_gas.fetch_add(1, Ordering::Relaxed);
                                            }
                                        }
                                    }
                                }
                                // On failure, do not consume the UTXO; leave it for retry
                                drop(local_view);
                            }
                        }
                    }

                    tx_count = tx_count.wrapping_add(1);
                    sleep(target_interval).await;
                }
            });

            handles.push(handle);
        }

        Ok(handles)
    }

    /// Start metrics collection task
    async fn start_metrics_collection(&mut self) -> tokio::task::JoinHandle<()> {
        let _mempool = self.mempool.clone();
        let _elite_mempool = self.elite_mempool.clone();
        let counter = self.transaction_counter.clone();
        let error_counter = self.error_counter.clone();

        // Clone rejection counters and metrics history for use in the task
        let std_bp = self.std_reject_backpressure.clone();
        let std_full = self.std_reject_full.clone();
        let std_val = self.std_reject_validation.clone();
        let std_ts = self.std_reject_timestamp.clone();
        let std_tx = self.std_reject_tx.clone();
        let elite_val = self.elite_reject_validation.clone();
        let elite_cap = self.elite_reject_capacity.clone();
        let elite_wp = self.elite_reject_worker_pool.clone();
        let elite_ch = self.elite_reject_channel.clone();
        let elite_tx = self.elite_reject_tx.clone();
        let metrics_history = self.metrics_history.clone();

        // Clone granular counters for metrics task
        let std_tx_omega = self.std_tx_omega_rejection.clone();
        let std_tx_invalid_addr = self.std_tx_invalid_address.clone();
        let std_tx_sig = self.std_tx_signature_verification.clone();
        let std_tx_insuf = self.std_tx_insufficient_funds.clone();
        let std_tx_homo = self.std_tx_homomorphic_error.clone();
        let std_tx_rate = self.std_tx_rate_limit.clone();
        let std_tx_anom = self.std_tx_anomaly_detected.clone();
        let std_tx_ser = self.std_tx_serialization_error.clone();
        let std_tx_ts_tx = self.std_tx_timestamp_error_via_tx.clone();
        let std_tx_wallet = self.std_tx_wallet_error.clone();
        let std_tx_meta = self.std_tx_invalid_metadata.clone();
        let std_tx_pq = self.std_tx_pqcrypto_error.clone();
        let std_tx_gas = self.std_tx_gas_fee_error.clone();
        let std_is_utxo_nf = self.std_tx_is_utxo_not_found.clone();
        let std_is_dup_in = self.std_tx_is_duplicate_input.clone();
        let std_is_io_mm = self.std_tx_is_input_output_mismatch.clone();
        let std_is_sender_mm = self.std_tx_is_sender_mismatch.clone();
        let std_is_cb_fee = self.std_tx_is_coinbase_fee_nonzero.clone();
        let std_is_cb_out0 = self.std_tx_is_coinbase_output_zero.clone();
        let std_is_empty_id = self.std_tx_is_empty_tx_id.clone();
        let std_is_empty_io = self.std_tx_is_empty_in_out.clone();
        let std_is_fee_req = self.std_tx_is_fee_required.clone();
        let std_is_out0 = self.std_tx_is_output_zero.clone();
        let std_tx_other_c = self.std_tx_other.clone();

        let elite_tx_omega = self.elite_tx_omega_rejection.clone();
        let elite_tx_invalid_addr = self.elite_tx_invalid_address.clone();
        let elite_tx_sig = self.elite_tx_signature_verification.clone();
        let elite_tx_insuf = self.elite_tx_insufficient_funds.clone();
        let elite_tx_homo = self.elite_tx_homomorphic_error.clone();
        let elite_tx_rate = self.elite_tx_rate_limit.clone();
        let elite_tx_anom = self.elite_tx_anomaly_detected.clone();
        let elite_tx_ser = self.elite_tx_serialization_error.clone();
        let elite_tx_ts_tx = self.elite_tx_timestamp_error_via_tx.clone();
        let elite_tx_wallet = self.elite_tx_wallet_error.clone();
        let elite_tx_meta = self.elite_tx_invalid_metadata.clone();
        let elite_tx_pq = self.elite_tx_pqcrypto_error.clone();
        let elite_tx_gas = self.elite_tx_gas_fee_error.clone();
        let elite_is_utxo_nf = self.elite_tx_is_utxo_not_found.clone();
        let elite_is_dup_in = self.elite_tx_is_duplicate_input.clone();
        let elite_is_io_mm = self.elite_tx_is_input_output_mismatch.clone();
        let elite_is_sender_mm = self.elite_tx_is_sender_mismatch.clone();
        let elite_is_cb_fee = self.elite_tx_is_coinbase_fee_nonzero.clone();
        let elite_is_cb_out0 = self.elite_tx_is_coinbase_output_zero.clone();
        let elite_is_empty_id = self.elite_tx_is_empty_tx_id.clone();
        let elite_is_empty_io = self.elite_tx_is_empty_in_out.clone();
        let elite_is_fee_req = self.elite_tx_is_fee_required.clone();
        let elite_is_out0 = self.elite_tx_is_output_zero.clone();
        let elite_tx_other_c = self.elite_tx_other.clone();

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
                let elite_metrics = if let Some(ref elite) = _elite_mempool {
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

                // Build rejection breakdowns from counters
                let mut std_breakdown = HashMap::new();
                std_breakdown.insert("backpressure".to_string(), std_bp.load(Ordering::Relaxed));
                std_breakdown.insert("mempool_full".to_string(), std_full.load(Ordering::Relaxed));
                std_breakdown.insert(
                    "validation_failed".to_string(),
                    std_val.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "timestamp_error".to_string(),
                    std_ts.load(Ordering::Relaxed),
                );
                std_breakdown.insert("tx_error".to_string(), std_tx.load(Ordering::Relaxed));
                // Granular TX errors (standard)
                std_breakdown.insert(
                    "tx_omega_rejection".to_string(),
                    std_tx_omega.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_invalid_address".to_string(),
                    std_tx_invalid_addr.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_signature_verification".to_string(),
                    std_tx_sig.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_insufficient_funds".to_string(),
                    std_tx_insuf.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_homomorphic_error".to_string(),
                    std_tx_homo.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_rate_limit".to_string(),
                    std_tx_rate.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_anomaly_detected".to_string(),
                    std_tx_anom.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_serialization_error".to_string(),
                    std_tx_ser.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_timestamp_error".to_string(),
                    std_tx_ts_tx.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_wallet_error".to_string(),
                    std_tx_wallet.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_invalid_metadata".to_string(),
                    std_tx_meta.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_pqcrypto_error".to_string(),
                    std_tx_pq.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_gas_fee_error".to_string(),
                    std_tx_gas.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "tx_other".to_string(),
                    std_tx_other_c.load(Ordering::Relaxed),
                );
                // Granular InvalidStructure (standard)
                std_breakdown.insert(
                    "is_utxo_not_found".to_string(),
                    std_is_utxo_nf.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_duplicate_input".to_string(),
                    std_is_dup_in.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_input_output_mismatch".to_string(),
                    std_is_io_mm.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_sender_mismatch".to_string(),
                    std_is_sender_mm.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_coinbase_fee_nonzero".to_string(),
                    std_is_cb_fee.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_coinbase_output_zero".to_string(),
                    std_is_cb_out0.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_empty_tx_id".to_string(),
                    std_is_empty_id.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_empty_in_out".to_string(),
                    std_is_empty_io.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_fee_required".to_string(),
                    std_is_fee_req.load(Ordering::Relaxed),
                );
                std_breakdown.insert(
                    "is_output_zero".to_string(),
                    std_is_out0.load(Ordering::Relaxed),
                );

                let mut elite_breakdown = HashMap::new();
                elite_breakdown.insert(
                    "validation_failed".to_string(),
                    elite_val.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "capacity_exceeded".to_string(),
                    elite_cap.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "worker_pool_error".to_string(),
                    elite_wp.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "channel_error".to_string(),
                    elite_ch.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "transaction_error".to_string(),
                    elite_tx.load(Ordering::Relaxed),
                );
                // Granular TX errors (elite)
                elite_breakdown.insert(
                    "tx_omega_rejection".to_string(),
                    elite_tx_omega.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_invalid_address".to_string(),
                    elite_tx_invalid_addr.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_signature_verification".to_string(),
                    elite_tx_sig.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_insufficient_funds".to_string(),
                    elite_tx_insuf.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_homomorphic_error".to_string(),
                    elite_tx_homo.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_rate_limit".to_string(),
                    elite_tx_rate.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_anomaly_detected".to_string(),
                    elite_tx_anom.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_serialization_error".to_string(),
                    elite_tx_ser.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_timestamp_error".to_string(),
                    elite_tx_ts_tx.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_wallet_error".to_string(),
                    elite_tx_wallet.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_invalid_metadata".to_string(),
                    elite_tx_meta.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_pqcrypto_error".to_string(),
                    elite_tx_pq.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_gas_fee_error".to_string(),
                    elite_tx_gas.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "tx_other".to_string(),
                    elite_tx_other_c.load(Ordering::Relaxed),
                );
                // Granular InvalidStructure (elite)
                elite_breakdown.insert(
                    "is_utxo_not_found".to_string(),
                    elite_is_utxo_nf.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_duplicate_input".to_string(),
                    elite_is_dup_in.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_input_output_mismatch".to_string(),
                    elite_is_io_mm.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_sender_mismatch".to_string(),
                    elite_is_sender_mm.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_coinbase_fee_nonzero".to_string(),
                    elite_is_cb_fee.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_coinbase_output_zero".to_string(),
                    elite_is_cb_out0.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_empty_tx_id".to_string(),
                    elite_is_empty_id.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_empty_in_out".to_string(),
                    elite_is_empty_io.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_fee_required".to_string(),
                    elite_is_fee_req.load(Ordering::Relaxed),
                );
                elite_breakdown.insert(
                    "is_output_zero".to_string(),
                    elite_is_out0.load(Ordering::Relaxed),
                );

                let metrics = PerformanceMetrics {
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    transactions_per_second: tps,
                    total_transactions: current_count,
                    memory_usage_mb: Self::get_memory_usage(),
                    cpu_usage_percent: Self::get_cpu_usage(),
                    disk_io_read_mb: 0.0,
                    disk_io_write_mb: 0.0,
                    network_rx_mb: 0.0,
                    network_tx_mb: 0.0,
                    error_count: errors,
                    success_rate,
                    elite_mempool_tps: elite_tps,
                    elite_mempool_processed: elite_processed,
                    elite_mempool_validated: elite_validated,
                    elite_mempool_rejected: elite_rejected,
                    elite_mempool_avg_latency_ns: elite_latency,
                    std_rejection_breakdown: Some(std_breakdown),
                    elite_rejection_breakdown: Some(elite_breakdown),
                };

                info!(
                    "TPS: {:.2}, Total: {}, Errors: {}, Success Rate: {:.2}%",
                    tps, current_count, errors, success_rate
                );

                // Append metrics to history
                {
                    let mut history = metrics_history.write().await;
                    history.push(metrics);
                }

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

        // Final rejection breakdowns
        let mut std_breakdown = HashMap::new();
        std_breakdown.insert(
            "backpressure".to_string(),
            self.std_reject_backpressure.load(Ordering::Relaxed),
        );
        std_breakdown.insert(
            "mempool_full".to_string(),
            self.std_reject_full.load(Ordering::Relaxed),
        );
        std_breakdown.insert(
            "validation_failed".to_string(),
            self.std_reject_validation.load(Ordering::Relaxed),
        );
        std_breakdown.insert(
            "timestamp_error".to_string(),
            self.std_reject_timestamp.load(Ordering::Relaxed),
        );
        std_breakdown.insert(
            "tx_error".to_string(),
            self.std_reject_tx.load(Ordering::Relaxed),
        );
        std_breakdown.insert(
            "is_output_zero".to_string(),
            self.std_tx_is_output_zero.load(Ordering::Relaxed),
        );

        let mut elite_breakdown = HashMap::new();
        elite_breakdown.insert(
            "validation_failed".to_string(),
            self.elite_reject_validation.load(Ordering::Relaxed),
        );
        elite_breakdown.insert(
            "capacity_exceeded".to_string(),
            self.elite_reject_capacity.load(Ordering::Relaxed),
        );
        elite_breakdown.insert(
            "worker_pool_error".to_string(),
            self.elite_reject_worker_pool.load(Ordering::Relaxed),
        );
        elite_breakdown.insert(
            "channel_error".to_string(),
            self.elite_reject_channel.load(Ordering::Relaxed),
        );
        elite_breakdown.insert(
            "transaction_error".to_string(),
            self.elite_reject_tx.load(Ordering::Relaxed),
        );

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
            std_rejection_breakdown: Some(std_breakdown),
            elite_rejection_breakdown: Some(elite_breakdown),
        }
    }

    /// Get current process memory usage in MB
    fn get_memory_usage() -> f64 {
        if let Ok(pid) = get_current_pid() {
            let sys_lock = PROCESS_SYS.get_or_init(|| Mutex::new(System::new()));
            let mut sys = sys_lock.lock().expect("failed to lock system");
            sys.refresh_processes_specifics(ProcessRefreshKind::new().with_memory());
            if let Some(proc_) = sys.process(pid) {
                return (proc_.memory() as f64) / (1024.0 * 1024.0);
            }
        }
        0.0
    }

    /// Get current process CPU usage percentage
    fn get_cpu_usage() -> f64 {
        if let Ok(pid) = get_current_pid() {
            let sys_lock = PROCESS_SYS.get_or_init(|| Mutex::new(System::new()));
            let mut sys = sys_lock.lock().expect("failed to lock system");
            sys.refresh_processes_specifics(ProcessRefreshKind::new().with_cpu());
            if let Some(proc_) = sys.process(pid) {
                return proc_.cpu_usage() as f64;
            }
        }
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

    // Allow simple CLI overrides for workers and duration
    let mut config = BenchmarkConfig::default();
    {
        let args: Vec<String> = std::env::args().collect();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--num-workers" => {
                    if let Some(v) = args.get(i + 1) {
                        if let Ok(n) = v.parse::<usize>() {
                            config.num_workers = n;
                        }
                        i += 1;
                    }
                }
                "--duration-secs" => {
                    if let Some(v) = args.get(i + 1) {
                        if let Ok(n) = v.parse::<u64>() {
                            config.duration_secs = n;
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

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
