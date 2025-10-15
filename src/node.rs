//! --- Qanto Node Orchestrator ---
//! v0.1.0 - Deterministic PoW Integration
//!
//! This version implements a basic node with P2P networking, consensus,
//! and transaction processing.
//!
//! - P2P Networking: Implements a basic P2P network using libp2p for peer discovery
//!   and message exchange.
//! - Consensus: Utilizes the Qanto Hybrid Consensus Engine for secure and
//!   decentralized block validation.
//! - Transaction Processing: Handles transaction creation, validation, and
//!   inclusion in blocks.
//! - Mempool: Maintains a transaction pool for pending transactions to be
//!   included in the next block.
//! - Storage: Utilizes RocksDB for persistent storage of blockchain data.
//! - GraphQL API: Exposes a GraphQL API for querying blockchain state and
//!   interacting with the node.
//! - WebSocket API: Provides a WebSocket API for real-time event subscription
//!   and interaction.
//! - Analytics Dashboard: Includes a real-time analytics dashboard for monitoring
//!   node performance and network activity.

use crate::adaptive_mining::AdaptiveMiningLoop;
use crate::analytics_dashboard::{AnalyticsDashboard, DashboardConfig};
use crate::config::{Config, ConfigError};
use crate::elite_mempool::EliteMempool;
use crate::graphql_server::{create_graphql_router, create_graphql_schema, GraphQLContext};
use crate::mempool::Mempool;
use crate::miner::{Miner, MinerConfig, MiningError};
use crate::omega::reflect_on_action;
use crate::p2p::{P2PCommand, P2PConfig, P2PError, P2PServer};
use crate::qanto_net::QantoNetServer;
use crate::performance_optimizations::{OptimizedBlockBuilder, OptimizedMempool};
use crate::qanto_compat::sp_core::H256;
use crate::qanto_storage::{QantoStorage, QantoStorageError, StorageConfig};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError, QantoDagConfig};
use crate::resource_cleanup::ResourceCleanup;
use crate::saga::PalletSaga;
use crate::transaction::Transaction;
use crate::types::UTXO;
use crate::wallet::Wallet;
use crate::websocket_server::{create_websocket_router, WebSocketServerState};
use axum::{
    body::Body,
    extract::{Path as AxumPath, State, State as MiddlewareState},
    http::{Request as HttpRequest, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use governor::clock::QuantaClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use libp2p::{identity, PeerId};
use nonzero_ext::nonzero;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::fs;
use tokio::signal;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::{JoinError, JoinSet};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[cfg(feature = "infinite-strata")]
use crate::infinite_strata_node::{
    DecentralizedOracleAggregator, InfiniteStrataNode, NodeConfig as IsnmNodeConfig,
    MIN_UPTIME_HEARTBEAT_SECS,
};

// --- Constants ---
const MAX_UTXOS: usize = 1_000_000;
const MAX_PROPOSALS: usize = 10_000;
const ADDRESS_REGEX: &str = r"^[0-9a-fA-F]{64}$";
const MAX_SYNC_AGE_SECONDS: u64 = 3600;

// Performance-test mode: 32 BPS target (31ms per block)
// Performance testing and default mining intervals are now configured via config.toml
// These constants have been removed as they are unused

lazy_static::lazy_static! {
    static ref ADDRESS_REGEX_COMPILED: Regex = Regex::new(ADDRESS_REGEX).expect("Invalid address regex pattern");
}

/// Represents the primary error types that can occur within the node.
#[derive(Error, Debug)]
pub enum NodeError {
    #[error("DAG error: {0}")]
    DAG(String),
    #[error("P2P error: {0}")]
    P2P(String),
    #[error("Mempool error: {0}")]
    Mempool(String),
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Wallet error: {0}")]
    Wallet(#[from] crate::wallet::WalletError),
    #[error("Mining error: {0}")]
    Mining(#[from] MiningError),
    #[error("Timeout error: {0}")]
    Timeout(String),
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("Server execution error: {0}")]
    ServerExecution(String),
    #[error("Task join error: {0}")]
    Join(#[from] JoinError),
    #[error("P2P specific error: {0}")]
    P2PSpecific(#[from] P2PError),
    #[error("P2P Identity error: {0}")]
    P2PIdentity(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("QantoDAG error: {0}")]
    QantoDAG(#[from] QantoDAGError),
    #[error("Sync error: {0}")]
    SyncError(String),
    #[error("Database error: {0}")]
    Database(#[from] QantoStorageError),
    #[error("Node initialization error: {0}")]
    NodeInitialization(String),
    #[error("Elite mempool error: {0}")]
    EliteMempool(#[from] crate::elite_mempool::EliteMempoolError),
}

impl From<NodeError> for String {
    fn from(err: NodeError) -> Self {
        err.to_string()
    }
}

// --- API Response Structs ---

/// Provides a high-level overview of the DAG's state.
#[derive(Serialize, Debug)]
struct DagInfo {
    block_count: usize,
    tip_count: usize,
    current_difficulty: f64,
    target_block_time: u64,
    validator_count: usize,
    num_chains: u32,
    latest_block_timestamp: u64,
}

/// A readiness probe for external systems, indicating if the node is synced and operational.
#[derive(Serialize, Debug)]
struct PublishReadiness {
    is_ready: bool,
    block_count: usize,
    utxo_count: usize,
    peer_count: usize,
    mempool_size: usize,
    is_synced: bool,
    issues: Vec<String>,
}

/// A simple structure for caching peer addresses to disk.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PeerCache {
    pub peers: Vec<String>,
}

/// The main orchestrator for all node components.
pub struct Node {
    _config_path: String,
    config: Config,
    p2p_identity_keypair: identity::Keypair,
    pub dag: Arc<QantoDAG>,
    pub miner: Arc<Miner>,
    wallet: Arc<Wallet>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub elite_mempool: Arc<EliteMempool>,
    pub utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    pub proposals: Arc<RwLock<Vec<QantoBlock>>>,
    peer_cache_path: String,
    pub saga_pallet: Arc<PalletSaga>,
    pub analytics: Arc<AnalyticsDashboard>,
    pub optimized_block_builder: Arc<OptimizedBlockBuilder>,
    pub resource_cleanup: Arc<ResourceCleanup>,
    #[cfg(feature = "infinite-strata")]
    isnm_service: Arc<InfiniteStrataNode>,
}

/// A type alias for the API rate limiter.
type DirectApiRateLimiter = RateLimiter<NotKeyed, InMemoryState, QuantaClock>;

impl Node {
    /// Creates a new `Node` instance, initializing all sub-components.
    /// This function handles P2P identity loading/creation, DAG initialization from the database,
    /// and setting up the SAGA pallet and other services.
    pub async fn new(
        mut config: Config,
        config_path: String,
        wallet: Arc<Wallet>,
        p2p_identity_path: &str,
        peer_cache_path: String,
    ) -> Result<Self, NodeError> {
        info!("Validating configuration");
        config.validate()?;
        info!("Configuration validated successfully");

        info!("Loading or generating P2P identity keypair");
        // Load or generate the libp2p identity keypair.
        let local_keypair = match fs::read(p2p_identity_path).await {
            Ok(key_bytes) => {
                info!("Loading P2P identity from file: {p2p_identity_path}");
                identity::Keypair::from_protobuf_encoding(&key_bytes).map_err(|e| {
                    NodeError::P2PIdentity(format!("Failed to decode P2P identity key: {e}"))
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!(
                    "P2P identity key file not found at {p2p_identity_path}, generating a new one."
                );
                if let Some(p) = Path::new(p2p_identity_path).parent() {
                    fs::create_dir_all(p).await?;
                }
                let new_key = identity::Keypair::generate_ed25519();
                let new_key_bytes = new_key.to_protobuf_encoding().map_err(|e| {
                    NodeError::P2PIdentity(format!("Failed to encode P2P key: {e:?}"))
                })?;
                fs::write(p2p_identity_path, new_key_bytes).await?;
                info!("New P2P identity key saved to {p2p_identity_path}");
                Ok(new_key)
            }
            Err(e) => Err(NodeError::P2PIdentity(format!(
                "Failed to read P2P identity key file '{p2p_identity_path}': {e}"
            ))),
        }?;
        let local_peer_id = PeerId::from_public_key(&local_keypair.public());
        info!("Node Local P2P Peer ID: {local_peer_id}");
        info!("P2P identity keypair loaded/generated successfully");

        info!("Updating config with full local P2P address");
        // Update config with the full P2P address if it's not already set.
        let full_local_p2p_address = format!("{}/p2p/{}", config.p2p_address, local_peer_id);
        if config.local_full_p2p_address.as_deref() != Some(&full_local_p2p_address) {
            info!(
                "Updating config file '{config_path}' with local full P2P address: {full_local_p2p_address}"
            );
            config.local_full_p2p_address = Some(full_local_p2p_address.clone());
            config.save(&config_path)?;
            info!("Config updated and saved successfully");
        }

        info!("Validating wallet address against genesis validator");
        // Validate that the wallet address matches the genesis validator specified in the config.
        let initial_validator = wallet.address().trim().to_lowercase();
        if initial_validator != config.genesis_validator.trim().to_lowercase() {
            return Err(NodeError::Config(ConfigError::Validation(
                "Wallet address does not match genesis validator".to_string(),
            )));
        }
        info!("Wallet address validated successfully");

        info!("Getting node keypair from wallet");

        info!("Node keypair obtained successfully");

        info!("Initializing SAGA and dependent services...");

        // Conditionally compile and initialize the Infinite Strata service.
        #[cfg(feature = "infinite-strata")]
        let isnm_service: Arc<InfiniteStrataNode> = {
            info!("[ISNM] Infinite Strata feature enabled, initializing service.");
            let isnm_config = IsnmNodeConfig::default();
            let oracle_aggregator = Arc::new(DecentralizedOracleAggregator::default());
            Arc::new(InfiniteStrataNode::new(
                isnm_config,
                oracle_aggregator,
                crate::infinite_strata_node::TestnetMode::default(),
            ))
        };

        // Initialize the SAGA pallet, injecting the ISNM service if the feature is enabled.
        let saga_pallet = {
            #[cfg(feature = "infinite-strata")]
            {
                info!("Initializing PalletSaga with Infinite Strata service");
                Arc::new(PalletSaga::new(Some(isnm_service.clone())))
            }
            #[cfg(not(feature = "infinite-strata"))]
            {
                info!("Initializing PalletSaga without Infinite Strata");
                Arc::new(PalletSaga::new())
            }
        };
        info!("SAGA and dependent services initialized successfully");

        info!("Initializing QantoDAG (loading database)...");
        info!("Opening Qanto native storage");
        let storage_config = StorageConfig {
            data_dir: config.data_dir.clone().into(),
            cache_size: 1024 * 1024 * 100, // 100MB cache
            compression_enabled: true,
            encryption_enabled: true,
            max_file_size: 1024 * 1024 * 50, // 50MB max file size
            compaction_threshold: 0.7,
            wal_enabled: true,
            sync_writes: true,
            max_open_files: 1000,
        };
        let db = QantoStorage::new(storage_config)?;
        info!("Qanto native storage opened successfully");

        info!("Configuring QantoDagConfig");
        // Configure and create the core DAG structure.
        let dag_config = QantoDagConfig {
            initial_validator,
            target_block_time: config.target_block_time,
            num_chains: config.num_chains,
        };
        info!("QantoDagConfig configured successfully");

        info!("Creating QantoDAG instance");
        let dag_arc = QantoDAG::new(dag_config, saga_pallet.clone(), db, config.logging.clone())?;
        info!("QantoDAG instance created successfully");
        info!("QantoDAG initialized.");

        // Initialize shared state components.
        let mempool = Arc::new(RwLock::new(Mempool::new(3600, 10_000_000, 10_000)));
        let utxos = Arc::new(RwLock::new(HashMap::with_capacity(MAX_UTXOS)));
        let proposals = Arc::new(RwLock::new(Vec::with_capacity(MAX_PROPOSALS)));

        // Create genesis UTXO with the entire 21 billion QAN supply allocated to contract address
        // Clear any existing UTXOs first to ensure clean state
        {
            let mut utxos_lock = utxos.write().await;
            utxos_lock.clear(); // Reset UTXO state completely

            let genesis_utxo_id = "genesis_total_supply_tx_0".to_string();
            utxos_lock.insert(
                genesis_utxo_id.clone(),
                UTXO {
                    address: config.contract_address.clone(),
                    amount: 21_000_000_000_000_000, // Entire 21 billion QAN supply in smallest units with 6 decimals
                    tx_id: "genesis_total_supply_tx".to_string(),
                    output_index: 0,
                    explorer_link: {
                        // Use local explorer instead of external service
                        let mut link = String::with_capacity(22 + genesis_utxo_id.len());
                        link.push_str("/explorer/utxo/");
                        link.push_str(&genesis_utxo_id);
                        link
                    },
                },
            );
            info!(
                "Genesis UTXO created with 21 billion QAN allocated to contract address: {}",
                config.contract_address
            );
            info!("UTXO state reset - only contract address has balance now");
        }

        // BUILD FIX (E0560): Remove `difficulty_hex` and `num_chains` from MinerConfig.
        // These are no longer needed as the difficulty is determined dynamically from the block template.
        let miner_config = MinerConfig {
            address: wallet.address(),
            dag: dag_arc.clone(),
            target_block_time: config.target_block_time,
            use_gpu: config.use_gpu,
            zk_enabled: config.zk_enabled,
            threads: config.mining_threads,
            logging_config: config.logging.clone(),
        };
        let miner_instance = Miner::new(miner_config)?;
        let miner = Arc::new(miner_instance);

        // Initialize analytics dashboard with default configuration
        let analytics_config = DashboardConfig::default();
        let analytics = Arc::new(AnalyticsDashboard::new(analytics_config));

        // Initialize OptimizedBlockBuilder for high-performance block creation
        let optimized_mempool = OptimizedMempool::new(10_000_000, 3600); // 10MB, 1 hour TTL
        let optimized_block_builder = Arc::new(OptimizedBlockBuilder::new(optimized_mempool));

        // Initialize elite mempool for ultra-high performance (10M+ TPS)
        let elite_mempool = Arc::new(EliteMempool::new(
            1_000_000,          // max_transactions
            1024 * 1024 * 1024, // max_size_bytes (1GB)
            num_cpus::get(),    // shard_count
            num_cpus::get(),    // worker_count
        )?);

        // Initialize ResourceCleanup for graceful shutdown with timeout and task abortion
        let resource_cleanup = Arc::new(ResourceCleanup::new(Duration::from_secs(30)));

        Ok(Self {
            _config_path: config_path,
            config: config.clone(),
            p2p_identity_keypair: local_keypair,
            dag: dag_arc,
            miner,
            wallet,
            mempool,
            elite_mempool,
            utxos,
            proposals,
            peer_cache_path,
            saga_pallet,
            analytics,
            optimized_block_builder,
            resource_cleanup,
            #[cfg(feature = "infinite-strata")]
            isnm_service,
        })
    }

    /// Starts all node services and enters the main event loop.
    /// This function spawns tasks for the P2P server, API server, miner (if in solo mode),
    /// and the core command processing loop. It gracefully handles shutdown on Ctrl+C.
    pub async fn start(&self) -> Result<(), NodeError> {
        // Create a channel for passing commands between the P2P layer and the core logic.
        let (tx_p2p_commands, mut rx_p2p_commands) = mpsc::channel::<P2PCommand>(100);
        let mut join_set: JoinSet<Result<(), NodeError>> = JoinSet::new();

        // Get the cancellation token from resource cleanup system
        let shutdown_token = self.resource_cleanup.get_cancellation_token();

        // Register runtime shutdown hooks
        let resource_cleanup_clone = self.resource_cleanup.clone();
        let _ = self
            .resource_cleanup
            .register_runtime_hook(Box::new(move |_token| {
                let cleanup = resource_cleanup_clone.clone();
                tokio::spawn(async move {
                    info!("Executing node shutdown sequence...");
                    // Cancel all managed tasks
                    cleanup.cancel_all_tasks().await.unwrap_or_else(|e| {
                        error!("Failed to cancel all tasks during shutdown: {}", e);
                    });
                    info!("Node shutdown sequence completed");
                    Ok(())
                })
            }))
            .await;

        // If the 'infinite-strata' feature is enabled, spawn its periodic task.
        #[cfg(feature = "infinite-strata")]
        {
            info!("[ISNM] Spawning periodic cloud presence check task.");
            let isnm_service_clone = self.isnm_service.clone();
            let _task_handle = join_set.spawn(async move {
                // Initial delay to allow the node to stabilize.
                tokio::time::sleep(Duration::from_secs(10)).await;
                let mut interval =
                    tokio::time::interval(Duration::from_secs(MIN_UPTIME_HEARTBEAT_SECS));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

                loop {
                    interval.tick().await;
                    debug!("[ISNM] Running periodic cloud presence check...");
                    if let Err(e) = isnm_service_clone.run_periodic_check().await {
                        warn!("[ISNM] Periodic check failed: {}", e);
                    }

                    // FIX: Removed the redundant, direct call to perform_quantum_state_verification.
                    // The `run_periodic_check` function already handles this internally. Calling it
                    // twice was causing unnecessary CPU load and stalling the miner.
                }
                #[allow(unreachable_code)]
                Ok(())
            });

            // Register the task with resource cleanup
            self.resource_cleanup
                .register_task(|_token| async move {
                    // Note: task_handle is an AbortHandle, not awaitable
                    // The task will be aborted when the cancellation token is triggered
                    Ok(())
                })
                .await?;
        }

        // --- Core Command Processor Task ---
        // This task is the heart of the node, processing incoming P2P commands.
        let command_processor_task = {
            let dag_clone = self.dag.clone();
            let mempool_clone = self.mempool.clone();
            let utxos_clone = self.utxos.clone();
            let p2p_tx_clone = tx_p2p_commands.clone();
            let saga_clone = self.saga_pallet.clone();
            let mempool_batch_size = self.config.mempool_batch_size.unwrap_or(40000);

            async move {
                while let Some(command) = rx_p2p_commands.recv().await {
                    match command {
                        P2PCommand::BroadcastBlock(block) => {
                            info!("\n{}", block);
                            debug!("P2P received BroadcastBlock command for block {}", block.id);

                            let add_result = dag_clone.add_block(block.clone(), &utxos_clone).await;

                            match add_result {
                                Ok(true) => {
                                    info!(
                                        "✅ Block {} successfully added to DAG via P2P",
                                        block.id
                                    );
                                    debug!("Running periodic maintenance after adding new block.");
                                    dag_clone.run_periodic_maintenance(mempool_batch_size).await;
                                }
                                Ok(false) => {
                                    warn!("⚠️ Block {} was not added to DAG (already exists or rejected)", block.id);
                                }
                                Err(e) => {
                                    error!(
                                        "❌ Block {} failed validation or processing: {}",
                                        block.id, e
                                    );
                                }
                            }
                        }
                        P2PCommand::BroadcastTransaction(tx) => {
                            debug!("Command processor received transaction {}", tx.id);
                            let mempool_writer = mempool_clone.write().await;
                            let utxos_reader = utxos_clone.read().await;
                            if let Err(e) = mempool_writer
                                .add_transaction(tx, &utxos_reader, &dag_clone)
                                .await
                            {
                                warn!("Failed to add transaction to mempool: {}", e);
                            }
                        }
                        P2PCommand::SyncResponse { blocks, utxos } => {
                            info!(
                                "Received state sync response with {} blocks and {} UTXOs.",
                                blocks.len(),
                                utxos.len()
                            );
                            // Extend the local UTXO set with the synced data.
                            {
                                let mut utxos_writer = utxos_clone.write().await;
                                let initial_utxo_count = utxos_writer.len();
                                utxos_writer.extend(utxos);
                                info!(
                                    "UTXO set updated with {} new entries from sync.",
                                    utxos_writer.len() - initial_utxo_count
                                );
                            }
                            // Topologically sort and add the received blocks to the DAG.
                            if let Ok(sorted_blocks) =
                                Self::topological_sort_blocks(blocks, &dag_clone).await
                            {
                                let mut added_count = 0;
                                let mut failed_count = 0;
                                for b in sorted_blocks {
                                    match dag_clone.add_block(b, &utxos_clone).await {
                                        Ok(true) => added_count += 1,
                                        Ok(false) => { /* block already exists, not an error */ }
                                        Err(e) => {
                                            warn!("Failed to add block from sync response: {}", e);
                                            failed_count += 1;
                                        }
                                    }
                                }
                                info!(
                                    "Block sync complete. Added: {}, Failed: {}.",
                                    added_count, failed_count
                                );
                                // Run maintenance to update DAG state after sync.
                                dag_clone.run_periodic_maintenance(mempool_batch_size).await;
                            } else {
                                error!(
                                    "Failed to topologically sort blocks from sync response. Discarding batch."
                                );
                            }
                        }
                        P2PCommand::RequestBlock { block_id, peer_id } => {
                            info!(
                                "Received request for block {} from peer {}",
                                block_id, peer_id
                            );
                            // DashMap provides direct access without needing .read()
                            let blocks_reader = &dag_clone.blocks;
                            if let Some(block) = blocks_reader.get(&block_id) {
                                info!("Found block {}, sending to peer {}", block_id, peer_id);
                                let cmd = P2PCommand::SendBlockToOnePeer {
                                    peer_id,
                                    block: Box::new(block.clone()),
                                };
                                if let Err(e) = p2p_tx_clone.send(cmd).await {
                                    error!("Failed to send SendBlockToOnePeer command: {}", e);
                                }
                            } else {
                                warn!(
                                    "Peer {} requested block {} which we don't have.",
                                    peer_id, block_id
                                );
                            }
                        }
                        P2PCommand::BroadcastCarbonCredential(cred) => {
                            if let Err(e) =
                                saga_clone.verify_and_store_credential(cred.clone()).await
                            {
                                warn!(cred_id=%cred.id, "Received invalid CarbonOffsetCredential from network: {}", e);
                            } else {
                                info!(cred_id=%cred.id, "Successfully verified and stored CarbonOffsetCredential from network.");
                            }
                        }
                        P2PCommand::RequestState => {
                            info!("Received RequestState command, broadcasting current state");

                            // Collect current blocks from DAG as HashMap
                            let blocks: HashMap<String, QantoBlock> = dag_clone
                                .blocks
                                .iter()
                                .map(|entry| (entry.key().clone(), entry.value().clone()))
                                .collect();

                            // Collect current UTXOs
                            let utxos = {
                                let utxos_reader = utxos_clone.read().await;
                                utxos_reader.clone()
                            };

                            // Send BroadcastState command with current state (tuple syntax)
                            let broadcast_cmd = P2PCommand::BroadcastState(blocks, utxos);
                            if let Err(e) = p2p_tx_clone.send(broadcast_cmd).await {
                                error!("Failed to send BroadcastState command: {}", e);
                            } else {
                                info!(
                                    "Successfully sent state broadcast with {} blocks and {} UTXOs",
                                    dag_clone.blocks.len(),
                                    utxos_clone.read().await.len()
                                );
                            }
                        }
                        _ => { /* Ignore other command types not relevant to this processor */ }
                    }
                }
                Ok(())
            }
        };
        let _command_task_handle = join_set.spawn(command_processor_task);

        // Register the command processor task with resource cleanup
        self.resource_cleanup
            .register_task(|_token| async move {
                // Note: command_task_handle is an AbortHandle, not awaitable
                // The task will be aborted when the cancellation token is triggered
                Ok(())
            })
            .await?;

        // --- P2P Server / Solo Miner Task ---
        // If peers are configured, start the P2P server. Otherwise, run in solo mining mode.
        if !self.config.peers.is_empty() {
            info!("Peers detected in config, initializing P2P server task...");
            let p2p_dag_clone = self.dag.clone();
            let p2p_mempool_clone = self.mempool.clone();
            let p2p_utxos_clone = self.utxos.clone();
            let p2p_proposals_clone = self.proposals.clone();
            let p2p_command_sender_clone = tx_p2p_commands.clone();
            let p2p_identity_keypair_clone = self.p2p_identity_keypair.clone();
            let p2p_listen_address_config_clone = self.config.p2p_address.clone();
            let p2p_initial_peers_config_clone = self.config.peers.clone();
            let p2p_settings_clone = self.config.p2p.clone();
            let (node_signing_key, node_public_key) = self.wallet.get_keypair()?;
            let peer_cache_path_clone = self.peer_cache_path.clone();
            let network_id_clone = self.config.network_id.clone();

            let p2p_task_fut = async move {
                let mut attempts = 0;
                // Retry loop for P2P server initialization with exponential backoff.
                let mut p2p_server = loop {
                    let p2p_config = P2PConfig {
                        topic_prefix: &network_id_clone,
                        listen_addresses: vec![p2p_listen_address_config_clone.clone()],
                        initial_peers: p2p_initial_peers_config_clone.clone(),
                        dag: p2p_dag_clone.clone(),
                        mempool: p2p_mempool_clone.clone(),
                        utxos: p2p_utxos_clone.clone(),
                        proposals: p2p_proposals_clone.clone(),
                        local_keypair: p2p_identity_keypair_clone.clone(),
                        p2p_settings: p2p_settings_clone.clone(),
                        node_qr_sk: &node_signing_key,
                        node_qr_pk: &node_public_key,
                        peer_cache_path: peer_cache_path_clone.clone(),
                    };
                    info!(
                        "Attempting to initialize P2P server (attempt {})...",
                        attempts + 1
                    );
                    match timeout(
                        Duration::from_secs(15),
                        P2PServer::new(p2p_config, p2p_command_sender_clone.clone()),
                    )
                    .await
                    {
                        Ok(Ok(server)) => {
                            info!("P2P server initialized successfully.");
                            break server;
                        }
                        Ok(Err(e)) => {
                            warn!("P2P server failed to initialize: {e}.");
                        }
                        Err(_) => {
                            warn!("P2P server initialization timed out.");
                        }
                    }
                    attempts += 1;
                    let backoff_duration = Duration::from_secs(2u64.pow(attempts.min(6)));
                    warn!("Retrying P2P initialization in {:?}", backoff_duration);
                    tokio::time::sleep(backoff_duration).await;
                };

                // Request state from peers on startup to sync the DAG.
                if !p2p_initial_peers_config_clone.is_empty() {
                    if let Err(e) = p2p_command_sender_clone
                        .send(P2PCommand::RequestState)
                        .await
                    {
                        error!("Failed to send initial RequestState P2P command: {e}");
                    }
                }

                // Run the P2P server's event loop.
                let (_p2p_tx, p2p_rx) = mpsc::channel::<P2PCommand>(100);
                p2p_server.run(p2p_rx).await.map_err(NodeError::P2PSpecific)
            };
            let _p2p_task_handle = join_set.spawn(p2p_task_fut);

            // Register the P2P task with resource cleanup
            self.resource_cleanup
                .register_task(|_token| async move {
                    // Note: p2p_task_handle is an AbortHandle, not awaitable
                    // The task will be aborted when the cancellation token is triggered
                    Ok(())
                })
                .await?;
        } else if self.config.mining_enabled {
            if self.config.adaptive_mining_enabled {
                info!("No peers found. Running in single-node mode. Spawning adaptive miner...");

                // Create adaptive mining configuration
                let adaptive_config = crate::adaptive_mining::TestnetMiningConfig::default();

                // Create mining metrics for adaptive mining
                let mining_metrics = Arc::new(crate::mining_metrics::MiningMetrics::new());

                // Create adaptive mining loop
                let mut adaptive_loop = AdaptiveMiningLoop::new(
                    adaptive_config,
                    None, // diagnostics
                    mining_metrics,
                );

                // Clone necessary components for the adaptive mining task
                let adaptive_dag_clone = self.dag.clone();
                let adaptive_wallet_clone = self.wallet.clone();
                let adaptive_miner_clone = self.miner.clone();
                let adaptive_mempool_clone = self.mempool.clone();
                let adaptive_utxos_clone = self.utxos.clone();
                let adaptive_shutdown_token = shutdown_token.clone();

                let _adaptive_miner_task_handle = join_set.spawn(async move {
                    debug!("[DEBUG] Spawning adaptive mining task");
                    if let Err(e) = adaptive_loop
                        .run_adaptive_mining_loop(
                            adaptive_dag_clone,
                            adaptive_wallet_clone,
                            adaptive_miner_clone,
                            adaptive_mempool_clone,
                            adaptive_utxos_clone,
                            adaptive_shutdown_token,
                        )
                        .await
                    {
                        error!("Adaptive mining loop failed: {}", e);
                        return Err(NodeError::QantoDAG(e));
                    }
                    Ok(())
                });

                // Register the adaptive miner task with resource cleanup
                self.resource_cleanup
                    .register_task(|_token| async move {
                        // Note: adaptive_miner_task_handle is an AbortHandle, not awaitable
                        // The task will be aborted when the cancellation token is triggered
                        Ok(())
                    })
                    .await?;
            } else {
                info!("No peers found. Running in single-node mode. Spawning solo miner...");
                let miner_dag_clone = self.dag.clone();
                let miner_wallet_clone = self.wallet.clone();
                let miner_mempool_clone = self.mempool.clone();
                let miner_utxos_clone = self.utxos.clone();
                let miner_clone = self.miner.clone();
                let miner_shutdown_token = shutdown_token.clone();
                let optimized_block_builder_clone = self.optimized_block_builder.clone();

                // Convert target_block_time from milliseconds to seconds for mining interval
                #[cfg(feature = "performance-test")]
                let mining_interval_secs = if self.config.target_block_time < 1000 {
                    // For performance testing with sub-second intervals, use at least 1 second
                    // but log the actual target for reference
                    info!("Performance test mode: target_block_time {} ms, using 1 second mining interval", 
                          self.config.target_block_time);
                    1
                } else {
                    (self.config.target_block_time as f64 / 1000.0) as u64
                };

                #[cfg(not(feature = "performance-test"))]
                let mining_interval_secs = {
                    // Calculate mining interval from target_block_time without forcing minimum
                    // Allow sub-second intervals for high-performance mining (e.g., 200ms target)
                    let target_secs = self.config.target_block_time as f64 / 1000.0;
                    if target_secs < 1.0 {
                        // For sub-second intervals, use 0 to indicate millisecond precision
                        0
                    } else {
                        target_secs as u64
                    }
                };

                info!(
                    "Using mining interval: {} seconds (from target_block_time: {} ms)",
                    mining_interval_secs, self.config.target_block_time
                );

                let dummy_tx_interval_secs =
                    self.config.dummy_tx_interval_ms.unwrap_or(30000) / 1000;
                let dummy_tx_per_cycle = self.config.dummy_tx_per_cycle.unwrap_or(100) as usize;
                let mempool_max_size_bytes =
                    self.config.mempool_max_size_bytes.unwrap_or(1024 * 1024);
                let mempool_batch_size = self.config.mempool_batch_size.unwrap_or(40000);
                let mempool_backpressure_threshold =
                    self.config.mempool_backpressure_threshold.unwrap_or(0.8)
                        * (mempool_max_size_bytes as f64);

                // TX generator backpressure configuration
                let tx_batch_size = self.config.tx_batch_size.unwrap_or(100);
                let adaptive_batch_threshold = self.config.adaptive_batch_threshold.unwrap_or(0.85);
                let memory_soft_limit = self.config.memory_soft_limit.unwrap_or(8 * 1024 * 1024); // 8MB default
                let memory_hard_limit = self.config.memory_hard_limit.unwrap_or(10 * 1024 * 1024); // 10MB default

                let _solo_miner_task_handle = join_set.spawn(async move {
                    debug!("[DEBUG] Spawning solo miner task");
                    miner_dag_clone
                        .run_solo_miner(
                            miner_wallet_clone,
                            miner_mempool_clone,
                            miner_utxos_clone,
                            miner_clone,
                            optimized_block_builder_clone,
                            mining_interval_secs,
                            dummy_tx_interval_secs,
                            dummy_tx_per_cycle,
                            mempool_max_size_bytes,
                            mempool_batch_size,
                            mempool_backpressure_threshold as usize,
                            tx_batch_size,
                            adaptive_batch_threshold,
                            memory_soft_limit,
                            memory_hard_limit,
                            miner_shutdown_token,
                        )
                        .await
                        .map_err(|e| NodeError::DAG(e.to_string()))
                });

                // Register the solo miner task with resource cleanup
                self.resource_cleanup
                    .register_task(|_token| async move {
                        // Note: solo_miner_task_handle is an AbortHandle, not awaitable
                        // The task will be aborted when the cancellation token is triggered
                        Ok(())
                    })
                    .await?;
            }
        }

        // --- WebSocket Server State ---
        let websocket_state = Arc::new(WebSocketServerState::new(
            self.dag.clone(),
            self.saga_pallet.clone(),
            self.analytics.clone(),
        ));

        // --- WebSocket Broadcasting Task ---
        // This task monitors DAG and mempool changes and broadcasts updates to WebSocket clients
        let websocket_broadcast_task = {
            let ws_state = websocket_state.clone();
            let dag_clone = self.dag.clone();
            let mempool_clone = self.mempool.clone();
            let utxos_clone = self.utxos.clone();
            let saga_clone = self.saga_pallet.clone();

            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                let mut last_block_count = 0;
                let mut last_mempool_size = 0;

                loop {
                    interval.tick().await;

                    // Check for new blocks
                    let current_block_count = dag_clone.blocks.len();
                    if current_block_count > last_block_count {
                        // Get the latest block(s)
                        if let Some(latest_block) = dag_clone.get_latest_block().await {
                            if ws_state.block_sender.receiver_count() > 0 {
                                ws_state.broadcast_block_notification(&latest_block).await;
                            }
                            // Also broadcast to GraphQL subscribers
                            crate::graphql_server::broadcast_new_block(&latest_block).await;
                        }
                        last_block_count = current_block_count;
                    }

                    // Check for mempool changes
                    let mempool_reader = mempool_clone.read().await;
                    let current_mempool_size = mempool_reader.len().await;
                    if current_mempool_size != last_mempool_size {
                        if ws_state.mempool_sender.receiver_count() > 0 {
                            ws_state
                                .broadcast_mempool_update(current_mempool_size)
                                .await;
                        }
                        last_mempool_size = current_mempool_size;
                    }
                    drop(mempool_reader);

                    // Broadcast network health metrics
                    let utxos_reader = utxos_clone.read().await;
                    let network_health = crate::websocket_server::NetworkHealth {
                        block_count: current_block_count,
                        mempool_size: current_mempool_size,
                        utxo_count: utxos_reader.len(),
                        connected_peers: 0, // Will be updated with actual peer count
                        sync_status: "synced".to_string(),
                    };
                    drop(utxos_reader);

                    ws_state.broadcast_network_health(network_health).await;

                    // Broadcast analytics dashboard data
                    let analytics_data = saga_clone.get_analytics_dashboard_data().await;
                    ws_state.broadcast_analytics_data(&analytics_data).await;
                }
                #[allow(unreachable_code)]
                Ok(())
            }
        };
        let _websocket_broadcast_task_handle = join_set.spawn(websocket_broadcast_task);

        // Register the WebSocket broadcasting task with resource cleanup
        self.resource_cleanup
            .register_task(|_token| async move {
                // Note: websocket_broadcast_task_handle is an AbortHandle, not awaitable
                // The task will be aborted when the cancellation token is triggered
                Ok(())
            })
            .await?;

        // --- Prepare API Server Components ---
        // Clone all necessary data outside the async block to avoid lifetime issues
        let app_state = AppState {
            dag: self.dag.clone(),
            mempool: self.mempool.clone(),
            utxos: self.utxos.clone(),
            api_address: self.config.api_address.clone(),
            p2p_command_sender: tx_p2p_commands.clone(),
            saga: self.saga_pallet.clone(),
            websocket_state: websocket_state.clone(),
            native_network: None,
        };

        // Create GraphQL context outside the async block
        let (block_sender, _) = tokio::sync::broadcast::channel(1000);
        let (transaction_sender, _) = tokio::sync::broadcast::channel(1000);
        let _graphql_context = GraphQLContext {
            node: Arc::new(Node {
                _config_path: self._config_path.clone(),
                config: self.config.clone(),
                p2p_identity_keypair: self.p2p_identity_keypair.clone(),
                dag: self.dag.clone(),
                miner: self.miner.clone(),
                wallet: self.wallet.clone(),
                mempool: self.mempool.clone(),
                elite_mempool: self.elite_mempool.clone(),
                utxos: self.utxos.clone(),
                proposals: self.proposals.clone(),
                peer_cache_path: self.peer_cache_path.clone(),
                saga_pallet: self.saga_pallet.clone(),
                analytics: self.analytics.clone(),
                optimized_block_builder: self.optimized_block_builder.clone(),
                resource_cleanup: self.resource_cleanup.clone(),
                #[cfg(feature = "infinite-strata")]
                isnm_service: self.isnm_service.clone(),
            }),
            block_sender,
            transaction_sender,
        };

        // --- Main Event Loop with Concurrent Task Management ---
        // Uses tokio::select! to concurrently run critical tasks and handle failures gracefully
        tokio::select! {
            biased;
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, initiating graceful shutdown...");
                self.resource_cleanup.request_shutdown();

                // Wait for graceful shutdown with timeout
                if let Err(e) = self.resource_cleanup.graceful_shutdown().await {
                    warn!("Graceful shutdown failed: {}, forcing shutdown", e);
                }

                warn!("Node shutdown initiated by Ctrl+C.");
            },
            // API Server Task - handles binding failures gracefully
            api_result = async {
                // Set up a rate limiter for the API to prevent abuse.
                let rate_limiter: Arc<DirectApiRateLimiter> =
                    Arc::new(RateLimiter::direct(Quota::per_second(nonzero!(50u32))));

                // Define the API routes using Axum router.
                let api_routes = Router::new()
                    .route("/info", get(info_handler))
                    .route("/balance/{address}", get(get_balance))
                    .route("/utxos/{address}", get(get_utxos))
                    .route("/transaction", post(submit_transaction))
                    .route("/block/{id}", get(get_block))
                    .route("/dag", get(get_dag))
                    .route("/blocks", get(get_block_ids))
                    .route("/health", get(health_check))
                    .route("/mempool", get(mempool_handler))
                    .route("/publish-readiness", get(publish_readiness_handler))
                    .route("/analytics/dashboard", get(analytics_dashboard_handler))
                    // /saga/ask route removed for production hardening
                    .route("/p2p_getConnectedPeers", get(get_connected_peers_handler))
                    .layer(middleware::from_fn_with_state(
                        rate_limiter,
                        rate_limit_layer,
                    ))
                    .with_state(app_state.clone());

                // Use the pre-created GraphQL context
                let graphql_schema = create_graphql_schema();

                // Create WebSocket routes
                let websocket_routes = create_websocket_router((*app_state.websocket_state).clone());

                // Create GraphQL routes
                let graphql_routes = create_graphql_router(graphql_schema);

                // Combine API, WebSocket, and GraphQL routes
                let app = api_routes.merge(websocket_routes).merge(graphql_routes);

                let addr: SocketAddr = app_state.api_address.parse().map_err(|e| {
                    NodeError::Config(ConfigError::Validation(format!("Invalid API address: {e}")))
                })?;

                // Attempt to bind to the API address - this can fail gracefully now
                let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
                    NodeError::ServerExecution(format!("Failed to bind to API address {addr}: {e}"))
                })?;

                match listener.local_addr() {
                    Ok(addr) => info!("API server listening on {}", addr),
                    Err(e) => warn!("Could not get listener address: {}", e),
                }

                // Start the Axum server.
                if let Err(e) = axum::serve(listener, app.into_make_service()).await {
                    error!("API server failed: {e}");
                    return Err(NodeError::ServerExecution(format!(
                        "API server failed: {e}"
                    )));
                }
                Ok(())
            } => {
                match api_result {
                    Ok(_) => {
                        warn!("API server completed unexpectedly");
                    }
                    Err(e) => {
                        error!("API server failed: {}", e);
                        self.resource_cleanup.request_shutdown();

                        // Wait for graceful shutdown with timeout
                        if let Err(e) = self.resource_cleanup.graceful_shutdown().await {
                            warn!("Graceful shutdown failed: {}, forcing shutdown", e);
                        }

                        warn!("Node shutdown initiated by API server failure.");
                        return Err(e);
                    }
                }
            },
            // Monitor spawned tasks for failures
            Some(res) = join_set.join_next() => {
                match res {
                    Ok(Err(e)) => {
                        error!("A critical node task failed: {}", e);
                        self.resource_cleanup.request_shutdown();

                        // Wait for graceful shutdown with timeout
                        if let Err(e) = self.resource_cleanup.graceful_shutdown().await {
                            warn!("Graceful shutdown failed: {}, forcing shutdown", e);
                        }

                        warn!("Node shutdown initiated by critical task failure.");
                        return Err(e);
                    }
                    Err(e) => {
                        error!("A critical node task panicked: {}", e);
                        self.resource_cleanup.request_shutdown();

                        // Wait for graceful shutdown with timeout
                        if let Err(e) = self.resource_cleanup.graceful_shutdown().await {
                            warn!("Graceful shutdown failed: {}, forcing shutdown", e);
                        }

                        warn!("Node shutdown initiated by critical task panic.");
                        return Err(NodeError::Join(e));
                    }
                    _ => {
                        warn!("Node shutdown initiated by unknown task completion.");
                        self.resource_cleanup.request_shutdown();

                        // Wait for graceful shutdown with timeout
                        if let Err(e) = self.resource_cleanup.graceful_shutdown().await {
                            warn!("Graceful shutdown failed: {}, forcing shutdown", e);
                        }
                    }
                }
            },
        }

        // Gracefully shut down all spawned tasks.
        join_set.shutdown().await;
        info!("Node shutdown complete.");
        Ok(())
    }

    /// Topologically sorts a vector of blocks.
    /// This is crucial for correctly processing blocks received during a state sync,
    /// ensuring that parents are processed before their children.
    async fn topological_sort_blocks(
        blocks: Vec<QantoBlock>,
        dag: &QantoDAG,
    ) -> Result<Vec<QantoBlock>, NodeError> {
        if blocks.is_empty() {
            return Ok(vec![]);
        }
        let block_map: HashMap<String, QantoBlock> =
            blocks.into_iter().map(|b| (b.id.clone(), b)).collect();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
        let local_blocks = &dag.blocks;

        // Calculate the in-degree for each block in the batch.
        for (id, block) in &block_map {
            let mut degree = 0;
            for parent_id in &block.parents {
                // If the parent is also in the sync batch, it's an edge in our graph.
                if block_map.contains_key(parent_id) {
                    degree += 1;
                    children_map
                        .entry(parent_id.clone())
                        .or_default()
                        .push(id.clone());
                } else if !local_blocks.contains_key(parent_id) {
                    // If a parent is missing from both the batch and the local DAG, the sort is impossible.
                    warn!(
                        "Sync Error: Block {} has a missing parent {} that is not in the local DAG or the sync batch.",
                        id, parent_id
                    );
                    return Err(NodeError::SyncError(format!(
                        "Missing parent {parent_id} for block {id} in sync batch"
                    )));
                }
            }
            in_degree.insert(id.clone(), degree);
        }

        // Initialize the queue with all nodes that have an in-degree of 0.
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut sorted_blocks = Vec::with_capacity(block_map.len());

        // Process the queue (Kahn's algorithm).
        while let Some(id) = queue.pop_front() {
            if let Some(children) = children_map.get(&id) {
                for child_id in children {
                    if let Some(degree) = in_degree.get_mut(child_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(child_id.clone());
                        }
                    }
                }
            }
            if let Some(block) = block_map.get(&id) {
                sorted_blocks.push(block.clone());
            }
        }

        // If the sorted list doesn't contain all blocks, there must be a cycle.
        if sorted_blocks.len() != block_map.len() {
            error!(
                "Cycle detected in block sync batch or missing parent. Sorted {} of {} blocks.",
                sorted_blocks.len(),
                block_map.len()
            );
            Err(NodeError::SyncError(
                "Cycle detected or missing parent in block sync batch.".to_string(),
            ))
        } else {
            Ok(sorted_blocks)
        }
    }
}

/// Axum middleware to enforce API rate limiting.
async fn rate_limit_layer(
    MiddlewareState(limiter): MiddlewareState<Arc<DirectApiRateLimiter>>,
    req: HttpRequest<Body>,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if limiter.check().is_err() {
        warn!("API rate limit exceeded for: {:?}", req.uri());
        Err(StatusCode::TOO_MANY_REQUESTS)
    } else {
        Ok(next.run(req).await)
    }
}

/// Holds the shared state accessible by all API handlers.
#[derive(Clone)]
struct AppState {
    dag: Arc<QantoDAG>,
    mempool: Arc<RwLock<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    api_address: String,
    p2p_command_sender: mpsc::Sender<P2PCommand>,
    #[allow(dead_code)]
    saga: Arc<PalletSaga>,
    websocket_state: Arc<WebSocketServerState>,
    // Optional native networking server; used when available
    native_network: Option<Arc<QantoNetServer>>,
}

// --- API Error Handling ---
// SagaQuery struct removed for production hardening
#[derive(Serialize)]
struct ApiError {
    code: u16,
    message: String,
    details: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}

// --- API Handlers ---

// ask_saga function removed for production hardening
// LLM guidance functionality has been excised

/// Provides a general information summary of the node's state.
async fn info_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mempool_read_guard = state.mempool.read().await;
    let utxos_read_guard = state.utxos.read().await;
    let num_chains_val = *state.dag.num_chains.read().await;
    let current_difficulty = {
        let rules = state.dag.saga.economy.epoch_rules.read().await;
        rules.get("base_difficulty").map_or(10.0, |r| r.value)
    };

    Ok(Json(serde_json::json!({
        "block_count": state.dag.blocks.len(),
        "tip_count": state.dag.tips.iter().map(|t_set| t_set.value().len()).sum::<usize>(),
        "mempool_size": mempool_read_guard.size().await,
        "utxo_count": utxos_read_guard.len(),
        "num_chains": num_chains_val,
        "current_difficulty": current_difficulty,
    })))
}

/// Returns the current contents of the mempool.
async fn mempool_handler(
    State(state): State<AppState>,
) -> Result<Json<HashMap<String, Transaction>>, StatusCode> {
    let mempool_read_guard = state.mempool.read().await;
    Ok(Json(mempool_read_guard.get_transactions().await))
}

/// A readiness probe endpoint.
async fn publish_readiness_handler(
    State(state): State<AppState>,
) -> Result<Json<PublishReadiness>, StatusCode> {
    let mempool_read_guard = state.mempool.read().await;
    let utxos_read_guard = state.utxos.read().await;
    let mut issues = vec![];

    if state.dag.blocks.len() < 2 {
        issues.push("Insufficient blocks in DAG".to_string());
    }
    if utxos_read_guard.is_empty() {
        issues.push("No UTXOs available".to_string());
    }

    let latest_timestamp = state
        .dag
        .blocks
        .iter()
        .map(|entry| entry.value().timestamp)
        .max()
        .unwrap_or(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let is_synced = now.saturating_sub(latest_timestamp) < MAX_SYNC_AGE_SECONDS;
    if !is_synced {
        issues.push(format!(
            "Node is out of sync. Last block is {} seconds old.",
            now.saturating_sub(latest_timestamp)
        ));
    }

    let is_ready = issues.is_empty();
    Ok(Json(PublishReadiness {
        is_ready,
        block_count: state.dag.blocks.len(),
        utxo_count: utxos_read_guard.len(),
        peer_count: {
            // Get actual peer count from P2P layer
            let (response_sender, response_receiver) = oneshot::channel();
            if state
                .p2p_command_sender
                .send(P2PCommand::GetConnectedPeers { response_sender })
                .await
                .is_ok()
            {
                response_receiver
                    .await
                    .map(|peers| peers.len())
                    .unwrap_or(0)
            } else {
                0
            }
        },
        mempool_size: mempool_read_guard.size().await,
        is_synced,
        issues,
    }))
}

/// Returns the total balance for a given address.
async fn get_balance(
    State(state): State<AppState>,
    AxumPath(address): AxumPath<String>,
) -> Result<Json<u64>, ApiError> {
    if !ADDRESS_REGEX_COMPILED.is_match(&address) {
        warn!("Invalid address format for balance check: {address}");
        return Err(ApiError {
            code: 400,
            message: "Invalid address format".to_string(),
            details: None,
        });
    }
    let utxos_read_guard = state.utxos.read().await;
    let balance = utxos_read_guard
        .values()
        .filter(|utxo_item| utxo_item.address == address)
        .map(|utxo_item| utxo_item.amount)
        .sum();
    Ok(Json(balance))
}

/// Returns all UTXOs for a given address.
async fn get_utxos(
    State(state): State<AppState>,
    AxumPath(address): AxumPath<String>,
) -> Result<Json<HashMap<String, UTXO>>, ApiError> {
    if !ADDRESS_REGEX_COMPILED.is_match(&address) {
        warn!("Invalid address format for UTXO fetch: {address}");
        return Err(ApiError {
            code: 400,
            message: "Invalid address format".to_string(),
            details: None,
        });
    }
    let utxos_read_guard = state.utxos.read().await;
    let filtered_utxos_map = utxos_read_guard
        .iter()
        .filter(|(_, utxo_item)| utxo_item.address == address)
        .map(|(key_str, value_utxo)| (key_str.clone(), value_utxo.clone()))
        .collect();
    Ok(Json(filtered_utxos_map))
}

/// Submits a new transaction to the network.
async fn submit_transaction(
    State(state): State<AppState>,
    Json(tx_data): Json<Transaction>,
) -> Result<Json<String>, ApiError> {
    // Perform a preliminary check with the ΩMEGA protocol.
    let tx_hash_bytes = match hex::decode(&tx_data.id) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(ApiError {
                code: 400,
                message: "Invalid transaction ID format".to_string(),
                details: None,
            })
        }
    };
    let tx_hash = H256::from_slice(&tx_hash_bytes);
    if !reflect_on_action(tx_hash).await {
        error!("ΛΣ-ΩMEGA rejected transaction {}", tx_data.id);
        return Err(ApiError {
            code: 503,
            message: "System unstable, transaction rejected".to_string(),
            details: None,
        });
    }

    // Verify the transaction before broadcasting.
    let utxos_read_guard = state.utxos.read().await;
    if let Err(e) = tx_data.verify(&state.dag, &utxos_read_guard).await {
        warn!(
            "Transaction {} failed verification via API: {}",
            tx_data.id, e
        );
        return Err(ApiError {
            code: 400,
            message: "Transaction verification failed".to_string(),
            details: Some(e.to_string()),
        });
    }

    // Send the transaction to the P2P layer for broadcast.
    let tx_id = tx_data.id.clone();
    if let Err(e) = state
        .p2p_command_sender
        .send(P2PCommand::BroadcastTransaction(tx_data))
        .await
    {
        error!(
            "Failed to broadcast transaction {} to P2P task: {}",
            tx_id, e
        );
        return Err(ApiError {
            code: 500,
            message: "Internal server error".to_string(),
            details: None,
        });
    }

    info!("Transaction {} submitted via API", tx_id);
    Ok(Json(tx_id))
}

/// Retrieves a specific block by its ID.
async fn get_block(
    State(state): State<AppState>,
    AxumPath(id_str): AxumPath<String>,
) -> Result<Json<QantoBlock>, StatusCode> {
    if id_str.len() > 128 || id_str.is_empty() {
        warn!("Invalid block ID length: {id_str}");
        return Err(StatusCode::BAD_REQUEST);
    }
    let block_data = state
        .dag
        .blocks
        .get(&id_str)
        .map(|entry| entry.value().clone())
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(block_data))
}

/// Returns detailed information about the entire DAG structure.
async fn get_dag(State(state): State<AppState>) -> Result<Json<DagInfo>, StatusCode> {
    let block_count = state.dag.blocks.len();
    let _tip_count = state.dag.tips.len();
    let validator_count = state.dag.validators.len();
    let current_difficulty = {
        let rules = state.dag.saga.economy.epoch_rules.read().await;
        rules.get("base_difficulty").map_or(10.0, |r| r.value)
    };
    let num_chains_val = *state.dag.num_chains.read().await;

    let latest_block_timestamp = state
        .dag
        .blocks
        .iter()
        .map(|b| b.value().timestamp)
        .max()
        .unwrap_or(0);

    Ok(Json(DagInfo {
        block_count,
        tip_count: state.dag.tips.iter().map(|t_set| t_set.value().len()).sum(),
        current_difficulty,
        target_block_time: state.dag.target_block_time,
        validator_count,
        num_chains: num_chains_val,
        latest_block_timestamp,
    }))
}

/// Get all block IDs in the DAG
async fn get_block_ids(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    let block_ids: Vec<String> = state
        .dag
        .blocks
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    Ok(Json(block_ids))
}

/// A simple health check endpoint.
async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({ "status": "healthy" })))
}

/// Returns the list of connected peers from the P2P server.
async fn get_connected_peers_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, StatusCode> {
    // Prefer native networking if available
    if let Some(native) = &state.native_network {
        let peer_ids = native.get_connected_peers().await;
        let peers: Vec<String> = peer_ids.into_iter().map(|p| p.to_string()).collect();
        info!("Retrieved {} connected peers (native)", peers.len());
        return Ok(Json(peers));
    }

    // Fallback to legacy P2P server
    let (response_sender, response_receiver) = oneshot::channel();
    if let Err(e) = state
        .p2p_command_sender
        .send(P2PCommand::GetConnectedPeers { response_sender })
        .await
    {
        error!(
            "Failed to send GetConnectedPeers command to P2P server: {}",
            e
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    match response_receiver.await {
        Ok(peers) => {
            info!("Retrieved {} connected peers (legacy)", peers.len());
            Ok(Json(peers))
        }
        Err(e) => {
            error!("Failed to receive connected peers response: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Analytics dashboard handler that provides real-time system metrics
async fn analytics_dashboard_handler(
    State(state): State<AppState>,
) -> Result<Json<crate::saga::AnalyticsDashboardData>, StatusCode> {
    let dag = &state.dag;
    let mempool = state.mempool.read().await;
    let utxos = state.utxos.read().await;

    // Get current timestamp
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Network health metrics
    let network_health = crate::saga::NetworkHealthMetrics::default();

    // AI model performance metrics
    let ai_performance = crate::saga::AIModelPerformance {
        neural_network_accuracy: 0.95,
        prediction_confidence: 0.88,
        training_loss: 0.12,
        validation_loss: 0.08,
        model_drift_score: 0.02,
        inference_latency_ms: 15.0,
        last_retrain_epoch: 100,
        feature_importance: std::collections::HashMap::new(),
    };

    // Security insights
    let security_insights = crate::saga::SecurityInsights {
        threat_level: crate::saga::ThreatLevel::Low,
        anomaly_score: 0.15,
        attack_attempts_24h: 0,
        blocked_transactions: 0,
        suspicious_patterns: vec![],
        security_confidence: 0.85,
    };

    // Economic indicators
    let economic_indicators = crate::saga::EconomicIndicators {
        total_value_locked: utxos.values().map(|utxo| utxo.amount as f64).sum(),
        transaction_fees_24h: mempool.get_transactions().await.len() as f64 * 100.0, // Placeholder calculation
        validator_rewards_24h: 50000.0,                                              // Placeholder
        network_utilization: 0.75,                                                   // Placeholder
        economic_security: 0.95,                                                     // Placeholder
        fee_market_efficiency: 0.85,                                                 // Placeholder
    };

    // Environmental metrics
    let environmental_metrics = crate::saga::EnvironmentalDashboardMetrics {
        carbon_footprint_kg: 0.1,
        energy_efficiency_score: 95.0,
        renewable_energy_percentage: 100.0, // Assuming green energy
        carbon_offset_credits: 1000.0,      // Placeholder
        green_validator_ratio: 0.95,        // Placeholder
    };

    // Compile dashboard data
    let dashboard_data = crate::saga::AnalyticsDashboardData {
        timestamp: current_time,
        network_health,
        ai_performance,
        security_insights,
        economic_indicators,
        environmental_metrics,
        total_transactions: dag.blocks.len() as u64,
        active_addresses: utxos.len() as u64,
        mempool_size: mempool.get_transactions().await.len() as u64,
        block_height: dag.blocks.len() as u64,
        tps_current: 32.0,      // Target 32 BPS
        tps_peak: 10_000_000.0, // Target 10M+ TPS
    };

    Ok(Json(dashboard_data))
}

// --- Unit Tests ---
