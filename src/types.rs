// src/types.rs

use libpaillier::crypto_bigint::{Encoding, U4096 as BigNumber};
use libpaillier::paillier2048::{DecryptionKey, EncryptionKey};
use pqcrypto_mldsa::mldsa65 as dilithium5;
use pqcrypto_traits::sign::{PublicKey, SignedMessage};
use serde::{Deserialize, Serialize};
use tracing::instrument;

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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct QuantumResistantSignature {
    pub signer_public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

impl QuantumResistantSignature {
    pub fn sign(
        signing_key: &dilithium5::SecretKey,
        public_key: &dilithium5::PublicKey,
        message: &[u8],
    ) -> Result<Self, String> {
        let signed_message = dilithium5::sign(message, signing_key);
        Ok(Self {
            signer_public_key: public_key.as_bytes().to_vec(),
            signature: signed_message.as_bytes().to_vec(),
        })
    }

    pub fn verify(&self, message: &[u8]) -> bool {
        let Ok(pk) = dilithium5::PublicKey::from_bytes(&self.signer_public_key) else {
            return false;
        };
        let Ok(signed_message) = dilithium5::SignedMessage::from_bytes(&self.signature) else {
            return false;
        };
        match dilithium5::open(&signed_message, &pk) {
            Ok(recovered_message) => recovered_message == message,
            Err(_) => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct HomomorphicEncrypted {
    pub ciphertext: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl HomomorphicEncrypted {
    #[instrument]
    pub fn new(amount: u64, public_key_material: &[u8]) -> Self {
        let pk: EncryptionKey = bincode::deserialize(public_key_material)
            .expect("Failed to deserialize encryption key");
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
        })
    }

    pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
        let sk = DecryptionKey::random().expect("Key generation failed");
        let pk = EncryptionKey::from(&sk);
        let public_key = bincode::serialize(&pk).expect("Serialization failed");
        let private_key = bincode::serialize(&sk).expect("Serialization failed");
        (public_key, private_key)
    }
}
