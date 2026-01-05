// src/types.rs

//! --- Qanto Types ---
//! v0.1.0 - Initial Version
//!
//! This version defines basic types used throughout the Qanto system, including
//! addresses, hashes, UTXOs, and post-quantum signatures.
use crate::post_quantum_crypto::{pq_sign, pq_verify};
use qanto_core::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey, QantoPQSignature};
use serde::{Deserialize, Serialize};
#[cfg(feature = "tfhe")]
use tfhe::{generate_keys, prelude::FheDecrypt, ConfigBuilder, FheUint64};

// GraphQL types
pub type Address = String;
pub type Hash = String;

/// WalletBalance represents a minimal, maturity-aware balance snapshot for an address.
///
/// Fields are expressed in base units (smallest divisible unit of QANTO) to avoid
/// cross-crate constants and rounding ambiguity. Consumers can format values into
/// human-readable QANTO amounts using their own unit constants.
///
/// Examples
///
/// ```rust
/// use qanto::types::WalletBalance;
/// // Construct from raw components (all values are base units)
/// let wb = WalletBalance {
///     spendable_confirmed: 1_000_000u64,
///     immature_coinbase_confirmed: 250_000u64,
///     unconfirmed_delta: 50_000u64,
///     total_confirmed: 1_250_000u64,
/// };
/// assert_eq!(wb.total_confirmed, wb.spendable_confirmed + wb.immature_coinbase_confirmed);
///
/// // Convert from a tuple returned by RPC backends
/// let wb2: WalletBalance = (900, 100, 10, 1000).into();
/// assert_eq!(wb2.spendable_confirmed, 900);
/// assert_eq!(wb2.immature_coinbase_confirmed, 100);
/// assert_eq!(wb2.unconfirmed_delta, 10);
/// assert_eq!(wb2.total_confirmed, 1000);
/// ```
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct WalletBalance {
    /// Confirmed and immediately spendable funds (excludes immature coinbase outputs)
    pub spendable_confirmed: u64,
    /// Confirmed but not yet spendable coinbase outputs (awaiting finality/maturity)
    pub immature_coinbase_confirmed: u64,
    /// Net delta from unconfirmed transactions in mempool (incoming - outgoing)
    pub unconfirmed_delta: u64,
    /// Total confirmed balance (spendable + immature_coinbase)
    pub total_confirmed: u64,
}

impl From<(u64, u64, u64, u64)> for WalletBalance {
    fn from(t: (u64, u64, u64, u64)) -> Self {
        Self {
            spendable_confirmed: t.0,
            immature_coinbase_confirmed: t.1,
            unconfirmed_delta: t.2,
            total_confirmed: t.3,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UTXO {
    pub address: String,
    pub amount: u64,
    pub tx_id: String,
    pub output_index: u32,
    pub explorer_link: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct QuantumResistantSignature {
    pub signer_public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

impl QuantumResistantSignature {
    pub fn sign(signing_key: &QantoPQPrivateKey, message: &[u8]) -> Result<Self, String> {
        let signature = pq_sign(signing_key, message).map_err(|e| e.to_string())?;
        Ok(Self {
            signer_public_key: signing_key.public_key().as_bytes().to_vec(),
            signature: signature.as_bytes().to_vec(),
        })
    }

    pub fn verify(&self, message: &[u8]) -> bool {
        let Ok(pk) = QantoPQPublicKey::from_bytes(&self.signer_public_key) else {
            return false;
        };
        let Ok(signature) = QantoPQSignature::from_bytes(&self.signature) else {
            return false;
        };
        pq_verify(&pk, message, &signature).unwrap_or(false)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct HomomorphicEncrypted {
    pub ciphertext: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl HomomorphicEncrypted {
    pub fn new(_amount: u64, public_key_material: &[u8]) -> Self {
        // For testing and development, use empty data to avoid 437MB serialization issue
        // TODO: In production, implement proper TFHE encryption with size limits
        Self {
            ciphertext: vec![], // Empty to avoid massive TFHE serialization
            public_key: public_key_material.to_vec(),
        }
    }

    #[cfg(not(feature = "tfhe"))]
    pub fn decrypt(&self, _private_key_material: &[u8]) -> Result<u64, String> {
        Err("Homomorphic decrypt unavailable (tfhe feature disabled)".to_string())
    }

    #[cfg(feature = "tfhe")]
    pub fn decrypt(&self, _private_key_material: &[u8]) -> Result<u64, String> {
        // For TFHE decryption, we need the client key
        // In a real implementation, the private_key_material would contain the serialized client key
        let config = ConfigBuilder::default().build();
        let (client_key, _server_key) = generate_keys(config);

        // Deserialize the encrypted value
        let encrypted_amount: FheUint64 = bincode::deserialize(&self.ciphertext)
            .map_err(|e| format!("Failed to deserialize ciphertext: {e}"))?;

        // Decrypt the value
        let decrypted_amount: u64 = encrypted_amount.decrypt(&client_key);

        Ok(decrypted_amount)
    }

    #[cfg(not(feature = "tfhe"))]
    pub fn add(&self, _other: &Self) -> Result<Self, String> {
        Err("Homomorphic add unavailable (tfhe feature disabled)".to_string())
    }

    #[cfg(feature = "tfhe")]
    pub fn add(&self, other: &Self) -> Result<Self, String> {
        // For homomorphic addition, we need the server key
        let config = ConfigBuilder::default().build();
        let (_client_key, _server_key) = generate_keys(config);

        // Deserialize both encrypted values
        let encrypted_a: FheUint64 = bincode::deserialize(&self.ciphertext)
            .map_err(|e| format!("Failed to deserialize first ciphertext: {e}"))?;
        let encrypted_b: FheUint64 = bincode::deserialize(&other.ciphertext)
            .map_err(|e| format!("Failed to deserialize second ciphertext: {e}"))?;

        // Perform homomorphic addition
        let result = &encrypted_a + &encrypted_b;

        // Serialize the result
        let result_ciphertext =
            bincode::serialize(&result).map_err(|e| format!("Failed to serialize result: {e}"))?;

        Ok(Self {
            ciphertext: result_ciphertext,
            public_key: self.public_key.clone(),
        })
    }

    pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
        // For testing and development, use minimal dummy keys to avoid massive TFHE serialization
        // TODO: In production, implement proper TFHE keypair generation with size limits
        let public_key = vec![0x42u8; 32]; // 32-byte dummy public key
        let private_key = vec![0x24u8; 32]; // 32-byte dummy private key
        (public_key, private_key)
    }
}
