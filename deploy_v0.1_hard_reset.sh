#!/bin/bash
set -e

# Configuration
REMOTE_HOST="44.205.8.182"
REMOTE_USER="ubuntu"

# 1. Detect SSH Key
if [ -f "$HOME/.ssh/qanto-key" ]; then
    SSH_KEY="$HOME/.ssh/qanto-key"
elif [ -f "$HOME/.ssh/qanto.pem" ]; then
    SSH_KEY="$HOME/.ssh/qanto.pem"
else
    echo "Error: No SSH key found (checked ~/.ssh/qanto-key and ~/.ssh/qanto.pem)"
    exit 1
fi

echo ">> Mission Status: GO"
echo ">> Target: $REMOTE_USER@$REMOTE_HOST"
echo ">> Key: $SSH_KEY"

# 2. Build Release Binary
echo ">> Building release binary (qanto)..."
cargo build --release --bin qanto

# 3. Package
echo ">> Packaging release artifact..."
# Copy binary to root temporarily to package with config
cp target/release/qanto .
tar -czf release_package.tar.gz qanto config_hyperscale.toml
rm qanto # Cleanup temporary binary copy

# 4. Stop Remote Node
echo ">> Stopping remote node..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no "$REMOTE_USER@$REMOTE_HOST" "pkill -f qanto || true"

# 5. Wipe Remote Data (Critical for tokenomics reset)
echo ">> Wiping remote data (Hard Reset)..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no "$REMOTE_USER@$REMOTE_HOST" "rm -rf data/"

# 6. Upload Package
echo ">> Uploading release package..."
scp -i "$SSH_KEY" -o StrictHostKeyChecking=no release_package.tar.gz "$REMOTE_USER@$REMOTE_HOST:~/"

# 7. Extract and Start
echo ">> extracting and launching..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no "$REMOTE_USER@$REMOTE_HOST" << EOF
    tar -xzf release_package.tar.gz
    chmod +x qanto
    # Launch with nohup and disown
    nohup ./qanto --config config_hyperscale.toml > qanto.log 2>&1 &
    disown
    echo ">> Node launched successfully."
EOF

echo ">> Phase XII Deployment Complete."
