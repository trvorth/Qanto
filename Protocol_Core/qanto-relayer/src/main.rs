//! QANTO Cross-Chain Relayer Daemon
//! Polls an EVM JSON-RPC endpoint for bridge lock events and forwards validated claim payloads.

use std::collections::HashSet;
use std::env;
use std::time::Duration;

use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use qanto_zk_sdk::{
    bridge_claim_signing_hash, build_bridge_claim_request, generate_bridge_proof_bundle,
    BridgeClaimRequest, BridgeEventWitness,
};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};
use tokio::time::sleep;

type DynError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
struct RelayerConfig {
    eth_rpc_url: String,
    qanto_rpc_url: String,
    bridge_address: String,
    source_chain: String,
    relayer_signing_key: SigningKey,
    poll_interval_secs: u64,
    confirmations: u64,
    start_block: Option<u64>,
}

impl RelayerConfig {
    fn from_env() -> Result<Self, DynError> {
        let eth_rpc_url = env::var("QANTO_RELAYER_ETH_RPC_URL")
            .or_else(|_| env::var("QANTO_RELAYER_ETH_WS_URL"))?;
        let qanto_rpc_url = env::var("QANTO_RELAYER_QANTO_RPC_URL")?;
        let bridge_address = normalize_address(&env::var("QANTO_RELAYER_ETH_BRIDGE_ADDRESS")?)?;
        let source_chain = env::var("QANTO_RELAYER_SOURCE_CHAIN")?;
        let relayer_signing_key = parse_signing_key(&env::var("QANTO_RELAYER_PRIVATE_KEY")?)?;
        let poll_interval_secs = env::var("QANTO_RELAYER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(5);
        let confirmations = env::var("QANTO_RELAYER_CONFIRMATIONS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1);
        let start_block = env::var("QANTO_RELAYER_START_BLOCK")
            .ok()
            .and_then(|value| value.parse::<u64>().ok());

        Ok(Self {
            eth_rpc_url,
            qanto_rpc_url,
            bridge_address,
            source_chain,
            relayer_signing_key,
            poll_interval_secs,
            confirmations,
            start_block,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthLog {
    address: String,
    topics: Vec<String>,
    data: String,
    block_hash: Option<String>,
    block_number: Option<String>,
    transaction_hash: Option<String>,
    log_index: Option<String>,
    #[serde(default)]
    removed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthTransactionReceipt {
    status: Option<String>,
    logs: Vec<EthLog>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthBlockHeader {
    hash: Option<String>,
    number: Option<String>,
}

#[derive(Debug, Clone)]
struct AssetLockedEvent {
    user: String,
    token: String,
    amount: u128,
    transaction_hash: String,
    block_hash: String,
    block_number: u64,
    log_index: u64,
}

fn strip_hex_prefix(value: &str) -> &str {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
}

fn normalize_hex(value: &str) -> Result<String, DynError> {
    let stripped = strip_hex_prefix(value);
    if stripped.is_empty()
        || stripped.len() % 2 != 0
        || !stripped.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return Err(format!("invalid hex string: {value}").into());
    }
    Ok(format!("0x{}", stripped.to_lowercase()))
}

fn normalize_address(value: &str) -> Result<String, DynError> {
    let stripped = strip_hex_prefix(value);
    if stripped.len() != 40 || !stripped.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("invalid 20-byte address: {value}").into());
    }
    Ok(format!("0x{}", stripped.to_lowercase()))
}

fn parse_signing_key(value: &str) -> Result<SigningKey, DynError> {
    let bytes = hex::decode(strip_hex_prefix(value))?;
    if bytes.len() != 32 {
        return Err(format!("relayer private key must be 32 bytes, got {}", bytes.len()).into());
    }
    Ok(SigningKey::from_slice(&bytes)?)
}

fn signing_key_address(signing_key: &SigningKey) -> String {
    let verifying_key = VerifyingKey::from(signing_key);
    let binding = verifying_key.to_encoded_point(false);
    let pub_bytes = binding.as_bytes();
    let mut hasher = Keccak256::new();
    hasher.update(&pub_bytes[1..]);
    let digest = hasher.finalize();
    format!("0x{}", hex::encode(&digest[12..]))
}

fn parse_hex_u64(value: &str) -> Result<u64, DynError> {
    Ok(u64::from_str_radix(strip_hex_prefix(value), 16)?)
}

fn parse_uint256_u128(data: &str) -> Result<u128, DynError> {
    let bytes = hex::decode(strip_hex_prefix(data))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32-byte uint256 payload, got {}", bytes.len()).into());
    }
    if bytes[..16].iter().any(|byte| *byte != 0) {
        return Err("bridge amount exceeds u128 range".into());
    }

    let mut amount_bytes = [0u8; 16];
    amount_bytes.copy_from_slice(&bytes[16..]);
    Ok(u128::from_be_bytes(amount_bytes))
}

fn decode_topic_address(topic: &str) -> Result<String, DynError> {
    let bytes = hex::decode(strip_hex_prefix(topic))?;
    if bytes.len() != 32 {
        return Err(format!("indexed topic must be 32 bytes, got {}", bytes.len()).into());
    }
    Ok(format!("0x{}", hex::encode(&bytes[12..])))
}

fn asset_locked_event_topic() -> String {
    let mut hasher = Keccak256::new();
    hasher.update(b"AssetLocked(address,address,uint256)");
    format!("0x{}", hex::encode(hasher.finalize()))
}

async fn rpc_call<T>(
    client: &Client,
    rpc_url: &str,
    method: &str,
    params: Value,
) -> Result<T, DynError>
where
    T: DeserializeOwned,
{
    let payload = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });

    let response = client.post(rpc_url).json(&payload).send().await?;
    let status = response.status();
    let body: Value = response.json().await?;

    if !status.is_success() {
        return Err(format!("rpc HTTP error for {method}: {status}").into());
    }
    if let Some(error_value) = body.get("error") {
        let error: RpcError = serde_json::from_value(error_value.clone())?;
        let extra = error
            .data
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        return Err(format!(
            "rpc error for {method}: {} {}{}",
            error.code, error.message, extra
        )
        .into());
    }

    let result_value = body
        .get("result")
        .cloned()
        .ok_or_else(|| format!("rpc result missing for method {method}"))?;

    serde_json::from_value(result_value)
        .map_err(|error| format!("failed to decode rpc result for {method}: {error}").into())
}

async fn fetch_latest_block_number(client: &Client, rpc_url: &str) -> Result<u64, DynError> {
    let hex_block: String = rpc_call(client, rpc_url, "eth_blockNumber", json!([])).await?;
    parse_hex_u64(&hex_block)
}

async fn fetch_block_header(
    client: &Client,
    rpc_url: &str,
    block_hash: &str,
) -> Result<EthBlockHeader, DynError> {
    let block = rpc_call::<EthBlockHeader>(
        client,
        rpc_url,
        "eth_getBlockByHash",
        json!([block_hash, false]),
    )
    .await?;
    Ok(block)
}

async fn fetch_transaction_receipt(
    client: &Client,
    rpc_url: &str,
    tx_hash: &str,
) -> Result<EthTransactionReceipt, DynError> {
    let receipt = rpc_call::<EthTransactionReceipt>(
        client,
        rpc_url,
        "eth_getTransactionReceipt",
        json!([tx_hash]),
    )
    .await?;
    Ok(receipt)
}

async fn fetch_asset_locked_logs(
    client: &Client,
    rpc_url: &str,
    bridge_address: &str,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<EthLog>, DynError> {
    rpc_call(
        client,
        rpc_url,
        "eth_getLogs",
        json!([{
            "address": bridge_address,
            "fromBlock": format!("0x{from_block:x}"),
            "toBlock": format!("0x{to_block:x}"),
            "topics": [asset_locked_event_topic()]
        }]),
    )
    .await
}

fn extract_asset_locked_event(
    config: &RelayerConfig,
    log: &EthLog,
    receipt: &EthTransactionReceipt,
    block: &EthBlockHeader,
) -> Result<AssetLockedEvent, DynError> {
    if log.removed {
        return Err("skipping removed log from reorg".into());
    }
    if normalize_address(&log.address)? != config.bridge_address {
        return Err("bridge log address mismatch".into());
    }
    if log.topics.len() != 3 {
        return Err(format!(
            "expected 3 topics for AssetLocked, got {}",
            log.topics.len()
        )
        .into());
    }

    let user = decode_topic_address(&log.topics[1])?;
    let token = decode_topic_address(&log.topics[2])?;
    let amount = parse_uint256_u128(&log.data)?;
    let transaction_hash = normalize_hex(
        log.transaction_hash
            .as_deref()
            .ok_or("missing transaction hash on bridge log")?,
    )?;
    let block_hash = normalize_hex(
        log.block_hash
            .as_deref()
            .ok_or("missing block hash on bridge log")?,
    )?;
    let block_number = parse_hex_u64(
        log.block_number
            .as_deref()
            .ok_or("missing block number on bridge log")?,
    )?;
    let log_index = parse_hex_u64(
        log.log_index
            .as_deref()
            .ok_or("missing log index on bridge log")?,
    )?;

    let receipt_status = receipt.status.as_deref().unwrap_or("0x0");
    if receipt_status != "0x1" {
        return Err(format!("bridge source transaction not successful: {receipt_status}").into());
    }
    let receipt_contains_log = receipt.logs.iter().any(|candidate| {
        candidate
            .transaction_hash
            .as_deref()
            .map(|hash| {
                strip_hex_prefix(hash).eq_ignore_ascii_case(strip_hex_prefix(&transaction_hash))
            })
            .unwrap_or(false)
            && candidate
                .log_index
                .as_deref()
                .map(|index| {
                    strip_hex_prefix(index).eq_ignore_ascii_case(strip_hex_prefix(
                        log.log_index.as_deref().unwrap_or_default(),
                    ))
                })
                .unwrap_or(false)
    });
    if !receipt_contains_log {
        return Err("bridge receipt does not contain the expected log entry".into());
    }

    let header_hash = normalize_hex(
        block
            .hash
            .as_deref()
            .ok_or("missing block hash in header")?,
    )?;
    let header_number = parse_hex_u64(
        block
            .number
            .as_deref()
            .ok_or("missing block number in header")?,
    )?;
    if header_hash != block_hash || header_number != block_number {
        return Err("bridge block header does not match log metadata".into());
    }

    Ok(AssetLockedEvent {
        user,
        token,
        amount,
        transaction_hash,
        block_hash,
        block_number,
        log_index,
    })
}

fn sign_claim_hash(signing_key: &SigningKey, signing_hash: [u8; 32]) -> Result<String, DynError> {
    let (signature, recovery_id): (Signature, RecoveryId) =
        signing_key.sign_prehash(&signing_hash)?;
    let mut signature_bytes = signature.to_bytes().to_vec();
    signature_bytes.push(recovery_id.to_byte() + 27);
    Ok(format!("0x{}", hex::encode(signature_bytes)))
}

fn build_claim_request(
    config: &RelayerConfig,
    event: &AssetLockedEvent,
) -> Result<BridgeClaimRequest, DynError> {
    let witness = BridgeEventWitness {
        source_chain: config.source_chain.clone(),
        source_tx_hash: event.transaction_hash.clone(),
        recipient: event.user.clone(),
        amount: event.amount.to_string(),
        token_address: event.token.clone(),
        block_hash: event.block_hash.clone(),
        block_number: event.block_number,
        log_index: event.log_index,
    };
    let proof_bundle = generate_bridge_proof_bundle(&witness)?;
    let signing_hash = bridge_claim_signing_hash(
        &witness.source_chain,
        &witness.source_tx_hash,
        &witness.amount,
        &witness.recipient,
    );
    let signature_hex = sign_claim_hash(&config.relayer_signing_key, signing_hash)?;

    build_bridge_claim_request(&witness, vec![signature_hex], &proof_bundle)
        .map_err(|err| err.into())
}

async fn transmit_to_qanto(
    client: &Client,
    rpc_url: &str,
    claim_request: &BridgeClaimRequest,
) -> Result<String, DynError> {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "qanto_bridgeMint",
        "params": [claim_request],
        "id": 1
    });

    let response = client.post(rpc_url).json(&payload).send().await?;
    let status = response.status();
    let body: Value = response.json().await?;

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
async fn main() -> Result<(), DynError> {
    let config = RelayerConfig::from_env()?;
    let client = Client::new();
    let relayer_address = signing_key_address(&config.relayer_signing_key);

    println!("========================================================");
    println!("IGNITING QANTO CROSS-CHAIN RELAYER DAEMON");
    println!("========================================================");
    println!("[+] Source chain: {}", config.source_chain);
    println!("[+] Source RPC: {}", config.eth_rpc_url);
    println!("[+] QANTO RPC: {}", config.qanto_rpc_url);
    println!("[+] Bridge contract: {}", config.bridge_address);
    println!("[+] Relayer signer: {}", relayer_address);
    println!("[+] Poll interval: {}s", config.poll_interval_secs);
    println!("[+] Confirmations: {}", config.confirmations);

    let latest_block = fetch_latest_block_number(&client, &config.eth_rpc_url).await?;
    let mut last_processed_block = config
        .start_block
        .map(|block| block.saturating_sub(1))
        .unwrap_or(latest_block);
    let mut seen_events = HashSet::new();

    println!(
        "[*] Starting EVM log poller from block {}",
        last_processed_block.saturating_add(1)
    );

    loop {
        let latest_block = fetch_latest_block_number(&client, &config.eth_rpc_url).await?;
        let safe_latest = latest_block.saturating_sub(config.confirmations.saturating_sub(1));

        if safe_latest <= last_processed_block {
            sleep(Duration::from_secs(config.poll_interval_secs)).await;
            continue;
        }

        let from_block = last_processed_block.saturating_add(1);
        let to_block = safe_latest;
        println!("[*] Scanning bridge logs from block {from_block} to {to_block}");

        let logs = fetch_asset_locked_logs(
            &client,
            &config.eth_rpc_url,
            &config.bridge_address,
            from_block,
            to_block,
        )
        .await?;

        for log in logs {
            let tx_hash = match log.transaction_hash.as_deref() {
                Some(hash) => normalize_hex(hash)?,
                None => {
                    eprintln!("    [-] Bridge log missing transaction hash; skipping");
                    continue;
                }
            };
            let log_index = match log.log_index.as_deref() {
                Some(index) => parse_hex_u64(index)?,
                None => {
                    eprintln!("    [-] Bridge log missing log index; skipping");
                    continue;
                }
            };
            let event_key = format!("{tx_hash}:{log_index}");
            if !seen_events.insert(event_key.clone()) {
                continue;
            }

            let receipt = fetch_transaction_receipt(&client, &config.eth_rpc_url, &tx_hash).await?;
            let block_hash = normalize_hex(
                log.block_hash
                    .as_deref()
                    .ok_or("missing block hash on bridge log")?,
            )?;
            let block = fetch_block_header(&client, &config.eth_rpc_url, &block_hash).await?;

            match extract_asset_locked_event(&config, &log, &receipt, &block) {
                Ok(event) => {
                    println!("\n[!] DETECTED LOCK EVENT");
                    println!("    User: {}", event.user);
                    println!("    Token: {}", event.token);
                    println!("    Amount (base units): {}", event.amount);
                    println!("    Source tx: {}", event.transaction_hash);

                    match build_claim_request(&config, &event) {
                        Ok(claim_request) => {
                            match transmit_to_qanto(&client, &config.qanto_rpc_url, &claim_request)
                                .await
                            {
                                Ok(claim_tx) => {
                                    println!("    [+] QANTO bridge claim accepted: {}", claim_tx);
                                }
                                Err(error) => {
                                    eprintln!("    [-] Failed to submit claim to QANTO: {}", error);
                                }
                            }
                        }
                        Err(error) => {
                            eprintln!("    [-] Failed to build bridge claim: {}", error);
                        }
                    }
                }
                Err(error) => {
                    eprintln!(
                        "    [-] Skipping invalid bridge event {}: {}",
                        event_key, error
                    );
                }
            }
        }

        last_processed_block = to_block;
        sleep(Duration::from_secs(config.poll_interval_secs)).await;
    }
}
