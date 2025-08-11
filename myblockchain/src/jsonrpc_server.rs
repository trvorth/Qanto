// myblockchain/src/jsonrpc_server.rs
//! Minimal Ethereum-compatible JSON-RPC adapter for Qanto blockchain.
//! Non-invasive: reads chain state via Arc<Blockchain> and exposes a small set
//! of methods used by wallets (MetaMask).

use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task;
use warp::Filter;

use crate::Blockchain;

/// Incoming JSON-RPC request (simple subset).
#[derive(Debug, Deserialize)]
struct RpcRequest {
    method: String,
    #[serde(default)]
    params: Value,
    id: Option<Value>,
}

/// Start the JSON-RPC server in a background tokio task.
/// - `blockchain` - Arc to your chain instance so we can read state.
/// - `addr` - a string like "127.0.0.1:8545" or "0.0.0.0:8545" to bind to.
/// - `chain_id` - decimal chain id (e.g. 13371337). Returned in hex for eth_chainId.
pub fn run_server(blockchain: Arc<Blockchain>, addr: String, chain_id: u64) -> task::JoinHandle<()> {
    task::spawn(async move {
        // parse socket address
        let socket: SocketAddr = match addr.parse() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("jsonrpc: failed to parse socket address {addr}: {e}");
                return;
            }
        };

        // POST / (JSON-RPC)
        let bc_filter = warp::any().map(move || blockchain.clone());
        let cid = chain_id;

        let rpc = warp::post()
            .and(warp::body::json())
            .and(bc_filter)
            .and_then(move |req: RpcRequest, bc: Arc<Blockchain>| {
                let cid = cid;
                async move {
                    let id = req.id.clone().unwrap_or_else(|| json!(1));
                    let method = req.method.as_str();

                    let result = match method {
                        // Return chain ID as hex string (eth_chainId)
                        "eth_chainId" => json!(format!("0x{:x}", cid)),

                        // Return decimal network id (net_version)
                        "net_version" => json!(format!("{}", cid)),

                        // client version
                        "web3_clientVersion" => json!("Qanto/v0.1"),

                        // block number (hex)
                        "eth_blockNumber" => {
                            let block = bc.get_last_block().await;
                            json!(format!("0x{:x}", block.header.index))
                        }

                        // Minimal eth_getBlockByNumber supporting "latest"
                        "eth_getBlockByNumber" => {
                            let params = req.params;
                            let which = if params.is_array() && !params.as_array().unwrap().is_empty() {
                                params[0].clone()
                            } else {
                                json!("latest")
                            };

                            if which == json!("latest") {
                                let block = bc.get_last_block().await;
                                // Use as_bytes() on QantoHash to get raw bytes for hex::encode
                                let hash_hex = hex::encode(block.get_hash().as_bytes());
                                let parent_hex = hex::encode(block.header.previous_hash.as_bytes());

                                json!({
                                    "number": format!("0x{:x}", block.header.index),
                                    "hash": format!("0x{}", hash_hex),
                                    "parentHash": format!("0x{}", parent_hex),
                                    "nonce": "0x0",
                                    "timestamp": format!("0x{:x}", block.header.timestamp),
                                    "transactions": json!([])
                                })
                            } else {
                                // Not implemented for other selectors
                                json!(null)
                            }
                        }

                        // default: not implemented (returns null result)
                        _ => json!(null),
                    };

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    });

                    Ok::<_, warp::Rejection>(warp::reply::json(&resp))
                }
            });

        eprintln!("jsonrpc: serving on {socket}");
        warp::serve(rpc).run(socket).await;
    })
}
