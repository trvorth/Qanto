# QANTO Node Upgrade & Version Policy

This document outlines the upgrade procedures, version compatibility policy, and emergency patch processes for Qanto validator nodes. Following these instructions ensures smooth network transitions and prevents consensus divergence or validator downtime.

---

## 1. Versioning Scheme

Qanto follows a strict Semantic Versioning format (`MAJOR.MINOR.PATCH`):
- **MAJOR**: Hard fork upgrades. Introduces backward-incompatible consensus or transaction rule changes. Requires coordinated network upgrades at a specific epoch.
- **MINOR**: Soft fork upgrades. Introduces backward-compatible consensus additions or optimization updates.
- **PATCH**: Safe hotfixes and client-only optimizations (e.g., RPC improvements, logging, database tuning). Backward-compatible and can be applied individually at any time.

---

## 2. Standard Upgrade Procedure

For standard minor and patch updates (e.g., upgrading from `v0.9.0` to `v0.9.1`):

### Step 1: Backup Keys & Database
Always back up your validator keys and state before performing an upgrade:
```bash
# Backup key files (copy to a secure offline location if not already done)
cp /opt/qanto/wallet.key /secure/backup/wallet.key
cp /opt/qanto/p2p_identity.key /secure/backup/p2p_identity.key

# (Optional) Snapshot state database (RocksDB)
tar -czf /opt/qanto/backup-qanto-db-$(date +%F).tar.gz -C /opt/qanto/data qanto.db
```

### Step 2: Download or Build the New Binary
#### Using Automated Script:
The bootstrap script automatically handles fetching the latest release binary:
```bash
curl -fsSL https://raw.githubusercontent.com/qanto-org/qanto/main/install.sh | sh
```
#### Manual Binary Retrieval:
Stop the active service and replace the binary:
```bash
# Stop running node
sudo systemctl stop qanto-node  # On Linux
# or: launchctl unload ~/Library/LaunchAgents/org.qanto.node.plist  # On macOS

# Fetch new binary
curl -L -o /opt/qanto/qanto-node.new https://github.com/qanto-org/qanto/releases/download/v0.9.1/qanto-linux-x86_64
chmod +x /opt/qanto/qanto-node.new

# Safely replace active binary
mv /opt/qanto/qanto-node.new /opt/qanto/qanto-node
```

### Step 3: Verify & Restart
Start the service and trace the logs to ensure successful startup and block synchronization:
```bash
# Start running node
sudo systemctl start qanto-node
# or: launchctl load ~/Library/LaunchAgents/org.qanto.node.plist

# Monitor startup logs
journalctl -u qanto-node -f -n 100
```
Verify the active version via RPC:
```bash
curl -s http://localhost:8081/info | jq '.version'
```

---

## 3. Coordinated Hard Fork (Epoch-Bound Upgrades)

When introducing a hard fork, the network must upgrade simultaneously. Qanto coordinates hard forks via the SAGA AI and Epoch Rules.

1. **Scheduling**: The upgrade epoch is announced in advance on official channels and proposed via a Governance Proposal (`UpdateRule`).
2. **Pre-release**: Version `vX.Y.Z` containing the hard-fork code is released 7–14 days prior to the hard-fork epoch.
3. **Execution**:
   - Validators must upgrade their node binaries before the scheduled epoch.
   - The new consensus code remains dormant until the `current_epoch` matches the fork target epoch.
   - At the target epoch boundary, the consensus engine automatically activates the new rules.

> **Warning**: Validators failing to upgrade before the hard-fork epoch will automatically diverge onto a split chain (non-consensus tips) and will be jailed/slashed for non-participation.

---

## 4. Rollback Recovery Policy

If an upgrade triggers unexpected consensus instability or performance degradation:

1. **Stop Node**: Immediately stop the node to prevent database corruption.
   ```bash
   sudo systemctl stop qanto-node
   ```
2. **Revert Binary**: Restore the previous stable binary version.
3. **Verify DB Integrity**:
   - If the database was upgraded to a new schema version, restore the database backup taken in Step 1.
   ```bash
   rm -rf /opt/qanto/data/qanto.db
   tar -xzf /opt/qanto/backup-qanto-db-*.tar.gz -C /opt/qanto/data/
   ```
4. **Restart**: Start the stable version and monitor synchronization.

---

## 5. Emergency Hotfix Procedure

In the event of a critical security vulnerability or zero-day exploit:

1. **Security Announcement**: A signed security advisory is published on the official repository.
2. **Hotfix Release**: An emergency patch version (e.g., `v0.9.1-hotfix`) is released immediately.
3. **Automated Alerts**: Nodes monitoring `/stats` and telemetry will show a critical security notice on their logs/telemetry feeds.
4. **Fast-path Upgrade**: Validators are expected to apply the hotfix within 12 hours of release.
