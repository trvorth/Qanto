#!/usr/bin/env bash
set -euo pipefail

BOOTNODE_PORT="${BOOTNODE_PORT:-30303}"
RPC_HTTP_PORT="${RPC_HTTP_PORT:-8081}"
RPC_GRPC_PORT="${RPC_GRPC_PORT:-50051}"
QANTO_DOMAIN="${QANTO_DOMAIN:-}"
RPC_UPSTREAM="${RPC_UPSTREAM:-http://127.0.0.1:${RPC_HTTP_PORT}}"
DOCKER_COMPOSE_FILE="${DOCKER_COMPOSE_FILE:-docker-compose.prod.yml}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

print_section() {
  printf '\n== %s ==\n' "$1"
}

check_port_listener() {
  local port="$1"
  if ss -lntup | grep -q ":${port} "; then
    echo "LISTEN OK: ${port}"
  else
    echo "LISTEN FAIL: ${port} is not listening"
    return 1
  fi
}

check_http() {
  local url="$1"
  local label="$2"
  if curl -fsS --max-time 8 "$url" >/tmp/qanto-http-check.out 2>/tmp/qanto-http-check.err; then
    echo "HTTP OK: ${label} -> ${url}"
    cat /tmp/qanto-http-check.out
  else
    echo "HTTP FAIL: ${label} -> ${url}" >&2
    cat /tmp/qanto-http-check.err >&2 || true
    return 1
  fi
}

require_cmd ss
require_cmd curl

print_section "Host Port Listeners"
check_port_listener "$BOOTNODE_PORT"
check_port_listener "$RPC_HTTP_PORT"
check_port_listener "$RPC_GRPC_PORT"

print_section "Host Firewall"
if command -v ufw >/dev/null 2>&1; then
  sudo ufw status numbered || true
else
  echo "ufw not installed"
fi

print_section "Docker Service Status"
if command -v docker >/dev/null 2>&1; then
  docker compose -f "$DOCKER_COMPOSE_FILE" ps || true
  docker compose -f "$DOCKER_COMPOSE_FILE" logs --tail=30 bootnode rpcnode qantoscan caddy || true
else
  echo "docker not installed"
fi

print_section "Local RPC Health"
check_http "${RPC_UPSTREAM}/health" "rpc health"
check_http "${RPC_UPSTREAM}/publish-readiness" "rpc publish readiness"

print_section "Public Explorer and API"
if [[ -n "$QANTO_DOMAIN" ]]; then
  check_http "https://${QANTO_DOMAIN}/healthz" "explorer healthz"
  check_http "https://${QANTO_DOMAIN}/api/health" "explorer api proxy"
else
  echo "QANTO_DOMAIN not set; skipping public endpoint checks"
fi

print_section "Bootnode Public Reachability Guidance"
cat <<EOF
Run these commands from a second machine on the public internet:

  nc -vz <SERVER_PUBLIC_IP> ${BOOTNODE_PORT}
  nc -vz <SERVER_PUBLIC_IP> ${RPC_HTTP_PORT}
  nc -vz <SERVER_PUBLIC_IP> ${RPC_GRPC_PORT}

Success criteria:
  - nc reports 'succeeded' or 'open'
  - /health returns HTTP 200
  - /publish-readiness contains '"is_ready":true'

If public checks fail:
  1. Verify cloud security group / firewall allows TCP ${BOOTNODE_PORT}, ${RPC_HTTP_PORT}, ${RPC_GRPC_PORT}
  2. Verify host firewall allows the same ports
  3. Verify docker compose published the expected host ports
  4. Verify qantoscan /api proxy reaches rpcnode without 502/504
EOF
