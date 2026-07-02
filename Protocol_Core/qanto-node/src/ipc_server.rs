use crate::{config::Config, qantodag::QantoDAG};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, error, info, warn};

const IPC_HTTP_ROUTE: &str = "/balance";
const IPC_SOCKET_FILE: &str = "qanto.ipc.sock";
const IPC_TCP_PORT_OFFSET: u16 = 10_000;
const IPC_TCP_DEFAULT_PORT: u16 = 39_999;
const MAX_HTTP_HEADER_BYTES: usize = 8 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpcEndpoint {
    #[cfg(unix)]
    Unix(PathBuf),
    Tcp(SocketAddr),
}

impl IpcEndpoint {
    pub fn display(&self) -> String {
        match self {
            #[cfg(unix)]
            Self::Unix(path) => path.display().to_string(),
            Self::Tcp(addr) => addr.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BalanceRequest {
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IpcResponse {
    pub balance: u128,
    pub height: u64,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

pub fn ipc_unix_socket_path(config: &Config) -> PathBuf {
    Path::new(&config.data_dir).join(IPC_SOCKET_FILE)
}

pub fn ipc_tcp_fallback_addr(config: &Config) -> SocketAddr {
    let api_addr = config.api_address.parse::<SocketAddr>().ok();
    let api_port = api_addr.map(|addr| addr.port()).unwrap_or(8_080);
    let fallback_port = api_port
        .checked_add(IPC_TCP_PORT_OFFSET)
        .unwrap_or(IPC_TCP_DEFAULT_PORT);

    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), fallback_port)
}

pub fn preferred_ipc_endpoint(config: &Config) -> IpcEndpoint {
    #[cfg(unix)]
    {
        IpcEndpoint::Unix(ipc_unix_socket_path(config))
    }
    #[cfg(not(unix))]
    {
        IpcEndpoint::Tcp(ipc_tcp_fallback_addr(config))
    }
}

pub fn fallback_ipc_endpoint(config: &Config) -> IpcEndpoint {
    IpcEndpoint::Tcp(ipc_tcp_fallback_addr(config))
}

pub fn build_balance_http_request(address: &str) -> Result<Vec<u8>, serde_json::Error> {
    let body = serde_json::to_vec(&BalanceRequest {
        address: address.to_string(),
    })?;
    let header = format!(
        "POST {IPC_HTTP_ROUTE} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );

    let mut request = header.into_bytes();
    request.extend_from_slice(&body);
    Ok(request)
}

pub async fn read_http_response_json<S>(stream: &mut S) -> std::io::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let mut header_end = None;

    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(pos) = find_header_end(&buf) {
            header_end = Some(pos);
            break;
        }
        if buf.len() > MAX_HTTP_HEADER_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "IPC response headers exceeded limit",
            ));
        }
    }

    let header_end = header_end.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "IPC response missing HTTP headers",
        )
    })?;
    let header = std::str::from_utf8(&buf[..header_end]).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC response headers are not valid UTF-8",
        )
    })?;
    let content_length = parse_content_length(header)?;

    if content_length > MAX_HTTP_BODY_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC response body exceeded limit",
        ));
    }

    let body_start = header_end + 4;
    while buf.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
    }

    if buf.len().saturating_sub(body_start) < content_length {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "IPC response body truncated",
        ));
    }

    Ok(buf[body_start..body_start + content_length].to_vec())
}

pub struct IpcServer;

impl IpcServer {
    pub async fn start(dag: Arc<QantoDAG>, config: Config) {
        #[cfg(unix)]
        {
            let socket_path = ipc_unix_socket_path(&config);
            if let Some(parent) = socket_path.parent() {
                if let Err(err) = tokio::fs::create_dir_all(parent).await {
                    error!(
                        "Failed to create IPC socket directory {}: {}. Falling back to TCP.",
                        parent.display(),
                        err
                    );
                    Self::start_tcp(dag, &config).await;
                    return;
                }
            }

            let _ = std::fs::remove_file(&socket_path);
            match tokio::net::UnixListener::bind(&socket_path) {
                Ok(listener) => {
                    info!(
                        "IPC server listening on Unix socket {}",
                        socket_path.display()
                    );
                    loop {
                        match listener.accept().await {
                            Ok((stream, _)) => {
                                let dag_clone = dag.clone();
                                let endpoint = socket_path.clone();
                                tokio::spawn(async move {
                                    Self::handle_connection(
                                        stream,
                                        dag_clone,
                                        endpoint.display().to_string(),
                                    )
                                    .await;
                                });
                            }
                            Err(err) => warn!("IPC Unix accept error: {}", err),
                        }
                    }
                }
                Err(err) => {
                    error!(
                        "Failed to bind IPC Unix socket {}: {}. Falling back to TCP.",
                        socket_path.display(),
                        err
                    );
                    Self::start_tcp(dag, &config).await;
                }
            }
        }

        #[cfg(not(unix))]
        {
            Self::start_tcp(dag, &config).await;
        }
    }

    async fn start_tcp(dag: Arc<QantoDAG>, config: &Config) {
        let bind_addr = ipc_tcp_fallback_addr(config);
        match tokio::net::TcpListener::bind(bind_addr).await {
            Ok(listener) => {
                info!("IPC server listening on TCP {}", bind_addr);
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let dag_clone = dag.clone();
                            tokio::spawn(async move {
                                Self::handle_connection(stream, dag_clone, bind_addr.to_string())
                                    .await;
                            });
                        }
                        Err(err) => warn!("IPC TCP accept error: {}", err),
                    }
                }
            }
            Err(err) => error!(
                "Failed to bind fallback TCP IPC server {}: {}",
                bind_addr, err
            ),
        }
    }

    async fn handle_connection<S>(mut stream: S, dag: Arc<QantoDAG>, endpoint: String)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        match read_http_request(&mut stream).await {
            Ok(request) => {
                debug!(
                    method = %request.method,
                    path = %request.path,
                    endpoint = %endpoint,
                    "IPC request received"
                );

                if request.method != "POST" {
                    let _ = write_http_json_response(
                        &mut stream,
                        405,
                        "Method Not Allowed",
                        &serde_json::json!({ "error": "method_not_allowed" }),
                    )
                    .await;
                    return;
                }

                if request.path != IPC_HTTP_ROUTE {
                    let _ = write_http_json_response(
                        &mut stream,
                        404,
                        "Not Found",
                        &serde_json::json!({ "error": "not_found" }),
                    )
                    .await;
                    return;
                }

                match serde_json::from_slice::<BalanceRequest>(&request.body) {
                    Ok(req) => {
                        let balance = dag
                            .account_state_cache
                            .get_balance(&req.address)
                            .unwrap_or(0);
                        let height = dag.get_block_count().await;
                        let response = IpcResponse { balance, height };
                        info!(
                            "IPC_QUERY endpoint=/balance transport={} address={} balance={} height={}",
                            endpoint, req.address, balance, height
                        );
                        if let Err(err) =
                            write_http_json_response(&mut stream, 200, "OK", &response).await
                        {
                            error!("Failed to write IPC response on {}: {}", endpoint, err);
                        }
                    }
                    Err(err) => {
                        let _ = write_http_json_response(
                            &mut stream,
                            400,
                            "Bad Request",
                            &serde_json::json!({ "error": "invalid_request", "details": err.to_string() }),
                        )
                        .await;
                    }
                }
            }
            Err(err) => {
                error!("IPC request read failed on {}: {}", endpoint, err);
            }
        }
    }
}

async fn read_http_request<S>(stream: &mut S) -> std::io::Result<HttpRequest>
where
    S: AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let mut header_end = None;

    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(pos) = find_header_end(&buf) {
            header_end = Some(pos);
            break;
        }
        if buf.len() > MAX_HTTP_HEADER_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "IPC request headers exceeded limit",
            ));
        }
    }

    let header_end = header_end.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "IPC request missing HTTP headers",
        )
    })?;
    let header = std::str::from_utf8(&buf[..header_end]).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC request headers are not valid UTF-8",
        )
    })?;
    let mut lines = header.split("\r\n");
    let request_line = lines.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC request missing request line",
        )
    })?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();
    let content_length = parse_content_length(header)?;

    if content_length > MAX_HTTP_BODY_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC request body exceeded limit",
        ));
    }

    let body_start = header_end + 4;
    while buf.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
    }

    if buf.len().saturating_sub(body_start) < content_length {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "IPC request body truncated",
        ));
    }

    Ok(HttpRequest {
        method,
        path,
        body: buf[body_start..body_start + content_length].to_vec(),
    })
}

async fn write_http_json_response<S, T>(
    stream: &mut S,
    status_code: u16,
    status_text: &str,
    payload: &T,
) -> std::io::Result<()>
where
    S: AsyncWrite + Unpin,
    T: Serialize,
{
    let body = serde_json::to_vec(payload).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to encode IPC JSON response: {err}"),
        )
    })?;
    let header = format!(
        "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.flush().await
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> std::io::Result<usize> {
    let mut content_length = 0usize;
    for line in headers.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "IPC content-length header is invalid",
                    )
                })?;
                break;
            }
        }
    }
    Ok(content_length)
}
