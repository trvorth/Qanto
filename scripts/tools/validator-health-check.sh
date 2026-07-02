#!/bin/bash
# Qanto Validator Health Checker

NODE_URL=${1:-"http://127.0.0.1:8081"}

# 1. Probe health endpoint
HEALTH_RESP=$(curl -s --max-time 3 "${NODE_URL}/health" 2>/dev/null)
if [ $? -ne 0 ] || [ "$(echo "$HEALTH_RESP" | grep -o "healthy")" != "healthy" ]; then
    cat <<EOF
{
  "healthy": false,
  "height": 0,
  "peers": 0,
  "version": "unknown",
  "reason": "Connection failed or unhealthy status"
}
EOF
    exit 0
fi

# 2. Probe stats endpoint
STATS_RESP=$(curl -s --max-time 3 "${NODE_URL}/stats" 2>/dev/null)
if [ $? -ne 0 ]; then
    cat <<EOF
{
  "healthy": false,
  "height": 0,
  "peers": 0,
  "version": "unknown",
  "reason": "Health endpoint online but stats endpoint failed"
}
EOF
    exit 0
fi

# 3. Extract JSON parameters
HEIGHT=$(echo "$STATS_RESP" | grep -o '"block_count":[0-9]*' | cut -d':' -f2 || echo 0)
PEERS=$(echo "$STATS_RESP" | grep -o '"peer_count":[0-9]*' | cut -d':' -f2 || echo 0)
VERSION=$(echo "$STATS_RESP" | grep -o '"version":"[^"]*"' | cut -d':' -f2 | tr -d '"' || echo "unknown")

cat <<EOF
{
  "healthy": true,
  "height": ${HEIGHT:-0},
  "peers": ${PEERS:-0},
  "version": "${VERSION:-"unknown"}"
}
EOF
