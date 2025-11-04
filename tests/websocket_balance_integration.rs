//! WebSocket Balance Integration Test
//!
//! Validates that a real WebSocket client can subscribe to balance updates
//! for a specific address and receive broadcast events filtered by address.

use std::sync::Arc;
use tokio::net::TcpListener;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use qanto::analytics_dashboard::{AnalyticsDashboard, DashboardConfig};
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::saga::PalletSaga;
use qanto::wallet::Wallet;
use qanto::websocket_server::WebSocketServerState;

#[tokio::test]
async fn websocket_balance_subscription_receives_updates() {
    // Build server state with dummy DAG and default analytics
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());
    let saga = {
        #[cfg(feature = "infinite-strata")]
        {
            Arc::new(PalletSaga::new(None))
        }
        #[cfg(not(feature = "infinite-strata"))]
        {
            Arc::new(PalletSaga::new())
        }
    };
    let analytics = Arc::new(AnalyticsDashboard::new(DashboardConfig::default()));
    let ws_state = WebSocketServerState::new(dag, saga, analytics);

    // Bind to ephemeral localhost port and start server in background
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind test listener");
    let local_addr = listener.local_addr().expect("local_addr");
    let router = qanto::websocket_server::create_websocket_router(ws_state.clone());
    let server_task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Deterministic wallet address from known fixed private key used across tests
    let test_private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let address = Wallet::from_private_key(test_private_key)
        .expect("Failed to create wallet from test private key")
        .address();

    // Connect WebSocket client and subscribe to balance for the address
    let url = format!("ws://{}/ws", local_addr);
    let (ws_stream, _resp) = connect_async(&url).await.expect("WS connect failed");
    let (mut write, mut read) = ws_stream.split();

    let subscribe_msg = serde_json::json!({
        "type": "subscribe",
        "subscription_type": "balances",
        "filters": {"address": address}
    })
    .to_string();

    write
        .send(Message::Text(subscribe_msg))
        .await
        .expect("Failed to send subscribe message");

    // Allow server to process subscription before broadcasting
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Broadcast a balance update for the subscribed address
    ws_state
        .broadcast_balance_update(address.clone(), 4242)
        .await;

    // Expect a balance_update for the address within a short timeout
    let recv = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(json) => json,
                    Err(_) => continue,
                };
                if v.get("type").and_then(|t| t.as_str()) == Some("balance_update") {
                    let addr = v.get("address").and_then(|a| a.as_str()).unwrap_or("");
                    let bal = v.get("balance").and_then(|b| b.as_u64()).unwrap_or(0);
                    if addr == address && bal == 4242 {
                        return Some(());
                    }
                }
            }
        }
        None
    })
    .await
    .expect("timeout waiting for balance update");

    assert!(recv.is_some(), "Did not receive expected balance update");

    // Cleanup: drop server task
    server_task.abort();
}
