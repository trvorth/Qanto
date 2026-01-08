FROM rust:latest AS builder

WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    build-essential \
    clang \
    cmake \
    libssl-dev \
 && rm -rf /var/lib/apt/lists/*

COPY . /build
RUN cargo build --release --bin qanto

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
 && rm -rf /var/lib/apt/lists/* && update-ca-certificates

COPY --from=builder /build/target/release/qanto /usr/local/bin/qanto
COPY config.toml /app/config.toml
RUN chmod +x /usr/local/bin/qanto && mkdir -p /root/.qanto /app

ENV RUST_LOG=info
VOLUME ["/root/.qanto"]
EXPOSE 30333 8080 9944

HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -fsS http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/qanto", "start", "--config", "/app/config.toml"]
