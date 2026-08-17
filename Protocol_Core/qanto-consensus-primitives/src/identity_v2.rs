//! V2 identity derivation and constant-time comparison.

use crate::{
    consensus_parameters::SchemaVersion,
    hash::{hash_v2, AccountIdV2, DomainV2, HashV2, ValidatorIdV2},
    v2_records::{account_id_record, validator_id_record},
};

/// ML-DSA-65 public key length in bytes (actual Dilithium3 size).
const PQ_PUBLIC_KEY_LENGTH: usize = 1952;

/// Derive AccountIdV2 from ML-DSA-65 public key.
/// AccountIdV2 = hash_v2(DomainV2::AccountId, chain_id, schema, canonical_record(pubkey))
pub fn account_id_v2(
    pubkey: &[u8; PQ_PUBLIC_KEY_LENGTH],
    chain_id: u32,
    schema: SchemaVersion,
) -> AccountIdV2 {
    let record = account_id_record(pubkey);
    let bytes = record.finish();
    AccountIdV2::from_hash(hash_v2(
        DomainV2::AccountId,
        chain_id,
        schema as u16,
        &bytes,
    ))
}

/// Derive ValidatorIdV2 from ML-DSA-65 public key.
/// ValidatorIdV2 = hash_v2(DomainV2::ValidatorId, chain_id, schema, canonical_record(pubkey))
pub fn validator_id_v2(
    pubkey: &[u8; PQ_PUBLIC_KEY_LENGTH],
    chain_id: u32,
    schema: SchemaVersion,
) -> ValidatorIdV2 {
    let record = validator_id_record(pubkey);
    let bytes = record.finish();
    ValidatorIdV2::from_hash(hash_v2(
        DomainV2::ValidatorId,
        chain_id,
        schema as u16,
        &bytes,
    ))
}

/// Constant-time comparison of two AccountIdV2 or ValidatorIdV2.
/// Uses subtle::ConstantTimeEq on the underlying 32-byte hashes.
pub fn identities_equal_constant_time<T: AsRef<[u8]> + ?Sized>(a: &T, b: &T) -> bool {
    use subtle::ConstantTimeEq;
    a.as_ref().ct_eq(b.as_ref()).into()
}
