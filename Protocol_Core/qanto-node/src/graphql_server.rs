//! GraphQL API Server for Qanto Network
//!
//! This module provides a comprehensive GraphQL API for the Qanto blockchain network,
//! including queries, mutations, and real-time subscriptions.

use crate::node::Node;
use crate::qantodag::QantoBlock;
use crate::transaction::Transaction;

use anyhow::Result;
use async_graphql::{
    Context, Object, Result as GraphQLResult, Schema, SimpleObject, Subscription, ID,
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

/// Root query object
#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get blockchain information
    async fn blockchain_info(&self, ctx: &Context<'_>) -> GraphQLResult<BlockchainInfo> {
        let context = ctx.data::<GraphQLContext>()?;
        let dag = &context.node.dag;

        Ok(BlockchainInfo {
            block_count: dag.get_block_count().await as i32,
            total_transactions: dag.get_total_transactions().await as i32,
            network_hash_rate: "1.5 TH/s".to_string(), // Placeholder
            difficulty: dag.get_current_difficulty().await,
            latest_block_hash: dag.get_latest_block_hash().await.unwrap_or_default(),
        })
    }

    /// Get block by ID or hash
    async fn block(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Block ID or hash")] id: String,
    ) -> GraphQLResult<Option<Block>> {
        let context = ctx.data::<GraphQLContext>()?;
        let dag = &context.node.dag;

        match dag.get_block(&id).await {
            Some(block) => Ok(Some(Block::from(block))),
            None => Ok(None),
        }
    }

    /// Get blocks with pagination
    async fn blocks(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Number of blocks to fetch", default = 10)] limit: i32,
        #[graphql(desc = "Offset for pagination", default = 0)] offset: i32,
    ) -> GraphQLResult<Vec<Block>> {
        let context = ctx.data::<GraphQLContext>()?;
        let dag = &context.node.dag;

        let blocks = dag
            .get_blocks_paginated(limit as usize, offset as usize)
            .await;

        Ok(blocks.into_iter().map(Block::from).collect())
    }

    /// Get transaction by ID
    async fn transaction(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Transaction ID")] id: String,
    ) -> GraphQLResult<Option<TransactionGQL>> {
        let context = ctx.data::<GraphQLContext>()?;
        let dag = &context.node.dag;

        match dag.get_transaction(&id).await {
            Some(tx) => Ok(Some(transaction_to_gql(&tx))),
            None => Ok(None),
        }
    }

    /// Get account balance
    async fn balance(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Account address")] address: String,
    ) -> GraphQLResult<Balance> {
        let _context = ctx.data::<GraphQLContext>()?;
        // Note: wallet is private, using placeholder balance
        let balance = 0.0;

        Ok(Balance {
            address,
            amount: balance,
            currency: "QANTO".to_string(),
        })
    }

    /// Get mempool status
    async fn mempool(&self, ctx: &Context<'_>) -> GraphQLResult<MempoolInfo> {
        let context = ctx.data::<GraphQLContext>()?;
        let mempool_guard = context.node.mempool.read().await;

        let pending_transactions: Vec<TransactionGQL> = mempool_guard
            .get_pending_transactions()
            .await
            .iter()
            .map(transaction_to_gql)
            .collect();

        Ok(MempoolInfo {
            size: mempool_guard.len().await as i32,
            total_fees: mempool_guard.get_total_fees().await as f64,
            pending_transactions,
        })
    }

    /// Get pending transactions
    async fn pending_transactions(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<TransactionGQL>> {
        let context = ctx.data::<GraphQLContext>()?;
        let mempool_guard = context.node.mempool.read().await;

        let transactions = mempool_guard.get_pending_transactions().await;
        Ok(transactions.iter().map(transaction_to_gql).collect())
    }

    /// Get network statistics
    async fn network_stats(&self, ctx: &Context<'_>) -> GraphQLResult<NetworkStats> {
        let context = ctx.data::<GraphQLContext>()?;
        let _node = &context.node;

        Ok(NetworkStats {
            connected_peers: 0, // Note: get_connected_peers method doesn't exist, using placeholder
            total_nodes: 100,   // Placeholder
            network_version: "1.0.0".to_string(),
            sync_status: "synced".to_string(),
        })
    }
}

/// Root mutation object
#[derive(Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Submit a new transaction
    async fn submit_transaction(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Transaction data")] input: TransactionInput,
    ) -> GraphQLResult<TransactionResponse> {
        let context = ctx.data::<GraphQLContext>()?;
        let node = &context.node;
        let dag = &node.dag;

        // Create transaction from input
        let transaction = Transaction {
            id: input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            sender: input.from.clone(),
            receiver: input.to.clone(),
            amount: input.amount as u64,
            fee: input.fee.unwrap_or(1000.0) as u64,
            gas_limit: 21000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: vec![],
            outputs: vec![],
            signature: crate::types::QuantumResistantSignature {
                signature: hex::decode(input.signature.unwrap_or_default()).unwrap_or_default(),
                signer_public_key: hex::decode("").unwrap_or_default(),
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: std::collections::HashMap::new(),
            fee_breakdown: None,
        };

        // Submit to mempool
        let utxos = std::collections::HashMap::new();
        match node
            .mempool
            .write()
            .await
            .add_transaction(transaction.clone(), &utxos, dag)
            .await
        {
            Ok(_) => {
                // Broadcast transaction
                let _ = context.transaction_sender.send(transaction.clone());

                Ok(TransactionResponse {
                    success: true,
                    transaction_id: transaction.id.clone(),
                    message: "Transaction submitted successfully".to_string(),
                })
            }
            Err(e) => {
                let mut error_msg = String::with_capacity(32 + e.to_string().len());
                error_msg.push_str("Failed to submit transaction: ");
                error_msg.push_str(&e.to_string());
                Ok(TransactionResponse {
                    success: false,
                    transaction_id: transaction.id.clone(),
                    message: error_msg,
                })
            }
        }
    }
}

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
struct BlockchainInfo {
    block_count: i32,
    total_transactions: i32,
    network_hash_rate: String,
    difficulty: f64,
    latest_block_hash: String,
}

#[derive(SimpleObject)]
struct Block {
    id: ID,
    height: i32,
    hash: String,
    previous_hash: String,
    timestamp: i32,
    transaction_count: i32,
    transactions: Vec<TransactionGQL>,
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
struct TransactionGQL {
    id: ID,
    from: String,
    to: String,
    amount: f64,
    fee: f64,
    timestamp: i32,
    signature: String,
    nonce: i32,
}

impl From<Transaction> for TransactionGQL {
    fn from(tx: Transaction) -> Self {
        Self {
            id: ID(tx.id.clone()),
            from: tx.sender.clone(),
            to: tx.receiver.clone(),
            amount: tx.amount as f64,
            fee: tx.fee as f64,
            timestamp: tx.timestamp as i32,
            signature: hex::encode(tx.signature.signature.clone()),
            nonce: 0, // Transaction struct doesn't have nonce field
        }
    }
}

// Helper function to avoid async_graphql conflicts
fn transaction_to_gql(tx: &Transaction) -> TransactionGQL {
    TransactionGQL {
        id: ID(tx.id.clone()),
        from: tx.sender.clone(),
        to: tx.receiver.clone(),
        amount: tx.amount as f64,
        fee: tx.fee as f64,
        timestamp: tx.timestamp as i32,
        signature: hex::encode(tx.signature.signature.clone()),
        nonce: 0,
    }
}

#[derive(SimpleObject)]
struct Balance {
    address: String,
    amount: f64,
    currency: String,
}

#[derive(SimpleObject)]
struct MempoolInfo {
    size: i32,
    total_fees: f64,
    pending_transactions: Vec<TransactionGQL>,
}

#[derive(SimpleObject)]
struct NetworkStats {
    connected_peers: i32,
    total_nodes: i32,
    network_version: String,
    sync_status: String,
}

#[derive(async_graphql::InputObject)]
struct TransactionInput {
    id: Option<String>,
    from: String,
    to: String,
    amount: f64,
    fee: Option<f64>,
    signature: Option<String>,
    nonce: Option<i32>,
}

#[derive(SimpleObject)]
struct TransactionResponse {
    success: bool,
    transaction_id: String,
    message: String,
}

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
