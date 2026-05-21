FROM rust:1.75 AS builder

# Install build dependencies, including git and ca-certificates for git cargo dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential clang llvm git ca-certificates

WORKDIR /usr/src/qanto
# Copy the entire workspace
COPY . .

# Build the qanto bin in release mode
RUN cargo build --release --bin qanto

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/qanto/target/release/qanto /app/qanto-node
COPY --from=builder /usr/src/qanto/config.toml /app/config.toml

ENV WALLET_PASSWORD=""
ENV RUST_LOG="info"

EXPOSE 7860
EXPOSE 30303

# Generate a default wallet on first run if none exists, then start node using rpc port 7860
CMD if [ ! -f wallet.key ]; then ./qanto-node generate-wallet --output wallet.key; fi && ./qanto-node start --config config.toml --wallet wallet.key --rpc-port 7860
