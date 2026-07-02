#!/bin/bash
# Qanto Validator Secure State Sync Snapshot Archiver

DATA_DIR=${1:-"data"}
CONFIG_FILE=${2:-"config.toml"}
TIMESTAMP=$(date +%s)
ARCHIVE_NAME="qanto-snapshot-${TIMESTAMP}.tar.gz"

echo "=== Qanto Secure State Sync Snapshot Archiver ==="

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "ERROR: Configuration file '${CONFIG_FILE}' not found."
    exit 1
fi

# Resolve API address from config file
API_ADDR=$(grep -E '^\s*api_address\s*=' "$CONFIG_FILE" | head -n 1 | cut -d'"' -f2)
if [[ "$API_ADDR" == 0.0.0.0* ]]; then
    API_ADDR="127.0.0.1:${API_ADDR#0.0.0.0:}"
fi
if [ -z "$API_ADDR" ]; then
    API_ADDR="127.0.0.1:8081" # Default fallback
fi

# Check if node is running by querying info endpoint
NODE_ONLINE=0
INFO_RES=$(curl -s -m 2 "http://${API_ADDR}/info")
if [ $? -eq 0 ] && [ ! -z "$INFO_RES" ]; then
    NODE_ONLINE=1
fi

IS_CHECKPOINT=0
TAR_TARGET="$DATA_DIR"

if [ "$NODE_ONLINE" -eq 1 ]; then
    echo "Node is online. Triggering consistent database checkpoint via HTTP API..."
    CHECKPOINT_RES=$(curl -s -X POST "http://${API_ADDR}/db/checkpoint")
    STATUS=$(echo "$CHECKPOINT_RES" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
    CHECKPOINT_DIR=$(echo "$CHECKPOINT_RES" | grep -o '"checkpoint_dir":"[^"]*"' | cut -d'"' -f4)

    if [ "$STATUS" = "success" ] && [ -d "$CHECKPOINT_DIR" ]; then
        echo "Checkpoint successfully created at: ${CHECKPOINT_DIR}"
        TMP_ARCHIVE_DIR="tmp_archive_${TIMESTAMP}"
        mkdir -p "${TMP_ARCHIVE_DIR}/data"
        cp -r "${CHECKPOINT_DIR}/"* "${TMP_ARCHIVE_DIR}/data/"
        TAR_TARGET="${TMP_ARCHIVE_DIR}/data"
        IS_CHECKPOINT=1
    else
        echo "ERROR: Failed to create database checkpoint via API. Response: ${CHECKPOINT_RES}"
        echo "Falling back to archiving live data directory directly (warning: may be inconsistent)..."
        # Check if data directory exists
        if [ ! -d "$DATA_DIR" ]; then
            echo "ERROR: Data directory '${DATA_DIR}' not found."
            exit 1
        fi
    fi
else
    echo "Node is offline. Archiving live data directory directly..."
    # Check if data directory exists
    if [ ! -d "$DATA_DIR" ]; then
        echo "ERROR: Data directory '${DATA_DIR}' not found."
        exit 1
    fi
fi

echo "Creating secure snapshot archive '${ARCHIVE_NAME}'..."
echo "Warning: Excluded files will not be exported (private keys, certs)."

# Build and execute tar archive command with safety patterns
# Excludes wallet.key, p2p_identity.key, any *.key files, and *.pem certs
tar \
  --exclude="*wallet.key*" \
  --exclude="*p2p_identity.key*" \
  --exclude="*.pem" \
  --exclude="*.key" \
  --exclude="*.tmp" \
  -czf "$ARCHIVE_NAME" "$TAR_TARGET" "$CONFIG_FILE"

TAR_STATUS=$?

# Clean up temporary folders if checkpoint was used
if [ "$IS_CHECKPOINT" -eq 1 ]; then
    rm -rf "$TMP_ARCHIVE_DIR"
    rm -rf "$CHECKPOINT_DIR"
fi

if [ $TAR_STATUS -eq 0 ]; then
    echo "SUCCESS: Archive created successfully: ${ARCHIVE_NAME}"
    echo "Verify contents using: tar -tf ${ARCHIVE_NAME}"
else
    echo "ERROR: Failed to create snapshot archive."
    exit 1
fi
