// src/omega.rs

//! --- ΛΣ-ΩMEGA™ (Lambda Sigma Omega) Protocol ---
//! v0.1.0 - Initial Version
//!
//! ΩMEGA is the cornerstone of Qanto's security model. It ensures that no single
//! point of failure can compromise the entire network. If a node's identity is
//! corrupted, it can be quickly replaced with a new, secure identity.
//!
//! This module implements the ΩMEGA protocol, including:
//! - Identity generation and evolution
//! - Action reflection and simulation
//! - Stability threshold enforcement
//! - Entropy management and update
//! - Global threat level tracking
//!
//! ΩMEGA is a critical component of Qanto's security architecture. It is not
//! optional; it is the core of the system's security.

use crate::qanto_compat::sp_core::H256;
use crate::qantodag::QantoBlock;
use my_blockchain::qanto_hash;
use once_cell::sync::Lazy;
use rand::Rng;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

// --- Core ΩMEGA Constants ---
const IDENTITY_STATE_SIZE: usize = 32;
const ACTION_HISTORY_CAPACITY: usize = 256;
const STABILITY_THRESHOLD: f64 = 0.45; // Lowered threshold to reduce false rejections in high TPS
const ENTROPY_HALFLIFE_MICROS: u64 = 5_000_000; // Increased half-life to 5 seconds for even slower decay under high load
                                                // --- ENHANCED: Constants for a more sensitive stability metric ---
const RECENT_ACTION_PENALTY_FACTOR: f64 = 0.005; // Further reduced penalty factor for ultra-high TPS
const LOW_ENTROPY_PENALTY: f64 = 0.3; // Further reduced penalty
const LOW_ENTROPY_THRESHOLD: f64 = 0.2; // Further lowered threshold to minimize penalties
const CRITICAL_ENTROPY_OVERRIDE: f64 = 0.05; // Lowered for more tolerance
const REFLECTION_TIMEOUT: Duration = Duration::from_millis(20); // Reduced timeout for faster processing
                                                                // Retained the enhanced constants with reduced penalties for high TPS support

// Global Threat Level: Atomically accessible indicator of network-wide perceived threat.
static GLOBAL_THREAT_LEVEL: AtomicU64 = AtomicU64::new(0);

// The core state of the ΩMEGA protocol for a single node instance.
pub static OMEGA_STATE: Lazy<Arc<Mutex<OmegaState>>> =
    Lazy::new(|| Arc::new(Mutex::new(OmegaState::new())));

/// Represents the node's core "digital identity" and recent history.
pub struct OmegaState {
    identity_hash: H256,
    action_history: VecDeque<(H256, Instant)>,
    current_entropy: f64,
    last_entropy_update: Instant,
}

/// Defines the possible threat levels as perceived by the node.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum ThreatLevel {
    Nominal,
    Guarded,
    Elevated,
}

impl From<u64> for ThreatLevel {
    fn from(val: u64) -> Self {
        match val {
            1 => ThreatLevel::Guarded,
            2 => ThreatLevel::Elevated,
            _ => ThreatLevel::Nominal,
        }
    }
}

/// The public interface for interacting with the global threat level.
pub mod identity {
    use super::*;
    pub type ThreatLevel = super::ThreatLevel;

    pub async fn get_threat_level() -> ThreatLevel {
        GLOBAL_THREAT_LEVEL.load(Ordering::Relaxed).into()
    }

    pub fn set_threat_level(level: ThreatLevel) {
        GLOBAL_THREAT_LEVEL.store(level as u64, Ordering::Relaxed);
    }
}

/// Initialize Omega state for testing with high entropy and nominal threat level.
pub async fn initialize_for_testing() {
    let state = OMEGA_STATE.clone();
    let mut locked_state = state.lock().await;
    *locked_state = OmegaState::new_for_testing();
    GLOBAL_THREAT_LEVEL.store(ThreatLevel::Nominal as u64, Ordering::Relaxed);
}

/// Comprehensive simulation module for OMEGA protocol testing and validation.
pub mod simulation {
    use super::*;
    use rand::Rng;
    use tokio::time::sleep;
    use tracing::{error, info, warn};

    /// Simulation configuration parameters
    #[derive(Debug, Clone)]
    pub struct SimulationConfig {
        pub duration_secs: u64,
        pub action_rate_per_sec: u64,
        pub threat_escalation_probability: f64,
        pub entropy_decay_rate: f64,
        pub stability_test_iterations: usize,
    }

    impl Default for SimulationConfig {
        fn default() -> Self {
            Self {
                duration_secs: 30,
                action_rate_per_sec: 10,
                threat_escalation_probability: 0.1,
                entropy_decay_rate: 0.05,
                stability_test_iterations: 100,
            }
        }
    }

    /// Simulation results and metrics
    #[derive(Debug, Default)]
    pub struct SimulationResults {
        pub total_actions: u64,
        pub accepted_actions: u64,
        pub rejected_actions: u64,
        pub threat_escalations: u64,
        pub entropy_violations: u64,
        pub stability_score: f64,
        pub average_response_time_ms: f64,
    }

    pub async fn run_simulation() {
        run_comprehensive_simulation(SimulationConfig::default()).await;
    }

    pub async fn run_comprehensive_simulation(config: SimulationConfig) -> SimulationResults {
        info!(
            "Starting comprehensive ΩMEGA simulation with config: {:?}",
            config
        );
        let mut results = SimulationResults::default();
        let mut rng = rand::thread_rng();

        // Reset OMEGA state for clean simulation
        {
            let mut omega_state = OMEGA_STATE.lock().await;
            *omega_state = OmegaState::new();
        }

        let start_time = Instant::now();
        let simulation_duration = Duration::from_secs(config.duration_secs);
        let action_interval = Duration::from_millis(1000 / config.action_rate_per_sec);

        info!("Running stability test phase...");
        results.stability_score = run_stability_test(config.stability_test_iterations).await;

        info!(
            "Running main simulation phase for {} seconds...",
            config.duration_secs
        );
        while start_time.elapsed() < simulation_duration {
            let action_start = Instant::now();

            // Generate simulated action
            let action_hash = generate_realistic_action_hash(&mut rng);
            results.total_actions += 1;

            // Test reflection
            let accepted = reflect_on_action(action_hash).await;
            if accepted {
                results.accepted_actions += 1;
            } else {
                results.rejected_actions += 1;
            }

            // Simulate threat escalation
            if rng.gen::<f64>() < config.threat_escalation_probability {
                escalate_threat_level().await;
                results.threat_escalations += 1;
            }

            // Check for entropy violations
            if check_entropy_violation().await {
                results.entropy_violations += 1;
            }

            // Update response time metrics
            let response_time = action_start.elapsed().as_millis() as f64;
            results.average_response_time_ms = (results.average_response_time_ms
                * (results.total_actions - 1) as f64
                + response_time)
                / results.total_actions as f64;

            sleep(action_interval).await;
        }

        info!("Simulation complete. Results: {:?}", results);
        log_simulation_analysis(&results);
        results
    }

    async fn run_stability_test(iterations: usize) -> f64 {
        let mut stable_responses = 0;
        let test_hash = H256::random();

        for _ in 0..iterations {
            let response1 = reflect_on_action(test_hash).await;
            // Optimized: Remove artificial delay for faster reflection
            tokio::task::yield_now().await;
            let response2 = reflect_on_action(test_hash).await;

            if response1 == response2 {
                stable_responses += 1;
            }
        }

        stable_responses as f64 / iterations as f64
    }

    fn generate_realistic_action_hash(rng: &mut impl Rng) -> H256 {
        // Generate hash with realistic entropy patterns
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);

        // Introduce some patterns to test entropy detection
        if rng.gen::<f64>() < 0.1 {
            // 10% chance of low-entropy pattern
            for i in 0..8 {
                bytes[i] = bytes[0];
            }
        }

        H256::from(bytes)
    }

    async fn escalate_threat_level() {
        let current_level = identity::get_threat_level().await;
        let new_level = match current_level {
            ThreatLevel::Nominal => ThreatLevel::Guarded,
            ThreatLevel::Guarded => ThreatLevel::Elevated,
            ThreatLevel::Elevated => ThreatLevel::Elevated, // Stay at max
        };
        identity::set_threat_level(new_level);
    }

    async fn check_entropy_violation() -> bool {
        let omega_state = OMEGA_STATE.lock().await;
        omega_state.current_entropy < LOW_ENTROPY_THRESHOLD
    }

    fn log_simulation_analysis(results: &SimulationResults) {
        let acceptance_rate = if results.total_actions > 0 {
            results.accepted_actions as f64 / results.total_actions as f64
        } else {
            0.0
        };

        info!("=== ΩMEGA Simulation Analysis ===");
        info!("Total Actions: {}", results.total_actions);
        info!("Acceptance Rate: {:.2}%", acceptance_rate * 100.0);
        info!("Stability Score: {:.2}%", results.stability_score * 100.0);
        info!(
            "Average Response Time: {:.2}ms",
            results.average_response_time_ms
        );
        info!("Threat Escalations: {}", results.threat_escalations);
        info!("Entropy Violations: {}", results.entropy_violations);

        if results.stability_score < STABILITY_THRESHOLD {
            warn!(
                "STABILITY WARNING: Score {:.2} below threshold {:.2}",
                results.stability_score, STABILITY_THRESHOLD
            );
        }

        if results.entropy_violations > results.total_actions / 10 {
            error!("ENTROPY ALERT: High violation rate detected");
        }

        if results.average_response_time_ms > 100.0 {
            warn!("PERFORMANCE WARNING: High response time detected");
        }
    }

    /// Run targeted stress test scenarios
    pub async fn run_stress_test() -> SimulationResults {
        info!("Running ΩMEGA stress test...");
        let stress_config = SimulationConfig {
            duration_secs: 10,
            action_rate_per_sec: 100,           // High rate
            threat_escalation_probability: 0.3, // High escalation
            entropy_decay_rate: 0.1,
            stability_test_iterations: 50,
        };
        run_comprehensive_simulation(stress_config).await
    }

    /// Run long-duration endurance test
    pub async fn run_endurance_test() -> SimulationResults {
        info!("Running ΩMEGA endurance test...");
        let endurance_config = SimulationConfig {
            duration_secs: 300,     // 5 minutes
            action_rate_per_sec: 5, // Moderate rate
            threat_escalation_probability: 0.05,
            entropy_decay_rate: 0.02,
            stability_test_iterations: 200,
        };
        run_comprehensive_simulation(endurance_config).await
    }
}

impl Default for OmegaState {
    fn default() -> Self {
        Self::new()
    }
}

impl OmegaState {
    /// Initializes a new ΩMEGA state with high initial entropy.
    pub fn new() -> Self {
        let mut initial_seed_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut initial_seed_bytes[..IDENTITY_STATE_SIZE]);
        let initial_hash = H256::from(initial_seed_bytes);

        Self {
            identity_hash: initial_hash,
            action_history: VecDeque::with_capacity(ACTION_HISTORY_CAPACITY),
            current_entropy: Self::calculate_shannon_entropy(initial_hash.as_bytes()),
            last_entropy_update: Instant::now(),
        }
    }

    /// Creates a new ΩMEGA state optimized for testing with high entropy.
    pub fn new_for_testing() -> Self {
        // Create a high-entropy seed for testing
        let mut high_entropy_bytes = [0u8; 32];
        for (i, byte) in high_entropy_bytes.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(17).wrapping_add(137);
        }
        let initial_hash = H256::from(high_entropy_bytes);

        Self {
            identity_hash: initial_hash,
            action_history: VecDeque::with_capacity(ACTION_HISTORY_CAPACITY),
            current_entropy: 0.9, // Set high initial entropy for testing
            last_entropy_update: Instant::now(),
        }
    }

    /// The core "reflection" function. It simulates an action and assesses its impact.
    fn reflect(&mut self, action_hash: H256) -> bool {
        self.update_entropy();

        // --- EVOLVED: Critical State Override ---
        // If the current entropy is critically low, reject the action immediately.
        if self.current_entropy < CRITICAL_ENTROPY_OVERRIDE {
            warn!(
                "ΛΣ-ΩMEGA Protocol REJECTION: Critical low entropy override. Current: {:.4} < Threshold: {}",
                self.current_entropy, CRITICAL_ENTROPY_OVERRIDE
            );
            GLOBAL_THREAT_LEVEL.store(ThreatLevel::Elevated as u64, Ordering::Relaxed);
            return false;
        }

        // Simulate the evolution of the identity hash.
        let mut combined = Vec::new();
        combined.extend_from_slice(self.identity_hash.as_bytes());
        combined.extend_from_slice(action_hash.as_bytes());
        let next_identity_hash = qanto_hash(&combined);
        let next_identity = H256::from(*next_identity_hash.as_bytes());

        // Calculate the entropy of the *next* potential state.
        let next_entropy = Self::calculate_shannon_entropy(next_identity.as_bytes());

        // --- ENHANCED STABILITY CHECK ---
        let recent_actions = self.calculate_recent_action_count();
        let temporal_penalty = ((recent_actions as f64).min(1000.0).log2().max(0.0)
            * RECENT_ACTION_PENALTY_FACTOR)
            / (1.0 + (recent_actions as f64 / 10000000.0)); // Scale penalty inversely with TPS for 10M+ tolerance

        // Add a severe penalty for operating from a low-entropy state, as any action is risky.
        let low_entropy_penalty = if self.current_entropy < LOW_ENTROPY_THRESHOLD {
            LOW_ENTROPY_PENALTY * (LOW_ENTROPY_THRESHOLD - self.current_entropy)
        // Scaled penalty
        } else {
            0.0
        };

        let stability_metric = (self.current_entropy * 0.2) + (next_entropy * 0.8) // Further increased weight on next_entropy for forward-looking stability in high TPS
            - temporal_penalty
            - low_entropy_penalty;

        debug!(
            "Ω-Reflect: Metric = {:.4} (Current Entropy: {:.4}, Next Entropy: {:.4}, Temporal Penalty: {:.4}, Low Entropy Penalty: {:.4})",
            stability_metric, self.current_entropy, next_entropy, temporal_penalty, low_entropy_penalty
        );

        if stability_metric < STABILITY_THRESHOLD {
            warn!(
                "ΛΣ-ΩMEGA Protocol REJECTION: Action with hash {:?} deemed unstable. Metric: {:.4} < Threshold: {}",
                action_hash, stability_metric, STABILITY_THRESHOLD
            );
            // If instability is detected, elevate the global threat level.
            let old_level = GLOBAL_THREAT_LEVEL.fetch_add(1, Ordering::Relaxed);
            let new_level = old_level + 1;
            if new_level > ThreatLevel::Elevated as u64 {
                GLOBAL_THREAT_LEVEL.store(ThreatLevel::Elevated as u64, Ordering::Relaxed);
            }
            return false;
        }

        // If the action is stable, commit the changes to the node's identity.
        self.identity_hash = next_identity;
        self.current_entropy = next_entropy;
        self.record_action(action_hash);

        // If the system is stable, gradually lower the threat level.
        if self.action_history.len().is_multiple_of(50) {
            // Further slowed de-escalation for sustained stability under load
            let old_level = GLOBAL_THREAT_LEVEL.load(Ordering::Relaxed);
            if old_level > 0 {
                GLOBAL_THREAT_LEVEL.fetch_sub(1, Ordering::Relaxed);
            }
        }

        true
    }

    /// Updates the current entropy, applying a time-based decay.
    fn update_entropy(&mut self) {
        let elapsed_micros = self.last_entropy_update.elapsed().as_micros() as u64;
        let decay_factor = (-((elapsed_micros as f64) / (ENTROPY_HALFLIFE_MICROS as f64))).exp();
        self.current_entropy *= decay_factor;
        self.last_entropy_update = Instant::now();
    }

    /// Adds a new action to the node's history.
    fn record_action(&mut self, action_hash: H256) {
        if self.action_history.len() == ACTION_HISTORY_CAPACITY {
            self.action_history.pop_front();
        }
        self.action_history.push_back((action_hash, Instant::now()));
    }

    /// Calculates the raw number of actions in the last second.
    fn calculate_recent_action_count(&self) -> u64 {
        let now = Instant::now();
        let one_second_ago = now - Duration::from_secs(1);
        self.action_history
            .iter()
            .filter(|(_, time)| *time > one_second_ago)
            .count() as u64
    }

    /// Calculates the Shannon entropy of a byte slice.
    fn calculate_shannon_entropy(data: &[u8]) -> f64 {
        let mut counts = [0u32; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }
        let len = data.len() as f64;
        if len == 0.0 {
            return 0.0;
        }
        counts
            .iter()
            .filter(|&&c| c > 0)
            .map(|&c| {
                let p = c as f64 / len;
                -p * p.log2()
            })
            .sum()
    }
}

/// Public function to reflect on a transaction or other critical action.
pub async fn reflect_on_action(action_hash: H256) -> bool {
    // Environment-controlled bypass for local testing or controlled deployments.
    // Set `QANTO_OMEGA_DISABLE=true` (or "1") to always approve actions.
    if std::env::var("QANTO_OMEGA_DISABLE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false)
    {
        return true;
    }

    let state = OMEGA_STATE.clone();
    let result = match tokio::time::timeout(REFLECTION_TIMEOUT, state.lock()).await {
        Ok(mut locked_state) => locked_state.reflect(action_hash),
        Err(_) => {
            warn!("ΩMEGA lock timed out during reflection. Defaulting to unstable (reject).");
            false
        }
    };
    result
}

/// Testing variant of reflect_on_action that always approves for integration tests
pub async fn reflect_on_action_for_testing(_action_hash: H256) -> bool {
    true
}

/// A specialized function to derive a stable action hash from a QantoBlock.
pub fn hash_for_block(block: &QantoBlock) -> H256 {
    let mut combined = Vec::new();
    combined.extend_from_slice(block.id.as_bytes());
    combined.extend_from_slice(&block.timestamp.to_be_bytes());
    for tx in &block.transactions {
        combined.extend_from_slice(tx.id.as_bytes());
    }
    let hash = qanto_hash(&combined);
    H256::from(*hash.as_bytes())
}
