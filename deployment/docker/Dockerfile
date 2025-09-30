# syntax=docker/dockerfile:1.7
ARG RUST_VERSION=1.75
ARG DEBIAN_BASE=debian:bookworm-slim

# ===== Builder Stage =====
FROM rust:${RUST_VERSION} as builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    git \
    clang \
    make \
    gcc \
    libc6-dev \
    libgmp-dev \
    && rm -rf /var/lib/apt/lists/*

# Set up build environment
WORKDIR /build
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source to build dependencies
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release --features infinite-strata && rm -rf src

# Copy actual source code
COPY src ./src
COPY myblockchain ./myblockchain

# Build with production optimizations
ENV RUSTFLAGS="-C target-cpu=native -C codegen-units=1 -C opt-level=3 -C panic=abort -C strip=symbols"
RUN cargo build --release --features infinite-strata --locked

# Verify binaries
RUN ldd target/release/qanto && \
    ./target/release/qanto --version

# ===== Runtime Stage =====
FROM ${DEBIAN_BASE} as runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    libssl3 \
    expect \
    curl \
    tini \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user with specific UID/GID
RUN groupadd -g 10001 qanto && \
    useradd -m -u 10001 -g 10001 -s /bin/bash qanto

# Set up directory structure
WORKDIR /opt/qanto
RUN mkdir -p config keys data logs certs temp && \
    mkdir -p data/{blockchain,state,cache} && \
    mkdir -p logs/{node,p2p,consensus,mining,api} && \
    chown -R qanto:qanto /opt/qanto && \
    chmod 755 /opt/qanto && \
    chmod 700 /opt/qanto/keys

# Copy binaries from builder
COPY --from=builder /build/target/release/qanto /usr/local/bin/qanto
COPY --from=builder /build/target/release/qantowallet /usr/local/bin/qantowallet
COPY --from=builder /build/target/release/generate_wallet /usr/local/bin/generate_wallet
COPY --from=builder /build/target/release/import_wallet /usr/local/bin/import_wallet

# Create entrypoint script
COPY --chown=qanto:qanto <<'ENTRYPOINT' /usr/local/bin/entrypoint.sh
#!/usr/bin/env bash
set -euo pipefail

# Logging function
log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*" >&2
}

log "Starting Qanto node entrypoint..."

# Environment variables with defaults
CONFIG_PATH="${CONFIG_PATH:-/opt/qanto/config/config.toml}"
WALLET_PATH="${WALLET_PATH:-/opt/qanto/keys/wallet.key}"
P2P_IDENTITY="${P2P_IDENTITY:-/opt/qanto/keys/p2p_identity.key}"
PEER_CACHE="${PEER_CACHE:-/opt/qanto/keys/peer_cache.json}"
LOG_DIR="${LOG_DIR:-/opt/qanto/logs}"
DATA_DIR="${DATA_DIR:-/opt/qanto/data}"

# Validate required files
if [ ! -f "$CONFIG_PATH" ]; then
    log "ERROR: Configuration file not found: $CONFIG_PATH"
    exit 1
fi

# Create directories if they don't exist
mkdir -p "$(dirname "$CONFIG_PATH")" "$(dirname "$WALLET_PATH")" "$LOG_DIR" "$DATA_DIR"
mkdir -p "$LOG_DIR"/{node,p2p,consensus,mining,api}
mkdir -p "$DATA_DIR"/{blockchain,state,cache}

# Set proper permissions
chmod 700 /opt/qanto/keys
chmod 755 /opt/qanto/logs /opt/qanto/data

# Initialize peer cache if it doesn't exist
if [ ! -f "$PEER_CACHE" ]; then
    log "Initializing peer cache: $PEER_CACHE"
    echo '{}' > "$PEER_CACHE"
    chmod 600 "$PEER_CACHE"
fi

# Generate wallet if it doesn't exist
if [ ! -f "$WALLET_PATH" ]; then
    log "Wallet not found, generating new wallet..."
    if [ -n "${WALLET_PASSWORD_FILE:-}" ] && [ -f "$WALLET_PASSWORD_FILE" ]; then
        PASSWORD=$(cat "$WALLET_PASSWORD_FILE")
    elif [ -n "${WALLET_PASSWORD:-}" ]; then
        PASSWORD="$WALLET_PASSWORD"
    else
        log "ERROR: No wallet password provided for wallet generation"
        exit 1
    fi
    
    # Generate wallet with password
    echo "$PASSWORD" | generate_wallet --output "$WALLET_PATH" --stdin-password
    chmod 600 "$WALLET_PATH"
    log "New wallet generated: $WALLET_PATH"
fi

# Get wallet password from file or environment
PASSWORD=""
if [ -n "${WALLET_PASSWORD_FILE:-}" ] && [ -f "$WALLET_PASSWORD_FILE" ]; then
    PASSWORD=$(cat "$WALLET_PASSWORD_FILE")
    log "Wallet password loaded from file"
elif [ -n "${WALLET_PASSWORD:-}" ]; then
    PASSWORD="$WALLET_PASSWORD"
    log "Wallet password loaded from environment"
else
    log "ERROR: No wallet password provided via WALLET_PASSWORD or WALLET_PASSWORD_FILE"
    exit 1
fi

# Validate wallet password
if [ -z "$PASSWORD" ]; then
    log "ERROR: Wallet password is empty"
    exit 1
fi

# Health check function
health_check() {
    local max_attempts=30
    local attempt=1
    
    log "Starting health check..."
    while [ $attempt -le $max_attempts ]; do
        if curl -f -s http://localhost:8081/health > /dev/null 2>&1; then
            log "Health check passed on attempt $attempt"
            return 0
        fi
        log "Health check attempt $attempt/$max_attempts failed, retrying in 10s..."
        sleep 10
        attempt=$((attempt + 1))
    done
    
    log "ERROR: Health check failed after $max_attempts attempts"
    return 1
}

# Signal handlers for graceful shutdown
shutdown() {
    log "Received shutdown signal, stopping Qanto node..."
    if [ -n "${QANTO_PID:-}" ]; then
        kill -TERM "$QANTO_PID" 2>/dev/null || true
        wait "$QANTO_PID" 2>/dev/null || true
    fi
    log "Qanto node stopped"
    exit 0
}

trap shutdown SIGTERM SIGINT

# Use expect to provide password non-interactively
log "Creating expect script for password automation..."
cat > /tmp/start_qanto.exp <<'EOF'
#!/usr/bin/expect -f
set timeout 30
set password [lindex $argv 0]
set config [lindex $argv 1]
set wallet [lindex $argv 2]
set p2p [lindex $argv 3]
set cache [lindex $argv 4]

log_user 1
spawn qanto start --config $config --wallet $wallet --p2p-identity $p2p --peer-cache $cache

expect {
    "Enter wallet password:" {
        send -- "$password\r"
        exp_continue
    }
    "Node started successfully" {
        puts "Qanto node started successfully"
    }
    "Error:" {
        puts "Error starting Qanto node"
        exit 1
    }
    timeout {
        puts "Timeout waiting for node to start"
        exit 1
    }
    eof {
        puts "Unexpected end of file"
        exit 1
    }
}

interact
EOF

chmod +x /tmp/start_qanto.exp

log "Starting Qanto node with configuration: $CONFIG_PATH"
log "Wallet: $WALLET_PATH"
log "P2P Identity: $P2P_IDENTITY"
log "Peer Cache: $PEER_CACHE"

# Start the node in background for health checking
/tmp/start_qanto.exp "$PASSWORD" "$CONFIG_PATH" "$WALLET_PATH" "$P2P_IDENTITY" "$PEER_CACHE" &
QANTO_PID=$!

# Wait a bit for the node to start
sleep 15

# Perform health check
if health_check; then
    log "Qanto node is healthy and ready"
else
    log "ERROR: Qanto node failed health check"
    kill -TERM "$QANTO_PID" 2>/dev/null || true
    exit 1
fi

# Wait for the main process
wait "$QANTO_PID"
ENTRYPOINT

# Make entrypoint executable and set proper ownership
RUN chmod +x /usr/local/bin/entrypoint.sh && \
    chown qanto:qanto /usr/local/bin/entrypoint.sh

# Create health check script
COPY --chown=qanto:qanto <<'HEALTHCHECK' /usr/local/bin/healthcheck.sh
#!/usr/bin/env bash
set -euo pipefail

# Check if the API is responding
if curl -f -s --max-time 10 http://localhost:8081/health > /dev/null 2>&1; then
    echo "Qanto node is healthy"
    exit 0
else
    echo "Qanto node health check failed"
    exit 1
fi
HEALTHCHECK

RUN chmod +x /usr/local/bin/healthcheck.sh && \
    chown qanto:qanto /usr/local/bin/healthcheck.sh

# Add health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/usr/local/bin/healthcheck.sh"]

# Switch to non-root user
USER qanto

# Set working directory
WORKDIR /opt/qanto

# Expose ports
# 8080: External API
# 8081: Internal API and health checks
# 8333: P2P network communication
EXPOSE 8080 8081 8333

# Add labels for better container management
LABEL maintainer="Qanto Development Team" \
      version="${VERSION:-latest}" \
      description="Qanto blockchain node with enhanced security and monitoring" \
      org.opencontainers.image.title="Qanto Node" \
      org.opencontainers.image.description="Production-ready Qanto blockchain node" \
      org.opencontainers.image.vendor="Qanto" \
      org.opencontainers.image.licenses="MIT"

# Use tini as init system for proper signal handling
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]

# Default command (can be overridden)
CMD []
