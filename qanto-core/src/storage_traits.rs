//! Mockable traits for expensive operations
//! This module defines traits that can be mocked for testing to avoid expensive operations

use crate::qanto_storage::{QantoStorageError, StorageStats, WriteBatch};
use async_trait::async_trait;

/// Trait for storage operations that can be mocked
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait StorageOperations: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError>;

    /// Put a key-value pair
    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError>;

    /// Delete a key
    async fn delete(&self, key: &[u8]) -> Result<(), QantoStorageError>;

    /// Check if a key exists
    async fn contains_key(&self, key: &[u8]) -> Result<bool, QantoStorageError>;

    /// Write a batch of operations
    async fn write_batch(&self, batch: WriteBatch) -> Result<(), QantoStorageError>;

    /// Get keys with a prefix
    async fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, QantoStorageError>;

    /// Begin a transaction
    async fn begin_transaction(&self) -> u64;

    /// Commit a transaction
    async fn commit_transaction(&self, tx_id: u64) -> Result<(), QantoStorageError>;

    /// Rollback a transaction
    async fn rollback_transaction(&self, tx_id: u64) -> Result<(), QantoStorageError>;

    /// Get storage statistics
    async fn stats(&self) -> StorageStats;

    /// Flush data to disk
    async fn flush(&self) -> Result<(), QantoStorageError>;

    /// Sync data to disk
    async fn sync(&self) -> Result<(), QantoStorageError>;
}

/// Trait for cryptographic operations that can be mocked
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CryptoOperations: Send + Sync {
    /// Hash data using the configured hash function
    async fn hash(&self, data: &[u8]) -> Vec<u8>;

    /// Verify a signature
    async fn verify_signature(&self, message: &[u8], signature: &[u8], public_key: &[u8]) -> bool;

    /// Generate a signature
    async fn sign(&self, message: &[u8], private_key: &[u8]) -> Result<Vec<u8>, String>;

    /// Generate a key pair
    async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>), String>; // (private, public)

    /// Derive a key from password
    async fn derive_key(&self, password: &str, salt: &[u8]) -> Result<Vec<u8>, String>;

    /// Encrypt data
    async fn encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>, String>;

    /// Decrypt data
    async fn decrypt(&self, encrypted_data: &[u8], key: &[u8]) -> Result<Vec<u8>, String>;
}

/// Trait for network operations that can be mocked
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait NetworkOperations: Send + Sync {
    /// Send data to a peer
    async fn send_to_peer(&self, peer_id: &str, data: &[u8]) -> Result<(), String>;

    /// Broadcast data to all peers
    async fn broadcast(&self, data: &[u8]) -> Result<(), String>;

    /// Get connected peers
    async fn get_peers(&self) -> Vec<String>;

    /// Connect to a peer
    async fn connect_peer(&self, address: &str) -> Result<String, String>; // Returns peer_id

    /// Disconnect from a peer
    async fn disconnect_peer(&self, peer_id: &str) -> Result<(), String>;

    /// Check if connected to a peer
    async fn is_connected(&self, peer_id: &str) -> bool;
}
