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
    GetNetworkStatsRequest, GetNetworkStatsResponse, QueryBalanceRequest, QueryBalanceResponse,
    SubmitTransactionRequest, SubmitTransactionResponse, WalletGetBalanceRequest,
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
    // Detailed balance with maturity-aware coinbase breakdown
    async fn query_wallet_balance(&self, address: String) -> Result<(u64, u64, u64, u64), String>;
    async fn request_airdrop(
        &self,
        request: generated::RequestAirdropRequest,
    ) -> Result<generated::RequestAirdropResponse, String>;
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

    async fn request_airdrop(
        &self,
        request: Request<generated::RequestAirdropRequest>,
    ) -> Result<Response<generated::RequestAirdropResponse>, Status> {
        let req = request.into_inner();
        match self.backend.request_airdrop(req).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn subscribe_balance(
        &self,
        request: Request<generated::SubscribeBalanceRequest>,
    ) -> Result<Response<Self::SubscribeBalanceStream>, Status> {
        let req = request.into_inner();
        let filter = req.address_filter;
        let finalized_only = req.finalized_only;
        let filter_opt = if filter.is_empty() {
            None
        } else {
            Some(filter)
        };
        let rx = self.backend.balance_updates_receiver();
        // First filter by address and finalized_only
        let filtered = BroadcastStream::new(rx).filter_map(move |item| {
            let filter_opt = filter_opt.clone();
            let finalized_only = finalized_only;
            match item {
                Ok(update) => {
                    if filter_opt.as_ref().is_none_or(|f| update.address == *f)
                        && (!finalized_only || update.finalized)
                    {
                        Some(Ok(update))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        });

        // Then enrich with WalletBalance breakdown for every item to keep API consistent
        let backend = self.backend.clone();
        let stream = filtered.then(move |res| {
            let backend = backend.clone();
            async move {
                match res {
                    Ok(update) => {
                        match backend.query_wallet_balance(update.address.clone()).await {
                            Ok((
                                spendable,
                                immature_coinbase,
                                unconfirmed_delta,
                                total_confirmed,
                            )) => Ok(generated::BalanceUpdate {
                                address: update.address,
                                base_units: update.base_units,
                                timestamp: update.timestamp,
                                finalized: update.finalized,
                                balance_bigint: update.balance_bigint,
                                spendable_confirmed: spendable,
                                immature_coinbase_confirmed: immature_coinbase,
                                unconfirmed_delta,
                                total_confirmed,
                            }),
                            Err(_e) => {
                                // Fallback: deliver original update without breakdown
                                Ok(update)
                            }
                        }
                    }
                    Err(status) => Err(status),
                }
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

    async fn query_balance(
        &self,
        request: Request<QueryBalanceRequest>,
    ) -> Result<Response<QueryBalanceResponse>, Status> {
        let addr = request.into_inner().address;
        match self.backend.query_wallet_balance(addr).await {
            Ok((spendable, immature_coinbase, unconfirmed_delta, total_confirmed)) => {
                Ok(Response::new(QueryBalanceResponse {
                    spendable_confirmed: spendable,
                    immature_coinbase_confirmed: immature_coinbase,
                    unconfirmed_delta,
                    total_confirmed,
                }))
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use tokio_stream::StreamExt;

    struct MockBackend {
        spendable: u64,
        immature: u64,
        unconfirmed: u64,
        total: u64,
        wallet_confirmed: u64,
        wallet_unconfirmed: u64,
        bal_tx: broadcast::Sender<generated::BalanceUpdate>,
    }

    #[async_trait]
    impl RpcBackend for MockBackend {
        async fn submit_transaction(&self, _tx: generated::Transaction) -> Result<(), String> {
            Ok(())
        }

        async fn get_balance(&self, _address: String) -> Result<u64, String> {
            Ok(self.total)
        }

        async fn get_block(&self, _block_id: String) -> Result<generated::QantoBlock, String> {
            Err("not implemented".to_string())
        }

        async fn get_network_stats(&self) -> Result<NetworkStats, String> {
            Ok(NetworkStats {
                tps: 0.0,
                bps: 0.0,
                mempool_tx_count: 0,
                mempool_size_bytes: 0,
                connected_peers: 0,
                block_count: 0,
                finality_ms: 0,
                network_throughput_mbps: 0.0,
            })
        }

        fn balance_updates_receiver(
            &self,
        ) -> tokio::sync::broadcast::Receiver<generated::BalanceUpdate> {
            self.bal_tx.subscribe()
        }

        async fn get_wallet_balance(&self, _address: String) -> Result<(u64, u64), String> {
            Ok((self.wallet_confirmed, self.wallet_unconfirmed))
        }

        async fn query_wallet_balance(
            &self,
            _address: String,
        ) -> Result<(u64, u64, u64, u64), String> {
            Ok((self.spendable, self.immature, self.unconfirmed, self.total))
        }

        async fn request_airdrop(
            &self,
            _request: generated::RequestAirdropRequest,
        ) -> Result<generated::RequestAirdropResponse, String> {
            Ok(generated::RequestAirdropResponse {
                tx_id: "mock_tx".to_string(),
                message: "mock airdrop".to_string(),
            })
        }
    }

    fn make_service() -> RpcService {
        let (tx, _rx) = broadcast::channel(16);
        let mock = MockBackend {
            spendable: 800_000_000,
            immature: 50_000_000,
            unconfirmed: 10_000_000,
            total: 850_000_000,
            wallet_confirmed: 850_000_000,
            wallet_unconfirmed: 10_000_000,
            bal_tx: tx,
        };
        RpcService::new(Arc::new(mock))
    }

    fn make_service_with_tx() -> (RpcService, broadcast::Sender<generated::BalanceUpdate>) {
        let (tx, _rx) = broadcast::channel(16);
        let mock = MockBackend {
            spendable: 800_000_000,
            immature: 50_000_000,
            unconfirmed: 10_000_000,
            total: 850_000_000,
            wallet_confirmed: 850_000_000,
            wallet_unconfirmed: 10_000_000,
            bal_tx: tx.clone(),
        };
        (RpcService::new(Arc::new(mock)), tx)
    }

    #[tokio::test]
    async fn query_balance_maps_fields_correctly() {
        let svc = make_service();
        let req = Request::new(QueryBalanceRequest {
            address: "ae...".to_string(),
        });
        let resp = WalletService::query_balance(&svc, req).await.unwrap();
        let inner = resp.into_inner();
        assert_eq!(inner.spendable_confirmed, 800_000_000);
        assert_eq!(inner.immature_coinbase_confirmed, 50_000_000);
        assert_eq!(inner.unconfirmed_delta, 10_000_000);
        assert_eq!(inner.total_confirmed, 850_000_000);
    }

    #[tokio::test]
    async fn legacy_get_balance_remains_unchanged() {
        let svc = make_service();
        let req = Request::new(WalletGetBalanceRequest {
            address: "ae...".to_string(),
        });
        let resp = WalletService::get_balance(&svc, req).await.unwrap();
        let inner = resp.into_inner();
        assert_eq!(inner.balance, 850_000_000);
        assert_eq!(inner.unconfirmed_balance, 10_000_000);
    }

    #[tokio::test]
    async fn subscribe_balance_enriches_breakdown() {
        let (svc, tx) = make_service_with_tx();
        // Subscribe to a specific address without finalized-only filter
        let req = Request::new(generated::SubscribeBalanceRequest {
            address_filter: "ae...".to_string(),
            finalized_only: false,
        });
        let resp = QantoRpc::subscribe_balance(&svc, req).await.unwrap();
        let mut stream = resp.into_inner();

        // Publish a balance update for the same address
        let _ = tx.send(generated::BalanceUpdate {
            address: "ae...".to_string(),
            base_units: 850_000_000,
            timestamp: 123,
            finalized: true,
            balance_bigint: "850000000".to_string(),
            ..Default::default()
        });

        // Receive enriched update
        let item = stream.next().await.expect("expected one item").unwrap();
        assert_eq!(item.address, "ae...");
        assert_eq!(item.base_units, 850_000_000);
        // Enrichment values come from MockBackend::query_wallet_balance
        assert_eq!(item.spendable_confirmed, 800_000_000);
        assert_eq!(item.immature_coinbase_confirmed, 50_000_000);
        assert_eq!(item.unconfirmed_delta, 10_000_000);
        assert_eq!(item.total_confirmed, 850_000_000);
    }

    #[tokio::test]
    async fn subscribe_balance_filters_finalized_only() {
        let (svc, tx) = make_service_with_tx();
        let req = Request::new(generated::SubscribeBalanceRequest {
            address_filter: "ae...".to_string(),
            finalized_only: true,
        });
        let resp = QantoRpc::subscribe_balance(&svc, req).await.unwrap();
        let mut stream = resp.into_inner();

        // Send an unfinalized update first (should be filtered out)
        let _ = tx.send(generated::BalanceUpdate {
            address: "ae...".to_string(),
            base_units: 840_000_000,
            timestamp: 100,
            finalized: false,
            balance_bigint: "840000000".to_string(),
            ..Default::default()
        });
        // Then a finalized update which should pass through
        let _ = tx.send(generated::BalanceUpdate {
            address: "ae...".to_string(),
            base_units: 850_000_000,
            timestamp: 200,
            finalized: true,
            balance_bigint: "850000000".to_string(),
            ..Default::default()
        });

        let item = stream.next().await.expect("expected one item").unwrap();
        assert!(item.finalized);
        assert_eq!(item.base_units, 850_000_000);
        // Enrichment is applied even when finalized-only
        assert_eq!(item.spendable_confirmed, 800_000_000);
        assert_eq!(item.total_confirmed, 850_000_000);
    }
}
use jsonrpsee::server::{PendingSubscriptionSink, Server as WsServer, SubscriptionSink};
use jsonrpsee::types::Params;
use jsonrpsee::RpcModule;
use jsonrpsee::SubscriptionMessage;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
#[derive(Clone, Serialize, Deserialize)]
pub struct BalanceUpdateWs {
    pub address: String,
    pub spendable: u64,
    pub total: u64,
    pub finalized: bool,
}

pub async fn start_ws(
    bus: broadcast::Sender<BalanceUpdateWs>,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let server = WsServer::builder().build(addr).await?;
    let mut module = RpcModule::new(());
    let bus_arc = std::sync::Arc::new(bus);
    module.register_subscription(
        "subscribe_balance",
        "balance",
        "unsubscribe_balance",
        move |params: Params<'static>, sink: PendingSubscriptionSink, _ctx, _| {
            let value = bus_arc.clone();
            async move {
                let address: String = params.one()?;
                let mut rx = value.subscribe();
                let accepted: SubscriptionSink = sink.accept().await?;
                tokio::spawn(async move {
                    while let Ok(update) = rx.recv().await {
                        if update.address == address {
                            let accepted_clone = accepted.clone();
                            tokio::spawn(async move {
                                if let Err(e) = accepted_clone
                                    .send(
                                        SubscriptionMessage::from_json(&update).expect("serialize"),
                                    )
                                    .await
                                {
                                    tracing::error!("Send failed: {}", e);
                                }
                            });
                        }
                    }
                });
                Ok(())
            }
        },
    )?;
    let handle = server.start(module);
    tokio::spawn(async move {
        let _ = handle.stopped().await;
    });
    Ok(())
}
