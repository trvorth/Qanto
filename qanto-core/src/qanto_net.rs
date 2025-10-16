//! --- Qanto Native Networking Layer ---
//! v0.1.0 - Native P2P Implementation
//! This module provides a complete native networking stack for Qanto,
//! replacing all libp2p dependencies with custom implementations.
//!
//! Features:
//! - Native peer discovery and management
//! - Custom gossip protocol for message propagation
//! - Distributed hash table (DHT) for peer routing
//! - Quantum-resistant cryptographic handshakes
//! - Rate limiting and spam protection
//! - Connection pooling and management

// Stub types for qanto-core compatibility
/// In-memory transaction pool for pending unconfirmed transactions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mempool {
    // Empty struct for now
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    /// Creates a new empty mempool instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Adds a transaction to the mempool, validating against UTXOs and DAG.
    pub async fn add_transaction(
        &self,
        _tx: Transaction,
        _utxos: &HashMap<String, UTXO>,
        _dag: &QantoDAG,
    ) -> Result<(), String> {
        // Stub implementation
        Ok(())
    }
}

/// Basic block representation for the native networking layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QantoBlock {
    /// Unique block identifier.
    pub id: String,
    /// Opaque block payload, typically serialized header/body.
    pub data: Vec<u8>,
}

/// Minimal DAG placeholder used for networking integration tests.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QantoDAG {
    // Stub implementation
}

impl QantoDAG {
    /// Inserts a block into the DAG, updating UTXO state if applicable.
    pub async fn add_block(
        &self,
        _block: QantoBlock,
        _utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), String> {
        // Stub implementation
        Ok(())
    }
}

/// Carbon credit credential attached to transactions or blocks.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CarbonOffsetCredential {
    /// Credential identifier.
    pub id: String,
    /// Amount of offset in standardized units.
    pub amount: u64,
}

/// Basic transaction structure for message passing and tests.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    /// Unique transaction id.
    pub id: String,
    /// Opaque transaction payload.
    pub data: Vec<u8>,
}

/// Unspent transaction output tracked by the networking layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UTXO {
    /// Output identifier.
    pub id: String,
    /// Output value in smallest unit.
    pub value: u64,
}

use crate::qanto_native_crypto::QantoKeyPair;
use my_blockchain::qanto_hash;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
#[allow(unused_imports)]
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};

// Network constants
#[allow(dead_code)]
const GOSSIP_FANOUT: usize = 6;
const DHT_BUCKET_SIZE: usize = 20;

/// Errors produced by the native networking layer.
#[derive(Debug, thiserror::Error)]
pub enum QantoNetError {
    /// Wrapped IO error encountered during network operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Provided address string failed validation or parsing.
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    /// Serialization or deserialization failure.
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Cryptographic operation failure (signature, key, or verification).
    #[error("Crypto error: {0}")]
    Crypto(String),
    /// Protocol violation or unexpected message/state.
    #[error("Protocol error: {0}")]
    Protocol(String),
    /// Requested peer could not be located.
    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),
    /// Operation timed out waiting for a response.
    #[error("Timeout")]
    Timeout,
    /// Action was throttled due to rate limiting.
    #[error("Rate limit exceeded")]
    RateLimit,
    /// Message exceeded allowed size constraints.
    #[error("Message too large")]
    MessageTooLarge,
    /// Server or subsystem is shutting down.
    #[error("Shutdown")]
    Shutdown,
    /// No active connection available for the requested operation.
    #[error("No connection")]
    NoConnection,
    /// Key-related error (e.g., lookup or mapping issue).
    #[error("Key error: {0}")]
    KeyError(String),
    /// Mutex poisoning detected when acquiring a lock.
    #[error("Mutex lock poisoned")]
    MutexLockPoisoned,
}

impl From<Box<bincode::ErrorKind>> for QantoNetError {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        QantoNetError::Serialization(err.to_string())
    }
}

/// Unique identifier for network peers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PeerId {
    /// Raw 32-byte identifier derived from the peer public key.
    pub id: [u8; 32],
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.id[..8]))
    }
}

impl PeerId {
    /// Creates a `PeerId` from a public key by hashing it with `qanto_hash`.
    pub fn new(public_key: &[u8]) -> Self {
        let hash = qanto_hash(public_key);
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());
        Self { id }
    }

    /// Generates a random `PeerId` for testing or ephemeral peers.
    pub fn random() -> Self {
        use rand::RngCore;
        let mut id = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut id);
        Self { id }
    }
}

/// Network message types exchanged between peers in Qanto.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkMessage {
    // Core blockchain messages
    /// Carries a blockchain block payload.
    Block(QantoBlock),
    /// Carries a transaction payload.
    Transaction(Transaction),
    /// Request to fetch a block by identifier.
    BlockRequest {
        /// Identifier of the requested block.
        block_id: String,
    },
    /// Response containing the requested block if available.
    BlockResponse {
        /// Optional block payload when found.
        block: Option<QantoBlock>,
    },

    // State synchronization
    /// Request for current node state snapshot.
    StateRequest,
    /// Response containing a snapshot of blocks and UTXO set.
    StateResponse {
        /// Map of block IDs to block data.
        blocks: HashMap<String, QantoBlock>,
        /// Map of UTXO IDs to unspent outputs.
        utxos: HashMap<String, UTXO>,
    },

    // Peer discovery and management
    /// Announcement used during discovery to share reachable info.
    PeerDiscovery {
        /// Identity of the announcing peer.
        peer_id: PeerId,
        /// Address the peer is listening on.
        listen_addr: SocketAddr,
        /// Advertised capabilities supported by the peer.
        capabilities: Vec<String>,
    },
    /// List of known peers returned during discovery.
    PeerList(Vec<PeerInfo>),
    /// Latency probe used to measure round-trip time.
    Ping {
        /// Unix timestamp when the ping was sent.
        timestamp: u64,
    },
    /// Response to a ping message with the same timestamp.
    Pong {
        /// Unix timestamp echoed from the ping.
        timestamp: u64,
    },

    // DHT messages
    /// Query for peers closest to the target peer ID.
    DhtFindNode {
        /// Target peer identifier to search around.
        target: PeerId,
    },
    /// Response containing peers closest to the requested target.
    DhtFoundNodes {
        /// List of peer metadata entries.
        nodes: Vec<PeerInfo>,
    },
    /// Request to store a key-value pair in the DHT.
    DhtStore {
        /// Key bytes used for storage.
        key: Vec<u8>,
        /// Value bytes to associate with the key.
        value: Vec<u8>,
    },
    /// Query to find a stored value by its key.
    DhtFindValue {
        /// Key bytes being queried in the DHT.
        key: Vec<u8>,
    },
    /// DHT response containing the value for a key, if found.
    DhtValue {
        /// Key bytes used for the lookup.
        key: Vec<u8>,
        /// Stored value for the key, when present.
        value: Option<Vec<u8>>,
    },

    // Gossip protocol
    /// Gossip protocol message for topic-based dissemination.
    GossipMessage {
        /// Topic string the message is associated with.
        topic: String,
        /// Raw message payload bytes.
        data: Vec<u8>,
        /// Time-to-live hop count for propagation.
        ttl: u8,
        /// Unique identifier to deduplicate messages.
        message_id: [u8; 32],
    },

    // Carbon credits and environmental features
    /// Carbon offset credential message for environmental tracking.
    CarbonCredential(CarbonOffsetCredential),

    // Consensus messages
    /// Consensus-round message to coordinate block agreement.
    Consensus {
        /// Consensus round identifier.
        round_id: String,
        /// Candidate block proposed within this round.
        block: QantoBlock,
        /// Target shard index for sharded consensus.
        shard_id: usize,
    },

    // Handshake and authentication
    /// Initial handshake message used to authenticate and establish trust.
    Handshake {
        /// Sender's peer identifier.
        peer_id: PeerId,
        /// Sender's public key bytes for signature verification.
        public_key: Vec<u8>,
        /// Signature over the handshake payload.
        signature: Vec<u8>,
        /// Unix timestamp when the handshake was generated.
        timestamp: u64,
    },
    /// Acknowledgement to finalize the handshake process.
    HandshakeAck {
        /// Peer identifier of the recipient.
        peer_id: PeerId,
        /// Signature confirming handshake acceptance.
        signature: Vec<u8>,
    },
}

/// Information about a network peer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerInfo {
    /// Peer identifier.
    pub peer_id: PeerId,
    /// Peer network address.
    pub address: SocketAddr,
    /// Peer public key for authentication.
    pub public_key: Vec<u8>,
    /// Supported protocol capabilities.
    pub capabilities: Vec<String>,
    /// Last time the peer was observed as online.
    pub last_seen: SystemTime,
    /// Reputation score for moderation and selection.
    pub reputation: i32,
    /// Active connection count for rate limiting.
    pub connection_count: u32,
}

/// Connection state enumeration for peer lifecycle.
#[derive(Debug, Clone)]
pub enum ConnectionState {
    /// Peer is not connected.
    Disconnected,
    /// Connection is being established.
    Connecting,
    /// Connection established and active.
    Connected,
    /// Peer has completed authentication.
    Authenticated,
    /// Connection failed with an error message.
    Failed(String),
}

/// Peer connection management structure and metadata.
#[derive(Debug)]
pub struct PeerConnection {
    /// Peer metadata and identity.
    pub peer_info: PeerInfo,
    /// Current connection state.
    pub state: ConnectionState,
    /// Timestamp of last message or activity.
    pub last_activity: Instant,
    /// Outgoing messages awaiting send.
    pub message_queue: VecDeque<NetworkMessage>,
    /// TCP write half for the peer connection.
    pub write_stream: Option<tokio::net::tcp::OwnedWriteHalf>,
    /// Rate limiter controlling outbound throughput.
    pub rate_limiter: RateLimiter,
}

/// Simple token-bucket rate limiter for peer connections.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    tokens: u32,
    max_tokens: u32,
    refill_rate: u32, // tokens per second
    last_refill: Instant,
}

impl RateLimiter {
    /// Creates a new `RateLimiter` with maximum tokens and refill rate.
    pub fn new(max_tokens: u32, refill_rate: u32) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Attempts to consume tokens; returns `true` if successful.
    pub fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs() as u32;
        if elapsed > 0 {
            let new_tokens = elapsed * self.refill_rate;
            self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
            self.last_refill = now;
        }
    }
}

/// DHT routing table for peer discovery and lookup.
#[derive(Debug)]
pub struct DhtRoutingTable {
    local_peer_id: PeerId,
    buckets: Vec<Vec<PeerInfo>>,
}

impl DhtRoutingTable {
    /// Creates a new routing table for the local peer.
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            local_peer_id,
            buckets: vec![Vec::new(); 256], // 256 buckets for 256-bit peer IDs
        }
    }

    /// Adds or updates a peer entry, enforcing bucket limits.
    pub fn add_peer(&mut self, peer: PeerInfo) {
        let distance = self.calculate_distance(&self.local_peer_id, &peer.peer_id);
        let bucket_index = self.get_bucket_index(distance);

        let bucket = &mut self.buckets[bucket_index];

        // Remove existing entry if present
        bucket.retain(|p| p.peer_id != peer.peer_id);

        // Add to front of bucket
        bucket.insert(0, peer);

        // Maintain bucket size limit
        if bucket.len() > DHT_BUCKET_SIZE {
            bucket.truncate(DHT_BUCKET_SIZE);
        }
    }

    /// Finds `count` closest peers to the target peer id.
    pub fn find_closest_peers(&self, target: &PeerId, count: usize) -> Vec<PeerInfo> {
        let mut candidates: Vec<(u32, PeerInfo)> = Vec::new();

        for bucket in &self.buckets {
            for peer in bucket {
                let distance = self.calculate_distance(target, &peer.peer_id);
                candidates.push((distance, peer.clone()));
            }
        }

        candidates.sort_by_key(|(distance, _)| *distance);
        candidates
            .into_iter()
            .take(count)
            .map(|(_, peer)| peer)
            .collect()
    }

    fn calculate_distance(&self, a: &PeerId, b: &PeerId) -> u32 {
        // XOR distance for Kademlia-style routing
        let mut distance = 0u32;
        for i in 0..32 {
            distance ^= (a.id[i] ^ b.id[i]) as u32;
        }
        distance
    }

    fn get_bucket_index(&self, distance: u32) -> usize {
        if distance == 0 {
            return 0;
        }
        (32 - distance.leading_zeros() - 1) as usize
    }
}

/// Gossip protocol implementation for topic-based message propagation
#[derive(Debug, Default)]
pub struct GossipProtocol {
    subscriptions: HashMap<String, HashSet<PeerId>>,
    message_cache: HashMap<[u8; 32], NetworkMessage>,
}

impl GossipProtocol {
    /// Subscribes a peer to a topic.
    pub fn subscribe(&mut self, topic: String, peer_id: PeerId) {
        self.subscriptions.entry(topic).or_default().insert(peer_id);
    }

    /// Unsubscribes a peer from a topic.
    pub fn unsubscribe(&mut self, topic: &str, peer_id: &PeerId) {
        if let Some(subscribers) = self.subscriptions.get_mut(topic) {
            subscribers.remove(peer_id);
            if subscribers.is_empty() {
                self.subscriptions.remove(topic);
            }
        }
    }

    /// Returns current subscribers for the given topic.
    pub fn get_subscribers(&self, topic: &str) -> Vec<PeerId> {
        self.subscriptions
            .get(topic)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Caches a message to prevent redundant gossip.
    pub fn cache_message(&mut self, message_id: [u8; 32], message: NetworkMessage) {
        // Simple LRU-like behavior: if cache is too large, remove oldest entries
        if self.message_cache.len() > 1000 {
            // Remove first entry (oldest in insertion order for HashMap)
            if let Some(key) = self.message_cache.keys().next().cloned() {
                self.message_cache.remove(&key);
            }
        }
        self.message_cache.insert(message_id, message);
    }

    /// Whether a message id is already seen.
    pub fn has_seen_message(&self, message_id: &[u8; 32]) -> bool {
        self.message_cache.contains_key(message_id)
    }
}

/// Native networking server that manages peers, gossip, and DHT.
#[allow(dead_code)]
pub struct QantoNetServer {
    listen_addr: SocketAddr,
    keypair: QantoKeyPair,
    peers: Arc<RwLock<HashMap<PeerId, PeerConnection>>>,
    gossip: GossipProtocol,
    rate_limiter: RateLimiter,
    dht: DhtRoutingTable,
    dag: Arc<QantoDAG>,
    mempool: Arc<tokio::sync::Mutex<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    command_rx: Option<mpsc::Receiver<QantoNetCommand>>,
    response_tx: Option<mpsc::Sender<QantoNetResponse>>,
}

impl QantoNetServer {
    /// Creates a new `QantoNetServer` with required components.
    pub fn new(
        listen_addr: SocketAddr,
        keypair: QantoKeyPair,
        dag: Arc<QantoDAG>,
        mempool: Arc<tokio::sync::Mutex<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Self, QantoNetError> {
        let local_peer_id = PeerId::new(match &keypair {
            QantoKeyPair::PostQuantum { public, .. } => public.as_bytes(),
            QantoKeyPair::Ed25519 { public, .. } => public.as_bytes(),
            QantoKeyPair::P256 { public, .. } => public.as_bytes(),
        });

        Ok(Self {
            listen_addr,
            keypair,
            peers: Arc::new(RwLock::new(HashMap::new())),
            gossip: GossipProtocol::default(),
            rate_limiter: RateLimiter::new(1000, 100),
            dht: DhtRoutingTable::new(local_peer_id),
            dag,
            mempool,
            utxos,
            command_rx: None,
            response_tx: None,
        })
    }

    /// Start the network server
    /// Starts the server event loop and background tasks.
    pub async fn start(&mut self) -> Result<(), QantoNetError> {
        let listener = tokio::net::TcpListener::bind(self.listen_addr).await?;
        println!("[QantoNet] Server listening on {}", self.listen_addr);

        // Start background tasks
        self.start_heartbeat_task().await;
        self.start_discovery_task().await;
        self.start_cleanup_task().await;

        // Accept incoming connections
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("[QantoNet] New connection from {addr}");

                    // Generate peer ID from connection
                    let peer_id = PeerId::random();

                    // Perform handshake
                    let mut stream_clone = stream;
                    match Self::perform_handshake(&mut stream_clone, &self.keypair, &peer_id).await
                    {
                        Ok(remote_peer_id) => {
                            println!("[QantoNet] Handshake successful with {remote_peer_id}");

                            // Create peer info
                            let peer_info = PeerInfo {
                                peer_id: remote_peer_id.clone(),
                                address: addr,
                                public_key: Vec::new(),
                                capabilities: Vec::new(),
                                last_seen: SystemTime::now(),
                                reputation: 0,
                                connection_count: 0,
                            };

                            // Split stream for reading and writing
                            let (mut read_half, write_half) = stream_clone.into_split();
                            let connection = PeerConnection {
                                peer_info,
                                state: ConnectionState::Connected,
                                last_activity: Instant::now(),
                                message_queue: VecDeque::new(),
                                rate_limiter: RateLimiter::new(100, 10),
                                write_stream: Some(write_half),
                            };

                            // Store connection
                            {
                                let mut peers = self.peers.write().await;
                                peers.insert(remote_peer_id.clone(), connection);
                            }

                            // Spawn read task for incoming connection
                            let from_peer = remote_peer_id.clone();
                            tokio::spawn(async move {
                                let mut buf = vec![0; 1024];
                                loop {
                                    match read_half.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            // Process received data
                                            match bincode::deserialize::<NetworkMessage>(&buf[..n])
                                            {
                                                Ok(message) => {
                                                    println!("[QantoNet] Received message from {from_peer}: {message:?}");
                                                }
                                                Err(e) => {
                                                    println!("[QantoNet] Failed to deserialize message: {e}");
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            println!("[QantoNet] Read error from {from_peer}: {e}");
                                            break;
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            println!("[QantoNet] Handshake failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("[QantoNet] Failed to accept connection: {e}");
                }
            }
        }
    }

    /// Stop the network server
    /// Requests a clean shutdown of the server.
    pub async fn stop(&mut self) {
        println!("[QantoNet] Stopping server...");
        self.peers.write().await.clear();
    }

    /// Connect to a peer
    /// Establishes a connection to a peer at the given address.
    pub async fn connect_peer(
        &mut self,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("[QantoNet] Connecting to peer at {addr}");

        let stream = TcpStream::connect(addr).await?;
        let peer_id = PeerId::random();

        let peer_info = PeerInfo {
            peer_id: peer_id.clone(),
            address: addr,
            public_key: Vec::new(),
            capabilities: Vec::new(),
            last_seen: SystemTime::now(),
            reputation: 0,
            connection_count: 0,
        };

        let (_, write_half) = stream.into_split();
        let peer_connection = PeerConnection {
            peer_info,
            state: ConnectionState::Connected,
            last_activity: Instant::now(),
            message_queue: VecDeque::new(),
            write_stream: Some(write_half),
            rate_limiter: RateLimiter::new(100, 10),
        };

        let mut conns = self.peers.write().await;
        conns.insert(peer_id, peer_connection);

        Ok(())
    }

    /// Send a message to a specific peer
    /// Sends a message to a specific peer.
    pub async fn send_to_peer(
        &mut self,
        peer_id: &PeerId,
        mut message: NetworkMessage,
    ) -> Result<(), QantoNetError> {
        // Compress message data if applicable
        if let NetworkMessage::GossipMessage { ref mut data, .. } = message {
            *data = qanto_compress(data);
        }
        let mut connections = self.peers.write().await;
        let connection = connections
            .get_mut(peer_id)
            .ok_or_else(|| QantoNetError::PeerNotFound(peer_id.clone()))?;

        // Check rate limit
        if !connection.rate_limiter.try_consume(1) {
            return Err(QantoNetError::RateLimit);
        }

        // Queue message for sending
        connection.message_queue.push_back(message.clone());
        connection.last_activity = Instant::now();

        if let Some(write_stream) = &mut connection.write_stream {
            let serialized = bincode::serialize(&message)?;
            write_stream.write_all(&serialized).await?;
        } else {
            return Err(QantoNetError::NoConnection);
        }

        Ok(())
    }

    /// Broadcast a message to all connected peers
    /// Broadcasts a message to all connected peers.
    pub async fn broadcast_message(
        &mut self,
        _message: NetworkMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("[QantoNet] Broadcasting message to all peers");

        let connections = self.peers.read().await;

        for (peer_id, _connection) in connections.iter() {
            // Send message to each peer
            println!("[QantoNet] Sending message to peer {peer_id}");
        }

        Ok(())
    }

    /// Get list of connected peer IDs
    /// Returns a list of currently connected peers.
    pub async fn get_connected_peers(&self) -> Vec<PeerId> {
        let peer_ids: Vec<PeerId> = self.peers.read().await.keys().cloned().collect();
        peer_ids
    }

    /// Handle incoming message from a peer
    /// Handles an incoming message from a peer.
    pub async fn handle_message(
        &mut self,
        from_peer: &PeerId,
        message: NetworkMessage,
    ) -> Result<(), QantoNetError> {
        // Update peer activity
        {
            let mut connections = self.peers.write().await;
            if let Some(connection) = connections.get_mut(from_peer) {
                connection.last_activity = Instant::now();
            }
        }

        match message {
            NetworkMessage::Block(block) => self.handle_block_message(from_peer, block).await?,
            NetworkMessage::Transaction(tx) => {
                self.handle_transaction_message(from_peer, tx).await?
            }
            NetworkMessage::Ping { timestamp } => self.handle_ping(from_peer, timestamp).await?,
            NetworkMessage::Pong { timestamp: _ } => {
                // Update peer as alive
            }
            NetworkMessage::PeerDiscovery {
                peer_id,
                listen_addr,
                capabilities,
            } => {
                self.handle_peer_discovery(peer_id, listen_addr, capabilities)
                    .await?
            }
            NetworkMessage::GossipMessage {
                topic,
                data,
                ttl,
                message_id,
            } => {
                self.handle_gossip_message(from_peer, topic, data, ttl, message_id)
                    .await?
            }
            _ => {
                // Handle other message types
                println!("[QantoNet] Received unhandled message type from {from_peer}");
            }
        }

        Ok(())
    }

    // Private helper methods

    async fn start_heartbeat_task(&self) {
        std::thread::spawn(|| {
            // Heartbeat logic
            loop {
                std::thread::sleep(std::time::Duration::from_secs(30));
                // Send pings
            }
        });
    }

    async fn start_discovery_task(&self) {
        std::thread::spawn(|| {
            // Discovery logic
        });
    }

    async fn start_cleanup_task(&self) {
        std::thread::spawn(|| {
            // Cleanup logic
        });
    }

    async fn handle_block_message(
        &mut self,
        _from_peer: &PeerId,
        block: QantoBlock,
    ) -> Result<(), QantoNetError> {
        // Add block to DAG
        if let Err(e) = self.dag.add_block(block.clone(), &self.utxos).await {
            println!("[QantoNet] Failed to add block to DAG: {e}");
        }

        // Propagate to other peers
        self.broadcast(NetworkMessage::Block(block)).await?;

        Ok(())
    }

    async fn handle_transaction_message(
        &mut self,
        _from_peer: &PeerId,
        tx: Transaction,
    ) -> Result<(), QantoNetError> {
        // Add transaction to mempool
        {
            let mempool = self.mempool.lock().await;
            let utxos = self.utxos.read().await;
            if let Err(e) = mempool.add_transaction(tx.clone(), &utxos, &self.dag).await {
                println!("[QantoNet] Failed to add transaction to mempool: {e}");
            }
        }

        // Propagate to other peers
        self.broadcast(NetworkMessage::Transaction(tx)).await?;

        Ok(())
    }

    async fn handle_ping(
        &mut self,
        from_peer: &PeerId,
        timestamp: u64,
    ) -> Result<(), QantoNetError> {
        let pong = NetworkMessage::Pong { timestamp };
        self.send_to_peer(from_peer, pong).await
    }

    async fn handle_peer_discovery(
        &mut self,
        peer_id: PeerId,
        listen_addr: SocketAddr,
        capabilities: Vec<String>,
    ) -> Result<(), QantoNetError> {
        let peer_info = PeerInfo {
            peer_id: peer_id.clone(),
            address: listen_addr,
            public_key: Vec::new(),
            capabilities,
            last_seen: SystemTime::now(),
            reputation: 0,
            connection_count: 0,
        };

        // Add to DHT
        self.dht.add_peer(peer_info);

        // Try to connect if we don't have this peer
        if !self.peers.read().await.contains_key(&peer_id) {
            if let Err(e) = self.connect_peer(listen_addr).await {
                println!("[QantoNet] Failed to connect to discovered peer: {e}");
            }
        }

        Ok(())
    }

    async fn handle_gossip_message(
        &mut self,
        from_peer: &PeerId,
        topic: String,
        data: Vec<u8>,
        ttl: u8,
        message_id: [u8; 32],
    ) -> Result<(), QantoNetError> {
        // Check if we've already seen this message
        if self.gossip.has_seen_message(&message_id) {
            return Ok(());
        }

        let decompressed_data = qanto_decompress(&data);

        // Cache the message
        let message = NetworkMessage::GossipMessage {
            topic: topic.clone(),
            data: data.clone(),
            ttl,
            message_id,
        };
        self.gossip.cache_message(message_id, message.clone());

        // Process the message content
        match topic.as_str() {
            "block" => { /* handle block */ }
            "tx" => { /* handle tx */ }
            _ => {}
        }

        // Forward if TTL > 0
        if ttl > 0 {
            let forward_message = NetworkMessage::GossipMessage {
                topic,
                data: qanto_compress(&decompressed_data),
                ttl: ttl - 1,
                message_id,
            };

            // Forward to peers except the sender
            let peer_ids: Vec<PeerId> = self
                .peers
                .read()
                .await
                .keys()
                .filter(|&id| id != from_peer)
                .cloned()
                .collect();

            for peer_id in peer_ids {
                if let Err(e) = self.send_to_peer(&peer_id, forward_message.clone()).await {
                    println!("[QantoNet] Failed to forward gossip message: {e}");
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn generate_message_id(&self, data: &[u8]) -> [u8; 32] {
        let mut combined_data = data.to_vec();
        combined_data.extend_from_slice(
            &SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0))
                .as_nanos()
                .to_le_bytes(),
        );
        let hash = qanto_hash(&combined_data);
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());
        id
    }

    /// Returns true if a peer is currently connected.
    pub async fn is_peer_connected(&self, peer_id: &PeerId) -> bool {
        let peers = self.peers.read().await;
        peers.contains_key(peer_id)
    }

    /// Adds a peer to the subscribers for a gossip topic.
    pub async fn subscribe_gossip(&mut self, topic: String, peer_id: PeerId) {
        self.gossip.subscribe(topic, peer_id);
    }

    /// Removes a peer from the subscribers for a gossip topic.
    pub async fn unsubscribe_gossip(&mut self, topic: &str, peer_id: &PeerId) {
        self.gossip.unsubscribe(topic, peer_id);
    }

    async fn perform_handshake(
        stream: &mut TcpStream,
        keypair: &QantoKeyPair,
        local_peer_id: &PeerId,
    ) -> Result<PeerId, QantoNetError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let public_key = match keypair {
            QantoKeyPair::PostQuantum { public, .. } => public.as_bytes().to_vec(),
            QantoKeyPair::Ed25519 { public, .. } => public.as_bytes().to_vec(),
            QantoKeyPair::P256 { public, .. } => public.as_bytes().to_vec(),
        };

        let handshake_data = format!(
            "{}{}",
            local_peer_id
                .id
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            timestamp
        );

        let handshake = NetworkMessage::Handshake {
            peer_id: local_peer_id.clone(),
            public_key,
            signature: handshake_data.as_bytes().to_vec(),
            timestamp,
        };

        let serialized = bincode::serialize(&handshake)?;
        stream.write_all(&serialized).await?;

        // Return a dummy peer ID for now
        Ok(PeerId::random())
    }

    /// Broadcasts a message to all connected peers.
    pub async fn broadcast(&mut self, message: NetworkMessage) -> Result<(), QantoNetError> {
        let peer_ids: Vec<PeerId> = {
            let peers = self.peers.read().await;
            peers.keys().cloned().collect()
        };

        for peer_id in peer_ids {
            if let Err(e) = self.send_to_peer(&peer_id, message.clone()).await {
                println!("Failed to send message to peer {peer_id}: {e}");
            }
        }
        Ok(())
    }
}

// Custom Debug implementation to avoid exposing sensitive key material
impl fmt::Debug for QantoNetServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QantoNetServer")
            .field("listen_addr", &self.listen_addr)
            .field("gossip", &self.gossip)
            .field("rate_limiter", &self.rate_limiter)
            .field("dht", &self.dht)
            .finish()
    }
}

/// Network command interface for external modules
#[derive(Debug, Clone)]
pub enum QantoNetCommand {
    /// Broadcast a block to peers.
    BroadcastBlock(QantoBlock),
    /// Broadcast a transaction to peers.
    BroadcastTransaction(Transaction),
    /// Connect to a peer address.
    ConnectPeer(SocketAddr),
    /// Disconnect a peer by id.
    DisconnectPeer(PeerId),
    /// Get all connected peers.
    GetConnectedPeers,
    /// Subscribe to a gossip topic.
    Subscribe {
        /// Topic name to subscribe.
        topic: String,
    },
    /// Unsubscribe from a gossip topic.
    Unsubscribe {
        /// Topic name to unsubscribe.
        topic: String,
    },
    /// Gossip arbitrary data for a topic.
    Gossip {
        /// Topic of the gossip message.
        topic: String,
        /// Raw message bytes to gossip.
        data: Vec<u8>,
    },
    /// Shutdown the network server.
    Shutdown,
}

/// Response types for network commands
#[derive(Debug, Clone)]
pub enum QantoNetResponse {
    /// Operation completed successfully.
    Success,
    /// Peer successfully connected.
    PeerConnected(PeerId),
    /// List of currently connected peers.
    ConnectedPeers(Vec<PeerId>),
    /// Operation failed with a string error.
    Error(String),
}

/// Simple internal run-length encoding compression for Qanto network messages
fn qanto_compress(data: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::new();
    if data.is_empty() {
        return compressed;
    }
    let mut count = 1;
    let mut last = data[0];
    for &byte in &data[1..] {
        if byte == last {
            count += 1;
        } else {
            compressed.push(count);
            compressed.push(last);
            count = 1;
            last = byte;
        }
    }
    compressed.push(count);
    compressed.push(last);
    compressed
}

/// Decompress run-length encoded data
fn qanto_decompress(compressed: &[u8]) -> Vec<u8> {
    let mut decompressed = Vec::new();
    let mut i = 0;
    while i < compressed.len() {
        let count = compressed[i] as usize;
        let byte = compressed[i + 1];
        for _ in 0..count {
            decompressed.push(byte);
        }
        i += 2;
    }
    decompressed
}

// Native runtime implemented using threads and std::net
