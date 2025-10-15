// src/types.rs

//! --- Qanto Types ---
//! v0.1.0 - Initial Version
//!
//! This version defines basic types used throughout the Qanto system, including
//! addresses, hashes, UTXOs, and post-quantum signatures.
use crate::post_quantum_crypto::{pq_sign, pq_verify};
use qanto_core::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey, QantoPQSignature};
use serde::{Deserialize, Serialize};
use tfhe::{generate_keys, prelude::FheDecrypt, ConfigBuilder, FheUint64};

// GraphQL types
pub type Address = String;
pub type Hash = String;

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
