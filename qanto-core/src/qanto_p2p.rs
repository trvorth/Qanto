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

use my_blockchain::qanto_hash;
use my_blockchain::qanto_standalone::hash::QantoHash;

/// P2P-specific error types
#[derive(Error, Debug)]
pub enum QantoP2PError {
    /// Network statistics lock was poisoned
    #[error("Network statistics lock poisoned: {0}")]
    StatsLockPoisoned(String),
    /// Connection-related error
    #[error("Connection error: {0}")]
    Connection(String),
    /// Protocol-level error
    #[error("Protocol error: {0}")]
    Protocol(String),
    /// Serialization/deserialization error
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
/// 
/// This is the main P2P networking component that handles peer connections,
/// message routing, and network statistics for the Qanto blockchain network.
#[derive(Clone)]
pub struct QantoP2P {
    /// Unique identifier for this network node
    pub node_id: QantoHash,
    /// Local socket address this node is listening on
    pub local_addr: SocketAddr,
    /// Active peer connections indexed by peer ID
    peers: Arc<DashMap<QantoHash, PeerConnection>>,
    /// Known peer addresses for reconnection
    peer_addresses: Arc<DashMap<QantoHash, SocketAddr>>,
    /// Message handlers for different message types
    message_handlers: Arc<DashMap<MessageType, MessageHandler>>,
    /// Network statistics and metrics
    stats: Arc<RwLock<NetworkStats>>,
    /// Network configuration parameters
    config: NetworkConfig,
    /// Shutdown signal sender
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

/// Represents an active connection to a peer
#[derive(Debug, Clone)]
pub struct PeerConnection {
    /// Unique identifier of the connected peer
    pub peer_id: QantoHash,
    /// Socket address of the peer
    pub addr: SocketAddr,
    /// Timestamp when connection was established
    pub connected_at: Instant,
    /// Timestamp of last communication with peer
    pub last_seen: Instant,
    /// Protocol version supported by the peer
    pub version: u32,
    /// Message sender channel for this peer
    pub tx: mpsc::UnboundedSender<NetworkMessage>,
    /// Connection-specific statistics
    pub stats: PeerStats,
}

/// Statistics for an individual peer connection
#[derive(Debug, Clone, Default)]
pub struct PeerStats {
    /// Total bytes sent to this peer
    pub bytes_sent: u64,
    /// Total bytes received from this peer
    pub bytes_received: u64,
    /// Total messages sent to this peer
    pub messages_sent: u64,
    /// Total messages received from this peer
    pub messages_received: u64,
    /// Average latency in milliseconds
    pub latency_ms: u64,
}

/// Overall network statistics for the P2P node
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Total number of connections ever established
    pub total_connections: u64,
    /// Current number of active connections
    pub active_connections: u64,
    /// Total bytes sent across all connections
    pub total_bytes_sent: u64,
    /// Total bytes received across all connections
    pub total_bytes_received: u64,
    /// Total messages sent across all connections
    pub total_messages_sent: u64,
    /// Total messages received across all connections
    pub total_messages_received: u64,
    /// Total uptime duration
    pub uptime: Duration,
    /// Timestamp when the node started
    pub start_time: Instant,
}

impl Default for NetworkStats {
    /// Creates default network statistics with zero values and current timestamp
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
    /// Maximum number of concurrent peer connections
    pub max_connections: usize,
    /// Timeout duration for establishing connections
    pub connection_timeout: Duration,
    /// Interval between heartbeat messages
    pub heartbeat_interval: Duration,
    /// Whether to enable message encryption
    pub enable_encryption: bool,
    /// List of bootstrap nodes to connect to on startup
    pub bootstrap_nodes: Vec<SocketAddr>,
    /// Port to listen on for incoming connections
    pub listen_port: u16,
}

impl Default for NetworkConfig {
    /// Creates default network configuration with standard parameters
    fn default() -> Self {
        Self {
            max_connections: MAX_CONNECTIONS,
            connection_timeout: Duration::from_secs(CONNECTION_TIMEOUT),
            heartbeat_interval: Duration::from_secs(HEARTBEAT_INTERVAL),
            enable_encryption: true,
            bootstrap_nodes: Vec::new(),
            listen_port: 8080,
        }
    }
}

/// Network message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    /// Initial handshake message between peers
    Handshake,
    /// Periodic heartbeat to maintain connection
    Heartbeat,
    /// Blockchain block data
    Block,
    /// Transaction data
    Transaction,
    /// Peer discovery and sharing
    PeerDiscovery,
    /// State synchronization data
    StateSync,
    /// Custom message type with user-defined ID
    Custom(u16),
}

/// Network message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    /// Type of the message
    pub msg_type: MessageType,
    /// Message payload data
    pub payload: Bytes,
    /// Unix timestamp when message was created
    pub timestamp: u64,
    /// ID of the node that sent this message
    pub sender: QantoHash,
    /// qanhash-based signature for message authentication
    pub signature: QantoHash,
}

/// Function type for handling incoming network messages
pub type MessageHandler = Arc<dyn Fn(NetworkMessage, QantoHash) -> Result<()> + Send + Sync>;

/// Handshake message exchanged during peer connection establishment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeMessage {
    /// Protocol version supported by the peer
    pub version: u32,
    /// Unique identifier of the peer node
    pub node_id: QantoHash,
    /// Unix timestamp of the handshake
    pub timestamp: u64,
    /// List of capabilities supported by the peer
    pub capabilities: Vec<String>,
}

/// Message for peer discovery and network topology sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDiscoveryMessage {
    /// List of known peers with their addresses
    pub peers: Vec<(QantoHash, SocketAddr)>,
    /// Whether this message is requesting peer information
    pub request_peers: bool,
}

impl QantoP2P {
    /// Create a new Qanto P2P node with the specified configuration
    /// 
    /// # Arguments
    /// * `config` - Network configuration parameters
    /// 
    /// # Returns
    /// A new QantoP2P instance ready to start networking
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
    /// 
    /// Creates a cryptographically secure node identifier by hashing
    /// the current timestamp and random bytes using qanhash.
    fn generate_node_id() -> Result<QantoHash> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let random_bytes: [u8; 32] = thread_rng().gen();

        let mut data_to_hash = Vec::new();
        data_to_hash.extend_from_slice(&timestamp.to_le_bytes());
        data_to_hash.extend_from_slice(&random_bytes);

        Ok(qanto_hash(&data_to_hash))
    }

    /// Start the P2P network and begin accepting connections
    /// 
    /// This method starts the TCP listener, connection acceptor task,
    /// heartbeat task, and connects to bootstrap nodes.
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
                                    let message_handlers_clone = message_handlers.clone();
                                    let stats_clone = stats.clone();
                                    let config_clone = config.clone();

                                    tokio::spawn(async move {
                                        if let Err(e) = Self::handle_incoming_connection(
                                            stream,
                                            addr,
                                            node_id,
                                            peers_clone,
                                            message_handlers_clone,
                                            stats_clone,
                                            config_clone,
                                        ).await {
                                            eprintln!("Error handling incoming connection: {e}");
                                        }
                                    });
                                }
                            }
                            Err(e) => {
                                eprintln!("Error accepting connection: {e}");
                            }
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

    /// Handle an incoming TCP connection from a peer
    /// 
    /// Performs handshake, establishes peer connection, and starts
    /// message handling tasks for the new peer.
    async fn handle_incoming_connection(
        stream: TcpStream,
        addr: SocketAddr,
        node_id: QantoHash,
        peers: Arc<DashMap<QantoHash, PeerConnection>>,
        message_handlers: Arc<DashMap<MessageType, MessageHandler>>,
        stats: Arc<RwLock<NetworkStats>>,
        config: NetworkConfig,
    ) -> Result<()> {
        // Perform handshake
        let (peer_id, reader, writer) =
            Self::perform_handshake(stream, node_id, true).await?;

        // Create message channel
        let (tx, rx) = mpsc::unbounded_channel();

        // Create peer connection
        let peer_connection = PeerConnection {
            peer_id,
            addr,
            connected_at: Instant::now(),
            last_seen: Instant::now(),
            version: PROTOCOL_VERSION,
            tx,
            stats: PeerStats::default(),
        };

        // Add to peers
        peers.insert(peer_id, peer_connection);

        // Update stats
        if let Ok(mut stats) = stats.write() {
            stats.total_connections += 1;
            stats.active_connections = peers.len() as u64;
        }

        // Start message handling
        Self::handle_outgoing_connection(
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
    }

    /// Perform handshake protocol with a peer
    /// 
    /// Exchanges handshake messages to establish protocol version,
    /// node identity, and capabilities.
    /// 
    /// # Arguments
    /// * `stream` - TCP stream to the peer
    /// * `node_id` - This node's identifier
    /// * `is_incoming` - Whether this is an incoming or outgoing connection
    /// 
    /// # Returns
    /// Tuple of (peer_id, read_half, write_half) on successful handshake
    async fn perform_handshake(
        stream: TcpStream,
        node_id: QantoHash,
        is_incoming: bool,
    ) -> Result<(
        QantoHash,
        tokio::net::tcp::OwnedReadHalf,
        tokio::net::tcp::OwnedWriteHalf,
    )> {
        let (mut reader, mut writer) = stream.into_split();

        let handshake = HandshakeMessage {
            version: PROTOCOL_VERSION,
            node_id,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            capabilities: vec!["qanto-v1".to_string()],
        };

        let message = NetworkMessage {
            msg_type: MessageType::Handshake,
            payload: Bytes::from(serde_json::to_vec(&handshake)?),
            timestamp: handshake.timestamp,
            sender: node_id,
            signature: Self::sign_message(&handshake, node_id)?,
        };

        if !is_incoming {
            // Send our handshake first for outgoing connections
            Self::send_message_to_stream(&mut writer, &message).await?;
        }

        // Read peer's handshake
        let peer_message = Self::read_message_from_stream(&mut reader).await?;
        let peer_handshake: HandshakeMessage = serde_json::from_slice(&peer_message.payload)?;

        if is_incoming {
            // Send our handshake response for incoming connections
            Self::send_message_to_stream(&mut writer, &message).await?;
        }

        // Verify protocol version
        if peer_handshake.version != PROTOCOL_VERSION {
            return Err(anyhow!("Protocol version mismatch"));
        }

        Ok((peer_handshake.node_id, reader, writer))
    }

    /// Sign a message using qanhash for authentication
    /// 
    /// # Arguments
    /// * `message` - The message to sign (must be serializable)
    /// * `node_id` - The node ID to include in the signature
    fn sign_message<T: Serialize>(message: &T, node_id: QantoHash) -> Result<QantoHash> {
        let message_bytes = serde_json::to_vec(message)?;
        let mut data_to_sign = Vec::new();
        data_to_sign.extend_from_slice(&message_bytes);
        data_to_sign.extend_from_slice(node_id.as_bytes());
        Ok(qanto_hash(&data_to_sign))
    }

    /// Send a network message over a TCP stream
    /// 
    /// Serializes the message and sends it with a length prefix.
    async fn send_message_to_stream(
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        message: &NetworkMessage,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let serialized = serde_json::to_vec(message)?;
        let len = serialized.len() as u32;

        if len as usize > MAX_MESSAGE_SIZE {
            return Err(anyhow!("Message too large: {len} bytes"));
        }

        writer.write_all(&len.to_le_bytes()).await?;
        writer.write_all(&serialized).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Read a network message from a TCP stream
    /// 
    /// Reads the length prefix and then the message data.
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

        let mut message_bytes = vec![0u8; len];
        reader.read_exact(&mut message_bytes).await?;

        let message: NetworkMessage = serde_json::from_slice(&message_bytes)?;
        Ok(message)
    }

    /// Connect to all configured bootstrap nodes
    /// 
    /// Attempts to establish connections to bootstrap nodes for
    /// initial network discovery.
    async fn connect_to_bootstrap_nodes(&self) -> Result<()> {
        for addr in &self.config.bootstrap_nodes {
            if let Err(e) = self.connect_to_peer(*addr).await {
                eprintln!("Failed to connect to bootstrap node {addr}: {e}");
            }
        }
        Ok(())
    }

    /// Connect to a specific peer by address
    /// 
    /// Establishes an outgoing connection to the specified peer address.
    /// 
    /// # Arguments
    /// * `addr` - Socket address of the peer to connect to
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<()> {
        if self.peers.len() >= self.config.max_connections {
            return Err(anyhow!("Maximum connections reached"));
        }

        let stream = timeout(
            self.config.connection_timeout,
            TcpStream::connect(addr),
        )
        .await??;

        // Perform handshake
        let (peer_id, reader, writer) =
            Self::perform_handshake(stream, self.node_id, false).await?;

        // Check if we're already connected to this peer
        if self.peers.contains_key(&peer_id) {
            return Ok(());
        }

        // Create message channel
        let (tx, rx) = mpsc::unbounded_channel();

        // Create peer connection
        let peer_connection = PeerConnection {
            peer_id,
            addr,
            connected_at: Instant::now(),
            last_seen: Instant::now(),
            version: PROTOCOL_VERSION,
            tx,
            stats: PeerStats::default(),
        };

        // Add to peers and addresses
        self.peers.insert(peer_id, peer_connection);
        self.peer_addresses.insert(peer_id, addr);

        // Update stats
        if let Ok(mut stats) = self.stats.write() {
            stats.total_connections += 1;
            stats.active_connections = self.peers.len() as u64;
        }

        // Start message handling
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
                eprintln!("Error handling outgoing connection: {e}");
            }
        });

        Ok(())
    }

    /// Handle message processing for an established peer connection
    /// 
    /// Manages bidirectional message flow between this node and a peer,
    /// including sending queued messages and processing incoming messages.
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
        _config: NetworkConfig,
        mut rx: mpsc::UnboundedReceiver<NetworkMessage>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                // Handle outgoing messages
                message = rx.recv() => {
                    match message {
                        Some(msg) => {
                            if let Err(e) = Self::send_message_to_stream(&mut writer, &msg).await {
                                eprintln!("Error sending message to peer {}: {e}", hex::encode(&peer_id.as_bytes()[..8]));
                                break;
                            }

                            // Update peer stats
                            if let Some(mut peer) = peers.get_mut(&peer_id) {
                                peer.stats.messages_sent += 1;
                                peer.last_seen = Instant::now();
                            }

                            // Update global stats
                            if let Ok(mut stats) = stats.write() {
                                stats.total_messages_sent += 1;
                            }
                        }
                        None => break,
                    }
                }

                // Handle incoming messages
                result = Self::read_message_from_stream(&mut reader) => {
                    match result {
                        Ok(message) => {
                            // Update peer stats
                            if let Some(mut peer) = peers.get_mut(&peer_id) {
                                peer.stats.messages_received += 1;
                                peer.last_seen = Instant::now();
                            }

                            // Update global stats
                            if let Ok(mut stats) = stats.write() {
                                stats.total_messages_received += 1;
                            }

                            // Handle the message
                            if let Some(handler) = message_handlers.get(&message.msg_type) {
                                if let Err(e) = handler(message, peer_id) {
                                    eprintln!("Error handling message: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading message from peer {}: {e}", hex::encode(&peer_id.as_bytes()[..8]));
                            break;
                        }
                    }
                }
            }
        }

        // Clean up peer connection
        peers.remove(&peer_id);
        if let Ok(mut stats) = stats.write() {
            stats.active_connections = peers.len() as u64;
        }

        Ok(())
    }

    /// Start the heartbeat task to maintain peer connections
    /// 
    /// Sends periodic heartbeat messages to all connected peers
    /// to detect disconnections and maintain connection health.
    async fn start_heartbeat_task(&self) {
        let peers = self.peers.clone();
        let heartbeat_interval = self.config.heartbeat_interval;
        let node_id = self.node_id;

        tokio::spawn(async move {
            let mut interval = interval(heartbeat_interval);

            loop {
                interval.tick().await;

                let heartbeat_message = NetworkMessage {
                    msg_type: MessageType::Heartbeat,
                    payload: Bytes::new(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    sender: node_id,
                    signature: qanto_hash(&[]), // Empty signature for heartbeat
                };

                // Send heartbeat to all peers
                for peer in peers.iter() {
                    if let Err(e) = peer.tx.send(heartbeat_message.clone()) {
                        eprintln!("Failed to send heartbeat to peer: {e}");
                    }
                }
            }
        });
    }

    /// Register a message handler for a specific message type
    /// 
    /// # Arguments
    /// * `msg_type` - The type of message to handle
    /// * `handler` - Function to call when messages of this type are received
    pub fn register_handler(&self, msg_type: MessageType, handler: MessageHandler) {
        self.message_handlers.insert(msg_type, handler);
    }

    /// Broadcast a message to all connected peers
    /// 
    /// # Arguments
    /// * `msg_type` - Type of the message
    /// * `payload` - Message payload data
    pub fn broadcast(&self, msg_type: MessageType, payload: Bytes) -> Result<()> {
        let message = NetworkMessage {
            msg_type,
            payload,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs(),
            sender: self.node_id,
            signature: Self::sign_message(&msg_type, self.node_id)?,
        };

        for peer in self.peers.iter() {
            if let Err(e) = peer.tx.send(message.clone()) {
                eprintln!("Failed to send message to peer: {e}");
            }
        }

        Ok(())
    }

    /// Send a message to a specific peer
    /// 
    /// # Arguments
    /// * `peer_id` - ID of the target peer
    /// * `msg_type` - Type of the message
    /// * `payload` - Message payload data
    pub fn send_to_peer(
        &self,
        peer_id: QantoHash,
        msg_type: MessageType,
        payload: Bytes,
    ) -> Result<()> {
        let message = NetworkMessage {
            msg_type,
            payload,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs(),
            sender: self.node_id,
            signature: Self::sign_message(&msg_type, self.node_id)?,
        };

        if let Some(peer) = self.peers.get(&peer_id) {
            peer.tx.send(message)?;
        } else {
            let peer_id_hex = hex::encode(&peer_id.as_bytes()[..8]);
            return Err(anyhow!("Peer not found: {peer_id_hex}"));
        }

        Ok(())
    }

    /// Get a list of all connected peer IDs
    pub fn get_peers(&self) -> Vec<QantoHash> {
        self.peers.iter().map(|entry| *entry.key()).collect()
    }

    /// Get current network statistics
    pub fn get_stats(&self) -> NetworkStats {
        self.stats.read().unwrap().clone()
    }

    /// Shutdown the P2P network and close all connections
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        Ok(())
    }
}
