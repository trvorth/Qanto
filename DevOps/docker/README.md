# Qanto Docker Deployment

This directory contains the maintained Docker deployment assets for the current Rust workspace layout under `Protocol_Core/`.

## What Is Included

- `docker-compose.yml`: local deployment with optional monitoring profile
- `Dockerfile`: multi-stage build for the `qanto` node binaries
- `config/config.toml`: compose-ready node configuration aligned to the current `Config` schema
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

For the full observability stack:

```bash
docker compose --profile full up --build
```

## Required Files

- `config/config.toml`: mounted read-only into `/opt/qanto/config/config.toml`
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
- The compose setup expects Docker Compose V2 (`docker compose ...`).
- The shipped config is intentionally conservative and should be tailored before public deployment.
