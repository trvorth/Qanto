use anyhow::Result;
use crate::qanto_compat::ed25519_dalek::SigningKey;
use log::info;
use qanto::config::Config;
use qanto::node::Node;
use qanto::wallet::Wallet;
use rand::rngs::OsRng;
use std::env;
use std::fs;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let private_key_hex = env::var("QANTODAG_PRIVATE_KEY").unwrap_or_else(|_| {
        info!("QANTODAG_PRIVATE_KEY not set, generating a new wallet for testnet.");
        let mut os_rng = OsRng;
        let signing_key = SigningKey::generate(&mut os_rng);
        hex::encode(signing_key.to_bytes())
    });
    let wallet = Wallet::from_private_key(&private_key_hex)?;
    info!("Testnet node wallet address: {address}", address = wallet.address());

    let config = Config {
        p2p_address: "/ip4/127.0.0.1/tcp/0".to_string(),
        local_full_p2p_address: None,
        api_address: "127.0.0.1:0".to_string(),
        network_id: "qantodag-testnet".to_string(),
        peers: vec![],
        genesis_validator: wallet.address(),
        target_block_time: 60,
        difficulty: 10,
        max_amount: 21_000_000_000,
        use_gpu: false,
        zk_enabled: false,
        mining_threads: 1,
        num_chains: 1,
        mining_chain_id: 0,
        logging: qanto::config::LoggingConfig {
            level: "info".to_string(),
            enable_block_celebrations: true, // Enable celebrations in testnet
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: None,
        },
        p2p: qanto::config::P2pConfig::default(),
    };
    config.validate()?;

    let temp_config_path = format!("./temp_testnet_config_{}.toml", rand::random::<u32>());
    config.save(&temp_config_path)?;

    let temp_identity_path = format!("./temp_testnet_identity_{}.key", rand::random::<u32>());
    let temp_peer_cache_path = format!("./temp_testnet_peercache_{}.json", rand::random::<u32>());

    info!("Starting QantoDAG testnet node instance (from qantodag_testnet.rs)...");
    let wallet_arc = Arc::new(wallet);
    let node = Node::new(
        config,
        temp_config_path.clone(),
        wallet_arc,
        &temp_identity_path,
        temp_peer_cache_path.clone(),
    )
    .await?;

    let node_start_result = node.start().await;

    // Clean up the temporary files
    let _ = fs::remove_file(&temp_config_path);
    let _ = fs::remove_file(&temp_identity_path);
    let _ = fs::remove_file(&temp_peer_cache_path);

    if let Err(e) = node_start_result {
        log::error!("QantoDAG testnet node (from qantodag_testnet.rs) failed: {e}");
        return Err(e.into());
    }

    info!("QantoDAG testnet node (from qantodag_testnet.rs) exited.");
    Ok(())
}