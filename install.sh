#!/usr/bin/env bash
# =============================================================================
# QANTO Network - Validator Node Installer & Onboarding Script
# =============================================================================
set -euo pipefail

# Style definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# ASCII Banner
clear
echo -e "${CYAN}${BOLD}"
echo "  ██████╗  █████╗ ███╗   ██╗████████╗ ██████╗ "
echo " ██╔═══██╗██╔══██╗████╗  ██║╚══██╔══╝██╔═══██╗"
echo " ██║   ██║███████║██╔██╗ ██║   ██║   ██║   ██║"
echo " ██║▄▄ ██║██╔══██║██║╚██╗██║   ██║   ██║   ██║"
echo " ╚██████╔╝██║  ██║██║ ╚████║   ██║   ╚██████╔╝"
echo "  ╚════▀▀ ╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ "
echo -e "         ${GREEN}${BOLD}QANTO Network Node Bootstrapper${NC}"
echo -e "====================================================="

# Host Detection
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

if [ "$ARCH" = "x86_64" ]; then
    ARCH="x86_64"
elif [ "$ARCH" = "arm64" ] || [ "$ARCH" = "aarch64" ]; then
    ARCH="arm64"
else
    echo -e "${RED}Error: Unsupported architecture: $ARCH. Qanto supports x86_64 and arm64.${NC}"
    exit 1
fi

if [ "$OS" != "linux" ] && [ "$OS" != "darwin" ]; then
    echo -e "${RED}Error: Unsupported OS: $OS. Qanto supports Linux and macOS.${NC}"
    exit 1
fi

echo -e "${BLUE}Detected OS:${NC} ${BOLD}$OS${NC} (${BOLD}$ARCH${NC})"

# Installation paths
INSTALL_DIR="/opt/qanto"
CONFIG_PATH="$INSTALL_DIR/config.toml"
DATA_DIR="$INSTALL_DIR/data"
WALLET_PATH="$INSTALL_DIR/wallet.key"
LOG_DIR="$INSTALL_DIR/logs"

# Interactive wizard
echo -e "\n${BOLD}Please choose an installation method:${NC}"
echo -e "  1) ${GREEN}Docker Compose${NC} (Recommended, automated container setup)"
echo -e "  2) ${GREEN}Local System Service${NC} (Native service deployment)"
read -p "Method [1-2]: " -r INSTALL_METHOD

# Validator identity setup
echo -e "\n${BOLD}Node Configuration:${NC}"
read -p "Enter Node Moniker (name): " -r MONIKER
if [ -z "$MONIKER" ]; then
    MONIKER="qanto-validator-$(hostname)"
fi

# -----------------------------------------------------------------------------
# DOCKER INSTALLATION METHOD
# -----------------------------------------------------------------------------
if [ "$INSTALL_METHOD" = "1" ]; then
    echo -e "\n${BLUE}Verifying Docker environment...${NC}"
    if ! command -v docker &>/dev/null; then
        echo -e "${YELLOW}Docker is not installed. Installing Docker...${NC}"
        if [ "$OS" = "linux" ]; then
            curl -fsSL https://get.docker.com | sh
            sudo usermod -aG docker "$USER"
        else
            echo -e "${RED}Please install Docker Desktop for macOS first: https://www.docker.com/products/docker-desktop/${NC}"
            exit 1
        fi
    fi

    if ! command -v docker-compose &>/dev/null && ! docker compose version &>/dev/null; then
        echo -e "${RED}Error: Docker Compose is required. Please install it before proceeding.${NC}"
        exit 1
    fi

    echo -e "${BLUE}Setting up directories in $INSTALL_DIR...${NC}"
    sudo mkdir -p "$INSTALL_DIR" "$DATA_DIR" "$LOG_DIR"
    sudo chown -R "$USER:$GROUP" "$INSTALL_DIR" || sudo chown -R "$USER" "$INSTALL_DIR"

    # Create config.toml
    echo -e "${BLUE}Creating node configuration...${NC}"
    cat <<EOF > "$CONFIG_PATH"
p2p_address = "/ip4/0.0.0.0/tcp/30303/ws"
api_address = "0.0.0.0:8081"
network_id = "qanto-testnet"
target_block_time = 31
difficulty = 1
max_amount = 21000000000
use_gpu = false
zk_enabled = true
mining_threads = 2
mining_enabled = true
adaptive_mining_enabled = false
num_chains = 1
mining_chain_id = 0
data_dir = "./data"
db_path = "./data/qanto.db"
wallet_path = "wallet.key"
p2p_identity_path = "p2p_identity.key"
log_file_path = "./logs/qanto.log"

[rpc]
address = "0.0.0.0:50051"

[logging]
level = "info"
EOF

    # Create docker-compose.yml
    echo -e "${BLUE}Creating docker-compose.yml...${NC}"
    cat <<EOF > "$INSTALL_DIR/docker-compose.yml"
version: '3.8'

services:
  qanto-node:
    image: ghcr.io/qanto-org/qanto:latest
    container_name: qanto-node
    restart: unless-stopped
    ports:
      - "30303:30303"  # P2P Port
      - "8081:8081"    # HTTP API Port
      - "50051:50051"  # gRPC Port
    environment:
      - WALLET_PASSWORD=""
      - RUST_LOG="info"
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
      - ./config.toml:/app/config.toml
      - ./wallet.key:/app/wallet.key
    command: ./qanto-node start --config config.toml --listen 0.0.0.0 --p2p-port 30303 --rpc-port 50051 --mine
EOF

    echo -e "\n${GREEN}Setup complete for Moniker: $MONIKER${NC}"
    echo -e "To launch your node, run:"
    echo -e "  ${BOLD}cd $INSTALL_DIR && docker compose up -d${NC}"
    echo -e "To follow the logs, run:"
    echo -e "  ${BOLD}docker compose logs -f${NC}"

# -----------------------------------------------------------------------------
# LOCAL NATIVE SERVICE INSTALLATION METHOD
# -----------------------------------------------------------------------------
elif [ "$INSTALL_METHOD" = "2" ]; then
    echo -e "\n${BLUE}Verifying dependencies for native build...${NC}"
    if [ "$OS" = "linux" ]; then
        echo -e "Installing build dependencies (requires sudo)..."
        sudo apt-get update && sudo apt-get install -y build-essential pkg-config libssl-dev git curl protobuf-compiler
    elif [ "$OS" = "darwin" ]; then
        if ! command -v brew &>/dev/null; then
            echo -e "${RED}Homebrew is required for dependency management on macOS. Please install it first.${NC}"
            exit 1
        fi
        brew install protobuf openssl pkg-config cmake
    fi

    echo -e "${BLUE}Setting up directories in $INSTALL_DIR...${NC}"
    sudo mkdir -p "$INSTALL_DIR" "$DATA_DIR" "$LOG_DIR"
    sudo chown -R "$USER:$GROUP" "$INSTALL_DIR" || sudo chown -R "$USER" "$INSTALL_DIR"

    # Offer to compile from source or download
    echo -e "\n${BOLD}Node Binary Option:${NC}"
    echo -e "  1) Download prebuilt release binary (Fastest)"
    echo -e "  2) Compile from source (Requires Rust/cargo, ~10-15 mins)"
    read -p "Option [1-2]: " -r BINARY_OPTION

    if [ "$BINARY_OPTION" = "1" ]; then
        RELEASE_TAG="latest"
        echo -e "${BLUE}Downloading Qanto binary for $OS-$ARCH...${NC}"
        # Construct download URL (standardizing file naming to match our GH actions pipeline outputs)
        BINARY_FILE="qanto-$OS-$ARCH"
        if [ "$OS" = "darwin" ]; then
            BINARY_FILE="qanto-macos-$ARCH"
        else
            BINARY_FILE="qanto-linux-$ARCH"
        fi
        
        # Pull release URL
        DOWNLOAD_URL="https://github.com/qanto-org/qanto/releases/latest/download/$BINARY_FILE"
        echo -e "Fetching from: $DOWNLOAD_URL"
        
        if curl -L --fail -o "$INSTALL_DIR/qanto-node" "$DOWNLOAD_URL"; then
            chmod +x "$INSTALL_DIR/qanto-node"
            echo -e "${GREEN}Binary downloaded successfully.${NC}"
        else
            echo -e "${YELLOW}Download failed. Falling back to compilation from source...${NC}"
            BINARY_OPTION="2"
        fi
    fi

    if [ "$BINARY_OPTION" = "2" ]; then
        echo -e "${BLUE}Installing Rust compiler if missing...${NC}"
        if ! command -v cargo &>/dev/null; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            source "$HOME/.cargo/env"
        fi

        echo -e "${BLUE}Cloning and building Qanto from source...${NC}"
        TEMP_BUILD_DIR=$(mktemp -d)
        git clone https://github.com/qanto-org/qanto.git "$TEMP_BUILD_DIR"
        cd "$TEMP_BUILD_DIR"
        cargo build --release --bin qanto
        cp target/release/qanto "$INSTALL_DIR/qanto-node"
        # Also build wallet
        cargo build --release --bin qantowallet
        cp target/release/qantowallet "$INSTALL_DIR/qantowallet"
        chmod +x "$INSTALL_DIR/qanto-node" "$INSTALL_DIR/qantowallet"
        cd - >/dev/null
        rm -rf "$TEMP_BUILD_DIR"
        echo -e "${GREEN}Built and installed successfully.${NC}"
    fi

    # Create config.toml
    echo -e "${BLUE}Creating node configuration...${NC}"
    cat <<EOF > "$CONFIG_PATH"
p2p_address = "/ip4/0.0.0.0/tcp/30303/ws"
api_address = "127.0.0.1:8081"
network_id = "qanto-testnet"
target_block_time = 31
difficulty = 1
max_amount = 21000000000
use_gpu = false
zk_enabled = true
mining_threads = 4
mining_enabled = true
adaptive_mining_enabled = false
num_chains = 1
mining_chain_id = 0
data_dir = "$DATA_DIR"
db_path = "$DATA_DIR/qanto.db"
wallet_path = "$WALLET_PATH"
p2p_identity_path = "$INSTALL_DIR/p2p_identity.key"
log_file_path = "$LOG_DIR/qanto.log"

[rpc]
address = "127.0.0.1:50051"

[logging]
level = "info"
EOF

    # Generate wallet automatically on first startup, or generate now if we want to expose address
    echo -e "${BLUE}Generating validator keys (headlessly)...${NC}"
    WALLET_PASSWORD=""
    # Start node once or run key generation to create key files
    cd "$INSTALL_DIR"
    # Auto-generate wallet file securely if missing by invoking the node once
    WALLET_PASSWORD="" ./qanto-node start --config config.toml --rpc-port 50051 --p2p-port 30303 &
    NODE_PID=$!
    sleep 3
    kill $NODE_PID || true
    cd - >/dev/null

    echo -e "${GREEN}Keys generated successfully.${NC}"

    # Setup systemd service on Linux
    if [ "$OS" = "linux" ]; then
        echo -e "${BLUE}Setting up systemd service (requires sudo)...${NC}"
        sudo tee /etc/systemd/system/qanto-node.service > /dev/null <<EOF
[Unit]
Description=Qanto Validator Node ($MONIKER)
After=network-online.target

[Service]
User=$USER
WorkingDirectory=$INSTALL_DIR
Environment=WALLET_PASSWORD=""
ExecStart=$INSTALL_DIR/qanto-node start --config $CONFIG_PATH --mine
Restart=always
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

        sudo systemctl daemon-reload
        sudo systemctl enable qanto-node
        echo -e "${GREEN}Systemd service registered: qanto-node${NC}"
        echo -e "To start the node, run:"
        echo -e "  ${BOLD}sudo systemctl start qanto-node${NC}"
        echo -e "To view logs, run:"
        echo -e "  ${BOLD}journalctl -u qanto-node -f${NC}"
    else
        # macOS service setup
        echo -e "${BLUE}Creating launchd agent plist for macOS...${NC}"
        PLIST_PATH="$HOME/Library/LaunchAgents/org.qanto.node.plist"
        cat <<EOF > "$PLIST_PATH"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.qanto.node</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/qanto-node</string>
        <string>start</string>
        <string>--config</string>
        <string>$CONFIG_PATH</string>
        <string>--mine</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/stderr.log</string>
</dict>
</plist>
EOF
        launchctl load "$PLIST_PATH" || true
        echo -e "${GREEN}Launchd service loaded for macOS.${NC}"
        echo -e "To start the node, run:"
        echo -e "  ${BOLD}launchctl start org.qanto.node${NC}"
        echo -e "To view logs, run:"
        echo -e "  ${BOLD}tail -f $LOG_DIR/stdout.log $LOG_DIR/stderr.log${NC}"
    fi
fi

# -----------------------------------------------------------------------------
# ONBOARDING GUIDANCE
# -----------------------------------------------------------------------------
echo -e "\n====================================================="
echo -e "${GREEN}${BOLD}Qanto Node Setup Completed Successfully!${NC}"
echo -e "====================================================="
echo -e "\n${BOLD}Follow these steps to complete validator onboarding:${NC}"
echo -e "1. ${BOLD}Verify Node Status${NC}"
echo -e "   Check that your node is syncing headers and blocks correctly via logs."
echo -e "2. ${BOLD}Get your Wallet Address${NC}"
echo -e "   Use the wallet CLI to view your public address:"
echo -e "     ${CYAN}./qantowallet show --wallet $INSTALL_DIR/wallet.key${NC}"
echo -e "3. ${BOLD}Fund your Wallet${NC}"
echo -e "   Acquire at least 10,000 QNTO testnet tokens to cover the validator stake requirement."
echo -e "4. ${BOLD}Stake to Register as an Active Validator${NC}"
echo -e "   Send a Staking Transaction to the Staking Contract address:"
echo -e "   Staking Contract: ${BOLD}9f00000000000000000000000000000000000011000000000000000000000000${NC}"
echo -e "   Run the send command (amount is in base units, where 1 QNTO = 1e9 base units):"
echo -e "     ${CYAN}./qantowallet send --wallet $INSTALL_DIR/wallet.key 9f00000000000000000000000000000000000011000000000000000000000000 10000000000000${NC}"
echo -e "\nNeed help? Join the Qanto Discord or check the docs at Docs/node-operator-guide.md"
