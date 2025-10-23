use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::{transport::Server, Request, Response, Status};

pub mod generated {
    tonic::include_proto!("qanto");
}

use generated::qanto_rpc_server::{QantoRpc, QantoRpcServer};
use generated::{
    GetBalanceRequest, GetBalanceResponse, GetBlockRequest, GetBlockResponse,
    GetNetworkStatsRequest, GetNetworkStatsResponse, SubmitTransactionRequest,
    SubmitTransactionResponse,
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
}

pub async fn start(
    addr: SocketAddr,
    backend: Arc<dyn RpcBackend>,
) -> Result<(), tonic::transport::Error> {
    let svc = RpcService::new(backend);

    Server::builder()
        .add_service(QantoRpcServer::new(svc))
        .serve(addr)
        .await
}
