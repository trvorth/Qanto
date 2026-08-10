//! Qanto RPC dispatcher adapter - submits claims to Qanto chain.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::error::{RelayerError, RelayerResult};
use crate::ports::{BridgeClaimRequest, ClaimDispatcher};

/// Adapter that dispatches bridge claims to Qanto via JSON-RPC.
pub struct QantoRpcDispatcher {
    client: Client,
    rpc_url: String,
}

impl QantoRpcDispatcher {
    pub fn new(client: Client, rpc_url: String) -> Self {
        Self { client, rpc_url }
    }
}

#[async_trait]
impl ClaimDispatcher for QantoRpcDispatcher {
    async fn dispatch(&self, claim: &BridgeClaimRequest) -> RelayerResult<String> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "qanto_bridgeMint",
            "params": [claim],
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| RelayerError::SourceChain(format!("HTTP error: {}", e)))?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|e| RelayerError::SourceChain(format!("JSON decode: {}", e)))?;

        if !status.is_success() {
            return Err(RelayerError::SourceChain(format!(
                "RPC HTTP error: {}",
                status
            )));
        }

        if let Some(error) = body.get("error") {
            let msg = error.to_string();
            return Err(RelayerError::SourceChain(format!(
                "Qanto RPC error: {}",
                msg
            )));
        }

        body.get("result")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                RelayerError::SourceChain(format!("missing result in response: {}", body))
            })
    }
}
