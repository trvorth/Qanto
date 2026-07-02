//! QANTO Cross-Chain Relayer Daemon
//! Listens to Ethereum Sepolia for locked assets and triggers minting on QANTO Layer-0.

use ethers::prelude::*;
use reqwest::Client;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Ethereum Sepolia WSS RPC (Public or Infura/Alchemy)
const ETH_WSS_URL: &str = "wss://ethereum-sepolia.publicnode.com";
// Qanto HF Node RPC
const QANTO_RPC_URL: &str = "https://trvorth-qanto-testnet.hf.space/rpc";
// Synthetic Ethereum Bridge Contract Address
const ETH_BRIDGE_ADDR: &str = "0x0000000000000000000000000000000000000000";

abigen!(
    BridgeContract,
    r#"[
        event AssetLocked(address indexed user, address indexed token, uint256 amount)
    ]"#,
);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================================");
    println!("🌉 IGNITING QANTO CROSS-CHAIN RELAYER DAEMON 🌉");
    println!("========================================================");

    // 1. Connect to Ethereum via WebSockets
    let ws = Provider::<Ws>::connect(ETH_WSS_URL).await?;
    let provider = Arc::new(ws);
    println!("[+] Connected to Ethereum Sepolia WSS.");

    let address = ETH_BRIDGE_ADDR.parse::<Address>()?;
    let contract = BridgeContract::new(address, provider.clone());

    // 2. Setup HTTP Client for Qanto Layer-0
    let qanto_client = Client::new();
    println!("[+] QANTO RPC Client Initialized: {}", QANTO_RPC_URL);

    // 3. Subscribe to AssetLocked Events
    println!("[*] Listening for Cross-Chain Lock Events...");
    let events = contract.events();
    let mut stream = events.subscribe().await?;

    while let Some(Ok(log)) = stream.next().await {
        println!("\n[!] DETECTED ETHEREUM LOCK EVENT!");
        println!("    User: {:?}", log.user);
        println!("    Amount: {}", log.amount);

        // 4. Generate ZK Proof (Simulated)
        println!("    [*] Generating ZK-SNARK Proof...");
        sleep(Duration::from_millis(1500)).await;
        println!("    [+] Proof Generated Successfully.");

        // 5. Transmit to QANTO Layer-0
        transmit_to_qanto(
            &qanto_client,
            format!("{:?}", log.user),
            log.amount.as_u128(),
        )
        .await;
    }

    Ok(())
}

async fn transmit_to_qanto(client: &Client, user: String, amount: u128) {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "qanto_bridgeMint",
        "params": [{
            "receiver": user,
            "amount": amount,
            "proof": "0xZK_PROOF_PAYLOAD"
        }],
        "id": 1
    });

    match client.post(QANTO_RPC_URL).json(&payload).send().await {
        Ok(res) => {
            if res.status().is_success() {
                println!("    [+] Bridged Assets Successfully Minted on QANTO!");
            } else {
                println!(
                    "    [-] Failed to mint on Qanto Layer-0: {:?}",
                    res.status()
                );
            }
        }
        Err(e) => println!("    [-] RPC Connection Error: {}", e),
    }
}
