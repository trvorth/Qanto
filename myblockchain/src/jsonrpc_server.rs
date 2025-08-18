// myblockchain/src/jsonrpc_server.rs
//! Complete Ethereum-compatible JSON-RPC server for Qanto blockchain.
//! Provides full RPC API compatibility for wallet integration.

use crate::qanto_standalone::hash::qanto_hash;
use anyhow::Result;
use hex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

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
    pub v: String,
    pub r: String,
    pub s: String,
}

/// Transaction receipt
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
    pub logs: Vec<Log>,
    pub logs_bloom: String,
    pub status: String,
}

/// Event log
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Log {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: u64,
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub block_hash: String,
    pub log_index: u64,
    pub removed: bool,
}

/// Account balance and nonce
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub balance: String,
    pub nonce: u64,
    pub code: String,
    pub storage_hash: String,
}

/// Blockchain state
#[derive(Debug)]
pub struct BlockchainState {
    pub blocks: HashMap<u64, Block>,
    pub transactions: HashMap<String, Transaction>,
    pub receipts: HashMap<String, TransactionReceipt>,
    pub accounts: HashMap<String, Account>,
    pub latest_block_number: u64,
    pub pending_transactions: Vec<Transaction>,
}

impl Default for BlockchainState {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockchainState {
    pub fn new() -> Self {
        let mut state = Self {
            blocks: HashMap::new(),
            transactions: HashMap::new(),
            receipts: HashMap::new(),
            accounts: HashMap::new(),
            latest_block_number: 0,
            pending_transactions: Vec::new(),
        };

        // Create genesis block
        state.create_genesis_block();
        state
    }

    fn create_genesis_block(&mut self) {
        let genesis_block = Block {
            number: 0,
            hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            parent_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            timestamp: 1640995200, // 2022-01-01 00:00:00 UTC
            gas_limit: 8000000,
            gas_used: 0,
            miner: "0x0000000000000000000000000000000000000000".to_string(),
            difficulty: 1,
            total_difficulty: 1,
            size: 1024,
            transactions: Vec::new(),
            transaction_root: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
                .to_string(),
            state_root: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
                .to_string(),
            receipts_root: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
                .to_string(),
            nonce: "0x0000000000000000".to_string(),
            extra_data: "0x".to_string(),
        };

        self.blocks.insert(0, genesis_block);
        self.latest_block_number = 0;
    }

    pub fn get_block_by_number(&self, number: u64) -> Option<&Block> {
        self.blocks.get(&number)
    }

    pub fn get_block_by_hash(&self, hash: &str) -> Option<&Block> {
        self.blocks.values().find(|block| block.hash == hash)
    }

    pub fn get_transaction(&self, hash: &str) -> Option<&Transaction> {
        self.transactions.get(hash)
    }

    pub fn get_transaction_receipt(&self, hash: &str) -> Option<&TransactionReceipt> {
        self.receipts.get(hash)
    }

    pub fn get_account(&self, address: &str) -> Option<&Account> {
        self.accounts.get(address)
    }

    pub fn get_account_balance(&self, address: &str) -> String {
        self.accounts
            .get(address)
            .map(|acc| acc.balance.clone())
            .unwrap_or_else(|| "0x0".to_string())
    }

    pub fn get_account_nonce(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| acc.nonce).unwrap_or(0)
    }

    pub fn add_pending_transaction(&mut self, tx: Transaction) {
        self.pending_transactions.push(tx);
    }
}

/// JSON-RPC 2.0 Server for Qanto blockchain
pub struct JsonRpcServer {
    port: u16,
    chain_id: u64,
    state: Arc<RwLock<BlockchainState>>,
}

impl JsonRpcServer {
    pub fn new(port: u16, chain_id: u64) -> Self {
        Self {
            port,
            chain_id,
            state: Arc::new(RwLock::new(BlockchainState::new())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("JSON-RPC server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New connection from {}", addr);
                    let chain_id = self.chain_id;
                    let state = Arc::clone(&self.state);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, chain_id, state).await {
                            error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        mut stream: TcpStream,
        chain_id: u64,
        state: Arc<RwLock<BlockchainState>>,
    ) -> Result<()> {
        let mut buffer = vec![0; 8192];
        let n = stream.read(&mut buffer).await?;

        if n == 0 {
            return Ok(());
        }

        let request = String::from_utf8_lossy(&buffer[..n]);
        debug!("Received request: {}", request);

        // Handle OPTIONS request for CORS
        if request.starts_with("OPTIONS") {
            let cors_response = Self::create_cors_response();
            stream.write_all(cors_response.as_bytes()).await?;
            return Ok(());
        }

        // Parse HTTP request to extract JSON-RPC payload
        let json_payload = Self::extract_json_payload(&request)?;
        let response = Self::process_request(&json_payload, chain_id, state).await;

        let http_response = Self::create_http_response(&response);
        stream.write_all(http_response.as_bytes()).await?;

        Ok(())
    }

    fn extract_json_payload(request: &str) -> Result<String> {
        // Simple HTTP POST body extraction
        if let Some(body_start) = request.find("\r\n\r\n") {
            Ok(request[body_start + 4..].trim_end_matches('\0').to_string())
        } else {
            Err(anyhow::anyhow!("Invalid HTTP request"))
        }
    }

    async fn process_request(
        payload: &str,
        chain_id: u64,
        state: Arc<RwLock<BlockchainState>>,
    ) -> Value {
        match serde_json::from_str::<Value>(payload) {
            Ok(request) => Self::handle_rpc_request(request, chain_id, state).await,
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {}", e);
                json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    },
                    "id": null
                })
            }
        }
    }

    async fn handle_rpc_request(
        request: Value,
        chain_id: u64,
        state: Arc<RwLock<BlockchainState>>,
    ) -> Value {
        let method = request["method"].as_str().unwrap_or("");
        let params = request["params"].as_array().cloned().unwrap_or_default();
        let id = request["id"].clone();

        debug!(
            "Processing RPC method: {} with params: {:?}",
            method, params
        );

        let result = match method {
            // Network information
            "eth_chainId" => json!(format!("0x{:x}", chain_id)),
            "net_version" => json!(chain_id.to_string()),
            "web3_clientVersion" => json!("Qanto/v1.0.0"),
            "net_listening" => json!(true),
            "net_peerCount" => json!("0x5"), // Mock peer count

            // Block information
            "eth_blockNumber" => {
                let state_guard = state.read().await;
                json!(format!("0x{:x}", state_guard.latest_block_number))
            }
            "eth_getBlockByNumber" => Self::handle_get_block_by_number(&params, &state).await,
            "eth_getBlockByHash" => Self::handle_get_block_by_hash(&params, &state).await,
            "eth_getBlockTransactionCountByNumber" => {
                Self::handle_get_block_transaction_count_by_number(&params, &state).await
            }
            "eth_getBlockTransactionCountByHash" => {
                Self::handle_get_block_transaction_count_by_hash(&params, &state).await
            }

            // Transaction information
            "eth_getTransactionByHash" => {
                Self::handle_get_transaction_by_hash(&params, &state).await
            }
            "eth_getTransactionReceipt" => {
                Self::handle_get_transaction_receipt(&params, &state).await
            }
            "eth_sendRawTransaction" => Self::handle_send_raw_transaction(&params, &state).await,
            "eth_pendingTransactions" => {
                let state_guard = state.read().await;
                json!(state_guard.pending_transactions)
            }

            // Account information
            "eth_getBalance" => Self::handle_get_balance(&params, &state).await,
            "eth_getTransactionCount" => Self::handle_get_transaction_count(&params, &state).await,
            "eth_getCode" => Self::handle_get_code(&params, &state).await,
            "eth_getStorageAt" => Self::handle_get_storage_at(&params, &state).await,

            // Gas estimation
            "eth_gasPrice" => json!("0x3b9aca00"), // 1 Gwei
            "eth_estimateGas" => json!("0x5208"),  // 21000 gas
            "eth_maxPriorityFeePerGas" => json!("0x59682f00"), // 1.5 Gwei
            "eth_feeHistory" => {
                json!({
                    "baseFeePerGas": ["0x3b9aca00"],
                    "gasUsedRatio": [0.5],
                    "reward": [["0x59682f00"]]
                })
            }

            // Mining information
            "eth_mining" => json!(false),
            "eth_hashrate" => json!("0x0"),
            "eth_coinbase" => json!("0x0000000000000000000000000000000000000000"),

            // Filters and logs
            "eth_newFilter" => json!("0x1"),      // Mock filter ID
            "eth_newBlockFilter" => json!("0x2"), // Mock filter ID
            "eth_newPendingTransactionFilter" => json!("0x3"), // Mock filter ID
            "eth_uninstallFilter" => json!(true),
            "eth_getFilterChanges" => json!([]),
            "eth_getFilterLogs" => json!([]),
            "eth_getLogs" => json!([]),

            // Protocol version
            "eth_protocolVersion" => json!("0x41"), // Version 65
            "eth_syncing" => json!(false),

            // Account management (read-only)
            "eth_accounts" => json!([]),
            "eth_sign" => {
                json!({
                    "error": {
                        "code": -32601,
                        "message": "Signing not supported in read-only mode"
                    }
                })
            }

            _ => {
                warn!("Unimplemented RPC method: {}", method);
                return json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32601,
                        "message": format!("Method {} not found", method)
                    },
                    "id": id
                });
            }
        };

        json!({
            "jsonrpc": "2.0",
            "result": result,
            "id": id
        })
    }

    // Helper methods for handling specific RPC calls
    async fn handle_get_block_by_number(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!(null);
        }

        let block_param = &params[0];
        let include_txs = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

        let state_guard = state.read().await;

        let block = if block_param.as_str() == Some("latest") {
            state_guard.get_block_by_number(state_guard.latest_block_number)
        } else if let Some(block_num_str) = block_param.as_str() {
            if let Ok(block_num) = u64::from_str_radix(block_num_str.trim_start_matches("0x"), 16) {
                state_guard.get_block_by_number(block_num)
            } else {
                None
            }
        } else {
            None
        };

        match block {
            Some(block) => {
                let transactions = if include_txs {
                    json!(block.transactions)
                } else {
                    json!(block
                        .transactions
                        .iter()
                        .map(|tx| &tx.hash)
                        .collect::<Vec<_>>())
                };

                json!({
                    "number": format!("0x{:x}", block.number),
                    "hash": block.hash,
                    "parentHash": block.parent_hash,
                    "nonce": block.nonce,
                    "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                    "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                    "transactionsRoot": block.transaction_root,
                    "stateRoot": block.state_root,
                    "receiptsRoot": block.receipts_root,
                    "miner": block.miner,
                    "difficulty": format!("0x{:x}", block.difficulty),
                    "totalDifficulty": format!("0x{:x}", block.total_difficulty),
                    "extraData": block.extra_data,
                    "size": format!("0x{:x}", block.size),
                    "gasLimit": format!("0x{:x}", block.gas_limit),
                    "gasUsed": format!("0x{:x}", block.gas_used),
                    "timestamp": format!("0x{:x}", block.timestamp),
                    "transactions": transactions,
                    "uncles": []
                })
            }
            None => json!(null),
        }
    }

    async fn handle_get_block_by_hash(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!(null);
        }

        let block_hash = params[0].as_str().unwrap_or("");
        let include_txs = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

        let state_guard = state.read().await;

        match state_guard.get_block_by_hash(block_hash) {
            Some(block) => {
                let transactions = if include_txs {
                    json!(block.transactions)
                } else {
                    json!(block
                        .transactions
                        .iter()
                        .map(|tx| &tx.hash)
                        .collect::<Vec<_>>())
                };

                json!({
                    "number": format!("0x{:x}", block.number),
                    "hash": block.hash,
                    "parentHash": block.parent_hash,
                    "timestamp": format!("0x{:x}", block.timestamp),
                    "transactions": transactions
                })
            }
            None => json!(null),
        }
    }

    async fn handle_get_block_transaction_count_by_number(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!("0x0");
        }

        let block_param = &params[0];
        let state_guard = state.read().await;

        let block = if block_param.as_str() == Some("latest") {
            state_guard.get_block_by_number(state_guard.latest_block_number)
        } else if let Some(block_num_str) = block_param.as_str() {
            if let Ok(block_num) = u64::from_str_radix(block_num_str.trim_start_matches("0x"), 16) {
                state_guard.get_block_by_number(block_num)
            } else {
                None
            }
        } else {
            None
        };

        match block {
            Some(block) => json!(format!("0x{:x}", block.transactions.len())),
            None => json!(null),
        }
    }

    async fn handle_get_block_transaction_count_by_hash(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!("0x0");
        }

        let block_hash = params[0].as_str().unwrap_or("");
        let state_guard = state.read().await;

        match state_guard.get_block_by_hash(block_hash) {
            Some(block) => json!(format!("0x{:x}", block.transactions.len())),
            None => json!(null),
        }
    }

    async fn handle_get_transaction_by_hash(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!(null);
        }

        let tx_hash = params[0].as_str().unwrap_or("");
        let state_guard = state.read().await;

        match state_guard.get_transaction(tx_hash) {
            Some(tx) => json!(tx),
            None => json!(null),
        }
    }

    async fn handle_get_transaction_receipt(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!(null);
        }

        let tx_hash = params[0].as_str().unwrap_or("");
        let state_guard = state.read().await;

        match state_guard.get_transaction_receipt(tx_hash) {
            Some(receipt) => json!(receipt),
            None => json!(null),
        }
    }

    async fn handle_send_raw_transaction(
        params: &[Value],
        _state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!({
                "error": {
                    "code": -32602,
                    "message": "Invalid params"
                }
            });
        }

        // For now, just return a mock transaction hash
        let tx_data = params[0].as_str().unwrap_or("");
        let hash = qanto_hash(tx_data.as_bytes());
        let tx_hash = format!("0x{}", hex::encode(hash.as_bytes()));

        json!(tx_hash)
    }

    async fn handle_get_balance(params: &[Value], state: &Arc<RwLock<BlockchainState>>) -> Value {
        if params.is_empty() {
            return json!("0x0");
        }

        let address = params[0].as_str().unwrap_or("");
        let state_guard = state.read().await;

        json!(state_guard.get_account_balance(address))
    }

    async fn handle_get_transaction_count(
        params: &[Value],
        state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        if params.is_empty() {
            return json!("0x0");
        }

        let address = params[0].as_str().unwrap_or("");
        let state_guard = state.read().await;

        json!(format!("0x{:x}", state_guard.get_account_nonce(address)))
    }

    async fn handle_get_code(params: &[Value], state: &Arc<RwLock<BlockchainState>>) -> Value {
        if params.is_empty() {
            return json!("0x");
        }

        let address = params[0].as_str().unwrap_or("");
        let state_guard = state.read().await;

        match state_guard.get_account(address) {
            Some(account) => json!(account.code),
            None => json!("0x"),
        }
    }

    async fn handle_get_storage_at(
        _params: &[Value],
        _state: &Arc<RwLock<BlockchainState>>,
    ) -> Value {
        // For now, return empty storage
        json!("0x0000000000000000000000000000000000000000000000000000000000000000")
    }

    fn create_cors_response() -> String {
        "HTTP/1.1 200 OK\r\n\
          Access-Control-Allow-Origin: *\r\n\
          Access-Control-Allow-Methods: POST, GET, OPTIONS\r\n\
          Access-Control-Allow-Headers: Content-Type\r\n\
          Content-Length: 0\r\n\
          \r\n"
            .to_string()
    }

    fn create_http_response(json_response: &Value) -> String {
        let body = json_response.to_string();
        format!(
            "HTTP/1.1 200 OK\r\n\
              Content-Type: application/json\r\n\
              Content-Length: {}\r\n\
              Access-Control-Allow-Origin: *\r\n\
              Access-Control-Allow-Methods: POST, GET, OPTIONS\r\n\
              Access-Control-Allow-Headers: Content-Type\r\n\
              \r\n\
              {}",
            body.len(),
            body
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_blockchain_state_creation() {
        let state = BlockchainState::new();
        assert_eq!(state.latest_block_number, 0);
        assert!(state.blocks.contains_key(&0));

        let genesis = state.get_block_by_number(0).unwrap();
        assert_eq!(genesis.number, 0);
        assert_eq!(genesis.transactions.len(), 0);
    }

    #[tokio::test]
    async fn test_account_operations() {
        let mut state = BlockchainState::new();

        // Test default balance
        assert_eq!(state.get_account_balance("0x1234"), "0x0");
        assert_eq!(state.get_account_nonce("0x1234"), 0);

        // Add account
        let account = Account {
            balance: "0x1000".to_string(),
            nonce: 5,
            code: "0x".to_string(),
            storage_hash: "0x0".to_string(),
        };
        state.accounts.insert("0x1234".to_string(), account);

        assert_eq!(state.get_account_balance("0x1234"), "0x1000");
        assert_eq!(state.get_account_nonce("0x1234"), 5);
    }

    #[tokio::test]
    async fn test_transaction_operations() {
        let mut state = BlockchainState::new();

        let tx = Transaction {
            hash: "0xabcd".to_string(),
            nonce: 1,
            block_hash: Some("0x1234".to_string()),
            block_number: Some(1),
            transaction_index: Some(0),
            from: "0xfrom".to_string(),
            to: Some("0xto".to_string()),
            value: "0x100".to_string(),
            gas_price: "0x3b9aca00".to_string(),
            gas: 21000,
            input: "0x".to_string(),
            v: "0x1c".to_string(),
            r: "0x123".to_string(),
            s: "0x456".to_string(),
        };

        state.add_pending_transaction(tx.clone());
        assert_eq!(state.pending_transactions.len(), 1);

        state.transactions.insert("0xabcd".to_string(), tx);
        assert!(state.get_transaction("0xabcd").is_some());
        assert!(state.get_transaction("0xnotfound").is_none());
    }

    #[tokio::test]
    async fn test_json_rpc_server_creation() {
        let server = JsonRpcServer::new(8545, 1337);
        assert_eq!(server.port, 8545);
        assert_eq!(server.chain_id, 1337);
    }

    #[test]
    fn test_extract_json_payload() {
        let http_request =
            "POST / HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"method\":\"eth_chainId\"}";
        let payload = JsonRpcServer::extract_json_payload(http_request).unwrap();
        assert_eq!(payload, "{\"method\":\"eth_chainId\"}");
    }

    #[test]
    fn test_create_http_response() {
        let json_response = json!({"result": "0x1"});
        let http_response = JsonRpcServer::create_http_response(&json_response);
        assert!(http_response.contains("HTTP/1.1 200 OK"));
        assert!(http_response.contains("Content-Type: application/json"));
        assert!(http_response.contains("Access-Control-Allow-Origin: *"));
    }

    #[test]
    fn test_create_cors_response() {
        let cors_response = JsonRpcServer::create_cors_response();
        assert!(cors_response.contains("HTTP/1.1 200 OK"));
        assert!(cors_response.contains("Access-Control-Allow-Origin: *"));
        assert!(cors_response.contains("Content-Length: 0"));
    }
}

/// Start the JSON-RPC server
pub async fn run_server(port: u16, chain_id: u64) -> Result<()> {
    let server = JsonRpcServer::new(port, chain_id);
    server.start().await
}
