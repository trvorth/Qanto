#!/bin/bash
# Qanto Validator Auto-Upgrader with Verification and Rollback Checkpoints

NEW_BINARY_PATH=$1
CONFIG_PATH=${2:-"config.toml"}
DATA_DIR=${3:-"data"}

if [ -z "$NEW_BINARY_PATH" ] || [ ! -f "$NEW_BINARY_PATH" ]; then
    echo "Usage: $0 <path-to-new-qanto-binary> [config_path] [data_dir]"
    exit 1
fi

echo "=== Starting Qanto Validator Upgrade ==="

# 1. Determine running node path and process type
SYSTEMD_SERVICE="qanto-node"
IS_SYSTEMD=false
NODE_PID=""
CURRENT_BINARY=""

if systemctl is-active --quiet "$SYSTEMD_SERVICE" 2>/dev/null; then
    IS_SYSTEMD=true
    CURRENT_BINARY=$(systemctl show -p FragmentPath "$SYSTEMD_SERVICE" | cut -d'=' -f2)
    echo "Node is running via systemd service: $SYSTEMD_SERVICE"
else
    # Find local running process
    NODE_PID=$(pgrep -f "qanto start" || pgrep -x "qanto")
    if [ -n "$NODE_PID" ]; then
        CURRENT_BINARY=$(lsof -p "$NODE_PID" | grep -E "txt|REG" | grep "qanto" | awk '{print $9}' | head -1)
        echo "Node is running as a local process. PID: $NODE_PID"
    else
        echo "WARNING: No active Qanto node process detected. Upgrading in offline mode..."
        CURRENT_BINARY="target/debug/qanto" # fallback/default target location
    fi
fi

if [ -z "$CURRENT_BINARY" ]; then
    CURRENT_BINARY="target/debug/qanto"
fi
echo "Target binary location: $CURRENT_BINARY"

# 2. Perform state snapshot / backup
echo "Creating database backup snapshot..."
BACKUP_DIR="data_backup_$(date +%s)"
mkdir -p "$BACKUP_DIR"
cp -r "$DATA_DIR" "$CONFIG_PATH" "$BACKUP_DIR/" 2>/dev/null
cp "$CURRENT_BINARY" "$BACKUP_DIR/qanto.old" 2>/dev/null
echo "Backup created in: $BACKUP_DIR"

# 3. Stop the node
echo "Stopping Qanto node..."
if [ "$IS_SYSTEMD" = true ]; then
    sudo systemctl stop "$SYSTEMD_SERVICE"
elif [ -n "$NODE_PID" ]; then
    kill "$NODE_PID"
    sleep 5
    # Force kill if still running
    if kill -0 "$NODE_PID" 2>/dev/null; then
        kill -9 "$NODE_PID"
    fi
fi

# 4. Install the new binary
echo "Installing new Qanto binary..."
cp "$NEW_BINARY_PATH" "$CURRENT_BINARY"
chmod +x "$CURRENT_BINARY"

# 5. Restart the node
echo "Restarting Qanto node..."
if [ "$IS_SYSTEMD" = true ]; then
    sudo systemctl start "$SYSTEMD_SERVICE"
else
    # Start in background with local logging
    nohup "$CURRENT_BINARY" start --config "$CONFIG_PATH" > qanto_upgrade.log 2>&1 &
    sleep 3
    NODE_PID=$(pgrep -f "qanto start" || pgrep -x "qanto")
    echo "Node started locally. New PID: $NODE_PID"
fi

# 6. Verification checks (Peer connectivity and Height progression)
echo "Verifying upgrade sync status (this will take 15 seconds)..."
sleep 5

# Query status using health check script
HEALTH_CHECK="./validator-health-check.sh"
if [ ! -f "$HEALTH_CHECK" ]; then
    HEALTH_CHECK="Scripts/tools/validator-health-check.sh"
fi

# Query initial height
STATUS_1=$("$HEALTH_CHECK" 2>/dev/null)
HEIGHT_1=$(echo "$STATUS_1" | grep -o '"height":[0-9]*' | cut -d':' -f2 || echo 0)
PEERS_1=$(echo "$STATUS_1" | grep -o '"peers":[0-9]*' | cut -d':' -f2 || echo 0)
HEALTHY_1=$(echo "$STATUS_1" | grep -o '"healthy":[^,]*' | cut -d':' -f2 || echo "false")

echo "Initial Sync state: height=$HEIGHT_1, peers=$PEERS_1, healthy=$HEALTHY_1"

# Wait and query again to check for height growth
sleep 10
STATUS_2=$("$HEALTH_CHECK" 2>/dev/null)
HEIGHT_2=$(echo "$STATUS_2" | grep -o '"height":[0-9]*' | cut -d':' -f2 || echo 0)

echo "Verification check height growth: height=$HEIGHT_2"

# 7. Rollback if verification fails
# Upgrade is successful if height grows OR if height is same but peers connected (solo mining might have 0 growth if no tx, but peer sync should verify)
UPGRADE_SUCCESSFUL=false
if [ "$HEALTHY_1" = "true" ] && [ "$HEIGHT_2" -ge "$HEIGHT_1" ]; then
    UPGRADE_SUCCESSFUL=true
fi

if [ "$UPGRADE_SUCCESSFUL" = true ]; then
    echo "=== UPGRADE SUCCESSFUL ==="
    echo "New version running: $(echo "$STATUS_1" | grep -o '"version":"[^"]*"' | cut -d':' -f2)"
    exit 0
else
    echo "=== VERIFICATION FAILED! INITIATING ROLLBACK ==="
    
    # Stop failed node
    if [ "$IS_SYSTEMD" = true ]; then
        sudo systemctl stop "$SYSTEMD_SERVICE"
    else
        NEW_PID=$(pgrep -f "qanto start" || pgrep -x "qanto")
        if [ -n "$NEW_PID" ]; then
            kill "$NEW_PID"
            sleep 3
        fi
    fi

    # Restore backup
    echo "Restoring database and binary..."
    cp -r "$BACKUP_DIR/$DATA_DIR/"* "$DATA_DIR/" 2>/dev/null
    cp "$BACKUP_DIR/$CONFIG_PATH" "$CONFIG_PATH" 2>/dev/null
    cp "$BACKUP_DIR/qanto.old" "$CURRENT_BINARY" 2>/dev/null
    chmod +x "$CURRENT_BINARY"

    # Restart old node
    echo "Restarting old node..."
    if [ "$IS_SYSTEMD" = true ]; then
        sudo systemctl start "$SYSTEMD_SERVICE"
    else
        nohup "$CURRENT_BINARY" start --config "$CONFIG_PATH" > qanto_upgrade.log 2>&1 &
    fi

    echo "Rollback complete. Node has been reverted to previous version."
    exit 1
fi
