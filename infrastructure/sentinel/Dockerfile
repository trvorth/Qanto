# QANTO Sentinel Node Dockerfile
# v1.0.0 - Phase 39: Resilience Wave

# 1. Build Stage
FROM rust:nightly-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build the QANTO core binary (Assumed 'qanto-node')
RUN cargo build --release --bin qanto-node

# 2. Runtime Stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /root/
COPY --from=builder /app/target/release/qanto-node /usr/local/bin/qanto-node

EXPOSE 30303 30303/udp 8545

ENTRYPOINT ["qanto-node"]
CMD ["--network", "mainnet"]
