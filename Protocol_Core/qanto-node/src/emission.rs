use log::debug;
use prometheus::{register_int_counter, IntCounter};
use serde::{Deserialize, Serialize};

// Made constants public
pub const INITIAL_REWARD: u128 = 50 * 1_000_000_000; // Initial block reward: 50.0 QAN (scale 1e9)
pub const TOTAL_SUPPLY: u128 = 21_000_000_000 * 1_000_000_000; // Total supply cap: 21,000,000,000 QAN (scale 1e9)
pub const HALVING_PERIOD: u64 = 8_400_000; // Halving period in seconds (~97.2 days)
pub const HALVING_FACTOR_FIXED: u128 = 999_000_000; // 0.999 represented as u128 (scale 1e9)
pub const SCALE: u128 = 1_000_000_000; // Fixed-point scale for precision (9 decimals)

lazy_static::lazy_static! {
    static ref HALVING_EVENTS: IntCounter = register_int_counter!(
        "emission_halving_events_total",
        "Total number of halving events"
    ).unwrap();
    static ref SUPPLY_UPDATED: IntCounter = register_int_counter!(
        "emission_supply_updated_total",
        "Total number of supply updates"
    ).unwrap();
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Emission {
    initial_reward: u128,
    total_supply: u128,
    halving_period: u64,
    halving_factor_fixed: u128,
    genesis_timestamp: u64,
    current_supply: u128,
    num_chains: u32,
    last_halving_period: u64,
}

impl Emission {
    pub fn new(
        initial_reward: u128,
        total_supply: u128,
        halving_period: u64,
        halving_factor_fixed: u128,
        genesis_timestamp: u64,
        num_chains: u32,
    ) -> Self {
        Self {
            initial_reward: initial_reward.max(1),
            total_supply,
            halving_period: halving_period.max(1),
            halving_factor_fixed: if halving_factor_fixed > crate::QANTO_SCALE {
                crate::QANTO_SCALE
            } else {
                halving_factor_fixed
            },
            genesis_timestamp,
            current_supply: 0,
            num_chains: num_chains.max(1),
            last_halving_period: 0,
        }
    }

    pub fn default_with_timestamp(genesis_timestamp: u64, num_chains: u32) -> Self {
        Self::new(
            INITIAL_REWARD,
            TOTAL_SUPPLY,
            HALVING_PERIOD,
            HALVING_FACTOR_FIXED,
            genesis_timestamp,
            num_chains,
        )
    }

    pub fn calculate_reward(&self, timestamp: u64) -> Result<u128, String> {
        if timestamp < self.genesis_timestamp {
            return Err("Timestamp cannot be before genesis".into());
        }

        let elapsed_time = timestamp.saturating_sub(self.genesis_timestamp);
        let elapsed_periods = elapsed_time / self.halving_period;

        // Perform reward calculation using deterministic fixed-point arithmetic
        let mut reward = self.initial_reward;
        let scale = SCALE;
        
        // Apply halving factors iteratively
        for _ in 0..elapsed_periods {
            reward = reward.checked_mul(self.halving_factor_fixed).and_then(|r| r.checked_div(scale)).unwrap_or(0);
        }

        let mut per_chain_reward = reward
            .checked_div(self.num_chains as u128)
            .unwrap_or(1)
            .max(1);

        // Mathematically seal the 21B cap: never emit more than the remaining supply
        let max_remaining = self.total_supply.saturating_sub(self.current_supply);
        if max_remaining == 0 {
            per_chain_reward = 0;
        } else {
            per_chain_reward = per_chain_reward.min(max_remaining);
        }

        if elapsed_periods > self.last_halving_period {
            HALVING_EVENTS.inc();
            debug!("Halving event: period {elapsed_periods}, current reward per chain: {per_chain_reward}");
        }

        Ok(per_chain_reward)
    }

    pub fn update_supply(&mut self, reward: u128) -> Result<(), String> {
        let new_supply = self.current_supply.saturating_add(reward);

        // If adding the reward exceeds the mathematically sealed cap, gracefully clamp it.
        // This ensures the node doesn't crash but simply enforces the supply limit.
        if new_supply > self.total_supply {
            self.current_supply = self.total_supply;
            debug!("Total supply cap of 21B QAN reached. Entering non-inflationary phase.");
            return Ok(());
        }

        self.current_supply = new_supply;
        SUPPLY_UPDATED.inc();
        debug!(
            "Updated supply: {}. Reward added to total supply: {}",
            self.current_supply, reward
        );
        Ok(())
    }

    pub fn current_supply(&self) -> u128 {
        self.current_supply
    }

    pub fn total_supply(&self) -> u128 {
        self.total_supply
    }

    pub fn update_last_halving_period(&mut self, timestamp: u64) {
        if self.halving_period == 0 {
            return;
        }
        let elapsed_time = timestamp.saturating_sub(self.genesis_timestamp);
        let current_calculated_period = elapsed_time / self.halving_period;
        if current_calculated_period > self.last_halving_period {
            debug!(
                "Emission state: Last halving period updated from {} to {}",
                self.last_halving_period, current_calculated_period
            );
            self.last_halving_period = current_calculated_period;
        }
    }
}
