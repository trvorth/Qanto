use bytes::Bytes;
use my_blockchain::qanto_standalone::hash::QantoHash;
use qanto::qanto_p2p::*;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_node_creation() {
    let config = NetworkConfig::default();
    let node = QantoP2P::new(config).unwrap();
    assert_eq!(node.node_id.as_bytes().len(), 32);
}

#[tokio::test]
async fn test_message_serialization() {
    let message = NetworkMessage {
        msg_type: MessageType::Heartbeat,
        payload: Bytes::from("test"),
        timestamp: 12345,
        sender: QantoHash::new([1u8; 32]),
        signature: QantoHash::new([2u8; 32]),
    };

    let serialized = bincode::serialize(&message).unwrap();
    let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

    assert_eq!(message.msg_type, deserialized.msg_type);
    assert_eq!(message.payload, deserialized.payload);
    assert_eq!(message.timestamp, deserialized.timestamp);
}

#[tokio::test]
async fn test_peer_connection() {
    let config1 = NetworkConfig {
        listen_port: 18333,
        ..Default::default()
    };

    let config2 = NetworkConfig {
        listen_port: 18334,
        bootstrap_nodes: vec![SocketAddr::new(
            IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            18333,
        )],
        ..Default::default()
    };

    let mut node1 = QantoP2P::new(config1).unwrap();
    let mut node2 = QantoP2P::new(config2).unwrap();

    // Start nodes
    node1.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;

    node2.start().await.unwrap();
    sleep(Duration::from_millis(500)).await;

    // Check connections
    assert!(!node1.get_peers().is_empty() || !node2.get_peers().is_empty());

    // Shutdown
    node1.shutdown().await.unwrap();
    node2.shutdown().await.unwrap();
}
