//! Canonical V2 record schemas and builder functions for every consensus-visible V2 object.
//! Every function returns a CanonicalRecord ready for .finish() and hash_v2.
//! No function in this module performs hashing; it only constructs canonical payloads.

use crate::{
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    hash::{BlockIdV2, DomainV2, HashV2},
    record::{CanonicalRecord, FieldSpec, SequenceSemantics, SequenceSpec, WireType},
};

/// FieldSpec arrays for schema-aware decoding. Keep in sync with builder functions.
pub const TX_BODY_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U8,
        required: true,
        sequence: None,
    }, // kind
    FieldSpec {
        id: 2,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // nonce
    FieldSpec {
        id: 3,
        wire_type: WireType::U128,
        required: true,
        sequence: None,
    }, // gas_limit
    FieldSpec {
        id: 4,
        wire_type: WireType::U128,
        required: true,
        sequence: None,
    }, // gas_price
    FieldSpec {
        id: 5,
        wire_type: WireType::U128,
        required: true,
        sequence: None,
    }, // priority_fee
    FieldSpec {
        id: 6,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // sender (AccountIdV2)
    FieldSpec {
        id: 7,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // receiver (AccountIdV2 or 0 for coinbase)
    FieldSpec {
        id: 8,
        wire_type: WireType::U128,
        required: true,
        sequence: None,
    }, // value
    FieldSpec {
        id: 9,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // payload (for EVMCall etc.)
    FieldSpec {
        id: 10,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // chain_id
    FieldSpec {
        id: 11,
        wire_type: WireType::U16,
        required: true,
        sequence: None,
    }, // schema_version
];

pub const TX_SIGNING_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // tx_body_hash (TxIdV2 as bytes)
    FieldSpec {
        id: 2,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // public_key (ML-DSA-65)
    FieldSpec {
        id: 3,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // signature
];

pub const TX_ENVELOPE_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // tx_body_hash
    FieldSpec {
        id: 2,
        wire_type: WireType::Sequence,
        required: true,
        sequence: Some(SequenceSpec {
            element_type: WireType::Bytes,
            semantics: SequenceSemantics::Ordered,
        }),
    }, // [signature, public_key] or [r,s,...] per suite
];

pub const UTXO_OUTPOINT_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // tx_id (TxIdV2)
    FieldSpec {
        id: 2,
        wire_type: WireType::U32,
        required: true,
        sequence: None,
    }, // output_index
];

pub const BLOCK_CORE_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U8,
        required: true,
        sequence: None,
    }, // version
    FieldSpec {
        id: 2,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // parent_block_id
    FieldSpec {
        id: 3,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // state_root
    FieldSpec {
        id: 4,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // transactions_root (TxMerkleRoot)
    FieldSpec {
        id: 5,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // timestamp
    FieldSpec {
        id: 6,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // difficulty
    FieldSpec {
        id: 7,
        wire_type: WireType::U128,
        required: true,
        sequence: None,
    }, // chain_work (cumulative work as u128)
    FieldSpec {
        id: 8,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // validator_id
    FieldSpec {
        id: 9,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // miner_id
    FieldSpec {
        id: 10,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // height
    FieldSpec {
        id: 11,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // state_parent (BlockIdV2)
];

pub const POW_WORK_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // block_core_hash (HashV2 from BlockCore)
    FieldSpec {
        id: 2,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // nonce
];

pub const BLOCK_SIGNING_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // block_id (BlockIdV2)
    FieldSpec {
        id: 2,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // validator_public_key
    FieldSpec {
        id: 3,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // signature
];

pub const BLOCK_ID_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // block_core_hash
];

pub const TX_MERKLE_EMPTY_SPECS: &[FieldSpec] = &[]; // Unit record (0 fields) -> 0x0000
pub const TX_MERKLE_LEAF_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // tx_id (TxIdV2)
];
pub const TX_MERKLE_NODE_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // left
    FieldSpec {
        id: 2,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // right
];
pub const TX_MERKLE_ROOT_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // root_hash
];

pub const STATE_KEY_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // key bytes
];
pub const STATE_LEAF_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // value hash
];
pub const STATE_NODE_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // left
    FieldSpec {
        id: 2,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // right
];
pub const STATE_ROOT_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // root hash
];

pub const ACCOUNT_ID_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // ML-DSA-65 public key (1952 bytes)
];
pub const VALIDATOR_ID_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // ML-DSA-65 public key (1952 bytes)
];
pub const PEER_ID_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // libp2p PeerId bytes
];

pub const FINALITY_VOTE_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // epoch
    FieldSpec {
        id: 2,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // round
    FieldSpec {
        id: 3,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // height
    FieldSpec {
        id: 4,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // block_id
    FieldSpec {
        id: 5,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // validator_set_commitment
    FieldSpec {
        id: 6,
        wire_type: WireType::U8,
        required: true,
        sequence: None,
    }, // vote_type (0=propose, 1=prevote, 2=precommit)
    FieldSpec {
        id: 7,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // validator_public_key
    FieldSpec {
        id: 8,
        wire_type: WireType::Bytes,
        required: true,
        sequence: None,
    }, // signature
];

pub const FINALITY_CERTIFICATE_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // epoch
    FieldSpec {
        id: 2,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // round
    FieldSpec {
        id: 3,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // block_id
    FieldSpec {
        id: 4,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // validator_set_commitment
    FieldSpec {
        id: 5,
        wire_type: WireType::Sequence,
        required: true,
        sequence: Some(SequenceSpec {
            element_type: WireType::Bytes,
            semantics: SequenceSemantics::Ordered,
        }),
    }, // [vote_byte...] aggregated
];

pub const VALIDATOR_SET_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U64,
        required: true,
        sequence: None,
    }, // epoch
    FieldSpec {
        id: 2,
        wire_type: WireType::Sequence,
        required: true,
        sequence: Some(SequenceSpec {
            element_type: WireType::Hash32,
            semantics: SequenceSemantics::Set,
        }),
    }, // sorted ValidatorIdV2
];

pub const GENESIS_MANIFEST_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::U32,
        required: true,
        sequence: None,
    }, // chain_id
    FieldSpec {
        id: 2,
        wire_type: WireType::Hash32,
        required: true,
        sequence: None,
    }, // genesis_hash
    FieldSpec {
        id: 3,
        wire_type: WireType::Sequence,
        required: true,
        sequence: Some(SequenceSpec {
            element_type: WireType::Bytes,
            semantics: SequenceSemantics::Ordered,
        }),
    }, // allocation records
];

pub const DOMAIN_REGISTRY_SPECS: &[FieldSpec] = &[
    FieldSpec {
        id: 1,
        wire_type: WireType::Sequence,
        required: true,
        sequence: Some(SequenceSpec {
            element_type: WireType::U16,
            semantics: SequenceSemantics::Ordered,
        }),
    }, // sorted domain codes
];

/// Build the canonical DomainRegistry record for a given schema version.
pub fn domain_registry_canonical_record(_schema: SchemaVersion) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    let codes: Vec<u16> = (0x0001..=0x0082)
        .filter(|c| DomainV2::try_from(*c).is_ok())
        .collect();
    let code_bytes: Vec<[u8; 2]> = codes.iter().map(|c| c.to_be_bytes()).collect();
    let code_refs: Vec<&[u8]> = code_bytes.iter().map(|b| b.as_slice()).collect();
    record
        .push_set_sequence(1, WireType::U16, &code_refs)
        .unwrap();
    record
}

/// Build the canonical TransactionBodyV2 record.
// kind: 0=Transfer, 1=Coinbase, 2=EVMCall (excluded from private profile), 3=BridgeCall (excluded).
#[allow(clippy::too_many_arguments)]
pub fn transaction_body_v2(
    kind: u8,
    nonce: u64,
    gas_limit: u128,
    gas_price: u128,
    priority_fee: u128,
    sender: HashV2,   // AccountIdV2 from hash_v2(AccountId, chain, schema, pubkey)
    receiver: HashV2, // AccountIdV2 or ZERO for coinbase
    value: u128,
    payload: &[u8],
    chain_id: u32,
    schema: SchemaVersion,
) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_u8(1, kind).unwrap();
    record.push_u64(2, nonce).unwrap();
    record.push_u128(3, gas_limit).unwrap();
    record.push_u128(4, gas_price).unwrap();
    record.push_u128(5, priority_fee).unwrap();
    record.push_hash32(6, sender.as_bytes()).unwrap();
    record.push_hash32(7, receiver.as_bytes()).unwrap();
    record.push_u128(8, value).unwrap();
    record.push_bytes(9, payload).unwrap();
    record.push_u32(10, chain_id).unwrap();
    record.push_u16(11, schema as u16).unwrap();
    record
}

/// Build the canonical BlockCoreV2 record.
#[allow(clippy::too_many_arguments)]
pub fn block_core_v2(
    version: u8,
    parent_block_id: BlockIdV2,
    state_root: HashV2,
    transactions_root: HashV2,
    timestamp: u64,
    difficulty: u64,
    chain_work: u128,
    validator_id: HashV2,
    miner_id: HashV2,
    height: u64,
    state_parent: BlockIdV2, // NEW PARAMETER: state parent for branch-scoped execution
) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_u8(1, version).unwrap();
    record.push_hash32(2, parent_block_id.as_bytes()).unwrap();
    record.push_hash32(3, state_root.as_bytes()).unwrap();
    record.push_hash32(4, transactions_root.as_bytes()).unwrap();
    record.push_u64(5, timestamp).unwrap();
    record.push_u64(6, difficulty).unwrap();
    record.push_u128(7, chain_work).unwrap();
    record.push_hash32(8, validator_id.as_bytes()).unwrap();
    record.push_hash32(9, miner_id.as_bytes()).unwrap();
    record.push_u64(10, height).unwrap();
    record.push_hash32(11, state_parent.as_bytes()).unwrap(); // NEW FIELD: state_parent
    record
}

/// Build the canonical PoW work record (block_core_hash + nonce).
pub fn pow_work_record(block_core_hash: HashV2, nonce: u64) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_hash32(1, block_core_hash.as_bytes()).unwrap();
    record.push_u64(2, nonce).unwrap();
    record
}

/// Build the canonical Merkle leaf record from a TxIdV2.
pub fn tx_merkle_leaf(tx_id: crate::hash::TxIdV2) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_hash32(1, tx_id.as_bytes()).unwrap();
    record
}

/// Build the canonical Merkle internal node record.
pub fn tx_merkle_node(left: crate::hash::TxIdV2, right: crate::hash::TxIdV2) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_hash32(1, left.as_bytes()).unwrap();
    record.push_hash32(2, right.as_bytes()).unwrap();
    record
}

/// Build the canonical Merkle root record.
pub fn tx_merkle_root(root_hash: crate::hash::TxIdV2) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_hash32(1, root_hash.as_bytes()).unwrap();
    record
}

/// Build the canonical StateRoot record.
pub fn state_root_record(root: HashV2) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_hash32(1, root.as_bytes()).unwrap();
    record
}

// ML-DSA-65 (Dilithium3) public key length in bytes.
const PQ_PUBLIC_KEY_LENGTH: usize = 1952;

/// Build the canonical AccountId record from ML-DSA-65 public key.
pub fn account_id_record(pubkey: &[u8; PQ_PUBLIC_KEY_LENGTH]) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_bytes(1, pubkey).unwrap();
    record
}

/// Build the canonical ValidatorId record from ML-DSA-65 public key.
pub fn validator_id_record(pubkey: &[u8; PQ_PUBLIC_KEY_LENGTH]) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_bytes(1, pubkey).unwrap();
    record
}
