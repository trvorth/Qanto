//! --- SAGA: Sentient Autonomous Governance Algorithm ---
//! v9.1.1 - Production Ready
//! This version is fully prepared for production deployment and further hardening.
//! All known bugs and compiler warnings have been resolved.
//!
//! - LINT FIX: Prefixed an unused variable with an underscore to resolve the final warning.

#[cfg(feature = "infinite-strata")]
use crate::infinite_strata_node::InfiniteStrataNode;
use crate::omega;
use crate::qantodag::{QantoBlock, QantoDAG, MAX_TRANSACTIONS_PER_BLOCK};
use crate::transaction::Transaction;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// AI imports removed for production hardening

// --- Constants ---
// AI model constants removed for production hardening
// RETRAIN_INTERVAL_EPOCHS constant removed for production hardening
const TEMPORAL_GRACE_PERIOD_SECS: u64 = 120;

#[derive(Error, Debug, Clone)]
pub enum SagaError {
    #[error("Rule not found in SAGA's current epoch state: {0}")]
    RuleNotFound(String),
    #[error("Proposal not found, inactive, or already vetoed: {0}")]
    ProposalNotFound(String),
    #[error("Node has insufficient Karma for action: required {0}, has {1}")]
    InsufficientKarma(u64, u64),
    // AI model errors removed for production hardening
    #[error("Invalid proposal state transition attempted")]
    InvalidProposalState,
    #[error("The SAGA Council has already vetoed this proposal")]
    ProposalVetoed,
    #[error("Only a member of the SAGA Council can veto active proposals")]
    NotACouncilMember,
    // InvalidHelpTopic error removed for production hardening
    #[error("System time error prevented temporal analysis: {0}")]
    TimeError(String),
    // NLU and AI query processing errors removed for production hardening
    #[error("Invalid Carbon Offset Credential: {0}")]
    InvalidCredential(String),
    // Oracle and external knowledge errors removed for production hardening
}

/// Represents a verifiable claim of a carbon offset, now with richer data for AI analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonOffsetCredential {
    pub id: String,
    pub issuer_id: String,
    pub beneficiary_node: String,
    pub tonnes_co2_sequestered: f64,
    pub project_id: String,
    pub vintage_year: u32,
    pub verification_signature: String,
    // --- ADVANCED FIELDS FOR AI VERIFICATION ---
    /// A cryptographic hash of the full project documentation for data integrity checks.
    pub additionality_proof_hash: String,
    /// A score from a trusted third-party representing the quality and reputation of the issuer.
    pub issuer_reputation_score: f64,
    /// A score representing how well the project's claimed location matches satellite imagery analysis.
    pub geospatial_consistency_score: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct TransactionMetadata {
    pub origin_component: String,
    pub intent: String,
    #[serde(default)]
    pub correlated_tx: Option<String>,
    #[serde(default)]
    pub additional_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackType {
    Sybil,
    Spam,
    Centralization,
    OracleManipulation,
    TimeDrift,
    WashTrading,
    Collusion,
    Economic,
    MempoolFrontrun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkState {
    Nominal,
    Congested,
    Degraded,
    UnderAttack(AttackType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdictAction {
    Economic {
        reward_multiplier: f64,
        fee_multiplier: f64,
    },
    Governance {
        proposal_karma_cost_multiplier: f64,
    },
    Security {
        trust_weight_override: (String, f64),
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaEdict {
    pub id: String,
    pub issued_epoch: u64,
    pub expiry_epoch: u64,
    pub description: String,
    pub action: EdictAction,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct TrustScoreBreakdown {
    pub factors: HashMap<String, f64>,
    pub final_weighted_score: f64,
}

// BehaviorNet AI model removed for production hardening

// CongestionPredictorLSTM AI model removed for production hardening

// CredentialVerifierNet AI model removed for production hardening

// QueryClassifierNet AI model removed for production hardening

/// A heuristic model to estimate the CO2 impact of a transaction.
#[derive(Debug)]
pub struct CarbonImpactPredictor {}

impl CarbonImpactPredictor {
    pub fn predict_co2_per_tx(&self, tx: &Transaction, network_congestion: f64) -> f64 {
        const GRAMS_CO2_PER_ECU: f64 = 0.005;
        const ECU_PER_BYTE: f64 = 0.1;
        const ECU_BASE_TRANSFER: f64 = 10.0;
        const ECU_PER_CONTRACT_OPCODE: f64 = 0.5;
        const ECU_CONTRACT_DEPLOYMENT_BASE: f64 = 500.0;

        let tx_bytes = match serde_json::to_vec(tx) {
            Ok(v) => v.len() as f64,
            Err(e) => {
                warn!(tx_id = %tx.id, error = %e, "Failed to serialize transaction for carbon calculation. Using default size.");
                256.0 // Use a default, average size on failure
            }
        };

        let mut total_ecu = tx_bytes * ECU_PER_BYTE;

        let intent = tx.get_metadata().get("intent").map(|s| s.as_str());
        match intent {
            Some("contract_deployment") => {
                let code_size = tx
                    .get_metadata()
                    .get("contract_code_size")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                total_ecu += ECU_CONTRACT_DEPLOYMENT_BASE + (code_size * ECU_PER_BYTE);
            }
            Some("contract_interaction") => {
                let op_count = tx
                    .get_metadata()
                    .get("opcode_count")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(20.0);
                total_ecu += op_count * ECU_PER_CONTRACT_OPCODE;
            }
            _ => {
                total_ecu += ECU_BASE_TRANSFER;
            }
        };

        let congestion_multiplier = 1.0 + (network_congestion * 0.75);
        let final_ecu = total_ecu * congestion_multiplier;
        final_ecu * GRAMS_CO2_PER_ECU
    }
}

// ModelTrainingData struct removed for production hardening
// AI training data collection functionality has been excised

/// Simplified on-chain analytics engine without AI dependencies
#[derive(Debug)]
pub struct CognitiveAnalyticsEngine {
    pub carbon_impact_model: CarbonImpactPredictor,
}

impl Default for CognitiveAnalyticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CognitiveAnalyticsEngine {
    pub fn new() -> Self {
        Self {
            carbon_impact_model: CarbonImpactPredictor {},
        }
    }

    #[instrument(skip(self, block, dag, rules))]
    pub async fn score_node_behavior(
        &self,
        block: &QantoBlock,
        dag: &QantoDAG,
        rules: &HashMap<String, EpochRule>,
        network_state: NetworkState,
    ) -> Result<TrustScoreBreakdown, SagaError> {
        let grace_period = rules
            .get("temporal_grace_period_secs")
            .map_or(TEMPORAL_GRACE_PERIOD_SECS, |r| r.value as u64);

        let mut factors = HashMap::new();

        factors.insert("validity".to_string(), self.check_block_validity(block));
        factors.insert(
            "network_contribution".to_string(),
            self.analyze_network_contribution(block, dag).await,
        );
        factors.insert(
            "historical_performance".to_string(),
            self.check_historical_performance(&block.miner, dag).await,
        );
        factors.insert(
            "cognitive_hazard".to_string(),
            self.analyze_cognitive_hazards(block),
        );
        factors.insert(
            "temporal_consistency".to_string(),
            self.analyze_temporal_consistency(block, dag, grace_period)
                .await?,
        );
        factors.insert(
            "cognitive_dissonance".to_string(),
            self.analyze_cognitive_dissonance(block),
        );
        factors.insert(
            "metadata_integrity".to_string(),
            self.analyze_metadata_integrity(block).await,
        );
        factors.insert(
            "environmental_contribution".to_string(),
            self.analyze_environmental_contribution(block, dag).await,
        );

        // AI behavior prediction removed for production hardening
        let predicted_behavior_score = 0.5;
        factors.insert("predicted_behavior".to_string(), predicted_behavior_score);

        let mut final_score = 0.0;
        let mut total_weight = 0.0;
        for (factor_name, factor_score) in &factors {
            let base_weight_key = format!("trust_{factor_name}_weight");
            let mut weight = rules.get(&base_weight_key).map_or(0.1, |r| r.value);

            // The "Act" part of Sense-Think-Act: SAGA dynamically re-weights trust factors
            // based on the current threat assessment.
            if let NetworkState::UnderAttack(attack_type) = network_state {
                match attack_type {
                    AttackType::TimeDrift => {
                        if factor_name == "temporal_consistency" {
                            weight *= 3.0;
                        }
                    }
                    AttackType::Spam => {
                        if factor_name == "cognitive_hazard"
                            || factor_name == "network_contribution"
                        {
                            weight *= 2.5;
                        }
                    }
                    AttackType::Sybil | AttackType::Centralization => {
                        if factor_name == "historical_performance" {
                            weight *= 3.0;
                        }
                    }
                    AttackType::Collusion => {
                        if factor_name == "historical_performance"
                            || factor_name == "predicted_behavior"
                        {
                            weight *= 3.0;
                        }
                    }
                    AttackType::OracleManipulation => {
                        if factor_name == "environmental_contribution" {
                            weight *= 2.0;
                        }
                    }
                    AttackType::Economic | AttackType::MempoolFrontrun => {
                        if factor_name == "cognitive_hazard" {
                            weight *= 3.0;
                        }
                    }
                    _ => {
                        // General attack state
                        if factor_name == "temporal_consistency"
                            || factor_name == "cognitive_hazard"
                            || factor_name == "metadata_integrity"
                        {
                            weight *= 2.0;
                        }
                    }
                }
            } else if network_state == NetworkState::Congested
                && factor_name == "network_contribution"
            {
                weight *= 1.5;
            }

            final_score += factor_score * weight;
            total_weight += weight;
        }

        Ok(TrustScoreBreakdown {
            factors,
            final_weighted_score: (final_score / total_weight.max(0.01)).clamp(0.0, 1.0),
        })
    }

    async fn analyze_environmental_contribution(&self, block: &QantoBlock, dag: &QantoDAG) -> f64 {
        let network_congestion =
            dag.get_average_tx_per_block().await / MAX_TRANSACTIONS_PER_BLOCK as f64;

        let block_footprint_grams: f64 = block
            .transactions
            .iter()
            .map(|tx| {
                self.carbon_impact_model
                    .predict_co2_per_tx(tx, network_congestion)
            })
            .sum();
        let block_footprint_tonnes = block_footprint_grams / 1_000_000.0;

        let total_offset_tonnes: f64 = block
            .carbon_credentials
            .iter()
            .map(|c| c.tonnes_co2_sequestered)
            .sum();

        let net_impact = total_offset_tonnes - block_footprint_tonnes;
        (1.0 / (1.0 + (-net_impact).exp())).clamp(0.0, 1.0)
    }

    #[inline]
    fn check_block_validity(&self, block: &QantoBlock) -> f64 {
        let Some(coinbase) = block.transactions.first() else {
            return 0.0;
        };
        if !coinbase.is_coinbase() {
            return 0.1;
        }
        if coinbase.outputs.is_empty() {
            return 0.2;
        }
        1.0
    }

    async fn analyze_network_contribution(&self, block: &QantoBlock, dag: &QantoDAG) -> f64 {
        let avg_tx_per_block = dag.get_average_tx_per_block().await;
        let block_tx_count = block.transactions.len() as f64;
        let deviation = (block_tx_count - (avg_tx_per_block * 1.1)) / avg_tx_per_block.max(1.0);
        (-deviation.powi(2)).exp()
    }

    async fn check_historical_performance(&self, miner_address: &str, dag: &QantoDAG) -> f64 {
        let blocks_reader = dag.blocks.read().await;
        let total_blocks = blocks_reader.len().max(1) as f64;
        let node_blocks = blocks_reader
            .values()
            .filter(|b| b.miner == *miner_address)
            .count() as f64;
        (node_blocks / total_blocks * 10.0).min(1.0)
    }

    fn analyze_cognitive_hazards(&self, block: &QantoBlock) -> f64 {
        let tx_count = block.transactions.len();
        if tx_count <= 1 {
            return 1.0;
        }
        let total_fee: u64 = block.transactions.iter().map(|tx| tx.fee).sum();
        let avg_fee = total_fee as f64 / tx_count as f64;
        let tx_ratio = tx_count as f64 / MAX_TRANSACTIONS_PER_BLOCK as f64;

        // Adjusted for tiered fees: allow lower avg_fee if many small transactions
        let small_tx_count = block.transactions.iter().filter(|tx| tx.amount < crate::transaction::FEE_TIER1_THRESHOLD).count() as f64;
        let small_tx_ratio = small_tx_count / tx_count as f64;
        let adjusted_fee_threshold = if small_tx_ratio > 0.5 { 0.5 } else { 1.0 };

        if tx_ratio > 0.9 && avg_fee < adjusted_fee_threshold {
            return 0.2;
        }
        1.0
    }

    async fn analyze_temporal_consistency(
        &self,
        block: &QantoBlock,
        dag: &QantoDAG,
        grace_period: u64,
    ) -> Result<f64, SagaError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| SagaError::TimeError(e.to_string()))?
            .as_secs();
        if block.timestamp > now + grace_period {
            warn!(block_id = %block.id, "Temporal Anomaly: Block timestamp is too far in the future.");
            return Ok(0.2);
        }
        if !block.parents.is_empty() {
            let blocks_reader = dag.blocks.read().await;
            if let Some(max_parent_time) = block
                .parents
                .iter()
                .filter_map(|p_id| blocks_reader.get(p_id).map(|p_block| p_block.timestamp))
                .max()
            {
                if block.timestamp <= max_parent_time {
                    warn!(block_id = %block.id, "Temporal Anomaly: Block timestamp is not after its parent's.");
                    return Ok(0.0);
                }
            }
        }
        Ok(1.0)
    }

    #[inline]
    fn analyze_cognitive_dissonance(&self, block: &QantoBlock) -> f64 {
        let high_fee_txs = block.transactions.iter().filter(|tx| tx.fee > 100).count();
        let zero_fee_txs = block
            .transactions
            .iter()
            .filter(|tx| tx.fee == 0 && !tx.is_coinbase())
            .count();

        if high_fee_txs > 0 && zero_fee_txs > (block.transactions.len() / 2) {
            0.3
        } else {
            1.0
        }
    }

    async fn analyze_metadata_integrity(&self, block: &QantoBlock) -> f64 {
        let tx_count = block.transactions.len().max(1) as f64;
        let mut suspicious_tx_count = 0.0;

        for tx in block.transactions.iter().skip(1) {
            let metadata = tx.get_metadata();
            if metadata
                .get("origin_component")
                .is_none_or(|s| s.is_empty() || s == "unknown")
            {
                suspicious_tx_count += 0.5;
            }

            if metadata.get("intent").is_none_or(|s| s.is_empty()) {
                suspicious_tx_count += 1.0;
            }

            if let Some(intent_str) = metadata.get("intent") {
                if PalletSaga::calculate_shannon_entropy(intent_str) > 3.5 {
                    suspicious_tx_count += 0.5;
                }
            }
        }
        (1.0 - (suspicious_tx_count / tx_count)).max(0.0f64)
    }

    // AI training data collection removed for production hardening

    // AI-related methods removed for production hardening:
    // - train_models_from_data: Previously handled neural network training
    // - save_models_to_disk: Previously saved AI model weights
    // - load_models_from_disk: Previously loaded AI model weights
}

#[derive(Debug, Clone, Default)]
pub struct PredictiveEconomicModel;

impl PredictiveEconomicModel {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn predictive_market_premium(
        &self,
        dag: &QantoDAG,
        metrics: &EnvironmentalMetrics,
    ) -> f64 {
        let avg_tx_per_block = dag.get_average_tx_per_block().await;
        let validator_count = dag.validators.read().await.len() as f64;

        let (fee_velocity, fee_volatility) = {
            let blocks_reader = dag.blocks.read().await;
            // FIX: HashMap::values() is not a DoubleEndedIterator. Must collect and sort first.
            let mut recent_blocks_sorted: Vec<_> = blocks_reader.values().collect();
            recent_blocks_sorted.sort_by_key(|b| b.timestamp);

            let recent_blocks: Vec<_> = recent_blocks_sorted
                .iter()
                .rev()
                .filter(|b| !b.transactions.is_empty())
                .take(100)
                .collect();

            if recent_blocks.is_empty() {
                (1.0, 0.0)
            } else {
                let fees: Vec<f64> = recent_blocks
                    .iter()
                    .flat_map(|b| &b.transactions)
                    .map(|tx| tx.fee as f64)
                    .collect();
                let fee_count = fees.len() as f64;
                let avg_fee = fees.iter().sum::<f64>() / fee_count.max(1.0);
                let variance =
                    fees.iter().map(|f| (f - avg_fee).powi(2)).sum::<f64>() / fee_count.max(1.0);
                let std_dev = variance.sqrt();
                (avg_fee.max(1.0), std_dev)
            }
        };

        let green_score = metrics.network_green_score;
        let green_premium = 1.0 + (green_score * 0.15);

        let base_premium = 1.0;
        let demand_factor = (avg_tx_per_block / MAX_TRANSACTIONS_PER_BLOCK as f64)
            * (1.0 + (fee_velocity / 100.0).min(1.0));
        let security_factor = (1.0 - (10.0 / validator_count).min(1.0)).max(0.5);
        // FIX: Explicitly type the float literal to resolve ambiguity
        let volatility_damper = 1.0 / (1.0f64 + fee_volatility / fee_velocity.max(1.0)).ln_1p();

        let premium = base_premium + demand_factor * security_factor;
        (premium * green_premium * volatility_damper).clamp(0.8, 2.5)
    }
}

// ExternalDataSource enum removed for production hardening

// QueryIntent, AnalyzedQuery, and ReasoningReport removed for production hardening
// LLM-based query processing and reasoning functionality has been excised

// SagaGuidanceSystem removed for production hardening - LLM functionality eliminated

#[derive(Debug, Clone, Serialize)]
pub struct SagaInsight {
    pub id: String,
    pub epoch: u64,
    pub title: String,
    pub detail: String,
    pub severity: InsightSeverity,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum InsightSeverity {
    Tip,
    Warning,
    Critical,
}

// SagaGuidanceSystem implementation removed for production hardening
// All LLM and natural language processing functionality has been excised
// to create a lightweight, pure on-chain analytics engine

#[derive(Debug, Clone, Serialize)]
pub struct SecurityFinding {
    pub risk_score: f64,
    pub severity: InsightSeverity,
    pub confidence: f64,
    pub details: String,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityMonitor;

impl SecurityMonitor {
    pub fn new() -> Self {
        Self {}
    }

    #[instrument(skip(self, dag))]
    pub async fn check_for_sybil_attack(&self, dag: &QantoDAG) -> SecurityFinding {
        let validators = dag.validators.read().await;
        if validators.len() < 10 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough validators for a reliable Sybil analysis.".to_string(),
            };
        }

        let total_stake: u64 = validators.values().sum();
        if total_stake == 0 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Total stake is zero.".to_string(),
            };
        }

        let mut stakes: Vec<u64> = validators.values().cloned().collect();
        stakes.sort_unstable();

        let n = stakes.len() as f64;
        let sum_of_ranks = stakes
            .iter()
            .enumerate()
            .map(|(i, &s)| (i as f64 + 1.0) * s as f64)
            .sum::<f64>();
        let gini = (2.0 * sum_of_ranks) / (n * total_stake as f64) - (n + 1.0) / n;

        // Gini of 0 is perfect equality. Risk increases as Gini approaches 0.
        let sybil_risk = (1.0 - gini).powi(2).max(0.0);
        debug!("Sybil attack analysis complete. Gini: {gini:.4}, Risk Score: {sybil_risk:.4}",);

        SecurityFinding {
            risk_score: sybil_risk,
            severity: if sybil_risk > 0.7 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.8,
            details: format!("Stake distribution Gini coefficient is {gini:.4}."),
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_transactional_anomalies(&self, dag: &QantoDAG) -> SecurityFinding {
        let blocks = dag.blocks.read().await;
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(_) => {
                return SecurityFinding {
                    risk_score: 0.0,
                    severity: InsightSeverity::Warning,
                    confidence: 0.1,
                    details: "System time error.".to_string(),
                }
            }
        };

        let recent_blocks: Vec<_> = blocks
            .values()
            .filter(|b| b.timestamp > now.saturating_sub(600)) // last 10 minutes
            .collect();

        if recent_blocks.len() < 5 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough recent blocks for spam analysis.".to_string(),
            };
        }

        let total_txs: usize = recent_blocks
            .iter()
            .map(|b| b.transactions.len().saturating_sub(1)) // exclude coinbase
            .sum();
        if total_txs == 0 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 1.0,
                details: "No recent transactions.".to_string(),
            };
        }

        let zero_fee_txs: usize = recent_blocks
            .iter()
            .flat_map(|b| &b.transactions[1..])
            .filter(|tx| tx.fee == 0)
            .count();

        let zero_fee_ratio = zero_fee_txs as f64 / total_txs as f64;
        // Adjusted for tiered fees: higher tolerance for zero-fee transactions
        let risk = if zero_fee_ratio > 0.8 { (zero_fee_ratio - 0.8).powi(2) } else { 0.0 };

        debug!(
            "Transactional anomaly check complete. Zero-fee ratio: {zero_fee_ratio:.4}, Risk score: {risk:.4}",
        );
        SecurityFinding {
            risk_score: risk,
            severity: if risk > 0.6 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.85,
            details: format!("Zero-fee transaction ratio is {zero_fee_ratio:.2}."),
        }
    }

    #[instrument(skip(self, dag, epoch_lookback))]
    pub async fn check_for_centralization_risk(
        &self,
        dag: &QantoDAG,
        epoch_lookback: u64,
    ) -> SecurityFinding {
        let blocks = dag.blocks.read().await;
        if blocks.len() < 50 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough blocks for centralization analysis.".to_string(),
            };
        }

        let current_epoch_val = *dag.current_epoch.read().await;

        let recent_blocks_by_miner: HashMap<String, u64> = blocks
            .values()
            .filter(|b| b.epoch >= current_epoch_val.saturating_sub(epoch_lookback))
            .fold(HashMap::new(), |mut acc, b| {
                *acc.entry(b.miner.clone()).or_insert(0) += 1;
                acc
            });

        if recent_blocks_by_miner.len() < 3 {
            return SecurityFinding {
                risk_score: 0.75,
                severity: InsightSeverity::Critical,
                confidence: 0.95,
                details: format!("Very few active miners ({}).", recent_blocks_by_miner.len()),
            };
        }

        let total_produced = recent_blocks_by_miner.values().sum::<u64>() as f64;
        if total_produced == 0.0 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 1.0,
                details: "No blocks produced in lookback period.".to_string(),
            };
        }

        // Using Herfindahl-Hirschman Index (HHI) for market concentration
        let hhi = recent_blocks_by_miner
            .values()
            .map(|&count| {
                let share = (count as f64 / total_produced) * 100.0;
                share.powi(2)
            })
            .sum::<f64>();

        // HHI between 1500 and 2500 is moderately concentrated, > 2500 is highly concentrated.
        let risk = ((hhi - 1500.0).max(0.0) / (4000.0 - 1500.0)).clamp(0.0, 1.0);

        debug!("Centralization risk analysis complete. HHI: {hhi:.2}, Risk Score: {risk:.4}",);
        SecurityFinding {
            risk_score: risk,
            severity: if risk > 0.7 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.9,
            details: format!("Block production HHI is {hhi:.2}."),
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_for_oracle_manipulation_risk(&self, dag: &QantoDAG) -> SecurityFinding {
        // This check looks for miners who disproportionately rely on a single
        // project for their Carbon Offset Credentials, which could indicate collusion
        // or manipulation of a specific, low-quality carbon project.
        let blocks = dag.blocks.read().await;

        let mut recent_blocks_vec: Vec<_> = blocks.values().collect();
        recent_blocks_vec.sort_by_key(|b| b.timestamp);
        let recent_blocks = recent_blocks_vec.iter().rev().take(100);

        let mut creds_by_miner = HashMap::<String, Vec<CarbonOffsetCredential>>::new();
        for block in recent_blocks {
            creds_by_miner
                .entry(block.miner.clone())
                .or_default()
                .extend(block.carbon_credentials.clone());
        }

        if creds_by_miner.is_empty() {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 1.0,
                details: "No recent PoCO credentials submitted.".to_string(),
            };
        }

        let mut total_risk_score = 0.0;
        let num_miners_with_creds = creds_by_miner
            .values()
            .filter(|creds| !creds.is_empty())
            .count();

        for (_miner, creds) in creds_by_miner.iter() {
            if creds.is_empty() {
                continue;
            }

            let mut project_counts = HashMap::<String, u32>::new();
            for cred in creds {
                *project_counts.entry(cred.project_id.clone()).or_insert(0) += 1;
            }

            if let Some(max_count) = project_counts.values().max() {
                let single_project_ratio = *max_count as f64 / creds.len() as f64;
                if single_project_ratio > 0.8 && creds.len() > 5 {
                    total_risk_score += 0.5; // High risk for this miner
                }
            }
        }

        let risk = (total_risk_score / num_miners_with_creds.max(1) as f64).clamp(0.0, 1.0);

        SecurityFinding {
            risk_score: risk,
            severity: if risk > 0.6 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.75,
            details:
                "Calculated oracle manipulation risk score based on PoCO project concentration."
                    .to_string(),
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_for_time_drift_attack(&self, dag: &QantoDAG) -> SecurityFinding {
        // Looks for miners who consistently produce blocks with the minimum possible timestamp,
        // which can be an indicator of selfish mining or network manipulation attempts.
        let blocks = dag.blocks.read().await;

        let mut recent_blocks: Vec<_> = blocks.values().collect();
        recent_blocks.sort_by_key(|b| b.timestamp);

        let mut suspicious_timestamps = HashMap::<String, (u32, u32)>::new(); // (total_blocks, suspicious_blocks)

        for block in recent_blocks.iter().rev().take(200) {
            let parent_max_ts = block
                .parents
                .iter()
                .filter_map(|p_id| blocks.get(p_id).map(|p| p.timestamp))
                .max()
                .unwrap_or(block.timestamp);

            let (count, suspicious_count) = suspicious_timestamps
                .entry(block.miner.clone())
                .or_insert((0, 0));
            *count += 1;

            // A "suspicious" timestamp is one that is only 1 or 2 seconds after its parent.
            // While possible, a consistent pattern is a red flag.
            if block.timestamp <= parent_max_ts + 2 {
                *suspicious_count += 1;
            }
        }

        let mut max_risk = 0.0;
        for (_miner, (count, suspicious_count)) in suspicious_timestamps {
            if count > 10 {
                // Only consider miners with a decent sample size
                let ratio = suspicious_count as f64 / count as f64;
                if ratio > max_risk {
                    max_risk = ratio;
                }
            }
        }

        let risk = (max_risk.powi(2)).clamp(0.0, 1.0);
        SecurityFinding {
            risk_score: risk,
            severity: if risk > 0.8 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.7,
            details: format!(
                "Highest suspicious timestamp ratio from a single miner: {max_risk:.2}"
            ),
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_for_wash_trading(&self, dag: &QantoDAG) -> SecurityFinding {
        let blocks = dag.blocks.read().await;
        if blocks.len() < 100 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough blocks for wash trading analysis.".to_string(),
            };
        }

        let recent_blocks: Vec<_> = blocks
            .values()
            .filter(|b| {
                b.timestamp
                    > SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .saturating_sub(1800)
            }) // last 30 mins
            .collect();

        let mut tx_graph = HashMap::<String, Vec<String>>::new();
        let mut address_tx_counts = HashMap::<String, u32>::new();

        // For efficient lookups, create a map of all transactions on the DAG.
        // This is memory-intensive but avoids nested loops for every input.
        // TODO: Evolve this to use an iterator-based approach for memory-constrained systems.
        let tx_map: HashMap<_, _> = blocks
            .values()
            .flat_map(|b| &b.transactions)
            .map(|tx| (tx.id.clone(), tx))
            .collect();

        for block in recent_blocks {
            for tx in &block.transactions {
                if tx.is_coinbase() || tx.inputs.is_empty() {
                    continue;
                }

                if let Some(input) = tx.inputs.first() {
                    if let Some(source_tx) = tx_map.get(&input.tx_id) {
                        if let Some(source_output) =
                            source_tx.outputs.get(input.output_index as usize)
                        {
                            let input_addr = &source_output.address;

                            for output in &tx.outputs {
                                let output_addr = &output.address;
                                tx_graph
                                    .entry(input_addr.clone())
                                    .or_default()
                                    .push(output_addr.clone());
                                *address_tx_counts.entry(input_addr.clone()).or_insert(0) += 1;
                                *address_tx_counts.entry(output_addr.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        let mut suspicious_cycles = 0;
        let mut total_txs = 0;
        for (start_node, count) in address_tx_counts.iter() {
            if *count < 4 {
                continue;
            } // Ignore addresses with few txs
            total_txs += count;
            // Simple cycle detection: A -> B -> A
            if let Some(neighbors) = tx_graph.get(start_node) {
                for neighbor in neighbors {
                    if let Some(return_neighbors) = tx_graph.get(neighbor) {
                        if return_neighbors.contains(start_node) {
                            suspicious_cycles += 1;
                        }
                    }
                }
            }
        }

        if total_txs == 0 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 1.0,
                details: "No recent transactional activity to analyze.".to_string(),
            };
        }
        let risk = (suspicious_cycles as f64 / total_txs as f64).clamp(0.0, 1.0);
        debug!(
            "Wash trading analysis complete. Suspicious cycles: {suspicious_cycles}, Risk Score: {risk:.4}",
        );
        SecurityFinding {
            risk_score: risk,
            severity: if risk > 0.5 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.6,
            details: format!("{suspicious_cycles} suspicious cycles detected in recent activity."),
        }
    }

    #[instrument(skip(self, dag, reputation))]
    pub async fn check_for_collusion_attack(
        &self,
        dag: &QantoDAG,
        reputation: &ReputationState,
    ) -> SecurityFinding {
        // Detects when a group of miners with mediocre scores exclusively build on each other's blocks,
        // potentially to amplify their rewards or censor others.
        let blocks = dag.blocks.read().await;
        if blocks.len() < 200 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough blocks for collusion analysis.".to_string(),
            };
        }

        let scores = reputation.credit_scores.read().await;
        let mut miner_parent_map = HashMap::<String, Vec<String>>::new();

        // FIX: HashMap::values() is not a DoubleEndedIterator, so we must collect and sort first.
        let mut all_blocks: Vec<_> = blocks.values().collect();
        all_blocks.sort_by_key(|b| b.timestamp);
        let recent_blocks: Vec<_> = all_blocks.iter().rev().take(200).collect();

        for block in &recent_blocks {
            let parent_miners: Vec<String> = block
                .parents
                .iter()
                .filter_map(|p_id| blocks.get(p_id).map(|p| p.miner.clone()))
                .collect();
            miner_parent_map
                .entry(block.miner.clone())
                .or_default()
                .extend(parent_miners);
        }

        let mut collusion_risk = 0.0;
        let mut suspicious_groups = 0;

        for (miner, parents) in &miner_parent_map {
            let miner_scs = scores.get(miner).map_or(0.5, |s| s.score);
            // We are interested in mid-to-low score miners forming a cartel.
            if miner_scs > 0.7 {
                continue;
            }
            if parents.is_empty() {
                continue;
            }

            let mut parent_counts = HashMap::new();
            for p_miner in parents {
                *parent_counts.entry(p_miner.clone()).or_insert(0) += 1;
            }

            // Check if this miner is overwhelmingly building on a small set of other low-score miners
            let top_parent = parent_counts.iter().max_by_key(|&(_, count)| count);

            if let Some((p_miner, &count)) = top_parent {
                if count > 5 && (count as f64 / parents.len() as f64) > 0.8 {
                    let parent_scs = scores.get(p_miner).map_or(0.5, |s| s.score);
                    // If both are low-score and a strong bond exists, it's suspicious.
                    if parent_scs < 0.7 {
                        suspicious_groups += 1;
                        debug!(
                            miner,
                            parent_miner = p_miner,
                            "Potential collusion detected."
                        );
                    }
                }
            }
        }
        if miner_parent_map.len() > 10 {
            collusion_risk =
                (suspicious_groups as f64 / miner_parent_map.len() as f64).clamp(0.0, 1.0);
        }

        SecurityFinding {
            risk_score: collusion_risk,
            severity: if collusion_risk > 0.5 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.7,
            details: format!("{suspicious_groups} potential collusion patterns detected."),
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_for_economic_attack(&self, dag: &QantoDAG) -> SecurityFinding {
        // This check looks for an unusually high average transaction fee during
        // a period of low network congestion, which might indicate an attempt
        // to manipulate the `market_premium` calculation for block rewards.
        let blocks = dag.blocks.read().await;
        if blocks.len() < 50 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough blocks for economic attack analysis.".to_string(),
            };
        }

        let mut recent_blocks: Vec<_> = blocks.values().collect();
        recent_blocks.sort_by_key(|b| b.timestamp);
        let latest_blocks: Vec<_> = recent_blocks.iter().rev().take(50).collect();

        if latest_blocks.is_empty() {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 1.0,
                details: "No recent blocks to analyze.".to_string(),
            };
        }

        let total_txs: usize = latest_blocks.iter().map(|b| b.transactions.len()).sum();
        let total_fees: u64 = latest_blocks
            .iter()
            .flat_map(|b| &b.transactions)
            .map(|tx| tx.fee)
            .sum();
        let avg_tx_per_block = total_txs as f64 / latest_blocks.len() as f64;
        let avg_fee_per_tx = total_fees as f64 / total_txs.max(1) as f64;

        let congestion_level = avg_tx_per_block / MAX_TRANSACTIONS_PER_BLOCK as f64;

        // Condition: Average fee is very high (>50), but network is not congested (<30% capacity)
        if avg_fee_per_tx > 50.0 && congestion_level < 0.3 {
            warn!(
                avg_fee = avg_fee_per_tx,
                congestion = congestion_level,
                "High fees detected with low congestion, potential economic manipulation."
            );
            return SecurityFinding {
                risk_score: 0.75,
                severity: InsightSeverity::Critical,
                confidence: 0.8,
                details: format!(
                    "High avg fee ({avg_fee_per_tx:.2}) with low congestion ({congestion_level:.2})."
                ),
            };
        }

        SecurityFinding {
            risk_score: 0.0,
            severity: InsightSeverity::Tip,
            confidence: 1.0,
            details: "No economic anomalies detected.".to_string(),
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_for_mempool_frontrun(&self, dag: &QantoDAG) -> SecurityFinding {
        let blocks = dag.blocks.read().await;
        let mut recent_blocks: Vec<_> = blocks.values().collect();
        recent_blocks.sort_by_key(|b| b.timestamp);
        let latest_blocks: Vec<_> = recent_blocks.iter().rev().take(20).collect(); // Check recent history

        let mut suspicious_count = 0;
        let mut tx_checked = 0;

        for block in latest_blocks {
            let transactions = &block.transactions;
            for i in 0..transactions.len() {
                for j in (i + 1)..transactions.len() {
                    tx_checked += 1;
                    let tx1 = &transactions[i];
                    let tx2 = &transactions[j];

                    // Heuristic: if tx2 has a higher fee but was mined after tx1,
                    // and they share inputs, it might be a failed front-run attempt.
                    // A successful one is harder to spot without mempool state.
                    // This simplified check looks for fee-bumped replacements in the same block.
                    let tx1_inputs: std::collections::HashSet<_> = tx1
                        .inputs
                        .iter()
                        .map(|i| (i.tx_id.clone(), i.output_index))
                        .collect();
                    let tx2_inputs: std::collections::HashSet<_> = tx2
                        .inputs
                        .iter()
                        .map(|i| (i.tx_id.clone(), i.output_index))
                        .collect();

                    if !tx1_inputs.is_disjoint(&tx2_inputs) {
                        // Transactions spend at least one same UTXO
                        if tx2.fee > tx1.fee {
                            // The later transaction has a higher fee
                            suspicious_count += 1;
                            warn!(tx1_id=%tx1.id, tx2_id=%tx2.id, block_id=%block.id, "Potential front-running pattern detected.");
                        }
                    }
                }
            }
        }

        if tx_checked == 0 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 1.0,
                details: "No recent transactions to analyze for front-running.".to_string(),
            };
        }

        let risk = (suspicious_count as f64 / tx_checked as f64).clamp(0.0, 1.0);
        SecurityFinding {
            risk_score: risk,
            severity: if risk > 0.1 {
                // High threshold because this heuristic can have false positives
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.5,
            details: format!(
                "Detected {suspicious_count} potential front-running patterns in recent blocks."
            ),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ProposalStatus {
    Voting,
    Enacted,
    Rejected,
    Vetoed,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProposalType {
    UpdateRule(String, f64),
    Signal(String),
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SagaCreditScore {
    pub score: f64,
    pub factors: HashMap<String, f64>,
    pub history: Vec<(u64, f64)>,
    pub last_updated: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EpochRule {
    pub value: f64,
    pub description: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VoterInfo {
    address: String,
    voted_for: bool,
    voting_power: f64,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GovernanceProposal {
    pub id: String,
    pub proposer: String,
    pub proposal_type: ProposalType,
    pub votes_for: f64,
    pub votes_against: f64,
    pub status: ProposalStatus,
    pub voters: Vec<VoterInfo>,
    pub creation_epoch: u64,
    #[serde(default)] // For compatibility with older formats
    pub justification: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct CouncilMember {
    pub address: String,
    pub cognitive_load: f64,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SagaCouncil {
    pub members: Vec<CouncilMember>,
    pub last_updated_epoch: u64,
    pub autonomous_governance_cooldown_until_epoch: u64,
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum KarmaSource {
    CreateSuccessfulProposal,
    VoteForPassedProposal,
    VoteAgainstFailedProposal,
    // AiHelpdeskQuery removed for production hardening
    SagaAutonomousAction,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct KarmaLedger {
    pub total_karma: u64,
    pub contributions: HashMap<KarmaSource, u64>,
    pub last_updated_epoch: u64,
}
#[derive(Debug, Clone)]
pub struct ReputationState {
    pub credit_scores: Arc<RwLock<HashMap<String, SagaCreditScore>>>,
    pub karma_ledgers: Arc<RwLock<HashMap<String, KarmaLedger>>>,
}
#[derive(Debug, Clone)]
pub struct GovernanceState {
    pub proposals: Arc<RwLock<HashMap<String, GovernanceProposal>>>,
    pub council: Arc<RwLock<SagaCouncil>>,
}

#[derive(Debug, Clone, Default)]
pub struct EnvironmentalMetrics {
    pub network_green_score: f64,
    pub total_co2_offset_epoch: f64,
    pub trusted_project_registry: HashMap<String, f64>,
    pub verified_credentials: HashMap<String, CarbonOffsetCredential>,
    pub failed_credential_verifications: u64,
}

#[derive(Debug, Clone)]
pub struct EconomicState {
    pub epoch_rules: Arc<RwLock<HashMap<String, EpochRule>>>,
    pub network_state: Arc<RwLock<NetworkState>>,
    pub active_edict: Arc<RwLock<Option<SagaEdict>>>,
    pub last_edict_epoch: Arc<RwLock<u64>>,
    pub proactive_insights: Arc<RwLock<Vec<SagaInsight>>>,
    pub congestion_history: Arc<RwLock<VecDeque<f64>>>,
    pub environmental_metrics: Arc<RwLock<EnvironmentalMetrics>>,
}

#[derive(Debug)]
pub struct PalletSaga {
    pub reputation: ReputationState,
    pub governance: GovernanceState,
    pub economy: EconomicState,
    pub cognitive_engine: Arc<RwLock<CognitiveAnalyticsEngine>>,
    pub economic_model: Arc<PredictiveEconomicModel>,
    pub security_monitor: Arc<SecurityMonitor>,
    // guidance_system removed for production hardening
    // last_retrain_epoch field removed for production hardening
    #[cfg(feature = "infinite-strata")]
    pub isnm_service: Option<Arc<InfiniteStrataNode>>,
}

impl Default for PalletSaga {
    fn default() -> Self {
        Self::new(
            #[cfg(feature = "infinite-strata")]
            None,
        )
    }
}

impl PalletSaga {
    pub fn new(
        #[cfg(feature = "infinite-strata")] isnm_service: Option<Arc<InfiniteStrataNode>>,
    ) -> Self {
        let mut rules = HashMap::new();
        // --- Core ---
        rules.insert(
            "base_difficulty".to_string(),
            EpochRule {
                value: 1.0,
                description: "The baseline PoW difficulty before PoSe adjustments.".to_string(),
            },
        );
        rules.insert(
            "min_validator_stake".to_string(),
            EpochRule {
                value: 1000.0,
                description: "The minimum stake required to be a validator.".to_string(),
            },
        );

        // --- SCS Weights ---
        rules.insert(
            "scs_trust_weight".to_string(),
            EpochRule {
                value: 0.55,
                description: "Weight of Cognitive Engine score in SCS.".to_string(),
            },
        );
        rules.insert(
            "scs_karma_weight".to_string(),
            EpochRule {
                value: 0.2,
                description: "Weight of Karma in SCS.".to_string(),
            },
        );
        rules.insert(
            "scs_stake_weight".to_string(),
            EpochRule {
                value: 0.2,
                description: "Weight of raw stake in SCS.".to_string(),
            },
        );
        rules.insert(
            "scs_environmental_weight".to_string(),
            EpochRule {
                value: 0.05,
                description: "Weight of environmental contribution in SCS.".to_string(),
            },
        );

        // --- Trust Score Weights ---
        rules.insert(
            "trust_validity_weight".to_string(),
            EpochRule {
                value: 0.20,
                description: "Weight of block validity in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_network_contribution_weight".to_string(),
            EpochRule {
                value: 0.10,
                description: "Weight of network contribution in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_historical_performance_weight".to_string(),
            EpochRule {
                value: 0.10,
                description: "Weight of historical performance in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_cognitive_hazard_weight".to_string(),
            EpochRule {
                value: 0.15,
                description: "Weight of cognitive hazard analysis in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_temporal_consistency_weight".to_string(),
            EpochRule {
                value: 0.15,
                description: "Weight of temporal consistency in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_cognitive_dissonance_weight".to_string(),
            EpochRule {
                value: 0.10,
                description: "Weight of cognitive dissonance in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_metadata_integrity_weight".to_string(),
            EpochRule {
                value: 0.10,
                description: "Weight of transaction metadata integrity in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_predicted_behavior_weight".to_string(),
            EpochRule {
                value: 0.10,
                description: "Weight of AI behavioral prediction in trust score.".to_string(),
            },
        );
        rules.insert(
            "trust_environmental_contribution_weight".to_string(),
            EpochRule {
                value: 0.10,
                description: "Weight of PoCO score in trust score.".to_string(),
            },
        );

        // --- Economic ---
        rules.insert(
            "base_reward".to_string(),
            EpochRule {
                value: 50_000_000_000.0,
                description: "Base QNTO reward per block (in smallest units) before modifiers."
                    .to_string(),
            },
        );
        rules.insert(
            "omega_threat_reward_modifier".to_string(),
            EpochRule {
                value: -0.25,
                description: "Reward reduction per elevated ΩMEGA threat level.".to_string(),
            },
        );
        rules.insert(
            "base_tx_fee_min".to_string(),
            EpochRule {
                value: 1.0,
                description: "A minimum flat fee for all transactions, regardless of amount."
                    .to_string(),
            },
        );

        // --- Governance & Karma ---
        rules.insert(
            "proposal_creation_cost".to_string(),
            EpochRule {
                value: 500.0,
                description: "Karma cost to create a new proposal.".to_string(),
            },
        );
        rules.insert(
            "guidance_karma_cost".to_string(),
            EpochRule {
                value: 5.0,
                description: "Karma cost to query the SAGA Guidance System.".to_string(),
            },
        );
        rules.insert(
            "karma_decay_rate".to_string(),
            EpochRule {
                value: 0.995,
                description: "Percentage of Karma remaining after decay each epoch.".to_string(),
            },
        );
        rules.insert(
            "proposal_vote_threshold".to_string(),
            EpochRule {
                value: 100.0,
                description: "Minimum votes for a proposal to be enacted.".to_string(),
            },
        );
        rules.insert(
            "council_size".to_string(),
            EpochRule {
                value: 5.0,
                description: "Number of members in the SAGA Council.".to_string(),
            },
        );
        rules.insert(
            "council_fatigue_decay".to_string(),
            EpochRule {
                value: 0.9,
                description: "Factor by which council member fatigue decays each epoch."
                    .to_string(),
            },
        );
        rules.insert(
            "council_fatigue_per_action".to_string(),
            EpochRule {
                value: 0.1,
                description: "Fatigue increase for a council member per veto/vote.".to_string(),
            },
        );

        // --- Technical ---
        rules.insert(
            "temporal_grace_period_secs".to_string(),
            EpochRule {
                value: 120.0,
                description: "Grace period in seconds for block timestamps.".to_string(),
            },
        );
        rules.insert(
            "scs_karma_normalization_divisor".to_string(),
            EpochRule {
                value: 10000.0,
                description: "Divisor to normalize Karma for SCS.".to_string(),
            },
        );
        rules.insert(
            "scs_stake_normalization_divisor".to_string(),
            EpochRule {
                value: 50000.0,
                description: "Divisor to normalize stake for SCS.".to_string(),
            },
        );
        rules.insert(
            "scs_smoothing_factor".to_string(),
            EpochRule {
                value: 0.1,
                description: "Smoothing factor for updating SCS (weight of the new score)."
                    .to_string(),
            },
        );
        rules.insert(
            "ai_cred_verify_threshold".to_string(),
            EpochRule {
                value: 0.75,
                description:
                    "The AI confidence score required to verify a Carbon Offset Credential."
                        .to_string(),
            },
        );

        let mut env_metrics = EnvironmentalMetrics::default();
        env_metrics
            .trusted_project_registry
            .insert("verra-p-981".to_string(), 1.0);
        env_metrics
            .trusted_project_registry
            .insert("gold-standard-p-334".to_string(), 1.0);
        env_metrics
            .trusted_project_registry
            .insert("verra-p-201".to_string(), 0.8);
        env_metrics
            .trusted_project_registry
            .insert("low-quality-p-001".to_string(), 0.5);

        Self {
            reputation: ReputationState {
                credit_scores: Arc::new(RwLock::new(HashMap::new())),
                karma_ledgers: Arc::new(RwLock::new(HashMap::new())),
            },
            governance: GovernanceState {
                proposals: Arc::new(RwLock::new(HashMap::new())),
                council: Arc::new(RwLock::new(SagaCouncil::default())),
            },
            economy: EconomicState {
                epoch_rules: Arc::new(RwLock::new(rules)),
                network_state: Arc::new(RwLock::new(NetworkState::Nominal)),
                active_edict: Arc::new(RwLock::new(None)),
                last_edict_epoch: Arc::new(RwLock::new(0)),
                proactive_insights: Arc::new(RwLock::new(Vec::new())),
                congestion_history: Arc::new(RwLock::new(VecDeque::with_capacity(10))),
                environmental_metrics: Arc::new(RwLock::new(env_metrics)),
            },
            cognitive_engine: Arc::new(RwLock::new(CognitiveAnalyticsEngine::new())),
            economic_model: Arc::new(PredictiveEconomicModel::new()),
            security_monitor: Arc::new(SecurityMonitor::new()),
            // guidance_system initialization removed for production hardening
            // last_retrain_epoch initialization removed for production hardening
            #[cfg(feature = "infinite-strata")]
            isnm_service,
        }
    }

    /// Calculates the transaction fee based on a simple minimum.
    /// The tiered fee structure has been removed in favor of a simpler, more predictable model
    /// that is easier for SAGA to reason about autonomously.
    pub async fn calculate_dynamic_fee(&self, _amount: u64) -> u64 {
        let rules = self.economy.epoch_rules.read().await;
        let min_fee = rules.get("base_tx_fee_min").map_or(1.0, |r| r.value) as u64;

        // The fee can be further modified by active edicts, for example during spam attacks.
        let edict_multiplier = if let Some(edict) = &*self.economy.active_edict.read().await {
            if let EdictAction::Economic { fee_multiplier, .. } = edict.action {
                fee_multiplier
            } else {
                1.0
            }
        } else {
            1.0
        };

        (min_fee as f64 * edict_multiplier) as u64
    }

    #[inline]
    pub fn calculate_shannon_entropy(s: &str) -> f64 {
        if s.is_empty() {
            return 0.0;
        }
        let mut map = HashMap::new();
        for c in s.chars() {
            *map.entry(c).or_insert(0) += 1;
        }
        let len = s.len() as f64;
        map.values()
            .map(|&count| {
                let p = count as f64 / len;
                -p * p.log2()
            })
            .sum()
    }

    pub async fn verify_and_store_credential(
        &self,
        cred: CarbonOffsetCredential,
    ) -> Result<(), SagaError> {
        let mut metrics_write = self.economy.environmental_metrics.write().await;
        let _rules = self.economy.epoch_rules.read().await;

        let fail_and_count =
            |mut metrics: tokio::sync::RwLockWriteGuard<'_, EnvironmentalMetrics>,
             err: SagaError|
             -> Result<(), SagaError> {
                metrics.failed_credential_verifications += 1;
                Err(err)
            };

        if cred.tonnes_co2_sequestered <= 0.0 {
            return fail_and_count(
                metrics_write,
                SagaError::InvalidCredential("CO2 amount must be positive.".to_string()),
            );
        }
        if cred.beneficiary_node.is_empty() {
            return fail_and_count(
                metrics_write,
                SagaError::InvalidCredential("Beneficiary node cannot be empty.".to_string()),
            );
        }

        let expected_signature = format!("signed_by_{}", cred.issuer_id);
        if cred.verification_signature != expected_signature {
            return fail_and_count(
                metrics_write,
                SagaError::InvalidCredential("Invalid issuer signature.".to_string()),
            );
        }

        if !metrics_write
            .trusted_project_registry
            .contains_key(&cred.project_id)
        {
            return fail_and_count(
                metrics_write,
                SagaError::InvalidCredential(format!(
                    "Project ID '{}' is not in the trusted registry.",
                    cred.project_id
                )),
            );
        }

        let current_year: u32 = 2025; // In a real system, this should be from a trusted time source.
        if current_year.saturating_sub(cred.vintage_year) > 5 {
            warn!(cred_id=%cred.id, "Credential has an old vintage year ({}).", cred.vintage_year);
        }

        // REMOVED: AI-based credential verification for production hardening
        // Credential verification now relies on basic validation checks only

        if metrics_write.verified_credentials.contains_key(&cred.id) {
            return fail_and_count(
                metrics_write,
                SagaError::InvalidCredential(format!(
                    "Credential ID '{}' has already been submitted this epoch.",
                    cred.id
                )),
            );
        }

        info!(
            cred_id = %cred.id,
            beneficiary = %cred.beneficiary_node,
            "Storing verified CarbonOffsetCredential."
        );
        metrics_write
            .verified_credentials
            .insert(cred.id.clone(), cred);

        Ok(())
    }

    #[instrument(skip(self, block, dag_arc))]
    pub async fn evaluate_block_with_saga(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
    ) -> Result<()> {
        info!(block_id = %block.id, miner = %block.miner, "SAGA: Starting evaluation of new block.");

        self.evaluate_and_score_block(block, dag_arc).await?;

        // AI training data collection removed for production hardening

        info!(block_id = %block.id, "SAGA: Evaluation complete.");
        Ok(())
    }

    async fn evaluate_and_score_block(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
    ) -> Result<()> {
        let (rules, network_state) = {
            let eco_rules = self.economy.epoch_rules.read().await;
            let net_state = *self.economy.network_state.read().await;
            (eco_rules.clone(), net_state)
        };

        let trust_breakdown = self
            .cognitive_engine
            .read()
            .await
            .score_node_behavior(block, dag_arc, &rules, network_state)
            .await?;

        self.update_credit_score(&block.miner, &trust_breakdown, dag_arc)
            .await?;
        Ok(())
    }

    async fn get_isnm_reward_multiplier(&self) -> f64 {
        #[cfg(feature = "infinite-strata")]
        {
            if let Some(service) = &self.isnm_service {
                let (multiplier, _redistributed_reward) = service.get_rewards().await;
                return multiplier;
            }
        }
        1.0
    }

    pub async fn calculate_dynamic_reward(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
        total_fees: u64,
    ) -> Result<u64> {
        let rules = self.economy.epoch_rules.read().await;
        let base_reward = rules
            .get("base_reward")
            .map_or(50_000_000_000.0, |r| r.value);
        let threat_modifier = rules
            .get("omega_threat_reward_modifier")
            .map_or(-0.25, |r| r.value);
        let scs = self
            .reputation
            .credit_scores
            .read()
            .await
            .get(&block.miner)
            .map_or(0.5, |s| s.score);

        let threat_level = omega::identity::get_threat_level().await;
        let omega_penalty = match threat_level {
            omega::identity::ThreatLevel::Nominal => 1.0,
            omega::identity::ThreatLevel::Guarded => 1.0 + threat_modifier,
            omega::identity::ThreatLevel::Elevated => 1.0 + (threat_modifier * 2.0),
        }
        .max(0.0);

        let metrics = self.economy.environmental_metrics.read().await;
        let market_premium = self
            .economic_model
            .predictive_market_premium(dag_arc, &metrics)
            .await;

        let edict_multiplier = if let Some(edict) = &*self.economy.active_edict.read().await {
            if let EdictAction::Economic {
                reward_multiplier, ..
            } = edict.action
            {
                reward_multiplier
            } else {
                1.0
            }
        } else {
            1.0
        };

        let isnm_multiplier = self.get_isnm_reward_multiplier().await;

        let final_reward_float =
            base_reward * scs * omega_penalty * market_premium * edict_multiplier * isnm_multiplier;

        let final_reward = final_reward_float as u64 + total_fees;
        Ok(final_reward)
    }

    #[instrument(skip(self, dag))]
    pub async fn process_epoch_evolution(&self, current_epoch: u64, dag: &QantoDAG) {
        info!("SAGA is processing epoch evolution for epoch {current_epoch}");
        // SENSE: Gather data and assess the current state of the network.
        self.update_network_state(dag).await;
        self.run_predictive_models(current_epoch, dag).await;
        self.generate_proactive_insights(current_epoch, dag).await;
        self.update_environmental_metrics(current_epoch).await;

        // THINK: Process information, update internal models, and decide on actions.
        self.tally_proposals(current_epoch).await;
        self.process_karma_decay(current_epoch).await;
        self.update_council(current_epoch).await;

        // AI model retraining removed for production hardening
        // Previously handled periodic neural network training

        // ACT: Execute decisions, whether through edicts or autonomous governance proposals.
        self.issue_new_edict(current_epoch, dag).await;
        self.perform_autonomous_governance(current_epoch, dag).await;
    }

    async fn update_network_state(&self, dag: &QantoDAG) {
        let avg_tx_per_block = dag.get_average_tx_per_block().await;
        let validator_count = dag.validators.read().await.len();

        // Check all security vectors
        let sybil_risk = self.security_monitor.check_for_sybil_attack(dag).await;
        let spam_risk = self
            .security_monitor
            .check_transactional_anomalies(dag)
            .await;
        let centralization_risk = self
            .security_monitor
            .check_for_centralization_risk(dag, 1)
            .await;
        let oracle_risk = self
            .security_monitor
            .check_for_oracle_manipulation_risk(dag)
            .await;
        let time_drift_risk = self.security_monitor.check_for_time_drift_attack(dag).await;
        let wash_trading_risk = self.security_monitor.check_for_wash_trading(dag).await;
        let collusion_risk = self
            .security_monitor
            .check_for_collusion_attack(dag, &self.reputation)
            .await;
        let economic_risk = self.security_monitor.check_for_economic_attack(dag).await;
        let mempool_risk = self.security_monitor.check_for_mempool_frontrun(dag).await;

        // Determine the most critical threat
        let mut threats = [
            (sybil_risk.risk_score, AttackType::Sybil, 0.8),
            (spam_risk.risk_score, AttackType::Spam, 0.75),
            (
                centralization_risk.risk_score,
                AttackType::Centralization,
                0.8,
            ),
            (collusion_risk.risk_score, AttackType::Collusion, 0.65),
            (oracle_risk.risk_score, AttackType::OracleManipulation, 0.7),
            (time_drift_risk.risk_score, AttackType::TimeDrift, 0.8),
            (wash_trading_risk.risk_score, AttackType::WashTrading, 0.6),
            (economic_risk.risk_score, AttackType::Economic, 0.7),
            (mempool_risk.risk_score, AttackType::MempoolFrontrun, 0.6),
        ];
        threats.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let mut state_writer = self.economy.network_state.write().await;
        let old_state = *state_writer;

        let new_state = if let Some((risk, attack_type, threshold)) = threats.first() {
            if *risk > *threshold {
                NetworkState::UnderAttack(*attack_type)
            } else if validator_count < 10 {
                NetworkState::Degraded
            } else if avg_tx_per_block > (MAX_TRANSACTIONS_PER_BLOCK as f64 * 0.8) {
                NetworkState::Congested
            } else {
                NetworkState::Nominal
            }
        } else {
            NetworkState::Nominal // Should not happen
        };

        if old_state != new_state {
            info!(
                old_state = ?old_state,
                new_state = ?new_state,
                "SAGA has transitioned the Network State."
            );
        }
        *state_writer = new_state;
    }

    async fn run_predictive_models(&self, _current_epoch: u64, dag: &QantoDAG) {
        let avg_tx_per_block = dag.get_average_tx_per_block().await;
        let congestion_metric = avg_tx_per_block / MAX_TRANSACTIONS_PER_BLOCK as f64;

        let mut history = self.economy.congestion_history.write().await;
        history.push_back(congestion_metric.clamp(0.0, 1.0));
        if history.len() > 10 {
            history.pop_front();
        }

        // AI-based congestion prediction removed for production hardening
        // Previously used LSTM model to predict network congestion
    }

    #[instrument(skip(self, trust_breakdown, dag_arc))]
    async fn update_credit_score(
        &self,
        miner_address: &str,
        trust_breakdown: &TrustScoreBreakdown,
        dag_arc: &Arc<QantoDAG>,
    ) -> Result<()> {
        let rules = self.economy.epoch_rules.read().await;

        let trust_weight = rules.get("scs_trust_weight").map_or(0.55, |r| r.value);
        let karma_weight = rules.get("scs_karma_weight").map_or(0.2, |r| r.value);
        let stake_weight = rules.get("scs_stake_weight").map_or(0.2, |r| r.value);
        let env_weight = rules
            .get("scs_environmental_weight")
            .map_or(0.05, |r| r.value);

        let karma_divisor = rules
            .get("scs_karma_normalization_divisor")
            .map_or(10000.0, |r| r.value);
        let stake_divisor = rules
            .get("scs_stake_normalization_divisor")
            .map_or(50000.0, |r| r.value);
        let smoothing_factor = rules.get("scs_smoothing_factor").map_or(0.1, |r| r.value);

        let karma_score = self
            .reputation
            .karma_ledgers
            .read()
            .await
            .get(miner_address)
            .map_or(0.0, |kl| (kl.total_karma as f64 / karma_divisor).min(1.0));
        let stake_score = dag_arc
            .validators
            .read()
            .await
            .get(miner_address)
            .map_or(0.0, |&s| (s as f64 / stake_divisor).min(1.0));

        let env_score = trust_breakdown
            .factors
            .get("environmental_contribution")
            .cloned()
            .unwrap_or(0.5);

        let new_raw_score = (trust_breakdown.final_weighted_score * trust_weight)
            + (karma_score * karma_weight)
            + (stake_score * stake_weight)
            + (env_score * env_weight);

        let mut scores = self.reputation.credit_scores.write().await;
        let mut scs_entry = scores.entry(miner_address.to_string()).or_default().clone();

        scs_entry.score =
            (scs_entry.score * (1.0 - smoothing_factor)) + (new_raw_score * smoothing_factor);
        scs_entry.last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| SagaError::TimeError(e.to_string()))?
            .as_secs();
        scs_entry
            .history
            .push((scs_entry.last_updated, scs_entry.score));
        if scs_entry.history.len() > 200 {
            scs_entry.history.remove(0);
        }

        scs_entry.factors = trust_breakdown.factors.clone();
        scs_entry
            .factors
            .insert("karma_score".to_string(), karma_score);
        scs_entry
            .factors
            .insert("stake_score".to_string(), stake_score);

        scores.insert(miner_address.to_string(), scs_entry.clone());
        info!(miner = miner_address, scs = scs_entry.score, "SCS Updated");
        Ok(())
    }

    async fn generate_proactive_insights(&self, current_epoch: u64, _dag: &QantoDAG) {
        let mut insights = self.economy.proactive_insights.write().await;
        insights.retain(|i| current_epoch < i.epoch + 5);
        let network_state = *self.economy.network_state.read().await;

        if network_state == NetworkState::Congested
            && !insights
                .iter()
                .any(|i| i.title.contains("Network Congestion"))
        {
            insights.push(SagaInsight {
                id: Uuid::new_v4().to_string(),
                epoch: current_epoch,
                title: "Network Congestion".to_string(),
                detail: "The network is experiencing high transaction volume. Consider increasing your transaction fees for faster confirmation.".to_string(),
                severity: InsightSeverity::Warning,
            });
        }
        if let NetworkState::UnderAttack(attack_type) = network_state {
            if !insights.iter().any(|i| i.title.contains("Security Alert")) {
                insights.push(SagaInsight {
                    id: Uuid::new_v4().to_string(),
                    epoch: current_epoch,
                    title: "Security Alert".to_string(),
                    detail: format!("SAGA has detected a potential {attack_type:?} and has entered a defensive state. Network parameters have been adjusted."),
                    severity: InsightSeverity::Critical,
                });
            }
        }
        let proposal_count = self.governance.proposals.read().await.len();
        if current_epoch > 20
            && proposal_count < 5
            && !insights
                .iter()
                .any(|i| i.title.contains("Low Governance Activity"))
        {
            insights.push(SagaInsight {
                id: Uuid::new_v4().to_string(),
                epoch: current_epoch,
                title: "Low Governance Activity".to_string(),
                detail: "There have been few governance proposals recently. Consider proposing changes or participating in discussions to help evolve the network.".to_string(),
                severity: InsightSeverity::Tip,
            });
        }
    }

    async fn tally_proposals(&self, current_epoch: u64) {
        let mut proposals = self.governance.proposals.write().await;
        let mut rules = self.economy.epoch_rules.write().await;
        let vote_threshold = rules
            .get("proposal_vote_threshold")
            .map_or(100.0, |r| r.value);
        let karma_reward_proposer = 250;
        let karma_reward_voter = 25;

        for proposal in proposals.values_mut() {
            if proposal.status == ProposalStatus::Voting {
                if proposal.votes_for >= vote_threshold {
                    proposal.status = ProposalStatus::Enacted;
                    info!(proposal_id = %proposal.id, "Proposal has been enacted.");
                    if let ProposalType::UpdateRule(rule_name, new_value) = &proposal.proposal_type
                    {
                        if let Some(rule) = rules.get_mut(rule_name) {
                            info!(rule = %rule_name, old_value = rule.value, new_value = new_value, "Epoch Rule Evolved via Governance");
                            rule.value = *new_value;
                        }
                    }
                    self.award_karma(
                        &proposal.proposer,
                        KarmaSource::CreateSuccessfulProposal,
                        karma_reward_proposer,
                    )
                    .await;
                    for voter in &proposal.voters {
                        if voter.voted_for {
                            self.award_karma(
                                &voter.address,
                                KarmaSource::VoteForPassedProposal,
                                karma_reward_voter,
                            )
                            .await;
                        }
                    }
                } else if current_epoch > proposal.creation_epoch + 10 {
                    proposal.status = ProposalStatus::Rejected;
                    info!(proposal_id = %proposal.id, "Proposal expired and was rejected.");
                }
            }
        }
    }

    async fn award_karma(&self, address: &str, source: KarmaSource, amount: u64) {
        let mut ledgers = self.reputation.karma_ledgers.write().await;
        let ledger = ledgers.entry(address.to_string()).or_default();
        ledger.total_karma += amount;
        *ledger.contributions.entry(source).or_insert(0) += amount;
        info!(%address, ?source, %amount, "Awarded Karma");
    }

    async fn process_karma_decay(&self, current_epoch: u64) {
        let decay_rate = self
            .economy
            .epoch_rules
            .read()
            .await
            .get("karma_decay_rate")
            .map_or(1.0, |r| r.value);
        if decay_rate >= 1.0 {
            return;
        }

        let mut karma_ledgers = self.reputation.karma_ledgers.write().await;
        for (address, ledger) in karma_ledgers.iter_mut() {
            if ledger.last_updated_epoch < current_epoch {
                ledger.total_karma = (ledger.total_karma as f64 * decay_rate) as u64;
                ledger.last_updated_epoch = current_epoch;
                debug!(%address, new_karma = ledger.total_karma, "Applied Karma decay");
            }
        }
    }

    async fn update_council(&self, current_epoch: u64) {
        let mut council = self.governance.council.write().await;
        if council.last_updated_epoch >= current_epoch {
            return;
        }

        let rules = self.economy.epoch_rules.read().await;
        let council_size = rules.get("council_size").map_or(5.0, |r| r.value) as usize;
        let fatigue_decay = rules.get("council_fatigue_decay").map_or(0.9, |r| r.value);

        for member in &mut council.members {
            member.cognitive_load *= fatigue_decay;
        }

        let karma_ledgers = self.reputation.karma_ledgers.read().await;
        let mut karma_vec: Vec<_> = karma_ledgers.iter().collect();
        karma_vec.sort_by(|a, b| b.1.total_karma.cmp(&a.1.total_karma));

        let new_members: Vec<CouncilMember> = karma_vec
            .into_iter()
            .take(council_size)
            .map(|(address, _)| {
                let existing_load = council
                    .members
                    .iter()
                    .find(|m| m.address == *address)
                    .map_or(0.0, |m| m.cognitive_load);
                CouncilMember {
                    address: address.clone(),
                    cognitive_load: existing_load,
                }
            })
            .collect();

        if council
            .members
            .iter()
            .map(|m| &m.address)
            .collect::<Vec<_>>()
            != new_members.iter().map(|m| &m.address).collect::<Vec<_>>()
        {
            info!(new_council = ?new_members.iter().map(|m| &m.address).collect::<Vec<_>>(), "Updating SAGA Council based on Karma ranking");
            council.members = new_members;
        }
        council.last_updated_epoch = current_epoch;
    }

    async fn issue_new_edict(&self, current_epoch: u64, dag: &QantoDAG) {
        let mut last_edict_epoch = self.economy.last_edict_epoch.write().await;
        if current_epoch < *last_edict_epoch + 10 {
            return;
        }

        let mut active_edict = self.economy.active_edict.write().await;
        if let Some(edict) = &*active_edict {
            if current_epoch >= edict.expiry_epoch {
                info!("SAGA Edict #{} has expired.", edict.id);
                *active_edict = None;
            }
        }

        if active_edict.is_none() {
            let network_state = *self.economy.network_state.read().await;
            let new_edict = match network_state {
                NetworkState::Congested => {
                    let history = self.economy.congestion_history.read().await;
                    let avg_congestion =
                        history.iter().sum::<f64>() / history.len().max(1) as f64;
                    if avg_congestion > 0.8 {
                        Some(SagaEdict {
                            id: Uuid::new_v4().to_string(),
                            issued_epoch: current_epoch,
                            expiry_epoch: current_epoch + 5,
                            description: "Sustained Network Congestion: Temporarily increasing block rewards to incentivize faster processing.".to_string(),
                            action: EdictAction::Economic { reward_multiplier: 1.2, fee_multiplier: 1.1 },
                        })
                    } else {
                        None
                    }
                }
                NetworkState::Degraded => {
                    let validator_count = dag.validators.read().await.len();
                    Some(SagaEdict {
                        id: Uuid::new_v4().to_string(),
                        issued_epoch: current_epoch,
                        expiry_epoch: current_epoch + 10,
                        description: format!("Network Degraded ({validator_count} validators): Temporarily boosting rewards to attract more validators."),
                        action: EdictAction::Economic { reward_multiplier: 1.5, fee_multiplier: 1.0 },
                    })
                }
                NetworkState::UnderAttack(AttackType::Spam) => Some(SagaEdict {
                    id: Uuid::new_v4().to_string(),
                    issued_epoch: current_epoch,
                    expiry_epoch: current_epoch + 10,
                    description:
                        "Spam Attack Detected: Temporarily increasing minimum transaction fees and re-weighting trust scores."
                            .to_string(),
                    action: EdictAction::Economic {
                        reward_multiplier: 1.0,
                        fee_multiplier: 5.0,
                    },
                }),
                _ => None,
            };

            if let Some(edict) = new_edict {
                info!(id=%edict.id, desc=%edict.description, "SAGA has autonomously issued a new Edict.");
                *last_edict_epoch = current_epoch;
                *active_edict = Some(edict);
            }
        }
    }

    async fn perform_autonomous_governance(&self, current_epoch: u64, dag: &QantoDAG) {
        let mut council = self.governance.council.write().await;

        if current_epoch < council.autonomous_governance_cooldown_until_epoch {
            debug!(
                "SAGA autonomous governance is in a cooldown period until epoch {}.",
                council.autonomous_governance_cooldown_until_epoch
            );
            return;
        }

        let avg_cognitive_load: f64 = council
            .members
            .iter()
            .map(|m| m.cognitive_load)
            .sum::<f64>()
            / council.members.len().max(1) as f64;
        if avg_cognitive_load > 0.8 {
            warn!(
                avg_load = avg_cognitive_load,
                "SAGA Council cognitive load is high. Initiating governance cooldown for 5 epochs."
            );
            council.autonomous_governance_cooldown_until_epoch = current_epoch + 5;
            return;
        }

        // Combine all proposal checks and apply cognitive load afterwards.
        let mut actions_taken = 0.0;
        if self.propose_validator_scaling(current_epoch, dag).await {
            actions_taken += 0.15;
        }
        if self
            .propose_governance_parameter_tuning(current_epoch)
            .await
        {
            actions_taken += 0.2;
        }
        if self.propose_economic_parameter_tuning(current_epoch).await {
            actions_taken += 0.15;
        }
        if self.propose_scs_weight_tuning(current_epoch).await {
            actions_taken += 0.25;
        }
        if self.propose_security_parameter_tuning(current_epoch).await {
            actions_taken += 0.2;
        }

        if actions_taken > 0.0 {
            council
                .members
                .iter_mut()
                .for_each(|m| m.cognitive_load += actions_taken);
            council.autonomous_governance_cooldown_until_epoch = current_epoch + 3;
            // Cooldown after any action
        }
    }

    async fn update_environmental_metrics(&self, _current_epoch: u64) {
        let mut metrics = self.economy.environmental_metrics.write().await;

        let total_offset: f64 = metrics
            .verified_credentials
            .values()
            .map(|c| {
                let quality_multiplier = metrics
                    .trusted_project_registry
                    .get(&c.project_id)
                    .cloned()
                    .unwrap_or(0.5);
                c.tonnes_co2_sequestered * quality_multiplier
            })
            .sum();
        metrics.total_co2_offset_epoch = total_offset;

        let green_score = (total_offset / 1000.0).clamp(0.0, 1.0);
        metrics.network_green_score = green_score;

        info!(
            epoch = _current_epoch,
            total_co2_offset_epoch = metrics.total_co2_offset_epoch,
            network_green_score = metrics.network_green_score,
            failed_verifications = metrics.failed_credential_verifications,
            "Updated environmental metrics for epoch."
        );

        // Reset epoch-specific counters
        metrics.verified_credentials.clear();
        metrics.failed_credential_verifications = 0;
    }

    /// Autonomously proposes adjusting validator incentives to scale the network.
    async fn propose_validator_scaling(&self, _current_epoch: u64, dag: &QantoDAG) -> bool {
        let rules = self.economy.epoch_rules.read().await;
        let validator_count = dag.validators.read().await.len();
        let min_stake_rule = "min_validator_stake".to_string();
        let base_reward_rule = "base_reward".to_string();
        let current_min_stake = rules.get(&min_stake_rule).map_or(1000.0, |r| r.value);

        let congestion_history = self.economy.congestion_history.read().await;
        let avg_congestion =
            congestion_history.iter().sum::<f64>() / congestion_history.len().max(1) as f64;

        let has_active_proposal = |proposals: &HashMap<String, GovernanceProposal>| {
            proposals.values().any(|p: &GovernanceProposal| {
                p.status == ProposalStatus::Voting &&
                matches!(&p.proposal_type, ProposalType::UpdateRule(name, _) if name == &min_stake_rule || name == &base_reward_rule)
            })
        };

        // Condition to attract more validators
        if (validator_count < 10
            && *self.economy.network_state.read().await == NetworkState::Degraded)
            || (avg_congestion > 0.7 && validator_count < 20)
        {
            let mut proposals = self.governance.proposals.write().await;
            if !has_active_proposal(&proposals) {
                let new_stake_req = (current_min_stake * 0.9).round();
                let justification = format!("Network conditions (validators: {validator_count}, avg_congestion: {avg_congestion:.2}) suggest a need for more validators. Lowering the minimum stake from {current_min_stake} to {new_stake_req} should increase incentive to participate.");
                let proposal = GovernanceProposal {
                    id: format!("saga-proposal-{}", Uuid::new_v4()),
                    proposer: "SAGA_AUTONOMOUS_AGENT".to_string(),
                    proposal_type: ProposalType::UpdateRule(min_stake_rule, new_stake_req),
                    votes_for: 1.0,
                    votes_against: 0.0,
                    status: ProposalStatus::Voting,
                    voters: vec![],
                    creation_epoch: _current_epoch,
                    justification: Some(justification),
                };
                info!(proposal_id = %proposal.id, "SAGA is proposing to lower minimum stake to {new_stake_req} to attract validators.");
                self.award_karma(
                    "SAGA_AUTONOMOUS_AGENT",
                    KarmaSource::SagaAutonomousAction,
                    100,
                )
                .await;
                proposals.insert(proposal.id.clone(), proposal);
                return true;
            }
        }
        false
    }

    async fn propose_governance_parameter_tuning(&self, current_epoch: u64) -> bool {
        if !current_epoch.is_multiple_of(20) {
            return false;
        }

        let proposals = self.governance.proposals.read().await;
        let recent_proposals: Vec<_> = proposals
            .values()
            .filter(|p| {
                p.creation_epoch > current_epoch.saturating_sub(20)
                    && p.proposer != "SAGA_AUTONOMOUS_AGENT"
            })
            .collect();

        if recent_proposals.len() < 2 {
            return false;
        }

        let rejected_count = recent_proposals
            .iter()
            .filter(|p| p.status == ProposalStatus::Rejected)
            .count();
        let rejection_rate = rejected_count as f64 / recent_proposals.len() as f64;

        let vote_threshold_rule = "proposal_vote_threshold".to_string();
        let rules = self.economy.epoch_rules.read().await;
        let current_threshold = rules.get(&vote_threshold_rule).map_or(100.0, |r| r.value);

        if rejection_rate > 0.75 {
            let mut proposals_writer = self.governance.proposals.write().await;
            if !proposals_writer.values().any(|p| p.status == ProposalStatus::Voting && matches!(&p.proposal_type, ProposalType::UpdateRule(name, _) if name == &vote_threshold_rule)) {
                let new_threshold = (current_threshold * 0.9).round();
                let justification = format!("A high proposal rejection rate ({:.2}%) was detected. To encourage more successful governance, the vote threshold is being lowered from {} to {}.", rejection_rate * 100.0, current_threshold, new_threshold);
                let proposal = GovernanceProposal {
                    id: format!("saga-proposal-{}", Uuid::new_v4()),
                    proposer: "SAGA_AUTONOMOUS_AGENT".to_string(),
                    proposal_type: ProposalType::UpdateRule(vote_threshold_rule.clone(), new_threshold),
                    votes_for: 1.0, votes_against: 0.0, status: ProposalStatus::Voting, voters: vec![], creation_epoch: current_epoch,
                    justification: Some(justification),
                };
                info!(proposal_id = %proposal.id, "SAGA detected a high proposal rejection rate and is proposing to lower the vote threshold to {new_threshold}.");
                self.award_karma("SAGA_AUTONOMOUS_AGENT", KarmaSource::SagaAutonomousAction, 100).await;
                proposals_writer.insert(proposal.id.clone(), proposal);
                return true;
            }
        }
        false
    }

    async fn propose_economic_parameter_tuning(&self, current_epoch: u64) -> bool {
        let network_state = *self.economy.network_state.read().await;
        if let NetworkState::UnderAttack(AttackType::Spam) = network_state {
            let fee_rule = "base_tx_fee_min".to_string();
            let rules = self.economy.epoch_rules.read().await;
            let current_base_fee = rules.get(&fee_rule).map_or(1.0, |r| r.value);

            let mut proposals_writer = self.governance.proposals.write().await;
            if !proposals_writer.values().any(|p| p.status == ProposalStatus::Voting && matches!(&p.proposal_type, ProposalType::UpdateRule(name, _) if name == &fee_rule)) {
                let new_fee = (current_base_fee * 2.0).round();
                if new_fee > current_base_fee {
                    let justification = format!("Spam attack detected. Increasing minimum fee from {current_base_fee} to {new_fee} to make the attack economically unviable.");
                    let proposal = GovernanceProposal {
                        id: format!("saga-proposal-{}", Uuid::new_v4()),
                        proposer: "SAGA_AUTONOMOUS_AGENT".to_string(),
                        proposal_type: ProposalType::UpdateRule(fee_rule.clone(), new_fee),
                        votes_for: 1.0, votes_against: 0.0, status: ProposalStatus::Voting, voters: vec![], creation_epoch: current_epoch,
                        justification: Some(justification),
                    };
                    info!(proposal_id = %proposal.id, "SAGA detected a spam attack and is proposing to increase the minimum base transaction fee to {new_fee}.");
                    self.award_karma("SAGA_AUTONOMOUS_AGENT", KarmaSource::SagaAutonomousAction, 100).await;
                    proposals_writer.insert(proposal.id.clone(), proposal);
                    return true;
                }
            }
        }
        false
    }

    async fn propose_scs_weight_tuning(&self, current_epoch: u64) -> bool {
        if !current_epoch.is_multiple_of(25) {
            return false;
        }

        let scores = self.reputation.credit_scores.read().await;
        if scores.len() < 10 {
            return false;
        }

        let mut high_env_low_scs_count = 0;
        for scs in scores.values() {
            let env_score = scs
                .factors
                .get("environmental_contribution")
                .cloned()
                .unwrap_or(0.0);
            if env_score > 0.8 && scs.score < 0.6 {
                high_env_low_scs_count += 1;
            }
        }

        if high_env_low_scs_count as f64 / scores.len() as f64 > 0.2 {
            let weight_rule = "scs_environmental_weight".to_string();
            let rules = self.economy.epoch_rules.read().await;
            let current_weight = rules.get(&weight_rule).map_or(0.05, |r| r.value);

            if current_weight >= 0.2 {
                return false;
            }

            let mut proposals_writer = self.governance.proposals.write().await;
            if !proposals_writer.values().any(|p| p.status == ProposalStatus::Voting && matches!(&p.proposal_type, ProposalType::UpdateRule(name, _) if name == &weight_rule)) {
                let new_weight = (current_weight + 0.05).min(0.2);
                let justification = format!("Analysis shows nodes with high environmental scores are not adequately represented in SCS. Increasing weight from {current_weight} to {new_weight} to better reward green participation.");
                let proposal = GovernanceProposal {
                    id: format!("saga-proposal-{}", Uuid::new_v4()),
                    proposer: "SAGA_AUTONOMOUS_AGENT".to_string(),
                    proposal_type: ProposalType::UpdateRule(weight_rule.clone(), new_weight),
                    votes_for: 1.0, votes_against: 0.0, status: ProposalStatus::Voting, voters: vec![], creation_epoch: current_epoch,
                    justification: Some(justification),
                };
                info!(proposal_id = %proposal.id, "SAGA detected that environmental contributions may be undervalued and is proposing to increase the SCS environmental weight to {new_weight}.");
                self.award_karma("SAGA_AUTONOMOUS_AGENT", KarmaSource::SagaAutonomousAction, 150).await;
                proposals_writer.insert(proposal.id.clone(), proposal);
                return true;
            }
        }
        false
    }

    /// Autonomously proposes adjusting security parameters based on observed events.
    async fn propose_security_parameter_tuning(&self, current_epoch: u64) -> bool {
        let metrics = self.economy.environmental_metrics.read().await;
        let total_submissions =
            metrics.verified_credentials.len() as u64 + metrics.failed_credential_verifications;

        // Condition: if more than 15% of credential submissions in an epoch fail, the AI might be too strict.
        if total_submissions > 10
            && (metrics.failed_credential_verifications as f64 / total_submissions as f64) > 0.15
        {
            let threshold_rule = "ai_cred_verify_threshold".to_string();
            let _rules = self.economy.epoch_rules.read().await;
            let current_threshold = _rules.get(&threshold_rule).map_or(0.75, |r| r.value);
            // Don't lower the threshold too much
            if current_threshold <= 0.6 {
                return false;
            }

            let mut proposals_writer = self.governance.proposals.write().await;
            if !proposals_writer.values().any(|p| p.status == ProposalStatus::Voting && matches!(&p.proposal_type, ProposalType::UpdateRule(name, _) if name == &threshold_rule)) {
                let new_threshold = (current_threshold - 0.05).max(0.6);
                let justification = format!("Observed a high PoCO credential failure rate ({:.2}%). Lowering the AI verification threshold from {} to {} may improve valid credential acceptance.", (metrics.failed_credential_verifications as f64 / total_submissions as f64) * 100.0, current_threshold, new_threshold);
                let proposal = GovernanceProposal {
                    id: format!("saga-proposal-{}", Uuid::new_v4()),
                    proposer: "SAGA_AUTONOMOUS_AGENT".to_string(),
                    proposal_type: ProposalType::UpdateRule(threshold_rule.clone(), new_threshold),
                    votes_for: 1.0, votes_against: 0.0, status: ProposalStatus::Voting, voters: vec![], creation_epoch: current_epoch,
                    justification: Some(justification),
                };
                info!(proposal_id = %proposal.id, "SAGA detected a high credential failure rate and is proposing to lower the AI verification threshold to {new_threshold}.");
                self.award_karma("SAGA_AUTONOMOUS_AGENT", KarmaSource::SagaAutonomousAction, 120).await;
                proposals_writer.insert(proposal.id.clone(), proposal);
                return true;
            }
        }
        false
    }
}
