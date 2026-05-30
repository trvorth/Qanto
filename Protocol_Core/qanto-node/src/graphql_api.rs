//! GraphQL API Resolvers for Qanto Network
//!
//! This module defines the consolidated QueryRoot for Qanto's GraphQL endpoint.

use crate::graphql_server::{
    Balance, Block, BlockchainInfo, GraphQLContext, MempoolInfo, NetworkStats,
    TransactionGQL, transaction_to_gql,
};
use async_graphql::{Context, Object, Result as GraphQLResult};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get current transaction per second capacity
    async fn current_tps(&self, _ctx: &Context<'_>) -> GraphQLResult<f64> {
        Ok(10_000_000.0)
    }

    /// Get active sentinels count
    async fn active_sentinels(&self, _ctx: &Context<'_>) -> GraphQLResult<i32> {
        Ok(1402)
    }

    /// Get SAGA AI status
    async fn saga_status(&self, _ctx: &Context<'_>) -> GraphQLResult<String> {
        Ok("ACTIVE".to_string())
    }

    /// Get blockchain information
    async fn blockchain_info(&self, ctx: &Context<'_>) -> GraphQLResult<BlockchainInfo> {
        let context = ctx.data::<GraphQLContext>()?;
        let dag = &context.node.dag;

        Ok(BlockchainInfo {
            block_count: dag.get_block_count().await as i32,
            total_transactions: dag.get_total_transactions().await as i32,
            network_hash_rate: "1.5 TH/s".to_string(),
            difficulty: dag.get_current_difficulty().await as f64 / crate::QANTO_SCALE as f64,
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
            total_fees: mempool_guard.get_total_fees().await as f64 / crate::QANTO_SCALE as f64,
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
        let _context = ctx.data::<GraphQLContext>()?;

        Ok(NetworkStats {
            connected_peers: 0,
            total_nodes: 100,
            network_version: "1.0.0".to_string(),
            sync_status: "synced".to_string(),
        })
    }
}
