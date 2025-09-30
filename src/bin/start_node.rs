use clap::{Arg, Command};
use qanto::{config::Config, node::Node, wallet::Wallet};
use secrecy::SecretString;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        .get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").unwrap();
    let mut config = Config::load(config_path)?;

    // Apply CLI overrides
    config.apply_cli_overrides(
        matches.get_one::<String>("wallet").cloned(),
        None, // p2p_identity not supported in this binary
        matches.get_one::<String>("data-dir").cloned(),
        matches.get_one::<String>("db-path").cloned(),
        matches.get_one::<String>("log-file").cloned(),
        None, // tls_cert not supported in this binary
        None, // tls_key not supported in this binary
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
