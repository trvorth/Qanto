#!/bin/bash
# Qanto Validator Diagnostics & System Requirements Auditor

OS_TYPE=$(uname)
CPU_CORES=0
RAM_GB=0
DISK_FREE_GB=0
DISK_TYPE="Unknown"
FILE_LIMIT=$(ulimit -n)
DOCKER_INSTALLED=false
RUST_VERSION="none"
CLOCK_OFFSET_MS=0

# 1. CPU Audit
if [ "$OS_TYPE" = "Darwin" ]; then
    CPU_CORES=$(sysctl -n hw.ncpu)
else
    CPU_CORES=$(nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo 2>/dev/null || echo 0)
fi

# 2. RAM Audit
if [ "$OS_TYPE" = "Darwin" ]; then
    RAM_BYTES=$(sysctl -n hw.memsize)
    RAM_GB=$((RAM_BYTES / 1024 / 1024 / 1024))
else
    RAM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
    RAM_GB=$((RAM_KB / 1024 / 1024))
fi

# 3. Disk Space Audit
DISK_FREE_KB=$(df -k . | tail -1 | awk '{print $4}')
DISK_FREE_GB=$((DISK_FREE_KB / 1024 / 1024))

# 4. Disk Type Identification
if [ "$OS_TYPE" = "Darwin" ]; then
    # On macOS, check if it's Solid State
    if diskutil info / | grep -q "Solid State"; then
        DISK_TYPE="SSD/NVMe"
    else
        DISK_TYPE="HDD"
    fi
else
    # On Linux, inspect rotational flag of mount block device
    DEV_NAME=$(df . | tail -1 | awk '{print $1}' | cut -d'/' -f3 | tr -d '0-9')
    if [ -f "/sys/block/${DEV_NAME}/queue/rotational" ]; then
        ROTATIONAL=$(cat "/sys/block/${DEV_NAME}/queue/rotational")
        if [ "$ROTATIONAL" = "0" ]; then
            DISK_TYPE="SSD/NVMe"
        else
            DISK_TYPE="HDD"
        fi
    else
        DISK_TYPE="SSD/NVMe" # Default assumption for modern clouds
    fi
fi

# 5. Software Audits
if command -v docker >/dev/null 2>&1; then
    DOCKER_INSTALLED=true
fi

if command -v rustc >/dev/null 2>&1; then
    RUST_VERSION=$(rustc --version | awk '{print $2}')
fi

# 6. HTTP Clock Drift Check
# Query Cloudflare to get absolute network time from response headers
CF_DATE=$(curl -sI https://1.1.1.1 | grep -i '^date:' | cut -d' ' -f2- | tr -d '\r')
if [ -n "$CF_DATE" ]; then
    if command -v python3 >/dev/null 2>&1; then
        SYSTEM_TIME_MS=$(python3 -c "import time; print(int(time.time() * 1000))")
    elif command -v perl >/dev/null 2>&1; then
        SYSTEM_TIME_MS=$(perl -MTime::HiRes=time -e 'print int(time()*1000)')
    else
        SYSTEM_TIME_MS=$(( $(date +%s) * 1000 ))
    fi
    # Parse HTTP Date format to timestamp in seconds
    if command -v date >/dev/null 2>&1; then
        if [ "$OS_TYPE" = "Darwin" ]; then
            NETWORK_TIME=$(date -j -f "%a, %d %b %Y %T %Z" "$CF_DATE" "+%s" 2>/dev/null)
        else
            NETWORK_TIME=$(date -d "$CF_DATE" "+%s" 2>/dev/null)
        fi
        if [ -n "$NETWORK_TIME" ]; then
            NETWORK_TIME_MS=$((NETWORK_TIME * 1000))
            OFFSET_MS=$((SYSTEM_TIME_MS - NETWORK_TIME_MS))
            # Get absolute value of offset
            CLOCK_OFFSET_MS=${OFFSET_MS#-}
        fi
    fi
fi

# 7. Check overall compatibility
COMPATIBLE=true
REASON=""

if [ "$CPU_CORES" -lt 4 ]; then
    COMPATIBLE=false
    REASON="Insufficient CPU cores (min 4). "
fi

if [ "$RAM_GB" -lt 8 ]; then
    COMPATIBLE=false
    REASON="${REASON}Insufficient RAM (min 8GB). "
fi

if [ "$DISK_FREE_GB" -lt 100 ]; then
    COMPATIBLE=false
    REASON="${REASON}Insufficient free disk space (min 100GB). "
fi

if [ "$DISK_TYPE" = "HDD" ]; then
    COMPATIBLE=false
    REASON="${REASON}SSD or NVMe disk type required. "
fi

# Output JSON
cat <<EOF
{
  "compatible": ${COMPATIBLE},
  "cpu_cores": ${CPU_CORES},
  "ram_gb": ${RAM_GB},
  "disk_free_gb": ${DISK_FREE_GB},
  "disk_type": "${DISK_TYPE}",
  "file_limit": ${FILE_LIMIT},
  "docker_installed": ${DOCKER_INSTALLED},
  "rust_version": "${RUST_VERSION}",
  "clock_offset_ms": ${CLOCK_OFFSET_MS},
  "reason": "${REASON% }"
}
EOF
