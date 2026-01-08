use crate::rpc_backend::NodeRpcBackend;
use axum::{extract::State, response::IntoResponse, Json};
use crate::p2p::P2PCommand;
use qanto_rpc::RpcBackend;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Vec<Value>>,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

pub async fn handle_json_rpc(
    State(backend): State<Arc<NodeRpcBackend>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let id = request.id.clone();

    let result = match request.method.as_str() {
        "get_balance" => {
            if let Some(params) = request.params {
                if let Some(addr_val) = params.first() {
                    if let Some(address) = addr_val.as_str() {
                        match backend.get_balance(address.to_string()).await {
                            Ok(balance) => Ok(serde_json::to_value(balance).unwrap()),
                            Err(e) => Err(JsonRpcError {
                                code: -32000,
                                message: e,
                            }),
                        }
                    } else {
                        Err(JsonRpcError {
                            code: -32602,
                            message: "Invalid params: expected string address".to_string(),
                        })
                    }
                } else {
                    Err(JsonRpcError {
                        code: -32602,
                        message: "Invalid params: missing address".to_string(),
                    })
                }
            } else {
                Err(JsonRpcError {
                    code: -32602,
                    message: "Invalid params".to_string(),
                })
            }
        }
        "p2p_getConnectedPeers" => {
            let (response_sender, response_receiver) = oneshot::channel();
            match backend
                .p2p_sender
                .send(P2PCommand::GetConnectedPeers { response_sender })
                .await
            {
                Ok(()) => match response_receiver.await {
                    Ok(peers) => Ok(serde_json::to_value(peers).unwrap()),
                    Err(e) => Err(JsonRpcError {
                        code: -32000,
                        message: format!("Failed to receive peers list: {e}"),
                    }),
                },
                Err(e) => Err(JsonRpcError {
                    code: -32000,
                    message: format!("Failed to send GetConnectedPeers: {e}"),
                }),
            }
        }
        // Add other methods as needed
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Method not found: {}", request.method),
        }),
    };

    let response = match result {
        Ok(res) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(res),
            error: None,
            id,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(err),
            id,
        },
    };

    Json(response)
}
