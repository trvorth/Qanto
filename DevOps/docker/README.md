# Qanto Docker Deployment

This directory contains the maintained Docker deployment assets for the current Rust workspace layout under `Protocol_Core/`.

## What Is Included

- `docker-compose.yml`: local deployment with optional monitoring profile
- `docker-compose.prod.yml`: public testnet deployment for bootnode, RPC node, explorer, and edge TLS
- `Dockerfile`: multi-stage build for the `qanto` node binaries
- `config/config.toml`: compose-ready node configuration aligned to the current `Config` schema
- `ubuntu-testnet-ignite.sh`: Ubuntu one-click bootstrapper for the public testnet stack
- `monitoring/`: Prometheus, Grafana, Loki, and Promtail configuration
- `.env.example`: environment template for ports, paths, and resource limits

## Current Port Model

- `8081`: HTTP API, health, metrics, WebSocket, and JSON-RPC `/rpc` endpoints
- `30303`: P2P listener
- `50051`: gRPC RPC server

## Quick Start

```bash
cd DevOps/docker
cp .env.example .env
mkdir -p config secrets
cp config/config.toml config/config.toml.local
printf 'replace-me\n' > secrets/wallet_password.txt

docker compose --profile qanto up --build
```

## Ubuntu One-Click Ignite

For a fresh Ubuntu cloud host, use the maintained ignition path instead of hand-assembling configs:

```bash
cd /path/to/qanto/DevOps/docker
chmod +x ubuntu-testnet-ignite.sh
QANTO_DOMAIN=scan.testnet.example.org \
ACME_EMAIL=ops@example.org \
QANTO_REF=main \
./ubuntu-testnet-ignite.sh
```

The script:

- installs Docker Engine and the Compose plugin if they are missing
- clones or refreshes the repository checkout
- generates a strong wallet password file when one does not already exist
- bootstraps the bootnode once to derive the real validator wallet and peer ID
- rewrites `genesis.json`, `bootnode.toml`, and `rpcnode.toml` into a single consistent testnet state
- relaunches the full stack and verifies `/publish-readiness` plus `block_count >= 1`

For the full observability stack:

```bash
docker compose --profile full up --build
```

## Required Files

- `config/config.toml`: mounted into `/opt/qanto/config/config.toml` and writable because the node persists `local_full_p2p_address`
- `config/genesis.json`: mounted into `/opt/qanto/config/genesis.json` as the canonical genesis source
- `secrets/wallet_password.txt`: mounted as a Docker secret and used to unlock or generate the wallet

## Health And Metrics

- Health endpoint: `http://localhost:8081/health`
- JSON metrics: `http://localhost:8081/metrics`
- Prometheus metrics: `http://localhost:8081/metrics/prometheus`
- Prometheus UI: `http://localhost:9091`
- Grafana UI: `http://localhost:3000`

## Notes

- The image now builds against the actual workspace structure instead of assuming a flat root `src/`.
- Wallet initialization is non-interactive and uses `WALLET_PASSWORD` or `WALLET_PASSWORD_FILE`.
- Public testnet compose requires `QANTO_GENESIS_PATH`, `QANTO_BOOTNODE_CONFIG_PATH`, and `QANTO_RPCNODE_CONFIG_PATH`.
- The compose setup expects Docker Compose V2 (`docker compose ...`).
- The shipped config is intentionally conservative and should be tailored before public deployment.
