use crate::keystore::{get_key_material, record_failed_unlock, unlock_wallet_impl, WalletSessionState};
use bincode::Options;
use my_blockchain::qanto_hash;
#[cfg(feature = "pqcrypto-legacy")]
use pqcrypto_traits::sign::{DetachedSignature as _, SecretKey as _};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tokio::time::timeout;

const DEFAULT_CHAIN_ID: u32 = 1234;
const DEFAULT_GAS_LIMIT: u64 = 50_000;
const DEFAULT_GAS_PRICE: u128 = 1;
const DEFAULT_PRIORITY_FEE: u128 = 0;
const DEFAULT_FEE: u128 = 0;
const DEFAULT_API_ADDR: &str = "127.0.0.1:8081";

fn validate_address(address: &str) -> Result<(), String> {
    let addr = address.trim();
    if addr.len() != 64 {
        return Err("invalid address: expected 64 hex characters".to_string());
    }
    if !addr.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid address: must be hex".to_string());
    }
    Ok(())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn parse_qnto_to_base_units(amount: &str) -> Result<u128, String> {
    let s = amount.trim();
    if s.is_empty() {
        return Err("amount is required".to_string());
    }
    if s.starts_with('-') {
        return Err("amount must be positive".to_string());
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() > 2 {
        return Err("invalid amount format".to_string());
    }
    let int_part = parts[0];
    if int_part.is_empty() {
        return Err("invalid amount format".to_string());
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) {
        return Err("amount must be numeric".to_string());
    }
    let frac_part = if parts.len() == 2 { parts[1] } else { "" };
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return Err("amount must be numeric".to_string());
    }
    if frac_part.len() > 9 {
        return Err("amount supports up to 9 decimal places".to_string());
    }
    let mut frac = frac_part.to_string();
    while frac.len() < 9 {
        frac.push('0');
    }
    let combined = format!("{}{}", int_part, frac);
    combined
        .trim_start_matches('0')
        .to_string()
        .parse::<u128>()
        .or_else(|_| {
            if combined.chars().all(|c| c == '0') {
                Ok(0u128)
            } else {
                Err("amount too large".to_string())
            }
        })
}

#[derive(Serialize)]
pub struct SendTransactionResponse {
    pub tx_hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TxRecord {
    pub tx_hash: String,
    pub sender: String,
    pub receiver: String,
    pub amount_base_units: String,
    pub block_timestamp: u64,
    pub confirmations: u64,
    pub is_finalized: bool,
    pub in_mempool: bool,
    pub status: String,
}

#[derive(Serialize)]
pub struct TxHistoryResponse {
    pub items: Vec<TxRecord>,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Default)]
pub struct TxSubscriptionState {
    pub tasks: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
}

#[derive(Clone, Serialize)]
struct CanonicalInput {
    tx_id: String,
    output_index: u32,
}

#[derive(Clone, Serialize)]
struct CanonicalHomomorphicEncrypted {
    ciphertext: Vec<u8>,
    public_key: Vec<u8>,
}

#[derive(Clone, Serialize)]
struct CanonicalOutput {
    address: String,
    amount: u128,
    homomorphic_encrypted: CanonicalHomomorphicEncrypted,
}

#[derive(Clone, Deserialize)]
struct WalletUtxo {
    address: String,
    amount: u128,
    tx_id: String,
    output_index: u32,
    explorer_link: String,
}

#[derive(Clone, Copy, Serialize)]
enum CanonicalTransactionKind {
    Transfer,
    Stake,
    Unstake,
    Delegate,
    Vote,
    Proposal,
    BridgeLock,
    BridgeObserve,
    BridgeClaim,
    AirdropClaim,
}

#[derive(Serialize)]
struct CanonicalTransactionIdPayload {
    sender: String,
    receiver: String,
    amount: u128,
    fee: u128,
    gas_limit: u128,
    gas_used: u128,
    gas_price: u128,
    priority_fee: u128,
    inputs: Vec<CanonicalInput>,
    outputs: Vec<CanonicalOutput>,
    metadata: BTreeMap<String, String>,
    timestamp: u64,
    transaction_kind: CanonicalTransactionKind,
    chain_id: u32,
}

#[derive(Serialize)]
struct CanonicalTransactionSigningPayload {
    id: String,
    sender: String,
    receiver: String,
    amount: u128,
    fee: u128,
    gas_limit: u128,
    gas_used: u128,
    gas_price: u128,
    priority_fee: u128,
    inputs: Vec<CanonicalInput>,
    outputs: Vec<CanonicalOutput>,
    metadata: BTreeMap<String, String>,
    timestamp: u64,
    transaction_kind: CanonicalTransactionKind,
    chain_id: u32,
}

fn canonical_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .serialize(value)
        .map_err(|e| format!("canonical transaction serialization failed: {e}"))
}

fn build_transfer_signing_payload(
    sender: String,
    receiver: String,
    amount: u128,
    timestamp: u64,
    inputs: Vec<CanonicalInput>,
    outputs: Vec<CanonicalOutput>,
) -> Result<(String, CanonicalTransactionSigningPayload), String> {
    let metadata = BTreeMap::new();
    let transaction_kind = CanonicalTransactionKind::Transfer;

    let id_payload = CanonicalTransactionIdPayload {
        sender: sender.clone(),
        receiver: receiver.clone(),
        amount,
        fee: DEFAULT_FEE,
        gas_limit: DEFAULT_GAS_LIMIT as u128,
        gas_used: 0,
        gas_price: DEFAULT_GAS_PRICE,
        priority_fee: DEFAULT_PRIORITY_FEE,
        inputs: inputs.clone(),
        outputs: outputs.clone(),
        metadata: metadata.clone(),
        timestamp,
        transaction_kind,
        chain_id: DEFAULT_CHAIN_ID,
    };
    let id = hex::encode(qanto_hash(&canonical_serialize(&id_payload)?).as_bytes());

    Ok((
        id.clone(),
        CanonicalTransactionSigningPayload {
            id,
            sender,
            receiver,
            amount,
            fee: DEFAULT_FEE,
            gas_limit: DEFAULT_GAS_LIMIT as u128,
            gas_used: 0,
            gas_price: DEFAULT_GAS_PRICE,
            priority_fee: DEFAULT_PRIORITY_FEE,
            inputs,
            outputs,
            metadata,
            timestamp,
            transaction_kind,
            chain_id: DEFAULT_CHAIN_ID,
        },
    ))
}

fn http_get(path: &str, api_addr: &str) -> Result<String, String> {
    let mut stream =
        TcpStream::connect(api_addr).map_err(|e| format!("API connect error ({api_addr}): {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("API read timeout setup failed ({api_addr}): {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("API write timeout setup failed ({api_addr}): {e}"))?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {api_addr}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("API request write failed ({api_addr}): {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("API request flush failed ({api_addr}): {e}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("API response read failed ({api_addr}): {e}"))?;

    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "API response missing body".to_string())?;
    if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.0 200") {
        return Err(format!("API request failed: {headers}"));
    }
    Ok(body.to_string())
}

fn fetch_utxos_for_address(address: &str, api_addr: &str) -> Result<Vec<WalletUtxo>, String> {
    let body = http_get(&format!("/utxos/{address}"), api_addr)?;
    let mut utxos: Vec<WalletUtxo> = serde_json::from_str::<HashMap<String, WalletUtxo>>(&body)
        .map_err(|e| format!("failed to parse UTXO response: {e}"))?
        .into_values()
        .collect();

    utxos.sort_by(|a, b| {
        b.amount
            .cmp(&a.amount)
            .then_with(|| a.tx_id.cmp(&b.tx_id))
            .then_with(|| a.output_index.cmp(&b.output_index))
    });
    Ok(utxos)
}

fn build_transfer_io(
    sender: &str,
    recipient: &str,
    amount: u128,
    fee: u128,
    utxos: &[WalletUtxo],
) -> Result<(Vec<CanonicalInput>, Vec<CanonicalOutput>), String> {
    let required = amount
        .checked_add(fee)
        .ok_or_else(|| "amount overflow".to_string())?;
    let mut selected = Vec::new();
    let mut selected_total = 0u128;

    for utxo in utxos {
        if utxo.address != sender {
            continue;
        }
        selected_total = selected_total.saturating_add(utxo.amount);
        selected.push(CanonicalInput {
            tx_id: utxo.tx_id.clone(),
            output_index: utxo.output_index,
        });
        if selected_total >= required {
            break;
        }
    }

    if selected_total < required {
        return Err(format!(
            "insufficient funds: required {required}, available {selected_total}"
        ));
    }

    let mut outputs = vec![CanonicalOutput {
        address: recipient.to_string(),
        amount,
        homomorphic_encrypted: CanonicalHomomorphicEncrypted {
            ciphertext: Vec::new(),
            public_key: Vec::new(),
        },
    }];

    let change = selected_total.saturating_sub(required);
    if change > 0 {
        outputs.push(CanonicalOutput {
            address: sender.to_string(),
            amount: change,
            homomorphic_encrypted: CanonicalHomomorphicEncrypted {
                ciphertext: Vec::new(),
                public_key: Vec::new(),
            },
        });
    }

    Ok((selected, outputs))
}

fn tx_status_from_entry(entry: &qanto_rpc::server::generated::AddressTransactionEntry) -> &'static str {
    if entry.in_mempool {
        "pending"
    } else if entry.is_finalized {
        "finalized"
    } else if entry.confirmations > 0 {
        "confirmed"
    } else {
        "pending"
    }
}

fn tx_record_from_entry(
    entry: qanto_rpc::server::generated::AddressTransactionEntry,
) -> Option<TxRecord> {
    let status = tx_status_from_entry(&entry).to_string();
    let tx = entry.transaction?;
    Some(TxRecord {
        tx_hash: tx.id,
        sender: tx.sender,
        receiver: tx.receiver,
        amount_base_units: tx.amount,
        block_timestamp: entry.block_timestamp,
        confirmations: entry.confirmations,
        is_finalized: entry.is_finalized,
        in_mempool: entry.in_mempool,
        status,
    })
}

pub async fn send_transaction_impl(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    state: &WalletSessionState,
    recipient: String,
    amount_string: String,
    password: String,
) -> Result<SendTransactionResponse, String> {
    validate_address(&recipient)?;

    let sender = match unlock_wallet_impl(app, state, &password).await {
        Ok(addr) => addr,
        Err(e) => {
            record_failed_unlock(state).await;
            return Err(e);
        }
    };

    let material = get_key_material(state)
        .await
        .ok_or_else(|| "wallet is locked".to_string())?;

    if material.address != sender {
        return Err("wallet state mismatch".to_string());
    }

    let amount_u128 = parse_qnto_to_base_units(&amount_string)?;
    if amount_u128 == 0 {
        return Err("amount must be greater than zero".to_string());
    }
    let amount_base_units = amount_u128.to_string();
    let timestamp = now_unix_seconds();
    let api_addr = std::env::var("QANTO_API_ADDR").unwrap_or_else(|_| DEFAULT_API_ADDR.to_string());
    let wallet_utxos = fetch_utxos_for_address(&sender, &api_addr)?;
    let (inputs, outputs) =
        build_transfer_io(&sender, &recipient, amount_u128, DEFAULT_FEE, &wallet_utxos)?;
    let (tx_hash, signing_payload) = build_transfer_signing_payload(
        sender.clone(),
        recipient.clone(),
        amount_u128,
        timestamp,
        inputs.clone(),
        outputs.clone(),
    )?;
    let payload_bytes = canonical_serialize(&signing_payload)?;

    let pq_sk = pqcrypto_dilithium::dilithium3::SecretKey::from_bytes(&material.dilithium3_secret_key)
        .map_err(|_| "invalid dilithium3 secret key".to_string())?;
    let pq_sig = pqcrypto_dilithium::dilithium3::detached_sign(&payload_bytes, &pq_sk);

    let mut metadata = HashMap::new();
    metadata.insert("transaction_kind".to_string(), "TRANSFER".to_string());

    let tx = qanto_rpc::server::generated::Transaction {
        id: tx_hash.clone(),
        sender: sender.clone(),
        receiver: recipient.clone(),
        amount: amount_base_units.clone(),
        fee: DEFAULT_FEE.to_string(),
        gas_limit: DEFAULT_GAS_LIMIT,
        gas_used: 0,
        gas_price: DEFAULT_GAS_PRICE.to_string(),
        priority_fee: DEFAULT_PRIORITY_FEE.to_string(),
        inputs: inputs
            .into_iter()
            .map(|input| qanto_rpc::server::generated::Input {
                tx_id: input.tx_id,
                output_index: input.output_index,
            })
            .collect(),
        outputs: outputs
            .into_iter()
            .map(|output| qanto_rpc::server::generated::Output {
                address: output.address,
                amount: output.amount.to_string(),
                homomorphic_encrypted: None,
            })
            .collect(),
        timestamp,
        metadata,
        signature: Some(qanto_rpc::server::generated::QuantumResistantSignature {
            signer_public_key: material.dilithium3_public_key.clone(),
            signature: pq_sig.as_bytes().to_vec(),
        }),
        fee_breakdown: None,
        chain_id: DEFAULT_CHAIN_ID,
    };

    let rpc_addr = std::env::var("QANTO_RPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".to_string());
    let endpoint = format!("http://{rpc_addr}");
    let mut client = timeout(
        Duration::from_secs(3),
        qanto_rpc::server::generated::qanto_rpc_client::QantoRpcClient::connect(endpoint.clone()),
    )
    .await
    .map_err(|_| format!("RPC connect timeout ({rpc_addr})"))?
    .map_err(|e| format!("RPC connect error ({rpc_addr}): {e}"))?;

    let req = qanto_rpc::server::generated::SubmitTransactionRequest {
        transaction: Some(tx),
    };
    let resp = timeout(Duration::from_secs(5), client.submit_transaction(req))
        .await
        .map_err(|_| format!("RPC request timeout ({rpc_addr})"))?
        .map_err(|status| format!("RPC error: {}", status.message()))?
        .into_inner();

    if !resp.accepted {
        return Err(format!("transaction rejected: {}", resp.message));
    }

    Ok(SendTransactionResponse { tx_hash })
}

pub async fn get_transaction_history_impl(
    _app: &tauri::AppHandle<impl tauri::Runtime>,
    address: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<TxHistoryResponse, String> {
    validate_address(&address)?;
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(5, 100);

    let rpc_addr = std::env::var("QANTO_RPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".to_string());
    let endpoint = format!("http://{rpc_addr}");

    let mut last_err: Option<String> = None;
    for attempt in 0..2u32 {
        let mut client = match timeout(
            Duration::from_secs(3),
            qanto_rpc::server::generated::qanto_rpc_client::QantoRpcClient::connect(endpoint.clone()),
        )
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                last_err = Some(format!("RPC connect error ({rpc_addr}): {e}"));
                if attempt == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
                continue;
            }
            Err(_) => {
                last_err = Some(format!("RPC connect timeout ({rpc_addr})"));
                if attempt == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
                continue;
            }
        };

        let req = qanto_rpc::server::generated::GetTransactionsByAddressRequest {
            address: address.clone(),
            page,
            page_size,
        };

        let resp = match timeout(Duration::from_secs(6), client.get_transactions_by_address(req)).await {
            Ok(Ok(r)) => r.into_inner(),
            Ok(Err(status)) => {
                last_err = Some(format!("RPC error: {}", status.message()));
                if attempt == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
                continue;
            }
            Err(_) => {
                last_err = Some(format!("RPC request timeout ({rpc_addr})"));
                if attempt == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
                continue;
            }
        };

        let mut items: Vec<TxRecord> = Vec::with_capacity(resp.transactions.len());
        for entry in resp.transactions {
            if let Some(record) = tx_record_from_entry(entry) {
                items.push(record);
            }
        }

        let total = resp.total;
        let has_more = (page as u64).saturating_mul(page_size as u64) < total;
        return Ok(TxHistoryResponse {
            items,
            page,
            page_size,
            has_more,
        });
    }

    Err(last_err.unwrap_or_else(|| "transaction history query failed".to_string()))
}

pub async fn subscribe_transactions_impl(
    window: tauri::Window,
    subscriptions: &TxSubscriptionState,
    address: String,
) -> Result<(), String> {
    validate_address(&address)?;

    let window_key = window.label().to_string();
    if let Some(existing) = subscriptions.tasks.lock().map_err(|_| "subscription lock poisoned".to_string())?.remove(&window_key) {
        existing.abort();
    }

    let window_for_task = window.clone();
    let address_for_task = address.clone();
    let handle = tokio::spawn(async move {
        let rpc_addr = std::env::var("QANTO_RPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".to_string());
        let endpoint = format!("http://{rpc_addr}");

        loop {
            let mut client = match timeout(
                Duration::from_secs(3),
                qanto_rpc::server::generated::qanto_rpc_client::QantoRpcClient::connect(endpoint.clone()),
            )
            .await
            {
                Ok(Ok(client)) => client,
                Ok(Err(_)) | Err(_) => {
                    tokio::time::sleep(Duration::from_millis(750)).await;
                    continue;
                }
            };

            let request = qanto_rpc::server::generated::SubscribeTransactionsRequest {
                address: address_for_task.clone(),
                include_mempool: true,
                include_confirmed: true,
                include_finalized: true,
            };

            let response = match timeout(
                Duration::from_secs(5),
                client.subscribe_transactions_by_address(request),
            )
            .await
            {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(_)) | Err(_) => {
                    tokio::time::sleep(Duration::from_millis(750)).await;
                    continue;
                }
            };

            let mut stream = response;
            loop {
                match stream.message().await {
                    Ok(Some(entry)) => {
                        if let Some(record) = tx_record_from_entry(entry) {
                            let _ = window_for_task.emit("tx-update", &record);
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    subscriptions
        .tasks
        .lock()
        .map_err(|_| "subscription lock poisoned".to_string())?
        .insert(window_key, handle);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::get_wallet_balance;
    use crate::keystore::{create_wallet_impl, keystore_path};
    use rand::RngCore;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use tauri::test::mock_app;
    use tokio::time::sleep;

    const LIVE_RPC_ADDR: &str = "127.0.0.1:50051";
    const LIVE_API_ADDR: &str = "127.0.0.1:8081";
    const TEST_PASSWORD: &str = "Qanto#E2E123";
    const AIRDROP_BASE_UNITS: u128 = 100_000_000_000;
    const SEND_BASE_UNITS: u128 = 5_000_000_000;

    fn remove_test_keystore<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
        if let Ok(path) = keystore_path(app) {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!("test keystore cleanup failed to remove {}: {err}", path.display()),
            }
            if let Some(parent) = path.parent() {
                let _ = fs::remove_dir_all(parent);
            }
            assert!(
                !path.exists(),
                "test keystore cleanup must remove {}",
                path.display()
            );
        }
    }

    fn live_test_data_dir() -> PathBuf {
        std::env::temp_dir().join("qanto-wallet-live-e2e")
    }

    fn random_hex_address() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    fn jsonrpc_post(method: &str, address: &str) -> Result<Value, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": [address],
            "id": 1
        })
        .to_string();

        let mut stream =
            TcpStream::connect(LIVE_API_ADDR).map_err(|e| format!("jsonrpc connect error: {e}"))?;
        stream
            .write_all(
                format!(
                    "POST /rpc HTTP/1.1\r\nHost: {LIVE_API_ADDR}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .map_err(|e| format!("jsonrpc write error: {e}"))?;
        stream
            .flush()
            .map_err(|e| format!("jsonrpc flush error: {e}"))?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|e| format!("jsonrpc read error: {e}"))?;

        let (_, body) = response
            .split_once("\r\n\r\n")
            .ok_or_else(|| "jsonrpc response missing body".to_string())?;
        serde_json::from_str(body).map_err(|e| format!("jsonrpc parse error: {e}"))
    }

    async fn wait_for_confirmed_balance(
        address: &str,
        min_confirmed: u128,
    ) -> Result<crate::app::WalletBalanceResponse, String> {
        let mut last_seen = String::new();
        for _ in 0..30 {
            let balance = get_wallet_balance(address.to_string()).await?;
            last_seen = balance.balance.clone();
            let confirmed = balance
                .balance
                .parse::<u128>()
                .map_err(|e| format!("balance parse error: {e}"))?;
            if confirmed >= min_confirmed {
                return Ok(balance);
            }
            sleep(Duration::from_secs(1)).await;
        }
        Err(format!(
            "timed out waiting for confirmed balance >= {min_confirmed}, last_seen={last_seen}"
        ))
    }

    async fn wait_for_transaction(
        app: &tauri::AppHandle<impl tauri::Runtime>,
        address: &str,
        tx_hash: &str,
    ) -> Result<TxRecord, String> {
        for _ in 0..120 {
            let history = get_transaction_history_impl(app, address.to_string(), Some(1), Some(20)).await?;
            if let Some(record) = history.items.into_iter().find(|item| item.tx_hash == tx_hash) {
                if !record.in_mempool {
                    return Ok(record);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
        Err(format!("timed out waiting for transaction {tx_hash} on address {address}"))
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires a running local QANTO testnet on 127.0.0.1:50051 and 127.0.0.1:8081"]
    async fn wallet_live_airdrop_send_history_e2e() {
        std::env::set_var("QANTO_RPC_ADDR", LIVE_RPC_ADDR);
        std::env::set_var("QANTO_APP_DATA_DIR", live_test_data_dir());

        let app = mock_app();
        let app_handle = app.handle();
        remove_test_keystore(&app_handle);

        let session = WalletSessionState::default();
        let recipient = random_hex_address();

        let recipient_before = get_wallet_balance(recipient.clone())
            .await
            .expect("recipient initial balance query should work");
        let recipient_before_confirmed = recipient_before
            .balance
            .parse::<u128>()
            .expect("recipient initial balance should parse");

        let sender = create_wallet_impl(&app_handle, TEST_PASSWORD)
            .await
            .expect("wallet should be created");
        assert_eq!(sender.len(), 64, "wallet address must remain 64 hex chars");

        let sender_initial = get_wallet_balance(sender.clone())
            .await
            .expect("sender initial balance query should work");
        assert_eq!(sender_initial.balance, "0");

        let airdrop_response = jsonrpc_post("qanto_claimAirdrop", &sender)
            .expect("airdrop JSON-RPC should succeed");
        assert!(
            airdrop_response.get("error").is_none(),
            "airdrop returned error payload: {airdrop_response}"
        );

        let funded_balance = wait_for_confirmed_balance(&sender, AIRDROP_BASE_UNITS)
            .await
            .expect("sender should receive airdrop funds");
        assert_eq!(
            funded_balance.balance.parse::<u128>().expect("funded sender balance should parse"),
            AIRDROP_BASE_UNITS
        );

        let send_response = send_transaction_impl(
            &app_handle,
            &session,
            recipient.clone(),
            "5".to_string(),
            TEST_PASSWORD.to_string(),
        )
        .await
        .expect("wallet should send a signed transfer");
        assert_eq!(send_response.tx_hash.len(), 64, "tx hash must remain 64 hex chars");

        let sender_tx = wait_for_transaction(&app_handle, &sender, &send_response.tx_hash)
            .await
            .expect("sender history should contain confirmed transaction");
        assert_eq!(sender_tx.sender, sender);
        assert_eq!(sender_tx.receiver, recipient);
        assert_eq!(sender_tx.amount_base_units, SEND_BASE_UNITS.to_string());
        assert!(!sender_tx.in_mempool, "sender tx should exit mempool");

        let recipient_tx = wait_for_transaction(&app_handle, &recipient, &send_response.tx_hash)
            .await
            .expect("recipient history should contain confirmed transaction");
        assert_eq!(recipient_tx.sender, sender_tx.sender);
        assert_eq!(recipient_tx.receiver, sender_tx.receiver);
        assert_eq!(recipient_tx.amount_base_units, SEND_BASE_UNITS.to_string());

        let sender_after = get_wallet_balance(sender_tx.sender.clone())
            .await
            .expect("sender post-send balance query should work");
        let sender_after_confirmed = sender_after
            .balance
            .parse::<u128>()
            .expect("sender post-send balance should parse");
        assert_eq!(
            sender_after_confirmed,
            AIRDROP_BASE_UNITS - SEND_BASE_UNITS,
            "sender confirmed balance should reflect outgoing transfer"
        );

        let recipient_after = wait_for_confirmed_balance(
            &recipient,
            recipient_before_confirmed + SEND_BASE_UNITS,
        )
        .await
        .expect("recipient confirmed balance should include transferred funds");
        let recipient_after_confirmed = recipient_after
            .balance
            .parse::<u128>()
            .expect("recipient post-send balance should parse");
        assert_eq!(
            recipient_after_confirmed,
            recipient_before_confirmed + SEND_BASE_UNITS,
            "recipient confirmed balance should reflect incoming transfer"
        );

        remove_test_keystore(&app_handle);
        std::env::remove_var("QANTO_APP_DATA_DIR");
    }
}
