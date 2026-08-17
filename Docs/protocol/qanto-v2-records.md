# Qanto V2 Canonical Records Specification

This document specifies the canonical record schemas, domains, and hash construction for the Qanto V2 protocol.

## Domain Registry

The V2 protocol uses an append-only domain registry with codes 0x0001-0x0082. Each domain corresponds to a specific type of consensus-visible object.

### Domain Codes

| Domain | Code | Description |
|--------|------|-------------|
| TxBody | 0x0001 | Transaction body (unsigned) |
| TxSigning | 0x0002 | Transaction signing data |
| TxEnvelope | 0x0003 | Transaction envelope (body + signatures) |
| UtxoOutpoint | 0x0004 | UTXO output reference |
| BlockCore | 0x0005 | Block header (without signatures) |
| PowWork | 0x0006 | Proof-of-work input (block core hash + nonce) |
| BlockSigning | 0x0007 | Block signing data |
| BlockId | 0x0008 | Block identifier |
| TxMerkleEmpty | 0x0009 | Empty transaction Merkle tree |
| TxMerkleLeaf | 0x000A | Transaction Merkle leaf |
| TxMerkleNode | 0x000B | Transaction Merkle internal node |
| TxMerkleRoot | 0x000C | Transaction Merkle root |
| StateKey | 0x000D | State tree key |
| StateLeaf | 0x000E | State tree leaf (value hash) |
| StateNode | 0x000F | State tree internal node |
| StateRoot | 0x0010 | State tree root |
| AccountId | 0x0011 | Account identifier (ML-DSA-65 derived) |
| ValidatorId | 0x0012 | Validator identifier (ML-DSA-65 derived) |
| PeerId | 0x0013 | Peer-to-peer node identifier |
| FinalityVote | 0x0014 | Consensus finality vote |
| FinalityCertificate | 0x0015 | Aggregated finality certificate |
| ValidatorSet | 0x0016 | Set of active validators |
| BridgeClaim | 0x0017 | Cross-chain bridge claim |
| BridgeNullifier | 0x0018 | Cross-chain bridge nullifier |
| ZkVerifyingKey | 0x0019 | Zero-knowledge verifying key |
| P2pEnvelope | 0x001A | Peer-to-peer message envelope |
| SnapshotManifest | 0x001B | State snapshot manifest |
| GenesisManifest | 0x001C | Chain genesis manifest |
| ConsensusParameters | 0x001D | Consensus parameters |
| DomainRegistry | 0x001E | The domain registry itself |

## Canonical Record Schemas

Each domain has a corresponding `FieldSpec` array that defines its canonical encoding schema.

### Transaction Body (TxBody)

```rust
pub const TX_BODY_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::U8, required: true, sequence: None }, // kind
    FieldSpec { id: 2, wire_type: WireType::U64, required: true, sequence: None }, // nonce
    FieldSpec { id: 3, wire_type: WireType::U128, required: true, sequence: None }, // gas_limit
    FieldSpec { id: 4, wire_type: WireType::U128, required: true, sequence: None }, // gas_price
    FieldSpec { id: 5, wire_type: WireType::U128, required: true, sequence: None }, // priority_fee
    FieldSpec { id: 6, wire_type: WireType::Hash32, required: true, sequence: None }, // sender (AccountIdV2)
    FieldSpec { id: 7, wire_type: WireType::Hash32, required: true, sequence: None }, // receiver (AccountIdV2 or 0 for coinbase)
    FieldSpec { id: 8, wire_type: WireType::U128, required: true, sequence: None }, // value
    FieldSpec { id: 9, wire_type: WireType::Bytes, required: true, sequence: None }, // payload (for EVMCall etc.)
    FieldSpec { id: 10, wire_type: WireType::U64, required: true, sequence: None }, // chain_id
    FieldSpec { id: 11, wire_type: WireType::U16, required: true, sequence: None }, // schema_version
];
```

### Transaction Signing (TxSigning)

```rust
pub const TX_SIGNING_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // tx_body_hash (TxIdV2 as bytes)
    FieldSpec { id: 2, wire_type: WireType::Bytes, required: true, sequence: None }, // public_key (ML-DSA-65)
    FieldSpec { id: 3, wire_type: WireType::Bytes, required: true, sequence: None }, // signature
];
```

### Transaction Envelope (TxEnvelope)

```rust
pub const TX_ENVELOPE_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // tx_body_hash
    FieldSpec { id: 2, wire_type: WireType::Sequence, required: true, sequence: Some(SequenceSpec { element_type: WireType::Bytes, semantics: SequenceSemantics::Ordered }) }, // [signature, public_key] or [r,s,...] per suite
];
```

### Block Core (BlockCore)

```rust
pub const BLOCK_CORE_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::U8, required: true, sequence: None }, // version
    FieldSpec { id: 2, wire_type: WireType::Hash32, required: true, sequence: None }, // parent_block_id
    FieldSpec { id: 3, wire_type: WireType::Hash32, required: true, sequence: None }, // state_root
    FieldSpec { id: 4, wire_type: WireType::Hash32, required: true, sequence: None }, // transactions_root (TxMerkleRoot)
    FieldSpec { id: 5, wire_type: WireType::U64, required: true, sequence: None }, // timestamp
    FieldSpec { id: 6, wire_type: WireType::U64, required: true, sequence: None }, // difficulty
    FieldSpec { id: 7, wire_type: WireType::U128, required: true, sequence: None }, // chain_work (cumulative work as u128)
    FieldSpec { id: 8, wire_type: WireType::Hash32, required: true, sequence: None }, // validator_id
    FieldSpec { id: 9, wire_type: WireType::Hash32, required: true, sequence: None }, // miner_id
    FieldSpec { id: 10, wire_type: WireType::U64, required: true, sequence: None }, // height
];
```

### Proof of Work (PowWork)

```rust
pub const POW_WORK_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // block_core_hash (HashV2 from BlockCore)
    FieldSpec { id: 2, wire_type: WireType::U64, required: true, sequence: None }, // nonce
];
```

### Block Id (BlockId)

```rust
pub const BLOCK_ID_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // block_core_hash
];
```

### Merkle Tree Components

```rust
pub const TX_MERKLE_EMPTY_SPECS: &[FieldSpec] = &[]; // Unit record (0 fields) -> 0x0000
pub const TX_MERKLE_LEAF_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // tx_id (TxIdV2)
];
pub const TX_MERKLE_NODE_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // left
    FieldSpec { id: 2, wire_type: WireType::Hash32, required: true, sequence: None }, // right
];
pub const TX_MERKLE_ROOT_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // root_hash
];
```

### State Tree Components

```rust
pub const STATE_KEY_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Bytes, required: true, sequence: None }, // key bytes
];
pub const STATE_LEAF_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // value hash
];
pub const STATE_NODE_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // left
    FieldSpec { id: 2, wire_type: WireType::Hash32, required: true, sequence: None }, // right
];
pub const STATE_ROOT_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Hash32, required: true, sequence: None }, // root hash
];
```

### Identity Components

```rust
pub const ACCOUNT_ID_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Bytes, required: true, sequence: None }, // ML-DSA-65 public key (2592 bytes)
];
pub const VALIDATOR_ID_SPECS: &[FieldSpec] = &[
    FieldSpec { id: 1, wire_type: WireType::Bytes, required: true, sequence: None }, // ML-DSA-65 public key
];
```

## Canonical Record Builder Functions

Each domain has a corresponding builder function that constructs a `CanonicalRecord` ready for `.finish()` and `hash_v2()`.

### Transaction Body Builder

```rust
/// Build the canonical TransactionBodyV2 record.
// kind: 0=Transfer, 1=Coinbase, 2=EVMCall (excluded from private profile), 3=BridgeCall (excluded).
pub fn transaction_body_v2(
    kind: u8,
    nonce: u64,
    gas_limit: u128,
    gas_price: u128,
    priority_fee: u128,
    sender: HashV2,  // AccountIdV2 from hash_v2(AccountId, chain, schema, pubkey)
    receiver: HashV2, // AccountIdV2 or ZERO for coinbase
    value: u128,
    payload: &[u8],
    chain_id: u32,
    schema: SchemaVersion,
) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_u8(1, kind).unwrap();
    record.push_u64(2, nonce).unwrap();
    record.push_u128(3, gas_limit).unwrap();
    record.push_u128(4, gas_price).unwrap();
    record.push_u128(5, priority_fee).unwrap();
    record.push_hash32(6, sender.as_bytes()).unwrap();
    record.push_hash32(7, receiver.as_bytes()).unwrap();
    record.push_u128(8, value).unwrap();
    record.push_bytes(9, payload).unwrap();
    record.push_u32(10, chain_id).unwrap();
    record.push_u16(11, schema as u16).unwrap();
    record
}
```

### Block Core Builder

```rust
/// Build the canonical BlockCoreV2 record.
pub fn block_core_v2(
    version: u8,
    parent_block_id: HashV2,
    state_root: HashV2,
    transactions_root: HashV2,
    timestamp: u64,
    difficulty: u64,
    chain_work: u128,
    validator_id: HashV2,
    miner_id: HashV2,
    height: u64,
) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_u8(1, version).unwrap();
    record.push_hash32(2, parent_block_id.as_bytes()).unwrap();
    record.push_hash32(3, state_root.as_bytes()).unwrap();
    record.push_hash32(4, transactions_root.as_bytes()).unwrap();
    record.push_u64(5, timestamp).unwrap();
    record.push_u64(6, difficulty).unwrap();
    record.push_u128(7, chain_work).unwrap();
    record.push_hash32(8, validator_id.as_bytes()).unwrap();
    record.push_hash32(9, miner_id.as_bytes()).unwrap();
    record.push_u64(10, height).unwrap();
    record
}
```

### Identity Builders

```rust
/// Build the canonical AccountId record from ML-DSA-65 public key.
pub fn account_id_record(pubkey: &[u8; 2592]) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_bytes(1, pubkey).unwrap();
    record
}

/// Build the canonical ValidatorId record from ML-DSA-65 public key.
pub fn validator_id_record(pubkey: &[u8; 2592]) -> CanonicalRecord {
    let mut record = CanonicalRecord::new();
    record.push_bytes(1, pubkey).unwrap();
    record
}
```

## Hash Construction

All V2 objects use the `hash_v2` function for cryptographic hashing.

### Hash Frame Layout

```
hash_v2 frame: HASH_V2_MAGIC (8) | suite:u16 | domain:u16 | chain_id:u32 | schema:u16 | payload_len:u64 | payload (all big-endian)
```

Where:
- `HASH_V2_MAGIC` = `0x51414e544f2d5632` ("QANTO-V2" in ASCII)
- `suite` = `0x0001` for SHA3-256
- `domain` = DomainV2 enum value (see Domain Registry above)
- `chain_id` = 32-bit chain identifier
- `schema` = 16-bit schema version
- `payload_len` = 64-bit length of the payload in bytes
- `payload` = canonical bytes from the appropriate builder function

### Example: Transaction Body Hash

```rust
let body_record = transaction_body_v2(
    /* kind */ 1,
    /* nonce */ 0,
    /* gas_limit */ 1_000_000_000_000,
    /* gas_price */ 1,
    /* priority_fee */ 0,
    /* sender */ sender_hash, // AccountIdV2
    /* receiver */ HashV2::ZERO, // coinbase
    /* value */ 50_000_000_000_000_000,
    /* payload */ &[],
    /* chain_id */ 0x5141_4E54,
    /* schema */ SchemaVersion::V2_INITIAL,
);
let body_bytes = body_record.finish();
let tx_id = TxIdV2::from_hash(hash_v2(
    DomainV2::TxBody,
    0x5141_4E54, // chain_id
    SchemaVersion::V2_INITIAL as u16,
    &body_bytes,
));
```

## Schema Versioning Policy

- Schema version (`u16`) commits to the exact `FieldSpec` array for each domain
- Any schema change requires a new schema version
- New domains require both a new domain code (via QIP) and a new schema version
- The domain registry is append-only: domains never change or get removed
- Schema versions are incremented for any change to FieldSpec arrays

## Chain and Domain Separation

The hash construction ensures proper separation:
- Different chain IDs produce different hashes for the same payload
- Different schema versions produce different hashes for the same payload  
- Different domains produce different hashes for the same payload

This prevents cross-chain, cross-schema, and cross-domain replay attacks.

## Identity Derivation

Account and validator identifiers are derived from ML-DSA-65 public keys:

```rust
// AccountIdV2 = hash_v2(DomainV2::AccountId, chain_id, schema, canonical_record(pubkey))
pub fn account_id_v2(pubkey: &[u8; 2592], chain_id: u32, schema: SchemaVersion) -> AccountIdV2 {
    let record = account_id_record(pubkey);
    let bytes = record.finish();
    AccountIdV2::from_hash(hash_v2(
        DomainV2::AccountId,
        chain_id,
        schema as u16,
        &bytes,
    ))
}

// ValidatorIdV2 = hash_v2(DomainV2::ValidatorId, chain_id, schema, canonical_record(pubkey))
pub fn validator_id_v2(pubkey: &[u8; 2592], chain_id: u32, schema: SchemaVersion) -> ValidatorIdV2 {
    let record = validator_id_record(pubkey);
    let bytes = record.finish();
    ValidatorIdV2::from_hash(hash_v2(
        DomainV2::ValidatorId,
        chain_id,
        schema as u16,
        &bytes,
    ))
}
```

## Constant-Time Comparison

Identity comparison uses constant-time operations to prevent timing attacks:

```rust
pub fn identities_equal_constant_time<T: AsRef<[u8]> + ?Sized>(a: &T, b: &T) -> bool {
    use subtle::ConstantTimeEq;
    a.as_ref().ct_eq(b.as_ref()).into()
}
```

## Activation Boundary

V2 consensus rules are governed by an activation height:

```rust
pub const ACTIVATION_HEIGHT: u64 = 0; // 0 = unreachable in private profile

pub fn is_v2_active(height: u64, params: &ConsensusParametersV2) -> bool {
    ACTIVATION_HEIGHT > 0 && height >= ACTIVATION_HEIGHT && params.version == ProtocolVersion::V2
}
```

In the private engineering profile, `ACTIVATION_HEIGHT = 0`, making V2 unreachable regardless of chain height.