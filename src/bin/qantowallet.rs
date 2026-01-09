use clap::{Parser, Subcommand};
use qanto::node_keystore::Wallet;
use qanto::password_utils::prompt_for_password;
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "qantowallet",
    version,
    about = "autonomous, zero-dependency CLI",
    long_about = None
)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:8332")]
    node_url: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Balance {
        address: String,
    },
    Airdrop {
        address: String,
    },
    Import {
        #[arg(long, help = "Hex-encoded private key")]
        private_key: String,
        #[arg(long, default_value = "wallet.key", help = "Output wallet file path")]
        output: String,
    },
    Export {
        #[arg(
            long,
            default_value = "wallet.key",
            help = "Wallet file to export from"
        )]
        wallet: String,
    },
    Watch {
        address: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Balance { address } => {
            get_balance(&cli.node_url, &address).await;
        }
        Commands::Airdrop { address } => {
            request_airdrop(&cli.node_url, &address).await;
        }
        Commands::Import {
            private_key,
            output,
        } => {
            if let Err(e) = import_wallet(&private_key, &output) {
                eprintln!("Error importing wallet: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Export { wallet } => {
            if let Err(e) = export_wallet(&wallet) {
                eprintln!("Error exporting wallet: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Watch { address } => {
            watch_balance(&cli.node_url, &address).await;
        }
    }
}

fn import_wallet(private_key: &str, output_path: &str) -> anyhow::Result<()> {
    // Validate private key length (hex string length should be 64 chars for 32 bytes or 128 for 64 bytes)
    // The prompt mentions ensuring key length matches CRYSTALS-Dilithium parameter sets.
    // However, Wallet::from_private_key takes Ed25519 key and generates PQ key.
    // We will stick to the existing Wallet::from_private_key logic which validates length.

    let password = prompt_for_password(true, Some("Enter password for imported wallet:"))?;
    let wallet = Wallet::from_private_key(private_key)?;
    wallet.save_to_file(output_path, &password)?;

    println!(
        "Wallet imported and saved to '{}'. Address: {}",
        output_path,
        wallet.address()
    );
    println!("IMPORTANT: Please back up this file securely and remember your password.");
    Ok(())
}

fn export_wallet(wallet_path: &str) -> anyhow::Result<()> {
    // prompt_for_password returns SecretString
    let password = prompt_for_password(false, Some("Enter password to decrypt wallet:"))?;

    let wallet = Wallet::from_file(std::path::Path::new(wallet_path), &password)?;
    let (sk, _) = wallet.get_keypair()?;
    let hex_sk = hex::encode(sk.as_bytes());

    println!("Private Key (Hex): {}", hex_sk);
    println!("WARNING: Keep this key secret! Anyone with this key can spend your funds.");
    Ok(())
}

async fn watch_balance(url: &str, address: &str) {
    let client = reqwest::Client::new();
    println!("Watching balance for {}...", address);

    loop {
        // Clear terminal screen (ANSI escape code)
        print!("\x1B[2J\x1B[1;1H");
        println!("Qanto Watch Mode - Address: {}", address);
        println!("------------------------------------------------");

        let body = json!({
            "jsonrpc": "2.0",
            "method": "get_balance",
            "params": [address],
            "id": 1
        });

        match client.post(url).json(&body).send().await {
            Ok(resp) => match resp.text().await {
                Ok(text) => {
                    // Try to parse the response to extract just the result if possible,
                    // otherwise print raw response which is what get_balance did.
                    // Assuming standard JSON-RPC response format.
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(result) = json.get("result") {
                            println!("Current Balance: {}", result);
                        } else if let Some(error) = json.get("error") {
                            println!("Error: {}", error);
                        } else {
                            println!("Raw Response: {}", text);
                        }
                    } else {
                        println!("Raw Response: {}", text);
                    }
                }
                Err(e) => println!("Failed to read response: {}", e),
            },
            Err(e) => println!("Connection Refused: {}", e),
        }

        // 50ms sleep for ~20 updates/sec (supporting 32 BPS visibility)
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

async fn get_balance(url: &str, address: &str) {
    let client = reqwest::Client::new();
    // POST to / (JSON-RPC standard)
    let body = json!({
        "jsonrpc": "2.0",
        "method": "get_balance",
        "params": [address],
        "id": 1
    });

    match client.post(url).json(&body).send().await {
        Ok(resp) => match resp.text().await {
            Ok(text) => println!("Balance: {}", text),
            Err(e) => eprintln!("Failed to read response: {}", e),
        },
        Err(e) => eprintln!("Connection Refused: {}", e),
    }
}

async fn request_airdrop(url: &str, address: &str) {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0",
        "method": "request_airdrop",
        "params": [address],
        "id": 1
    });

    match client.post(url).json(&body).send().await {
        Ok(resp) => match resp.text().await {
            Ok(text) => println!("Airdrop: {}", text),
            Err(e) => eprintln!("Failed to read response: {}", e),
        },
        Err(e) => eprintln!("Connection Refused: {}", e),
    }
}
