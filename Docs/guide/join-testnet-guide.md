# QANTO Testnet Join Guide

This guide shows community operators how to connect a clean QANTO follower or validator node to the public testnet from a second machine.

## What The Join Script Does

`DevOps/docker/join-testnet.sh` launches a single `qanto-node` container only.

- No explorer
- No Caddy
- No extra edge services
- Uses the canonical repo `config/genesis.json`
- Injects your public bootnode multiaddr into the node `peers` list
- Exposes the local operator API on `http://127.0.0.1:8081`

The script creates an isolated runtime workspace under `DevOps/docker/.join-testnet/` so it does not collide with the public cloud deployment topology.

## Prerequisites

- Docker Engine or Docker Desktop with `docker compose`
- `python3`
- `openssl`
- A reachable public bootnode websocket multiaddr, for example:

```text
/ip4/203.0.113.10/tcp/30303/ws/p2p/12D3KooW...
```

## Start A Follower Node

From the repository root:

```bash
cd DevOps/docker
chmod +x join-testnet.sh
QANTO_BOOTNODE_ADDR="/ip4/<BOOTNODE_IP>/tcp/30303/ws/p2p/<BOOTNODE_PEER_ID>" \
./join-testnet.sh
```

Default behavior:

- `QANTO_HTTP_PORT=8081`
- `QANTO_GRPC_PORT=50051`
- `QANTO_P2P_PORT=30303`
- `QANTO_MINING_ENABLED=false`

This is the recommended mode for proving that your second machine is physically joining the cloud testnet and syncing blocks over the network.

## Optional Validator / Miner Mode

If you intentionally want the second machine to mine and behave as a validator candidate, enable mining explicitly:

```bash
cd DevOps/docker
QANTO_BOOTNODE_ADDR="/ip4/<BOOTNODE_IP>/tcp/30303/ws/p2p/<BOOTNODE_PEER_ID>" \
QANTO_MINING_ENABLED=true \
./join-testnet.sh
```

Only use validator mode after you understand the wallet, funding, and staking implications for your operator setup.

## Verify Network Attachment

### 1. Check readiness

```bash
curl -fsS http://127.0.0.1:8081/publish-readiness | jq .
```

Expected signals:

- `is_ready` eventually becomes `true`
- `peer_count >= 1`

### 2. Check block sync progress

```bash
curl -fsS http://127.0.0.1:8081/stats | jq '{peer_count,block_count,total_transactions,finality_ms,tps_current}'
```

Expected signals:

- `peer_count >= 1`
- `block_count` keeps increasing as your machine catches up to the cloud bootnode / RPC node height

### 3. Watch live sync

```bash
watch -n 3 'curl -fsS http://127.0.0.1:8081/stats | jq "{peer_count,block_count,total_transactions,finality_ms,tps_current}"'
```

## Follow Logs

The join script writes a dedicated single-service compose file under `DevOps/docker/.join-testnet/`.

To stream logs:

```bash
docker compose -f DevOps/docker/.join-testnet/docker-compose.join-testnet.yml logs -f qanto-node
```

## Key Paths

- Runtime workspace: `DevOps/docker/.join-testnet/`
- Generated config: `DevOps/docker/.join-testnet/config/config.toml`
- Canonical genesis source: `config/genesis.json`

## Success Criteria

Your second machine is successfully attached to the decentralized QANTO testnet when all of the following are true:

- The node process is healthy
- `publish-readiness` reports `is_ready: true`
- `peer_count >= 1`
- `block_count` keeps advancing toward the cloud node height

At that point, you have proven that QANTO is no longer a single-host stack but a real multi-machine distributed network.
