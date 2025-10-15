//! Unified metrics system for Qanto blockchain
//! This module provides a centralized metrics collection and reporting system
//! that eliminates duplication across the codebase.
//! v0.1.0
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;

/// Comprehensive performance metrics for the entire Qanto system
#[derive(Debug, Serialize, Deserialize)]
pub struct QantoMetrics {
    // Core blockchain metrics
    #[serde(with = "atomic_serde")]
    pub blocks_processed: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub transactions_processed: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub validation_time_ms: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub block_creation_time_ms: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub cache_hits: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub cache_misses: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub concurrent_validations: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub queue_depth: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub last_block_time: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub throughput_bps: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub memory_usage_mb: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub cpu_utilization: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub disk_io_ops: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub network_bytes_sent: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub network_bytes_received: AtomicU64,

    // Memory metrics
    #[serde(with = "atomic_serde")]
    pub qanto_rss_bytes: AtomicU64, // Resident Set Size in bytes

    // Performance optimization metrics
    pub simd_operations: AtomicU64,
    pub lock_free_operations: AtomicU64,
    pub pipeline_stages_completed: AtomicU64,
    pub prefetch_hits: AtomicU64,
    pub prefetch_misses: AtomicU64,
    pub batch_processing_time_ns: AtomicU64,
    pub memory_pool_allocations: AtomicU64,
    pub zero_copy_operations: AtomicU64,
    pub work_stealing_tasks: AtomicU64,
    pub parallel_signature_verifications: AtomicU64,
    pub utxo_cache_efficiency: AtomicU64,
    pub transaction_throughput_tps: AtomicU64,
    pub parallel_validations: AtomicU64,
    pub batch_processing_ops: AtomicU64,
    pub compression_ratio: AtomicU64, // stored as percentage * 100

    // Network and consensus metrics
    pub tps: AtomicU64,
    pub finality_ms: AtomicU64,
    pub validator_count: AtomicU64,
    pub shard_count: AtomicU64,
    pub cross_shard_tps: AtomicU64,
    pub storage_efficiency: AtomicU64, // Stored as u64 * 1000 for precision
    pub network_bandwidth: AtomicU64,
    pub consensus_latency: AtomicU64,
    pub transaction_volume: AtomicU64,
    pub average_gas_usage: AtomicU64, // stored as integer * 1000
    pub network_load: AtomicU64,      // stored as percentage * 10000

    // SAGA-specific metrics
    pub throughput: AtomicU64, // Stored as u64 * 1000 for precision
    pub latency: AtomicU64,    // Stored as u64 * 1000 for precision
    pub error_rate: AtomicU64, // Stored as u64 * 1000 for precision
    pub resource_utilization: AtomicU64, // Stored as u64 * 1000 for precision
    pub security_score: AtomicU64, // Stored as u64 * 1000 for precision
    pub stability_index: AtomicU64, // Stored as u64 * 1000 for precision

    // Validator metrics
    pub blocks_produced: AtomicU64,
    pub blocks_validated: AtomicU64,
    pub uptime_percentage: AtomicU64, // stored as percentage * 10000
    pub response_time_ms: AtomicU64,
    pub byzantine_faults: AtomicU64,
    pub governance_participation: AtomicU64, // stored as percentage * 10000

    // Network metrics
    pub latency_ms: AtomicU64,            // stored as integer * 1000
    pub bandwidth_utilization: AtomicU64, // stored as percentage * 10000
    pub packet_loss_rate: AtomicU64,      // stored as percentage * 10000
    pub connection_stability: AtomicU64,  // stored as percentage * 10000
    pub message_throughput: AtomicU64,    // stored as integer * 1000

    // Network health metrics
    pub tps_current: AtomicU64,    // stored as integer * 1000
    pub tps_average_1h: AtomicU64, // stored as integer * 1000
    pub tps_peak_24h: AtomicU64,   // stored as integer * 1000
    pub finality_time_ms: AtomicU64,
    pub network_congestion: AtomicU64, // stored as percentage * 10000
    pub block_propagation_time: AtomicU64, // stored as integer * 1000
    pub mempool_size: AtomicU64,

    // Relayer metrics
    pub relayer_success_rate: AtomicU64, // Stored as percentage * 100
    pub relayer_average_latency: AtomicU64,
    pub relayer_total_fees_earned: AtomicU64, // Stored as u64 (truncated from u128)
    pub relayer_packets_relayed: AtomicU64,
    pub relayer_last_active: AtomicU64,

    // Interoperability metrics
    pub total_channels: AtomicU64,
    pub open_channels: AtomicU64,
    pub total_bridges: AtomicU64,
    pub active_bridges: AtomicU64,
    pub total_swaps: AtomicU64,
    pub completed_swaps: AtomicU64,
    pub active_relayers: AtomicU64,
    pub total_relayers: AtomicU64,

    // Enhanced simulation metrics
    pub omega_rejected_transactions: AtomicU64,
    pub cross_chain_effects_generated: AtomicU64,
    pub governance_proposals_created: AtomicU64,
    pub governance_proposals_passed: AtomicU64,
    pub governance_proposals_omega_rejected: AtomicU64,
    pub quantum_proofs_generated: AtomicU64,
    pub average_stability_score: AtomicU64, // Stored as percentage * 100
    pub threat_level_escalations: AtomicU64,

    // Mining metrics
    #[serde(with = "atomic_serde")]
    pub mining_attempts: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub hash_rate_thousandths: AtomicU64, // stored as H/s * 1000 for precision
    #[serde(with = "atomic_serde")]
    pub last_hash_attempts: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub hash_rate_last_sample_ms: AtomicU64,

    // Timestamp for metrics collection
    pub last_updated: AtomicU64,
}

impl Default for QantoMetrics {
    fn default() -> Self {
        Self {
            // Core blockchain metrics
            blocks_processed: AtomicU64::new(0),
            transactions_processed: AtomicU64::new(0),
            validation_time_ms: AtomicU64::new(0),
            block_creation_time_ms: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            concurrent_validations: AtomicU64::new(0),
            queue_depth: AtomicU64::new(0),
            last_block_time: AtomicU64::new(0),
            throughput_bps: AtomicU64::new(0),
            memory_usage_mb: AtomicU64::new(0),
            cpu_utilization: AtomicU64::new(0),
            disk_io_ops: AtomicU64::new(0),
            network_bytes_sent: AtomicU64::new(0),
            network_bytes_received: AtomicU64::new(0),
            qanto_rss_bytes: AtomicU64::new(0),

            // Performance optimization metrics
            simd_operations: AtomicU64::new(0),
            lock_free_operations: AtomicU64::new(0),
            pipeline_stages_completed: AtomicU64::new(0),
            prefetch_hits: AtomicU64::new(0),
            prefetch_misses: AtomicU64::new(0),
            batch_processing_time_ns: AtomicU64::new(0),
            memory_pool_allocations: AtomicU64::new(0),
            zero_copy_operations: AtomicU64::new(0),
            work_stealing_tasks: AtomicU64::new(0),
            parallel_signature_verifications: AtomicU64::new(0),
            utxo_cache_efficiency: AtomicU64::new(0),
            transaction_throughput_tps: AtomicU64::new(0),
            parallel_validations: AtomicU64::new(0),
            batch_processing_ops: AtomicU64::new(0),
            compression_ratio: AtomicU64::new(0),

            // Network and consensus metrics
            tps: AtomicU64::new(0),
            finality_ms: AtomicU64::new(0),
            validator_count: AtomicU64::new(0),
            shard_count: AtomicU64::new(0),
            cross_shard_tps: AtomicU64::new(0),
            storage_efficiency: AtomicU64::new(0),
            network_bandwidth: AtomicU64::new(0),
            consensus_latency: AtomicU64::new(0),
            transaction_volume: AtomicU64::new(0),
            average_gas_usage: AtomicU64::new(0),
            network_load: AtomicU64::new(0),

            // SAGA-specific metrics
            throughput: AtomicU64::new(0),
            latency: AtomicU64::new(0),
            error_rate: AtomicU64::new(0),
            resource_utilization: AtomicU64::new(0),
            security_score: AtomicU64::new(0),
            stability_index: AtomicU64::new(0),

            // Validator metrics
            blocks_produced: AtomicU64::new(0),
            blocks_validated: AtomicU64::new(0),
            uptime_percentage: AtomicU64::new(0),
            response_time_ms: AtomicU64::new(0),
            byzantine_faults: AtomicU64::new(0),
            governance_participation: AtomicU64::new(0),

            // Network metrics
            latency_ms: AtomicU64::new(0),
            bandwidth_utilization: AtomicU64::new(0),
            packet_loss_rate: AtomicU64::new(0),
            connection_stability: AtomicU64::new(0),
            message_throughput: AtomicU64::new(0),

            // Network health metrics
            tps_current: AtomicU64::new(0),
            tps_average_1h: AtomicU64::new(0),
            tps_peak_24h: AtomicU64::new(0),
            finality_time_ms: AtomicU64::new(0),
            network_congestion: AtomicU64::new(0),
            block_propagation_time: AtomicU64::new(0),
            mempool_size: AtomicU64::new(0),

            // Relayer metrics
            relayer_success_rate: AtomicU64::new(0),
            relayer_average_latency: AtomicU64::new(0),
            relayer_total_fees_earned: AtomicU64::new(0),
            relayer_packets_relayed: AtomicU64::new(0),
            relayer_last_active: AtomicU64::new(0),

            // Interoperability metrics
            total_channels: AtomicU64::new(0),
            open_channels: AtomicU64::new(0),
            total_bridges: AtomicU64::new(0),
            active_bridges: AtomicU64::new(0),
            total_swaps: AtomicU64::new(0),
            completed_swaps: AtomicU64::new(0),
            active_relayers: AtomicU64::new(0),
            total_relayers: AtomicU64::new(0),

            // Enhanced simulation metrics
            omega_rejected_transactions: AtomicU64::new(0),
            cross_chain_effects_generated: AtomicU64::new(0),
            governance_proposals_created: AtomicU64::new(0),
            governance_proposals_passed: AtomicU64::new(0),
            governance_proposals_omega_rejected: AtomicU64::new(0),
            quantum_proofs_generated: AtomicU64::new(0),
            average_stability_score: AtomicU64::new(0),
            threat_level_escalations: AtomicU64::new(0),

            // Mining metrics
            mining_attempts: AtomicU64::new(0),
            hash_rate_thousandths: AtomicU64::new(0),
            last_hash_attempts: AtomicU64::new(0),
            hash_rate_last_sample_ms: AtomicU64::new(0),

            // Timestamp for metrics collection
            last_updated: AtomicU64::new(0),
        }
    }
}

impl Clone for QantoMetrics {
    fn clone(&self) -> Self {
        QantoMetrics {
            blocks_processed: AtomicU64::new(self.blocks_processed.load(Ordering::Relaxed)),
            transactions_processed: AtomicU64::new(
                self.transactions_processed.load(Ordering::Relaxed),
            ),
            validation_time_ms: AtomicU64::new(self.validation_time_ms.load(Ordering::Relaxed)),
            block_creation_time_ms: AtomicU64::new(
                self.block_creation_time_ms.load(Ordering::Relaxed),
            ),
            cache_hits: AtomicU64::new(self.cache_hits.load(Ordering::Relaxed)),
            cache_misses: AtomicU64::new(self.cache_misses.load(Ordering::Relaxed)),
            concurrent_validations: AtomicU64::new(
                self.concurrent_validations.load(Ordering::Relaxed),
            ),
            queue_depth: AtomicU64::new(self.queue_depth.load(Ordering::Relaxed)),
            last_block_time: AtomicU64::new(self.last_block_time.load(Ordering::Relaxed)),
            throughput_bps: AtomicU64::new(self.throughput_bps.load(Ordering::Relaxed)),
            memory_usage_mb: AtomicU64::new(self.memory_usage_mb.load(Ordering::Relaxed)),
            cpu_utilization: AtomicU64::new(self.cpu_utilization.load(Ordering::Relaxed)),
            disk_io_ops: AtomicU64::new(self.disk_io_ops.load(Ordering::Relaxed)),
            network_bytes_sent: AtomicU64::new(self.network_bytes_sent.load(Ordering::Relaxed)),
            network_bytes_received: AtomicU64::new(
                self.network_bytes_received.load(Ordering::Relaxed),
            ),
            qanto_rss_bytes: AtomicU64::new(self.qanto_rss_bytes.load(Ordering::Relaxed)),
            simd_operations: AtomicU64::new(self.simd_operations.load(Ordering::Relaxed)),
            lock_free_operations: AtomicU64::new(self.lock_free_operations.load(Ordering::Relaxed)),
            pipeline_stages_completed: AtomicU64::new(
                self.pipeline_stages_completed.load(Ordering::Relaxed),
            ),
            prefetch_hits: AtomicU64::new(self.prefetch_hits.load(Ordering::Relaxed)),
            prefetch_misses: AtomicU64::new(self.prefetch_misses.load(Ordering::Relaxed)),
            batch_processing_time_ns: AtomicU64::new(
                self.batch_processing_time_ns.load(Ordering::Relaxed),
            ),
            memory_pool_allocations: AtomicU64::new(
                self.memory_pool_allocations.load(Ordering::Relaxed),
            ),
            zero_copy_operations: AtomicU64::new(self.zero_copy_operations.load(Ordering::Relaxed)),
            work_stealing_tasks: AtomicU64::new(self.work_stealing_tasks.load(Ordering::Relaxed)),
            parallel_signature_verifications: AtomicU64::new(
                self.parallel_signature_verifications
                    .load(Ordering::Relaxed),
            ),
            utxo_cache_efficiency: AtomicU64::new(
                self.utxo_cache_efficiency.load(Ordering::Relaxed),
            ),
            transaction_throughput_tps: AtomicU64::new(
                self.transaction_throughput_tps.load(Ordering::Relaxed),
            ),
            parallel_validations: AtomicU64::new(self.parallel_validations.load(Ordering::Relaxed)),
            batch_processing_ops: AtomicU64::new(self.batch_processing_ops.load(Ordering::Relaxed)),
            compression_ratio: AtomicU64::new(self.compression_ratio.load(Ordering::Relaxed)),
            tps: AtomicU64::new(self.tps.load(Ordering::Relaxed)),
            finality_ms: AtomicU64::new(self.finality_ms.load(Ordering::Relaxed)),
            validator_count: AtomicU64::new(self.validator_count.load(Ordering::Relaxed)),
            shard_count: AtomicU64::new(self.shard_count.load(Ordering::Relaxed)),
            cross_shard_tps: AtomicU64::new(self.cross_shard_tps.load(Ordering::Relaxed)),
            storage_efficiency: AtomicU64::new(self.storage_efficiency.load(Ordering::Relaxed)),
            network_bandwidth: AtomicU64::new(self.network_bandwidth.load(Ordering::Relaxed)),
            consensus_latency: AtomicU64::new(self.consensus_latency.load(Ordering::Relaxed)),
            transaction_volume: AtomicU64::new(self.transaction_volume.load(Ordering::Relaxed)),
            average_gas_usage: AtomicU64::new(self.average_gas_usage.load(Ordering::Relaxed)),
            network_load: AtomicU64::new(self.network_load.load(Ordering::Relaxed)),
            throughput: AtomicU64::new(self.throughput.load(Ordering::Relaxed)),
            latency: AtomicU64::new(self.latency.load(Ordering::Relaxed)),
            error_rate: AtomicU64::new(self.error_rate.load(Ordering::Relaxed)),
            resource_utilization: AtomicU64::new(self.resource_utilization.load(Ordering::Relaxed)),
            security_score: AtomicU64::new(self.security_score.load(Ordering::Relaxed)),
            stability_index: AtomicU64::new(self.stability_index.load(Ordering::Relaxed)),
            blocks_produced: AtomicU64::new(self.blocks_produced.load(Ordering::Relaxed)),
            blocks_validated: AtomicU64::new(self.blocks_validated.load(Ordering::Relaxed)),
            uptime_percentage: AtomicU64::new(self.uptime_percentage.load(Ordering::Relaxed)),
            response_time_ms: AtomicU64::new(self.response_time_ms.load(Ordering::Relaxed)),
            byzantine_faults: AtomicU64::new(self.byzantine_faults.load(Ordering::Relaxed)),
            governance_participation: AtomicU64::new(
                self.governance_participation.load(Ordering::Relaxed),
            ),
            latency_ms: AtomicU64::new(self.latency_ms.load(Ordering::Relaxed)),
            bandwidth_utilization: AtomicU64::new(
                self.bandwidth_utilization.load(Ordering::Relaxed),
            ),
            packet_loss_rate: AtomicU64::new(self.packet_loss_rate.load(Ordering::Relaxed)),
            connection_stability: AtomicU64::new(self.connection_stability.load(Ordering::Relaxed)),
            message_throughput: AtomicU64::new(self.message_throughput.load(Ordering::Relaxed)),
            tps_current: AtomicU64::new(self.tps_current.load(Ordering::Relaxed)),
            tps_average_1h: AtomicU64::new(self.tps_average_1h.load(Ordering::Relaxed)),
            tps_peak_24h: AtomicU64::new(self.tps_peak_24h.load(Ordering::Relaxed)),
            finality_time_ms: AtomicU64::new(self.finality_time_ms.load(Ordering::Relaxed)),
            network_congestion: AtomicU64::new(self.network_congestion.load(Ordering::Relaxed)),
            block_propagation_time: AtomicU64::new(
                self.block_propagation_time.load(Ordering::Relaxed),
            ),
            mempool_size: AtomicU64::new(self.mempool_size.load(Ordering::Relaxed)),
            relayer_success_rate: AtomicU64::new(self.relayer_success_rate.load(Ordering::Relaxed)),
            relayer_average_latency: AtomicU64::new(
                self.relayer_average_latency.load(Ordering::Relaxed),
            ),
            relayer_total_fees_earned: AtomicU64::new(
                self.relayer_total_fees_earned.load(Ordering::Relaxed),
            ),
            relayer_packets_relayed: AtomicU64::new(
                self.relayer_packets_relayed.load(Ordering::Relaxed),
            ),
            relayer_last_active: AtomicU64::new(self.relayer_last_active.load(Ordering::Relaxed)),
            total_channels: AtomicU64::new(self.total_channels.load(Ordering::Relaxed)),
            open_channels: AtomicU64::new(self.open_channels.load(Ordering::Relaxed)),
            total_bridges: AtomicU64::new(self.total_bridges.load(Ordering::Relaxed)),
            active_bridges: AtomicU64::new(self.active_bridges.load(Ordering::Relaxed)),
            total_swaps: AtomicU64::new(self.total_swaps.load(Ordering::Relaxed)),
            completed_swaps: AtomicU64::new(self.completed_swaps.load(Ordering::Relaxed)),
            active_relayers: AtomicU64::new(self.active_relayers.load(Ordering::Relaxed)),
            total_relayers: AtomicU64::new(self.total_relayers.load(Ordering::Relaxed)),
            omega_rejected_transactions: AtomicU64::new(
                self.omega_rejected_transactions.load(Ordering::Relaxed),
            ),
            cross_chain_effects_generated: AtomicU64::new(
                self.cross_chain_effects_generated.load(Ordering::Relaxed),
            ),
            governance_proposals_created: AtomicU64::new(
                self.governance_proposals_created.load(Ordering::Relaxed),
            ),
            governance_proposals_passed: AtomicU64::new(
                self.governance_proposals_passed.load(Ordering::Relaxed),
            ),
            governance_proposals_omega_rejected: AtomicU64::new(
                self.governance_proposals_omega_rejected
                    .load(Ordering::Relaxed),
            ),
            quantum_proofs_generated: AtomicU64::new(
                self.quantum_proofs_generated.load(Ordering::Relaxed),
            ),
            average_stability_score: AtomicU64::new(
                self.average_stability_score.load(Ordering::Relaxed),
            ),
            threat_level_escalations: AtomicU64::new(
                self.threat_level_escalations.load(Ordering::Relaxed),
            ),
            // Mining metrics
            mining_attempts: AtomicU64::new(self.mining_attempts.load(Ordering::Relaxed)),
            hash_rate_thousandths: AtomicU64::new(
                self.hash_rate_thousandths.load(Ordering::Relaxed),
            ),
            last_hash_attempts: AtomicU64::new(self.last_hash_attempts.load(Ordering::Relaxed)),
            hash_rate_last_sample_ms: AtomicU64::new(
                self.hash_rate_last_sample_ms.load(Ordering::Relaxed),
            ),
            last_updated: AtomicU64::new(self.last_updated.load(Ordering::Relaxed)),
        }
    }
}

impl QantoMetrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Update the last updated timestamp
    pub fn touch(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_updated.store(now, Ordering::Relaxed);
    }

    /// Get current TPS (Transactions Per Second)
    pub fn get_tps(&self) -> f64 {
        self.tps.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set TPS value
    pub fn set_tps(&self, value: f64) {
        self.tps.store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get current BPS (Blocks Per Second)
    pub fn get_bps(&self) -> f64 {
        let blocks = self.blocks_processed.load(Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_time = self.last_updated.load(Ordering::Relaxed);

        if now > start_time && start_time > 0 {
            blocks as f64 / (now - start_time) as f64
        } else {
            0.0
        }
    }

    /// Calculate real-time TPS based on recent transactions
    pub fn calculate_real_time_tps(&self) -> f64 {
        let transactions = self.transactions_processed.load(Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_time = self.last_updated.load(Ordering::Relaxed);

        if now > start_time && start_time > 0 {
            transactions as f64 / (now - start_time) as f64
        } else {
            0.0
        }
    }

    /// Get average latency in milliseconds
    pub fn get_average_latency(&self) -> f64 {
        let total_validation_time = self.validation_time_ms.load(Ordering::Relaxed);
        let total_blocks = self.blocks_processed.load(Ordering::Relaxed);

        if total_blocks > 0 {
            total_validation_time as f64 / total_blocks as f64
        } else {
            0.0
        }
    }

    /// Get memory utilization percentage
    pub fn get_memory_utilization(&self) -> f64 {
        let used_mb = self.memory_usage_mb.load(Ordering::Relaxed);
        // Get actual system memory instead of hardcoded 16GB
        let mut sys = System::new_all();
        sys.refresh_memory();
        let total_mb = (sys.total_memory() / (1024 * 1024)) as f64; // Convert from bytes to MB
        (used_mb as f64 / total_mb) * 100.0
    }

    /// Get CPU utilization percentage
    pub fn get_cpu_utilization(&self) -> f64 {
        self.cpu_utilization.load(Ordering::Relaxed) as f64 / 100.0
    }

    /// Get network throughput in MB/s
    pub fn get_network_throughput(&self) -> f64 {
        let bytes_sent = self.network_bytes_sent.load(Ordering::Relaxed);
        let bytes_received = self.network_bytes_received.load(Ordering::Relaxed);
        let total_bytes = bytes_sent + bytes_received;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_time = self.last_updated.load(Ordering::Relaxed);

        if now > start_time && start_time > 0 {
            (total_bytes as f64 / (1024.0 * 1024.0)) / (now - start_time) as f64
        } else {
            0.0
        }
    }

    /// Record system resource metrics
    pub fn record_system_metrics(&self) {
        // Update CPU utilization (would be populated by system monitoring)
        // This is a placeholder - in production, this would read from /proc/stat or similar

        // Update memory usage (would be populated by system monitoring)
        // This is a placeholder - in production, this would read from /proc/meminfo or similar

        self.touch();
    }

    /// Get comprehensive performance summary
    pub fn get_performance_summary(&self) -> HashMap<String, f64> {
        let mut summary = HashMap::new();

        summary.insert("bps".to_string(), self.get_bps());
        summary.insert("tps_real_time".to_string(), self.calculate_real_time_tps());
        summary.insert("tps_configured".to_string(), self.get_tps());
        summary.insert("average_latency_ms".to_string(), self.get_average_latency());
        summary.insert(
            "memory_utilization_percent".to_string(),
            self.get_memory_utilization(),
        );
        summary.insert(
            "cpu_utilization_percent".to_string(),
            self.get_cpu_utilization(),
        );
        summary.insert(
            "network_throughput_mbps".to_string(),
            self.get_network_throughput(),
        );
        summary.insert("cache_hit_ratio".to_string(), self.get_cache_hit_ratio());
        summary.insert("finality_ms".to_string(), self.get_finality_ms() as f64);
        summary.insert(
            "validator_count".to_string(),
            self.validator_count.load(Ordering::Relaxed) as f64,
        );
        summary.insert(
            "queue_depth".to_string(),
            self.queue_depth.load(Ordering::Relaxed) as f64,
        );
        summary.insert(
            "concurrent_validations".to_string(),
            self.concurrent_validations.load(Ordering::Relaxed) as f64,
        );

        // Performance optimization metrics
        summary.insert(
            "simd_operations".to_string(),
            self.simd_operations.load(Ordering::Relaxed) as f64,
        );
        summary.insert(
            "lock_free_operations".to_string(),
            self.lock_free_operations.load(Ordering::Relaxed) as f64,
        );
        summary.insert(
            "zero_copy_operations".to_string(),
            self.zero_copy_operations.load(Ordering::Relaxed) as f64,
        );
        summary.insert(
            "parallel_validations".to_string(),
            self.parallel_validations.load(Ordering::Relaxed) as f64,
        );

        // Resource efficiency metrics
        summary.insert(
            "storage_efficiency".to_string(),
            self.get_storage_efficiency(),
        );
        summary.insert("error_rate".to_string(), self.get_error_rate());
        summary.insert("stability_index".to_string(), self.get_stability_index());

        summary
    }

    /// Start performance monitoring with periodic updates
    pub fn start_monitoring(&self) -> std::thread::JoinHandle<()> {
        let metrics = Arc::new(self.clone());

        std::thread::spawn(move || {
            loop {
                // Record system metrics every second
                metrics.record_system_metrics();

                // Calculate and update real-time TPS
                let real_time_tps = metrics.calculate_real_time_tps();
                metrics.set_tps(real_time_tps);

                // Calculate and update real-time hash rate (H/s)
                let current_attempts = metrics.mining_attempts.load(Ordering::Relaxed);
                let previous_attempts = metrics.last_hash_attempts.load(Ordering::Relaxed);
                metrics
                    .last_hash_attempts
                    .store(current_attempts, Ordering::Relaxed);

                // Use actual elapsed milliseconds for precision
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let prev_ms = metrics
                    .hash_rate_last_sample_ms
                    .swap(now_ms, Ordering::Relaxed);
                let interval_ms = if prev_ms == 0 {
                    1000 // bootstrap interval for the first sample to avoid epoch-based duration
                } else {
                    now_ms.saturating_sub(prev_ms)
                };
                let delta_attempts = current_attempts.saturating_sub(previous_attempts);
                let hps = QantoMetrics::compute_hash_rate(delta_attempts, interval_ms);
                metrics.set_hash_rate_hps(hps);

                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        })
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Core blockchain metrics
        output.push_str(&format!(
            "qanto_blocks_processed_total {}\n",
            self.blocks_processed.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_transactions_processed_total {}\n",
            self.transactions_processed.load(Ordering::Relaxed)
        ));
        output.push_str(&format!("qanto_bps {:.2}\n", self.get_bps()));
        output.push_str(&format!(
            "qanto_tps {:.2}\n",
            self.calculate_real_time_tps()
        ));
        output.push_str(&format!(
            "qanto_average_latency_ms {:.2}\n",
            self.get_average_latency()
        ));

        // Mining metrics
        output.push_str(&format!(
            "qanto_mining_attempts_total {}\n",
            self.mining_attempts.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_hash_rate_hps {:.2}\n",
            self.get_hash_rate_hps()
        ));

        // Resource utilization
        output.push_str(&format!(
            "qanto_memory_utilization_percent {:.2}\n",
            self.get_memory_utilization()
        ));
        output.push_str(&format!(
            "qanto_rss_bytes {}\n",
            self.qanto_rss_bytes.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_cpu_utilization_percent {:.2}\n",
            self.get_cpu_utilization()
        ));
        output.push_str(&format!(
            "qanto_network_throughput_mbps {:.2}\n",
            self.get_network_throughput()
        ));

        // Performance metrics
        output.push_str(&format!(
            "qanto_cache_hit_ratio {:.4}\n",
            self.get_cache_hit_ratio()
        ));
        output.push_str(&format!("qanto_finality_ms {}\n", self.get_finality_ms()));
        output.push_str(&format!(
            "qanto_validator_count {}\n",
            self.validator_count.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_queue_depth {}\n",
            self.queue_depth.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_mempool_size {}\n",
            self.mempool_size.load(Ordering::Relaxed)
        ));

        // Optimization metrics
        output.push_str(&format!(
            "qanto_simd_operations_total {}\n",
            self.simd_operations.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_lock_free_operations_total {}\n",
            self.lock_free_operations.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_zero_copy_operations_total {}\n",
            self.zero_copy_operations.load(Ordering::Relaxed)
        ));

        output
    }

    /// Add hash rate getter/setter and computation helpers
    /// Compute hash rate in H/s given the number of attempts over an interval in milliseconds
    pub fn compute_hash_rate(attempts_delta: u64, interval_ms: u64) -> f64 {
        if interval_ms == 0 {
            return 0.0;
        }
        let interval_seconds = interval_ms as f64 / 1000.0;
        attempts_delta as f64 / interval_seconds
    }

    /// Format hash rate into human-friendly units (H/s, kH/s, MH/s, GH/s, TH/s, PH/s)
    pub fn format_hash_rate(hps: f64) -> String {
        let mut value = hps;
        if !value.is_finite() || value < 0.0 {
            value = 0.0;
        }
        let units = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s"];
        let mut unit_idx = 0;
        while value >= 1000.0 && unit_idx < units.len() - 1 {
            value /= 1000.0;
            unit_idx += 1;
        }
        format!("{:.2} {}", value, units[unit_idx])
    }

    /// Get the current hash rate in H/s
    pub fn get_hash_rate_hps(&self) -> f64 {
        self.hash_rate_thousandths.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set the current hash rate in H/s
    pub fn set_hash_rate_hps(&self, value: f64) {
        self.hash_rate_thousandths
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get finality time in milliseconds
    pub fn get_finality_ms(&self) -> u64 {
        self.finality_ms.load(Ordering::Relaxed)
    }

    /// Set finality time in milliseconds
    pub fn set_finality_ms(&self, value: u64) {
        self.finality_ms.store(value, Ordering::Relaxed);
        self.touch();
    }

    /// Get storage efficiency as floating point
    pub fn get_storage_efficiency(&self) -> f64 {
        self.storage_efficiency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set storage efficiency from floating point
    pub fn set_storage_efficiency(&self, value: f64) {
        self.storage_efficiency
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get SAGA throughput as floating point
    pub fn get_saga_throughput(&self) -> f64 {
        self.throughput.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set SAGA throughput from floating point
    pub fn set_saga_throughput(&self, value: f64) {
        self.throughput
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get SAGA latency as floating point
    pub fn get_saga_latency(&self) -> f64 {
        self.latency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set SAGA latency from floating point
    pub fn set_saga_latency(&self, value: f64) {
        self.latency
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get error rate as floating point
    pub fn get_error_rate(&self) -> f64 {
        self.error_rate.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set error rate from floating point
    pub fn set_error_rate(&self, value: f64) {
        self.error_rate
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get resource utilization as floating point
    pub fn get_resource_utilization(&self) -> f64 {
        self.resource_utilization.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set resource utilization from floating point
    pub fn set_resource_utilization(&self, value: f64) {
        self.resource_utilization
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get security score as floating point
    pub fn get_security_score(&self) -> f64 {
        self.security_score.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set security score from floating point
    pub fn set_security_score(&self, value: f64) {
        self.security_score
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Get stability index as floating point
    pub fn get_stability_index(&self) -> f64 {
        self.stability_index.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set stability index from floating point
    pub fn set_stability_index(&self, value: f64) {
        self.stability_index
            .store((value * 1000.0) as u64, Ordering::Relaxed);
        self.touch();
    }

    /// Increment blocks processed counter
    pub fn increment_blocks_processed(&self) {
        self.blocks_processed.fetch_add(1, Ordering::Relaxed);
        self.touch();
    }

    /// Increment transactions processed counter
    pub fn increment_transactions_processed(&self, count: u64) {
        self.transactions_processed
            .fetch_add(count, Ordering::Relaxed);
        self.touch();
    }

    /// Record validation time
    pub fn record_validation_time(&self, time_ms: u64) {
        self.validation_time_ms.store(time_ms, Ordering::Relaxed);
        self.touch();
    }

    /// Record block creation time
    pub fn record_block_creation_time(&self, time_ms: u64) {
        self.block_creation_time_ms
            .store(time_ms, Ordering::Relaxed);
        self.touch();
    }

    /// Increment cache hits
    pub fn increment_cache_hits(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
        self.touch();
    }

    /// Increment cache misses
    pub fn increment_cache_misses(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        self.touch();
    }

    /// Get cache hit ratio
    pub fn get_cache_hit_ratio(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.cache_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }

    /// Export metrics as a HashMap for external systems
    pub fn export_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();

        metrics.insert(
            "blocks_processed".to_string(),
            self.blocks_processed.load(Ordering::Relaxed) as f64,
        );
        metrics.insert(
            "transactions_processed".to_string(),
            self.transactions_processed.load(Ordering::Relaxed) as f64,
        );
        metrics.insert(
            "validation_time_ms".to_string(),
            self.validation_time_ms.load(Ordering::Relaxed) as f64,
        );
        metrics.insert(
            "block_creation_time_ms".to_string(),
            self.block_creation_time_ms.load(Ordering::Relaxed) as f64,
        );
        metrics.insert(
            "cache_hits".to_string(),
            self.cache_hits.load(Ordering::Relaxed) as f64,
        );
        metrics.insert(
            "cache_misses".to_string(),
            self.cache_misses.load(Ordering::Relaxed) as f64,
        );
        metrics.insert("cache_hit_ratio".to_string(), self.get_cache_hit_ratio());
        metrics.insert("tps".to_string(), self.get_tps());
        metrics.insert("finality_ms".to_string(), self.get_finality_ms() as f64);
        metrics.insert("rss_bytes".to_string(), self.get_rss_bytes() as f64);
        metrics.insert(
            "validator_count".to_string(),
            self.validator_count.load(Ordering::Relaxed) as f64,
        );
        metrics.insert(
            "storage_efficiency".to_string(),
            self.get_storage_efficiency(),
        );
        metrics.insert("saga_throughput".to_string(), self.get_saga_throughput());
        metrics.insert("saga_latency".to_string(), self.get_saga_latency());
        metrics.insert("error_rate".to_string(), self.get_error_rate());
        metrics.insert("security_score".to_string(), self.get_security_score());
        metrics.insert("stability_index".to_string(), self.get_stability_index());

        metrics
    }

    /// Get RSS memory usage in bytes
    pub fn get_rss_bytes(&self) -> u64 {
        self.qanto_rss_bytes.load(Ordering::Relaxed)
    }

    /// Set RSS memory usage in bytes
    pub fn set_rss_bytes(&self, bytes: u64) {
        self.qanto_rss_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Update RSS memory usage from system
    pub fn update_rss_memory(&self) {
        if let Ok(rss_bytes) = get_rss_memory_bytes() {
            self.set_rss_bytes(rss_bytes);
        }
    }

    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.blocks_processed.store(0, Ordering::Relaxed);
        self.transactions_processed.store(0, Ordering::Relaxed);
        self.validation_time_ms.store(0, Ordering::Relaxed);
        self.block_creation_time_ms.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.concurrent_validations.store(0, Ordering::Relaxed);
        self.queue_depth.store(0, Ordering::Relaxed);
        self.last_block_time.store(0, Ordering::Relaxed);
        self.throughput_bps.store(0, Ordering::Relaxed);
        self.simd_operations.store(0, Ordering::Relaxed);
        self.lock_free_operations.store(0, Ordering::Relaxed);
        self.pipeline_stages_completed.store(0, Ordering::Relaxed);
        self.prefetch_hits.store(0, Ordering::Relaxed);
        self.prefetch_misses.store(0, Ordering::Relaxed);
        self.batch_processing_time_ns.store(0, Ordering::Relaxed);
        self.memory_pool_allocations.store(0, Ordering::Relaxed);
        self.zero_copy_operations.store(0, Ordering::Relaxed);
        self.work_stealing_tasks.store(0, Ordering::Relaxed);
        self.parallel_signature_verifications
            .store(0, Ordering::Relaxed);
        self.utxo_cache_efficiency.store(0, Ordering::Relaxed);
        self.transaction_throughput_tps.store(0, Ordering::Relaxed);
        self.tps.store(0, Ordering::Relaxed);
        self.finality_ms.store(0, Ordering::Relaxed);
        self.validator_count.store(0, Ordering::Relaxed);
        self.shard_count.store(0, Ordering::Relaxed);
        self.cross_shard_tps.store(0, Ordering::Relaxed);
        self.storage_efficiency.store(0, Ordering::Relaxed);
        self.network_bandwidth.store(0, Ordering::Relaxed);
        self.consensus_latency.store(0, Ordering::Relaxed);
        self.throughput.store(0, Ordering::Relaxed);
        self.latency.store(0, Ordering::Relaxed);
        self.error_rate.store(0, Ordering::Relaxed);
        self.resource_utilization.store(0, Ordering::Relaxed);
        self.security_score.store(0, Ordering::Relaxed);
        self.stability_index.store(0, Ordering::Relaxed);

        // Reset relayer metrics
        self.relayer_success_rate.store(0, Ordering::Relaxed);
        self.relayer_average_latency.store(0, Ordering::Relaxed);
        self.relayer_total_fees_earned.store(0, Ordering::Relaxed);
        self.relayer_packets_relayed.store(0, Ordering::Relaxed);
        self.relayer_last_active.store(0, Ordering::Relaxed);

        // Reset interoperability metrics
        self.total_channels.store(0, Ordering::Relaxed);
        self.open_channels.store(0, Ordering::Relaxed);
        self.total_bridges.store(0, Ordering::Relaxed);
        self.active_bridges.store(0, Ordering::Relaxed);
        self.total_swaps.store(0, Ordering::Relaxed);
        self.completed_swaps.store(0, Ordering::Relaxed);
        self.active_relayers.store(0, Ordering::Relaxed);
        self.total_relayers.store(0, Ordering::Relaxed);

        // Reset enhanced simulation metrics
        self.omega_rejected_transactions.store(0, Ordering::Relaxed);
        self.cross_chain_effects_generated
            .store(0, Ordering::Relaxed);
        self.governance_proposals_created
            .store(0, Ordering::Relaxed);
        self.governance_proposals_passed.store(0, Ordering::Relaxed);
        self.governance_proposals_omega_rejected
            .store(0, Ordering::Relaxed);
        self.quantum_proofs_generated.store(0, Ordering::Relaxed);
        self.average_stability_score.store(0, Ordering::Relaxed);
        self.threat_level_escalations.store(0, Ordering::Relaxed);

        self.touch();
    }
}

/// Get RSS memory usage in bytes from the system
pub fn get_rss_memory_bytes() -> Result<u64, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let status = fs::read_to_string("/proc/self/status")?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()?;
                    return Ok(kb * 1024); // Convert KB to bytes
                }
            }
        }
        Err("VmRSS not found in /proc/self/status".into())
    }

    #[cfg(target_os = "macos")]
    {
        use std::mem;

        // Use mach_task_basic_info to get memory usage on macOS
        let mut info: libc::mach_task_basic_info = unsafe { mem::zeroed() };
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;

        let result = unsafe {
            #[allow(deprecated)]
            libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as *mut i32,
                &mut count,
            )
        };

        if result == libc::KERN_SUCCESS {
            Ok(info.resident_size)
        } else {
            Err("Failed to get task info on macOS".into())
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Fallback for other platforms - use sysinfo
        let mut system = System::new();
        system.refresh_process(sysinfo::get_current_pid().unwrap());
        if let Some(process) = system.process(sysinfo::get_current_pid().unwrap()) {
            Ok(process.memory() * 1024) // sysinfo returns KB, convert to bytes
        } else {
            Err("Failed to get process memory info".into())
        }
    }
}

pub static GLOBAL_METRICS: once_cell::sync::Lazy<Arc<QantoMetrics>> =
    once_cell::sync::Lazy::new(|| Arc::new(QantoMetrics::new()));

/// Get a reference to the global metrics instance
pub fn get_global_metrics() -> Arc<QantoMetrics> {
    GLOBAL_METRICS.clone()
}

/// Convenience macro for incrementing metrics
#[macro_export]
macro_rules! increment_metric {
    ($metric:ident) => {
        $crate::metrics::get_global_metrics()
            .$metric
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    };
    ($metric:ident, $value:expr) => {
        $crate::metrics::get_global_metrics()
            .$metric
            .fetch_add($value, std::sync::atomic::Ordering::Relaxed);
    };
}

/// Convenience macro for setting metrics
#[macro_export]
macro_rules! set_metric {
    ($metric:ident, $value:expr) => {
        $crate::metrics::get_global_metrics()
            .$metric
            .store($value, std::sync::atomic::Ordering::Relaxed);
    };
}

mod atomic_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::atomic::{AtomicU64, Ordering};

    pub fn serialize<S>(atomic: &AtomicU64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(atomic.load(Ordering::Relaxed))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AtomicU64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <u64 as Deserialize>::deserialize(deserializer)?;
        Ok(AtomicU64::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::QantoMetrics;

    #[test]
    fn test_compute_hash_rate() {
        // 1000 attempts over 1 second -> 1000 H/s
        assert!((QantoMetrics::compute_hash_rate(1000, 1000) - 1000.0).abs() < 1e-6);
        // 500 attempts over 2 seconds -> 250 H/s
        assert!((QantoMetrics::compute_hash_rate(500, 2000) - 250.0).abs() < 1e-6);
    }

    #[test]
    fn test_format_hash_rate_units() {
        assert_eq!(QantoMetrics::format_hash_rate(0.0), "0.00 H/s");
        assert_eq!(QantoMetrics::format_hash_rate(999.0), "999.00 H/s");
        assert_eq!(QantoMetrics::format_hash_rate(1000.0), "1.00 kH/s");
        assert_eq!(QantoMetrics::format_hash_rate(12_345.0), "12.35 kH/s");
        assert_eq!(QantoMetrics::format_hash_rate(1_000_000.0), "1.00 MH/s");
        assert_eq!(QantoMetrics::format_hash_rate(12_345_678.0), "12.35 MH/s");
        assert_eq!(QantoMetrics::format_hash_rate(1_234_567_890.0), "1.23 GH/s");
        assert_eq!(QantoMetrics::format_hash_rate(-5.0), "0.00 H/s"); // clamp negatives
    }
}
