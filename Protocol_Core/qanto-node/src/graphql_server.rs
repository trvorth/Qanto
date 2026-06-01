//! GraphQL API Server for Qanto Network
//!
//! This module provides a comprehensive GraphQL API for the Qanto blockchain network,
//! including queries, mutations, and real-time subscriptions.

use crate::node::Node;
use crate::qantodag::QantoBlock;
use crate::transaction::Transaction;

use anyhow::Result;
use async_graphql::{
    Context, Schema, SimpleObject, Subscription, ID,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use hex;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, info, warn};

/// Channel health metrics
static BLOCK_BROADCAST_ERRORS: AtomicU64 = AtomicU64::new(0);
static TRANSACTION_BROADCAST_ERRORS: AtomicU64 = AtomicU64::new(0);

/// Channel manager for automatic reinitialization
struct ChannelManager {
    block_sender: Mutex<broadcast::Sender<QantoBlock>>,
    transaction_sender: Mutex<broadcast::Sender<Transaction>>,
}

impl ChannelManager {
    fn new() -> Self {
        let (block_sender, _) = broadcast::channel(1000);
        let (transaction_sender, _) = broadcast::channel(1000);

        Self {
            block_sender: Mutex::new(block_sender),
            transaction_sender: Mutex::new(transaction_sender),
        }
    }

    fn get_block_sender(&self) -> broadcast::Sender<QantoBlock> {
        let sender = self.block_sender.lock().unwrap();

        // Check if channel is closed (no receivers and send would fail)
        if sender.receiver_count() == 0 && !sender.is_empty() {
            // Channel might be stale, but we'll let the broadcast function handle recreation
        }

        sender.clone()
    }

    fn get_transaction_sender(&self) -> broadcast::Sender<Transaction> {
        let sender = self.transaction_sender.lock().unwrap();

        // Check if channel is closed (no receivers and send would fail)
        if sender.receiver_count() == 0 && !sender.is_empty() {
            // Channel might be stale, but we'll let the broadcast function handle recreation
        }

        sender.clone()
    }

    fn reinitialize_block_channel(&self) -> broadcast::Sender<QantoBlock> {
        let (new_sender, _) = broadcast::channel(1000);
        let mut sender = self.block_sender.lock().unwrap();
        *sender = new_sender.clone();
        debug!("Reinitialized GraphQL block broadcast channel");
        new_sender
    }

    fn reinitialize_transaction_channel(&self) -> broadcast::Sender<Transaction> {
        let (new_sender, _) = broadcast::channel(1000);
        let mut sender = self.transaction_sender.lock().unwrap();
        *sender = new_sender.clone();
        debug!("Reinitialized GraphQL transaction broadcast channel");
        new_sender
    }
}

/// Global channel manager
static CHANNEL_MANAGER: Lazy<ChannelManager> = Lazy::new(ChannelManager::new);

/// Broadcast a new block to GraphQL subscribers
pub async fn broadcast_new_block(block: &QantoBlock) {
    let sender = CHANNEL_MANAGER.get_block_sender();
    let subscriber_count = sender.receiver_count();

    // Early return if no subscribers to avoid unnecessary work
    if subscriber_count == 0 {
        debug!("No GraphQL block subscribers, skipping broadcast");
        return;
    }

    match sender.send(block.clone()) {
        Ok(_) => {
            debug!(
                "Successfully broadcast block to {} GraphQL subscribers",
                subscriber_count
            );
        }
        Err(tokio::sync::broadcast::error::SendError(_)) => {
            // Channel is closed, try to reinitialize and retry once
            warn!("GraphQL block broadcast channel closed, reinitializing...");
            let new_sender = CHANNEL_MANAGER.reinitialize_block_channel();

            match new_sender.send(block.clone()) {
                Ok(_) => {
                    debug!("Successfully broadcast block after channel reinitialization");
                }
                Err(_) => {
                    let error_count = BLOCK_BROADCAST_ERRORS.fetch_add(1, Ordering::Relaxed) + 1;

                    if error_count.is_multiple_of(100) {
                        warn!(
                            "GraphQL block broadcast failed {} times (channel issues persist)",
                            error_count
                        );
                    } else {
                        debug!("GraphQL block broadcast failed after reinitialization");
                    }
                }
            }
        }
    }
}

/// Broadcast a new transaction to GraphQL subscribers
pub async fn broadcast_new_transaction(transaction: &Transaction) {
    let sender = CHANNEL_MANAGER.get_transaction_sender();
    let subscriber_count = sender.receiver_count();

    // Early return if no subscribers to avoid unnecessary work
    if subscriber_count == 0 {
        debug!("No GraphQL transaction subscribers, skipping broadcast");
        return;
    }

    match sender.send(transaction.clone()) {
        Ok(_) => {
            debug!(
                "Successfully broadcast transaction to {} GraphQL subscribers",
                subscriber_count
            );
        }
        Err(tokio::sync::broadcast::error::SendError(_)) => {
            // Channel is closed, try to reinitialize and retry once
            warn!("GraphQL transaction broadcast channel closed, reinitializing...");
            let new_sender = CHANNEL_MANAGER.reinitialize_transaction_channel();

            match new_sender.send(transaction.clone()) {
                Ok(_) => {
                    debug!("Successfully broadcast transaction after channel reinitialization");
                }
                Err(_) => {
                    let error_count =
                        TRANSACTION_BROADCAST_ERRORS.fetch_add(1, Ordering::Relaxed) + 1;

                    if error_count.is_multiple_of(100) {
                        warn!(
                            "GraphQL transaction broadcast failed {} times (channel issues persist)",
                            error_count
                        );
                    } else {
                        debug!("GraphQL transaction broadcast failed after reinitialization");
                    }
                }
            }
        }
    }
}

/// Get channel health metrics for monitoring
pub fn get_channel_health() -> (u64, u64) {
    (
        BLOCK_BROADCAST_ERRORS.load(Ordering::Relaxed),
        TRANSACTION_BROADCAST_ERRORS.load(Ordering::Relaxed),
    )
}

/// Reset channel health metrics (useful for testing or periodic resets)
pub fn reset_channel_health() {
    BLOCK_BROADCAST_ERRORS.store(0, Ordering::Relaxed);
    TRANSACTION_BROADCAST_ERRORS.store(0, Ordering::Relaxed);
}

/// Create GraphQL schema
pub fn create_graphql_schema() -> QantoSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot).finish()
}

/// GraphQL Schema type
pub type QantoSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// GraphQL context containing shared state
#[derive(Clone)]
pub struct GraphQLContext {
    pub node: Arc<Node>,
    pub block_sender: broadcast::Sender<QantoBlock>,
    pub transaction_sender: broadcast::Sender<Transaction>,
}

use crate::graphql_api::{QueryRoot, MutationRoot};

/// Root subscription object
#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to new blocks
    async fn new_blocks(&self, _ctx: &Context<'_>) -> impl futures::Stream<Item = Block> {
        let sender = CHANNEL_MANAGER.get_block_sender();
        let receiver = sender.subscribe();

        use futures::StreamExt;
        BroadcastStream::new(receiver).filter_map(|result| async move {
            match result {
                Ok(block) => {
                    debug!("GraphQL: Streaming new block to subscriber");
                    Some(Block::from(block))
                }
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(missed)) => {
                    // Log lagged error but continue stream
                    warn!("Block subscription lagged, {} blocks missed", missed);
                    None
                }
            }
        })
    }

    /// Subscribe to new transactions
    async fn new_transactions(
        &self,
        _ctx: &Context<'_>,
    ) -> impl futures::Stream<Item = TransactionGQL> {
        let sender = CHANNEL_MANAGER.get_transaction_sender();
        let receiver = sender.subscribe();

        use futures::StreamExt;
        BroadcastStream::new(receiver).filter_map(|result| async move {
            match result {
                Ok(transaction) => {
                    debug!("GraphQL: Streaming new transaction to subscriber");
                    Some(transaction_to_gql(&transaction))
                }
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(missed)) => {
                    // Log lagged error but continue stream
                    warn!(
                        "Transaction subscription lagged, {} transactions missed",
                        missed
                    );
                    None
                }
            }
        })
    }
}

// GraphQL Types

#[derive(SimpleObject)]
pub struct BlockchainInfo {
    pub block_count: i32,
    pub total_transactions: i32,
    pub network_hash_rate: String,
    pub difficulty: f64,
    pub latest_block_hash: String,
}

#[derive(SimpleObject)]
pub struct Block {
    pub id: ID,
    pub height: i32,
    pub hash: String,
    pub previous_hash: String,
    pub timestamp: i32,
    pub transaction_count: i32,
    pub transactions: Vec<TransactionGQL>,
}

impl From<QantoBlock> for Block {
    fn from(block: QantoBlock) -> Self {
        Self {
            id: ID(block.id.clone()),
            height: block.height as i32,
            hash: block.id,
            previous_hash: block.parents.first().cloned().unwrap_or_default(),
            timestamp: block.timestamp as i32,
            transaction_count: block.transactions.len() as i32,
            transactions: block.transactions.into_iter().map(|tx| tx.into()).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TransactionGQL {
    pub id: ID,
    pub from: String,
    pub to: String,
    pub amount: f64,
    pub fee: f64,
    pub timestamp: i32,
    pub signature: String,
    pub nonce: i32,
}

impl From<Transaction> for TransactionGQL {
    fn from(tx: Transaction) -> Self {
        Self {
            id: ID(tx.id.clone()),
            from: tx.sender.clone(),
            to: tx.receiver.clone(),
            amount: tx.amount as f64 / crate::QANTO_SCALE as f64,
            fee: tx.fee as f64 / crate::QANTO_SCALE as f64,
            timestamp: tx.timestamp as i32,
            signature: hex::encode(tx.signature.signature.clone()),
            nonce: 0, // Transaction struct doesn't have nonce field
        }
    }
}

// Helper function to avoid async_graphql conflicts
pub fn transaction_to_gql(tx: &Transaction) -> TransactionGQL {
    TransactionGQL {
        id: ID(tx.id.clone()),
        from: tx.sender.clone(),
        to: tx.receiver.clone(),
        amount: tx.amount as f64 / crate::QANTO_SCALE as f64,
        fee: tx.fee as f64 / crate::QANTO_SCALE as f64,
        timestamp: tx.timestamp as i32,
        signature: hex::encode(tx.signature.signature.clone()),
        nonce: 0,
    }
}

#[derive(SimpleObject)]
pub struct Balance {
    pub address: String,
    pub amount: f64,
    pub currency: String,
}

#[derive(SimpleObject)]
pub struct MempoolInfo {
    pub size: i32,
    pub total_fees: f64,
    pub pending_transactions: Vec<TransactionGQL>,
}

#[derive(SimpleObject)]
pub struct NetworkStats {
    pub connected_peers: i32,
    pub total_nodes: i32,
    pub network_version: String,
    pub sync_status: String,
}

pub use crate::graphql_api::{TransactionInput, TransactionResponse};

/// Create GraphQL schema
pub fn create_schema(context: GraphQLContext) -> QantoSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(context)
        .finish()
}

/// GraphQL playground handler
pub async fn graphql_playground() -> impl IntoResponse {
    Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql")
            .subscription_endpoint("/graphql/ws"),
    ))
}

/// GraphQL query handler
pub async fn graphql_handler(
    State(schema): State<QantoSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

/// Create GraphQL router
pub fn create_graphql_router(schema: QantoSchema) -> Router {
    Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/graphql/playground", get(graphql_playground))
        .route_service("/graphql/ws", GraphQLSubscription::new(schema.clone()))
        .with_state(schema)
}

/// Start GraphQL server
pub async fn start_graphql_server(
    context: GraphQLContext,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let schema = create_schema(context);
    let app = create_graphql_router(schema);

    let mut addr = String::with_capacity(8 + port.to_string().len());
    addr.push_str("0.0.0.0:");
    addr.push_str(&port.to_string());
    info!("Starting GraphQL server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
