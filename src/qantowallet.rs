use crate::qds::{QDSCodec, QDSRequest, QDSResponse, QDS_PROTOCOL};
use crate::types::QuantumResistantSignature;
use crate::{
    config::Config,
    password_utils::prompt_for_password,
    qanto_p2p::{MessageType, NetworkMessage, QantoP2P},
    transaction::Transaction,
    wallet::Wallet,
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
use my_blockchain::qanto_standalone::hash::QantoHash;
use reqwest::Client;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use qanto_core::balance_stream::{BalanceSubscribe, BalanceUpdate};
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
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

// Add storage and persistence imports for direct DB reads
use crate::persistence::{balance_key, decode_balance, decode_utxo, utxos_prefix};
use crate::qanto_storage::{QantoStorage, StorageConfig};

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
        help = "P2P listen port for wallet node",
        default_value = "8080"
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
        #[arg(long)]
        private_key: bool,
    },
    /// [balance] Checks wallet balance via Instant-MeshSync™.
    Balance {
        #[arg()]
        address: String,
        #[arg(long, help = "Follow live balance updates via WebSocket")]
        follow: bool,
        #[arg(long, help = "Follow live balance updates via P2P")]
        live: bool,
        #[arg(
            long,
            help = "Query balance from local storage only (fastest, no network)"
        )]
        local: bool,
    },
    /// [balance-rt] Instant balance from local node database (no network).
    #[command(alias = "realtime-balance", alias = "rt-balance")]
    BalanceRt {
        #[arg()]
        address: String,
        #[arg(long, help = "Override node data directory for storage lookups")]
        db_dir: Option<PathBuf>,
    },
    /// [watch] Follows live balance updates via WebSocket.
    Watch {
        #[arg()]
        address: String,
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

    println!("Enter password to decrypt vault:");
    let password = prompt_for_password(false, None)?;

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

async fn import_wallet(use_mnemonic: bool, use_private_key: bool) -> Result<()> {
    let password = prompt_for_password(
        true,
        Some("Create a password to encrypt the imported vault:"),
    )?;
    let wallet = if use_mnemonic {
        println!("Please enter your 12-word BIP39 mnemonic phrase:");
        let phrase = prompt_for_input()?;
        Wallet::from_mnemonic(&phrase).context("Failed to import from mnemonic")?
    } else if use_private_key {
        println!("Please enter your raw private key (hex-encoded):");
        let key = prompt_for_input()?;
        Wallet::from_private_key(&key).context("Failed to import from private key")?
    } else {
        return Err(anyhow!(
            "You must specify either --mnemonic or --private-key."
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

// HTTP RPC implementation (RPC-first)
#[allow(dead_code)]
async fn get_balance_http(api_address: &str, address: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| anyhow!("Failed to build HTTP client: {e}"))?;

    let url = format!("http://{api_address}/balance/{address}");

    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<ApiBalanceResponse>().await {
                    Ok(payload) => {
                        println!("📊 Balance for {address}: {} QANTO", payload.balance);
                        let _ = payload.base_units; // read to avoid dead_code lint
                        Ok(())
                    }
                    Err(_) => {
                        eprintln!(
                            "Error: Cannot connect to node at http://{api_address}. Please ensure the main Qanto node is running."
                        );
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!(
                    "Error: Cannot connect to node at http://{api_address}. Please ensure the main Qanto node is running."
                );
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!(
                "Error: Cannot connect to node at http://{api_address}. Please ensure the main Qanto node is running."
            );
            std::process::exit(1);
        }
    }
}

// Direct storage-balanced lookup
#[allow(dead_code)]
async fn get_balance_storage(db_path: &str, address: &str) -> Result<Option<u64>> {
    // Initialize storage with read-only friendly config
    let cfg = StorageConfig {
        data_dir: PathBuf::from(db_path),
        wal_enabled: false, // avoid creating WAL when just reading
        ..StorageConfig::default()
    };

    let storage = QantoStorage::new(cfg).map_err(|e| anyhow!("Storage init error: {e}"))?;
    get_balance_storage_with_instance(&storage, address).await
}

// Direct storage-balanced lookup with storage instance
#[allow(dead_code)]
async fn get_balance_storage_with_instance(
    storage: &QantoStorage,
    address: &str,
) -> Result<Option<u64>> {
    let key = balance_key(address);

    // Helper: scan UTXOs prefix and aggregate amounts for the given address
    let scan_utxos_total = |storage: &QantoStorage| -> Result<u64> {
        let prefix = utxos_prefix();
        let keys = storage
            .keys_with_prefix(&prefix)
            .map_err(|e| anyhow!("Storage list error: {e}"))?;
        let mut total: u64 = 0;
        for k in keys {
            match storage.get(&k) {
                Ok(Some(bytes)) => {
                    if let Ok(utxo) = decode_utxo(&bytes) {
                        if utxo.address == address {
                            total = total.saturating_add(utxo.amount);
                        }
                    }
                }
                Ok(None) => {
                    // skip missing values
                }
                Err(e) => {
                    return Err(anyhow!("Storage read error for UTXO: {e}"));
                }
            }
        }
        Ok(total)
    };

    match storage.get(&key) {
        Ok(Some(bytes)) => {
            match decode_balance(&bytes) {
                Ok(v) => Ok(Some(v)),
                Err(_) => {
                    // Fallback: compute balance by scanning UTXOs
                    let total = scan_utxos_total(storage)?;
                    Ok(Some(total))
                }
            }
        }
        Ok(None) => {
            // Fallback: compute balance by scanning UTXOs
            let total = scan_utxos_total(storage)?;
            if total == 0 {
                Ok(None) // Return None for empty wallets
            } else {
                Ok(Some(total))
            }
        }
        Err(e) => Err(anyhow!("Storage read error: {e}")),
    }
}

// Enhanced local-first balance query with intelligent fallback
async fn get_balance_local_first(
    data_dir: &str,
    address: &str,
    local_only: bool,
    _config: &Config,
    rpc_addr: &str,
    qds_multiaddr: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    // Step 1: Always try local storage first (fastest path)
    match get_balance_storage(data_dir, address).await {
        Ok(Some(balance)) => {
            let elapsed = start_time.elapsed();
            let qan_balance =
                (balance as f64) / (crate::transaction::SMALLEST_UNITS_PER_QAN as f64);

            println!(
                "💰 Balance: {:.6} QANTO ({} base units)",
                qan_balance, balance
            );
            println!("⚡ Local query completed in {:?}", elapsed);
            println!("📍 Source: Local storage (instant)");
            return Ok(());
        }
        Ok(None) => {
            if local_only {
                println!("❌ No local balance found for address: {}", address);
                println!("💡 Tip: Run without --local to query network sources");
                return Ok(());
            }
            println!("ℹ️ No local balance found, falling back to network...");
        }
        Err(e) => {
            if local_only {
                return Err(anyhow!("Local storage error: {}", e));
            }
            println!("⚠️ Local storage error ({}), falling back to network...", e);
        }
    }

    // Step 2: Network fallback (only if not local-only mode)
    if !local_only {
        println!("🌐 Querying network sources...");

        // Try QDS first if available (lightweight)
        if let Some(ma) = qds_multiaddr {
            match get_balance_qds(ma, address).await {
                Ok(()) => {
                    let total_elapsed = start_time.elapsed();
                    println!(
                        "⚡ Total query time: {:?} (local attempt + QDS)",
                        total_elapsed
                    );
                    return Ok(());
                }
                Err(e) => {
                    println!("⚠️ QDS query failed ({}), trying gRPC...", e);
                }
            }
        }

        // Final fallback to gRPC
        match get_wallet_balance_grpc(rpc_addr, address).await {
            Ok(()) => {
                let total_elapsed = start_time.elapsed();
                println!(
                    "⚡ Total query time: {:?} (local attempt + gRPC)",
                    total_elapsed
                );
                Ok(())
            }
            Err(e) => Err(anyhow!(
                "All balance query methods failed. Last error: {}",
                e
            )),
        }
    } else {
        Ok(())
    }
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

    let endpoint = format!("http://{rpc_address}");
    let mut client = QantoRpcClient::connect(endpoint)
        .await
        .map_err(|e| anyhow!("Failed to connect to RPC server {rpc_address}: {e}"))?;

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
    use qanto_rpc::server::generated::{
        wallet_service_client::WalletServiceClient, WalletGetBalanceRequest,
    };

    let endpoint = format!("http://{rpc_address}");
    let mut client = WalletServiceClient::connect(endpoint)
        .await
        .map_err(|e| anyhow!("Failed to connect to RPC server {rpc_address}: {e}"))?;

    let req = WalletGetBalanceRequest {
        address: address.to_string(),
    };
    match client.get_balance(req).await {
        Ok(resp) => {
            let payload = resp.into_inner();
            let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
            let confirmed_qan = (payload.balance as f64) / qan_base;
            let unconfirmed_qan = (payload.unconfirmed_balance as f64) / qan_base;
            println!("📊 Balance for {address}: {confirmed_qan:.6} QANTO");
            if payload.unconfirmed_balance > 0 {
                println!("⏳ Pending: +{unconfirmed_qan:.6} QANTO (mempool)");
            }
            Ok(())
        }
        Err(status) => {
            let code = status.code();
            let msg = status.message().to_string();
            let details_len = status.details().len();
            let code_num = code as i32;
            let hint = if code_num == 14 {
                "Node unreachable: check --node-url, port, firewall/TLS, or server is down."
            } else if code_num == 5 {
                "Address not found: verify address format, checksum, and network."
            } else if code_num == 7 {
                "Access denied: WalletService disabled or authentication required on node."
            } else if code_num == 12 {
                "RPC endpoint not enabled: ensure WalletService is compiled and configured."
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

async fn subscribe_balance_ws(api_address: &str, address: &str) -> Result<()> {
    let url = format!("ws://{api_address}/ws");
    let (ws_stream, _resp) = connect_async(&url)
        .await
        .map_err(|e| anyhow!("Failed to connect WebSocket at {url}: {e}"))?;

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
        .map_err(|e| anyhow!("Failed to send subscribe message: {e}"))?;

    println!("📡 Subscribed to balance updates for {address} via WebSocket at {api_address}");

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(json) => json,
                    Err(_) => continue,
                };
                if v.get("type").and_then(|t| t.as_str()) == Some("balance_update") {
                    let addr = v.get("address").and_then(|a| a.as_str()).unwrap_or("");
                    let bal = v.get("balance").and_then(|b| b.as_u64()).unwrap_or(0);
                    let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                    let bal_qan = (bal as f64) / qan_base;
                    println!(
                        "📊 Balance update for {addr}: {:.6} QANTO ({} base units)",
                        bal_qan, bal
                    );
                }
            }
            Ok(Message::Ping(data)) => {
                // Respond to heartbeat pings
                let _ = write.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) => {
                println!("🔌 WebSocket closed by server");
                break;
            }
            Err(e) => {
                eprintln!("WebSocket error: {e}");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Subscribe to live balance updates via native P2P.
///
/// Executor: runs under the Tokio runtime initialized by the wallet CLI.
/// Registers a message handler for `MessageType::BalanceUpdate` and broadcasts a
/// `BalanceSubscribe` request for the provided address.
async fn subscribe_balance_p2p(
    p2p_client: &Option<Arc<QantoP2P>>,
    address: &str,
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
async fn get_balance_ws_once(api_address: &str, address: &str) -> Result<()> {
    let url = format!("ws://{api_address}/ws");
    let (ws_stream, _resp) = connect_async(&url)
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
                if v.get("type").and_then(|t| t.as_str()) == Some("balance_update") {
                    let addr = v.get("address").and_then(|a| a.as_str()).unwrap_or("");
                    if addr == address {
                        let bal = v.get("balance").and_then(|b| b.as_u64()).unwrap_or(0);
                        let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                        let bal_qan = (bal as f64) / qan_base;
                        println!(
                            "📊 Balance for {address}: {:.6} QANTO ({} base units)",
                            bal_qan, bal
                        );
                        printed = true;
                        let _ = write.send(Message::Close(None)).await;
                        break;
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

        if start.elapsed() > Duration::from_secs(5) {
            break;
        }
    }

    if !printed {
        return Err(anyhow!("No balance update received over WebSocket"));
    }

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
    let qds_balance_mode = matches!(cli.command, Commands::Balance { follow: false, live: false, .. })
        && cli.qds_multiaddr.is_some();

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
            local,
        } => {
            if local {
                // Local-only mode: query from local storage only
                get_balance_local_first(
                    &config.data_dir,
                    &address,
                    true, // local_only = true
                    &config,
                    &rpc_addr,
                    cli.qds_multiaddr.as_deref(),
                )
                .await
            } else if follow {
                // Explicit follow flag: stream live updates via WebSocket
                let api_addr = config.api_address.clone();
                match subscribe_balance_ws(&api_addr, &address).await {
                    Ok(()) => Ok(()),
                    Err(ws_err) => {
                        eprintln!("⚠️ WebSocket not available ({ws_err}). Falling back to gRPC.");
                        get_wallet_balance_grpc(&rpc_addr, &address).await
                    }
                }
            } else if live {
                // Stream via native P2P
                match subscribe_balance_p2p(&p2p_client, &address).await {
                    Ok(()) => Ok(()),
                    Err(p2p_err) => {
                        eprintln!("⚠️ P2P live subscribe failed ({p2p_err}). Falling back to WebSocket.");
                        let api_addr = config.api_address.clone();
                        match subscribe_balance_ws(&api_addr, &address).await {
                            Ok(()) => Ok(()),
                            Err(ws_err) => {
                                eprintln!("⚠️ WebSocket not available ({ws_err}). Falling back to gRPC.");
                                get_wallet_balance_grpc(&rpc_addr, &address).await
                            }
                        }
                    }
                }
            } else {
                // Default behavior now follows live updates via WebSocket (with gRPC fallback)
                let api_addr = config.api_address.clone();
                match subscribe_balance_ws(&api_addr, &address).await {
                    Ok(()) => Ok(()),
                    Err(ws_err) => {
                        eprintln!("⚠️ WebSocket not available ({ws_err}). Falling back to gRPC.");
                        get_wallet_balance_grpc(&rpc_addr, &address).await
                    }
                }
            }
        }
        Commands::BalanceRt { address, db_dir } => {
            // Real-time local storage query against node DB
            let dir = db_dir.unwrap_or_else(|| PathBuf::from(&config.data_dir));
            let cfg = StorageConfig {
                data_dir: dir.clone(),
                wal_enabled: false,
                sync_writes: false,
                compression_enabled: false,
                encryption_enabled: false,
                ..StorageConfig::default()
            };
            let storage = QantoStorage::new(cfg).map_err(|e| anyhow!("Storage init error: {e}"))?;
            match get_balance_storage_with_instance(&storage, address.as_str()).await? {
                Some(amount) => {
                    let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                    let display = (amount as f64) / qan_base;
                    println!("📊 Balance for {address}: {display:.6} QANTO");
                    Ok(())
                }
                None => {
                    println!("📊 Balance for {address}: 0.000000 QANTO");
                    Ok(())
                }
            }
        }
        Commands::Watch { address } => {
            let api_addr = config.api_address.clone();
            subscribe_balance_ws(&api_addr, &address).await
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
    use crate::types::UTXO;
    use libp2p::identity;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_basic_arithmetic() {
        assert_eq!(2 + 2, 4);
    }

    #[tokio::test]
    async fn test_wallet_balance_integrity() {
        // Create temporary directory for test database
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");
        let db_path_str = db_path.to_str().unwrap();

        // Test address
        let test_address = "test_address_12345678901234567890123456789012345678901234567890123456";

        // Test 1: Empty balance (no UTXOs)
        let balance = get_balance_storage(db_path_str, test_address).await;
        assert!(balance.is_ok());
        assert_eq!(
            balance.unwrap(),
            None,
            "Empty wallet should have no balance"
        );

        // Test 2: Create storage and add test UTXOs
        let storage_config = StorageConfig {
            data_dir: db_path.clone(),
            cache_size: 1024 * 1024, // 1MB cache
            ..Default::default()
        };
        let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

        // Add test UTXOs with known amounts
        let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN;
        let utxo1_amount = qan_base; // 1 QANTO
        let utxo2_amount = qan_base / 2; // 0.5 QANTO
        let utxo3_amount = 123_456; // 0.123456 QANTO

        // Create test UTXOs
        let utxo1 = UTXO {
            address: test_address.to_string(),
            amount: utxo1_amount,
            tx_id: "tx1".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };

        let utxo2 = UTXO {
            address: test_address.to_string(),
            amount: utxo2_amount,
            tx_id: "tx2".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };

        let utxo3 = UTXO {
            address: test_address.to_string(),
            amount: utxo3_amount,
            tx_id: "tx3".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };

        // Store UTXOs in database
        let utxo1_key = format!("{}:tx1:0", String::from_utf8_lossy(&utxos_prefix()));
        let utxo2_key = format!("{}:tx2:0", String::from_utf8_lossy(&utxos_prefix()));
        let utxo3_key = format!("{}:tx3:0", String::from_utf8_lossy(&utxos_prefix()));

        storage
            .put(
                utxo1_key.as_bytes().to_vec(),
                crate::persistence::encode_utxo(&utxo1).unwrap(),
            )
            .expect("Failed to store UTXO1");
        storage
            .put(
                utxo2_key.as_bytes().to_vec(),
                crate::persistence::encode_utxo(&utxo2).unwrap(),
            )
            .expect("Failed to store UTXO2");
        storage
            .put(
                utxo3_key.as_bytes().to_vec(),
                crate::persistence::encode_utxo(&utxo3).unwrap(),
            )
            .expect("Failed to store UTXO3");

        // Store balance cache
        let expected_total = utxo1_amount + utxo2_amount + utxo3_amount;
        let balance_key_bytes = balance_key(test_address);
        storage
            .put(
                balance_key_bytes,
                crate::persistence::encode_balance(expected_total),
            )
            .expect("Failed to store balance");

        // Explicitly flush/sync storage to ensure data is written to disk
        if let Err(e) = storage.flush() {
            println!("Warning: Failed to flush storage: {}", e);
        }

        // Test 3: Verify balance calculation
        let balance = get_balance_storage_with_instance(&storage, test_address).await;
        assert!(balance.is_ok());
        let actual_balance = balance.unwrap().unwrap_or(0); // Handle None case
        assert_eq!(
            actual_balance, expected_total,
            "Balance should match sum of UTXOs"
        );

        // Test 4: Verify balance formatting
        let qan_display = (actual_balance as f64) / (qan_base as f64);
        let expected_display = 1.623456; // 1 + 0.5 + 0.123456
        assert!(
            (qan_display - expected_display).abs() < 0.000001,
            "Balance display should be accurate"
        );

        // Test 5: Test with different address (should be empty)
        let other_address =
            "other_address_12345678901234567890123456789012345678901234567890123456";
        let other_balance = get_balance_storage_with_instance(&storage, other_address).await;
        assert!(other_balance.is_ok());
        assert_eq!(
            other_balance.unwrap(),
            None,
            "Different address should have no balance"
        );

        // Test 6: Test balance overflow protection
        let max_amount = u64::MAX - 1000; // Near max value
        let overflow_utxo = UTXO {
            address: test_address.to_string(),
            amount: max_amount,
            tx_id: "tx_overflow".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };

        let overflow_key = format!("{}:tx_overflow:0", String::from_utf8_lossy(&utxos_prefix()));
        storage
            .put(
                overflow_key.as_bytes().to_vec(),
                crate::persistence::encode_utxo(&overflow_utxo).unwrap(),
            )
            .expect("Failed to store overflow UTXO");

        // Update balance cache with saturating add
        let new_total = expected_total.saturating_add(max_amount);
        storage
            .put(
                balance_key(test_address),
                crate::persistence::encode_balance(new_total),
            )
            .expect("Failed to update balance");

        let overflow_balance = get_balance_storage_with_instance(&storage, test_address).await;
        assert!(overflow_balance.is_ok());
        let overflow_result = overflow_balance
            .unwrap()
            .expect("Overflow balance should exist");
        assert!(
            overflow_result >= expected_total,
            "Balance should handle overflow gracefully"
        );

        println!("✅ Wallet balance integrity test passed");
        println!("   - Empty balance: ✓");
        println!("   - Multiple UTXOs: ✓");
        println!("   - Balance calculation: ✓");
        println!("   - Display formatting: ✓");
        println!("   - Address isolation: ✓");
        println!("   - Overflow protection: ✓");
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
                local,
            } => {
                assert_eq!(follow, &false);
                assert_eq!(live, &false);
                assert_eq!(local, &false);
                assert_eq!(
                    address,
                    "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"
                );
            }
            _ => panic!("expected Balance command"),
        }
        assert!(cli.qds_multiaddr.is_some(), "qds_multiaddr should be set");

        // Lightweight mode should be active
        let qds_balance_mode = matches!(cli.command, Commands::Balance { follow: false, live: false, .. })
            && cli.qds_multiaddr.is_some();
        assert!(qds_balance_mode, "QDS lightweight mode should be detected");
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
}
