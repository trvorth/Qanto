//! Simple test to verify mock generation works

#[cfg(test)]
mod tests {
    use qanto::mock_traits::{MockCrypto, MockNetwork, MockStorage};
    use qanto::storage_traits::{CryptoOperations, NetworkOperations, StorageOperations};

    #[tokio::test]
    async fn test_mock_storage() {
        let mock_storage = MockStorage::new();
        let key = b"test_key".to_vec();
        let value = b"test_value".to_vec();

        mock_storage.put(key.clone(), value.clone()).await.unwrap();
        let result = mock_storage.get(&key).await.unwrap();
        assert_eq!(result, Some(value));
    }

    #[tokio::test]
    async fn test_mock_crypto() {
        let mock_crypto = MockCrypto::new();
        let data = b"test_data";
        let hash = mock_crypto.hash(data).await;
        assert!(!hash.is_empty());
    }

    #[tokio::test]
    async fn test_mock_network() {
        let mock_network = MockNetwork::new();
        let peers = mock_network.get_peers().await;
        assert!(peers.is_empty());
    }
}
