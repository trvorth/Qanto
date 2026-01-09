#!/bin/bash
set -e

echo "=== Qanto Public Beta v0.1.0 Launch Sequence ==="

# 1. Check SSH Key
if [ ! -f ~/.ssh/qanto-key ]; then
    echo "Error: SSH key ~/.ssh/qanto-key not found!"
    exit 1
fi
echo "[OK] SSH Key found."

# 2. Build Release Binary
echo "Building release binary..."
cargo build --release --bin qanto
echo "[OK] Build complete."

# 3. Execute Deployment
echo "Deploying to Hyperscale Cluster..."
./deploy_hyperscale.sh

# 4. Print Connection Command
echo ""
echo "=== Launch Complete ==="
echo "Users can now join the network with:"
echo ""
echo "qanto --bootnodes /ip4/54.81.139.232/tcp/30333/p2p/bb734ccc0d779a46968d625f4235a6a3ac4973dc1bdd3694cc5c00a6b1a86452 --p2p-port 30333"
echo ""
