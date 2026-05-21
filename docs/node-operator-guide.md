# QANTO Node Operator Guide

## Overview

This guide covers everything you need to run a **QANTO Validator Node** on the Layer-0 network. Validators are responsible for proposing and finalizing blocks, securing the network through the hybrid PoW/DPoS/PoSe consensus, and earning the **2.5 QNTO block reward**.

---

## Hardware Requirements

| Component       | Minimum                | Recommended            |
| --------------- | ---------------------- | ---------------------- |
| **CPU**         | 4-core x86_64 / ARM64 | 8-core x86_64          |
| **RAM**         | 16 GB                  | 32 GB                  |
| **Storage**     | 500 GB NVMe SSD        | 1 TB NVMe SSD          |
| **Network**     | 100 Mbps symmetric     | 1 Gbps symmetric       |
| **OS**          | Ubuntu 22.04 LTS       | Ubuntu 22.04 / 24.04   |

> **Note:** The full QANTO chain state is lightweight by design. With a **1.0-second block time** and a total supply of **21,000,000,000 QNTO**, storage growth is predictable and manageable.

---

## Software Prerequisites

- **Rust** ≥ 1.75 (install via [rustup](https://rustup.rs))
- **Git** ≥ 2.30
- **OpenSSL** ≥ 3.0 (for PQC key generation)
- **Build essentials**: `build-essential`, `pkg-config`, `libssl-dev`

```bash
# Ubuntu / Debian
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Building the Node

```bash
git clone https://github.com/qanto-org/qanto.git
cd qanto
cargo build --release
```

The compiled binary will be located at `./target/release/qanto-node`.

---

## Configuration

Copy the example configuration and edit to taste:

```bash
cp config.toml.example config.toml
```

### Key Configuration Parameters

```toml
[node]
chain_id       = "qanto-mainnet-1"
listen_addr    = "0.0.0.0:26656"
rpc_addr       = "0.0.0.0:26657"
block_time     = "1.0s"             # 1-second block production

[consensus]
mode           = "hybrid"           # PoW + DPoS + PoSe
validator      = true
stake_minimum  = 10000              # 10,000 QNTO minimum stake

[mining]
block_reward   = 2.5                # 2.5 QNTO per block
threads        = 4                  # PoW mining threads
```

---

## Running the Validator

### Start the node

```bash
./target/release/qanto-node --config config.toml
```

### Run as a systemd service (recommended)

```bash
sudo tee /etc/systemd/system/qanto-node.service > /dev/null <<EOF
[Unit]
Description=QANTO Validator Node
After=network-online.target

[Service]
User=qanto
ExecStart=/opt/qanto/target/release/qanto-node --config /opt/qanto/config.toml
Restart=always
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now qanto-node
```

### Verify sync status

```bash
curl -s http://localhost:26657/status | jq '.result.sync_info'
```

---

## Staking & Becoming a Validator

1. Ensure your node is **fully synced** (check `catching_up: false`).
2. Create or import a validator wallet:
   ```bash
   qanto-node keys add validator --keyring-backend file
   ```
3. Submit a validator registration transaction:
   ```bash
   qanto-node tx staking create-validator \
     --amount 10000000000qnto \
     --commission-rate 0.05 \
     --moniker "MyValidator" \
     --from validator
   ```

> **Tokenomics reference:** Total supply is **21,000,000,000 QNTO**. Block rewards are **2.5 QNTO** per block at a **1.0-second block time**.

---

## Monitoring

- **Prometheus metrics**: Exposed on port `26660` by default.
- **Debug HUD**: Available on the Testnet frontend at [qanto.org](https://qanto.org).
- **RPC health check**: `GET /health` on the RPC endpoint.

---

## Security Best Practices

1. Run behind a reverse proxy (nginx/caddy) with TLS.
2. Use a dedicated non-root user (`qanto`).
3. Enable firewall rules — expose only ports `26656` (P2P) and `26657` (RPC).
4. Back up your validator key (`wallet.key`) securely offline.
5. Enable automatic security updates on your OS.

---

## Troubleshooting

| Issue                          | Solution                                                       |
| ------------------------------ | -------------------------------------------------------------- |
| Node stuck at genesis          | Delete `data/` directory and re-sync from peers.               |
| High memory usage              | Increase `LimitNOFILE` in systemd; reduce `--db-cache` size.   |
| Validator jailed               | Unjail via `qanto-node tx slashing unjail --from validator`.   |
| Peer connection failures       | Check firewall rules; ensure port `26656` is open.             |

---

*Last updated: April 2026 — QANTO v1.0.0*
