//! WebSocket Server for Real-time Qanto Network Subscriptions
//! Production-Grade Implementation v0.1.0
//!
//! This module provides real-time WebSocket subscriptions for:
//! - Block notifications
//! - Transaction confirmations
//! - Network health metrics
//! - Analytics dashboard data
//! - Security alerts
//! - Economic indicators

use crate::analytics_dashboard::{Alert, AnalyticsDashboard};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::saga::{AnalyticsDashboardData, PalletSaga};
use crate::transaction::Transaction;

use anyhow::Result;
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Network health metrics structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealth {
    pub block_count: usize,
    pub mempool_size: usize,
    pub utxo_count: usize,
    pub connected_peers: usize,
    pub sync_status: String,
}

/// WebSocket subscription types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionType {
    Blocks,
    Transactions,
    NetworkHealth,
    Analytics,
    Alerts,
    Economic,
    All,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    Subscribe {
        subscription_type: SubscriptionType,
        filters: Option<HashMap<String, String>>,
    },
    Unsubscribe {
        subscription_type: SubscriptionType,
    },
    BlockNotification {
        block: QantoBlock,
        timestamp: u64,
    },
    TransactionNotification {
        transaction: Transaction,
        status: String,
        timestamp: u64,
    },
    TransactionConfirmation {
        tx_id: String,
        block_id: String,
        confirmations: u32,
    },
    AnalyticsUpdate {
        data: Box<AnalyticsDashboardData>,
        timestamp: u64,
    },
    AlertNotification {
        alert: Alert,
        timestamp: u64,
    },
    NetworkHealthUpdate {
        tps: f64,
        block_time: f64,
        validator_count: u64,
        timestamp: u64,
    },
    NetworkHealth(NetworkHealth),
    MempoolUpdate {
        size: usize,
        timestamp: u64,
    },
    EconomicUpdate {
        total_supply: u64,
        market_cap: f64,
        staking_ratio: f64,
        timestamp: u64,
    },
    EconomicIndicators {
        total_supply: u64,
        circulating_supply: u64,
        market_metrics: serde_json::Value,
    },
    SecurityAlert {
        severity: String,
        message: String,
        timestamp: u64,
    },
    Error {
        message: String,
        code: u16,
    },
    SubscriptionConfirmed {
        subscription_type: SubscriptionType,
        client_id: String,
    },
    Pong,
}

/// Client connection information
#[derive(Debug, Clone)]
pub struct ClientConnection {
    subscriptions: Vec<SubscriptionType>,
    filters: HashMap<String, String>,
    connected_at: u64,
}

/// WebSocket server state
#[derive(Clone)]
pub struct WebSocketServerState {
    pub dag: Arc<QantoDAG>,
    pub saga: Arc<PalletSaga>,
    pub analytics: Arc<AnalyticsDashboard>,
    pub clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
    pub block_sender: broadcast::Sender<QantoBlock>,
    pub transaction_sender: broadcast::Sender<Transaction>,
    pub network_health_sender: broadcast::Sender<NetworkHealth>,
    pub mempool_sender: broadcast::Sender<usize>,
}

impl WebSocketServerState {
    pub fn new(
        dag: Arc<QantoDAG>,
        saga: Arc<PalletSaga>,
        analytics: Arc<AnalyticsDashboard>,
    ) -> Self {
        let (block_sender, _) = broadcast::channel(1000);
        let (transaction_sender, _) = broadcast::channel(1000);
        let (network_health_sender, _) = broadcast::channel(100);
        let (mempool_sender, _) = broadcast::channel(100);

        Self {
            dag,
            saga,
            analytics,
            clients: Arc::new(RwLock::new(HashMap::new())),
            block_sender,
            transaction_sender,
            network_health_sender,
            mempool_sender,
        }
    }

    /// Broadcast a new block to subscribed clients
    pub async fn broadcast_block(&self, block: QantoBlock) {
        if let Err(e) = self.block_sender.send(block) {
            warn!("Failed to broadcast block: {}", e);
        }
    }

    /// Broadcast block notification to subscribed clients
    pub async fn broadcast_block_notification(&self, block: &QantoBlock) {
        let _message = WebSocketMessage::BlockNotification {
            block: block.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        if let Err(e) = self.block_sender.send(block.clone()) {
            warn!("Failed to broadcast block notification: {}", e);
        }
    }

    /// Broadcast a new transaction to subscribed clients
    pub async fn broadcast_transaction(&self, transaction: Transaction) {
        if let Err(e) = self.transaction_sender.send(transaction) {
            warn!("Failed to broadcast transaction: {}", e);
        }
    }

    /// Broadcast analytics dashboard data to subscribed clients
    pub async fn broadcast_analytics_data(&self, data: &AnalyticsDashboardData) {
        let _message = WebSocketMessage::AnalyticsUpdate {
            data: Box::new(data.clone()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // This would typically broadcast through a dedicated analytics channel
        // For now, we'll log the broadcast attempt
        debug!("Broadcasting analytics data update");
    }

    /// Broadcast network health update to subscribed clients
    pub async fn broadcast_network_health(&self, health: NetworkHealth) {
        if self.network_health_sender.receiver_count() > 0 {
            if let Err(e) = self.network_health_sender.send(health) {
                warn!("Failed to broadcast network health: {}", e);
            }
        }
    }

    /// Broadcast mempool update to subscribed clients
    pub async fn broadcast_mempool_update(&self, size: usize) {
        if let Err(e) = self.mempool_sender.send(size) {
            warn!("Failed to broadcast mempool update: {}", e);
        }
    }

    /// Get connected client count
    pub async fn get_client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Get connection statistics including average connection duration
    pub async fn get_connection_stats(&self) -> (usize, f64, u64) {
        let clients = self.clients.read().await;
        let count = clients.len();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if count == 0 {
            return (0, 0.0, 0);
        }

        let total_duration: u64 = clients
            .values()
            .map(|client| current_time.saturating_sub(client.connected_at))
            .sum();
        let avg_duration = total_duration as f64 / count as f64;
        let oldest_connection = clients
            .values()
            .map(|client| current_time.saturating_sub(client.connected_at))
            .max()
            .unwrap_or(0);

        (count, avg_duration, oldest_connection)
    }

    /// Clean up stale connections (older than specified seconds)
    pub async fn cleanup_stale_connections(&self, max_age_seconds: u64) -> usize {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut clients = self.clients.write().await;
        let initial_count = clients.len();

        clients.retain(|client_id, client| {
            let age = current_time.saturating_sub(client.connected_at);
            if age > max_age_seconds {
                info!("Removing stale client {} (age: {}s)", client_id, age);
                false
            } else {
                true
            }
        });

        let removed_count = initial_count - clients.len();
        if removed_count > 0 {
            info!("Cleaned up {} stale connections", removed_count);
        }
        removed_count
    }

    async fn remove_client(&self, client_id: &str) {
        if let Some(client) = self.clients.write().await.remove(client_id) {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let duration = current_time.saturating_sub(client.connected_at);
            info!("Client {} disconnected after {}s", client_id, duration);
        }
    }
}

/// WebSocket upgrade handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketServerState>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle individual WebSocket connections
async fn handle_websocket(socket: WebSocket, state: WebSocketServerState) {
    let client_id = Uuid::new_v4().to_string();
    let client = ClientConnection {
        subscriptions: Vec::new(),
        filters: HashMap::new(),
        connected_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Add client to state
    state
        .clients
        .write()
        .await
        .insert(client_id.clone(), client);
    info!("New WebSocket client connected: {}", client_id);

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to various data streams
    let mut analytics_rx = state.analytics.subscribe_to_data();
    let mut alerts_rx = state.analytics.subscribe_to_alerts();
    let mut blocks_rx = state.block_sender.subscribe();
    let mut transactions_rx = state.transaction_sender.subscribe();

    // Create a channel for pong responses
    let (pong_tx, mut pong_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // Spawn task to handle incoming messages from client
    let state_clone = state.clone();
    let client_id_clone = client_id.clone();
    let mut incoming_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    if let Err(e) =
                        handle_client_message(&text, &client_id_clone, &state_clone).await
                    {
                        error!("Error handling client message: {}", e);
                    }
                }
                Ok(axum::extract::ws::Message::Ping(data)) => {
                    if let Err(e) = pong_tx.send(data.to_vec()) {
                        error!("Failed to send pong data: {}", e);
                        break;
                    }
                }
                Ok(axum::extract::ws::Message::Close(_)) => {
                    info!("Client {} requested close", client_id_clone);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error for client {}: {}", client_id_clone, e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Main message broadcasting loop
    let mut heartbeat_interval = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            // Handle analytics updates
            Ok(analytics_data) = analytics_rx.recv() => {
                if client_has_subscription(&state, &client_id, &SubscriptionType::Analytics).await {
                    let message = WebSocketMessage::AnalyticsUpdate {
                        data: Box::new(analytics_data),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    if let Err(e) = send_message(&mut sender, &message).await {
                        error!("Failed to send analytics update: {}", e);
                        break;
                    }
                }
            }

            // Handle alert notifications
            Ok(alert) = alerts_rx.recv() => {
                if client_has_subscription(&state, &client_id, &SubscriptionType::Alerts).await {
                    let message = WebSocketMessage::AlertNotification {
                        alert,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    if let Err(e) = send_message(&mut sender, &message).await {
                        error!("Failed to send alert: {}", e);
                        break;
                    }
                }
            }

            // Handle block notifications
            Ok(block) = blocks_rx.recv() => {
                if client_has_subscription(&state, &client_id, &SubscriptionType::Blocks).await {
                    let message = WebSocketMessage::BlockNotification {
                        block,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    if let Err(e) = send_message(&mut sender, &message).await {
                        error!("Failed to send block notification: {}", e);
                        break;
                    }
                }
            }

            // Handle transaction notifications
            Ok(transaction) = transactions_rx.recv() => {
                if client_has_subscription(&state, &client_id, &SubscriptionType::Transactions).await {
                    let message = WebSocketMessage::TransactionNotification {
                        transaction,
                        status: "confirmed".to_string(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    if let Err(e) = send_message(&mut sender, &message).await {
                        error!("Failed to send transaction notification: {}", e);
                        break;
                    }
                }
            }

            // Handle pong responses
            Some(pong_data) = pong_rx.recv() => {
                if let Err(e) = sender.send(axum::extract::ws::Message::Pong(pong_data.into())).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }

            // Heartbeat
            _ = heartbeat_interval.tick() => {
                if let Err(e) = sender.send(axum::extract::ws::Message::Ping(vec![].into())).await {
                    error!("Failed to send heartbeat ping: {}", e);
                    break;
                }
            }

            // Check if incoming task finished
            _ = &mut incoming_task => {
                debug!("Incoming message task finished for client {}", client_id);
                break;
            }
        }
    }

    // Cleanup
    state.remove_client(&client_id).await;
    incoming_task.abort();
}

/// Handle incoming client messages
async fn handle_client_message(
    text: &str,
    client_id: &str,
    state: &WebSocketServerState,
) -> Result<()> {
    let message: WebSocketMessage = serde_json::from_str(text)?;

    match message {
        WebSocketMessage::Subscribe {
            subscription_type,
            filters,
        } => {
            let mut clients = state.clients.write().await;
            if let Some(client) = clients.get_mut(client_id) {
                if !client.subscriptions.contains(&subscription_type) {
                    client.subscriptions.push(subscription_type.clone());
                }
                if let Some(f) = filters {
                    client.filters.extend(f);
                }
                info!("Client {} subscribed to {:?}", client_id, subscription_type);
            }
        }
        WebSocketMessage::Unsubscribe { subscription_type } => {
            let mut clients = state.clients.write().await;
            if let Some(client) = clients.get_mut(client_id) {
                client.subscriptions.retain(|s| s != &subscription_type);
                info!(
                    "Client {} unsubscribed from {:?}",
                    client_id, subscription_type
                );
            }
        }
        _ => {
            warn!("Unexpected message type from client {}", client_id);
        }
    }

    Ok(())
}

/// Check if client has a specific subscription
async fn client_has_subscription(
    state: &WebSocketServerState,
    client_id: &str,
    subscription_type: &SubscriptionType,
) -> bool {
    let clients = state.clients.read().await;
    if let Some(client) = clients.get(client_id) {
        client.subscriptions.contains(subscription_type)
            || client.subscriptions.contains(&SubscriptionType::All)
    } else {
        false
    }
}

/// Send a message to the WebSocket client
async fn send_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, axum::extract::ws::Message>,
    message: &WebSocketMessage,
) -> Result<()> {
    let json = serde_json::to_string(message)?;
    sender
        .send(axum::extract::ws::Message::Text(json.into()))
        .await?;
    Ok(())
}

/// Create WebSocket router
pub fn create_websocket_router(state: WebSocketServerState) -> Router {
    Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(state)
}

/// Start the WebSocket server
pub async fn start_websocket_server(port: u16, state: WebSocketServerState) -> Result<()> {
    let app = create_websocket_router(state);
    let mut addr = String::with_capacity(8 + port.to_string().len());
    addr.push_str("0.0.0.0:");
    addr.push_str(&port.to_string());

    info!("Starting WebSocket server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
