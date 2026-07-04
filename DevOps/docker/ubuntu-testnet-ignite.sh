#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_PATH="$(python3 - <<'PY'
import os, sys
print(os.path.realpath(sys.argv[1]))
PY
"${BASH_SOURCE[0]}")"

if [[ ! -x "${SCRIPT_PATH}" ]]; then
  echo "ERROR: ${SCRIPT_PATH} is not executable. Run: chmod +x ${SCRIPT_PATH}" >&2
  exit 1
fi

on_error() {
  local line="$1"
  local command="$2"
  echo "ERROR: command failed at line ${line}: ${command}" >&2
}
trap 'on_error "${LINENO}" "${BASH_COMMAND}"' ERR

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

require_env() {
  local name="$1"
  [[ -n "${!name:-}" ]] || fail "Environment variable ${name} is required"
}

if [[ "${EUID}" -eq 0 ]]; then
  SUDO=()
else
  require_cmd sudo
  SUDO=(sudo)
fi

run_root() {
  "${SUDO[@]}" "$@"
}

docker_compose() {
  (
    cd "${QANTO_REPO_DIR}"
    "${SUDO[@]}" docker compose \
      --env-file "${QANTO_ENV_FILE}" \
      -f "${COMPOSE_FILE}" \
      "$@"
  )
}

extract_toml_string() {
  local key="$1"
  local file="$2"
  grep "^${key} = " "${file}" | tail -1 | sed -E 's/^[^"]*"([^"]+)".*$/\1/'
}

wait_for_toml_value() {
  local key="$1"
  local file="$2"
  local timeout_secs="$3"
  local elapsed=0

  while (( elapsed < timeout_secs )); do
    if [[ -f "${file}" ]]; then
      local value
      value="$(extract_toml_string "${key}" "${file}" || true)"
      if [[ -n "${value}" ]]; then
        printf '%s\n' "${value}"
        return 0
      fi
    fi
    sleep 2
    elapsed=$((elapsed + 2))
  done

  return 1
}

wait_for_http_jq() {
  local url="$1"
  local jq_filter="$2"
  local timeout_secs="$3"
  local elapsed=0
  local body

  while (( elapsed < timeout_secs )); do
    if body="$(curl -fsS --max-time 10 "${url}" 2>/dev/null)"; then
      if jq -e "${jq_filter}" >/dev/null 2>&1 <<<"${body}"; then
        printf '%s\n' "${body}"
        return 0
      fi
    fi
    sleep 3
    elapsed=$((elapsed + 3))
  done

  return 1
}

install_docker_if_needed() {
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    log "Docker and Docker Compose are already installed"
    return 0
  fi

  log "Installing Docker Engine and Docker Compose plugin"
  run_root apt-get update
  run_root apt-get install -y ca-certificates curl git gnupg jq lsb-release openssl python3
  run_root install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg | run_root gpg --dearmor -o /etc/apt/keyrings/docker.gpg
  run_root chmod a+r /etc/apt/keyrings/docker.gpg

  local codename
  codename="$(. /etc/os-release && echo "${VERSION_CODENAME}")"
  echo \
    "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu ${codename} stable" \
    | run_root tee /etc/apt/sources.list.d/docker.list >/dev/null

  run_root apt-get update
  run_root apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  run_root systemctl enable --now docker
}

clone_or_update_repo() {
  if [[ -d "${QANTO_REPO_DIR}/.git" ]]; then
    log "Updating existing repository checkout at ${QANTO_REPO_DIR}"
    git -C "${QANTO_REPO_DIR}" fetch --tags --prune origin
  else
    log "Cloning repository ${QANTO_GIT_URL} into ${QANTO_REPO_DIR}"
    mkdir -p "$(dirname "${QANTO_REPO_DIR}")"
    git clone "${QANTO_GIT_URL}" "${QANTO_REPO_DIR}"
  fi

  git -C "${QANTO_REPO_DIR}" checkout "${QANTO_REF}"
  git -C "${QANTO_REPO_DIR}" pull --ff-only origin "${QANTO_REF}" >/dev/null 2>&1 || true
}

prepare_directories() {
  log "Preparing persistent host directories under ${QANTO_BASE_DIR}"
  run_root mkdir -p \
    "${QANTO_BASE_DIR}/config" \
    "${QANTO_BASE_DIR}/secrets"
  run_root chmod 700 "${QANTO_BASE_DIR}/secrets"
}

write_env_file() {
  log "Writing compose environment file ${QANTO_ENV_FILE}"
  cat > "${QANTO_ENV_FILE}" <<EOF
QANTO_NODE_TAG=${QANTO_NODE_TAG}
QANTOSCAN_TAG=${QANTOSCAN_TAG}
QANTO_RUST_LOG=${QANTO_RUST_LOG}

QANTO_BOOTNODE_P2P_PORT=${QANTO_BOOTNODE_P2P_PORT}
QANTO_RPC_HTTP_PORT=${QANTO_RPC_HTTP_PORT}
QANTO_RPC_GRPC_PORT=${QANTO_RPC_GRPC_PORT}

QANTO_DOMAIN=${QANTO_DOMAIN}
ACME_EMAIL=${ACME_EMAIL}

QANTO_WALLET_PASSWORD_FILE=${QANTO_WALLET_PASSWORD_FILE}
QANTO_BOOTNODE_CONFIG_PATH=${QANTO_BOOTNODE_CONFIG_PATH}
QANTO_RPCNODE_CONFIG_PATH=${QANTO_RPCNODE_CONFIG_PATH}
QANTO_GENESIS_PATH=${QANTO_GENESIS_PATH}
EOF
  chmod 600 "${QANTO_ENV_FILE}"
}

ensure_wallet_password() {
  if [[ -s "${QANTO_WALLET_PASSWORD_FILE}" ]]; then
    log "Using existing wallet password file ${QANTO_WALLET_PASSWORD_FILE}"
    return 0
  fi

  log "Generating wallet password file ${QANTO_WALLET_PASSWORD_FILE}"
  mkdir -p "$(dirname "${QANTO_WALLET_PASSWORD_FILE}")"
  openssl rand -base64 48 | tr -d '\n' > "${QANTO_WALLET_PASSWORD_FILE}"
  chmod 600 "${QANTO_WALLET_PASSWORD_FILE}"
}

write_node_config() {
  local file="$1"
  local p2p_addr="$2"
  local peers_csv="$3"
  local mining_enabled="$4"
  local genesis_validator="$5"
  local local_full_addr="$6"

  cat > "${file}" <<EOF
p2p_address = "${p2p_addr}"
api_address = "0.0.0.0:8081"
peers = [${peers_csv}]
local_full_p2p_address = "${local_full_addr}"
network_id = "qanto-testnet"
chain_id = 1234
genesis_validator = "${genesis_validator}"
contract_address = "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394"
target_block_time = 1000
difficulty = 1
max_amount = 21000000000
use_gpu = false
zk_enabled = true
mining_threads = 4
mining_enabled = ${mining_enabled}
adaptive_mining_enabled = false
num_chains = 1
mining_chain_id = 0
mining_interval_ms = 1000
dummy_tx_interval_ms = 250
dummy_tx_per_cycle = 10
mempool_max_age_secs = 3600
mempool_max_size_bytes = 67108864
mempool_max_size = 100000
mempool_batch_size = 1000
mempool_backpressure_threshold = 800000000
tx_batch_size = 500
adaptive_batch_threshold = 700000000
memory_soft_limit = 8388608
memory_hard_limit = 10485760
dev_fee_rate = 100000000
data_dir = "/opt/qanto/data"
db_path = "/opt/qanto/data/qanto.db"
wallet_path = "/opt/qanto/keys/wallet.key"
p2p_identity_path = "/opt/qanto/keys/p2p_identity.key"
log_file_path = "/opt/qanto/logs/qanto.log"

[logging]
level = "info"
enable_block_celebrations = false
celebration_log_level = "info"

[p2p]
heartbeat_interval = 2000
mesh_n_low = 2
mesh_n = 4
mesh_n_high = 8
mesh_outbound_min = 2

[rpc]
address = "0.0.0.0:50051"

[mempool]
capacity = 100000
parallel_verification_threads = 4
EOF

  run_root chown 10001:10001 "${file}"
  run_root chmod 664 "${file}"
}

seed_genesis_file() {
  log "Seeding genesis file from repository source"
  cp "${QANTO_REPO_DIR}/config/genesis.json" "${QANTO_GENESIS_PATH}"
  chmod 644 "${QANTO_GENESIS_PATH}"
}

rewrite_genesis_file() {
  local validator="$1"
  local tmp_file
  tmp_file="$(mktemp)"

  jq \
    --arg validator "${validator}" \
    '.genesisValidator = $validator
     | .allocations = ((.allocations // []) | map(.address = $validator))' \
    "${QANTO_GENESIS_PATH}" > "${tmp_file}"

  mv "${tmp_file}" "${QANTO_GENESIS_PATH}"
  chmod 644 "${QANTO_GENESIS_PATH}"
}

show_service_logs() {
  docker_compose logs --tail=120 bootnode rpcnode qantoscan caddy || true
}

bootstrap_bootnode_identity() {
  log "Bootstrapping bootnode identity and deterministic validator address"

  docker_compose rm -sf bootnode rpcnode qantoscan caddy >/dev/null 2>&1 || true
  docker_compose up -d --build bootnode

  if ! BOOTNODE_LOCAL_FULL_ADDR="$(wait_for_toml_value "local_full_p2p_address" "${QANTO_BOOTNODE_CONFIG_PATH}" 180)"; then
    show_service_logs
    fail "Bootnode did not publish local_full_p2p_address within timeout"
  fi

  BOOTNODE_GENESIS_VALIDATOR="$(extract_toml_string "genesis_validator" "${QANTO_BOOTNODE_CONFIG_PATH}")"
  [[ -n "${BOOTNODE_GENESIS_VALIDATOR}" ]] || fail "Bootnode config did not expose genesis_validator"

  BOOTNODE_PEER_ID="${BOOTNODE_LOCAL_FULL_ADDR##*/p2p/}"
  [[ -n "${BOOTNODE_PEER_ID}" ]] || fail "Failed to extract bootnode peer ID from ${BOOTNODE_LOCAL_FULL_ADDR}"

  BOOTNODE_INTERNAL_MULTIADDR="/dns4/bootnode/tcp/30303/ws/p2p/${BOOTNODE_PEER_ID}"

  log "Derived bootnode validator address ${BOOTNODE_GENESIS_VALIDATOR}"
  log "Derived bootnode internal multiaddr ${BOOTNODE_INTERNAL_MULTIADDR}"
}

configure_final_stack() {
  log "Rewriting genesis and node configuration with derived bootnode identity"
  rewrite_genesis_file "${BOOTNODE_GENESIS_VALIDATOR}"

  write_node_config \
    "${QANTO_BOOTNODE_CONFIG_PATH}" \
    "/ip4/0.0.0.0/tcp/30303/ws" \
    "" \
    "${QANTO_BOOTNODE_MINING_ENABLED}" \
    "${BOOTNODE_GENESIS_VALIDATOR}" \
    "${BOOTNODE_LOCAL_FULL_ADDR}"

  write_node_config \
    "${QANTO_RPCNODE_CONFIG_PATH}" \
    "/ip4/0.0.0.0/tcp/30304/ws" \
    "\"${BOOTNODE_INTERNAL_MULTIADDR}\"" \
    "${QANTO_RPCNODE_MINING_ENABLED}" \
    "${BOOTNODE_GENESIS_VALIDATOR}" \
    ""

  cat > "${QANTO_BASE_DIR}/runtime.env" <<EOF
BOOTNODE_GENESIS_VALIDATOR=${BOOTNODE_GENESIS_VALIDATOR}
BOOTNODE_LOCAL_FULL_ADDR=${BOOTNODE_LOCAL_FULL_ADDR}
BOOTNODE_INTERNAL_MULTIADDR=${BOOTNODE_INTERNAL_MULTIADDR}
EOF
  chmod 600 "${QANTO_BASE_DIR}/runtime.env"
}

launch_full_stack() {
  log "Launching full public testnet stack"
  docker_compose rm -sf bootnode rpcnode qantoscan caddy >/dev/null 2>&1 || true
  docker_compose up -d --build
}

validate_stack() {
  log "Waiting for RPC readiness"
  if ! wait_for_http_jq "http://127.0.0.1:${QANTO_RPC_HTTP_PORT}/publish-readiness" '.is_ready == true' 240 >/dev/null; then
    show_service_logs
    fail "RPC publish-readiness never reached is_ready=true"
  fi

  log "Waiting for the first block to appear on /stats"
  if ! wait_for_http_jq "http://127.0.0.1:${QANTO_RPC_HTTP_PORT}/stats" '.block_count >= 1' 240 >/dev/null; then
    show_service_logs
    fail "RPC /stats never reported block_count >= 1"
  fi

  log "Running local cloud operations checklist"
  BOOTNODE_PORT="${QANTO_BOOTNODE_P2P_PORT}" \
  RPC_HTTP_PORT="${QANTO_RPC_HTTP_PORT}" \
  RPC_GRPC_PORT="${QANTO_RPC_GRPC_PORT}" \
  QANTO_DOMAIN="${QANTO_DOMAIN}" \
  DOCKER_COMPOSE_FILE="${COMPOSE_FILE}" \
  bash "${QANTO_REPO_DIR}/DevOps/docker/testnet-cloud-ops-check.sh"
}

main() {
  [[ -r /etc/os-release ]] || fail "Cannot read /etc/os-release"
  # shellcheck disable=SC1091
  source /etc/os-release
  [[ "${ID:-}" == "ubuntu" ]] || fail "This script only supports Ubuntu hosts"

  QANTO_GIT_URL="${QANTO_GIT_URL:-https://github.com/trvorth/qanto.git}"
  QANTO_REF="${QANTO_REF:-main}"
  QANTO_BASE_DIR="${QANTO_BASE_DIR:-/srv/qanto}"
  QANTO_REPO_DIR="${QANTO_REPO_DIR:-${QANTO_BASE_DIR}/repo}"
  QANTO_ENV_FILE="${QANTO_ENV_FILE:-${QANTO_BASE_DIR}/qanto-public-testnet.env}"
  QANTO_WALLET_PASSWORD_FILE="${QANTO_WALLET_PASSWORD_FILE:-${QANTO_BASE_DIR}/secrets/wallet_password.txt}"
  QANTO_BOOTNODE_CONFIG_PATH="${QANTO_BOOTNODE_CONFIG_PATH:-${QANTO_BASE_DIR}/config/bootnode.toml}"
  QANTO_RPCNODE_CONFIG_PATH="${QANTO_RPCNODE_CONFIG_PATH:-${QANTO_BASE_DIR}/config/rpcnode.toml}"
  QANTO_GENESIS_PATH="${QANTO_GENESIS_PATH:-${QANTO_BASE_DIR}/config/genesis.json}"
  QANTO_NODE_TAG="${QANTO_NODE_TAG:-public-testnet}"
  QANTOSCAN_TAG="${QANTOSCAN_TAG:-public-testnet}"
  QANTO_RUST_LOG="${QANTO_RUST_LOG:-info}"
  QANTO_BOOTNODE_P2P_PORT="${QANTO_BOOTNODE_P2P_PORT:-30303}"
  QANTO_RPC_HTTP_PORT="${QANTO_RPC_HTTP_PORT:-8081}"
  QANTO_RPC_GRPC_PORT="${QANTO_RPC_GRPC_PORT:-50051}"
  QANTO_BOOTNODE_MINING_ENABLED="${QANTO_BOOTNODE_MINING_ENABLED:-true}"
  QANTO_RPCNODE_MINING_ENABLED="${QANTO_RPCNODE_MINING_ENABLED:-false}"
  COMPOSE_FILE="${QANTO_REPO_DIR}/DevOps/docker/docker-compose.prod.yml"

  require_env QANTO_DOMAIN
  require_env ACME_EMAIL

  install_docker_if_needed
  require_cmd git
  require_cmd jq
  require_cmd curl
  require_cmd openssl
  require_cmd python3

  clone_or_update_repo
  [[ -f "${COMPOSE_FILE}" ]] || fail "Compose file not found: ${COMPOSE_FILE}"
  [[ -f "${QANTO_REPO_DIR}/DevOps/docker/testnet-cloud-ops-check.sh" ]] || fail "Cloud ops checker missing"

  prepare_directories
  ensure_wallet_password
  seed_genesis_file

  local initial_genesis_validator
  initial_genesis_validator="$(jq -r '.genesisValidator' "${QANTO_GENESIS_PATH}")"
  write_node_config \
    "${QANTO_BOOTNODE_CONFIG_PATH}" \
    "/ip4/0.0.0.0/tcp/30303/ws" \
    "" \
    "false" \
    "${initial_genesis_validator}" \
    ""

  write_node_config \
    "${QANTO_RPCNODE_CONFIG_PATH}" \
    "/ip4/0.0.0.0/tcp/30304/ws" \
    "" \
    "${QANTO_RPCNODE_MINING_ENABLED}" \
    "${initial_genesis_validator}" \
    ""

  write_env_file
  bootstrap_bootnode_identity
  configure_final_stack
  write_env_file
  launch_full_stack
  validate_stack

  log "QANTO public testnet stack is live"
  log "Explorer: https://${QANTO_DOMAIN}"
  log "RPC health: http://127.0.0.1:${QANTO_RPC_HTTP_PORT}/health"
  log "Runtime bootstrap metadata: ${QANTO_BASE_DIR}/runtime.env"
}

main "$@"
