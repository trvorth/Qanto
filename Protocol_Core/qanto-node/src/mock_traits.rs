//! Mock implementations for testing expensive operations
//! This module provides mock implementations that can be used in tests

use crate::qanto_storage::{QantoStorageError, StorageStats, WriteBatch};
use crate::storage_traits::{CryptoOperations, NetworkOperations, StorageOperations};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Mock storage implementation for testing
#[derive(Debug, Clone)]
pub struct MockStorage {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    stats: Arc<Mutex<StorageStats>>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(StorageStats {
                total_keys: 0,
                total_size: 0,
                cache_hits: 0,
                cache_misses: 0,
                compactions: 0,
                writes: 0,
                reads: 0,
                deletes: 0,
            })),
        }
    }
}

// Default implementation for MockStorage - calls Self::new() for consistent initialization
impl Default for MockStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageOperations for MockStorage {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, QantoStorageError> {
        let data = self.data.lock().unwrap();
        Ok(data.get(key).cloned())
    }

    async fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), QantoStorageError> {
        let mut data = self.data.lock().unwrap();
        data.insert(key, value);
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<(), QantoStorageError> {
        let mut data = self.data.lock().unwrap();
        data.remove(key);
        Ok(())
    }

    async fn contains_key(&self, key: &[u8]) -> Result<bool, QantoStorageError> {
        let data = self.data.lock().unwrap();
        Ok(data.contains_key(key))
    }

    async fn write_batch(&self, batch: WriteBatch) -> Result<(), QantoStorageError> {
        let mut data = self.data.lock().unwrap();
        for operation in batch.operations {
            match operation {
                crate::qanto_storage::LogEntry::Put { key, value } => {
                    data.insert(key, value);
                }
                crate::qanto_storage::LogEntry::Delete { key } => {
                    data.remove(&key);
                }
                _ => {} // Ignore other log entry types for mock
            }
        }
        Ok(())
    }

    async fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, QantoStorageError> {
        let data = self.data.lock().unwrap();
        let keys: Vec<Vec<u8>> = data
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }

    async fn begin_transaction(&self) -> u64 {
        1 // Mock transaction ID
    }

    async fn commit_transaction(&self, _tx_id: u64) -> Result<(), QantoStorageError> {
        Ok(())
    }

    async fn rollback_transaction(&self, _tx_id: u64) -> Result<(), QantoStorageError> {
        Ok(())
    }

    async fn stats(&self) -> StorageStats {
        self.stats.lock().unwrap().clone()
    }

    async fn flush(&self) -> Result<(), QantoStorageError> {
        Ok(())
    }

    async fn sync(&self) -> Result<(), QantoStorageError> {
        Ok(())
    }
}

/// Mock crypto implementation for testing
#[derive(Debug, Clone)]
pub struct MockCrypto;

impl MockCrypto {
    pub fn new() -> Self {
        Self
    }
}

// Default implementation for MockCrypto - calls Self::new() for consistent initialization
impl Default for MockCrypto {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CryptoOperations for MockCrypto {
    async fn hash(&self, data: &[u8]) -> Vec<u8> {
        // Simple mock hash - just return first 32 bytes or pad with zeros
        let mut hash = vec![0u8; 32];
        let len = std::cmp::min(data.len(), 32);
        hash[..len].copy_from_slice(&data[..len]);
        hash
    }

    async fn verify_signature(
        &self,
        _message: &[u8],
        _signature: &[u8],
        _public_key: &[u8],
    ) -> bool {
        true // Mock always returns true
    }

    async fn sign(&self, message: &[u8], _private_key: &[u8]) -> Result<Vec<u8>, String> {
        // Mock signature - just return hash of message
        Ok(self.hash(message).await)
    }

    async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>), String> {
        // Mock keypair
        let private_key = vec![1u8; 32];
        let public_key = vec![2u8; 32];
        Ok((private_key, public_key))
    }

    async fn derive_key(&self, _password: &str, _salt: &[u8]) -> Result<Vec<u8>, String> {
        Ok(vec![3u8; 32])
    }

    async fn encrypt(&self, data: &[u8], _key: &[u8]) -> Result<Vec<u8>, String> {
        // Mock encryption - just XOR with 0x42
        let encrypted: Vec<u8> = data.iter().map(|b| b ^ 0x42).collect();
        Ok(encrypted)
    }

    async fn decrypt(&self, encrypted_data: &[u8], _key: &[u8]) -> Result<Vec<u8>, String> {
        // Mock decryption - reverse the XOR
        let decrypted: Vec<u8> = encrypted_data.iter().map(|b| b ^ 0x42).collect();
        Ok(decrypted)
    }
}

/// Mock network implementation for testing
#[derive(Debug, Clone)]
pub struct MockNetwork {
    peers: Arc<Mutex<Vec<String>>>,
}

impl MockNetwork {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

// Default implementation for MockNetwork - calls Self::new() for consistent initialization
impl Default for MockNetwork {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetworkOperations for MockNetwork {
    async fn send_to_peer(&self, _peer_id: &str, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }

    async fn broadcast(&self, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }

    async fn get_peers(&self) -> Vec<String> {
        self.peers.lock().unwrap().clone()
    }

    async fn connect_peer(&self, address: &str) -> Result<String, String> {
        // Fixed: inline format argument for cleaner code
        let peer_id = format!("peer_{address}");
        let mut peers = self.peers.lock().unwrap();
        peers.push(peer_id.clone());
        Ok(peer_id)
    }

    async fn disconnect_peer(&self, peer_id: &str) -> Result<(), String> {
        let mut peers = self.peers.lock().unwrap();
        peers.retain(|p| p != peer_id);
        Ok(())
    }

    async fn is_connected(&self, peer_id: &str) -> bool {
        let peers = self.peers.lock().unwrap();
        peers.contains(&peer_id.to_string())
    }
}

#[cfg(feature = "zk")]
use crate::zkp::{ZKProof, ZKProofType};

/// Mock ZK Proof System for fast testing
#[cfg(feature = "zk")]
#[derive(Debug, Clone)]
pub struct MockZKProofSystem {
    /// Mock proof cache
    proof_cache: Arc<RwLock<HashMap<String, ZKProof>>>,
    /// Initialization state
    initialized: Arc<RwLock<bool>>,
}

#[cfg(feature = "zk")]
impl Default for MockZKProofSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl MockZKProofSystem {
    pub fn new() -> Self {
        Self {
            proof_cache: Arc::new(RwLock::new(HashMap::new())),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Mock initialization - instant completion
    pub async fn initialize(&self) -> Result<()> {
        let mut init = self.initialized.write().await;
        *init = true;
        Ok(())
    }

    /// Generate mock range proof - instant completion
    pub async fn generate_range_proof(&self, value: u64, min: u64, max: u64) -> Result<ZKProof> {
        let proof = ZKProof {
            proof: vec![0u8; 32], // Mock proof bytes
            public_inputs: vec![vec![min as u8], vec![max as u8]],
            proof_type: ZKProofType::RangeProof,
            vk_hash: vec![0u8; 32],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        // Cache the proof
        let proof_id = format!("range_{}_{}_{}_{}", value, min, max, proof.timestamp);
        let mut cache = self.proof_cache.write().await;
        cache.insert(proof_id, proof.clone());

        Ok(proof)
    }

    /// Generate mock balance proof - instant completion
    pub async fn generate_balance_proof(
        &self,
        inputs: Vec<u64>,
        outputs: Vec<u64>,
    ) -> Result<ZKProof> {
        let proof = ZKProof {
            proof: vec![1u8; 32], // Mock proof bytes
            public_inputs: vec![
                inputs.iter().map(|&x| x as u8).collect(),
                outputs.iter().map(|&x| x as u8).collect(),
            ],
            proof_type: ZKProofType::BalanceProof,
            vk_hash: vec![1u8; 32],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        let proof_id = format!("balance_{:?}_{:?}_{}", inputs, outputs, proof.timestamp);
        let mut cache = self.proof_cache.write().await;
        cache.insert(proof_id, proof.clone());

        Ok(proof)
    }

    /// Generate mock computation proof - instant completion
    pub async fn generate_computation_proof(
        &self,
        private_inputs: Vec<u64>,
        public_inputs: Vec<u64>,
        expected_output: u64,
        computation_type: u8,
    ) -> Result<ZKProof> {
        let proof = ZKProof {
            proof: vec![2u8; 32], // Mock proof bytes
            public_inputs: vec![
                public_inputs.iter().map(|&x| x as u8).collect(),
                vec![expected_output as u8],
                vec![computation_type],
            ],
            proof_type: ZKProofType::ComputationProof,
            vk_hash: vec![2u8; 32],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        let proof_id = format!(
            "computation_{:?}_{:?}_{}_{}",
            private_inputs, public_inputs, expected_output, proof.timestamp
        );
        let mut cache = self.proof_cache.write().await;
        cache.insert(proof_id, proof.clone());

        Ok(proof)
    }

    /// Generate mock identity proof - instant completion
    pub async fn generate_identity_proof(
        &self,
        identity_key: u64,
        identity_commitment: u64,
        nonce: u64,
        _age: u64,
        age_threshold: u64,
    ) -> Result<ZKProof> {
        let proof = ZKProof {
            proof: vec![3u8; 32], // Mock proof bytes
            public_inputs: vec![vec![identity_commitment as u8], vec![age_threshold as u8]],
            proof_type: ZKProofType::IdentityProof,
            vk_hash: vec![3u8; 32],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        let proof_id = format!(
            "identity_{}_{}_{}_{}",
            identity_key, identity_commitment, nonce, proof.timestamp
        );
        let mut cache = self.proof_cache.write().await;
        cache.insert(proof_id, proof.clone());

        Ok(proof)
    }

    /// Generate mock voting proof - instant completion
    pub async fn generate_voting_proof(
        &self,
        voter_key: u64,
        nullifier: u64,
        _vote: u64,
        election_pubkey: u64,
    ) -> Result<ZKProof> {
        let proof = ZKProof {
            proof: vec![4u8; 32], // Mock proof bytes
            public_inputs: vec![vec![nullifier as u8], vec![election_pubkey as u8]],
            proof_type: ZKProofType::VotingProof,
            vk_hash: vec![4u8; 32],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        let proof_id = format!("voting_{}_{}_{}", voter_key, nullifier, proof.timestamp);
        let mut cache = self.proof_cache.write().await;
        cache.insert(proof_id, proof.clone());

        Ok(proof)
    }

    /// Mock proof verification - always returns true for valid mock proofs
    pub async fn verify_proof(&self, proof: &ZKProof) -> Result<bool> {
        // Mock verification: check if proof has expected mock structure
        let is_valid = !proof.proof.is_empty() && !proof.vk_hash.is_empty() && proof.timestamp > 0;
        Ok(is_valid)
    }
}
