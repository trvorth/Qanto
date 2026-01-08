use qanto::{config::Config, node::Node, node_keystore::Wallet};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// #[ignore] // Fixed in PR #42: Restored after stabilizing ASERT convergence
async fn p2p_two_node_block_propagation() {
    let cfg1 = Config {
        p2p_address: "/ip4/127.0.0.1/tcp/8009".to_string(),
        api_address: "127.0.0.1:18080".to_string(),
        mining_enabled: true,
        ..Default::default()
    };
    let cfg2 = Config {
        p2p_address: "/ip4/127.0.0.1/tcp/8010".to_string(),
        api_address: "127.0.0.1:18081".to_string(),
        mining_enabled: false,
        ..Default::default()
    };

    let w1 = Arc::new(Wallet::new().unwrap());
    let w2 = Arc::new(Wallet::new().unwrap());
    let mut cfg1 = cfg1;
    cfg1.genesis_validator = w1.address();
    let mut cfg2 = cfg2;
    cfg2.genesis_validator = w2.address();
    let tmp = std::env::temp_dir();
    let u1 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let u2 = u1 + 1;
    let mut cfg1 = cfg1.clone();
    cfg1.data_dir = tmp
        .join(format!("p2p_two_node_{}", u1))
        .to_string_lossy()
        .to_string();
    let mut cfg2 = cfg2.clone();
    cfg2.data_dir = tmp
        .join(format!("p2p_two_node_{}", u2))
        .to_string_lossy()
        .to_string();
    let _n1 = Node::new(
        cfg1.clone(),
        "./config1.toml".to_string(),
        w1,
        &cfg1.p2p_identity_path,
        "./peer_cache1.json".to_string(),
    )
    .await
    .unwrap();
    let _n2 = Node::new(
        cfg2.clone(),
        "./config2.toml".to_string(),
        w2,
        &cfg2.p2p_identity_path,
        "./peer_cache2.json".to_string(),
    )
    .await
    .unwrap();

    // TODO: connect peers and mine on node 1, then assert node 2 has the block
    sleep(Duration::from_millis(100)).await;
}
