// src/monitor.rs

use crate::decentralization::PeerDiscoveryService;
use crate::qanto_net::{PeerId, PeerInfo};
use qanto_core::qanto_p2p::{NetworkConfig, QantoP2P};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

/// Represents a discovered node to be monitored
struct DiscoveredNode {
    name: String,
    peer_info: PeerInfo,
    last_response: Option<NodeStatus>,
}

/// Node status information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct NodeStatus {
    peer_id: String,
    total_blocks: u64,
    last_seen: u64,
    is_responsive: bool,
}

impl DiscoveredNode {
    fn new(name: String, peer_info: PeerInfo) -> Self {
        Self {
            name,
            peer_info,
            last_response: None,
        }
    }

    /// Attempts to get status from the peer via P2P network
    async fn fetch_status_p2p(&mut self, _p2p_node: &QantoP2P) -> Result<NodeStatus, String> {
        // Try to get status via P2P messaging instead of HTTP
        let status = NodeStatus {
            peer_id: self.peer_info.peer_id.to_string(),
            total_blocks: 0, // Would be fetched via P2P message
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            is_responsive: true,
        };

        self.last_response = Some(status.clone());
        Ok(status)
    }
}

pub async fn run() {
    // Initialize decentralized peer discovery
    let _peer_discovery = Arc::new(RwLock::new(PeerDiscoveryService::new()));

    // Initialize P2P node for decentralized monitoring
    let p2p_config = NetworkConfig {
        listen_port: 18080,
        bootstrap_nodes: vec![], // Will discover peers dynamically
        max_connections: 50,
        connection_timeout: Duration::from_secs(10),
        heartbeat_interval: Duration::from_secs(30),
        enable_encryption: true,
        // Compression settings: use defaults unless explicitly configured
        ..Default::default()
    };

    let mut p2p_node = match QantoP2P::new(p2p_config) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Failed to initialize P2P node: {e}");
            return;
        }
    };

    // Start P2P node
    if let Err(e) = p2p_node.start().await {
        eprintln!("Failed to start P2P node: {e}");
        return;
    }

    let mut discovered_nodes: HashMap<String, DiscoveredNode> = HashMap::new();

    // Set an interval to refresh the status every 5 seconds.
    let mut interval = time::interval(Duration::from_secs(5));
    let mut discovery_interval = time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Clear the console screen for a clean display on each update.
                print!("\x1B[2J\x1B[1;1H");

                println!("--- QantoChain Decentralized Network Monitor ---");
                println!(
                    "{:<22} | {:<20} | {:<15} | {:<10}",
                    "Node", "Peer ID", "Last Seen", "Status"
                );
                println!("{:-<23}|{:-<22}|{:-<16}|{:-<11}", "", "", "", "");

                // Display discovered nodes
                if discovered_nodes.is_empty() {
                    println!("{:<22} | {:<20} | {:<15} | {:<10}", "Discovering...", "N/A", "N/A", "Searching");
                } else {
                    for node in discovered_nodes.values_mut() {
                        match node.fetch_status_p2p(&p2p_node).await {
                            Ok(status) => {
                                let short_peer_id = if status.peer_id.len() > 12 {
                                    format!("...{}", &status.peer_id[status.peer_id.len() - 12..])
                                } else {
                                    status.peer_id.clone()
                                };

                                let time_ago = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() - status.last_seen;

                                let last_seen_str = if time_ago < 60 {
                                    format!("{time_ago}s ago")
                                } else {
                                    format!("{minutes}m ago", minutes = time_ago / 60)
                                };

                                println!("{:<22} | {:<20} | {:<15} | {:<10}",
                                    node.name, short_peer_id, last_seen_str, "Online");
                            }
                            Err(_) => {
                                println!("{:<22} | {:<20} | {:<15} | {:<10}",
                                    node.name, "Error", "N/A", "Offline");
                            }
                        }
                    }
                }

                println!(
                    "\nConnected Peers: {} | Last updated: {}",
                    p2p_node.get_peers().len(),
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                );
            }
            _ = discovery_interval.tick() => {
                // Discover new peers periodically
                let peers = p2p_node.get_peers();
                for (i, peer_id) in peers.iter().enumerate() {
                    let node_name = format!("Node-{}", i + 1);
                    let peer_id_str = peer_id.to_string();
                    discovered_nodes.entry(peer_id_str).or_insert_with(|| {
                        // Create a mock PeerInfo for discovered peers
                        let peer_info = PeerInfo {
                            peer_id: PeerId { id: *peer_id.as_bytes() },
                            address: "0.0.0.0:0".parse().unwrap(),
                            public_key: Vec::new(),
                            capabilities: Vec::new(),
                            last_seen: std::time::SystemTime::now(),
                            reputation: 0,
                            connection_count: 0,
                        };
                        DiscoveredNode::new(node_name, peer_info)
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
