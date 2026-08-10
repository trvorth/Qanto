//! Canonical protocol identities derived from post-quantum public keys.
//!
//! Version 2 identities use SHA3-256 over an unambiguous, domain-separated
//! preimage. The network chain identifier, signature suite, and public-key
//! length are committed so the same key cannot be replayed across domains,
//! networks, suites, or ambiguous concatenations.

use crate::qanto_native_crypto::QANTO_PQ_PUBLIC_KEY_LENGTH;
use sha3::{Digest, Sha3_256};
use subtle::ConstantTimeEq;
use thiserror::Error;

const ACCOUNT_DOMAIN_V2: &[u8] = b"QANTO\0ACCOUNT\0";
const VALIDATOR_DOMAIN_V2: &[u8] = b"QANTO\0VALIDATOR\0";

/// Canonical identity scheme version.
pub const IDENTITY_VERSION_V2: u16 = 2;

/// Signature-suite identifier for the current 1,952-byte ML-DSA-65 key.
pub const ML_DSA_65_SUITE_ID: u16 = 0x0201;

/// Errors returned while deriving or validating a protocol identity.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityError {
    /// The supplied public key is not the canonical ML-DSA-65 length.
    #[error("invalid ML-DSA-65 public-key length: expected {expected}, got {actual}")]
    InvalidPublicKeyLength { expected: usize, actual: usize },

    /// The address is not the canonical 32-byte lowercase hexadecimal form.
    #[error("identity must be exactly 64 lowercase hexadecimal characters")]
    NonCanonicalAddress,

    /// The declared address was not derived from the supplied public key.
    #[error("declared identity is not bound to the supplied public key")]
    AddressMismatch,
}

/// Derive the canonical account address for an ML-DSA-65 public key.
pub fn account_address_v2(
    network_chain_id: u32,
    public_key: &[u8],
) -> Result<String, IdentityError> {
    derive_identity_v2(ACCOUNT_DOMAIN_V2, network_chain_id, public_key)
}

/// Derive the canonical validator identity for an ML-DSA-65 public key.
///
/// Validators and accounts deliberately use distinct domains so an identifier
/// cannot silently change roles without an explicit protocol rule.
pub fn validator_id_v2(network_chain_id: u32, public_key: &[u8]) -> Result<String, IdentityError> {
    derive_identity_v2(VALIDATOR_DOMAIN_V2, network_chain_id, public_key)
}

/// Validate an account address without leaking mismatch position information.
pub fn validate_account_address_v2(
    declared_address: &str,
    network_chain_id: u32,
    public_key: &[u8],
) -> Result<(), IdentityError> {
    let declared = decode_canonical_address(declared_address)?;
    let expected = derive_identity_bytes_v2(ACCOUNT_DOMAIN_V2, network_chain_id, public_key)?;

    if bool::from(declared.ct_eq(&expected)) {
        Ok(())
    } else {
        Err(IdentityError::AddressMismatch)
    }
}

fn derive_identity_v2(
    domain: &[u8],
    network_chain_id: u32,
    public_key: &[u8],
) -> Result<String, IdentityError> {
    derive_identity_bytes_v2(domain, network_chain_id, public_key).map(hex::encode)
}

fn derive_identity_bytes_v2(
    domain: &[u8],
    network_chain_id: u32,
    public_key: &[u8],
) -> Result<[u8; 32], IdentityError> {
    if public_key.len() != QANTO_PQ_PUBLIC_KEY_LENGTH {
        return Err(IdentityError::InvalidPublicKeyLength {
            expected: QANTO_PQ_PUBLIC_KEY_LENGTH,
            actual: public_key.len(),
        });
    }

    let mut hasher = Sha3_256::new();
    hasher.update(domain);
    hasher.update(IDENTITY_VERSION_V2.to_be_bytes());
    hasher.update(network_chain_id.to_be_bytes());
    hasher.update(ML_DSA_65_SUITE_ID.to_be_bytes());
    hasher.update((public_key.len() as u32).to_be_bytes());
    hasher.update(public_key);

    Ok(hasher.finalize().into())
}

fn decode_canonical_address(address: &str) -> Result<[u8; 32], IdentityError> {
    if address.len() != 64
        || !address
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(IdentityError::NonCanonicalAddress);
    }

    let mut decoded = [0u8; 32];
    hex::decode_to_slice(address, &mut decoded).map_err(|_| IdentityError::NonCanonicalAddress)?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_address_v2_has_stable_golden_vector() {
        let public_key = [7u8; QANTO_PQ_PUBLIC_KEY_LENGTH];
        let address = account_address_v2(1234, &public_key).unwrap();

        assert_eq!(
            address,
            "d45c7ef8ce3931b25ca2ea68342551b642ca4f5e1ad64d29c8e8629ecb7d30c8"
        );
    }

    #[test]
    fn identities_are_domain_and_network_separated() {
        let public_key = [3u8; QANTO_PQ_PUBLIC_KEY_LENGTH];

        assert_ne!(
            account_address_v2(1234, &public_key).unwrap(),
            validator_id_v2(1234, &public_key).unwrap()
        );
        assert_ne!(
            account_address_v2(1234, &public_key).unwrap(),
            account_address_v2(1235, &public_key).unwrap()
        );
    }

    #[test]
    fn validation_rejects_malformed_keys_and_noncanonical_addresses() {
        assert!(matches!(
            account_address_v2(1234, &[0u8; 32]),
            Err(IdentityError::InvalidPublicKeyLength { .. })
        ));

        let public_key = [5u8; QANTO_PQ_PUBLIC_KEY_LENGTH];
        let canonical = account_address_v2(1234, &public_key).unwrap();
        validate_account_address_v2(&canonical, 1234, &public_key).unwrap();
        assert_eq!(
            validate_account_address_v2(&canonical.to_uppercase(), 1234, &public_key),
            Err(IdentityError::NonCanonicalAddress)
        );
        assert_eq!(
            validate_account_address_v2(&"0".repeat(64), 1234, &public_key),
            Err(IdentityError::AddressMismatch)
        );
    }
}
