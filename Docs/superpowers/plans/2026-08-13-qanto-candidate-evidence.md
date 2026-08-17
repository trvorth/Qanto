# Qanto Candidate and Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin one exact Qanto candidate, classify the existing attack suite honestly, create a fail-closed controlled-private release profile, and capture a fresh baseline whose failures remain durable evidence.

**Architecture:** A Python standard-library module owns canonical JSON, hashing, validation, and evidence-record writes; thin shell entry points collect Git/Cargo/system inputs and preserve native exit codes. Candidate identity is the SHA3-256 hash of a canonical manifest projection that excludes the self-referential `candidate_id` and capture timestamp. Generated evidence is written once beneath a candidate-specific directory and may not be reused after any candidate input changes.

**Tech Stack:** Python 3.9+ standard library, POSIX shell with Bash 3.2 compatibility, Git, Cargo, Rust/nextest, JSON Schema documents, TOML configuration, SHA3-256, existing `genesis_manifest` Rust binary.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- This workstream does not change consensus, monetary, genesis, cryptographic, governance, compatibility, timing, or public-network policy.
- The private profile preserves the current fixed-difficulty/fee-only containment semantics; it does not activate the untracked adaptive-difficulty draft or Emission V2.
- Generated candidate directories are append-only. If an input changes, capture a new candidate ID rather than modifying prior evidence.
- Host metadata must omit usernames, home-directory paths, environment values, credentials, private keys, and network tokens.
- The present Rust package selector is `-p qanto`, because `Protocol_Core/qanto-node/Cargo.toml` declares `name = "qanto"`.

---

### Task 1: Define Canonical Candidate and Evidence Types

**Files:**
- Create: `tools/readiness/readiness.py`
- Create: `tools/readiness/schema/candidate-manifest.schema.json`
- Create: `tools/readiness/schema/evidence-record.schema.json`
- Create: `tools/readiness/schema/evidence-index.schema.json`
- Create: `tools/readiness/tests/test_readiness.py`

**Interfaces:**
- Produces: `canonical_json_bytes(value: object) -> bytes`
- Produces: `sha3_hex(data: bytes) -> str`
- Produces: `candidate_projection(manifest: dict) -> dict`
- Produces: `candidate_id(manifest: dict) -> str`
- Produces: `validate_candidate(manifest: dict) -> list[str]`
- Produces: `validate_evidence(record: dict, candidate: dict) -> list[str]`
- Produces: CLI subcommands `candidate-id`, `validate-candidate`, `validate-evidence`, and `index`.
- Consumed by: all scripts and later workstreams.

- [ ] **Step 1: Write failing canonicalization and identity tests.**

Add these tests to `tools/readiness/tests/test_readiness.py`:

```python
import copy
import json
import pathlib
import sys
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS_DIR))

import readiness


class CandidateIdentityTests(unittest.TestCase):
    def setUp(self):
        self.manifest = {
            "schema_version": 1,
            "candidate_id": "",
            "captured_at_utc": "2026-08-13T12:00:00Z",
            "git": {
                "head": "a" * 40,
                "branch": "feature/qanto-testnet-readiness-design",
                "tracked_diff_sha3_256": "b" * 64,
                "untracked_inventory_sha3_256": "c" * 64,
                "dirty": True,
            },
            "inputs": {
                "cargo_lock_sha3_256": "d" * 64,
                "rust_toolchain_sha3_256": "e" * 64,
                "rustc_verbose": "rustc test",
                "cargo_verbose": "cargo test",
                "feature_allowlist": [],
                "workspace_members": ["Protocol_Core/qanto-node"],
                "config_sha3_256": "f" * 64,
                "chain_id": 1234,
                "network_id": "qanto-engineering-private-v1",
                "genesis_manifest_sha3_256": "1" * 64,
                "genesis_commitment": "2" * 64,
                "allocation_state_root": "3" * 64,
                "consensus_parameters_sha3_256": "4" * 64,
            },
            "release": {
                "stage": "private-engineering",
                "included_binaries": ["genesis_manifest", "qanto", "qantowallet"],
                "excluded_capabilities": ["bridge", "evmcall"],
            },
        }

    def test_canonical_json_is_key_order_independent(self):
        reordered = json.loads(json.dumps(self.manifest, sort_keys=True))
        self.assertEqual(
            readiness.canonical_json_bytes(self.manifest),
            readiness.canonical_json_bytes(reordered),
        )

    def test_candidate_id_ignores_capture_time_and_declared_id(self):
        changed = copy.deepcopy(self.manifest)
        changed["candidate_id"] = "sha3-256:" + "9" * 64
        changed["captured_at_utc"] = "2030-01-01T00:00:00Z"
        self.assertEqual(
            readiness.candidate_id(self.manifest),
            readiness.candidate_id(changed),
        )

    def test_candidate_id_changes_when_feature_set_changes(self):
        changed = copy.deepcopy(self.manifest)
        changed["inputs"]["feature_allowlist"] = ["zk"]
        self.assertNotEqual(
            readiness.candidate_id(self.manifest),
            readiness.candidate_id(changed),
        )
```

- [ ] **Step 2: Run the focused tests and preserve the expected import failure.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_readiness.CandidateIdentityTests -v
```

Expected: `FAIL` because `tools/readiness/readiness.py` does not exist or lacks the functions.

- [ ] **Step 3: Implement canonical JSON and candidate identity.**

Create `tools/readiness/readiness.py` with these exact core functions:

```python
#!/usr/bin/env python3
import argparse
import copy
import hashlib
import json
import pathlib
import sys
from typing import Any, Dict, List

HEX64 = set("0123456789abcdef")


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha3_hex(data: bytes) -> str:
    return hashlib.sha3_256(data).hexdigest()


def candidate_projection(manifest: Dict[str, Any]) -> Dict[str, Any]:
    projection = copy.deepcopy(manifest)
    projection.pop("candidate_id", None)
    projection.pop("captured_at_utc", None)
    return projection


def candidate_id(manifest: Dict[str, Any]) -> str:
    digest = sha3_hex(canonical_json_bytes(candidate_projection(manifest)))
    return "sha3-256:" + digest


def _is_lower_hex(value: Any, length: int) -> bool:
    return (
        isinstance(value, str)
        and len(value) == length
        and all(character in HEX64 for character in value)
    )
```

- [ ] **Step 4: Write failing validation tests.**

Add tests that mutate the valid fixture and assert exact errors:

```python
    def test_validation_rejects_zero_consensus_commitment(self):
        self.manifest["inputs"]["consensus_parameters_sha3_256"] = "0" * 64
        errors = readiness.validate_candidate(self.manifest)
        self.assertIn("inputs.consensus_parameters_sha3_256 must be nonzero lowercase hex", errors)

    def test_validation_rejects_unsorted_sets(self):
        self.manifest["release"]["included_binaries"] = ["qanto", "genesis_manifest"]
        errors = readiness.validate_candidate(self.manifest)
        self.assertIn("release.included_binaries must be sorted and unique", errors)

    def test_validation_rejects_incorrect_declared_id(self):
        self.manifest["candidate_id"] = "sha3-256:" + "0" * 64
        errors = readiness.validate_candidate(self.manifest)
        self.assertIn("candidate_id does not match canonical manifest projection", errors)
```

- [ ] **Step 5: Implement fail-closed validation.**

`validate_candidate` must check every required object/key and type, exact allowed stage values (`private-engineering`, `public-candidate`), exact hex lengths, nonzero commitments, sorted unique arrays, `chain_id` in `1..=u32::MAX`, nonempty network ID, dirty-tree hashes, and equality with `candidate_id(manifest)`. It returns all errors in stable lexical order.

Use helpers with this signature:

```python
def validate_candidate(manifest: Dict[str, Any]) -> List[str]:
    errors: List[str] = []
    # Append explicit path-qualified errors for every violated invariant.
    return sorted(errors)
```

Do not use the `jsonschema` package; no additional candidate dependency is allowed.

- [ ] **Step 6: Define strict JSON Schema documents and parity tests.**

The schemas must set `"additionalProperties": false` at every object level, define all required keys, enums, numeric bounds, and regexes, and use `sha3-256:[0-9a-f]{64}` for candidate IDs. Add a test that loads each schema as JSON and compares every schema `required` set to the keys exercised by the Python validator fixture.

- [ ] **Step 7: Run the readiness unit tests.**

```bash
rtk python3 -m unittest discover -s tools/readiness/tests -p 'test_*.py' -v
```

Expected: `PASS`.

- [ ] **Step 8: Record the intended commit boundary without committing.**

Intended commit: `test: define readiness evidence contracts`. Do not run `git commit` unless explicitly authorized.

### Task 2: Capture Dirty-Tree Inputs Without Modifying Them

**Files:**
- Create: `tools/readiness/capture-candidate.sh`
- Create: `tools/readiness/tests/test_capture_candidate.py`
- Modify: `tools/readiness/readiness.py`
- Reuse: `Protocol_Core/qanto-node/src/bin/genesis_manifest.rs`

**Interfaces:**
- CLI: `capture-candidate.sh --stage private-engineering --config PATH --genesis PATH --allowlist PATH --output DIR`
- Output: a single `candidate_id=sha3-256:...` line and an append-only candidate directory.
- Failure: nonzero with no partially valid manifest presented as complete.

- [ ] **Step 1: Write failing fixture-repository tests.**

Use `tempfile.TemporaryDirectory` and a local Git fixture. The tests must prove:

```python
class CaptureCandidateTests(unittest.TestCase):
    def test_dirty_diff_changes_candidate_id(self): ...
    def test_untracked_file_content_changes_candidate_id(self): ...
    def test_untracked_inventory_never_reads_git_directory(self): ...
    def test_capture_rejects_missing_config(self): ...
    def test_capture_rejects_malformed_genesis_output(self): ...
    def test_existing_candidate_directory_is_not_overwritten(self): ...
    def test_paths_are_repo_relative_and_sorted(self): ...
```

The untracked inventory fixture records for each file: repository-relative UTF-8 path, byte length, and SHA3-256 content hash. It must not record file contents.

- [ ] **Step 2: Run the fixture tests to establish failure.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_capture_candidate -v
```

Expected: `FAIL` because the capture command is absent.

- [ ] **Step 3: Implement deterministic Git inventory helpers.**

Add to `readiness.py`:

```python
def hash_length_prefixed_records(records: List[bytes]) -> str:
    digest = hashlib.sha3_256()
    for record in records:
        digest.update(len(record).to_bytes(8, "big"))
        digest.update(record)
    return digest.hexdigest()


def untracked_inventory(entries: List[Dict[str, Any]]) -> str:
    ordered = sorted(entries, key=lambda item: item["path"].encode("utf-8"))
    return hash_length_prefixed_records(
        [canonical_json_bytes(entry) for entry in ordered]
    )
```

The shell script obtains NUL-delimited names with `git ls-files --others --exclude-standard -z`; Python hashes files in binary mode and rejects paths outside the repository root or under `.git/`.

- [ ] **Step 4: Implement candidate capture in fail-closed phases.**

The script must:

1. Resolve repository root with `git rev-parse --show-toplevel`.
2. Refuse a non-repository working directory.
3. Record `git rev-parse HEAD`, `git branch --show-current`, and `git diff --binary --no-ext-diff` hash.
4. Generate the untracked inventory through `readiness.py inventory`.
5. Hash `Cargo.lock`, `rust-toolchain.toml`, selected config, genesis, allowlist, root `Cargo.toml`, qanto-node `Cargo.toml`, and local `[patch.crates-io]` trees.
6. Parse the workspace member list and literal sorted feature allowlist.
7. run `cargo run --locked -p qanto --bin genesis_manifest -- --path "$GENESIS"` and require exactly one valid `genesis_manifest_hash=` and `allocation_state_root=` line.
8. Parse chain/network/consensus commitment from the selected config and reject disagreement with genesis.
9. Build the manifest in the job temporary directory.
10. Compute and validate the candidate ID.
11. Atomically rename the temporary candidate directory into `audit-results/readiness/${candidate_id#sha3-256:}`.
12. Refuse to overwrite an existing directory unless its manifest bytes are identical; identical recapture exits successfully without mutation.

- [ ] **Step 5: Ensure candidate identity covers local dependency patches.**

Add `inputs.source_inputs_sha3_256`, computed from sorted records for:

```text
Cargo.toml
Protocol_Core/qanto-node/Cargo.toml
patches/ark-relations/**
patches/libp2p-dns/**
patches/libp2p-mdns/**
```

Ignore `.git`, build outputs, and generated Graphify output for this field; dirty-tree fields still commit every Git-visible source change.

- [ ] **Step 6: Run capture tests and a dry capture.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_capture_candidate -v
rtk bash tools/readiness/capture-candidate.sh \
  --stage private-engineering \
  --config config/readiness/private-engineering.toml \
  --genesis config/readiness/private-engineering-genesis.json \
  --allowlist config/readiness/private-release-allowlist.toml \
  --output "$CLAUDE_JOB_DIR/tmp/readiness-capture"
```

Expected: tests pass; dry capture either succeeds with one candidate ID or fails explicitly because Task 4 has not yet supplied valid private config/genesis. Both outcomes are retained in the session log and are not called baseline evidence.

- [ ] **Step 7: Record the intended commit boundary without committing.**

Intended commit: `feat: capture exact readiness candidate`. Do not commit unless explicitly authorized.

### Task 3: Preserve Command Results and Native Exit Codes

**Files:**
- Create: `tools/readiness/run-evidence.sh`
- Create: `tools/readiness/validate-evidence.sh`
- Modify: `tools/readiness/readiness.py`
- Create: `tools/readiness/tests/test_run_evidence.py`

**Interfaces:**
- CLI: `run-evidence.sh --candidate sha3-256:HEX --record-id ID --claim TEXT --label LABEL -- command args...`
- Output: `evidence/ID.json`, `raw/ID.stdout`, `raw/ID.stderr`, atomically updated `evidence-index.json`.
- Return: exact child exit code after durable record creation; wrapper failure uses exit 125.

- [ ] **Step 1: Write failing wrapper tests.**

Create tests that invoke harmless fixture commands and assert:

```python
class EvidenceRunnerTests(unittest.TestCase):
    def test_success_record_preserves_zero_exit(self): ...
    def test_failure_record_preserves_nonzero_exit(self): ...
    def test_stdout_and_stderr_are_separate_and_byte_exact(self): ...
    def test_duplicate_record_id_is_rejected(self): ...
    def test_mismatched_candidate_is_rejected(self): ...
    def test_fail_status_cannot_be_overridden_to_pass(self): ...
    def test_index_is_sorted_and_contains_record_hash(self): ...
    def test_command_is_stored_as_argument_array(self): ...
```

Use `python3 -c` fixture commands instead of shell interpolation so argument fidelity is testable.

- [ ] **Step 2: Run the tests to establish failure.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_run_evidence -v
```

Expected: `FAIL` because the wrapper is absent.

- [ ] **Step 3: Implement evidence status derivation.**

Use this exact rule in `readiness.py`:

```python
def gate_status(exit_code: int, blocked: bool = False) -> str:
    if blocked:
        return "BLOCKED"
    return "PASS" if exit_code == 0 else "FAIL"
```

`PARTIAL` and `NOT APPLICABLE` may only be authored by a separate reviewed decision record that cites component records; the command wrapper never guesses them.

- [ ] **Step 4: Implement the shell wrapper.**

Use `mktemp` under `$CLAUDE_JOB_DIR/tmp`, trap cleanup, timestamps from `date -u +%Y-%m-%dT%H:%M:%SZ`, and a command array. Disable `set -e` only around the child command:

```bash
set +e
"${COMMAND[@]}" >"$STDOUT_TMP" 2>"$STDERR_TMP"
EXIT_CODE=$?
set -e
```

Then call `readiness.py write-evidence`; only after it succeeds, return `EXIT_CODE`. Never use `|| true` around the evidence write.

- [ ] **Step 5: Add redacted host identity.**

Hash a canonical object containing only OS name/version, kernel, architecture, logical CPU count, total RAM, and tool versions. Do not include hostname, username, home path, IP, MAC, environment, serial numbers, or cloud metadata.

- [ ] **Step 6: Run all wrapper tests.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_run_evidence -v
```

Expected: `PASS`.

- [ ] **Step 7: Record the intended commit boundary without committing.**

Intended commit: `feat: retain immutable readiness evidence`. Do not commit unless explicitly authorized.

### Task 4: Define the Controlled Private Release Inputs

**Files:**
- Create: `config/readiness/private-engineering.toml`
- Create: `config/readiness/private-engineering-genesis.json`
- Create: `config/readiness/private-release-allowlist.toml`
- Create: `Protocol_Core/qanto-node/tests/private_release_profile.rs`
- Create: `Docs/guide/private-engineering-testnet.md`
- Create: `Docs/runbooks/private-testnet-halt-reset.md`

**Interfaces:**
- Produces: one deterministic local-only, non-value-bearing private configuration.
- Does not produce: approved public chain ID, public genesis, public emission, public timing, bridge access, or permissionless claims.

- [ ] **Step 1: Write the failing release-profile parser tests.**

The test constructs the selected config and allowlist and asserts:

```rust
#[test]
fn private_profile_is_contained_and_non_value_bearing() {
    let profile = ReleaseProfile::load("config/readiness/private-release-allowlist.toml")
        .expect("private release profile must parse");
    assert_eq!(profile.stage, ReleaseStage::PrivateEngineering);
    assert!(profile.bind_private_only);
    assert!(profile.resettable);
    assert!(!profile.external_value_claims);
    assert_eq!(profile.consensus.difficulty_mode, DifficultyMode::Fixed);
    assert_eq!(profile.consensus.emission_mode, EmissionMode::LegacyFeeOnly);
    for capability in EXCLUDED_PUBLIC_CAPABILITIES {
        assert!(!profile.capabilities.contains(capability));
    }
}
```

`ReleaseProfile`, `ReleaseStage`, and typed capabilities are implemented by the optional-feature quarantine workstream. Until then, place this test behind `#[ignore = "release profile implementation is Workstream 8"]` and add a Python TOML/text structural test that runs now.

- [ ] **Step 2: Select a dedicated development identity without changing public defaults.**

Use a new local-only chain ID that is not `1234`; derive it deterministically as the first nonzero `u32` from SHA3-256 of `b"QANTO\0PRIVATE-ENGINEERING\02026-08-13"`. Implement the derivation in `readiness.py derive-private-chain-id` and record the integer in both config and genesis. The test recomputes it rather than trusting a copied value.

- [ ] **Step 3: Generate a deterministic disposable genesis keypair through existing offline tooling.**

Store only the public genesis document in the repository. Store the disposable private key under `$CLAUDE_JOB_DIR/tmp/private-engineering-keys/` during local runs; never print or commit it. Every allocation must include its V2 account public key and chain-derived address, satisfy `absolute_genesis.rs`, and be explicitly valueless/resettable in documentation.

- [ ] **Step 4: Freeze private containment values.**

The private config must set:

```toml
stage = "private-engineering"
network_id = "qanto-engineering-private-v1"
bind_private_only = true
resettable = true
external_value_claims = false
consensus_mode = "current-v1-contained"
difficulty_mode = "fixed"
emission_mode = "legacy-fee-only"
bridge_enabled = false
evm_call_enabled = false
dynamic_sharding_enabled = false
saga_consensus_enabled = false
omega_consensus_enabled = false
heterogeneous_mining_enabled = false
```

The existing active block timing/difficulty values must be copied exactly from the contained runtime after baseline discovery; do not select new policy values here. The candidate capture rejects a config that omits them.

- [ ] **Step 5: Write operator truth-in-labeling and reset procedures.**

`Docs/guide/private-engineering-testnet.md` must state: known operators only, no monetary value, no bridge, no sale, no decentralization claim, reset at any time, no history guarantee, fixed-difficulty containment, and not evidence of public readiness.

`Docs/runbooks/private-testnet-halt-reset.md` must use an explicit `$QANTO_DISPOSABLE_DATA_DIR`; verify it contains a generated sentinel `.qanto-private-disposable`; refuse `/`, `$HOME`, the repository root, empty paths, symlinks, or paths without the sentinel before deletion. Never include `rm -rf` without these checks.

- [ ] **Step 6: Validate genesis commitments and input consistency.**

```bash
rtk cargo run --locked -p qanto --bin genesis_manifest -- \
  --path config/readiness/private-engineering-genesis.json
rtk python3 tools/readiness/readiness.py validate-private-profile \
  --config config/readiness/private-engineering.toml \
  --genesis config/readiness/private-engineering-genesis.json \
  --allowlist config/readiness/private-release-allowlist.toml
```

Expected: exact nonzero manifest and allocation roots; all chain/network/profile values agree.

- [ ] **Step 7: Record the intended commit boundary without committing.**

Intended commit: `chore: define contained private testnet profile`. Do not commit unless explicitly authorized.

### Task 5: Classify Every Attack Test Semantically

**Files:**
- Create: `tools/readiness/attack-tests.toml`
- Create: `tools/readiness/validate-attack-metadata.py`
- Create: `tools/readiness/tests/test_attack_metadata.py`
- Modify later: `Protocol_Core/qanto-node/src/attack_tests.rs`
- Inspect: `Protocol_Core/qanto-node/src/attack_1_single_parent_dag_safety_violation.rs` through `attack_12_omega_threat_manipulation.rs`

**Interfaces:**
- Produces: one metadata row for every registered attack module and explicit pairing to a remediation regression or blocker.

- [ ] **Step 1: Write a failing coverage test.**

Parse `attack_tests.rs` module declarations and the TOML rows, then assert exact set equality:

```python
self.assertEqual(
    declared_modules,
    metadata_modules,
    "every attack module must have exactly one semantic metadata record",
)
```

Also assert allowed classifications, allowed gate statuses, nonempty threat/preconditions/results/interpretation, and a `paired_regression` for every exploit reproduction.

- [ ] **Step 2: Run the metadata test to establish failure.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_attack_metadata -v
```

Expected: `FAIL` because the metadata file is absent.

- [ ] **Step 3: Populate all twelve records from source behavior.**

Use these initial classifications, adjusting only when direct source/test inspection proves otherwise:

| Module | Classification | Present interpretation |
|---|---|---|
| attack 1 single-parent DAG | exploit-reproduction | Passing demonstrates reorg/double-spend precondition, not a fix |
| attack 2 fixed difficulty | exploit-reproduction | Passing demonstrates noncanonical difficulty rejection/liveness issue |
| attack 3 absent finality | exploit-reproduction | Passing demonstrates deep reorganization remains possible |
| attack 4 EVM PQ bypass | exploit-reproduction or containment | Count as containment only if all release ingress paths reject EVMCall |
| attack 5 forged bridge | exploit-reproduction or containment | Count as containment only if bridge is unreachable and fails closed |
| attack 6 unauthenticated RPC/WS | exploit-reproduction | Passing demonstrates unauthorized reachability unless private binding/auth regression pairs it |
| attack 7 fixed-topic eclipse | performance-characterization/exploit-reproduction | Requires local multi-peer experiment; unit proof alone is insufficient |
| attack 8 reservation griefing | exploit-reproduction | Pair with bounded lease/fairness regression |
| attack 9 consensus lock | performance-characterization | Passing records contention; it is not a security fix |
| attack 10 blocks map | performance-characterization | Passing records growth behavior; it is not pruning evidence |
| attack 11 SAGA f64 | containment | Only if negative dependency tests prove no consensus reachability |
| attack 12 ΩMEGA manipulation | containment | Only if negative dependency tests prove advisory-only behavior |

- [ ] **Step 4: Add metadata IDs to test names or comments without changing assertions.**

Each module must expose its metadata ID in a module-level constant:

```rust
pub(crate) const READINESS_ATTACK_ID: &str = "attack-2-fixed-difficulty-liveness";
```

`attack_tests.rs` registers all IDs in a test-only array and asserts uniqueness. Do not rewrite exploit assertions during classification.

- [ ] **Step 5: Run metadata and attack tests.**

```bash
rtk python3 -m unittest tools.readiness.tests.test_attack_metadata -v
rtk cargo nextest run -p qanto --locked -- attack_
```

Expected: metadata passes. Attack results are recorded as observed and interpreted by metadata; a process pass is not automatically a remediation pass.

- [ ] **Step 6: Record the intended commit boundary without committing.**

Intended commit: `test: classify security attack evidence`. Do not commit unless explicitly authorized.

### Task 6: Capture a Fresh Baseline Without Repairing It

**Files:**
- Create: `tools/readiness/baseline.toml`
- Create: generated `audit-results/readiness/$QANTO_CANDIDATE_ID/evidence/`
- Create: generated `audit-results/readiness/$QANTO_CANDIDATE_ID/baseline-summary.json`
- Create: generated `audit-results/readiness/$QANTO_CANDIDATE_ID/stage-0-decision.md`

**Interfaces:**
- Consumes: validated candidate manifest and evidence runner.
- Produces: current results for format, check, build, Clippy, nextest, standard test parity, dependency advisories, licenses, secrets, and feature configurations.

- [ ] **Step 1: Define the literal baseline matrix.**

`tools/readiness/baseline.toml` contains these exact records:

```toml
[[command]]
id = "cargo-fmt-check"
claim = "Workspace Rust formatting"
label = "CONFIRMED"
argv = ["rtk", "cargo", "fmt", "--all", "--", "--check"]

[[command]]
id = "cargo-check-workspace"
claim = "Locked workspace compile check"
label = "CONFIRMED"
argv = ["rtk", "cargo", "check", "--workspace", "--locked"]

[[command]]
id = "cargo-clippy-workspace"
claim = "Locked all-target Clippy with warnings denied"
label = "CONFIRMED"
argv = ["rtk", "cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]

[[command]]
id = "cargo-nextest-workspace"
claim = "Locked workspace nextest suite"
label = "CONFIRMED"
argv = ["rtk", "cargo", "nextest", "run", "--workspace", "--locked"]

[[command]]
id = "cargo-test-docs"
claim = "Locked workspace doctests and standard harness parity"
label = "CONFIRMED"
argv = ["rtk", "cargo", "test", "--workspace", "--locked", "--doc"]

[[command]]
id = "cargo-build-private-release"
claim = "Locked allowlisted private release binaries"
label = "CONFIRMED"
argv = ["rtk", "cargo", "build", "--release", "--locked", "-p", "qanto", "--bin", "qanto", "--bin", "qantowallet", "--bin", "genesis_manifest"]
```

Dependency, license, and secret tools are separate records. If a required tool is unavailable, write a `BLOCKED` record with installation/version evidence; do not silently install or skip it.

- [ ] **Step 2: Capture the exact candidate into the durable output root.**

```bash
rtk bash tools/readiness/capture-candidate.sh \
  --stage private-engineering \
  --config config/readiness/private-engineering.toml \
  --genesis config/readiness/private-engineering-genesis.json \
  --allowlist config/readiness/private-release-allowlist.toml \
  --output audit-results/readiness
```

Export the printed value only after manifest validation:

```bash
export QANTO_CANDIDATE_ID='sha3-256:'"$(rtk python3 tools/readiness/readiness.py latest-candidate --output audit-results/readiness)"
```

- [ ] **Step 3: Run each baseline command independently.**

The matrix driver must continue after a child failure while retaining each exit code. It must not use stale binaries to satisfy a failed build.

```bash
rtk python3 tools/readiness/readiness.py run-matrix \
  --candidate "$QANTO_CANDIDATE_ID" \
  --matrix tools/readiness/baseline.toml
```

Expected: every defined command has exactly one evidence record. No expected pass count is stated in advance.

- [ ] **Step 4: Capture feature and all-feature diagnostics separately.**

Run private allowlist first. Then run `--all-features` checks as diagnostic evidence only because optional features are excluded from the release:

```bash
rtk bash tools/readiness/run-evidence.sh \
  --candidate "$QANTO_CANDIDATE_ID" \
  --record-id cargo-check-all-features-diagnostic \
  --claim "Diagnostic all-feature compile check" \
  --label CONFIRMED -- \
  rtk cargo check --workspace --all-targets --all-features --locked
```

A diagnostic failure is a finding but does not become a private release pass/fail without capability-matrix analysis.

- [ ] **Step 5: Capture supply-chain and secret-scan evidence.**

Use pinned, recorded tool versions. Required commands are:

```bash
rtk cargo audit --json
rtk cargo deny check advisories bans licenses sources
rtk gitleaks detect --no-banner --redact --source .
```

If tool installation is not already present or authorized, record `BLOCKED` and exact missing executable. Never print unredacted candidate secrets.

- [ ] **Step 6: Generate a mechanical baseline summary.**

The summary contains record ID, status, exit code, and record hash only. Derive overall status:

```python
required = [record for record in records if record["required"]]
overall = "PASS" if all(record["gate_status"] == "PASS" for record in required) else "FAIL"
```

No prose interpretation is generated automatically.

- [ ] **Step 7: Reconcile historical claims.**

Create `historical-claims.md` listing the commit/toolchain/command/result asserted by `AUDIT_LEDGER.md`, `THREAT_MODEL.md`, and `Docs/security/AUDIT_BUILD_GUIDE.md`, then state whether the fresh candidate confirms, contradicts, or cannot compare with each claim. Historical counts never replace fresh records.

- [ ] **Step 8: Issue Stage 0 decision.**

Write exactly one first line:

```text
BASELINE PINNED
```

only when all candidate inputs and evidence records exist. Otherwise use:

```text
BASELINE FAILED
```

Follow with candidate ID and blocker/failure record IDs. This is not a testnet verdict.

- [ ] **Step 9: Update Graphify after source/test changes.**

```bash
rtk graphify update .
```

Expected: update succeeds or a `BLOCKED` evidence record is written. Generated evidence output need not be graphed.

- [ ] **Step 10: Record the intended commit boundary without committing.**

Intended commit: `chore: capture Qanto readiness baseline`. Do not stage generated raw evidence or commit anything unless explicitly authorized and repository policy approves the evidence retention set.

## Acceptance Criteria

- One candidate ID commits all Git, dependency, toolchain, feature, configuration, chain, genesis, consensus-parameter, and source-patch inputs.
- Dirty tracked and untracked content is inventoried without changing or exposing it.
- Every command result preserves stdout, stderr, exit code, command argv, candidate ID, host class, and record hash.
- Existing attack tests cannot be misrepresented as fixes merely because the process exits zero.
- The private profile is explicitly resettable, access-controlled, valueless, fixed-difficulty, fee-only containment with excluded capabilities denied.
- No historical result is presented as current evidence.
- No public deployment or policy activation occurs.
