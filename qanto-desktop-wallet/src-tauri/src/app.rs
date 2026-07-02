use crate::keystore::{create_wallet_impl, export_mnemonic_impl, has_wallet_impl, record_failed_unlock, unlock_wallet_impl, WalletSessionState};
use crate::tx::{
    get_transaction_history_impl, send_transaction_impl, subscribe_transactions_impl, SendTransactionResponse,
    TxHistoryResponse, TxSubscriptionState,
};
use serde::Serialize;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Serialize)]
pub struct WalletBalanceResponse {
    pub balance: String,
    pub unconfirmed_balance: String,
}

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

fn ensure_main_window(window: &tauri::Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("unauthorized caller".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn generate_new_address() -> Result<String, String> {
    qanto_zk_sdk::generate_new_address().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn has_wallet(window: tauri::Window, app: tauri::AppHandle) -> Result<bool, String> {
    ensure_main_window(&window)?;
    has_wallet_impl(&app).await
}

#[tauri::command]
pub async fn create_wallet(
    window: tauri::Window,
    app: tauri::AppHandle,
    password: String,
) -> Result<String, String> {
    ensure_main_window(&window)?;
    create_wallet_impl(&app, &password).await
}

#[tauri::command]
pub async fn unlock_wallet(
    window: tauri::Window,
    app: tauri::AppHandle,
    state: tauri::State<'_, WalletSessionState>,
    password: String,
) -> Result<String, String> {
    ensure_main_window(&window)?;
    match unlock_wallet_impl(&app, &state, &password).await {
        Ok(addr) => Ok(addr),
        Err(e) => {
            record_failed_unlock(&state).await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn export_mnemonic(
    window: tauri::Window,
    app: tauri::AppHandle,
    password: String,
) -> Result<String, String> {
    ensure_main_window(&window)?;
    export_mnemonic_impl(&app, &password).await
}

#[tauri::command]
pub async fn send_transaction(
    window: tauri::Window,
    app: tauri::AppHandle,
    state: tauri::State<'_, WalletSessionState>,
    recipient: String,
    amount_string: String,
    password: String,
) -> Result<SendTransactionResponse, String> {
    ensure_main_window(&window)?;
    send_transaction_impl(&app, &state, recipient, amount_string, password).await
}

#[tauri::command]
pub async fn get_transaction_history(
    window: tauri::Window,
    app: tauri::AppHandle,
    address: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<TxHistoryResponse, String> {
    ensure_main_window(&window)?;
    get_transaction_history_impl(&app, address, page, page_size).await
}

#[tauri::command]
pub async fn subscribe_transactions(
    window: tauri::Window,
    subscriptions: tauri::State<'_, TxSubscriptionState>,
    address: String,
) -> Result<(), String> {
    ensure_main_window(&window)?;
    subscribe_transactions_impl(window, &subscriptions, address).await
}

#[tauri::command]
pub async fn get_wallet_balance(address: String) -> Result<WalletBalanceResponse, String> {
    validate_address(&address)?;

    let rpc_addr = std::env::var("QANTO_RPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".to_string());
    let endpoint = format!("http://{rpc_addr}");

    let mut client = timeout(
        Duration::from_secs(3),
        qanto_rpc::server::generated::wallet_service_client::WalletServiceClient::connect(
            endpoint.clone(),
        ),
    )
    .await
    .map_err(|_| format!("RPC connect timeout ({rpc_addr})"))?
    .map_err(|e| format!("RPC connect error ({rpc_addr}): {e}"))?;

    let req = qanto_rpc::server::generated::WalletGetBalanceRequest {
        address: address.to_string(),
    };

    let resp = timeout(Duration::from_secs(4), client.get_balance(req))
        .await
        .map_err(|_| format!("RPC request timeout ({rpc_addr})"))?
        .map_err(|status| format!("RPC error: {}", status.message()))?;

    let payload = resp.into_inner();

    Ok(WalletBalanceResponse {
        balance: payload.balance,
        unconfirmed_balance: payload.unconfirmed_balance,
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(WalletSessionState::default())
        .manage(TxSubscriptionState::default())
        .invoke_handler(tauri::generate_handler![
            generate_new_address,
            has_wallet,
            create_wallet,
            unlock_wallet,
            export_mnemonic,
            get_wallet_balance,
            send_transaction,
            get_transaction_history,
            subscribe_transactions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
