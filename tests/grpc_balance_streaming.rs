//! gRPC balance streaming integration tests
//!
//! Validates that the SubscribeBalance RPC:
//! - Respects `finalized_only` filtering
//! - Enriches updates with wallet balance breakdown via `query_wallet_balance`
//!
//! Executor context: Tokio runtime provided by `#[tokio::test]`.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};

use qanto_rpc::server::generated::{
    self, qanto_rpc_client::QantoRpcClient, BalanceUpdate, SubscribeBalanceRequest,
};
use qanto_rpc::{start, NetworkStats, RpcBackend};

#[derive(Clone)]
struct TestBackend {
    spendable: u64,
    immature: u64,
    unconfirmed: u64,
    total: u64,
    wallet_confirmed: u64,
    wallet_unconfirmed: u64,
    bal_tx: broadcast::Sender<generated::BalanceUpdate>,
}

#[async_trait::async_trait]
impl RpcBackend for TestBackend {
    async fn submit_transaction(&self, _tx: generated::Transaction) -> Result<(), String> {
        Ok(())
    }

    async fn get_balance(&self, _address: String) -> Result<u64, String> {
        Ok(self.total)
    }

    async fn get_block(&self, _block_id: String) -> Result<generated::QantoBlock, String> {
        Err("not implemented".into())
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

    async fn query_wallet_balance(&self, _address: String) -> Result<(u64, u64, u64, u64), String> {
        Ok((self.spendable, self.immature, self.unconfirmed, self.total))
    }

    async fn request_airdrop(
        &self,
        _request: generated::RequestAirdropRequest,
    ) -> Result<generated::RequestAirdropResponse, String> {
        Err("not implemented".into())
    }
}

async fn start_test_server(
    port: u16,
) -> (
    tokio::task::JoinHandle<()>,
    broadcast::Sender<generated::BalanceUpdate>,
) {
    let (tx, _rx) = broadcast::channel(16);
    let backend = TestBackend {
        spendable: 800_000_000,
        immature: 50_000_000,
        unconfirmed: 10_000_000,
        total: 850_000_000,
        wallet_confirmed: 850_000_000,
        wallet_unconfirmed: 10_000_000,
        bal_tx: tx.clone(),
    };
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    let handle = tokio::spawn(async move {
        // Ignore server result type here to avoid test-level tonic dependency.
        let _ = start(addr, Arc::new(backend)).await;
    });
    // Allow the server a brief moment to start listening
    sleep(Duration::from_millis(150)).await;
    (handle, tx)
}

/// End-to-end test for SubscribeBalance: ensures finalized-only filtering
/// and enrichment fields are populated from `query_wallet_balance`.
#[tokio::test]
async fn grpc_subscribe_balance_enrichment_and_finalized_filter() {
    let port = 50071;
    let (server_handle, tx) = start_test_server(port).await;

    let endpoint = format!("http://127.0.0.1:{}", port);
    let mut client = QantoRpcClient::connect(endpoint)
        .await
        .expect("client connect");

    // Subscribe with filter and finalized_only true.
    let req = SubscribeBalanceRequest {
        address_filter: "qanto_test_address_123".to_string(),
        finalized_only: true,
    };
    let response = client.subscribe_balance(req).await.expect("subscribe ok");
    let mut stream = response.into_inner();

    // Send unfinalized update first (should be filtered).
    let _ = tx.send(BalanceUpdate {
        address: "qanto_test_address_123".to_string(),
        base_units: 840_000_000,
        timestamp: 1,
        finalized: false,
        balance_bigint: "840000000".to_string(),
        ..Default::default()
    });

    // Send finalized update (should be received).
    let _ = tx.send(BalanceUpdate {
        address: "qanto_test_address_123".to_string(),
        base_units: 850_000_000,
        timestamp: 2,
        finalized: true,
        balance_bigint: "850000000".to_string(),
        ..Default::default()
    });

    let item = stream
        .message()
        .await
        .expect("stream recv")
        .expect("item present");

    assert_eq!(item.address, "qanto_test_address_123");
    assert!(item.finalized);
    assert_eq!(item.base_units, 850_000_000);

    // Server enriches fields using query_wallet_balance
    assert_eq!(item.spendable_confirmed, 800_000_000);
    assert_eq!(item.immature_coinbase_confirmed, 50_000_000);
    assert_eq!(item.unconfirmed_delta, 10_000_000);
    assert_eq!(item.total_confirmed, 850_000_000);

    // Cleanup: stop the server to avoid port conflicts for other tests
    server_handle.abort();
}

/// Ensures address filtering drops updates for other addresses and delivers
/// only those matching the `address_filter`.
#[tokio::test]
async fn grpc_subscribe_balance_filters_by_address() {
    let port = 50072;
    let (server_handle, tx) = start_test_server(port).await;

    let endpoint = format!("http://127.0.0.1:{}", port);
    let mut client = QantoRpcClient::connect(endpoint)
        .await
        .expect("client connect");

    let filter_addr = "qanto_addr_A".to_string();
    let other_addr = "qanto_addr_B".to_string();

    let req = SubscribeBalanceRequest {
        address_filter: filter_addr.clone(),
        finalized_only: false,
    };
    let response = client.subscribe_balance(req).await.expect("subscribe ok");
    let mut stream = response.into_inner();

    // Send update for different address (should be filtered out)
    let _ = tx.send(BalanceUpdate {
        address: other_addr.clone(),
        base_units: 1,
        timestamp: 10,
        finalized: true,
        balance_bigint: "1".to_string(),
        ..Default::default()
    });

    // Send matching address update (should be received)
    let _ = tx.send(BalanceUpdate {
        address: filter_addr.clone(),
        base_units: 2,
        timestamp: 11,
        finalized: true,
        balance_bigint: "2".to_string(),
        ..Default::default()
    });

    let item = stream
        .message()
        .await
        .expect("stream recv")
        .expect("item present");

    assert_eq!(item.address, filter_addr);
    assert_eq!(item.base_units, 2);

    server_handle.abort();
}
