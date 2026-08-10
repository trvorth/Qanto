//! Strict, inactive V2 genesis-allocation manifest schema.
//!
//! This module defines validation rules only. It intentionally contains no
//! production recipient identities and does not activate a genesis reset.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{serde_u128_decimal, EMISSION_V2_PROTOCOL_VERSION, EMISSION_V2_SUBSIDY_CHAIN_ID};

/// Reserved allocation buckets prefunded by a V2 genesis reset. Community
/// subsidy is deliberately absent because it must be issued by `EmissionV2`.
#[derive(Clone, Copy, Debug, Deserialize, Ord, PartialEq, PartialOrd, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenesisAllocationBucketV2 {
    EcosystemDevelopment,
    Liquidity,
}

/// One prefunded reserved allocation. The recipient is a lowercase 32-byte
/// V2 account identity encoded as exactly 64 hexadecimal characters.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisAllocationEntryV2 {
    pub bucket: GenesisAllocationBucketV2,
    pub recipient_account_id_v2: String,
    #[serde(with = "serde_u128_decimal")]
    pub amount: u128,
}

/// Declarative input for a future V2 genesis reset.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisAllocationManifestV2 {
    pub protocol_version: u16,
    pub genesis_reset: bool,
    pub network_id: String,
    /// Network-domain identifier committed by V2 account derivation.
    pub network_chain_id: u32,
    /// DAG chain allowed to issue the community subsidy.
    pub subsidy_chain_id: u32,
    #[serde(with = "serde_u128_decimal")]
    pub community_emission_cap: u128,
    pub allocations: Vec<GenesisAllocationEntryV2>,
}

/// Totals proven by successful strict manifest validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedGenesisAllocationManifestV2 {
    pub ecosystem_development_total: u128,
    pub liquidity_total: u128,
    pub reserved_genesis_supply: u128,
    pub maximum_supply: u128,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GenesisAllocationManifestErrorV2 {
    #[error("unsupported genesis manifest version {found}; expected {expected}")]
    UnsupportedProtocolVersion { found: u16, expected: u16 },
    #[error("the V2 manifest must explicitly declare a genesis reset")]
    GenesisResetRequired,
    #[error("network_id must be 1-64 lowercase ASCII letters, digits, or interior hyphens")]
    InvalidNetworkId,
    #[error("manifest network_id does not match the expected activation network")]
    NetworkIdMismatch,
    #[error("manifest network chain {found} does not match expected chain {expected}")]
    NetworkChainIdMismatch { found: u32, expected: u32 },
    #[error("subsidy chain must be {expected}, found {found}")]
    UnexpectedSubsidyChain { found: u32, expected: u32 },
    #[error("community cap {found} does not match sealed allocation {expected}")]
    CommunityCapMismatch { found: u128, expected: u128 },
    #[error("at least one reserved genesis allocation is required")]
    EmptyAllocations,
    #[error("allocation {index} has an invalid V2 recipient identity")]
    InvalidRecipient { index: usize },
    #[error("allocation {index} has a zero amount")]
    ZeroAmount { index: usize },
    #[error("recipient identity is duplicated at allocation {index}")]
    DuplicateRecipient { index: usize },
    #[error("allocation {index} is not in canonical bucket/recipient order")]
    NonCanonicalOrder { index: usize },
    #[error("checked arithmetic failed while totaling genesis allocations")]
    AllocationArithmeticOverflow,
    #[error("ecosystem/development total {found} does not match sealed allocation {expected}")]
    EcosystemDevelopmentTotalMismatch { found: u128, expected: u128 },
    #[error("liquidity total {found} does not match sealed allocation {expected}")]
    LiquidityTotalMismatch { found: u128, expected: u128 },
    #[error("validated allocation sum {found} does not match sealed maximum supply {expected}")]
    SealedSupplyMismatch { found: u128, expected: u128 },
}

impl GenesisAllocationManifestV2 {
    /// Validates schema invariants, canonical ordering, unique recipients, exact
    /// reserved bucket totals, the sealed 21B allocation equation, and the
    /// externally selected activation network. Requiring the expected network
    /// prevents a valid manifest from being replayed into another genesis.
    pub fn validate_for_network(
        &self,
        expected_network_id: &str,
        expected_network_chain_id: u32,
    ) -> Result<ValidatedGenesisAllocationManifestV2, GenesisAllocationManifestErrorV2> {
        if self.protocol_version != EMISSION_V2_PROTOCOL_VERSION {
            return Err(
                GenesisAllocationManifestErrorV2::UnsupportedProtocolVersion {
                    found: self.protocol_version,
                    expected: EMISSION_V2_PROTOCOL_VERSION,
                },
            );
        }
        if !self.genesis_reset {
            return Err(GenesisAllocationManifestErrorV2::GenesisResetRequired);
        }
        if !is_canonical_network_id(&self.network_id)
            || !is_canonical_network_id(expected_network_id)
        {
            return Err(GenesisAllocationManifestErrorV2::InvalidNetworkId);
        }
        if self.network_id != expected_network_id {
            return Err(GenesisAllocationManifestErrorV2::NetworkIdMismatch);
        }
        if self.network_chain_id != expected_network_chain_id {
            return Err(GenesisAllocationManifestErrorV2::NetworkChainIdMismatch {
                found: self.network_chain_id,
                expected: expected_network_chain_id,
            });
        }
        if self.subsidy_chain_id != EMISSION_V2_SUBSIDY_CHAIN_ID {
            return Err(GenesisAllocationManifestErrorV2::UnexpectedSubsidyChain {
                found: self.subsidy_chain_id,
                expected: EMISSION_V2_SUBSIDY_CHAIN_ID,
            });
        }
        if self.community_emission_cap != crate::COMMUNITY_ALLOC {
            return Err(GenesisAllocationManifestErrorV2::CommunityCapMismatch {
                found: self.community_emission_cap,
                expected: crate::COMMUNITY_ALLOC,
            });
        }
        if self.allocations.is_empty() {
            return Err(GenesisAllocationManifestErrorV2::EmptyAllocations);
        }

        let mut seen_recipients = HashSet::with_capacity(self.allocations.len());
        let mut previous_key: Option<(GenesisAllocationBucketV2, &str)> = None;
        let mut ecosystem_development_total = 0_u128;
        let mut liquidity_total = 0_u128;

        for (index, allocation) in self.allocations.iter().enumerate() {
            if !is_canonical_v2_account_id(&allocation.recipient_account_id_v2) {
                return Err(GenesisAllocationManifestErrorV2::InvalidRecipient { index });
            }
            if allocation.amount == 0 {
                return Err(GenesisAllocationManifestErrorV2::ZeroAmount { index });
            }
            if !seen_recipients.insert(allocation.recipient_account_id_v2.as_str()) {
                return Err(GenesisAllocationManifestErrorV2::DuplicateRecipient { index });
            }

            let key = (
                allocation.bucket,
                allocation.recipient_account_id_v2.as_str(),
            );
            if previous_key.is_some_and(|previous| previous >= key) {
                return Err(GenesisAllocationManifestErrorV2::NonCanonicalOrder { index });
            }
            previous_key = Some(key);

            let bucket_total = match allocation.bucket {
                GenesisAllocationBucketV2::EcosystemDevelopment => &mut ecosystem_development_total,
                GenesisAllocationBucketV2::Liquidity => &mut liquidity_total,
            };
            *bucket_total = bucket_total
                .checked_add(allocation.amount)
                .ok_or(GenesisAllocationManifestErrorV2::AllocationArithmeticOverflow)?;
        }

        if ecosystem_development_total != crate::ECO_DEV_ALLOC {
            return Err(
                GenesisAllocationManifestErrorV2::EcosystemDevelopmentTotalMismatch {
                    found: ecosystem_development_total,
                    expected: crate::ECO_DEV_ALLOC,
                },
            );
        }
        if liquidity_total != crate::LIQUIDITY_ALLOC {
            return Err(GenesisAllocationManifestErrorV2::LiquidityTotalMismatch {
                found: liquidity_total,
                expected: crate::LIQUIDITY_ALLOC,
            });
        }

        let reserved_genesis_supply = ecosystem_development_total
            .checked_add(liquidity_total)
            .ok_or(GenesisAllocationManifestErrorV2::AllocationArithmeticOverflow)?;
        let maximum_supply = reserved_genesis_supply
            .checked_add(self.community_emission_cap)
            .ok_or(GenesisAllocationManifestErrorV2::AllocationArithmeticOverflow)?;
        if maximum_supply != crate::MAX_TOTAL_SUPPLY {
            return Err(GenesisAllocationManifestErrorV2::SealedSupplyMismatch {
                found: maximum_supply,
                expected: crate::MAX_TOTAL_SUPPLY,
            });
        }

        Ok(ValidatedGenesisAllocationManifestV2 {
            ecosystem_development_total,
            liquidity_total,
            reserved_genesis_supply,
            maximum_supply,
        })
    }
}

fn is_canonical_network_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    let is_edge_character = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();

    !bytes.is_empty()
        && bytes.len() <= 64
        && is_edge_character(bytes[0])
        && is_edge_character(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn is_canonical_v2_account_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> GenesisAllocationManifestV2 {
        GenesisAllocationManifestV2 {
            protocol_version: EMISSION_V2_PROTOCOL_VERSION,
            genesis_reset: true,
            network_id: "qanto-v2-test".to_owned(),
            network_chain_id: 12_340,
            subsidy_chain_id: EMISSION_V2_SUBSIDY_CHAIN_ID,
            community_emission_cap: crate::COMMUNITY_ALLOC,
            allocations: vec![
                GenesisAllocationEntryV2 {
                    bucket: GenesisAllocationBucketV2::EcosystemDevelopment,
                    recipient_account_id_v2: "11".repeat(32),
                    amount: crate::ECO_DEV_ALLOC,
                },
                GenesisAllocationEntryV2 {
                    bucket: GenesisAllocationBucketV2::Liquidity,
                    recipient_account_id_v2: "22".repeat(32),
                    amount: crate::LIQUIDITY_ALLOC,
                },
            ],
        }
    }

    fn validate(
        manifest: &GenesisAllocationManifestV2,
    ) -> Result<ValidatedGenesisAllocationManifestV2, GenesisAllocationManifestErrorV2> {
        manifest.validate_for_network("qanto-v2-test", 12_340)
    }

    #[test]
    fn validates_exact_reserved_allocations_and_sealed_sum() {
        let validated = validate(&valid_manifest()).unwrap();
        assert_eq!(validated.ecosystem_development_total, crate::ECO_DEV_ALLOC);
        assert_eq!(validated.liquidity_total, crate::LIQUIDITY_ALLOC);
        assert_eq!(
            validated.reserved_genesis_supply,
            crate::ECO_DEV_ALLOC + crate::LIQUIDITY_ALLOC
        );
        assert_eq!(validated.maximum_supply, crate::MAX_TOTAL_SUPPLY);
    }

    #[test]
    fn serde_uses_strict_decimal_strings_and_rejects_unknown_fields() {
        let manifest = valid_manifest();
        let encoded = serde_json::to_string(&manifest).unwrap();
        assert!(encoded.contains(&format!(
            "\"community_emission_cap\":\"{}\"",
            crate::COMMUNITY_ALLOC
        )));
        assert_eq!(
            serde_json::from_str::<GenesisAllocationManifestV2>(&encoded).unwrap(),
            manifest
        );

        let numeric_cap = encoded.replace(
            &format!("\"{}\"", crate::COMMUNITY_ALLOC),
            &crate::COMMUNITY_ALLOC.to_string(),
        );
        assert!(serde_json::from_str::<GenesisAllocationManifestV2>(&numeric_cap).is_err());

        let leading_zero_cap = encoded.replace(
            &format!("\"{}\"", crate::COMMUNITY_ALLOC),
            &format!("\"0{}\"", crate::COMMUNITY_ALLOC),
        );
        assert!(serde_json::from_str::<GenesisAllocationManifestV2>(&leading_zero_cap).is_err());

        let with_unknown = encoded.replacen('{', "{\"unknown\":true,", 1);
        assert!(serde_json::from_str::<GenesisAllocationManifestV2>(&with_unknown).is_err());
    }

    #[test]
    fn rejects_wrong_version_reset_chain_cap_and_network() {
        let mut manifest = valid_manifest();
        manifest.protocol_version = 1;
        assert!(matches!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::UnsupportedProtocolVersion { .. })
        ));

        let mut manifest = valid_manifest();
        manifest.genesis_reset = false;
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::GenesisResetRequired)
        );

        let mut manifest = valid_manifest();
        manifest.subsidy_chain_id = 1;
        assert!(matches!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::UnexpectedSubsidyChain { .. })
        ));

        let mut manifest = valid_manifest();
        manifest.community_emission_cap -= 1;
        assert!(matches!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::CommunityCapMismatch { .. })
        ));

        for network_id in ["", "Qanto-v2", "-qanto", "qanto-", "qanto_v2"] {
            let mut manifest = valid_manifest();
            manifest.network_id = network_id.to_owned();
            assert_eq!(
                validate(&manifest),
                Err(GenesisAllocationManifestErrorV2::InvalidNetworkId)
            );
        }

        let manifest = valid_manifest();
        assert_eq!(
            manifest.validate_for_network("qanto-v2-main", 12_340),
            Err(GenesisAllocationManifestErrorV2::NetworkIdMismatch)
        );
        assert_eq!(
            manifest.validate_for_network("qanto-v2-test", 12_341),
            Err(GenesisAllocationManifestErrorV2::NetworkChainIdMismatch {
                found: 12_340,
                expected: 12_341,
            })
        );
    }

    #[test]
    fn rejects_invalid_duplicate_or_noncanonical_recipients() {
        let mut manifest = valid_manifest();
        manifest.allocations[0].recipient_account_id_v2 = "0".repeat(64);
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::InvalidRecipient { index: 0 })
        );

        for invalid_recipient in ["aa".repeat(31), "AA".repeat(32)] {
            let mut manifest = valid_manifest();
            manifest.allocations[0].recipient_account_id_v2 = invalid_recipient;
            assert_eq!(
                validate(&manifest),
                Err(GenesisAllocationManifestErrorV2::InvalidRecipient { index: 0 })
            );
        }

        let mut manifest = valid_manifest();
        manifest.allocations[1].recipient_account_id_v2 = "11".repeat(32);
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::DuplicateRecipient { index: 1 })
        );

        let mut manifest = valid_manifest();
        manifest.allocations.swap(0, 1);
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::NonCanonicalOrder { index: 1 })
        );
    }

    #[test]
    fn rejects_zero_and_inexact_bucket_totals() {
        let mut manifest = valid_manifest();
        manifest.allocations.clear();
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::EmptyAllocations)
        );

        let mut manifest = valid_manifest();
        manifest.allocations[0].amount = 0;
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::ZeroAmount { index: 0 })
        );

        let mut manifest = valid_manifest();
        manifest.allocations[0].amount -= 1;
        assert!(matches!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::EcosystemDevelopmentTotalMismatch { .. })
        ));

        let mut manifest = valid_manifest();
        manifest.allocations[1].amount -= 1;
        assert!(matches!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::LiquidityTotalMismatch { .. })
        ));

        let mut manifest = valid_manifest();
        manifest.allocations[0].amount = u128::MAX;
        manifest.allocations.insert(
            1,
            GenesisAllocationEntryV2 {
                bucket: GenesisAllocationBucketV2::EcosystemDevelopment,
                recipient_account_id_v2: "12".repeat(32),
                amount: 1,
            },
        );
        assert_eq!(
            validate(&manifest),
            Err(GenesisAllocationManifestErrorV2::AllocationArithmeticOverflow)
        );
    }
}
