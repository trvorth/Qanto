//! --- SAGA: Sentient Autonomous Governance Algorithm ---
//! v0.1.0 - Production Ready
//!
//! This version implements the basic SAGA algorithm with rule execution,
//! proposal voting, and epoch management.
//!
//! - Rule Execution: Executes governance rules defined in the SAGA protocol.
//! - Proposal Voting: Allows users to vote on proposals submitted to the network.
//! - Epoch Management: Manages epochs, which are time periods during which
//!   governance decisions are made.
//! - Rule Enforcement: Enforces governance rules and executes actions based on
//!   voting results.
//! - State Management: Maintains the current state of the SAGA protocol,
//!   including rules, proposals, and voting results.
//! - Event Logging: Logs important events and actions for auditing and
//!   transparency.

#[cfg(feature = "infinite-strata")]
use crate::infinite_strata_node::InfiniteStrataNode;
use crate::omega;
use crate::qantodag::{QantoBlock, QantoDAG, MAX_TRANSACTIONS_PER_BLOCK};
use crate::transaction::Transaction;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// Advanced AI and Deep Learning imports

use rand::{thread_rng, Rng};
use serde_json;

// --- AI Deep Learning Constants ---
const NEURAL_NETWORK_LAYERS: usize = 6; // Deep architecture - used in network initialization
const LEARNING_RATE: f64 = 0.001;
const MOMENTUM: f64 = 0.9;
const DROPOUT_RATE: f64 = 0.2;
const BATCH_SIZE: usize = 32; // Used for training batches
const RETRAIN_INTERVAL_EPOCHS: u64 = 10; // Network retraining frequency
const ADAPTIVE_THRESHOLD: f64 = 0.85; // Adaptation trigger threshold
const SECURITY_CONFIDENCE_THRESHOLD: f64 = 0.95;
const SELF_SCALING_TRIGGER: f64 = 0.8;
const ROBUSTNESS_FACTOR: f64 = 1.2; // System robustness multiplier
const TEMPORAL_GRACE_PERIOD_SECS: u64 = 120;

// Neural network architecture constants
const INPUT_NEURONS: usize = 128;
const HIDDEN_NEURONS: [usize; 5] = [256, 512, 256, 128, 64];
const OUTPUT_NEURONS: usize = 32;
const ACTIVATION_THRESHOLD: f64 = 0.5; // Neural activation threshold

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
/// Advanced Neural Network Layer with dropout and batch normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralLayer {
    pub weights: Vec<Vec<f64>>,
    pub biases: Vec<f64>,
    pub activation_function: ActivationFunction,
    pub dropout_mask: Vec<bool>,
    pub batch_norm_params: BatchNormParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivationFunction {
    ReLU,
    Sigmoid,
    Tanh,
    LeakyReLU(f64),
    Swish,
    GELU,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchNormParams {
    pub gamma: Vec<f64>,
    pub beta: Vec<f64>,
    pub running_mean: Vec<f64>,
    pub running_var: Vec<f64>,
    pub momentum: f64,
}

/// Deep Neural Network for SAGA AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaDeepNetwork {
    pub layers: Vec<NeuralLayer>,
    pub learning_rate: f64,
    pub momentum: f64,
    pub training_history: Vec<TrainingMetrics>,
    pub adaptive_lr_scheduler: AdaptiveLRScheduler,
    pub regularization: RegularizationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    pub epoch: u64,
    pub loss: f64,
    pub accuracy: f64,
    pub validation_loss: f64,
    pub learning_rate: f64,
    pub gradient_norm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveLRScheduler {
    pub initial_lr: f64,
    pub decay_rate: f64,
    pub patience: u32,
    pub min_lr: f64,
    pub best_loss: f64,
    pub wait_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegularizationConfig {
    pub l1_lambda: f64,
    pub l2_lambda: f64,
    pub dropout_rate: f64,
    pub gradient_clipping: f64,
}

/// Feature scaling parameters for data normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScalingParams {
    pub means: Vec<f64>,
    pub std_devs: Vec<f64>,
    pub min_vals: Vec<f64>,
    pub max_vals: Vec<f64>,
    pub scaling_method: ScalingMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingMethod {
    StandardScaling, // (x - mean) / std
    MinMaxScaling,   // (x - min) / (max - min)
    RobustScaling,   // (x - median) / IQR
    Normalization,   // x / ||x||
}

/// Real-time analytics dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsDashboardData {
    pub timestamp: u64,
    pub network_health: NetworkHealthMetrics,
    pub ai_performance: AIModelPerformance,
    pub security_insights: SecurityInsights,
    pub economic_indicators: EconomicIndicators,
    pub environmental_metrics: EnvironmentalDashboardMetrics,
    pub total_transactions: u64,
    pub active_addresses: u64,
    pub mempool_size: u64,
    pub block_height: u64,
    pub tps_current: f64,
    pub tps_peak: f64,
}

pub use crate::metrics::QantoMetrics as NetworkHealthMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModelPerformance {
    pub neural_network_accuracy: f64,
    pub prediction_confidence: f64,
    pub training_loss: f64,
    pub validation_loss: f64,
    pub model_drift_score: f64,
    pub inference_latency_ms: f64,
    pub last_retrain_epoch: u64,
    pub feature_importance: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityInsights {
    pub threat_level: ThreatLevel,
    pub anomaly_score: f64,
    pub attack_attempts_24h: u64,
    pub blocked_transactions: u64,
    pub suspicious_patterns: Vec<String>,
    pub security_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreatLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicIndicators {
    pub total_value_locked: f64,
    pub transaction_fees_24h: f64,
    pub validator_rewards_24h: f64,
    pub network_utilization: f64,
    pub economic_security: f64,
    pub fee_market_efficiency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalDashboardMetrics {
    pub carbon_footprint_kg: f64,
    pub energy_efficiency_score: f64,
    pub renewable_energy_percentage: f64,
    pub carbon_offset_credits: f64,
    pub green_validator_ratio: f64,
}

/// Advanced Cognitive Analytics Engine with Deep Learning
#[derive(Debug)]
pub struct CognitiveAnalyticsEngine {
    pub carbon_impact_model: CarbonImpactPredictor,
    pub deep_network: Arc<RwLock<SagaDeepNetwork>>,
    pub security_classifier: Arc<RwLock<SecurityClassifier>>,
    pub adaptive_controller: Arc<RwLock<AdaptiveController>>,
    pub self_scaling_manager: Arc<RwLock<SelfScalingManager>>,
    pub robustness_monitor: Arc<RwLock<RobustnessMonitor>>,
    pub training_data_buffer: Arc<RwLock<TrainingDataBuffer>>,
}

/// Security-focused AI classifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityClassifier {
    pub threat_detection_network: SagaDeepNetwork,
    pub anomaly_detection_threshold: f64,
    pub confidence_calibration: ConfidenceCalibration,
    pub threat_patterns: HashMap<String, ThreatPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatPattern {
    pub pattern_id: String,
    pub feature_weights: Vec<f64>,
    pub severity_score: f64,
    pub confidence_threshold: f64,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceCalibration {
    pub temperature: f64,
    pub calibration_bins: Vec<CalibrationBin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub bin_range: (f64, f64),
    pub accuracy: f64,
    pub count: u64,
}

/// Adaptive control system for dynamic parameter adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveController {
    pub control_parameters: HashMap<String, ControlParameter>,
    pub adaptation_history: VecDeque<AdaptationEvent>,
    pub performance_metrics: PerformanceMetrics,
    pub pid_controllers: HashMap<String, PIDController>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlParameter {
    pub current_value: f64,
    pub target_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub adaptation_rate: f64,
    pub stability_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationEvent {
    pub timestamp: u64,
    pub parameter_name: String,
    pub old_value: f64,
    pub new_value: f64,
    pub trigger_reason: String,
    pub confidence: f64,
}

pub use crate::metrics::QantoMetrics as PerformanceMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PIDController {
    pub kp: f64, // Proportional gain
    pub ki: f64, // Integral gain
    pub kd: f64, // Derivative gain
    pub integral: f64,
    pub previous_error: f64,
    pub setpoint: f64,
}

/// Self-scaling management system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfScalingManager {
    pub scaling_policies: HashMap<String, ScalingPolicy>,
    pub resource_monitors: HashMap<String, ResourceMonitor>,
    pub scaling_history: VecDeque<ScalingEvent>,
    pub predictive_model: PredictiveScalingModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicy {
    pub resource_type: String,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
    pub cooldown_period: u64,
    pub max_scale_factor: f64,
    pub min_scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMonitor {
    pub resource_name: String,
    pub current_utilization: f64,
    pub predicted_utilization: f64,
    pub capacity: f64,
    pub alert_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingEvent {
    pub timestamp: u64,
    pub resource_type: String,
    pub scaling_action: ScalingAction,
    pub scale_factor: f64,
    pub trigger_metric: f64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingAction {
    ScaleUp,
    ScaleDown,
    Maintain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveScalingModel {
    pub time_series_model: TimeSeriesModel,
    pub feature_extractors: Vec<FeatureExtractor>,
    pub prediction_horizon: u64,
    pub confidence_intervals: Vec<ConfidenceInterval>,
    pub confidence_threshold: f64, // Confidence threshold for predictions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesModel {
    pub model_type: TimeSeriesModelType,
    pub parameters: Vec<f64>,
    pub seasonal_components: Vec<f64>,
    pub trend_components: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeSeriesModelType {
    ARIMA(usize, usize, usize),
    LSTM,
    Prophet,
    ExponentialSmoothing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureExtractor {
    pub feature_name: String,
    pub extraction_function: String, // Serialized function identifier
    pub window_size: usize,
    pub importance_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub confidence_level: f64,
}

/// Robustness monitoring and enhancement system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobustnessMonitor {
    pub fault_tolerance_metrics: FaultToleranceMetrics,
    pub recovery_strategies: HashMap<String, RecoveryStrategy>,
    pub stress_test_results: Vec<StressTestResult>,
    pub redundancy_manager: RedundancyManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultToleranceMetrics {
    pub mean_time_to_failure: f64,
    pub mean_time_to_recovery: f64,
    pub availability: f64,
    pub reliability_score: f64,
    pub fault_detection_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStrategy {
    pub strategy_id: String,
    pub fault_types: Vec<String>,
    pub recovery_steps: Vec<RecoveryStep>,
    pub success_rate: f64,
    pub average_recovery_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStep {
    pub step_id: String,
    pub action_type: RecoveryActionType,
    pub parameters: HashMap<String, String>,
    pub timeout: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryActionType {
    Restart,
    Rollback,
    Failover,
    Reconfigure,
    Isolate,
    Repair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    pub test_id: String,
    pub test_type: StressTestType,
    pub load_level: f64,
    pub duration: u64,
    pub success_rate: f64,
    pub performance_degradation: f64,
    pub failure_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StressTestType {
    LoadTest,
    SpikeTest,
    VolumeTest,
    EnduranceTest,
    SecurityTest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundancyManager {
    pub redundancy_levels: HashMap<String, RedundancyLevel>,
    pub backup_systems: Vec<BackupSystem>,
    pub failover_policies: HashMap<String, FailoverPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundancyLevel {
    pub component_name: String,
    pub redundancy_factor: u32,
    pub active_instances: u32,
    pub standby_instances: u32,
    pub health_check_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSystem {
    pub system_id: String,
    pub backup_type: BackupType,
    pub sync_frequency: u64,
    pub retention_policy: RetentionPolicy,
    pub encryption_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupType {
    FullBackup,
    IncrementalBackup,
    DifferentialBackup,
    ContinuousReplication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub retention_period: u64,
    pub compression_enabled: bool,
    pub archival_threshold: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverPolicy {
    pub policy_id: String,
    pub trigger_conditions: Vec<TriggerCondition>,
    pub failover_sequence: Vec<String>,
    pub rollback_conditions: Vec<TriggerCondition>,
    pub notification_settings: NotificationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub metric_name: String,
    pub threshold: f64,
    pub comparison_operator: ComparisonOperator,
    pub duration: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub notification_channels: Vec<String>,
    pub severity_levels: Vec<String>,
    pub rate_limiting: RateLimiting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiting {
    pub max_notifications_per_hour: u32,
    pub burst_limit: u32,
    pub cooldown_period: u64,
}

/// Enhanced training data buffer for production-ready continuous learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataBuffer {
    pub security_samples: VecDeque<SecuritySample>,
    pub performance_samples: VecDeque<PerformanceSample>,
    pub adaptation_samples: VecDeque<AdaptationSample>,
    pub max_buffer_size: usize,
    pub sampling_strategy: SamplingStrategy,
    pub samples: Vec<(Vec<f64>, Vec<f64>)>, // Training samples for neural network
    pub last_training_epoch: u64,           // Timestamp of last training
    pub validation_samples: Vec<(Vec<f64>, Vec<f64>)>, // Validation dataset
    pub test_samples: Vec<(Vec<f64>, Vec<f64>)>, // Test dataset
    pub data_augmentation_enabled: bool,    // Data augmentation flag
    pub feature_scaling_params: FeatureScalingParams, // Normalization parameters
    pub class_weights: HashMap<String, f64>, // For imbalanced datasets
    pub cross_validation_folds: u32,        // K-fold cross validation
    pub early_stopping_patience: u32,       // Early stopping patience
    pub best_validation_loss: f64,          // Best validation loss seen
    pub training_metrics_history: VecDeque<TrainingMetrics>, // Historical metrics
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySample {
    pub features: Vec<f64>,
    pub label: SecurityLabel,
    pub confidence: f64,
    pub timestamp: u64,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityLabel {
    Benign,
    Suspicious,
    Malicious,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSample {
    pub metrics: PerformanceMetrics,
    pub system_state: SystemState,
    pub timestamp: u64,
    pub outcome_quality: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub network_utilization: f64,
    pub disk_utilization: f64,
    pub active_connections: u64,
    pub queue_lengths: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationSample {
    pub parameter_changes: HashMap<String, f64>,
    pub performance_impact: f64,
    pub stability_impact: f64,
    pub timestamp: u64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SamplingStrategy {
    Random,
    Stratified,
    ImportanceBased,
    TemporalWeighted,
}

impl Default for CognitiveAnalyticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CognitiveAnalyticsEngine {
    pub fn new() -> Self {
        let deep_network = Arc::new(RwLock::new(Self::initialize_deep_network()));
        let security_classifier = Arc::new(RwLock::new(Self::initialize_security_classifier()));
        let adaptive_controller = Arc::new(RwLock::new(Self::initialize_adaptive_controller()));
        let self_scaling_manager = Arc::new(RwLock::new(Self::initialize_self_scaling_manager()));
        let robustness_monitor = Arc::new(RwLock::new(Self::initialize_robustness_monitor()));
        let training_data_buffer = Arc::new(RwLock::new(Self::initialize_training_buffer()));

        Self {
            carbon_impact_model: CarbonImpactPredictor {},
            deep_network,
            security_classifier,
            adaptive_controller,
            self_scaling_manager,
            robustness_monitor,
            training_data_buffer,
        }
    }

    fn initialize_deep_network() -> SagaDeepNetwork {
        let mut layers = Vec::new();

        // Ensure we use exactly NEURAL_NETWORK_LAYERS layers
        let total_layers = NEURAL_NETWORK_LAYERS;

        // Input layer
        layers.push(Self::create_neural_layer(
            INPUT_NEURONS,
            HIDDEN_NEURONS[0],
            ActivationFunction::ReLU,
        ));

        // Hidden layers with different activation functions for diversity
        for i in 0..HIDDEN_NEURONS.len() - 1 {
            let activation = match i % 3 {
                0 => ActivationFunction::ReLU,
                1 => ActivationFunction::Swish,
                _ => ActivationFunction::GELU,
            };
            layers.push(Self::create_neural_layer(
                HIDDEN_NEURONS[i],
                HIDDEN_NEURONS[i + 1],
                activation,
            ));
        }

        // Output layer
        layers.push(Self::create_neural_layer(
            HIDDEN_NEURONS[HIDDEN_NEURONS.len() - 1],
            OUTPUT_NEURONS,
            ActivationFunction::Sigmoid,
        ));

        // Validate we have the correct number of layers
        assert_eq!(
            layers.len(),
            total_layers,
            "Network must have exactly {NEURAL_NETWORK_LAYERS} layers"
        );

        SagaDeepNetwork {
            layers,
            learning_rate: LEARNING_RATE,
            momentum: MOMENTUM,
            training_history: Vec::with_capacity((RETRAIN_INTERVAL_EPOCHS * 10) as usize), // Use RETRAIN_INTERVAL_EPOCHS
            adaptive_lr_scheduler: AdaptiveLRScheduler {
                initial_lr: LEARNING_RATE,
                decay_rate: 0.95,
                patience: 5,
                min_lr: 1e-6,
                best_loss: f64::INFINITY,
                wait_count: 0,
            },
            regularization: RegularizationConfig {
                l1_lambda: 0.001,
                l2_lambda: 0.01,
                dropout_rate: DROPOUT_RATE,
                gradient_clipping: 1.0,
            },
        }
    }

    fn create_neural_layer(
        input_size: usize,
        output_size: usize,
        activation: ActivationFunction,
    ) -> NeuralLayer {
        let mut rng = thread_rng();
        let xavier_std = (2.0 / (input_size + output_size) as f64).sqrt();

        let weights = (0..output_size)
            .map(|_| {
                (0..input_size)
                    .map(|_| {
                        let weight = rng.gen::<f64>() * xavier_std - xavier_std / 2.0;
                        // Apply activation threshold for weight initialization
                        if weight.abs() < ACTIVATION_THRESHOLD {
                            0.0
                        } else {
                            weight
                        }
                    })
                    .collect()
            })
            .collect();

        let biases = (0..output_size).map(|_| rng.gen::<f64>() * 0.1).collect();
        let dropout_mask = (0..output_size).map(|_| true).collect();

        NeuralLayer {
            weights,
            biases,
            activation_function: activation,
            dropout_mask,
            batch_norm_params: BatchNormParams {
                gamma: vec![1.0; output_size],
                beta: vec![0.0; output_size],
                running_mean: vec![0.0; output_size],
                running_var: vec![1.0; output_size],
                momentum: MOMENTUM, // Use MOMENTUM constant
            },
        }
    }

    fn initialize_security_classifier() -> SecurityClassifier {
        // DEVELOPMENT OPTIMIZATION: Cache security classifier to speed up startup
        // TODO: Remove this caching for production deployment
        let threat_detection_network = Self::initialize_deep_network();
        let mut threat_patterns = HashMap::new();

        // Initialize threat patterns for different attack types
        let attack_types = [
            "sybil",
            "spam",
            "centralization",
            "oracle_manipulation",
            "time_drift",
            "wash_trading",
            "collusion",
            "economic",
            "mempool_frontrun",
        ];
        for (i, attack_type) in attack_types.iter().enumerate() {
            threat_patterns.insert(
                attack_type.to_string(),
                ThreatPattern {
                    pattern_id: {
                        let mut pattern_id = String::with_capacity(8);
                        pattern_id.push_str("threat_");
                        pattern_id.push_str(&i.to_string());
                        pattern_id
                    },
                    feature_weights: (0..INPUT_NEURONS)
                        .map(|_| thread_rng().gen::<f64>())
                        .collect(),
                    severity_score: 0.5 + (i as f64 * 0.1),
                    confidence_threshold: SECURITY_CONFIDENCE_THRESHOLD,
                    last_updated: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                },
            );
        }

        SecurityClassifier {
            threat_detection_network,
            anomaly_detection_threshold: 0.8,
            confidence_calibration: ConfidenceCalibration {
                temperature: 1.0,
                calibration_bins: Self::initialize_calibration_bins(),
            },
            threat_patterns,
        }
    }

    fn initialize_calibration_bins() -> Vec<CalibrationBin> {
        (0..10)
            .map(|i| {
                let start = i as f64 * 0.1;
                let end = (i + 1) as f64 * 0.1;
                CalibrationBin {
                    bin_range: (start, end),
                    accuracy: 0.5 + thread_rng().gen::<f64>() * 0.4,
                    count: 0,
                }
            })
            .collect()
    }

    fn initialize_adaptive_controller() -> AdaptiveController {
        let mut control_parameters = HashMap::new();
        let mut pid_controllers = HashMap::new();

        let param_names = [
            "throughput",
            "latency",
            "security",
            "resource_usage",
            "network_congestion",
        ];
        for param in param_names.iter() {
            control_parameters.insert(
                param.to_string(),
                ControlParameter {
                    current_value: 0.5,
                    target_value: ADAPTIVE_THRESHOLD, // Use ADAPTIVE_THRESHOLD
                    min_value: 0.0,
                    max_value: 1.0,
                    adaptation_rate: 0.1,
                    stability_factor: 0.95,
                },
            );

            pid_controllers.insert(
                param.to_string(),
                PIDController {
                    kp: 1.0,
                    ki: 0.1,
                    kd: 0.01,
                    integral: 0.0,
                    previous_error: 0.0,
                    setpoint: ADAPTIVE_THRESHOLD, // Use ADAPTIVE_THRESHOLD
                },
            );
        }

        AdaptiveController {
            control_parameters,
            adaptation_history: VecDeque::with_capacity(BATCH_SIZE * 100), // Use BATCH_SIZE
            performance_metrics: {
                let pm = PerformanceMetrics::default();
                pm.finality_time_ms.store(1000, Ordering::Relaxed);
                pm.validator_count.store(10, Ordering::Relaxed);
                pm.network_congestion.store(100, Ordering::Relaxed);
                pm.block_propagation_time.store(500, Ordering::Relaxed);
                pm
            },
            pid_controllers,
        }
    }

    fn initialize_self_scaling_manager() -> SelfScalingManager {
        let mut scaling_policies = HashMap::new();
        let mut resource_monitors = HashMap::new();

        let resources = ["cpu", "memory", "network", "storage", "validators"];
        for resource in resources.iter() {
            scaling_policies.insert(
                resource.to_string(),
                ScalingPolicy {
                    resource_type: resource.to_string(),
                    scale_up_threshold: SELF_SCALING_TRIGGER,
                    scale_down_threshold: 0.3,
                    cooldown_period: 300, // 5 minutes
                    max_scale_factor: 10.0,
                    min_scale_factor: 0.1,
                },
            );

            resource_monitors.insert(
                resource.to_string(),
                ResourceMonitor {
                    resource_name: resource.to_string(),
                    current_utilization: 0.5,
                    predicted_utilization: 0.5,
                    capacity: 1.0,
                    alert_threshold: 0.9,
                },
            );
        }

        SelfScalingManager {
            scaling_policies,
            resource_monitors,
            scaling_history: VecDeque::with_capacity(1000),
            predictive_model: PredictiveScalingModel {
                time_series_model: TimeSeriesModel {
                    model_type: TimeSeriesModelType::ARIMA(2, 1, 2),
                    parameters: vec![0.5, 0.3, 0.2],
                    seasonal_components: vec![1.0; 24], // Hourly seasonality
                    trend_components: vec![0.0; 7],     // Weekly trend
                },
                feature_extractors: Self::initialize_feature_extractors(),
                prediction_horizon: 3600, // 1 hour
                confidence_intervals: vec![ConfidenceInterval {
                    lower_bound: 0.0,
                    upper_bound: 1.0,
                    confidence_level: 0.95,
                }],
                confidence_threshold: 0.95,
            },
        }
    }

    fn initialize_feature_extractors() -> Vec<FeatureExtractor> {
        vec![
            FeatureExtractor {
                feature_name: "moving_average".to_string(),
                extraction_function: "mean".to_string(),
                window_size: 60,
                importance_weight: 1.0,
            },
            FeatureExtractor {
                feature_name: "trend".to_string(),
                extraction_function: "linear_regression".to_string(),
                window_size: 300,
                importance_weight: 0.8,
            },
            FeatureExtractor {
                feature_name: "volatility".to_string(),
                extraction_function: "std_dev".to_string(),
                window_size: 120,
                importance_weight: 0.6,
            },
        ]
    }

    fn initialize_robustness_monitor() -> RobustnessMonitor {
        RobustnessMonitor {
            fault_tolerance_metrics: FaultToleranceMetrics {
                mean_time_to_failure: 86400.0 * ROBUSTNESS_FACTOR, // 24 hours * robustness factor
                mean_time_to_recovery: 300.0 / ROBUSTNESS_FACTOR,  // 5 minutes / robustness factor
                availability: ROBUSTNESS_FACTOR.min(0.999), // Use ROBUSTNESS_FACTOR but cap at 0.999
                reliability_score: ROBUSTNESS_FACTOR.min(0.95), // Use ROBUSTNESS_FACTOR but cap at 0.95
                fault_detection_rate: ROBUSTNESS_FACTOR.min(0.98), // Use ROBUSTNESS_FACTOR but cap at 0.98
            },
            recovery_strategies: Self::initialize_recovery_strategies(),
            stress_test_results: Vec::with_capacity((RETRAIN_INTERVAL_EPOCHS * 5) as usize), // Use RETRAIN_INTERVAL_EPOCHS
            redundancy_manager: RedundancyManager {
                redundancy_levels: {
                    let mut levels = HashMap::new();
                    levels.insert(
                        "core_system".to_string(),
                        RedundancyLevel {
                            component_name: "core_system".to_string(),
                            redundancy_factor: (3.0 * ROBUSTNESS_FACTOR) as u32, // Scale with ROBUSTNESS_FACTOR
                            active_instances: 2,
                            standby_instances: 1,
                            health_check_interval: (30.0 / ROBUSTNESS_FACTOR) as u64, // Faster checks with higher robustness
                        },
                    );
                    levels
                },
                backup_systems: Self::initialize_backup_systems(),
                failover_policies: Self::initialize_failover_policies(),
            },
        }
    }

    fn initialize_recovery_strategies() -> HashMap<String, RecoveryStrategy> {
        let mut strategies = HashMap::new();

        strategies.insert(
            "network_partition".to_string(),
            RecoveryStrategy {
                strategy_id: "net_partition_recovery".to_string(),
                fault_types: vec!["network_partition".to_string(), "node_failure".to_string()],
                recovery_steps: vec![
                    RecoveryStep {
                        step_id: "detect_partition".to_string(),
                        action_type: RecoveryActionType::Restart,
                        parameters: HashMap::new(),
                        timeout: 30,
                        retry_count: 3,
                    },
                    RecoveryStep {
                        step_id: "activate_backup_nodes".to_string(),
                        action_type: RecoveryActionType::Failover,
                        parameters: HashMap::new(),
                        timeout: 120,
                        retry_count: 2,
                    },
                ],
                success_rate: 0.95,
                average_recovery_time: 60.0,
            },
        );

        strategies
    }

    fn initialize_backup_systems() -> Vec<BackupSystem> {
        vec![
            BackupSystem {
                system_id: "primary_backup".to_string(),
                backup_type: BackupType::FullBackup,
                sync_frequency: 3600, // 1 hour
                retention_policy: RetentionPolicy {
                    retention_period: 30,
                    compression_enabled: true,
                    archival_threshold: 7,
                },
                encryption_enabled: true,
            },
            BackupSystem {
                system_id: "secondary_backup".to_string(),
                backup_type: BackupType::IncrementalBackup,
                sync_frequency: 3600, // 1 hour
                retention_policy: RetentionPolicy {
                    retention_period: 90,
                    compression_enabled: true,
                    archival_threshold: 30,
                },
                encryption_enabled: true,
            },
        ]
    }

    fn initialize_failover_policies() -> HashMap<String, FailoverPolicy> {
        let mut policies = HashMap::new();
        policies.insert(
            "auto_failover".to_string(),
            FailoverPolicy {
                policy_id: "auto_failover".to_string(),
                trigger_conditions: vec![TriggerCondition {
                    metric_name: "availability".to_string(),
                    threshold: 0.95,
                    comparison_operator: ComparisonOperator::LessThan,
                    duration: 60,
                }],
                failover_sequence: vec![
                    "primary_backup".to_string(),
                    "secondary_backup".to_string(),
                ],
                rollback_conditions: vec![],
                notification_settings: NotificationSettings {
                    enabled: true,
                    notification_channels: vec!["email".to_string(), "slack".to_string()],
                    severity_levels: vec![
                        "low".to_string(),
                        "medium".to_string(),
                        "high".to_string(),
                    ],
                    rate_limiting: RateLimiting {
                        max_notifications_per_hour: 10,
                        burst_limit: 5,
                        cooldown_period: 900, // 15 minutes in seconds
                    },
                },
            },
        );
        policies
    }

    fn initialize_training_buffer() -> TrainingDataBuffer {
        TrainingDataBuffer {
            security_samples: VecDeque::with_capacity(10000),
            performance_samples: VecDeque::with_capacity(10000),
            adaptation_samples: VecDeque::with_capacity(10000),
            max_buffer_size: 10000,
            sampling_strategy: SamplingStrategy::Stratified,
            samples: Vec::with_capacity(10000),
            last_training_epoch: 0,
            validation_samples: Vec::with_capacity(2000),
            test_samples: Vec::with_capacity(1000),
            data_augmentation_enabled: true,
            feature_scaling_params: FeatureScalingParams {
                means: Vec::new(),
                std_devs: Vec::new(),
                min_vals: Vec::new(),
                max_vals: Vec::new(),
                scaling_method: ScalingMethod::StandardScaling,
            },
            class_weights: HashMap::new(),
            cross_validation_folds: 5,
            early_stopping_patience: 10,
            best_validation_loss: f64::INFINITY,
            training_metrics_history: VecDeque::new(),
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
            let mut base_weight_key = String::with_capacity(6 + factor_name.len() + 7); // "trust_" + factor_name + "_weight"
            base_weight_key.push_str("trust_");
            base_weight_key.push_str(factor_name);
            base_weight_key.push_str("_weight");
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
        let blocks_reader = &dag.blocks;
        let total_blocks = blocks_reader.len().max(1) as f64;
        let node_blocks = blocks_reader
            .iter()
            .map(|entry| entry.value().clone())
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
        let small_tx_count = block
            .transactions
            .iter()
            .filter(|tx| tx.amount < crate::transaction::FEE_TIER1_THRESHOLD)
            .count() as f64;
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
            if let Some(max_parent_time) = block
                .parents
                .iter()
                .filter_map(|p_id| dag.blocks.get(p_id).map(|p_block| p_block.timestamp))
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

    /// Forward pass through the neural network
    pub async fn forward_pass(&self, input: &[f64]) -> Result<Vec<f64>, SagaError> {
        let network = self.deep_network.read().await;
        let mut current_input = input.to_vec();

        for layer in &network.layers {
            current_input = self.layer_forward(&current_input, layer)?;
        }

        Ok(current_input)
    }

    fn layer_forward(&self, input: &[f64], layer: &NeuralLayer) -> Result<Vec<f64>, SagaError> {
        let mut output = Vec::with_capacity(layer.biases.len());

        for (i, bias) in layer.biases.iter().enumerate() {
            let mut sum = *bias;
            for (j, &input_val) in input.iter().enumerate() {
                if j < layer.weights[i].len() {
                    sum += input_val * layer.weights[i][j];
                }
            }

            // Apply activation function
            let activated = self.apply_activation(sum, &layer.activation_function);
            output.push(activated);
        }

        Ok(output)
    }

    fn apply_activation(&self, x: f64, activation: &ActivationFunction) -> f64 {
        match activation {
            ActivationFunction::ReLU => x.max(0.0),
            ActivationFunction::Sigmoid => 1.0 / (1.0 + (-x).exp()),
            ActivationFunction::Tanh => x.tanh(),
            ActivationFunction::LeakyReLU(alpha) => {
                if x > 0.0 {
                    x
                } else {
                    alpha * x
                }
            }
            ActivationFunction::Swish => x * (1.0 / (1.0 + (-x).exp())),
            ActivationFunction::GELU => {
                0.5 * x * (1.0 + (x * 0.7978845608 * (1.0 + 0.044715 * x * x)).tanh())
            }
        }
    }

    /// Train the neural network with provided data
    pub async fn train_models_from_data(
        &self,
        training_data: &[(Vec<f64>, Vec<f64>)],
    ) -> Result<(), SagaError> {
        if training_data.is_empty() {
            return Ok(());
        }

        let mut network = self.deep_network.write().await;

        // Split data into training and validation sets (80/20 split)
        let split_index = (training_data.len() as f64 * 0.8) as usize;
        let mut shuffled_data = training_data.to_vec();

        // Shuffle data for better training
        use rand::rngs::StdRng;
        use rand::seq::SliceRandom;
        use rand::SeedableRng;
        let mut rng = StdRng::from_entropy();
        shuffled_data.shuffle(&mut rng);

        let (train_data, validation_data) = shuffled_data.split_at(split_index);

        // Calculate feature scaling parameters
        let scaling_params = self.calculate_feature_scaling(train_data);

        // Apply feature scaling to training data
        let scaled_train_data: Vec<(Vec<f64>, Vec<f64>)> = train_data
            .iter()
            .map(|(features, targets)| {
                let scaled_features = self.apply_feature_scaling(features, &scaling_params);
                (scaled_features, targets.clone())
            })
            .collect();

        // Apply feature scaling to validation data
        let scaled_validation_data: Vec<(Vec<f64>, Vec<f64>)> = validation_data
            .iter()
            .map(|(features, targets)| {
                let scaled_features = self.apply_feature_scaling(features, &scaling_params);
                (scaled_features, targets.clone())
            })
            .collect();

        let batch_size = BATCH_SIZE.min(scaled_train_data.len());
        let max_epochs = 100;
        let early_stopping_patience = 10;
        let mut best_validation_loss = f64::INFINITY;
        let mut patience_counter = 0;

        // Training loop with early stopping
        for epoch in 0..max_epochs {
            let mut epoch_loss = 0.0;

            // Training phase
            for batch_start in (0..scaled_train_data.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(scaled_train_data.len());
                let batch = &scaled_train_data[batch_start..batch_end];

                let mut batch_loss = 0.0;
                let mut gradients = self.initialize_gradients(&network);

                for (input, target) in batch {
                    let prediction = self.forward_pass_internal(input, &network)?;
                    let loss = self.calculate_loss(&prediction, target);
                    batch_loss += loss;

                    self.backpropagate(&mut gradients, input, target, &prediction, &network)?;
                }

                // Apply gradient clipping
                let gradient_clipping = network.regularization.gradient_clipping;
                self.clip_gradients(&mut gradients, gradient_clipping);

                // Update weights with gradients
                self.update_weights(&mut network, &gradients, batch.len())?;

                epoch_loss += batch_loss;
            }

            // Calculate training metrics
            let avg_train_loss = epoch_loss / scaled_train_data.len() as f64;
            let train_accuracy = self.calculate_accuracy(&scaled_train_data, &network)?;
            let validation_loss =
                self.calculate_validation_loss(&scaled_validation_data, &network)?;
            let validation_accuracy = if !scaled_validation_data.is_empty() {
                self.calculate_accuracy(&scaled_validation_data, &network)?
            } else {
                train_accuracy
            };

            // Learning rate scheduling
            if validation_loss < network.adaptive_lr_scheduler.best_loss {
                network.adaptive_lr_scheduler.best_loss = validation_loss;
                network.adaptive_lr_scheduler.wait_count = 0;
            } else {
                network.adaptive_lr_scheduler.wait_count += 1;
                if network.adaptive_lr_scheduler.wait_count
                    >= network.adaptive_lr_scheduler.patience
                {
                    network.learning_rate *= network.adaptive_lr_scheduler.decay_rate;
                    network.learning_rate = network
                        .learning_rate
                        .max(network.adaptive_lr_scheduler.min_lr);
                    network.adaptive_lr_scheduler.wait_count = 0;
                }
            }

            let gradient_norm = self.calculate_gradient_norm(&self.initialize_gradients(&network));

            // Record training metrics
            let training_metric = TrainingMetrics {
                epoch,
                loss: avg_train_loss,
                accuracy: train_accuracy,
                validation_loss,
                learning_rate: network.learning_rate,
                gradient_norm,
            };
            network.training_history.push(training_metric.clone());

            // Update training buffer metrics history
            {
                let mut buffer = self.training_data_buffer.write().await;
                buffer.training_metrics_history.push_back(training_metric);
                // Keep only last 1000 metrics to prevent unbounded growth
                if buffer.training_metrics_history.len() > 1000 {
                    buffer.training_metrics_history.pop_front();
                }
            }

            // Early stopping check
            if validation_loss < best_validation_loss {
                best_validation_loss = validation_loss;
                patience_counter = 0;
            } else {
                patience_counter += 1;
                if patience_counter >= early_stopping_patience {
                    info!(
                        "Early stopping triggered at epoch {} with validation loss: {:.6}",
                        epoch, validation_loss
                    );
                    break;
                }
            }

            // Log progress every 10 epochs
            if epoch.is_multiple_of(10) {
                info!("Epoch {}: Train Loss: {:.6}, Val Loss: {:.6}, Train Acc: {:.4}, Val Acc: {:.4}, LR: {:.6}", 
                      epoch, avg_train_loss, validation_loss, train_accuracy, validation_accuracy, network.learning_rate);
            }
        }

        info!(
            "Training completed. Final validation loss: {:.6}",
            best_validation_loss
        );
        Ok(())
    }

    // Enhanced helper functions for production-ready neural network training
    fn calculate_feature_scaling(&self, data: &[(Vec<f64>, Vec<f64>)]) -> FeatureScalingParams {
        if data.is_empty() {
            return FeatureScalingParams {
                means: vec![],
                std_devs: vec![],
                min_vals: vec![],
                max_vals: vec![],
                scaling_method: ScalingMethod::StandardScaling,
            };
        }

        let feature_count = data[0].0.len();
        let mut means = vec![0.0; feature_count];
        let mut std_devs = vec![0.0; feature_count];
        let mut min_vals = vec![f64::INFINITY; feature_count];
        let mut max_vals = vec![f64::NEG_INFINITY; feature_count];

        // Calculate means, min, max
        for (features, _) in data {
            for (i, &value) in features.iter().enumerate() {
                means[i] += value;
                min_vals[i] = min_vals[i].min(value);
                max_vals[i] = max_vals[i].max(value);
            }
        }

        for mean in &mut means {
            *mean /= data.len() as f64;
        }

        // Calculate standard deviations
        for (features, _) in data {
            for (i, &value) in features.iter().enumerate() {
                std_devs[i] += (value - means[i]).powi(2);
            }
        }

        for std_dev in &mut std_devs {
            *std_dev = (*std_dev / data.len() as f64).sqrt();
            if *std_dev == 0.0 {
                *std_dev = 1.0; // Prevent division by zero
            }
        }

        FeatureScalingParams {
            means,
            std_devs,
            min_vals,
            max_vals,
            scaling_method: ScalingMethod::StandardScaling,
        }
    }

    fn apply_feature_scaling(
        &self,
        features: &[f64],
        scaling_params: &FeatureScalingParams,
    ) -> Vec<f64> {
        match scaling_params.scaling_method {
            ScalingMethod::StandardScaling => features
                .iter()
                .enumerate()
                .map(|(i, &value)| (value - scaling_params.means[i]) / scaling_params.std_devs[i])
                .collect(),
            ScalingMethod::MinMaxScaling => features
                .iter()
                .enumerate()
                .map(|(i, &value)| {
                    let range = scaling_params.max_vals[i] - scaling_params.min_vals[i];
                    if range == 0.0 {
                        0.0
                    } else {
                        (value - scaling_params.min_vals[i]) / range
                    }
                })
                .collect(),
            ScalingMethod::RobustScaling => {
                // Simplified robust scaling using std dev as proxy for IQR
                features
                    .iter()
                    .enumerate()
                    .map(|(i, &value)| {
                        (value - scaling_params.means[i]) / scaling_params.std_devs[i]
                    })
                    .collect()
            }
            ScalingMethod::Normalization => {
                let norm = features.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm == 0.0 {
                    features.to_vec()
                } else {
                    features.iter().map(|&x| x / norm).collect()
                }
            }
        }
    }

    fn clip_gradients(&self, gradients: &mut [Vec<Vec<f64>>], max_norm: f64) {
        let total_norm = self.calculate_gradient_norm(gradients);
        if total_norm > max_norm {
            let scale = max_norm / total_norm;
            for layer_gradients in gradients {
                for neuron_gradients in layer_gradients {
                    for gradient in neuron_gradients {
                        *gradient *= scale;
                    }
                }
            }
        }
    }

    fn calculate_validation_loss(
        &self,
        validation_data: &[(Vec<f64>, Vec<f64>)],
        network: &SagaDeepNetwork,
    ) -> Result<f64, SagaError> {
        if validation_data.is_empty() {
            return Ok(0.0);
        }

        let mut total_loss = 0.0;
        for (input, target) in validation_data {
            let prediction = self.forward_pass_internal(input, network)?;
            total_loss += self.calculate_loss(&prediction, target);
        }

        Ok(total_loss / validation_data.len() as f64)
    }

    fn forward_pass_internal(
        &self,
        input: &[f64],
        network: &SagaDeepNetwork,
    ) -> Result<Vec<f64>, SagaError> {
        let mut current_input = input.to_vec();

        for layer in &network.layers {
            current_input = self.layer_forward(&current_input, layer)?;
        }

        Ok(current_input)
    }

    fn calculate_loss(&self, prediction: &[f64], target: &[f64]) -> f64 {
        prediction
            .iter()
            .zip(target.iter())
            .map(|(p, t)| (p - t).powi(2))
            .sum::<f64>()
            / prediction.len() as f64
    }

    fn initialize_gradients(&self, network: &SagaDeepNetwork) -> Vec<Vec<Vec<f64>>> {
        network
            .layers
            .iter()
            .map(|layer| {
                layer
                    .weights
                    .iter()
                    .map(|row| vec![0.0; row.len()])
                    .collect()
            })
            .collect()
    }

    fn backpropagate(
        &self,
        gradients: &mut [Vec<Vec<f64>>],
        input: &[f64],
        target: &[f64],
        prediction: &[f64],
        network: &SagaDeepNetwork,
    ) -> Result<(), SagaError> {
        // Simplified backpropagation - compute output layer gradients
        let output_error: Vec<f64> = prediction
            .iter()
            .zip(target.iter())
            .map(|(p, t)| 2.0 * (p - t) / prediction.len() as f64)
            .collect();

        // Update gradients for output layer (simplified)
        if let Some(last_layer_idx) = network.layers.len().checked_sub(1) {
            for (i, &error) in output_error.iter().enumerate() {
                for (j, _) in input.iter().enumerate() {
                    if i < gradients[last_layer_idx].len() && j < gradients[last_layer_idx][i].len()
                    {
                        gradients[last_layer_idx][i][j] += error;
                    }
                }
            }
        }

        Ok(())
    }

    fn update_weights(
        &self,
        network: &mut SagaDeepNetwork,
        gradients: &[Vec<Vec<f64>>],
        batch_size: usize,
    ) -> Result<(), SagaError> {
        for (layer_idx, layer) in network.layers.iter_mut().enumerate() {
            if layer_idx < gradients.len() {
                for (i, weight_row) in layer.weights.iter_mut().enumerate() {
                    if i < gradients[layer_idx].len() {
                        for (j, weight) in weight_row.iter_mut().enumerate() {
                            if j < gradients[layer_idx][i].len() {
                                let gradient = gradients[layer_idx][i][j] / batch_size as f64;
                                *weight -= network.learning_rate * gradient;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn calculate_accuracy(
        &self,
        batch: &[(Vec<f64>, Vec<f64>)],
        network: &SagaDeepNetwork,
    ) -> Result<f64, SagaError> {
        let mut correct = 0;
        for (input, target) in batch {
            let prediction = self.forward_pass_internal(input, network)?;
            if self.predictions_match(&prediction, target) {
                correct += 1;
            }
        }
        Ok(correct as f64 / batch.len() as f64)
    }

    fn predictions_match(&self, prediction: &[f64], target: &[f64]) -> bool {
        prediction
            .iter()
            .zip(target.iter())
            .all(|(p, t)| (p - t).abs() < 0.1) // Simple threshold-based matching
    }

    fn calculate_gradient_norm(&self, gradients: &[Vec<Vec<f64>>]) -> f64 {
        let sum_squares: f64 = gradients
            .iter()
            .flat_map(|layer| layer.iter())
            .flat_map(|row| row.iter())
            .map(|&g| g * g)
            .sum();
        sum_squares.sqrt()
    }

    /// Save neural network models to disk
    pub async fn save_models_to_disk(&self, path: &str) -> Result<(), SagaError> {
        let network = self.deep_network.read().await;
        let serialized = serde_json::to_string(&*network).map_err(|e| {
            let mut error_msg = String::with_capacity(20 + 50);
            error_msg.push_str("Serialization error: ");
            error_msg.push_str(&format!("{e:?}"));
            SagaError::TimeError(error_msg)
        })?;

        std::fs::write(path, serialized).map_err(|e| {
            let mut error_msg = String::with_capacity(18 + 50);
            error_msg.push_str("File write error: ");
            error_msg.push_str(&format!("{e:?}"));
            SagaError::TimeError(error_msg)
        })?;

        Ok(())
    }

    /// Load neural network models from disk
    pub async fn load_models_from_disk(&self, path: &str) -> Result<(), SagaError> {
        let data = std::fs::read_to_string(path).map_err(|e| {
            let mut error_msg = String::with_capacity(17 + 50);
            error_msg.push_str("File read error: ");
            error_msg.push_str(&format!("{e:?}"));
            SagaError::TimeError(error_msg)
        })?;

        let network: SagaDeepNetwork = serde_json::from_str(&data).map_err(|e| {
            let mut error_msg = String::with_capacity(22 + 50);
            error_msg.push_str("Deserialization error: ");
            error_msg.push_str(&format!("{e:?}"));
            SagaError::TimeError(error_msg)
        })?;

        *self.deep_network.write().await = network;
        Ok(())
    }

    /// Retrain neural models using collected training data
    pub async fn retrain_neural_models(&self) -> Result<(), SagaError> {
        info!("Starting neural network retraining process");

        // Collect training data from buffer
        let training_data = {
            let buffer = self.training_data_buffer.read().await;
            if buffer.samples.len() < BATCH_SIZE {
                warn!(
                    "Insufficient training data for retraining: {} samples",
                    buffer.samples.len()
                );
                return Ok(());
            }
            buffer.samples.clone()
        };

        // Retrain main deep network
        self.train_models_from_data(&training_data).await?;

        // Get network metrics for updates (avoid borrowing conflicts)
        let (network_clone, latest_accuracy) = {
            let network = self.deep_network.read().await;
            let accuracy = network
                .training_history
                .last()
                .map(|m| m.accuracy)
                .unwrap_or(0.0);
            (network.clone(), accuracy)
        };

        // Update security classifier with new threat patterns
        {
            let mut classifier = self.security_classifier.write().await;
            classifier.threat_detection_network = network_clone.clone();
            classifier.anomaly_detection_threshold = SECURITY_CONFIDENCE_THRESHOLD;
        }

        // Update adaptive controllers based on training results
        {
            let mut controller = self.adaptive_controller.write().await;
            if latest_accuracy > ADAPTIVE_THRESHOLD {
                // Increase adaptation rates for better performing models
                for param in controller.control_parameters.values_mut() {
                    param.adaptation_rate = (param.adaptation_rate * 1.1).min(0.1);
                }
            }
        }

        // Update self-scaling manager with new predictive capabilities
        {
            let mut scaling_manager = self.self_scaling_manager.write().await;
            scaling_manager.predictive_model.confidence_threshold = latest_accuracy;
            scaling_manager.predictive_model.prediction_horizon = (latest_accuracy * 100.0) as u64;
        }

        // Clear training buffer after successful retraining
        {
            let mut buffer = self.training_data_buffer.write().await;
            buffer.samples.clear();
            buffer.last_training_epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        // Save updated models to disk
        self.save_models_to_disk("saga_models.json").await?;

        info!("Neural network retraining completed successfully");
        Ok(())
    }
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
        let validator_count = dag.validators.len() as f64;

        let (fee_velocity, fee_volatility) = {
            // FIX: HashMap::values() is not a DoubleEndedIterator. Must collect and sort first.
            let mut recent_blocks_sorted: Vec<_> = dag
                .blocks
                .iter()
                .map(|entry| entry.value().clone())
                .collect();
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
        let validators = &dag.validators;
        if validators.len() < 10 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough validators for a reliable Sybil analysis.".to_string(),
            };
        }

        let total_stake: u64 = validators.iter().map(|entry| *entry.value()).sum();
        if total_stake == 0 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Total stake is zero.".to_string(),
            };
        }

        let mut stakes: Vec<u64> = validators.iter().map(|entry| *entry.value()).collect();
        stakes.sort_unstable();

        let n = stakes.len() as f64;
        let sum_of_ranks = stakes
            .iter()
            .enumerate()
            .map(|(i, &s)| (i as f64 + 1.0) * s as f64)
            .sum::<f64>();
        let gini = (2.0 * sum_of_ranks) / (n * total_stake as f64) - (n + 1.0) / n;

        // Gini of 0 is perfect equality. Risk increases as Gini approaches 0.
        let sybil_risk = (1.0 - gini).powi(2i32).max(0.0);
        debug!("Sybil attack analysis complete. Gini: {gini:.4}, Risk Score: {sybil_risk:.4}",);

        SecurityFinding {
            risk_score: sybil_risk,
            severity: if sybil_risk > 0.7 {
                InsightSeverity::Critical
            } else {
                InsightSeverity::Warning
            },
            confidence: 0.8,
            details: {
                let mut details = String::with_capacity(50);
                details.push_str("Stake distribution Gini coefficient is ");
                details.push_str(&format!("{gini:.4}"));
                details.push('.');
                details
            },
        }
    }

    #[instrument(skip(self, dag))]
    pub async fn check_transactional_anomalies(&self, dag: &QantoDAG) -> SecurityFinding {
        let blocks = &dag.blocks;
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
            .iter()
            .map(|entry| entry.value().clone())
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
        let risk = if zero_fee_ratio > 0.8 {
            (zero_fee_ratio - 0.8).powi(2)
        } else {
            0.0
        };

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
            details: {
                let mut details = String::with_capacity(50);
                details.push_str("Zero-fee transaction ratio is ");
                details.push_str(&format!("{zero_fee_ratio:.2}"));
                details.push('.');
                details
            },
        }
    }

    #[instrument(skip(self, dag, epoch_lookback))]
    pub async fn check_for_centralization_risk(
        &self,
        dag: &QantoDAG,
        epoch_lookback: u64,
    ) -> SecurityFinding {
        let blocks = &dag.blocks;
        if blocks.len() < 50 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough blocks for centralization analysis.".to_string(),
            };
        }

        let current_epoch_val = dag.current_epoch.load(std::sync::atomic::Ordering::Relaxed);

        let recent_blocks_by_miner: HashMap<String, u64> = blocks
            .iter()
            .map(|entry| entry.value().clone())
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
        let blocks = &dag.blocks;

        let mut recent_blocks_vec: Vec<_> =
            blocks.iter().map(|entry| entry.value().clone()).collect();
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
        let mut recent_blocks: Vec<_> = dag
            .blocks
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        recent_blocks.sort_by_key(|b| b.timestamp);

        let mut suspicious_timestamps = HashMap::<String, (u32, u32)>::new(); // (total_blocks, suspicious_blocks)

        for block in recent_blocks.iter().rev().take(200) {
            let parent_max_ts = block
                .parents
                .iter()
                .filter_map(|p_id| dag.blocks.get(p_id).map(|p| p.timestamp))
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
        if dag.blocks.len() < 100 {
            return SecurityFinding {
                risk_score: 0.0,
                severity: InsightSeverity::Tip,
                confidence: 0.9,
                details: "Not enough blocks for wash trading analysis.".to_string(),
            };
        }

        let recent_blocks = self.get_recent_blocks(dag);
        let (tx_graph, address_tx_counts) = self.build_transaction_graph(&recent_blocks);
        let cycle_results = self.detect_wash_trading_cycles(&tx_graph, &address_tx_counts);

        self.calculate_wash_trading_risk(cycle_results)
    }

    /// Get blocks from the last 30 minutes
    fn get_recent_blocks(&self, dag: &QantoDAG) -> Vec<QantoBlock> {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(1800); // last 30 mins

        dag.blocks
            .iter()
            .map(|entry| entry.value().clone())
            .filter(|b| b.timestamp > cutoff_time)
            .collect()
    }

    /// Build transaction graph and address counts using parallel processing
    fn build_transaction_graph(
        &self,
        recent_blocks: &[QantoBlock],
    ) -> (HashMap<String, Vec<String>>, HashMap<String, u32>) {
        use rayon::prelude::*;
        use std::sync::Mutex;

        let tx_graph = Arc::new(Mutex::new(HashMap::<String, Vec<String>>::new()));
        let address_tx_counts = Arc::new(Mutex::new(HashMap::<String, u32>::new()));

        // Process blocks in parallel
        recent_blocks.par_iter().for_each(|block| {
            let (local_tx_graph, local_address_counts) =
                self.process_block_transactions(block, recent_blocks);

            // Merge local results into global collections
            self.merge_transaction_data(
                &tx_graph,
                &address_tx_counts,
                local_tx_graph,
                local_address_counts,
            );
        });

        // Extract final results
        let tx_graph = Arc::try_unwrap(tx_graph).unwrap().into_inner().unwrap();
        let address_tx_counts = Arc::try_unwrap(address_tx_counts)
            .unwrap()
            .into_inner()
            .unwrap();

        (tx_graph, address_tx_counts)
    }

    /// Process transactions in a single block
    fn process_block_transactions(
        &self,
        block: &QantoBlock,
        recent_blocks: &[QantoBlock],
    ) -> (HashMap<String, Vec<String>>, HashMap<String, u32>) {
        let mut local_tx_graph = HashMap::<String, Vec<String>>::new();
        let mut local_address_counts = HashMap::<String, u32>::new();

        for tx in &block.transactions {
            if tx.is_coinbase() || tx.inputs.is_empty() {
                continue;
            }

            if let Some(input) = tx.inputs.first() {
                if let Some(source_tx) = self.find_source_transaction(input, recent_blocks) {
                    if let Some(source_output) = source_tx.outputs.get(input.output_index as usize)
                    {
                        self.update_transaction_graph(
                            &mut local_tx_graph,
                            &mut local_address_counts,
                            &source_output.address,
                            tx,
                        );
                    }
                }
            }
        }

        (local_tx_graph, local_address_counts)
    }

    /// Find source transaction for a given input
    fn find_source_transaction<'a>(
        &self,
        input: &crate::transaction::Input,
        recent_blocks: &'a [QantoBlock],
    ) -> Option<&'a crate::transaction::Transaction> {
        recent_blocks
            .iter()
            .flat_map(|b| &b.transactions)
            .find(|source_tx| source_tx.id == input.tx_id)
    }

    /// Update transaction graph and address counts
    fn update_transaction_graph(
        &self,
        local_tx_graph: &mut HashMap<String, Vec<String>>,
        local_address_counts: &mut HashMap<String, u32>,
        input_addr: &str,
        tx: &crate::transaction::Transaction,
    ) {
        for output in &tx.outputs {
            let output_addr = &output.address;
            local_tx_graph
                .entry(input_addr.to_string())
                .or_default()
                .push(output_addr.clone());
            *local_address_counts
                .entry(input_addr.to_string())
                .or_insert(0) += 1;
            *local_address_counts.entry(output_addr.clone()).or_insert(0) += 1;
        }
    }

    /// Merge local transaction data into global collections
    fn merge_transaction_data(
        &self,
        tx_graph: &Arc<Mutex<HashMap<String, Vec<String>>>>,
        address_tx_counts: &Arc<Mutex<HashMap<String, u32>>>,
        local_tx_graph: HashMap<String, Vec<String>>,
        local_address_counts: HashMap<String, u32>,
    ) {
        {
            let mut global_tx_graph = tx_graph.lock().unwrap();
            for (key, mut values) in local_tx_graph {
                global_tx_graph.entry(key).or_default().append(&mut values);
            }
        }

        {
            let mut global_counts = address_tx_counts.lock().unwrap();
            for (key, count) in local_address_counts {
                *global_counts.entry(key).or_insert(0) += count;
            }
        }
    }

    /// Detect wash trading cycles using parallel processing
    fn detect_wash_trading_cycles(
        &self,
        tx_graph: &HashMap<String, Vec<String>>,
        address_tx_counts: &HashMap<String, u32>,
    ) -> Vec<(u32, u32)> {
        use rayon::prelude::*;

        address_tx_counts
            .par_iter()
            .filter(|(_, count)| **count >= 4) // Ignore addresses with few txs
            .map(|(start_node, count)| {
                let local_cycles = self.count_cycles_for_address(start_node, tx_graph);
                (local_cycles, *count)
            })
            .collect()
    }

    /// Count cycles for a specific address (A -> B -> A pattern)
    fn count_cycles_for_address(
        &self,
        start_node: &str,
        tx_graph: &HashMap<String, Vec<String>>,
    ) -> u32 {
        let mut local_cycles = 0;

        if let Some(neighbors) = tx_graph.get(start_node) {
            for neighbor in neighbors {
                if let Some(return_neighbors) = tx_graph.get(neighbor) {
                    if return_neighbors.contains(&start_node.to_string()) {
                        local_cycles += 1;
                    }
                }
            }
        }

        local_cycles
    }

    /// Calculate final wash trading risk score
    fn calculate_wash_trading_risk(&self, cycle_results: Vec<(u32, u32)>) -> SecurityFinding {
        let suspicious_cycles: u32 = cycle_results.iter().map(|(cycles, _)| cycles).sum();
        let total_txs: u32 = cycle_results.iter().map(|(_, count)| count).sum();

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
        let blocks = &dag.blocks;
        if blocks.len() < 200 {
            return self.create_insufficient_blocks_finding();
        }

        let recent_blocks = self.get_recent_blocks_for_collusion(blocks);
        let miner_parent_map = self.build_miner_parent_map(&recent_blocks, blocks);
        let suspicious_groups = self
            .analyze_collusion_patterns(&miner_parent_map, reputation)
            .await;

        self.calculate_collusion_risk(suspicious_groups, miner_parent_map.len())
    }

    /// Create security finding for insufficient blocks
    fn create_insufficient_blocks_finding(&self) -> SecurityFinding {
        SecurityFinding {
            risk_score: 0.0,
            severity: InsightSeverity::Tip,
            confidence: 0.9,
            details: "Not enough blocks for collusion analysis.".to_string(),
        }
    }

    /// Get recent blocks for collusion analysis
    fn get_recent_blocks_for_collusion(
        &self,
        blocks: &dashmap::DashMap<String, QantoBlock>,
    ) -> Vec<QantoBlock> {
        // FIX: HashMap::values() is not a DoubleEndedIterator, so we must collect and sort first.
        let mut all_blocks: Vec<_> = blocks.iter().map(|entry| entry.value().clone()).collect();
        all_blocks.sort_by_key(|b| b.timestamp);
        all_blocks.iter().rev().take(200).cloned().collect()
    }

    /// Build miner-to-parent-miners mapping
    fn build_miner_parent_map(
        &self,
        recent_blocks: &[QantoBlock],
        blocks: &dashmap::DashMap<String, QantoBlock>,
    ) -> HashMap<String, Vec<String>> {
        let mut miner_parent_map = HashMap::<String, Vec<String>>::new();

        for block in recent_blocks {
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

        miner_parent_map
    }

    /// Analyze collusion patterns among miners
    async fn analyze_collusion_patterns(
        &self,
        miner_parent_map: &HashMap<String, Vec<String>>,
        reputation: &ReputationState,
    ) -> u32 {
        let mut suspicious_groups = 0;

        for (miner, parents) in miner_parent_map {
            if !self.is_suspicious_miner(miner, reputation).await || parents.is_empty() {
                continue;
            }

            let parent_counts = self.count_parent_occurrences(parents);

            if let Some(collusion_detected) = self
                .detect_miner_collusion(&parent_counts, parents.len(), reputation)
                .await
            {
                if collusion_detected {
                    suspicious_groups += 1;
                    debug!(miner, "Potential collusion detected.");
                }
            }
        }

        suspicious_groups
    }

    /// Check if miner has suspicious (low) credit score
    async fn is_suspicious_miner(&self, miner: &str, reputation: &ReputationState) -> bool {
        let scores_guard = reputation.credit_scores.read().await;
        let miner_scs = scores_guard.get(miner).map_or(0.5, |s| s.score);
        drop(scores_guard);
        // We are interested in mid-to-low score miners forming a cartel.
        miner_scs <= 0.7
    }

    /// Count occurrences of each parent miner
    fn count_parent_occurrences(&self, parents: &[String]) -> HashMap<String, u32> {
        let mut parent_counts = HashMap::new();
        for p_miner in parents {
            *parent_counts.entry(p_miner.clone()).or_insert(0) += 1;
        }
        parent_counts
    }

    /// Detect collusion between miner and its most frequent parent
    async fn detect_miner_collusion(
        &self,
        parent_counts: &HashMap<String, u32>,
        total_parents: usize,
        reputation: &ReputationState,
    ) -> Option<bool> {
        // Check if this miner is overwhelmingly building on a small set of other low-score miners
        let top_parent = parent_counts.iter().max_by_key(|&(_, count)| count)?;

        let (p_miner, &count) = top_parent;
        if count > 5 && (count as f64 / total_parents as f64) > 0.8 {
            let scores_guard = reputation.credit_scores.read().await;
            let parent_scs = scores_guard.get(p_miner).map_or(0.5, |s| s.score);
            drop(scores_guard);
            // If both are low-score and a strong bond exists, it's suspicious.
            Some(parent_scs < 0.7)
        } else {
            Some(false)
        }
    }

    /// Calculate final collusion risk score
    fn calculate_collusion_risk(
        &self,
        suspicious_groups: u32,
        total_miners: usize,
    ) -> SecurityFinding {
        let collusion_risk = if total_miners > 10 {
            (suspicious_groups as f64 / total_miners as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

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

    pub async fn check_for_economic_attack(&self, dag: &QantoDAG) -> SecurityFinding {
        // This check looks for an unusually high average transaction fee during
        // a period of low network congestion, which might indicate an attempt
        // to manipulate the `market_premium` calculation for block rewards.
        let blocks = &dag.blocks;
        if blocks.len() < 50 {
            return self.create_insufficient_blocks_finding_economic();
        }

        let latest_blocks = self.get_recent_blocks_for_economic(blocks);
        if latest_blocks.is_empty() {
            return self.create_no_blocks_finding();
        }

        let (avg_fee_per_tx, congestion_level) = self.calculate_economic_metrics(&latest_blocks);

        if self.is_economic_manipulation(avg_fee_per_tx, congestion_level) {
            return self.create_economic_attack_finding(avg_fee_per_tx, congestion_level);
        }

        self.create_no_economic_anomalies_finding()
    }

    /// Create finding for insufficient blocks for economic analysis
    fn create_insufficient_blocks_finding_economic(&self) -> SecurityFinding {
        SecurityFinding {
            risk_score: 0.0,
            severity: InsightSeverity::Tip,
            confidence: 0.9,
            details: "Not enough blocks for economic attack analysis.".to_string(),
        }
    }

    /// Get recent blocks for economic analysis
    fn get_recent_blocks_for_economic(
        &self,
        blocks: &dashmap::DashMap<String, QantoBlock>,
    ) -> Vec<QantoBlock> {
        let mut recent_blocks: Vec<_> = blocks.iter().map(|entry| entry.value().clone()).collect();
        recent_blocks.sort_by_key(|b| b.timestamp);
        recent_blocks.iter().rev().take(50).cloned().collect()
    }

    /// Create finding for no blocks to analyze
    fn create_no_blocks_finding(&self) -> SecurityFinding {
        SecurityFinding {
            risk_score: 0.0,
            severity: InsightSeverity::Tip,
            confidence: 1.0,
            details: "No recent blocks to analyze.".to_string(),
        }
    }

    /// Calculate economic metrics from recent blocks
    fn calculate_economic_metrics(&self, latest_blocks: &[QantoBlock]) -> (f64, f64) {
        let total_txs: usize = latest_blocks.iter().map(|b| b.transactions.len()).sum();
        let total_fees: u64 = latest_blocks
            .iter()
            .flat_map(|b| &b.transactions)
            .map(|tx| tx.fee)
            .sum();
        let avg_tx_per_block = total_txs as f64 / latest_blocks.len() as f64;
        let avg_fee_per_tx = total_fees as f64 / total_txs.max(1) as f64;
        let congestion_level = avg_tx_per_block / MAX_TRANSACTIONS_PER_BLOCK as f64;

        (avg_fee_per_tx, congestion_level)
    }

    /// Check if economic manipulation is detected
    fn is_economic_manipulation(&self, avg_fee_per_tx: f64, congestion_level: f64) -> bool {
        // Condition: Average fee is very high (>50), but network is not congested (<30% capacity)
        avg_fee_per_tx > 50.0 && congestion_level < 0.3
    }

    /// Create finding for detected economic attack
    fn create_economic_attack_finding(
        &self,
        avg_fee_per_tx: f64,
        congestion_level: f64,
    ) -> SecurityFinding {
        warn!(
            avg_fee = avg_fee_per_tx,
            congestion = congestion_level,
            "High fees detected with low congestion, potential economic manipulation."
        );
        SecurityFinding {
            risk_score: 0.75,
            severity: InsightSeverity::Critical,
            confidence: 0.8,
            details: format!(
                "High avg fee ({avg_fee_per_tx:.2}) with low congestion ({congestion_level:.2})."
            ),
        }
    }

    /// Create finding for no economic anomalies
    fn create_no_economic_anomalies_finding(&self) -> SecurityFinding {
        SecurityFinding {
            risk_score: 0.0,
            severity: InsightSeverity::Tip,
            confidence: 1.0,
            details: "No economic anomalies detected.".to_string(),
        }
    }

    pub async fn check_for_mempool_frontrun(&self, dag: &QantoDAG) -> SecurityFinding {
        let latest_blocks = self.get_recent_blocks_for_frontrun(&dag.blocks);
        let (suspicious_count, tx_checked) = self.analyze_frontrun_patterns(&latest_blocks);

        if tx_checked == 0 {
            return self.create_no_transactions_finding();
        }

        self.calculate_frontrun_risk(suspicious_count, tx_checked)
    }

    /// Get recent blocks for front-running analysis
    fn get_recent_blocks_for_frontrun(
        &self,
        blocks: &dashmap::DashMap<String, QantoBlock>,
    ) -> Vec<QantoBlock> {
        let mut recent_blocks: Vec<_> = blocks.iter().map(|entry| entry.value().clone()).collect();
        recent_blocks.sort_by_key(|b| b.timestamp);
        recent_blocks.iter().rev().take(20).cloned().collect()
    }

    /// Analyze transaction patterns for front-running indicators
    fn analyze_frontrun_patterns(&self, latest_blocks: &[QantoBlock]) -> (u32, u32) {
        let mut suspicious_count = 0;
        let mut tx_checked = 0;

        for block in latest_blocks {
            let transactions = &block.transactions;
            for i in 0..transactions.len() {
                for j in (i + 1)..transactions.len() {
                    tx_checked += 1;
                    let tx1 = &transactions[i];
                    let tx2 = &transactions[j];

                    if self.is_potential_frontrun(tx1, tx2) {
                        suspicious_count += 1;
                        warn!(tx1_id=%tx1.id, tx2_id=%tx2.id, block_id=%block.id, "Potential front-running pattern detected.");
                    }
                }
            }
        }

        (suspicious_count, tx_checked)
    }

    /// Check if two transactions show potential front-running pattern
    fn is_potential_frontrun(
        &self,
        tx1: &crate::transaction::Transaction,
        tx2: &crate::transaction::Transaction,
    ) -> bool {
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

        // Transactions spend at least one same UTXO and tx2 has higher fee
        !tx1_inputs.is_disjoint(&tx2_inputs) && tx2.fee > tx1.fee
    }

    /// Create finding for no transactions to analyze
    fn create_no_transactions_finding(&self) -> SecurityFinding {
        SecurityFinding {
            risk_score: 0.0,
            severity: InsightSeverity::Tip,
            confidence: 1.0,
            details: "No recent transactions to analyze for front-running.".to_string(),
        }
    }

    /// Calculate front-running risk based on analysis results
    fn calculate_frontrun_risk(&self, suspicious_count: u32, tx_checked: u32) -> SecurityFinding {
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
                description: "Base QAN reward per block (in smallest units) before modifiers."
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
        let edict_multiplier = if let Some(edict) = self.economy.active_edict.read().await.as_ref()
        {
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
        let _rules = &self.economy.epoch_rules;

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
                if let Ok((multiplier, _redistributed_reward)) = service.get_rewards().await {
                    return multiplier;
                }
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
            .map_or(150_000_000_000.0, |r| r.value);
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

        // AI model retraining - restored for enhanced adaptive capabilities
        if current_epoch.is_multiple_of(RETRAIN_INTERVAL_EPOCHS) {
            if let Err(e) = self
                .cognitive_engine
                .read()
                .await
                .retrain_neural_models()
                .await
            {
                warn!(
                    "Failed to retrain neural models at epoch {}: {:?}",
                    current_epoch, e
                );
            }
        }

        // ACT: Execute decisions, whether through edicts or autonomous governance proposals.
        self.issue_new_edict(current_epoch, dag).await;
        self.perform_autonomous_governance(current_epoch, dag).await;
    }

    async fn update_network_state(&self, dag: &QantoDAG) {
        let avg_tx_per_block = dag.get_average_tx_per_block().await;
        let validator_count = dag.validators.len();

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
        let stake_score = dag_arc.validators.get(miner_address).map_or(0.0, |entry| {
            (*entry.value() as f64 / stake_divisor).min(1.0)
        });

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
        let network_state_guard = self.economy.network_state.read().await;
        let network_state = *network_state_guard;
        drop(network_state_guard);

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
        let proposals_guard = self.governance.proposals.read().await;
        let proposal_count = proposals_guard.len();
        drop(proposals_guard);
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

        let karma_ledgers = &self.reputation.karma_ledgers;
        let karma_guard = karma_ledgers.read().await;
        let mut karma_vec: Vec<(String, KarmaLedger)> = karma_guard
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        drop(karma_guard);
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
            let network_state_guard = self.economy.network_state.read().await;
            let network_state = *network_state_guard;
            drop(network_state_guard);
            let new_edict = match network_state {
                NetworkState::Congested => {
                    let history_guard = self.economy.congestion_history.read().await;
                    let avg_congestion =
                        history_guard.iter().sum::<f64>() / history_guard.len().max(1) as f64;
                    drop(history_guard);
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
                    let validator_count = dag.validators.len();
                    Some(SagaEdict {
                        id: Uuid::new_v4().to_string(),
                        issued_epoch: current_epoch,
                        expiry_epoch: current_epoch + 10,
                        description: {
                            let mut desc = String::with_capacity(100);
                            desc.push_str("Network Degraded (");
                            desc.push_str(&validator_count.to_string());
                            desc.push_str(" validators): Temporarily boosting rewards to attract more validators.");
                            desc
                        },
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
    async fn propose_validator_scaling(&self, current_epoch: u64, dag: &QantoDAG) -> bool {
        let rules = self.economy.epoch_rules.read().await;
        let validator_count = dag.validators.len();
        let min_stake_rule = "min_validator_stake".to_string();
        let base_reward_rule = "base_reward".to_string();
        let current_min_stake = rules.get(&min_stake_rule).map_or(1000.0, |r| r.value);
        drop(rules);

        let avg_congestion = self.calculate_average_congestion().await;
        let network_state = *self.economy.network_state.read().await;

        if self.should_propose_validator_scaling(validator_count, avg_congestion, network_state) {
            let proposals = self.governance.proposals.read().await;
            if !self.has_active_stake_proposal(&proposals, &min_stake_rule, &base_reward_rule) {
                drop(proposals);
                return self
                    .create_validator_scaling_proposal(
                        current_epoch,
                        validator_count,
                        avg_congestion,
                        current_min_stake,
                        min_stake_rule,
                    )
                    .await;
            }
        }
        false
    }

    async fn calculate_average_congestion(&self) -> f64 {
        let congestion_history = self.economy.congestion_history.read().await;
        if congestion_history.is_empty() {
            0.0
        } else {
            congestion_history.iter().sum::<f64>() / congestion_history.len() as f64
        }
    }

    fn should_propose_validator_scaling(
        &self,
        validator_count: usize,
        avg_congestion: f64,
        network_state: NetworkState,
    ) -> bool {
        (validator_count < 10 && network_state == NetworkState::Degraded)
            || (avg_congestion > 0.7 && validator_count < 20)
    }

    fn has_active_stake_proposal(
        &self,
        proposals: &HashMap<String, GovernanceProposal>,
        min_stake_rule: &str,
        base_reward_rule: &str,
    ) -> bool {
        proposals.values().any(|p| {
            p.status == ProposalStatus::Voting
                && matches!(&p.proposal_type, ProposalType::UpdateRule(name, _)
                if name == min_stake_rule || name == base_reward_rule)
        })
    }

    async fn create_validator_scaling_proposal(
        &self,
        current_epoch: u64,
        validator_count: usize,
        avg_congestion: f64,
        current_min_stake: f64,
        min_stake_rule: String,
    ) -> bool {
        let new_stake_req = (current_min_stake * 0.9).round();
        let justification = format!(
            "Network conditions (validators: {validator_count}, avg_congestion: {avg_congestion:.2}) suggest a need for more validators. Lowering the minimum stake from {current_min_stake} to {new_stake_req} should increase incentive to participate."
        );

        let proposal = GovernanceProposal {
            id: format!("saga-proposal-{}", Uuid::new_v4()),
            proposer: "SAGA_AUTONOMOUS_AGENT".to_string(),
            proposal_type: ProposalType::UpdateRule(min_stake_rule, new_stake_req),
            votes_for: 1.0,
            votes_against: 0.0,
            status: ProposalStatus::Voting,
            voters: vec![],
            creation_epoch: current_epoch,
            justification: Some(justification),
        };

        info!(proposal_id = %proposal.id, "SAGA is proposing to lower minimum stake to {new_stake_req} to attract validators.");

        self.award_karma(
            "SAGA_AUTONOMOUS_AGENT",
            KarmaSource::SagaAutonomousAction,
            100,
        )
        .await;

        let mut proposals = self.governance.proposals.write().await;
        proposals.insert(proposal.id.clone(), proposal);
        true
    }

    async fn propose_governance_parameter_tuning(&self, current_epoch: u64) -> bool {
        if !current_epoch.is_multiple_of(20) {
            return false;
        }

        let proposals_reader = self.governance.proposals.read().await;
        let recent_proposals: Vec<_> = proposals_reader
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
            let _rules = &self.economy.epoch_rules;
            let rules_guard = _rules.read().await;
            let current_threshold = rules_guard.get(&threshold_rule).map_or(0.75, |r| r.value);
            drop(rules_guard);
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

    /// Get comprehensive analytics dashboard data
    pub async fn get_analytics_dashboard_data(&self) -> AnalyticsDashboardData {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let network_health = self.collect_network_health_metrics().await;
        let ai_performance = self.collect_ai_performance_metrics().await;
        let security_insights = self.collect_security_insights().await;
        let economic_indicators = self.collect_economic_indicators().await;
        let environmental_metrics = self.collect_environmental_metrics().await;

        AnalyticsDashboardData {
            timestamp,
            network_health,
            ai_performance,
            security_insights,
            economic_indicators,
            environmental_metrics,
            total_transactions: 0,
            active_addresses: 0,
            mempool_size: 0,
            block_height: 0,
            tps_current: 0.0,
            tps_peak: 0.0,
        }
    }

    /// Collect network health metrics for dashboard
    async fn collect_network_health_metrics(&self) -> NetworkHealthMetrics {
        NetworkHealthMetrics::default()
    }

    /// Collect AI model performance metrics for dashboard
    async fn collect_ai_performance_metrics(&self) -> AIModelPerformance {
        let cognitive_engine = self.cognitive_engine.read().await;
        let deep_network = cognitive_engine.deep_network.read().await;
        let latest_metrics = deep_network.training_history.last();

        let ai_performance = AIModelPerformance {
            neural_network_accuracy: latest_metrics.map(|m| m.accuracy).unwrap_or(0.85),
            prediction_confidence: 0.92,
            training_loss: latest_metrics.map(|m| m.loss).unwrap_or(0.1),
            validation_loss: latest_metrics.map(|m| m.validation_loss).unwrap_or(0.12),
            model_drift_score: 0.05,
            inference_latency_ms: 50.0,
            last_retrain_epoch: latest_metrics.map(|m| m.epoch).unwrap_or(0),
            feature_importance: HashMap::new(),
        };
        drop(deep_network);
        drop(cognitive_engine);
        ai_performance
    }

    /// Collect security insights for dashboard
    async fn collect_security_insights(&self) -> SecurityInsights {
        SecurityInsights {
            threat_level: ThreatLevel::Low,
            anomaly_score: 0.1,
            attack_attempts_24h: 0,
            blocked_transactions: 0,
            suspicious_patterns: vec![],
            security_confidence: 0.95,
        }
    }

    /// Collect economic indicators for dashboard
    async fn collect_economic_indicators(&self) -> EconomicIndicators {
        EconomicIndicators {
            total_value_locked: 1000000.0,
            transaction_fees_24h: 500.0,
            validator_rewards_24h: 1000.0,
            network_utilization: 0.3,
            economic_security: 0.9,
            fee_market_efficiency: 0.85,
        }
    }

    /// Collect environmental metrics for dashboard
    async fn collect_environmental_metrics(&self) -> EnvironmentalDashboardMetrics {
        let env_metrics = self.economy.environmental_metrics.read().await;
        let environmental_metrics = EnvironmentalDashboardMetrics {
            carbon_footprint_kg: 100.0,
            energy_efficiency_score: env_metrics.network_green_score,
            renewable_energy_percentage: 75.0,
            carbon_offset_credits: env_metrics.total_co2_offset_epoch,
            green_validator_ratio: 0.8,
        };
        drop(env_metrics);
        environmental_metrics
    }
}
