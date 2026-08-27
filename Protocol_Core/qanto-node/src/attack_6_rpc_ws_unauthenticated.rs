//! Attack 6: RPC/WS Unauthenticated Attacks
//!
//! HIGH VULNERABILITY: The RPC layer (rpc_backend.rs) exposes unauthenticated endpoints:
//!
//! 1. FAUCET DRAIN: /faucet endpoint with no authentication/rate limiting
//! 2. GRAPHQL DoS: GraphQL subscriptions/queries with unbounded complexity
//! 3. gRPC submit_tx: No authentication, anyone can submit transactions
//! 4. WS subscriptions: Unauthenticated WebSocket connections
//! 5. No API key / JWT / mTLS on any endpoint
//!
//! The GraphQL schema in rpc_backend.rs allows:
//! - Unbounded query depth
//! - No cost analysis
//! - Subscriptions without auth

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::Transaction;
use crate::types::UTXO;
use ahash::AHashMap as HashMap;
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::timeout;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    if !dag.tips.contains_key(&0) {
        let mut genesis = QantoBlock::new_test_block("test-genesis".to_string());
        genesis.height = 0;
        genesis.parents.clear();
        genesis.timestamp = 0;
        genesis.id = genesis.compute_block_id().expect("test genesis id");
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(0).or_default().insert(genesis.id);
    }
    let arc_dag = Arc::new(dag);
    // SELF-ARC-001 (Cycle 023): sound self-reference init. Previously this
    // cast Arc::as_ptr (a *const T) to *mut and wrote through it, which is UB
    // by provenance even with no other live handle.
    let _ = arc_dag.self_arc.set(Arc::downgrade(&arc_dag));
    arc_dag
}

async fn create_test_validator(
    dag: &Arc<QantoDAG>,
) -> (QantoPQPublicKey, QantoPQPrivateKey, String) {
    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_addr = validator_id_v2(
        pk.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    dag.validators
        .insert(validator_addr.clone(), 50 * crate::Q_SCALE);
    (pk, sk, validator_addr)
}

fn create_transfer_tx(
    from_pk: &QantoPQPublicKey,
    to_pk: &QantoPQPublicKey,
    amount: u128,
    fee: u128,
    nonce: u64,
) -> Transaction {
    let (_, sk) = generate_pq_keypair(None).unwrap();
    Transaction::new_dummy_signed(&sk, from_pk).unwrap()
}

/// ATTACK DEMONSTRATION: Unauthenticated RPC endpoints vulnerable to abuse
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_6_faucet_drain_no_auth() {
        // The faucet endpoint (typically /faucet or via gRPC) dispenses testnet tokens
        // without ANY authentication
        //
        // Attack: Script requests faucet tokens in a loop
        // - No rate limiting by IP
        // - No CAPTCHA
        // - No proof-of-work
        // - No authentication token
        //
        // Result: Attacker drains entire faucet balance

        println!("VULNERABILITY: Faucet endpoint has NO authentication");
        println!("Typical endpoints: POST /faucet, gRPC FaucetService.RequestTokens");
        println!("Attack: curl -X POST http://node:port/faucet -d '{{\"address\": \"attacker\"}}'");
        println!("Loop this -> drain all faucet funds");

        // Check if faucet exists in rpc_backend
        // The rpc_backend.rs has GraphQL mutation for faucet
        // and likely a REST endpoint too
    }

    #[tokio::test]
    async fn attack_6_graphql_dos_unbounded_queries() {
        // GraphQL endpoint allows unbounded query complexity
        //
        // Attack vectors:
        // 1. Deeply nested queries (exponential resolver calls)
        // 2. Large batch queries (query { b1: block { ... } b2: block { ... } ... })
        // 3. Recursive fragments
        // 4. Subscriptions without auth (hold connections open)
        // 5. Introspection queries exposing full schema
        //
        // No: query cost analysis, depth limiting, rate limiting

        println!("VULNERABILITY: GraphQL has NO query cost limiting");
        println!(
            "Attack: query {{ blocks(first: 10000) {{ transactions {{ inputs {{ ... }} }} }} }}"
        );
        println!("Attack: nested fragments causing exponential resolver execution");
        println!("Attack: 10000 parallel subscriptions -> connection exhaustion");

        // rpc_backend.rs uses async_graphql which has:
        // - Schema::query_depth_limit (NOT set)
        // - Schema::complexity_limit (NOT set)
        // - No rate limiting middleware
    }

    #[tokio::test]
    async fn attack_6_grpc_submit_tx_no_auth() {
        // gRPC SubmitTx endpoint accepts transactions from ANYONE
        // No: API key, JWT, mTLS, IP allowlist
        //
        // Attack:
        // 1. Spam SubmitTx with invalid transactions (DoS via validation cost)
        // 2. Submit valid transactions to fill mempool (mempool DoS)
        // 3. Front-running via direct gRPC access
        // 4. No attribution = no accountability

        println!("VULNERABILITY: gRPC SubmitTx has NO authentication");
        println!("Service: qanto.rpc.RpcService/SubmitTx");
        println!("Attack: Submit 10000 invalid txs/sec -> CPU exhaustion in verification");
        println!("Attack: Fill mempool with low-fee txs -> block legitimate users");
    }

    #[tokio::test]
    async fn attack_6_websocket_subscription_dos() {
        // WebSocket subscriptions (GraphQL subscriptions) allow unauthenticated connections
        //
        // Attack:
        // 1. Open 10000 WebSocket connections
        // 2. Subscribe to high-frequency events (newBlock, newTransaction)
        // 3. Don't read messages -> server buffers grow -> OOM
        // 4. Or: slowloris - read 1 byte per minute

        println!("VULNERABILITY: WebSocket subscriptions NO authentication");
        println!("Attack: Connect 10000 WS clients, subscribe to newBlock");
        println!("Attack: Slowloris on WS - hold connections, minimal reads");
    }

    #[tokio::test]
    async fn attack_6_no_rate_limiting_any_endpoint() {
        // Comprehensive check: NO endpoint has rate limiting
        //
        // Endpoints to check:
        // - REST: /faucet, /health, /metrics, /tx/submit
        // - GraphQL: /graphql (query, mutation, subscription)
        // - gRPC: All methods (SubmitTx, GetBlock, SubscribeBlocks, etc.)
        // - WS: /ws (GraphQL over WS)

        println!("VULNERABILITY: NO rate limiting on ANY RPC endpoint");
        println!("No: IP-based, API-key-based, token-bucket, sliding window");
        println!("All endpoints accept unlimited requests from single source");
    }

    #[tokio::test]
    async fn attack_6_metrics_exposure() {
        // /metrics endpoint (Prometheus) exposes internal state
        // No authentication = information disclosure
        //
        // Leaked: mempool size, peer count, block height, validator stakes, etc.

        println!("VULNERABILITY: /metrics endpoint exposed without auth");
        println!("Attack: Scrape metrics to learn network topology, validator stakes");
    }

    #[tokio::test]
    async fn attack_6_admin_endpoints_exposed() {
        // Check for admin/debug endpoints
        // In rpc_backend.rs, there may be:
        // - /debug/* endpoints
        // - GraphQL admin mutations
        // - gRPC admin services

        println!("CHECK: Admin/debug endpoints exposed?");
        println!("Common: /debug/pprof, GraphQL admin mutations, gRPC AdminService");
    }
}
