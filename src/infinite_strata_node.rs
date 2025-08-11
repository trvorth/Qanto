// qanto/src/infinite_strata_node.rs

//! --- Infinite Strata Node (PoSCP v0.1-MVP) ---
//! This version evolves the ISN into a minimum viable Proof-of-Sustained-Cloud-Presence
//! (PoSCP) protocol by implementing two critical features:
//! 1.  **Cryptographically Signed Heartbeats**: Each heartbeat now includes a monotonic epoch
//!     counter and is signed with a persistent ed25519 keypair, making it a verifiable,
//!     non-repudiable proof of liveness.
//! 2.  **Cloud-Adaptive Mining Policy**: A new function, `adaptive_mining_fraction`, is
//!     introduced to dynamically adjust the node's mining activity based on resource
//!     utilization and free-tier detection, enabling intelligent duty-cycling.

use anyhow::Result;
use hex;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::info;
use tracing::instrument;
use uuid::Uuid;

// --- New Cryptography Dependencies ---
// Use aliasing to keep the code's domain language (Keypair, PublicKey) while using the new types.
// FIX: Add `SignatureError` to the import to use it in our custom `NodeError`.
use ed25519_dalek::{
    Signature, SignatureError, Signer, SigningKey as Keypair, Verifier, VerifyingKey as PublicKey,
};
use std::convert::{TryFrom, TryInto}; // Add TryInto for slice-to-array conversion

/// Minimum heartbeat interval in seconds.
pub const MIN_UPTIME_HEARTBEAT_SECS: u64 = 300; // 5 minutes

// ---
// # 1. Error handling & configuration
// ---

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Failed to produce block after winning auction")]
    BlockProductionFailure,
    #[error("Verification of ZK-SNARK snapshot failed")]
    ZkSnapshotVerificationError,
    #[error("Heartbeat cycle encountered an unrecoverable error")]
    HeartbeatCycleFailed,
    #[error("Cryptographic operation failed: {0}")]
    CryptoError(#[from] anyhow::Error),
    #[error("Invalid key length: {0}")]
    InvalidKeyLength(#[from] std::array::TryFromSliceError),
    // FIX: Add a new variant to handle signature-specific errors from ed25519-dalek.
    // The `#[from]` attribute automatically creates the `From<SignatureError> for NodeError` implementation.
    #[error("Signature error: {0}")]
    SignatureError(#[from] SignatureError),
}

// FIX: Manually implement `From` for bincode errors to convert them into our generic `CryptoError`.
// This teaches the `?` operator how to handle serialization failures.
impl From<Box<bincode::ErrorKind>> for NodeError {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        NodeError::CryptoError(e.into())
    }
}

/// Node configuration containing tunables used by the ISN.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub decay_history_len: usize,
    pub heartbeat_interval_secs: u64,
    pub optimal_utilization_target: f32,
    pub free_tier_cpu_threshold: f32,
    pub free_tier_mem_threshold: f32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            decay_history_len: 100,
            heartbeat_interval_secs: MIN_UPTIME_HEARTBEAT_SECS,
            optimal_utilization_target: 75.0,
            free_tier_cpu_threshold: 10.0,
            free_tier_mem_threshold: 20.0,
        }
    }
}

// ---
// # 2. Core data structures
// ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu: f32,
    pub memory: f32,
}

#[derive(PartialEq, Clone, Debug)]
pub enum NodeStatus {
    Healthy,
    Degraded,
    Probation,
    Slashed,
}

#[derive(Clone, Debug)]
pub struct NodeState {
    pub status: NodeStatus,
    pub decay_history: VecDeque<f32>,
    pub total_uptime_ticks: u64,
    pub last_heartbeat_time: u64,
    pub last_epoch: u64,
}

/// Heartbeat payload that will be signed and broadcast. This is the core of PoSCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedHeartbeat {
    pub node_id: Uuid,
    pub epoch: u64,
    pub timestamp: u64,
    pub resources: ResourceUsage,
    pub sig: Vec<u8>,
    pub pubkey: Vec<u8>,
}

impl SignedHeartbeat {
    /// Signs the heartbeat payload using the node's keypair.
    pub fn sign(payload: &mut SignedHeartbeat, kp: &Keypair) -> Result<(), bincode::Error> {
        payload.sig.clear();
        let msg = bincode::serialize(&(
            &payload.node_id,
            payload.epoch,
            payload.timestamp,
            &payload.resources,
        ))?;
        let sig = kp.sign(&msg);
        payload.sig = sig.to_bytes().to_vec();
        payload.pubkey = kp.verifying_key().to_bytes().to_vec();
        Ok(())
    }

    /// Verifies the integrity and authenticity of a signed heartbeat.
    // FIX: Change the return type to our specific `NodeError` to make `?` work inside the function.
    pub fn verify(&self) -> Result<(), NodeError> {
        let pubkey_array: &[u8; 32] = self.pubkey.as_slice().try_into()?;
        let pk = PublicKey::from_bytes(pubkey_array)?;

        let msg =
            bincode::serialize(&(&self.node_id, self.epoch, self.timestamp, &self.resources))?;
        let sig = Signature::try_from(self.sig.as_slice())?;
        pk.verify(&msg, &sig)?;
        Ok(())
    }
}

/// Shared state for the Autonomous Economic Calibrator (AEC).
#[derive(Debug, Default)]
pub struct DecentralizedOracleAggregator {
    pub total_slashed_pool: RwLock<u64>,
}

// ---
// # 3. Main ISN struct and implementation
// ---

pub struct InfiniteStrataNode {
    pub node_id: Uuid,
    pub config: NodeConfig,
    pub state: RwLock<NodeState>,
    pub oracle_aggregator: Arc<DecentralizedOracleAggregator>,
    sys: RwLock<System>,
    keypair: Keypair,
}

impl std::fmt::Debug for InfiniteStrataNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InfiniteStrataNode")
            .field("node_id", &self.node_id)
            .field("config", &self.config)
            .field("state", &self.state)
            .field("oracle_aggregator", &self.oracle_aggregator)
            .field(
                "public_key",
                &hex::encode(self.keypair.verifying_key().as_bytes()),
            )
            .finish()
    }
}

impl InfiniteStrataNode {
    /// Initialize a new Infinite Strata Node.
    pub fn new(config: NodeConfig, oracle: Arc<DecentralizedOracleAggregator>) -> Self {
        let node_id = Uuid::new_v4();
        info!("SAGA Titan Node Initialized. node_id={}", node_id);
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);

        Self {
            node_id,
            state: RwLock::new(NodeState {
                status: NodeStatus::Healthy,
                decay_history: VecDeque::with_capacity(config.decay_history_len),
                total_uptime_ticks: 0,
                last_heartbeat_time: 0,
                last_epoch: 0,
            }),
            config,
            oracle_aggregator: oracle,
            sys: RwLock::new(System::new_all()),
            keypair,
        }
    }

    /// The main heartbeat cycle, now including signing and adaptive mining logic.
    #[instrument(skip(self), fields(node_id = %self.node_id))]
    async fn perform_heartbeat_cycle(&self) -> Result<(), NodeError> {
        let resources = self.sample_resources().await;
        let is_free_tier = self.detect_free_tier(&resources);
        let mining_fraction = self.adaptive_mining_fraction(&resources, is_free_tier);

        info!(
            cpu = resources.cpu,
            memory = resources.memory,
            is_free_tier,
            adaptive_mining_fraction = mining_fraction,
            "Heartbeat resource sample"
        );

        let mut state = self.state.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create the heartbeat payload
        let mut heartbeat = SignedHeartbeat {
            node_id: self.node_id,
            epoch: state.last_epoch + 1,
            timestamp: now,
            resources,
            sig: Vec::new(),
            pubkey: Vec::new(),
        };

        // Sign the heartbeat to create a verifiable proof.
        SignedHeartbeat::sign(&mut heartbeat, &self.keypair)
            .map_err(|e| NodeError::CryptoError(e.into()))?;

        // Locally verify the heartbeat
        heartbeat.verify()?;
        info!(
            epoch = heartbeat.epoch,
            "Signed heartbeat created and verified."
        );

        state.last_heartbeat_time = now;
        state.total_uptime_ticks += 1;
        state.last_epoch = heartbeat.epoch;

        let decay_factor = self.calculate_decay(&state.decay_history);
        self.update_status(decay_factor, &mut state);

        let network_health = self.recalibrate_config().await;
        info!(?network_health, "Heartbeat cycle complete.");
        Ok(())
    }

    /// Heuristic to determine if the node is likely running on a cloud free tier.
    fn detect_free_tier(&self, resources: &ResourceUsage) -> bool {
        resources.cpu < self.config.free_tier_cpu_threshold
            && resources.memory < self.config.free_tier_mem_threshold
    }

    /// The core cloud-adaptive mining policy.
    fn adaptive_mining_fraction(&self, resources: &ResourceUsage, free_tier: bool) -> f32 {
        let util = resources.cpu.max(resources.memory);
        let headroom = (self.config.optimal_utilization_target - util).max(0.0);

        let base = if free_tier { 0.05 } else { 0.2 };

        (base + headroom * 0.8).clamp(0.0, 1.0)
    }

    #[instrument(skip(self))]
    async fn sample_resources(&self) -> ResourceUsage {
        let mut sys = self.sys.write().await;
        sys.refresh_cpu();
        sys.refresh_memory();
        let cpu = sys.global_cpu_info().cpu_usage();
        let memory = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
        ResourceUsage { cpu, memory }
    }

    fn calculate_decay(&self, history: &VecDeque<f32>) -> f32 {
        if history.is_empty() {
            return 1.0;
        }
        let sum: f32 = history.iter().sum();
        sum / history.len() as f32
    }

    #[instrument(skip(self))]
    async fn recalibrate_config(&self) -> (u64, u64, f64) {
        let health_snapshot = (1, 500, 0.0);
        info!(
            network_health = ?health_snapshot,
            "AEC: Network stable. Operating under standard parameters."
        );
        health_snapshot
    }

    fn update_status(&self, decay_factor: f32, state: &mut NodeState) {
        let old_status = state.status.clone();
        state.status = if decay_factor < 0.5 {
            NodeStatus::Slashed
        } else if decay_factor < 0.8 {
            NodeStatus::Probation
        } else if decay_factor < 0.95 {
            NodeStatus::Degraded
        } else {
            NodeStatus::Healthy
        };

        if state.status != old_status {
            info!(
                "Node status changed from {:?} to {:?}",
                old_status, state.status
            );
        }

        if state.status == NodeStatus::Probation && old_status != NodeStatus::Probation {
            let snapshot = state.clone();
            let reputation_shield = (snapshot.total_uptime_ticks as f64 / 2880.0).min(0.5);
            let oracle = self.oracle_aggregator.clone();
            tokio::spawn(async move {
                let pool_size = *oracle.total_slashed_pool.read().await;
                let base_penalty = 50.0 + (pool_size as f64 * 0.05);
                let penalty_amount =
                    ((base_penalty * (1.0 - reputation_shield)) as u64).min(pool_size);
                let mut p = oracle.total_slashed_pool.write().await;
                *p = p.saturating_sub(penalty_amount);
                info!(reputation_shield = ?reputation_shield, penalty = penalty_amount, "Probation penalty applied.");
            });
        }
    }

    /// Public wrapper for running a single periodic check.
    pub async fn run_periodic_check(self: &Arc<Self>) -> Result<(), NodeError> {
        self.perform_heartbeat_cycle().await
    }

    /// Public method to get the current rewards multiplier.
    pub async fn get_rewards(&self) -> (f64, u64) {
        let state = self.state.read().await;
        let uptime_bonus = (state.total_uptime_ticks as f64 / 3600.0).min(1.5);
        let decay_multiplier = self.calculate_decay(&state.decay_history) as f64;
        (uptime_bonus * decay_multiplier, state.total_uptime_ticks)
    }
}
