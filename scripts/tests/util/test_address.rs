//! Test address utilities for generating valid miner addresses in tests.
//!
//! This module provides deterministic address generation for testing purposes,
//! ensuring all test addresses conform to the 64-character hexadecimal format
//! required by the miner validation logic.

use qanto::wallet::Wallet;

/// Generates a deterministic test miner address.
///
/// This function creates a wallet from a fixed seed to ensure deterministic
/// test behavior while producing a valid 64-character hexadecimal address
/// that passes miner validation.
///
/// # Returns
///
/// A valid 64-character hexadecimal string suitable for use as a miner address.
///
/// # Panics
///
/// Panics if wallet creation fails, which should not happen in test environments.
pub fn make_test_miner_address() -> String {
    // Use a fixed private key for deterministic test behavior
    // This is a valid 64-character hex string that will generate a consistent address
    let test_private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    let wallet = Wallet::from_private_key(test_private_key)
        .expect("Failed to create test wallet from fixed private key");

    wallet.address()
}

/// Generates a test miner address with a custom seed.
///
/// This function allows creating different test addresses by varying the seed,
/// useful when tests need multiple distinct miner addresses.
///
/// # Arguments
///
/// * `seed` - A seed value to generate different addresses
///
/// # Returns
///
/// A valid 64-character hexadecimal string suitable for use as a miner address.
///
/// # Panics
///
/// Panics if wallet creation fails, which should not happen in test environments.
pub fn make_test_miner_address_with_seed(seed: u64) -> String {
    // Create a deterministic private key based on the seed
    let mut private_key_bytes = [0u8; 32];
    private_key_bytes[..8].copy_from_slice(&seed.to_le_bytes());

    // Fill the rest with a pattern to ensure it's a valid private key
    for (i, byte) in private_key_bytes.iter_mut().enumerate().skip(8) {
        *byte = ((seed
            .wrapping_mul(0x9e3779b97f4a7c15_u64)
            .wrapping_shr((i * 8) as u32))
            & 0xff) as u8;
    }

    let private_key_hex = hex::encode(private_key_bytes);

    let wallet = Wallet::from_private_key(&private_key_hex)
        .expect("Failed to create test wallet from seeded private key");

    wallet.address()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_test_miner_address_format() {
        let address = make_test_miner_address();

        // Should be exactly 64 characters
        assert_eq!(address.len(), 64);

        // Should be valid hexadecimal
        assert!(address.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_make_test_miner_address_deterministic() {
        let address1 = make_test_miner_address();
        let address2 = make_test_miner_address();

        // Should always return the same address
        assert_eq!(address1, address2);
    }

    #[test]
    fn test_make_test_miner_address_with_seed_format() {
        let address = make_test_miner_address_with_seed(12345);

        // Should be exactly 64 characters
        assert_eq!(address.len(), 64);

        // Should be valid hexadecimal
        assert!(address.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_make_test_miner_address_with_seed_different() {
        let address1 = make_test_miner_address_with_seed(1);
        let address2 = make_test_miner_address_with_seed(2);

        // Different seeds should produce different addresses
        assert_ne!(address1, address2);
    }

    #[test]
    fn test_make_test_miner_address_with_seed_deterministic() {
        let address1 = make_test_miner_address_with_seed(42);
        let address2 = make_test_miner_address_with_seed(42);

        // Same seed should always produce the same address
        assert_eq!(address1, address2);
    }
}
