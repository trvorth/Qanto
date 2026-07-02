# PUBLIC TESTNET LAUNCH

This guide defines the production rollout procedure for the QANTO public testnet stack on a fresh Ubuntu 22.04 LTS host.

## 1. Prerequisites

### Minimum host specification

- CPU: 8 vCPU minimum, 16 vCPU recommended for sustained validation and RPC traffic
- Memory: 16 GB minimum, 32 GB recommended
- Storage: 500 GB NVMe SSD minimum, 1 TB recommended
- Network: 1 Gbps uplink recommended, static public IPv4 required
- OS: Ubuntu Server 22.04 LTS x86_64

### Required open ports

- `80/tcp`: HTTP challenge path and public edge ingress
- `443/tcp`: HTTPS explorer ingress
- `30303/tcp`: Bootnode P2P ingress
- `30303/udp`: Bootnode discovery ingress
- `50051/tcp`: Public gRPC RPC
- `8081/tcp`: Public HTTP RPC / explorer backend

### Required DNS

- `A` or `AAAA` record for the explorer domain, for example `scan.testnet.qanto.org`
- DNS must already point to the public server before Caddy can complete ACME issuance

## 2. Install Docker And Compose

Run the following on a fresh Ubuntu 22.04 host:

```bash
sudo apt-get update
sudo apt-get install -y ca-certificates curl gnupg lsb-release

sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg

echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
  https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

sudo systemctl enable docker
sudo systemctl start docker
sudo usermod -aG docker "$USER"
newgrp docker
```

Verify:

```bash
docker --version
docker compose version
```

## 3. Fetch The Project

```bash
git clone https://github.com/<your-org>/qanto.git
cd qanto
git checkout <public-testnet-release-branch>
```

## 4. Prepare Host Directories

Create persistent host paths:

```bash
sudo mkdir -p /srv/qanto/config
sudo mkdir -p /srv/qanto/bootnode/{keys,data,logs}
sudo mkdir -p /srv/qanto/rpcnode/{keys,data,logs}
sudo mkdir -p /srv/qanto/secrets
sudo chown -R "$USER":"$USER" /srv/qanto
chmod 700 /srv/qanto/secrets
```

## 5. Prepare Required Secrets

Create the wallet password file referenced by the compose stack:

```bash
printf '%s' '<strong-wallet-password>' > /srv/qanto/secrets/wallet_password.txt
chmod 600 /srv/qanto/secrets/wallet_password.txt
```

## 6. Prepare Node Configuration Files

Create two production TOML configs.

- `/srv/qanto/config/bootnode.toml`
- `/srv/qanto/config/rpcnode.toml`

Use the same `network_id`, `genesis_validator`, and chain parameters in both files.

### Bootnode config requirements

- `api_address = "0.0.0.0:8081"`
- `p2p_address = "/ip4/0.0.0.0/tcp/30303/ws"`
- `rpc.address = "0.0.0.0:50051"` can remain enabled or be omitted if not required
- `mining_enabled = false`
- `wallet_path`, `p2p_identity_path`, `data_dir`, and `db_path` must match the container-mounted paths under `/opt/qanto`

### RPC node config requirements

- `api_address = "0.0.0.0:8081"`
- `rpc.address = "0.0.0.0:50051"`
- `p2p_address` should bind to the desired internal node port, for example `/ip4/0.0.0.0/tcp/30304/ws`
- `peers` should include the bootnode multiaddr
- `mining_enabled = false` for a dedicated public RPC node

If you already validated configs during local cluster testing, reuse the same consensus-critical values and only change bind addresses and file paths.

## 7. Export Compose Environment

Create an env file next to the compose stack:

```bash
cat > /srv/qanto/qanto-public-testnet.env <<'EOF'
QANTO_NODE_TAG=public-testnet
QANTOSCAN_TAG=public-testnet
QANTO_RUST_LOG=info

QANTO_BOOTNODE_P2P_PORT=30303
QANTO_RPC_HTTP_PORT=8081
QANTO_RPC_GRPC_PORT=50051

QANTO_DOMAIN=scan.testnet.qanto.org
ACME_EMAIL=ops@qanto.org

QANTO_WALLET_PASSWORD_FILE=/srv/qanto/secrets/wallet_password.txt
QANTO_BOOTNODE_CONFIG_PATH=/srv/qanto/config/bootnode.toml
QANTO_RPCNODE_CONFIG_PATH=/srv/qanto/config/rpcnode.toml
EOF
```

Load it:

```bash
set -a
source /srv/qanto/qanto-public-testnet.env
set +a
```

## 8. Launch The Full Public Testnet Stack

From the repository root:

```bash
docker compose \
  --env-file /srv/qanto/qanto-public-testnet.env \
  -f DevOps/docker/docker-compose.prod.yml \
  up -d --build
```

This starts:

- `bootnode`
- `rpcnode`
- `qantoscan`
- `caddy` for automatic HTTPS and certificate renewal

## 9. Validation Procedure

### Check container health

```bash
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml ps
```

All services must be `healthy` or `running`.

### Validate bootnode P2P reachability

```bash
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml logs --tail=200 bootnode
```

Confirm:

- bootnode starts without config or wallet errors
- P2P listener binds to `30303`
- remote peers begin connecting after network exposure

### Validate HTTP RPC

```bash
curl -fsS http://127.0.0.1:8081/health
curl -fsS http://127.0.0.1:8081/stats | jq .
curl -fsS http://127.0.0.1:8081/publish-readiness | jq .
```

Required:

- `/health` returns healthy JSON
- `/stats` returns live network fields
- `/publish-readiness` returns `"is_ready": true`

### Validate gRPC RPC

Use `grpcurl` if installed:

```bash
grpcurl -plaintext 127.0.0.1:50051 list
```

### Validate explorer

```bash
curl -I http://127.0.0.1
curl -I https://$QANTO_DOMAIN
curl -fsS https://$QANTO_DOMAIN/healthz
```

Open:

- `https://$QANTO_DOMAIN`

Confirm:

- dashboard loads over HTTPS
- stats cards populate
- latest blocks and latest transactions stream updates

### Validate sync progress

```bash
watch -n 5 'curl -fsS http://127.0.0.1:8081/stats | jq "{block_count, total_transactions, finality_ms, tps_current}"'
```

## 10. Operations

### View logs

```bash
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml logs -f rpcnode
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml logs -f qantoscan
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml logs -f caddy
```

### Restart one service

```bash
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml restart rpcnode
```

### Stop the stack

```bash
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml down
```

### Backup persistent data

```bash
tar -czf bootnode-backup-$(date +%F).tgz -C /var/lib/docker/volumes bootnode_data bootnode_keys
tar -czf rpcnode-backup-$(date +%F).tgz -C /var/lib/docker/volumes rpcnode_data rpcnode_keys
```

### Upgrade to a new release

```bash
git fetch origin
git checkout <new-release-branch-or-tag>
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml pull
docker compose --env-file /srv/qanto/qanto-public-testnet.env -f DevOps/docker/docker-compose.prod.yml up -d --build
```

## 11. Troubleshooting

### Port conflict

Symptom:

- container exits on startup
- `bind: address already in use`

Fix:

```bash
sudo ss -ltnup | grep -E ':(80|443|30303|50051|8081)\b'
```

Stop the conflicting process or remap the published port.

### RPC node never becomes healthy

Symptom:

- `rpcnode` healthcheck keeps failing
- `publish-readiness` reports sync or UTXO issues

Fix:

- verify bootnode is reachable and healthy
- verify bootnode multiaddr is present in `rpcnode.toml`
- confirm `network_id` and `genesis_validator` match on both configs
- inspect `docker compose logs -f rpcnode`

### Explorer returns 502 or blank data

Symptom:

- browser loads shell but API calls fail
- Caddy reports upstream errors

Fix:

- confirm `rpcnode` is healthy on `qanto_app`
- confirm `qantoscan` healthcheck passes
- validate `QANTO_API_UPSTREAM` remains `http://rpcnode:8081`

### TLS certificate not issued

Symptom:

- Caddy stays on HTTP only
- ACME errors in logs

Fix:

- verify DNS already points to the host
- verify `QANTO_DOMAIN` and `ACME_EMAIL` are exported
- ensure ports `80/tcp` and `443/tcp` are reachable from the internet

## 12. Monitoring And Alerting

- Volume warning thresholds are labeled at `80%` warning and `90%` critical on all persistent volumes
- Feed those labels into your Docker or Prometheus volume usage exporter
- Alert on:
  - `rpcnode` healthcheck failures
  - `publish-readiness.is_ready = false`
  - volume usage above threshold labels
  - bootnode peer count collapse
  - explorer HTTPS certificate renewal failure

## 13. Appendix

### Key environment variables

- `QANTO_BOOTNODE_CONFIG_PATH`: host path to the bootnode TOML
- `QANTO_RPCNODE_CONFIG_PATH`: host path to the RPC node TOML
- `QANTO_WALLET_PASSWORD_FILE`: host path to the wallet password secret
- `QANTO_DOMAIN`: public explorer domain
- `ACME_EMAIL`: ACME registration email for automatic TLS
- `QANTO_NODE_TAG`: node container tag
- `QANTOSCAN_TAG`: explorer container tag

### Default ports

- `80/tcp`: Caddy HTTP
- `443/tcp`: Caddy HTTPS
- `30303/tcp`: Bootnode P2P
- `30303/udp`: Bootnode discovery
- `50051/tcp`: RPC gRPC
- `8081/tcp`: RPC HTTP

### Emergency contacts

- Platform owner: `platform-ops@qanto.org`
- Protocol owner: `core@qanto.org`
- Security response: `security@qanto.org`
