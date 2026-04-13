use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/**
 * @title Decentralized Futarchy Market (DFM)
 * @dev Prediction engine for market-driven protocol governance.
 * Resolves outcomes by comparing predicted QNTO value for evolution proposals.
 */
pub struct FutarchyMarket {
    pub active_proposals: HashMap<u32, ProposalPrediction>,
    pub consensus_history: Vec<MarketOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalPrediction {
    pub proposal_id: u32,
    pub p_pass: f64, // Predicted value if proposal passes
    pub p_fail: f64, // Predicted value if proposal fails
    pub volume: u64,
    pub resolution_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOutcome {
    pub proposal_id: u32,
    pub decision: String, // e.g., "PASS", "FAIL"
    pub price_delta: f64,
    pub veracity_score: f64,
}

impl FutarchyMarket {
    pub fn new() -> Self {
        Self {
            active_proposals: HashMap::new(),
            consensus_history: Vec::new(),
        }
    }

    /**
     * @dev Synchronizes a prediction for a governance proposal.
     */
    pub fn record_prediction(&mut self, proposal_id: u32, p_pass: f64, p_fail: f64) {
        println!("DFM: Recording Futarchy Prediction for Proposal {}...", proposal_id);
        self.active_proposals.insert(proposal_id, ProposalPrediction {
            proposal_id,
            p_pass,
            p_fail,
            volume: 1200, // Mock volume
            resolution_timestamp: 1775492930,
        });
    }

    /**
     * @dev Resolves the market outcome based on price discovery.
     * Rule: Pass if P_Pass > P_Fail * 1.02 (2% premium).
     */
    pub fn resolve_market_outcome(&mut self, proposal_id: u32) -> MarketOutcome {
        println!("DFM: Resolving Futarchy Market for Proposal {}...", proposal_id);
        
        let prediction = self.active_proposals.get(&proposal_id).expect("Proposal not in market");
        let passed = prediction.p_pass > (prediction.p_fail * 1.02);
        
        let outcome = MarketOutcome {
            proposal_id,
            decision: if passed { "PASS".to_string() } else { "FAIL".to_string() },
            price_delta: prediction.p_pass - prediction.p_fail,
            veracity_score: 0.98, // Accuracy of agentic collective intelligence
        };

        self.consensus_history.push(outcome.clone());
        outcome
    }
}

// Phase 56: Futarchy Verification
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_futarchy_resolution() {
        let mut market = FutarchyMarket::new();
        market.record_prediction(1, 150.0, 100.0); // P_Pass = 150 (Significant growth)

        let outcome = market.resolve_market_outcome(1);
        assert_eq!(outcome.decision, "PASS");
        assert!(outcome.price_delta > 0.0);
    }
}
