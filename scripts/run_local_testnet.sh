#!/bin/bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TESTNET_ROOT="${REPO_ROOT}/.testnet"
CONFIG_ROOT="${TESTNET_ROOT}/config"
DATA_ROOT="${TESTNET_ROOT}/data"
LOG_ROOT="${REPO_ROOT}/logs/testnet"
PID_ROOT="${TESTNET_ROOT}/pids"
SHARED_ROOT="${TESTNET_ROOT}/shared"
GENESIS_FILE="${REPO_ROOT}/config/genesis.json"
QANTO_BIN="${REPO_ROOT}/target/release/qanto"
HEALTH_SCRIPT="${REPO_ROOT}/Scripts/check_local_testnet_health.sh"
GENESIS_VALIDATOR_FILE="${SHARED_ROOT}/genesis_validator.txt"

ACTION="${1:-start}"
PREFUND_ADDRESS="${QANTO_TESTNET_PREFUND_ADDRESS:-}"

BOOTNODE_NAME="bootnode"
VALIDATOR_NAME="validator1"
RPC_NAME="rpcnode"

BOOTNODE_P2P_PORT=30303
BOOTNODE_API_PORT=18081
BOOTNODE_RPC_PORT=15051

VALIDATOR_P2P_PORT=30304
VALIDATOR_API_PORT=18082
VALIDATOR_RPC_PORT=15052

RPCNODE_P2P_PORT=30305
RPCNODE_API_PORT=8081
RPCNODE_RPC_PORT=50051

DEFAULT_PREFUND_ADDRESS="ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"
MAX_SUPPLY_QNTO="21000000000"
SMALLEST_UNITS_PER_QANTO="1000000000"
MAX_SUPPLY_BASE_UNITS="21000000000000000000"
NETWORK_ID="qanto-local-testnet-alpha"
CHAIN_ID="1234"

mkdir -p "${CONFIG_ROOT}" "${DATA_ROOT}" "${LOG_ROOT}" "${PID_ROOT}" "${SHARED_ROOT}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

validate_hex_address() {
  local address="$1"
  if [[ ! "${address}" =~ ^[0-9a-fA-F]{64}$ ]]; then
    echo "Invalid 64-byte hex address: ${address}" >&2
    exit 1
  fi
}

config_path_for() {
  echo "${CONFIG_ROOT}/$1.toml"
}

pid_path_for() {
  echo "${PID_ROOT}/$1.pid"
}

log_path_for() {
  echo "${LOG_ROOT}/$1.log"
}

wallet_path() {
  echo "${SHARED_ROOT}/validator.wallet.key"
}

peer_cache_path_for() {
  echo "${DATA_ROOT}/$1/peer_cache.json"
}

identity_path_for() {
  echo "${DATA_ROOT}/$1/p2p_identity.key"
}

data_dir_for() {
  echo "${DATA_ROOT}/$1"
}

db_path_for() {
  echo "$(data_dir_for "$1")/qanto.db"
}

runtime_env_path() {
  echo "${TESTNET_ROOT}/runtime.env"
}

write_runtime_env() {
  local bootnode_multiaddr="$1"
  local validator_multiaddr="$2"
  local genesis_validator="$3"

  cat > "$(runtime_env_path)" <<EOF
BOOTNODE_URL=http://127.0.0.1:${BOOTNODE_API_PORT}
VALIDATOR_URL=http://127.0.0.1:${VALIDATOR_API_PORT}
RPC_URL=http://127.0.0.1:${RPCNODE_API_PORT}
BOOTNODE_MULTIADDR=${bootnode_multiaddr}
VALIDATOR_MULTIADDR=${validator_multiaddr}
GENESIS_VALIDATOR=${genesis_validator}
PREFUND_ADDRESS=${PREFUND_ADDRESS}
GENESIS_FILE=${GENESIS_FILE}
EOF
}

seed_genesis_validator() {
  if [[ -f "${GENESIS_VALIDATOR_FILE}" ]]; then
    cat "${GENESIS_VALIDATOR_FILE}"
  else
    echo "${DEFAULT_PREFUND_ADDRESS}"
  fi
}

stop_existing_node() {
  local node_name="$1"
  local pid_file
  pid_file="$(pid_path_for "${node_name}")"
  if [[ -f "${pid_file}" ]]; then
    local pid
    pid="$(cat "${pid_file}")"
    if kill -0 "${pid}" >/dev/null 2>&1; then
      echo "Stopping ${node_name} (pid ${pid})"
      kill "${pid}" >/dev/null 2>&1 || true
      local waited=0
      while kill -0 "${pid}" >/dev/null 2>&1; do
        sleep 1
        waited=$((waited + 1))
        if (( waited >= 20 )); then
          echo "Force stopping ${node_name} (pid ${pid})"
          kill -9 "${pid}" >/dev/null 2>&1 || true
          break
        fi
      done
    fi
    rm -f "${pid_file}"
  fi
}

stop_cluster() {
  stop_existing_node "${RPC_NAME}"
  stop_existing_node "${VALIDATOR_NAME}"
  stop_existing_node "${BOOTNODE_NAME}"
}

extract_toml_string() {
  local key="$1"
  local file="$2"
  grep "^${key} = " "${file}" | tail -1 | sed -E 's/^[^"]*"([^"]+)".*$/\1/'
}

write_genesis_file() {
  local genesis_validator="$1"
  cat > "${GENESIS_FILE}" <<EOF
{
  "networkId": "${NETWORK_ID}",
  "chainId": ${CHAIN_ID},
  "genesisTimestamp": 1717250400,
  "genesisValidator": "${genesis_validator}",
  "contractAddress": "${PREFUND_ADDRESS}",
  "tokenomics": {
    "symbol": "QNTO",
    "maxSupply": "${MAX_SUPPLY_QNTO}",
    "tokenDecimals": 9,
    "smallestUnitsPerQanto": "${SMALLEST_UNITS_PER_QANTO}",
    "maxSupplyBaseUnits": "${MAX_SUPPLY_BASE_UNITS}"
  },
  "allocations": [
    {
      "address": "${PREFUND_ADDRESS}",
      "amountQanto": "${MAX_SUPPLY_QNTO}",
      "amountBaseUnits": "${MAX_SUPPLY_BASE_UNITS}",
      "role": "local-testnet-prefund"
    }
  ]
}
EOF
}

write_node_config() {
  local node_name="$1"
  local api_port="$2"
  local rpc_port="$3"
  local p2p_port="$4"
  local mining_enabled="$5"
  local genesis_validator="$6"
  local peers_csv="$7"

  local config_file
  config_file="$(config_path_for "${node_name}")"
  local data_dir
  data_dir="$(data_dir_for "${node_name}")"
  local db_path
  db_path="$(db_path_for "${node_name}")"
  local wallet_file
  wallet_file="$(wallet_path)"
  local identity_file
  identity_file="$(identity_path_for "${node_name}")"

  mkdir -p "${data_dir}"

  cat > "${config_file}" <<EOF
p2p_address = "/ip4/127.0.0.1/tcp/${p2p_port}/ws"
api_address = "127.0.0.1:${api_port}"
chain_id = ${CHAIN_ID}
peers = [${peers_csv}]
network_id = "${NETWORK_ID}"
genesis_validator = "${genesis_validator}"
contract_address = "${PREFUND_ADDRESS}"
target_block_time = 1000
difficulty = 1
max_amount = 21000000000
use_gpu = false
zk_enabled = false
mining_threads = 1
mining_enabled = ${mining_enabled}
adaptive_mining_enabled = false
producer_type = "solo"
hash_rate_interval_secs = 5
enable_detailed_telemetry = false
block_target_ms = 1000
solve_timeout_ms = 30000
difficulty_min = 1
db_cache_bytes = 67108864
mempool_max_bytes = 134217728
block_cache_size_mb = 64
write_buffer_size_mb = 32
enable_dummy_tx = false
max_dummy_per_block = 10
num_chains = 1
mining_chain_id = 0
mining_interval_ms = 1000
dummy_tx_interval_ms = 1000
dummy_tx_per_cycle = 10
mempool_max_age_secs = 3600
mempool_max_size_bytes = 67108864
mempool_max_size = 100000
mempool_batch_size = 100
mempool_backpressure_threshold = 800000000
tx_batch_size = 100
adaptive_batch_threshold = 700000000
memory_soft_limit = 8388608
memory_hard_limit = 10485760
dev_fee_rate = 100000000
data_dir = "${data_dir}"
db_path = "${db_path}"
wallet_path = "${wallet_file}"
p2p_identity_path = "${identity_file}"

[logging]
level = "info"
enable_block_celebrations = true
celebration_log_level = "info"

[p2p]
heartbeat_interval = 2000
mesh_n_low = 1
mesh_n = 2
mesh_n_high = 4
mesh_outbound_min = 1

[rpc]
address = "127.0.0.1:${rpc_port}"

[mempool]
capacity = 100000
parallel_verification_threads = 4
EOF
}

wait_for_file_key() {
  local key="$1"
  local file="$2"
  local timeout_secs="$3"
  local elapsed=0
  while (( elapsed < timeout_secs )); do
    if grep -q "^${key} = " "${file}" 2>/dev/null; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  return 1
}

wait_for_http() {
  local url="$1"
  local timeout_secs="$2"
  local elapsed=0
  while (( elapsed < timeout_secs )); do
    if curl -fsS --max-time 3 "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  return 1
}

wait_for_peer_count() {
  local url="$1"
  local min_peers="$2"
  local timeout_secs="$3"
  local elapsed=0
  while (( elapsed < timeout_secs )); do
    local peers_json
    peers_json="$(curl -fsS --max-time 3 "${url}" 2>/dev/null || true)"
    if [[ -n "${peers_json}" ]]; then
      local peer_count
      peer_count="$(python3 -c 'import json,sys; print(len(json.loads(sys.argv[1])))' "${peers_json}" 2>/dev/null || echo 0)"
      if [[ "${peer_count}" =~ ^[0-9]+$ ]] && (( peer_count >= min_peers )); then
        return 0
      fi
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  return 1
}

start_node_process() {
  local node_name="$1"
  local config_file
  config_file="$(config_path_for "${node_name}")"
  local log_file
  log_file="$(log_path_for "${node_name}")"

  mkdir -p "$(dirname "${log_file}")"
  : > "${log_file}"

  nohup env \
    WALLET_PASSWORD="" \
    RUST_LOG=info \
    QANTO_OMEGA_DISABLE=true \
    QANTO_GENESIS_FILE="${GENESIS_FILE}" \
    "${QANTO_BIN}" \
    --config "${config_file}" \
    --wallet "$(wallet_path)" \
    --p2p-identity "$(identity_path_for "${node_name}")" \
    --peer-cache "$(peer_cache_path_for "${node_name}")" \
    --data-dir "$(data_dir_for "${node_name}")" \
    --db-path "$(db_path_for "${node_name}")" \
    > "${log_file}" 2>&1 &

  local pid=$!
  echo "${pid}" > "$(pid_path_for "${node_name}")"
  echo "Started ${node_name} with pid ${pid}, log=${log_file}"
}

ensure_binary() {
  require_cmd cargo
  if [[ -x "${QANTO_BIN}" ]]; then
    echo "Using existing release binary: ${QANTO_BIN}"
    return
  fi
  echo "Release binary not found; building qanto node binary..."
  cargo build --release -p qanto --bin qanto
}

show_usage() {
  cat <<EOF
Usage:
  QANTO_TESTNET_PREFUND_ADDRESS=<64-hex-address> ./Scripts/run_local_testnet.sh
  ./Scripts/run_local_testnet.sh stop
  ./Scripts/run_local_testnet.sh status

Behavior:
  - Generates/updates config/genesis.json with canonical tokenomics
  - Starts bootnode, validator1, and rpcnode with isolated data/log directories
  - Writes runtime metadata to .testnet/runtime.env
  - Runs Scripts/check_local_testnet_health.sh at the end of startup
EOF
}

status_cluster() {
  for node_name in "${BOOTNODE_NAME}" "${VALIDATOR_NAME}" "${RPC_NAME}"; do
    local pid_file
    pid_file="$(pid_path_for "${node_name}")"
    if [[ -f "${pid_file}" ]] && kill -0 "$(cat "${pid_file}")" >/dev/null 2>&1; then
      echo "${node_name}: running (pid $(cat "${pid_file}"))"
    else
      echo "${node_name}: stopped"
    fi
  done

  if [[ -x "${HEALTH_SCRIPT}" ]]; then
    "${HEALTH_SCRIPT}" || true
  fi
}

start_cluster() {
  require_cmd curl
  require_cmd python3

  if [[ -z "${PREFUND_ADDRESS}" ]]; then
    PREFUND_ADDRESS="${DEFAULT_PREFUND_ADDRESS}"
    echo "QANTO_TESTNET_PREFUND_ADDRESS not set; defaulting prefund address to ${PREFUND_ADDRESS}"
  fi
  validate_hex_address "${PREFUND_ADDRESS}"

  stop_cluster
  ensure_binary

  rm -rf "${CONFIG_ROOT}" "${DATA_ROOT}" "${PID_ROOT}" "${LOG_ROOT}"
  mkdir -p "${CONFIG_ROOT}" "${DATA_ROOT}" "${PID_ROOT}" "${LOG_ROOT}" "${SHARED_ROOT}"

  if [[ -f "$(wallet_path)" && ! -f "${GENESIS_VALIDATOR_FILE}" ]]; then
    rm -f "$(wallet_path)"
  fi

  mkdir -p "$(data_dir_for "${BOOTNODE_NAME}")" "$(data_dir_for "${VALIDATOR_NAME}")" "$(data_dir_for "${RPC_NAME}")"

  write_node_config "${BOOTNODE_NAME}" "${BOOTNODE_API_PORT}" "${BOOTNODE_RPC_PORT}" "${BOOTNODE_P2P_PORT}" "false" "$(seed_genesis_validator)" ""
  start_node_process "${BOOTNODE_NAME}"

  if ! wait_for_file_key "local_full_p2p_address" "$(config_path_for "${BOOTNODE_NAME}")" 60; then
    echo "Bootnode did not publish local_full_p2p_address" >&2
    exit 1
  fi

  local genesis_validator
  genesis_validator="$(extract_toml_string "genesis_validator" "$(config_path_for "${BOOTNODE_NAME}")")"
  local bootnode_multiaddr
  bootnode_multiaddr="$(extract_toml_string "local_full_p2p_address" "$(config_path_for "${BOOTNODE_NAME}")")"
  echo "${genesis_validator}" > "${GENESIS_VALIDATOR_FILE}"

  write_genesis_file "${genesis_validator}"
  stop_existing_node "${BOOTNODE_NAME}"
  write_node_config "${BOOTNODE_NAME}" "${BOOTNODE_API_PORT}" "${BOOTNODE_RPC_PORT}" "${BOOTNODE_P2P_PORT}" "false" "${genesis_validator}" ""
  start_node_process "${BOOTNODE_NAME}"

  if ! wait_for_http "http://127.0.0.1:${BOOTNODE_API_PORT}/health" 90; then
    echo "Bootnode failed to expose /health; inspect $(log_path_for "${BOOTNODE_NAME}")" >&2
    exit 1
  fi

  write_node_config "${RPC_NAME}" "${RPCNODE_API_PORT}" "${RPCNODE_RPC_PORT}" "${RPCNODE_P2P_PORT}" "false" "${genesis_validator}" "\"${bootnode_multiaddr}\""
  start_node_process "${RPC_NAME}"

  if ! wait_for_http "http://127.0.0.1:${RPCNODE_API_PORT}/health" 90; then
    echo "RPC node failed to expose /health; inspect $(log_path_for "${RPC_NAME}")" >&2
    exit 1
  fi

  if ! wait_for_peer_count "http://127.0.0.1:${BOOTNODE_API_PORT}/peers" 1 90; then
    echo "Bootnode did not establish initial peer connectivity before mining start" >&2
    exit 1
  fi

  if ! wait_for_peer_count "http://127.0.0.1:${RPCNODE_API_PORT}/peers" 1 90; then
    echo "RPC node did not establish initial peer connectivity before mining start" >&2
    exit 1
  fi

  write_node_config "${VALIDATOR_NAME}" "${VALIDATOR_API_PORT}" "${VALIDATOR_RPC_PORT}" "${VALIDATOR_P2P_PORT}" "true" "${genesis_validator}" "\"${bootnode_multiaddr}\""
  start_node_process "${VALIDATOR_NAME}"

  if ! wait_for_http "http://127.0.0.1:${VALIDATOR_API_PORT}/health" 90; then
    echo "Validator failed to expose /health; inspect $(log_path_for "${VALIDATOR_NAME}")" >&2
    exit 1
  fi

  if ! wait_for_file_key "local_full_p2p_address" "$(config_path_for "${VALIDATOR_NAME}")" 60; then
    echo "Validator did not publish local_full_p2p_address" >&2
    exit 1
  fi

  local validator_multiaddr
  validator_multiaddr="$(extract_toml_string "local_full_p2p_address" "$(config_path_for "${VALIDATOR_NAME}")")"

  write_runtime_env "${bootnode_multiaddr}" "${validator_multiaddr}" "${genesis_validator}"

  chmod +x "${HEALTH_SCRIPT}"
  "${HEALTH_SCRIPT}"

  echo
  echo "Local QANTO testnet started successfully."
  echo "  Genesis file: ${GENESIS_FILE}"
  echo "  Prefunded address: ${PREFUND_ADDRESS}"
  echo "  Bootnode API: http://127.0.0.1:${BOOTNODE_API_PORT}"
  echo "  Validator API: http://127.0.0.1:${VALIDATOR_API_PORT}"
  echo "  RPC API: http://127.0.0.1:${RPCNODE_API_PORT}"
  echo "  RPC gRPC: 127.0.0.1:${RPCNODE_RPC_PORT}"
  echo "  Logs: ${LOG_ROOT}"
}

case "${ACTION}" in
  start)
    start_cluster
    ;;
  stop)
    stop_cluster
    echo "Local QANTO testnet stopped."
    ;;
  status)
    status_cluster
    ;;
  help|-h|--help)
    show_usage
    ;;
  *)
    echo "Unknown action: ${ACTION}" >&2
    show_usage
    exit 1
    ;;
esac
