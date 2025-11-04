use async_trait::async_trait;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use libp2p::request_response::Codec;
use serde::{Deserialize, Serialize};

pub const QDS_PROTOCOL: &str = "/qanto/qds/1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QDSRequest {
    GetBalance { address: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QDSResponse {
    Balance {
        address: String,
        confirmed_balance: u64,
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
        io.read_to_end(&mut buf).await?;
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
        io.read_to_end(&mut buf).await?;
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
