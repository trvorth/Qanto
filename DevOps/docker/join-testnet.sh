#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

RUNTIME_ROOT="${QANTO_JOIN_ROOT:-${SCRIPT_DIR}/.join-testnet}"
TEMPLATE_CONFIG="${SCRIPT_DIR}/config/config.toml"
GENESIS_SOURCE="${REPO_ROOT}/config/genesis.json"
CONFIG_DIR="${RUNTIME_ROOT}/config"
SECRETS_DIR="${RUNTIME_ROOT}/secrets"
DATA_DIR="${RUNTIME_ROOT}/data"
KEYS_DIR="${RUNTIME_ROOT}/keys"
LOGS_DIR="${RUNTIME_ROOT}/logs"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
COMPOSE_FILE="${RUNTIME_ROOT}/docker-compose.join-testnet.yml"
WALLET_PASSWORD_FILE="${SECRETS_DIR}/wallet_password.txt"

QANTO_BOOTNODE_ADDR="${QANTO_BOOTNODE_ADDR:-}"
QANTO_NODE_NAME="${QANTO_NODE_NAME:-qanto-testnet-peer}"
QANTO_NODE_TAG="${QANTO_NODE_TAG:-public-testnet}"
QANTO_IMAGE="${QANTO_IMAGE:-qanto/qanto-node:${QANTO_NODE_TAG}}"
QANTO_HTTP_PORT="${QANTO_HTTP_PORT:-8081}"
QANTO_GRPC_PORT="${QANTO_GRPC_PORT:-50051}"
QANTO_P2P_PORT="${QANTO_P2P_PORT:-30303}"
QANTO_RUST_LOG="${QANTO_RUST_LOG:-info}"
QANTO_MINING_ENABLED="${QANTO_MINING_ENABLED:-false}"

log() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

validate_bootnode_addr() {
  [[ -n "${QANTO_BOOTNODE_ADDR}" ]] || fail "QANTO_BOOTNODE_ADDR is required"
  [[ "${QANTO_BOOTNODE_ADDR}" == *"/p2p/"* ]] || fail "QANTO_BOOTNODE_ADDR must include a /p2p/<peer-id> suffix"
  [[ "${QANTO_BOOTNODE_ADDR}" == /ip4/* || "${QANTO_BOOTNODE_ADDR}" == /dns4/* || "${QANTO_BOOTNODE_ADDR}" == /dns/* ]] || \
    fail "QANTO_BOOTNODE_ADDR must be a websocket multiaddr such as /ip4/<ip>/tcp/30303/ws/p2p/<peer-id>"
}

ensure_prerequisites() {
  require_cmd docker
  require_cmd python3
  require_cmd openssl
  docker compose version >/dev/null 2>&1 || fail "Docker Compose V2 is required"
  [[ -f "${TEMPLATE_CONFIG}" ]] || fail "Template config missing: ${TEMPLATE_CONFIG}"
  [[ -f "${GENESIS_SOURCE}" ]] || fail "Canonical genesis file missing: ${GENESIS_SOURCE}"
}

prepare_runtime_dirs() {
  mkdir -p "${CONFIG_DIR}" "${SECRETS_DIR}" "${DATA_DIR}" "${KEYS_DIR}" "${LOGS_DIR}"
}

ensure_wallet_password() {
  if [[ -s "${WALLET_PASSWORD_FILE}" ]]; then
    log "Using existing wallet password file ${WALLET_PASSWORD_FILE}"
    return 0
  fi

  log "Generating wallet password file ${WALLET_PASSWORD_FILE}"
  openssl rand -base64 48 | tr -d '\n' > "${WALLET_PASSWORD_FILE}"
  chmod 600 "${WALLET_PASSWORD_FILE}"
}

write_config() {
  python3 - "${TEMPLATE_CONFIG}" "${CONFIG_FILE}" "${QANTO_BOOTNODE_ADDR}" "${QANTO_MINING_ENABLED}" <<'PY'
import pathlib
import sys

template_path = pathlib.Path(sys.argv[1])
config_path = pathlib.Path(sys.argv[2])
bootnode_addr = sys.argv[3]
mining_enabled = sys.argv[4].lower()

if mining_enabled not in {"true", "false"}:
    raise SystemExit("QANTO_MINING_ENABLED must be either true or false")

template_lines = template_path.read_text().splitlines()
out_lines = []
in_rpc_section = False

scalar_replacements = {
    "p2p_address": '"/ip4/0.0.0.0/tcp/30303/ws"',
    "api_address": '"0.0.0.0:8081"',
    "peers": f'["{bootnode_addr}"]',
    "local_full_p2p_address": '""',
    "mining_enabled": mining_enabled,
    "data_dir": '"/opt/qanto/data"',
    "db_path": '"/opt/qanto/data/qanto.db"',
    "wallet_path": '"/opt/qanto/keys/wallet.key"',
    "p2p_identity_path": '"/opt/qanto/keys/p2p_identity.key"',
    "log_file_path": '"/opt/qanto/logs/qanto.log"',
}

for raw_line in template_lines:
    stripped = raw_line.strip()

    if stripped.startswith("[") and stripped.endswith("]"):
        in_rpc_section = stripped == "[rpc]"
        out_lines.append(raw_line)
        continue

    if in_rpc_section and stripped.startswith("address = "):
        out_lines.append('address = "0.0.0.0:50051"')
        continue

    replaced = False
    if not in_rpc_section:
        for key, value in scalar_replacements.items():
            prefix = f"{key} = "
            if stripped.startswith(prefix):
                out_lines.append(f"{key} = {value}")
                replaced = True
                break

    if not replaced:
        out_lines.append(raw_line)

config_path.write_text("\n".join(out_lines) + "\n")
PY
}

write_compose_file() {
  cat > "${COMPOSE_FILE}" <<EOF
version: "3.9"

services:
  qanto-node:
    build:
      context: ${REPO_ROOT}
      dockerfile: DevOps/docker/Dockerfile
    image: ${QANTO_IMAGE}
    container_name: ${QANTO_NODE_NAME}
    hostname: ${QANTO_NODE_NAME}
    restart: unless-stopped
    init: true
    environment:
      CONFIG_PATH: /opt/qanto/config/config.toml
      QANTO_GENESIS_FILE: /opt/qanto/config/genesis.json
      WALLET_PATH: /opt/qanto/keys/wallet.key
      P2P_IDENTITY: /opt/qanto/keys/p2p_identity.key
      PEER_CACHE: /opt/qanto/keys/peer_cache.json
      LOG_DIR: /opt/qanto/logs
      DATA_DIR: /opt/qanto/data
      HEALTHCHECK_URL: http://127.0.0.1:8081/health
      WALLET_PASSWORD_FILE: /run/secrets/qanto_wallet_password
      RUST_LOG: ${QANTO_RUST_LOG}
    ports:
      - "${QANTO_HTTP_PORT}:8081"
      - "${QANTO_GRPC_PORT}:50051"
      - "${QANTO_P2P_PORT}:30303/tcp"
      - "${QANTO_P2P_PORT}:30303/udp"
    volumes:
      - type: bind
        source: ${CONFIG_FILE}
        target: /opt/qanto/config/config.toml
      - type: bind
        source: ${GENESIS_SOURCE}
        target: /opt/qanto/config/genesis.json
        read_only: true
      - type: bind
        source: ${KEYS_DIR}
        target: /opt/qanto/keys
      - type: bind
        source: ${DATA_DIR}
        target: /opt/qanto/data
      - type: bind
        source: ${LOGS_DIR}
        target: /opt/qanto/logs
    secrets:
      - qanto_wallet_password
    healthcheck:
      test: ["CMD-SHELL", "curl -fsS http://127.0.0.1:8081/health >/dev/null || exit 1"]
      interval: 30s
      timeout: 10s
      retries: 5
      start_period: 60s
    logging:
      driver: json-file
      options:
        max-size: "100m"
        max-file: "5"

secrets:
  qanto_wallet_password:
    file: ${WALLET_PASSWORD_FILE}
EOF
}

start_node() {
  log "Launching follower/validator node with bootnode ${QANTO_BOOTNODE_ADDR}"
  docker compose -f "${COMPOSE_FILE}" up -d --build
}

print_next_steps() {
  cat <<EOF

QANTO testnet join node is starting.

Compose file:
  ${COMPOSE_FILE}

Config file:
  ${CONFIG_FILE}

Recommended verification:
  curl -fsS http://127.0.0.1:${QANTO_HTTP_PORT}/publish-readiness | jq .
  curl -fsS http://127.0.0.1:${QANTO_HTTP_PORT}/stats | jq '{peer_count,block_count,total_transactions,finality_ms,tps_current}'
  docker compose -f "${COMPOSE_FILE}" logs -f qanto-node

Notes:
  - This script always mounts the canonical repo genesis file at ${GENESIS_SOURCE}
  - Set QANTO_MINING_ENABLED=true only after you intentionally want validator/miner behavior
EOF
}

main() {
  validate_bootnode_addr
  ensure_prerequisites
  prepare_runtime_dirs
  ensure_wallet_password
  write_config
  write_compose_file
  start_node
  print_next_steps
}

main "$@"
