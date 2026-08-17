//! Versioned consensus parameters and activation logic.
//! All fields are fixed for the lifetime of a protocol version.
//! A new version requires a QIP, new genesis, and human approval.

use crate::record::{CanonicalRecord, FieldSpec, WireType};

/// Protocol version discriminant. V1 = current private/testnet. V2 = public candidate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum ProtocolVersion {
    V1 = 1,
    V2 = 2,
}

impl ProtocolVersion {
    pub const fn is_v2_active(self) -> bool {
        matches!(self, Self::V2)
    }
}

/// Schema version within a protocol version. Incremented for any FieldSpec change.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum SchemaVersion {
    V1_LEGACY = 0,
    V2_INITIAL = 1,
}

/// The V2 activation height. Zero means V2 is unreachable (current state).
/// A human-approved QIP-002 sets this to a positive block height on a V2 genesis.
pub const ACTIVATION_HEIGHT: u64 = 0;

/// Check if V2 consensus rules are active for the given height and parameters.
/// Returns true only when ACTIVATION_HEIGHT > 0, height >= ACTIVATION_HEIGHT, and params.version == V2.
pub fn is_v2_active(height: u64, params: &ConsensusParametersV2) -> bool {
    // ACTIVATION_HEIGHT is a governance-controlled constant (default 0 = V2 inactive).
    // V2 activates only when QIP-002 sets ACTIVATION_HEIGHT > 0 on a V2 genesis.
    // clippy::absurd_extreme_comparisons: Short-circuit evaluation means
    // height >= ACTIVATION_HEIGHT is only evaluated when ACTIVATION_HEIGHT != 0.
    #[allow(clippy::absurd_extreme_comparisons)]
    let active = ACTIVATION_HEIGHT != 0 && height >= ACTIVATION_HEIGHT && params.version == ProtocolVersion::V2;
    active
}

/// Consensus parameters committed in genesis and referenced by every V2 block.
/// This struct is canonically encoded and hashed via DomainV2::ConsensusParameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsensusParametersV2 {
    pub version: ProtocolVersion,
    pub schema_version: SchemaVersion,
    pub chain_id: u32,
    pub genesis_hash: [u8; 32],
    pub max_block_weight: u64,
    pub target_block_interval_ms: u64,
    pub difficulty_window: u64,
    pub difficulty_adjustment_bounds: (u64, u64), // (min_ratio, max_ratio) as basis points, e.g., (7500, 12500) for ±25%
    pub min_difficulty: u64,
    pub max_difficulty: u64,
    pub median_time_past_window: usize,
    pub future_timestamp_drift_ms: u64,
    pub pow_suite: crate::hash::HashSuiteV2,
    pub initial_validator_set_commitment: [u8; 32],
    // Additional parameters are added as new schema versions.
}

impl ConsensusParametersV2 {
    /// Parameters for the private engineering testnet candidate (V1 profile only).
    pub fn for_private_engineering() -> Self {
        Self {
            version: ProtocolVersion::V1,
            schema_version: SchemaVersion::V1_LEGACY,
            chain_id: 0x5141_4E54, // "QANT" as u32
            genesis_hash: [0x00u8; 32],
            max_block_weight: 1_000_000,
            target_block_interval_ms: 15_000,
            difficulty_window: 10,
            difficulty_adjustment_bounds: (7500, 12500), // ±25%
            min_difficulty: 1,
            max_difficulty: u64::MAX,
            median_time_past_window: 11,
            future_timestamp_drift_ms: 2000,
            pow_suite: crate::hash::HashSuiteV2::Sha3_256,
            initial_validator_set_commitment: [0xffu8; 32],
        }
    }

    /// Parameters for the public testnet candidate (V2, but ACTIVATION_HEIGHT = 0).
    pub fn for_testnet_candidate() -> Self {
        Self {
            version: ProtocolVersion::V2,
            schema_version: SchemaVersion::V2_INITIAL,
            chain_id: 0x5141_4E54, // "QANT" as u32 (1363527796)
            genesis_hash: [0x00u8; 32],
            max_block_weight: 1_000_000,
            target_block_interval_ms: 15_000,
            difficulty_window: 10,
            difficulty_adjustment_bounds: (7500, 12500), // ±25%
            min_difficulty: 1,
            max_difficulty: u64::MAX,
            median_time_past_window: 11,
            future_timestamp_drift_ms: 2000,
            pow_suite: crate::hash::HashSuiteV2::Sha3_256,
            initial_validator_set_commitment: [0xffu8; 32],
        }
    }

    /// Encode as a CanonicalRecord with schema-aware field IDs.
    /// Field allocation:
    /// 1: version (u8)
    /// 2: schema_version (u16)
    /// 3: chain_id (u32)
    /// 4: genesis_hash (Hash32)
    /// 5: max_block_weight (u64)
    /// 6: target_block_interval_ms (u64)
    /// 7: difficulty_window (u64)
    /// 8: difficulty_adjustment_min_bps (u16)
    /// 9: difficulty_adjustment_max_bps (u16)
    /// 10: min_difficulty (u64)
    /// 11: max_difficulty (u64)
    /// 12: median_time_past_window (u32)
    /// 13: future_timestamp_drift_ms (u64)
    /// 14: pow_suite (u16)
    /// 15: initial_validator_set_commitment (Hash32)
    pub fn to_canonical_record(&self, schema: SchemaVersion) -> CanonicalRecord {
        let mut record = CanonicalRecord::new();
        record.push_u8(1, self.version as u8).unwrap();
        record.push_u16(2, self.schema_version as u16).unwrap();
        record.push_u32(3, self.chain_id).unwrap();
        record.push_hash32(4, &self.genesis_hash).unwrap();
        record.push_u64(5, self.max_block_weight).unwrap();
        record.push_u64(6, self.target_block_interval_ms).unwrap();
        record.push_u64(7, self.difficulty_window).unwrap();
        record
            .push_u16(8, self.difficulty_adjustment_bounds.0 as u16)
            .unwrap();
        record
            .push_u16(9, self.difficulty_adjustment_bounds.1 as u16)
            .unwrap();
        record.push_u64(10, self.min_difficulty).unwrap();
        record.push_u64(11, self.max_difficulty).unwrap();
        record
            .push_u32(12, self.median_time_past_window as u32)
            .unwrap();
        record.push_u64(13, self.future_timestamp_drift_ms).unwrap();
        record.push_u16(14, self.pow_suite as u16).unwrap();
        record
            .push_hash32(15, &self.initial_validator_set_commitment)
            .unwrap();
        record
    }
}

/// The append-only domain registry for V2. Maps DomainV2 enum variants to their on-wire codes.
/// This is canonically encoded as DomainV2::DomainRegistry.
pub const DOMAIN_REGISTRY: &[(crate::hash::DomainV2, u16)] = &[
    (crate::hash::DomainV2::TxBody, 0x0001),
    (crate::hash::DomainV2::TxSigning, 0x0002),
    (crate::hash::DomainV2::TxEnvelope, 0x0003),
    (crate::hash::DomainV2::UtxoOutpoint, 0x0004),
    (crate::hash::DomainV2::BlockCore, 0x0010),
    (crate::hash::DomainV2::PowWork, 0x0011),
    (crate::hash::DomainV2::BlockSigning, 0x0012),
    (crate::hash::DomainV2::BlockId, 0x0013),
    (crate::hash::DomainV2::TxMerkleEmpty, 0x0020),
    (crate::hash::DomainV2::TxMerkleLeaf, 0x0021),
    (crate::hash::DomainV2::TxMerkleNode, 0x0022),
    (crate::hash::DomainV2::TxMerkleRoot, 0x0023),
    (crate::hash::DomainV2::StateKey, 0x0030),
    (crate::hash::DomainV2::StateLeaf, 0x0031),
    (crate::hash::DomainV2::StateNode, 0x0032),
    (crate::hash::DomainV2::StateRoot, 0x0033),
    (crate::hash::DomainV2::AccountId, 0x0040),
    (crate::hash::DomainV2::ValidatorId, 0x0041),
    (crate::hash::DomainV2::PeerId, 0x0042),
    (crate::hash::DomainV2::FinalityVote, 0x0050),
    (crate::hash::DomainV2::FinalityCertificate, 0x0051),
    (crate::hash::DomainV2::ValidatorSet, 0x0052),
    (crate::hash::DomainV2::BridgeClaim, 0x0060),
    (crate::hash::DomainV2::BridgeNullifier, 0x0061),
    (crate::hash::DomainV2::ZkVerifyingKey, 0x0062),
    (crate::hash::DomainV2::P2pEnvelope, 0x0070),
    (crate::hash::DomainV2::SnapshotManifest, 0x0071),
    (crate::hash::DomainV2::GenesisManifest, 0x0080),
    (crate::hash::DomainV2::ConsensusParameters, 0x0081),
    (crate::hash::DomainV2::DomainRegistry, 0x0082),
];

/// FieldSpec array for schema-aware decoding of ConsensusParametersV2.
pub const CONSENSUS_PARAMETERS_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U8,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 2,
        wire_type: WireType::U16,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 3,
        wire_type: WireType::U32,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 4,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 5,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 6,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 7,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 8,
        wire_type: WireType::U16,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 9,
        wire_type: WireType::U16,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 10,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 11,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 12,
        wire_type: WireType::U32,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 13,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 14,
        wire_type: WireType::U16,
        required: true,
        sequence: None,
    },
    FieldSpec {
        id: 15,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    },
];
