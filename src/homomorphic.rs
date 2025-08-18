// src/homomorphic.rs

use libpaillier::crypto_bigint::{Encoding, U4096 as BigNumber};
use libpaillier::paillier2048::{DecryptionKey, EncryptionKey};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct HomomorphicEncrypted {
    pub ciphertext: Vec<u8>, // Serialized BigNumber
    pub public_key: Vec<u8>, // Serialized EncryptionKey
}

impl HomomorphicEncrypted {
    #[instrument]
    pub fn new(amount: u64, public_key_material: &[u8]) -> Self {
        let pk: EncryptionKey = bincode::deserialize(public_key_material)
            .expect("Failed to deserialize encryption key");
        // Convert u64 to bytes for encryption
        let amount_bytes = amount.to_be_bytes();
        let res = pk.encrypt(amount_bytes).expect("Encryption failed");
        let ciphertext = res.0;
        Self {
            ciphertext: bincode::serialize(&ciphertext).expect("Serialization failed"),
            public_key: public_key_material.to_vec(),
        }
    }

    pub fn decrypt(&self, private_key_material: &[u8]) -> Result<u64, String> {
        let sk: DecryptionKey = bincode::deserialize(private_key_material)
            .expect("Failed to deserialize decryption key");
        let ciphertext: BigNumber =
            bincode::deserialize(&self.ciphertext).expect("Deserialization failed");
        let bytes = sk.decrypt(&ciphertext).expect("Decryption failed");
        let big_num = BigNumber::from_le_slice(&bytes);
        // Convert to u64 by taking the least significant 8 bytes
        let le_bytes = big_num.to_le_bytes();
        let decrypted = u64::from_le_bytes([
            le_bytes[0],
            le_bytes[1],
            le_bytes[2],
            le_bytes[3],
            le_bytes[4],
            le_bytes[5],
            le_bytes[6],
            le_bytes[7],
        ]);
        Ok(decrypted)
    }

    pub fn add(&self, other: &Self) -> Result<Self, String> {
        let pk: EncryptionKey =
            bincode::deserialize(&self.public_key).expect("Failed to deserialize encryption key");
        let c1: BigNumber = bincode::deserialize(&self.ciphertext).expect("Deserialization failed");
        let c2: BigNumber =
            bincode::deserialize(&other.ciphertext).expect("Deserialization failed");
        let sum = pk.add(&c1, &c2).expect("Homomorphic addition failed");
        Ok(Self {
            ciphertext: bincode::serialize(&sum).expect("Serialization failed"),
            public_key: self.public_key.clone(),
        }
    }

    pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
        let sk = DecryptionKey::random().expect("Key generation failed");
        let pk = EncryptionKey::from(&sk);
        let public_key = bincode::serialize(&pk).expect("Serialization failed");
        let private_key = bincode::serialize(&sk).expect("Serialization failed");
        (public_key, private_key)
    }
}