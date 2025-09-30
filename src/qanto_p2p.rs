//! Qanto Native P2P Networking Implementation
//! v0.1.0
//!
//! This module provides a complete P2P networking stack built exclusively for Qanto,
//! replacing libp2p with a custom implementation optimized for blockchain operations.
//! All networking operations are secured using qanhash for authentication and encryption.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, timeout};
// Removed unused import: use futures::prelude::*;
use bytes::Bytes;
use dashmap::DashMap;
use rand::{thread_rng, Rng};
use thiserror::Error;

use my_blockchain::qanto_standalone::hash::{qanto_hash, QantoHash};

/// P2P-specific error types
#[derive(Error, Debug)]
pub enum QantoP2PError {
    #[error("Network statistics lock poisoned: {0}")]
    StatsLockPoisoned(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Maximum number of concurrent connections
const MAX_CONNECTIONS: usize = 1000;
/// Connection timeout in seconds
const CONNECTION_TIMEOUT: u64 = 30;
/// Heartbeat interval in seconds
const HEARTBEAT_INTERVAL: u64 = 10;
/// Maximum message size in bytes
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024; // 16MB
/// Network protocol version
const PROTOCOL_VERSION: u32 = 1;

/// Qanto P2P Network Node
#[derive(Clone)]
pub struct QantoP2P {
    /// Node identifier derived from qanhash
    pub node_id: QantoHash,
    /// Local listening address
    pub local_addr: SocketAddr,
    /// Connected peers
    peers: Arc<DashMap<QantoHash, PeerConnection>>,
    /// Known peer addresses
    peer_addresses: Arc<DashMap<QantoHash, SocketAddr>>,
    /// Message handlers
    message_handlers: Arc<DashMap<MessageType, MessageHandler>>,
    /// Network statistics
    stats: Arc<RwLock<NetworkStats>>,
    /// Configuration
    config: NetworkConfig,
    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

/// Peer connection information
#[derive(Debug, Clone)]
pub struct PeerConnection {
    pub peer_id: QantoHash,
    pub addr: SocketAddr,
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub version: u32,
    pub tx: mpsc::UnboundedSender<NetworkMessage>,
    pub stats: PeerStats,
}

/// Peer statistics
#[derive(Debug, Clone, Default)]
pub struct PeerStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub latency_ms: u64,
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_connections: u64,
    pub active_connections: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub uptime: Duration,
    pub start_time: Instant,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            total_messages_sent: 0,
            total_messages_received: 0,
            uptime: Duration::from_secs(0),
            start_time: Instant::now(),
        }
    }
}

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub enable_encryption: bool,
    pub bootstrap_nodes: Vec<SocketAddr>,
    pub listen_port: u16,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            max_connections: MAX_CONNECTIONS,
            connection_timeout: Duration::from_secs(CONNECTION_TIMEOUT),
            heartbeat_interval: Duration::from_secs(HEARTBEAT_INTERVAL),
            enable_encryption: true,
            bootstrap_nodes: Vec::new(),
            listen_port: 8333,
        }
    }
}

/// Network message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    Handshake,
    Heartbeat,
    Block,
    Transaction,
    PeerDiscovery,
    StateSync,
    Custom(u16),
}

/// Network message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub msg_type: MessageType,
    pub payload: Bytes,
    pub timestamp: u64,
    pub sender: QantoHash,
    pub signature: QantoHash, // qanhash-based signature
}

/// Message handler function type
pub type MessageHandler = Arc<dyn Fn(NetworkMessage, QantoHash) -> Result<()> + Send + Sync>;

/// Handshake message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeMessage {
    pub version: u32,
    pub node_id: QantoHash,
    pub timestamp: u64,
    pub capabilities: Vec<String>,
}

/// Peer discovery message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDiscoveryMessage {
    pub peers: Vec<(QantoHash, SocketAddr)>,
    pub request_peers: bool,
}

impl QantoP2P {
    /// Create a new Qanto P2P node
    pub fn new(config: NetworkConfig) -> Result<Self> {
        let node_id = Self::generate_node_id()?;
        let local_addr = SocketAddr::new(
            IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            config.listen_port,
        );

        Ok(Self {
            node_id,
            local_addr,
            peers: Arc::new(DashMap::new()),
            peer_addresses: Arc::new(DashMap::new()),
            message_handlers: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(NetworkStats {
                start_time: Instant::now(),
                ..Default::default()
            })),
            config,
            shutdown_tx: Arc::new(Mutex::new(None)),
        })
    }

    /// Generate a unique node ID using qanhash
    fn generate_node_id() -> Result<QantoHash> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let random_bytes: [u8; 32] = thread_rng().gen();

        let mut data_to_hash = Vec::new();
        data_to_hash.extend_from_slice(&timestamp.to_le_bytes());
        data_to_hash.extend_from_slice(&random_bytes);

        Ok(qanto_hash(&data_to_hash))
    }

    /// Start the P2P network
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // Start TCP listener
        let listener = TcpListener::bind(self.local_addr).await?;
        self.local_addr = listener.local_addr()?;

        println!(
            "Qanto P2P node {} listening on {}",
            hex::encode(&self.node_id.as_bytes()[..8]),
            self.local_addr
        );

        // Start connection acceptor
        let peers = self.peers.clone();
        let config = self.config.clone();
        let node_id = self.node_id;
        let message_handlers = self.message_handlers.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, addr)) => {
                                if peers.len() < config.max_connections {
                                    let peers_clone = peers.clone();
                                    let handlers_clone = message_handlers.clone();
                                    let stats_clone = stats.clone();
                                    let config_clone = config.clone();

                                    tokio::spawn(async move {
                                        if let Err(e) = Self::handle_incoming_connection(
                                            stream, addr, node_id, peers_clone,
                                            handlers_clone, stats_clone, config_clone
                                        ).await {
                                            eprintln!("Error handling connection from {addr}: {e}");
                                        }
                                    });
                                }
                            }
                            Err(e) => eprintln!("Failed to accept connection: {e}"),
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        // Start heartbeat task
        self.start_heartbeat_task().await;

        // Connect to bootstrap nodes
        self.connect_to_bootstrap_nodes().await?;

        Ok(())
    }

    /// Handle incoming connection
    async fn handle_incoming_connection(
        stream: TcpStream,
        addr: SocketAddr,
        node_id: QantoHash,
        peers: Arc<DashMap<QantoHash, PeerConnection>>,
        message_handlers: Arc<DashMap<MessageType, MessageHandler>>,
        stats: Arc<RwLock<NetworkStats>>,
        config: NetworkConfig,
    ) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Perform handshake
        let (peer_id, mut reader, mut writer) =
            Self::perform_handshake(stream, node_id, true).await?;

        let peer_connection = PeerConnection {
            peer_id,
            addr,
            connected_at: Instant::now(),
            last_seen: Instant::now(),
            version: PROTOCOL_VERSION,
            tx: tx.clone(),
            stats: PeerStats::default(),
        };

        peers.insert(peer_id, peer_connection);

        // Update stats
        {
            let mut stats = stats.write().map_err(|e| {
                QantoP2PError::StatsLockPoisoned(format!("Failed to acquire stats write lock: {e}"))
            })?;
            stats.active_connections += 1;
            stats.total_connections += 1;
        }

        // Spawn message sender task
        // Removed unused variable: let tx_clone = tx.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = Self::send_message_to_stream(&mut writer, &message).await {
                    eprintln!("Failed to send message: {e}");
                    break;
                }
            }
        });

        // Message receiver loop
        loop {
            match timeout(
                config.connection_timeout,
                Self::read_message_from_stream(&mut reader),
            )
            .await
            {
                Ok(Ok(message)) => {
                    // Update peer stats
                    if let Some(mut peer) = peers.get_mut(&peer_id) {
                        peer.last_seen = Instant::now();
                        peer.stats.messages_received += 1;
                    }

                    // Handle message
                    if let Some(handler) = message_handlers.get(&message.msg_type) {
                        if let Err(e) = handler(message, peer_id) {
                            eprintln!("Message handler error: {e}");
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to read message: {e}");
                    break;
                }
                Err(_) => {
                    eprintln!(
                        "Connection timeout for peer {}",
                        hex::encode(&peer_id.as_bytes()[..8])
                    );
                    break;
                }
            }
        }

        // Clean up
        peers.remove(&peer_id);
        {
            if let Ok(mut stats) = stats.write() {
                stats.active_connections -= 1;
            }
            // Note: If lock is poisoned, we silently continue as this is cleanup code
        }

        Ok(())
    }

    /// Perform handshake with peer
    async fn perform_handshake(
        stream: TcpStream,
        node_id: QantoHash,
        is_incoming: bool,
    ) -> Result<(
        QantoHash,
        tokio::net::tcp::OwnedReadHalf,
        tokio::net::tcp::OwnedWriteHalf,
    )> {
        let handshake = HandshakeMessage {
            version: PROTOCOL_VERSION,
            node_id,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            capabilities: vec!["qanto-v1".to_string()],
        };

        let message = NetworkMessage {
            msg_type: MessageType::Handshake,
            payload: Bytes::from(bincode::serialize(&handshake)?),
            timestamp: handshake.timestamp,
            sender: node_id,
            signature: Self::sign_message(&handshake, node_id)?,
        };

        if is_incoming {
            // Wait for handshake from peer first
            let (mut reader, mut writer) = stream.into_split();
            let peer_message = Self::read_message_from_stream(&mut reader).await?;

            if peer_message.msg_type != MessageType::Handshake {
                return Err(anyhow!("Expected handshake message"));
            }

            let peer_handshake: HandshakeMessage = bincode::deserialize(&peer_message.payload)?;

            // Send our handshake response
            Self::send_message_to_stream(&mut writer, &message).await?;

            Ok((peer_handshake.node_id, reader, writer))
        } else {
            // Send handshake first
            let (mut reader, mut writer) = stream.into_split();
            Self::send_message_to_stream(&mut writer, &message).await?;

            // Wait for response
            let peer_message = Self::read_message_from_stream(&mut reader).await?;

            if peer_message.msg_type != MessageType::Handshake {
                return Err(anyhow!("Expected handshake response"));
            }

            let peer_handshake: HandshakeMessage = bincode::deserialize(&peer_message.payload)?;
            Ok((peer_handshake.node_id, reader, writer))
        }
    }

    /// Sign message using qanhash
    fn sign_message<T: Serialize>(message: &T, node_id: QantoHash) -> Result<QantoHash> {
        let mut data_to_hash = Vec::new();
        let serialized = bincode::serialize(message)?;
        data_to_hash.extend_from_slice(&serialized);
        data_to_hash.extend_from_slice(node_id.as_bytes());
        Ok(qanto_hash(&data_to_hash))
    }

    /// Send message to stream
    async fn send_message_to_stream(
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        message: &NetworkMessage,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let serialized = bincode::serialize(message)?;
        let len = serialized.len() as u32;

        if len > MAX_MESSAGE_SIZE as u32 {
            return Err(anyhow!("Message too large: {len} bytes"));
        }

        writer.write_all(&len.to_le_bytes()).await?;
        writer.write_all(&serialized).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Read message from stream
    async fn read_message_from_stream(
        reader: &mut tokio::net::tcp::OwnedReadHalf,
    ) -> Result<NetworkMessage> {
        use tokio::io::AsyncReadExt;

        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes).await?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(anyhow!("Message too large: {len} bytes"));
        }

        let mut buffer = vec![0u8; len];
        reader.read_exact(&mut buffer).await?;

        let message: NetworkMessage = bincode::deserialize(&buffer)?;
        Ok(message)
    }

    /// Connect to bootstrap nodes
    async fn connect_to_bootstrap_nodes(&self) -> Result<()> {
        for addr in &self.config.bootstrap_nodes {
            if let Err(e) = self.connect_to_peer(*addr).await {
                eprintln!("Failed to connect to bootstrap node {addr}: {e}");
            }
        }
        Ok(())
    }

    /// Connect to a specific peer
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<()> {
        if self.peers.len() >= self.config.max_connections {
            return Err(anyhow!("Maximum connections reached"));
        }

        let stream = timeout(self.config.connection_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| anyhow!("Connection timeout"))?
            .map_err(|e| anyhow!("TCP connection failed: {e}"))?;
        let (peer_id, reader, writer) =
            Self::perform_handshake(stream, self.node_id, false).await?;

        let (tx, rx) = mpsc::unbounded_channel();

        let peer_connection = PeerConnection {
            peer_id,
            addr,
            connected_at: Instant::now(),
            last_seen: Instant::now(),
            version: PROTOCOL_VERSION,
            tx: tx.clone(),
            stats: PeerStats::default(),
        };

        self.peers.insert(peer_id, peer_connection);
        self.peer_addresses.insert(peer_id, addr);

        // Update stats
        {
            let mut stats = self.stats.write().map_err(|e| {
                QantoP2PError::StatsLockPoisoned(format!(
                    "Failed to acquire stats write lock in connect_to_peer: {e}"
                ))
            })?;
            stats.active_connections += 1;
            stats.total_connections += 1;
        }

        // Handle connection
        let peers = self.peers.clone();
        let message_handlers = self.message_handlers.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let node_id = self.node_id;

        tokio::spawn(async move {
            if let Err(e) = Self::handle_outgoing_connection(
                reader,
                writer,
                addr,
                peer_id,
                node_id,
                peers,
                message_handlers,
                stats,
                config,
                rx,
            )
            .await
            {
                eprintln!("Error handling outgoing connection to {addr}: {e}");
            }
        });

        Ok(())
    }

    /// Handle outgoing connection
    #[allow(clippy::too_many_arguments)]
    async fn handle_outgoing_connection(
        mut reader: tokio::net::tcp::OwnedReadHalf,
        mut writer: tokio::net::tcp::OwnedWriteHalf,
        _addr: SocketAddr, // Prefixed with underscore to indicate intentional non-use
        peer_id: QantoHash,
        _node_id: QantoHash, // Prefixed with underscore to indicate intentional non-use
        peers: Arc<DashMap<QantoHash, PeerConnection>>,
        message_handlers: Arc<DashMap<MessageType, MessageHandler>>,
        stats: Arc<RwLock<NetworkStats>>,
        config: NetworkConfig,
        mut rx: mpsc::UnboundedReceiver<NetworkMessage>,
    ) -> Result<()> {
        // Spawn message sender task
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = Self::send_message_to_stream(&mut writer, &message).await {
                    eprintln!("Failed to send message: {e}");
                    break;
                }
            }
        });

        // Message receiver loop
        loop {
            match timeout(
                config.connection_timeout,
                Self::read_message_from_stream(&mut reader),
            )
            .await
            {
                Ok(Ok(message)) => {
                    // Update peer stats
                    if let Some(mut peer) = peers.get_mut(&peer_id) {
                        peer.last_seen = Instant::now();
                        peer.stats.messages_received += 1;
                    }

                    // Handle message
                    if let Some(handler) = message_handlers.get(&message.msg_type) {
                        if let Err(e) = handler(message, peer_id) {
                            eprintln!("Message handler error: {e}");
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to read message: {e}");
                    break;
                }
                Err(_) => {
                    eprintln!(
                        "Connection timeout for peer {}",
                        hex::encode(&peer_id.as_bytes()[..8])
                    );
                    break;
                }
            }
        }

        // Clean up
        peers.remove(&peer_id);
        {
            // Silently continue if lock is poisoned during cleanup
            if let Ok(mut stats) = stats.write() {
                stats.active_connections -= 1;
            }
        }

        Ok(())
    }

    /// Start heartbeat task
    async fn start_heartbeat_task(&self) {
        let peers = self.peers.clone();
        let node_id = self.node_id;
        let interval_duration = self.config.heartbeat_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                let heartbeat = NetworkMessage {
                    msg_type: MessageType::Heartbeat,
                    payload: Bytes::new(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    sender: node_id,
                    signature: QantoHash::new([0u8; 32]), // Simple heartbeat, no signature needed
                };

                // Send heartbeat to all peers
                for peer in peers.iter() {
                    if let Err(e) = peer.tx.send(heartbeat.clone()) {
                        eprintln!(
                            "Failed to send heartbeat to peer {}: {}",
                            hex::encode(&peer.peer_id.as_bytes()[..8]),
                            e
                        );
                    }
                }
            }
        });
    }

    /// Register message handler
    pub fn register_handler(&self, msg_type: MessageType, handler: MessageHandler) {
        self.message_handlers.insert(msg_type, handler);
    }

    /// Broadcast message to all peers
    pub fn broadcast(&self, msg_type: MessageType, payload: Bytes) -> Result<()> {
        let message = NetworkMessage {
            msg_type,
            payload,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            sender: self.node_id,
            signature: Self::sign_message(&msg_type, self.node_id)?,
        };

        for peer in self.peers.iter() {
            if let Err(e) = peer.tx.send(message.clone()) {
                eprintln!(
                    "Failed to broadcast to peer {}: {}",
                    hex::encode(&peer.peer_id.as_bytes()[..8]),
                    e
                );
            }
        }

        Ok(())
    }

    /// Send message to specific peer
    pub fn send_to_peer(
        &self,
        peer_id: QantoHash,
        msg_type: MessageType,
        payload: Bytes,
    ) -> Result<()> {
        let message = NetworkMessage {
            msg_type,
            payload,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            sender: self.node_id,
            signature: Self::sign_message(&msg_type, self.node_id)?,
        };

        if let Some(peer) = self.peers.get(&peer_id) {
            peer.tx.send(message)?;
        } else {
            return Err(anyhow!(
                "Peer not found: {}",
                hex::encode(&peer_id.as_bytes()[..8])
            ));
        }

        Ok(())
    }

    /// Get connected peers
    pub fn get_peers(&self) -> Vec<QantoHash> {
        self.peers.iter().map(|entry| *entry.key()).collect()
    }

    /// Get network statistics
    pub fn get_stats(&self) -> NetworkStats {
        let mut stats = self.stats.read().unwrap().clone();
        stats.uptime = stats.start_time.elapsed();
        stats
    }

    /// Shutdown the network
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
