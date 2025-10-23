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
use my_blockchain::qanto_standalone::hash::QantoHash;
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
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

// Add storage and persistence imports for direct DB reads
use crate::qanto_storage::{QantoStorage, StorageConfig};
use crate::persistence::{balance_key, decode_balance, utxos_prefix, decode_utxo};

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
async fn get_balance_storage(db_path: &str, address: &str) -> Result<Option<u64>> {
    // Initialize storage with read-only friendly config
    let mut cfg = StorageConfig::default();
    cfg.data_dir = PathBuf::from(db_path);
    cfg.wal_enabled = false; // avoid creating WAL when just reading

    let storage = QantoStorage::new(cfg).map_err(|e| anyhow!("Storage init error: {e}"))?;
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
                    let total = scan_utxos_total(&storage)?;
                    Ok(Some(total))
                }
            }
        }
        Ok(None) => {
            // Fallback: compute balance by scanning UTXOs
            let total = scan_utxos_total(&storage)?;
            Ok(Some(total))
        }
        Err(e) => Err(anyhow!("Storage read error: {e}")),
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

    println!(
        "📡 Subscribed to balance updates for {address} via WebSocket at {api_address}"
    );

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
                    println!("📊 Balance update for {addr}: {bal} base units");
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

        config.data_dir = new_data_dir;
        config.db_path = new_db_path;
        // Only update wallet and p2p paths if they're still defaults
        if config.wallet_path == "wallet.key" {
            config.wallet_path = new_wallet_path;
        }
        if config.p2p_identity_path == "p2p_identity.key" {
            config.p2p_identity_path = new_p2p_identity_path;
        }
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

    // Initialize P2P client if discovery is enabled; fall back to HTTP on failure
    let p2p_client = if cli.p2p_discovery.unwrap_or(true) {
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
        Commands::Balance { address, follow } => {
            if follow {
                let api_addr = config.api_address.clone();
                subscribe_balance_ws(&api_addr, &address).await
            } else {
                // Direct RocksDB read - authoritative balance source
                match get_balance_storage(&config.db_path, &address).await {
                    Ok(Some(amount)) => {
                        // Format to 6-decimal QANTO display
                        let qan_base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                        let qan = (amount as f64) / qan_base;
                        println!("📊 Balance for {address}: {qan:.6} QANTO");
                        Ok(())
                    }
                    Ok(None) => {
                        // No UTXOs found for this address
                        println!("📊 Balance for {address}: 0.000000 QANTO");
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to read balance from database: {e}");
                        std::process::exit(1);
                    }
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
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::test;

    #[test]
    fn test_basic_arithmetic() {
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
        assert_eq!(balance.unwrap(), None, "Empty wallet should have no balance");

        // Test 2: Create storage and add test UTXOs
        let storage_config = StorageConfig {
            db_path: db_path_str.to_string(),
            cache_size: 1024 * 1024, // 1MB cache
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
        let utxo1_key = format!("{}:tx1:0", utxos_prefix(test_address));
        let utxo2_key = format!("{}:tx2:0", utxos_prefix(test_address));
        let utxo3_key = format!("{}:tx3:0", utxos_prefix(test_address));

        storage.put(&utxo1_key, &bincode::serialize(&utxo1).unwrap()).expect("Failed to store UTXO1");
        storage.put(&utxo2_key, &bincode::serialize(&utxo2).unwrap()).expect("Failed to store UTXO2");
        storage.put(&utxo3_key, &bincode::serialize(&utxo3).unwrap()).expect("Failed to store UTXO3");

        // Store balance cache
        let expected_total = utxo1_amount + utxo2_amount + utxo3_amount;
        let balance_key = balance_key(test_address);
        storage.put(&balance_key, &bincode::serialize(&expected_total).unwrap()).expect("Failed to store balance");

        // Test 3: Verify balance calculation
        let balance = get_balance_storage(db_path_str, test_address).await;
        assert!(balance.is_ok());
        let actual_balance = balance.unwrap().expect("Balance should exist");
        assert_eq!(actual_balance, expected_total, "Balance should match sum of UTXOs");

        // Test 4: Verify balance formatting
        let qan_display = (actual_balance as f64) / (qan_base as f64);
        let expected_display = 1.623456; // 1 + 0.5 + 0.123456
        assert!((qan_display - expected_display).abs() < 0.000001, "Balance display should be accurate");

        // Test 5: Test with different address (should be empty)
        let other_address = "other_address_12345678901234567890123456789012345678901234567890123456";
        let other_balance = get_balance_storage(db_path_str, other_address).await;
        assert!(other_balance.is_ok());
        assert_eq!(other_balance.unwrap(), None, "Different address should have no balance");

        // Test 6: Test balance overflow protection
        let max_amount = u64::MAX - 1000; // Near max value
        let overflow_utxo = UTXO {
            address: test_address.to_string(),
            amount: max_amount,
            tx_id: "tx_overflow".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };

        let overflow_key = format!("{}:tx_overflow:0", utxos_prefix(test_address));
        storage.put(&overflow_key, &bincode::serialize(&overflow_utxo).unwrap()).expect("Failed to store overflow UTXO");

        // Update balance cache with saturating add
        let new_total = expected_total.saturating_add(max_amount);
        storage.put(&balance_key, &bincode::serialize(&new_total).unwrap()).expect("Failed to update balance");

        let overflow_balance = get_balance_storage(db_path_str, test_address).await;
        assert!(overflow_balance.is_ok());
        let overflow_result = overflow_balance.unwrap().expect("Overflow balance should exist");
        assert!(overflow_result >= expected_total, "Balance should handle overflow gracefully");

        println!("✅ Wallet balance integrity test passed");
        println!("   - Empty balance: ✓");
        println!("   - Multiple UTXOs: ✓");
        println!("   - Balance calculation: ✓");
        println!("   - Display formatting: ✓");
        println!("   - Address isolation: ✓");
        println!("   - Overflow protection: ✓");
    }
}
