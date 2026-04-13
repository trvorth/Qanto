use crate::{config::Config, node::Node, wallet::Wallet};
use clap::{Arg, ArgAction, Command};
use secrecy::SecretString;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

#[cfg(feature = "dhat-heap")]
use dhat;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    let matches = Command::new("start_node")
        .about("Start a Qanto node")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.toml"),
        )
        .arg(
            Arg::new("wallet")
                .short('w')
                .long("wallet")
                .value_name("FILE")
                .help("Wallet file path (overrides config)"),
        )
        .arg(
            Arg::new("data-dir")
                .short('d')
                .long("data-dir")
                .value_name("DIR")
                .help("Data directory path (overrides config)"),
        )
        .arg(
            Arg::new("db-path")
                .long("db-path")
                .value_name("PATH")
                .help("Database path (overrides config)"),
        )
        .arg(
            Arg::new("log-file")
                .long("log-file")
                .value_name("FILE")
                .help("Log file path (overrides config)"),
        )
        .arg(
            Arg::new("mine")
                .long("mine")
                .action(clap::ArgAction::SetTrue)
                .help("Enable mining (default: true, use --no-mine for light client mode)"),
        )
        .arg(
            Arg::new("no-mine")
                .long("no-mine")
                .help("Disable mining")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("adaptive-mining")
                .long("adaptive-mining")
                .help("Enable adaptive mining with difficulty adjustments")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-adaptive-mining")
                .long("no-adaptive-mining")
                .help("Disable adaptive mining")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("celebrate")
                .long("celebrate")
                .help("Enable block celebration logging (overrides config)")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").unwrap();
    let mut config = Config::load(config_path)?;

    // Apply CLI overrides
    let adaptive_mining_flag = if matches.get_flag("adaptive-mining") {
        Some(true)
    } else if matches.get_flag("no-adaptive-mining") {
        Some(false)
    } else {
        None
    };

    // Handle celebrate flag
    if matches.get_flag("celebrate") {
        config.logging.enable_block_celebrations = true;
    }

    config.apply_cli_overrides(
        matches.get_one::<String>("wallet").cloned(),
        None, // p2p_identity not supported in this binary
        matches.get_one::<String>("data-dir").cloned(),
        matches.get_one::<String>("db-path").cloned(),
        matches.get_one::<String>("log-file").cloned(),
        None, // tls_cert not supported in this binary
        None, // tls_key not supported in this binary
        adaptive_mining_flag,
    );

    // Initialize tracing (console + optional file) using config.logging.level
    let env_filter =
        EnvFilter::try_new(config.logging.level.clone()).unwrap_or_else(|_| EnvFilter::new("info"));
    let console_layer = fmt::layer().with_target(true);

    if let Some(ref log_path) = config.log_file_path {
        if let Some(parent) = Path::new(log_path).parent() {
            // Ensure parent directory exists for log file
            let _ = create_dir_all(parent);
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        let file = Arc::new(Mutex::new(file));
        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_writer(move || LockedWriter { file: file.clone() });
        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .init();
    }

    // Validate configuration addresses
    if config.genesis_validator.len() != 64 || hex::decode(&config.genesis_validator).is_err() {
        return Err(format!(
            "Invalid genesis_validator address format: {}",
            config.genesis_validator
        )
        .into());
    }
    if config.contract_address.len() != 64 || hex::decode(&config.contract_address).is_err() {
        return Err(format!(
            "Invalid contract_address format: {}",
            config.contract_address
        )
        .into());
    }

    // Check mining configuration
    let mining_enabled = if matches.get_flag("no-mine") {
        false // Light client mode
    } else {
        // Default to true if neither flag is set
        // If --mine is provided, use it; otherwise default to true
        matches.get_flag("mine") || !matches.get_flag("no-mine")
    };

    // Set test difficulty for easier mining
    config.difficulty = 1; // Set to minimum allowed difficulty for testing

    // Apply mining flag to config
    config.mining_enabled = mining_enabled;

    let mode = if mining_enabled {
        "full node"
    } else {
        "light client"
    };

    println!("Configuration loaded successfully:");
    println!(
        "  Genesis Validator: {genesis}",
        genesis = config.genesis_validator
    );
    println!(
        "  Contract Address: {contract}",
        contract = config.contract_address
    );
    println!(
        "  Test Difficulty: {difficulty}",
        difficulty = config.difficulty
    );
    println!(
        "  Mining enabled: {enabled} ({mode})",
        enabled = config.mining_enabled,
        mode = mode
    );

    // Check for WALLET_PASSWORD environment variable first
    let pass = std::env::var("WALLET_PASSWORD").unwrap_or_else(|_| {
        // Fallback to interactive prompt if no environment variable
        rpassword::prompt_password("Enter wallet password: ").expect("Failed to read password")
    });

    let password = SecretString::new(pass);
    let wallet = Wallet::from_file(&config.wallet_path, &password)?;
    let wallet = Arc::new(wallet);

    let node = Node::new(
        config.clone(),
        config_path.to_string(),
        wallet,
        &config.p2p_identity_path,
        "start_node_peer_cache.json".to_string(),
    )
    .await?;
    node.start().await?;

    Ok(())
}

// Custom writer that safely writes to a shared File using a mutex
struct LockedWriter {
    file: Arc<Mutex<std::fs::File>>,
}

impl Write for LockedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut f = self
            .file
            .lock()
            .map_err(|_| io::Error::other("log file mutex poisoned"))?;
        f.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut f = self
            .file
            .lock()
            .map_err(|_| io::Error::other("log file mutex poisoned"))?;
        f.flush()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
