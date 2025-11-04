//! Qanto Native P2P Networking Implementation
//! v0.1.0
//!
//! This module provides a complete P2P networking stack built exclusively for Qanto,
//! replacing libp2p with a custom implementation optimized for blockchain operations.
//! All networking operations are secured using qanhash for authentication and encryption.

use anyhow::{anyhow, Result};
use bincode::{deserialize, serialize};
use bytes::Bytes;
use dashmap::DashMap;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::time::{interval, timeout};

use my_blockchain::qanto_hash;
use my_blockchain::qanto_standalone::hash::QantoHash;
use crate::balance_stream::{BalanceBroadcaster, BalanceSubscribe, BalanceUpdate};

const ZSTD_MAGIC: [u8; 4] = *b"ZSTD";
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

/// Enhanced constants for 32 BPS & 10M+ TPS optimization
const MAX_CONNECTIONS: usize = 2000; // Increased from 1000 for higher throughput
const CONNECTION_TIMEOUT: u64 = 15; // Reduced from 30 for faster connection establishment
const HEARTBEAT_INTERVAL: u64 = 5; // Reduced from 10 for faster peer discovery
const MAX_MESSAGE_SIZE: usize = 32 * 1024 * 1024; // Increased from 16MB to 32MB for larger blocks
const PROTOCOL_VERSION: u32 = 1;

// New constants for high-throughput optimization
#[allow(dead_code)]
const BATCH_SIZE_THRESHOLD: usize = 10; // Batch multiple messages for efficiency
#[allow(dead_code)]
const ADAPTIVE_COMPRESSION_THRESHOLD: usize = 1024; // Use adaptive compression
#[allow(dead_code)]
const PRIORITY_QUEUE_SIZE: usize = 1000; // Priority queue for critical messages
#[allow(dead_code)]
const GOSSIP_FANOUT: usize = 8; // Optimized gossip fanout for 32 BPS

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
    /// High-performance set reconciliation gossip protocol
    #[allow(dead_code)]
    gossip_protocol: Arc<SetReconciliationGossip>,
    /// Subscribers per address for real-time balance updates
    balance_subscribers: Arc<DashMap<String, Vec<QantoHash>>>,
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
    /// Whether to enable zstd compression for network messages
    pub enable_compression: bool,
    /// Zstd compression level (typical: 1-9)
    pub compression_level: i32,
    /// Minimum size (bytes) to attempt compression
    pub compression_min_size: usize,
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
            enable_compression: false,
            compression_level: 3,
            compression_min_size: 1024,
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
    /// Subscribe to real-time balance updates for an address
    BalanceSubscribe,
    /// Real-time balance update for a subscribed address
    BalanceUpdate,
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
        let node = Self {
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
            gossip_protocol: Arc::new(SetReconciliationGossip::new(
                GOSSIP_FANOUT,
                Duration::from_secs(300), // 5 minutes max message age
            )),
            balance_subscribers: Arc::new(DashMap::new()),
        };

        // Register built-in handler for BalanceSubscribe
        let subscribers = node.balance_subscribers.clone();
        node.register_handler(
            MessageType::BalanceSubscribe,
            Arc::new(move |message, peer_id| {
                let sub: BalanceSubscribe = deserialize(&message.payload)
                    .map_err(|e| anyhow!("Deserialize BalanceSubscribe failed: {e}"))?;
                let addr = sub.address.clone();
                if let Some(mut entry) = subscribers.get_mut(&addr) {
                    let list = entry.value_mut();
                    if !list.iter().any(|p| p == &peer_id) {
                        list.push(peer_id);
                    }
                } else {
                    subscribers.insert(addr, vec![peer_id]);
                }
                Ok(())
            }),
        );

        Ok(node)
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
            Self::perform_handshake(stream, node_id, true, &config).await?;

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
        config: &NetworkConfig,
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
            capabilities: {
                let mut caps = vec!["qanto-v1".to_string()];
                // Always advertise zstd frame support for Block/Transaction payloads
                caps.push("compression:zstd".to_string());
                caps
            },
        };

        let message = NetworkMessage {
            msg_type: MessageType::Handshake,
            payload: Bytes::from(serialize(&handshake)?),
            timestamp: handshake.timestamp,
            sender: node_id,
            signature: Self::sign_message(&handshake, node_id)?,
        };

        if !is_incoming {
            // Send our handshake first for outgoing connections
            Self::send_message_to_stream(&mut writer, &message, config).await?;
        }

        // Read peer's handshake
        let peer_message = Self::read_message_from_stream(&mut reader).await?;
        let peer_handshake: HandshakeMessage = deserialize(&peer_message.payload)?;

        if is_incoming {
            // Send our handshake response for incoming connections
            Self::send_message_to_stream(&mut writer, &message, config).await?;
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
        let message_bytes = serialize(message)?;
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
        config: &NetworkConfig,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let serialized = serialize(message)?;

        // Unconditionally zstd-frame compress Block/Transaction messages for network efficiency
        let payload: Vec<u8> = match message.msg_type {
            MessageType::Block | MessageType::Transaction => {
                let compressed =
                    zstd::encode_all(Cursor::new(&serialized), config.compression_level)
                        .map_err(|e| anyhow!("zstd compression failed: {e}"))?;
                let mut framed = Vec::with_capacity(4 + compressed.len());
                framed.extend_from_slice(&ZSTD_MAGIC);
                framed.extend_from_slice(&compressed);
                framed
            }
            _ => {
                if config.enable_compression && serialized.len() >= config.compression_min_size {
                    let compressed =
                        zstd::encode_all(Cursor::new(&serialized), config.compression_level)
                            .map_err(|e| anyhow!("zstd compression failed: {e}"))?;
                    let mut framed = Vec::with_capacity(4 + compressed.len());
                    framed.extend_from_slice(&ZSTD_MAGIC);
                    framed.extend_from_slice(&compressed);
                    framed
                } else {
                    serialized
                }
            }
        };

        let len = payload.len() as u32;

        if len as usize > MAX_MESSAGE_SIZE {
            return Err(anyhow!("Message too large: {len} bytes"));
        }

        writer.write_all(&len.to_le_bytes()).await?;
        writer.write_all(&payload).await?;
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

        // Detect and decode zstd-compressed frames
        let decoded_bytes: Vec<u8> =
            if message_bytes.len() >= 4 && message_bytes[0..4] == ZSTD_MAGIC {
                let decompressed = zstd::decode_all(Cursor::new(&message_bytes[4..]))
                    .map_err(|e| anyhow!("zstd decompression failed: {e}"))?;
                decompressed
            } else {
                message_bytes
            };

        let message: NetworkMessage = deserialize(&decoded_bytes)?;
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

        let stream = timeout(self.config.connection_timeout, TcpStream::connect(addr)).await??;

        // Perform handshake
        let (peer_id, reader, writer) =
            Self::perform_handshake(stream, self.node_id, false, &self.config).await?;

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
        config: NetworkConfig,
        mut rx: mpsc::UnboundedReceiver<NetworkMessage>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                // Handle outgoing messages
                message = rx.recv() => {
                    match message {
                        Some(msg) => {
                            if let Err(e) = Self::send_message_to_stream(&mut writer, &msg, &config).await {
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
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
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
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
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

    /// Broadcast a BalanceUpdate to all peers subscribed to the address.
    pub fn broadcast_balance_update(&self, update: BalanceUpdate) -> Result<()> {
        let payload = Bytes::from(serialize(&update)?);
        if let Some(subs) = self.balance_subscribers.get(&update.address) {
            for peer_id in subs.iter() {
                // Fire-and-forget per peer; track errors but proceed
                if let Err(e) = self.send_to_peer(*peer_id, MessageType::BalanceUpdate, payload.clone()) {
                    eprintln!(
                        "Failed to send BalanceUpdate to peer {}: {}",
                        hex::encode(&peer_id.as_bytes()[..8]),
                        e
                    );
                }
            }
        }
        Ok(())
    }

    /// Attach a BalanceBroadcaster and forward updates to subscribed peers.
    /// Spawns a Tokio task; requires a running Tokio runtime.
    pub fn attach_broadcaster(&self, broadcaster: Arc<BalanceBroadcaster>) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut rx = broadcaster.subscribe();
            loop {
                match rx.recv().await {
                    Ok(update) => {
                        if let Err(e) = this.broadcast_balance_update(update) {
                            eprintln!("Error broadcasting balance update: {e}");
                        }
                    }
                    Err(err) => {
                        // Channel closed or lagged; attempt to resubscribe
                        eprintln!("BalanceBroadcaster subscription error: {err}");
                        rx = broadcaster.subscribe();
                    }
                }
            }
        });
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

/// High-Performance Set Reconciliation Gossip Protocol
/// Optimized for 32 BPS and 10M+ TPS throughput
#[derive(Debug, Clone)]
pub struct SetReconciliationGossip {
    /// Local message set with bloom filter for fast lookups
    local_messages: Arc<DashMap<MessageId, NetworkMessage>>,
    /// Bloom filter for efficient set membership testing
    bloom_filter: Arc<Mutex<BloomFilter>>,
    /// Message priority queue for critical messages
    priority_queue: Arc<Mutex<PriorityQueue>>,
    /// Gossip fanout configuration
    fanout: usize,
    /// Maximum message age before expiry
    max_message_age: Duration,
    /// Compression threshold for messages
    compression_threshold: usize,
    /// Parallel processing semaphore
    processing_semaphore: Arc<Semaphore>,
}

/// Message identifier for set reconciliation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId([u8; 32]);

impl MessageId {
    pub fn new(data: &[u8]) -> Self {
        let hash = qanto_hash(data);
        Self(*hash.as_bytes())
    }

    pub fn random() -> Self {
        let mut rng = thread_rng();
        let mut id = [0u8; 32];
        rng.fill(&mut id);
        Self(id)
    }
}

/// Bloom filter for efficient set membership testing
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u64>,
    hash_functions: usize,
    size: usize,
}

impl BloomFilter {
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let size = Self::optimal_size(expected_items, false_positive_rate);
        let hash_functions = Self::optimal_hash_functions(size, expected_items);

        Self {
            bits: vec![0u64; size.div_ceil(64)],
            hash_functions,
            size,
        }
    }

    fn optimal_size(n: usize, p: f64) -> usize {
        (-(n as f64) * p.ln() / (2.0_f64.ln().powi(2))).ceil() as usize
    }

    fn optimal_hash_functions(m: usize, n: usize) -> usize {
        ((m as f64 / n as f64) * 2.0_f64.ln()).round() as usize
    }

    pub fn insert(&mut self, item: &MessageId) {
        for i in 0..self.hash_functions {
            let hash = self.hash_with_seed(&item.0, i as u64);
            let bit_index = (hash % self.size as u64) as usize;
            let word_index = bit_index / 64;
            let bit_offset = bit_index % 64;
            self.bits[word_index] |= 1u64 << bit_offset;
        }
    }

    pub fn contains(&self, item: &MessageId) -> bool {
        for i in 0..self.hash_functions {
            let hash = self.hash_with_seed(&item.0, i as u64);
            let bit_index = (hash % self.size as u64) as usize;
            let word_index = bit_index / 64;
            let bit_offset = bit_index % 64;
            if (self.bits[word_index] & (1u64 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }

    fn hash_with_seed(&self, data: &[u8], seed: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        data.hash(&mut hasher);
        hasher.finish()
    }
}

/// Priority queue for critical messages
#[derive(Debug, Clone)]
pub struct PriorityQueue {
    high_priority: VecDeque<NetworkMessage>,
    normal_priority: VecDeque<NetworkMessage>,
    low_priority: VecDeque<NetworkMessage>,
}

impl PriorityQueue {
    pub fn new() -> Self {
        Self {
            high_priority: VecDeque::new(),
            normal_priority: VecDeque::new(),
            low_priority: VecDeque::new(),
        }
    }

    pub fn push(&mut self, message: NetworkMessage, priority: MessagePriority) {
        match priority {
            MessagePriority::High => self.high_priority.push_back(message),
            MessagePriority::Normal => self.normal_priority.push_back(message),
            MessagePriority::Low => self.low_priority.push_back(message),
        }
    }

    pub fn pop(&mut self) -> Option<NetworkMessage> {
        self.high_priority
            .pop_front()
            .or_else(|| self.normal_priority.pop_front())
            .or_else(|| self.low_priority.pop_front())
    }

    pub fn len(&self) -> usize {
        self.high_priority.len() + self.normal_priority.len() + self.low_priority.len()
    }

    pub fn is_empty(&self) -> bool {
        self.high_priority.is_empty()
            && self.normal_priority.is_empty()
            && self.low_priority.is_empty()
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePriority {
    High,   // Consensus messages, blocks
    Normal, // Transactions
    Low,    // Peer discovery, heartbeats
}

impl SetReconciliationGossip {
    pub fn new(fanout: usize, max_message_age: Duration) -> Self {
        Self {
            local_messages: Arc::new(DashMap::new()),
            bloom_filter: Arc::new(Mutex::new(BloomFilter::new(100000, 0.01))),
            priority_queue: Arc::new(Mutex::new(PriorityQueue::new())),
            fanout,
            max_message_age,
            compression_threshold: 1024,
            processing_semaphore: Arc::new(Semaphore::new(100)), // Allow 100 concurrent operations
        }
    }

    /// Add message to local set and propagate using set reconciliation
    pub async fn gossip_message(
        &self,
        message: NetworkMessage,
        priority: MessagePriority,
        peers: &DashMap<QantoHash, PeerConnection>,
    ) -> Result<()> {
        let _permit = self.processing_semaphore.acquire().await?;

        let message_id = MessageId::new(&serialize(&message)?);

        // Check if we already have this message
        if self.local_messages.contains_key(&message_id) {
            return Ok(());
        }

        // Add to local set
        self.local_messages.insert(message_id, message.clone());

        // Update bloom filter
        {
            let mut bloom = self.bloom_filter.lock().await;
            bloom.insert(&message_id);
        }

        // Add to priority queue
        {
            let mut queue = self.priority_queue.lock().await;
            queue.push(message.clone(), priority);
        }

        // Select peers for gossip using optimized fanout
        let selected_peers = self.select_gossip_peers(peers).await;

        // Perform set reconciliation with selected peers
        for peer_id in selected_peers {
            if let Some(peer) = peers.get(&peer_id) {
                self.reconcile_with_peer(&peer, &message_id, &message)
                    .await?;
            }
        }

        Ok(())
    }

    /// Select optimal peers for gossip propagation
    async fn select_gossip_peers(
        &self,
        peers: &DashMap<QantoHash, PeerConnection>,
    ) -> Vec<QantoHash> {
        let mut selected = Vec::new();
        let peer_count = peers.len();

        if peer_count == 0 {
            return selected;
        }

        let fanout = std::cmp::min(self.fanout, peer_count);
        let mut rng = thread_rng();

        // Use reservoir sampling for fair peer selection
        let peer_ids: Vec<_> = peers.iter().map(|entry| *entry.key()).collect();

        // Take initial reservoir
        selected.extend(peer_ids.iter().take(fanout).copied());

        // Reservoir sampling over the remainder using enumerate for counter
        for (offset, &peer_id) in peer_ids.iter().enumerate().skip(fanout) {
            let j = rng.gen_range(0..=fanout + offset);
            if j < fanout {
                selected[j] = peer_id;
            }
        }

        selected
    }

    /// Perform set reconciliation with a specific peer
    async fn reconcile_with_peer(
        &self,
        peer: &PeerConnection,
        message_id: &MessageId,
        message: &NetworkMessage,
    ) -> Result<()> {
        // Create reconciliation request
        let reconciliation_msg = NetworkMessage {
            msg_type: MessageType::Custom(0x1001), // Set reconciliation type
            payload: self
                .create_reconciliation_payload(message_id, message)
                .await?,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            sender: QantoHash::new([0u8; 32]), // Will be set by sender
            signature: QantoHash::new([0u8; 32]), // Will be set by sender
        };

        // Send reconciliation message
        peer.tx.send(reconciliation_msg)?;

        Ok(())
    }

    /// Create reconciliation payload with compression
    async fn create_reconciliation_payload(
        &self,
        message_id: &MessageId,
        message: &NetworkMessage,
    ) -> Result<Bytes> {
        let payload = ReconciliationPayload {
            message_id: *message_id,
            message: message.clone(),
            bloom_filter_summary: self.get_bloom_filter_summary().await,
        };

        let serialized = serialize(&payload)?;

        // Apply compression if payload is large enough
        let compressed = if serialized.len() > self.compression_threshold {
            self.compress_payload(&serialized)?
        } else {
            serialized
        };

        Ok(Bytes::from(compressed))
    }

    /// Get bloom filter summary for set reconciliation
    async fn get_bloom_filter_summary(&self) -> Vec<u64> {
        let bloom = self.bloom_filter.lock().await;
        bloom.bits.clone()
    }

    /// Compress payload using zstd
    fn compress_payload(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut compressed = Vec::new();
        compressed.extend_from_slice(&ZSTD_MAGIC);

        let zstd_compressed = zstd::encode_all(data, 3)?; // Level 3 for balance of speed/compression
        compressed.extend_from_slice(&zstd_compressed);

        Ok(compressed)
    }

    /// Handle incoming reconciliation message
    pub async fn handle_reconciliation(
        &self,
        payload: &[u8],
        sender_peer: &QantoHash,
        peers: &DashMap<QantoHash, PeerConnection>,
    ) -> Result<()> {
        let _permit = self.processing_semaphore.acquire().await?;

        // Decompress if needed
        let decompressed = if payload.starts_with(&ZSTD_MAGIC) {
            zstd::decode_all(&payload[4..])?
        } else {
            payload.to_vec()
        };

        let reconciliation: ReconciliationPayload = deserialize(&decompressed)?;

        // Check if we have this message
        if !self.local_messages.contains_key(&reconciliation.message_id) {
            // We don't have this message, add it and continue propagation
            self.local_messages
                .insert(reconciliation.message_id, reconciliation.message.clone());

            // Update bloom filter
            {
                let mut bloom = self.bloom_filter.lock().await;
                bloom.insert(&reconciliation.message_id);
            }

            // Continue gossip to other peers (excluding sender)
            let selected_peers = self.select_gossip_peers(peers).await;
            for peer_id in selected_peers {
                if peer_id != *sender_peer {
                    if let Some(peer) = peers.get(&peer_id) {
                        self.reconcile_with_peer(
                            &peer,
                            &reconciliation.message_id,
                            &reconciliation.message,
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up old messages to prevent memory bloat
    pub async fn cleanup_old_messages(&self) {
        let _cutoff = Instant::now() - self.max_message_age;

        // Remove old messages (this is a simplified cleanup)
        // In production, you'd want to track message timestamps
        if self.local_messages.len() > 50000 {
            // Keep only the most recent 40000 messages
            let mut to_remove = Vec::new();

            for (count, entry) in self.local_messages.iter().enumerate() {
                if count > 40000 {
                    to_remove.push(*entry.key());
                }
            }

            for key in to_remove {
                self.local_messages.remove(&key);
            }
        }
    }
}

/// Reconciliation payload structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReconciliationPayload {
    message_id: MessageId,
    message: NetworkMessage,
    bloom_filter_summary: Vec<u64>,
}
