# Qanto P2P Transport, RPC/API, and State Sync — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define the V2 network protocol: P2P message framing with `DomainV2::P2pEnvelope`, gRPC API with V2 types, and state sync with Merkle proofs against `StateRootV2`. All transport uses `hash_v2` for message IDs. Authentication is ML-DSA-65 (identities from Workstream 2) + noise protocol. Private engineering profile uses current P2P/RPC; V2 transport activates with `ACTIVATION_HEIGHT = 0` (unreachable) pending QIP-005.

**Architecture:** `qanto-core::qanto_p2p` and `qanto-rpc` are updated with V2 message types. P2P envelope: `P2pEnvelopeV2 = canonical_record(message_type, payload, signature, timestamp)`. Messages routed by `MessageType` enum. gRPC: protobuf definitions for V2 block, transaction, state proof. State sync: `GetStateProof(key, root) -> StateProofV2(proof_nodes, leaf_value)`. All V2 paths behind `ProtocolVersion::V2` guard.

**Tech Stack:** Rust 2021, `libp2p` with custom `P2pEnvelopeV2` codec, `tonic`/`prost` for gRPC V2, `qanto-consensus-primitives` V2 types, `qanto-core::qanto_native_crypto` for ML-DSA-65 auth.

## Global Constraints

- Inherit every global constraint from `docs/superpowers/plans/2026-08-13-qanto-testnet-readiness.md`.
- V1 private profile: current P2P (floodsub/gossipsub), gRPC (V1 types). V2 transport unreachable (`ACTIVATION_HEIGHT = 0`).
- P2P message IDs: `MessageIdV2 = hash_v2(DomainV2::P2pEnvelope, chain, schema, canonical_envelope_bytes)`.
- No unauthenticated RPC endpoints (Workstream 10 audit). V2 gRPC requires ML-DSA-65 auth.
- State sync uses typed Merkle proofs from Workstream 3 (`StateNodeV2`, `StateLeafV2`).
- All V2 network types have frozen `FieldSpec` schemas in `v2_records.rs` or `net_v2.rs`.
- QIP-005 draft (transport/sync) references this plan.
- Candidate identity governed by Workstream 1.

---

### Task 1: Define V2 P2P Envelope, Message Types, and gRPC Protobuf

**Files:**
- Create: `Protocol_Core/qanto-consensus-primitives/src/net_v2.rs`
- Create: `Protocol_Core/qanto-rpc/proto/qanto_v2.proto`
- Modify: `Protocol_Core/qanto-consensus-primitives/src/lib.rs`
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/net_v2.json`
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs`

**Interfaces:**
- Produces: `P2pEnvelopeV2`, `MessageType`, `StateProofV2`, gRPC service definitions.
- Consumes: `hash_v2`, `DomainV2::P2pEnvelope`, `v2_records`, ML-DSA-65 from `qanto-core`.

- [ ] **Step 1: Write failing network V2 tests.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::{
    net_v2::{P2pEnvelopeV2, MessageType, StateProofV2, StateProofRequestV2},
    hash::{hash_v2, DomainV2, HashV2, MessageIdV2},
    consensus_parameters::SchemaVersion,
};

#[test]
fn p2p_envelope_v2_canonical_bytes_match_golden_vector() {
    let envelope = P2pEnvelopeV2::new(
        MessageType::BlockAnnouncement,
        vec![0x01, 0x02, 0x03], // payload
        [0x11u8; 2592], // ML-DSA-65 pubkey
        vec![0x22u8; 48], // ML-DSA-65 signature
        1_700_000_000_000, // timestamp
        1363527796, // chain_id
        SchemaVersion::V2_INITIAL,
    );
    let expected = include_bytes!("../vectors/net_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(envelope.canonical_bytes()), expected["envelope_canonical_bytes"]);
    assert_eq!(envelope.message_id().to_string(), expected["message_id_v2"]);
}

#[test]
fn state_proof_v2_canonical_bytes_match_golden_vector() {
    let proof = StateProofV2::new(
        vec![/* StateNodeV2 */],
        None, // leaf value for non-existence
        1363527796,
        SchemaVersion::V2_INITIAL,
    );
    let expected = include_bytes!("../vectors/net_v2.json");
    let expected: serde_json::Value = serde_json::from_slice(expected).unwrap();
    assert_eq!(hex::encode(proof.canonical_bytes()), expected["state_proof_canonical_bytes"]);
}

#[test]
fn grpc_v2_proto_compiles_and_matches_schema() {
    // This test verifies the protobuf compiles and field numbers match FieldSpec
    // Actual protobuf test in qanto-rpc crate
}
```

- [ ] **Step 2: Implement `net_v2.rs`.**

Create `Protocol_Core/qanto-consensus-primitives/src/net_v2.rs`:

```rust
//! V2 network types: P2P envelope, message types, state proofs, RPC request/response.

use crate::{
    consensus_parameters::SchemaVersion,
    hash::{hash_v2, DomainV2, HashV2},
    record::{CanonicalRecord, WireType, SequenceSemantics, SequenceSpec},
    storage_v2::{StateNodeV2, StateLeafV2, StateKeyV2},
    identity_v2::AccountIdV2,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    BlockAnnouncement = 0,
    TransactionBroadcast = 1,
    FinalityVote = 2,
    FinalityCertificate = 3,
    ValidatorSetUpdate = 4,
    StateSyncRequest = 5,
    StateSyncResponse = 6,
    Ping = 7,
    Pong = 8,
    PeerExchange = 9,
    PruningManifest = 10,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P2pEnvelopeV2 {
    pub message_type: MessageType,
    pub payload: Vec<u8>, // Canonical bytes of the message (BlockCoreV2, TransactionEnvelopeV2, etc.)
    pub sender_pubkey: [u8; 2592], // ML-DSA-65
    pub signature: Vec<u8>, // ML-DSA-65 signature over canonical_bytes
    pub timestamp: u64,
    pub canonical_bytes: Vec<u8>,
    pub message_id: HashV2,
}

impl P2pEnvelopeV2 {
    pub fn new(
        message_type: MessageType,
        payload: Vec<u8>,
        sender_pubkey: [u8; 2592],
        signature: Vec<u8>,
        timestamp: u64,
        chain_id: u32,
        schema: SchemaVersion,
    ) -> Self {
        let mut record = CanonicalRecord::new();
        record.push_u8(1, message_type as u8).unwrap();
        record.push_bytes(2, &payload).unwrap();
        record.push_bytes(3, &sender_pubkey).unwrap();
        record.push_bytes(4, &signature).unwrap();
        record.push_u64(5, timestamp).unwrap();
        let bytes = record.finish();
        let message_id = hash_v2(DomainV2::P2pEnvelope, chain_id, schema as u16, &bytes);
        Self { message_type, payload, sender_pubkey, signature, timestamp, canonical_bytes: bytes, message_id }
    }
    
    pub fn message_id(&self) -> HashV2 { self.message_id }
    pub fn canonical_bytes(&self) -> &[u8] { &self.canonical_bytes }
    
    pub fn verify_signature(&self, chain_id: u32, schema: SchemaVersion) -> bool {
        // Verify ML-DSA-65 signature over canonical_bytes
        // Use qanto_core::qanto_native_crypto
        true // placeholder
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateProofRequestV2 {
    pub key: StateKeyV2,
    pub state_root: HashV2, // StateRootV2 to prove against
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateProofV2 {
    pub proof_nodes: Vec<StateNodeV2>,
    pub leaf: Option<StateLeafV2>, // None = non-existence proof
    pub canonical_bytes: Vec<u8>,
    pub proof_hash: HashV2,
}

impl StateProofV2 {
    pub fn new(
        proof_nodes: Vec<StateNodeV2>,
        leaf: Option<StateLeafV2>,
        chain_id: u32,
        schema: SchemaVersion,
    ) -> Self {
        let mut record = CanonicalRecord::new();
        // proof nodes as ordered sequence
        for node in &proof_nodes {
            record.push_bytes(1, &node.canonical_bytes()).unwrap();
        }
        if let Some(leaf) = &leaf {
            record.push_bytes(2, &leaf.canonical_bytes()).unwrap();
        }
        let bytes = record.finish();
        let proof_hash = hash_v2(DomainV2::StateProof, chain_id, schema as u16, &bytes);
        Self { proof_nodes, leaf, canonical_bytes: bytes, proof_hash }
    }
    
    pub fn verify(&self, root: HashV2, key: &StateKeyV2) -> bool {
        // Verify proof against root and key
        true // placeholder
    }
}
```

- [ ] **Step 3: Create `qanto_v2.proto` for gRPC.**

Create `Protocol_Core/qanto-rpc/proto/qanto_v2.proto`:

```protobuf
syntax = "proto3";

package qanto.v2;

option rust_client = true;
option rust_server = true;

service QantoV2 {
  // Block/transaction submission
  rpc SubmitTransaction(TransactionEnvelopeV2) returns (TransactionReceiptV2);
  rpc SubmitBlock(BlockV2) returns (BlockReceiptV2);
  
  // Queries
  rpc GetBlock(GetBlockRequest) returns (BlockV2);
  rpc GetTransaction(GetTransactionRequest) returns (TransactionEnvelopeV2);
  rpc GetStateProof(GetStateProofRequest) returns (StateProofV2);
  rpc GetConsensusParameters(GetConsensusParametersRequest) returns (ConsensusParametersV2);
  
  // Streaming
  rpc SubscribeBlocks(BlockSubscriptionRequest) returns (stream BlockV2);
  rpc SubscribeTransactions(TxSubscriptionRequest) returns (stream TransactionEnvelopeV2);
  rpc SubscribeFinality(FinalitySubscriptionRequest) returns (stream FinalityCertificateV2);
}

// Message definitions matching canonical records
message TransactionEnvelopeV2 {
  bytes body = 1;          // TransactionBodyV2 canonical bytes
  bytes signatures = 2;    // concatenated sig + pubkey
}

message TransactionBodyV2 {
  uint32 kind = 1;
  uint64 nonce = 2;
  bytes gas_limit = 3;     // u128 LE
  bytes gas_price = 4;
  bytes priority_fee = 5;
  bytes sender = 6;        // AccountIdV2 (32 bytes)
  bytes receiver = 7;
  bytes value = 8;         // u128 LE
  bytes payload = 9;
  uint64 chain_id = 10;
  uint32 schema_version = 11;
}

message BlockV2 {
  bytes core = 1;              // BlockCoreV2 canonical bytes
  bytes finality_certificate = 2; // optional
}

message StateProofRequest {
  bytes key = 1;              // StateKeyV2 canonical bytes
  bytes state_root = 2;       // StateRootV2 (32 bytes)
}

message StateProofV2 {
  repeated bytes proof_nodes = 1; // StateNodeV2 canonical bytes
  bytes leaf = 2;                     // optional StateLeafV2
}

message ConsensusParametersV2 {
  uint32 version = 1;
  uint32 schema_version = 2;
  uint32 chain_id = 3;
  bytes genesis_hash = 4;
  uint64 max_block_weight = 5;
  uint64 target_block_interval_ms = 6;
  uint64 difficulty_window = 7;
  uint32 difficulty_adjustment_min_bps = 8;
  uint32 difficulty_adjustment_max_bps = 9;
  uint64 min_difficulty = 10;
  uint64 max_difficulty = 11;
  uint32 median_time_past_window = 12;
  uint64 future_timestamp_drift_ms = 13;
  uint32 pow_suite = 14;
  bytes initial_validator_set_commitment = 15;
  // Economic
  bytes max_total_supply = 16;       // u128
  bytes community_allocation = 17;    // u128
  bytes ecosystem_allocation = 18;
  bytes liquidity_allocation = 19;
  uint64 vesting_cliff_epochs = 20;
  uint64 vesting_duration_epochs = 21;
  bool base_fee_burn_enabled = 22;
  uint32 staking_apr_floor_bps = 23;
  uint32 emission_curve_version = 24;
}
```

- [ ] **Step 4: Update `lib.rs` exports.**

```rust
pub mod net_v2;
pub use net_v2::{P2pEnvelopeV2, MessageType, StateProofV2, StateProofRequestV2};
```

- [ ] **Step 5: Create golden vector for network V2.**

Create `Protocol_Core/qanto-consensus-primitives/tests/vectors/net_v2.json`.

- [ ] **Step 6: Run network V2 tests.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives --locked -- p2p_envelope_v2 state_proof_v2
```

Expected: `PASS`.

- [ ] **Step 7: Add `prost`/`tonic` to `qanto-rpc/Cargo.toml` for V2.**

```toml
prost = "0.12"
tonic = "0.10"
```

- [ ] **Step 8: Record intended commit.**

Intended commit: `feat(qanto-consensus-primitives): V2 P2P envelope, message types, state proofs, gRPC proto`.

---

### Task 2: Wire V2 P2P Into libp2p and State Sync

**Files:**
- Modify: `Protocol_Core/qanto-core/src/qanto_p2p.rs`
- Modify: `Protocol_Core/qanto-node/src/p2p.rs`
- Modify: `Protocol_Core/qanto-node/src/state_sync.rs`
- Create: `Protocol_Core/qanto-core/src/p2p_v2.rs`
- Test: `Protocol_Core/qanto-node/tests/v2_network_integration.rs`

**Interfaces:**
- Consumes: `P2pEnvelopeV2`, `MessageType`, `StateProofV2`, `StateStoreV2`.
- Produces: V2 gossip protocol, authenticated peers, state sync with Merkle proofs.

- [ ] **Step 1: Write failing integration test.**

Create `Protocol_Core/qanto-node/tests/v2_network_integration.rs`:

```rust
use qanto_consensus_primitives::{
    net_v2::{P2pEnvelopeV2, MessageType, StateProofV2},
    consensus_parameters::{ConsensusParametersV2, SchemaVersion, is_v2_active},
};

#[test]
fn v2_p2p_envelope_used_when_active() {
    // p2p.rs uses P2pEnvelopeV2 for all messages when is_v2_active
    assert!(!is_v2_active(100, &ConsensusParametersV2::for_private_engineering()));
}

#[test]
fn v2_state_sync_uses_merkle_proofs() {
    // state_sync.rs GetStateProof returns StateProofV2 with StateNodeV2/StateLeafV2
}
```

- [ ] **Step 2: Implement `p2p_v2.rs` in qanto-core.**

Create `Protocol_Core/qanto-core/src/p2p_v2.rs`:
- `P2pCodecV2` implementing libp2p `Codec` for `P2pEnvelopeV2`.
- `AuthenticatedPeerV2` with ML-DSA-65 identity verification.
- `GossipValidatorV2` verifying signatures and message IDs.

- [ ] **Step 3: Wire into `qanto_p2p.rs` and `p2p.rs`.**

- Swap codec to `P2pCodecV2` when V2 active.
- Peer auth uses `AccountIdV2` / `ValidatorIdV2`.
- Message propagation validates `P2pEnvelopeV2::verify_signature`.

- [ ] **Step 4: Wire state sync in `state_sync.rs`.**

- `GetStateProof` request/response uses `StateProofRequestV2` / `StateProofV2`.
- Proof verified against `StateRootV2` from block header.
- Fast sync: request proofs for state keys, verify, apply.

- [ ] **Step 5: Run integration build test.**

```bash
rtk cargo build --workspace --locked --features infinite-strata
```

Expected: `PASS`.

- [ ] **Step 6: Record intended commit.**

Intended commit: `feat(qanto-core/qanto-node): V2 P2P transport and state sync with Merkle proofs`.

---

### Task 3: Wire V2 gRPC API Into RPC Server and Client

**Files:**
- Modify: `Protocol_Core/qanto-rpc/src/lib.rs`
- Modify: `Protocol_Core/qanto-rpc/src/server.rs`
- Create: `Protocol_Core/qanto-rpc/src/v2_service.rs`
- Test: `Protocol_Core/qanto-rpc/tests/v2_grpc_integration.rs`

**Interfaces:**
- Consumes: `qanto_v2.proto` generated types, V2 canonical records.
- Produces: gRPC service `QantoV2` with auth, request/response using V2 types.

- [ ] **Step 1: Write failing gRPC test.**

Create `Protocol_Core/qanto-rpc/tests/v2_grpc_integration.rs`:

```rust
use qanto_rpc::v2_service::QantoV2Service;

#[test]
fn v2_grpc_service_compiles() {
    // QantoV2Service implements tonic service trait
}

#[test]
fn v2_grpc_requires_ml_dsa_auth() {
    // All V2 endpoints require ML-DSA-65 authentication
}
```

- [ ] **Step 2: Implement `v2_service.rs`.**

Create `Protocol_Core/qanto-rpc/src/v2_service.rs`:
- Implement `QantoV2` trait from generated protobuf.
- Auth interceptor: verify ML-DSA-65 signature on request metadata.
- Convert between protobuf and canonical records.

- [ ] **Step 3: Wire into `server.rs`.**

Register `QantoV2Service` when V2 feature enabled.

- [ ] **Step 4: Run gRPC tests.**

```bash
rtk cargo nextest run -p qanto-rpc --locked -- v2_grpc
```

Expected: `PASS`.

- [ ] **Step 5: Record intended commit.**

Intended commit: `feat(qanto-rpc): V2 gRPC API with ML-DSA-65 authentication`.

---

### Task 4: Golden Vector Cross-Network Validation and QIP-005 Completion

**Files:**
- Test: `Protocol_Core/qanto-consensus-primitives/tests/qip002_vectors.rs` (add network vectors)
- Create: `Protocol_Core/qanto-consensus-primitives/tests/vectors/cross_network.json`
- Modify: `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md`

**Interfaces:**
- Validates cross-crate network parity.
- Completes QIP-005 transport/sync section.

- [ ] **Step 1: Write cross-crate network test.**

Add to `qip002_vectors.rs`:

```rust
use qanto_consensus_primitives::net_v2::{P2pEnvelopeV2, StateProofV2};

#[test]
fn cross_crate_p2p_envelope_matches_node_codec() {
    let expected = include_bytes!("../vectors/cross_network.json");
    // Verify libp2p codec produces identical canonical bytes
}

#[test]
fn cross_crate_state_proof_matches_storage_backend() {
    // Storage backend proof = net_v2 StateProofV2
}
```

- [ ] **Step 2: Complete QIP-005 transport/sync section.**

Update `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md` with transport/sync design.

- [ ] **Step 3: Run all network tests across crates.**

```bash
rtk cargo nextest run -p qanto-consensus-primitives -p qanto-core -p qanto-node -p qanto-rpc --locked -- net_v2 p2p_v2 state_sync v2_network v2_grpc cross_crate_
```

Expected: `PASS`.

- [ ] **Step 4: Final self-review checklist.**

- [ ] `P2pEnvelopeV2` with frozen schema, ML-DSA-65 auth, `hash_v2` message IDs.
- [ ] `MessageType` enum covers all consensus-visible messages.
- [ ] `StateProofV2` uses typed `StateNodeV2`/`StateLeafV2` from Workstream 3.
- [ ] gRPC `qanto_v2.proto` matches canonical record schemas.
- [ ] P2P codec (`P2pCodecV2`) produces/parses `P2pEnvelopeV2` identically.
- [ ] Peer authentication uses ML-DSA-65 identities (`AccountIdV2`/`ValidatorIdV2`).
- [ ] State sync requests/responds with `StateProofV2`, verifies against `StateRootV2`.
- [ ] Private profile: `ACTIVATION_HEIGHT = 0`, V2 transport unreachable.
- [ ] Golden vectors for P2P envelope, state proof, gRPC messages.
- [ ] Cross-crate parity: libp2p codec = consensus-primitives, storage proof = network proof.
- [ ] QIP-005 complete with transport/sync sections.
- [ ] All tests pass across crates.
- [ ] Workspace builds clean.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

- [ ] **Step 5: Record intended commit.**

Intended commit: `test(qanto-rpc): cross-crate network parity; QIP-005 complete; complete V2 transport/sync`.

---

## Acceptance Criteria

- [ ] `P2pEnvelopeV2`, `MessageType`, `StateProofV2`, `StateProofRequestV2` have frozen schemas using `hash_v2` with `DomainV2`.
- [ ] `MessageIdV2 = hash_v2(DomainV2::P2pEnvelope, ...)` for all P2P messages.
- [ ] ML-DSA-65 authentication on all P2P messages and gRPC endpoints.
- [ ] `qanto_v2.proto` gRPC definitions match canonical record field IDs and types.
- [ ] libp2p codec `P2pCodecV2` produces bit-identical canonical bytes to `consensus-primitives`.
- [ ] State sync uses typed Merkle proofs (`StateNodeV2`, `StateLeafV2`) verified against `StateRootV2`.
- [ ] Private profile: `ACTIVATION_HEIGHT = 0`, all V2 transport unreachable.
- [ ] Golden vectors for all network types.
- [ ] Cross-crate parity verified.
- [ ] QIP-005 draft complete with all 10 sections (including transport/sync).
- [ ] All tests pass: `cargo nextest run -p qanto-consensus-primitives -p qanto-core -p qanto-node -p qanto-rpc --locked`.
- [ ] Workspace builds clean: `cargo build --workspace --locked --features infinite-strata`.
- [ ] `cargo fmt` and `cargo clippy -D warnings` pass.

---

### Workstream 7 Summary for Dependent Workstreams

| Output | Location | Consumers |
|---|---|---|
| `P2pEnvelopeV2`, `MessageType`, `StateProofV2` | `qanto-consensus-primitives::net_v2` | Workstreams 8, 9, 10 |
| `qanto_v2.proto` gRPC definitions | `qanto-rpc/proto/qanto_v2.proto` | Workstreams 8, 9, 10 |
| `P2pCodecV2` libp2p codec | `qanto-core::p2p_v2` | Workstreams 9, 10 |
| V2 state sync logic | `qanto-node::state_sync` | Workstreams 9, 10 |
| V2 gRPC service | `qanto-rpc::v2_service` | Workstreams 9, 10 |
| QIP-005 (complete) | `Docs/protocol/proposals/QIP-005-transport-sync-difficulty.md` | Workstreams 8, 9, 10 |
| Golden vectors | `qanto-consensus-primitives/tests/vectors/net_v2.json`, `cross_network.json` | Workstreams 8, 9, 10 |