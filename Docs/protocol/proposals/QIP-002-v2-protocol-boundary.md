---
qip: QIP-002
status: REQUIRES PROTOCOL APPROVAL
changes:
  - consensus
  - cryptography
  - compatibility
  - chain_id
activation: unreachable
migration: new-v2-genesis
rollback: replace-prelaunch-candidate
---

# QIP-002: Qanto V2 Protocol Boundary

## Abstract

This QIP proposes the Qanto V2 protocol as a canonical, versioned successor to the current V1 protocol. V2 introduces canonical transaction and block encoding, ML-DSA-65 based identity, strict domain separation in hashing, and an explicit activation boundary requiring protocol upgrade consensus.

## 1. Current Behavior (V1 Legacy Encoding)

The current V1 protocol exhibits several limitations:
- Mixed encoding schemes: JSON, bincode, manual byte layout
- No domain separation in hash construction (same hash function used for different purposes)
- Mutable global state (e.g., GLOBAL_CHAIN_ID)
- Legacy Merkle tree implementation using string concatenation
- Coupling with experimental subsystems (ΩMEGA, SAGA)
- Inconsistent endianness handling across platforms
- No explicit schema versioning for consensus-critical structures
- Identity derivation uses legacy address formats instead of ML-DSA-65

## 2. Proven Defects

Analysis of the V1 implementation reveals several concrete defects:
- Non-canonical encodings: Different code paths produce different byte representations for the same logical object
- Hash collisions possible: Lack of domain separation increases collision risk between different object types
- No chain separation: Same payload produces same hash on different chains
- Mutable difficulty: Difficulty parameters can be changed without proper versioning
- Legacy fallbacks: EVM transaction bypass creates inconsistent validation paths
- Global state coupling: ΩMEGA/SAGA subsystems affect core consensus logic

## 3. Security and Economic Impact

The V1 defects create several security and economic risks:
- **Consensus risk**: Non-deterministic encoding could lead to chain splits
- **Replay attack vulnerability**: Missing chain/domain/replay protection
- **Griefing vectors**: Mutable globals enable denial-of-service attacks
- **Economic instability**: Unpredictable fee mechanics due to inconsistent encoding
- **Privacy leakage**: Legacy identity formats may reveal unintended information

## 4. Alternative Designs Considered

### Incremental vs Atomic Migration
- **Incremental**: Gradually replace V1 components with V2 equivalents
  - Pros: Lower coordination complexity, backward compatibility during transition
  - Cons: Long-term maintenance of two encoding systems, transitional security risks
- **Atomic (chosen)**: Clean break via new genesis block
  - Pros: Clean security boundary, no transitional complexity, clear upgrade path
  - Cons: Requires coordinated migration, temporary interruption during cutover

### JSON vs Binary Canonical
- **JSON**: Human-readable, self-describing
  - Pros: Debuggability, tool compatibility
  - Cons: Verbose, non-deterministic whitespace, parsing complexity
- **Binary Canonical (chosen)**: Strictly defined byte layout
  - Pros: Deterministic, minimal size, fast parsing, cryptographic agility
  - Cons: Requires specialized tooling for inspection

## 5. Simulations and Measurements

### Parity Tests
All V2 implementation paths have been validated to produce identical byte representations:
- Miner and validator use identical PoW oracles (`qanhash_v2`)
- Validation and mining use identical target arithmetic (`work_target_v2`)
- Transaction ID parity: body → signing → envelope → replay all yield same TxIdV2
- Block ID parity: validation ↔ replay ↔ sync all yield same BlockIdV2
- Merkle root parity: mempool ↔ validation ↔ sync ↔ tooling all yield identical roots
- State root parity: execution ↔ verification paths identical

### Golden Vectors
Comprehensive test vectors validate canonical byte production:
- Consensus parameters canonical encoding and hash
- Domain registry canonical encoding and hash
- Transaction body/signing/envelope canonical bytes and derived IDs
- Block core canonical bytes, Merkle root, PoW work hash
- Account/validator/state root identities
- Cross-crate validation between reference implementation and node implementation

### Fuzz Testing
Structural fuzzing confirms:
- Malformed records fail closed (reject invalid input)
- Unknown fields and wire types are rejected
- Non-canonical sets (unsorted, duplicate elements) are rejected
- Length violations are properly handled

## 6. Recommended Design (V2 Primitives)

The V2 protocol implements the following canonical primitives:

### Consensus Parameters
- Fixed protocol version (V1/V2) and schema version
- Explicit activation height governing V2 consensus rules
- Canonically encoded and hashed via `DomainV2::ConsensusParameters`

### Canonical Records
- Every consensus-visible object has a predefined `FieldSpec` schema
- Builder functions produce `CanonicalRecord` instances ready for hashing
- No manual byte layout, bincode, or JSON encoding in V2 paths

### Hash Construction
- `hash_v2` with explicit domain separation
- Frame layout: `MAGIC | suite | domain | chain_id | schema | len | payload`
- Big-endian integers throughout
- Domain-separated: different domains → different hashes for same payload
- Chain-separated: different chains → different hashes for same payload
- Schema-separated: different schemas → different hashes for same payload

### Identity System
- ML-DSA-65 keys (2592 byte public keys) for accounts and validators
- `AccountIdV2 = hash_v2(AccountId, chain_id, schema, pubkey_record)`
- `ValidatorIdV2 = hash_v2(ValidatorId, chain_id, schema, pubkey_record)`
- Constant-time comparison to prevent timing attacks

### Merkle Trees
- Typed Merkle construction: `TxMerkleLeaf` → `TxMerkleNode` → `TxMerkleRoot`
- Empty tree: `TxMerkleEmpty` hashed to `TxMerkleRoot`
- No string-based Merkle construction

### Proof of Work
- `WorkHashV2 = hash_v2(PowWork, chain_id, schema, block_core_hash || nonce)`
- Target: `floor((2^256 - 1) / difficulty)` computed via exact 256-bit division
- Verification: inclusive big-endian integer comparison `work <= target`

### Block Identification
- `BlockIdV2 = hash_v2(BlockId, chain_id, schema, block_core_record)`
- Block core includes: version, parent_id, state_root, tx_root, timestamp, difficulty, chain_work, validator_id, miner_id, height

## 7. Compatibility Impact

### Breaking Changes
- V1 objects are invalid on V2 chain and vice versa
- Transaction and block IDs change format (still 32 bytes but different derivation)
- Identity format changes from legacy addresses to ML-DSA-65 derived identifiers
- Merkle proof format changes due to typed tree construction

### Mitigation Strategies
- **Offline migration tool**: Converts V1 genesis state to V2-compatible format
- **Clear signaling**: Nodes explicitly advertise supported protocol versions
- **Graceful degradation**: V2 nodes can still validate V1 historical blocks (pre-activation)

## 8. Migration Strategy

### New V2 Genesis
V2 launch requires a new genesis block with:
- Chain ID distinct from V1 chains
- Genesis hash committing to V2 consensus parameters
- Initial validator set derived from allocation manifest
- Distributed allocation manifest proving initial token distribution

### Allocation Manifest
Pre-launch process includes:
1. **Snapshot**: Record V1 token balances at cutover block
2. **Allocation**: Convert balances to V2 genesis allocation records
3. **Proof**: Generate cryptographic proof of correct allocation conversion
4. **Genesis**: Construct V2 genesis block with allocation manifest

## 9. Test Strategy

### Unit Tests
- Golden vector validation for all canonical record types
- Parity tests comparing implementation paths
- Property-based testing for encoding/decoding roundtrips
- Fuzz testing for malformed input handling

### Integration Tests
- Cross-crate validation between `qanto-consensus-primitives` and `qanto-node`
- Miner/validator parity in testnet environments
- Replay sync validation against historical blocks
- State root computation verification

### Simulation Tests
- Network split scenarios with mixed V1/V2 nodes
- Activation boundary testing with mock chain heights
- Difficulty adjustment validation at boundary conditions

### Chaos Testing
- Random network partitioning and node restart
- Message delay and reordering injection
- Byzantine validator simulation

## 10. Rollback Strategy

### Pre-Launch Only
- Rollback only possible before V2 activation height is reached
- Post-activation, downgrade is impossible without chain fork

### Replace Pre-Launch Candidate
- If issues discovered during testnet phase, replace V2 genesis candidate
- Requires new allocation manifest and genesis block
- Increment V2 schema version to distinguish candidates

### No Public Rollback
- Once V2 activates on mainnet, no protocol downgrade mechanism exists
- Chain continuation requires supermajority consensus for any changes
- Emergency fixes must be forward-compatible soft forks

## Conclusion

QIP-002 establishes the Qanto V2 protocol as a canonical, secure, and upgradeable foundation for future network evolution. By implementing strict canonical encoding, domain-separated hashing, ML-DSA-65 identity, and an explicit activation boundary, QIP-002 addresses critical defects in the V1 implementation while providing a clear path for future protocol improvements.