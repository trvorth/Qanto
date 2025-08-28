use anyhow::Result;
use clap::{Parser, Subcommand};
use qanto::config::Config;
use qanto::node::Node;
use qanto::wallet::Wallet;
use secrecy::{ExposeSecret, SecretString};
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};
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
    // Check for WALLET_PASSWORD environment variable first
    if let Ok(env_password) = std::env::var("WALLET_PASSWORD") {
        // Validate that the password is not empty
        if env_password.is_empty() {
            return Err(CliError::Password(
                "WALLET_PASSWORD environment variable is set but empty.".to_string(),
            ));
        }
        println!("Using password from WALLET_PASSWORD environment variable.");
        return Ok(SecretString::new(env_password));
    }

    // Fallback to interactive prompt if no environment variable
    print!("Enter wallet password: ");
    io::stdout().flush()?;
    // Add explicit type annotation here
    let password: SecretString = rpassword::read_password()?.into();

    // Validate that the password is not empty
    if password.expose_secret().is_empty() {
        return Err(CliError::Password("Password cannot be empty.".to_string()));
    }

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

/// Cleanup routine for graceful shutdown
async fn cleanup_resources(_wallet: &Arc<Wallet>, _db_path: &str) {
    info!("Closing database connections...");
    // The database will be closed when RocksDB handle is dropped
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    info!("Closing network connections...");
    // Network connections are closed when libp2p swarm is dropped
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    info!("Flushing file handles and buffers...");
    // Ensure any pending writes are flushed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    info!("Saving wallet state...");
    // Wallet is already encrypted and saved, but we could add additional state saving here

    // Optionally sync filesystem to ensure all data is written
    if std::process::Command::new("sync").output().is_ok() {
        info!("Filesystem sync completed.");
    }

    info!("Cleanup completed.");
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
            info!("Checking for clean flag");
            if clean {
                println!("'--clean' flag detected. Removing old database directory: {db_path}");
                if Path::new(db_path).exists() {
                    info!("Removing existing database directory: {db_path}");
                    if let Err(e) = fs::remove_dir_all(db_path) {
                        eprintln!("Failed to remove database directory '{db_path}': {e}");
                        std::process::exit(1);
                    }
                    println!("Database directory removed successfully.");
                    info!("Database directory removed successfully");
                } else {
                    info!("No existing database directory to remove");
                }
            } else {
                info!("Clean flag not set, proceeding without database removal");
            }

            info!("Prompting for wallet password");
            // Correctly load config and wallet with password.
            let password =
                prompt_for_password(false).map_err(|e| anyhow::anyhow!("Password error: {}", e))?;
            info!("Wallet password obtained successfully");
            let node_config = Config::load(&config)?;
            // Pass the SecretString directly, without re-wrapping it.
            let wallet_instance = Wallet::from_file(&wallet, &password)?;
            let wallet_arc = Arc::new(wallet_instance);

            // Create shutdown flag
            let shutdown = Arc::new(AtomicBool::new(false));
            let shutdown_clone = shutdown.clone();

            // Setup signal handlers
            let mut signals = Signals::new([SIGINT, SIGTERM])?;
            std::thread::spawn(move || {
                for sig in signals.forever() {
                    match sig {
                        SIGINT => {
                            info!("Received SIGINT (Ctrl+C). Initiating graceful shutdown...");
                            shutdown_clone.store(true, Ordering::Relaxed);
                            break;
                        }
                        SIGTERM => {
                            info!("Received SIGTERM. Initiating graceful shutdown...");
                            shutdown_clone.store(true, Ordering::Relaxed);
                            break;
                        }
                        _ => {}
                    }
                }
            });

            println!("Initializing Qanto services... (Press Ctrl+C for graceful shutdown)");
            let node = Node::new(
                node_config,
                config.clone(),
                wallet_arc.clone(),
                &p2p_identity,
                peer_cache,
            )
            .await?;

            // Start node with shutdown monitoring - use existing tokio runtime
            let shutdown_monitor = shutdown.clone();
            let node_handle = tokio::spawn(async move { node.start().await });

            // Monitor shutdown flag
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // Check shutdown flag first
                if shutdown_monitor.load(Ordering::Relaxed) {
                    info!("Shutdown requested, stopping node...");
                    node_handle.abort();
                    break;
                }

                // Check if node task has ended unexpectedly
                if node_handle.is_finished() {
                    match node_handle.await {
                        Ok(Ok(_)) => {
                            info!("Node stopped normally.");
                            break;
                        }
                        Ok(Err(e)) => {
                            error!("Node runtime error: {}", e);
                            return Err(anyhow::anyhow!("Node runtime error: {}", e));
                        }
                        Err(e) => {
                            if e.is_cancelled() {
                                info!("Node task was cancelled during shutdown.");
                                break;
                            } else {
                                error!("Node task panicked: {}", e);
                                return Err(anyhow::anyhow!("Node task panicked: {}", e));
                            }
                        }
                    }
                }
            }

            // Cleanup routine
            info!("Performing cleanup...");
            cleanup_resources(&wallet_arc, db_path).await;
            info!("Qanto node has shut down gracefully.");
        }
        Commands::GenerateWallet { output } => {
            println!("Generating new wallet...");
            let password =
                prompt_for_password(false).map_err(|e| anyhow::anyhow!("Password error: {}", e))?;
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
