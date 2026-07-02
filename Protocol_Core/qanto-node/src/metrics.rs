//! Unified metrics system for Qanto blockchain
//! This module provides a centralized metrics collection and reporting system
//! that eliminates duplication across the codebase.
//! v0.1.0
use ahash::AHashMap as HashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tokio::sync::RwLock;

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

    // Persistence metrics
    pub persistence_batches: AtomicU64,
    pub persistence_last_batch_ops: AtomicU64,
    pub persistence_last_batch_bytes: AtomicU64,
    pub persistence_overflows: AtomicU64,

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
    #[serde(with = "rwlock_u128_serde")]
    pub total_value_locked: Arc<RwLock<u128>>,
    #[serde(with = "rwlock_u128_serde")]
    pub validator_rewards_24h: Arc<RwLock<u128>>,
    #[serde(with = "rwlock_u128_serde")]
    pub transaction_fees_24h: Arc<RwLock<u128>>,

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

    // New metrics for Cohort A fork and peer tracking
    #[serde(with = "atomic_serde")]
    pub parallel_tips_total: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub fork_events_total: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub forks_last_24h: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub max_fork_depth: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub last_fork_timestamp: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub height_divergence_events: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub chain_reconciliation_events: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub unique_peers_seen_24h: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub peer_session_duration_p50: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub peer_session_duration_p95: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub peer_disconnects_24h: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub tip_count: AtomicU64,
    #[serde(with = "atomic_serde")]
    pub tip_count_max_24h: AtomicU64,

    #[serde(with = "rwlock_vec_u64_serde")]
    pub fork_timestamps: Arc<RwLock<Vec<u64>>>,
    #[serde(with = "rwlock_vec_tuple_serde")]
    pub tip_count_timestamps: Arc<RwLock<Vec<(u64, u64)>>>,
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

            // Persistence metrics
            persistence_batches: AtomicU64::new(0),
            persistence_last_batch_ops: AtomicU64::new(0),
            persistence_last_batch_bytes: AtomicU64::new(0),
            persistence_overflows: AtomicU64::new(0),

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
            total_value_locked: Arc::new(RwLock::new(0)),
            validator_rewards_24h: Arc::new(RwLock::new(0)),
            transaction_fees_24h: Arc::new(RwLock::new(0)),

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

            // New metrics for Cohort A fork and peer tracking
            parallel_tips_total: AtomicU64::new(0),
            fork_events_total: AtomicU64::new(0),
            forks_last_24h: AtomicU64::new(0),
            max_fork_depth: AtomicU64::new(0),
            last_fork_timestamp: AtomicU64::new(0),
            height_divergence_events: AtomicU64::new(0),
            chain_reconciliation_events: AtomicU64::new(0),
            unique_peers_seen_24h: AtomicU64::new(0),
            peer_session_duration_p50: AtomicU64::new(0),
            peer_session_duration_p95: AtomicU64::new(0),
            peer_disconnects_24h: AtomicU64::new(0),
            tip_count: AtomicU64::new(0),
            tip_count_max_24h: AtomicU64::new(0),
            fork_timestamps: Arc::new(RwLock::new(Vec::new())),
            tip_count_timestamps: Arc::new(RwLock::new(Vec::new())),
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
            persistence_batches: AtomicU64::new(self.persistence_batches.load(Ordering::Relaxed)),
            persistence_last_batch_ops: AtomicU64::new(
                self.persistence_last_batch_ops.load(Ordering::Relaxed),
            ),
            persistence_last_batch_bytes: AtomicU64::new(
                self.persistence_last_batch_bytes.load(Ordering::Relaxed),
            ),
            persistence_overflows: AtomicU64::new(
                self.persistence_overflows.load(Ordering::Relaxed),
            ),
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
            total_value_locked: Arc::clone(&self.total_value_locked),
            validator_rewards_24h: Arc::clone(&self.validator_rewards_24h),
            transaction_fees_24h: Arc::clone(&self.transaction_fees_24h),
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
            parallel_tips_total: AtomicU64::new(self.parallel_tips_total.load(Ordering::Relaxed)),
            fork_events_total: AtomicU64::new(self.fork_events_total.load(Ordering::Relaxed)),
            forks_last_24h: AtomicU64::new(self.forks_last_24h.load(Ordering::Relaxed)),
            max_fork_depth: AtomicU64::new(self.max_fork_depth.load(Ordering::Relaxed)),
            last_fork_timestamp: AtomicU64::new(self.last_fork_timestamp.load(Ordering::Relaxed)),
            height_divergence_events: AtomicU64::new(
                self.height_divergence_events.load(Ordering::Relaxed),
            ),
            chain_reconciliation_events: AtomicU64::new(
                self.chain_reconciliation_events.load(Ordering::Relaxed),
            ),
            unique_peers_seen_24h: AtomicU64::new(
                self.unique_peers_seen_24h.load(Ordering::Relaxed),
            ),
            peer_session_duration_p50: AtomicU64::new(
                self.peer_session_duration_p50.load(Ordering::Relaxed),
            ),
            peer_session_duration_p95: AtomicU64::new(
                self.peer_session_duration_p95.load(Ordering::Relaxed),
            ),
            peer_disconnects_24h: AtomicU64::new(self.peer_disconnects_24h.load(Ordering::Relaxed)),
            tip_count: AtomicU64::new(self.tip_count.load(Ordering::Relaxed)),
            tip_count_max_24h: AtomicU64::new(self.tip_count_max_24h.load(Ordering::Relaxed)),
            fork_timestamps: Arc::clone(&self.fork_timestamps),
            tip_count_timestamps: Arc::clone(&self.tip_count_timestamps),
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

    /// Get current TPS (Transactions Per Second) - Scaled by 1e9
    pub fn get_tps(&self) -> u128 {
        self.tps.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set TPS value (input value is scaled by 1e9)
    pub fn set_tps(&self, value_scaled: u128) {
        self.tps.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get current BPS (Blocks Per Second) - Scaled by 1e9
    pub fn get_bps(&self) -> u128 {
        let blocks = self.blocks_processed.load(Ordering::Relaxed) as u128;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_time = self.last_updated.load(Ordering::Relaxed);

        if now > start_time && start_time > 0 {
            (blocks * crate::QANTO_SCALE) / (now - start_time) as u128
        } else {
            0
        }
    }

    /// Calculate real-time TPS based on recent transactions - Scaled by 1e9
    pub fn calculate_real_time_tps(&self) -> u128 {
        let transactions = self.transactions_processed.load(Ordering::Relaxed) as u128;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_time = self.last_updated.load(Ordering::Relaxed);

        if now > start_time && start_time > 0 {
            (transactions * crate::QANTO_SCALE) / (now - start_time) as u128
        } else {
            0
        }
    }

    /// Get average latency in milliseconds - Scaled by 1e9
    pub fn get_average_latency(&self) -> u128 {
        let total_validation_time = self.validation_time_ms.load(Ordering::Relaxed) as u128;
        let total_blocks = self.blocks_processed.load(Ordering::Relaxed) as u128;

        if total_blocks > 0 {
            (total_validation_time * crate::QANTO_SCALE) / total_blocks
        } else {
            0
        }
    }

    /// Get memory utilization percentage - Scaled by 1e9 (e.g. 100% = 1e9)
    pub fn get_memory_utilization(&self) -> u128 {
        let used_mb = self.memory_usage_mb.load(Ordering::Relaxed) as u128;
        // Get actual system memory instead of hardcoded 16GB
        let mut sys = System::new_all();
        sys.refresh_memory();
        let total_mb = (sys.total_memory() / (1024 * 1024)) as u128; // Convert from bytes to MB
        if total_mb > 0 {
            (used_mb * crate::QANTO_SCALE * 100) / total_mb
        } else {
            0
        }
    }

    /// Get CPU utilization percentage - Scaled by 1e9 (e.g. 100% = 1e9)
    pub fn get_cpu_utilization(&self) -> u128 {
        self.cpu_utilization.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 100)
    }

    /// Get network throughput in MB/s - Scaled by 1e9
    pub fn get_network_throughput(&self) -> u128 {
        let bytes_sent = self.network_bytes_sent.load(Ordering::Relaxed) as u128;
        let bytes_received = self.network_bytes_received.load(Ordering::Relaxed) as u128;
        let total_bytes = bytes_sent + bytes_received;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_time = self.last_updated.load(Ordering::Relaxed);

        if now > start_time && start_time > 0 {
            let total_mb_scaled = (total_bytes * crate::QANTO_SCALE) / (1024 * 1024);
            total_mb_scaled / (now - start_time) as u128
        } else {
            0
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
    pub fn get_performance_summary(&self) -> HashMap<String, u128> {
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
        summary.insert("finality_ms".to_string(), self.get_finality_ms() as u128);
        summary.insert(
            "validator_count".to_string(),
            self.validator_count.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        summary.insert(
            "queue_depth".to_string(),
            self.queue_depth.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        summary.insert(
            "concurrent_validations".to_string(),
            self.concurrent_validations.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );

        // Performance optimization metrics
        summary.insert(
            "simd_operations".to_string(),
            self.simd_operations.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        summary.insert(
            "lock_free_operations".to_string(),
            self.lock_free_operations.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        summary.insert(
            "zero_copy_operations".to_string(),
            self.zero_copy_operations.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        summary.insert(
            "parallel_validations".to_string(),
            self.parallel_validations.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
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

                // Prune 24h window for forks
                if let Ok(mut fork_ts) = metrics.fork_timestamps.try_write() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let limit_24h = 86400;
                    fork_ts.retain(|ts| now.saturating_sub(*ts) < limit_24h);
                    metrics
                        .forks_last_24h
                        .store(fork_ts.len() as u64, Ordering::Relaxed);
                }

                // Prune 24h window for tip count
                if let Ok(mut tip_history) = metrics.tip_count_timestamps.try_write() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let limit_24h = 86400;
                    tip_history.retain(|(ts, _)| now.saturating_sub(*ts) < limit_24h);
                    let max_tips = tip_history
                        .iter()
                        .map(|(_, count)| *count)
                        .max()
                        .unwrap_or(0);
                    metrics.tip_count_max_24h.store(max_tips, Ordering::Relaxed);
                }

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
        output.push_str(&format!(
            "qanto_bps {:.2}\n",
            self.get_bps() as f64 / crate::QANTO_SCALE as f64
        ));
        output.push_str(&format!(
            "qanto_tps {:.2}\n",
            self.calculate_real_time_tps() as f64 / crate::QANTO_SCALE as f64
        ));
        output.push_str(&format!(
            "qanto_average_latency_ms {:.2}\n",
            self.get_average_latency() as f64 / crate::QANTO_SCALE as f64
        ));

        // Mining metrics
        output.push_str(&format!(
            "qanto_mining_attempts_total {}\n",
            self.mining_attempts.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_hash_rate_hps {:.2}\n",
            self.get_hash_rate_hps() as f64 / crate::QANTO_SCALE as f64
        ));

        // Resource utilization
        output.push_str(&format!(
            "qanto_memory_utilization_percent {:.2}\n",
            self.get_memory_utilization() as f64 / (crate::QANTO_SCALE as f64 / 100.0)
        ));
        output.push_str(&format!(
            "qanto_rss_bytes {}\n",
            self.qanto_rss_bytes.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_cpu_utilization_percent {:.2}\n",
            self.get_cpu_utilization() as f64 / (crate::QANTO_SCALE as f64 / 100.0)
        ));
        output.push_str(&format!(
            "qanto_network_throughput_mbps {:.2}\n",
            self.get_network_throughput() as f64 / crate::QANTO_SCALE as f64
        ));

        // Performance metrics
        output.push_str(&format!(
            "qanto_cache_hit_ratio {:.4}\n",
            self.get_cache_hit_ratio() as f64 / crate::QANTO_SCALE as f64
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
        output.push_str(&format!(
            "qanto_total_value_locked {}\n",
            *self.total_value_locked.blocking_read()
        ));
        output.push_str(&format!(
            "qanto_validator_rewards_24h {}\n",
            *self.validator_rewards_24h.blocking_read()
        ));
        output.push_str(&format!(
            "qanto_transaction_fees_24h {}\n",
            *self.transaction_fees_24h.blocking_read()
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

        // Persistence metrics
        output.push_str(&format!(
            "qanto_persistence_batches_total {}\n",
            self.persistence_batches.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_persistence_last_batch_ops {}\n",
            self.persistence_last_batch_ops.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_persistence_last_batch_bytes {}\n",
            self.persistence_last_batch_bytes.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_persistence_overflows_total {}\n",
            self.persistence_overflows.load(Ordering::Relaxed)
        ));

        // Cohort A Prometheus metrics
        output.push_str(&format!(
            "qanto_parallel_tips_total {}\n",
            self.parallel_tips_total.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_fork_events_total {}\n",
            self.fork_events_total.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_forks_last_24h {}\n",
            self.forks_last_24h.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_max_fork_depth {}\n",
            self.max_fork_depth.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_last_fork_timestamp {}\n",
            self.last_fork_timestamp.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_height_divergence_events {}\n",
            self.height_divergence_events.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_chain_reconciliation_events {}\n",
            self.chain_reconciliation_events.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_unique_peers_seen_24h {}\n",
            self.unique_peers_seen_24h.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_peer_session_duration_p50 {}\n",
            self.peer_session_duration_p50.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_peer_session_duration_p95 {}\n",
            self.peer_session_duration_p95.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_peer_disconnects_24h {}\n",
            self.peer_disconnects_24h.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_tip_count {}\n",
            self.tip_count.load(Ordering::Relaxed)
        ));
        output.push_str(&format!(
            "qanto_tip_count_max_24h {}\n",
            self.tip_count_max_24h.load(Ordering::Relaxed)
        ));

        output
    }

    /// Add hash rate getter/setter and computation helpers
    /// Compute hash rate in H/s given the number of attempts over an interval in milliseconds
    /// Compute hash rate from attempts and interval - Scaled by 1e9
    pub fn compute_hash_rate(attempts_delta: u64, interval_ms: u64) -> u128 {
        if interval_ms == 0 {
            return 0;
        }
        (attempts_delta as u128 * crate::QANTO_SCALE * 1000) / interval_ms as u128
    }

    /// Format hash rate into human-friendly units (H/s, kH/s, MH/s, GH/s, TH/s, PH/s) - Input scaled by 1e9
    pub fn format_hash_rate(hps_scaled: u128) -> String {
        let units = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s"];
        let mut unit_idx = 0;
        let mut value = hps_scaled;
        while value >= 1000 * crate::QANTO_SCALE && unit_idx < units.len() - 1 {
            value /= 1000;
            unit_idx += 1;
        }
        let whole = value / crate::QANTO_SCALE;
        let fraction = (value % crate::QANTO_SCALE) / (crate::QANTO_SCALE / 100); // 2 decimals
        format!("{}.{:02} {}", whole, fraction, units[unit_idx])
    }

    /// Get the current hash rate in H/s - Scaled by 1e9
    pub fn get_hash_rate_hps(&self) -> u128 {
        self.hash_rate_thousandths.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set the current hash rate in H/s (input value is scaled by 1e9)
    pub fn set_hash_rate_hps(&self, value_scaled: u128) {
        self.hash_rate_thousandths.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
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

    /// Get storage efficiency as fixed-point - Scaled by 1e9
    pub fn get_storage_efficiency(&self) -> u128 {
        self.storage_efficiency.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set storage efficiency from fixed-point (input value is scaled by 1e9)
    pub fn set_storage_efficiency(&self, value_scaled: u128) {
        self.storage_efficiency.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get SAGA throughput as fixed-point - Scaled by 1e9
    pub fn get_saga_throughput(&self) -> u128 {
        self.throughput.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set SAGA throughput from fixed-point (input value is scaled by 1e9)
    pub fn set_saga_throughput(&self, value_scaled: u128) {
        self.throughput.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get SAGA latency as fixed-point - Scaled by 1e9
    pub fn get_saga_latency(&self) -> u128 {
        self.latency.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set SAGA latency from fixed-point (input value is scaled by 1e9)
    pub fn set_saga_latency(&self, value_scaled: u128) {
        self.latency.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get error rate as fixed-point - Scaled by 1e9
    pub fn get_error_rate(&self) -> u128 {
        self.error_rate.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set error rate from fixed-point (input value is scaled by 1e9)
    pub fn set_error_rate(&self, value_scaled: u128) {
        self.error_rate.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get resource utilization as fixed-point - Scaled by 1e9
    pub fn get_resource_utilization(&self) -> u128 {
        self.resource_utilization.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set resource utilization from fixed-point (input value is scaled by 1e9)
    pub fn set_resource_utilization(&self, value_scaled: u128) {
        self.resource_utilization.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get security score as fixed-point - Scaled by 1e9
    pub fn get_security_score(&self) -> u128 {
        self.security_score.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set security score from fixed-point (input value is scaled by 1e9)
    pub fn set_security_score(&self, value_scaled: u128) {
        self.security_score.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Get stability index as fixed-point - Scaled by 1e9
    pub fn get_stability_index(&self) -> u128 {
        self.stability_index.load(Ordering::Relaxed) as u128 * (crate::QANTO_SCALE / 1000)
    }

    /// Set stability index from fixed-point (input value is scaled by 1e9)
    pub fn set_stability_index(&self, value_scaled: u128) {
        self.stability_index.store(
            (value_scaled / (crate::QANTO_SCALE / 1000)) as u64,
            Ordering::Relaxed,
        );
        self.touch();
    }

    /// Increment blocks processed counter
    pub async fn get_total_value_locked(&self) -> u128 {
        *self.total_value_locked.read().await
    }

    pub async fn set_total_value_locked(&self, value: u128) {
        *self.total_value_locked.write().await = value;
        self.touch();
    }

    pub async fn add_total_value_locked(&self, delta: u128) {
        let mut lock = self.total_value_locked.write().await;
        *lock = lock.saturating_add(delta);
        self.touch();
    }

    pub async fn get_validator_rewards_24h(&self) -> u128 {
        *self.validator_rewards_24h.read().await
    }

    pub async fn set_validator_rewards_24h(&self, value: u128) {
        *self.validator_rewards_24h.write().await = value;
        self.touch();
    }

    pub async fn get_transaction_fees_24h(&self) -> u128 {
        *self.transaction_fees_24h.read().await
    }

    pub async fn set_transaction_fees_24h(&self, value: u128) {
        *self.transaction_fees_24h.write().await = value;
        self.touch();
    }

    pub async fn add_transaction_fees_24h(&self, delta: u128) {
        let mut lock = self.transaction_fees_24h.write().await;
        *lock = lock.saturating_add(delta);
        self.touch();
    }

    pub async fn add_validator_rewards_24h(&self, delta: u128) {
        let mut lock = self.validator_rewards_24h.write().await;
        *lock = lock.saturating_add(delta);
        self.touch();
    }

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

    /// Get cache hit ratio - Scaled by 1e9 (e.g. 1.0 = 1e9)
    pub fn get_cache_hit_ratio(&self) -> u128 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as u128;
        let misses = self.cache_misses.load(Ordering::Relaxed) as u128;
        let total = hits + misses;
        if total > 0 {
            (hits * crate::QANTO_SCALE) / total
        } else {
            0
        }
    }

    /// Export metrics as a HashMap for external systems
    /// Export all metrics as a HashMap - All values scaled by 1e9
    pub fn export_metrics(&self) -> HashMap<String, u128> {
        let mut metrics = HashMap::new();

        metrics.insert(
            "blocks_processed".to_string(),
            self.blocks_processed.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "transactions_processed".to_string(),
            self.transactions_processed.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "validation_time_ms".to_string(),
            self.validation_time_ms.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "block_creation_time_ms".to_string(),
            self.block_creation_time_ms.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "cache_hits".to_string(),
            self.cache_hits.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "cache_misses".to_string(),
            self.cache_misses.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert("cache_hit_ratio".to_string(), self.get_cache_hit_ratio());
        metrics.insert("tps".to_string(), self.get_tps());
        metrics.insert(
            "finality_ms".to_string(),
            self.get_finality_ms() as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "rss_bytes".to_string(),
            self.get_rss_bytes() as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "validator_count".to_string(),
            self.validator_count.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
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

        // Scaled Cohort A metrics
        metrics.insert(
            "parallel_tips_total".to_string(),
            self.parallel_tips_total.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "fork_events_total".to_string(),
            self.fork_events_total.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "forks_last_24h".to_string(),
            self.forks_last_24h.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "max_fork_depth".to_string(),
            self.max_fork_depth.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "last_fork_timestamp".to_string(),
            self.last_fork_timestamp.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "height_divergence_events".to_string(),
            self.height_divergence_events.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "chain_reconciliation_events".to_string(),
            self.chain_reconciliation_events.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "unique_peers_seen_24h".to_string(),
            self.unique_peers_seen_24h.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "peer_session_duration_p50".to_string(),
            self.peer_session_duration_p50.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "peer_session_duration_p95".to_string(),
            self.peer_session_duration_p95.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "peer_disconnects_24h".to_string(),
            self.peer_disconnects_24h.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "tip_count".to_string(),
            self.tip_count.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );
        metrics.insert(
            "tip_count_max_24h".to_string(),
            self.tip_count_max_24h.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE,
        );

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

        self.parallel_tips_total.store(0, Ordering::Relaxed);
        self.fork_events_total.store(0, Ordering::Relaxed);
        self.forks_last_24h.store(0, Ordering::Relaxed);
        self.max_fork_depth.store(0, Ordering::Relaxed);
        self.last_fork_timestamp.store(0, Ordering::Relaxed);
        self.height_divergence_events.store(0, Ordering::Relaxed);
        self.chain_reconciliation_events.store(0, Ordering::Relaxed);
        self.unique_peers_seen_24h.store(0, Ordering::Relaxed);
        self.peer_session_duration_p50.store(0, Ordering::Relaxed);
        self.peer_session_duration_p95.store(0, Ordering::Relaxed);
        self.peer_disconnects_24h.store(0, Ordering::Relaxed);
        self.tip_count.store(0, Ordering::Relaxed);
        self.tip_count_max_24h.store(0, Ordering::Relaxed);
        if let Ok(mut fork_ts) = self.fork_timestamps.try_write() {
            fork_ts.clear();
        }
        if let Ok(mut tip_history) = self.tip_count_timestamps.try_write() {
            tip_history.clear();
        }

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

mod rwlock_u128_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    pub fn serialize<S>(val: &Arc<RwLock<u128>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // For serialization, we just do a blocking read or 0 if we can't
        // Since this is async RwLock, blocking read is tricky in sync context.
        // As a workaround, we'll try_read or default to 0
        let v = match val.try_read() {
            Ok(guard) => *guard,
            Err(_) => 0,
        };
        // u128 serialization isn't always supported by all formats, but we'll use it
        serializer.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<RwLock<u128>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        let val = s.parse::<u128>().map_err(serde::de::Error::custom)?;
        Ok(Arc::new(RwLock::new(val)))
    }
}

mod rwlock_vec_u64_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    pub fn serialize<S>(val: &Arc<RwLock<Vec<u64>>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let v = match val.try_read() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        };
        serde::Serialize::serialize(&v, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<RwLock<Vec<u64>>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = <Vec<u64> as Deserialize>::deserialize(deserializer)?;
        Ok(Arc::new(RwLock::new(v)))
    }
}

mod rwlock_vec_tuple_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    pub fn serialize<S>(
        val: &Arc<RwLock<Vec<(u64, u64)>>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let v = match val.try_read() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        };
        serde::Serialize::serialize(&v, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<RwLock<Vec<(u64, u64)>>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = <Vec<(u64, u64)> as Deserialize>::deserialize(deserializer)?;
        Ok(Arc::new(RwLock::new(v)))
    }
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
        // 1000 attempts over 1 second -> 1000 H/s (scaled by 1e9)
        assert_eq!(
            QantoMetrics::compute_hash_rate(1000, 1000),
            1000 * crate::QANTO_SCALE
        );
        // 500 attempts over 2 seconds -> 250 H/s (scaled by 1e9)
        assert_eq!(
            QantoMetrics::compute_hash_rate(500, 2000),
            250 * crate::QANTO_SCALE
        );
    }

    #[test]
    fn test_format_hash_rate_units() {
        assert_eq!(QantoMetrics::format_hash_rate(0), "0.00 H/s");
        assert_eq!(
            QantoMetrics::format_hash_rate(999 * crate::QANTO_SCALE),
            "999.00 H/s"
        );
        assert_eq!(
            QantoMetrics::format_hash_rate(1000 * crate::QANTO_SCALE),
            "1.00 kH/s"
        );
        assert_eq!(
            QantoMetrics::format_hash_rate(12_345 * crate::QANTO_SCALE),
            "12.34 kH/s"
        );
        assert_eq!(
            QantoMetrics::format_hash_rate(1_000_000 * crate::QANTO_SCALE),
            "1.00 MH/s"
        );
        assert_eq!(
            QantoMetrics::format_hash_rate(12_345_678 * crate::QANTO_SCALE),
            "12.34 MH/s"
        );
        assert_eq!(
            QantoMetrics::format_hash_rate(1_234_567_890 * crate::QANTO_SCALE),
            "1.23 GH/s"
        );
    }
}
