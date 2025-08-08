// qanto/src/infinite_strata_node.rs
//! Infinite Strata Node (refactor) — SAGA / Qanto integration.
//! - Preserves original core logic (heartbeat, auction, decay, reward flow, AEC).
//! - Improves sysinfo resource sampling accuracy and increases decay-history smoothing.
//! - Fixes Send / borrow issues so this file compiles when spawned with tokio.

use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, instrument, warn};
use uuid::Uuid;

/// Minimum heartbeat interval in seconds (original constant preserved).
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
}

/// Node configuration containing tunables used by the ISN.
/// Kept original fields, with decay_history_len increased for smoothing.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub heartbeat_interval: Duration,
    pub max_decay_score: f64,
    pub compliance_failure_decay_rate: f64,
    pub bpf_audit_failure_decay_rate: f64,
    pub regeneration_rate: f64,
    pub decay_history_len: usize,
    // Auction parameters
    pub auction_fee: f64,
    pub max_bid_leverage: f64,
    // Resource governor
    pub optimal_utilization_target: f32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(MIN_UPTIME_HEARTBEAT_SECS),
            max_decay_score: 1.0,
            compliance_failure_decay_rate: 0.05,
            bpf_audit_failure_decay_rate: 0.25,
            regeneration_rate: 0.01,
            // increase smoothing window for predictive failure analysis
            decay_history_len: 128,
            auction_fee: 0.1,
            max_bid_leverage: 2.5,
            optimal_utilization_target: 0.80,
        }
    }
}

// ---
// # 2. Local Node State & types
// ---

/// Node identifier type alias (preserve original behaviour).
pub type NodeId = Uuid;
/// Challenge used for heartbeat challenge/response.
pub type HeartbeatChallenge = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    Active,
    Warned,
    Probation,
}

/// Resource usage snapshot recorded per node.
#[derive(Clone, Debug, Default)]
pub struct ResourceUsage {
    pub cpu: f32,    // normalized 0.0..1.0
    pub memory: f32, // normalized 0.0..1.0
}

/// NodeState: runtime metrics & reputation scoring.
#[derive(Clone, Debug)]
pub struct NodeState {
    pub id: NodeId,
    pub status: NodeStatus,
    pub total_uptime_ticks: u64,
    pub decay_score: f64,
    pub decay_score_history: VecDeque<f64>,
    pub last_slash_amount: u64,
    pub resources: Arc<RwLock<ResourceUsage>>,
    pub last_won_auction: bool,
}

impl NodeState {
    fn new(cfg: &NodeConfig) -> Self {
        NodeState {
            id: Uuid::new_v4(),
            status: NodeStatus::Active,
            total_uptime_ticks: 0,
            decay_score: cfg.max_decay_score,
            decay_score_history: VecDeque::with_capacity(cfg.decay_history_len),
            last_slash_amount: 0,
            resources: Arc::new(RwLock::new(ResourceUsage::default())),
            last_won_auction: false,
        }
    }
}

// ---
// # 3. Security & small primitives
// ---

#[derive(Debug)]
struct SecureChannel<T> {
    inner: T,
}
impl<T> SecureChannel<T> {
    fn new(inner: T) -> Self {
        Self { inner }
    }
    async fn receive(&self) -> &T {
        &self.inner
    }
}

/// Deterministic solver for heartbeat (kept SHA256).
fn solve_challenge(challenge: &HeartbeatChallenge, node_id: &NodeId) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(challenge);
    hasher.update(node_id.as_bytes());
    let res = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&res[..]);
    out
}

// ---
// # 4. Decentralized services
// ---

#[derive(Clone, Debug)]
pub struct DecentralizedOracleAggregator {
    pub total_slashed_pool: Arc<RwLock<u64>>,
}

impl DecentralizedOracleAggregator {
    pub fn new() -> Self {
        Self {
            total_slashed_pool: Arc::new(RwLock::new(500)),
        }
    }

    /// Issue heartbeat challenge (deterministic from pool).
    pub async fn issue_heartbeat_challenge(&self) -> HeartbeatChallenge {
        let pool = *self.total_slashed_pool.read().await;
        let digest = Sha256::digest(&pool.to_le_bytes());
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }
}

// ---
// # 5. Small functional modules
// ---

mod resource_governor {
    use super::*;
    /// Simple governor throttle: if resources approach limits, pause briefly.
    #[allow(dead_code)]
    pub async fn govern_workload(resources: &ResourceUsage) {
        if resources.cpu > 0.95 || resources.memory > 0.95 {
            warn!(cpu = ?resources.cpu, memory = ?resources.memory, "Approaching resource limits. Throttling work.");
            sleep(Duration::from_millis(500)).await;
        }
    }
}

mod block_auctioneer {
    use super::*;
    pub type Bid = (f64, u64);

    pub fn place_bid(state: &NodeState, config: &NodeConfig, potential_reward: u64) -> Bid {
        let collateral = (potential_reward as f64 * 0.25 * state.decay_score) as u64;
        let leverage = 1.0 + (config.max_bid_leverage * state.decay_score);
        let bid_score = (collateral as f64 * leverage) - config.auction_fee;
        ((bid_score.max(0.0)), collateral)
    }

    pub fn run_auction(_our_bid: Bid) -> bool {
        // Use StdRng local to the function; it's Send and short-lived.
        let mut rng = StdRng::from_entropy();
        rng.gen_bool(0.8)
    }
}

mod reward_calculator {
    use super::*;
    /// Calculate base reward split and winner/general splits. Preserves original behavior.
    #[instrument(skip(state, _config, network_health, risk_factor))]
    pub async fn calculate(
        state: &NodeState,
        _config: &NodeConfig,
        network_health: (u64, u64, f64),
        risk_factor: f64,
    ) -> (f64, u64, u64) {
        let (base_supply, slashed_pool, _volatility) = network_health;
        let uptime_bonus = (state.total_uptime_ticks as f64 / 3600.0).min(10.0) * 0.01;
        let base_multiplier = (state.decay_score + uptime_bonus).clamp(0.0, 1.5);
        let base = (base_supply as f64) * base_multiplier * (1.0 - risk_factor);
        let general = ((slashed_pool as f64) * 0.4) as u64;
        let winner = ((slashed_pool as f64) * 0.6) as u64;
        (base, general, winner)
    }
}

// ---
// # 6. Autonomous economic calibrator (AEC)
// ---

#[derive(Clone, Debug)]
pub struct AutonomousEconomicCalibrator {}

impl AutonomousEconomicCalibrator {
    pub fn new() -> Self {
        Self {}
    }

    #[instrument(skip(self, config))]
    pub async fn calibrate(&self, config: &mut NodeConfig, network_health: (u64, u64, f64)) {
        let (_, _, volatility) = network_health;
        if volatility > 1.5 {
            info!("AEC: High volatility detected. Entering stabilization mode.");
            config.auction_fee = 0.05;
            config.optimal_utilization_target = 0.70;
        } else {
            info!("AEC: Network stable. Operating under standard parameters.");
            let default = NodeConfig::default();
            config.auction_fee = default.auction_fee;
            config.optimal_utilization_target = default.optimal_utilization_target;
        }
    }
}

// ---
// # 7. Infinite Strata Node (public)
// ---

#[derive(Debug)]
pub struct InfiniteStrataNode {
    pub config: RwLock<NodeConfig>,
    pub state: RwLock<NodeState>,
    pub system: Arc<RwLock<System>>,
    governance_channel: SecureChannel<SagaGovernanceClient>,
    pub oracle_aggregator: DecentralizedOracleAggregator,
    pub aec: AutonomousEconomicCalibrator,
}

#[derive(Debug)]
pub struct SagaGovernanceClient {}

impl SagaGovernanceClient {
    pub fn new() -> Self {
        SagaGovernanceClient {}
    }

    /// Predictive failure analysis: returns risk factor in [0.0, 1.0].
    pub fn predictive_failure_analysis(&self, history: &VecDeque<f64>) -> f64 {
        if history.is_empty() {
            0.0
        } else {
            let avg: f64 = history.iter().sum::<f64>() / (history.len() as f64);
            (1.0 - avg).clamp(0.0, 1.0)
        }
    }
}

impl InfiniteStrataNode {
    /// Create a new node instance with provided config and oracle aggregator.
    pub fn new(config: NodeConfig, oracle_aggregator: DecentralizedOracleAggregator) -> Self {
        let initial_state = NodeState::new(&config);
        info!(node_id = %initial_state.id, "SAGA Titan Node Initialized.");
        Self {
            config: RwLock::new(config),
            state: RwLock::new(initial_state),
            system: Arc::new(RwLock::new(System::new_all())),
            governance_channel: SecureChannel::new(SagaGovernanceClient::new()),
            oracle_aggregator,
            aec: AutonomousEconomicCalibrator::new(),
        }
    }

    /// Start the main heartbeat loop (async).
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let interval_duration = {
            let cfg = self.config.read().await;
            cfg.heartbeat_interval
        };

        let mut interval = tokio::time::interval(interval_duration);
        loop {
            interval.tick().await;
            let node_clone = self.clone();
            // tokio::spawn requires the future to be Send; ensure perform_heartbeat_cycle uses only Send locals
            tokio::spawn(async move {
                if let Err(e) = node_clone.perform_heartbeat_cycle().await {
                    warn!(error = ?e, "Heartbeat cycle returned error");
                }
            });
        }
    }

    /// Perform a single heartbeat cycle.
    pub async fn perform_heartbeat_cycle(&self) -> Result<(), NodeError> {
        // 1) Issue challenge & proof
        let challenge = self.oracle_aggregator.issue_heartbeat_challenge().await;
        let node_id = self.state.read().await.id;
        let _proof = solve_challenge(&challenge, &node_id);

        // 2) Sample resources: clone resources Arc then update inner fields (avoid holding NodeState guard across awaits)
        let resources_arc = {
            let st = self.state.read().await;
            st.resources.clone()
        };

        {
            let mut sys = self.system.write().await;
            // double refresh for more accurate CPU sampling on certain sysinfo versions
            sys.refresh_cpu();
            sys.refresh_cpu();
            sys.refresh_memory();

            let cpu_usage = sys.global_cpu_info().cpu_usage(); // 0.0..100.0
            let mem_usage = if sys.total_memory() > 0 {
                sys.used_memory() as f32 / sys.total_memory() as f32
            } else {
                0.0
            };

            let mut resources = resources_arc.write().await;
            resources.cpu = (cpu_usage / 100.0).clamp(0.0, 1.0);
            resources.memory = mem_usage.clamp(0.0, 1.0);
        }

        // 3) Governance predictive failure analysis (reads decay history)
        let saga_client = self.governance_channel.receive().await;
        let history = self.state.read().await.decay_score_history.clone();
        let risk_factor = saga_client.predictive_failure_analysis(&history);

        // 4) Compliance simulation — use StdRng which is Send
        let mut rng = StdRng::from_entropy();
        let is_compliant = rng.gen_bool(0.95);
        let bpf_audit_passed = rng.gen_bool(0.98);

        // update decay & status (async)
        self.update_decay_score(is_compliant, bpf_audit_passed)
            .await;
        self.update_node_status().await;

        // 5) Auction & rewards
        let network_health = (10000u64, 200u64, 0.5f64); // placeholder
        let potential_reward = network_health.1 / 2;
        let bid = {
            let st = self.state.read().await;
            let cfg = self.config.read().await;
            block_auctioneer::place_bid(&st, &cfg, potential_reward)
        };
        let we_won = block_auctioneer::run_auction(bid);
        {
            let mut st = self.state.write().await;
            st.last_won_auction = we_won;
        }

        let (base, general, winner) = {
            let st = self.state.read().await;
            let cfg = self.config.read().await;
            reward_calculator::calculate(&st, &cfg, network_health, risk_factor).await
        };

        // deduct from slashed pool safely
        {
            let mut pool = self.oracle_aggregator.total_slashed_pool.write().await;
            *pool = pool.saturating_sub(general).saturating_sub(winner);
        }

        info!(
            total_reward = base + (general + winner) as f64,
            "Heartbeat cycle complete."
        );

        // 6) AEC calibration
        {
            let mut cfg = self.config.write().await;
            self.aec.calibrate(&mut cfg, network_health).await;
        }

        Ok(())
    }

    /// Update decay score (reads config first to avoid borrow conflicts).
    pub async fn update_decay_score(&self, is_compliant: bool, bpf_audit_passed: bool) {
        // clone config to avoid holding both locks at once
        let config = self.config.read().await.clone();

        // acquire state mutably and compute new decay in local vars to avoid transient borrows
        let mut state = self.state.write().await;
        let mut new_score = state.decay_score;

        if is_compliant {
            new_score += config.regeneration_rate;
            state.total_uptime_ticks = state.total_uptime_ticks.saturating_add(1);
        } else {
            new_score -= config.compliance_failure_decay_rate;
        }

        if !bpf_audit_passed {
            new_score -= config.bpf_audit_failure_decay_rate;
        }

        // clamp
        let clamped = new_score.clamp(0.0, config.max_decay_score);
        state.decay_score = clamped;

        // update history window
        if state.decay_score_history.len() == config.decay_history_len {
            // pop front first
            state.decay_score_history.pop_front();
        }
        state.decay_score_history.push_back(clamped);
    }

    /// Update node status and apply probation penalties asynchronously.
    pub async fn update_node_status(&self) {
        let mut state = self.state.write().await;
        let old_status = state.status;
        state.status = if state.decay_score < 0.2 {
            NodeStatus::Probation
        } else if state.decay_score < 0.6 {
            NodeStatus::Warned
        } else {
            NodeStatus::Active
        };

        if state.status == NodeStatus::Probation && old_status != NodeStatus::Probation {
            // take snapshot for calculation, then operate on oracle in spawned task
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

    /// Run a single periodic check (public wrapper).
    pub async fn run_periodic_check(self: &Arc<Self>) -> Result<(), NodeError> {
        self.perform_heartbeat_cycle().await
    }

    /// Get rewards multiplier (public).
    pub async fn get_rewards(&self) -> (f64, u64) {
        let state = self.state.read().await;
        let uptime_bonus = (state.total_uptime_ticks as f64 / 3600.0).min(10.0) * 0.01;
        let base_multiplier = (state.decay_score + uptime_bonus).clamp(0.0, 1.5);
        (base_multiplier, 0)
    }
}

// --- End of file
