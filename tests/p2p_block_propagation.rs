use bytes::Bytes;
use qanto_core::qanto_p2p::*;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn two_node_block_propagation() {
    // Node 1 listens on 18433, Node 2 bootstraps to Node 1
    let config1 = NetworkConfig {
        listen_port: 18433,
        ..Default::default()
    };
    let config2 = NetworkConfig {
        listen_port: 18434,
        bootstrap_nodes: vec![SocketAddr::new(
            IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            18433,
        )],
        ..Default::default()
    };

    let mut node1 = QantoP2P::new(config1).unwrap();
    let mut node2 = QantoP2P::new(config2).unwrap();

    // Capture block receipt on node2
    let received_block = Arc::new(Mutex::new(false));
    let received_block_clone = Arc::clone(&received_block);
    node2.register_handler(
        MessageType::Block,
        Arc::new(move |msg, _sender| {
            let mut guard = futures::executor::block_on(received_block_clone.lock());
            *guard = true;
            assert_eq!(msg.msg_type, MessageType::Block);
            Ok(())
        }),
    );

    // Start both nodes
    node1.start().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    node2.start().await.unwrap();
    sleep(Duration::from_millis(500)).await;

    // Explicitly dial node1 to ensure connection
    let addr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 18433);
    let _ = node2.connect_to_peer(addr).await;
    sleep(Duration::from_millis(200)).await;

    // Ensure they discovered each other
    assert!(
        !node1.get_peers().is_empty() || !node2.get_peers().is_empty(),
        "Peers should be connected"
    );

    // Broadcast a mock block from node1
    let mock_block_bytes = Bytes::from_static(b"mock_block_0001");
    node1
        .broadcast(MessageType::Block, mock_block_bytes)
        .unwrap();

    // Wait for propagation
    sleep(Duration::from_millis(300)).await;
    let saw_block = { *received_block.lock().await };
    assert!(saw_block, "Node2 should have received the block");

    // Shutdown
    node1.shutdown().await.unwrap();
    node2.shutdown().await.unwrap();
}
