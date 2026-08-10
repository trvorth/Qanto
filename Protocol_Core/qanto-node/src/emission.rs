use log::debug;
use prometheus::{register_int_counter, IntCounter};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

pub mod genesis_allocation_manifest_v2;

pub use genesis_allocation_manifest_v2::{
    GenesisAllocationBucketV2, GenesisAllocationEntryV2, GenesisAllocationManifestErrorV2,
    GenesisAllocationManifestV2, ValidatedGenesisAllocationManifestV2,
};

// Made constants public
pub const INITIAL_REWARD: u128 = 50 * 1_000_000_000; // Initial block reward: 50.0 QAN (scale 1e9)
pub const TOTAL_SUPPLY: u128 = 21_000_000_000 * 1_000_000_000; // Total supply cap: 21,000,000,000 QAN (scale 1e9)
pub const HALVING_PERIOD: u64 = 8_400_000; // Halving period in seconds (~97.2 days)
pub const HALVING_FACTOR_FIXED: u128 = 999_000_000; // 0.999 represented as u128 (scale 1e9)
pub const SCALE: u128 = 1_000_000_000; // Fixed-point scale for precision (9 decimals)

/// Inactive QIP-003 emission protocol version.
///
/// This version is deliberately not connected to live block acceptance. A future
/// activation gate must select a new V2 genesis before these rules can mint supply.
pub const EMISSION_V2_PROTOCOL_VERSION: u16 = 2;
/// The only chain allowed to issue V2 community subsidy.
pub const EMISSION_V2_SUBSIDY_CHAIN_ID: u32 = 0;
/// Target block interval approved by QIP-003, in milliseconds.
pub const EMISSION_V2_TARGET_BLOCK_TIME_MS: u64 = 31;
/// Duration represented by one reward-decay period, in seconds.
pub const EMISSION_V2_PERIOD_DURATION_SECONDS: u64 = HALVING_PERIOD;
/// Number of blocks in one reward-decay period, rounded up.
pub const EMISSION_V2_PERIOD_BLOCKS: u64 =
    (EMISSION_V2_PERIOD_DURATION_SECONDS * 1_000 + EMISSION_V2_TARGET_BLOCK_TIME_MS - 1)
        / EMISSION_V2_TARGET_BLOCK_TIME_MS;
/// Initial chain-0 subsidy under the inactive V2 rules.
pub const EMISSION_V2_INITIAL_REWARD: u128 = INITIAL_REWARD;
/// Fixed-point reward recurrence multiplier under the inactive V2 rules.
pub const EMISSION_V2_REWARD_FACTOR_FIXED: u128 = HALVING_FACTOR_FIXED;
/// Sealed community budget available to V2 block subsidy.
pub const EMISSION_V2_COMMUNITY_CAP: u128 = crate::COMMUNITY_ALLOC;
/// Sealed ecosystem/development amount prefunded at V2 genesis.
pub const EMISSION_V2_ECOSYSTEM_DEVELOPMENT_PREFUND: u128 = crate::ECO_DEV_ALLOC;
/// Sealed liquidity amount prefunded at V2 genesis.
pub const EMISSION_V2_LIQUIDITY_PREFUND: u128 = crate::LIQUIDITY_ALLOC;
/// Total reserved supply prefunded at V2 genesis.
pub const EMISSION_V2_RESERVED_GENESIS_SUPPLY: u128 =
    EMISSION_V2_ECOSYSTEM_DEVELOPMENT_PREFUND + EMISSION_V2_LIQUIDITY_PREFUND;
/// Sealed maximum supply after all V2 community subsidy has been issued.
pub const EMISSION_V2_MAX_SUPPLY: u128 = crate::MAX_TOTAL_SUPPLY;

const _: () = assert!(EMISSION_V2_PERIOD_BLOCKS == 270_967_742);
const _: () = assert!(
    crate::COMMUNITY_ALLOC + crate::ECO_DEV_ALLOC + crate::LIQUIDITY_ALLOC
        == crate::MAX_TOTAL_SUPPLY
);

/// Failures returned by the inactive, replay-validated V2 emission state machine.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EmissionV2Error {
    #[error("unsupported emission protocol version {found}; expected {expected}")]
    UnsupportedProtocolVersion { found: u16, expected: u16 },
    #[error("next chain-0 height must be at least 1")]
    InvalidNextHeight,
    #[error("cumulative subsidy {found} exceeds the sealed community cap {cap}")]
    CommunityCapExceeded { found: u128, cap: u128 },
    #[error(
        "emission replay mismatch at next height {next_height}: state has {found}, expected {expected}"
    )]
    ReplayMismatch {
        next_height: u64,
        found: u128,
        expected: u128,
    },
    #[error("unexpected chain-0 height {found}; expected {expected}")]
    UnexpectedHeight { found: u64, expected: u64 },
    #[error("checked arithmetic failed while computing {0}")]
    ArithmeticOverflow(&'static str),
}

/// Persisted V2 emission state. Deserialization alone does not make a state
/// trusted; pass it through [`EmissionV2::from_state`] to validate replay.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmissionStateV2 {
    protocol_version: u16,
    next_height: u64,
    #[serde(with = "serde_u128_decimal")]
    cumulative_subsidy: u128,
}

impl EmissionStateV2 {
    /// Creates the state at the V2 genesis boundary. Genesis itself issues no
    /// community subsidy, so the first eligible block has height 1.
    pub fn genesis() -> Self {
        Self {
            protocol_version: EMISSION_V2_PROTOCOL_VERSION,
            next_height: 1,
            cumulative_subsidy: 0,
        }
    }

    /// Reconstructs the only valid state for a given next chain-0 height.
    pub fn replay_to_next_height(next_height: u64) -> Result<Self, EmissionV2Error> {
        let cumulative_subsidy = EmissionV2::expected_cumulative_before_height(next_height)?;
        Ok(Self {
            protocol_version: EMISSION_V2_PROTOCOL_VERSION,
            next_height,
            cumulative_subsidy,
        })
    }

    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    pub fn next_height(&self) -> u64 {
        self.next_height
    }

    pub fn cumulative_subsidy(&self) -> u128 {
        self.cumulative_subsidy
    }

    /// Total QNTO supply represented by this state: both prefunded reserved
    /// allocations plus community subsidy. Transaction fees are excluded.
    pub fn total_supply(&self) -> Result<u128, EmissionV2Error> {
        reserved_genesis_supply_v2()
            .checked_add(self.cumulative_subsidy)
            .ok_or(EmissionV2Error::ArithmeticOverflow("V2 total supply"))
    }
}

impl Default for EmissionStateV2 {
    fn default() -> Self {
        Self::genesis()
    }
}

/// Result of evaluating one block against the inactive V2 economic primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmissionOutcomeV2 {
    pub block_height: u64,
    pub chain_id: u32,
    /// Reward dictated by height before applying the remaining-community clamp.
    pub scheduled_subsidy: u128,
    /// Newly issued QNTO after the exact community-budget clamp.
    pub minted_subsidy: u128,
    /// Existing QNTO transferred from transaction senders to the producer.
    pub transferred_fees: u128,
    /// Producer credit. Only `minted_subsidy` changes total supply.
    pub producer_credit: u128,
    pub cumulative_subsidy: u128,
    pub total_supply: u128,
}

/// Inactive QIP-003 height-based emission engine.
///
/// The engine tracks chain-0 heights only. Blocks from every other chain always
/// receive zero subsidy and cannot mutate the community-emission state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmissionV2 {
    state: EmissionStateV2,
}

impl EmissionV2 {
    pub fn new() -> Self {
        Self {
            state: EmissionStateV2::genesis(),
        }
    }

    /// Restores state only after proving that cumulative subsidy is exactly what
    /// deterministic replay to `next_height` produces.
    pub fn from_state(state: EmissionStateV2) -> Result<Self, EmissionV2Error> {
        if state.protocol_version != EMISSION_V2_PROTOCOL_VERSION {
            return Err(EmissionV2Error::UnsupportedProtocolVersion {
                found: state.protocol_version,
                expected: EMISSION_V2_PROTOCOL_VERSION,
            });
        }
        if state.next_height == 0 {
            return Err(EmissionV2Error::InvalidNextHeight);
        }
        if state.cumulative_subsidy > EMISSION_V2_COMMUNITY_CAP {
            return Err(EmissionV2Error::CommunityCapExceeded {
                found: state.cumulative_subsidy,
                cap: EMISSION_V2_COMMUNITY_CAP,
            });
        }

        let expected = Self::expected_cumulative_before_height(state.next_height)?;
        if state.cumulative_subsidy != expected {
            return Err(EmissionV2Error::ReplayMismatch {
                next_height: state.next_height,
                found: state.cumulative_subsidy,
                expected,
            });
        }

        Ok(Self { state })
    }

    /// Restores the canonical replay result for a chain-0 height without
    /// iterating once per block.
    pub fn replay_to_next_height(next_height: u64) -> Result<Self, EmissionV2Error> {
        Self::from_state(EmissionStateV2::replay_to_next_height(next_height)?)
    }

    pub fn state(&self) -> &EmissionStateV2 {
        &self.state
    }

    pub fn into_state(self) -> EmissionStateV2 {
        self.state
    }

    pub fn community_budget_remaining(&self) -> u128 {
        EMISSION_V2_COMMUNITY_CAP - self.state.cumulative_subsidy
    }

    /// Returns the unclamped height-scheduled subsidy. Height zero is genesis
    /// and never earns subsidy. Nonzero chain IDs never earn subsidy.
    pub fn scheduled_subsidy_at_height(
        block_height: u64,
        chain_id: u32,
    ) -> Result<u128, EmissionV2Error> {
        if block_height == 0 || chain_id != EMISSION_V2_SUBSIDY_CHAIN_ID {
            return Ok(0);
        }

        let period = (block_height - 1) / EMISSION_V2_PERIOD_BLOCKS;
        reward_for_period_v2(period)
    }

    /// Computes cumulative chain-0 subsidy before `next_height` using period
    /// runs. The final run is clamped to the exact sealed community budget.
    pub fn expected_cumulative_before_height(next_height: u64) -> Result<u128, EmissionV2Error> {
        if next_height == 0 {
            return Err(EmissionV2Error::InvalidNextHeight);
        }

        let mut blocks_remaining = next_height - 1;
        let mut reward = EMISSION_V2_INITIAL_REWARD;
        let mut cumulative = 0_u128;

        while blocks_remaining > 0 && reward > 0 && cumulative < EMISSION_V2_COMMUNITY_CAP {
            let blocks_this_period = blocks_remaining.min(EMISSION_V2_PERIOD_BLOCKS);
            let scheduled = reward
                .checked_mul(u128::from(blocks_this_period))
                .ok_or(EmissionV2Error::ArithmeticOverflow("V2 period subsidy"))?;
            let remaining_budget = EMISSION_V2_COMMUNITY_CAP - cumulative;
            let minted = scheduled.min(remaining_budget);
            cumulative = cumulative
                .checked_add(minted)
                .ok_or(EmissionV2Error::ArithmeticOverflow("V2 cumulative subsidy"))?;
            blocks_remaining -= blocks_this_period;

            if blocks_remaining > 0 && cumulative < EMISSION_V2_COMMUNITY_CAP {
                reward = next_reward_v2(reward)?;
            }
        }

        Ok(cumulative)
    }

    /// Evaluates a block without mutating state.
    pub fn preview_block(
        &self,
        block_height: u64,
        chain_id: u32,
        transferred_fees: u128,
    ) -> Result<EmissionOutcomeV2, EmissionV2Error> {
        if chain_id == EMISSION_V2_SUBSIDY_CHAIN_ID && block_height != self.state.next_height {
            return Err(EmissionV2Error::UnexpectedHeight {
                found: block_height,
                expected: self.state.next_height,
            });
        }

        let scheduled_subsidy = Self::scheduled_subsidy_at_height(block_height, chain_id)?;
        let minted_subsidy = scheduled_subsidy.min(self.community_budget_remaining());
        let cumulative_subsidy = self
            .state
            .cumulative_subsidy
            .checked_add(minted_subsidy)
            .ok_or(EmissionV2Error::ArithmeticOverflow("V2 cumulative subsidy"))?;
        let producer_credit = minted_subsidy
            .checked_add(transferred_fees)
            .ok_or(EmissionV2Error::ArithmeticOverflow("V2 producer credit"))?;
        let total_supply = reserved_genesis_supply_v2()
            .checked_add(cumulative_subsidy)
            .ok_or(EmissionV2Error::ArithmeticOverflow("V2 total supply"))?;

        Ok(EmissionOutcomeV2 {
            block_height,
            chain_id,
            scheduled_subsidy,
            minted_subsidy,
            transferred_fees,
            producer_credit,
            cumulative_subsidy,
            total_supply,
        })
    }

    /// Applies a block atomically. A failed arithmetic or height check leaves
    /// state unchanged. Non-chain-0 blocks return a zero subsidy and do not
    /// advance chain-0 replay state.
    pub fn apply_block(
        &mut self,
        block_height: u64,
        chain_id: u32,
        transferred_fees: u128,
    ) -> Result<EmissionOutcomeV2, EmissionV2Error> {
        let outcome = self.preview_block(block_height, chain_id, transferred_fees)?;

        if chain_id == EMISSION_V2_SUBSIDY_CHAIN_ID {
            let next_height = self.state.next_height.checked_add(1).ok_or(
                EmissionV2Error::ArithmeticOverflow("V2 next chain-0 height"),
            )?;
            self.state.cumulative_subsidy = outcome.cumulative_subsidy;
            self.state.next_height = next_height;
        }

        Ok(outcome)
    }
}

impl Default for EmissionV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for EmissionV2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.state.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EmissionV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let state = EmissionStateV2::deserialize(deserializer)?;
        Self::from_state(state).map_err(D::Error::custom)
    }
}

fn reserved_genesis_supply_v2() -> u128 {
    EMISSION_V2_RESERVED_GENESIS_SUPPLY
}

fn next_reward_v2(reward: u128) -> Result<u128, EmissionV2Error> {
    reward
        .checked_mul(EMISSION_V2_REWARD_FACTOR_FIXED)
        .and_then(|value| value.checked_div(SCALE))
        .ok_or(EmissionV2Error::ArithmeticOverflow("V2 reward recurrence"))
}

fn reward_for_period_v2(period: u64) -> Result<u128, EmissionV2Error> {
    let mut reward = EMISSION_V2_INITIAL_REWARD;
    for _ in 0..period {
        if reward == 0 {
            break;
        }
        reward = next_reward_v2(reward)?;
    }
    Ok(reward)
}

pub(crate) mod serde_u128_decimal {
    use serde::{de::Error as _, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let canonical = value == "0"
            || (!value.starts_with('0') && value.bytes().all(|byte| byte.is_ascii_digit()));
        if !canonical {
            return Err(D::Error::custom(
                "expected a canonical unsigned decimal string",
            ));
        }
        value
            .parse::<u128>()
            .map_err(|_| D::Error::custom("unsigned decimal value exceeds u128"))
    }
}

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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
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

    /// Legacy V1 state for the already fully-prefunded genesis.
    ///
    /// No V1 block may increase this value. A new, versioned genesis is
    /// required before [`EmissionV2`] can become active.
    pub fn legacy_full_supply_with_timestamp(genesis_timestamp: u64, num_chains: u32) -> Self {
        let mut emission = Self::default_with_timestamp(genesis_timestamp, num_chains);
        emission.current_supply = emission.total_supply;
        emission
    }

    /// Reject persisted V1 state that does not exactly match fee-only
    /// full-supply containment for this network configuration.
    pub fn validate_legacy_containment(
        &self,
        genesis_timestamp: u64,
        num_chains: u32,
    ) -> Result<(), String> {
        let expected = Self::legacy_full_supply_with_timestamp(genesis_timestamp, num_chains);
        if self == &expected {
            Ok(())
        } else {
            Err("persisted V1 emission state does not match full-supply containment".to_string())
        }
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
            reward = reward
                .checked_mul(self.halving_factor_fixed)
                .and_then(|r| r.checked_div(scale))
                .unwrap_or(0);
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

#[cfg(test)]
mod v2_tests {
    use super::*;

    #[test]
    fn period_length_is_the_approved_ceiling() {
        let duration_ms = EMISSION_V2_PERIOD_DURATION_SECONDS * 1_000;
        assert_eq!(
            EMISSION_V2_PERIOD_BLOCKS,
            duration_ms.div_ceil(EMISSION_V2_TARGET_BLOCK_TIME_MS)
        );
        assert_eq!(EMISSION_V2_PERIOD_BLOCKS, 270_967_742);
    }

    #[test]
    fn height_boundaries_follow_the_fixed_integer_recurrence() {
        assert_eq!(
            EmissionV2::scheduled_subsidy_at_height(0, EMISSION_V2_SUBSIDY_CHAIN_ID).unwrap(),
            0
        );
        assert_eq!(
            EmissionV2::scheduled_subsidy_at_height(1, EMISSION_V2_SUBSIDY_CHAIN_ID).unwrap(),
            EMISSION_V2_INITIAL_REWARD
        );
        assert_eq!(
            EmissionV2::scheduled_subsidy_at_height(
                EMISSION_V2_PERIOD_BLOCKS,
                EMISSION_V2_SUBSIDY_CHAIN_ID,
            )
            .unwrap(),
            EMISSION_V2_INITIAL_REWARD
        );
        assert_eq!(
            EmissionV2::scheduled_subsidy_at_height(
                EMISSION_V2_PERIOD_BLOCKS + 1,
                EMISSION_V2_SUBSIDY_CHAIN_ID,
            )
            .unwrap(),
            49_950_000_000
        );
    }

    #[test]
    fn reward_recurrence_is_monotonic_and_eventually_reaches_zero() {
        let mut reward = EMISSION_V2_INITIAL_REWARD;
        let mut reached_zero = false;

        for _ in 0..50_000 {
            let next = next_reward_v2(reward).unwrap();
            assert!(next <= reward);
            reward = next;
            if reward == 0 {
                reached_zero = true;
                break;
            }
        }

        assert!(reached_zero);
        assert_eq!(next_reward_v2(0).unwrap(), 0);
    }

    #[test]
    fn only_chain_zero_can_emit_or_advance_state() {
        let mut emission = EmissionV2::new();
        let before = emission.state().clone();

        let outcome = emission.apply_block(42, 1, 123).unwrap();
        assert_eq!(outcome.scheduled_subsidy, 0);
        assert_eq!(outcome.minted_subsidy, 0);
        assert_eq!(outcome.transferred_fees, 123);
        assert_eq!(outcome.producer_credit, 123);
        assert_eq!(emission.state(), &before);
        assert_eq!(
            EmissionV2::scheduled_subsidy_at_height(1, u32::MAX).unwrap(),
            0
        );
    }

    #[test]
    fn final_partial_subsidy_clamps_the_exact_community_budget() {
        let first_period_total = EMISSION_V2_INITIAL_REWARD
            .checked_mul(u128::from(EMISSION_V2_PERIOD_BLOCKS))
            .unwrap();
        assert!(first_period_total < crate::COMMUNITY_ALLOC);

        let second_period_reward = next_reward_v2(EMISSION_V2_INITIAL_REWARD).unwrap();
        let remaining_after_first_period = crate::COMMUNITY_ALLOC - first_period_total;
        let full_second_period_blocks = remaining_after_first_period / second_period_reward;
        let final_remainder = remaining_after_first_period % second_period_reward;
        assert_eq!(full_second_period_blocks, 65_097_355);
        assert_eq!(final_remainder, 17_750_000_000);

        let final_height = EMISSION_V2_PERIOD_BLOCKS
            .checked_add(u64::try_from(full_second_period_blocks).unwrap())
            .and_then(|height| height.checked_add(1))
            .unwrap();
        assert_eq!(final_height, 336_065_098);
        let mut emission = EmissionV2::replay_to_next_height(final_height).unwrap();
        assert_eq!(emission.community_budget_remaining(), final_remainder);

        let final_outcome = emission
            .apply_block(final_height, EMISSION_V2_SUBSIDY_CHAIN_ID, 0)
            .unwrap();
        assert_eq!(final_outcome.scheduled_subsidy, second_period_reward);
        assert_eq!(final_outcome.minted_subsidy, final_remainder);
        assert_eq!(final_outcome.cumulative_subsidy, crate::COMMUNITY_ALLOC);
        assert_eq!(final_outcome.total_supply, crate::MAX_TOTAL_SUPPLY);

        let post_cap = emission
            .apply_block(final_height + 1, EMISSION_V2_SUBSIDY_CHAIN_ID, 0)
            .unwrap();
        assert_eq!(post_cap.scheduled_subsidy, second_period_reward);
        assert_eq!(post_cap.minted_subsidy, 0);
        assert_eq!(post_cap.total_supply, crate::MAX_TOTAL_SUPPLY);
    }

    #[test]
    fn persisted_state_and_engine_validate_replay_on_restart() {
        let mut emission = EmissionV2::new();
        for height in 1..=3 {
            emission
                .apply_block(height, EMISSION_V2_SUBSIDY_CHAIN_ID, height as u128)
                .unwrap();
        }

        let encoded_state = serde_json::to_string(emission.state()).unwrap();
        assert!(encoded_state.contains("\"cumulative_subsidy\":\"150000000000\""));
        let decoded_state: EmissionStateV2 = serde_json::from_str(&encoded_state).unwrap();
        let restored = EmissionV2::from_state(decoded_state).unwrap();
        assert_eq!(restored, emission);

        let encoded_engine = serde_json::to_string(&emission).unwrap();
        let decoded_engine: EmissionV2 = serde_json::from_str(&encoded_engine).unwrap();
        assert_eq!(decoded_engine, emission);

        let mismatch = EmissionStateV2 {
            protocol_version: EMISSION_V2_PROTOCOL_VERSION,
            next_height: 4,
            cumulative_subsidy: 1,
        };
        assert!(matches!(
            EmissionV2::from_state(mismatch),
            Err(EmissionV2Error::ReplayMismatch { .. })
        ));
    }

    #[test]
    fn restore_rejects_wrong_version_height_and_cap() {
        let wrong_version = EmissionStateV2 {
            protocol_version: 1,
            next_height: 1,
            cumulative_subsidy: 0,
        };
        assert!(matches!(
            EmissionV2::from_state(wrong_version),
            Err(EmissionV2Error::UnsupportedProtocolVersion { .. })
        ));

        let zero_height = EmissionStateV2 {
            protocol_version: EMISSION_V2_PROTOCOL_VERSION,
            next_height: 0,
            cumulative_subsidy: 0,
        };
        assert_eq!(
            EmissionV2::from_state(zero_height),
            Err(EmissionV2Error::InvalidNextHeight)
        );

        let over_cap = EmissionStateV2 {
            protocol_version: EMISSION_V2_PROTOCOL_VERSION,
            next_height: 1,
            cumulative_subsidy: crate::COMMUNITY_ALLOC + 1,
        };
        assert!(matches!(
            EmissionV2::from_state(over_cap),
            Err(EmissionV2Error::CommunityCapExceeded { .. })
        ));
    }

    #[test]
    fn fees_are_transfers_and_never_change_issuance() {
        let mut emission = EmissionV2::new();
        let fees = 7_500 * crate::QANTO_SCALE;
        let outcome = emission
            .apply_block(1, EMISSION_V2_SUBSIDY_CHAIN_ID, fees)
            .unwrap();

        assert_eq!(outcome.minted_subsidy, EMISSION_V2_INITIAL_REWARD);
        assert_eq!(outcome.transferred_fees, fees);
        assert_eq!(outcome.producer_credit, EMISSION_V2_INITIAL_REWARD + fees);
        assert_eq!(outcome.cumulative_subsidy, EMISSION_V2_INITIAL_REWARD);
        assert_eq!(
            outcome.total_supply,
            crate::ECO_DEV_ALLOC + crate::LIQUIDITY_ALLOC + EMISSION_V2_INITIAL_REWARD
        );
        assert_eq!(
            emission.state().total_supply().unwrap(),
            outcome.total_supply
        );
    }

    #[test]
    fn arithmetic_failure_is_atomic_and_sequential_height_is_enforced() {
        let mut emission = EmissionV2::new();
        let before = emission.state().clone();
        assert_eq!(
            emission.apply_block(2, EMISSION_V2_SUBSIDY_CHAIN_ID, 0),
            Err(EmissionV2Error::UnexpectedHeight {
                found: 2,
                expected: 1,
            })
        );
        assert_eq!(emission.state(), &before);

        assert_eq!(
            emission.apply_block(1, EMISSION_V2_SUBSIDY_CHAIN_ID, u128::MAX),
            Err(EmissionV2Error::ArithmeticOverflow("V2 producer credit"))
        );
        assert_eq!(emission.state(), &before);

        emission
            .apply_block(1, EMISSION_V2_SUBSIDY_CHAIN_ID, 0)
            .unwrap();
        assert_eq!(emission.state().next_height(), 2);
    }

    #[test]
    fn replay_is_bounded_by_periods_and_stays_capped_at_extreme_height() {
        let emission = EmissionV2::replay_to_next_height(u64::MAX).unwrap();
        assert_eq!(
            emission.state().cumulative_subsidy(),
            crate::COMMUNITY_ALLOC
        );
        assert_eq!(
            emission.state().total_supply().unwrap(),
            crate::MAX_TOTAL_SUPPLY
        );
    }

    #[test]
    fn sealed_allocations_sum_to_the_supply_cap() {
        assert_eq!(
            crate::COMMUNITY_ALLOC + crate::ECO_DEV_ALLOC + crate::LIQUIDITY_ALLOC,
            crate::MAX_TOTAL_SUPPLY
        );
        assert_eq!(crate::QANTO_SCALE, 1_000_000_000);
    }
}
