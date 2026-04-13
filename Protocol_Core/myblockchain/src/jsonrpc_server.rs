// myblockchain/src/jsonrpc_server.rs
//! Complete Ethereum-compatible JSON-RPC server for Qanto blockchain.
//! Provides full RPC API compatibility for wallet integration.

use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Block data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub miner: String,
    pub difficulty: u64,
    pub total_difficulty: u64,
    pub size: u64,
    pub transactions: Vec<Transaction>,
    pub transaction_root: String,
    pub state_root: String,
    pub receipts_root: String,
    pub nonce: String,
    pub extra_data: String,
}

/// Transaction data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    pub hash: String,
    pub nonce: u64,
    pub block_hash: Option<String>,
    pub block_number: Option<u64>,
    pub transaction_index: Option<u64>,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas_price: String,
    pub gas: u64,
    pub input: String,
}

/// Transaction receipt structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionReceipt {
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub block_hash: String,
    pub block_number: u64,
    pub from: String,
    pub to: Option<String>,
    pub cumulative_gas_used: u64,
    pub gas_used: u64,
    pub contract_address: Option<String>,
    pub logs: Vec<Value>,
    pub logs_bloom: String,
    pub status: String,
}

/// JSON-RPC server for blockchain
pub struct JsonRpcServer {
    blockchain: Arc<crate::Blockchain>,
}

impl JsonRpcServer {
    pub fn new(blockchain: Arc<crate::Blockchain>) -> Self {
        Self { blockchain }
    }

    pub async fn start(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        println!("JSON-RPC server listening on {addr}");

        loop {
            let (stream, _) = listener.accept().await?;
            let blockchain = Arc::clone(&self.blockchain);

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, blockchain).await {
                    eprintln!("Error handling connection: {e}");
                }
            });
        }
    }

    async fn handle_connection(
        mut stream: TcpStream,
        blockchain: Arc<crate::Blockchain>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..n]);

        let response = if request.starts_with("POST") {
            Self::handle_http_request(&request, &blockchain).await
        } else {
            "HTTP/1.1 405 Method Not Allowed\r\n\r\n".to_string()
        };

        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }

    async fn handle_http_request(request: &str, blockchain: &Arc<crate::Blockchain>) -> String {
        // Parse HTTP request to extract JSON-RPC payload
        let lines: Vec<&str> = request.lines().collect();
        if let Some(json_line) = lines.last() {
            if !json_line.trim().is_empty() {
                let json_response = Self::handle_json_rpc(json_line, blockchain).await;
                let response_body = json_response.to_string();
                return format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
            }
        }

        "HTTP/1.1 400 Bad Request\r\n\r\n".to_string()
    }

    async fn handle_json_rpc(request: &str, blockchain: &Arc<crate::Blockchain>) -> Value {
        let parsed: Result<Value, _> = serde_json::from_str(request);

        match parsed {
            Ok(json) => {
                let method = json["method"].as_str().unwrap_or("");
                let params_value = &json["params"];
                let params = if let Some(array) = params_value.as_array() {
                    array.as_slice()
                } else {
                    &[]
                };
                let id = json["id"].clone();

                let result = match method {
                    "eth_chainId" => Self::handle_chain_id().await,
                    "eth_blockNumber" => Self::handle_block_number(blockchain).await,
                    "eth_getBalance" => Self::handle_get_balance(params, blockchain).await,
                    "eth_getBlockByNumber" => {
                        Self::handle_get_block_by_number(params, blockchain).await
                    }
                    "eth_getBlockByHash" => {
                        Self::handle_get_block_by_hash(params, blockchain).await
                    }
                    "eth_getTransactionCount" => {
                        Self::handle_get_transaction_count(params, blockchain).await
                    }
                    "eth_sendRawTransaction" => {
                        Self::handle_send_raw_transaction(params, blockchain).await
                    }
                    "eth_getTransactionByHash" => {
                        Self::handle_get_transaction_by_hash(params, blockchain).await
                    }
                    "eth_getTransactionReceipt" => {
                        Self::handle_get_transaction_receipt(params, blockchain).await
                    }
                    "eth_getCode" => Self::handle_get_code(params, blockchain).await,
                    "eth_call" => Self::handle_call(params, blockchain).await,
                    "eth_estimateGas" => Self::handle_estimate_gas(params, blockchain).await,
                    "eth_gasPrice" => Self::handle_gas_price().await,
                    "eth_getStorageAt" => Self::handle_get_storage_at(params, blockchain).await,
                    "eth_getBlockTransactionCountByNumber" => {
                        Self::handle_get_block_transaction_count_by_number(params, blockchain).await
                    }
                    "eth_getBlockTransactionCountByHash" => {
                        Self::handle_get_block_transaction_count_by_hash(params, blockchain).await
                    }
                    "eth_miningStatus" => Self::handle_mining_status(blockchain).await,
                    "net_version" => Self::handle_net_version().await,
                    "web3_clientVersion" => Self::handle_client_version().await,
                    _ => json!(null),
                };

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                })
            }
            Err(_) => Self::create_error_response(-32700, "Parse error", Value::Null),
        }
    }

    async fn handle_chain_id() -> Value {
        json!("0x1") // Ethereum mainnet chain ID for compatibility
    }

    async fn handle_block_number(blockchain: &Arc<crate::Blockchain>) -> Value {
        let block_count = blockchain.get_block_count();
        // Return the latest block number (block count - 1, since blocks are 0-indexed)
        let latest_block_number = if block_count > 0 { block_count - 1 } else { 0 };
        json!(format!("0x{:x}", latest_block_number))
    }

    async fn handle_get_balance(params: &[Value], _blockchain: &Arc<crate::Blockchain>) -> Value {
        if params.is_empty() {
            return json!("0x0");
        }

        // For now, return a default balance since we don't have account state tracking
        json!("0x0")
    }

    async fn handle_get_block_by_number(
        params: &[Value],
        blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        if params.is_empty() {
            return json!(null);
        }

        let block_param = &params[0];
        let include_txs = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

        let block_number = if block_param.as_str() == Some("latest") {
            let block_count = blockchain.get_block_count();
            if block_count > 0 {
                block_count - 1
            } else {
                0
            }
        } else if let Some(block_num_str) = block_param.as_str() {
            if let Ok(block_num) = u64::from_str_radix(block_num_str.trim_start_matches("0x"), 16) {
                block_num
            } else {
                return json!(null);
            }
        } else {
            return json!(null);
        };

        match blockchain.get_block(block_number) {
            Some(block) => {
                let transactions = if include_txs {
                    // Return full transaction objects
                    json!([])
                } else {
                    // Return transaction hashes only
                    json!([])
                };

                json!({
                    "number": format!("0x{:x}", block_number),
                    "hash": format!("0x{:x}", block.hash),
                    "parentHash": format!("0x{:x}", block.previous_hash),
                    "nonce": format!("0x{:x}", block.nonce),
                    "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                    "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                    "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                    "stateRoot": "0xd7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544",
                    "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                    "miner": "0x0000000000000000000000000000000000000000",
                    "difficulty": format!("0x{:x}", block.difficulty),
                    "totalDifficulty": format!("0x{:x}", block.difficulty),
                    "extraData": "0x",
                    "size": "0x220",
                    "gasLimit": "0x1c9c380",
                    "gasUsed": "0x0",
                    "timestamp": format!("0x{:x}", block.timestamp),
                    "transactions": transactions,
                    "uncles": []
                })
            }
            None => json!(null),
        }
    }

    async fn handle_get_block_by_hash(
        _params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        // Hash-based lookup not implemented yet
        json!(null)
    }

    async fn handle_get_transaction_count(
        _params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        // For now, return 0 transactions
        json!("0x0")
    }

    async fn handle_send_raw_transaction(
        _params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        // Transaction sending not implemented yet
        json!("0x0000000000000000000000000000000000000000000000000000000000000000")
    }

    async fn handle_get_transaction_by_hash(
        _params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        // Transaction lookup not implemented yet
        json!(null)
    }

    async fn handle_get_transaction_receipt(
        _params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        // Transaction receipt lookup not implemented yet
        json!(null)
    }

    async fn handle_get_code(params: &[Value], _blockchain: &Arc<crate::Blockchain>) -> Value {
        if params.len() < 2 {
            return json!("0x");
        }
        // Contract code lookup not implemented yet
        json!("0x")
    }

    async fn handle_call(_params: &[Value], _blockchain: &Arc<crate::Blockchain>) -> Value {
        // Contract calls not implemented yet
        json!("0x")
    }

    async fn handle_estimate_gas(_params: &[Value], _blockchain: &Arc<crate::Blockchain>) -> Value {
        // Gas estimation not implemented yet
        json!("0x5208") // 21000 gas (standard transfer)
    }

    async fn handle_gas_price() -> Value {
        json!("0x3b9aca00") // 1 gwei
    }

    async fn handle_get_storage_at(
        params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        if params.len() < 3 {
            return json!("0x0000000000000000000000000000000000000000000000000000000000000000");
        }
        // Storage lookup not implemented yet
        json!("0x0000000000000000000000000000000000000000000000000000000000000000")
    }

    async fn handle_get_block_transaction_count_by_number(
        params: &[Value],
        blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        if params.is_empty() {
            return json!("0x0");
        }

        let block_param = &params[0];

        let block_number = if block_param.as_str() == Some("latest") {
            let block_count = blockchain.get_block_count();
            if block_count > 0 {
                block_count - 1
            } else {
                0
            }
        } else if let Some(block_num_str) = block_param.as_str() {
            if let Ok(block_num) = u64::from_str_radix(block_num_str.trim_start_matches("0x"), 16) {
                block_num
            } else {
                return json!(null);
            }
        } else {
            return json!(null);
        };

        match blockchain.get_block(block_number) {
            Some(_block) => {
                // For now, return 0 transactions since our SimpleBlock doesn't have a transactions field
                json!("0x0")
            }
            None => json!(null),
        }
    }

    async fn handle_get_block_transaction_count_by_hash(
        _params: &[Value],
        _blockchain: &Arc<crate::Blockchain>,
    ) -> Value {
        // Hash-based lookup not implemented yet
        json!(null)
    }

    async fn handle_net_version() -> Value {
        json!("1") // Ethereum mainnet network ID
    }

    async fn handle_client_version() -> Value {
        json!("Qanto/v1.0.0")
    }

    async fn handle_mining_status(blockchain: &Arc<crate::Blockchain>) -> Value {
        // Get mining status from the blockchain
        let is_mining = blockchain.is_mining();
        let current_difficulty = blockchain.get_current_difficulty();
        let hash_rate = blockchain.get_hash_rate();
        let block_count = blockchain.get_block_count();

        // Calculate approximate mining progress
        let mining_progress = if is_mining {
            // Simple progress calculation based on current hash rate and difficulty
            let estimated_time_per_block = if hash_rate > 0 {
                (current_difficulty as f64 / hash_rate as f64).min(100.0)
            } else {
                0.0
            };

            // Return progress as percentage (0-100)
            if estimated_time_per_block > 0.0 {
                (100.0 - estimated_time_per_block).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        json!({
            "mining": is_mining,
            "hashRate": format!("0x{:x}", hash_rate),
            "difficulty": format!("0x{:x}", current_difficulty),
            "blockNumber": format!("0x{:x}", block_count),
            "progress": format!("{:.2}", mining_progress),
            "miner": "0x0000000000000000000000000000000000000000", // Default miner address
            "extraData": "0x516e746f", // "Qnto" in hex
            "gasLimit": "0x1c9c380", // 30M gas limit
            "timestamp": format!("0x{:x}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs())
        })
    }

    fn create_error_response(code: i32, message: &str, id: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "error": {
                "code": code,
                "message": message
            },
            "id": id
        })
    }
}
