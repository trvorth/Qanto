use anyhow::Result;
use clap::{Parser, Subcommand};
use qanto::config::Config;
use qanto::node::Node;
use qanto::wallet::Wallet;
use secrecy::{ExposeSecret, SecretString};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(author, version, about = "Qanto Node CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Starts the Qanto node
    Start {
        #[arg(short, long, default_value = "config.toml")]
        config: String,
        #[arg(short, long, default_value = "wallet.key")]
        wallet: String,
        #[arg(long, default_value = "p2p_identity.key")]
        p2p_identity: String,
        #[arg(long, default_value = "peer_cache.json")]
        peer_cache: String,
        /// Clean the database directory before starting to prevent corruption errors.
        #[arg(long)]
        clean: bool,
    },
    /// Generates a new wallet
    GenerateWallet {
        #[arg(short, long, default_value = "wallet.key")]
        output: String,
    },
}

#[derive(Debug, Error)]
enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Password(String),
}

/// Helper function to securely prompt for a password from the command line.
fn prompt_for_password(confirm: bool) -> Result<SecretString, CliError> {
    print!("Enter wallet password: ");
    io::stdout().flush()?;
    // Add explicit type annotation here
    let password: SecretString = rpassword::read_password()?.into();

    if confirm {
        print!("Confirm wallet password: ");
        io::stdout().flush()?;
        // And here as well
        let confirmation: SecretString = rpassword::read_password()?.into();
        // Correctly compare the secret values
        if password.expose_secret() != confirmation.expose_secret() {
            return Err(CliError::Password("Passwords do not match.".to_string()));
        }
    }
    Ok(password)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Correctly initialize logging.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            config,
            wallet,
            p2p_identity,
            peer_cache,
            clean,
        } => {
            println!("Qanto node starting...");
            let config_path = Path::new(&config);
            if !config_path.exists() {
                eprintln!("Configuration file not found: {}", config_path.display());
                std::process::exit(1);
            }
            println!("Configuration loaded from '{}'.", config_path.display());

            let db_path = "qantodag_db_evolved";
            if clean {
                println!(
                    "'--clean' flag detected. Removing old database directory: {}",
                    db_path
                );
                if Path::new(db_path).exists() {
                    if let Err(e) = fs::remove_dir_all(db_path) {
                        eprintln!("Failed to remove database directory '{}': {}", db_path, e);
                        std::process::exit(1);
                    }
                    println!("Database directory removed successfully.");
                }
            }

            // Correctly load config and wallet with password.
            let password = prompt_for_password(false)?;
            let node_config = Config::load(&config)?;
            // Pass the SecretString directly, without re-wrapping it.
            let wallet_instance = Wallet::from_file(&wallet, &password)?;
            let wallet_arc = Arc::new(wallet_instance);

            println!("Initializing Qanto services...");
            let node = Node::new(
                node_config,
                config.clone(),
                wallet_arc,
                &p2p_identity,
                peer_cache,
            )
            .await?;

            node.start().await?;
        }
        Commands::GenerateWallet { output } => {
            println!("Generating new wallet...");
            let password = prompt_for_password(true)?;
            let wallet = Wallet::new()?;
            // Correctly save the wallet with the SecretString.
            wallet.save_to_file(&output, &password)?;
            println!(
                "New wallet saved to '{}'. Address: {}",
                output,
                wallet.address()
            );
            println!("IMPORTANT: Please back up this file securely and remember your password.");
        }
    }

    Ok(())
}
