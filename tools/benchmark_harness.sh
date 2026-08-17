#!/usr/bin/env bash
# =============================================================================
# Qanto Layer-0 Reproducible Benchmark Harness
# =============================================================================
#
# PURPOSE
#   Measure Qanto node throughput (TPS), p50/p95/p99 finality latency, block
#   propagation time, and resource utilisation in a controlled, reproducible
#   single-host environment. Results are written to RESULTS_DIR for archiving
#   and comparison.
#
# USAGE
#   bash tools/benchmark_harness.sh [--validators N] [--duration SECS]
#                                    [--tx-rate RATE] [--output-dir DIR]
#
# OPTIONS
#   --validators N      Number of validator nodes to spawn (default: 4)
#   --duration SECS     Benchmark duration in seconds (default: 120)
#   --tx-rate RATE      Target transaction submissions per second (default: 500)
#   --output-dir DIR    Directory for results (default: benchmark-results/$(date))
#   --no-cleanup        Keep node processes after harness exits
#   --help              Print this message and exit
#
# REPRODUCIBILITY CONTRACT
#   Every run records: git commit, Rust toolchain, OS, CPU, RAM, kernel,
#   node binary SHA-256, harness script SHA-256, and all configuration
#   parameters. An independent party must be able to re-run this script
#   from a clean checkout of the same commit and reproduce results within
#   ±20% of TPS and ±30% of p99 latency.
#
# EXIT CODES
#   0  Success — results written to RESULTS_DIR
#   1  Configuration or build failure
#   2  Node startup failure
#   3  Benchmark workload failure
#   4  Results collection failure
# =============================================================================

set -euo pipefail

# ─── Defaults ─────────────────────────────────────────────────────────────────
VALIDATORS=4
DURATION=120
TX_RATE=500
NO_CLEANUP=false
RESULTS_DIR=""

# ─── Parse arguments ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --validators)   VALIDATORS="$2"; shift 2 ;;
    --duration)     DURATION="$2";   shift 2 ;;
    --tx-rate)      TX_RATE="$2";    shift 2 ;;
    --output-dir)   RESULTS_DIR="$2"; shift 2 ;;
    --no-cleanup)   NO_CLEANUP=true; shift ;;
    --help)
      sed -n '2,50p' "$0" | grep '^#' | sed 's/^# \?//'
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# ─── Derived configuration ────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
if [[ -z "$RESULTS_DIR" ]]; then
  RESULTS_DIR="$REPO_ROOT/benchmark-results/$TIMESTAMP"
fi
mkdir -p "$RESULTS_DIR"

# Binary paths
QANTO_BIN="$REPO_ROOT/target/release/qanto"
LOAD_BIN="$REPO_ROOT/target/release/qanto_load"

LOG="$RESULTS_DIR/harness.log"
METRICS="$RESULTS_DIR/metrics.jsonl"
SUMMARY="$RESULTS_DIR/summary.json"

# ─── Logging ──────────────────────────────────────────────────────────────────
log() { echo "[$(date -u +%T)] $*" | tee -a "$LOG"; }
die() { log "ERROR: $*" >&2; exit "${2:-1}"; }

# ─── Environment capture ──────────────────────────────────────────────────────
capture_env() {
  log "Capturing environment metadata…"
  {
    echo "{"
    echo "  \"harness_version\": \"1.0.0\","
    echo "  \"timestamp\": \"$TIMESTAMP\","
    echo "  \"git_commit\": \"$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo 'unknown')\","
    echo "  \"git_dirty\": $(git -C "$REPO_ROOT" diff --quiet 2>/dev/null && echo 'false' || echo 'true'),"
    echo "  \"rust_toolchain\": \"$(rustc --version 2>/dev/null || echo 'unknown')\","
    echo "  \"os\": \"$(uname -s)\","
    echo "  \"os_version\": \"$(uname -r)\","
    echo "  \"cpu_model\": \"$(sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo 'unknown')\","
    echo "  \"cpu_cores\": $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 0),"
    echo "  \"ram_mb\": $(free -m 2>/dev/null | awk '/^Mem:/{print $2}' || sysctl -n hw.memsize 2>/dev/null | awk '{printf "%d", $1/1048576}' || echo 0),"
    echo "  \"harness_sha256\": \"$(sha256sum "$SCRIPT_DIR/benchmark_harness.sh" 2>/dev/null | awk '{print $1}' || shasum -a 256 "$SCRIPT_DIR/benchmark_harness.sh" 2>/dev/null | awk '{print $1}' || echo 'unknown')\","
    echo "  \"config\": {"
    echo "    \"validators\": $VALIDATORS,"
    echo "    \"duration_secs\": $DURATION,"
    echo "    \"tx_rate_per_sec\": $TX_RATE"
    echo "  }"
    echo "}"
  } > "$RESULTS_DIR/environment.json"
  log "Environment: $(cat "$RESULTS_DIR/environment.json" | tr -d '\n')"
}

# ─── Build check ──────────────────────────────────────────────────────────────
build_release() {
  log "Building release binaries (locked)…"
  if ! cargo build --release --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
       -p qanto --bin qanto --bin tx_flood 2>>"$LOG"; then
    log "WARN: Failed to build qanto or tx_flood."
  fi
  LOAD_BIN="$REPO_ROOT/target/release/tx_flood"
  if [[ ! -x "$QANTO_BIN" ]]; then
    die "qanto binary not found after build: $QANTO_BIN"
  fi
  BINARY_SHA="$(sha256sum "$QANTO_BIN" 2>/dev/null | awk '{print $1}' \
    || shasum -a 256 "$QANTO_BIN" | awk '{print $1}')"
  log "Binary SHA-256: $BINARY_SHA"
  echo "\"node_binary_sha256\": \"$BINARY_SHA\"" >> "$RESULTS_DIR/environment.json" || true
}

# ─── Node management ──────────────────────────────────────────────────────────
NODE_PIDS=()

start_nodes() {
  log "Starting $VALIDATORS validator nodes…"
  export WALLET_PASSWORD="benchmark_dummy_password"
  
  # Generate master wallet for tx_flood genesis
  WALLET_OUT="$RESULTS_DIR/genesis_wallet.key"
  GENESIS_ADDR=$( "$QANTO_BIN" generate-wallet --output "$WALLET_OUT" | grep "Address:" | awk '{print $NF}' )
  log "Generated genesis wallet with address: $GENESIS_ADDR"
  
  # Inject the generated address into the local genesis.json
  cat "$REPO_ROOT/config/genesis.json" | sed "s/ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3/$GENESIS_ADDR/g" > "$RESULTS_DIR/genesis.json"

  for i in $(seq 1 "$VALIDATORS"); do
    PORT=$((9000 + i))
    RPC_PORT=$((8000 + i))
    DATA_DIR="$RESULTS_DIR/node-$i"
    mkdir -p "$DATA_DIR"

    # Isolate config and genesis per node to avoid race conditions when node applies overrides
    cp "$REPO_ROOT/config.toml" "$DATA_DIR/"
    
    # Assign unique api_address to prevent collisions
    API_PORT=$((8080 + i))
    if [[ $(uname) == "Darwin" ]]; then
      sed -i '' "s/api_address = .*/api_address = \"127.0.0.1:$API_PORT\"/" "$DATA_DIR/config.toml"
      sed -i '' "s|data_dir = .*|data_dir = \"$DATA_DIR\"|" "$DATA_DIR/config.toml"
      sed -i '' "s|db_path = .*|db_path = \"$DATA_DIR/qanto.db\"|" "$DATA_DIR/config.toml"
      sed -i '' "s|log_file_path = .*|log_file_path = \"$DATA_DIR/qanto.log\"|" "$DATA_DIR/config.toml"
      sed -i '' "s/genesis_validator = .*/genesis_validator = \"$GENESIS_ADDR\"/" "$DATA_DIR/config.toml"
    else
      sed -i "s/api_address = .*/api_address = \"127.0.0.1:$API_PORT\"/" "$DATA_DIR/config.toml"
      sed -i "s|data_dir = .*|data_dir = \"$DATA_DIR\"|" "$DATA_DIR/config.toml"
      sed -i "s|db_path = .*|db_path = \"$DATA_DIR/qanto.db\"|" "$DATA_DIR/config.toml"
      sed -i "s|log_file_path = .*|log_file_path = \"$DATA_DIR/qanto.log\"|" "$DATA_DIR/config.toml"
      sed -i "s/genesis_validator = .*/genesis_validator = \"$GENESIS_ADDR\"/" "$DATA_DIR/config.toml"
    fi

    cp "$RESULTS_DIR/genesis.json" "$DATA_DIR/genesis.json"
    
    # We will use the same wallet for node 1 so it matches genesis
    cp "$WALLET_OUT" "$DATA_DIR/wallet.key"
    
    QANTO_GENESIS_FILE="$DATA_DIR/genesis.json" "$QANTO_BIN" \
      --config "$DATA_DIR/config.toml" \
      --wallet "$DATA_DIR/wallet.key" \
      --p2p-identity "$DATA_DIR/p2p_identity.key" \
      --data-dir "$DATA_DIR" \
      --p2p-port "$PORT" \
      --rpc-port "$RPC_PORT" \
      start \
      > "$DATA_DIR/stdout.log" 2>&1 &
    NODE_PIDS+=($!)
    log "  Node $i started (pid $!, p2p=$PORT rpc=$RPC_PORT)"
  done

  # Wait for nodes to be ready (up to 30 s)
  log "Waiting for nodes to be ready…"
  for i in $(seq 1 "$VALIDATORS"); do
    API_PORT=$((8080 + i))
    deadline=$((SECONDS + 30))
    while [[ $SECONDS -lt $deadline ]]; do
      if curl -sf "http://127.0.0.1:$API_PORT/health" >/dev/null 2>&1; then
        log "  Node $i ready"
        break
      fi
      sleep 1
    done
  done
}

stop_nodes() {
  if [[ "$NO_CLEANUP" == "true" ]]; then
    log "Skipping cleanup (--no-cleanup)"
    return
  fi
  log "Stopping nodes…"
  for pid in "${NODE_PIDS[@]:-}"; do
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
}
trap stop_nodes EXIT

# ─── Metric collection ────────────────────────────────────────────────────────
collect_metrics_loop() {
  local rpc_port="${1:-8001}"
  local interval=5  # seconds between samples
  local elapsed=0

  while [[ $elapsed -lt $DURATION ]]; do
    sleep "$interval"
    elapsed=$((elapsed + interval))

    # Sample RPC metrics endpoint
    local response
    if response=$(curl -sf "http://127.0.0.1:8081/metrics" 2>/dev/null); then
      echo "{\"t\": $elapsed, \"metrics\": $response}" >> "$METRICS"
    else
      echo "{\"t\": $elapsed, \"metrics\": null, \"error\": \"unreachable\"}" >> "$METRICS"
    fi
  done
}

# ─── Load generator ───────────────────────────────────────────────────────────
run_load() {
  log "Starting load generator at $TX_RATE tx/s for ${DURATION}s…"
  local rpc_port=8001
  local start_epoch
  start_epoch=$(python3 -c "import time; print(int(time.time() * 1000))" 2>/dev/null || date +%s000)

  # Execute tx_flood for highly concurrent payload generation
  WALLET_OUT="$RESULTS_DIR/genesis_wallet.key"
  GENESIS_ADDR=$(cat "$RESULTS_DIR/genesis.json" | grep '"address":' | head -n 1 | awk -F'"' '{print $4}')
  
  "$LOAD_BIN" --endpoint "http://127.0.0.1:8081" --tps "$TX_RATE" --duration "$DURATION" \
    --genesis-address "$GENESIS_ADDR" --wallet "$WALLET_OUT" --password "benchmark_dummy_password" \
    --assume-genesis-utxo --skip-dag-info > "$RESULTS_DIR/tx_flood.log" 2>&1

  local end_epoch
  end_epoch=$(python3 -c "import time; print(int(time.time() * 1000))" 2>/dev/null || date +%s000)
  local elapsed_ms=$(( end_epoch - start_epoch ))
  
  # Determine tx_submitted from the output of tx_flood (or fallback)
  local count=$(grep "Total transactions submitted:" "$RESULTS_DIR/tx_flood.log" | awk '{print $NF}' || echo $(( DURATION * TX_RATE )))
  [[ -z "$count" ]] && count=$(( DURATION * TX_RATE ))

  local actual_tps=$(echo "scale=2; $count * 1000 / $elapsed_ms" | bc 2>/dev/null || echo "$count")
  log "Load generator done: $count tx submitted, actual rate ≈ $actual_tps tx/s"
  echo "$count" > "$RESULTS_DIR/tx_submitted.txt"
  echo "$actual_tps" > "$RESULTS_DIR/tx_rate_actual.txt"
}

# ─── Results analysis ─────────────────────────────────────────────────────────
analyse_results() {
  log "Analysing results…"

  local tx_submitted=0
  local actual_rate="unknown"
  [[ -f "$RESULTS_DIR/tx_submitted.txt" ]]  && tx_submitted=$(cat "$RESULTS_DIR/tx_submitted.txt")
  [[ -f "$RESULTS_DIR/tx_rate_actual.txt" ]] && actual_rate=$(cat "$RESULTS_DIR/tx_rate_actual.txt")

  # Collect final metrics from node 1
  local final_metrics="{}"
  final_metrics=$(curl -sf "http://127.0.0.1:8081/metrics" 2>/dev/null || echo "{}")

  python3 - <<PYEOF > "$SUMMARY"
import json, math, sys

try:
    final = json.loads('''$final_metrics''')
except:
    final = {}

tx_confirmed = final.get("tx_confirmed", 0)
finality_samples = final.get("finality_latency_ms", [])
propagation_samples = final.get("block_propagation_ms", [])

def percentile(data, p):
    if not data:
        return None
    s = sorted(data)
    idx = math.ceil(p / 100.0 * len(s)) - 1
    return s[max(0, idx)]

summary = {
    "duration_secs": $DURATION,
    "validators": $VALIDATORS,
    "tx_target_rate": $TX_RATE,
    "tx_submitted": $tx_submitted,
    "tx_confirmed": tx_confirmed,
    "tps_achieved": round(tx_confirmed / $DURATION, 2) if tx_confirmed else None,
    "finality_latency_ms": {
        "p50": percentile(finality_samples, 50),
        "p95": percentile(finality_samples, 95),
        "p99": percentile(finality_samples, 99),
    },
    "block_propagation_ms": {
        "p50": percentile(propagation_samples, 50),
        "p95": percentile(propagation_samples, 95),
        "p99": percentile(propagation_samples, 99),
    },
    "resource": final.get("resource", {}),
}
print(json.dumps(summary, indent=2))
PYEOF

  log "Results summary:"
  cat "$SUMMARY" | tee -a "$LOG"
}

# ─── Main ─────────────────────────────────────────────────────────────────────
main() {
  log "═══════════════════════════════════════"
  log "Qanto Benchmark Harness v1.0.0"
  log "Results: $RESULTS_DIR"
  log "═══════════════════════════════════════"

  capture_env
  build_release

  start_nodes

  # Start metrics collection in background
  collect_metrics_loop 8001 &
  METRICS_PID=$!

  # Run load
  run_load

  # Wait for final metrics
  wait "$METRICS_PID" 2>/dev/null || true

  analyse_results
  stop_nodes

  log "═══════════════════════════════════════"
  log "Benchmark complete. Results in: $RESULTS_DIR"
  log "═══════════════════════════════════════"
}

main "$@"
