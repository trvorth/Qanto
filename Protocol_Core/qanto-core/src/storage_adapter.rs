//! Storage adapter implementations
//! This module provides an adapter that implements the storage traits for QantoStorage

use crate::qanto_native_crypto::{QantoNativeCrypto, QantoSignatureAlgorithm};
use crate::qanto_storage::{QantoStorage, QantoStorageError, StorageStats, WriteBatch};
use crate::storage_traits::{CryptoOperations, NetworkOperations, StorageOperations};
use async_trait::async_trait;
use rand::thread_rng;

/// Adapter for QantoStorage to implement StorageOperations trait
pub struct QantoStorageAdapter {
    storage: QantoStorage,
}

impl QantoStorageAdapter {
    pub fn new(storage: QantoStorage) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl StorageOperations for QantoStorageAdapter {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError> {
        self.storage.get(key)
    }

    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError> {
        self.storage.put(key, value)
    }

    async fn delete(&self, key: &[u8]) -> Result<(), QantoStorageError> {
        self.storage.delete(key)
    }

    async fn contains_key(&self, key: &[u8]) -> Result<bool, QantoStorageError> {
        self.storage.contains_key(key)
    }

    async fn write_batch(&self, batch: WriteBatch) -> Result<(), QantoStorageError> {
        self.storage.write_batch(batch)
    }

    async fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, QantoStorageError> {
        self.storage.keys_with_prefix(prefix)
    }

    async fn begin_transaction(&self) -> u64 {
        self.storage.begin_transaction()
    }

    async fn commit_transaction(&self, tx_id: u64) -> Result<(), QantoStorageError> {
        self.storage.commit_transaction(tx_id)
    }

    async fn rollback_transaction(&self, tx_id: u64) -> Result<(), QantoStorageError> {
        self.storage.rollback_transaction(tx_id)
    }

    async fn stats(&self) -> StorageStats {
        self.storage.stats()
    }

    async fn flush(&self) -> Result<(), QantoStorageError> {
        self.storage.flush()
    }

    async fn sync(&self) -> Result<(), QantoStorageError> {
        self.storage.sync()
    }
}

/// Adapter for cryptographic operations
pub struct QantoCryptoAdapter {
    crypto: QantoNativeCrypto,
}

impl QantoCryptoAdapter {
    pub fn new() -> Self {
        Self {
            crypto: QantoNativeCrypto::new(),
        }
    }
}

// Default implementation for QantoCryptoAdapter - calls Self::new() for consistent initialization
impl Default for QantoCryptoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CryptoOperations for QantoCryptoAdapter {
    async fn hash(&self, data: &[u8]) -> Vec<u8> {
        let h = my_blockchain::qanto_hash(data);
        h.as_bytes().to_vec()
    }

    async fn verify_signature(&self, message: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
        // Use Ed25519 for simplicity in the adapter
        if let (Ok(pub_key), Ok(sig)) = (
            crate::qanto_native_crypto::QantoEd25519PublicKey::from_bytes(public_key),
            crate::qanto_native_crypto::QantoEd25519Signature::from_bytes(signature),
        ) {
            pub_key.verify(message, &sig).is_ok()
        } else {
            false
        }
    }

    async fn sign(&self, message: &[u8], private_key: &[u8]) -> Result<Vec<u8>, String> {
        let priv_key = crate::qanto_native_crypto::QantoEd25519PrivateKey::from_bytes(private_key)
            .map_err(|e| e.to_string())?;
        let signature = priv_key.sign(message).map_err(|e| e.to_string())?;
        Ok(signature.0.to_vec())
    }

    async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>), String> {
        let mut rng = thread_rng();
        let keypair = self
            .crypto
            .generate_keypair(QantoSignatureAlgorithm::Ed25519, &mut rng);

        match keypair {
            crate::qanto_native_crypto::QantoKeyPair::Ed25519 { public, private } => {
                Ok((private.as_bytes().to_vec(), public.as_bytes().to_vec()))
            }
            _ => Err("Unexpected keypair type".to_string()),
        }
    }

    async fn derive_key(&self, password: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
        use argon2::password_hash::{PasswordHasher, SaltString};
        use argon2::Argon2;

        let salt_string = SaltString::encode_b64(salt).map_err(|e| e.to_string())?;
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt_string)
            .map_err(|e| e.to_string())?;

        Ok(password_hash.hash.unwrap().as_bytes().to_vec())
    }

    async fn encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
        // Simple XOR encryption for demonstration
        // In production, use proper encryption like AES-GCM
        if key.len() < 32 {
            return Err("Key too short".to_string());
        }

        let mut encrypted = Vec::with_capacity(data.len());
        for (i, &byte) in data.iter().enumerate() {
            encrypted.push(byte ^ key[i % key.len()]);
        }
        Ok(encrypted)
    }

    async fn decrypt(&self, encrypted_data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
        // XOR decryption (same as encryption for XOR)
        self.encrypt(encrypted_data, key).await
    }
}

/// Adapter for network operations
pub struct QantoNetworkAdapter;

impl QantoNetworkAdapter {
    pub fn new() -> Self {
        Self
    }
}

// Default implementation for QantoNetworkAdapter - calls Self::new() for consistent initialization
impl Default for QantoNetworkAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetworkOperations for QantoNetworkAdapter {
    async fn send_to_peer(&self, _peer_id: &str, _data: &[u8]) -> Result<(), String> {
        // Implement actual network sending
        // For now, just simulate success
        Ok(())
    }

    async fn broadcast(&self, _data: &[u8]) -> Result<(), String> {
        // Implement actual broadcasting
        // For now, just simulate success
        Ok(())
    }

    async fn get_peers(&self) -> Vec<String> {
        // Return actual connected peers
        // For now, return empty list
        vec![]
    }

    async fn connect_peer(&self, address: &str) -> Result<String, String> {
        // Implement actual peer connection
        // For now, just return a mock peer ID
        Ok(format!("peer_{}", address.replace(":", "_")))
    }

    async fn disconnect_peer(&self, _peer_id: &str) -> Result<(), String> {
        // Implement actual peer disconnection
        // For now, just simulate success
        Ok(())
    }

    async fn is_connected(&self, _peer_id: &str) -> bool {
        // Check actual connection status
        // For now, just return false
        false
    }
}
