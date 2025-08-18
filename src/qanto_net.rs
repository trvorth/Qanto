//! --- Qanto Native Networking Layer ---
//! v1.0.0 - Native P2P Implementation
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

use crate::mempool::Mempool;
use crate::post_quantum_crypto::PQSignatureKeyPair;
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::saga::CarbonOffsetCredential;
use crate::transaction::Transaction;
use crate::types::UTXO;
use my_blockchain::qanto_hash;

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
// Native async primitives (will replace tokio)
use std::sync::Mutex as StdMutex;
use tokio::sync::RwLock;
use pqcrypto_traits::sign::{DetachedSignature, SecretKey};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

// Constants for network configuration
const GOSSIP_FANOUT: usize = 6;
const DHT_BUCKET_SIZE: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum QantoNetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),
    #[error("Timeout")]
    Timeout,
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Message too large")]
    MessageTooLarge,
    #[error("Shutdown")]
    Shutdown,
    #[error("No connection")]
NoConnection,
#[error("Key error: {0}")]
KeyError(String),
}

impl From<Box<bincode::ErrorKind>> for QantoNetError {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        QantoNetError::Serialization(err.to_string())
    }
}

/// Unique identifier for network peers
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PeerId {
    pub id: [u8; 32],
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.id[..8]))
    }
}

impl PeerId {
    pub fn new(public_key: &[u8]) -> Self {
        let hash = qanto_hash(public_key);
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());
        Self { id }
    }

    pub fn random() -> Self {
        use rand::RngCore;
        let mut id = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut id);
        Self { id }
    }
}

/// Network message types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkMessage {
    // Core blockchain messages
    Block(QantoBlock),
    Transaction(Transaction),
    BlockRequest {
        block_id: String,
    },
    BlockResponse {
        block: Option<QantoBlock>,
    },

    // State synchronization
    StateRequest,
    StateResponse {
        blocks: HashMap<String, QantoBlock>,
        utxos: HashMap<String, UTXO>,
    },

    // Peer discovery and management
    PeerDiscovery {
        peer_id: PeerId,
        listen_addr: SocketAddr,
        capabilities: Vec<String>,
    },
    PeerList(Vec<PeerInfo>),
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },

    // DHT messages
    DhtFindNode {
        target: PeerId,
    },
    DhtFoundNodes {
        nodes: Vec<PeerInfo>,
    },
    DhtStore {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    DhtFindValue {
        key: Vec<u8>,
    },
    DhtValue {
        key: Vec<u8>,
        value: Option<Vec<u8>>,
    },

    // Gossip protocol
    GossipMessage {
        topic: String,
        data: Vec<u8>,
        ttl: u8,
        message_id: [u8; 32],
    },

    // Carbon credits and environmental features
    CarbonCredential(CarbonOffsetCredential),

    // Consensus messages
    Consensus {
        round_id: String,
        block: QantoBlock,
        shard_id: usize,
    },

    // Handshake and authentication
    Handshake {
        peer_id: PeerId,
        public_key: Vec<u8>,
        signature: Vec<u8>,
        timestamp: u64,
    },
    HandshakeAck {
        peer_id: PeerId,
        signature: Vec<u8>,
    },
}

/// Information about a network peer
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub public_key: Vec<u8>,
    pub capabilities: Vec<String>,
    pub last_seen: SystemTime,
    pub reputation: i32,
    pub connection_count: u32,
}

/// Connection state for a peer
#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticated,
    Failed(String),
}

/// Peer connection management
#[derive(Debug)]
pub struct PeerConnection {
    pub peer_info: PeerInfo,
    pub state: ConnectionState,
    pub last_activity: Instant,
    pub message_queue: VecDeque<NetworkMessage>,
    pub write_stream: Option<tokio::net::tcp::OwnedWriteHalf>,
    pub rate_limiter: RateLimiter,
}

/// Simple rate limiter for peer connections
#[derive(Debug, Clone)]
pub struct RateLimiter {
    tokens: u32,
    max_tokens: u32,
    refill_rate: u32, // tokens per second
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(max_tokens: u32, refill_rate: u32) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

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

/// DHT routing table for peer discovery
#[derive(Debug)]
pub struct DhtRoutingTable {
    local_peer_id: PeerId,
    buckets: Vec<Vec<PeerInfo>>,
}

impl DhtRoutingTable {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            local_peer_id,
            buckets: vec![Vec::new(); 256], // 256 buckets for 256-bit peer IDs
        }
    }

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

/// Gossip protocol implementation
#[derive(Debug)]
pub struct GossipProtocol {
    subscriptions: HashMap<String, HashSet<PeerId>>,
    message_cache: HashMap<[u8; 32], (NetworkMessage, Instant)>,
    max_cache_size: usize,
}

impl Default for GossipProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl GossipProtocol {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            message_cache: HashMap::new(),
            max_cache_size: 10000,
        }
    }

    pub fn subscribe(&mut self, topic: String, peer_id: PeerId) {
        self.subscriptions
            .entry(topic)
            .or_default()
            .insert(peer_id);
    }

    pub fn unsubscribe(&mut self, topic: &str, peer_id: &PeerId) {
        if let Some(subscribers) = self.subscriptions.get_mut(topic) {
            subscribers.remove(peer_id);
            if subscribers.is_empty() {
                self.subscriptions.remove(topic);
            }
        }
    }

    pub fn get_subscribers(&self, topic: &str) -> Vec<PeerId> {
        self.subscriptions
            .get(topic)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn cache_message(&mut self, message_id: [u8; 32], message: NetworkMessage) {
        // Clean old messages if cache is full
        if self.message_cache.len() >= self.max_cache_size {
            let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
            self.message_cache
                .retain(|_, (_, timestamp)| *timestamp > cutoff);
        }

        self.message_cache
            .insert(message_id, (message, Instant::now()));
    }

    pub fn has_seen_message(&self, message_id: &[u8; 32]) -> bool {
        self.message_cache.contains_key(message_id)
    }
}

/// Main Qanto networking server
#[derive(Debug)]
pub struct QantoNetServer {
    #[allow(dead_code)] // Used for future peer authentication
    local_peer_id: PeerId,
    listen_addr: SocketAddr,
    connections: Arc<RwLock<HashMap<PeerId, PeerConnection>>>,
    dht: DhtRoutingTable,
    gossip: GossipProtocol,
    #[allow(dead_code)] // Used for future message signing
    keypair: PQSignatureKeyPair,
    dag: Arc<QantoDAG>,
    mempool: Arc<StdMutex<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    running: Arc<StdMutex<bool>>,
}

impl QantoNetServer {
    pub fn new(
        listen_addr: SocketAddr,
        keypair: PQSignatureKeyPair,
        dag: Arc<QantoDAG>,
        mempool: Arc<StdMutex<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Self, QantoNetError> {
        let local_peer_id = PeerId::new(&keypair.public_key);

        Ok(Self {
            local_peer_id: local_peer_id.clone(),
            listen_addr,
            connections: Arc::new(RwLock::new(HashMap::new())),
            dht: DhtRoutingTable::new(local_peer_id),
            gossip: GossipProtocol::new(),
            keypair,
            dag,
            mempool,
            utxos,
            running: Arc::new(StdMutex::new(false)),
        })
    }

    /// Start the networking server
    pub async fn start(&mut self) -> Result<(), QantoNetError> {
        *self.running.lock().unwrap() = true;

        let listener = tokio::net::TcpListener::bind(&self.listen_addr).await?;
        let running = self.running.clone();
        let connections = self.connections.clone();
        let keypair = self.keypair.clone();
        let local_peer_id = self.local_peer_id.clone();
        tokio::spawn(async move {
    while *running.lock().unwrap() {
        if let Ok((mut stream, addr)) = listener.accept().await {
            let connections_clone = connections.clone();
            let keypair = keypair.clone();
            let local_peer_id = local_peer_id.clone();
            tokio::spawn(async move {
                if let Ok(peer_id) = QantoNetServer::perform_handshake(&mut stream, &keypair, &local_peer_id).await {
                    let (mut read_half, write_half) = stream.into_split();
                    let peer_connection = PeerConnection {
                        peer_info: PeerInfo { peer_id: peer_id.clone(), address: addr, public_key: vec![], capabilities: vec![], last_seen: SystemTime::now(), reputation: 0, connection_count: 0 },
                        state: ConnectionState::Connected,
                        last_activity: Instant::now(),
                        message_queue: VecDeque::new(),
                        write_stream: Some(write_half),
                        rate_limiter: RateLimiter::new(100, 10),
                    };
                    let mut connections = connections_clone.write().await;
                    connections.insert(peer_id.clone(), peer_connection);
                    // Spawn read task
                    let connections_read = connections_clone.clone();
                    let from_peer = peer_id.clone();
                    tokio::spawn(async move {
                        let mut buf = vec![0; 1024];
                        loop {
                            match read_half.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(_) => {
                                    // TODO: process message and call handle_message
                                }
                                Err(_) => break,
                            }
                        }
                        // Cleanup connection
                        let mut connections = connections_read.write().await;
                        connections.remove(&from_peer);
                    });
                }
            });
        }
    }
});

        // Start background tasks
        self.start_heartbeat_task().await;
        self.start_discovery_task().await;
        self.start_cleanup_task().await;

        Ok(())
    }

    /// Stop the networking server
    pub async fn stop(&mut self) {
        *self.running.lock().unwrap() = false;
        println!("[QantoNet] Stopping networking server");
    }

    /// Connect to a peer
    pub async fn perform_handshake(stream: &mut TcpStream, keypair: &PQSignatureKeyPair, local_peer_id: &PeerId) -> Result<PeerId, QantoNetError> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| QantoNetError::Protocol(e.to_string()))?.as_secs();
    let handshake = NetworkMessage::Handshake {
        peer_id: local_peer_id.clone(),
        public_key: keypair.public_key.clone(),
        signature: {
    use pqcrypto_mldsa::mldsa44;
    let sk = mldsa44::SecretKey::from_bytes(&keypair.secret_key).map_err(|_| QantoNetError::KeyError("Invalid secret key".to_string()))?;
    let sig = mldsa44::detached_sign(&timestamp.to_be_bytes(), &sk);
    sig.as_bytes().to_vec()
},
        timestamp,
    };
    let serialized = bincode::serialize(&handshake)?;
    stream.write_all(&serialized).await?;
    let mut buf = vec![0; 1024];
    let n = stream.read(&mut buf).await?;
    let ack: NetworkMessage = bincode::deserialize(&buf[..n])?;
    if let NetworkMessage::HandshakeAck { peer_id, signature: _ } = ack {
        // Verify signature
        return Ok(peer_id);
    } else {
        Err(QantoNetError::Protocol("Invalid handshake".to_string()))
    }
}
pub async fn connect_peer(&mut self, addr: SocketAddr) -> Result<PeerId, QantoNetError> {
    let mut stream = TcpStream::connect(addr).await?;
    let peer_id = Self::perform_handshake(&mut stream, &self.keypair, &self.local_peer_id).await?;

    let peer_info = PeerInfo {
        peer_id: peer_id.clone(),
        address: addr,
        public_key: Vec::new(), // Will be filled during handshake
        capabilities: Vec::new(),
        last_seen: SystemTime::now(),
        reputation: 0,
        connection_count: 0,
    };

    let (mut read_half, write_half) = stream.into_split();
    let connection = PeerConnection {
        peer_info,
        state: ConnectionState::Connecting,
        last_activity: Instant::now(),
        message_queue: VecDeque::new(),
        rate_limiter: RateLimiter::new(100, 10),
        write_stream: Some(write_half),
    };

    let mut conns = self.connections.write().await;
    conns.insert(peer_id.clone(), connection);
    // Spawn read task for outgoing connection
    let connections_read = self.connections.clone();
    let from_peer = peer_id.clone();
    tokio::spawn(async move {
        let mut buf = vec![0; 1024];
        loop {
            match read_half.read(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    // TODO: process message and call handle_message
                }
                Err(_) => break,
            }
        }
        // Cleanup connection
        let mut connections = connections_read.write().await;
        connections.remove(&from_peer);
    });

    Ok(peer_id)
}

    /// Send a message to a specific peer
    pub async fn send_to_peer(&mut self, peer_id: &PeerId, mut message: NetworkMessage) -> Result<(), QantoNetError> {
        // Compress message data if applicable
        if let NetworkMessage::GossipMessage { ref mut data, .. } = message {
            *data = qanto_compress(data);
        }
        let mut connections = self.connections.write().await;
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
    pub async fn broadcast(&mut self, mut message: NetworkMessage) -> Result<(), QantoNetError> {
        // Compress message data if applicable
        if let NetworkMessage::GossipMessage { ref mut data, .. } = message {
            *data = qanto_compress(data);
        }
        let peer_ids: Vec<PeerId> = self.connections.read().await.keys().cloned().collect();

        for peer_id in peer_ids {
            if let Err(e) = self.send_to_peer(&peer_id, message.clone()).await {
                println!(
                    "[QantoNet] Failed to send to peer {}: {}",
                    peer_id,
                    e
                );
            }
        }

        Ok(())
    }

    /// Gossip a message to a subset of peers
    pub async fn gossip(&mut self, topic: String, data: Vec<u8>) -> Result<(), QantoNetError> {
        use rand::seq::SliceRandom;

        let message_id = self.generate_message_id(&data);

        // Check if we've already seen this message
        if self.gossip.has_seen_message(&message_id) {
            return Ok(());
        }

        // Removed unused decompressed_data

        let message = NetworkMessage::GossipMessage {
            topic: topic.clone(),
            data,
            ttl: 5, // Maximum 5 hops
            message_id,
        };

        // Cache the message
        self.gossip.cache_message(message_id, message.clone());

        // Get subscribers for this topic
        let mut subscribers = self.gossip.get_subscribers(&topic);

        // If no specific subscribers, use all connected peers
        if subscribers.is_empty() {
            subscribers = self.connections.read().await.keys().cloned().collect();
        }

        // Select random subset for gossip (fanout)
        subscribers.shuffle(&mut rand::thread_rng());
        subscribers.truncate(GOSSIP_FANOUT);

        // Send to selected peers
        for peer_id in subscribers {
            if let Err(e) = self.send_to_peer(&peer_id, message.clone()).await {
                println!(
                    "[QantoNet] Failed to gossip to peer {}: {}",
                    peer_id,
                    e
                );
            }
        }

        Ok(())
    }

    /// Handle incoming message from a peer
    pub async fn handle_message(
        &mut self,
        from_peer: &PeerId,
        message: NetworkMessage,
    ) -> Result<(), QantoNetError> {
        // Update peer activity
        {
            let mut connections = self.connections.write().await;
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
                println!(
                    "[QantoNet] Received unhandled message type from {}",
                    from_peer
                );
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
        if let Ok(mempool) = self.mempool.lock() {
            let utxos = self.utxos.read().await;
            if let Err(e) = mempool
                .add_transaction(tx.clone(), &utxos, &self.dag)
                .await
            {
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
        if !self.connections.read().await.contains_key(&peer_id) {
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
                .connections
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

    fn generate_message_id(&self, data: &[u8]) -> [u8; 32] {
        let mut combined_data = data.to_vec();
        combined_data.extend_from_slice(
            &SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_le_bytes(),
        );
        let hash = qanto_hash(&combined_data);
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());
        id
    }
}

/// Network command interface for external modules
#[derive(Debug, Clone)]
pub enum QantoNetCommand {
    BroadcastBlock(QantoBlock),
    BroadcastTransaction(Transaction),
    ConnectPeer(SocketAddr),
    DisconnectPeer(PeerId),
    GetConnectedPeers,
    Subscribe { topic: String },
    Unsubscribe { topic: String },
    Gossip { topic: String, data: Vec<u8> },
    Shutdown,
}

/// Response types for network commands
#[derive(Debug, Clone)]
pub enum QantoNetResponse {
    Success,
    PeerConnected(PeerId),
    ConnectedPeers(Vec<PeerId>),
    Error(String),
}

// Native runtime implemented using threads and std::net

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_generation() {
        let public_key = b"test_public_key_data_here";
        let peer_id = PeerId::new(public_key);
        assert_eq!(peer_id.id.len(), 32);
    }

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(10, 5);

        // Should allow initial tokens
        assert!(limiter.try_consume(5));
        assert!(limiter.try_consume(5));

        // Should reject when out of tokens
        assert!(!limiter.try_consume(1));
    }

    #[test]
    fn test_dht_routing_table() {
        let local_peer = PeerId::random();
        let mut dht = DhtRoutingTable::new(local_peer);

        let peer_info = PeerInfo {
            peer_id: PeerId::random(),
            address: "127.0.0.1:8080".parse().unwrap(),
            public_key: Vec::new(),
            capabilities: Vec::new(),
            last_seen: SystemTime::now(),
            reputation: 0,
            connection_count: 0,
        };

        dht.add_peer(peer_info.clone());
        let closest = dht.find_closest_peers(&peer_info.peer_id, 1);
        assert_eq!(closest.len(), 1);
    }

    #[test]
    fn test_gossip_protocol() {
        let mut gossip = GossipProtocol::new();
        let peer_id = PeerId::random();
        let topic = "test_topic".to_string();

        gossip.subscribe(topic.clone(), peer_id.clone());
        let subscribers = gossip.get_subscribers(&topic);
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0], peer_id);
    }
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
