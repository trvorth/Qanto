# Qanto Cohort A Operational Launch Plan

This document establishes the official Go/No-Go gates, daily operations checklist, incident severity response procedures, drill protocols, and success/exit criteria for the Cohort A Genesis Testnet.

---

## 1. Go / No-Go Decision Gates

Before Genesis activation, the Cohort A network must pass all validation gates. Any `FAIL` on any gate will immediately halt the launch process.

### Go / No-Go Decision Table

| Gate | Requirement | Description | Status |
| :--- | :--- | :--- | :--- |
| **Gate 1: Build Integrity** | 100% Validator Build Success | All validators must report successful `cargo check` and `cargo test` passing on target platforms (Linux x86_64, Linux ARM64, and Apple Silicon/macOS). | `PENDING` |
| **Gate 2: Connectivity** | 100% Peer Reachability | All validators must run diagnostics, report reachable REST `/stats` and P2P endpoints, and pass NTP clock sanity checks. | `PENDING` |
| **Gate 3: Telemetry** | NOC Telemetry Visibility | Metrics must be reporting correctly to the global telemetry and Prometheus endpoints, visible in the NOC dashboard. | `PENDING` |
| **Gate 4: Upgrade Test** | Dry-Run Upgrade Success | Dry-run of the node upgrade procedure must pass successfully on a local/staging instance. | `PENDING` |
| **Gate 5: Snapshot Test** | State Restore Validation | Successful validation of the compressed database snapshot restore procedure on staging. | `PENDING` |

> [!IMPORTANT]
> **Decision Rule**: Every single checklist item must resolve to `PASS` to proceed to Genesis block generation. Any `FAIL` = **No Launch**.

---

## 2. Genesis Ceremony & Topology

To minimize operational complexity while maintaining high infrastructure diversity, the Genesis Ceremony will boot with exactly **5 whitelisted validators**:

| Node | Location / Host | Infrastructure Type | Port Configuration |
| :--- | :--- | :--- | :--- |
| **Core-1** | Oracle Cloud | Enterprise Cloud VM (ARM64) | REST: 8080 / P2P: 8000 |
| **Core-2** | AWS (us-east-1) | Enterprise Cloud VM (x86_64) | REST: 8080 / P2P: 8000 |
| **Core-3** | Hetzner | Dedicated Bare-Metal Server (x86_64) | REST: 8080 / P2P: 8000 |
| **Operator-1** | Residential ISP | Local Home Server (x86_64) behind NAT | REST: 8080 / P2P: 8000 |
| **Operator-2** | macOS Home Office | Apple Silicon (M-series) Mac Mini | REST: 8080 / P2P: 8000 |

---

## 3. Daily Operations Checklist & Monitoring

Operators must monitor the network daily and record key statistics. This raw log will compile into the final **Genesis Testnet Report v1**.

### Telemetry Parameters to Inspect Daily

* **Consensus**:
  - `fork_events_total`: Cumulative count of canonical fork conflicts (expected: `0`).
  - `max_fork_depth`: Highest depth of any resolved fork branch (expected: `0`).
  - `height_divergence_events`: Tip height mismatches (expected: `0` under normal operation).
  - `chain_reconciliation_events`: Cumulative count of parent tips successfully merged (expected: `0` under normal operation).
* **Networking (P2P)**:
  - `unique_peers_seen_24h`: Count of unique connected peer addresses.
  - `peer_disconnects_24h`: Peer connection churn.
  - `peer_session_duration_p50` & `peer_session_duration_p95`: Median and tail-end session duration percentiles.
  - `tip_count` & `tip_count_max_24h`: Active tips and peak concurrent tips.
* **State Growth**:
  - `database_size_bytes`: Raw RocksDB disk footprint.
  - `logical_utxo_set_size_bytes`: Active UTXO space.
  - `state_amplification_ratio`: Database size vs. raw transaction payload size.

### Daily Log Format

```text
Date: YYYY-MM-DD
Network Height: [Block Height]
Validator Count: [Active Validators]
Fork Events: [Count]
Max Fork Depth: [Depth]
Divergence Events: [Count]
Reconciliation Events: [Count]
Unique Peers 24h: [Count]
Peer Disconnects 24h: [Count]
P50 Session Duration: [Seconds]
P95 Session Duration: [Seconds]
Active Tip Count: [Current]
Max Tip Count 24h: [Peak]
DB Size (MB): [Size]
UTXO Set Size (MB): [Size]
State Amplification Ratio: [Ratio]
Consensus Status: [Healthy / Degraded / Halted]
Notes: [Any alerts, restarts, or warnings]
```

---

## 4. Incident Severity & Emergency Response

### SEV-1: Critical Consensus Failure
* **Definition**: Chain halt (no blocks for > 5 min), unrecoverable fork (permanent split), or corrupted state database.
* **Response Protocol**:
  1. Trigger emergency operator page.
  2. Coordinate validators on the secure communication channel.
  3. Prepare and deploy emergency patch or initiate database state rollback using the genesis snapshot.

### SEV-2: Network Degradation
* **Definition**: Elevated height divergence, excessive peer disconnects/churn, or sync delays (> 1 min behind tips).
* **Response Protocol**:
  1. NOC team initiates debug diagnostics analysis.
  2. Perform root-cause analysis (NTP clock drift, P2P NAT issues, or validator resource limits).
  3. Publish mitigation strategy or rolling restarts.

### SEV-3: Individual Validator Issues
* **Definition**: Offline node, local telemetry outage, or minor hardware failure not affecting global consensus liveness.
* **Response Protocol**:
  1. Core network remains unaffected; local operator performs node recovery.
  2. Inspect local logs, verify connectivity, and restart daemon.

---

## 5. Operations Drill Protocols

### Drill A: Upgrade Drill (Day 7)
* **Goal**: Upgrade the live network to `v0.1.1-testnet` without consensus failure or state corruption.
* **Execution**:
  1. Deploy the new build to one validator node first (e.g., Core-1).
  2. Monitor `fork_events_total`, `height_divergence_events`, and `tip_count` for 30–60 minutes.
  3. Verify that the upgraded node successfully processes new blocks and remains in consensus.
  4. Sequentially roll out the upgrade to the remaining 4 validators.
* **Success Criteria**: 100% validator upgrade success with 0 consensus failures.

### Drill B: Partition Drill (Week 3)
* **Goal**: Test the hybrid consensus's resilience to major network partitions and its ability to self-heal.
* **Execution**:
  1. Intentionally isolate 2 validators (Core-2, Operator-1) from the remaining 3 validators (Core-1, Core-3, Operator-2) using firewall rules.
  2. Observe and record independent tip count growth and height divergence on both sides.
  3. Remove partition rules after 2 hours to allow network reconnection.
  4. Monitor reconciliation.
* **Expected Result**:
  - During partition: `tip_count` and divergence spike.
  - After reconnect: `chain_reconciliation_events > 0` as the network merges tips. `max_fork_depth` remains bounded, and the chain converges automatically.

### Drill C: Snapshot Restore Drill (Week 4)
* **Goal**: Validate that new/existing nodes can quickly sync and restore state from compressed snapshots.
* **Targets**:
  - **Restore Time**: < 15 minutes.
  - **Rejoin Network**: < 10 minutes.
  - **State Validation**: 100% correct UTXO set validation.
  - **Consensus Recovery**: 100% active block processing.

---

## 6. Exit Criteria

The Cohort A testnet will be considered successful and ready for public testnet graduation if the network sustains:

```text
30 consecutive days of operation
```

with the following conditions satisfied:
1. **Zero Critical Consensus Failures**: No SEV-1 chain halts or unrecoverable forks.
2. **100% Drill Success**: Flawless completion of the Upgrade, Partition, and Snapshot drills.
3. **Controlled State Growth**: RocksDB footprint stays within forecasted linear bounds, and `state_amplification_ratio` remains stable.

---

## 7. Post-Testnet Deliverables

Upon completion of the 30-day run, the core team must produce the following four deliverables:

1. **Genesis Testnet Report v1**: The comprehensive statistical audit of the testnet (template below).
2. **Validator Operator Retrospective**: Feedback and operational insights collected from residential and enterprise operators.
3. **State Growth Analysis**: Extrapolated state size calculations and NVMe hardware suggestions for the next 12 months.
4. **Testnet Readiness Assessment**: A formal Go/No-Go report on cryptographic liveness, P2P network stability, and security posture.

---

## 8. Genesis Testnet Report v1 (Template)

```markdown
# Genesis Testnet Report v1 - Cohort A

## 1. Executive Summary
[High-level summary of the 30-day run, highlighting overall stability, success of drills, and the final public testnet recommendation.]

## 2. Validator Participation & Uptime
* **Validator Count**: [Target: 5]
* **Uptime Percentage**:
  - Core-1: [XX.XX%]
  - Core-2: [XX.XX%]
  - Core-3: [XX.XX%]
  - Operator-1: [XX.XX%]
  - Operator-2: [XX.XX%]
* **Peer Churn Rate**: [Average peer disconnects per hour]

## 3. Consensus Telemetry Summary
* **Total Blocks Validated**: [Count]
* **Total Transactions Processed**: [Count]
* **Fork Events Total**: [Count]
* **Max Fork Depth Observed**: [Depth]
* **Height Divergence Events**: [Count]
* **Chain Reconciliation Events**: [Count]

## 4. P2P Connectivity & Latency
* **Average Peer Count**: [Average]
* **Unique Peers Seen**: [Count]
* **P50 Session Duration**: [Seconds]
* **P95 Session Duration**: [Seconds]
* **Max Tip Count**: [Count]

## 5. Operations Drills Outcomes
* **Upgrade Drill (v0.1.1-testnet)**: [Success / Failure] - [Notes on convergence times]
* **Partition Drill**: [Success / Failure] - [Notes on self-healing and reconciliation events]
* **Snapshot Restore Drill**:
  - Average Restore Time: [Minutes]
  - Average Rejoin Time: [Minutes]
  - State Correctness: [100% / Fail]

## 6. State Growth & Database Footprint
* **Initial DB Size**: [MB]
* **Final DB Size**: [MB]
* **Average Growth/Day**: [MB]
* **Average UTXO Set Size**: [MB]
* **State Amplification Ratio Trend**: [Graph / Explanation of stability]
* **Sync Estimate Trend (12 Months)**: [Extrapolated sync time for new nodes]

## 7. Security & IDS Slashing
* **Double-Sign Incidents**: [Count]
* **Slashing Events**: [Count]
* **Invalid Block Submissions Rejected**: [Count]
* **P2P Rate-Limiting Infractions**: [Count]

## 8. Conclusion & Testnet Recommendation
[Formal sign-off and public testnet readiness verdict.]
```
