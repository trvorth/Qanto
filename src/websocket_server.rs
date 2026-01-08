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
use crate::rpc_backend::NodeRpcBackend;
use crate::saga::ThreatLevel;
use crate::saga::{AnalyticsDashboardData, PalletSaga};
use crate::transaction::Transaction;
use crate::types::WalletBalance;
use qanto_rpc::RpcBackend;

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
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Default maximum WebSocket message size (bytes) when not provided via config
pub const DEFAULT_MAX_WS_MESSAGE_BYTES: usize = 512 * 1024; // 512 KiB
/// Per-client buffer capacity for balance updates; prioritizes liveness over completeness
const BALANCE_OUTBOUND_BUFFER: usize = 256;

/// Network health metrics structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealth {
    pub block_count: usize,
    pub mempool_size: usize,
    pub utxo_count: usize,
    pub connected_peers: usize,
    pub sync_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceEvent {
    pub address: String,
    pub balance: WalletBalance,
    pub timestamp: u64,
    pub finalized: bool,
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
    Balances,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filters {
    address: Option<String>,
    finalized_only: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SubscriptionRequest {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default, deserialize_with = "deserialize_address_flexible")]
    address: Option<String>,
    #[serde(default)]
    filters: Option<Filters>,
}

fn deserialize_address_flexible<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as DeError;
    let v = serde_json::Value::deserialize(deserializer)?;
    if v.is_null() {
        return Ok(None);
    }
    if let Some(s) = v.as_str() {
        return Ok(Some(s.to_string()));
    }
    if let Some(obj) = v.as_object() {
        if let Some(filters) = obj.get("filters") {
            if let Some(addr) = filters.get("address").and_then(|a| a.as_str()) {
                return Ok(Some(addr.to_string()));
            }
        }
    }
    Err(DeError::custom("invalid address field"))
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    Subscribe {
        subscription_type: SubscriptionType,
        filters: Option<HashMap<String, String>>,
    },
    // Task 2: Repair WebSocket API Schema
    // Split generic Subscribe from MonitorAddress to allow optional/distinct schema
    MonitorAddress {
        address: Option<String>,
    },
    // Added missing variant for balance snapshot requests
    #[serde(rename = "balance_snapshot_request")]
    BalanceSnapshotRequest {
        #[serde(default)]
        address: Option<String>,
        #[serde(default)]
        filters: Option<Filters>,
        #[serde(default)]
        subscription_type: Option<String>,
    },
    Unsubscribe {
        subscription_type: SubscriptionType,
    },
    UnsubscribeBalance {
        address: String,
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
    AnalyticsSummaryUpdate {
        data: Box<AnalyticsSummary>,
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
    BalanceUpdate {
        address: String,
        balance: WalletBalance,
        timestamp: u64,
        finalized: bool,
    },
    BalanceSnapshot {
        address: String,
        total_confirmed: u64,
        total_unconfirmed: u64,
        timestamp: u64,
    },
    BalanceDeltaUpdate {
        address: String,
        delta_confirmed: i64,
        delta_unconfirmed: i64,
        timestamp: u64,
        finalized: bool,
    },
    Error {
        message: String,
        code: u16,
    },
    SubscriptionConfirmed {
        subscription_type: SubscriptionType,
        client_id: String,
    },
    BalanceSubscriptionConfirmed {
        address: String,
        client_id: String,
    },
    ChunkStart {
        id: String,
        total: usize,
        content_type: String,
    },
    ChunkData {
        id: String,
        index: usize,
        data: String,
    },
    ChunkEnd {
        id: String,
    },
    Pong,
}

/// Compact analytics summary that fits within conservative WebSocket limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsSummary {
    // Core chain stats
    pub block_height: u64,
    pub mempool_size: u64,
    pub total_transactions: u64,
    pub active_addresses: u64,
    pub tps_current: f64,
    pub tps_peak: f64,

    // AI performance (key signals only)
    pub nn_accuracy: f64,
    pub inference_latency_ms: f64,
    pub model_drift_score: f64,

    // Security
    pub threat_level: ThreatLevel,
    pub anomaly_score: f64,
    pub suspicious_patterns_top: Vec<String>,

    // Economic indicators (key values only)
    pub total_value_locked: f64,
    pub transaction_fees_24h: f64,
    pub validator_rewards_24h: f64,

    // Environmental snapshot
    pub carbon_footprint_kg: f64,
    pub energy_efficiency_score: f64,
    pub renewable_energy_percentage: f64,
}

fn summarize_analytics_data(data: &AnalyticsDashboardData) -> AnalyticsSummary {
    // Trim suspicious patterns to top 3, each capped to 64 chars to contain payload size
    let mut patterns: Vec<String> = data
        .security_insights
        .suspicious_patterns
        .iter()
        .take(3)
        .map(|s| {
            if s.len() > 64 {
                let mut truncated = s[..64].to_string();
                truncated.push('…');
                truncated
            } else {
                s.clone()
            }
        })
        .collect();
    // Ensure deterministic ordering to avoid payload churn
    patterns.sort();

    AnalyticsSummary {
        // Core
        block_height: data.block_height,
        mempool_size: data.mempool_size,
        total_transactions: data.total_transactions,
        active_addresses: data.active_addresses,
        tps_current: data.tps_current,
        tps_peak: data.tps_peak,

        // AI
        nn_accuracy: data.ai_performance.neural_network_accuracy,
        inference_latency_ms: data.ai_performance.inference_latency_ms,
        model_drift_score: data.ai_performance.model_drift_score,

        // Security
        threat_level: data.security_insights.threat_level.clone(),
        anomaly_score: data.security_insights.anomaly_score,
        suspicious_patterns_top: patterns,

        // Economics
        total_value_locked: data.economic_indicators.total_value_locked,
        transaction_fees_24h: data.economic_indicators.transaction_fees_24h,
        validator_rewards_24h: data.economic_indicators.validator_rewards_24h,

        // Environmental
        carbon_footprint_kg: data.environmental_metrics.carbon_footprint_kg,
        energy_efficiency_score: data.environmental_metrics.energy_efficiency_score,
        renewable_energy_percentage: data.environmental_metrics.renewable_energy_percentage,
    }
}

fn build_analytics_message_with_fallback(
    state: &WebSocketServerState,
    data: &AnalyticsDashboardData,
) -> WebSocketMessage {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Attempt full payload first
    let full_msg = WebSocketMessage::AnalyticsUpdate {
        data: Box::new(data.clone()),
        timestamp: ts,
    };

    // Serialize to estimate size
    match serde_json::to_string(&full_msg) {
        Ok(json) => {
            if json.len() <= state.max_message_bytes {
                full_msg
            } else {
                // Build compact summary version
                let mut summary = summarize_analytics_data(data);
                let mut summary_msg = WebSocketMessage::AnalyticsSummaryUpdate {
                    data: Box::new(summary.clone()),
                    timestamp: ts,
                };
                // Ensure summary fits; if not, compact further by dropping patterns
                if let Ok(sjson) = serde_json::to_string(&summary_msg) {
                    if sjson.len() > state.max_message_bytes {
                        summary.suspicious_patterns_top.clear();
                        summary_msg = WebSocketMessage::AnalyticsSummaryUpdate {
                            data: Box::new(summary.clone()),
                            timestamp: ts,
                        };
                        if let Ok(sjson2) = serde_json::to_string(&summary_msg) {
                            if sjson2.len() > state.max_message_bytes {
                                // Final compact fallback: core metrics only
                                let compact = AnalyticsSummary {
                                    block_height: summary.block_height,
                                    mempool_size: summary.mempool_size,
                                    total_transactions: summary.total_transactions,
                                    active_addresses: summary.active_addresses,
                                    tps_current: summary.tps_current,
                                    tps_peak: summary.tps_peak,
                                    nn_accuracy: summary.nn_accuracy,
                                    inference_latency_ms: summary.inference_latency_ms,
                                    model_drift_score: summary.model_drift_score,
                                    threat_level: summary.threat_level.clone(),
                                    anomaly_score: summary.anomaly_score,
                                    suspicious_patterns_top: Vec::new(),
                                    total_value_locked: 0.0,
                                    transaction_fees_24h: 0.0,
                                    validator_rewards_24h: 0.0,
                                    carbon_footprint_kg: 0.0,
                                    energy_efficiency_score: 0.0,
                                    renewable_energy_percentage: 0.0,
                                };
                                summary_msg = WebSocketMessage::AnalyticsSummaryUpdate {
                                    data: Box::new(compact),
                                    timestamp: ts,
                                };
                            }
                        }
                    }
                }
                summary_msg
            }
        }
        Err(_) => {
            // On serialization error, fall back to summary to preserve liveness
            let summary = summarize_analytics_data(data);
            WebSocketMessage::AnalyticsSummaryUpdate {
                data: Box::new(summary),
                timestamp: ts,
            }
        }
    }
}

/// Client connection information
#[derive(Debug, Clone)]
pub struct ClientConnection {
    subscriptions: Vec<SubscriptionType>,
    balance_subscriptions: Vec<String>, // Addresses for balance subscriptions
    filters: HashMap<String, String>,
    connected_at: u64,
    last_balances: HashMap<String, (u64, u64)>,
    /// Per-client bounded outbound queue for balance updates
    #[allow(dead_code)]
    // TODO: Wire up per-client balance outbound channel to enforce backpressure.
    // Currently unused; retained for planned bounded buffering of balance updates.
    balance_outbound_tx: Option<mpsc::Sender<WebSocketMessage>>,
}

/// WebSocket server state
#[derive(Clone)]
pub struct WebSocketServerState {
    pub dag: Arc<QantoDAG>,
    pub saga: Arc<PalletSaga>,
    pub analytics: Arc<AnalyticsDashboard>,
    pub balances_index: Arc<RwLock<HashMap<String, u64>>>,
    pub clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
    pub block_sender: broadcast::Sender<QantoBlock>,
    pub transaction_sender: broadcast::Sender<Transaction>,
    pub network_health_sender: broadcast::Sender<NetworkHealth>,
    pub mempool_sender: broadcast::Sender<usize>,
    pub balance_sender: broadcast::Sender<BalanceEvent>,
    pub rpc_backend: Arc<RwLock<Option<Arc<NodeRpcBackend>>>>,
    // Count warnings for dropped outbound messages due to size cap
    pub dropped_outbound_warnings: Arc<AtomicU64>,
    // Maximum allowed bytes for any inbound or outbound WebSocket message
    pub max_message_bytes: usize,
}

impl WebSocketServerState {
    pub fn new(
        dag: Arc<QantoDAG>,
        saga: Arc<PalletSaga>,
        analytics: Arc<AnalyticsDashboard>,
        balances_index: Arc<RwLock<HashMap<String, u64>>>,
        max_message_bytes: usize,
    ) -> Self {
        let (block_sender, _) = broadcast::channel(1000);
        let (transaction_sender, _) = broadcast::channel(1000);
        let (network_health_sender, _) = broadcast::channel(100);
        let (mempool_sender, _) = broadcast::channel(100);
        let (balance_sender, _) = broadcast::channel(1000);

        Self {
            dag,
            saga,
            analytics,
            balances_index,
            clients: Arc::new(RwLock::new(HashMap::new())),
            block_sender,
            transaction_sender,
            network_health_sender,
            mempool_sender,
            balance_sender,
            rpc_backend: Arc::new(RwLock::new(None)),
            dropped_outbound_warnings: Arc::new(AtomicU64::new(0)),
            max_message_bytes,
        }
    }

    /// Attach the Node RPC backend used for balance queries
    pub async fn set_rpc_backend(&self, backend: Arc<NodeRpcBackend>) {
        let mut guard = self.rpc_backend.write().await;
        *guard = Some(backend);
    }

    /// Get count of dropped outbound message warnings
    pub fn get_dropped_outbound_warnings(&self) -> u64 {
        self.dropped_outbound_warnings.load(Ordering::Relaxed)
    }

    pub async fn broadcast_balance_update(&self, address: String, balance: WalletBalance) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let event = BalanceEvent {
            address,
            balance,
            timestamp,
            finalized: false,
        };
        if let Err(e) = self.balance_sender.send(event) {
            warn!("Failed to broadcast balance update: {}", e);
        }
    }

    /// Broadcast a balance update with explicit finalized status
    pub async fn broadcast_balance_update_with_finalized(
        &self,
        address: String,
        balance: WalletBalance,
        finalized: bool,
    ) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let event = BalanceEvent {
            address,
            balance,
            timestamp,
            finalized,
        };
        if let Err(e) = self.balance_sender.send(event) {
            warn!("Failed to broadcast balance update: {}", e);
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

    pub async fn get_balance_subscription_addresses(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        let mut addrs = Vec::new();
        for client in clients.values() {
            if client.subscriptions.contains(&SubscriptionType::Balances)
                || client.subscriptions.contains(&SubscriptionType::All)
            {
                if let Some(addr) = client.filters.get("address") {
                    addrs.push(addr.clone());
                }
            }
        }
        addrs
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
    // Per-client bounded channel for balance updates (coalesced via drop-old policy)
    let (balance_outbound_tx, mut balance_outbound_rx) =
        mpsc::channel::<WebSocketMessage>(BALANCE_OUTBOUND_BUFFER);
    let client = ClientConnection {
        subscriptions: Vec::new(),
        balance_subscriptions: Vec::new(),
        filters: HashMap::new(),
        connected_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        last_balances: HashMap::new(),
        balance_outbound_tx: Some(balance_outbound_tx.clone()),
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
    let mut balances_rx = state.balance_sender.subscribe();

    // Create a channel for pong responses
    let (pong_tx, mut pong_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // Spawn task to handle incoming messages from client
    let state_clone = state.clone();
    let client_id_clone = client_id.clone();
    let mut incoming_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    // Enforce inbound message size limit
                    if text.len() > state_clone.max_message_bytes {
                        warn!(
                            target: "websocket",
                            size = text.len(),
                            max = state_clone.max_message_bytes,
                            "Inbound WebSocket message exceeds configured max size; closing"
                        );
                        // Attempt graceful close
                        // Note: cannot access sender here; break incoming task
                        break;
                    }
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
    let mut heartbeat_interval = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            // Handle analytics updates
            Ok(analytics_data) = analytics_rx.recv() => {
                if client_has_subscription(&state, &client_id, &SubscriptionType::Analytics).await {
                    let message = build_analytics_message_with_fallback(&state, &analytics_data);
                    if let Err(e) = send_message(&state, &mut sender, &message).await {
                        error!("Failed to send analytics update: {}", e);
                        break;
                    }
                }
            }
            // Handle balance updates
            Ok(balance_event) = balances_rx.recv() => {
                if client_has_subscription(&state, &client_id, &SubscriptionType::Balances).await {
                    // Check address filter if present
                    let mut allow = true;
                    if let Some(client) = state.clients.read().await.get(&client_id) {
                        if let Some(addr_filter) = client.filters.get("address") {
                            allow = addr_filter == &balance_event.address;
                        }
                        // Optional finalized-only filter
                        if allow {
                            if let Some(final_only) = client.filters.get("finalized_only") {
                                if final_only == "true" && !balance_event.finalized {
                                    allow = false;
                                }
                            }
                        }
                    }
                    if allow {
                        let (delta_confirmed, delta_unconfirmed) = {
                            let mut clients = state.clients.write().await;
                            if let Some(client) = clients.get_mut(&client_id) {
                                let prev = client
                                    .last_balances
                                    .get(&balance_event.address)
                                    .copied()
                                    .unwrap_or((0, 0));
                                let mut dc = balance_event.balance.total_confirmed as i128 - prev.0 as i128;
                                let du = balance_event.balance.unconfirmed_delta as i128;
                                let new_unconfirmed_total_i128 = prev.1 as i128 + du;
                                let new_unconfirmed_total_u64 = if new_unconfirmed_total_i128 < 0 { 0 } else { new_unconfirmed_total_i128 as u64 };
                                if prev.0 > 0 && dc.unsigned_abs() >= prev.0 as u128 {
                                    dc = 0;
                                }
                                client
                                    .last_balances
                                    .insert(balance_event.address.clone(), (balance_event.balance.total_confirmed, new_unconfirmed_total_u64));
                                (dc as i64, du as i64)
                            } else {
                                (balance_event.balance.total_confirmed as i64, balance_event.balance.unconfirmed_delta as i64)
                            }
                        };
                        let message = WebSocketMessage::BalanceDeltaUpdate {
                            address: balance_event.address.clone(),
                            delta_confirmed,
                            delta_unconfirmed,
                            timestamp: balance_event.timestamp,
                            finalized: balance_event.finalized,
                        };
                        // Try to enqueue to per-client buffer; on full, drop oldest and retry
                        match balance_outbound_tx.try_send(message.clone()) {
                            Ok(_) => {}
                            Err(TrySendError::Full(_)) => {
                                // Drop oldest to prioritize liveness
                                let _ = balance_outbound_rx.try_recv();
                                let _ = balance_outbound_tx.try_send(message);
                                state
                                    .dropped_outbound_warnings
                                    .fetch_add(1, Ordering::Relaxed);
                            }
                            Err(TrySendError::Closed(_)) => {
                                warn!("Balance outbound channel closed for client {}", client_id);
                                break;
                            }
                        }
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
                    if let Err(e) = send_message(&state, &mut sender, &message).await {
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
                    if let Err(e) = send_message(&state, &mut sender, &message).await {
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
                    if let Err(e) = send_message(&state, &mut sender, &message).await {
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

            // Drain per-client balance outbound buffer and send to socket
            Some(out_msg) = balance_outbound_rx.recv() => {
                if let Err(e) = send_message(&state, &mut sender, &out_msg).await {
                    error!("Failed to deliver buffered balance update: {}", e);
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
    match serde_json::from_str::<SubscriptionRequest>(text) {
        Ok(req) => {
            if req.r#type == "subscribe" {
                let mut clients = state.clients.write().await;
                if let Some(client) = clients.get_mut(client_id) {
                    let mut addr_opt = req.address.clone();
                    if addr_opt.is_none() {
                        addr_opt = req.filters.as_ref().and_then(|f| f.address.clone());
                    }
                    if let Some(addr) = addr_opt {
                        if !client.subscriptions.contains(&SubscriptionType::Balances) {
                            client.subscriptions.push(SubscriptionType::Balances);
                        }
                        client.filters.insert("address".to_string(), addr.clone());
                        if let Some(f) = req.filters.as_ref().and_then(|f| f.finalized_only.clone())
                        {
                            client.filters.insert("finalized_only".to_string(), f);
                        }
                        info!("Client subscribed");
                        if let Some(tx) = client.balance_outbound_tx.clone() {
                            let confirm = WebSocketMessage::BalanceSubscriptionConfirmed {
                                address: addr.clone(),
                                client_id: client_id.to_string(),
                            };
                            let _ = tx.try_send(confirm);
                            let finalized_only = client
                                .filters
                                .get("finalized_only")
                                .map(|v| v == "true")
                                .unwrap_or(false);
                            if !finalized_only {
                                let balance: WalletBalance = if let Some(backend) =
                                    state.rpc_backend.read().await.as_ref().cloned()
                                {
                                    match backend.query_wallet_balance(addr.clone()).await {
                                        Ok(tuple) => WalletBalance::from(tuple),
                                        Err(_e) => {
                                            let current = {
                                                let idx = state.balances_index.read().await;
                                                idx.get(&addr).copied().unwrap_or(0)
                                            };
                                            WalletBalance {
                                                spendable_confirmed: current,
                                                immature_coinbase_confirmed: 0,
                                                unconfirmed_delta: 0,
                                                total_confirmed: current,
                                            }
                                        }
                                    }
                                } else {
                                    let current = {
                                        let idx = state.balances_index.read().await;
                                        idx.get(&addr).copied().unwrap_or(0)
                                    };
                                    WalletBalance {
                                        spendable_confirmed: current,
                                        immature_coinbase_confirmed: 0,
                                        unconfirmed_delta: 0,
                                        total_confirmed: current,
                                    }
                                };
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                let initial = WebSocketMessage::BalanceSnapshot {
                                    address: addr.clone(),
                                    total_confirmed: balance.total_confirmed,
                                    total_unconfirmed: 0,
                                    timestamp: ts,
                                };
                                client
                                    .last_balances
                                    .insert(addr.clone(), (balance.total_confirmed, 0));
                                let _ = tx.try_send(initial);
                            }
                        }
                        return Ok(());
                    }
                }
            }
        }
        Err(e) => {
            warn!("WS Handshake parse error: {} | Payload: {}", e, text);
        }
    }
    // First handle alias topic form: { "type": "subscribe", "topics": ["wallet_balance:<addr>"] }
    // This provides a friendly shorthand for balance subscriptions.
    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(text) {
        if raw.get("type").and_then(|t| t.as_str()) == Some("subscribe") {
            if let Some(topics) = raw.get("topics").and_then(|t| t.as_array()) {
                let mut handled_alias = false;
                for topic in topics {
                    if let Some(tstr) = topic.as_str() {
                        if let Some(rest) = tstr.strip_prefix("wallet_balance:") {
                            let addr = rest.to_string();
                            let mut clients = state.clients.write().await;
                            if let Some(client) = clients.get_mut(client_id) {
                                // Ensure balances subscription is active
                                if !client.subscriptions.contains(&SubscriptionType::Balances) {
                                    client.subscriptions.push(SubscriptionType::Balances);
                                }
                                // Set address filter (single-address semantics)
                                client.filters.insert("address".to_string(), addr.clone());
                                handled_alias = true;
                                info!("Client {} subscribed via alias wallet_balance", client_id);

                                // Emit confirmation and initial snapshot (skip initial when finalized_only=true)
                                if let Some(tx) = client.balance_outbound_tx.clone() {
                                    let confirm = WebSocketMessage::BalanceSubscriptionConfirmed {
                                        address: addr.clone(),
                                        client_id: client_id.to_string(),
                                    };
                                    // Best-effort enqueue; on backpressure we keep liveness
                                    let _ = tx.try_send(confirm);
                                    let finalized_only = client
                                        .filters
                                        .get("finalized_only")
                                        .map(|v| v == "true")
                                        .unwrap_or(false);
                                    if !finalized_only {
                                        // Prefer RPC backend breakdown; fallback to index total
                                        let balance: WalletBalance = if let Some(backend) =
                                            state.rpc_backend.read().await.as_ref().cloned()
                                        {
                                            match backend.query_wallet_balance(addr.clone()).await {
                                                Ok(tuple) => WalletBalance::from(tuple),
                                                Err(e) => {
                                                    warn!("RPC backend query_wallet_balance failed for {}: {}", addr, e);
                                                    let current = {
                                                        let idx = state.balances_index.read().await;
                                                        idx.get(&addr).copied().unwrap_or(0)
                                                    };
                                                    WalletBalance {
                                                        spendable_confirmed: current,
                                                        immature_coinbase_confirmed: 0,
                                                        unconfirmed_delta: 0,
                                                        total_confirmed: current,
                                                    }
                                                }
                                            }
                                        } else {
                                            let current = {
                                                let idx = state.balances_index.read().await;
                                                idx.get(&addr).copied().unwrap_or(0)
                                            };
                                            WalletBalance {
                                                spendable_confirmed: current,
                                                immature_coinbase_confirmed: 0,
                                                unconfirmed_delta: 0,
                                                total_confirmed: current,
                                            }
                                        };
                                        let ts = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let initial = WebSocketMessage::BalanceSnapshot {
                                            address: addr.clone(),
                                            total_confirmed: balance.total_confirmed,
                                            total_unconfirmed: 0,
                                            timestamp: ts,
                                        };
                                        // record last totals (confirmed total, unconfirmed total starts at 0)
                                        if let Some(client) = clients.get_mut(client_id) {
                                            client
                                                .last_balances
                                                .insert(addr.clone(), (balance.total_confirmed, 0));
                                        }
                                        match tx.try_send(initial) {
                                            Ok(_) => {}
                                            Err(tokio::sync::mpsc::error::TrySendError::Full(
                                                _,
                                            )) => {
                                                warn!("Initial balance snapshot channel full; skipping for client {}", client_id);
                                            }
                                            Err(e) => {
                                                warn!("Failed to enqueue initial balance snapshot: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if handled_alias {
                    return Ok(());
                }
            }
        }
    }

    // Fall back to structured message handling
    let message: WebSocketMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            warn!("WS Handshake parse error: {} | Payload: {}", e, text);
            return Ok(());
        }
    };

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
                info!("Client subscribed");

                // If balances subscription with an address filter, emit confirmation and initial snapshot
                if subscription_type == SubscriptionType::Balances {
                    if let Some(addr) = client.filters.get("address").cloned() {
                        if let Some(tx) = client.balance_outbound_tx.clone() {
                            let confirm = WebSocketMessage::BalanceSubscriptionConfirmed {
                                address: addr.clone(),
                                client_id: client_id.to_string(),
                            };
                            let _ = tx.try_send(confirm);
                            let finalized_only = client
                                .filters
                                .get("finalized_only")
                                .map(|v| v == "true")
                                .unwrap_or(false);
                            if !finalized_only {
                                // Prefer RPC backend breakdown; fallback to index total
                                let balance: WalletBalance = if let Some(backend) =
                                    state.rpc_backend.read().await.as_ref().cloned()
                                {
                                    match backend.query_wallet_balance(addr.clone()).await {
                                        Ok(tuple) => WalletBalance::from(tuple),
                                        Err(e) => {
                                            warn!("RPC backend query_wallet_balance failed for {}: {}", addr, e);
                                            let current = {
                                                let idx = state.balances_index.read().await;
                                                idx.get(&addr).copied().unwrap_or(0)
                                            };
                                            WalletBalance {
                                                spendable_confirmed: current,
                                                immature_coinbase_confirmed: 0,
                                                unconfirmed_delta: 0,
                                                total_confirmed: current,
                                            }
                                        }
                                    }
                                } else {
                                    let current = {
                                        let idx = state.balances_index.read().await;
                                        idx.get(&addr).copied().unwrap_or(0)
                                    };
                                    WalletBalance {
                                        spendable_confirmed: current,
                                        immature_coinbase_confirmed: 0,
                                        unconfirmed_delta: 0,
                                        total_confirmed: current,
                                    }
                                };
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                let initial = WebSocketMessage::BalanceSnapshot {
                                    address: addr.clone(),
                                    total_confirmed: balance.total_confirmed,
                                    total_unconfirmed: 0,
                                    timestamp: ts,
                                };
                                if let Some(client) = clients.get_mut(client_id) {
                                    client
                                        .last_balances
                                        .insert(addr.clone(), (balance.total_confirmed, 0));
                                }
                                match tx.try_send(initial) {
                                    Ok(_) => {}
                                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                        warn!("Initial balance snapshot channel full; skipping for client {}", client_id);
                                    }
                                    Err(e) => {
                                        warn!("Failed to enqueue initial balance snapshot: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        WebSocketMessage::MonitorAddress { address } => {
            let mut clients = state.clients.write().await;
            if let Some(client) = clients.get_mut(client_id) {
                let addr = address.clone().unwrap_or_default(); // Handle optional address
                if !client.balance_subscriptions.contains(&addr) {
                    client.balance_subscriptions.push(addr.clone());
                    // Also enable balances subscription for delivery
                    if !client.subscriptions.contains(&SubscriptionType::Balances) {
                        client.subscriptions.push(SubscriptionType::Balances);
                    }
                    // Apply address filter to ensure updates flow through existing pipeline
                    client.filters.insert("address".to_string(), addr.clone());
                    info!(
                        "Client {} subscribed to balance updates for {}",
                        client_id, addr
                    );

                    // Emit confirmation and initial snapshot (respect finalized_only)
                    if let Some(tx) = client.balance_outbound_tx.clone() {
                        let confirm = WebSocketMessage::BalanceSubscriptionConfirmed {
                            address: addr.clone(),
                            client_id: client_id.to_string(),
                        };
                        let _ = tx.try_send(confirm);
                        let _finalized_only = client
                            .filters
                            .get("finalized_only")
                            .map(|v| v == "true")
                            .unwrap_or(false);

                        // Force immediate snapshot regardless of finalized_only filter for initial sync
                        // Prefer RPC backend breakdown; fallback to index total
                        let balance: WalletBalance = if let Some(backend) =
                            state.rpc_backend.read().await.as_ref().cloned()
                        {
                            match backend.query_wallet_balance(addr.clone()).await {
                                Ok(tuple) => WalletBalance::from(tuple),
                                Err(e) => {
                                    warn!(
                                        "RPC backend query_wallet_balance failed for {}: {}",
                                        addr, e
                                    );
                                    let current = {
                                        let idx = state.balances_index.read().await;
                                        idx.get(&addr).copied().unwrap_or(0)
                                    };
                                    WalletBalance {
                                        spendable_confirmed: current,
                                        immature_coinbase_confirmed: 0,
                                        unconfirmed_delta: 0,
                                        total_confirmed: current,
                                    }
                                }
                            }
                        } else {
                            let current = {
                                let idx = state.balances_index.read().await;
                                idx.get(&addr).copied().unwrap_or(0)
                            };
                            WalletBalance {
                                spendable_confirmed: current,
                                immature_coinbase_confirmed: 0,
                                unconfirmed_delta: 0,
                                total_confirmed: current,
                            }
                        };
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let initial = WebSocketMessage::BalanceSnapshot {
                            address: addr.clone(),
                            total_confirmed: balance.total_confirmed,
                            total_unconfirmed: 0,
                            timestamp: ts,
                        };
                        if let Some(client) = clients.get_mut(client_id) {
                            client
                                .last_balances
                                .insert(addr.clone(), (balance.total_confirmed, 0));
                        }
                        match tx.try_send(initial) {
                            Ok(_) => {}
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                warn!(
                                    "Initial balance snapshot channel full; skipping for client {}",
                                    client_id
                                );
                            }
                            Err(e) => {
                                warn!("Failed to enqueue initial balance snapshot: {}", e);
                            }
                        }
                    }
                }
            }
        }
        WebSocketMessage::BalanceSnapshotRequest {
            address,
            filters,
            subscription_type: _,
        } => {
            let clients = state.clients.read().await;
            if let Some(client) = clients.get(client_id) {
                if let Some(tx) = client.balance_outbound_tx.clone() {
                    let resolved_address = address
                        .clone()
                        .or_else(|| filters.as_ref().and_then(|f| f.address.clone()));
                    if let Some(addr) = resolved_address {
                        let balance: WalletBalance = if let Some(backend) =
                            state.rpc_backend.read().await.as_ref().cloned()
                        {
                            match backend.query_wallet_balance(addr.clone()).await {
                                Ok(tuple) => WalletBalance::from(tuple),
                                Err(e) => {
                                    warn!(
                                        "RPC backend query_wallet_balance failed for {}: {}",
                                        addr, e
                                    );
                                    let current = {
                                        let idx = state.balances_index.read().await;
                                        idx.get(&addr).copied().unwrap_or(0)
                                    };
                                    WalletBalance {
                                        spendable_confirmed: current,
                                        immature_coinbase_confirmed: 0,
                                        unconfirmed_delta: 0,
                                        total_confirmed: current,
                                    }
                                }
                            }
                        } else {
                            let current = {
                                let idx = state.balances_index.read().await;
                                idx.get(&addr).copied().unwrap_or(0)
                            };
                            WalletBalance {
                                spendable_confirmed: current,
                                immature_coinbase_confirmed: 0,
                                unconfirmed_delta: 0,
                                total_confirmed: current,
                            }
                        };

                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        let snapshot = WebSocketMessage::BalanceSnapshot {
                            address: addr.clone(),
                            total_confirmed: balance.total_confirmed,
                            total_unconfirmed: 0, // We don't track unconfirmed total in index currently
                            timestamp: ts,
                        };

                        match tx.try_send(snapshot) {
                            Ok(_) => {}
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                warn!(
                                    "Balance snapshot channel full; skipping for client {}",
                                    client_id
                                );
                            }
                            Err(e) => {
                                warn!("Failed to enqueue balance snapshot: {}", e);
                            }
                        }
                    }
                }
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
        WebSocketMessage::UnsubscribeBalance { address } => {
            let mut clients = state.clients.write().await;
            if let Some(client) = clients.get_mut(client_id) {
                client.balance_subscriptions.retain(|a| a != &address);
                info!(
                    "Client {} unsubscribed from balance updates for {}",
                    client_id, address
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
    state: &WebSocketServerState,
    sender: &mut futures_util::stream::SplitSink<WebSocket, axum::extract::ws::Message>,
    message: &WebSocketMessage,
) -> Result<()> {
    let json = serde_json::to_string(message)?;
    if json.len() <= state.max_message_bytes {
        sender
            .send(axum::extract::ws::Message::Text(json.into()))
            .await?;
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    let overhead = 256usize;
    let max_chunk_payload = state.max_message_bytes.saturating_sub(overhead).max(1);
    let mut chunks: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let bytes = json.as_bytes();
    while start < bytes.len() {
        let end = (start + max_chunk_payload).min(bytes.len());
        let slice = &json[start..end];
        chunks.push(slice);
        start = end;
    }
    let total = chunks.len();

    let start_msg = WebSocketMessage::ChunkStart {
        id: id.clone(),
        total,
        content_type: "application/json".to_string(),
    };
    let start_json = serde_json::to_string(&start_msg)?;
    sender
        .send(axum::extract::ws::Message::Text(start_json.into()))
        .await?;

    for (index, chunk) in chunks.into_iter().enumerate() {
        let data_msg = WebSocketMessage::ChunkData {
            id: id.clone(),
            index,
            data: chunk.to_string(),
        };
        let data_json = serde_json::to_string(&data_msg)?;
        sender
            .send(axum::extract::ws::Message::Text(data_json.into()))
            .await?;
    }

    let end_msg = WebSocketMessage::ChunkEnd { id };
    let end_json = serde_json::to_string(&end_msg)?;
    sender
        .send(axum::extract::ws::Message::Text(end_json.into()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics_dashboard::DashboardConfig;
    use crate::performance_optimizations::QantoDAGOptimizations;

    #[tokio::test]
    async fn websocket_state_initializes_with_dummy_dag() {
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

        let balances_index: Arc<RwLock<HashMap<String, u64>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let state = WebSocketServerState::new(
            dag.clone(),
            saga.clone(),
            analytics.clone(),
            balances_index,
            DEFAULT_MAX_WS_MESSAGE_BYTES,
        );

        assert_eq!(state.clients.read().await.len(), 0);
        assert_eq!(state.block_sender.receiver_count(), 0);
        assert_eq!(state.transaction_sender.receiver_count(), 0);
        assert_eq!(state.network_health_sender.receiver_count(), 0);
        assert_eq!(state.mempool_sender.receiver_count(), 0);
        assert_eq!(state.balance_sender.receiver_count(), 0);
    }

    #[tokio::test]
    async fn broadcasts_reach_subscribers() {
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
        let balances_index: Arc<RwLock<HashMap<String, u64>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let state = WebSocketServerState::new(
            dag,
            saga,
            analytics,
            balances_index,
            DEFAULT_MAX_WS_MESSAGE_BYTES,
        );

        let mut nh_rx = state.network_health_sender.subscribe();
        let mut bal_rx = state.balance_sender.subscribe();

        let health = NetworkHealth {
            block_count: 0,
            mempool_size: 0,
            utxo_count: 0,
            connected_peers: 0,
            sync_status: "ok".to_string(),
        };
        state.broadcast_network_health(health.clone()).await;
        let received_health = tokio::time::timeout(std::time::Duration::from_secs(1), nh_rx.recv())
            .await
            .expect("timeout waiting for network health")
            .expect("channel closed");
        assert_eq!(received_health.sync_status, health.sync_status);

        let wb = WalletBalance {
            spendable_confirmed: 42,
            immature_coinbase_confirmed: 0,
            unconfirmed_delta: 0,
            total_confirmed: 42,
        };
        state
            .broadcast_balance_update("addr1".to_string(), wb)
            .await;
        let received_bal = tokio::time::timeout(std::time::Duration::from_secs(1), bal_rx.recv())
            .await
            .expect("timeout waiting for balance update")
            .expect("channel closed");
        assert_eq!(received_bal.address, "addr1");
        assert_eq!(received_bal.balance.total_confirmed, 42);
        assert!(!received_bal.finalized);
    }

    #[tokio::test]
    async fn analytics_summary_fallback_on_oversize() {
        // Create minimal state with very small max outbound size to force fallback
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
        let balances_index: Arc<RwLock<HashMap<String, u64>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let state = WebSocketServerState::new(
            dag,
            saga,
            analytics,
            balances_index,
            512, // 512 bytes to trigger fallback
        );

        // Construct an oversized analytics payload
        let mut suspicious = Vec::new();
        for i in 0..50 {
            suspicious.push(format!("pattern_{}:{}", i, "x".repeat(200)));
        }

        let data = AnalyticsDashboardData {
            timestamp: 0,
            network_health: crate::metrics::QantoMetrics::new(),
            ai_performance: crate::saga::AIModelPerformance {
                neural_network_accuracy: 0.987,
                prediction_confidence: 0.92,
                training_loss: 0.12,
                validation_loss: 0.15,
                model_drift_score: 0.07,
                inference_latency_ms: 12.4,
                last_retrain_epoch: 42,
                feature_importance: (0..300).map(|i| (format!("f{}", i), i as f64)).collect(),
            },
            security_insights: crate::saga::SecurityInsights {
                threat_level: ThreatLevel::Medium,
                anomaly_score: 0.23,
                attack_attempts_24h: 17,
                blocked_transactions: 3,
                suspicious_patterns: suspicious,
                security_confidence: 0.95,
            },
            economic_indicators: crate::saga::EconomicIndicators {
                total_value_locked: 12_345_678.0,
                transaction_fees_24h: 1234.56,
                validator_rewards_24h: 9876.54,
                network_utilization: 0.76,
                economic_security: 0.88,
                fee_market_efficiency: 0.67,
            },
            environmental_metrics: crate::saga::EnvironmentalDashboardMetrics {
                carbon_footprint_kg: 12345.0,
                energy_efficiency_score: 0.83,
                renewable_energy_percentage: 42.0,
                carbon_offset_credits: 123.0,
                green_validator_ratio: 0.34,
            },
            total_transactions: 10_000,
            active_addresses: 1_234,
            mempool_size: 5_678,
            block_height: 9_999,
            tps_current: 123.4,
            tps_peak: 456.7,
        };

        let msg = build_analytics_message_with_fallback(&state, &data);
        // Expect summary variant chosen
        match msg {
            WebSocketMessage::AnalyticsSummaryUpdate { .. } => {
                let json = serde_json::to_string(&msg).unwrap();
                assert!(json.len() <= state.max_message_bytes);
            }
            other => panic!("expected AnalyticsSummaryUpdate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn analytics_full_payload_when_under_limit() {
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
        let balances_index: Arc<RwLock<HashMap<String, u64>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let state = WebSocketServerState::new(
            dag,
            saga,
            analytics,
            balances_index,
            DEFAULT_MAX_WS_MESSAGE_BYTES,
        );

        // Use a moderate payload that should fit under the default 512KiB limit
        let data = AnalyticsDashboardData {
            timestamp: 0,
            network_health: crate::metrics::QantoMetrics::new(),
            ai_performance: crate::saga::AIModelPerformance {
                neural_network_accuracy: 0.95,
                prediction_confidence: 0.90,
                training_loss: 0.2,
                validation_loss: 0.22,
                model_drift_score: 0.05,
                inference_latency_ms: 10.0,
                last_retrain_epoch: 10,
                feature_importance: (0..50).map(|i| (format!("f{}", i), i as f64)).collect(),
            },
            security_insights: crate::saga::SecurityInsights {
                threat_level: ThreatLevel::Low,
                anomaly_score: 0.1,
                attack_attempts_24h: 3,
                blocked_transactions: 1,
                suspicious_patterns: vec!["a".into(), "b".into(), "c".into()],
                security_confidence: 0.99,
            },
            economic_indicators: crate::saga::EconomicIndicators {
                total_value_locked: 1_234_567.0,
                transaction_fees_24h: 234.56,
                validator_rewards_24h: 876.54,
                network_utilization: 0.5,
                economic_security: 0.9,
                fee_market_efficiency: 0.7,
            },
            environmental_metrics: crate::saga::EnvironmentalDashboardMetrics {
                carbon_footprint_kg: 2345.0,
                energy_efficiency_score: 0.8,
                renewable_energy_percentage: 40.0,
                carbon_offset_credits: 12.0,
                green_validator_ratio: 0.3,
            },
            total_transactions: 100,
            active_addresses: 50,
            mempool_size: 5,
            block_height: 99,
            tps_current: 12.3,
            tps_peak: 45.6,
        };

        let msg = build_analytics_message_with_fallback(&state, &data);
        match msg {
            WebSocketMessage::AnalyticsUpdate { .. } => {
                let json = serde_json::to_string(&msg).unwrap();
                assert!(json.len() <= state.max_message_bytes);
            }
            other => panic!("expected AnalyticsUpdate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn initial_snapshot_emitted_on_alias_subscription() {
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
        let balances_index: Arc<RwLock<HashMap<String, u64>>> =
            Arc::new(RwLock::new(HashMap::new()));
        {
            let mut idx = balances_index.write().await;
            idx.insert("addrX".to_string(), 123);
        }

        let state = WebSocketServerState::new(
            dag,
            saga,
            analytics,
            balances_index,
            DEFAULT_MAX_WS_MESSAGE_BYTES,
        );

        let client_id = "test-client".to_string();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WebSocketMessage>(BALANCE_OUTBOUND_BUFFER);
        // Register client with balances subscription channel
        {
            let mut clients = state.clients.write().await;
            clients.insert(
                client_id.clone(),
                ClientConnection {
                    subscriptions: vec![],
                    balance_subscriptions: vec![],
                    filters: HashMap::new(),
                    connected_at: 0,
                    last_balances: HashMap::new(),
                    balance_outbound_tx: Some(tx.clone()),
                },
            );
        }

        // Send alias subscription message
        let alias_msg = serde_json::json!({
            "type": "subscribe",
            "topics": ["wallet_balance:addrX"],
        });
        let alias_text = alias_msg.to_string();
        handle_client_message(&alias_text, &client_id, &state)
            .await
            .expect("alias subscription should succeed");

        // Expect confirmation then initial snapshot
        let confirm = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for confirmation")
            .expect("channel closed");
        match confirm {
            WebSocketMessage::BalanceSubscriptionConfirmed {
                address,
                client_id: cid,
            } => {
                assert_eq!(address, "addrX");
                assert_eq!(cid, client_id);
            }
            other => panic!("unexpected first message: {:?}", other),
        }

        let initial = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for initial snapshot")
            .expect("channel closed");
        match initial {
            WebSocketMessage::BalanceSnapshot {
                address,
                total_confirmed,
                total_unconfirmed,
                ..
            } => {
                assert_eq!(address, "addrX");
                assert_eq!(total_confirmed, 123);
                assert_eq!(total_unconfirmed, 0);
            }
            other => panic!("unexpected second message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn initial_snapshot_skipped_when_finalized_only_true() {
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
        let balances_index: Arc<RwLock<HashMap<String, u64>>> =
            Arc::new(RwLock::new(HashMap::new()));
        {
            let mut idx = balances_index.write().await;
            idx.insert("addrY".to_string(), 555);
        }

        let state = WebSocketServerState::new(
            dag,
            saga,
            analytics,
            balances_index,
            DEFAULT_MAX_WS_MESSAGE_BYTES,
        );

        let client_id = "test-client-f".to_string();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WebSocketMessage>(BALANCE_OUTBOUND_BUFFER);
        {
            let mut clients = state.clients.write().await;
            let mut filters = HashMap::new();
            filters.insert("finalized_only".to_string(), "true".to_string());
            clients.insert(
                client_id.clone(),
                ClientConnection {
                    subscriptions: vec![],
                    balance_subscriptions: vec![],
                    filters,
                    connected_at: 0,
                    last_balances: HashMap::new(),
                    balance_outbound_tx: Some(tx.clone()),
                },
            );
        }

        // Structured subscribe with filters including address and finalized_only=true
        let msg = WebSocketMessage::Subscribe {
            subscription_type: SubscriptionType::Balances,
            filters: Some(HashMap::from([
                ("address".to_string(), "addrY".to_string()),
                ("finalized_only".to_string(), "true".to_string()),
            ])),
        };
        let text = serde_json::to_string(&msg).unwrap();
        handle_client_message(&text, &client_id, &state)
            .await
            .expect("structured subscription should succeed");

        // Expect confirmation but no initial snapshot within timeout
        let confirm = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for confirmation")
            .expect("channel closed");
        matches!(
            confirm,
            WebSocketMessage::BalanceSubscriptionConfirmed { .. }
        );

        // Ensure no second message arrives quickly (best-effort check)
        let maybe_next =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
        assert!(
            maybe_next.is_err(),
            "initial snapshot should be skipped when finalized_only=true"
        );
    }
}
