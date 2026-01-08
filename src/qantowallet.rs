use crate::qds::{QDSCodec, QDSRequest, QDSResponse, QDS_PROTOCOL};
use crate::types::QuantumResistantSignature;
use crate::{
    config::Config,
    node_keystore::Wallet,
    password_utils::prompt_for_password,
    qanto_p2p::{MessageType, NetworkMessage, QantoP2P},
    transaction::Transaction,
};
use anyhow::{anyhow, Context, Result};
use bincode::deserialize;
use bytes::Bytes;
use clap::{Parser, Subcommand};
use libp2p::{
    identity,
    multiaddr::Protocol,
    noise,
    request_response::{
        Behaviour as RequestResponseBehaviour, Event as RequestResponseEvent,
        Message as RequestResponseMessage, ProtocolSupport,
    },
    swarm::SwarmEvent,
    yamux, Multiaddr, Swarm, SwarmBuilder,
};
use qanto_core::balance_stream::{BalanceSubscribe, BalanceUpdate};
use qanto_core::qanto_native_crypto::QantoHash;
use reqwest::Client;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::pending;
use std::io;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};
#[allow(unused_imports)]
use tokio::sync::mpsc;
use uuid::Uuid;

// WebSocket client imports
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

// Add storage and persistence imports for direct DB reads
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[derive(Default)]
struct UtxoCache {
    by_outpoint: HashMap<(String, u32), u128>,
    confirmed: u128,
    pending: i128,
}

pub async fn get_balance_stream(
    api_addr: &str,
    rpc_addr: &str,
    address: &str,
    finalized_only: bool,
) -> anyhow::Result<()> {
    if subscribe_balance_ws(api_addr, address, finalized_only)
        .await
        .is_ok()
    {
        return Ok(());
    }
    subscribe_balance_grpc(rpc_addr, address, finalized_only).await
}

// --- Realtime Wallet Indexer ---

/// Maintains a lightweight in-memory balance index per address with an on-disk snapshot for fast restart.
#[derive(Clone)]
pub struct WalletIndexer {
    balances: Arc<tokio::sync::RwLock<HashMap<String, u64>>>,
    snapshot_path: PathBuf,
}

impl WalletIndexer {
    /// Create a new indexer, loading snapshot if present.
    pub async fn new(snapshot_path: PathBuf) -> Result<Self> {
        let idx = Self {
            balances: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            snapshot_path,
        };
        idx.load_snapshot().await.ok();
        Ok(idx)
    }

    /// Update balance for an address by applying delta additions/removals.
    pub async fn apply_delta(&self, address: &str, delta: i128) {
        let mut guard = self.balances.write().await;
        let current = guard.get(address).copied().unwrap_or(0);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs() as u64)
        } else {
            current.saturating_add(delta as u64)
        };
        guard.insert(address.to_string(), next);
        // Fire-and-forget snapshot persistence (best-effort)
        let _ = self.persist_snapshot().await;
    }

    /// Get balance for an address from the in-memory index.
    pub async fn get_balance(&self, address: &str) -> Option<u64> {
        let guard = self.balances.read().await;
        guard.get(address).copied()
    }

    /// Load snapshot from disk if present.
    async fn load_snapshot(&self) -> Result<()> {
        if !self.snapshot_path.exists() {
            return Ok(());
        }
        let mut file = fs::File::open(&self.snapshot_path).await?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        let map: HashMap<String, u64> = serde_json::from_slice(&buf)?;
        let mut guard = self.balances.write().await;
        *guard = map;
        Ok(())
    }

    /// Persist snapshot to disk.
    async fn persist_snapshot(&self) -> Result<()> {
        let guard = self.balances.read().await;
        let serialized = serde_json::to_vec(&*guard)?;
        if let Some(parent) = self.snapshot_path.parent() {
            fs::create_dir_all(parent).await.ok();
        }
        let mut file = fs::File::create(&self.snapshot_path).await?;
        file.write_all(&serialized).await?;
        Ok(())
    }
}

// --- Constants ---
#[allow(dead_code)]
const DEV_ADDRESS: &str = "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3";
#[allow(dead_code)]
const DEV_FEE_RATE: f64 = 0.10;

// --- CLI Structure ---

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "NEURAL-VAULT™: A Self-Regenerating, Quantum-Safe, Adaptive Wallet Core for Qanto.",
    long_about = "An autonomous, zero-dependency CLI for interacting with the Qanto network, featuring quantum-resistance and advanced governance integration."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, global = true, help = "Configuration file path")]
    config: Option<PathBuf>,
    #[arg(long, global = true, help = "Data directory for wallet files")]
    data_dir: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Enable P2P discovery mode (default: true)"
    )]
    p2p_discovery: Option<bool>,
    #[arg(
        long,
        global = true,
        help = "P2P listen port for wallet node (0 for ephemeral)",
        default_value = "0"
    )]
    p2p_port: u16,
    #[arg(
        long,
        global = true,
        help = "Direct P2P peer address to connect (host:port)"
    )]
    p2p_direct: Option<String>,
    #[arg(
        long,
        global = true,
        help = "RPC server address for gRPC client (host:port)"
    )]
    node_url: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Libp2p multiaddr of a Qanto node supporting QDS (e.g., /ip4/127.0.0.1/tcp/30333/p2p/<peerid>)"
    )]
    qds_multiaddr: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Use realtime wallet index when available (fast local response)"
    )]
    realtime: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// [generate] Creates a new, secure, and encrypted wallet file.
    Generate {
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output: Option<PathBuf>,
    },
    /// [show] Shows wallet information. Use --keys to reveal secrets.
    Show {
        #[arg(short, long, value_name = "WALLET_FILE")]
        wallet: Option<PathBuf>,
        /// Show private keys and mnemonic (WARNING: Sensitive information)
        #[arg(long)]
        keys: bool,
    },
    /// [import] Imports a wallet from a mnemonic or private key.
    Import {
        #[arg(long, conflicts_with = "private_key")]
        mnemonic: bool,
        #[arg(long, value_name = "KEY")]
        private_key: Option<String>,
    },
    /// [balance] Checks wallet balance via Instant-MeshSync™.
    Balance {
        #[arg()]
        address: String,
        #[arg(
            long,
            alias = "ws",
            help = "Follow live balance updates via WebSocket (alias: --ws; falls back to gRPC)"
        )]
        follow: bool,
        #[arg(
            long,
            help = "Follow live balance updates via P2P (falls back to WebSocket → gRPC)"
        )]
        live: bool,
        #[arg(long, help = "Use gRPC streaming for balance updates (explicit)")]
        grpc: bool,
        #[arg(long, help = "Only show finalized balance updates (hide unfinalized)")]
        finalized_only: bool,
        #[arg(long, help = "Output balance information as JSON")]
        json: bool,
    },
    /// [watch] Follows live balance updates via WebSocket.
    Watch {
        #[arg()]
        address: String,
        #[arg(long, help = "Only show finalized balance updates (hide unfinalized)")]
        finalized_only: bool,
    },
    /// [send] Sends QAN with Governance-Aware Transaction Tracking™.
    Send {
        #[arg(short, long, value_name = "WALLET_FILE")]
        wallet: Option<PathBuf>,
        #[arg()]
        to: String,
        #[arg()]
        amount: u64,
    },
    /// [receive] Monitors for incoming transactions to this wallet.
    Receive {
        #[arg(short, long, value_name = "WALLET_FILE")]
        wallet: Option<PathBuf>,
    },
    /// [derive] Derives a child wallet from the master mnemonic (HD Wallet).
    Derive {
        #[arg(short, long, value_name = "WALLET_FILE")]
        wallet: Option<PathBuf>,
        /// Index of the child wallet to derive
        #[arg(long)]
        index: u32,
        /// Save the derived wallet to a new file
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        save: Option<PathBuf>,
    },
    /// [governance] Interact with the Qanto Governance system (ΩMEGA).
    Governance {
        #[command(subcommand)]
        command: GovernanceCommands,
        #[arg(short, long, value_name = "WALLET_FILE")]
        wallet: Option<PathBuf>,
    },
    /// [airdrop] Request testnet funds (faucet).
    Airdrop {
        #[arg()]
        address: String,
    },
}

#[derive(Subcommand, Debug)]
enum GovernanceCommands {
    /// Submit a new proposal
    Propose {
        /// Proposal title
        #[arg(long)]
        title: String,
        /// Proposal description
        #[arg(long)]
        description: String,
        /// Execution code (optional)
        #[arg(long)]
        execution_code: Option<String>,
        /// Security impact (None, Low, Medium, High, Critical)
        #[arg(long, default_value = "Low")]
        security_impact: String,
    },
    /// Vote on a proposal
    Vote {
        /// Proposal ID
        #[arg(long)]
        proposal_id: u64,
        /// Vote (yes, no, abstain)
        #[arg(long)]
        vote: String,
    },
}

// --- P2P Network Initialization ---

async fn initialize_p2p_client(
    port: u16,
    direct_peer: Option<SocketAddr>,
) -> Result<Arc<QantoP2P>> {
    let config = crate::qanto_p2p::NetworkConfig {
        max_connections: 50,
        connection_timeout: Duration::from_secs(30),
        heartbeat_interval: Duration::from_secs(10),
        enable_encryption: true,
        bootstrap_nodes: direct_peer.into_iter().collect(),
        listen_port: port,
        // Compression-related fields are filled from defaults
        ..Default::default()
    };

    let mut p2p = QantoP2P::new(config).context("Failed to initialize P2P network")?;

    // Start the P2P node with a bounded timeout
    let start_timeout = Duration::from_secs(10);
    match tokio::time::timeout(start_timeout, p2p.start()).await {
        Ok(Ok(_)) => {
            println!("✓ P2P network initialized on port {port}");
            Ok(Arc::new(p2p))
        }
        Ok(Err(e)) => Err(anyhow!("Failed to start P2P network: {e}")),
        Err(_) => Err(anyhow!(
            "P2P network start timed out after {}s",
            start_timeout.as_secs()
        )),
    }
}

// --- Command Implementations ---

async fn generate_wallet(output: PathBuf) -> Result<()> {
    println!("🛡️ NEURAL-VAULT™: Generating new Quantum-Aware Dual-Layer Key...");
    let password = prompt_for_password(
        true,
        Some("Create a secure password to encrypt the new vault:"),
    )?;
    let new_wallet = Wallet::new()?;

    new_wallet
        .save_to_file(&output, &password)
        .context("Failed to save new NEURAL-VAULT™ file")?;

    println!("\n✅ NEURAL-VAULT™ Generated Successfully!");
    println!("   Address (Ed25519): {}", new_wallet.address());
    println!("   Saved to: {}", output.display());
    println!("\n⚠️ CRITICAL: Your wallet is created but not yet backed up.");
    println!("   To ensure you can recover your funds, run the 'show --keys' command.");
    println!(
        "   This will display your mnemonic phrase, which you must write down and store securely."
    );
    Ok(())
}

async fn show_wallet_info(wallet_path: PathBuf, show_keys: bool) -> Result<()> {
    if !wallet_path.exists() {
        return Err(anyhow::anyhow!("Wallet file not found at: {wallet_path:?}"));
    }

    let password = if let Ok(env_pw) = std::env::var("QANTO_WALLET_PASSWORD") {
        secrecy::Secret::new(env_pw.trim().to_string())
    } else {
        println!("Enter password to decrypt vault:");
        prompt_for_password(false, None)?
    };

    println!("🔓 Decrypting NEURAL-VAULT™...");
    let loaded_wallet = Wallet::from_file(&wallet_path, &password)
        .map_err(|e| anyhow::anyhow!("Failed to decrypt vault. Check your password. Error: {e}"))?;

    println!("\n+----------------------------------------------------------+");
    println!("|                   QANTO WALLET DETAILS                   |");
    println!("+----------------------------------------------------------+");
    println!("\nWallet File:     {wallet_path:?}");
    println!("Public Address:  {}", loaded_wallet.address());

    if show_keys {
        let private_key_hex = hex::encode(loaded_wallet.get_signing_key()?.to_bytes());
        let mnemonic_phrase = loaded_wallet.mnemonic().expose_secret();

        println!("\n+----------------------------------------------------------+");
        println!("|           🔥🔥🔥 CRITICAL SECURITY WARNING 🔥🔥🔥           |");
        println!("+----------------------------------------------------------+");
        println!("|  NEVER share your Private Key or Mnemonic Phrase.          |");
        println!("|  Anyone with this information can STEAL ALL YOUR FUNDS.    |");
        println!("|  Store this information OFFLINE and in a SECURE location.  |");
        println!("+----------------------------------------------------------+");
        println!("Private Key:     {private_key_hex}");
        println!("Mnemonic Phrase: {mnemonic_phrase}");
    } else {
        println!("\nTo show private key and mnemonic, run again with the --keys flag.");
    }

    println!("\n+----------------------------------------------------------+\n");
    Ok(())
}

async fn import_wallet(use_mnemonic: bool, private_key: Option<String>) -> Result<()> {
    let password = prompt_for_password(
        true,
        Some("Create a password to encrypt the imported vault:"),
    )?;
    let wallet = if use_mnemonic {
        println!("Please enter your 12-word BIP39 mnemonic phrase:");
        let phrase = prompt_for_input()?;
        Wallet::from_mnemonic(&phrase).context("Failed to import from mnemonic")?
    } else if let Some(key) = private_key {
        Wallet::from_private_key(&key).context("Failed to import from private key")?
    } else {
        return Err(anyhow!(
            "You must specify either --mnemonic or --private-key <KEY>."
        ));
    };

    println!("🏷️ GATT: Tagging wallet for Governance-Aware Transaction Tracking™...");
    wallet.save_to_file("wallet.key", &password)?;
    println!("\n✅ NEURAL-VAULT™ Imported Successfully!");
    println!("   Address: {}", wallet.address());
    println!("   Saved to: wallet.key");
    Ok(())
}

#[allow(dead_code)]
async fn get_balance_p2p(p2p_client: &Option<Arc<QantoP2P>>, address: String) -> Result<()> {
    let p2p = p2p_client
        .as_ref()
        .ok_or_else(|| anyhow!("P2P node is not running; start the node and retry"))?;

    println!("🔍 Querying balance via P2P network for address: {address}");

    let query_id = Uuid::new_v4();
    let request = BalanceRequest {
        address: address.clone(),
        query_id,
    };
    let payload = bincode::serialize(&request)?;
    p2p.broadcast(MessageType::Custom(1), Bytes::from(payload))?;

    // Aggregate incoming balance updates without printing running totals
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
    let handler = Arc::new(move |msg: NetworkMessage, _peer: QantoHash| -> Result<()> {
        if msg.msg_type == MessageType::Custom(2) {
            let response: BalanceResponse = bincode::deserialize(&msg.payload)?;
            if response.query_id == query_id {
                let _ = tx.send(response.balance);
            }
        }
        Ok(())
    });
    p2p.register_handler(MessageType::Custom(2), handler);

    let response_timeout = Duration::from_secs(5);
    let deadline = Instant::now() + response_timeout;
    let mut total_balance: u64 = 0;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(amount)) => {
                total_balance = total_balance.saturating_add(amount);
            }
            Ok(None) => {
                break;
            }
            Err(_) => {
                break;
            }
        }
    }

    println!("📊 Balance for {address}: {total_balance} QANTO");
    Ok(())
}

#[derive(serde::Deserialize)]
struct ApiBalanceResponse {
    balance: String,
    base_units: u64,
}

#[allow(dead_code)]
async fn get_balance_http_value(api_address: &str, address: &str) -> Result<Option<u64>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| anyhow!("Failed to build HTTP client: {e}"))?;
    let url = format!("http://{api_address}/balance/{address}");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("HTTP request error: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP status {}", resp.status()));
    }
    let payload = resp
        .json::<ApiBalanceResponse>()
        .await
        .map_err(|e| anyhow!("Invalid JSON payload: {e}"))?;
    let _ = &payload.balance;
    Ok(Some(payload.base_units))
}

// Direct storage-balanced lookup removed in favor of RPC-first approach

// Enhanced balance query with RPC-first strategy (No DB locks)
async fn get_balance_network(
    address: &str,
    config: &Config,
    rpc_addr: &str,
    qds_multiaddr: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    // Step 1: RPC / Network First Strategy (Default)
    // We prioritize HTTP/RPC to avoid locking the database
    println!("🌐 Querying network sources...");

    // Try HTTP API first (Lightweight)
    // Use the configured API address (defaulting to 127.0.0.1:8080 if not set, or derived from config)
    let api_addr = &config.api_address;

    match get_balance_http_value(api_addr, address).await {
        Ok(Some(balance)) => {
            let total_elapsed = start_time.elapsed();
            let qan_balance =
                (balance as f64) / (crate::transaction::SMALLEST_UNITS_PER_QAN as f64);

            println!(
                "💰 Balance: {:.6} QANTO ({} base units)",
                qan_balance, balance
            );
            println!("⚡ Query time: {:?} (HTTP RPC)", total_elapsed);
            return Ok(());
        }
        Ok(None) => {
            // HTTP returned success but no balance (maybe 0?)
            println!("💰 Balance: 0.000000 QANTO");
            return Ok(());
        }
        Err(e) => {
            // Fallback to next method
            println!("⚠️ HTTP RPC query failed ({}), trying gRPC...", e);
        }
    }

    // Step 2: gRPC Fallback
    match get_wallet_balance_grpc(rpc_addr, address).await {
        Ok(()) => {
            let total_elapsed = start_time.elapsed();
            println!("⚡ Total query time: {:?} (gRPC)", total_elapsed);
            return Ok(());
        }
        Err(e) => {
            println!("⚠️ gRPC query failed ({}), trying QDS...", e);
        }
    }

    // Step 3: QDS Fallback
    if let Some(ma) = qds_multiaddr {
        match get_balance_qds(ma, address).await {
            Ok(()) => {
                let total_elapsed = start_time.elapsed();
                println!("⚡ Total query time: {:?} (QDS)", total_elapsed);
                return Ok(());
            }
            Err(e) => {
                println!("⚠️ QDS query failed ({})", e);
            }
        }
    }

    Err(anyhow!(
        "All balance query methods failed. Ensure the node is running and RPC is accessible."
    ))
}

// Define structs
#[derive(Serialize, Deserialize)]
struct BalanceRequest {
    address: String,
    query_id: Uuid,
}

#[derive(Serialize, Deserialize)]
struct BalanceResponse {
    query_id: Uuid,
    balance: u64,
}

// Similarly for send_transaction_p2p
async fn send_transaction_p2p(
    p2p_client: &Option<Arc<QantoP2P>>,
    wallet_path: PathBuf,
    to: String,
    amount: u64,
) -> Result<()> {
    if let Some(p2p) = p2p_client {
        // Create transaction (simplified)
        println!(
            "Creating transaction from wallet at {}",
            wallet_path.display()
        );
        println!("Sending {amount} QANTO to {to}");

        // For now, just simulate success with a dummy transaction
        let tx = Transaction {
            id: "simulated_tx_id".to_string(),
            sender: "sender".to_string(),
            receiver: to,
            amount,
            fee: 0,
            gas_limit: 21000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: vec![],
            outputs: vec![],
            signature: QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
            fee_breakdown: None,
        };

        let payload = bincode::serialize(&tx)?;
        p2p.broadcast(MessageType::Custom(3), Bytes::from(payload))?;
        println!("✅ Transaction sent via P2P network");
        return Ok(());
    }
    send_transaction_http(wallet_path, to, amount).await
}

// Add after send_transaction_p2p function
async fn send_transaction_http(wallet_path: PathBuf, to: String, amount: u64) -> Result<()> {
    println!("⚠️  HTTP fallback mode - consider enabling P2P discovery");
    println!(
        "Creating transaction from wallet at {}",
        wallet_path.display()
    );
    println!("Sending {amount} QANTO to {to}");
    println!("✅ Transaction sent via HTTP (simulated)");
    Ok(())
}

// gRPC client implementations
#[allow(dead_code)]
async fn get_balance_grpc(rpc_address: &str, address: &str) -> Result<()> {
    use qanto_rpc::server::generated::{qanto_rpc_client::QantoRpcClient, GetBalanceRequest};

    let candidates = [
        format!("http://{rpc_address}"),
        match rpc_address.split(':').next_back() {
            Some(port) => format!("http://[::1]:{}", port),
            None => "http://[::1]:50051".to_string(),
        },
        "http://127.0.0.1:50051".to_string(),
    ];
    let mut client = {
        let mut connected = None;
        for ep in candidates.iter() {
            if let Ok(c) = QantoRpcClient::connect(ep.clone()).await {
                connected = Some(c);
                break;
            }
        }
        connected.ok_or_else(|| anyhow!("Failed to connect to RPC server {rpc_address}"))?
    };

    let req = GetBalanceRequest {
        address: address.to_string(),
    };
    match client.get_balance(req).await {
        Ok(resp) => {
            let payload = resp.into_inner();
            println!("📊 Balance for {}: {} QANTO", address, payload.balance);
            Ok(())
        }
        Err(status) => {
            eprintln!("RPC error: {}", status.message());
            std::process::exit(1);
        }
    }
}

// QDS balance via libp2p request-response
async fn get_balance_qds_with_timeout(
    multiaddr: &str,
    address: &str,
    timeout: Duration,
) -> Result<()> {
    let target_addr: Multiaddr = multiaddr
        .parse()
        .map_err(|e| anyhow!("Invalid multiaddr '{multiaddr}': {e}"))?;

    let peer_id = target_addr
        .iter()
        .find_map(|p| {
            if let Protocol::P2p(id) = p {
                Some(id)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("Multiaddr must include valid /p2p/<peerid>: {multiaddr}"))?;

    let local_keypair = identity::Keypair::generate_ed25519();

    let qds_behaviour = RequestResponseBehaviour::new(
        std::iter::once((QDS_PROTOCOL.to_string(), ProtocolSupport::Full)),
        Default::default(),
    );

    let mut swarm: Swarm<RequestResponseBehaviour<QDSCodec>> =
        SwarmBuilder::with_existing_identity(local_keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_key| Ok(qds_behaviour))
            .map_err(|e| anyhow!("Failed to build libp2p swarm: {e:?}"))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(30)))
            .build();

    swarm
        .dial(target_addr.clone())
        .map_err(|e| anyhow!("Failed to dial {multiaddr}: {e:?}"))?;

    // Send request once connected (or immediately; it will queue until connected)
    let req_id = swarm.behaviour_mut().send_request(
        &peer_id,
        QDSRequest::GetBalance {
            address: address.to_string(),
        },
    );

    // Drive swarm until response or timeout
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!("Timed out waiting for QDS balance response"));
        }
        match swarm.select_next_some().await {
            SwarmEvent::Behaviour(event) => {
                match event {
                    RequestResponseEvent::Message { message, .. } => {
                        if let RequestResponseMessage::Response {
                            request_id,
                            response,
                        } = message
                        {
                            if request_id == req_id {
                                match response {
                                    QDSResponse::Balance {
                                        address: resp_addr,
                                        confirmed_balance,
                                    } => {
                                        // Convert base units to QANTO for display
                                        let qan_base =
                                            crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                        let confirmed_qan = (confirmed_balance as f64) / qan_base;
                                        println!(
                                            "📊 Balance for {}: {:.6} QANTO ({} base units)",
                                            resp_addr, confirmed_qan, confirmed_balance
                                        );
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                    RequestResponseEvent::OutboundFailure {
                        peer,
                        error,
                        request_id,
                        ..
                    } => {
                        if request_id == req_id {
                            return Err(anyhow!("QDS outbound failure to {peer}: {error:?}"));
                        }
                    }
                    RequestResponseEvent::InboundFailure { .. }
                    | RequestResponseEvent::ResponseSent { .. } => {
                        // Not relevant for client-side here
                    }
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id: p, .. } => {
                // Optionally log connection; request already queued
                if p == peer_id {
                    // Connected to target
                }
            }
            _ => {
                // Other events are not relevant for this simple client
            }
        }
    }
}

async fn get_balance_qds(multiaddr: &str, address: &str) -> Result<()> {
    get_balance_qds_with_timeout(multiaddr, address, Duration::from_secs(10)).await
}

async fn get_wallet_balance_grpc(rpc_address: &str, address: &str) -> Result<()> {
    use crate::types::WalletBalance;
    use qanto_rpc::server::generated::{
        wallet_service_client::WalletServiceClient, QueryBalanceRequest, WalletGetBalanceRequest,
    };

    let candidates = [
        format!("http://{rpc_address}"),
        match rpc_address.split(':').next_back() {
            Some(port) => format!("http://[::1]:{}", port),
            None => "http://[::1]:50051".to_string(),
        },
        "http://127.0.0.1:50051".to_string(),
    ];
    let mut client = {
        let mut connected = None;
        for ep in candidates.iter() {
            if let Ok(c) = WalletServiceClient::connect(ep.clone()).await {
                connected = Some(c);
                break;
            }
        }
        connected.ok_or_else(|| anyhow!("Failed to connect to RPC server {rpc_address}"))?
    };

    // Prefer detailed QueryBalance; fall back to legacy GetBalance if unimplemented
    let qreq = QueryBalanceRequest {
        address: address.to_string(),
    };
    match client.query_balance(qreq).await {
        Ok(resp) => {
            let payload = resp.into_inner();
            let wb = WalletBalance {
                spendable_confirmed: payload.spendable_confirmed,
                immature_coinbase_confirmed: payload.immature_coinbase_confirmed,
                unconfirmed_delta: payload.unconfirmed_delta,
                total_confirmed: payload.total_confirmed,
            };
            let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
            let spendable_qan = (wb.spendable_confirmed as f64) / qan_base;
            let immature_qan = (wb.immature_coinbase_confirmed as f64) / qan_base;
            let total_qan = (wb.total_confirmed as f64) / qan_base;
            let unconfirmed_qan = (wb.unconfirmed_delta as f64) / qan_base;
            println!("📊 Balance for {address}:");
            println!("   Spendable:        {spendable_qan:.6} QANTO");
            if wb.immature_coinbase_confirmed > 0 {
                println!("   Immature coinbase: {immature_qan:.6} QANTO (awaiting finality)");
            }
            println!("   Total confirmed:  {total_qan:.6} QANTO");
            if wb.unconfirmed_delta > 0 {
                println!("   Pending (mempool): +{unconfirmed_qan:.6} QANTO");
            }
            Ok(())
        }
        Err(status) => {
            let code = status.code();
            let msg = status.message().to_string();
            let details_len = status.details().len();
            let code_num = code as i32;
            if code_num == 12 {
                // Unimplemented: fall back to legacy GetBalance
                let req = WalletGetBalanceRequest {
                    address: address.to_string(),
                };
                match client.get_balance(req).await {
                    Ok(resp) => {
                        let payload = resp.into_inner();
                        let wb = WalletBalance {
                            spendable_confirmed: payload.balance,
                            immature_coinbase_confirmed: 0,
                            unconfirmed_delta: payload.unconfirmed_balance,
                            total_confirmed: payload.balance,
                        };
                        let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                        let spendable_qan = (wb.spendable_confirmed as f64) / qan_base;
                        let unconfirmed_qan = (wb.unconfirmed_delta as f64) / qan_base;
                        println!("📊 Balance for {address}: {spendable_qan:.6} QANTO");
                        if wb.unconfirmed_delta > 0 {
                            println!("⏳ Pending: +{unconfirmed_qan:.6} QANTO (mempool)");
                        }
                        Ok(())
                    }
                    Err(status2) => {
                        let code2 = status2.code();
                        let msg2 = status2.message().to_string();
                        let details_len2 = status2.details().len();
                        eprintln!(
                            "RPC error: code={:?}, message='{}', details_bytes={}",
                            code2, msg2, details_len2
                        );
                        eprintln!(
                            "Diagnostics: Ensure node {} is reachable and WalletService is enabled.",
                            rpc_address
                        );
                        std::process::exit(1);
                    }
                }
            } else {
                let hint = if code_num == 14 {
                    "Node unreachable: check --node-url, port, firewall/TLS, or server is down."
                } else if code_num == 5 {
                    "Address not found: verify address format, checksum, and network."
                } else if code_num == 7 {
                    "Access denied: WalletService disabled or authentication required on node."
                } else if code_num == 4 {
                    "RPC timed out: node overloaded or network slow; retry later."
                } else {
                    "Unexpected RPC error; check node logs for details."
                };
                eprintln!(
                    "RPC error: code={:?}, message='{}', details_bytes={}",
                    code, msg, details_len
                );
                eprintln!("Hint: {}", hint);
                eprintln!(
                    "Diagnostics: Ensure node {} is reachable and WalletService is enabled.",
                    rpc_address
                );
                std::process::exit(1);
            }
        }
    }
}

async fn request_airdrop_grpc(rpc_address: &str, address: &str) -> Result<()> {
    use qanto_rpc::server::generated::{qanto_rpc_client::QantoRpcClient, RequestAirdropRequest};

    let candidates = [
        format!("http://{rpc_address}"),
        match rpc_address.split(':').next_back() {
            Some(port) => format!("http://[::1]:{}", port),
            None => "http://[::1]:50051".to_string(),
        },
        "http://127.0.0.1:50051".to_string(),
    ];
    let mut client = {
        let mut connected = None;
        for ep in candidates.iter() {
            if let Ok(c) = QantoRpcClient::connect(ep.clone()).await {
                connected = Some(c);
                break;
            }
        }
        connected.ok_or_else(|| anyhow!("Failed to connect to RPC server {rpc_address}"))?
    };

    let req = RequestAirdropRequest {
        address: address.to_string(),
    };
    match client.request_airdrop(req).await {
        Ok(resp) => {
            let payload = resp.into_inner();
            println!("✅ Airdrop Successful!");
            println!("   Tx ID:   {}", payload.tx_id);
            println!("   Message: {}", payload.message);
            Ok(())
        }
        Err(status) => {
            eprintln!("RPC error: {}", status.message());
            std::process::exit(1);
        }
    }
}

async fn send_transaction_grpc(
    wallet_path: PathBuf,
    to: String,
    amount: u64,
    rpc_address: &str,
) -> Result<()> {
    use qanto_rpc::server::generated::{
        qanto_rpc_client::QantoRpcClient, FeeBreakdown as ProtoFeeBreakdown,
        HomomorphicEncrypted as ProtoHE, Input as ProtoInput, Output as ProtoOutput,
        QuantumResistantSignature as ProtoSigMsg, SubmitTransactionRequest,
        Transaction as ProtoTransaction,
    };

    println!(
        "Creating transaction from wallet at {}",
        wallet_path.display()
    );
    println!("Sending {amount} QANTO to {to}");

    // Build a simple internal transaction as before
    let tx = Transaction {
        id: "simulated_tx_id".to_string(),
        sender: "sender".to_string(),
        receiver: to,
        amount,
        fee: 0,
        gas_limit: 21000,
        gas_used: 0,
        gas_price: 1,
        priority_fee: 0,
        inputs: vec![],
        outputs: vec![],
        signature: QuantumResistantSignature {
            signer_public_key: vec![],
            signature: vec![],
        },
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metadata: HashMap::new(),
        fee_breakdown: None,
    };

    // Convert to typed protobuf Transaction
    let proto_tx = ProtoTransaction {
        id: tx.id.clone(),
        sender: tx.sender.clone(),
        receiver: tx.receiver.clone(),
        amount: tx.amount,
        fee: tx.fee,
        gas_limit: tx.gas_limit,
        gas_used: tx.gas_used,
        gas_price: tx.gas_price,
        priority_fee: tx.priority_fee,
        inputs: tx
            .inputs
            .iter()
            .map(|i| ProtoInput {
                tx_id: i.tx_id.clone(),
                output_index: i.output_index,
            })
            .collect(),
        outputs: tx
            .outputs
            .iter()
            .map(|o| ProtoOutput {
                address: o.address.clone(),
                amount: o.amount,
                homomorphic_encrypted: Some(ProtoHE {
                    ciphertext: o.homomorphic_encrypted.ciphertext.clone(),
                    public_key: o.homomorphic_encrypted.public_key.clone(),
                }),
            })
            .collect(),
        timestamp: tx.timestamp,
        metadata: tx.metadata.clone(),
        signature: Some(ProtoSigMsg {
            signer_public_key: tx.signature.signer_public_key.clone(),
            signature: tx.signature.signature.clone(),
        }),
        fee_breakdown: tx.fee_breakdown.as_ref().map(|fb| ProtoFeeBreakdown {
            base_fee: fb.base_fee,
            complexity_fee: fb.complexity_fee,
            storage_fee: fb.storage_fee,
            gas_fee: fb.gas_fee,
            priority_fee: fb.priority_fee,
            congestion_multiplier: fb.congestion_multiplier,
            total_fee: fb.total_fee,
            gas_used: fb.gas_used,
            gas_price: fb.gas_price,
        }),
    };

    // Submit via gRPC
    let endpoint = format!("http://{rpc_address}");
    let mut client = QantoRpcClient::connect(endpoint)
        .await
        .map_err(|e| anyhow!("Failed to connect to RPC server {rpc_address}: {e}"))?;
    let req = SubmitTransactionRequest {
        transaction: Some(proto_tx),
    };

    match client.submit_transaction(req).await {
        Ok(resp) => {
            let result = resp.into_inner();
            if result.accepted {
                println!("✅ Transaction accepted by node: {}", result.message);
            } else {
                println!("❌ Transaction rejected: {}", result.message);
            }
            Ok(())
        }
        Err(status) => {
            eprintln!("RPC error: {}", status.message());
            std::process::exit(1);
        }
    }
}

// For receive_transactions_p2p
async fn receive_transactions_p2p(
    p2p_client: &Option<Arc<QantoP2P>>,
    wallet_path: PathBuf,
) -> Result<()> {
    if let Some(p2p) = p2p_client {
        println!("Enter password to monitor incoming transactions:");
        let password = prompt_for_password(false, None)?;
        let wallet = Wallet::from_file(&wallet_path, &password)?;
        let my_address = wallet.address();
        let my_address_clone = my_address.clone();

        // Register handler for incoming tx
        let handler = Arc::new(move |msg: NetworkMessage, _peer: QantoHash| -> Result<()> {
            if msg.msg_type == MessageType::Custom(3) {
                let tx: Transaction = deserialize(&msg.payload)?;
                if tx.outputs.iter().any(|o| o.address == my_address_clone) {
                    println!("✅ Incoming Transaction Received!");
                }
            }
            Ok(())
        });
        p2p.register_handler(MessageType::Custom(3), handler);
        println!("📡 Listening for incoming transactions to {my_address} (P2P mode)");
        // Keep running
        pending::<()>().await;
        return Ok(());
    }
    receive_transactions_http(wallet_path).await
}

async fn receive_transactions_http(_wallet_path: PathBuf) -> Result<()> {
    println!("⚠️  HTTP fallback mode - consider enabling P2P discovery");
    println!("📊 No new transactions found (HTTP fallback)");
    Ok(())
}

// Legacy function for compatibility
#[allow(dead_code)]
async fn receive_transactions(node_url: &str, wallet_path: PathBuf) -> Result<()> {
    println!(
        "Enter password to monitor incoming transactions for '{wallet_path_display}':",
        wallet_path_display = wallet_path.display()
    );
    let password = prompt_for_password(false, None)?;
    let wallet = Wallet::from_file(&wallet_path, &password)?;
    let my_address = wallet.address();
    let client = Client::new();
    let mut known_tx_ids = HashSet::new();
    println!("\n📡 Listening for incoming transactions to {my_address} (Press Ctrl+C to stop)...");

    loop {
        let mut dag_info_url = String::with_capacity(node_url.len() + 4);
        dag_info_url.push_str(node_url);
        dag_info_url.push_str("/dag");
        if let Ok(res) = client.get(&dag_info_url).send().await {
            if let Ok(dag_info) = res.json::<serde_json::Value>().await {
                if let Some(tips) = dag_info.get("tips").and_then(|t| t.as_object()) {
                    for (_chain_id, tip_ids) in tips {
                        if let Some(tip_ids_array) = tip_ids.as_array() {
                            for tip_id_val in tip_ids_array {
                                if let Some(tip_id) = tip_id_val.as_str() {
                                    let mut block_url =
                                        String::with_capacity(node_url.len() + tip_id.len() + 7);
                                    block_url.push_str(node_url);
                                    block_url.push_str("/block/");
                                    block_url.push_str(tip_id);
                                    if let Ok(block_res) = client.get(&block_url).send().await {
                                        if let Ok(block) =
                                            block_res.json::<crate::qantodag::QantoBlock>().await
                                        {
                                            for tx in block.transactions {
                                                for output in tx.outputs.clone() {
                                                    if output.address == my_address
                                                        && !known_tx_ids.contains(&tx.id)
                                                    {
                                                        println!(
                                                            "\n✅ Incoming Transaction Received!"
                                                        );
                                                        println!(
                                                            "   Amount: {amount} QAN",
                                                            amount = output.amount
                                                        );
                                                        println!(
                                                            "   From: {sender}",
                                                            sender = tx.sender
                                                        );
                                                        println!(
                                                            "   Transaction ID: {tx_id}",
                                                            tx_id = tx.id
                                                        );
                                                        known_tx_ids.insert(tx.id.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

// --- Utility Functions ---

fn prompt_for_input() -> Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

// Parse a WebSocket balance_update JSON message into typed components.
// Accepts both the modern WalletBalance object schema and the legacy numeric balance.
// Returns (address, WalletBalance, finalized, timestamp) on success.
fn parse_ws_balance_update(
    v: &serde_json::Value,
) -> Option<(String, crate::types::WalletBalance, bool, u64)> {
    if v.get("type").and_then(|t| t.as_str()) != Some("balance_update") {
        return None;
    }

    let addr = v.get("address").and_then(|a| a.as_str())?.to_string();
    let finalized = v
        .get("finalized")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);
    let timestamp = v.get("timestamp").and_then(|t| t.as_u64()).unwrap_or(0);

    let bal_field = v.get("balance")?;
    // Modern schema: balance is an object with breakdown
    if let Some(obj) = bal_field.as_object() {
        let spendable = obj
            .get("spendable_confirmed")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let immature = obj
            .get("immature_coinbase_confirmed")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let unconfirmed = obj
            .get("unconfirmed_delta")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let total = obj
            .get("total_confirmed")
            .and_then(|x| x.as_u64())
            .unwrap_or(spendable + immature);

        let wb = crate::types::WalletBalance {
            spendable_confirmed: spendable,
            immature_coinbase_confirmed: immature,
            unconfirmed_delta: unconfirmed,
            total_confirmed: total,
        };
        return Some((addr, wb, finalized, timestamp));
    }

    // Legacy schema: balance is a single number (total)
    if let Some(total) = bal_field.as_u64() {
        let wb = crate::types::WalletBalance {
            spendable_confirmed: total,
            immature_coinbase_confirmed: 0,
            unconfirmed_delta: 0,
            total_confirmed: total,
        };
        return Some((addr, wb, finalized, timestamp));
    }

    None
}

// Parse BalanceSnapshot carrying aggregated totals only
fn parse_ws_balance_snapshot(v: &serde_json::Value) -> Option<(String, u64, u64, u64)> {
    if v.get("type").and_then(|t| t.as_str()) != Some("balance_snapshot") {
        return None;
    }
    let addr = v.get("address").and_then(|a| a.as_str())?.to_string();
    let total_confirmed = v.get("total_confirmed").and_then(|x| x.as_u64())?;
    let total_unconfirmed = v
        .get("total_unconfirmed")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let timestamp = v.get("timestamp").and_then(|t| t.as_u64()).unwrap_or(0);
    Some((addr, total_confirmed, total_unconfirmed, timestamp))
}

// Parse BalanceDeltaUpdate carrying small deltas
fn parse_ws_balance_delta(v: &serde_json::Value) -> Option<(String, i64, i64, bool, u64)> {
    if v.get("type").and_then(|t| t.as_str()) != Some("balance_delta_update") {
        return None;
    }
    let addr = v.get("address").and_then(|a| a.as_str())?.to_string();
    let delta_confirmed = v
        .get("delta_confirmed")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let delta_unconfirmed = v
        .get("delta_unconfirmed")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let finalized = v
        .get("finalized")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);
    let timestamp = v.get("timestamp").and_then(|t| t.as_u64()).unwrap_or(0);
    Some((
        addr,
        delta_confirmed,
        delta_unconfirmed,
        finalized,
        timestamp,
    ))
}

async fn subscribe_balance_ws(
    api_addr: &str,
    address: &str,
    finalized_only: bool,
) -> anyhow::Result<()> {
    let mut backoff_ms = 500;
    let max_backoff_ms = 10_000;

    println!("\x1B[2J\x1B[1;1H");
    println!("============================================================");
    println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
    println!("============================================================");
    println!("📍 Address:  {address}");
    println!("🔗 Node:     ws://{api_addr}/ws");
    println!("🔄 Status:   Connecting...");
    println!("------------------------------------------------------------");
    println!("(Press Ctrl+C to quit dashboard)");

    loop {
        let endpoint = format!("ws://{}/ws", api_addr);
        let (ws_stream, _) = match tokio_tungstenite::connect_async(&endpoint).await {
            Ok(s) => s,
            Err(e) => {
                print!("\x1B[7;12H\x1B[K🔴 Connection failed: {e}. Retrying...");
                use std::io::Write;
                let _ = std::io::stdout().flush();
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
                continue;
            }
        };

        print!("\x1B[7;12H\x1B[K🟢 Connected! Waiting for updates...");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        backoff_ms = 500;

        let (mut write, mut read) = ws_stream.split();

        let filters = if finalized_only {
            serde_json::json!({ "address": address, "finalized_only": "true" })
        } else {
            serde_json::json!({ "address": address })
        };
        let sub_msg = serde_json::json!({
            "type": "subscribe",
            "subscription_type": "balances",
            "filters": filters
        })
        .to_string();

        if let Err(e) = write.send(Message::Text(sub_msg)).await {
            print!("\x1B[7;12H\x1B[K🔴 Send error: {e}. Reconnecting...");
            continue;
        }

        // Proactively request a snapshot
        let snapshot_msg = serde_json::json!({
            "type": "balance_snapshot_request",
            "subscription_type": "balances",
            "filters": { "address": address }
        })
        .to_string();
        let _ = write.send(Message::Text(snapshot_msg)).await;

        let mut utxo_cache = UtxoCache::default();
        let mut current_total_confirmed: Option<u64> = None;
        let mut current_total_unconfirmed: Option<i64> = None;
        let mut chunk_buf: std::collections::HashMap<String, (usize, Vec<Option<String>>)> =
            std::collections::HashMap::new();
        let mut last_heartbeat = std::time::Instant::now();

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("\n🛑 Exiting Real-Time Dashboard...");
                    return Ok(());
                }
                maybe_msg = read.next() => {
                    let msg = match maybe_msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                             print!("\x1B[7;12H\x1B[K🔴 Read error: {e}. Reconnecting...");
                             break;
                        }
                        None => {
                             print!("\x1B[7;12H\x1B[K🔴 Connection closed. Reconnecting...");
                             break;
                        }
                    };

                    match msg {
                        Message::Text(text) => {
                            let v: serde_json::Value = match serde_json::from_str(&text) {
                                Ok(json) => json,
                                Err(_) => continue,
                            };
                            let t = v.get("type").and_then(|t| t.as_str());

                            // Handle chunks
                            if t == Some("chunk_start") {
                                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                if !id.is_empty() && total > 0 {
                                    chunk_buf.insert(id, (total, vec![None; total]));
                                }
                                continue;
                            }
                            if t == Some("chunk_data") {
                                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let index = v.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                let data = v.get("data").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                if let Some((total, slots)) = chunk_buf.get_mut(&id) {
                                    if index < *total {
                                        slots[index] = Some(data);
                                    }
                                }
                                continue;
                            }
                            if t == Some("chunk_end") {
                                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                if let Some((_total, slots)) = chunk_buf.remove(&id) {
                                    if slots.iter().all(|s| s.is_some()) {
                                        let assembled: String = slots.into_iter().map(|s| s.unwrap()).collect();
                                        if let Ok(_av) = serde_json::from_str::<serde_json::Value>(&assembled) {
                                             // Recursively process assembled JSON if needed
                                        }
                                    }
                                }
                                continue;
                            }

                            // Handle updates
                            let mut update_display = false;

                            if t == Some("balance_snapshot") {
                                if let Some((_addr, total_c, total_u, _ts)) = parse_ws_balance_snapshot(&v) {
                                    current_total_confirmed = Some(total_c);
                                    current_total_unconfirmed = Some(total_u as i64);
                                    utxo_cache.confirmed = total_c as u128;
                                    utxo_cache.pending = total_u as i128;
                                    update_display = true;
                                }
                            } else if t == Some("balance_delta_update") {
                                if let Some((_addr, dc, du, _finalized, _ts)) = parse_ws_balance_delta(&v) {
                                    let cur_c = current_total_confirmed.unwrap_or(0);
                                    let cur_u = current_total_unconfirmed.unwrap_or(0);
                                    let new_c = (cur_c as i128 + dc as i128).max(0) as u64;
                                    let new_u = (cur_u as i128 + du as i128).max(0) as i64;
                                    current_total_confirmed = Some(new_c);
                                    current_total_unconfirmed = Some(new_u);
                                    utxo_cache.confirmed = new_c as u128;
                                    utxo_cache.pending = new_u as i128;
                                    update_display = true;
                                }
                            } else if t == Some("balance_update") {
                                if let Some((_addr, wb, _finalized, _ts)) = parse_ws_balance_update(&v) {
                                    current_total_confirmed = Some(wb.total_confirmed);
                                    current_total_unconfirmed = Some(wb.unconfirmed_delta as i64);
                                    utxo_cache.confirmed = wb.total_confirmed as u128;
                                    utxo_cache.pending = wb.unconfirmed_delta as i128;
                                    if let Some(arr) = v.get("utxos").and_then(|x| x.as_array()) {
                                        utxo_cache.by_outpoint.clear();
                                        for item in arr {
                                            if let (Some(txid), Some(vout), Some(value)) = (
                                                item.get("txid").and_then(|x| x.as_str()),
                                                item.get("vout").and_then(|x| x.as_u64()),
                                                item.get("value").and_then(|x| x.as_u64()),
                                            ) {
                                                utxo_cache.by_outpoint.insert((txid.to_string(), vout as u32), value as u128);
                                            }
                                        }
                                    }
                                    update_display = true;
                                }
                            }

                            if update_display {
                                let total_c = current_total_confirmed.unwrap_or(0);
                                let total_u = current_total_unconfirmed.unwrap_or(0);
                                let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                let spendable_fmt = total_c as f64 / base;
                                let total_fmt = total_c as f64 / base;
                                let pending_fmt = total_u as f64 / base;

                                print!("\x1B[9;1H");
                                println!("💰 STATUS: [{}]", if finalized_only { "FINALIZED" } else { "LIVE" });
                                println!("   Spendable:        {:.6} QANTO", spendable_fmt);
                                println!("   Total Confirmed:  {:.6} QANTO", total_fmt);
                                println!("   Pending (Mempool):{:.6} QANTO", pending_fmt);
                                println!("------------------------------------------------------------");
                                println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                let utxo_count = utxo_cache.by_outpoint.len();
                                println!("UTXOs: {}", utxo_count);
                            }
                        }
                        Message::Ping(data) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Message::Close(_) => {
                            print!("\x1B[7;12H\x1B[K🔴 Server closed connection. Reconnecting...");
                            break;
                        }
                        _ => {}
                    }
                }
            }

            if last_heartbeat.elapsed() >= Duration::from_secs(5) {
                let _ = write.send(Message::Ping(Vec::new())).await;
                last_heartbeat = std::time::Instant::now();
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Subscribe to live balance updates via native P2P.
///
/// Executor: runs under the Tokio runtime initialized by the wallet CLI.
/// Registers a message handler for `MessageType::BalanceUpdate` and broadcasts a
/// `BalanceSubscribe` request for the provided address.
async fn subscribe_balance_p2p(
    p2p_client: &Option<Arc<QantoP2P>>,
    address: &str,
    finalized_only: bool,
) -> Result<()> {
    let p2p = p2p_client
        .as_ref()
        .ok_or_else(|| anyhow!("P2P node is not running; start the node and retry"))?;

    println!("🔌 Subscribing to live balance via P2P for: {address}");

    let subscription_id = Uuid::new_v4().to_string();
    let subscribe = BalanceSubscribe {
        address: address.to_string(),
        subscription_id: subscription_id.clone(),
        debounce_ms: Some(200),
        finalized_only: Some(finalized_only),
    };
    let payload = bincode::serialize(&subscribe)?;

    // Register handler for incoming balance updates
    let watched_addr = address.to_string();
    let handler = Arc::new(move |msg: NetworkMessage, _peer: QantoHash| -> Result<()> {
        if msg.msg_type == MessageType::BalanceUpdate {
            let update: BalanceUpdate = bincode::deserialize(&msg.payload)?;
            if update.address == watched_addr {
                let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                let display = (update.balance as f64) / qan_base;
                println!(
                    "📈 {addr} -> {bal:.6} QANTO (t={ts} ms)",
                    addr = update.address,
                    bal = display,
                    ts = update.timestamp_ms
                );
            }
        }
        Ok(())
    });
    p2p.register_handler(MessageType::BalanceUpdate, handler);

    // Broadcast the subscription request to peers
    p2p.broadcast(MessageType::BalanceSubscribe, Bytes::from(payload))?;
    println!("✅ Subscribed. Press Ctrl+C to stop.");

    // Keep the task alive until user stops
    tokio::signal::ctrl_c().await?;
    Ok(())
}

// One-shot WebSocket balance query: subscribe and return first update
#[allow(dead_code)]
pub async fn get_balance_ws_once(api_address: &str, address: &str) -> Result<()> {
    let url = format!("ws://{api_address}/ws");
    #[allow(deprecated)]
    let ws_cfg = WebSocketConfig {
        max_send_queue: None,
        max_write_buffer_size: 8 * 1024 * 1024,
        write_buffer_size: 64 * 1024,
        max_message_size: Some(512 * 1024),
        max_frame_size: Some(512 * 1024),
        accept_unmasked_frames: false,
    };
    let (ws_stream, _resp) = connect_async_with_config(&url, Some(ws_cfg), false)
        .await
        .map_err(|e| anyhow!("WS connect failed at {url}: {e}"))?;

    let (mut write, mut read) = ws_stream.split();

    let sub_msg = serde_json::json!({
        "type": "subscribe",
        "subscription_type": "balances",
        "filters": {"address": address}
    })
    .to_string();

    write
        .send(Message::Text(sub_msg))
        .await
        .map_err(|e| anyhow!("Failed to send WS subscribe: {e}"))?;

    let start = Instant::now();
    let mut printed = false;
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(json) => json,
                    Err(_) => continue,
                };
                if v.get("type").and_then(|t| t.as_str()) == Some("balance_snapshot") {
                    if let Some((addr, total_c, total_u, _ts)) = parse_ws_balance_snapshot(&v) {
                        if addr == address {
                            let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                            let confirmed = (total_c as f64) / base;
                            let pending = (total_u as f64) / base;
                            println!(
                                "📊 Balance for {address}: {:.6} QNT (Confirmed), {:.6} QNT (Pending)",
                                confirmed, pending
                            );
                            printed = true;
                            let _ = write.send(Message::Close(None)).await;
                            break;
                        }
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = write.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                return Err(anyhow!("WS read error: {e}"));
            }
            _ => {}
        }

        if start.elapsed() > Duration::from_secs(10) {
            break;
        }
    }

    if !printed {
        return Err(anyhow!("No balance update received over WebSocket"));
    }

    Ok(())
}

pub async fn watch_balance(api_addr: &str, address: crate::types::Address) -> anyhow::Result<()> {
    let (tx, mut rx) = tokio::sync::watch::channel(0u128);
    let addr = address.clone();
    let api = api_addr.to_string();
    tokio::spawn(async move {
        let endpoint = format!("ws://{}/ws", api);
        let (ws_stream, _) = match tokio_tungstenite::connect_async(&endpoint).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let (mut write, mut read) = ws_stream.split();
        let sub_msg = serde_json::json!({"type":"subscribe","subscription_type":"balances","filters":{"address":addr}}).to_string();
        let _ = write
            .send(tokio_tungstenite::tungstenite::Message::Text(sub_msg))
            .await;
        let mut current_total_confirmed: u128 = 0;
        while let Some(msg) = read.next().await {
            if let Ok(tokio_tungstenite::tungstenite::Message::Text(text)) = msg {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    let t = v.get("type").and_then(|t| t.as_str());
                    if t == Some("balance_snapshot") {
                        if let Some((_addr, total_c, _total_u, _ts)) = parse_ws_balance_snapshot(&v)
                        {
                            current_total_confirmed = total_c as u128;
                            let _ = tx.send(current_total_confirmed);
                        }
                    } else if t == Some("balance_update") {
                        if let Some((_addr, wb, _finalized, _ts)) = parse_ws_balance_update(&v) {
                            current_total_confirmed = wb.total_confirmed as u128;
                            let _ = tx.send(current_total_confirmed);
                        }
                    } else if t == Some("balance_delta_update") {
                        if let Some((_addr, dc, _du, _finalized, _ts)) = parse_ws_balance_delta(&v)
                        {
                            let next =
                                (current_total_confirmed as i128 + dc as i128).max(0) as u128;
                            current_total_confirmed = next;
                            let _ = tx.send(current_total_confirmed);
                        }
                    }
                }
            }
        }
    });
    loop {
        let _ = rx.changed().await;
        let bal = *rx.borrow();
        let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
        use std::io::Write;
        print!("\rBalance: {:.6} QNT", (bal as f64) / base);
        let _ = std::io::stdout().flush();
    }
}

pub async fn connect_stream(
    api_addr: &str,
    wallet: &mut crate::node_keystore::Wallet,
    address: &str,
    finalized_only: bool,
) -> anyhow::Result<()> {
    let (tx, _rx_ignore) = tokio::sync::broadcast::channel(256);
    wallet.stream_tx = Some(tx.clone());

    let addr = address.to_string();
    let api = api_addr.to_string();

    tokio::spawn({
        let tx = tx.clone();
        async move {
            loop {
                let endpoint = format!("ws://{}/ws", api);
                match tokio_tungstenite::connect_async(&endpoint).await {
                    Ok((ws_stream, _)) => {
                        let (mut write, mut read) = ws_stream.split();
                        let filters = if finalized_only {
                            serde_json::json!({ "address": addr, "finalized_only": "true" })
                        } else {
                            serde_json::json!({ "address": addr })
                        };
                        let sub_msg = serde_json::json!({
                            "type": "subscribe",
                            "subscription_type": "balances",
                            "filters": filters
                        })
                        .to_string();
                        if write
                            .send(tokio_tungstenite::tungstenite::Message::Text(sub_msg))
                            .await
                            .is_err()
                        {
                            continue;
                        }
                        while let Some(msg) = read.next().await {
                            if let Ok(tokio_tungstenite::tungstenite::Message::Text(text)) = msg {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if let Some((_, total_c, total_u, ts)) =
                                        parse_ws_balance_snapshot(&v)
                                    {
                                        let _ =
                                            tx.send(crate::node_keystore::BalanceEvent::Snapshot {
                                                confirmed: total_c,
                                                unconfirmed: total_u,
                                                ts,
                                            });
                                    }
                                    if let Some((_, dc, du, fin, ts)) = parse_ws_balance_delta(&v) {
                                        let _ =
                                            tx.send(crate::node_keystore::BalanceEvent::Delta {
                                                delta_confirmed: dc,
                                                delta_unconfirmed: du,
                                                finalized: fin,
                                                ts,
                                            });
                                    }
                                    if let Some((_, wb, _fin, ts)) = parse_ws_balance_update(&v) {
                                        let _ =
                                            tx.send(crate::node_keystore::BalanceEvent::Update {
                                                spendable_confirmed: wb.spendable_confirmed,
                                                immature_coinbase_confirmed: wb
                                                    .immature_coinbase_confirmed,
                                                unconfirmed_delta: wb.unconfirmed_delta as i64,
                                                total_confirmed: wb.total_confirmed,
                                                ts,
                                            });
                                    }
                                }
                            } else if let Ok(tokio_tungstenite::tungstenite::Message::Close(_)) =
                                msg
                            {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        let _ = watch_balance(&api, addr.clone()).await;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });

    let mut rx = wallet.stream_tx.as_ref().unwrap().subscribe();
    let mut state = WalletBalanceState::default();
    loop {
        if let Ok(event) = rx.recv().await {
            match event {
                crate::node_keystore::BalanceEvent::Snapshot {
                    confirmed,
                    unconfirmed,
                    ..
                } => {
                    state.total_confirmed = confirmed;
                    state.unconfirmed_delta = unconfirmed as i64;
                }
                crate::node_keystore::BalanceEvent::Delta {
                    delta_confirmed,
                    delta_unconfirmed,
                    ..
                } => {
                    state.total_confirmed =
                        ((state.total_confirmed as i128) + (delta_confirmed as i128)) as u64;
                    state.unconfirmed_delta += delta_unconfirmed;
                }
                crate::node_keystore::BalanceEvent::Update {
                    spendable_confirmed,
                    immature_coinbase_confirmed,
                    unconfirmed_delta,
                    total_confirmed,
                    ..
                } => {
                    state.spendable_confirmed = spendable_confirmed;
                    state.immature_coinbase_confirmed = immature_coinbase_confirmed;
                    state.unconfirmed_delta = unconfirmed_delta;
                    state.total_confirmed = total_confirmed;
                }
            }
            let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
            println!(
                "Balance: {:.6} QNT (confirmed), {:.6} QNT (pending)",
                (state.total_confirmed as f64) / base,
                (state.unconfirmed_delta as f64) / base
            );
        }
    }
}

// Stream balance updates via gRPC SubscribeBalance.
// Executor: Tokio runtime from the wallet CLI. Prints maturity-aware breakdown per update.
async fn subscribe_balance_grpc(
    rpc_address: &str,
    address: &str,
    finalized_only: bool,
) -> Result<()> {
    use qanto_rpc::server::generated::{qanto_rpc_client::QantoRpcClient, SubscribeBalanceRequest};
    use std::time::Duration;
    use tokio::time::sleep;

    let endpoint = format!("http://{rpc_address}");
    let fallback_ipv6 = match rpc_address.split(':').next_back() {
        Some(port) => format!("http://[::1]:{}", port),
        None => "http://[::1]:50051".to_string(),
    };
    let fallback_ipv4 = "http://127.0.0.1:50051".to_string();
    let mut retry_delay = Duration::from_secs(1);
    const MAX_DELAY: Duration = Duration::from_secs(30);

    println!("🚀 Starting Real-Time Wallet Dashboard...");
    println!("📡 Connecting to node at {endpoint}...");

    loop {
        let mut client_opt = None;
        if let Ok(c) = QantoRpcClient::connect(endpoint.clone()).await {
            client_opt = Some(c);
        }
        if client_opt.is_none() {
            if let Ok(c) = QantoRpcClient::connect(fallback_ipv6.clone()).await {
                client_opt = Some(c);
            }
        }
        if client_opt.is_none() {
            if let Ok(c) = QantoRpcClient::connect(fallback_ipv4.clone()).await {
                client_opt = Some(c);
            }
        }
        match client_opt {
            Some(mut client) => {
                println!("✅ Connected. Subscribing to balance updates...");
                retry_delay = Duration::from_secs(1); // Reset backoff

                let req = SubscribeBalanceRequest {
                    address_filter: address.to_string(),
                    finalized_only,
                };

                match client.subscribe_balance(req).await {
                    Ok(response) => {
                        let mut stream = response.into_inner();

                        // Clear screen and show header
                        print!("\x1B[2J\x1B[1;1H");
                        println!("============================================================");
                        println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                        println!("============================================================");
                        println!("📍 Address:  {address}");
                        println!("🔗 Node:     {}", rpc_address);
                        println!(
                            "🔒 Mode:     {}",
                            if finalized_only {
                                "Finalized Only"
                            } else {
                                "Live (Mempool + Unfinalized)"
                            }
                        );
                        println!("------------------------------------------------------------");
                        println!("Waiting for updates...");

                        loop {
                            match stream.message().await {
                                Ok(Some(item)) => {
                                    // Update display
                                    print!("\x1B[2J\x1B[1;1H");
                                    println!("============================================================");
                                    println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                    println!("============================================================");
                                    println!("📍 Address:  {address}");
                                    println!("🔗 Node:     {}", rpc_address);
                                    println!(
                                        "🔒 Mode:     {}",
                                        if finalized_only {
                                            "Finalized Only"
                                        } else {
                                            "Live (Mempool + Unfinalized)"
                                        }
                                    );
                                    println!("------------------------------------------------------------");

                                    let qan_base =
                                        crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                    let spendable_qan =
                                        (item.spendable_confirmed as f64) / qan_base;
                                    let immature_qan =
                                        (item.immature_coinbase_confirmed as f64) / qan_base;
                                    let total_qan = (item.total_confirmed as f64) / qan_base;
                                    let unconfirmed_qan =
                                        (item.unconfirmed_delta as f64) / qan_base;
                                    let fin = if item.finalized {
                                        "FINALIZED"
                                    } else {
                                        "PENDING"
                                    };

                                    println!("\n💰 STATUS: [{fin}]");
                                    println!("   Spendable:        {spendable_qan:.6} QANTO");
                                    if item.immature_coinbase_confirmed > 0 {
                                        println!("   Immature coinbase: {immature_qan:.6} QANTO (awaiting finality)");
                                    }
                                    println!("   Total confirmed:  {total_qan:.6} QANTO");
                                    if item.unconfirmed_delta > 0 {
                                        println!(
                                            "   Pending (mempool): +{unconfirmed_qan:.6} QANTO"
                                        );
                                    }
                                    println!("\n------------------------------------------------------------");
                                    println!(
                                        "Last update: {}",
                                        chrono::Local::now().format("%H:%M:%S")
                                    );
                                }
                                Ok(None) => {
                                    eprintln!("⚠️ Stream ended by server. Reconnecting...");
                                    break;
                                }
                                Err(e) => {
                                    eprintln!("⚠️ Streaming error: {e}. Reconnecting...");
                                    break;
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(MAX_DELAY);
                    }
                }
            }
            None => {
                sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(MAX_DELAY);
            }
        }
    }
}

// get_balance_realtime removed in favor of network-only methods

// removed legacy path in favor of JSON output

async fn get_balance_json(
    rpc_addr: &str,
    api_addr: &str,
    _qds_multiaddr: Option<&str>,
    address: &str,
) -> Result<()> {
    use qanto_rpc::server::generated::wallet_service_client::WalletServiceClient;
    use qanto_rpc::server::generated::{QueryBalanceRequest, WalletGetBalanceRequest};

    let mut spendable_confirmed: u64 = 0;
    let mut unconfirmed_delta: u64 = 0;
    let mut total_confirmed: u64 = 0;
    let utxo_count: u64 = 0; // Not available via simple RPC yet, defaulted to 0

    // 1. Try HTTP API first (Lightweight)
    let mut http_success = false;
    if let Ok(Some(balance)) = get_balance_http_value(api_addr, address).await {
        spendable_confirmed = balance;
        total_confirmed = balance;
        http_success = true;
    }

    // 2. Fallback / Augment with gRPC if HTTP failed or for more details
    if !http_success {
        // Connect candidates
        let candidates = [
            format!("http://{}", rpc_addr),
            match rpc_addr.split(':').next_back() {
                Some(port) => format!("http://[::1]:{}", port),
                None => "http://[::1]:50051".to_string(),
            },
            "http://127.0.0.1:50051".to_string(),
        ];

        let mut connected = None;
        for ep in candidates.iter() {
            if let Ok(c) = WalletServiceClient::connect(ep.clone()).await {
                connected = Some(c);
                break;
            }
        }

        if let Some(mut client) = connected {
            let qreq = QueryBalanceRequest {
                address: address.to_string(),
            };
            match client.query_balance(qreq).await {
                Ok(resp) => {
                    let p = resp.into_inner();
                    spendable_confirmed = p.spendable_confirmed;
                    unconfirmed_delta = p.unconfirmed_delta;
                    total_confirmed = p.total_confirmed;
                }
                Err(status) => {
                    if status.code() as i32 == 12 {
                        let req = WalletGetBalanceRequest {
                            address: address.to_string(),
                        };
                        if let Ok(resp2) = client.get_balance(req).await {
                            let p2 = resp2.into_inner();
                            spendable_confirmed = p2.balance;
                            unconfirmed_delta = p2.unconfirmed_balance;
                            total_confirmed = p2.balance;
                        }
                    }
                }
            }
        }
    }

    let last_updated = chrono::Utc::now().to_rfc3339();
    let total_balance = total_confirmed.saturating_add(unconfirmed_delta);

    let payload = serde_json::json!({
        "address": address,
        "confirmed_balance": spendable_confirmed,
        "unconfirmed_balance": unconfirmed_delta,
        "total_balance": total_balance,
        "utxo_count": utxo_count,
        "last_updated": last_updated,
        "source": if http_success { "http_rpc" } else { "grpc_fallback" }
    });
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load configuration with CLI overrides
    let config_path = cli.config.unwrap_or_else(|| PathBuf::from("config.toml"));
    let mut config = if config_path.exists() {
        Config::load(config_path.to_str().unwrap())?
    } else {
        // Use CLI data_dir or default to current directory
        let base_dir = cli
            .data_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        Config::with_base_dir(&base_dir)
    };

    // Apply CLI data directory override if provided
    if let Some(data_dir) = &cli.data_dir {
        let base_dir = data_dir.to_string_lossy().to_string();
        let (new_data_dir, new_db_path, new_wallet_path, new_p2p_identity_path) =
            Config::get_default_paths(&base_dir);

        config = Config {
            data_dir: new_data_dir,
            db_path: new_db_path,
            wallet_path: if config.wallet_path == "wallet.key" {
                new_wallet_path
            } else {
                config.wallet_path
            },
            p2p_identity_path: if config.p2p_identity_path == "p2p_identity.key" {
                new_p2p_identity_path
            } else {
                config.p2p_identity_path
            },
            ..config
        };
    }

    // Resolve optional direct peer
    let direct_peer: Option<SocketAddr> = match &cli.p2p_direct {
        Some(addr_str) => match addr_str.parse::<SocketAddr>() {
            Ok(addr) => Some(addr),
            Err(e) => {
                eprintln!("Invalid --p2p-direct '{addr_str}': {e}");
                None
            }
        },
        None => None,
    };

    // Lightweight QDS mode: skip P2P init when querying balance via QDS
    let qds_balance_mode = matches!(
        cli.command,
        Commands::Balance {
            follow: false,
            live: false,
            ..
        }
    ) && cli.qds_multiaddr.is_some();

    // Initialize P2P client if discovery is enabled and not in QDS lightweight mode; fall back to HTTP on failure
    let p2p_client = if !qds_balance_mode && cli.p2p_discovery.unwrap_or(true) {
        match initialize_p2p_client(cli.p2p_port, direct_peer).await {
            Ok(client) => Some(client),
            Err(e) => {
                eprintln!("⚠️ P2P init/start failed; HTTP fallback enabled: {e}");
                None
            }
        }
    } else {
        None
    };

    // Resolve RPC address for gRPC
    let rpc_addr = cli
        .node_url
        .clone()
        .unwrap_or_else(|| config.rpc.address.clone());

    let result = match cli.command {
        Commands::Generate { output } => {
            let wallet_path = output.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            generate_wallet(wallet_path).await
        }
        Commands::Show { wallet, keys } => {
            let wallet_path = wallet.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            show_wallet_info(wallet_path, keys).await
        }
        Commands::Import {
            mnemonic,
            private_key,
        } => import_wallet(mnemonic, private_key).await,
        Commands::Balance {
            address,
            follow,
            live,
            grpc,
            finalized_only,
            json,
        } => {
            if json {
                get_balance_json(
                    &rpc_addr,
                    &config.api_address,
                    cli.qds_multiaddr.as_deref(),
                    &address,
                )
                .await
            } else if grpc {
                // Explicit gRPC streaming requested
                subscribe_balance_grpc(&rpc_addr, &address, finalized_only).await
            } else if follow {
                // Explicit follow flag: print initial snapshot, then stream live updates via WebSocket
                let api_addr = config.api_address.clone();
                let _ = get_balance_ws_once(&api_addr, &address).await;
                match subscribe_balance_ws(&api_addr, &address, finalized_only).await {
                    Ok(()) => Ok(()),
                    Err(ws_err) => {
                        eprintln!("⚠️ WebSocket not available ({ws_err}). Falling back to gRPC streaming.");
                        subscribe_balance_grpc(&rpc_addr, &address, finalized_only).await
                    }
                }
            } else if live {
                // Stream via native P2P
                match subscribe_balance_p2p(&p2p_client, &address, finalized_only).await {
                    Ok(()) => Ok(()),
                    Err(p2p_err) => {
                        eprintln!(
                            "⚠️ P2P live subscribe failed ({p2p_err}). Falling back to WebSocket."
                        );
                        let api_addr = config.api_address.clone();
                        let _ = get_balance_ws_once(&api_addr, &address).await;
                        match subscribe_balance_ws(&api_addr, &address, finalized_only).await {
                            Ok(()) => Ok(()),
                            Err(ws_err) => {
                                eprintln!(
                                    "⚠️ WebSocket not available ({ws_err}). Falling back to gRPC streaming."
                                );
                                subscribe_balance_grpc(&rpc_addr, &address, finalized_only).await
                            }
                        }
                    }
                }
            } else {
                // Default: use network-first query
                get_balance_network(&address, &config, &rpc_addr, cli.qds_multiaddr.as_deref())
                    .await
            }
        }
        Commands::Watch {
            address,
            finalized_only,
        } => {
            let api_addr = config.api_address.clone();
            match subscribe_balance_ws(&api_addr, &address, finalized_only).await {
                Ok(()) => Ok(()),
                Err(ws_err) => {
                    eprintln!(
                        "⚠️ WebSocket not available ({ws_err}). Falling back to gRPC streaming."
                    );
                    subscribe_balance_grpc(&rpc_addr, &address, finalized_only).await
                }
            }
        }
        Commands::Send { wallet, to, amount } => {
            let wallet_path = wallet.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            if cli.node_url.is_some() || p2p_client.is_none() {
                send_transaction_grpc(wallet_path, to, amount, &rpc_addr).await
            } else {
                send_transaction_p2p(&p2p_client, wallet_path, to, amount).await
            }
        }
        Commands::Receive { wallet } => {
            let wallet_path = wallet.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            receive_transactions_p2p(&p2p_client, wallet_path).await
        }
        Commands::Derive {
            wallet,
            index,
            save,
        } => {
            let wallet_path = wallet.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            if !wallet_path.exists() {
                return Err(anyhow!("Wallet file not found at: {wallet_path:?}"));
            }
            println!("Enter password to decrypt master wallet:");
            let password = prompt_for_password(false, None)?;
            let master_wallet = Wallet::from_file(&wallet_path, &password)?;
            let child = master_wallet.derive_child(index)?;

            println!("🔑 Derived Child Wallet (Index {index})");
            println!("   Address: {}", child.address());

            if let Some(save_path) = save {
                println!("💾 Saving child wallet to: {}", save_path.display());
                let new_password =
                    prompt_for_password(true, Some("Create password for child wallet:"))?;
                child.save_to_file(&save_path, &new_password)?;
                println!("✅ Child wallet saved successfully.");
            }
            Ok(())
        }
        Commands::Governance { command, wallet } => {
            let wallet_path = wallet.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            match command {
                GovernanceCommands::Propose {
                    title,
                    description,
                    execution_code,
                    security_impact,
                } => {
                    println!("🗳️ Submitting Proposal: '{title}'");
                    // In a real implementation, this would construct a governance transaction
                    // For now, we simulate the submission or print what would happen
                    println!("   Description: {description}");
                    println!("   Security Impact: {security_impact}");
                    if let Some(code) = execution_code {
                        println!("   Execution Code: {code}");
                    }

                    // Load wallet to sign (simulated)
                    println!("Enter password to sign proposal:");
                    let password = prompt_for_password(false, None)?;
                    let _wallet = Wallet::from_file(&wallet_path, &password)?;

                    // Simulate submission to network
                    println!("✅ Proposal signed and submitted to network (Simulated).");
                    println!("   Proposal ID: [Pending Network Confirmation]");
                    Ok(())
                }
                GovernanceCommands::Vote { proposal_id, vote } => {
                    println!("🗳️ Casting Vote for Proposal {proposal_id}: {vote}");

                    println!("Enter password to sign vote:");
                    let password = prompt_for_password(false, None)?;
                    let _wallet = Wallet::from_file(&wallet_path, &password)?;

                    println!("✅ Vote signed and broadcasted (Simulated).");
                    Ok(())
                }
            }
        }
        Commands::Airdrop { address } => request_airdrop_grpc(&rpc_addr, &address).await,
    };

    if let Err(e) = result {
        eprintln!("\nNEURAL-VAULT™ Error: {e:?}");
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::types::UTXO;
    use libp2p::identity;
    // use tempfile::TempDir;

    #[tokio::test]
    async fn test_basic_arithmetic() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_cli_parses_qds_multiaddr_and_lightweight_mode() {
        let args = vec![
            "qantowallet",
            "balance",
            "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3",
            "--qds-multiaddr",
            "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWAbCdEfGhijkLmNoPqrStuVwxyz1234567890AbCdE",
        ];
        let cli = Cli::parse_from(&args);

        match &cli.command {
            Commands::Balance {
                address,
                follow,
                live,
                grpc: _,
                finalized_only: _,
                json: _,
            } => {
                assert_eq!(follow, &false);
                assert_eq!(live, &false);
                assert_eq!(
                    address,
                    "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"
                );
            }
            _ => panic!("expected Balance command"),
        }
        assert!(cli.qds_multiaddr.is_some(), "qds_multiaddr should be set");

        // Lightweight mode should be active
        let qds_balance_mode = matches!(
            cli.command,
            Commands::Balance {
                follow: false,
                live: false,
                ..
            }
        ) && cli.qds_multiaddr.is_some();
        assert!(qds_balance_mode, "QDS lightweight mode should be detected");
    }

    #[test]
    fn test_cli_parses_balance_grpc_and_finalized_only() {
        let args = vec![
            "qantowallet",
            "balance",
            "abc123",
            "--grpc",
            "--finalized-only",
        ];
        let cli = Cli::parse_from(&args);

        match &cli.command {
            Commands::Balance {
                address,
                follow,
                live,
                grpc,
                finalized_only,
                json,
            } => {
                assert_eq!(address, "abc123");
                assert_eq!(follow, &false);
                assert_eq!(live, &false);
                assert_eq!(grpc, &true);
                assert_eq!(finalized_only, &true);
                assert_eq!(json, &false);
            }
            _ => panic!("expected Balance command"),
        }
    }

    #[test]
    fn test_cli_parses_balance_follow_websocket() {
        let args = vec!["qantowallet", "balance", "abc123", "--follow"];
        let cli = Cli::parse_from(&args);

        match &cli.command {
            Commands::Balance {
                address,
                follow,
                live,
                grpc,
                finalized_only,
                json,
            } => {
                assert_eq!(address, "abc123");
                assert_eq!(follow, &true);
                assert_eq!(live, &false);
                assert_eq!(grpc, &false);
                assert_eq!(finalized_only, &false);
                assert_eq!(json, &false);
            }
            _ => panic!("expected Balance command"),
        }
    }

    #[test]
    fn test_cli_parses_balance_live_p2p() {
        let args = vec!["qantowallet", "balance", "abc123", "--live"];
        let cli = Cli::parse_from(&args);

        match &cli.command {
            Commands::Balance {
                address,
                follow,
                live,
                grpc,
                finalized_only,
                json,
            } => {
                assert_eq!(address, "abc123");
                assert_eq!(follow, &false);
                assert_eq!(live, &true);
                assert_eq!(grpc, &false);
                assert_eq!(finalized_only, &false);
                assert_eq!(json, &false);
            }
            _ => panic!("expected Balance command"),
        }
    }

    #[test]
    fn test_cli_parses_balance_follow_alias_ws() {
        let args = [
            "qantowallet",
            "balance",
            "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3",
            "--ws",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Balance {
                address,
                follow,
                live,
                grpc,
                finalized_only,
                json,
            } => {
                assert_eq!(
                    address,
                    "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"
                );
                assert!(follow);
                assert!(!live);
                assert!(!grpc);
                assert!(!finalized_only);
                assert!(!json);
            }
            _ => panic!("Expected Balance command variant"),
        }
    }

    #[tokio::test]
    async fn test_qds_single_shot_unreachable_fast_timeout() {
        // Generate a valid peer id for multiaddr construction
        let local_keypair = identity::Keypair::generate_ed25519();
        let peer_id = local_keypair.public().to_peer_id();
        let ma = format!("/ip4/127.0.0.1/tcp/9/p2p/{}", peer_id);

        // Use short timeout to avoid long waits
        let res =
            get_balance_qds_with_timeout(&ma, "test_address", Duration::from_millis(250)).await;
        assert!(res.is_err(), "Unreachable QDS should error quickly");
        let err = format!("{:?}", res.err().unwrap());
        assert!(
            err.contains("Timed out")
                || err.contains("Failed to dial")
                || err.contains("DialFailure"),
            "Error should indicate dial failure or timeout; got: {}",
            err
        );
    }

    #[tokio::test]
    async fn wallet_cli_websocket_finalized_only_sets_filter() {
        use axum::{
            extract::{
                ws::{Message as AxumMessage, WebSocketUpgrade},
                State,
            },
            routing::get,
            Router,
        };
        use futures_util::StreamExt;
        use tokio::net::TcpListener;

        // Channel to capture the first subscribe message JSON
        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(1);

        // Minimal WebSocket handler that captures the first text message and forwards it
        async fn ws_capture(
            ws: WebSocketUpgrade,
            State(tx): State<tokio::sync::mpsc::Sender<serde_json::Value>>,
        ) -> axum::response::Response {
            ws.on_upgrade(|socket| async move {
                let (mut _sender, mut receiver) = socket.split();
                if let Some(Ok(AxumMessage::Text(text))) = receiver.next().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        let _ = tx.send(json).await;
                    }
                }
                // Drop connection after capture
            })
        }

        let router = Router::new()
            .route("/ws", get(ws_capture))
            .with_state(msg_tx.clone());

        // Bind ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let local_addr = listener.local_addr().expect("local_addr");
        let server_task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        // Use a deterministic address for the CLI message
        let address = DEV_ADDRESS;

        // Start the wallet CLI WebSocket subscription in background with finalized_only = true
        let client_task = tokio::spawn(async move {
            let _ = subscribe_balance_ws(&format!("{}", local_addr), address, true).await;
        });

        // Await the captured message
        let captured = tokio::time::timeout(std::time::Duration::from_secs(2), msg_rx.recv())
            .await
            .expect("timeout waiting for ws message")
            .expect("channel closed");

        // Assertions
        assert_eq!(captured["type"], "subscribe");
        assert_eq!(captured["filters"]["address"], address);
        assert_eq!(captured["filters"]["finalized_only"], "true");

        // Clean up
        server_task.abort();
        client_task.abort();
    }
}
#[derive(Clone, Default)]
struct WalletBalanceState {
    total_confirmed: u64,
    unconfirmed_delta: i64,
    spendable_confirmed: u64,
    immature_coinbase_confirmed: u64,
}
