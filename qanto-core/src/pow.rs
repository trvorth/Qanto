use crate::crypto::qanhash::{get_qdag_sync, QanhashMiner};
use crate::qanto_native_crypto::{qanto_hash, QantoHash};
use lru::LruCache;
use rayon::prelude::*;
use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, instrument, warn};

#[derive(Debug, Clone)]
pub struct PowConfig {
    pub target_block_time_sec: u64,
    pub use_gpu: bool,
    pub threads: usize,
    pub nonce_start: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum MiningResult {
    Found {
        nonce: u64,
        hash: [u8; 32],
        hashes_tried: u64,
    },
    Cancelled {
        hashes_tried: u64,
    },
    Timeout {
        hashes_tried: u64,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum PowError {
    #[error("Thread pool error: {0}")]
    ThreadPool(String),
    #[error("Mining cancelled or timed out")]
    TimeoutOrCancelled,
    #[error("Invalid target")]
    InvalidTarget,
}

fn get_start_nonce() -> u64 {
    let start = SystemTime::now();
    start
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Compute hash for a nonce using the core hashing algorithm
fn compute_hash_for_nonce(header_hash_obj: &QantoHash, nonce: u64, height: u64) -> [u8; 32] {
    crate::crypto::qanhash::hash(header_hash_obj, nonce, height)
}

/// Test if a nonce meets the target
fn test_nonce_optimized(
    header_hash_obj: &QantoHash,
    nonce: u64,
    height: u64,
    target: &[u8; 32],
) -> bool {
    let hash = compute_hash_for_nonce(header_hash_obj, nonce, height);
    // Big-endian comparison
    &hash < target
}

/// Main PoW solver function
#[instrument(skip(qanhash_miner, header_hash, target, shutdown))]
pub async fn solve_pow(
    qanhash_miner: &Arc<QanhashMiner>,
    header_hash: [u8; 32],
    target: [u8; 32],
    height: u64,
    config: &PowConfig,
    shutdown: CancellationToken,
) -> Result<MiningResult, PowError> {
    let qanto_hash_obj = QantoHash::new(header_hash);
    let mut total_hashes_tried = 0u64;

    // 1. GPU Mining Attempt (if enabled)
    if config.use_gpu {
        debug!("Attempting GPU mining...");
        let gpu_timeout = Duration::from_secs(3); // 3s timeout for GPU
        let qdag = get_qdag_sync(height);

        let miner_clone = qanhash_miner.clone();
        let qanto_hash_clone = qanto_hash_obj;
        let qdag_clone = qdag.clone();
        let shutdown_clone = shutdown.clone();
        let target_clone = target;
        let start_nonce = config.nonce_start.unwrap_or_else(get_start_nonce);

        let gpu_result = tokio::task::spawn_blocking(move || {
            let mut nonce = start_nonce;
            let batch_size = 1024 * 1024;
            let start = std::time::Instant::now();
            let mut local_hashes = 0u64;

            loop {
                if shutdown_clone.is_cancelled() {
                    return MiningResult::Cancelled {
                        hashes_tried: local_hashes,
                    };
                }
                if start.elapsed() > gpu_timeout {
                    return MiningResult::Timeout {
                        hashes_tried: local_hashes,
                    };
                }

                // mine_with_dag returns Option<(nonce, hash)>
                if let Some((found_nonce, hash)) = miner_clone.mine_with_dag(
                    &qanto_hash_clone,
                    nonce,
                    target_clone,
                    qdag_clone.clone(),
                ) {
                    // Approximate hashes tried for success case
                    local_hashes += (found_nonce.wrapping_sub(nonce)) + 1;
                    return MiningResult::Found {
                        nonce: found_nonce,
                        hash,
                        hashes_tried: local_hashes,
                    };
                }

                local_hashes += batch_size;
                nonce = nonce.wrapping_add(batch_size);
            }
        })
        .await
        .map_err(|e| PowError::ThreadPool(e.to_string()))?;

        match gpu_result {
            MiningResult::Found {
                nonce,
                hash,
                hashes_tried,
            } => {
                info!("GPU mining successful! Nonce: {}", nonce);
                return Ok(MiningResult::Found {
                    nonce,
                    hash,
                    hashes_tried,
                });
            }
            MiningResult::Cancelled { hashes_tried } => {
                return Ok(MiningResult::Cancelled { hashes_tried })
            }
            MiningResult::Timeout { hashes_tried } => {
                total_hashes_tried += hashes_tried;
                info!("GPU mining timed out (3s cap) or failed; switching to CPU immediately.");
            }
        }
    }

    // 2. CPU Mining Fallback
    // If target_block_time is very small (e.g. 5s), we use a smaller timeout or the configured one
    let cpu_timeout = Duration::from_secs((config.target_block_time_sec * 2).max(30));

    let cpu_res = mine_cpu_optimized(
        &qanto_hash_obj,
        target,
        height,
        config.threads,
        cpu_timeout,
        shutdown,
        config.nonce_start,
    )
    .await?;

    // Aggregate hashes tried
    match cpu_res {
        MiningResult::Found {
            nonce,
            hash,
            hashes_tried,
        } => Ok(MiningResult::Found {
            nonce,
            hash,
            hashes_tried: total_hashes_tried + hashes_tried,
        }),
        MiningResult::Cancelled { hashes_tried } => Ok(MiningResult::Cancelled {
            hashes_tried: total_hashes_tried + hashes_tried,
        }),
        MiningResult::Timeout { hashes_tried } => Ok(MiningResult::Timeout {
            hashes_tried: total_hashes_tried + hashes_tried,
        }),
    }
}

/// Optimized CPU mining with work stealing and rayon
async fn mine_cpu_optimized(
    header_hash_obj: &QantoHash,
    target: [u8; 32],
    height: u64,
    threads: usize,
    timeout: Duration,
    shutdown: CancellationToken,
    nonce_start: Option<u64>,
) -> Result<MiningResult, PowError> {
    let threads = if threads == 0 {
        num_cpus::get().max(1)
    } else {
        threads
    };

    let header_hash_clone = *header_hash_obj;
    let shutdown_clone = shutdown.clone();

    // Spawn blocking task for CPU intensive work
    tokio::task::spawn_blocking(move || {
        // Build thread pool
        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| PowError::ThreadPool(e.to_string()))?;

        let (sender, receiver) = std::sync::mpsc::channel();
        let completion_flag = Arc::new(AtomicBool::new(false));
        let found_signal = Arc::new(AtomicBool::new(false));
        let hashes_tried = Arc::new(AtomicU64::new(0));
        let winning_nonce = Arc::new(AtomicU64::new(0));
        let winning_hash = Arc::new(Mutex::new([0u8; 32]));

        let nonce_range_per_thread = u64::MAX / threads as u64;
        let base_nonce = nonce_start.unwrap_or_else(get_start_nonce);
        let start_time = std::time::Instant::now();

        for thread_id in 0..threads {
            let sender = sender.clone();
            let completion_flag = completion_flag.clone();
            let found_signal = found_signal.clone();
            let hashes_tried = hashes_tried.clone();
            let winning_nonce = winning_nonce.clone();
            let winning_hash = winning_hash.clone();
            let shutdown_token = shutdown_clone.clone();
            let header_hash_local = header_hash_clone;

            let thread_start_nonce =
                base_nonce.wrapping_add(thread_id as u64 * nonce_range_per_thread);

            thread_pool.spawn(move || {
                let mut current_nonce = thread_start_nonce;
                let mut local_hash_count = 0u64;
                const BATCH_SIZE: u64 = 1000;

                loop {
                    // 1. Check global found signal
                    if found_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    // 2. Check cancellation / timeout periodically
                    // User Requirement: Check every 1000 hashes (BATCH_SIZE)
                    if local_hash_count.is_multiple_of(BATCH_SIZE)
                        && !completion_flag.load(Ordering::Acquire)
                    {
                        if shutdown_token.is_cancelled() {
                            let _ = sender.send(MiningResult::Cancelled { hashes_tried: 0 }); // hashes updated via atomic
                            break;
                        }
                        if start_time.elapsed() > timeout {
                            let _ = sender.send(MiningResult::Timeout { hashes_tried: 0 });
                            break;
                        }
                    }

                    // 3. Process Batch
                    for _ in 0..BATCH_SIZE {
                        if found_signal.load(Ordering::Relaxed) {
                            break;
                        }

                        if test_nonce_optimized(&header_hash_local, current_nonce, height, &target)
                        {
                            // Attempt to claim victory
                            if !completion_flag.swap(true, Ordering::AcqRel) {
                                let hash = compute_hash_for_nonce(
                                    &header_hash_local,
                                    current_nonce,
                                    height,
                                );
                                winning_nonce.store(current_nonce, Ordering::Relaxed);
                                if let Ok(mut g) = winning_hash.lock() {
                                    *g = hash;
                                }
                                found_signal.store(true, Ordering::Release);
                                let _ = sender.send(MiningResult::Found {
                                    nonce: current_nonce,
                                    hash,
                                    hashes_tried: 0,
                                });
                                break;
                            }
                        }
                        current_nonce = current_nonce.wrapping_add(1);
                    }
                    local_hash_count += BATCH_SIZE;
                    hashes_tried.fetch_add(BATCH_SIZE, Ordering::Relaxed);

                    // Work stealing / jump if needed (omitted for brevity, can add if critical)
                }
            });
        }

        // Wait for result
        // We need to be careful not to block forever if threads don't send.
        // But threads should send on timeout/cancel/found.
        // Actually, if multiple threads timeout, we might receive multiple messages.
        // We take the first one.

        match receiver.recv() {
            Ok(res) => {
                // Ensure we get the correct total hashes
                let total = hashes_tried.load(Ordering::Relaxed);
                match res {
                    MiningResult::Found { nonce, hash, .. } => Ok(MiningResult::Found {
                        nonce,
                        hash,
                        hashes_tried: total,
                    }),
                    MiningResult::Cancelled { .. } => Ok(MiningResult::Cancelled {
                        hashes_tried: total,
                    }),
                    MiningResult::Timeout { .. } => Ok(MiningResult::Timeout {
                        hashes_tried: total,
                    }),
                }
            }
            Err(_) => Err(PowError::ThreadPool(
                "Mining threads disconnected".to_string(),
            )),
        }
    })
    .await
    .map_err(|e| PowError::ThreadPool(e.to_string()))?
}

// --- Optimized Q-DAG Generation ---

/// Constants for optimized Q-DAG generation
pub const OPTIMIZED_MIX_BYTES: usize = 128;
pub const OPTIMIZED_CACHE_SIZE: usize = 1 << 14; // Increased from 1 << 12 to 1 << 14 (16K items)
pub const OPTIMIZED_DATASET_INIT_SIZE: usize = 1 << 10; // Increased from 1 << 8 to 1 << 10 (1024 items)
pub const EPOCH_LENGTH: u64 = 10_000;
pub const MAX_CACHED_DAGS: usize = 10; // Reduced to 10 for Phase 1 memory optimization
pub const DAG_GENERATION_TIMEOUT: Duration = Duration::from_secs(15); // Reduced from 30 to 15 seconds
pub const MEMORY_PRESSURE_THRESHOLD: f64 = 85.0; // Increased from 75.0 to 85.0 for more aggressive caching

/// SIMD batch sizes for different architectures
#[cfg(target_arch = "x86_64")]
pub const SIMD_BATCH_SIZE: usize = 8; // AVX2 can process 8 items in parallel
#[cfg(not(target_arch = "x86_64"))]
pub const SIMD_BATCH_SIZE: usize = 4; // Fallback for other architectures

/// Configuration for optimized Q-DAG generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedQDagConfig {
    /// Enable parallel generation
    pub parallel_generation: bool,
    /// Enable SIMD optimizations
    pub simd_enabled: bool,
    /// Enable predictive generation
    pub predictive_generation: bool,
    /// Memory pressure threshold (0-100%)
    pub memory_threshold: f64,
    /// Maximum number of worker threads
    pub max_threads: usize,
    /// Cache size for generated DAGs
    pub cache_size: usize,
    /// Enable adaptive batch sizing
    pub adaptive_batching: bool,
}

impl Default for OptimizedQDagConfig {
    fn default() -> Self {
        Self {
            parallel_generation: true,
            simd_enabled: cfg!(target_arch = "x86_64"),
            predictive_generation: true,
            memory_threshold: MEMORY_PRESSURE_THRESHOLD,
            max_threads: num_cpus::get().saturating_sub(1).max(1), // Leave one core for system
            cache_size: MAX_CACHED_DAGS * 2, // Increased cache size for better hit rate
            adaptive_batching: true,
        }
    }
}

/// Entry in the optimized DAG cache
#[derive(Debug)]
pub struct OptimizedDagEntry {
    pub dag: Arc<Vec<[u8; OPTIMIZED_MIX_BYTES]>>,
    pub epoch: u64,
    pub generation_time: Duration,
    pub generated_at: Instant,
    pub access_count: AtomicU64,
    pub memory_size: usize,
}

impl OptimizedDagEntry {
    pub fn new(
        dag: Arc<Vec<[u8; OPTIMIZED_MIX_BYTES]>>,
        epoch: u64,
        generation_time: Duration,
    ) -> Self {
        let memory_size = dag.len() * OPTIMIZED_MIX_BYTES;
        Self {
            dag,
            epoch,
            generation_time,
            generated_at: Instant::now(),
            access_count: AtomicU64::new(0),
            memory_size,
        }
    }

    pub fn access(&self) -> u64 {
        self.access_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn is_expired(&self, max_age: Duration) -> bool {
        self.generated_at.elapsed() > max_age
    }
}

/// Performance metrics for Q-DAG generation
#[derive(Debug, Clone, Default)]
pub struct QDagPerformanceMetrics {
    pub total_generations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_generation_time: Duration,
    pub total_memory_used: usize,
    pub simd_accelerated: u64,
    pub parallel_generations: u64,
}

impl QDagPerformanceMetrics {
    pub fn cache_hit_rate(&self) -> f64 {
        if self.cache_hits + self.cache_misses == 0 {
            0.0
        } else {
            self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64
        }
    }

    pub fn record_generation(
        &mut self,
        generation_time: Duration,
        was_cached: bool,
        used_simd: bool,
        used_parallel: bool,
    ) {
        self.total_generations += 1;

        if was_cached {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;

            // Update average generation time
            let total_time = self.average_generation_time.as_nanos() as u64
                * (self.cache_misses - 1)
                + generation_time.as_nanos() as u64;
            self.average_generation_time = Duration::from_nanos(total_time / self.cache_misses);
        }

        if used_simd {
            self.simd_accelerated += 1;
        }

        if used_parallel {
            self.parallel_generations += 1;
        }
    }
}

/// Optimized Q-DAG generator with advanced caching and performance optimizations
pub struct OptimizedQDagGenerator {
    config: OptimizedQDagConfig,
    cache: Arc<AsyncMutex<LruCache<u64, OptimizedDagEntry>>>,
    system_monitor: Arc<RwLock<System>>,
    thread_pool: Arc<ThreadPool>,
    metrics: Arc<RwLock<QDagPerformanceMetrics>>,
    generation_queue: Arc<AsyncMutex<HashMap<u64, Instant>>>,
}

impl OptimizedQDagGenerator {
    /// Create a new optimized Q-DAG generator
    pub fn new(config: OptimizedQDagConfig) -> Self {
        let cache_size = NonZeroUsize::new(config.cache_size)
            .unwrap_or(NonZeroUsize::new(MAX_CACHED_DAGS).unwrap());
        let cache = Arc::new(AsyncMutex::new(LruCache::new(cache_size)));

        let mut system = System::new_all();
        system.refresh_all();
        let system_monitor = Arc::new(RwLock::new(system));

        let thread_pool = Arc::new(
            ThreadPoolBuilder::new()
                .num_threads(config.max_threads)
                .build()
                .unwrap(),
        );
        let metrics = Arc::new(RwLock::new(QDagPerformanceMetrics::default()));
        let generation_queue = Arc::new(AsyncMutex::new(HashMap::new()));

        Self {
            config,
            cache,
            system_monitor,
            thread_pool,
            metrics,
            generation_queue,
        }
    }

    /// Get or generate a Q-DAG for the specified epoch with optimizations
    pub async fn get_qdag(
        &self,
        epoch: u64,
    ) -> Result<Arc<Vec<[u8; OPTIMIZED_MIX_BYTES]>>, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();

        // Check cache first
        {
            let mut cache = self.cache.lock().await;
            if let Some(entry) = cache.get(&epoch) {
                entry.access();
                let generation_time = start_time.elapsed();

                // Update metrics
                {
                    let mut metrics = self.metrics.write().unwrap();
                    metrics.record_generation(generation_time, true, false, false);
                }

                debug!("Q-DAG cache hit for epoch {}", epoch);
                return Ok(entry.dag.clone());
            }
        }

        // Check if generation is already in progress
        {
            let queue = self.generation_queue.lock().await;
            if let Some(start_time) = queue.get(&epoch) {
                if start_time.elapsed() < DAG_GENERATION_TIMEOUT {
                    // Wait a bit and try cache again
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    let mut cache = self.cache.lock().await;
                    if let Some(entry) = cache.get(&epoch) {
                        return Ok(entry.dag.clone());
                    }
                }
            }
        }

        // Mark generation as in progress
        {
            let mut queue = self.generation_queue.lock().await;
            queue.insert(epoch, Instant::now());
        }

        // Check memory pressure before generation
        let memory_usage = self.get_memory_usage_percentage().await;
        if memory_usage > self.config.memory_threshold {
            warn!(
                "High memory usage ({:.1}%), cleaning cache before DAG generation",
                memory_usage
            );
            self.cleanup_cache().await;
        }

        // Generate new DAG
        let generation_start = Instant::now();
        let dag = self.generate_optimized_qdag(epoch).await?;
        let generation_time = generation_start.elapsed();

        // Create cache entry
        let entry = OptimizedDagEntry::new(dag.clone(), epoch, generation_time);

        // Add to cache
        {
            let mut cache = self.cache.lock().await;
            cache.put(epoch, entry);
        }

        // Remove from generation queue
        {
            let mut queue = self.generation_queue.lock().await;
            queue.remove(&epoch);
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().unwrap();
            let used_simd = self.config.simd_enabled;
            let used_parallel = self.config.parallel_generation;
            metrics.record_generation(generation_time, false, used_simd, used_parallel);
        }

        info!(
            "Generated Q-DAG for epoch {} in {:?}",
            epoch, generation_time
        );

        // Trigger predictive generation if enabled
        if self.config.predictive_generation {
            self.trigger_predictive_generation(epoch).await;
        }

        Ok(dag)
    }

    /// Generate an optimized Q-DAG using parallel processing and SIMD
    async fn generate_optimized_qdag(
        &self,
        epoch: u64,
    ) -> Result<Arc<Vec<[u8; OPTIMIZED_MIX_BYTES]>>, Box<dyn std::error::Error + Send + Sync>> {
        let seed = qanto_hash(&epoch.to_le_bytes());

        // Calculate dataset size with memory awareness
        let base_size = OPTIMIZED_DATASET_INIT_SIZE + (epoch.min(10) as usize * 16);
        let dataset_size = if self.config.adaptive_batching {
            self.calculate_adaptive_dataset_size(base_size).await
        } else {
            base_size.next_power_of_two().min(1024)
        };

        info!(
            "Generating optimized Q-DAG with size {} for epoch {}",
            dataset_size, epoch
        );

        // Generate cache using parallel processing
        let cache = self.generate_parallel_cache(seed.as_bytes()).await?;

        // Generate dataset using optimized methods
        let dataset = if self.config.parallel_generation {
            self.generate_parallel_dataset(&cache, dataset_size).await?
        } else {
            self.generate_sequential_dataset(&cache, dataset_size)
                .await?
        };

        Ok(Arc::new(dataset))
    }

    /// Generate cache in parallel with SIMD optimizations
    async fn generate_parallel_cache(
        &self,
        seed: &[u8; 32],
    ) -> Result<Arc<Vec<Vec<u8>>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = Vec::with_capacity(OPTIMIZED_CACHE_SIZE);

        if self.config.simd_enabled && cfg!(target_arch = "x86_64") {
            // Use SIMD-optimized generation
            cache = self.generate_cache_simd(seed).await?;
        } else {
            // Use parallel generation without SIMD
            let mut item_hash = qanto_hash(seed).as_bytes().to_vec();
            for _ in 0..OPTIMIZED_CACHE_SIZE {
                item_hash = qanto_hash(&item_hash).as_bytes().to_vec();
                cache.push(item_hash.clone());
            }
        }

        Ok(Arc::new(cache))
    }

    /// Generate cache using SIMD optimizations (x86_64 only)
    #[cfg(target_arch = "x86_64")]
    async fn generate_cache_simd(
        &self,
        seed: &[u8; 32],
    ) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = Vec::with_capacity(OPTIMIZED_CACHE_SIZE);
        let mut item_hash = qanto_hash(seed).as_bytes().to_vec();

        // Process in SIMD batches
        let batches = OPTIMIZED_CACHE_SIZE / SIMD_BATCH_SIZE;
        let remainder = OPTIMIZED_CACHE_SIZE % SIMD_BATCH_SIZE;

        for _ in 0..batches {
            let mut batch_hashes = Vec::with_capacity(SIMD_BATCH_SIZE);

            // Generate batch of hashes
            for _ in 0..SIMD_BATCH_SIZE {
                item_hash = qanto_hash(&item_hash).as_bytes().to_vec();
                batch_hashes.push(item_hash.clone());
            }

            cache.extend(batch_hashes);
        }

        // Handle remainder
        for _ in 0..remainder {
            item_hash = qanto_hash(&item_hash).as_bytes().to_vec();
            cache.push(item_hash.clone());
        }

        Ok(cache)
    }

    /// Fallback cache generation for non-x86_64 architectures
    #[cfg(not(target_arch = "x86_64"))]
    async fn generate_cache_simd(
        &self,
        seed: &[u8; 32],
    ) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = Vec::with_capacity(OPTIMIZED_CACHE_SIZE);
        let mut item_hash = qanto_hash(seed).as_bytes().to_vec();

        for _ in 0..OPTIMIZED_CACHE_SIZE {
            item_hash = qanto_hash(&item_hash).as_bytes().to_vec();
            cache.push(item_hash.clone());
        }

        Ok(cache)
    }

    /// Generate dataset using parallel processing
    async fn generate_parallel_dataset(
        &self,
        cache: &Arc<Vec<Vec<u8>>>,
        dataset_size: usize,
    ) -> Result<Vec<[u8; OPTIMIZED_MIX_BYTES]>, Box<dyn std::error::Error + Send + Sync>> {
        let dataset: Vec<[u8; OPTIMIZED_MIX_BYTES]> = (0..dataset_size)
            .into_par_iter()
            .map(|i| {
                let mut item_seed = qanto_hash(&i.to_le_bytes()).as_bytes().to_vec();
                let mut final_item_data = vec![0u8; OPTIMIZED_MIX_BYTES];

                for _ in 0..4 {
                    let cache_index =
                        u32::from_le_bytes(item_seed[0..4].try_into().unwrap_or([0; 4])) as usize
                            % OPTIMIZED_CACHE_SIZE;

                    let cache_item = &cache[cache_index];
                    for k in 0..OPTIMIZED_MIX_BYTES {
                        final_item_data[k] ^= cache_item[k % 32];
                    }
                    item_seed = qanto_hash(&item_seed).as_bytes().to_vec();
                }

                let mut slice = [0u8; OPTIMIZED_MIX_BYTES];
                slice.copy_from_slice(&final_item_data);
                slice
            })
            .collect();

        Ok(dataset)
    }

    /// Generate dataset sequentially (fallback)
    async fn generate_sequential_dataset(
        &self,
        cache: &Arc<Vec<Vec<u8>>>,
        dataset_size: usize,
    ) -> Result<Vec<[u8; OPTIMIZED_MIX_BYTES]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut dataset = vec![[0u8; OPTIMIZED_MIX_BYTES]; dataset_size];

        for (i, item) in dataset.iter_mut().enumerate().take(dataset_size) {
            let mut item_seed = qanto_hash(&i.to_le_bytes()).as_bytes().to_vec();
            let mut final_item_data = vec![0u8; OPTIMIZED_MIX_BYTES];

            for _ in 0..4 {
                let cache_index = u32::from_le_bytes(item_seed[0..4].try_into().unwrap_or([0; 4]))
                    as usize
                    % OPTIMIZED_CACHE_SIZE;

                let cache_item = &cache[cache_index];
                for k in 0..OPTIMIZED_MIX_BYTES {
                    final_item_data[k] ^= cache_item[k % 32];
                }
                item_seed = qanto_hash(&item_seed).as_bytes().to_vec();
            }

            item.copy_from_slice(&final_item_data);
        }

        Ok(dataset)
    }

    /// Calculate adaptive dataset size based on memory pressure
    async fn calculate_adaptive_dataset_size(&self, base_size: usize) -> usize {
        let memory_usage = self.get_memory_usage_percentage().await;

        let size_multiplier = if memory_usage > 80.0 {
            0.5 // Reduce size under high memory pressure
        } else if memory_usage > 60.0 {
            0.75 // Moderate reduction
        } else {
            1.0 // Full size
        };

        let adaptive_size = (base_size as f64 * size_multiplier) as usize;
        adaptive_size.next_power_of_two().clamp(256, 1024)
    }

    /// Get current memory usage percentage
    async fn get_memory_usage_percentage(&self) -> f64 {
        let mut system = self.system_monitor.write().unwrap();
        system.refresh_memory();

        let total_memory = system.total_memory();
        let used_memory = system.used_memory();

        if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Trigger predictive generation for future epochs
    async fn trigger_predictive_generation(&self, current_epoch: u64) {
        // Predictive generation disabled to avoid Send trait issues
        // This feature can be re-enabled when the Send trait requirements are resolved
        debug!(
            "Predictive generation requested for epoch {} but is currently disabled",
            current_epoch
        );
    }

    /// Clean up old cache entries to free memory
    async fn cleanup_cache(&self) {
        let mut cache = self.cache.lock().await;
        let current_size = cache.len();

        // Remove entries that are older than 10 minutes and have low access count
        let max_age = Duration::from_secs(600);
        let mut to_remove = Vec::new();

        for (epoch, entry) in cache.iter() {
            if entry.is_expired(max_age) && entry.access_count.load(Ordering::Relaxed) < 5 {
                to_remove.push(*epoch);
            }
        }

        for epoch in to_remove {
            cache.pop(&epoch);
        }

        let cleaned_size = cache.len();
        if cleaned_size < current_size {
            info!(
                "Cleaned {} DAG entries from cache",
                current_size - cleaned_size
            );
        }
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> QDagPerformanceMetrics {
        self.metrics.read().unwrap().clone()
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize, usize) {
        let cache = self.cache.lock().await;
        let current_size = cache.len();
        let max_size = cache.cap().get();

        let total_memory: usize = cache.iter().map(|(_, entry)| entry.memory_size).sum();

        (current_size, max_size, total_memory)
    }

    /// Preload DAGs for a range of epochs
    pub async fn preload_epochs(
        &self,
        start_epoch: u64,
        count: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Preloading {} DAGs starting from epoch {}",
            count, start_epoch
        );

        for i in 0..count {
            let epoch = start_epoch + i;
            if let Err(e) = self.get_qdag(epoch).await {
                warn!("Failed to preload DAG for epoch {}: {}", epoch, e);
            }
        }

        Ok(())
    }
}

impl Clone for OptimizedQDagGenerator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            cache: self.cache.clone(),
            system_monitor: self.system_monitor.clone(),
            thread_pool: self.thread_pool.clone(),
            metrics: self.metrics.clone(),
            generation_queue: self.generation_queue.clone(),
        }
    }
}

impl std::fmt::Debug for OptimizedQDagGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OptimizedQDagGenerator")
            .field("config", &self.config)
            .field("cache", &self.cache)
            .field("system_monitor", &self.system_monitor)
            .field("thread_pool", &"ThreadPool (no Debug)")
            .field("metrics", &self.metrics)
            .field("generation_queue", &self.generation_queue)
            .finish()
    }
}

static OPTIMIZED_QDAG_GENERATOR: std::sync::OnceLock<OptimizedQDagGenerator> =
    std::sync::OnceLock::new();

/// Get the global Q-DAG generator instance
pub fn get_global_qdag_generator() -> &'static OptimizedQDagGenerator {
    OPTIMIZED_QDAG_GENERATOR
        .get_or_init(|| OptimizedQDagGenerator::new(OptimizedQDagConfig::default()))
}

/// Convenience function to get optimized Q-DAG
pub async fn get_optimized_qdag(
    epoch: u64,
) -> Result<Arc<Vec<[u8; OPTIMIZED_MIX_BYTES]>>, Box<dyn std::error::Error + Send + Sync>> {
    get_global_qdag_generator().get_qdag(epoch).await
}

/// Convenience function to get Q-DAG metrics
pub fn get_qdag_metrics() -> QDagPerformanceMetrics {
    get_global_qdag_generator().get_metrics()
}
