# Build Stage
FROM rust:1.75 AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential clang llvm git ca-certificates protobuf-compiler
WORKDIR /usr/src/qanto
COPY . .
RUN cargo build --release --bin qanto

# Runtime Stage
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /usr/src/qanto/target/release/qanto /app/qanto-node
COPY --from=builder /usr/src/qanto/config.toml /app/

# Expose Hugging Face Proxy Port (7860) and P2P (30303)
EXPOSE 7860 30303 9090

ENV WALLET_PASSWORD=""
ENV RUST_LOG="info"

# Start the node (wallet and P2P identity keys auto-generated headlessly if missing)
CMD ./qanto-node start --config config.toml --wallet wallet.key --rpc-port 7860 --p2p-port 30303 --listen 0.0.0.0 --bootnode --mine
