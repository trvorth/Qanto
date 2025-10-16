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
// use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(author, version, about = "Qanto Node CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Starts the Qanto node
    Start {
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
        #[command(flatten)]
        diagnostics: Box<DiagnosticsArgs>,
    },
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

pub async fn run() -> Result<()> {
    // Initialize logging with env_logger to support RUST_LOG environment variable
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            config,
            wallet,
            p2p_identity,
            peer_cache,
            data_dir,
            db_path,
            log_file,
            tls_cert,
            tls_key,
            clean,
            debug_mode,
            diagnostics,
        } => {
            println!("Qanto node starting...");
            let config_path = Path::new(&config);
            if !config_path.exists() {
                eprintln!("Configuration file not found: {}", config_path.display());
                std::process::exit(1);
            }
            println!("Configuration loaded from '{}'.", config_path.display());

            let mut node_config = Config::load(&config)?;

            // Apply CLI path overrides using the new method
            node_config.apply_cli_overrides(
                wallet,
                p2p_identity,
                data_dir,
                db_path,
                log_file,
                tls_cert,
                tls_key,
                None, // adaptive_mining_enabled not supported in this binary
            );

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

            info!("Prompting for wallet password");
            // Correctly load config and wallet with password.
            let password = prompt_for_password(false, None)
                .map_err(|e| anyhow::anyhow!("Password error: {e}"))?;
            info!("Wallet password obtained successfully");

            // Use paths from config instead of hardcoded values
            let wallet_path = node_config.wallet_path.clone();
            let p2p_identity_path = node_config.p2p_identity_path.clone();

            // Pass the SecretString directly, without re-wrapping it.
            let wallet_instance = Wallet::from_file(&wallet_path, &password)?;
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

            // Use peer_cache from CLI or default
            let peer_cache_path = peer_cache.unwrap_or_else(|| "peer_cache.json".to_string());

            let node = Node::new(
                node_config,
                config.clone(),
                wallet_arc.clone(),
                &p2p_identity_path,
                peer_cache_path,
            )
            .await?;

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
                            return Err(anyhow::anyhow!("Node runtime error: {e}"));
                        }
                        Err(e) => {
                            if e.is_cancelled() {
                                info!("Node task was cancelled during shutdown.");
                                break;
                            } else {
                                error!("Node task panicked: {}", e);
                                return Err(anyhow::anyhow!("Node task panicked: {e}"));
                            }
                        }
                    }
                }
            }

            // Cleanup routine
            info!("Performing cleanup...");
            cleanup_resources(&wallet_arc, &db_path).await;
            info!("Qanto node has shut down gracefully.");
        }
        Commands::GenerateWallet { output } => {
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
