//! --- P2P Networking Layer ---
//! v0.1.0 - Initial Version
//!
//! This module implements the P2P networking layer for Qanto. It includes:
//! - Peer discovery and management
//! - Message routing and delivery
//! - Network security and encryption
//! - Mempool management
//! - DAG synchronization
//! - Peer-to-peer message exchange
//! - Network event handling

use crate::config::P2pConfig;
use crate::mempool::Mempool;
use crate::node::PeerCache;
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::saga::CarbonOffsetCredential;
use crate::transaction::Transaction;
use crate::types::{QuantumResistantSignature, UTXO};
use futures::stream::StreamExt;
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use qanto_core::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey};
// Removed HMAC import - using custom implementation with qanto_hash
use hex;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identity,
    kad::{store::MemoryStore, Behaviour as KadBehaviour, Event as KadEvent},
    mdns::tokio::Behaviour as MdnsTokioBehaviour,
    mdns::Event as MdnsEvent,
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    yamux, Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use my_blockchain::qanto_hash;
use nonzero_ext::nonzero;

use prometheus::{register_int_counter, IntCounter};
use prost::Message;
use qanto_rpc::server::generated as proto;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::env;
use std::error::Error as StdError;
use std::fs;
// (removed) use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::{
    sync::{mpsc, RwLock},
    time::interval,
};
use tracing::{error, info, instrument, warn};

const MAX_MESSAGE_SIZE: usize = 2_000_000;
const MIN_PEERS_FOR_MESH: usize = 1;
const DEFAULT_HMAC_SECRET: &str = "qanto_secret_key_for_p2p";

lazy_static::lazy_static! {
    static ref MESSAGES_SENT: IntCounter = register_int_counter!("p2p_messages_sent_total", "Total messages sent").unwrap();
    static ref MESSAGES_RECEIVED: IntCounter = register_int_counter!("p2p_messages_received_total", "Total messages received").unwrap();
    static ref PEERS_BLACKLISTED: IntCounter = register_int_counter!("p2p_peers_blacklisted_total", "Total peers blacklisted").unwrap();
}

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Invalid configuration: {0}")]
    Config(String),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Libp2p Core Transport error: {0}")]
    Libp2pTransport(#[from] libp2p::core::transport::TransportError<std::io::Error>),
    #[error("Noise protocol error: {0}")]
    Noise(#[from] libp2p::noise::Error),
    #[error("Multiaddr parsing error: {0}")]
    Multiaddr(#[from] libp2p::multiaddr::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("HMAC error")]
    Hmac,
    #[error("Invalid HMAC key length")]
    HmacKeyLength,
    #[error("Gossipsub configuration error: {0}")]
    GossipsubConfig(String),
    #[error("Gossipsub subscription error: {0}")]
    GossipsubSubscription(#[from] gossipsub::SubscriptionError),
    #[error("Timeout error: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),
    #[error("Broadcast error: {0}")]
    Broadcast(#[from] libp2p::gossipsub::PublishError),
    #[error("Quantum signature error: {0}")]
    QuantumSignature(String),
    #[error("Swarm build error: {0}")]
    SwarmBuild(String),
    #[error("Boxed STD error: {0}")]
    BoxedStd(#[from] Box<dyn StdError + Send + Sync>),
    #[error("Infallible error (should not happen): {0}")]
    Infallible(#[from] Infallible),
    #[error("mDNS error: {0}")]
    Mdns(String),
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NodeBehaviourEvent")]
pub struct NodeBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: MdnsTokioBehaviour,
    kademlia: KadBehaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum NodeBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Mdns(MdnsEvent),
    Kademlia(KadEvent),
}

impl From<gossipsub::Event> for NodeBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        NodeBehaviourEvent::Gossipsub(event)
    }
}
impl From<MdnsEvent> for NodeBehaviourEvent {
    fn from(event: MdnsEvent) -> Self {
        NodeBehaviourEvent::Mdns(event)
    }
}
impl From<KadEvent> for NodeBehaviourEvent {
    fn from(event: KadEvent) -> Self {
        NodeBehaviourEvent::Kademlia(event)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkMessage {
    data: NetworkMessageData,
    hmac: Vec<u8>,
    signature: QuantumResistantSignature,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkMessageData {
    Block(QantoBlock),
    Transaction(Transaction),
    TransactionBatch(Vec<Transaction>),
    State(HashMap<String, QantoBlock>, HashMap<String, UTXO>),
    StateRequest,
    CarbonOffsetCredential(CarbonOffsetCredential),
}

impl NetworkMessage {
    // Corrected: Skipped non-debuggable keys in the instrument macro.
    #[instrument(skip(signing_key))]
    #[allow(dead_code)]
    fn new(data: NetworkMessageData, signing_key: &QantoPQPrivateKey) -> Result<Self, P2PError> {
        let hmac_secret = Self::get_hmac_secret();
        let serialized_data = serde_json::to_vec(&data)?;
        let hmac = Self::compute_hmac(&serialized_data, &hmac_secret)?;

        let signature = QuantumResistantSignature::sign(signing_key, &serialized_data)
            .map_err(|e| P2PError::QuantumSignature(e.to_string()))?;

        Ok(Self {
            data,
            hmac,
            signature,
        })
    }

    fn get_hmac_secret() -> String {
        dotenvy::dotenv().ok();
        let secret = env::var("HMAC_SECRET").unwrap_or_else(|_| DEFAULT_HMAC_SECRET.to_string());
        if secret == DEFAULT_HMAC_SECRET {
            warn!("SECURITY: Using default HMAC secret. This is not secure for production. Please set the HMAC_SECRET environment variable.");
        }
        secret
    }

    fn compute_hmac(data: &[u8], secret: &str) -> Result<Vec<u8>, P2PError> {
        // Custom HMAC implementation using qanto_hash
        let secret_bytes = secret.as_bytes();
        let mut combined_data = Vec::new();
        combined_data.extend_from_slice(secret_bytes);
        combined_data.extend_from_slice(data);
        let hash = qanto_hash(&combined_data);
        Ok(hash.as_bytes().to_vec())
    }
}

fn convert_internal_tx_to_proto(tx: &crate::transaction::Transaction) -> proto::Transaction {
    let inputs = tx
        .inputs
        .iter()
        .map(|i| proto::Input {
            tx_id: i.tx_id.clone(),
            output_index: i.output_index,
        })
        .collect::<Vec<_>>();

    let outputs = tx
        .outputs
        .iter()
        .map(|o| proto::Output {
            address: o.address.clone(),
            amount: o.amount,
            homomorphic_encrypted: Some(proto::HomomorphicEncrypted {
                ciphertext: o.homomorphic_encrypted.ciphertext.clone(),
                public_key: o.homomorphic_encrypted.public_key.clone(),
            }),
        })
        .collect::<Vec<_>>();

    let signature = Some(proto::QuantumResistantSignature {
        signer_public_key: tx.signature.signer_public_key.clone(),
        signature: tx.signature.signature.clone(),
    });

    let fee_breakdown = tx.fee_breakdown.as_ref().map(|fb| proto::FeeBreakdown {
        base_fee: fb.base_fee,
        complexity_fee: fb.complexity_fee,
        storage_fee: fb.storage_fee,
        gas_fee: fb.gas_fee,
        priority_fee: fb.priority_fee,
        congestion_multiplier: fb.congestion_multiplier,
        total_fee: fb.total_fee,
        gas_used: fb.gas_used,
        gas_price: fb.gas_price,
    });

    proto::Transaction {
        id: tx.id.clone(),
        sender: tx.sender.clone(),
        receiver: tx.receiver.clone(),
        amount: tx.amount,
        fee: tx.fee,
        gas_limit: tx.gas_limit,
        gas_used: tx.gas_used,
        gas_price: tx.gas_price,
        priority_fee: tx.priority_fee,
        inputs,
        outputs,
        timestamp: tx.timestamp,
        metadata: tx.metadata.clone(),
        signature,
        fee_breakdown,
    }
}

fn convert_proto_tx(ptx: proto::Transaction) -> Result<crate::transaction::Transaction, String> {
    let inputs = ptx
        .inputs
        .into_iter()
        .map(|i| crate::transaction::Input {
            tx_id: i.tx_id,
            output_index: i.output_index,
        })
        .collect::<Vec<_>>();

    let outputs = ptx
        .outputs
        .into_iter()
        .map(|o| crate::transaction::Output {
            address: o.address,
            amount: o.amount,
            homomorphic_encrypted: match o.homomorphic_encrypted {
                Some(he) => crate::types::HomomorphicEncrypted {
                    ciphertext: he.ciphertext,
                    public_key: he.public_key,
                },
                None => crate::types::HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            },
        })
        .collect::<Vec<_>>();

    let signature = match ptx.signature {
        Some(sig) => crate::types::QuantumResistantSignature {
            signer_public_key: sig.signer_public_key,
            signature: sig.signature,
        },
        None => return Err("Missing transaction signature".to_string()),
    };

    let fee_breakdown = ptx
        .fee_breakdown
        .map(|fb| crate::gas_fee_model::FeeBreakdown {
            base_fee: fb.base_fee,
            complexity_fee: fb.complexity_fee,
            storage_fee: fb.storage_fee,
            gas_fee: fb.gas_fee,
            priority_fee: fb.priority_fee,
            congestion_multiplier: fb.congestion_multiplier,
            total_fee: fb.total_fee,
            gas_used: fb.gas_used,
            gas_price: fb.gas_price,
        });

    Ok(crate::transaction::Transaction {
        id: ptx.id,
        sender: ptx.sender,
        receiver: ptx.receiver,
        amount: ptx.amount,
        fee: ptx.fee,
        gas_limit: ptx.gas_limit,
        gas_used: ptx.gas_used,
        gas_price: ptx.gas_price,
        priority_fee: ptx.priority_fee,
        inputs,
        outputs,
        timestamp: ptx.timestamp,
        metadata: ptx.metadata,
        signature,
        fee_breakdown,
    })
}

fn convert_internal_utxo_to_proto(u: &crate::types::UTXO) -> proto::Utxo {
    proto::Utxo {
        address: u.address.clone(),
        amount: u.amount,
        tx_id: u.tx_id.clone(),
        output_index: u.output_index,
        explorer_link: u.explorer_link.clone(),
    }
}

fn convert_proto_utxo(u: proto::Utxo) -> crate::types::UTXO {
    crate::types::UTXO {
        address: u.address,
        amount: u.amount,
        tx_id: u.tx_id,
        output_index: u.output_index,
        explorer_link: u.explorer_link,
    }
}

#[derive(Debug)]
pub enum P2PCommand {
    BroadcastBlock(QantoBlock),
    BroadcastTransaction(Transaction),
    BroadcastTransactionBatch(Vec<Transaction>),
    RequestState,
    BroadcastState(HashMap<String, QantoBlock>, HashMap<String, UTXO>),
    BroadcastCarbonCredential(CarbonOffsetCredential),
    SyncResponse {
        blocks: Vec<QantoBlock>,
        utxos: HashMap<String, UTXO>,
    },
    RequestBlock {
        block_id: String,
        peer_id: PeerId,
    },
    SendBlockToOnePeer {
        peer_id: PeerId,
        block: Box<QantoBlock>,
    },
    GetConnectedPeers {
        response_sender: tokio::sync::oneshot::Sender<Vec<String>>,
    },
}

type KeyedPeerRateLimiter = RateLimiter<PeerId, DashMapStateStore<PeerId>, DefaultClock>;

// Corrected: P2PConfig now takes the Dilithium keys directly.
#[derive(Clone)]
pub struct P2PConfig<'a> {
    pub topic_prefix: &'a str,
    pub listen_addresses: Vec<String>,
    pub initial_peers: Vec<String>,
    pub dag: Arc<QantoDAG>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    pub proposals: Arc<RwLock<Vec<QantoBlock>>>,
    pub local_keypair: identity::Keypair,
    pub p2p_settings: P2pConfig,
    pub node_qr_sk: &'a QantoPQPrivateKey,
    pub node_qr_pk: &'a QantoPQPublicKey,
    pub peer_cache_path: String,
}

pub struct P2PServer {
    swarm: Swarm<NodeBehaviour>,
    topics: Vec<IdentTopic>,
    node_qr_sk: QantoPQPrivateKey,
    initial_peers_config: Vec<String>,
    peer_cache_path: String,
    p2p_command_sender: mpsc::Sender<P2PCommand>,
    dag: Arc<QantoDAG>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
}

#[derive(Clone)]
struct GossipRateLimiters {
    block: Arc<KeyedPeerRateLimiter>,
    tx: Arc<KeyedPeerRateLimiter>,
    state: Arc<KeyedPeerRateLimiter>,
    credential: Arc<KeyedPeerRateLimiter>,
}

impl P2PServer {
    pub async fn new(
        config: P2PConfig<'_>,
        p2p_command_sender: mpsc::Sender<P2PCommand>,
    ) -> Result<Self, P2PError> {
        let local_peer_id = PeerId::from_public_key(&config.local_keypair.public());
        info!("P2PServer using Local P2P Peer ID: {}", local_peer_id);

        let store = MemoryStore::new(local_peer_id);
        let mut kademlia_behaviour = KadBehaviour::new(local_peer_id, store);

        // Add initial peers to Kademlia routing table
        for peer_addr_str in &config.initial_peers {
            if let Ok(multiaddr) = peer_addr_str.parse::<Multiaddr>() {
                if let Some(peer_id) = multiaddr.iter().find_map(|p| {
                    if let libp2p::multiaddr::Protocol::P2p(id) = p {
                        Some(id)
                    } else {
                        None
                    }
                }) {
                    kademlia_behaviour.add_address(&peer_id, multiaddr.clone());
                    info!(
                        "Added initial peer to Kademlia: {} at {}",
                        peer_id, multiaddr
                    );
                } else {
                    warn!("Could not extract peer ID from address: {}", peer_addr_str);
                }
            } else {
                warn!(
                    "Invalid multiaddr format for initial peer: {}",
                    peer_addr_str
                );
            }
        }

        let gossipsub_behaviour =
            Self::build_gossipsub_behaviour(config.local_keypair.clone(), &config.p2p_settings)?;
        let mdns_behaviour =
            MdnsTokioBehaviour::new(Default::default(), local_peer_id).map_err(|e| {
                let mut error_msg = String::with_capacity(35 + e.to_string().len());
                error_msg.push_str("Failed to create mDNS behaviour: ");
                error_msg.push_str(&e.to_string());
                P2PError::Mdns(error_msg)
            })?;

        let behaviour = NodeBehaviour {
            gossipsub: gossipsub_behaviour,
            mdns: mdns_behaviour,
            kademlia: kademlia_behaviour,
        };

        let mut swarm = SwarmBuilder::with_existing_identity(config.local_keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_key| Ok(behaviour))
            .map_err(|e| {
                let e_str = format!("{e:?}");
                let mut error_msg = String::with_capacity(23 + e_str.len());
                error_msg.push_str("Behaviour setup error: ");
                error_msg.push_str(&e_str);
                P2PError::SwarmBuild(error_msg)
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        if !config.initial_peers.is_empty() {
            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                warn!("Failed to start Kademlia bootstrap: {e:?}");
            }
        }

        let topics =
            Self::subscribe_to_topics(config.topic_prefix, &mut swarm.behaviour_mut().gossipsub)?;

        Self::listen_on_addresses(&mut swarm, &config.listen_addresses, &local_peer_id)?;

        // Attempt to load peers from cache and connect to them at startup
        if let Ok(cache_json) = fs::read_to_string(&config.peer_cache_path) {
            match serde_json::from_str::<PeerCache>(&cache_json) {
                Ok(cache) => {
                    for addr_str in cache.peers.iter() {
                        if let Ok(multiaddr) = addr_str.parse::<Multiaddr>() {
                            // Add to Kademlia routing table if PeerId is present
                            if let Some(peer_id) = multiaddr.iter().find_map(|p| {
                                if let libp2p::multiaddr::Protocol::P2p(id) = p {
                                    Some(id)
                                } else {
                                    None
                                }
                            }) {
                                swarm
                                    .behaviour_mut()
                                    .kademlia
                                    .add_address(&peer_id, multiaddr.clone());
                                info!(
                                    "Loaded cached peer into Kademlia: {} at {}",
                                    peer_id, multiaddr
                                );
                            }
                            // Try dialing regardless, to establish connection
                            if let Err(e) = swarm.dial(multiaddr.clone()) {
                                warn!("Failed to dial cached peer {multiaddr}: {e}");
                            }
                        } else {
                            warn!("Invalid multiaddr in peer cache: {}", addr_str);
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to parse peer cache at {}: {}",
                        &config.peer_cache_path, e
                    );
                }
            }
        } else {
            info!("Peer cache file not found at {}", &config.peer_cache_path);
        }

        if !config.initial_peers.is_empty() {
            Self::dial_initial_peers(&mut swarm, &config.initial_peers).await;
        }

        Ok(Self {
            swarm,
            topics,
            node_qr_sk: config.node_qr_sk.clone(),
            initial_peers_config: config.initial_peers,
            peer_cache_path: config.peer_cache_path,
            p2p_command_sender,
            dag: config.dag.clone(),
            utxos: config.utxos.clone(),
        })
    }

    fn build_gossipsub_behaviour(
        local_key: identity::Keypair,
        p2p_config: &P2pConfig,
    ) -> Result<gossipsub::Behaviour, P2PError> {
        let message_id_fn = |message: &gossipsub::Message| {
            // Only use message data for deduplication, not source peer ID
            // This allows nodes to receive their own broadcast messages back
            let hash_result = qanto_hash(&message.data);
            gossipsub::MessageId::from(hex::encode(hash_result.as_bytes()))
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_millis(p2p_config.heartbeat_interval))
            .validation_mode(ValidationMode::Strict)
            .max_transmit_size(MAX_MESSAGE_SIZE)
            .mesh_n_low(p2p_config.mesh_n_low)
            .mesh_n(p2p_config.mesh_n)
            .mesh_n_high(p2p_config.mesh_n_high)
            .mesh_outbound_min(p2p_config.mesh_outbound_min)
            .message_id_fn(message_id_fn)
            .build()
            .map_err(|e_str| {
                let error_string = format!("{e_str}");
                let mut error_msg = String::with_capacity(33 + error_string.len());
                error_msg.push_str("Error building Gossipsub config: ");
                error_msg.push_str(&error_string);
                P2PError::GossipsubConfig(error_msg)
            })?;

        gossipsub::Behaviour::new(MessageAuthenticity::Signed(local_key), gossipsub_config).map_err(
            |e_str| {
                let mut error_msg = String::with_capacity(35 + e_str.len());
                error_msg.push_str("Error creating Gossipsub behaviour: ");
                error_msg.push_str(e_str);
                P2PError::GossipsubConfig(error_msg)
            },
        )
    }

    fn subscribe_to_topics(
        _topic_prefix: &str,
        gossipsub: &mut gossipsub::Behaviour,
    ) -> Result<Vec<IdentTopic>, P2PError> {
        // Use specific versioned Qanto topics for better protocol organization
        let topics_str = [
            "/qanto/blocks/1",             // Versioned block propagation topic
            "/qanto/utxo/1",               // Versioned UTXO state updates topic
            "/qanto/transactions/1",       // Versioned transaction propagation topic
            "/qanto/carbon_credentials/1", // Versioned carbon offset credentials topic
            "/qanto/state_sync/1",         // Versioned state synchronization topic
        ];

        let mut topics = Vec::new();
        for topic_s in topics_str.iter() {
            let topic = IdentTopic::new(*topic_s);
            gossipsub.subscribe(&topic)?;
            topics.push(topic);
            info!("Subscribed to gossipsub topic: {}", topic_s);
        }
        Ok(topics)
    }

    fn listen_on_addresses(
        swarm: &mut Swarm<NodeBehaviour>,
        addresses: &[String],
        local_peer_id: &PeerId,
    ) -> Result<(), P2PError> {
        for addr_str in addresses {
            let multiaddr: Multiaddr = addr_str.parse()?;
            swarm.listen_on(multiaddr)?;
        }
        info!("P2P Server initialized with Local Peer ID: {local_peer_id}");
        Ok(())
    }

    async fn dial_initial_peers(swarm: &mut Swarm<NodeBehaviour>, peers_addrs: &[String]) {
        for peer_addr_str in peers_addrs {
            if let Ok(multiaddr) = peer_addr_str.parse::<Multiaddr>() {
                if let Err(e) = swarm.dial(multiaddr.clone()) {
                    warn!("Failed to dial peer {multiaddr}: {e}");
                }
            }
        }
    }

    pub async fn run(&mut self, mut rx: mpsc::Receiver<P2PCommand>) -> Result<(), P2PError> {
        let mut mesh_check_ticker = interval(Duration::from_secs(60));
        let mut peer_cache_ticker = interval(Duration::from_secs(300));
        let blacklist = Arc::new(RwLock::new(HashSet::new()));

        let rate_limiters = GossipRateLimiters {
            block: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(10u32)))),
            tx: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(50u32)))),
            state: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(5u32)))),
            credential: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(20u32)))),
        };

        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(NodeBehaviourEvent::Gossipsub(gossipsub::Event::Message { propagation_source, message, .. })) = event {
                        tokio::spawn({
                            let blacklist = blacklist.clone();
                            let p2p_sender = self.p2p_command_sender.clone();
                            let rate_limiters = rate_limiters.clone();
                            async move {
                                Self::static_process_gossip_message(
                                    message,
                                    propagation_source,
                                    blacklist,
                                    p2p_sender,
                                    rate_limiters,
                                )
                                .await;
                            }
                        });
                    } else {
                        self.handle_swarm_event(event).await;
                    }
                }
                Some(command) = rx.recv() => {
                    if let Err(e) = self.process_internal_command(command).await {
                        error!("Failed to process internal P2P command: {e}");
                    }
                }
                _ = mesh_check_ticker.tick() => { self.check_mesh_peers().await; }
                _ = peer_cache_ticker.tick() => {
                    if let Err(e) = self.save_peers_to_cache().await {
                        warn!("Failed to save peer cache: {e}");
                    }
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<NodeBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(
                    "P2P Server listening on: {}/p2p/{}",
                    address,
                    self.swarm.local_peer_id()
                );
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connection established with peer: {peer_id}");
                // Log current mesh status after connection
                for topic in &self.topics {
                    let mesh_peers: Vec<_> = self
                        .swarm
                        .behaviour()
                        .gossipsub
                        .mesh_peers(&topic.hash())
                        .collect();
                    info!(
                        "After connection to {}, topic {} has {} mesh peers",
                        peer_id,
                        topic,
                        mesh_peers.len()
                    );
                }
            }
            SwarmEvent::Behaviour(NodeBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source,
                message_id: _,
                message,
            })) => {
                // Route gossip message through the static handler with fresh rate limiters
                let blacklist = Arc::new(RwLock::new(HashSet::new()));
                let rate_limiters = GossipRateLimiters {
                    block: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(10u32)))),
                    tx: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(50u32)))),
                    state: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(5u32)))),
                    credential: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(20u32)))),
                };

                Self::static_process_gossip_message(
                    message,
                    propagation_source,
                    blacklist,
                    self.p2p_command_sender.clone(),
                    rate_limiters,
                )
                .await;
            }
            SwarmEvent::Behaviour(NodeBehaviourEvent::Mdns(MdnsEvent::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    info!("mDNS discovered peer: {} at {}", peer_id, multiaddr);
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, multiaddr);
                }
            }
            SwarmEvent::Behaviour(NodeBehaviourEvent::Kademlia(
                KadEvent::OutboundQueryProgressed { result, .. },
            )) => match result {
                libp2p::kad::QueryResult::Bootstrap(Ok(_)) => {
                    info!("Kademlia bootstrap completed successfully");
                }
                libp2p::kad::QueryResult::Bootstrap(Err(e)) => {
                    warn!("Kademlia bootstrap failed: {:?}", e);
                }
                _ => {}
            },
            _ => {}
        }
    }

    async fn save_peers_to_cache(&mut self) -> Result<(), P2PError> {
        let mut cache_peers = HashSet::new();
        for kbucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            for entry in kbucket.iter() {
                for addr in entry.node.value.iter() {
                    cache_peers.insert(addr.to_string());
                }
            }
        }

        if !cache_peers.is_empty() {
            let cache = PeerCache {
                peers: cache_peers.into_iter().collect(),
            };
            let cache_json = serde_json::to_string_pretty(&cache)?;
            fs::write(&self.peer_cache_path, cache_json)?;
        }
        Ok(())
    }

    async fn process_internal_command(&mut self, command: P2PCommand) -> Result<(), P2PError> {
        match command {
            P2PCommand::BroadcastBlock(block) => {
                let mut log_msg = String::with_capacity(6 + block.id.len());
                log_msg.push_str("block ");
                log_msg.push_str(&block.id);
                self.broadcast_message(NetworkMessageData::Block(block.clone()), 0, &log_msg)
                    .await
            }
            P2PCommand::BroadcastTransaction(tx) => {
                let mut log_msg = String::with_capacity(12 + tx.id.len());
                log_msg.push_str("transaction ");
                log_msg.push_str(&tx.id);
                self.broadcast_message(NetworkMessageData::Transaction(tx.clone()), 2, &log_msg)
                    .await
            }
            P2PCommand::BroadcastTransactionBatch(txs) => {
                let log_msg = format!("transaction_batch {} txs", txs.len());
                self.broadcast_message(
                    NetworkMessageData::TransactionBatch(txs.clone()),
                    2,
                    &log_msg,
                )
                .await
            }
            P2PCommand::RequestState => {
                // Collect current blocks from DAG as HashMap
                let blocks: HashMap<String, QantoBlock> = self
                    .dag
                    .blocks
                    .iter()
                    .map(|entry| (entry.key().clone(), entry.value().clone()))
                    .collect();

                // Collect current UTXOs
                let utxos = {
                    let utxos_guard = self.utxos.read().await;
                    utxos_guard.clone()
                };

                // Broadcast the current state
                self.broadcast_message(NetworkMessageData::State(blocks, utxos), 4, "state data")
                    .await
            }
            P2PCommand::BroadcastState(blocks, utxos) => {
                self.broadcast_message(NetworkMessageData::State(blocks, utxos), 4, "state data")
                    .await
            }
            P2PCommand::BroadcastCarbonCredential(cred) => {
                let mut log_msg = String::with_capacity(18 + cred.id.len());
                log_msg.push_str("carbon credential ");
                log_msg.push_str(&cred.id);
                self.broadcast_message(
                    NetworkMessageData::CarbonOffsetCredential(cred.clone()),
                    3,
                    &log_msg,
                )
                .await
            }
            P2PCommand::GetConnectedPeers { response_sender } => {
                let connected_peers = self.get_connected_peers();
                let _ = response_sender.send(connected_peers);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn static_process_gossip_message(
        message: gossipsub::Message,
        source: PeerId,
        blacklist: Arc<RwLock<HashSet<PeerId>>>,
        p2p_command_sender: mpsc::Sender<P2PCommand>,
        rate_limiters: GossipRateLimiters,
    ) {
        info!(
            "Received gossipsub message from {} on topic {}",
            source,
            message.topic.as_str()
        );

        if blacklist.read().await.contains(&source) {
            warn!("Ignoring message from blacklisted peer: {}", source);
            return;
        }

        let topic_str = message.topic.as_str();
        let rate_limiter_to_use = if topic_str.contains("blocks") {
            &rate_limiters.block
        } else if topic_str.contains("transactions") {
            &rate_limiters.tx
        } else if topic_str.contains("utxo") || topic_str.contains("state_sync") {
            &rate_limiters.state
        } else if topic_str.contains("carbon_credentials") {
            &rate_limiters.credential
        } else {
            warn!("Received message on unknown topic: {}", topic_str);
            return;
        };

        if rate_limiter_to_use.check_key(&source).is_err() {
            let mut blacklist_writer = blacklist.write().await;
            if blacklist_writer.insert(source) {
                warn!("Peer {} exceeded rate limit. Blacklisting.", source);
                PEERS_BLACKLISTED.inc();
            }
            return;
        }

        // Decode protobuf envelope
        let envelope = match proto::P2pNetworkMessage::decode(message.data.as_slice()) {
            Ok(env) => env,
            Err(e) => {
                error!("Failed to decode protobuf envelope from {}: {}", source, e);
                return;
            }
        };

        // Verify HMAC over payload_bytes
        let hmac_secret = NetworkMessage::get_hmac_secret();
        let expected_hmac =
            match NetworkMessage::compute_hmac(&envelope.payload_bytes, &hmac_secret) {
                Ok(h) => h,
                Err(e) => {
                    error!("HMAC computation error: {}", e);
                    return;
                }
            };
        if envelope.hmac != expected_hmac {
            warn!(
                "Invalid HMAC from peer {} on topic {}",
                source,
                message.topic.as_str()
            );
            let mut blacklist_writer = blacklist.write().await;
            if blacklist_writer.insert(source) {
                warn!("Peer {} failed HMAC check. Blacklisting.", source);
                PEERS_BLACKLISTED.inc();
            }
            return;
        }

        // Verify Dilithium signature over payload_bytes
        let sig = match &envelope.signature {
            Some(s) => s,
            None => {
                warn!(
                    "Missing signature from peer {} on topic {}",
                    source,
                    message.topic.as_str()
                );
                let mut blacklist_writer = blacklist.write().await;
                if blacklist_writer.insert(source) {
                    warn!("Peer {} failed signature check. Blacklisting.", source);
                    PEERS_BLACKLISTED.inc();
                }
                return;
            }
        };
        let internal_sig = QuantumResistantSignature {
            signer_public_key: sig.signer_public_key.clone(),
            signature: sig.signature.clone(),
        };
        if !internal_sig.verify(&envelope.payload_bytes) {
            warn!(
                "Invalid signature from peer {} on topic {}",
                source,
                message.topic.as_str()
            );
            let mut blacklist_writer = blacklist.write().await;
            if blacklist_writer.insert(source) {
                warn!("Peer {} failed signature check. Blacklisting.", source);
                PEERS_BLACKLISTED.inc();
            }
            return;
        }

        // Metrics: count valid received messages
        MESSAGES_RECEIVED.inc();

        // Route by payload type
        let payload_type = match proto::P2pPayloadType::try_from(envelope.payload_type) {
            Ok(pt) => pt,
            Err(_) => {
                warn!(
                    "Unknown payload type {} from {}",
                    envelope.payload_type, source
                );
                return;
            }
        };

        let cmd = match payload_type {
            proto::P2pPayloadType::Transaction => {
                match proto::Transaction::decode(envelope.payload_bytes.as_slice()) {
                    Ok(ptx) => match convert_proto_tx(ptx) {
                        Ok(tx) => {
                            info!("Processing transaction message: {}", tx.id);
                            P2PCommand::BroadcastTransaction(tx)
                        }
                        Err(e) => {
                            error!("Failed to convert protobuf Transaction: {}", e);
                            return;
                        }
                    },
                    Err(e) => {
                        error!("Failed to decode protobuf Transaction: {}", e);
                        return;
                    }
                }
            }
            proto::P2pPayloadType::Block => {
                match proto::QantoBlock::decode(envelope.payload_bytes.as_slice()) {
                    Ok(pb) => match convert_proto_block(pb) {
                        Ok(block) => {
                            info!("Processing block message: {}", block.id);
                            P2PCommand::BroadcastBlock(block)
                        }
                        Err(e) => {
                            error!("Failed to convert protobuf Block: {}", e);
                            return;
                        }
                    },
                    Err(e) => {
                        error!("Failed to decode protobuf Block: {}", e);
                        return;
                    }
                }
            }
            proto::P2pPayloadType::Credential => {
                match proto::CarbonOffsetCredential::decode(envelope.payload_bytes.as_slice()) {
                    Ok(pc) => {
                        let cred = convert_proto_credential(pc);
                        info!("Processing carbon credential message: {}", cred.id);
                        P2PCommand::BroadcastCarbonCredential(cred)
                    }
                    Err(e) => {
                        error!("Failed to decode protobuf Credential: {}", e);
                        return;
                    }
                }
            }
            proto::P2pPayloadType::State => {
                match proto::StateSnapshot::decode(envelope.payload_bytes.as_slice()) {
                    Ok(state) => {
                        let blocks_res: Result<Vec<QantoBlock>, String> =
                            state.blocks.into_iter().map(convert_proto_block).collect();
                        let blocks = match blocks_res {
                            Ok(b) => b,
                            Err(e) => {
                                error!("Failed to convert StateSnapshot blocks: {}", e);
                                return;
                            }
                        };
                        let mut utxos_map: HashMap<String, UTXO> = HashMap::new();
                        for pu in state.utxos.into_iter() {
                            let u = convert_proto_utxo(pu);
                            let key = format!("{}_{}", u.tx_id, u.output_index);
                            utxos_map.insert(key, u);
                        }
                        info!(
                            "Processing state snapshot: {} blocks, {} utxos",
                            blocks.len(),
                            utxos_map.len()
                        );
                        P2PCommand::SyncResponse {
                            blocks,
                            utxos: utxos_map,
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode StateSnapshot: {}", e);
                        return;
                    }
                }
            }
            proto::P2pPayloadType::TransactionBatch => {
                match proto::TransactionBatch::decode(envelope.payload_bytes.as_slice()) {
                    Ok(batch) => {
                        let txs_res: Result<Vec<Transaction>, String> = batch
                            .transactions
                            .into_iter()
                            .map(convert_proto_tx)
                            .collect();
                        match txs_res {
                            Ok(txs) => {
                                info!("Processing transaction batch: {} transactions", txs.len());
                                P2PCommand::BroadcastTransactionBatch(txs)
                            }
                            Err(e) => {
                                error!("Failed to convert TransactionBatch: {}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode TransactionBatch: {}", e);
                        return;
                    }
                }
            }
        };

        match p2p_command_sender.send(cmd).await {
            Ok(_) => info!("Successfully forwarded message to command processor"),
            Err(e) => error!("Failed to forward message to command processor: {}", e),
        }
    }

    async fn check_mesh_peers(&mut self) {
        for topic_instance in &self.topics {
            let mesh_peers: Vec<_> = self
                .swarm
                .behaviour()
                .gossipsub
                .mesh_peers(&topic_instance.hash())
                .collect();
            info!(
                "Topic {} has {} mesh peers (min required: {})",
                topic_instance,
                mesh_peers.len(),
                MIN_PEERS_FOR_MESH
            );
            if mesh_peers.len() < MIN_PEERS_FOR_MESH {
                warn!(
                    "Insufficient mesh peers for topic {}, attempting reconnection",
                    topic_instance
                );
                self.reconnect_to_initial_peers().await;
                break;
            }
        }
    }

    async fn reconnect_to_initial_peers(&mut self) {
        info!("Attempting to reconnect to initial peers from configuration.");
        Self::dial_initial_peers(&mut self.swarm, &self.initial_peers_config).await;
    }

    async fn broadcast_message(
        &mut self,
        data: NetworkMessageData,
        topic_index: usize,
        log_info: &str,
    ) -> Result<(), P2PError> {
        let topic = &self.topics[topic_index];

        // Check mesh peers before broadcasting
        let mesh_peers: Vec<_> = self
            .swarm
            .behaviour()
            .gossipsub
            .mesh_peers(&topic.hash())
            .collect();
        info!(
            "Broadcasting {} to topic {} with {} mesh peers",
            log_info,
            topic,
            mesh_peers.len()
        );

        if mesh_peers.is_empty() {
            warn!(
                "No mesh peers available for topic {} when broadcasting {}",
                topic, log_info
            );
        }

        // Build protobuf envelope
        let (payload_type, payload_bytes) = match data {
            NetworkMessageData::Transaction(ref tx) => {
                let ptx = convert_internal_tx_to_proto(tx);
                (
                    proto::P2pPayloadType::Transaction as i32,
                    ptx.encode_to_vec(),
                )
            }
            NetworkMessageData::Block(ref b) => {
                // Minimal block conversion using rpc_backend's logic mirrored here
                let transactions = b
                    .transactions
                    .iter()
                    .map(convert_internal_tx_to_proto)
                    .collect::<Vec<_>>();
                let cross_chain_references = b
                    .cross_chain_references
                    .iter()
                    .map(|(cid, bid)| proto::CrossChainReference {
                        chain_id: *cid,
                        block_id: bid.clone(),
                    })
                    .collect::<Vec<_>>();
                let cross_chain_swaps = vec![];
                let signature = Some(proto::QuantumResistantSignature {
                    signer_public_key: b.signature.signer_public_key.clone(),
                    signature: b.signature.signature.clone(),
                });
                let homomorphic_encrypted = b
                    .homomorphic_encrypted
                    .iter()
                    .map(|he| proto::HomomorphicEncrypted {
                        ciphertext: he.ciphertext.clone(),
                        public_key: he.public_key.clone(),
                    })
                    .collect::<Vec<_>>();
                let smart_contracts = b
                    .smart_contracts
                    .iter()
                    .map(|sc| proto::SmartContract {
                        contract_id: sc.contract_id.clone(),
                        code: sc.code.clone(),
                        storage: sc.storage.clone(),
                        owner: sc.owner.clone(),
                        gas_balance: sc.gas_balance,
                    })
                    .collect::<Vec<_>>();
                let carbon_credentials = b
                    .carbon_credentials
                    .iter()
                    .map(|cc| proto::CarbonOffsetCredential {
                        id: cc.id.clone(),
                        issuer_id: cc.issuer_id.clone(),
                        beneficiary_node: cc.beneficiary_node.clone(),
                        tonnes_co2_sequestered: cc.tonnes_co2_sequestered,
                        project_id: cc.project_id.clone(),
                        vintage_year: cc.vintage_year,
                        verification_signature: cc.verification_signature.clone(),
                        additionality_proof_hash: cc.additionality_proof_hash.clone(),
                        issuer_reputation_score: cc.issuer_reputation_score,
                        geospatial_consistency_score: cc.geospatial_consistency_score,
                    })
                    .collect::<Vec<_>>();
                let pb = proto::QantoBlock {
                    chain_id: b.chain_id,
                    id: b.id.clone(),
                    parents: b.parents.clone(),
                    transactions,
                    difficulty: b.difficulty,
                    validator: b.validator.clone(),
                    miner: b.miner.clone(),
                    nonce: b.nonce,
                    timestamp: b.timestamp,
                    height: b.height,
                    reward: b.reward,
                    effort: b.effort,
                    cross_chain_references,
                    cross_chain_swaps,
                    merkle_root: b.merkle_root.clone(),
                    signature,
                    homomorphic_encrypted,
                    smart_contracts,
                    carbon_credentials,
                    epoch: b.epoch,
                };
                (proto::P2pPayloadType::Block as i32, pb.encode_to_vec())
            }
            NetworkMessageData::CarbonOffsetCredential(ref cc) => {
                let pc = proto::CarbonOffsetCredential {
                    id: cc.id.clone(),
                    issuer_id: cc.issuer_id.clone(),
                    beneficiary_node: cc.beneficiary_node.clone(),
                    tonnes_co2_sequestered: cc.tonnes_co2_sequestered,
                    project_id: cc.project_id.clone(),
                    vintage_year: cc.vintage_year,
                    verification_signature: cc.verification_signature.clone(),
                    additionality_proof_hash: cc.additionality_proof_hash.clone(),
                    issuer_reputation_score: cc.issuer_reputation_score,
                    geospatial_consistency_score: cc.geospatial_consistency_score,
                };
                (proto::P2pPayloadType::Credential as i32, pc.encode_to_vec())
            }
            NetworkMessageData::TransactionBatch(ref txs) => {
                let batch = proto::TransactionBatch {
                    transactions: txs.iter().map(convert_internal_tx_to_proto).collect(),
                };
                (
                    proto::P2pPayloadType::TransactionBatch as i32,
                    batch.encode_to_vec(),
                )
            }
            NetworkMessageData::State(ref blocks_map, ref utxos_map) => {
                let blocks = blocks_map
                    .values()
                    .map(|b| {
                        let transactions = b
                            .transactions
                            .iter()
                            .map(convert_internal_tx_to_proto)
                            .collect::<Vec<_>>();
                        let cross_chain_references = b
                            .cross_chain_references
                            .iter()
                            .map(|(cid, bid)| proto::CrossChainReference {
                                chain_id: *cid,
                                block_id: bid.clone(),
                            })
                            .collect::<Vec<_>>();
                        let cross_chain_swaps = vec![];
                        let signature = Some(proto::QuantumResistantSignature {
                            signer_public_key: b.signature.signer_public_key.clone(),
                            signature: b.signature.signature.clone(),
                        });
                        let homomorphic_encrypted = b
                            .homomorphic_encrypted
                            .iter()
                            .map(|he| proto::HomomorphicEncrypted {
                                ciphertext: he.ciphertext.clone(),
                                public_key: he.public_key.clone(),
                            })
                            .collect::<Vec<_>>();
                        let smart_contracts = b
                            .smart_contracts
                            .iter()
                            .map(|sc| proto::SmartContract {
                                contract_id: sc.contract_id.clone(),
                                code: sc.code.clone(),
                                storage: sc.storage.clone(),
                                owner: sc.owner.clone(),
                                gas_balance: sc.gas_balance,
                            })
                            .collect::<Vec<_>>();
                        let carbon_credentials = b
                            .carbon_credentials
                            .iter()
                            .map(|cc| proto::CarbonOffsetCredential {
                                id: cc.id.clone(),
                                issuer_id: cc.issuer_id.clone(),
                                beneficiary_node: cc.beneficiary_node.clone(),
                                tonnes_co2_sequestered: cc.tonnes_co2_sequestered,
                                project_id: cc.project_id.clone(),
                                vintage_year: cc.vintage_year,
                                verification_signature: cc.verification_signature.clone(),
                                additionality_proof_hash: cc.additionality_proof_hash.clone(),
                                issuer_reputation_score: cc.issuer_reputation_score,
                                geospatial_consistency_score: cc.geospatial_consistency_score,
                            })
                            .collect::<Vec<_>>();
                        proto::QantoBlock {
                            chain_id: b.chain_id,
                            id: b.id.clone(),
                            parents: b.parents.clone(),
                            transactions,
                            difficulty: b.difficulty,
                            validator: b.validator.clone(),
                            miner: b.miner.clone(),
                            nonce: b.nonce,
                            timestamp: b.timestamp,
                            height: b.height,
                            reward: b.reward,
                            effort: b.effort,
                            cross_chain_references,
                            cross_chain_swaps,
                            merkle_root: b.merkle_root.clone(),
                            signature,
                            homomorphic_encrypted,
                            smart_contracts,
                            carbon_credentials,
                            epoch: b.epoch,
                        }
                    })
                    .collect::<Vec<_>>();
                let utxos = utxos_map
                    .values()
                    .map(convert_internal_utxo_to_proto)
                    .collect::<Vec<_>>();
                let snap = proto::StateSnapshot { blocks, utxos };
                (proto::P2pPayloadType::State as i32, snap.encode_to_vec())
            }
            NetworkMessageData::StateRequest => {
                // Not yet supported in protobuf path
                warn!("Skipping unsupported protobuf payload for {}", log_info);
                return Ok(());
            }
        };

        let hmac_secret = NetworkMessage::get_hmac_secret();
        let hmac = NetworkMessage::compute_hmac(&payload_bytes, &hmac_secret)?;
        let signature = QuantumResistantSignature::sign(&self.node_qr_sk, &payload_bytes)
            .map_err(|e| P2PError::QuantumSignature(e.to_string()))?;

        let envelope = proto::P2pNetworkMessage {
            payload_type,
            payload_bytes,
            hmac,
            signature: Some(proto::QuantumResistantSignature {
                signer_public_key: signature.signer_public_key,
                signature: signature.signature,
            }),
        };
        let msg_bytes = envelope.encode_to_vec();

        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), msg_bytes)?;
        MESSAGES_SENT.inc();
        let mut log_msg = String::with_capacity(log_info.len() + 50);
        log_msg.push_str("Broadcasted ");
        log_msg.push_str(log_info);
        log_msg.push_str(": ");
        info!("{}", log_msg);
        Ok(())
    }

    /// Get the list of currently connected peer IDs
    pub fn get_connected_peers(&self) -> Vec<String> {
        self.swarm
            .connected_peers()
            .map(|peer_id| peer_id.to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::gossipsub::IdentTopic;
    use libp2p::{gossipsub, identity, PeerId};
    use prost::Message;
    use qanto_rpc::server::generated as proto;
    use serial_test::serial;
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn gen_peer_id() -> PeerId {
        let kp = identity::Keypair::generate_ed25519();
        PeerId::from(kp.public())
    }

    fn make_rate_limiters_tx_1() -> GossipRateLimiters {
        GossipRateLimiters {
            block: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(10u32)))),
            tx: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(1u32)))),
            state: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(5u32)))),
            credential: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(10u32)))),
        }
    }

    fn make_default_rate_limiters() -> GossipRateLimiters {
        GossipRateLimiters {
            block: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(10u32)))),
            tx: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(50u32)))),
            state: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(5u32)))),
            credential: Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(20u32)))),
        }
    }

    // Helper to construct a gossipsub::Message with our payload
    fn make_message(topic: &str, data: Vec<u8>) -> gossipsub::Message {
        // Note: Depending on libp2p version, Message fields are public and include
        // source, data, sequence_number, topic.
        gossipsub::Message {
            source: None,
            data,
            sequence_number: None,
            topic: IdentTopic::new(topic).into(),
        }
    }

    #[serial]
    #[tokio::test]
    async fn valid_transaction_message_is_forwarded() {
        let orig_secret = std::env::var("HMAC_SECRET").ok();
        std::env::set_var("HMAC_SECRET", "test_secret");
        let (_pk, sk) = crate::post_quantum_crypto::generate_pq_keypair(None).expect("pq keypair");
        let tx = crate::transaction::Transaction::new_dummy();

        // Build protobuf payload and envelope
        let ptx = convert_internal_tx_to_proto(&tx);
        let payload_bytes = ptx.encode_to_vec();
        let hmac_secret = NetworkMessage::get_hmac_secret();
        let hmac =
            NetworkMessage::compute_hmac(&payload_bytes, &hmac_secret).expect("compute hmac");
        let signature =
            QuantumResistantSignature::sign(&sk, &payload_bytes).expect("sign network payload");
        let msg = proto::P2pNetworkMessage {
            payload_type: proto::P2pPayloadType::Transaction as i32,
            payload_bytes,
            hmac: hmac.clone(),
            signature: Some(proto::QuantumResistantSignature {
                signer_public_key: signature.signer_public_key.clone(),
                signature: signature.signature.clone(),
            }),
        };

        // Sanity-check: locally verify HMAC and signature match what the handler expects
        let expected_hmac =
            NetworkMessage::compute_hmac(&msg.payload_bytes, &hmac_secret).expect("expected hmac");
        assert_eq!(
            msg.hmac, expected_hmac,
            "constructed HMAC should match expected"
        );
        let internal_sig = QuantumResistantSignature {
            signer_public_key: signature.signer_public_key.clone(),
            signature: signature.signature.clone(),
        };
        assert!(
            internal_sig.verify(&msg.payload_bytes),
            "constructed signature should verify"
        );

        let bytes = msg.encode_to_vec();
        let message = make_message("/qanto/transactions/1", bytes);
        let source = gen_peer_id();
        let blacklist = Arc::new(RwLock::new(HashSet::new()));
        let rate_limiters = make_default_rate_limiters();
        let (cmd_tx, mut cmd_rx) = mpsc::channel(1);

        P2PServer::static_process_gossip_message(
            message,
            source,
            blacklist.clone(),
            cmd_tx,
            rate_limiters,
        )
        .await;

        let cmd = cmd_rx.recv().await.expect("expected forwarded command");
        match cmd {
            P2PCommand::BroadcastTransaction(t) => assert_eq!(t.id, tx.id),
            _ => panic!("unexpected command variant"),
        }
        assert!(!blacklist.read().await.contains(&source));
        if let Some(s) = orig_secret {
            std::env::set_var("HMAC_SECRET", s);
        } else {
            std::env::remove_var("HMAC_SECRET");
        }
    }

    #[serial]
    #[tokio::test]
    async fn invalid_hmac_blacklists_peer() {
        let orig_secret = std::env::var("HMAC_SECRET").ok();
        std::env::set_var("HMAC_SECRET", "valid_secret");
        let (_pk, sk) = crate::post_quantum_crypto::generate_pq_keypair(None).expect("pq keypair");
        let tx = crate::transaction::Transaction::new_dummy();

        let ptx = convert_internal_tx_to_proto(&tx);
        let payload_bytes = ptx.encode_to_vec();
        let hmac = NetworkMessage::compute_hmac(&payload_bytes, &NetworkMessage::get_hmac_secret())
            .expect("hmac");
        let signature = QuantumResistantSignature::sign(&sk, &payload_bytes).expect("sign");
        let msg = proto::P2pNetworkMessage {
            payload_type: proto::P2pPayloadType::Transaction as i32,
            payload_bytes,
            hmac,
            signature: Some(proto::QuantumResistantSignature {
                signer_public_key: signature.signer_public_key,
                signature: signature.signature,
            }),
        };
        let bytes = msg.encode_to_vec();

        let message = make_message("/qanto/transactions/1", bytes);
        let source = gen_peer_id();
        let blacklist = Arc::new(RwLock::new(HashSet::new()));
        let rate_limiters = make_default_rate_limiters();
        let (cmd_tx, mut cmd_rx) = mpsc::channel(1);

        // Change secret to break HMAC verification path
        std::env::set_var("HMAC_SECRET", "invalid_secret");

        P2PServer::static_process_gossip_message(
            message,
            source,
            blacklist.clone(),
            cmd_tx,
            rate_limiters,
        )
        .await;

        assert!(blacklist.read().await.contains(&source));
        assert!(cmd_rx.try_recv().is_err());
        if let Some(s) = orig_secret {
            std::env::set_var("HMAC_SECRET", s);
        } else {
            std::env::remove_var("HMAC_SECRET");
        }
    }

    #[serial]
    #[tokio::test]
    async fn tampered_signature_blacklists_peer() {
        let orig_secret = std::env::var("HMAC_SECRET").ok();
        std::env::set_var("HMAC_SECRET", "test_secret");
        let (_pk, sk) = crate::post_quantum_crypto::generate_pq_keypair(None).expect("pq keypair");
        let mut tx = crate::transaction::Transaction::new_dummy();
        let ptx_valid = convert_internal_tx_to_proto(&tx);
        let payload_bytes_valid = ptx_valid.encode_to_vec();
        let hmac_valid =
            NetworkMessage::compute_hmac(&payload_bytes_valid, &NetworkMessage::get_hmac_secret())
                .expect("hmac");
        let signature_valid =
            QuantumResistantSignature::sign(&sk, &payload_bytes_valid).expect("sign");

        // Tamper the payload to invalidate signature
        tx.amount = tx.amount.saturating_add(1);
        let ptx_tampered = convert_internal_tx_to_proto(&tx);
        let payload_bytes_tampered = ptx_tampered.encode_to_vec();
        let msg_tampered = proto::P2pNetworkMessage {
            payload_type: proto::P2pPayloadType::Transaction as i32,
            payload_bytes: payload_bytes_tampered,
            hmac: hmac_valid, // keep HMAC consistent with secret
            signature: Some(proto::QuantumResistantSignature {
                signer_public_key: signature_valid.signer_public_key,
                signature: signature_valid.signature,
            }),
        };

        let bytes = msg_tampered.encode_to_vec();
        let message = make_message("/qanto/transactions/1", bytes);
        let source = gen_peer_id();
        let blacklist = Arc::new(RwLock::new(HashSet::new()));
        let rate_limiters = make_default_rate_limiters();
        let (cmd_tx, mut cmd_rx) = mpsc::channel(1);

        P2PServer::static_process_gossip_message(
            message,
            source,
            blacklist.clone(),
            cmd_tx,
            rate_limiters,
        )
        .await;

        assert!(blacklist.read().await.contains(&source));
        assert!(cmd_rx.try_recv().is_err());
        if let Some(s) = orig_secret {
            std::env::set_var("HMAC_SECRET", s);
        } else {
            std::env::remove_var("HMAC_SECRET");
        }
    }

    #[serial]
    #[tokio::test]
    async fn rate_limit_exceeded_blacklists_peer() {
        let orig_secret = std::env::var("HMAC_SECRET").ok();
        std::env::set_var("HMAC_SECRET", "test_secret");
        let (_pk, sk) = crate::post_quantum_crypto::generate_pq_keypair(None).expect("pq keypair");
        let tx = crate::transaction::Transaction::new_dummy();
        let ptx = convert_internal_tx_to_proto(&tx);
        let payload_bytes = ptx.encode_to_vec();
        let hmac = NetworkMessage::compute_hmac(&payload_bytes, &NetworkMessage::get_hmac_secret())
            .expect("hmac");
        let signature = QuantumResistantSignature::sign(&sk, &payload_bytes).expect("sign");
        let msg = proto::P2pNetworkMessage {
            payload_type: proto::P2pPayloadType::Transaction as i32,
            payload_bytes,
            hmac,
            signature: Some(proto::QuantumResistantSignature {
                signer_public_key: signature.signer_public_key,
                signature: signature.signature,
            }),
        };
        let bytes = msg.encode_to_vec();

        // Sanity-check: verify constructed message authentication before sending
        let expected_hmac =
            NetworkMessage::compute_hmac(&msg.payload_bytes, &NetworkMessage::get_hmac_secret())
                .expect("expected hmac");
        assert_eq!(msg.hmac, expected_hmac, "HMAC matches expected");
        let internal_sig = QuantumResistantSignature {
            signer_public_key: msg.signature.as_ref().unwrap().signer_public_key.clone(),
            signature: msg.signature.as_ref().unwrap().signature.clone(),
        };
        assert!(
            internal_sig.verify(&msg.payload_bytes),
            "signature verifies"
        );

        let source = gen_peer_id();
        let blacklist = Arc::new(RwLock::new(HashSet::new()));
        let rate_limiters = make_rate_limiters_tx_1();
        let (cmd_tx, mut cmd_rx) = mpsc::channel(2);

        let m1 = make_message("/qanto/transactions/1", bytes.clone());
        P2PServer::static_process_gossip_message(
            m1,
            source,
            blacklist.clone(),
            cmd_tx.clone(),
            rate_limiters.clone(),
        )
        .await;
        let cmd1 = tokio::time::timeout(Duration::from_millis(200), cmd_rx.recv())
            .await
            .expect("timeout waiting for first command")
            .expect("expected first command");
        match cmd1 {
            P2PCommand::BroadcastTransaction(_) => {}
            _ => panic!("unexpected"),
        }

        // Small delay to ensure the rate limiter clock registers the first event
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let m2 = make_message("/qanto/transactions/1", bytes.clone());
        P2PServer::static_process_gossip_message(
            m2,
            source,
            blacklist.clone(),
            cmd_tx.clone(),
            rate_limiters.clone(),
        )
        .await;

        assert!(blacklist.read().await.contains(&source));
        if let Some(s) = orig_secret {
            std::env::set_var("HMAC_SECRET", s);
        } else {
            std::env::remove_var("HMAC_SECRET");
        }
    }
}

fn convert_proto_block(pb: proto::QantoBlock) -> Result<crate::qantodag::QantoBlock, String> {
    use crate::qantodag::{CrossChainSwap, SmartContract, SwapState};
    use crate::types::{HomomorphicEncrypted, QuantumResistantSignature};

    // Transactions
    let transactions = pb
        .transactions
        .into_iter()
        .map(convert_proto_tx)
        .collect::<Result<Vec<_>, _>>()?;

    // Cross-chain references
    let cross_chain_references = pb
        .cross_chain_references
        .into_iter()
        .map(|r| (r.chain_id, r.block_id))
        .collect::<Vec<_>>();

    // Cross-chain swaps
    let cross_chain_swaps = pb
        .cross_chain_swaps
        .into_iter()
        .map(|s| {
            let state = match s.state {
                x if x == proto::SwapState::Initiated as i32 => SwapState::Initiated,
                x if x == proto::SwapState::Redeemed as i32 => SwapState::Redeemed,
                x if x == proto::SwapState::Refunded as i32 => SwapState::Refunded,
                _ => return Err(format!("Unknown SwapState value: {}", s.state)),
            };
            Ok(CrossChainSwap {
                swap_id: s.swap_id,
                source_chain: s.source_chain,
                target_chain: s.target_chain,
                amount: s.amount,
                initiator: s.initiator,
                responder: s.responder,
                timelock: s.timelock,
                state,
                secret_hash: s.secret_hash,
                secret: s.secret,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Signature
    let signature = match pb.signature {
        Some(sig) => QuantumResistantSignature {
            signer_public_key: sig.signer_public_key,
            signature: sig.signature,
        },
        None => return Err("Missing block signature".to_string()),
    };

    // Homomorphic encrypted data
    let homomorphic_encrypted = pb
        .homomorphic_encrypted
        .into_iter()
        .map(|h| HomomorphicEncrypted {
            ciphertext: h.ciphertext,
            public_key: h.public_key,
        })
        .collect::<Vec<_>>();

    // Smart contracts
    let smart_contracts = pb
        .smart_contracts
        .into_iter()
        .map(|sc| SmartContract {
            contract_id: sc.contract_id,
            code: sc.code,
            storage: sc.storage,
            owner: sc.owner,
            gas_balance: sc.gas_balance,
        })
        .collect::<Vec<_>>();

    // Carbon credentials
    let carbon_credentials = pb
        .carbon_credentials
        .into_iter()
        .map(convert_proto_credential)
        .collect::<Vec<_>>();

    Ok(crate::qantodag::QantoBlock {
        chain_id: pb.chain_id,
        id: pb.id,
        parents: pb.parents,
        transactions,
        difficulty: pb.difficulty,
        validator: pb.validator,
        miner: pb.miner,
        nonce: pb.nonce,
        timestamp: pb.timestamp,
        height: pb.height,
        reward: pb.reward,
        effort: pb.effort,
        cross_chain_references,
        cross_chain_swaps,
        merkle_root: pb.merkle_root,
        signature,
        homomorphic_encrypted,
        smart_contracts,
        carbon_credentials,
        epoch: pb.epoch,
    })
}

fn convert_proto_credential(
    pc: proto::CarbonOffsetCredential,
) -> crate::saga::CarbonOffsetCredential {
    crate::saga::CarbonOffsetCredential {
        id: pc.id,
        issuer_id: pc.issuer_id,
        beneficiary_node: pc.beneficiary_node,
        tonnes_co2_sequestered: pc.tonnes_co2_sequestered,
        project_id: pc.project_id,
        vintage_year: pc.vintage_year,
        verification_signature: pc.verification_signature,
        additionality_proof_hash: pc.additionality_proof_hash,
        issuer_reputation_score: pc.issuer_reputation_score,
        geospatial_consistency_score: pc.geospatial_consistency_score,
    }
}
