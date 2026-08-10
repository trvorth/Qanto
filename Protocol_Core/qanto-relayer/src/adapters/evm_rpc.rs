//! EVM JSON-RPC gateway adapter.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{RelayerError, RelayerResult};
use crate::ports::types::EthLog as PortEthLog;
use crate::ports::{AssetLockedEvent, BlockHeader, SourceChainGateway, TransactionReceipt};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthBlockHeader {
    hash: Option<String>,
    number: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthTransactionReceipt {
    status: Option<String>,
    logs: Vec<EthLog>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EthLog {
    address: String,
    topics: Vec<String>,
    data: String,
    block_hash: Option<String>,
    block_number: Option<String>,
    transaction_hash: Option<String>,
    log_index: Option<String>,
    #[serde(default)]
    removed: bool,
}

pub struct EvmRpcGateway {
    client: Client,
    rpc_url: String,
    bridge_address: String,
}

impl EvmRpcGateway {
    pub fn new(client: Client, rpc_url: String, bridge_address: String) -> Self {
        Self {
            client,
            rpc_url,
            bridge_address,
        }
    }

    fn asset_locked_topic() -> String {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(b"AssetLocked(address,address,uint256)");
        format!("0x{}", hex::encode(hasher.finalize()))
    }

    async fn rpc_call<T>(&self, method: &str, params: Value) -> RelayerResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| RelayerError::SourceChain(e.to_string()))?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|e| RelayerError::SourceChain(e.to_string()))?;

        if !status.is_success() {
            return Err(RelayerError::SourceChain(format!("HTTP {}", status)));
        }

        if let Some(err) = body.get("error") {
            let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(RelayerError::SourceChain(format!("RPC {}: {}", code, msg)));
        }

        let result = body
            .get("result")
            .cloned()
            .ok_or_else(|| RelayerError::SourceChain("missing result".into()))?;

        serde_json::from_value(result)
            .map_err(|e| RelayerError::SourceChain(format!("decode error: {}", e)))
    }

    fn strip_hex(s: &str) -> &str {
        s.strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s)
    }

    fn parse_hex_u64(s: &str) -> RelayerResult<u64> {
        u64::from_str_radix(Self::strip_hex(s), 16)
            .map_err(|e| RelayerError::SourceChain(format!("parse u64: {}", e)))
    }

    fn parse_uint256_u128(data: &str) -> RelayerResult<u128> {
        let bytes = hex::decode(Self::strip_hex(data))
            .map_err(|e| RelayerError::SourceChain(format!("hex decode: {}", e)))?;
        if bytes.len() != 32 {
            return Err(RelayerError::SourceChain(format!(
                "expected 32-byte uint256, got {}",
                bytes.len()
            )));
        }
        if bytes[..16].iter().any(|b| *b != 0) {
            return Err(RelayerError::SourceChain("amount exceeds u128".into()));
        }
        let mut amount_bytes = [0u8; 16];
        amount_bytes.copy_from_slice(&bytes[16..]);
        Ok(u128::from_be_bytes(amount_bytes))
    }

    fn decode_topic_address(topic: &str) -> RelayerResult<String> {
        let bytes = hex::decode(Self::strip_hex(topic))
            .map_err(|e| RelayerError::SourceChain(format!("topic decode: {}", e)))?;
        if bytes.len() != 32 {
            return Err(RelayerError::SourceChain(format!(
                "topic must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        Ok(format!("0x{}", hex::encode(&bytes[12..])))
    }

    fn normalize_hex(s: &str) -> RelayerResult<String> {
        let stripped = Self::strip_hex(s);
        if stripped.is_empty()
            || !stripped.len().is_multiple_of(2)
            || !stripped.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(RelayerError::SourceChain(format!("invalid hex: {}", s)));
        }
        Ok(format!("0x{}", stripped.to_lowercase()))
    }

    fn normalize_address(addr: &str) -> RelayerResult<String> {
        let stripped = Self::strip_hex(addr);
        if stripped.len() != 40 || !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(RelayerError::SourceChain(format!(
                "invalid address: {}",
                addr
            )));
        }
        Ok(format!("0x{}", stripped.to_lowercase()))
    }
}

#[async_trait]
impl SourceChainGateway for EvmRpcGateway {
    async fn latest_block_number(&self) -> RelayerResult<u64> {
        let hex_block: String = self.rpc_call("eth_blockNumber", json!([])).await?;
        Self::parse_hex_u64(&hex_block)
    }

    async fn fetch_locked_events(
        &self,
        bridge_address: &str,
        from_block: u64,
        to_block: u64,
    ) -> RelayerResult<Vec<AssetLockedEvent>> {
        let logs: Vec<EthLog> = self
            .rpc_call(
                "eth_getLogs",
                json!([{
                    "address": bridge_address,
                    "fromBlock": format!("0x{:x}", from_block),
                    "toBlock": format!("0x{:x}", to_block),
                    "topics": [Self::asset_locked_topic()],
                }]),
            )
            .await?;

        let mut events = Vec::new();
        for log in logs {
            if log.removed {
                continue;
            }

            let receipt = self
                .fetch_receipt(
                    log.transaction_hash
                        .as_deref()
                        .ok_or_else(|| RelayerError::SourceChain("missing tx hash".into()))?,
                )
                .await?;

            let block = self
                .fetch_block_header(
                    log.block_hash
                        .as_deref()
                        .ok_or_else(|| RelayerError::SourceChain("missing block hash".into()))?,
                )
                .await?;

            if let Ok(event) = self.decode_log(&log, &receipt, &block) {
                events.push(event);
            }
        }

        Ok(events)
    }

    async fn fetch_receipt(&self, tx_hash: &str) -> RelayerResult<TransactionReceipt> {
        let receipt: EthTransactionReceipt = self
            .rpc_call("eth_getTransactionReceipt", json!([tx_hash]))
            .await?;

        let logs = receipt
            .logs
            .into_iter()
            .map(|l| PortEthLog {
                address: l.address,
                topics: l.topics,
                data: l.data,
                block_hash: l.block_hash,
                block_number: l.block_number,
                transaction_hash: l.transaction_hash,
                log_index: l.log_index,
                removed: l.removed,
            })
            .collect();

        Ok(TransactionReceipt {
            status: receipt.status.unwrap_or("0x0".into()),
            logs,
        })
    }

    async fn fetch_block_header(&self, block_hash: &str) -> RelayerResult<BlockHeader> {
        let block: EthBlockHeader = self
            .rpc_call("eth_getBlockByHash", json!([block_hash, false]))
            .await?;
        Ok(BlockHeader {
            hash: Self::normalize_hex(block.hash.as_deref().unwrap_or_default())?,
            number: Self::parse_hex_u64(block.number.as_deref().unwrap_or("0x0"))?,
        })
    }
}

impl EvmRpcGateway {
    fn decode_log(
        &self,
        log: &EthLog,
        receipt: &TransactionReceipt,
        block: &BlockHeader,
    ) -> RelayerResult<AssetLockedEvent> {
        // Verify log address matches bridge
        if Self::normalize_address(&log.address)? != self.bridge_address {
            return Err(RelayerError::SourceChain("bridge address mismatch".into()));
        }

        if log.topics.len() != 3 {
            return Err(RelayerError::SourceChain(format!(
                "expected 3 topics, got {}",
                log.topics.len()
            )));
        }

        let user = Self::decode_topic_address(&log.topics[1])?;
        let token = Self::decode_topic_address(&log.topics[2])?;
        let amount = Self::parse_uint256_u128(&log.data)?;

        let transaction_hash = Self::normalize_hex(
            log.transaction_hash
                .as_deref()
                .ok_or_else(|| RelayerError::SourceChain("missing tx hash".into()))?,
        )?;

        let block_hash = Self::normalize_hex(
            log.block_hash
                .as_deref()
                .ok_or_else(|| RelayerError::SourceChain("missing block hash".into()))?,
        )?;

        let block_number = Self::parse_hex_u64(
            log.block_number
                .as_deref()
                .ok_or_else(|| RelayerError::SourceChain("missing block number".into()))?,
        )?;

        let log_index = Self::parse_hex_u64(
            log.log_index
                .as_deref()
                .ok_or_else(|| RelayerError::SourceChain("missing log index".into()))?,
        )?;

        // Verify receipt status
        if receipt.status != "0x1" {
            return Err(RelayerError::SourceChain(format!(
                "tx failed: {}",
                receipt.status
            )));
        }

        // Verify receipt contains this log
        let log_in_receipt = receipt.logs.iter().any(|l| {
            l.transaction_hash.as_deref().is_some_and(|h| {
                Self::strip_hex(h).eq_ignore_ascii_case(Self::strip_hex(&transaction_hash))
            }) && l.log_index.as_deref().is_some_and(|i| {
                Self::parse_hex_u64(i)
                    .map(|idx| idx == log_index)
                    .unwrap_or(false)
            })
        });

        if !log_in_receipt {
            return Err(RelayerError::SourceChain("log not in receipt".into()));
        }

        // Verify block header matches
        if block.hash != block_hash || block.number != block_number {
            return Err(RelayerError::SourceChain("block header mismatch".into()));
        }

        Ok(AssetLockedEvent {
            user,
            token,
            amount,
            transaction_hash,
            block_hash,
            block_number,
            log_index,
        })
    }
}
