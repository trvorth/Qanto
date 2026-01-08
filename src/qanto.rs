// src/qanto.rs

use crate::config::Config;
use crate::diagnostics::{cli::DiagnosticsArgs, DiagnosticsEngine};
use crate::node::Node;
use crate::node_keystore::Wallet;
use crate::password_utils::prompt_for_password;
use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info};
// use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Qanto Node CLI",
    long_about = None,
    args_conflicts_with_subcommands = true,
    subcommand_required = false
)]
struct Cli {
    #[command(flatten)]
    start: StartArgs,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Args, Debug)]
struct StartArgs {
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
    #[arg(long)]
    clean: bool,
    #[arg(
        long,
        help = "Enable debug mode with detailed mining and strata logging"
    )]
    debug_mode: bool,
    #[command(flatten)]
    diagnostics: Box<DiagnosticsArgs>,
}

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Starts the Qanto node
    Start(StartArgs),
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
    // Initialize logging with env_logger to support RUST_LOG environment variable
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    async fn run_start(start: StartArgs) -> Result<()> {
        let StartArgs {
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
        } = start;

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
        let data_dir = node_config.data_dir.clone();

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

            if data_dir != "/" && data_dir != "." && !data_dir.is_empty() {
                println!("Removing old data directory: {data_dir}");
                if Path::new(&data_dir).exists() {
                    info!("Removing existing data directory: {data_dir}");
                    if let Err(e) = fs::remove_dir_all(&data_dir) {
                        eprintln!("Failed to remove data directory '{data_dir}': {e}");
                        std::process::exit(1);
                    }
                    println!("Data directory removed successfully.");
                    info!("Data directory removed successfully");
                } else {
                    info!("No existing data directory to remove");
                }
            }
        } else {
            info!("Clean flag not set, proceeding without database removal");
        }

        let password = if let Ok(env_pw) = std::env::var("QANTO_WALLET_PASSWORD") {
            secrecy::SecretString::new(env_pw.trim().to_string())
        } else {
            prompt_for_password(false, None).map_err(|e| anyhow::anyhow!("Password error: {e}"))?
        };

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

        // Hold the node in an Arc so we can trigger shutdown while it runs
        let node = Arc::new(node);

        // Start background DAG preloading task (non-blocking)
        // Narrow initial scope to epoch 0 only to reduce startup memory.
        let qdag_gen_for_preload = node.dag.qdag_generator.clone();
        let shutdown_for_preload = shutdown.clone();
        tokio::spawn(async move {
            info!("🚀 Starting background DAG preloading for epoch 0 only...");
            for epoch in 0..1 {
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
                        info!("✅ Background DAG preloading for epoch {} complete", epoch);
                    }
                    Err(e) => {
                        error!("❌ Failed to preload DAG for epoch {}: {}", epoch, e);
                    }
                }
            }
            info!("🎯 Background DAG preloading completed for epoch 0");
        });

        // Deferred preloading for epochs 1–9 driven by node readiness
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let node_for_deferred = node.clone();
        let shutdown_for_deferred = shutdown.clone();
        tokio::spawn(async move {
            const SYNC_AGE_SECS: u64 = 3600; // Aligns with node readiness criteria
            info!("⏳ Deferring DAG preloading for epochs 1–9 until node is ready...");

            loop {
                if shutdown_for_deferred.load(Ordering::Relaxed) {
                    info!("🛑 Shutdown requested before deferred preloading. Skipping remaining epochs.");
                    return;
                }

                // Readiness conditions: sufficient blocks, non-empty UTXO set, recent tip age
                let dag = &node_for_deferred.dag;
                let utxos_guard = node_for_deferred.utxos.read().await;
                let block_count = dag.get_block_count().await;
                let latest_ts = dag
                    .get_latest_block()
                    .await
                    .map(|b| b.timestamp)
                    .unwrap_or(0);
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let is_synced = now.saturating_sub(latest_ts) <= SYNC_AGE_SECS;
                let is_ready = block_count >= 2 && !utxos_guard.is_empty() && is_synced;
                drop(utxos_guard);

                if is_ready {
                    info!(
                        "✅ Node readiness satisfied; continuing DAG preloading for epochs 1–9..."
                    );
                    break;
                }

                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            for epoch in 1..10 {
                if shutdown_for_deferred.load(Ordering::Relaxed) {
                    info!("🛑 Shutdown requested during deferred preloading. Aborting.");
                    break;
                }

                let qdag_gen = node_for_deferred.dag.qdag_generator.clone();
                let handle = tokio::task::spawn_blocking(move || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(qdag_gen.get_qdag(epoch as u64))
                });

                match handle.await {
                    Ok(_) => {
                        info!("📦 Deferred DAG preloading complete for epoch {}", epoch);
                    }
                    Err(e) => {
                        error!("❌ Deferred preload failed for epoch {}: {}", epoch, e);
                    }
                }
            }

            info!("🎯 Deferred DAG preloading finished for epochs 1–9");
        });

        info!("🌟 Node services starting immediately (DAG preloading continues in background)...");

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
        Ok(())
    }

    match cli.command {
        Some(Commands::Start(start)) => run_start(start).await,
        Some(Commands::GenerateWallet { output }) => {
            println!("Generating new wallet...");
            let password = if let Ok(env_pw) = std::env::var("QANTO_WALLET_PASSWORD") {
                secrecy::SecretString::new(env_pw.trim().to_string())
            } else {
                prompt_for_password(true, None)
                    .map_err(|e| anyhow::anyhow!("Password error: {e}"))?
            };
            let wallet = Wallet::new()?;
            // Correctly save the wallet with the SecretString.
            wallet.save_to_file(&output, &password)?;
            println!(
                "New wallet saved to {}. Address: {}",
                output,
                wallet.address()
            );
            println!("IMPORTANT: Please back up this file securely and remember your password.");
            Ok(())
        }
        None => run_start(cli.start).await,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
