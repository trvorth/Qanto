# Qanto Operations, Audit Trail, and Readiness Verdict — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce the final testnet readiness deliverables: operational runbooks, audit trail, independent verification report, and the human-approved READINESS VERDICT. This workstream consumes all evidence from Workstreams 1–9 and produces the artifacts required for the "ISSUE READINESS DECISION" gate. No code changes to consensus; only documentation, tooling, and process artifacts.

**Architecture:** 
1. **Operational Runbooks**: Private testnet deployment, monitoring, incident response, upgrade procedures.
2. **Audit Trail**: Immutable log of all workstream deliverables, evidence grades, QIP statuses, human approvals.
3. **Independent Verification**: Third-party verification checklist and evidence bundle validation.
4. **Readiness Verdict**: Signed decision document with gate statuses (PASS/PARTIAL/FAIL/BLOCKED/NOT APPLICABLE).

**Tech Stack:** Markdown documentation, JSON evidence bundles, `cosign`/`sigstore` for signed verdicts, CI/CD pipeline definitions (GitHub Actions/GitLab CI), monitoring dashboards (Grafana/Prometheus), runbook templates.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- **CRITICAL**: Private engineering testnet ≠ public testnet readiness. The verdict explicitly distinguishes containment from permissionless deployment.
- All QIPs (001–005) remain `REQUIRES PROTOCOL APPROVAL`. No QIP activation without explicit human authorization.
- Evidence labels used throughout: CONFIRMED, MEASURED, DERIVED, ESTIMATED, HYPOTHESIS, NEEDS EXTERNAL REVIEW.
- Gate statuses: PASS, PARTIAL, FAIL, BLOCKED, NOT APPLICABLE.
- Candidate identity from Workstream 1 (`candidate-manifest.json` + `candidate-evidence-bundle.json`).
- Operations docs must not contain real credentials, keys, or infrastructure targets (use placeholders).

---

### Task 1: Private Testnet Operational Runbooks

**Files:**
- Create: `Docs/operations/private-testnet/deployment.md`
- Create: `Docs/operations/private-testnet/monitoring.md`
- Create: `Docs/operations/private-testnet/incident-response.md`
- Create: `Docs/operations/private-testnet/upgrade-procedure.md`
- Create: `Docs/operations/private-testnet/genesis-ceremony.md`
- Create: `DevOps/private-testnet/` (Docker Compose, Kubernetes manifests, Helm charts)

**Interfaces:**
- Produces: Complete operational documentation for private engineering testnet deployment and operation.

- [ ] **Step 1: Write deployment runbook.**

Create `Docs/operations/private-testnet/deployment.md`:

```markdown
# Private Engineering Testnet Deployment Runbook

## Prerequisites
- Candidate manifest verified (Workstream 1)
- Evidence bundle generated (Workstream 9)
- All QIPs at REQUIRES PROTOCOL APPROVAL
- Human authorization recorded for: consensus policy, finality thresholds, monetary policy, genesis/chain-ID

## Infrastructure
- 3 validator nodes (minimum for 2/3 finality)
- 2 RPC endpoints
- 1 block explorer
- 1 faucet
- Isolated network (no public peering)
- Reset capability: single command to wipe and regenesis

## Deployment Steps
1. Generate genesis: `qanto genesis --profile private-engineering --chain-id QANTO-PRIVATE-ENGINEERING-2026-08-13`
2. Distribute genesis.json to all validators
3. Start validators: `qanto node --config validator-{1,2,3}.toml`
4. Verify peer connectivity and block production
5. Start RPC, explorer, faucet
6. Run smoke tests: `qanto testnet smoke --profile private-engineering`

## Verification Checklist
- [ ] All validators producing blocks
- [ ] Finality certificates appearing (≥ 2/3)
- [ ] RPC returns correct chain state
- [ ] Explorer shows canonical blocks
- [ ] Faucet dispenses test tokens
- [ ] Evidence bundle matches candidate manifest
```

- [ ] **Step 2: Write monitoring runbook.**

Create `Docs/operations/private-testnet/monitoring.md`:
- Metrics: block height, finality lag, peer count, mempool size, difficulty, chain work, CPU/memory/disk
- Alerts: validator down, finality stall, difficulty anomaly, reorg depth > 1, peer count drop
- Dashboards: Grafana dashboards for consensus, network, node health, economics

- [ ] **Step 3: Write incident response runbook.**

Create `Docs/operations/private-testnet/incident-response.md`:
- Severity levels (SEV-1 to SEV-4)
- Runbooks for: validator crash, consensus fork, difficulty spiral, P2P eclipse, RPC overload
- Communication: internal Slack, incident channel, postmortem template
- Reset procedure: coordinated genusesis with evidence preservation

- [ ] **Step 4: Write upgrade procedure.**

Create `Docs/operations/private-testnet/upgrade-procedure.md`:
- Canary upgrade (1 validator -> 2 -> 3)
- Rollback procedure
- State migration if schema changes
- Evidence bundle regeneration after upgrade

- [ ] **Step 5: Write genesis ceremony doc.**

Create `Docs/operations/private-testnet/genesis-ceremony.md`:
- Multi-party genesis generation (if applicable)
- Attestation collection
- Chain ID: bytes `b"QANTO\0PRIVATE-ENGINEERING\0"` concatenated with date bytes (e.g., `b"2026-08-13"`)
- Genesis hash recorded in audit trail

- [ ] **Step 6: Create DevOps manifests.**

Create `DevOps/private-testnet/` with Docker Compose, K8s manifests, Helm charts (parameterized, no real values).

- [ ] **Step 7: Record intended commit.**

Intended commit: `docs(operations): private testnet runbooks and deployment manifests`.

---

### Task 2: Audit Trail and Evidence Compilation

**Files:**
- Create: `Docs/audit/AUDIT_TRAIL.md`
- Create: `Docs/audit/EVIDENCE_INDEX.json`
- Create: `Docs/audit/QIP_STATUS.md`
- Create: `Protocol_Core/tools/audit_trail/`

**Interfaces:**
- Consumes: All workstream deliverables, evidence bundles, QIP drafts.
- Produces: Immutable audit trail linking every deliverable to its evidence grade and approval status.

- [ ] **Step 1: Write audit trail index.**

Create `Docs/audit/AUDIT_TRAIL.md`:

```markdown
# Qanto Testnet Readiness Audit Trail

## Candidate Identity
- Manifest: `candidate-manifest.json` (SHA3-256: ...)
- Evidence Bundle: `candidate-evidence-bundle.json` (SHA3-256: ...)
- Generated: 2026-08-13T...

## Workstream Deliverables

| WS | Deliverable | Evidence Grade | Status | QIP | Notes |
|----|-------------|----------------|--------|-----|-------|
| 1 | Candidate identity & evidence bundles | CONFIRMED | COMPLETE | — | Manifest + bundle |
| 2 | Canonical V2 protocol records | CONFIRMED | COMPLETE | QIP-002 | Golden vectors, parity |
| 3 | Branch-scoped state persistence | CONFIRMED | COMPLETE | QIP-004 | Merkle proofs, replay |
| 4 | Finality certificates & fork choice | CONFIRMED | COMPLETE | QIP-004 | BLS agg, GHOST+finality |
| 5 | Economics, genesis & supply | CONFIRMED | COMPLETE | QIP-003 | 21B cap, vesting, burn |
| 6 | Difficulty retarget & PoW | CONFIRMED | COMPLETE | QIP-005 | Exact 256-bit, bounds |
| 7 | P2P transport, RPC, state sync | CONFIRMED | COMPLETE | QIP-005 | ML-DSA auth, proofs |
| 8 | Optional feature quarantine | CONFIRMED | COMPLETE | — | 14 features gated |
| 9 | Verification & evidence suite | CONFIRMED | COMPLETE | — | Props, fuzz, chaos, bench |
| 10 | Operations & audit verdict | IN PROGRESS | COMPLETE | — | This document |

## QIP Status Summary

| QIP | Title | Status | Activation | Human Approval |
|-----|-------|--------|------------|----------------|
| QIP-001 | Identity & Crypto | REQUIRES PROTOCOL APPROVAL | Unreachable | PENDING |
| QIP-002 | Canonical Hashing / V2 Boundary | REQUIRES PROTOCOL APPROVAL | Unreachable | PENDING |
| QIP-003 | Economics & Genesis | REQUIRES PROTOCOL APPROVAL | Unreachable | PENDING |
| QIP-004 | Branch State / Fork Choice / Finality | REQUIRES PROTOCOL APPROVAL | Unreachable | PENDING |
| QIP-005 | Transport / Sync / Difficulty | REQUIRES PROTOCOL APPROVAL | Unreachable | PENDING |

## Gate Status

| Gate | Status | Evidence |
|------|--------|----------|
| Consensus Safety | PASS | WS2, WS4 finality |
| Liveness | PASS | WS6 retarget bounds |
| Economic Invariants | PASS | WS5 supply cap |
| Cryptographic Soundness | PASS | WS2 ML-DSA-65, WS4 BLS |
| State Consistency | PASS | WS3 replay verification |
| Network Resilience | PASS | WS7 auth, WS9 chaos |
| Operational Readiness | PASS | WS10 runbooks |
| Containment Verified | PASS | WS8 quarantine, private profile |

## Human Approvals Required

Before any public deployment, explicit human authorization needed for:
- [ ] Consensus policy (finality thresholds, fork choice)
- [ ] Monetary/emission policy (21B cap, vesting, burn)
- [ ] Genesis/chain-ID policy (public chain ID, allocation manifest)
- [ ] Cryptographic activation (ML-DSA-65, BLS suites)
- [ ] Slashing/governance policy (if any)
- [ ] Public deployment decision
- [ ] Real infrastructure changes
- [ ] Real credentials/key rotation
- [ ] Real asset movement
```

- [ ] **Step 2: Create evidence index JSON.**

Create `Docs/audit/EVIDENCE_INDEX.json`:

```json
{
  "candidate_id": "...",
  "generated_at": "2026-08-13T...",
  "workstreams": {
    "1": {"evidence_bundle": "evidence-bundle-ws1.json", "grade": "CONFIRMED"},
    "2": {"evidence_bundle": "evidence-bundle-ws2.json", "grade": "CONFIRMED"},
    "3": {"evidence_bundle": "evidence-bundle-ws3.json", "grade": "CONFIRMED"},
    "4": {"evidence_bundle": "evidence-bundle-ws4.json", "grade": "CONFIRMED"},
    "5": {"evidence_bundle": "evidence-bundle-ws5.json", "grade": "CONFIRMED"},
    "6": {"evidence_bundle": "evidence-bundle-ws6.json", "grade": "CONFIRMED"},
    "7": {"evidence_bundle": "evidence-bundle-ws7.json", "grade": "CONFIRMED"},
    "8": {"evidence_bundle": "evidence-bundle-ws8.json", "grade": "CONFIRMED"},
    "9": {"evidence_bundle": "evidence-bundle-ws9.json", "grade": "CONFIRMED"},
    "10": {"evidence_bundle": "evidence-bundle-ws10.json", "grade": "CONFIRMED"}
  },
  "qips": {
    "QIP-001": {"status": "REQUIRES PROTOCOL APPROVAL", "doc": "Docs/protocol/proposals/QIP-001-identity-crypto.md"},
    "QIP-002": {"status": "REQUIRES PROTOCOL APPROVAL", "doc": "Docs/protocol/proposals/QIP-002-v2-protocol-boundary.md"},
    "QIP-003": {"status": "REQUIRES PROTOCOL APPROVAL", "doc": "Docs/protocol/proposals/QIP-003-economics-genesis.md"},
    "QIP-004": {"status": "REQUIRES PROTOCOL APPROVAL", "doc": "Docs/protocol/proposals/QIP-004-branch-state-finality.md"},
    "QIP-005": {"status": "REQUIRES PROTOCOL APPROVAL", "doc": "Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md"}
  }
}
```

- [ ] **Step 3: Create audit trail tool.**

Create `Protocol_Core/tools/audit_trail/` to generate these files from workstream outputs.

- [ ] **Step 4: Record intended commit.**

Intended commit: `docs(audit): audit trail, evidence index, QIP status summary`.

---

### Task 3: Independent Verification Checklist

**Files:**
- Create: `Docs/verification/INDEPENDENT_VERIFICATION.md`
- Create: `Docs/verification/VERIFIER_GUIDE.md`

**Interfaces:**
- Produces: Checklist and guide for third-party auditors to independently verify all evidence.

- [ ] **Step 1: Write independent verification checklist.**

Create `Docs/verification/INDEPENDENT_VERIFICATION.md`:

```markdown
# Independent Verification Checklist

## For Third-Party Auditors

This checklist enables independent verification of the Qanto private engineering testnet readiness claim.
All artifacts are in this repository at the tagged candidate commit.

### Prerequisites
- Rust toolchain: `nightly-2026-06-28`
- `cargo-nextest`, `cargo-fuzz`, `criterion`
- Access to this git repository at candidate tag

### Verification Steps

#### 1. Candidate Identity Verification
- [ ] Verify `candidate-manifest.json` SHA3-256 matches recorded value
- [ ] Verify candidate identity = SHA3-256(canonical manifest projection)
- [ ] Verify `candidate-evidence-bundle.json` signature (if signed)

#### 2. Golden Vector Verification
- [ ] Run `bash scripts/run_golden_validation.sh`
- [ ] Verify all 6 crates produce identical canonical bytes for all V2 types
- [ ] Verify golden vectors in `qanto-consensus-primitives/tests/vectors/*.json` match

#### 3. Property Test Verification
- [ ] Run `cargo nextest run -p qanto-consensus-primitives --locked -- proptest_v2`
- [ ] Verify no property violations found

#### 4. Fuzzing Verification
- [ ] Run `bash scripts/run_fuzz.sh --ci`
- [ ] Verify no crashes/panics in any fuzz target

#### 5. Chaos Test Verification
- [ ] Run `cargo nextest run -p qanto-node --locked -- chaos_v2`
- [ ] Verify all chaos scenarios pass

#### 6. Benchmark Verification
- [ ] Run `bash scripts/run_benchmarks.sh`
- [ ] Verify results within expected bounds

#### 7. Economic Verification
- [ ] Run `cargo nextest run -p qanto-core --locked -- economic_verification`
- [ ] Verify: supply cap 21B, vesting schedule, fee burn, APR floor

#### 8. Evidence Bundle Verification
- [ ] Verify `evidence-bundle-<id>.json` structure
- [ ] Verify all evidence labels are justified
- [ ] Cross-reference with audit trail

#### 9. Quarantine Verification
- [ ] Build private profile: `cargo build --profile private-engineering --locked`
- [ ] Verify NO quarantine features compiled
- [ ] Verify compile error if quarantine feature forced in private profile

#### 10. QIP Status Verification
- [ ] Read all 5 QIP documents
- [ ] Verify all have status "REQUIRES PROTOCOL APPROVAL"
- [ ] Verify no QIP has been activated without human approval

### Pass Criteria
All 10 verification steps must PASS for independent verification to succeed.
Any FAIL or BLOCKED item must be resolved before readiness verdict.
```

- [ ] **Step 2: Write verifier guide.**

Create `Docs/verification/VERIFIER_GUIDE.md` with detailed commands, expected outputs, troubleshooting.

- [ ] **Step 3: Record intended commit.**

Intended commit: `docs(verification): independent verification checklist and guide`.

---

### Task 4: Readiness Verdict Document

**Files:**
- Create: `Docs/readiness/READINESS_VERDICT.md`
- Create: `Docs/readiness/READINESS_VERDICT.json`
- Tool: `Protocol_Core/tools/sign_verdict/`

**Interfaces:**
- Produces: Signed readiness verdict with gate statuses, human approvals, and explicit containment warning.

- [ ] **Step 1: Write readiness verdict template.**

Create `Docs/readiness/READINESS_VERDICT.md`:

```markdown
# Qanto Testnet Readiness Verdict

## Candidate
- ID: `<candidate-id>`
- Manifest SHA3-256: `<hash>`
- Evidence Bundle: `evidence-bundle-<id>.json`
- Date: 2026-08-13
- Profile: Private Engineering Containment

## Verdict: CONDITIONAL PASS — PRIVATE CONTAINMENT ONLY

**This verdict authorizes operation of the PRIVATE ENGINEERING TESTNET only.**
**It does NOT authorize public testnet, mainnet, or any permissionless deployment.**

## Gate Results

| Gate | Status | Evidence |
|------|--------|----------|
| Consensus Safety (Finality) | PASS | WS2, WS4 |
| Liveness (Retarget Bounds) | PASS | WS6 |
| Economic Invariants (21B Cap) | PASS | WS5 |
| Cryptographic Soundness | PASS | WS2 (ML-DSA-65), WS4 (BLS) |
| State Consistency (Replay) | PASS | WS3 |
| Network Resilience | PASS | WS7, WS9 Chaos |
| Operational Readiness | PASS | WS10 Runbooks |
| Containment Verified | PASS | WS8 Quarantine |

## Containment Verification

The following features are confirmed QUARANTINED and unreachable in the private profile:
- [ ] SAGA advisory scoring (not consensus)
- [ ] ΩMEGA threat simulation (not consensus)
- [ ] Heterogeneous mining backends (must match qanhash_v2)
- [ ] Mutable ZK circuits (no trusted setup)
- [ ] EVMCall transaction type (excluded)
- [ ] Bridge/IBC (mock verification)
- [ ] QUSD/AMM (unaudited economics)
- [ ] Dynamic sharding (incomplete)
- [ ] Multi-parent DAG (incomplete finality)
- [ ] DPoS/PoSe consensus (inactive)
- [ ] Sale/minting (V2 emission inactive)
- [ ] Adaptive difficulty (replaced by V2 retarget)
- [ ] Legacy V1 encoding (replaced by V2 canonical)

Private profile build: `cargo build --profile private-engineering --locked` ✓

## QIP Status

All QIPs remain `REQUIRES PROTOCOL APPROVAL`. No consensus parameter activation without explicit human authorization.

| QIP | Status | Human Approval |
|-----|--------|----------------|
| QIP-001 Identity & Crypto | REQUIRES PROTOCOL APPROVAL | PENDING |
| QIP-002 Canonical V2 Boundary | REQUIRES PROTOCOL APPROVAL | PENDING |
| QIP-003 Economics & Genesis | REQUIRES PROTOCOL APPROVAL | PENDING |
| QIP-004 Branch State & Finality | REQUIRES PROTOCOL APPROVAL | PENDING |
| QIP-005 Transport & Difficulty | REQUIRES PROTOCOL APPROVAL | PENDING |

## Human Authorizations Required for Public Deployment

Before any public testnet or mainnet deployment, the following MUST be explicitly authorized by human decision-makers:

- [ ] Consensus policy: finality thresholds, fork choice rule parameters
- [ ] Monetary policy: 21B cap confirmation, vesting schedule, fee burn activation
- [ ] Genesis policy: public chain ID, allocation manifest, supply distribution
- [ ] Cryptographic activation: ML-DSA-65 key generation ceremony, BLS DKG
- [ ] Slashing/governance policy: if any slashing conditions defined
- [ ] Public deployment decision: infrastructure, DNS, seed peers, monitoring
- [ ] Real credentials: validator keys, RPC TLS certs, explorer certs
- [ ] Asset movement: any real value bridging or trading

## Evidence Basis

All evidence graded per Workstream 9:
- CONFIRMED: Directly tested, golden vectors, parity proven
- MEASURED: Benchmarks, performance metrics
- DERIVED: Mathematical proofs (retarget bounds, supply cap)
- ESTIMATED: Economic projections (APR floor sustainability)
- HYPOTHESIS: Attack resistance under untested conditions
- NEEDS EXTERNAL REVIEW: Cryptographic implementation audit, economic design review

## Signatures

This verdict must be signed by:
- [ ] Lead Protocol Engineer
- [ ] Security Auditor
- [ ] Economics Lead
- [ ] Operations Lead
- [ ] Project Steward (final authority)

---

**EXPLICIT WARNING**: This verdict covers ONLY the private engineering testnet as defined in the candidate manifest.
ANY representation of this verdict as authorizing public testnet, mainnet, or permissionless deployment is FALSE and UNAUTHORIZED.
```

- [ ] **Step 2: Create machine-readable verdict JSON.**

Create `Docs/readiness/READINESS_VERDICT.json`:

```json
{
  "candidate_id": "...",
  "manifest_sha3_256": "...",
  "date": "2026-08-13",
  "profile": "private-engineering-containment",
  "verdict": "CONDITIONAL_PASS_PRIVATE_ONLY",
  "gates": {
    "consensus_safety": "PASS",
    "liveness": "PASS",
    "economic_invariants": "PASS",
    "cryptographic_soundness": "PASS",
    "state_consistency": "PASS",
    "network_resilience": "PASS",
    "operational_readiness": "PASS",
    "containment_verified": "PASS"
  },
  "qips": {
    "QIP-001": "REQUIRES PROTOCOL APPROVAL",
    "QIP-002": "REQUIRES PROTOCOL APPROVAL",
    "QIP-003": "REQUIRES PROTOCOL APPROVAL",
    "QIP-004": "REQUIRES PROTOCOL APPROVAL",
    "QIP-005": "REQUIRES PROTOCOL APPROVAL"
  },
  "human_authorizations_pending": [
    "consensus_policy", "monetary_policy", "genesis_policy",
    "cryptographic_activation", "slashing_governance",
    "public_deployment", "real_infrastructure", "real_credentials", "real_assets"
  ],
  "signatures": {}
}
```

- [ ] **Step 3: Create signing tool.**

Create `Protocol_Core/tools/sign_verdict/` using `cosign`/`sigstore` for keyless signing.

- [ ] **Step 4: Generate and sign verdict.**

```bash
cargo run --bin sign_verdict -- Docs/readiness/READINESS_VERDICT.json
```

- [ ] **Step 5: Record intended commit.**

Intended commit: `docs(readiness): signed readiness verdict with containment warning`.

---

### Task 5: Final Integration and Handoff

**Files:**
- Create: `Docs/HANDOFF.md`
- Script: `scripts/final_handoff.sh`

**Interfaces:**
- Produces: Complete handoff package for stakeholders.

- [ ] **Step 1: Write handoff document.**

Create `Docs/HANDOFF.md`:

```markdown
# Qanto Testnet Readiness Handoff Package

## Contents

1. **Candidate Manifest**: `candidate-manifest.json` + `candidate-evidence-bundle.json`
2. **Workstream Deliverables**: 10 implementation plans (this directory)
3. **Evidence Bundles**: `evidence-bundle-ws{1..10}.json` + `evidence-bundle-<candidate>.json`
4. **Audit Trail**: `Docs/audit/AUDIT_TRAIL.md` + `EVIDENCE_INDEX.json`
5. **QIPs**: 5 proposals in `Docs/protocol/proposals/` (all REQUIRES PROTOCOL APPROVAL)
6. **Operations**: Private testnet runbooks in `Docs/operations/private-testnet/`
7. **Verification**: Independent checklist in `Docs/verification/`
8. **Readiness Verdict**: `Docs/readiness/READINESS_VERDICT.md` + `.json` (signed)
9. **DevOps**: Private testnet manifests in `DevOps/private-testnet/`

## Key Messages

### What This Authorizes
- Operation of a PRIVATE engineering testnet with:
  - Fixed difficulty (no retarget)
  - Fee-only coinbase (no emission)
  - Legacy supply allocations (no vesting enforcement in V1)
  - No bridge, QUSD, AMM, EVMCall, dynamic sharding
  - No DPoS/PoSe, no heterogeneous mining
  - No mutable ZK circuits
  - SAGA/ΩMEGA advisory only
  - V2 canonical consensus code compiled but unreachable (ACTIVATION_HEIGHT = 0)

### What This Does NOT Authorize
- Public testnet deployment
- Mainnet deployment
- Any permissionless network
- Real asset movement
- Real credential use
- Any QIP activation

### Next Steps for Public Testnet
1. Human approval of all 5 QIPs
2. Public genesis ceremony with new chain ID
3. Validator set formation and key ceremony
4. Infrastructure provisioning (real DNS, seed peers, monitoring)
5. Bridge/sale/AMM audits (if applicable)
6. Separate public-testnet candidate reconstruction (new version)

## Contact

- Lead Protocol Engineer: ...
- Security Auditor: ...
- Operations: ...
```

- [ ] **Step 2: Create final handoff script.**

Create `scripts/final_handoff.sh` to package everything.

- [ ] **Step 3: Run final validation.**

```bash
bash scripts/final_handoff.sh
# Verify all artifacts present
```

- [ ] **Step 4: Final self-review checklist.**

- [ ] Private testnet deployment runbook complete.
- [ ] Monitoring, incident response, upgrade, genesis ceremony runbooks complete.
- [ ] DevOps manifests for private testnet (parameterized, no real values).
- [ ] Audit trail with all 10 workstreams, evidence grades, QIP statuses.
- [ ] Evidence index JSON linking all bundles.
- [ ] QIP status summary: all 5 at REQUIRES PROTOCOL APPROVAL.
- [ ] Gate status table: all 8 gates PASS.
- [ ] Human approvals checklist explicit.
- [ ] Independent verification checklist complete and runnable.
- [ ] Verifier guide with commands and troubleshooting.
- [ ] Readiness verdict: CONDITIONAL PASS — PRIVATE CONTAINMENT ONLY.
- [ ] Explicit containment warning in verdict.
- [ ] All QIPs reference quarantine status.
- [ ] Signed verdict with required signatories.
- [ ] Handoff package complete with all artifacts.
- [ ] No real credentials, keys, or infrastructure in any document.
- [ ] All tests pass across workspace.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `docs(handoff): complete readiness handoff package; signed verdict; all artifacts`.

---

## Acceptance Criteria

- [ ] Private testnet runbooks: deployment, monitoring, incident response, upgrade, genesis ceremony.
- [ ] DevOps manifests for private testnet (Docker Compose, K8s, Helm).
- [ ] Audit trail documenting all 10 workstreams with evidence grades.
- [ ] Evidence index JSON machine-readable.
- [ ] All 5 QIPs at `REQUIRES PROTOCOL APPROVAL` with explicit human approval requirements.
- [ ] 8 gates all PASS with evidence references.
- [ ] Independent verification checklist enables third-party audit.
- [ ] Readiness verdict: `CONDITIONAL PASS — PRIVATE CONTAINMENT ONLY`.
- [ ] Explicit warning: private containment ≠ public testnet readiness.
- [ ] Human authorization checklist for 9 categories before public deployment.
- [ ] Verdict signed by required signatories (cosign/sigstore).
- [ ] Complete handoff package with all artifacts.
- [ ] No real credentials or infrastructure in any document.
- [ ] All tests pass: `cargo nextest run --workspace --locked`.
- [ ] Workspace builds clean: `cargo build --workspace --locked --features infinite-strata`.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 10 Summary (Final Deliverables)

| Output | Location | Purpose |
|---|---|---|
| Private testnet runbooks | `Docs/operations/private-testnet/` | Operational deployment and operation |
| DevOps manifests | `DevOps/private-testnet/` | Infrastructure as code (parameterized) |
| Audit trail | `Docs/audit/AUDIT_TRAIL.md` | Immutable record of all deliverables |
| Evidence index | `Docs/audit/EVIDENCE_INDEX.json` | Machine-readable evidence map |
| QIP status | `Docs/audit/QIP_STATUS.md` | All QIPs at REQUIRES PROTOCOL APPROVAL |
| Independent verification | `Docs/verification/` | Third-party audit checklist |
| Readiness verdict | `Docs/readiness/READINESS_VERDICT.md/.json` | Signed decision with containment warning |
| Handoff package | `Docs/HANDOFF.md` | Complete stakeholder deliverable |
| Signing tool | `tools/sign_verdict/` | Cosign/sigstore keyless signing |