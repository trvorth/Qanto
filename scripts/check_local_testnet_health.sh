#!/bin/bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_ENV_FILE="${REPO_ROOT}/.testnet/runtime.env"

if [[ -f "${RUNTIME_ENV_FILE}" ]]; then
  # shellcheck disable=SC1090
  source "${RUNTIME_ENV_FILE}"
fi

BOOTNODE_URL="${BOOTNODE_URL:-http://127.0.0.1:18081}"
VALIDATOR_URL="${VALIDATOR_URL:-http://127.0.0.1:18082}"
RPC_URL="${RPC_URL:-http://127.0.0.1:8081}"
BLOCK_GROWTH_WAIT_SECS="${BLOCK_GROWTH_WAIT_SECS:-12}"
MIN_NEW_BLOCKS="${MIN_NEW_BLOCKS:-3}"

for cmd in curl python3; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}" >&2
    exit 1
  fi
done

fetch_json() {
  local url="$1"
  curl -fsS --max-time 5 "${url}"
}

json_field() {
  local json_payload="$1"
  local field_name="$2"
  python3 -c '
import json
import sys

field = sys.argv[1]
payload = json.loads(sys.argv[2])
value = payload.get(field)
if value is None:
    print("")
elif isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
' "${field_name}" "${json_payload}"
}

require_healthy() {
  local node_name="$1"
  local base_url="$2"
  local health_json
  health_json="$(fetch_json "${base_url}/health")"
  local status
  status="$(json_field "${health_json}" "status")"
  if [[ "${status}" != "healthy" ]]; then
    echo "[FAIL] ${node_name}: /health returned unexpected payload: ${health_json}" >&2
    exit 1
  fi
}

require_peers() {
  local node_name="$1"
  local base_url="$2"
  local peers_json
  peers_json="$(fetch_json "${base_url}/peers")"
  local peer_count
  peer_count="$(python3 -c 'import json, sys; print(len(json.loads(sys.argv[1])))' "${peers_json}")"
  if [[ "${peer_count}" -lt 1 ]]; then
    echo "[FAIL] ${node_name}: no connected peers detected" >&2
    exit 1
  fi
  echo "[OK] ${node_name}: connected peers = ${peer_count}"
}

rpc_stats_before="$(fetch_json "${RPC_URL}/stats")"
height_before="$(json_field "${rpc_stats_before}" "latest_block_height")"
finality_ms="$(json_field "${rpc_stats_before}" "finality_ms")"

require_healthy "bootnode" "${BOOTNODE_URL}"
require_healthy "validator1" "${VALIDATOR_URL}"
require_healthy "rpcnode" "${RPC_URL}"

echo "[OK] health endpoint is healthy on all three nodes"

require_peers "bootnode" "${BOOTNODE_URL}"
require_peers "validator1" "${VALIDATOR_URL}"
require_peers "rpcnode" "${RPC_URL}"

sleep "${BLOCK_GROWTH_WAIT_SECS}"

rpc_stats_after="$(fetch_json "${RPC_URL}/stats")"
height_after="$(json_field "${rpc_stats_after}" "latest_block_height")"
latest_block_hash="$(json_field "${rpc_stats_after}" "latest_block_hash")"
avg_block_interval="$(json_field "${rpc_stats_after}" "avg_block_interval")"
blocks_last_minute="$(json_field "${rpc_stats_after}" "blocks_last_minute")"

if [[ -z "${height_before}" || -z "${height_after}" ]]; then
  echo "[FAIL] rpcnode: missing latest_block_height in /stats response" >&2
  exit 1
fi

if (( height_after <= 0 )); then
  echo "[FAIL] rpcnode: latest_block_height must be greater than zero, got ${height_after}" >&2
  exit 1
fi

if (( height_after - height_before < MIN_NEW_BLOCKS )); then
  echo "[FAIL] rpcnode: block height growth is below threshold (${height_before} -> ${height_after}, required +${MIN_NEW_BLOCKS})" >&2
  exit 1
fi

echo "[OK] block height is growing: ${height_before} -> ${height_after} (delta=$((height_after - height_before)))"

if [[ -z "${finality_ms}" || "${finality_ms}" == "0" ]]; then
  echo "[FAIL] rpcnode: finality_ms is empty or zero, finality path is not confirmed" >&2
  exit 1
fi

if [[ -z "${blocks_last_minute}" || "${blocks_last_minute}" == "0" ]]; then
  echo "[FAIL] rpcnode: blocks_last_minute indicates no active block production" >&2
  exit 1
fi

echo "[OK] finality metrics are present: finality_ms=${finality_ms}, latest_block_hash=${latest_block_hash}, avg_block_interval=${avg_block_interval}, blocks_last_minute=${blocks_last_minute}"

readiness_json="$(fetch_json "${RPC_URL}/publish-readiness")"
is_ready="$(json_field "${readiness_json}" "is_ready")"

if [[ "${is_ready}" != "true" ]]; then
  echo "[WARN] publish-readiness is not fully green yet: ${readiness_json}"
else
  echo "[OK] publish-readiness reports node is ready"
fi

echo
echo "Local testnet health summary"
echo "  1) Peer connectivity: OK"
echo "  2) Block growth: OK"
echo "  3) Finality metrics: OK"
