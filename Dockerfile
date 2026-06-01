# Build Stage
FROM rust:latest AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential clang llvm git ca-certificates protobuf-compiler
WORKDIR /usr/src/qanto
COPY . .
RUN cargo build --release --bin qanto -j 1

# Runtime Stage
FROM rust:latest
RUN apt-get update && apt-get install -y libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /usr/src/qanto/target/release/qanto /app/qanto-node
COPY --from=builder /usr/src/qanto/config.toml /app/

# Expose Hugging Face Proxy Port (7860) and P2P (30303)
EXPOSE 7860 30303

ENV WALLET_PASSWORD=""
ENV RUST_LOG="info"

# Start the node securely binding to all interfaces for HF Space compatibility
CMD ["./qanto-node", "start", "--config", "config.toml", "--listen", "0.0.0.0", "--p2p-port", "30303", "--rpc-port", "7860", "--mine"]
