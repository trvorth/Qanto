FROM rust:1.77-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake build-essential clang llvm protobuf-compiler

WORKDIR /usr/src/qanto
# Copy the entire workspace
COPY . .

# Build the qanto and qantowallet bins in release mode with pinned dependencies
RUN cargo build --release --locked --bin qanto
RUN cargo build --release --locked --bin qantowallet

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/qanto/target/release/qanto /app/
COPY --from=builder /usr/src/qanto/target/release/qantowallet /app/
# Copy default config template if we have one
COPY --from=builder /usr/src/qanto/Protocol_Core/qanto-node/config.toml /app/config.toml || echo "No config.toml found, will use args" > /dev/null

ENV WALLET_PASSWORD=""
ENV RUST_LOG="info"

EXPOSE 8081
EXPOSE 50051
EXPOSE 30303

# Add Healthcheck to automatically monitor node health
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
  CMD curl -f http://localhost:8081/health || exit 1

# Generate a default wallet on first run if none exists, then start node
CMD ./qanto generate-wallet --output wallet.key && ./qanto start --wallet wallet.key --mine
