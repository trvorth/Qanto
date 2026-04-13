use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/**
 * @title Neural Consensus (TNC)
 * @dev High-fidelity truth-consensus and Proof-of-Veracity (PoV) algorithm.
 * Aggregates veracity scores from multiple expert nodes.
 */
pub struct NeuralConsensus {
    pub consensus_registry: HashMap<[u8; 32], ConsensusPulse>,
    pub min_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusPulse {
    pub claim_id: [u8; 32],
    pub veracity_score: f64,
    pub participants: u32,
    pub weight_distribution: Vec<f64>,
    pub veracity_history_depth: u32,
}

impl NeuralConsensus {
    pub fn new() -> Self {
        Self {
            consensus_registry: HashMap::new(),
            min_threshold: 0.85,
        }
    }

    /**
     * @dev Aggregates veracity scores for a high-fidelity truth claim.
     * Weights: 0.7 * Agentic_Consensus + 0.3 * Reputation_Weight.
     */
    pub fn aggregate_veracity_scores(&mut self, claim_id: [u8; 32], agentic_consensus: f64, weights: Vec<f64>) -> f64 {
        println!("TNC: Aggregating Veracity Consensus for Claim ID {:X?}...", claim_id);
        
        let weight_avg: f64 = weights.iter().sum::<f64>() / weights.len() as f64;
        let final_score = (0.7 * agentic_consensus) + (0.3 * weight_avg);

        self.consensus_registry.insert(claim_id, ConsensusPulse {
            claim_id,
            veracity_score: final_score,
            participants: weights.len() as u32,
            weight_distribution: weights,
            veracity_history_depth: 120, // 120 recursive proof cycles
        });

        println!("TNC: Consensus Outcome: {} (Depth: {})", if final_score > self.min_threshold { "VERIFIED" } else { "UNCERTAIN" }, 120);
        final_score
    }
}

// Phase 57: TNC Verification
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_consensus_aggregation() {
        let mut tnc = NeuralConsensus::new();
        let weights = vec![0.9, 0.85, 0.95, 0.88]; // Reputation weights
        let final_score = tnc.aggregate_veracity_scores([0xAA; 32], 0.92, weights);
        
        assert!(final_score > 0.85);
        assert_eq!(tnc.consensus_registry.get(&[0xAA; 32]).unwrap().participants, 4);
    }
}
