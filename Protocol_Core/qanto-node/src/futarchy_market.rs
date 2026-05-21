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
    pub p_pass: u128, // Predicted value if proposal passes (scaled)
    pub p_fail: u128, // Predicted value if proposal fails (scaled)
    pub volume: u64,
    pub resolution_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOutcome {
    pub proposal_id: u32,
    pub decision: String, // e.g., "PASS", "FAIL"
    pub price_delta: i128,
    pub veracity_score: u128,
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
    pub fn record_prediction(&mut self, proposal_id: u32, p_pass: u128, p_fail: u128) {
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
        // Rule: Pass if P_Pass > P_Fail * 1.02 (2% premium)
        let pass_threshold = prediction.p_fail
            .checked_mul(102)
            .and_then(|v| v.checked_div(100))
            .unwrap_or(u128::MAX);
            
        let passed = prediction.p_pass > pass_threshold;
        
        let price_delta = if prediction.p_pass >= prediction.p_fail {
            (prediction.p_pass - prediction.p_fail) as i128
        } else {
            -((prediction.p_fail - prediction.p_pass) as i128)
        };

        let outcome = MarketOutcome {
            proposal_id,
            decision: if passed { "PASS".to_string() } else { "FAIL".to_string() },
            price_delta,
            veracity_score: (98 * crate::QANTO_SCALE) / 100, // 0.98 accuracy
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
        let scale = 1_000_000_000u128;
        market.record_prediction(1, 150 * scale, 100 * scale); // P_Pass = 150 (Significant growth)

        let outcome = market.resolve_market_outcome(1);
        assert_eq!(outcome.decision, "PASS");
        assert!(outcome.price_delta > 0);
    }
}
