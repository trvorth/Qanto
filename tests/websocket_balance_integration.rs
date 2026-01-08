use axum::{
    extract::ws::{Message as AxumMessage, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::test]
async fn websocket_balance_snapshot_and_delta_flow() {
    async fn ws_server(ws: WebSocketUpgrade) -> Response {
        ws.on_upgrade(|socket| async move {
            let (mut sender, mut receiver) = socket.split();
            // Wait for subscribe message
            if let Some(Ok(AxumMessage::Text(_text))) = receiver.next().await {
                // Send BalanceSnapshot (aggregated totals only)
                let snapshot = serde_json::json!({
                    "type": "balance_snapshot",
                    "address": "ADDR_TEST",
                    "total_confirmed": 1_000_000u64,
                    "total_unconfirmed": 50_000u64,
                    "timestamp": 12345u64,
                })
                .to_string();
                let _ = sender.send(AxumMessage::Text(snapshot.into())).await;

                // Then send a small BalanceDeltaUpdate
                let delta = serde_json::json!({
                    "type": "balance_delta_update",
                    "address": "ADDR_TEST",
                    "delta_confirmed": 250_000i64,
                    "delta_unconfirmed": -10_000i64,
                    "timestamp": 12346u64,
                    "finalized": true,
                })
                .to_string();
                let _ = sender.send(AxumMessage::Text(delta.into())).await;
            }
        })
    }

    let router = Router::new().route("/ws", get(ws_server));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let server_task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    // Client connects and subscribes
    let url = format!("ws://{}/ws", addr);
    let (ws_stream, _resp) = connect_async(&url).await.expect("connect");
    let (mut write, mut read) = ws_stream.split();

    let sub_msg = serde_json::json!({
        "type": "subscribe",
        "subscription_type": "balances",
        "filters": {"address": "ADDR_TEST"}
    })
    .to_string();
    write
        .send(Message::Text(sub_msg))
        .await
        .expect("send subscribe");

    // Expect snapshot then delta
    let mut saw_snapshot = false;
    let mut saw_delta = false;
    while let Some(msg) = read.next().await {
        if let Ok(Message::Text(text)) = msg {
            let v: serde_json::Value = serde_json::from_str(&text).expect("json");
            match v.get("type").and_then(|t| t.as_str()) {
                Some("balance_snapshot") => {
                    assert_eq!(v.get("address").and_then(|a| a.as_str()), Some("ADDR_TEST"));
                    assert_eq!(
                        v.get("total_confirmed").and_then(|x| x.as_u64()),
                        Some(1_000_000)
                    );
                    assert_eq!(
                        v.get("total_unconfirmed").and_then(|x| x.as_u64()),
                        Some(50_000)
                    );
                    saw_snapshot = true;
                }
                Some("balance_delta_update") => {
                    assert!(saw_snapshot, "delta should follow snapshot");
                    assert_eq!(v.get("address").and_then(|a| a.as_str()), Some("ADDR_TEST"));
                    assert_eq!(
                        v.get("delta_confirmed").and_then(|x| x.as_i64()),
                        Some(250_000)
                    );
                    assert_eq!(
                        v.get("delta_unconfirmed").and_then(|x| x.as_i64()),
                        Some(-10_000)
                    );
                    saw_delta = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(saw_snapshot && saw_delta);
    server_task.abort();
}
