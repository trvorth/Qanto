// src/decentralization.rs

//! --- Qanto Decentralization Engine ---
//! v0.1.0 - Initial Version
//!
//! This module implements comprehensive decentralization features including:
//! - Distributed consensus mechanisms
//! - Peer-to-peer node discovery and management
//! - Decentralized governance and voting
//! - Validator selection and rotation
//! - Network topology optimization
//! - Byzantine fault tolerance
//! - Shard coordination and cross-shard communication

use crate::consensus::{Consensus, ConsensusError};
use crate::metrics::QantoMetrics;

use crate::qanto_net::{NetworkMessage, PeerId, QantoNetServer};
use crate::qantodag::{QantoBlock as DagBlock, QantoDAG};
use qanto_core::qanto_net as core_net;

fn to_core_block(block: &DagBlock) -> core_net::QantoBlock {
    core_net::QantoBlock {
        id: block.id.clone(),
        data: serde_json::to_vec(block).unwrap_or_default(),
        ..Default::default()
    }
}
use crate::saga::{GovernanceProposal, PalletSaga};
use crate::transaction::Transaction;
use crate::zkp::{ZKProof, ZKProofSystem};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

// --- Decentralization Constants ---
const MIN_VALIDATOR_COUNT: usize = 21;
const VALIDATOR_ROTATION_EPOCHS: u64 = 100;
const BYZANTINE_FAULT_TOLERANCE: u128 = 333_333_333; // 33.3% as 0.333333333 * 1e9
const CONSENSUS_TIMEOUT_MS: u64 = 30000;
const SHARD_COUNT: usize = 64;

// Diversity and selection constants
const MAX_STAKE_CONCENTRATION: u128 = 400_000_000;
const PERFORMANCE_WEIGHT: u128 = 400_000_000;
const DIVERSITY_WEIGHT: u128 = 300_000_000;
const STAKE_WEIGHT: u128 = 300_000_000;

#[derive(Error, Debug)]
pub enum DecentralizationError {
    #[error("Insufficient validators: {0} < {1}")]
    InsufficientValidators(usize, usize),
    #[error("Byzantine fault detected: {0}")]
    ByzantineFault(String),
    #[error("Consensus timeout exceeded")]
    ConsensusTimeout,
    #[error("Invalid validator signature: {0}")]
    InvalidValidatorSignature(String),
    #[error("Governance proposal not found: {0}")]
    ProposalNotFound(String),
    #[error("Insufficient voting power: {0} < {1}")]
    InsufficientVotingPower(u128, u128),
    #[error("Shard coordination failed: {0}")]
    ShardCoordinationFailed(String),
    #[error("Network partition detected")]
    NetworkPartition,
    #[error("Peer discovery failed: {0}")]
    PeerDiscoveryFailed(String),
    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub peer_id: PeerId,
    pub stake: u128,
    pub reputation_score: u128,
    pub last_active_epoch: u64,
    pub performance_metrics: ValidatorMetrics,
    pub public_key: Vec<u8>,
    pub network_address: String,
    pub shard_assignments: Vec<usize>,
}

// Use unified metrics system
pub type ValidatorMetrics = QantoMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusRound {
    pub round_id: String,
    pub epoch: u64,
    pub proposed_block: Option<DagBlock>,
    pub validator_votes: HashMap<PeerId, ValidatorVote>,
    pub status: ConsensusStatus,
    pub start_time: u64,
    pub timeout_time: u64,
    pub shard_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorVote {
    pub validator_id: PeerId,
    pub block_hash: String,
    pub vote_type: VoteType,
    pub signature: Vec<u8>,
    pub timestamp: u64,
    #[cfg(feature = "zk")]
    pub zk_proof: Option<ZKProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteType {
    Approve,
    Reject,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusStatus {
    Proposing,
    Voting,
    Finalizing,
    Completed,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    pub shard_id: usize,
    pub validators: Vec<PeerId>,
    pub current_leader: Option<PeerId>,
    pub transaction_pool: Vec<Transaction>,
    pub state_root: String,
    pub last_block_hash: String,
    pub cross_shard_links: HashMap<usize, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossShardMessage {
    pub message_id: String,
    pub source_shard: usize,
    pub target_shard: usize,
    pub message_type: CrossShardMessageType,
    pub payload: Vec<u8>,
    pub timestamp: u64,
    #[cfg(feature = "zk")]
    pub proof: Option<ZKProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossShardMessageType {
    TransactionTransfer,
    StateUpdate,
    ValidatorRotation,
    ConsensusSync,
    GovernanceUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTopology {
    pub nodes: HashMap<PeerId, NodeInfo>,
    pub connections: HashMap<PeerId, Vec<PeerId>>,
    pub shards: HashMap<usize, ShardInfo>,
    pub partition_detection: PartitionDetector,
    pub load_balancer: LoadBalancer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub peer_id: PeerId,
    pub node_type: NodeType,
    pub capabilities: NodeCapabilities,
    pub network_metrics: NetworkMetrics,
    pub last_seen: u64,
    pub trust_score: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Validator,
    FullNode,
    LightClient,
    ArchiveNode,
    Bridge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub can_validate: bool,
    pub can_store_full_state: bool,
    pub can_bridge_shards: bool,
    pub supports_zkp: bool,
    pub max_connections: usize,
    pub bandwidth_mbps: u128, // Scaled by 1e9
}

// Use unified metrics system
pub type NetworkMetrics = QantoMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionDetector {
    pub suspected_partitions: Vec<NetworkPartition>,
    pub detection_threshold: u128, // Scaled by 1e9
    pub recovery_strategies: Vec<RecoveryStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPartition {
    pub partition_id: String,
    pub affected_nodes: Vec<PeerId>,
    pub detected_at: u64,
    pub severity: PartitionSeverity,
    pub recovery_status: RecoveryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionSeverity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStatus {
    Detected,
    Recovering,
    Recovered,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStrategy {
    pub strategy_id: String,
    pub strategy_type: RecoveryStrategyType,
    pub success_rate: u128,          // Scaled by 1e9
    pub estimated_recovery_time: u64, // ms
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategyType {
    AutomaticReconnection,
    ValidatorRebalancing,
    ShardReorganization,
    EmergencyConsensus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancer {
    pub load_distribution: HashMap<PeerId, u128>,
    pub balancing_strategy: BalancingStrategy,
    pub rebalancing_threshold: u128,
    pub performance_weights: HashMap<String, u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    PerformanceBased,
    GeographicProximity,
}

/// Main decentralization engine coordinating all distributed operations
#[derive(Debug)]
pub struct DecentralizationEngine {
    pub validators: Arc<RwLock<HashMap<PeerId, ValidatorInfo>>>,
    pub consensus_rounds: Arc<RwLock<HashMap<String, ConsensusRound>>>,
    pub network_topology: Arc<RwLock<NetworkTopology>>,
    pub governance_system: Arc<RwLock<DecentralizedGovernance>>,
    pub shard_coordinator: Arc<ShardCoordinator>,
    pub peer_discovery: Arc<PeerDiscoveryService>,
    pub consensus_engine: Arc<Consensus>,
    pub saga: Arc<PalletSaga>,
    #[cfg(feature = "zk")]
    pub zkp_system: Arc<ZKProofSystem>,
    pub network_server: Arc<QantoNetServer>,
}

#[derive(Debug)]
pub struct DecentralizedGovernance {
    pub active_proposals: HashMap<String, GovernanceProposal>,
    pub voting_records: HashMap<String, VotingRecord>,
    pub delegation_system: DelegationSystem,
    pub proposal_queue: VecDeque<String>,
    pub governance_metrics: GovernanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingRecord {
    pub proposal_id: String,
    pub voter_id: PeerId,
    pub vote: VoteChoice,
    pub voting_power: u128,
    pub timestamp: u64,
    #[cfg(feature = "zk")]
    pub zk_proof: Option<ZKProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationSystem {
    pub delegations: HashMap<PeerId, PeerId>, // delegator -> delegate
    pub delegation_weights: HashMap<PeerId, u128>,
    pub delegation_history: Vec<DelegationEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationEvent {
    pub delegator: PeerId,
    pub delegate: PeerId,
    pub action: DelegationAction,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegationAction {
    Delegate,
    Undelegate,
    Redelegate(PeerId), // new delegate
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceMetrics {
    pub total_proposals: u64,
    pub active_proposals: u64,
    pub passed_proposals: u64,
    pub rejected_proposals: u64,
    pub average_participation_rate: u128,
    pub delegation_rate: u128,
    pub governance_health_score: u128,
}

#[derive(Debug)]
pub struct ShardCoordinator {
    pub shards: Arc<RwLock<HashMap<usize, ShardInfo>>>,
    pub cross_shard_messages: Arc<RwLock<VecDeque<CrossShardMessage>>>,
    pub shard_assignments: Arc<RwLock<HashMap<PeerId, Vec<usize>>>>,
    pub load_balancer: Arc<RwLock<LoadBalancer>>,
}

#[derive(Debug)]
pub struct PeerDiscoveryService {
    pub known_peers: Arc<RwLock<HashMap<PeerId, NodeInfo>>>,
    pub discovery_protocols: Vec<DiscoveryProtocol>,
    pub bootstrap_nodes: Vec<PeerId>,
    pub discovery_metrics: Arc<RwLock<DiscoveryMetrics>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryProtocol {
    DHT,
    Gossip,
    Bootstrap,
    MDns,
    PeerExchange,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryMetrics {
    pub peers_discovered: u64,
    pub successful_connections: u64,
    pub failed_connections: u64,
    pub average_discovery_time: u128, // Scaled by 1e9
    pub network_coverage: u128,       // Scaled by 1e9
}

impl DecentralizationEngine {
    pub fn new(
        consensus_engine: Arc<Consensus>,
        saga: Arc<PalletSaga>,
        #[cfg(feature = "zk")] zkp_system: Arc<ZKProofSystem>,
        network_server: Arc<QantoNetServer>,
    ) -> Self {
        let shard_coordinator = Arc::new(ShardCoordinator::new());
        let peer_discovery = Arc::new(PeerDiscoveryService::new());

        Self {
            validators: Arc::new(RwLock::new(HashMap::new())),
            consensus_rounds: Arc::new(RwLock::new(HashMap::new())),
            network_topology: Arc::new(RwLock::new(NetworkTopology::new())),
            governance_system: Arc::new(RwLock::new(DecentralizedGovernance::new())),
            shard_coordinator,
            peer_discovery,
            consensus_engine,
            saga,
            #[cfg(feature = "zk")]
            zkp_system,
            network_server,
        }
    }

    /// Initialize the decentralization engine with bootstrap configuration
    pub async fn initialize(
        &self,
        bootstrap_validators: Vec<ValidatorInfo>,
    ) -> Result<(), DecentralizationError> {
        info!(
            "Initializing decentralization engine with {} bootstrap validators",
            bootstrap_validators.len()
        );

        if bootstrap_validators.len() < MIN_VALIDATOR_COUNT {
            return Err(DecentralizationError::InsufficientValidators(
                bootstrap_validators.len(),
                MIN_VALIDATOR_COUNT,
            ));
        }

        // Initialize validators
        let mut validators = self.validators.write().await;
        for validator in bootstrap_validators {
            validators.insert(validator.peer_id.clone(), validator);
        }
        drop(validators);

        // Initialize shards and assign validators
        self.initialize_shards().await?;

        // Start peer discovery
        self.start_peer_discovery().await?;

        // Initialize governance system
        self.initialize_governance().await?;

        info!("Decentralization engine initialized successfully");
        Ok(())
    }

    /// Initialize shard system and assign validators
    async fn initialize_shards(&self) -> Result<(), DecentralizationError> {
        let validators = self.validators.read().await;
        let validator_count = validators.len();

        if validator_count < SHARD_COUNT {
            warn!(
                "Not enough validators for optimal sharding: {} < {}",
                validator_count, SHARD_COUNT
            );
        }

        let mut shards = self.shard_coordinator.shards.write().await;
        let mut assignments = self.shard_coordinator.shard_assignments.write().await;

        // Create shards
        for shard_id in 0..SHARD_COUNT {
            shards.insert(
                shard_id,
                ShardInfo {
                    shard_id,
                    validators: Vec::new(),
                    current_leader: None,
                    transaction_pool: Vec::new(),
                    state_root: String::new(),
                    last_block_hash: String::new(),
                    cross_shard_links: HashMap::new(),
                },
            );
        }

        // Assign validators to shards using round-robin with load balancing
        let validator_ids: Vec<_> = validators.keys().cloned().collect();
        for (i, validator_id) in validator_ids.iter().enumerate() {
            let shard_id = i % SHARD_COUNT;

            if let Some(shard) = shards.get_mut(&shard_id) {
                shard.validators.push(validator_id.clone());
            }

            assignments
                .entry(validator_id.clone())
                .or_insert_with(Vec::new)
                .push(shard_id);
        }

        info!(
            "Initialized {} shards with validator assignments",
            SHARD_COUNT
        );
        Ok(())
    }

    /// Start peer discovery service
    async fn start_peer_discovery(&self) -> Result<(), DecentralizationError> {
        // Implementation for peer discovery startup
        info!("Starting peer discovery service");

        // Initialize discovery protocols
        let mut metrics = self.peer_discovery.discovery_metrics.write().await;
        *metrics = DiscoveryMetrics::default();

        // Start discovery background tasks
        self.spawn_discovery_tasks().await;

        Ok(())
    }

    /// Initialize governance system
    async fn initialize_governance(&self) -> Result<(), DecentralizationError> {
        let mut governance = self.governance_system.write().await;
        *governance = DecentralizedGovernance::new();

        info!("Governance system initialized");
        Ok(())
    }

    /// Spawn background tasks for peer discovery
    async fn spawn_discovery_tasks(&self) {
        // Implementation for spawning discovery background tasks
        debug!("Spawning peer discovery background tasks");
    }

    /// Start a new consensus round for block validation
    pub async fn start_consensus_round(
        &self,
        block: DagBlock,
        _dag: &Arc<QantoDAG>,
        shard_id: usize,
    ) -> Result<String, DecentralizationError> {
        let round_id = Uuid::new_v4().to_string();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| DecentralizationError::Anyhow(anyhow!("System time error: {e}")))?
            .as_millis() as u64;

        let consensus_round = ConsensusRound {
            round_id: round_id.clone(),
            epoch: block.epoch,
            proposed_block: Some(block.clone()),
            validator_votes: HashMap::new(),
            status: ConsensusStatus::Proposing,
            start_time: current_time,
            timeout_time: current_time + CONSENSUS_TIMEOUT_MS,
            shard_id,
        };

        let mut rounds = self.consensus_rounds.write().await;
        rounds.insert(round_id.clone(), consensus_round);

        info!(
            "Started consensus round {} for shard {}",
            round_id, shard_id
        );

        // Notify validators in the shard
        self.notify_shard_validators(&round_id, shard_id, &block)
            .await?;

        Ok(round_id)
    }

    /// Notify validators in a shard about a new consensus round
    async fn notify_shard_validators(
        &self,
        round_id: &str,
        shard_id: usize,
        block: &DagBlock,
    ) -> Result<(), DecentralizationError> {
        let shards = self.shard_coordinator.shards.read().await;

        if let Some(shard) = shards.get(&shard_id) {
            for validator_id in &shard.validators {
                // Send consensus message to validator
                let _message = NetworkMessage::Consensus {
                    round_id: round_id.to_string(),
                    block: to_core_block(block),
                    shard_id,
                };

                // Use network server to send message
                debug!(
                    "Notifying validator {} about consensus round {}",
                    validator_id, round_id
                );
            }
        }

        Ok(())
    }

    /// Process a validator vote in a consensus round
    pub async fn process_validator_vote(
        &self,
        round_id: &str,
        vote: ValidatorVote,
    ) -> Result<(), DecentralizationError> {
        let mut rounds = self.consensus_rounds.write().await;

        if let Some(round) = rounds.get_mut(round_id) {
            // Verify validator is authorized for this shard
            if !self
                .is_validator_authorized(&vote.validator_id, round.shard_id)
                .await?
            {
                let mut error_msg = String::with_capacity(64);
                error_msg.push_str("Validator ");
                error_msg.push_str(&vote.validator_id.to_string());
                error_msg.push_str(" not authorized for shard ");
                error_msg.push_str(&round.shard_id.to_string());
                return Err(DecentralizationError::InvalidValidatorSignature(error_msg));
            }

            // Verify vote signature and ZK proof if present
            self.verify_validator_vote(&vote).await?;

            // Record the vote
            round
                .validator_votes
                .insert(vote.validator_id.clone(), vote);

            // Check if we have enough votes to finalize
            if self.check_consensus_threshold(round).await? {
                round.status = ConsensusStatus::Finalizing;
                self.finalize_consensus_round(round_id).await?;
            }
        } else {
            return Err(DecentralizationError::Anyhow(anyhow!(
                "Consensus round not found: {round_id}"
            )));
        }

        Ok(())
    }

    /// Check if validator is authorized for a specific shard
    async fn is_validator_authorized(
        &self,
        validator_id: &PeerId,
        shard_id: usize,
    ) -> Result<bool, DecentralizationError> {
        let assignments = self.shard_coordinator.shard_assignments.read().await;

        if let Some(validator_shards) = assignments.get(validator_id) {
            Ok(validator_shards.contains(&shard_id))
        } else {
            Ok(false)
        }
    }

    /// Verify a validator vote's authenticity
    async fn verify_validator_vote(
        &self,
        vote: &ValidatorVote,
    ) -> Result<(), DecentralizationError> {
        let validators = self.validators.read().await;

        if let Some(_validator_info) = validators.get(&vote.validator_id) {
            // Verify signature using validator's public key
            // Implementation would use actual cryptographic verification

            // Verify ZK proof if present
            if let Some(zk_proof) = &vote.zk_proof {
                let verification_result =
                    self.zkp_system.verify_proof(zk_proof).await.map_err(|e| {
                        DecentralizationError::Anyhow(anyhow!("ZK proof verification failed: {e}"))
                    })?;

                if !verification_result {
                    return Err(DecentralizationError::InvalidValidatorSignature(
                        "ZK proof verification failed".to_string(),
                    ));
                }
            }

            Ok(())
        } else {
            // Optimized: Manual string building for error message
            let mut error_msg = String::with_capacity(20 + vote.validator_id.to_string().len());
            error_msg.push_str("Unknown validator: ");
            error_msg.push_str(&vote.validator_id.to_string());
            Err(DecentralizationError::InvalidValidatorSignature(error_msg))
        }
    }

    /// Check if consensus threshold is met
    async fn check_consensus_threshold(
        &self,
        round: &ConsensusRound,
    ) -> Result<bool, DecentralizationError> {
        let shards = self.shard_coordinator.shards.read().await;

        if let Some(shard) = shards.get(&round.shard_id) {
            let total_validators = shard.validators.len();
            let votes_received = round.validator_votes.len();

            // Check if we have enough votes (Byzantine fault tolerance)
            // (total_validators * (1.0 - BYZANTINE_FAULT_TOLERANCE)) ceil
            let bft_factor = crate::QANTO_SCALE - BYZANTINE_FAULT_TOLERANCE;
            let required_votes =
                ((total_validators as u128 * bft_factor + crate::QANTO_SCALE - 1)
                    / crate::QANTO_SCALE) as usize;

            Ok(votes_received >= required_votes)
        } else {
            Err(DecentralizationError::ShardCoordinationFailed(format!(
                "Shard {} not found",
                round.shard_id
            )))
        }
    }

    /// Finalize a consensus round
    async fn finalize_consensus_round(&self, round_id: &str) -> Result<(), DecentralizationError> {
        let mut rounds = self.consensus_rounds.write().await;

        if let Some(round) = rounds.get_mut(round_id) {
            // Count votes
            let mut approve_votes = 0;
            let mut reject_votes = 0;

            for vote in round.validator_votes.values() {
                match vote.vote_type {
                    VoteType::Approve => approve_votes += 1,
                    VoteType::Reject => reject_votes += 1,
                    VoteType::Abstain => {}
                }
            }

            // Determine consensus result
            if approve_votes > reject_votes {
                round.status = ConsensusStatus::Completed;
                info!("Consensus round {} completed successfully", round_id);

                // Apply the block to the shard
                if let Some(block) = &round.proposed_block {
                    self.apply_block_to_shard(block, round.shard_id).await?;
                }
            } else {
                round.status = ConsensusStatus::Failed;
                warn!("Consensus round {} failed", round_id);
            }
        }

        Ok(())
    }

    /// Apply a validated block to a shard
    async fn apply_block_to_shard(
        &self,
        block: &DagBlock,
        shard_id: usize,
    ) -> Result<(), DecentralizationError> {
        let mut shards = self.shard_coordinator.shards.write().await;

        if let Some(shard) = shards.get_mut(&shard_id) {
            shard.last_block_hash = block.id.clone();
            shard.transaction_pool.clear(); // Clear processed transactions

            info!("Applied block {} to shard {}", block.id, shard_id);
        }

        Ok(())
    }

    /// Rotate validators across shards
    #[instrument(skip(self))]
    pub async fn rotate_validators(&self, epoch: u64) -> Result<(), DecentralizationError> {
        if !epoch.is_multiple_of(VALIDATOR_ROTATION_EPOCHS) {
            return Ok(()); // Only rotate at specific intervals
        }

        info!(
            "Starting diversity-based validator rotation for epoch {}",
            epoch
        );

        let validators = self.validators.read().await;
        let validator_infos: Vec<_> = validators.values().cloned().collect();
        drop(validators);

        // Select diverse validators for each shard
        let mut shards = self.shard_coordinator.shards.write().await;
        let mut assignments = self.shard_coordinator.shard_assignments.write().await;

        // Clear current assignments
        for shard in shards.values_mut() {
            shard.validators.clear();
        }
        assignments.clear();

        // Calculate validators per shard
        let validators_per_shard = validator_infos.len().div_ceil(SHARD_COUNT);
        let min_validators_per_shard = validator_infos.len() / SHARD_COUNT;

        // Assign validators to shards using diversity-based selection
        for shard_id in 0..SHARD_COUNT {
            let target_count = if shard_id < validator_infos.len() % SHARD_COUNT {
                validators_per_shard
            } else {
                min_validators_per_shard
            };

            if target_count == 0 {
                continue;
            }

            // Select diverse validators for this shard
            let selected_validators = self
                .select_diverse_validators(&validator_infos, target_count, epoch, shard_id)
                .await?;

            // Assign selected validators to shard
            if let Some(shard) = shards.get_mut(&shard_id) {
                for validator in &selected_validators {
                    shard.validators.push(validator.peer_id.clone());

                    assignments
                        .entry(validator.peer_id.clone())
                        .or_insert_with(Vec::new)
                        .push(shard_id);
                }
            }
        }

        info!("Diversity-based validator rotation completed for epoch {} with enhanced geographic and performance distribution", epoch);
        Ok(())
    }

    /// Select diverse validators based on geographic distribution, performance, and stake
    async fn select_diverse_validators(
        &self,
        all_validators: &[ValidatorInfo],
        target_count: usize,
        epoch: u64,
        shard_id: usize,
    ) -> Result<Vec<ValidatorInfo>, DecentralizationError> {
        if all_validators.len() <= target_count {
            return Ok(all_validators.to_vec());
        }

        let scored_validators = self
            .score_validators(all_validators, epoch, shard_id)
            .await?;
        let selected = self
            .apply_diversity_constraints(scored_validators, target_count)
            .await?;

        Ok(selected)
    }

    /// Score all validators based on performance, stake, diversity, and rotation factors
    async fn score_validators(
        &self,
        all_validators: &[ValidatorInfo],
        epoch: u64,
        shard_id: usize,
    ) -> Result<Vec<(ValidatorInfo, u128)>, DecentralizationError> {
        let mut scored_validators = Vec::new();

        for validator in all_validators {
            let performance_score =
                self.calculate_individual_performance_score(&validator.performance_metrics);
            
            // Deterministic logarithmic stake scoring: (log2(stake) * SCALE) / 30
            // Approximates ln(stake) / 20.0
            let stake_bits = 128 - validator.stake.leading_zeros() as u128;
            let stake_score = (stake_bits * crate::QANTO_SCALE) / 30;

            // Extract geographic info for diversity calculation
            let (country, region, _) = self
                .extract_geographic_info(&validator.network_address)
                .await;

            // Calculate diversity bonus (higher for underrepresented regions)
            let diversity_bonus = self
                .calculate_diversity_bonus(&country, &region, all_validators)
                .await;

            // Combine scores with weights using mul_scale_u128
            let total_score = crate::math::mul_scale_u128(performance_score, PERFORMANCE_WEIGHT)
                + crate::math::mul_scale_u128(stake_score, STAKE_WEIGHT)
                + crate::math::mul_scale_u128(diversity_bonus, DIVERSITY_WEIGHT);

            // Add epoch-based rotation factor to ensure fairness over time
            // rotation_factor = ((epoch + shard_id) * name_len) * SCALE / 1000
            let rotation_factor = (((epoch + shard_id as u64) as u128)
                * (validator.peer_id.to_string().len() as u128)
                * crate::QANTO_SCALE)
                / 1000;
            let final_score = total_score + rotation_factor;

            scored_validators.push((validator.clone(), final_score));
        }

        // Sort by score (descending)
        scored_validators.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(scored_validators)
    }

    /// Apply diversity and stake concentration constraints to validator selection
    async fn apply_diversity_constraints(
        &self,
        scored_validators: Vec<(ValidatorInfo, u128)>,
        target_count: usize,
    ) -> Result<Vec<ValidatorInfo>, DecentralizationError> {
        let mut selected = Vec::new();
        let mut geographic_counts: HashMap<String, usize> = HashMap::new();
        let mut used_validators = HashSet::new();

        // First pass: apply constraints
        for (validator, _score) in &scored_validators {
            if selected.len() >= target_count {
                break;
            }

            if self
                .validator_meets_constraints(validator, &selected, &geographic_counts, target_count)
                .await?
            {
                let (country, _, _) = self
                    .extract_geographic_info(&validator.network_address)
                    .await;

                selected.push(validator.clone());
                used_validators.insert(validator.peer_id.clone());
                *geographic_counts.entry(country).or_insert(0) += 1;
            }
        }

        // Second pass: fill remaining slots if needed
        self.fill_remaining_slots(
            &scored_validators,
            &mut selected,
            &used_validators,
            target_count,
        );

        Ok(selected)
    }

    /// Check if a validator meets diversity and stake concentration constraints
    async fn validator_meets_constraints(
        &self,
        validator: &ValidatorInfo,
        selected: &[ValidatorInfo],
        geographic_counts: &HashMap<String, usize>,
        target_count: usize,
    ) -> Result<bool, DecentralizationError> {
        let (country, _, _) = self
            .extract_geographic_info(&validator.network_address)
            .await;

        // Check geographic diversity constraint
        let max_per_country = (target_count / 3).max(1); // At most 1/3 from same country
        let country_count = geographic_counts.get(&country).unwrap_or(&0);
        if *country_count >= max_per_country {
            return Ok(false);
        }

        // Check stake concentration constraint
        if selected.len() > 1 {
            let total_selected_stake =
                selected.iter().map(|v| v.stake).sum::<u128>() + validator.stake;
            let stake_concentration = crate::math::div_scale_u128(validator.stake, total_selected_stake);

            if stake_concentration > MAX_STAKE_CONCENTRATION {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Fill remaining validator slots with best available validators
    fn fill_remaining_slots(
        &self,
        scored_validators: &[(ValidatorInfo, u128)],
        selected: &mut Vec<ValidatorInfo>,
        used_validators: &HashSet<PeerId>,
        target_count: usize,
    ) {
        for (validator, _score) in scored_validators {
            if selected.len() >= target_count {
                break;
            }

            if !used_validators.contains(&validator.peer_id) {
                selected.push(validator.clone());
            }
        }
    }

    /// Calculate diversity bonus for underrepresented geographic regions
    async fn calculate_diversity_bonus(
        &self,
        country: &str,
        region: &str,
        all_validators: &[ValidatorInfo],
    ) -> u128 {
        let mut country_counts: HashMap<String, usize> = HashMap::new();
        let mut region_counts: HashMap<String, usize> = HashMap::new();

        // Count validators by geographic location
        for validator in all_validators {
            let (val_country, val_region, _) = self
                .extract_geographic_info(&validator.network_address)
                .await;
            *country_counts.entry(val_country).or_insert(0) += 1;
            *region_counts.entry(val_region).or_insert(0) += 1;
        }

        let total_count = all_validators.len() as u128;
        if total_count == 0 {
            return 0;
        }

        // Higher bonus for less represented regions (inverse frequency)
        let country_bonus = if let Some(&count) = country_counts.get(country) {
            (total_count * crate::QANTO_SCALE) / count as u128
        } else {
            2 * crate::QANTO_SCALE
        };

        let region_bonus = if let Some(&count) = region_counts.get(region) {
            (total_count * crate::QANTO_SCALE) / count as u128
        } else {
            (3 * crate::QANTO_SCALE) / 2
        };

        // Normalize and combine bonuses
        ((country_bonus + region_bonus) / 2).min(3 * crate::QANTO_SCALE) // Cap at 3.0 to prevent extreme values
    }

    /// Select validators for a specific epoch using comprehensive selection criteria
    #[instrument(skip(self))]
    pub async fn select_validators_for_epoch(
        &self,
        epoch: u64,
        validator_count: usize,
    ) -> Result<Vec<ValidatorInfo>, DecentralizationError> {
        let validators = self.validators.read().await;
        let all_validators: Vec<ValidatorInfo> = validators.values().cloned().collect();

        if all_validators.len() < MIN_VALIDATOR_COUNT {
            return Err(DecentralizationError::InsufficientValidators(
                all_validators.len(),
                MIN_VALIDATOR_COUNT,
            ));
        }

        // Use existing validator selection logic with default shard_id 0
        let selected = self
            .select_diverse_validators(&all_validators, validator_count, epoch, 0)
            .await?;

        info!("Selected {} validators for epoch {}", selected.len(), epoch);

        Ok(selected)
    }

    /// Submit a governance proposal
    #[instrument(skip(self, proposal))]
    pub async fn submit_governance_proposal(
        &self,
        proposal: GovernanceProposal,
        submitter: PeerId,
    ) -> Result<String, DecentralizationError> {
        let mut governance = self.governance_system.write().await;

        // Verify submitter has sufficient stake/reputation
        let validators = self.validators.read().await;
        if let Some(validator) = validators.get(&submitter) {
            if validator.stake < 100 {
                // Minimum stake requirement
                return Err(DecentralizationError::InsufficientVotingPower(
                    validator.stake,
                    100,
                ));
            }
        } else {
            return Err(DecentralizationError::InvalidValidatorSignature(
                "Submitter not a registered validator".to_string(),
            ));
        }

        let proposal_id = proposal.id.clone();
        governance
            .active_proposals
            .insert(proposal_id.clone(), proposal);
        governance.proposal_queue.push_back(proposal_id.clone());
        governance.governance_metrics.total_proposals += 1;
        governance.governance_metrics.active_proposals += 1;

        info!(
            "Governance proposal {} submitted by {}",
            proposal_id, submitter
        );
        Ok(proposal_id)
    }

    /// Cast a vote on a governance proposal
    #[instrument(skip(self))]
    pub async fn cast_governance_vote(
        &self,
        proposal_id: &str,
        voter_id: PeerId,
        vote: VoteChoice,
        #[cfg(feature = "zk")] zk_proof: Option<ZKProof>,
    ) -> Result<(), DecentralizationError> {
        let mut governance = self.governance_system.write().await;

        // Verify proposal exists and is active
        if !governance.active_proposals.contains_key(proposal_id) {
            return Err(DecentralizationError::ProposalNotFound(
                proposal_id.to_string(),
            ));
        }

        // Calculate voting power
        let voting_power = self.calculate_voting_power(&voter_id).await?;

        // Verify ZK proof if provided (for anonymous voting)
        #[cfg(feature = "zk")]
        if let Some(proof) = &zk_proof {
            let verification_result = self.zkp_system.verify_proof(proof).await.map_err(|e| {
                DecentralizationError::Anyhow(anyhow!("ZK proof verification failed: {e}"))
            })?;

            if !verification_result {
                return Err(DecentralizationError::InvalidValidatorSignature(
                    "Anonymous voting proof verification failed".to_string(),
                ));
            }
        }

        // Record the vote
        let voting_record = VotingRecord {
            proposal_id: proposal_id.to_string(),
            voter_id: voter_id.clone(),
            vote,
            voting_power,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            #[cfg(feature = "zk")]
            zk_proof,
        };

        let vote_id = format!("{proposal_id}-{voter_id}");
        governance.voting_records.insert(vote_id, voting_record);

        info!("Vote cast by {} on proposal {}", voter_id, proposal_id);
        Ok(())
    }

    /// Calculate voting power for a validator
    async fn calculate_voting_power(
        &self,
        validator_id: &PeerId,
    ) -> Result<u128, DecentralizationError> {
        let validators = self.validators.read().await;

        if let Some(validator) = validators.get(validator_id) {
            // Voting power based on stake and reputation
            // sqrt(stake) * reputation
            let stake_weight = crate::math::integer_sqrt(validator.stake * crate::QANTO_SCALE);
            let reputation_weight = validator.reputation_score;

            Ok((stake_weight * reputation_weight) / crate::math::integer_sqrt(crate::QANTO_SCALE))
        } else {
            Err(DecentralizationError::InvalidValidatorSignature(format!(
                "Validator not found: {validator_id}"
            )))
        }
    }

    /// Process cross-shard communication
    #[instrument(skip(self, message))]
    pub async fn process_cross_shard_message(
        &self,
        message: CrossShardMessage,
    ) -> Result<(), DecentralizationError> {
        let mut messages = self.shard_coordinator.cross_shard_messages.write().await;
        messages.push_back(message.clone());

        match message.message_type {
            CrossShardMessageType::TransactionTransfer => {
                self.handle_cross_shard_transaction(&message).await?
            }
            CrossShardMessageType::StateUpdate => {
                self.handle_cross_shard_state_update(&message).await?
            }
            CrossShardMessageType::ValidatorRotation => {
                self.handle_cross_shard_validator_rotation(&message).await?
            }
            CrossShardMessageType::ConsensusSync => {
                self.handle_cross_shard_consensus_sync(&message).await?
            }
            CrossShardMessageType::GovernanceUpdate => {
                self.handle_cross_shard_governance_update(&message).await?
            }
        }

        Ok(())
    }

    /// Handle cross-shard transaction transfer
    async fn handle_cross_shard_transaction(
        &self,
        message: &CrossShardMessage,
    ) -> Result<(), DecentralizationError> {
        debug!(
            "Handling cross-shard transaction from shard {} to shard {}",
            message.source_shard, message.target_shard
        );

        // Implementation for cross-shard transaction processing
        Ok(())
    }

    /// Handle cross-shard state update
    async fn handle_cross_shard_state_update(
        &self,
        message: &CrossShardMessage,
    ) -> Result<(), DecentralizationError> {
        debug!(
            "Handling cross-shard state update from shard {} to shard {}",
            message.source_shard, message.target_shard
        );

        // Implementation for cross-shard state synchronization
        Ok(())
    }

    /// Handle cross-shard validator rotation
    async fn handle_cross_shard_validator_rotation(
        &self,
        message: &CrossShardMessage,
    ) -> Result<(), DecentralizationError> {
        debug!(
            "Handling cross-shard validator rotation from shard {} to shard {}",
            message.source_shard, message.target_shard
        );

        // Implementation for cross-shard validator coordination
        Ok(())
    }

    /// Handle cross-shard consensus synchronization
    async fn handle_cross_shard_consensus_sync(
        &self,
        message: &CrossShardMessage,
    ) -> Result<(), DecentralizationError> {
        debug!(
            "Handling cross-shard consensus sync from shard {} to shard {}",
            message.source_shard, message.target_shard
        );

        // Implementation for cross-shard consensus coordination
        Ok(())
    }

    /// Handle cross-shard governance update
    async fn handle_cross_shard_governance_update(
        &self,
        message: &CrossShardMessage,
    ) -> Result<(), DecentralizationError> {
        debug!(
            "Handling cross-shard governance update from shard {} to shard {}",
            message.source_shard, message.target_shard
        );

        // Implementation for cross-shard governance synchronization
        Ok(())
    }

    /// Detect and handle network partitions
    pub async fn detect_network_partitions(
        &self,
    ) -> Result<Vec<NetworkPartition>, DecentralizationError> {
        let _topology = self.network_topology.read().await;
        let detected_partitions: Vec<NetworkPartition> = Vec::new();

        // Implementation for network partition detection
        // This would analyze network connectivity and identify isolated groups

        for partition in &detected_partitions {
            info!(
                "Network partition detected: {} affecting {} nodes",
                partition.partition_id,
                partition.affected_nodes.len()
            );
        }

        Ok(detected_partitions)
    }

    /// Get current network health metrics
    pub async fn get_network_health(&self) -> Result<NetworkHealthMetrics, DecentralizationError> {
        let validators = self.validators.read().await;
        let _topology = self.network_topology.read().await;

        let active_validators = validators.len() as u64;
        let total_connections: usize = _topology.connections.values().map(|v| v.len()).sum();
        let node_count = _topology.nodes.len() as u128;
        let average_latency = if node_count > 0 {
            let total_latency: u128 = _topology
                .nodes
                .values()
                .map(|n| n.network_metrics.latency_ms.load(Ordering::Relaxed) as u128)
                .sum();
            (total_latency * crate::QANTO_SCALE) / node_count
        } else {
            0
        };

        Ok(NetworkHealthMetrics {
            validator_count: active_validators,
            total_connections: total_connections as u64,
            average_latency,
            network_coverage: self.calculate_network_coverage().await,
            partition_risk: self.calculate_partition_risk().await,
        })
    }

    /// Calculate network coverage percentage
    async fn calculate_network_coverage(&self) -> u128 {
        let topology = self.network_topology.read().await;
        let total_nodes = topology.nodes.len() as u128;

        if total_nodes == 0 {
            return 0;
        }

        // Calculate connectivity ratio
        let total_connections: usize = topology.connections.values().map(|v| v.len()).sum();
        let max_possible_connections = total_nodes * (total_nodes - 1);
        let connectivity_ratio = if max_possible_connections > 0 {
            (total_connections as u128 * crate::QANTO_SCALE) / max_possible_connections
        } else {
            0
        };

        // Calculate geographic distribution
        let mut geographic_regions = HashSet::new();
        for node in topology.nodes.values() {
            // Extract region from peer_id (simplified approach)
            let region = format!("{:02x}", node.peer_id.id[0] % 10);
            geographic_regions.insert(region);
        }
        let geographic_diversity = (geographic_regions.len() as u128 * crate::QANTO_SCALE) / 10;

        // Calculate validator distribution across shards
        // Calculate validator distribution across shards
        let validators = self.validators.read().await;
        let mut shard_validator_counts = Vec::new();
        let mut shard_map = HashMap::new();
        for validator in validators.values() {
            for &shard_id in &validator.shard_assignments {
                *shard_map.entry(shard_id).or_insert(0) += 1;
            }
        }
        for count in shard_map.values() {
            shard_validator_counts.push((*count as u128) * crate::QANTO_SCALE);
        }

        let shard_balance = if !shard_validator_counts.is_empty() {
            let avg_validators_per_shard = (validators.len() as u128 * crate::QANTO_SCALE) / SHARD_COUNT as u128;
            let variance = self.calculate_variance(&shard_validator_counts);
            let std_dev = crate::math::integer_sqrt(variance * crate::QANTO_SCALE);
            
            let dev_ratio = if avg_validators_per_shard > 0 {
                (std_dev * crate::QANTO_SCALE) / avg_validators_per_shard
            } else {
                crate::QANTO_SCALE
            };
            
            crate::QANTO_SCALE.saturating_sub(dev_ratio.min(crate::QANTO_SCALE))
        } else {
            0
        };

        // Weighted average of coverage metrics
        (connectivity_ratio * 40 / 100)
            + (geographic_diversity.min(crate::QANTO_SCALE) * 30 / 100)
            + (shard_balance * 30 / 100)
    }

    /// Calculate partition risk score
    async fn calculate_partition_risk(&self) -> u128 {
        let topology = self.network_topology.read().await;
        let validators = self.validators.read().await;

        if topology.nodes.is_empty() || validators.is_empty() {
            return crate::QANTO_SCALE; // Maximum risk if no nodes or validators
        }

        // Calculate network fragmentation risk
        let mut risk_factors: Vec<u128> = Vec::new();

        // 1. Connection density risk
        let total_nodes = topology.nodes.len() as u128;
        let total_connections: usize = topology.connections.values().map(|v| v.len()).sum();
        let connection_density = if total_nodes > 0 {
            (total_connections as u128 * crate::QANTO_SCALE) / (total_nodes * total_nodes)
        } else {
            0
        };
        
        let density_threshold = crate::QANTO_SCALE / 10;
        let connection_risk = if connection_density < density_threshold {
            (density_threshold - connection_density) * 10
        } else {
            0
        };
        risk_factors.push(connection_risk.min(crate::QANTO_SCALE));

        // 2. Validator concentration risk
        let mut validator_regions = HashMap::new();
        for validator in validators.values() {
            let region = validator
                .network_address
                .split('.')
                .next()
                .unwrap_or("unknown");
            *validator_regions.entry(region).or_insert(0) += 1;
        }

        let max_validators_in_region = *validator_regions.values().max().unwrap_or(&0) as u128;
        let val_len = validators.len() as u128;
        let concentration_risk = if val_len > 0 {
            let ratio = (max_validators_in_region * crate::QANTO_SCALE) / val_len;
            let threshold = (33 * crate::QANTO_SCALE) / 100;
            if ratio > threshold {
                ((ratio - threshold) * 100) / 67
            } else {
                0
            }
        } else {
            0
        };
        risk_factors.push(concentration_risk.min(crate::QANTO_SCALE));

        // 3. Network latency risk
        let avg_latency = if total_nodes > 0 {
            let total_latency: u128 = topology
                .nodes
                .values()
                .map(|n| n.network_metrics.latency_ms.load(Ordering::Relaxed) as u128)
                .sum();
            (total_latency * crate::QANTO_SCALE) / total_nodes
        } else {
            0
        };
        // Risk increases above 100ms
        let latency_risk = if avg_latency > (100 * crate::QANTO_SCALE) {
            (avg_latency - 100 * crate::QANTO_SCALE) / 1000
        } else {
            0
        };
        risk_factors.push(latency_risk.min(crate::QANTO_SCALE));

        // 4. Packet loss risk
        let avg_packet_loss = if total_nodes > 0 {
            let total_loss: u128 = topology
                .nodes
                .values()
                .map(|n| n.network_metrics.packet_loss_rate.load(Ordering::Relaxed) as u128)
                .sum();
            (total_loss * crate::QANTO_SCALE) / total_nodes
        } else {
            0
        };
        // Risk increases above 1% (0.01)
        let loss_threshold = crate::QANTO_SCALE / 100;
        let packet_loss_risk = if avg_packet_loss > loss_threshold {
            (avg_packet_loss - loss_threshold) * 10
        } else {
            0
        };
        risk_factors.push(packet_loss_risk.min(crate::QANTO_SCALE));

        // 5. Byzantine fault risk
        let byzantine_validators = validators
            .values()
            .filter(|v| v.performance_metrics.byzantine_faults.load(Ordering::Relaxed) > 0)
            .count() as u128;
        
        let byzantine_risk = if val_len > 0 {
            (byzantine_validators * crate::QANTO_SCALE * 3) / val_len // Assuming 1/3 limit
        } else {
            0
        };
        risk_factors.push(byzantine_risk.min(crate::QANTO_SCALE));

        // Calculate weighted average risk
        let total_risk_sum: u128 = risk_factors.iter().sum();
        total_risk_sum / risk_factors.len() as u128
    }

    /// Get comprehensive decentralization metrics
    pub async fn get_decentralization_metrics(
        &self,
    ) -> Result<DecentralizationMetrics, DecentralizationError> {
        let system_state = self.gather_system_state().await;
        let metrics_components = self.calculate_metrics_components(&system_state).await;
        let decentralization_score = self.calculate_final_score(&metrics_components).await;

        Ok(self.build_decentralization_metrics(metrics_components, decentralization_score))
    }

    /// Gather all necessary system state for metrics calculation
    async fn gather_system_state(&self) -> SystemState {
        let validators = self.validators.read().await;
        let topology = self.network_topology.read().await;
        let governance = self.governance_system.read().await;
        let shards = self.shard_coordinator.shards.read().await;

        SystemState {
            validators: validators.clone(),
            topology: topology.clone(),
            governance_participation: governance.governance_metrics.average_participation_rate,
            delegation_ratio: governance.governance_metrics.delegation_rate,
            shards: shards.clone(),
        }
    }

    /// Calculate all metric components
    async fn calculate_metrics_components(&self, state: &SystemState) -> MetricsComponents {
        let geographic_distribution = self
            .calculate_geographic_distribution(&state.validators)
            .await;
        let stake_distribution = self.calculate_stake_distribution(&state.validators).await;
        let validator_performance = self
            .calculate_validator_performance(&state.validators)
            .await;
        let network_decentralization = self
            .calculate_network_decentralization(&state.topology)
            .await;
        let shard_balance = self.calculate_shard_balance(&state.shards).await;

        MetricsComponents {
            geographic_distribution,
            stake_distribution,
            validator_performance,
            network_decentralization,
            shard_balance,
            governance_participation: state.governance_participation,
            delegation_ratio: state.delegation_ratio,
        }
    }

    /// Calculate the final decentralization score
    async fn calculate_final_score(&self, components: &MetricsComponents) -> u128 {
        self.calculate_overall_decentralization_score(
            &components.geographic_distribution,
            &components.stake_distribution,
            &components.network_decentralization,
            components.governance_participation,
        )
        .await
    }

    /// Build the final DecentralizationMetrics struct
    fn build_decentralization_metrics(
        &self,
        components: MetricsComponents,
        decentralization_score: u128,
    ) -> DecentralizationMetrics {
        DecentralizationMetrics {
            validator_count: components
                .geographic_distribution
                .country_counts
                .values()
                .sum(),
            geographic_distribution: components.geographic_distribution,
            stake_distribution: components.stake_distribution,
            validator_performance: components.validator_performance,
            network_decentralization: components.network_decentralization,
            shard_balance: components.shard_balance,
            governance_participation: components.governance_participation,
            delegation_ratio: components.delegation_ratio,
            decentralization_score,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Monitor validator diversity and geographic distribution
    async fn calculate_geographic_distribution(
        &self,
        validators: &HashMap<PeerId, ValidatorInfo>,
    ) -> GeographicDistribution {
        let mut country_counts = HashMap::new();
        let mut region_counts = HashMap::new();
        let mut continent_counts = HashMap::new();

        // Simulate geographic data extraction from network addresses
        for validator in validators.values() {
            // In practice, this would use GeoIP or similar service
            let (country, region, continent) = self
                .extract_geographic_info(&validator.network_address)
                .await;
            *country_counts.entry(country).or_insert(0) += 1;
            *region_counts.entry(region).or_insert(0) += 1;
            *continent_counts.entry(continent).or_insert(0) += 1;
        }

        let total_validators = validators.len() as u128;
        let country_diversity = self.calculate_diversity_index(&country_counts, total_validators);
        let region_diversity = self.calculate_diversity_index(&region_counts, total_validators);
        let continent_diversity =
            self.calculate_diversity_index(&continent_counts, total_validators);

        GeographicDistribution {
            country_counts,
            region_counts,
            continent_counts,
            country_diversity_index: country_diversity,
            region_diversity_index: region_diversity,
            continent_diversity_index: continent_diversity,
        }
    }

    /// Calculate stake distribution metrics
    async fn calculate_stake_distribution(
        &self,
        validators: &HashMap<PeerId, ValidatorInfo>,
    ) -> StakeDistribution {
        let stakes: Vec<u128> = validators.values().map(|v| v.stake).collect();
        let total_stake: u128 = stakes.iter().sum::<u128>();

        if stakes.is_empty() {
            return StakeDistribution::default();
        }

        let mut sorted_stakes = stakes.clone();
        sorted_stakes.sort_unstable();
        sorted_stakes.reverse();

        let (top_10_percent_concentration, top_33_percent_concentration) =
            self.calculate_concentration_metrics(&sorted_stakes, total_stake, validators.len());
        let gini_coefficient = self.calculate_gini_coefficient(&stakes);
        let nakamoto_coefficient = self.calculate_nakamoto_coefficient(&sorted_stakes, total_stake);

        StakeDistribution {
            total_stake,
            average_stake: total_stake / validators.len() as u128,
            median_stake: sorted_stakes[sorted_stakes.len() / 2],
            top_10_percent_concentration,
            top_33_percent_concentration,
            gini_coefficient,
            nakamoto_coefficient,
        }
    }

    /// Calculate stake concentration metrics for top percentiles
    fn calculate_concentration_metrics(
        &self,
        sorted_stakes: &[u128],
        total_stake: u128,
        validator_count: usize,
    ) -> (u128, u128) {
        if total_stake == 0 {
            return (0, 0);
        }

        let top_10_percent_count = (validator_count + 9) / 10;
        let top_10_percent_stake: u128 = sorted_stakes.iter().take(top_10_percent_count).sum();
        let top_10_percent_concentration = (top_10_percent_stake * crate::QANTO_SCALE) / total_stake;

        let top_33_percent_count = (validator_count * 33 + 99) / 100;
        let top_33_percent_stake: u128 = sorted_stakes.iter().take(top_33_percent_count).sum();
        let top_33_percent_concentration = (top_33_percent_stake * crate::QANTO_SCALE) / total_stake;

        (top_10_percent_concentration, top_33_percent_concentration)
    }

    /// Calculate validator performance metrics
    async fn calculate_validator_performance(
        &self,
        validators: &HashMap<PeerId, ValidatorInfo>,
    ) -> ValidatorPerformanceMetrics {
        let (aggregated_metrics, performance_scores) = self.aggregate_validator_metrics(validators);

        let validator_count = validators.len() as u128;
        if validator_count == 0 {
            return ValidatorPerformanceMetrics {
                average_uptime: 0,
                average_response_time: 0,
                total_blocks_produced: 0,
                total_byzantine_faults: 0,
                performance_variance: 0,
                underperforming_validators: 0,
            };
        }
        
        let underperforming_count = self.count_underperforming_validators(&performance_scores);

        ValidatorPerformanceMetrics {
            average_uptime: aggregated_metrics.total_uptime / validator_count,
            average_response_time: aggregated_metrics.total_response_time / validator_count,
            total_blocks_produced: aggregated_metrics.total_blocks_produced,
            total_byzantine_faults: aggregated_metrics.total_byzantine_faults,
            performance_variance: self.calculate_variance(&performance_scores),
            underperforming_validators: underperforming_count,
        }
    }

    /// Aggregate metrics from all validators
    fn aggregate_validator_metrics(
        &self,
        validators: &HashMap<PeerId, ValidatorInfo>,
    ) -> (AggregatedMetrics, Vec<u128>) {
        let mut aggregated = AggregatedMetrics::default();
        let mut performance_scores = Vec::new();

        for validator in validators.values() {
            aggregated.total_uptime += validator
                .performance_metrics
                .uptime_percentage
                .load(Ordering::Relaxed) as u128;
            aggregated.total_response_time += validator
                .performance_metrics
                .response_time_ms
                .load(Ordering::Relaxed) as u128;
            aggregated.total_blocks_produced += validator
                .performance_metrics
                .blocks_produced
                .load(Ordering::Relaxed);
            aggregated.total_byzantine_faults += validator
                .performance_metrics
                .byzantine_faults
                .load(Ordering::Relaxed);

            let performance_score =
                self.calculate_individual_performance_score(&validator.performance_metrics);
            performance_scores.push(performance_score);
        }

        (aggregated, performance_scores)
    }

    /// Count validators with performance below threshold
    fn count_underperforming_validators(&self, performance_scores: &[u128]) -> usize {
        performance_scores
            .iter()
            .filter(|&&score| score < (70 * crate::QANTO_SCALE / 100))
            .count()
    }

    /// Calculate network decentralization metrics
    async fn calculate_network_decentralization(
        &self,
        topology: &NetworkTopology,
    ) -> NetworkDecentralization {
        let node_count = topology.nodes.len() as u128;
        let connection_count: usize = topology.connections.values().map(|conns| conns.len()).sum();

        // Calculate network density
        let max_possible_connections = if node_count <= 1 {
            0
        } else {
            node_count * (node_count - 1)
        };
        let network_density = if max_possible_connections > 0 {
            (connection_count as u128 * crate::QANTO_SCALE) / max_possible_connections
        } else {
            0
        };

        // Calculate clustering coefficient
        let clustering_coefficient = self.calculate_clustering_coefficient(topology).await;

        // Calculate path length distribution
        let average_path_length = self.calculate_average_path_length(topology).await;

        // Calculate network resilience (resistance to node failures)
        let network_resilience = self.calculate_network_resilience(topology).await;

        NetworkDecentralization {
            node_count: node_count as usize,
            connection_count,
            network_density,
            clustering_coefficient,
            average_path_length,
            network_resilience,
        }
    }

    /// Calculate shard balance metrics
    async fn calculate_shard_balance(&self, shards: &HashMap<usize, ShardInfo>) -> ShardBalance {
        let shard_sizes: Vec<usize> = shards.values().map(|s| s.validators.len()).collect();
        let total_validators: usize = shard_sizes.iter().sum();

        if shard_sizes.is_empty() {
            return ShardBalance::default();
        }

        let average_shard_size = (total_validators as u128 * crate::QANTO_SCALE) / shard_sizes.len() as u128;
        let shard_size_variance = self.calculate_shard_size_variance(&shard_sizes);
        let load_balance_coefficient = self.calculate_load_balance_coefficient(shards);

        ShardBalance {
            shard_count: shards.len(),
            average_shard_size,
            shard_size_variance,
            load_balance_coefficient,
            cross_shard_communication_ratio: self.calculate_cross_shard_ratio(shards).await,
        }
    }

    /// Calculate variance in shard sizes
    fn calculate_shard_size_variance(&self, shard_sizes: &[usize]) -> u128 {
        let shard_sizes_u128: Vec<u128> = shard_sizes.iter().map(|&s| s as u128 * crate::QANTO_SCALE).collect();
        self.calculate_variance(&shard_sizes_u128)
    }

    /// Calculate load balance coefficient across shards
    fn calculate_load_balance_coefficient(&self, shards: &HashMap<usize, ShardInfo>) -> u128 {
        let transaction_loads: Vec<usize> =
            shards.values().map(|s| s.transaction_pool.len()).collect();

        if transaction_loads.is_empty() {
            return crate::QANTO_SCALE;
        }

        let max_load = *transaction_loads.iter().max().unwrap_or(&0usize) as u128;
        let min_load = *transaction_loads.iter().min().unwrap_or(&0usize) as u128;

        if max_load > 0 {
            (min_load * crate::QANTO_SCALE) / max_load
        } else {
            crate::QANTO_SCALE
        }
    }

    /// Calculate overall decentralization score
    async fn calculate_overall_decentralization_score(
        &self,
        geographic: &GeographicDistribution,
        stake: &StakeDistribution,
        network: &NetworkDecentralization,
        governance_participation: u128,
    ) -> u128 {
        // Weighted scoring of different decentralization aspects
        let geographic_score =
            (geographic.country_diversity_index + geographic.region_diversity_index) / 2;
        let stake_score = crate::QANTO_SCALE.saturating_sub(stake.gini_coefficient); // Lower Gini = better distribution
        let network_score = (network.network_density + network.network_resilience) / 2;
        let governance_score = governance_participation;

        // Weighted average (geographic 25%, stake 30%, network 25%, governance 20%)
        (geographic_score * 25 / 100)
            + (stake_score * 30 / 100)
            + (network_score * 25 / 100)
            + (governance_score * 20 / 100)
    }

    /// Helper function to extract geographic information from network address
    async fn extract_geographic_info(&self, network_address: &str) -> (String, String, String) {
        // Simulate geographic extraction - in practice would use GeoIP service
        // For now, return mock data based on address patterns
        let country = if network_address.contains("us") || network_address.contains("com") {
            "United States".to_string()
        } else if network_address.contains("eu") || network_address.contains("de") {
            "Germany".to_string()
        } else if network_address.contains("asia") || network_address.contains("jp") {
            "Japan".to_string()
        } else {
            "Unknown".to_string()
        };

        let region = match country.as_str() {
            "United States" => "North America".to_string(),
            "Germany" => "Europe".to_string(),
            "Japan" => "Asia Pacific".to_string(),
            _ => "Unknown".to_string(),
        };

        let continent = match region.as_str() {
            "North America" => "Americas".to_string(),
            "Europe" => "Europe".to_string(),
            "Asia Pacific" => "Asia".to_string(),
            _ => "Unknown".to_string(),
        };

        (country, region, continent)
    }

    /// Calculate Shannon diversity index for geographic distribution
    fn calculate_diversity_index(&self, counts: &HashMap<String, usize>, total: u128) -> u128 {
        if total == 0 {
            return 0;
        }

        let mut diversity = 0;
        for &count in counts.values() {
            if count > 0 {
                let c = count as u128;
                // proportion = c / total
                // diversity -= (c/total) * ln(c/total)
                // diversity -= (c/total) * (ln(c) - ln(total))
                
                let ln_c = ((128 - c.leading_zeros() as u128) * crate::QANTO_SCALE) / 30;
                let ln_total = ((128 - total.leading_zeros() as u128) * crate::QANTO_SCALE) / 30;
                
                // term = (c * (ln_c - ln_total)) / total
                // Note: ln_c - ln_total will be negative or zero.
                if ln_total > ln_c {
                    let diff = ln_total - ln_c;
                    diversity += (c * diff) / total;
                }
            }
        }
        diversity
    }

    /// Calculate Gini coefficient for stake distribution
    fn calculate_gini_coefficient(&self, stakes: &[u128]) -> u128 {
        if stakes.len() <= 1 {
            return 0;
        }

        let mut sorted_stakes = stakes.to_vec();
        sorted_stakes.sort_unstable();

        let n = sorted_stakes.len() as u128;
        let total_sum: u128 = sorted_stakes.iter().map(|&s| s as u128).sum();

        if total_sum == 0 {
            return 0;
        }

        let mut gini_sum: i128 = 0;
        for (i, &stake) in sorted_stakes.iter().enumerate() {
            // (2 * (i + 1) - n - 1) * stake
            let factor = 2 * (i as i128 + 1) - n as i128 - 1;
            gini_sum += factor * stake as i128;
        }

        if gini_sum < 0 {
            0
        } else {
            // gini = gini_sum / (n * n * mean) = gini_sum / (n * sum)
            (gini_sum as u128 * crate::QANTO_SCALE) / (n * total_sum)
        }
    }

    /// Calculate Nakamoto coefficient
    fn calculate_nakamoto_coefficient(&self, sorted_stakes: &[u128], total_stake: u128) -> usize {
        let threshold = total_stake / 2;
        let mut cumulative_stake = 0;

        for (i, &stake) in sorted_stakes.iter().enumerate() {
            cumulative_stake += stake;
            if cumulative_stake > threshold {
                return i + 1;
            }
        }

        sorted_stakes.len()
    }

    /// Calculate individual validator performance score
    fn calculate_individual_performance_score(&self, metrics: &ValidatorMetrics) -> u128 {
        let uptime_score =
            self.calculate_uptime_score(metrics.uptime_percentage.load(Ordering::Relaxed));
        let response_score =
            self.calculate_response_score(metrics.response_time_ms.load(Ordering::Relaxed));
        let reliability_score =
            self.calculate_reliability_score(metrics.byzantine_faults.load(Ordering::Relaxed));
        let participation_score = (metrics.governance_participation.load(Ordering::Relaxed) as u128 * crate::QANTO_SCALE) / 10000; // Assuming 100.00% is 10000

        // Weighted average of performance factors
        // 0.3 * U + 0.3 * R + 0.2 * B + 0.2 * P
        crate::math::mul_scale_u128(uptime_score, 300_000_000)
            + crate::math::mul_scale_u128(response_score, 300_000_000)
            + crate::math::mul_scale_u128(reliability_score, 200_000_000)
            + crate::math::mul_scale_u128(participation_score, 200_000_000)
    }

    /// Calculate uptime score from uptime percentage
    fn calculate_uptime_score(&self, uptime_percentage: u64) -> u128 {
        // uptime_percentage is scaled by 1000 (99900 = 99.9%)
        (uptime_percentage as u128 * crate::QANTO_SCALE) / 100000
    }

    /// Calculate response score from response time
    fn calculate_response_score(&self, response_time_ms: u64) -> u128 {
        if response_time_ms > 0 {
            // (1000 / response_time_ms).min(1.0)
            let score = (1000 * crate::QANTO_SCALE) / response_time_ms as u128;
            score.min(crate::QANTO_SCALE)
        } else {
            0
        }
    }

    /// Calculate reliability score from byzantine faults
    fn calculate_reliability_score(&self, byzantine_faults: u64) -> u128 {
        if byzantine_faults == 0 {
            crate::QANTO_SCALE
        } else {
            crate::QANTO_SCALE / 2
        }
    }

    /// Calculate variance of a dataset
    fn calculate_variance(&self, values: &[u128]) -> u128 {
        if values.len() <= 1 {
            return 0;
        }

        let n = values.len() as u128;
        let sum: u128 = values.iter().sum();
        let mean = sum / n;
        
        let mut variance_sum = 0;
        for &value in values {
            let diff = if value > mean {
                value - mean
            } else {
                mean - value
            };
            // Square the diff and rescale: (diff * diff) / SCALE
            variance_sum += crate::math::mul_scale_u128(diff, diff);
        }

        variance_sum / n
    }

    /// Calculate clustering coefficient for network topology
    async fn calculate_clustering_coefficient(&self, topology: &NetworkTopology) -> u128 {
        let mut total_clustering = 0;
        let mut node_count = 0;

        for connections in topology.connections.values() {
            if connections.len() < 2 {
                continue; // Need at least 2 connections for clustering
            }

            let mut triangles = 0;
            let possible_triangles = connections.len() * (connections.len() - 1) / 2;

            // Count triangles (connections between neighbors)
            for i in 0..connections.len() {
                for j in (i + 1)..connections.len() {
                    if let Some(neighbor_connections) = topology.connections.get(&connections[i]) {
                        if neighbor_connections.contains(&connections[j]) {
                            triangles += 1;
                        }
                    }
                }
            }

            if possible_triangles > 0 {
                total_clustering += (triangles as u128 * crate::QANTO_SCALE) / possible_triangles as u128;
                node_count += 1;
            }
        }

        if node_count > 0 {
            total_clustering / node_count as u128
        } else {
            0
        }
    }

    /// Calculate average path length in network
    async fn calculate_average_path_length(&self, topology: &NetworkTopology) -> u128 {
        // Simplified calculation - in practice would use BFS/Dijkstra
        let node_count = topology.nodes.len() as u128;
        if node_count <= 1 {
            return 0;
        }

        let total_connections: usize = topology.connections.values().map(|conns| conns.len()).sum();
        // average_degree = total_connections / node_count
        
        if total_connections > 0 {
            // L ~ ln(N) / ln(k)
            let ln_n = ((128 - node_count.leading_zeros() as u128) * crate::QANTO_SCALE) / 30;
            let k = total_connections as u128 / node_count;
            if k > 1 {
                let ln_k = ((128 - k.leading_zeros() as u128) * crate::QANTO_SCALE) / 30;
                (ln_n * crate::QANTO_SCALE) / ln_k
            } else {
                crate::QANTO_SCALE * 10 // Mock high value for low connectivity
            }
        } else {
            u128::MAX // Infinity replacement
        }
    }

    /// Calculate network resilience to node failures
    async fn calculate_network_resilience(&self, topology: &NetworkTopology) -> u128 {
        let node_count = topology.nodes.len() as u128;
        if node_count == 0 {
            return 0;
        }

        // Calculate minimum cut size (simplified)
        let mut min_connections = usize::MAX;
        for connections in topology.connections.values() {
            if connections.len() < min_connections {
                min_connections = connections.len();
            }
        }

        if min_connections == usize::MAX {
            min_connections = 0;
        }

        // Resilience based on minimum connectivity
        (min_connections as u128 * crate::QANTO_SCALE / node_count).min(crate::QANTO_SCALE)
    }

    /// Calculate cross-shard communication ratio
    async fn calculate_cross_shard_ratio(&self, shards: &HashMap<usize, ShardInfo>) -> u128 {
        let total_transactions: usize = shards.values().map(|s| s.transaction_pool.len()).sum();
        if total_transactions == 0 {
            return 0;
        }

        // Estimate cross-shard transactions (simplified)
        let cross_shard_links: usize = shards.values().map(|s| s.cross_shard_links.len()).sum();
        (cross_shard_links as u128 * crate::QANTO_SCALE) / total_transactions as u128
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealthMetrics {
    pub validator_count: u64,
    pub total_connections: u64,
    pub average_latency: u128, // Scaled by 1e9
    pub network_coverage: u128, // Scaled by 1e9
    pub partition_risk: u128,   // Scaled by 1e9
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecentralizationMetrics {
    pub validator_count: usize,
    pub geographic_distribution: GeographicDistribution,
    pub stake_distribution: StakeDistribution,
    pub validator_performance: ValidatorPerformanceMetrics,
    pub network_decentralization: NetworkDecentralization,
    pub shard_balance: ShardBalance,
    pub governance_participation: u128, // Scaled by 1e9
    pub delegation_ratio: u128,          // Scaled by 1e9
    pub decentralization_score: u128,    // Scaled by 1e9
    pub timestamp: u64,
}

/// Internal struct to hold system state for metrics calculation
#[derive(Debug, Clone)]
struct SystemState {
    validators: HashMap<PeerId, ValidatorInfo>,
    topology: NetworkTopology,
    governance_participation: u128, // Scaled by 1e9
    delegation_ratio: u128,          // Scaled by 1e9
    shards: HashMap<usize, ShardInfo>,
}

/// Internal struct to hold calculated metric components
#[derive(Debug, Clone)]
struct MetricsComponents {
    geographic_distribution: GeographicDistribution,
    stake_distribution: StakeDistribution,
    validator_performance: ValidatorPerformanceMetrics,
    network_decentralization: NetworkDecentralization,
    shard_balance: ShardBalance,
    governance_participation: u128, // Scaled by 1e9
    delegation_ratio: u128,          // Scaled by 1e9
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicDistribution {
    pub country_counts: HashMap<String, usize>,
    pub region_counts: HashMap<String, usize>,
    pub continent_counts: HashMap<String, usize>,
    pub country_diversity_index: u128, // Scaled by 1e9
    pub region_diversity_index: u128,  // Scaled by 1e9
    pub continent_diversity_index: u128, // Scaled by 1e9
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StakeDistribution {
    pub total_stake: u128,
    pub average_stake: u128,
    pub median_stake: u128,
    pub top_10_percent_concentration: u128, // Scaled by 1e9
    pub top_33_percent_concentration: u128, // Scaled by 1e9
    pub gini_coefficient: u128,              // Scaled by 1e9
    pub nakamoto_coefficient: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorPerformanceMetrics {
    pub average_uptime: u128, // Scaled by 1e9
    pub average_response_time: u128, // Scaled by 1e9
    pub total_blocks_produced: u64,
    pub total_byzantine_faults: u64,
    pub performance_variance: u128, // Scaled by 1e9
    pub underperforming_validators: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDecentralization {
    pub node_count: usize,
    pub connection_count: usize,
    pub network_density: u128,        // Scaled by 1e9
    pub clustering_coefficient: u128, // Scaled by 1e9
    pub average_path_length: u128,    // Scaled by 1e9
    pub network_resilience: u128,     // Scaled by 1e9
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShardBalance {
    pub shard_count: usize,
    pub average_shard_size: u128,         // Scaled by 1e9
    pub shard_size_variance: u128,        // Scaled by 1e9
    pub load_balance_coefficient: u128,   // Scaled by 1e9
    pub cross_shard_communication_ratio: u128, // Scaled by 1e9
}

#[derive(Debug, Default)]
struct AggregatedMetrics {
    total_uptime: u128,         // Scaled by 1e9
    total_response_time: u128, // Scaled by 1e9
    total_blocks_produced: u64,
    total_byzantine_faults: u64,
}

// Implementation for supporting structures

impl Default for NetworkTopology {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkTopology {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: HashMap::new(),
            shards: HashMap::new(),
            partition_detection: PartitionDetector {
                suspected_partitions: Vec::new(),
                detection_threshold: crate::QANTO_SCALE / 10,
                recovery_strategies: Vec::new(),
            },
            load_balancer: LoadBalancer {
                load_distribution: HashMap::new(),
                balancing_strategy: BalancingStrategy::PerformanceBased,
                rebalancing_threshold: (8 * crate::QANTO_SCALE) / 10,
                performance_weights: HashMap::new(),
            },
        }
    }
}

impl Default for DecentralizedGovernance {
    fn default() -> Self {
        Self::new()
    }
}

impl DecentralizedGovernance {
    pub fn new() -> Self {
        Self {
            active_proposals: HashMap::new(),
            voting_records: HashMap::new(),
            delegation_system: DelegationSystem {
                delegations: HashMap::new(),
                delegation_weights: HashMap::new(),
                delegation_history: Vec::new(),
            },
            proposal_queue: VecDeque::new(),
            governance_metrics: GovernanceMetrics::default(),
        }
    }
}

impl Default for ShardCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ShardCoordinator {
    pub fn new() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
            cross_shard_messages: Arc::new(RwLock::new(VecDeque::new())),
            shard_assignments: Arc::new(RwLock::new(HashMap::new())),
            load_balancer: Arc::new(RwLock::new(LoadBalancer {
                load_distribution: HashMap::new(),
                balancing_strategy: BalancingStrategy::PerformanceBased,
                rebalancing_threshold: (8 * crate::QANTO_SCALE) / 10,
                performance_weights: HashMap::new(),
            })),
        }
    }
}

impl Default for PeerDiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerDiscoveryService {
    pub fn new() -> Self {
        Self {
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            discovery_protocols: vec![
                DiscoveryProtocol::DHT,
                DiscoveryProtocol::Gossip,
                DiscoveryProtocol::Bootstrap,
            ],
            bootstrap_nodes: Vec::new(),
            discovery_metrics: Arc::new(RwLock::new(DiscoveryMetrics::default())),
        }
    }
}
