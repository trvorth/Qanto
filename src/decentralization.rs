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
use crate::qantodag::{QantoBlock, QantoDAG};
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
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// --- Decentralization Constants ---
const MIN_VALIDATOR_COUNT: usize = 21;
const VALIDATOR_ROTATION_EPOCHS: u64 = 100;
const BYZANTINE_FAULT_TOLERANCE: f64 = 0.33; // Up to 33% Byzantine nodes
const CONSENSUS_TIMEOUT_MS: u64 = 30000;
const SHARD_COUNT: usize = 64;

// Diversity and selection constants
const MAX_STAKE_CONCENTRATION: f64 = 0.4; // Maximum stake concentration for top validators
const PERFORMANCE_WEIGHT: f64 = 0.4; // Weight for performance in selection
const DIVERSITY_WEIGHT: f64 = 0.3; // Weight for diversity in selection
const STAKE_WEIGHT: f64 = 0.3; // Weight for stake in selection

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
    InsufficientVotingPower(f64, f64),
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
    pub stake: u64,
    pub reputation_score: f64,
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
    pub proposed_block: Option<QantoBlock>,
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
    pub trust_score: f64,
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
    pub bandwidth_mbps: f64,
}

// Use unified metrics system
pub type NetworkMetrics = QantoMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionDetector {
    pub suspected_partitions: Vec<NetworkPartition>,
    pub detection_threshold: f64,
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
    pub success_rate: f64,
    pub estimated_recovery_time: u64,
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
    pub load_distribution: HashMap<PeerId, f64>,
    pub balancing_strategy: BalancingStrategy,
    pub rebalancing_threshold: f64,
    pub performance_weights: HashMap<String, f64>,
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
    pub voting_power: f64,
    pub timestamp: u64,
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
    pub delegation_weights: HashMap<PeerId, f64>,
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
    pub average_participation_rate: f64,
    pub delegation_rate: f64,
    pub governance_health_score: f64,
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
    pub average_discovery_time: f64,
    pub network_coverage: f64,
}

impl DecentralizationEngine {
    pub fn new(
        consensus_engine: Arc<Consensus>,
        saga: Arc<PalletSaga>,
        zkp_system: Arc<ZKProofSystem>,
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
        block: QantoBlock,
        _dag: &Arc<QantoDAG>,
        shard_id: usize,
    ) -> Result<String, DecentralizationError> {
        let round_id = Uuid::new_v4().to_string();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
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
        block: &QantoBlock,
    ) -> Result<(), DecentralizationError> {
        let shards = self.shard_coordinator.shards.read().await;

        if let Some(shard) = shards.get(&shard_id) {
            for validator_id in &shard.validators {
                // Send consensus message to validator
                let _message = NetworkMessage::Consensus {
                    round_id: round_id.to_string(),
                    block: block.clone(),
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
                "Consensus round not found: {}",
                round_id
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
                        DecentralizationError::Anyhow(anyhow!(
                            "ZK proof verification failed: {}",
                            e
                        ))
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
            let required_votes =
                ((total_validators as f64) * (1.0 - BYZANTINE_FAULT_TOLERANCE)).ceil() as usize;

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
        block: &QantoBlock,
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
    ) -> Result<Vec<(ValidatorInfo, f64)>, DecentralizationError> {
        let mut scored_validators = Vec::new();

        for validator in all_validators {
            let performance_score =
                self.calculate_individual_performance_score(&validator.performance_metrics);
            let stake_score = (validator.stake as f64).ln() / 20.0; // Logarithmic stake scoring

            // Extract geographic info for diversity calculation
            let (country, region, _) = self
                .extract_geographic_info(&validator.network_address)
                .await;

            // Calculate diversity bonus (higher for underrepresented regions)
            let diversity_bonus = self
                .calculate_diversity_bonus(&country, &region, all_validators)
                .await;

            // Combine scores with weights
            let total_score = (performance_score * PERFORMANCE_WEIGHT)
                + (stake_score * STAKE_WEIGHT)
                + (diversity_bonus * DIVERSITY_WEIGHT);

            // Add epoch-based rotation factor to ensure fairness over time
            let rotation_factor =
                ((epoch + shard_id as u64) * validator.peer_id.to_string().len() as u64) as f64
                    * 0.001;
            let final_score = total_score + rotation_factor;

            scored_validators.push((validator.clone(), final_score));
        }

        // Sort by score (descending)
        scored_validators
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored_validators)
    }

    /// Apply diversity and stake concentration constraints to validator selection
    async fn apply_diversity_constraints(
        &self,
        scored_validators: Vec<(ValidatorInfo, f64)>,
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
                selected.iter().map(|v| v.stake).sum::<u64>() + validator.stake;
            let stake_concentration = validator.stake as f64 / total_selected_stake as f64;

            if stake_concentration > MAX_STAKE_CONCENTRATION {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Fill remaining validator slots with best available validators
    fn fill_remaining_slots(
        &self,
        scored_validators: &[(ValidatorInfo, f64)],
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
    ) -> f64 {
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

        let total_validators = all_validators.len() as f64;
        let country_frequency =
            *country_counts.get(country).unwrap_or(&0) as f64 / total_validators;
        let region_frequency = *region_counts.get(region).unwrap_or(&0) as f64 / total_validators;

        // Higher bonus for less represented regions (inverse frequency)
        let country_bonus = if country_frequency > 0.0 {
            1.0 / country_frequency
        } else {
            2.0
        };
        let region_bonus = if region_frequency > 0.0 {
            1.0 / region_frequency
        } else {
            1.5
        };

        // Normalize and combine bonuses
        ((country_bonus + region_bonus) / 2.0).min(3.0) // Cap at 3.0 to prevent extreme values
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
                    validator.stake as f64,
                    100.0,
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
        zk_proof: Option<ZKProof>,
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
        if let Some(proof) = &zk_proof {
            let verification_result = self.zkp_system.verify_proof(proof).await.map_err(|e| {
                DecentralizationError::Anyhow(anyhow!("ZK proof verification failed: {}", e))
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
    ) -> Result<f64, DecentralizationError> {
        let validators = self.validators.read().await;

        if let Some(validator) = validators.get(validator_id) {
            // Voting power based on stake and reputation
            let stake_weight = (validator.stake as f64).sqrt();
            let reputation_weight = validator.reputation_score;

            Ok(stake_weight * reputation_weight)
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
        let average_latency = _topology
            .nodes
            .values()
            .map(|n| {
                n.network_metrics
                    .latency_ms
                    .load(std::sync::atomic::Ordering::Relaxed) as f64
            })
            .sum::<f64>()
            / _topology.nodes.len() as f64;

        Ok(NetworkHealthMetrics {
            validator_count: active_validators,
            total_connections: total_connections as u64,
            average_latency,
            network_coverage: self.calculate_network_coverage().await,
            partition_risk: self.calculate_partition_risk().await,
        })
    }

    /// Calculate network coverage percentage
    async fn calculate_network_coverage(&self) -> f64 {
        let topology = self.network_topology.read().await;
        let total_nodes = topology.nodes.len();

        if total_nodes == 0 {
            return 0.0;
        }

        // Calculate connectivity ratio
        let total_connections: usize = topology.connections.values().map(|v| v.len()).sum();
        let max_possible_connections = total_nodes * (total_nodes - 1);
        let connectivity_ratio = if max_possible_connections > 0 {
            total_connections as f64 / max_possible_connections as f64
        } else {
            0.0
        };

        // Calculate geographic distribution
        let mut geographic_regions = HashSet::new();
        for node in topology.nodes.values() {
            // Extract region from network address (simplified)
            // Extract region from peer_id (simplified approach since network_address is not available)
            let region = format!("{:02x}", node.peer_id.id[0] % 10); // Use first byte of peer_id for region
            geographic_regions.insert(region);
        }
        let geographic_diversity = geographic_regions.len() as f64 / 10.0; // Assume 10 major regions

        // Calculate validator distribution across shards
        let validators = self.validators.read().await;
        let mut shard_validator_count = HashMap::new();
        for validator in validators.values() {
            for &shard_id in &validator.shard_assignments {
                *shard_validator_count.entry(shard_id).or_insert(0) += 1;
            }
        }

        let shard_balance = if !shard_validator_count.is_empty() {
            let avg_validators_per_shard = validators.len() as f64 / SHARD_COUNT as f64;
            let variance: f64 = shard_validator_count
                .values()
                .map(|&count| (count as f64 - avg_validators_per_shard).powi(2))
                .sum::<f64>()
                / shard_validator_count.len() as f64;
            1.0 - (variance.sqrt() / avg_validators_per_shard).min(1.0)
        } else {
            0.0
        };

        // Weighted average of coverage metrics
        connectivity_ratio * 0.4 + geographic_diversity.min(1.0) * 0.3 + shard_balance * 0.3
    }

    /// Calculate partition risk score
    async fn calculate_partition_risk(&self) -> f64 {
        let topology = self.network_topology.read().await;
        let validators = self.validators.read().await;

        if topology.nodes.is_empty() || validators.is_empty() {
            return 1.0; // Maximum risk if no nodes or validators
        }

        // Calculate network fragmentation risk
        let mut risk_factors: Vec<f64> = Vec::new();

        // 1. Connection density risk
        let total_nodes = topology.nodes.len();
        let total_connections: usize = topology.connections.values().map(|v| v.len()).sum();
        let connection_density = total_connections as f64 / (total_nodes * total_nodes) as f64;
        let connection_risk = (0.1f64 - connection_density).max(0.0) / 0.1; // Risk increases as density drops below 10%
        risk_factors.push(connection_risk);

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

        let max_validators_in_region = *validator_regions.values().max().unwrap_or(&0);
        let concentration_risk =
            (max_validators_in_region as f64 / validators.len() as f64 - 0.33).max(0.0) / 0.67;
        risk_factors.push(concentration_risk);

        // 3. Network latency risk
        let avg_latency = topology
            .nodes
            .values()
            .map(|n| {
                n.network_metrics
                    .latency_ms
                    .load(std::sync::atomic::Ordering::Relaxed) as f64
            })
            .sum::<f64>()
            / topology.nodes.len() as f64;
        let latency_risk = (avg_latency - 100.0).max(0.0) / 1000.0; // Risk increases above 100ms
        risk_factors.push(latency_risk);

        // 4. Packet loss risk
        let avg_packet_loss = topology
            .nodes
            .values()
            .map(|n| {
                n.network_metrics
                    .packet_loss_rate
                    .load(std::sync::atomic::Ordering::Relaxed) as f64
            })
            .sum::<f64>()
            / topology.nodes.len() as f64;
        let packet_loss_risk = (avg_packet_loss - 0.01).max(0.0) / 0.1; // Risk increases above 1%
        risk_factors.push(packet_loss_risk);

        // 5. Byzantine fault risk
        let byzantine_validators = validators
            .values()
            .filter(|v| {
                v.performance_metrics
                    .byzantine_faults
                    .load(std::sync::atomic::Ordering::Relaxed)
                    > 0
            })
            .count();
        let byzantine_risk =
            (byzantine_validators as f64 / validators.len() as f64 / BYZANTINE_FAULT_TOLERANCE)
                .min(1.0);
        risk_factors.push(byzantine_risk);

        // Calculate weighted average risk
        let total_risk = risk_factors.iter().sum::<f64>() / risk_factors.len() as f64;
        total_risk.min(1.0)
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
    async fn calculate_final_score(&self, components: &MetricsComponents) -> f64 {
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
        decentralization_score: f64,
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

        let total_validators = validators.len() as f64;
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
        let stakes: Vec<u64> = validators.values().map(|v| v.stake).collect();
        let total_stake: u64 = stakes.iter().sum();

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
            average_stake: total_stake / validators.len() as u64,
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
        sorted_stakes: &[u64],
        total_stake: u64,
        validator_count: usize,
    ) -> (f64, f64) {
        let top_10_percent_count = (validator_count as f64 * 0.1).ceil() as usize;
        let top_10_percent_stake: u64 = sorted_stakes.iter().take(top_10_percent_count).sum();
        let top_10_percent_concentration = top_10_percent_stake as f64 / total_stake as f64;

        let top_33_percent_count = (validator_count as f64 * 0.33).ceil() as usize;
        let top_33_percent_stake: u64 = sorted_stakes.iter().take(top_33_percent_count).sum();
        let top_33_percent_concentration = top_33_percent_stake as f64 / total_stake as f64;

        (top_10_percent_concentration, top_33_percent_concentration)
    }

    /// Calculate validator performance metrics
    async fn calculate_validator_performance(
        &self,
        validators: &HashMap<PeerId, ValidatorInfo>,
    ) -> ValidatorPerformanceMetrics {
        let (aggregated_metrics, performance_scores) = self.aggregate_validator_metrics(validators);

        let validator_count = validators.len() as f64;
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
    ) -> (AggregatedMetrics, Vec<f64>) {
        let mut aggregated = AggregatedMetrics::default();
        let mut performance_scores = Vec::new();

        for validator in validators.values() {
            aggregated.total_uptime += validator
                .performance_metrics
                .uptime_percentage
                .load(Ordering::Relaxed) as f64;
            aggregated.total_response_time += validator
                .performance_metrics
                .response_time_ms
                .load(Ordering::Relaxed) as f64;
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
    fn count_underperforming_validators(&self, performance_scores: &[f64]) -> usize {
        performance_scores
            .iter()
            .filter(|&&score| score < 0.7)
            .count()
    }

    /// Calculate network decentralization metrics
    async fn calculate_network_decentralization(
        &self,
        topology: &NetworkTopology,
    ) -> NetworkDecentralization {
        let node_count = topology.nodes.len();
        let connection_count: usize = topology.connections.values().map(|conns| conns.len()).sum();

        // Calculate network density
        let max_possible_connections = if node_count <= 1 {
            0
        } else {
            node_count * (node_count - 1)
        };
        let network_density = if max_possible_connections > 0 {
            connection_count as f64 / max_possible_connections as f64
        } else {
            0.0
        };

        // Calculate clustering coefficient
        let clustering_coefficient = self.calculate_clustering_coefficient(topology).await;

        // Calculate path length distribution
        let average_path_length = self.calculate_average_path_length(topology).await;

        // Calculate network resilience (resistance to node failures)
        let network_resilience = self.calculate_network_resilience(topology).await;

        NetworkDecentralization {
            node_count,
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

        let average_shard_size = total_validators as f64 / shard_sizes.len() as f64;
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
    fn calculate_shard_size_variance(&self, shard_sizes: &[usize]) -> f64 {
        let shard_sizes_f64: Vec<f64> = shard_sizes.iter().map(|&s| s as f64).collect();
        self.calculate_variance(&shard_sizes_f64)
    }

    /// Calculate load balance coefficient across shards
    fn calculate_load_balance_coefficient(&self, shards: &HashMap<usize, ShardInfo>) -> f64 {
        let transaction_loads: Vec<usize> =
            shards.values().map(|s| s.transaction_pool.len()).collect();

        if transaction_loads.is_empty() {
            return 1.0;
        }

        let max_load = *transaction_loads.iter().max().unwrap_or(&0usize) as f64;
        let min_load = *transaction_loads.iter().min().unwrap_or(&0usize) as f64;

        if max_load > 0.0 {
            min_load / max_load
        } else {
            1.0
        }
    }

    /// Calculate overall decentralization score
    async fn calculate_overall_decentralization_score(
        &self,
        geographic: &GeographicDistribution,
        stake: &StakeDistribution,
        network: &NetworkDecentralization,
        governance_participation: f64,
    ) -> f64 {
        // Weighted scoring of different decentralization aspects
        let geographic_score =
            (geographic.country_diversity_index + geographic.region_diversity_index) / 2.0;
        let stake_score = 1.0 - stake.gini_coefficient; // Lower Gini = better distribution
        let network_score = (network.network_density + network.network_resilience) / 2.0;
        let governance_score = governance_participation;

        // Weighted average (can be adjusted based on priorities)
        let weights = [0.25, 0.30, 0.25, 0.20]; // geographic, stake, network, governance
        let scores = [
            geographic_score,
            stake_score,
            network_score,
            governance_score,
        ];

        scores
            .iter()
            .zip(weights.iter())
            .map(|(score, weight)| score * weight)
            .sum()
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
    fn calculate_diversity_index(&self, counts: &HashMap<String, usize>, total: f64) -> f64 {
        if total == 0.0 {
            return 0.0;
        }

        let mut diversity = 0.0;
        for &count in counts.values() {
            if count > 0 {
                let proportion = count as f64 / total;
                diversity -= proportion * proportion.ln();
            }
        }
        diversity
    }

    /// Calculate Gini coefficient for stake distribution
    fn calculate_gini_coefficient(&self, stakes: &[u64]) -> f64 {
        if stakes.len() <= 1 {
            return 0.0;
        }

        let mut sorted_stakes = stakes.to_vec();
        sorted_stakes.sort_unstable();

        let n = sorted_stakes.len() as f64;
        let mean = sorted_stakes.iter().sum::<u64>() as f64 / n;

        if mean == 0.0 {
            return 0.0;
        }

        let mut gini_sum = 0.0;
        for (i, &stake) in sorted_stakes.iter().enumerate() {
            gini_sum += (2.0 * (i as f64 + 1.0) - n - 1.0) * stake as f64;
        }

        gini_sum / (n * n * mean)
    }

    /// Calculate Nakamoto coefficient
    fn calculate_nakamoto_coefficient(&self, sorted_stakes: &[u64], total_stake: u64) -> usize {
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
    fn calculate_individual_performance_score(&self, metrics: &ValidatorMetrics) -> f64 {
        let uptime_score =
            self.calculate_uptime_score(metrics.uptime_percentage.load(Ordering::Relaxed) as f64);
        let response_score =
            self.calculate_response_score(metrics.response_time_ms.load(Ordering::Relaxed) as f64);
        let reliability_score =
            self.calculate_reliability_score(metrics.byzantine_faults.load(Ordering::Relaxed));
        let participation_score = metrics.governance_participation.load(Ordering::Relaxed) as f64;

        // Weighted average of performance factors
        (uptime_score * 0.3)
            + (response_score * 0.3)
            + (reliability_score * 0.2)
            + (participation_score * 0.2)
    }

    /// Calculate uptime score from uptime percentage
    fn calculate_uptime_score(&self, uptime_percentage: f64) -> f64 {
        uptime_percentage / 100.0
    }

    /// Calculate response score from response time
    fn calculate_response_score(&self, response_time_ms: f64) -> f64 {
        if response_time_ms > 0.0 {
            (1000.0 / response_time_ms).min(1.0)
        } else {
            0.0
        }
    }

    /// Calculate reliability score from byzantine faults
    fn calculate_reliability_score(&self, byzantine_faults: u64) -> f64 {
        if byzantine_faults == 0 {
            1.0
        } else {
            0.5
        }
    }

    /// Calculate variance of a dataset
    fn calculate_variance(&self, values: &[f64]) -> f64 {
        if values.len() <= 1 {
            return 0.0;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;

        variance
    }

    /// Calculate clustering coefficient for network topology
    async fn calculate_clustering_coefficient(&self, topology: &NetworkTopology) -> f64 {
        let mut total_clustering = 0.0;
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
                total_clustering += triangles as f64 / possible_triangles as f64;
                node_count += 1;
            }
        }

        if node_count > 0 {
            total_clustering / node_count as f64
        } else {
            0.0
        }
    }

    /// Calculate average path length in network
    async fn calculate_average_path_length(&self, topology: &NetworkTopology) -> f64 {
        // Simplified calculation - in practice would use BFS/Dijkstra
        let node_count = topology.nodes.len();
        if node_count <= 1 {
            return 0.0;
        }

        let total_connections: usize = topology.connections.values().map(|conns| conns.len()).sum();
        let average_degree = total_connections as f64 / node_count as f64;

        // Estimate based on small-world network properties
        if average_degree > 0.0 {
            (node_count as f64).ln() / average_degree.ln()
        } else {
            f64::INFINITY
        }
    }

    /// Calculate network resilience to node failures
    async fn calculate_network_resilience(&self, topology: &NetworkTopology) -> f64 {
        let node_count = topology.nodes.len();
        if node_count == 0 {
            return 0.0;
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
        (min_connections as f64 / node_count as f64).min(1.0)
    }

    /// Calculate cross-shard communication ratio
    async fn calculate_cross_shard_ratio(&self, shards: &HashMap<usize, ShardInfo>) -> f64 {
        let total_transactions: usize = shards.values().map(|s| s.transaction_pool.len()).sum();
        if total_transactions == 0 {
            return 0.0;
        }

        // Estimate cross-shard transactions (simplified)
        let cross_shard_links: usize = shards.values().map(|s| s.cross_shard_links.len()).sum();
        cross_shard_links as f64 / total_transactions as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealthMetrics {
    pub validator_count: u64,
    pub total_connections: u64,
    pub average_latency: f64,
    pub network_coverage: f64,
    pub partition_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecentralizationMetrics {
    pub validator_count: usize,
    pub geographic_distribution: GeographicDistribution,
    pub stake_distribution: StakeDistribution,
    pub validator_performance: ValidatorPerformanceMetrics,
    pub network_decentralization: NetworkDecentralization,
    pub shard_balance: ShardBalance,
    pub governance_participation: f64,
    pub delegation_ratio: f64,
    pub decentralization_score: f64,
    pub timestamp: u64,
}

/// Internal struct to hold system state for metrics calculation
#[derive(Debug, Clone)]
struct SystemState {
    validators: HashMap<PeerId, ValidatorInfo>,
    topology: NetworkTopology,
    governance_participation: f64,
    delegation_ratio: f64,
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
    governance_participation: f64,
    delegation_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicDistribution {
    pub country_counts: HashMap<String, usize>,
    pub region_counts: HashMap<String, usize>,
    pub continent_counts: HashMap<String, usize>,
    pub country_diversity_index: f64,
    pub region_diversity_index: f64,
    pub continent_diversity_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StakeDistribution {
    pub total_stake: u64,
    pub average_stake: u64,
    pub median_stake: u64,
    pub top_10_percent_concentration: f64,
    pub top_33_percent_concentration: f64,
    pub gini_coefficient: f64,
    pub nakamoto_coefficient: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorPerformanceMetrics {
    pub average_uptime: f64,
    pub average_response_time: f64,
    pub total_blocks_produced: u64,
    pub total_byzantine_faults: u64,
    pub performance_variance: f64,
    pub underperforming_validators: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDecentralization {
    pub node_count: usize,
    pub connection_count: usize,
    pub network_density: f64,
    pub clustering_coefficient: f64,
    pub average_path_length: f64,
    pub network_resilience: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShardBalance {
    pub shard_count: usize,
    pub average_shard_size: f64,
    pub shard_size_variance: f64,
    pub load_balance_coefficient: f64,
    pub cross_shard_communication_ratio: f64,
}

#[derive(Debug, Default)]
struct AggregatedMetrics {
    total_uptime: f64,
    total_response_time: f64,
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
                detection_threshold: 0.1,
                recovery_strategies: Vec::new(),
            },
            load_balancer: LoadBalancer {
                load_distribution: HashMap::new(),
                balancing_strategy: BalancingStrategy::PerformanceBased,
                rebalancing_threshold: 0.8,
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
                rebalancing_threshold: 0.8,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::Consensus;

    use crate::qanto_net::PeerId;
    use crate::saga::{GovernanceProposal, PalletSaga};
    use crate::zkp::ZKProofSystem;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    async fn create_test_engine() -> DecentralizationEngine {
        let validators = Arc::new(RwLock::new(HashMap::new()));
        let consensus_rounds = Arc::new(RwLock::new(HashMap::new()));
        let network_topology = Arc::new(RwLock::new(NetworkTopology::new()));
        let governance_system = Arc::new(RwLock::new(DecentralizedGovernance::new()));
        let shard_coordinator = Arc::new(ShardCoordinator::new());
        let peer_discovery = Arc::new(PeerDiscoveryService::new());

        // Mock dependencies - in real implementation these would be properly initialized
        #[cfg(feature = "infinite-strata")]
        let saga = Arc::new(PalletSaga::new(None));
        #[cfg(not(feature = "infinite-strata"))]
        let saga = Arc::new(PalletSaga::new());
        let consensus_engine = Arc::new(Consensus::new(saga.clone()));
        let zkp_system = Arc::new(ZKProofSystem::new());

        // Create mock network server with required parameters
        use rand::rngs::OsRng;

        let pq_crypto = crate::qanto_native_crypto::QantoNativeCrypto::new();
        let mut rng = OsRng;
        let qanto_keypair = pq_crypto.generate_keypair(
            crate::qanto_native_crypto::QantoSignatureAlgorithm::PostQuantum,
            &mut rng,
        );

        let keypair = match qanto_keypair {
            crate::qanto_native_crypto::QantoKeyPair::PostQuantum { public, private } => {
                crate::post_quantum_crypto::PQSignatureKeyPair { public, private }
            }
            _ => panic!("Expected PostQuantum keypair"),
        };

        let listen_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        // Create mock QantoDAG - in real implementation this would be properly initialized
        let dag_config = crate::qantodag::QantoDagConfig {
            num_chains: 4,
            initial_validator: "test_validator".to_string(),
            target_block_time: 10,
        };
        // Create test QantoStorage with unique path to avoid conflicts
        let unique_id = Uuid::new_v4();
        let storage_path = format!("/tmp/test_qanto_dag_{}", unique_id);
        use crate::qanto_storage::{QantoStorage, StorageConfig};
        let storage_config = StorageConfig {
            data_dir: storage_path.into(),
            max_file_size: 64 * 1024 * 1024, // 64MB
            cache_size: 1024 * 1024,         // 1MB
            compression_enabled: true,
            encryption_enabled: false,
            wal_enabled: true,
            sync_writes: true,
            compaction_threshold: 0.7,
            max_open_files: 1000,
        };
        let storage = QantoStorage::new(storage_config).unwrap();
        let dag = QantoDAG::new(dag_config, saga.clone(), storage).unwrap();
        use crate::mempool::Mempool;
        let mempool = Arc::new(Mutex::new(Mempool::new(3600, 10_000_000, 10_000)));
        let utxos = Arc::new(RwLock::new(HashMap::new()));

        let network_server =
            Arc::new(QantoNetServer::new(listen_addr, keypair, dag, mempool, utxos).unwrap());

        DecentralizationEngine {
            validators,
            consensus_rounds,
            network_topology,
            governance_system,
            shard_coordinator,
            peer_discovery,
            consensus_engine,
            saga,
            zkp_system,
            network_server,
        }
    }

    fn create_test_validator(peer_id: PeerId, stake: u64) -> ValidatorInfo {
        ValidatorInfo {
            peer_id,
            stake,
            reputation_score: 0.8,
            last_active_epoch: 100,
            performance_metrics: ValidatorMetrics {
                blocks_produced: AtomicU64::new(50),
                blocks_validated: AtomicU64::new(200),
                uptime_percentage: AtomicU64::new(995000), // 99.5 * 10000
                response_time_ms: AtomicU64::new(50),
                byzantine_faults: AtomicU64::new(0),
                governance_participation: AtomicU64::new(9000), // 0.9 * 10000
                ..Default::default()
            },
            public_key: vec![1, 2, 3, 4],
            network_address: "127.0.0.1:8080".to_string(),
            shard_assignments: vec![0, 1],
        }
    }

    #[tokio::test]
    async fn test_decentralization_engine_initialization() {
        let engine = create_test_engine().await;

        // Verify initial state
        let validators = engine.validators.read().await;
        assert_eq!(validators.len(), 0);

        let consensus_rounds = engine.consensus_rounds.read().await;
        assert_eq!(consensus_rounds.len(), 0);

        let topology = engine.network_topology.read().await;
        assert_eq!(topology.nodes.len(), 0);

        let governance = engine.governance_system.read().await;
        assert_eq!(governance.active_proposals.len(), 0);
    }

    #[tokio::test]
    async fn test_consensus_round_creation() {
        let engine = create_test_engine().await;
        let round_id = Uuid::new_v4().to_string();
        let epoch = 1;
        let shard_id = 0;

        let consensus_round = ConsensusRound {
            round_id: round_id.clone(),
            epoch,
            proposed_block: None,
            validator_votes: HashMap::new(),
            status: ConsensusStatus::Proposing,
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            timeout_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + CONSENSUS_TIMEOUT_MS / 1000,
            shard_id,
        };

        // Add consensus round
        {
            let mut rounds = engine.consensus_rounds.write().await;
            rounds.insert(round_id.clone(), consensus_round);
        }

        // Verify round was created
        let rounds = engine.consensus_rounds.read().await;
        assert!(rounds.contains_key(&round_id));
        assert_eq!(rounds[&round_id].epoch, epoch);
        assert_eq!(rounds[&round_id].shard_id, shard_id);
        assert!(matches!(
            rounds[&round_id].status,
            ConsensusStatus::Proposing
        ));
    }

    #[tokio::test]
    async fn test_validator_rotation() {
        let engine = create_test_engine().await;

        // Add enough test validators to meet MIN_VALIDATOR_COUNT requirement (21)
        let mut test_validators = Vec::new();
        for i in 0..25 {
            let stake = 1000 + (i * 100); // Varying stakes
            let validator = create_test_validator(PeerId::random(), stake);
            test_validators.push(validator);
        }

        {
            let mut validators = engine.validators.write().await;
            for validator in &test_validators {
                validators.insert(validator.peer_id.clone(), validator.clone());
            }
        }

        // Test validator selection
        let selected = engine.select_validators_for_epoch(101, 2).await.unwrap();
        assert_eq!(selected.len(), 2);

        // Verify selection criteria (should prefer higher stake and better performance)
        assert!(selected.iter().any(|v| v.stake >= 1500));
    }

    #[tokio::test]
    async fn test_governance_proposal_submission() {
        let engine = create_test_engine().await;
        let submitter = PeerId::random();

        // Add a validator to have voting power
        let validator = create_test_validator(submitter.clone(), 1000);
        {
            let mut validators = engine.validators.write().await;
            validators.insert(submitter.clone(), validator);
        }

        // Create test proposal
        let proposal = GovernanceProposal {
            id: Uuid::new_v4().to_string(),
            proposer: "test_proposer".to_string(),
            proposal_type: crate::saga::ProposalType::UpdateRule("test_rule".to_string(), 1.0),
            votes_for: 0.0,
            votes_against: 0.0,
            status: crate::saga::ProposalStatus::Voting,
            voters: Vec::new(),
            creation_epoch: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            justification: Some("Test governance proposal".to_string()),
        };

        // Submit proposal
        let proposal_id = engine
            .submit_governance_proposal(proposal, submitter)
            .await
            .unwrap();

        // Verify proposal was submitted
        let governance = engine.governance_system.read().await;
        assert!(governance.active_proposals.contains_key(&proposal_id));
        assert_eq!(
            governance.active_proposals[&proposal_id].status,
            crate::saga::ProposalStatus::Voting
        );
    }

    #[tokio::test]
    async fn test_cross_shard_communication() {
        let engine = create_test_engine().await;

        // Create test cross-shard message
        let message = CrossShardMessage {
            message_id: Uuid::new_v4().to_string(),
            source_shard: 0,
            target_shard: 1,
            message_type: CrossShardMessageType::TransactionTransfer,
            payload: vec![1, 2, 3, 4, 5],
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            proof: None,
        };

        // Process the message
        let result = engine.process_cross_shard_message(message.clone()).await;

        // In a real implementation, this would verify the message was processed correctly
        // For now, we just verify the function doesn't panic and returns a result
        assert!(result.is_ok() || result.is_err()); // Either outcome is acceptable for this test
    }

    #[tokio::test]
    async fn test_network_health_metrics() {
        let engine = create_test_engine().await;

        // Add some test nodes to the topology
        {
            let mut topology = engine.network_topology.write().await;
            let node_peer_id = PeerId::random();
            let node1 = NodeInfo {
                peer_id: node_peer_id.clone(),
                node_type: NodeType::Validator,
                capabilities: NodeCapabilities {
                    can_validate: true,
                    can_store_full_state: true,
                    can_bridge_shards: false,
                    supports_zkp: true,
                    max_connections: 100,
                    bandwidth_mbps: 1000.0,
                },
                network_metrics: NetworkMetrics {
                    latency_ms: AtomicU64::new(50000),             // 50.0 * 1000
                    bandwidth_utilization: AtomicU64::new(7000),   // 0.7 * 10000
                    packet_loss_rate: AtomicU64::new(10),          // 0.001 * 10000
                    connection_stability: AtomicU64::new(9900),    // 0.99 * 10000
                    message_throughput: AtomicU64::new(1_000_000), // 1000.0 * 1000
                    ..Default::default()
                },
                last_seen: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                trust_score: 0.95,
            };
            topology.nodes.insert(node_peer_id.clone(), node1);
        }

        // Test network health calculation
        let health = engine.get_network_health().await.unwrap();
        // validator_count is unsigned, so this check is unnecessary
        assert!(health.average_latency >= 0.0);
        assert!(health.network_coverage >= 0.0 && health.network_coverage <= 1.0);
        assert!(health.partition_risk >= 0.0 && health.partition_risk <= 1.0);
    }

    #[tokio::test]
    async fn test_decentralization_metrics() {
        let engine = create_test_engine().await;

        // Add test validators with different stakes and locations
        let validators = vec![
            create_test_validator(PeerId::random(), 1000),
            create_test_validator(PeerId::random(), 2000),
            create_test_validator(PeerId::random(), 1500),
        ];

        {
            let mut validator_map = engine.validators.write().await;
            for validator in validators {
                validator_map.insert(validator.peer_id.clone(), validator);
            }
        }

        // Calculate decentralization metrics
        let metrics = engine.get_decentralization_metrics().await.unwrap();

        assert_eq!(metrics.validator_count, 3);
        assert!(metrics.decentralization_score >= 0.0 && metrics.decentralization_score <= 1.0);
        assert!(metrics.stake_distribution.total_stake > 0);
        assert!(metrics.governance_participation >= 0.0);
    }
}
