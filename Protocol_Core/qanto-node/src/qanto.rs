// src/qanto.rs

use crate::config::Config;
use crate::diagnostics::{cli::DiagnosticsArgs, DiagnosticsEngine};
use crate::node::Node;
use crate::password_utils::prompt_for_password;
use crate::wallet::Wallet;
use anyhow::Result;
use clap::{Parser, Subcommand};
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about = "Qanto Node CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short,
        long,
        default_value = "config.toml",
        help = "Path to configuration file"
    )]
    config: String,
    #[arg(short, long, help = "Path to wallet key file (overrides config)")]
    wallet: Option<String>,
    #[arg(long, help = "Path to P2P identity key file (overrides config)")]
    p2p_identity: Option<String>,
    #[arg(long, help = "Path to peer cache file (overrides config)")]
    peer_cache: Option<String>,
    #[arg(long, help = "Data directory path (overrides config)")]
    data_dir: Option<String>,
    #[arg(long, help = "Database path (overrides config)")]
    db_path: Option<String>,
    #[arg(long, help = "Log file path (overrides config)")]
    log_file: Option<String>,
    #[arg(long, help = "TLS certificate path (overrides config)")]
    tls_cert: Option<String>,
    #[arg(long, help = "TLS private key path (overrides config)")]
    tls_key: Option<String>,
    /// Clean database on startup
    #[arg(long)]
    clean: bool,
    /// Enable debug mode with detailed logging
    #[arg(
        long,
        help = "Enable debug mode with detailed mining and strata logging"
    )]
    debug_mode: bool,
    /// Enable public mining mode
    #[arg(long, help = "Enable mining (overrides config)")]
    mine: bool,
    #[arg(long, help = "RPC port to bind to (overrides config)")]
    rpc_port: Option<u16>,
    #[arg(long, help = "P2P port to bind to (overrides config)")]
    p2p_port: Option<u16>,
    #[arg(long, help = "Listen multiaddress to bind to (overrides config)")]
    listen: Option<String>,
    #[arg(long, help = "Bootnode peer multiaddresses to connect to (overrides config)", num_args = 0..)]
    bootnode: Vec<String>,
    #[command(flatten)]
    diagnostics: Box<DiagnosticsArgs>,
}

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Starts the Qanto node
    Start,
    /// Generates a new wallet
    GenerateWallet {
        #[arg(
            short,
            long,
            default_value = "wallet.key",
            help = "Output path for wallet key file"
        )]
        output: String,
    },
}

pub async fn run() -> Result<()> {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("🚨 CRITICAL NODE EXCEPTION DETECTED: {}", info);
    }));

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .try_init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::GenerateWallet { output }) => {
            println!("Generating new wallet...");
            let password = prompt_for_password(true, None)
                .map_err(|e| anyhow::anyhow!("Password error: {e}"))?;
            let wallet = Wallet::new()?;
            // Correctly save the wallet with the SecretString.
            wallet.save_to_file(&output, &password)?;
            println!(
                "New wallet saved to {}. Address: {}",
                output,
                wallet.address()
            );
            println!("IMPORTANT: Please back up this file securely and remember your password.");
        }
        None | Some(Commands::Start) => {
            let config = cli.config;
            let wallet = cli.wallet;
            let p2p_identity = cli.p2p_identity;
            let peer_cache = cli.peer_cache;
            let data_dir = cli.data_dir;
            let db_path = cli.db_path;
            let log_file = cli.log_file;
            let tls_cert = cli.tls_cert;
            let tls_key = cli.tls_key;
            let clean = cli.clean;
            let debug_mode = cli.debug_mode;
            let mine = cli.mine;
            let rpc_port = cli.rpc_port;
            let p2p_port = cli.p2p_port;
            let listen = cli.listen;
            let bootnode = cli.bootnode;
            let diagnostics = cli.diagnostics;

            println!("Qanto node starting...");
            let config_path = Path::new(&config);
            if !config_path.exists() {
                eprintln!("Configuration file not found: {}", config_path.display());
                std::process::exit(1);
            }
            println!("Configuration loaded from '{}'.", config_path.display());

            let mut node_config = Config::load(&config)?;

            let bootnodes_opt = if bootnode.is_empty() {
                None
            } else {
                Some(bootnode)
            };

            // Apply CLI overrides including network overrides
            node_config.apply_cli_overrides(
                wallet,
                p2p_identity,
                data_dir,
                db_path,
                log_file,
                tls_cert,
                tls_key,
                None, // adaptive_mining_enabled not supported in this binary
                rpc_port,
                p2p_port,
                listen,
                bootnodes_opt,
            );

            if mine {
                node_config.mining_enabled = true;
            }

            let db_path = node_config.db_path.clone();

            info!("Checking for clean flag");
            if clean {
                println!("'--clean' flag detected. Removing old database directory: {db_path}");
                if Path::new(&db_path).exists() {
                    info!("Removing existing database directory: {db_path}");
                    if let Err(e) = fs::remove_dir_all(&db_path) {
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

            info!("Node running seamlessly without interactive password prompt.");
            // Use environment variable if set, otherwise default to empty password for seamless startup
            let password_str = std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| String::new());
            let password = secrecy::SecretString::new(password_str.trim().to_string());
            info!("Wallet password loaded (defaulting to empty if not provided)");

            // Use paths from config instead of hardcoded values
            let wallet_path = node_config.wallet_path.clone();
            let p2p_identity_path = node_config.p2p_identity_path.clone();

            // Load wallet or generate new one headlessly if it doesn't exist
            let wallet_instance = if !Path::new(&wallet_path).exists() {
                info!("Wallet file not found at {}. Generating a new one headlessly...", wallet_path);
                let new_wallet = Wallet::new()?;
                new_wallet.save_to_file(&wallet_path, &password)?;
                info!("Generated new wallet at {} with address: {}", wallet_path, new_wallet.address());
                node_config.genesis_validator = new_wallet.address();
                node_config.save(&config)?;
                info!("Updated genesis_validator in configuration file to match the generated wallet address.");
                new_wallet
            } else {
                Wallet::from_file(&wallet_path, &password)?
            };
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

            // Initialize diagnostics engine
            let diagnostics_engine = Arc::new(DiagnosticsEngine::new(debug_mode));

            // Process diagnostics CLI commands if any
            if diagnostics.show_mining_stats
                || diagnostics.analyze_issues
                || diagnostics.export_diagnostics.is_some()
            {
                crate::diagnostics::cli::process_diagnostics_commands(
                    &diagnostics,
                    &diagnostics_engine,
                )
                .await?;
                return Ok(());
            }

            if debug_mode {
                println!("🔍 Debug mode enabled - detailed logging active");
                info!("Debug mode enabled with comprehensive diagnostics");
            }

            // Default peer cache under the node data dir so multiple local nodes do not collide.
            let peer_cache_path =
                peer_cache.unwrap_or_else(|| format!("{}/peer_cache.json", node_config.data_dir));

            let node = Node::new(
                node_config,
                config.clone(),
                wallet_arc.clone(),
                &p2p_identity_path,
                peer_cache_path,
            )
            .await?;

            // Hold the node in an Arc so we can trigger shutdown while it runs
            let node = Arc::new(node);

            // Start background DAG preloading task (non-blocking)
            let qdag_gen_for_preload = node.dag.qdag_generator.clone();
            let shutdown_for_preload = shutdown.clone();
            tokio::spawn(async move {
                info!("🚀 Starting background DAG preloading for epochs 0-9...");
                for epoch in 0..10 {
                    if shutdown_for_preload.load(Ordering::Relaxed) {
                        info!("⚠️ Shutdown requested during DAG preloading. Aborting preloading.");
                        break;
                    }

                    let qdag_gen = qdag_gen_for_preload.clone();
                    let handle = tokio::task::spawn_blocking(move || {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(qdag_gen.get_qdag(epoch as u64))
                    });

                    match handle.await {
                        Ok(_) => {
                            info!(
                                "✅ Background DAG preloading for epoch {}/9 complete",
                                epoch
                            );
                        }
                        Err(e) => {
                            error!("❌ Failed to preload DAG for epoch {}: {}", epoch, e);
                        }
                    }
                }
                info!("🎯 Background DAG preloading completed for all epochs");
            });

            info!(
                "🌟 Node services starting immediately (DAG preloading continues in background)..."
            );

            // Start node with shutdown monitoring using tokio::select!
            let node_for_task = node.clone();
            let node_handle = tokio::spawn(async move { node_for_task.start().await });

            // Wire OS signals to the coordinated cancellation token
            let cleanup_for_signals = node.resource_cleanup.clone();
            let shutdown_signal_flag = shutdown.clone();
            std::thread::spawn(move || {
                let mut signals =
                    Signals::new([SIGINT, SIGTERM]).expect("Failed to register signal handlers");
                for sig in signals.forever() {
                    match sig {
                        SIGINT => {
                            info!("Received SIGINT (Ctrl+C). Initiating graceful shutdown...");
                            shutdown_signal_flag.store(true, Ordering::Relaxed);
                            cleanup_for_signals.request_shutdown();
                            break;
                        }
                        SIGTERM => {
                            info!("Received SIGTERM. Initiating graceful shutdown...");
                            shutdown_signal_flag.store(true, Ordering::Relaxed);
                            cleanup_for_signals.request_shutdown();
                            break;
                        }
                        _ => {}
                    }
                }
            });

            // Concurrently await either cancellation or node task completion
            let shutdown_token = node.resource_cleanup.get_cancellation_token();
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    info!("Shutdown requested via cancellation token; stopping node...");
                    // Unconditionally await the node's shutdown to completion
                    info!("Waiting for node shutdown to complete...");
                    if let Err(e) = node.shutdown().await {
                        error!("Node shutdown error: {}", e);
                    } else {
                        info!("Node shutdown completed successfully.");
                    }
                }
                res = node_handle => {
                    match res {
                        Ok(Ok(_)) => {
                            info!("Node stopped normally.");
                            // Still ensure proper shutdown even if node stopped normally
                            if let Err(e) = node.shutdown().await {
                                error!("Node shutdown error: {}", e);
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Node runtime error: {}", e);
                            // Ensure shutdown even on error
                            if let Err(shutdown_err) = node.shutdown().await {
                                error!("Node shutdown error after runtime error: {}", shutdown_err);
                            }
                            return Err(anyhow::anyhow!("Node runtime error: {e}"));
                        }
                        Err(e) => {
                            if e.is_cancelled() {
                                info!("Node task was cancelled.");
                            } else {
                                error!("Node task panicked: {}", e);
                            }
                            // Ensure shutdown even on panic/cancellation
                            if let Err(shutdown_err) = node.shutdown().await {
                                error!("Node shutdown error after task failure: {}", shutdown_err);
                            }
                            if !e.is_cancelled() {
                                return Err(anyhow::anyhow!("Node task panicked: {e}"));
                            }
                        }
                    }
                }
            }

            info!("Qanto node has shut down gracefully.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
