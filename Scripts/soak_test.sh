#!/usr/bin/env bash
# =============================================================================
# QANTO Soak Test Harness
# Sprint 8 - Workstream 5: Load & Soak Testing
#
# Starts a 3-node local validator network, floods transactions under
# configurable traffic shapes, and monitors resources over multiple hours.
#
# Usage:
#   ./soak_test.sh [--duration HOURS] [--tps TARGET_TPS] [--shape SHAPE]
#
# Traffic shapes: constant, burst, spike
# =============================================================================

set -euo pipefail

# --- Configuration ---
DURATION_HOURS="${1:-2}"
TARGET_TPS="${2:-1000}"
TRAFFIC_SHAPE="${3:-constant}"
BINARY_DIR="${BINARY_DIR:-$(cd "$(dirname "$0")/.." && pwd)/target/release}"
DATA_ROOT="/tmp/qanto_soak_test_$$"
LOG_DIR="${DATA_ROOT}/logs"
RESULTS_FILE="${DATA_ROOT}/soak_results.csv"
REPORT_INTERVAL_SECS=60

# Node configuration
NODE_COUNT=3
BASE_P2P_PORT=30300
BASE_RPC_PORT=8081
BASE_METRICS_PORT=9100

# Resource thresholds (alerts if exceeded)
MAX_RSS_MB=4096           # 4 GB per node
MAX_FD_COUNT=10000        # 10k file descriptors
MAX_THREAD_COUNT=500      # 500 threads per process
RSS_GROWTH_THRESHOLD=20   # 20% RSS growth over baseline = potential leak

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# --- Helper functions ---
log_info()  { echo -e "${CYAN}[SOAK]${NC} $(date '+%H:%M:%S') $*"; }
log_ok()    { echo -e "${GREEN}[SOAK]${NC} $(date '+%H:%M:%S') $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $(date '+%H:%M:%S') $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $(date '+%H:%M:%S') $*"; }

cleanup() {
    log_info "Cleaning up soak test processes..."
    for pid_file in "${DATA_ROOT}"/node_*.pid; do
        if [[ -f "$pid_file" ]]; then
            local pid
            pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
                log_info "Stopped node PID $pid"
            fi
        fi
    done
    # Stop tx_flood if running
    if [[ -f "${DATA_ROOT}/flood.pid" ]]; then
        local flood_pid
        flood_pid=$(cat "${DATA_ROOT}/flood.pid")
        if kill -0 "$flood_pid" 2>/dev/null; then
            kill "$flood_pid" 2>/dev/null || true
            wait "$flood_pid" 2>/dev/null || true
        fi
    fi
    log_info "Cleanup complete. Results at: ${DATA_ROOT}"
}
trap cleanup EXIT INT TERM

# --- Setup ---
log_info "=== QANTO Soak Test ==="
log_info "Duration:      ${DURATION_HOURS}h"
log_info "Target TPS:    ${TARGET_TPS}"
log_info "Traffic shape: ${TRAFFIC_SHAPE}"
log_info "Nodes:         ${NODE_COUNT}"
log_info "Data root:     ${DATA_ROOT}"

mkdir -p "${LOG_DIR}"
echo "timestamp,node,rss_mb,fd_count,thread_count,blocks,tips,mempool" > "${RESULTS_FILE}"

# Check binary exists
QANTO_BIN="${BINARY_DIR}/qanto"
TX_FLOOD_BIN="${BINARY_DIR}/tx_flood"
GET_ADDRESS_BIN="${BINARY_DIR}/get_address"

if [[ ! -x "$QANTO_BIN" || ! -x "$GET_ADDRESS_BIN" ]]; then
    log_warn "Binary not found, attempting cargo build..."
    cd "$(dirname "$0")/.."
    cargo build --release -p qanto --bin qanto --bin tx_flood --bin get_address 2>&1 | tail -5
    cd -
fi

if [[ ! -x "$QANTO_BIN" ]]; then
    log_error "Cannot find qanto binary."
    exit 1
fi

# --- Preflight Determinism Checks ---
log_info "Running preflight serialization determinism checks..."
cd "$(dirname "$0")/.."
if ! cargo test -p qanto --lib security_tests::test_transaction_roundtrip_determinism; then
    echo -e "${RED}[ERROR] ABORT SOAK TEST: Transaction serialization determinism check failed${NC}" >&2
    exit 1
fi
if ! cargo test -p qanto --lib security_tests::test_block_roundtrip_determinism; then
    echo -e "${RED}[ERROR] ABORT SOAK TEST: Block serialization determinism check failed${NC}" >&2
    exit 1
fi
cd - >/dev/null
log_ok "Preflight serialization determinism check passed."

# --- Start validator nodes ---
PIDS=()

log_info "Pre-generating shared wallet for nodes..."
SHARED_WALLET="${DATA_ROOT}/shared_wallet.key"
WALLET_PASSWORD="" "$QANTO_BIN" generate-wallet --output "$SHARED_WALLET" >/dev/null

# Extract the wallet address
GENESIS_ADDR=$(WALLET_PASSWORD="" "$GET_ADDRESS_BIN" --wallet "$SHARED_WALLET" | awk '{print $3}')
log_ok "Shared Wallet Address: ${GENESIS_ADDR}"

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_DATA="${DATA_ROOT}/node_${i}"
    NODE_LOG="${LOG_DIR}/node_${i}.log"
    P2P_PORT=$((BASE_P2P_PORT + i))
    RPC_PORT=$((BASE_RPC_PORT + i))

    mkdir -p "${NODE_DATA}"
    # Copy shared wallet key so all nodes share the same validator identity/wallet
    cp "$SHARED_WALLET" "${NODE_DATA}/wallet.key"

    # Generate config for each node
    if [[ $i -eq 0 ]]; then
        PEERS_JSON="[]"
    else
        PEERS_JSON='["/ip4/127.0.0.1/tcp/30300"]'
    fi

    cat > "${NODE_DATA}/config.toml" <<EOF
p2p_address = "/ip4/127.0.0.1/tcp/${P2P_PORT}"
api_address = "127.0.0.1:${RPC_PORT}"
peers = ${PEERS_JSON}
network_id = "qanto-soaktest"
chain_id = 9999
genesis_validator = "${GENESIS_ADDR}"
contract_address = "${GENESIS_ADDR}"
target_block_time = 3000
difficulty = 1
max_amount = 21000000000
use_gpu = false
zk_enabled = false
mining_threads = 1
mining_enabled = true
adaptive_mining_enabled = false
num_chains = 1
mining_chain_id = 0
data_dir = "${NODE_DATA}/db"
db_path = "${NODE_DATA}/db/qanto.db"
wallet_path = "${NODE_DATA}/wallet.key"
p2p_identity_path = "${NODE_DATA}/p2p_identity.key"
log_file_path = "${NODE_LOG}"

[logging]
level = "warn"
enable_block_celebrations = false
celebration_log_level = "info"

[p2p]
heartbeat_interval = 2000
mesh_n_low = 2
mesh_n = 4
mesh_n_high = 8
mesh_outbound_min = 2

[rpc]
address = "127.0.0.1:$((RPC_PORT + 1000))"

[mempool]
parallel_verification_threads = 2
capacity = 1000000
EOF

    log_info "Starting node ${i} on RPC port ${RPC_PORT}..."
    QANTO_OMEGA_DISABLE=true QANTO_BYPASS_CLOCK_DRIFT=true "$QANTO_BIN" --config "${NODE_DATA}/config.toml" > "${NODE_LOG}" 2>&1 &
    local_pid=$!
    echo "$local_pid" > "${DATA_ROOT}/node_${i}.pid"
    PIDS+=("$local_pid")
    log_ok "Node ${i} started (PID: ${local_pid})"
done

# Wait for all nodes to initialize
log_info "Waiting for nodes to initialize..."
sleep 5

# --- Resource monitoring function ---
collect_metrics() {
    local ts
    ts=$(date '+%Y-%m-%dT%H:%M:%S')
    
    for i in $(seq 0 $((NODE_COUNT - 1))); do
        local pid_file="${DATA_ROOT}/node_${i}.pid"
        if [[ ! -f "$pid_file" ]]; then continue; fi
        local pid
        pid=$(cat "$pid_file")
        
        if ! kill -0 "$pid" 2>/dev/null; then
            log_error "Node ${i} (PID ${pid}) is no longer running!"
            echo "${ts},node_${i},DEAD,0,0,0,0" >> "${RESULTS_FILE}"
            continue
        fi

        # RSS in MB (macOS compatible)
        local rss_kb
        rss_kb=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || echo "0")
        local rss_mb=$(( rss_kb / 1024 ))
        
        # File descriptor count
        local fd_count
        if [[ -d "/proc/${pid}/fd" ]]; then
            fd_count=$(ls /proc/${pid}/fd 2>/dev/null | wc -l || echo "0")
        else
            # macOS fallback
            fd_count=$(lsof -p "$pid" 2>/dev/null | wc -l | tr -d ' ' || echo "0")
        fi
        
        # Thread count
        local thread_count
        if [[ -f "/proc/${pid}/status" ]]; then
            thread_count=$(grep Threads /proc/${pid}/status 2>/dev/null | awk '{print $2}' || echo "0")
        else
            # macOS fallback
            thread_count=$(ps -M -p "$pid" 2>/dev/null | wc -l | tr -d ' ' || echo "0")
        fi
        
        # Block count and mempool size from RPC (if available)
        local rpc_port=$((BASE_RPC_PORT + i))
        local blocks="N/A"
        local tips="N/A"
        local mempool="N/A"
        local stats_info
        stats_info=$(curl -s --max-time 2 "http://127.0.0.1:${rpc_port}/stats" 2>/dev/null || echo "")
        if [[ -n "$stats_info" ]]; then
            blocks=$(echo "$stats_info" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('block_count','N/A'))" 2>/dev/null || echo "N/A")
            mempool=$(echo "$stats_info" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('mempool_size','N/A'))" 2>/dev/null || echo "N/A")
        fi
        local dag_info
        dag_info=$(curl -s --max-time 2 "http://127.0.0.1:${rpc_port}/dag" 2>/dev/null || echo "")
        if [[ -n "$dag_info" ]]; then
            tips=$(echo "$dag_info" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tip_count','N/A'))" 2>/dev/null || echo "N/A")
        fi
        
        echo "${ts},node_${i},${rss_mb},${fd_count},${thread_count},${blocks},${tips},${mempool}" >> "${RESULTS_FILE}"
        
        # Alert on threshold violations
        if (( rss_mb > MAX_RSS_MB )); then
            log_warn "Node ${i}: RSS ${rss_mb}MB exceeds threshold ${MAX_RSS_MB}MB"
        fi
        if (( fd_count > MAX_FD_COUNT )); then
            log_warn "Node ${i}: FD count ${fd_count} exceeds threshold ${MAX_FD_COUNT}"
        fi
        if (( thread_count > MAX_THREAD_COUNT )); then
            log_warn "Node ${i}: Thread count ${thread_count} exceeds threshold ${MAX_THREAD_COUNT}"
        fi
    done
}

# --- Traffic shape controller ---
get_current_tps() {
    local elapsed_secs=$1
    local cycle_secs=300  # 5-minute cycles
    
    case "$TRAFFIC_SHAPE" in
        constant)
            echo "$TARGET_TPS"
            ;;
        burst)
            # Alternates between 2x TPS and 0.5x TPS every 5 minutes
            local phase=$(( (elapsed_secs / cycle_secs) % 2 ))
            if [[ $phase -eq 0 ]]; then
                echo $(( TARGET_TPS * 2 ))
            else
                echo $(( TARGET_TPS / 2 ))
            fi
            ;;
        spike)
            # Normal TPS with 10x spikes every 10 minutes for 30 seconds
            local within_cycle=$(( elapsed_secs % 600 ))
            if (( within_cycle >= 570 && within_cycle < 600 )); then
                echo $(( TARGET_TPS * 10 ))
            else
                echo "$TARGET_TPS"
            fi
            ;;
        *)
            echo "$TARGET_TPS"
            ;;
    esac
}

# --- Main soak loop ---
TOTAL_SECS=$(python3 -c "print(int(float('${DURATION_HOURS}') * 3600))")
START_TIME=$(date +%s)
LAST_REPORT=0

# Collect baseline metrics
log_info "Collecting baseline metrics..."
sleep 3
collect_metrics

log_info "Starting soak test (${DURATION_HOURS} hours)..."
log_info "Metrics CSV: ${RESULTS_FILE}"

# Start tx_flood in background (if binary exists)
if [[ -x "$TX_FLOOD_BIN" ]]; then
    log_info "Starting transaction flood at ${TARGET_TPS} TPS..."
    "$TX_FLOOD_BIN" \
        --endpoint "http://127.0.0.1:${BASE_RPC_PORT}" \
        --tps "$TARGET_TPS" \
        --duration "$TOTAL_SECS" \
        --concurrency 16 \
        --batch-size 100 \
        --genesis-address "$GENESIS_ADDR" \
        --assume-genesis-utxo \
        --skip-dag-info \
        --wallet "${DATA_ROOT}/node_0/wallet.key" \
        --chain-id 9999 \
        > "${LOG_DIR}/tx_flood.log" 2>&1 &
    FLOOD_PID=$!
    echo "$FLOOD_PID" > "${DATA_ROOT}/flood.pid"
    log_ok "Transaction flood started (PID: ${FLOOD_PID})"
else
    log_warn "tx_flood binary not found; running without transaction load"
    FLOOD_PID=""
fi

# Monitor loop
while true; do
    NOW=$(date +%s)
    ELAPSED=$(( NOW - START_TIME ))
    
    if (( ELAPSED >= TOTAL_SECS )); then
        log_ok "Soak test duration reached (${DURATION_HOURS}h). Stopping..."
        break
    fi
    
    # Periodic resource collection
    if (( ELAPSED - LAST_REPORT >= REPORT_INTERVAL_SECS )); then
        LAST_REPORT=$ELAPSED
        collect_metrics
        
        # Print progress
        local_pct=$(( (ELAPSED * 100) / TOTAL_SECS ))
        log_info "Progress: ${local_pct}% (${ELAPSED}s / ${TOTAL_SECS}s)"
        
        # Check if all nodes are still alive
        ALL_ALIVE=true
         for i in $(seq 0 $((NODE_COUNT - 1))); do
             pid_file="${DATA_ROOT}/node_${i}.pid"
             if [[ -f "$pid_file" ]]; then
                 pid=$(cat "$pid_file")
                 if ! kill -0 "$pid" 2>/dev/null; then
                     log_error "Node ${i} has crashed! Check ${LOG_DIR}/node_${i}.log"
                     ALL_ALIVE=false
                 fi
             fi
         done
        
        if [[ "$ALL_ALIVE" != "true" ]]; then
            log_error "One or more nodes crashed during soak test."
        fi
        
        # Check if tx_flood is still running
        if [[ -n "${FLOOD_PID:-}" ]] && ! kill -0 "$FLOOD_PID" 2>/dev/null; then
            log_warn "tx_flood process has exited. Check ${LOG_DIR}/tx_flood.log"
            FLOOD_PID=""
        fi
    fi
    
    sleep 5
done

# Run post-soak analyzer before stopping processes
log_info "Running post-soak analyzer..."
ANALYZER_DURATION=$(( $(date +%s) - START_TIME ))
python3 "$(dirname "$0")/analyze_soak.py" \
    --rpc-url "http://127.0.0.1:${BASE_RPC_PORT}" \
    --results-csv "${RESULTS_FILE}" \
    --flood-log "${LOG_DIR}/tx_flood.log" \
    --duration "${ANALYZER_DURATION}" \
    --output "${LOG_DIR}/soak_report.md" || log_warn "Post-soak analyzer failed"

# --- Final report ---
log_info "=== SOAK TEST COMPLETE ==="
log_info ""

# Analyze results
if [[ -f "${RESULTS_FILE}" ]]; then
    log_info "Resource Summary (per node):"
    for i in $(seq 0 $((NODE_COUNT - 1))); do
        # Extract max RSS, max FD, max threads
        MAX_RSS=$(awk -F',' -v node="node_${i}" '$2==node && $3!="DEAD" {if($3+0>max) max=$3+0} END{print max+0}' "${RESULTS_FILE}")
        MAX_FD=$(awk -F',' -v node="node_${i}" '$2==node && $3!="DEAD" {if($4+0>max) max=$4+0} END{print max+0}' "${RESULTS_FILE}")
        MAX_THREADS=$(awk -F',' -v node="node_${i}" '$2==node && $3!="DEAD" {if($5+0>max) max=$5+0} END{print max+0}' "${RESULTS_FILE}")
        
        # First and last RSS for growth check
        FIRST_RSS=$(awk -F',' -v node="node_${i}" '$2==node && $3!="DEAD" {print $3+0; exit}' "${RESULTS_FILE}")
        LAST_RSS=$(awk -F',' -v node="node_${i}" '$2==node && $3!="DEAD" {last=$3+0} END{print last}' "${RESULTS_FILE}")
        
        # Extract RSS at t = 2h (the 120th record with 60s intervals)
        RSS_2H=$(awk -F',' -v node="node_${i}" '
            BEGIN { count=0; val=0 }
            $2==node && $3!="DEAD" {
                count++
                if (count == 120) { val=$3 }
            }
            END { print val+0 }
        ' "${RESULTS_FILE}")
        
        if (( FIRST_RSS > 0 )); then
            GROWTH_PCT=$(( ((LAST_RSS - FIRST_RSS) * 100) / FIRST_RSS ))
        else
            GROWTH_PCT=0
        fi
        
        PLATEAU_GROWTH="N/A"
        PLATEAU_STATUS="OK"
        if (( RSS_2H > 0 )); then
            PLATEAU_GROWTH=$(( ((LAST_RSS - RSS_2H) * 100) / RSS_2H ))
            if (( PLATEAU_GROWTH >= 10 )); then
                PLATEAU_STATUS="FAIL (leak suspected: ${PLATEAU_GROWTH}%)"
            else
                PLATEAU_STATUS="PASS (${PLATEAU_GROWTH}%)"
            fi
        fi
        
        STATUS="${GREEN}PASS${NC}"
        if (( MAX_RSS > MAX_RSS_MB )); then STATUS="${RED}FAIL (RSS)${NC}"; fi
        if (( MAX_FD > MAX_FD_COUNT )); then STATUS="${RED}FAIL (FD)${NC}"; fi
        if [[ "$PLATEAU_STATUS" == FAIL* ]]; then STATUS="${RED}FAIL (Plateau)${NC}"; fi
        if (( GROWTH_PCT > RSS_GROWTH_THRESHOLD )); then STATUS="${YELLOW}WARN (leak?)${NC}"; fi
        
        echo -e "  Node ${i}: Max RSS=${MAX_RSS}MB (growth: ${GROWTH_PCT}%), Plateau Check (2h->24h): ${PLATEAU_STATUS}, Max FD=${MAX_FD}, Max Threads=${MAX_THREADS} [${STATUS}]"
    done
fi

# tx_flood results
if [[ -f "${LOG_DIR}/tx_flood.log" ]]; then
    log_info ""
    log_info "Transaction Flood Summary:"
    grep -E "(Submitted|Average|P50|P90|P99|Peak RSS|Write Throughput)" "${LOG_DIR}/tx_flood.log" | tail -10 || true
fi

log_info ""
log_info "Full metrics: ${RESULTS_FILE}"
log_info "Node logs:    ${LOG_DIR}/"
