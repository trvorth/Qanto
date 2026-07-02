use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::{transport::Server, Request, Response, Status};

use std::pin::Pin;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

pub mod generated {
    #![allow(clippy::all)]
    tonic::include_proto!("qanto");
}

use generated::qanto_rpc_server::{QantoRpc, QantoRpcServer};
use generated::wallet_service_server::{WalletService, WalletServiceServer};
use generated::{
    AddressTransactionEntry, BalanceResponse, GetBalanceRequest, GetBalanceResponse,
    GetBlockRequest, GetBlockResponse, GetNetworkStatsRequest, GetNetworkStatsResponse,
    GetTransactionsByAddressRequest, GetTransactionsByAddressResponse, SubmitTransactionRequest,
    SubmitTransactionResponse, SubscribeTransactionsRequest, WalletGetBalanceRequest,
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
    async fn get_balance(&self, address: String) -> Result<u128, String>;
    async fn get_block(&self, block_id: String) -> Result<generated::QantoBlock, String>;
    async fn get_network_stats(&self) -> Result<NetworkStats, String>;
    fn balance_updates_receiver(
        &self,
    ) -> tokio::sync::broadcast::Receiver<generated::BalanceUpdate>;
    fn transaction_updates_receiver(
        &self,
    ) -> tokio::sync::broadcast::Receiver<AddressTransactionEntry>;
    // Wallet-specific balance with unconfirmed delta
    async fn get_wallet_balance(&self, address: String) -> Result<(u128, u128), String>;
    async fn get_transactions_by_address(
        &self,
        address: String,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<AddressTransactionEntry>, u64), String>;
}

#[derive(Clone)]
pub struct RpcService {
    backend: Arc<dyn RpcBackend>,
}

impl RpcService {
    pub fn new(backend: Arc<dyn RpcBackend>) -> Self {
        Self { backend }
    }

    fn format_monetary_value(base_units: u128) -> String {
        // Represent monetary values as base-unit strings to preserve full u128 precision.
        base_units.to_string()
    }

    fn parse_monetary_u128(field: &'static str, value: &str) -> Result<u128, Status> {
        value.parse::<u128>().map_err(|_| {
            Status::invalid_argument(format!(
                "invalid monetary value for {field}: must be base-unit unsigned integer string"
            ))
        })
    }

    fn validate_transaction_monetary_fields(tx: &generated::Transaction) -> Result<(), Status> {
        Self::parse_monetary_u128("transaction.amount", &tx.amount)?;
        Self::parse_monetary_u128("transaction.fee", &tx.fee)?;
        Self::parse_monetary_u128("transaction.gas_price", &tx.gas_price)?;
        Self::parse_monetary_u128("transaction.priority_fee", &tx.priority_fee)?;

        for (index, output) in tx.outputs.iter().enumerate() {
            let field = if index < 10 {
                match index {
                    0 => "transaction.outputs[0].amount",
                    1 => "transaction.outputs[1].amount",
                    2 => "transaction.outputs[2].amount",
                    3 => "transaction.outputs[3].amount",
                    4 => "transaction.outputs[4].amount",
                    5 => "transaction.outputs[5].amount",
                    6 => "transaction.outputs[6].amount",
                    7 => "transaction.outputs[7].amount",
                    8 => "transaction.outputs[8].amount",
                    _ => "transaction.outputs[9].amount",
                }
            } else {
                "transaction.outputs[n].amount"
            };

            Self::parse_monetary_u128(field, &output.amount)?;
        }

        if let Some(fee_breakdown) = tx.fee_breakdown.as_ref() {
            Self::parse_monetary_u128(
                "transaction.fee_breakdown.base_fee",
                &fee_breakdown.base_fee,
            )?;
            Self::parse_monetary_u128(
                "transaction.fee_breakdown.complexity_fee",
                &fee_breakdown.complexity_fee,
            )?;
            Self::parse_monetary_u128(
                "transaction.fee_breakdown.storage_fee",
                &fee_breakdown.storage_fee,
            )?;
            Self::parse_monetary_u128("transaction.fee_breakdown.gas_fee", &fee_breakdown.gas_fee)?;
            Self::parse_monetary_u128(
                "transaction.fee_breakdown.priority_fee",
                &fee_breakdown.priority_fee,
            )?;
            Self::parse_monetary_u128(
                "transaction.fee_breakdown.total_fee",
                &fee_breakdown.total_fee,
            )?;
            Self::parse_monetary_u128(
                "transaction.fee_breakdown.gas_price",
                &fee_breakdown.gas_price,
            )?;
        }

        Ok(())
    }
}

#[tonic::async_trait]
impl QantoRpc for RpcService {
    type SubscribeBalanceStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<generated::BalanceUpdate, Status>> + Send>>;
    type SubscribeTransactionsByAddressStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<AddressTransactionEntry, Status>> + Send>>;

    async fn submit_transaction(
        &self,
        request: Request<SubmitTransactionRequest>,
    ) -> Result<Response<SubmitTransactionResponse>, Status> {
        let req = request.into_inner();
        let tx = req
            .transaction
            .ok_or_else(|| Status::invalid_argument("missing transaction"))?;

        Self::validate_transaction_monetary_fields(&tx)?;
        match self.backend.submit_transaction(tx).await {
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
                base_units: Self::format_monetary_value(total),
                balance: Self::format_monetary_value(total),
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

    async fn get_transactions_by_address(
        &self,
        request: Request<GetTransactionsByAddressRequest>,
    ) -> Result<Response<GetTransactionsByAddressResponse>, Status> {
        let req = request.into_inner();
        let address = req.address;
        if address.len() != 64 || !address.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Status::invalid_argument("invalid address format"));
        }

        let page = req.page.max(1);
        let page_size = req.page_size.clamp(1, 200);

        match self
            .backend
            .get_transactions_by_address(address, page, page_size)
            .await
        {
            Ok((transactions, total)) => Ok(Response::new(GetTransactionsByAddressResponse {
                transactions,
                total,
                current_page: page,
                page_size,
            })),
            Err(e) => Err(Status::unavailable(e)),
        }
    }

    async fn subscribe_transactions_by_address(
        &self,
        request: Request<SubscribeTransactionsRequest>,
    ) -> Result<Response<Self::SubscribeTransactionsByAddressStream>, Status> {
        let req = request.into_inner();
        let address = req.address;
        if address.len() != 64 || !address.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Status::invalid_argument("invalid address format"));
        }

        let include_mempool = req.include_mempool;
        let include_confirmed = req.include_confirmed;
        let include_finalized = req.include_finalized;

        let rx = self.backend.transaction_updates_receiver();
        let stream = BroadcastStream::new(rx).filter_map(move |item| match item {
            Ok(update) => {
                let tx = update.transaction.as_ref()?;
                let matches_address = tx.sender == address
                    || tx.receiver == address
                    || tx.outputs.iter().any(|output| output.address == address);
                if !matches_address {
                    return None;
                }

                if update.in_mempool && !include_mempool {
                    return None;
                }
                if update.is_finalized && !include_finalized {
                    return None;
                }
                if !update.in_mempool
                    && !update.is_finalized
                    && update.confirmations > 0
                    && !include_confirmed
                {
                    return None;
                }

                Some(Ok(update))
            }
            Err(_) => None,
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
                balance: Self::format_monetary_value(confirmed),
                unconfirmed_balance: Self::format_monetary_value(unconfirmed_delta),
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

#[cfg(test)]
mod tests {
    use super::*;
    use generated::{
        BalanceUpdate, FeeBreakdown, GetBalanceRequest, Output, QantoBlock,
        QuantumResistantSignature, SubmitTransactionRequest, Transaction, WalletGetBalanceRequest,
    };
    use tokio::sync::broadcast;

    struct TestBackend {
        balance: u128,
        wallet_balance: (u128, u128),
        balance_sender: broadcast::Sender<BalanceUpdate>,
        tx_sender: broadcast::Sender<AddressTransactionEntry>,
    }

    impl TestBackend {
        fn new(balance: u128, wallet_balance: (u128, u128)) -> Self {
            let (balance_sender, _) = broadcast::channel(8);
            let (tx_sender, _) = broadcast::channel(8);
            Self {
                balance,
                wallet_balance,
                balance_sender,
                tx_sender,
            }
        }
    }

    #[async_trait]
    impl RpcBackend for TestBackend {
        async fn submit_transaction(&self, _tx: generated::Transaction) -> Result<(), String> {
            Ok(())
        }

        async fn get_balance(&self, _address: String) -> Result<u128, String> {
            Ok(self.balance)
        }

        async fn get_block(&self, block_id: String) -> Result<QantoBlock, String> {
            Ok(QantoBlock {
                id: block_id,
                signature: Some(QuantumResistantSignature::default()),
                ..QantoBlock::default()
            })
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

        fn balance_updates_receiver(&self) -> tokio::sync::broadcast::Receiver<BalanceUpdate> {
            self.balance_sender.subscribe()
        }

        fn transaction_updates_receiver(
            &self,
        ) -> tokio::sync::broadcast::Receiver<AddressTransactionEntry> {
            self.tx_sender.subscribe()
        }

        async fn get_wallet_balance(&self, _address: String) -> Result<(u128, u128), String> {
            Ok(self.wallet_balance)
        }

        async fn get_transactions_by_address(
            &self,
            _address: String,
            _page: u32,
            _page_size: u32,
        ) -> Result<(Vec<AddressTransactionEntry>, u64), String> {
            Ok((Vec::new(), 0))
        }
    }

    #[tokio::test]
    async fn get_balance_returns_full_precision_strings() {
        let large_balance = (u64::MAX as u128) + 42;
        let service = RpcService::new(Arc::new(TestBackend::new(large_balance, (0, 0))));

        let response = QantoRpc::get_balance(
            &service,
            Request::new(GetBalanceRequest {
                address: "abc".to_string(),
            }),
        )
        .await
        .expect("get_balance response")
        .into_inner();

        assert_eq!(response.base_units, large_balance.to_string());
        assert_eq!(response.balance, large_balance.to_string());
    }

    #[tokio::test]
    async fn wallet_balance_returns_full_precision_strings() {
        let confirmed = (u64::MAX as u128) + 7;
        let unconfirmed = (u64::MAX as u128) + 99;
        let service = RpcService::new(Arc::new(TestBackend::new(0, (confirmed, unconfirmed))));

        let response = WalletService::get_balance(
            &service,
            Request::new(WalletGetBalanceRequest {
                address: "def".to_string(),
            }),
        )
        .await
        .expect("wallet get_balance response")
        .into_inner();

        assert_eq!(response.balance, confirmed.to_string());
        assert_eq!(response.unconfirmed_balance, unconfirmed.to_string());
    }

    #[tokio::test]
    async fn submit_transaction_rejects_invalid_monetary_strings() {
        let service = RpcService::new(Arc::new(TestBackend::new(0, (0, 0))));
        let tx = Transaction {
            id: "tx".to_string(),
            sender: "sender".to_string(),
            receiver: "receiver".to_string(),
            amount: "12x".to_string(),
            fee: "1".to_string(),
            gas_limit: 0,
            gas_used: 0,
            gas_price: "1".to_string(),
            priority_fee: "0".to_string(),
            outputs: vec![Output {
                address: "receiver".to_string(),
                amount: "1".to_string(),
                ..Output::default()
            }],
            fee_breakdown: Some(FeeBreakdown {
                base_fee: "1".to_string(),
                complexity_fee: "0".to_string(),
                storage_fee: "0".to_string(),
                gas_fee: "0".to_string(),
                priority_fee: "0".to_string(),
                total_fee: "1".to_string(),
                gas_used: 0,
                gas_price: "1".to_string(),
                ..FeeBreakdown::default()
            }),
            ..Transaction::default()
        };

        let err = QantoRpc::submit_transaction(
            &service,
            Request::new(SubmitTransactionRequest {
                transaction: Some(tx),
            }),
        )
        .await
        .unwrap_err();

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid monetary value"));
    }
}
