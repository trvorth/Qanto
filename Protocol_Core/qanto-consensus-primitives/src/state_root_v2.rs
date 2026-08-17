//! V2 state root derivation.

use crate::{
    consensus_parameters::SchemaVersion,
    hash::{hash_v2, DomainV2, HashV2, StateRootV2},
    record::{CanonicalRecord, DecodedRecord},
    v2_records::{state_root_record, STATE_ROOT_SPECS},
};

/// State root record payload (authenticated state tree root).
/// For now commits to the state tree root hash. Workstream 3 extends this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateRootRecordV2 {
    pub root_hash: HashV2,
}

impl StateRootRecordV2 {
    pub fn new(root_hash: HashV2) -> Self {
        Self { root_hash }
    }

    pub fn to_canonical_record(&self) -> CanonicalRecord {
        state_root_record(self.root_hash)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, crate::record::RecordError> {
        let decoded = DecodedRecord::decode_with_schema(bytes, STATE_ROOT_SPECS)?;
        let root_field = decoded
            .field(1)
            .ok_or(crate::record::RecordError::MissingRequiredField { field_id: 1 })?;
        let root = HashV2::from_bytes(root_field.value.try_into().map_err(|_| {
            crate::record::RecordError::UnexpectedEof {
                offset: 0,
                needed: 32,
            }
        })?);
        Ok(Self { root_hash: root })
    }
}

/// Derive StateRootV2 from a state tree root hash.
/// StateRootV2 = hash_v2(DomainV2::StateRoot, chain_id, schema, canonical_record(root_hash))
pub fn state_root_v2(root_hash: HashV2, chain_id: u32, schema: SchemaVersion) -> StateRootV2 {
    let record = state_root_record(root_hash);
    let bytes = record.finish();
    StateRootV2::from_hash(hash_v2(
        DomainV2::StateRoot,
        chain_id,
        schema as u16,
        &bytes,
    ))
}
