//! Mock test helpers for expensive operations

#[cfg(test)]
use qanto::mock_traits::{MockCrypto, MockNetwork, MockStorage};

/// Create a mock storage with common setup
#[cfg(test)]
pub fn create_mock_storage() -> MockStorage {
    MockStorage::new()
}

/// Create an in-memory mock storage for testing
#[cfg(test)]
pub fn create_in_memory_mock_storage() -> MockStorage {
    MockStorage::new()
}

/// Create a mock crypto operations handler
#[cfg(test)]
pub fn create_mock_crypto() -> MockCrypto {
    MockCrypto::new()
}

/// Create a mock network operations handler
#[cfg(test)]
pub fn create_mock_network() -> MockNetwork {
    MockNetwork::new()
}

/// Configuration for mock test environment
#[allow(dead_code)]
pub struct MockTestConfig {
    pub storage: MockStorage,
    pub crypto: MockCrypto,
    pub network: MockNetwork,
}

impl MockTestConfig {
    pub fn new() -> Self {
        Self {
            storage: create_mock_storage(),
            crypto: create_mock_crypto(),
            network: create_mock_network(),
        }
    }

    pub fn with_in_memory_storage() -> Self {
        Self {
            storage: create_in_memory_mock_storage(),
            crypto: create_mock_crypto(),
            network: create_mock_network(),
        }
    }
}

impl Default for MockTestConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qanto::storage_traits::{CryptoOperations, NetworkOperations, StorageOperations};

    #[tokio::test]
    async fn test_mock_storage_operations() {
        let storage = create_mock_storage();
        let key = b"test_key";
        let value = b"test_value".to_vec();

        // Test put and get
        storage.put(key.to_vec(), value.clone()).await.unwrap();
        let result = storage.get(key).await.unwrap();
        assert_eq!(result, Some(value));

        // Test delete
        storage.delete(key).await.unwrap();
        let result = storage.get(key).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_mock_crypto_operations() {
        let crypto = create_mock_crypto();
        let data = b"test_data";

        // Test hash
        let hash = crypto.hash(data).await;
        assert!(!hash.is_empty());

        // Test signature verification (always returns true in mock)
        let result = crypto.verify_signature(data, &[1, 2, 3], &[4, 5, 6]).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_mock_network_operations() {
        let network = create_mock_network();

        // Test get peers (empty by default)
        let peers = network.get_peers().await;
        assert!(
            peers.is_empty(),
            "Expected empty peers list, got: {peers:?}"
        );

        // Test connect peer
        let peer_id = network.connect_peer("127.0.0.1:8080").await.unwrap();
        assert!(!peer_id.is_empty());

        // Verify peer was added
        let peers = network.get_peers().await;
        assert_eq!(peers.len(), 1);
        assert!(peers.contains(&peer_id));
    }
}
