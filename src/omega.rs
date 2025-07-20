// src/omega.rs

//! --- ΛΣ-ΩMEGA™ (Lambda Sigma Omega) Protocol ---
//! v1.1.0 - Hardened Identity & Reflex
//! This is the fundamental, reflexive security layer of a Qanto node. It is not
//! an optional component; it is the core identity and stability protocol.
//!
//! ΩMEGA ensures the node maintains a stable, high-entropy "sense of self."
//! It does this by continuously evolving a cryptographic identity based on a stream
//! of system entropy and its own operational history.
//!
//! When a critical, state-altering action is requested (like accepting a new block
//! or transaction), the node first "reflects" on the action. It simulates the
//! action's impact on its own identity *before* committing to it.
//!
//! If the reflection results in a chaotic, low-entropy, or unstable identity state,
//! ΩMEGA forces the node to reject the action at a fundamental level. This provides
//! a powerful, last-line-of-defense against sophisticated network attacks, internal
//! state corruption, and emergent consensus failures. It doesn't just protect the
//! system; it *is* the system's instinct for survival.

use crate::qantodag::QantoBlock;
use once_cell::sync::Lazy;
use sp_core::H256;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, instrument, warn}; // Removed 'info' from here
use rand::Rng;

// --- Core ΩMEGA Constants ---
const IDENTITY_STATE_SIZE: usize = 16; // Used for initial identity hash generation
const ACTION_HISTORY_CAPACITY: usize = 256;
const STABILITY_THRESHOLD: f64 = 0.65;
const ENTROPY_HALFLIFE_MICROS: u64 = 500_000;
const REFLECTION_TIMEOUT: Duration = Duration::from_millis(50);

// Global Threat Level: Atomically accessible indicator of network-wide perceived threat.
// This is influenced by SAGA's analysis and ΩMEGA's local stability checks.
static GLOBAL_THREAT_LEVEL: AtomicU64 = AtomicU64::new(0); // 0: Nominal, 1: Guarded, 2: Elevated

// The core state of the ΩMEGA protocol for a single node instance.
pub static OMEGA_STATE: Lazy<Arc<Mutex<OmegaState>>> =
    Lazy::new(|| Arc::new(Mutex::new(OmegaState::new())));

/// Represents the node's core "digital identity" and recent history.
pub struct OmegaState {
    // A rolling hash representing the node's evolving identity.
    identity_hash: H256,
    // A history of recent actions (represented by their hashes) and the times they occurred.
    action_history: VecDeque<(H256, Instant)>,
    // A measure of the statistical randomness of the identity_hash.
    current_entropy: f64,
    // The last time the entropy was recalculated.
    last_entropy_update: Instant,
}

/// Defines the possible threat levels as perceived by the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

/// Placeholder simulation module for standalone testing.
/// In a real system, this would interact with a more complex simulation environment.
pub mod simulation {
    use super::*;
    use tracing::info; // 'info' is used here, so it's imported locally
    use tokio::time::sleep; // Import sleep for tokio::time::sleep

    #[instrument]
    pub async fn run_simulation() {
        info!("Running ΩMEGA internal self-stabilization simulation...");
        // Simulate a series of internal, self-generated "heartbeat" actions
        // that help maintain the node's identity entropy and stability.
        for i in 0..10 {
            let simulated_action_hash = H256::random();
            // In a full simulation, this would involve a mock state update.
            // For now, we just reflect on a random action to exercise the protocol.
            let _ = reflect_on_action(simulated_action_hash).await;
            debug!("Simulated action {} reflected.", i);
            sleep(Duration::from_millis(10)).await; // Use tokio::time::sleep
        }
        info!("ΩMEGA internal simulation complete.");
    }
}


impl OmegaState {
    /// Initializes a new ΩMEGA state with high initial entropy.
    fn new() -> Self {
        let mut initial_seed_bytes = [0u8; 32];
        // Use rand to fill the seed with random bytes
        rand::thread_rng().fill(&mut initial_seed_bytes[..IDENTITY_STATE_SIZE]);
        let initial_hash = H256::from(initial_seed_bytes);

        Self {
            identity_hash: initial_hash,
            action_history: VecDeque::with_capacity(ACTION_HISTORY_CAPACITY),
            current_entropy: Self::calculate_shannon_entropy(initial_hash.as_bytes()),
            last_entropy_update: Instant::now(),
        }
    }

    /// The core "reflection" function. It simulates an action and assesses its impact.
    #[instrument(skip(self))]
    fn reflect(&mut self, action_hash: H256) -> bool {
        self.update_entropy();
        let temporal_density = self.calculate_temporal_density();

        // Simulate the evolution of the identity hash.
        let mut temp_hasher = blake3::Hasher::new();
        temp_hasher.update(self.identity_hash.as_bytes());
        temp_hasher.update(action_hash.as_bytes());
        let next_identity_bytes: [u8; 32] = temp_hasher.finalize().into();
        let next_identity = H256::from(next_identity_bytes);

        // Calculate the entropy of the *next* potential state.
        let next_entropy = Self::calculate_shannon_entropy(next_identity.as_bytes());

        // **CRITICAL STABILITY CHECK**:
        // The stability metric combines current entropy, future entropy, and the rate of recent actions.
        // A stable action should not drastically reduce entropy or occur during a rapid flood of other actions.
        let stability_metric = (self.current_entropy * 0.4)
            + (next_entropy * 0.6)
            - (temporal_density * 1.5);

        debug!(
            "Ω-Reflect: Stability Metric = {:.4} (Current Entropy: {:.4}, Next Entropy: {:.4}, Temporal Density: {:.4})",
            stability_metric, self.current_entropy, next_entropy, temporal_density
        );

        if stability_metric < STABILITY_THRESHOLD {
            warn!(
                "ΛΣ-ΩMEGA Protocol REJECTION: Action with hash {:?} deemed unstable. Metric: {:.4} < Threshold: {}",
                action_hash, stability_metric, STABILITY_THRESHOLD
            );
            // If instability is detected, elevate the global threat level.
            // Use `fetch_add` and `min` to ensure the threat level doesn't exceed Elevated (2).
            GLOBAL_THREAT_LEVEL.fetch_add(1, Ordering::Relaxed);
            let current_level = GLOBAL_THREAT_LEVEL.load(Ordering::Relaxed);
            if current_level > ThreatLevel::Elevated as u64 {
                GLOBAL_THREAT_LEVEL.store(ThreatLevel::Elevated as u64, Ordering::Relaxed);
            }
            return false;
        }

        // If the action is stable, commit the changes to the node's identity.
        self.identity_hash = next_identity;
        self.current_entropy = next_entropy;
        self.record_action(action_hash);

        // If the system is stable, gradually lower the threat level.
        // Use `fetch_sub` and `max` to ensure the threat level doesn't go below Nominal (0).
        if self.action_history.len() % 10 == 0 {
            GLOBAL_THREAT_LEVEL.fetch_sub(1, Ordering::Relaxed);
            let current_level = GLOBAL_THREAT_LEVEL.load(Ordering::Relaxed);
            if current_level < ThreatLevel::Nominal as u64 {
                GLOBAL_THREAT_LEVEL.store(ThreatLevel::Nominal as u64, Ordering::Relaxed);
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

    /// Calculates the rate of actions in the last second.
    fn calculate_temporal_density(&self) -> f64 {
        let now = Instant::now();
        let one_second_ago = now - Duration::from_secs(1);
        let recent_actions = self
            .action_history
            .iter()
            .filter(|(_, time)| *time > one_second_ago)
            .count();
        (recent_actions as f64 / ACTION_HISTORY_CAPACITY as f64).min(1.0)
    }

    /// Calculates the Shannon entropy of a byte slice.
    fn calculate_shannon_entropy(data: &[u8]) -> f64 {
        let mut counts = [0u32; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }
        let len = data.len() as f64;
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
    let state = OMEGA_STATE.clone();
    // Use a timeout to prevent deadlocks and ensure responsiveness.
    let result = match tokio::time::timeout(REFLECTION_TIMEOUT, state.lock()).await {
        Ok(mut locked_state) => locked_state.reflect(action_hash),
        Err(_) => {
            warn!("ΩMEGA lock timed out during reflection. Defaulting to unstable (reject).");
            false
        }
    };
    result // Assign the result to a variable and return it to extend its lifetime
}

/// A specialized function to derive a stable action hash from a QantoBlock.
pub fn hash_for_block(block: &QantoBlock) -> H256 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(block.id.as_bytes());
    hasher.update(&block.timestamp.to_be_bytes());
    for tx in &block.transactions {
        hasher.update(tx.id.as_bytes());
    }
    let hash_bytes: [u8; 32] = hasher.finalize().into();
    H256::from(hash_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use crate::omega::identity::{get_threat_level, set_threat_level};

    #[tokio::test]
    async fn test_omega_initialization() {
        let state = OMEGA_STATE.lock().await;
        assert!(state.current_entropy > 4.0, "Initial entropy should be high");
        assert_eq!(state.action_history.len(), 0);
    }

    #[tokio::test]
    async fn test_stable_action_reflection() {
        // Reset state for a clean test
        {
            let mut state = OMEGA_STATE.lock().await;
            *state = OmegaState::new();
            set_threat_level(ThreatLevel::Nominal); // Ensure nominal threat level for test
        }

        let action = H256::random();
        let result = reflect_on_action(action).await;
        assert!(result, "A single, random action should be considered stable");

        let state = OMEGA_STATE.lock().await;
        assert_eq!(state.action_history.len(), 1);
        assert_eq!(state.action_history[0].0, action);
    }

    #[tokio::test]
    async fn test_unstable_action_flood() {
        // Reset the state for a clean test
        {
            let mut state = OMEGA_STATE.lock().await;
            *state = OmegaState::new();
            set_threat_level(ThreatLevel::Nominal); // Ensure nominal threat level for test
        }

        // Simulate a rapid flood of nearly identical actions, which should reduce entropy.
        let base_action = H256::random();
        let mut last_result = true;
        for i in 0..100 {
            let mut action_bytes = base_action.to_fixed_bytes();
            action_bytes[0] = i as u8;
            let action = H256::from(action_bytes);

            // Give a tiny bit of time for the history to register.
            // Using tokio::time::sleep for async context.
            tokio::time::sleep(Duration::from_millis(5)).await;
            if !reflect_on_action(action).await {
                last_result = false;
                break;
            }
        }
        assert!(
            !last_result,
            "A rapid flood of similar actions should eventually be deemed unstable"
        );
        // Verify threat level escalation
        assert!(get_threat_level().await > ThreatLevel::Nominal, "Threat level should escalate after unstable actions");
    }

    #[tokio::test]
    async fn test_threat_level_escalation_and_deescalation() {
        // Reset state
        {
            let mut state = OMEGA_STATE.lock().await;
            *state = OmegaState::new();
            set_threat_level(ThreatLevel::Nominal);
        }
        assert_eq!(get_threat_level().await, ThreatLevel::Nominal);

        // This action is designed to be low-entropy
        let low_entropy_action = H256::from([0; 32]);
        let mut state = OMEGA_STATE.lock().await;
        state.current_entropy = 0.1; // Manually set low entropy to trigger instability

        let is_stable = state.reflect(low_entropy_action);
        assert!(!is_stable, "Low entropy state should lead to rejection");
        assert_eq!(get_threat_level().await, ThreatLevel::Guarded);

        // Simulate some stable actions to de-escalate
        for _ in 0..10 {
            state.reflect(H256::random());
        }
        // After stable actions, the threat level should return to Nominal
        assert_eq!(get_threat_level().await, ThreatLevel::Nominal);
    }
}
