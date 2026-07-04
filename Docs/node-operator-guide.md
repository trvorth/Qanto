# QANTO Node Operator Guide

## Overview

This guide covers everything you need to run a **QANTO Validator Node** on the Layer-0 network. Validators are responsible for proposing and finalizing blocks, securing the network through the hybrid PoW/DPoS/PoSe consensus, and earning the **2.5 QNTO block reward**.

---

## Hardware Requirements

| Component       | Minimum                | Recommended            |
| --------------- | ---------------------- | ---------------------- |
| **CPU**         | 4-core x86_64 / ARM64  | 8-core x86_64          |
| **RAM**         | 16 GB                  | 32 GB                  |
| **Storage**     | 500 GB NVMe SSD        | 1 TB NVMe SSD          |
| **Network**     | 100 Mbps symmetric     | 1 Gbps symmetric       |
| **OS**          | Ubuntu 22.04 LTS       | Ubuntu 22.04 / 24.04   |

---

## Automated Installation (Recommended)

You can automatically set up a Qanto validator node using our bootstrap installer script, which supports both Docker container deployment and native system daemon deployment.

```bash
curl -fsSL https://raw.githubusercontent.com/qanto-org/qanto/main/install.sh | sh
```

The script will automatically detect your OS/architecture, install required dependencies, generate your node configuration, set up keys, and configure services.

---

## Manual Installation

### Software Prerequisites

- **Rust** ≥ 1.77 (install via [rustup](https://rustup.rs))
- **Git** ≥ 2.30
- **OpenSSL** ≥ 3.0 (for key generation and secure channel communications)
- **Protobuf Compiler** (`protobuf-compiler`)
- **Build essentials**: `build-essential`, `pkg-config`, `libssl-dev`

```bash
# Ubuntu / Debian
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev protobuf-compiler git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Building the Node

Clone the repository and compile the binaries:

```bash
git clone https://github.com/qanto-org/qanto.git
cd qanto
cargo build --release
```

The compiled node daemon binary will be located at `./target/release/qanto` and the wallet client binary will be at `./target/release/qantowallet`.

---

## Configuration

Copy the example configuration:

```bash
cp config.toml.example config.toml
```

### Key Configuration Parameters

Open `config.toml` and configure the following parameters:

```toml
p2p_address = "/ip4/0.0.0.0/tcp/30303/ws"      # Address to listen for P2P connections
api_address = "127.0.0.1:8081"             # HTTP/WebSocket API endpoint
network_id = "qanto-testnet"               # Target network identifier
use_gpu = false                            # Enable GPU acceleration (requires opencl3)
zk_enabled = true                          # Enable Zero-Knowledge verification
mining_threads = 4                         # CPU threads assigned to PoW mining
mining_enabled = true                      # Enable validating/mining blocks
data_dir = "./data"                        # Path to save node data
db_path = "./data/qanto.db"                # Path to local RocksDB instance
wallet_path = "wallet.key"                 # File containing encrypted node wallet keys
p2p_identity_path = "p2p_identity.key"     # File containing node P2P identity keys

[rpc]
address = "127.0.0.1:50051"                # gRPC endpoint address
```

---

## Running the Validator

### Option A: Running with Docker Compose

If using Docker, configure your `docker-compose.yml` to map volumes for your keys and data, then run:

```bash
docker compose up -d
```

### Option B: Running Natively

Simply start the node with the `--mine` flag (and specify the config path):

```bash
./target/release/qanto start --config config.toml --mine
```

On first startup, if `wallet.key` does not exist, the node will **automatically generate a new wallet headlessly** and configure it as the validator wallet.

---

## Staking & Becoming a Validator

To prevent spam and secure the consensus, you must register your node as a validator by staking a minimum of **10,000 QNTO** (which equals `10,000,000,000,000` base units since 1 QNTO = 1,000,000,000 base units (9-decimal fixed-point scale)).

### 1. View your Wallet Address
Use the `qantowallet` CLI to extract your validator wallet's public address (ensure you enter the decryption password when prompted):

```bash
./target/release/qantowallet show --wallet wallet.key
```

### 2. Fund your Wallet
Acquire at least 10,000 testnet QNTO tokens from the faucet or an existing account. You can verify your balance using:

```bash
./target/release/qantowallet balance <YOUR_ADDRESS>
```

### 3. Send a Staking Transaction
Register as a validator by sending your stake directly to the Qanto Staking Contract Address:

* Staking Contract Address: `9f00000000000000000000000000000000000011000000000000000000000000`

Run the following command, specifying the amount in base units (10,000 QNTO = `10000000000000`):

```bash
./target/release/qantowallet send --wallet wallet.key 9f00000000000000000000000000000000000011000000000000000000000000 10000000000000
```

Once this transaction is processed and included in a finalized block, your node will be recognized as an active validator in the DPoS cohort.

---

## Monitoring & Telemetry

- **HTTP/JSON Metrics**: Exported on `GET /metrics` on the API port (`8081` by default).
- **gRPC Services**: Exposed on port `50051` by default.
- **WebSocket Feed**: Live event subscription on `ws://127.0.0.1:8081/ws`.

---

## Testnet Coordination & Tooling

We provide a suite of automation scripts in `Scripts/tools/` to simplify node maintenance:

### 1. Diagnostics Auditor (`validator-diagnostics.sh`)
Audits hardware specs (CPU, RAM, free space) and environments to ensure your machine meets requirement thresholds:
```bash
./Scripts/tools/validator-diagnostics.sh
```

### 2. Health Checker (`validator-health-check.sh`)
Fetches node reachability, peer connections, and sync height status:
```bash
./Scripts/tools/validator-health-check.sh [node_url]
```

### 3. Secure State Sync Snapshotter (`validator-snapshot.sh`)
Compresses database and configuration archives. Private validator keys (`wallet.key`, `p2p_identity.key`, `*.pem`, `*.key`) are **automatically excluded** from the archive for safety:
```bash
./Scripts/tools/validator-snapshot.sh [data_dir] [config_path]
```

### 4. Safe Node Upgrader (`validator-upgrade.sh`)
Automates stopping, snapshotting, upgrading binary, and checking sync growth. Performs an **automatic rollback** to restore database state and binary if node check fails:
```bash
./Scripts/tools/validator-upgrade.sh <path-to-new-binary>
```

---

## Slashing & Participation Expectations

### Operator SLA Targets
- **Uptime Target**: Minimum 95% per epoch.
- **Maintenance Notice**: 24 hours notice prior to planned downtime.
- **Upgrade Window**: 48 hours window following a release announcement.

### Slashing Rules
- **Double-Signing Consensus Violation**: 30% stake slash.
- **Bridge Fraud / Collusion**: 100% stake slash.
- **Downtime or Consensus Lag**: Reduction of SAGA reputation score (reducing validator eligibility and block reward share).

---

## Security Best Practices

1. **Firewall**: Expose port `30303` (P2P) to the public web. Keep port `8081` (HTTP API) and `50051` (gRPC RPC) firewalled and only accessible locally or through a secure VPN/proxy.
2. **Key Backup**: Always back up `wallet.key` and `p2p_identity.key` in a secure, offsite, offline location.
3. **Dedicated User**: Run the node under a dedicated, unprivileged system user (e.g., `qanto`).
4. **Limits**: Set standard file descriptor limits (`ulimit -n 65536`) to accommodate high P2P connections.

---

## Troubleshooting

| Issue | Solution |
| --- | --- |
| Peer connection failures | Verify firewall settings; ensure port `30303` is open for incoming TCP/WebSocket traffic. |
| DB write locks / crashes | Ensure only one instance of the `qanto` process is accessing the database directory. |
| Insufficient balance to stake | Check your wallet address balance; make sure to use 9 decimals when specifying amount. |

---

*Last updated: June 2026 — QANTO v1.0.0*
