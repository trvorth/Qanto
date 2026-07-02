# syntax=docker/dockerfile:1.7
ARG RUST_VERSION=1.75
ARG DEBIAN_BASE=debian:bookworm-slim

FROM rust:${RUST_VERSION} AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    clang \
    cmake \
    git \
    libssl-dev \
    pkg-config \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

COPY . .

RUN cargo build --release --locked -p qanto --bin qanto --bin qantowallet --bin generate_wallet --bin import_wallet

FROM ${DEBIAN_BASE} AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libssl3 \
    tini \
    tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

RUN groupadd -g 10001 qanto && \
    useradd -m -u 10001 -g 10001 -s /bin/bash qanto

WORKDIR /opt/qanto
RUN mkdir -p config keys data logs && \
    chown -R qanto:qanto /opt/qanto && \
    chmod 755 /opt/qanto && \
    chmod 700 /opt/qanto/keys

COPY --from=builder /build/target/release/qanto /usr/local/bin/qanto
COPY --from=builder /build/target/release/qantowallet /usr/local/bin/qantowallet
COPY --from=builder /build/target/release/generate_wallet /usr/local/bin/generate_wallet
COPY --from=builder /build/target/release/import_wallet /usr/local/bin/import_wallet

COPY --chown=qanto:qanto <<'ENTRYPOINT' /usr/local/bin/entrypoint.sh
#!/usr/bin/env bash
set -euo pipefail

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*" >&2
}

CONFIG_PATH="${CONFIG_PATH:-/opt/qanto/config/config.toml}"
WALLET_PATH="${WALLET_PATH:-/opt/qanto/keys/wallet.key}"
P2P_IDENTITY="${P2P_IDENTITY:-/opt/qanto/keys/p2p_identity.key}"
PEER_CACHE="${PEER_CACHE:-/opt/qanto/keys/peer_cache.json}"
LOG_DIR="${LOG_DIR:-/opt/qanto/logs}"
DATA_DIR="${DATA_DIR:-/opt/qanto/data}"

if [ ! -f "$CONFIG_PATH" ]; then
    log "ERROR: Configuration file not found: $CONFIG_PATH"
    exit 1
fi

mkdir -p "$(dirname "$CONFIG_PATH")" "$(dirname "$WALLET_PATH")" "$LOG_DIR" "$DATA_DIR"
chmod 700 /opt/qanto/keys

if [ -n "${WALLET_PASSWORD_FILE:-}" ] && [ -f "$WALLET_PASSWORD_FILE" ]; then
    export WALLET_PASSWORD
    WALLET_PASSWORD="$(cat "$WALLET_PASSWORD_FILE")"
    log "Wallet password loaded from file"
elif [ -n "${WALLET_PASSWORD:-}" ]; then
    export WALLET_PASSWORD
    log "Wallet password loaded from environment"
else
    log "ERROR: Provide WALLET_PASSWORD or WALLET_PASSWORD_FILE"
    exit 1
fi

if [ ! -f "$PEER_CACHE" ]; then
    printf '{}' > "$PEER_CACHE"
    chmod 600 "$PEER_CACHE"
fi

if [ ! -f "$WALLET_PATH" ]; then
    log "Wallet not found, generating a new encrypted wallet"
    generate_wallet --output "$WALLET_PATH"
    chmod 600 "$WALLET_PATH"
fi

if [ ! -f "$P2P_IDENTITY" ]; then
    log "P2P identity file missing at $P2P_IDENTITY; qanto will create it on first start if supported by config"
fi

exec qanto start \
    --config "$CONFIG_PATH" \
    --wallet "$WALLET_PATH" \
    --p2p-identity "$P2P_IDENTITY" \
    --peer-cache "$PEER_CACHE"
ENTRYPOINT

RUN chmod +x /usr/local/bin/entrypoint.sh && \
    chown qanto:qanto /usr/local/bin/entrypoint.sh

COPY --chown=qanto:qanto <<'HEALTHCHECK' /usr/local/bin/healthcheck.sh
#!/usr/bin/env bash
set -euo pipefail

HEALTHCHECK_URL="${HEALTHCHECK_URL:-http://localhost:8081/health}"
curl -fsS --max-time 10 "$HEALTHCHECK_URL" >/dev/null
HEALTHCHECK

RUN chmod +x /usr/local/bin/healthcheck.sh && \
    chown qanto:qanto /usr/local/bin/healthcheck.sh

HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/usr/local/bin/healthcheck.sh"]

USER qanto
WORKDIR /opt/qanto

EXPOSE 8081 30303 50051

LABEL maintainer="Qanto Development Team" \
      version="${VERSION:-latest}" \
      description="Qanto blockchain node container image" \
      org.opencontainers.image.title="Qanto Node" \
      org.opencontainers.image.description="Qanto blockchain node" \
      org.opencontainers.image.vendor="Qanto" \
      org.opencontainers.image.licenses="MIT"

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD []
