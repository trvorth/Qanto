use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/**
 * @title Universal Basic Inference (UBI)
 * @dev High-fidelity credit refill and Pioneer reward algorithm.
 * Daily_Credit = f(Reputation_Score, Pioneer_Rank).
 */
pub struct UBICreditEngine {
    pub credit_registry: HashMap<String, UBICredit>,
    pub base_refill: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UBICredit {
    pub user_address: String,
    pub available_credits: u64,
    pub reputation_score: u32,
    pub pioneer_rank: u32, // 1 to 5
    pub last_refill: u64,
}

impl UBICreditEngine {
    pub fn new() -> Self {
        Self {
            credit_registry: HashMap::new(),
            base_refill: 100, // 100 base inference units per day
        }
    }

    /**
     * @dev Calculates the daily credit refill for a PioneerNFT holder.
     * Multipliers: 1.0 to 5.0x based on rank.
     */
    pub fn calculate_daily_credits(&self, reputation: u32, rank: u32) -> u64 {
        let reputation_multiplier = (reputation as f64 / 100.0).max(1.0);
        let rank_multiplier = rank as f64;
        
        println!("UBI: Calculating credits... Reputation: {}, Rank: {}.", reputation, rank);
        (self.base_refill as f64 * reputation_multiplier * rank_multiplier) as u64
    }

    /**
     * @dev Simulates the daily refill cycle for a user.
     */
    pub fn refill_credits(&mut self, user: String, reputation: u32, rank: u32) -> u64 {
        let refill_amount = self.calculate_daily_credits(reputation, rank);
        
        let credit = self.credit_registry.entry(user.clone()).or_insert(UBICredit {
            user_address: user.clone(),
            available_credits: 0,
            reputation_score: reputation,
            pioneer_rank: rank,
            last_refill: 0,
        });

        credit.available_credits += refill_amount;
        credit.last_refill = 850; // Cycle 850 (recursive veracity pulse)
        
        println!("UBI: User {} refilled with {} credits. New Balance: {}.", user, refill_amount, credit.available_credits);
        refill_amount
    }
}

// Phase 59: UBI Verification
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ubi_credit_calculation() {
        let engine = UBICreditEngine::new();
        let refill = engine.calculate_daily_credits(150, 3); // 1.5x rep, 3x rank
        
        assert_eq!(refill, 450); // 100 * 1.5 * 3 = 450
    }
}
