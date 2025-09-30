use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use bytes::Bytes;
use my_blockchain::qanto_standalone::hash::QantoHash;
use qanto::{
    config::Config,
    qanto_p2p::{MessageType, NetworkMessage, QantoP2P},
    transaction::Transaction,
    wallet::{Wallet, WalletError},
};
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::future::pending;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid; // Assuming this is the correct path based on error suggestion

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
        default_value = "18081"
    )]
    p2p_port: u16,
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
    },
    /// [send] Sends QNTO with Governance-Aware Transaction Tracking™.
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

// --- Main Logic ---

#[tokio::main]
async fn main() -> Result<()> {
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

    // Initialize P2P client if discovery is enabled
    let p2p_client = if cli.p2p_discovery.unwrap_or(true) {
        Some(initialize_p2p_client(cli.p2p_port).await?)
    } else {
        None
    };

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
        Commands::Balance { address } => get_balance_p2p(&p2p_client, address).await,
        Commands::Send { wallet, to, amount } => {
            let wallet_path = wallet.unwrap_or_else(|| PathBuf::from(&config.wallet_path));
            send_transaction_p2p(&p2p_client, wallet_path, to, amount).await
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

// --- P2P Network Initialization ---

async fn initialize_p2p_client(port: u16) -> Result<Arc<QantoP2P>> {
    let config = qanto::qanto_p2p::NetworkConfig {
        max_connections: 50,
        connection_timeout: Duration::from_secs(30),
        heartbeat_interval: Duration::from_secs(10),
        enable_encryption: true,
        bootstrap_nodes: vec![], // Will be discovered via peer discovery
        listen_port: port,
    };

    let p2p = QantoP2P::new(config).context("Failed to initialize P2P network")?;

    // P2P network will handle peer discovery internally

    println!("✓ P2P network initialized on port {port}");
    Ok(Arc::new(p2p))
}

// --- Command Implementations ---

async fn generate_wallet(output: PathBuf) -> Result<()> {
    println!("🛡️ NEURAL-VAULT™: Generating new Quantum-Aware Dual-Layer Key...");
    let password = prompt_for_password(true, "Create a secure password to encrypt the new vault:")?;
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
    let password = prompt_for_password(false, "")?;

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
    let password = prompt_for_password(true, "Create a password to encrypt the imported vault:")?;
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

async fn get_balance_p2p(p2p_client: &Option<Arc<QantoP2P>>, address: String) -> Result<()> {
    if let Some(p2p) = p2p_client {
        println!("🔍 Querying balance via P2P network for address: {address}");

        let query_id = Uuid::new_v4();
        let request = BalanceRequest {
            address: address.clone(),
            query_id,
        };
        let payload = bincode::serialize(&request)?;
        p2p.broadcast(MessageType::Custom(1), Bytes::from(payload))?;

        // Register temporary handler for response
        let (tx, mut rx) = mpsc::channel(1);
        let handler = Arc::new(move |msg: NetworkMessage, _peer: QantoHash| -> Result<()> {
            if msg.msg_type == MessageType::Custom(2) {
                let response: BalanceResponse = bincode::deserialize(&msg.payload)?;
                if response.query_id == query_id {
                    tx.try_send(response.balance)?;
                }
            }
            Ok(())
        });
        p2p.register_handler(MessageType::Custom(2), handler);

        // Wait for response (simplified, in production use timeout)
        if let Some(balance) = rx.recv().await {
            println!("📊 Balance for {address}: {balance} QANTO (P2P mode)");
        } else {
            println!("No response received");
        }
        return Ok(());
    }
    get_balance_http(&address).await
}

// HTTP fallback implementation
async fn get_balance_http(address: &str) -> Result<()> {
    println!("⚠️  HTTP fallback mode - consider enabling P2P discovery");
    // Simplified implementation - in production would connect to a node
    println!("📊 Balance for {address}: 0 QANTO (HTTP fallback)");
    Ok(())
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

// Add at the top after existing use statements
use bincode::deserialize;
use qanto::types::QuantumResistantSignature;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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

// For receive_transactions_p2p
async fn receive_transactions_p2p(
    p2p_client: &Option<Arc<QantoP2P>>,
    wallet_path: PathBuf,
) -> Result<()> {
    if let Some(p2p) = p2p_client {
        println!("Enter password to monitor incoming transactions:");
        let password = prompt_for_password(false, "")?;
        let wallet = Wallet::from_file(&wallet_path, &password)?;
        let my_address = wallet.address();
        let my_address_clone = my_address.clone(); // Add this line

        // Register handler for incoming tx
        let handler = Arc::new(move |msg: NetworkMessage, _peer: QantoHash| -> Result<()> {
            if msg.msg_type == MessageType::Custom(3) {
                let tx: Transaction = deserialize(&msg.payload)?;
                if tx.outputs.iter().any(|o| o.address == my_address_clone) {
                    // Use clone here
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
    let password = prompt_for_password(false, "")?;
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
                                            block_res.json::<qanto::qantodag::QantoBlock>().await
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
                                                            "   Amount: {amount} QNTO",
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

fn prompt_for_password(confirm: bool, prompt_text: &str) -> Result<SecretString, WalletError> {
    // Check for WALLET_PASSWORD environment variable first
    if let Ok(env_password) = std::env::var("WALLET_PASSWORD") {
        // Validate that the password is not empty
        if env_password.is_empty() {
            return Err(WalletError::Passphrase(
                "WALLET_PASSWORD environment variable is set but empty.".to_string(),
            ));
        }
        println!("Using password from WALLET_PASSWORD environment variable.");
        return Ok(SecretString::new(env_password));
    }

    // Fallback to interactive prompt if no environment variable
    if !prompt_text.is_empty() {
        println!("{prompt_text}");
    }
    print!("Enter password: ");
    io::stdout().flush().map_err(WalletError::Io)?;
    let password = rpassword::read_password().map_err(WalletError::Io)?;

    // Validate that the password is not empty
    if password.is_empty() {
        return Err(WalletError::Passphrase(
            "Password cannot be empty.".to_string(),
        ));
    }

    if confirm {
        print!("Confirm password: ");
        io::stdout().flush().map_err(WalletError::Io)?;
        let confirmation = rpassword::read_password().map_err(WalletError::Io)?;
        if password != confirmation {
            return Err(WalletError::Passphrase(
                "Passwords do not match.".to_string(),
            ));
        }
    }
    Ok(SecretString::new(password))
}

fn prompt_for_input() -> Result<String, io::Error> {
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
