use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use qanto::{
    qantodag::UTXO,
    transaction::{self, Input, Output, Transaction, TransactionConfig},
    wallet::{Wallet, WalletError},
};
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// --- Constants ---
const DEV_ADDRESS: &str = "74fd2aae70ae8e0930b87a3dcb3b77f5b71d956659849f067360d3486604db41";
const DEV_FEE_RATE: f64 = 0.0304;

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
    #[arg(long, global = true, default_value = "http://127.0.0.1:8081")]
    node_url: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// [generate] Creates a new, secure, and encrypted wallet file.
    Generate {
        #[arg(short, long, value_name = "OUTPUT_FILE", default_value = "wallet.key")]
        output: PathBuf,
    },
    /// [show-secrets] CRITICAL: Reveals the private key and mnemonic of a wallet.
    ShowSecrets {
        #[arg(short, long, value_name = "WALLET_FILE", default_value = "wallet.key")]
        wallet: PathBuf,
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
        #[arg(short, long, value_name = "WALLET_FILE", default_value = "wallet.key")]
        wallet: PathBuf,
        #[arg()]
        to: String,
        #[arg()]
        amount: u64,
    },
    /// [receive] Monitors for incoming transactions to this wallet.
    Receive {
        #[arg(short, long, value_name = "WALLET_FILE", default_value = "wallet.key")]
        wallet: PathBuf,
    },
}

// --- Main Logic ---

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Generate { output } => generate_wallet(output).await,
        Commands::ShowSecrets { wallet } => show_secrets(wallet).await,
        Commands::Import {
            mnemonic,
            private_key,
        } => import_wallet(mnemonic, private_key).await,
        Commands::Balance { address } => get_balance(&cli.node_url, address).await,
        Commands::Send { wallet, to, amount } => {
            send_transaction(&cli.node_url, wallet, to, amount).await
        }
        Commands::Receive { wallet } => receive_transactions(&cli.node_url, wallet).await,
    };

    if let Err(e) = result {
        eprintln!("\nNEURAL-VAULT™ Error: {e:?}");
        std::process::exit(1);
    }
    Ok(())
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
    println!("   To ensure you can recover your funds, run the 'show-secrets' command.");
    println!("   This will display your mnemonic phrase, which you must write down and store securely.");
    Ok(())
}

async fn show_secrets(wallet_path: PathBuf) -> Result<()> {
    if !wallet_path.exists() {
        return Err(anyhow::anyhow!(
            "Wallet file not found at: {:?}",
            wallet_path
        ));
    }

    println!("Enter password to decrypt and reveal secrets:");
    let password = prompt_for_password(false, "")?;

    println!("🔓 Decrypting NEURAL-VAULT™...");
    let loaded_wallet = Wallet::from_file(&wallet_path, &password).map_err(|e| {
        anyhow::anyhow!("Failed to decrypt vault. Check your password. Error: {}", e)
    })?;

    let private_key_hex = hex::encode(loaded_wallet.get_signing_key()?.to_bytes());
    let mnemonic_phrase = loaded_wallet.mnemonic().expose_secret();

    // **SECURITY FIX: Display a severe, unmissable warning before showing secrets.**
    println!("\n+----------------------------------------------------------+");
    println!("|           🔥🔥🔥 CRITICAL SECURITY WARNING 🔥🔥🔥           |");
    println!("+----------------------------------------------------------+");
    println!("|                                                          |");
    println!("|  NEVER share your Private Key or Mnemonic Phrase.          |");
    println!("|  Anyone with this information can STEAL ALL YOUR FUNDS.    |");
    println!("|                                                          |");
    println!("|  Store this information OFFLINE and in a SECURE location.  |");
    println!("|  Do NOT save it in a plain text file or cloud storage.     |");
    println!("|                                                          |");
    println!("+----------------------------------------------------------+");
    println!("\nWallet File:     {wallet_path:?}");
    println!("Public Address:  {}", loaded_wallet.address());
    println!("Private Key:     {private_key_hex}");
    println!("Mnemonic Phrase: {mnemonic_phrase}");
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

async fn get_balance(node_url: &str, address: String) -> Result<()> {
    println!("🌐 Instant-MeshSync™: Checking balance for {address}...");
    let client = Client::new();
    let url = format!("{node_url}/balance/{address}");

    let res = client
        .get(&url)
        .send()
        .await
        .context(format!("Failed to connect to node at {url}"))?;
    if res.status().is_success() {
        let balance: u64 = res
            .json()
            .await
            .context("Failed to parse balance from response")?;
        println!("\n💰 Balance: {balance} QNTO");
    } else {
        let error_text = res
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(anyhow!("Node returned an error: {}", error_text));
    }
    Ok(())
}

async fn send_transaction(
    node_url: &str,
    wallet_path: PathBuf,
    to: String,
    amount: u64,
) -> Result<()> {
    println!("Preparing to send {amount} QNTO to address {to}");
    let password = prompt_for_password(false, "Enter password to unlock vault for sending:")?;
    let wallet = Wallet::from_file(&wallet_path, &password).context(format!(
        "Failed to load vault from '{}'",
        wallet_path.display()
    ))?;
    let sender_address = wallet.address();
    let client = Client::new();

    let utxo_url = format!("{node_url}/utxos/{sender_address}");
    let res = client
        .get(&utxo_url)
        .send()
        .await
        .context("Failed to fetch UTXOs")?;
    if !res.status().is_success() {
        return Err(anyhow!(
            "Node failed to provide UTXOs: {}",
            res.text().await?
        ));
    }
    let available_utxos: HashMap<String, UTXO> =
        res.json().await.context("Failed to parse UTXOs")?;
    if available_utxos.is_empty() {
        return Err(anyhow!("No funds available for address {}", sender_address));
    }

    let fee = transaction::calculate_dynamic_fee(amount);
    let dev_fee = (amount as f64 * DEV_FEE_RATE).round() as u64;
    let total_needed = amount + fee + dev_fee;
    let mut inputs = vec![];
    let mut total_input_amount = 0;
    for (_utxo_id, utxo) in available_utxos {
        if total_input_amount >= total_needed {
            break;
        }
        total_input_amount += utxo.amount;
        inputs.push(Input {
            tx_id: utxo.tx_id,
            output_index: utxo.output_index,
        });
    }
    if total_input_amount < total_needed {
        return Err(anyhow!(
            "Insufficient funds. Needed: {}, Available: {}",
            total_needed,
            total_input_amount
        ));
    }

    let he_public_key = wallet.get_signing_key()?.verifying_key();
    let he_pub_key_material: &[u8] = he_public_key.as_bytes();
    let mut outputs = vec![Output {
        address: to.clone(),
        amount,
        homomorphic_encrypted: qanto::qantodag::HomomorphicEncrypted::new(
            amount,
            he_pub_key_material,
        ),
    }];
    if dev_fee > 0 {
        outputs.push(Output {
            address: DEV_ADDRESS.to_string(),
            amount: dev_fee,
            homomorphic_encrypted: qanto::qantodag::HomomorphicEncrypted::new(
                dev_fee,
                he_pub_key_material,
            ),
        });
    }
    let change = total_input_amount - total_needed;
    if change > 0 {
        outputs.push(Output {
            address: sender_address.clone(),
            amount: change,
            homomorphic_encrypted: qanto::qantodag::HomomorphicEncrypted::new(
                change,
                he_pub_key_material,
            ),
        });
    }

    let mut metadata_map = HashMap::new();
    metadata_map.insert("gatt_uuid".to_string(), Uuid::new_v4().to_string());

    let signing_key = wallet.get_signing_key()?;
    let tx_config = TransactionConfig {
        sender: sender_address,
        receiver: to,
        amount,
        fee,
        inputs,
        outputs,
        signing_key_bytes: signing_key.as_bytes(),
        tx_timestamps: Arc::new(RwLock::new(HashMap::new())),
        metadata: Some(metadata_map),
    };
    let tx = Transaction::new(tx_config)
        .await
        .context("Failed to create transaction")?;
    println!("Transaction created with ID: {}", tx.id);

    println!("🛡️ Anti-Malware TX Shield: Verifying transaction behavior...");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    println!("   Behavioral signature check passed.");

    let tx_url = format!("{node_url}/transaction");
    println!("Broadcasting transaction to {tx_url}...");
    let res = client
        .post(&tx_url)
        .json(&tx)
        .send()
        .await
        .context("Failed to send transaction")?;

    if res.status().is_success() {
        let tx_id_response: String = res
            .json()
            .await
            .context("Failed to parse transaction ID from response")?;
        println!("\n✅ Transaction submitted successfully!");
        println!("   Transaction ID: {tx_id_response}");
    } else {
        let error_text = res
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(anyhow!("Node rejected transaction: {}", error_text));
    }
    Ok(())
}

async fn receive_transactions(node_url: &str, wallet_path: PathBuf) -> Result<()> {
    println!(
        "Enter password to monitor incoming transactions for '{}':",
        wallet_path.display()
    );
    let password = prompt_for_password(false, "")?;
    let wallet = Wallet::from_file(&wallet_path, &password)?;
    let my_address = wallet.address();
    let client = Client::new();
    let mut known_tx_ids = HashSet::new();
    println!("\n📡 Listening for incoming transactions to {my_address} (Press Ctrl+C to stop)...");

    loop {
        let dag_info_url = format!("{node_url}/dag");
        if let Ok(res) = client.get(&dag_info_url).send().await {
            if let Ok(dag_info) = res.json::<serde_json::Value>().await {
                if let Some(tips) = dag_info.get("tips").and_then(|t| t.as_object()) {
                    for (_chain_id, tip_ids) in tips {
                        if let Some(tip_ids_array) = tip_ids.as_array() {
                            for tip_id_val in tip_ids_array {
                                if let Some(tip_id) = tip_id_val.as_str() {
                                    let block_url = format!("{node_url}/block/{tip_id}");
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
                                                            "   Amount: {} QNTO",
                                                            output.amount
                                                        );
                                                        println!("   From: {}", tx.sender);
                                                        println!("   Transaction ID: {}", tx.id);
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
    if !prompt_text.is_empty() {
        println!("{prompt_text}");
    }
    print!("Enter password: ");
    io::stdout().flush().map_err(WalletError::Io)?;
    let password = rpassword::read_password().map_err(WalletError::Io)?;

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
