use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::{transport::Server, Request, Response, Status};

use std::pin::Pin;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

pub mod generated {
    tonic::include_proto!("qanto");
}

use generated::qanto_rpc_server::{QantoRpc, QantoRpcServer};
use generated::wallet_service_server::{WalletService, WalletServiceServer};
use generated::{
    BalanceResponse, GetBalanceRequest, GetBalanceResponse, GetBlockRequest, GetBlockResponse,
    GetNetworkStatsRequest, GetNetworkStatsResponse, SubmitTransactionRequest,
    SubmitTransactionResponse, WalletGetBalanceRequest,
};

#[derive(Clone, Debug)]
pub struct NetworkStats {
    pub tps: f64,
    pub bps: f64,
    pub mempool_tx_count: u64,
    pub mempool_size_bytes: u64,
    pub connected_peers: u64,
    pub block_count: u64,
    pub finality_ms: u64,
    pub network_throughput_mbps: f64,
}

#[async_trait]
pub trait RpcBackend: Send + Sync + 'static {
    async fn submit_transaction(&self, tx: generated::Transaction) -> Result<(), String>;
    async fn get_balance(&self, address: String) -> Result<u64, String>;
    async fn get_block(&self, block_id: String) -> Result<generated::QantoBlock, String>;
    async fn get_network_stats(&self) -> Result<NetworkStats, String>;
    fn balance_updates_receiver(
        &self,
    ) -> tokio::sync::broadcast::Receiver<generated::BalanceUpdate>;
    // Wallet-specific balance with unconfirmed delta
    async fn get_wallet_balance(&self, address: String) -> Result<(u64, u64), String>;
}

#[derive(Clone)]
pub struct RpcService {
    backend: Arc<dyn RpcBackend>,
}

impl RpcService {
    pub fn new(backend: Arc<dyn RpcBackend>) -> Self {
        Self { backend }
    }

    fn format_balance(base_units: u64) -> String {
        // Represent balance as base units string to avoid cross-crate constants.
        base_units.to_string()
    }
}

#[tonic::async_trait]
impl QantoRpc for RpcService {
    type SubscribeBalanceStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<generated::BalanceUpdate, Status>> + Send>>;

    async fn submit_transaction(
        &self,
        request: Request<SubmitTransactionRequest>,
    ) -> Result<Response<SubmitTransactionResponse>, Status> {
        let req = request.into_inner();
        match self
            .backend
            .submit_transaction(req.transaction.unwrap_or_default())
            .await
        {
            Ok(()) => Ok(Response::new(SubmitTransactionResponse {
                accepted: true,
                message: "Transaction accepted and broadcast".to_string(),
            })),
            Err(e) => Err(Status::invalid_argument(e)),
        }
    }

    async fn get_balance(
        &self,
        request: Request<GetBalanceRequest>,
    ) -> Result<Response<GetBalanceResponse>, Status> {
        let addr = request.into_inner().address;
        match self.backend.get_balance(addr).await {
            Ok(total) => Ok(Response::new(GetBalanceResponse {
                base_units: total,
                balance: Self::format_balance(total),
            })),
            Err(e) => Err(Status::invalid_argument(e)),
        }
    }

    async fn get_block(
        &self,
        request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockResponse>, Status> {
        let block_id = request.into_inner().block_id;
        match self.backend.get_block(block_id).await {
            Ok(block) => Ok(Response::new(GetBlockResponse { block: Some(block) })),
            Err(e) => Err(Status::not_found(e)),
        }
    }

    async fn get_network_stats(
        &self,
        _request: Request<GetNetworkStatsRequest>,
    ) -> Result<Response<GetNetworkStatsResponse>, Status> {
        match self.backend.get_network_stats().await {
            Ok(stats) => Ok(Response::new(GetNetworkStatsResponse {
                tps: stats.tps,
                bps: stats.bps,
                mempool_tx_count: stats.mempool_tx_count,
                mempool_size_bytes: stats.mempool_size_bytes,
                connected_peers: stats.connected_peers,
                block_count: stats.block_count,
                finality_ms: stats.finality_ms,
                network_throughput_mbps: stats.network_throughput_mbps,
            })),
            Err(e) => Err(Status::unavailable(e)),
        }
    }

    async fn subscribe_balance(
        &self,
        request: Request<generated::SubscribeBalanceRequest>,
    ) -> Result<Response<Self::SubscribeBalanceStream>, Status> {
        let filter = request.into_inner().address_filter;
        let filter_opt = if filter.is_empty() {
            None
        } else {
            Some(filter)
        };
        let rx = self.backend.balance_updates_receiver();
        let stream = BroadcastStream::new(rx).filter_map(move |item| {
            let filter_opt = filter_opt.clone();
            match item {
                Ok(update) => {
                    if filter_opt.as_ref().is_none_or(|f| update.address == *f) {
                        Some(Ok(update))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        });
        Ok(Response::new(Box::pin(stream)))
    }
}

#[tonic::async_trait]
impl WalletService for RpcService {
    async fn get_balance(
        &self,
        request: Request<WalletGetBalanceRequest>,
    ) -> Result<Response<BalanceResponse>, Status> {
        let addr = request.into_inner().address;
        match self.backend.get_wallet_balance(addr).await {
            Ok((confirmed, unconfirmed_delta)) => Ok(Response::new(BalanceResponse {
                balance: confirmed,
                unconfirmed_balance: unconfirmed_delta,
            })),
            Err(e) => Err(Status::invalid_argument(e)),
        }
    }
}

pub async fn start(
    addr: SocketAddr,
    backend: Arc<dyn RpcBackend>,
) -> Result<(), tonic::transport::Error> {
    let svc = RpcService::new(backend);

    Server::builder()
        .add_service(QantoRpcServer::new(svc.clone()))
        .add_service(WalletServiceServer::new(svc))
        .serve(addr)
        .await
}
