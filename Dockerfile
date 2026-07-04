# Build Stage
FROM rust:1.77-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential clang llvm git ca-certificates protobuf-compiler
WORKDIR /usr/src/qanto
COPY . .
RUN cargo build --release --locked --bin qanto -j 1

# Runtime Stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /usr/src/qanto/target/release/qanto /app/qanto-node
COPY --from=builder /usr/src/qanto/config.toml /app/

# Expose HTTP API (8081), gRPC RPC (50051), and P2P (30303)
EXPOSE 8081 30303 50051

ENV WALLET_PASSWORD=""
ENV RUST_LOG="info"

# Add Healthcheck to automatically monitor node health
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
  CMD curl -f http://localhost:8081/health || exit 1

# Start the node securely binding to all interfaces for HF Space compatibility
CMD ["./qanto-node", "start", "--config", "config.toml", "--listen", "0.0.0.0", "--p2p-port", "30303", "--rpc-port", "50051", "--mine"]
