use crate::qantodag::QantoBlock;
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::request_response::Codec;
use serde::{Deserialize, Serialize};
use std::io;

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
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.take(MAX_QDS_PAYLOAD_SIZE as u64)
            .read_to_end(&mut buf)
            .await?;
        serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.take(MAX_QDS_PAYLOAD_SIZE as u64)
            .read_to_end(&mut buf)
            .await?;
        serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&req).map_err(io::Error::other)?;
        io.write_all(&data).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&res).map_err(io::Error::other)?;
        io.write_all(&data).await
    }
}
