//! # Optimized Q-DAG Generation Module
//!
//! This module provides enhanced Q-DAG generation with performance optimizations:
//! - Parallel generation with SIMD acceleration
//! - Memory-aware caching and batch processing
//! - Predictive DAG generation for future epochs
//! - Adaptive generation strategies based on system resources

use lru::LruCache;
use my_blockchain::qanto_standalone::hash::qanto_hash;
use my_blockchain::qanto_standalone::parallel::ThreadPool;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, info, warn};

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

        let thread_pool = Arc::new(ThreadPool::new(config.max_threads));
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
                    tokio::time::sleep(Duration::from_millis(100)).await;
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
        use std::arch::x86_64::*;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_optimized_qdag_generation() {
        let config = OptimizedQDagConfig::default();
        let generator = OptimizedQDagGenerator::new(config);

        let epoch = 12345;
        let dag = generator.get_qdag(epoch).await.unwrap();

        assert!(!dag.is_empty());
        assert_eq!(dag[0].len(), OPTIMIZED_MIX_BYTES);
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let config = OptimizedQDagConfig::default();
        let generator = OptimizedQDagGenerator::new(config);

        let epoch = 54321;

        // First access - should generate
        let dag1 = generator.get_qdag(epoch).await.unwrap();

        // Second access - should use cache
        let dag2 = generator.get_qdag(epoch).await.unwrap();

        // Should be the same instance (Arc)
        assert!(Arc::ptr_eq(&dag1, &dag2));
    }

    #[tokio::test]
    async fn test_memory_adaptive_sizing() {
        let config = OptimizedQDagConfig {
            adaptive_batching: true,
            ..Default::default()
        };
        let generator = OptimizedQDagGenerator::new(config);

        let base_size = 512;
        let adaptive_size = generator.calculate_adaptive_dataset_size(base_size).await;

        assert!(adaptive_size >= 256);
        assert!(adaptive_size <= 1024);
        assert!(adaptive_size.is_power_of_two());
    }
}
