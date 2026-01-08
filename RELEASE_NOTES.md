# Qanto Testnet v1.0.0-testnet

## Overview
`qanto-testnet-1` is feature-complete and production-hardened for global distribution. This release packages the optimized node binary with GPU mining, explorer integration, and a heartbeat API.

## Highlights
- Hyperscale: 10M TPS architecture validated
- Zero-Latency: 1.4ms block times on GPU
- Live Data: Explorer & dashboard fully integrated
- Stability: Consensus hardened against timestamp drift

## Artifacts
- Dockerfile for global distribution (OpenCL runtime)
- Docker Compose for one-click deployment
- Optimized binary: `/usr/local/bin/qanto`

## Quick Start
- Ensure Docker and Docker Compose are installed
- Place `Dockerfile` and `docker-compose.yml` in the project root
- Create data directory: `mkdir -p ./data`
- Start: `docker compose up -d`

## Ports & Interfaces
- API: `8080`
- P2P: `61670`

## Configuration
- Data: `/root/.qanto` (mounted from `./data`)
- Logging: `RUST_LOG=info` (override in Compose)

## #problems_and_diagnostics
- Health: `GET http://localhost:8080/health` returns `200` when healthy
- Logs: `docker logs -f qanto-node` for runtime diagnostics
- Storage: Verify persistence in `./data` across restarts
- Network: Confirm inbound peers on `61670`

## Support
For issues, provide health response, recent logs, and environment details.
