//! QANTO Cross-Chain Relayer Daemon
//! Listens for external bridge lock events and submits validated claim payloads to QANTO Layer-0.

use std::env;
use std::str::FromStr;
use std::sync::Arc;

use ethers::prelude::*;
use ethers::types::H256;
use futures_util::StreamExt;
use qanto_zk_sdk::{
    BridgeClaimRequest, BridgeEventWitness, bridge_claim_signing_hash, build_bridge_claim_request,
    generate_bridge_proof_bundle,
};
use reqwest::Client;
use serde_json::json;

abigen!(
    BridgeContract,
    r#"[
        event AssetLocked(address indexed user, address indexed token, uint256 amount)
    ]"#,
);

#[derive(Debug, Clone)]
struct RelayerConfig {
    eth_ws_url: String,
    qanto_rpc_url: String,
    bridge_address: Address,
    source_chain: String,
    relayer_wallet: LocalWallet,
}

impl RelayerConfig {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let eth_ws_url = env::var("QANTO_RELAYER_ETH_WS_URL")?;
        let qanto_rpc_url = env::var("QANTO_RELAYER_QANTO_RPC_URL")?;
        let bridge_address =
            Address::from_str(&env::var("QANTO_RELAYER_ETH_BRIDGE_ADDRESS")?)?;
        let source_chain = env::var("QANTO_RELAYER_SOURCE_CHAIN")?;
        let relayer_wallet = LocalWallet::from_str(&env::var("QANTO_RELAYER_PRIVATE_KEY")?)?;

        Ok(Self {
            eth_ws_url,
            qanto_rpc_url,
            bridge_address,
            source_chain,
            relayer_wallet,
        })
    }
}

fn signature_to_hex(signature: &Signature) -> String {
    let mut sig_bytes = [0u8; 65];
    signature.r.to_big_endian(&mut sig_bytes[..32]);
    signature.s.to_big_endian(&mut sig_bytes[32..64]);
    sig_bytes[64] = signature.v as u8;
    format!("0x{}", hex::encode(sig_bytes))
}

fn build_claim_request(
    config: &RelayerConfig,
    user: Address,
    token: Address,
    amount: U256,
    meta: &LogMeta,
) -> Result<BridgeClaimRequest, Box<dyn std::error::Error>> {
    let amount_u128: u128 = amount
        .try_into()
        .map_err(|_| "bridge amount exceeds u128 range")?;
    let witness = BridgeEventWitness {
        source_chain: config.source_chain.clone(),
        source_tx_hash: format!("{:#x}", meta.transaction_hash),
        recipient: format!("{:#x}", user),
        amount: amount_u128.to_string(),
        token_address: format!("{:#x}", token),
        block_hash: format!("{:#x}", meta.block_hash),
        block_number: meta.block_number.as_u64(),
        log_index: meta.log_index.as_u64(),
    };
    let proof_bundle = generate_bridge_proof_bundle(&witness)?;
    let signing_hash = bridge_claim_signing_hash(
        &witness.source_chain,
        &witness.source_tx_hash,
        &witness.amount,
        &witness.recipient,
    );
    let signature = config
        .relayer_wallet
        .sign_hash(H256::from(signing_hash))?;
    let signature_hex = signature_to_hex(&signature);

    build_bridge_claim_request(&witness, vec![signature_hex], &proof_bundle)
        .map_err(|err| err.into())
}

async fn transmit_to_qanto(
    client: &Client,
    rpc_url: &str,
    claim_request: &BridgeClaimRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "qanto_bridgeMint",
        "params": [claim_request],
        "id": 1
    });

    let response = client.post(rpc_url).json(&payload).send().await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;

    if !status.is_success() {
        return Err(format!("qanto RPC HTTP error {}: {}", status, body).into());
    }
    if let Some(error) = body.get("error") {
        return Err(format!("qanto RPC bridge claim rejected: {}", error).into());
    }

    body.get("result")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("missing bridge claim result in RPC response: {}", body).into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RelayerConfig::from_env()?;

    println!("========================================================");
    println!("🌉 IGNITING QANTO CROSS-CHAIN RELAYER DAEMON 🌉");
    println!("========================================================");
    println!("[+] Source chain: {}", config.source_chain);
    println!("[+] QANTO RPC: {}", config.qanto_rpc_url);
    println!("[+] Bridge contract: {:#x}", config.bridge_address);
    println!("[+] Relayer signer: {:#x}", config.relayer_wallet.address());

    let ws = Provider::<Ws>::connect(&config.eth_ws_url).await?;
    let provider = Arc::new(ws);
    let contract = BridgeContract::new(config.bridge_address, provider);
    let client = Client::new();

    println!("[*] Listening for cross-chain lock events...");
    let events = contract.events();
    let mut stream = events.subscribe_with_meta().await?;

    while let Some(event) = stream.next().await {
        match event {
            Ok((log, meta)) => {
                println!("\n[!] DETECTED LOCK EVENT");
                println!("    User: {:#x}", log.user);
                println!("    Token: {:#x}", log.token);
                println!("    Amount (base units): {}", log.amount);
                println!("    Source tx: {:#x}", meta.transaction_hash);

                let claim_request =
                    build_claim_request(&config, log.user, log.token, log.amount, &meta)?;
                let claim_tx =
                    transmit_to_qanto(&client, &config.qanto_rpc_url, &claim_request).await?;

                println!("    [+] QANTO bridge claim accepted: {}", claim_tx);
            }
            Err(err) => {
                eprintln!("    [-] Relayer stream error: {}", err);
            }
        }
    }

    Ok(())
}
