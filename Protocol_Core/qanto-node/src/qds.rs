use crate::qantodag::QantoBlock;
use async_trait::async_trait;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use libp2p::request_response::Codec;
use serde::{Deserialize, Serialize};

pub const QDS_PROTOCOL: &str = "/qanto/qds/1";
pub const MAX_QDS_PAYLOAD_SIZE: usize = 4 * 1024 * 1024; // 4MB limit to prevent OOM (D-C1)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QDSRequest {
    GetBalance { address: String },
    PushBlock { block: Box<QantoBlock> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QDSResponse {
    Balance {
        address: String,
        confirmed_balance: u64,
    },
    Ack {
        accepted: bool,
        detail: String,
    },
}

#[derive(Clone, Default)]
pub struct QDSCodec;

#[async_trait]
impl Codec for QDSCodec {
    type Protocol = String;
    type Request = QDSRequest;
    type Response = QDSResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: futures::io::AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        // Limit reading to MAX_QDS_PAYLOAD_SIZE to prevent memory bombs (D-C1)
        io.take(MAX_QDS_PAYLOAD_SIZE as u64)
            .read_to_end(&mut buf)
            .await?;
        serde_json::from_slice(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::io::AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        // Limit reading to MAX_QDS_PAYLOAD_SIZE to prevent memory bombs (D-C1)
        io.take(MAX_QDS_PAYLOAD_SIZE as u64)
            .read_to_end(&mut buf)
            .await?;
        serde_json::from_slice(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: futures::io::AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&req).map_err(std::io::Error::other)?;
        io.write_all(&data).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: futures::io::AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&res).map_err(std::io::Error::other)?;
        io.write_all(&data).await
    }
}
