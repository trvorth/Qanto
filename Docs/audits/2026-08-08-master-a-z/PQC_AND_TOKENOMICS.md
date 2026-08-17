# Qanto PQC, Cryptography, QanHash, ZK, and Tokenomics Audit

Audit date: 2026-08-08  
Effective-tree reconciliation: 2026-08-09  
Audit target: effective working tree at `/Users/cps/qanto`  
Scope: mandate workstreams 3 and 4  
Execution mode: document author remained read-only for protocol source; concurrent shared-worktree remediation is recorded separately  

## Executive verdict

**NOT READY.** Workstreams 3 and 4 contain independent launch blockers:

1. Native ML-DSA signatures are cryptographically verified, but the signing key is not bound to the transaction sender or validator identity.
2. The canonical `qanto_hash` construction has trivial trailing-zero collisions.
3. CPU, OpenCL, CUDA, and Metal mining implementations do not implement the same algorithm.
4. The bridge has no real source-chain/ZK verifier. Concurrent remediation now fails the SDK closed and disables bridge claims in block validation, so unauthorized minting is contained at the cost of bridge availability.
5. Live block rewards bypass the deterministic `Emission` engine and instead depend on SAGA `f64` arithmetic and mutable local state.
6. The shipped testnet genesis allocates the full 21 billion QNTO while the emission counter starts at zero and is not restored from chain state.

The mathematically sealed token supply and allocation constants themselves are internally consistent and were not changed.

## Evidence labels

- `CONFIRMED`: directly established from current source or a reproduced build/test result.
- `DERIVED`: follows mathematically from confirmed source constants and stated assumptions.
- `MEASURED`: produced by an executed local check.
- `NEEDS_EXTERNAL_REVIEW`: cannot be certified solely from repository source and local self-tests.

---

# Workstream 3 — Cryptographic Layer and Post-Quantum Security

## 3.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| PQC-001 | SEV-1 | Native transaction authorization | `Protocol_Core/qanto-node/src/wallet.rs:141-200`, `:259-261`; `Protocol_Core/qanto-node/src/transaction.rs:496-548`, `:801-804`, `:842-900` | Wallet addresses are raw Ed25519 public keys, while native transactions are signed by independently generated ML-DSA keys. Verification accepts the public key embedded in the signature but never derives or binds that key to `sender`. An attacker can sign with its own PQ key, set `sender` to a victim address, and satisfy the UTXO ownership check. | CONFIRMED, launch blocker |
| PQC-002 | SEV-1 | Validator authorization/finality | `Protocol_Core/qanto-node/src/qantodag.rs:445-462`, `:567-595`, `:728-750`, `:3010-3012`, `:3489-3509` | `block.validator` is signed as text but is not cryptographically mapped to the embedded PQ key. Any PQ key can sign a block labeled with a staked validator name; finality later credits stake using that string. | CONFIRMED, launch blocker |
| PQC-003 | INFO | Default ML-DSA-65 integration | `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:9-18`, `:138-143`, `:533-541`, `:589-614`, `:1006-1055`; `Protocol_Core/qanto-node/src/post_quantum_crypto.rs:34-75` | The default path uses `dilithium-rs` ML-DSA-65 key generation, signing, and verification, rather than a local accept-all stub. Three focused round-trip/forgery-rejection tests pass. | CONFIRMED integration; not an interoperability or side-channel certification |
| PQC-004 | SEV-2 | Public PQ stub API | `Protocol_Core/qanto-node/src/pq_crypto.rs:15-38`; `Protocol_Core/qanto-node/src/lib.rs:45` | The baseline API returned `Ok(true)` for every message, key, and signature. Concurrent remediation now returns an explicit error and adds a fail-closed regression test. Lexical call search found no production caller. | FIXED_IN_WORKTREE, containment verified by source |
| PQC-005 | SEV-2 | Hybrid/legacy PQ APIs | `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:801-878`; `Protocol_Core/qanto-node/src/post_quantum_crypto.rs:53-55` | With the default feature set, `PostQuantumEngine::generate_keypair` returns two empty vectors and verification always rejects. The comment claiming the ML-KEM production path is handled by `QantoKeyPair` is inaccurate: `QantoKeyPair` contains signature keys, not a KEM state machine. | CONFIRMED |
| PQC-006 | SEV-2 | Legacy PQ feature | `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:562-567`; `Protocol_Core/qanto-zk-sdk/Cargo.toml:16-18` | The qanto-core legacy feature still attempts to parse a 4,032-byte secret key as a 1,952-byte public key and fails targeted tests. Concurrent remediation removed the ZK SDK's undeclared legacy import and converted its compatibility feature to an empty no-op; the SDK all-feature matrix now passes. | PARTIALLY_FIXED, qanto-core failure remains MEASURED |
| PQC-007 | SEV-2 | ML-KEM/Kyber P2P handshake | `Protocol_Core/qanto-core/src/qanto_p2p.rs:12`, `:246-259`, `:482-550` | ML-KEM-768 keys are generated and advertised, but the decapsulation key is immediately discarded, the peer encapsulation key is never used, no shared secret is derived, and no transport encryption is installed. The handshake "signature" is an unkeyed hash of public data. | CONFIRMED; advertised PQ key exchange is not integrated |
| CRY-001 | SEV-2 | P-256 compatibility wrapper | `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:620-717`; `Protocol_Core/qanto-node/src/hame.rs:29-31`, `:216-267` | The type named P-256 is not ECDSA. Verification accepts `s = qanto_hash(r \|\| public_key \|\| qanto_hash(message))`; anyone knowing the public key can choose `r` and calculate an accepted signature without the private key. HAME uses this wrapper. | CONFIRMED; SEV-1 if HAME authorizes value or consensus state |
| CRY-002 | SEV-1 | Canonical hash | `Protocol_Core/myblockchain/src/qanhash32x.rs:67-97`; `Protocol_Core/myblockchain/src/lib.rs:41-45`, `:104-106`; `Protocol_Core/qanto-node/src/transaction.rs:300-303`; `Protocol_Core/qanto-node/src/qantodag.rs:710-718` | `qanhash32x` zero-pads the final partial word but encodes neither message length nor a sponge padding delimiter. Therefore `qanhash32x(b"a") == qanhash32x(b"a\0")`. `qanto_hash` delegates directly to this construction and is used for transaction IDs, Merkle nodes, PoW finalization, and other commitments. | DERIVED from exact state equivalence, launch blocker |
| CRY-003 | SEV-3 | Constant-time/key hygiene | `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:154-203`, `:644-660`; `Protocol_Core/qanto-node/src/transaction.rs:515-517` | The only explicit constant-time comparison is inside the forgeable pseudo-P256 verifier. Native public-key and address comparisons use normal equality, but those values are public and no local secret-dependent comparison was identified. PQ private keys are cloneable and serializable, increasing the number of secret copies despite zeroization on drop. | No confirmed local timing leak; dependency behavior needs external review |
| POW-001 | SEV-1 | CPU/OpenCL/CUDA/Metal parity | `Protocol_Core/myblockchain/src/qanhash.rs:1396-1427`; `Protocol_Core/myblockchain/src/kernel.cl:27-34`, `:50-123`; `Protocol_Core/myblockchain/src/qanhash.cu:80-162`; `Protocol_Core/myblockchain/src/qanhash.metal:80-160` | The backends implement different algorithms. CPU performs 32 multiply/add/rotate/XOR rounds followed by `qanto_hash`; OpenCL uses a different vector rotate/XOR mixer and treats raw mix bytes as the hash; CUDA/Metal use 64 DAG-XOR rounds with repeated Keccak and final Keccak. | CONFIRMED, consensus-breaking if heterogeneous results are accepted |
| POW-002 | SEV-1 | CUDA/Metal parity | `Protocol_Core/myblockchain/src/qanhash.cu:9-18`; `Protocol_Core/myblockchain/src/qanhash.metal:9-18` | CUDA round constant 10 is `0x000000008000009`, while Metal uses `0x0000000080008009`; even the otherwise similar CUDA and Metal kernels diverge. | CONFIRMED |
| POW-003 | SEV-2 | CPU portability | `Protocol_Core/myblockchain/src/qanhash.rs:1396-1403`, `:1414-1420`, `:1884-1888` | The baseline CPU path cast alignment-1 byte arrays to `&[u64;16]` and read host-native words. Concurrent remediation replaced both casts with explicit `u64::from_le_bytes` decoding through `decode_dag_words`. | FIXED_IN_WORKTREE, source verified |
| POW-004 | SEV-2 | GPU integration | `Protocol_Core/myblockchain/src/cuda_gpu.rs:5-15`, `:43-56`; `Protocol_Core/myblockchain/src/metal_gpu.rs:45-53`; `Protocol_Core/myblockchain/src/qanhash.rs:242-345`, `:2140-2165`; `Protocol_Core/myblockchain/Cargo.toml:103-119` | CUDA does not compile, root GPU wrappers hard-code Q-DAG epoch zero, and the unified miner resolves to nested Metal/CUDA modules that are always unavailable before falling back to OpenCL. | CONFIRMED, MEASURED |
| ZK-001 | SEV-2 | Bridge ZK SDK | `Protocol_Core/qanto-zk-sdk/src/lib.rs:146-214`, `:216-250`, `:314-334`, `:373-394` | Proof construction remains a deterministic family of Keccak hashes rather than a proving system, and the claim schema still omits ZK proof/public inputs. Concurrent remediation changed `verify_bridge_transaction` to return an error unconditionally, so synthetic bundle generation fails closed. | CONTAINED_IN_WORKTREE; real bridge verification remains unimplemented |
| ZK-002 | SEV-2 | Live bridge mint validation | `Protocol_Core/qanto-relayer/src/main.rs:397-421`; `Protocol_Core/qanto-node/src/qantodag.rs:89-91`, `:2938-2953`, `:4002-4014`, `:4179-4224` | The underlying claim handler still contains only non-empty proof checks followed by relayer quorum and minting, but concurrent remediation sets `BRIDGE_PROOF_VERIFIER_AVAILABLE = false`, rejects bridge-claim transactions during block validation, and returns before the handler can mint. | CONTAINED_IN_WORKTREE; bridge unavailable pending real verifier |
| ZK-003 | SEV-2 | Internal Groth16 system | `Protocol_Core/qanto-node/src/zkp.rs:21-28`, `:927-995`, `:1895-1948` | Internal verification does call Arkworks Groth16/BLS12-381. However, each process generates random circuit-specific keys at runtime and stores them only in memory, so no canonical cross-node CRS/VK exists. | CONFIRMED; SEV-1 if proofs authorize consensus/value |
| ZK-004 | SEV-2 | ZK circuit soundness | `Protocol_Core/qanto-node/src/zkp.rs:174-326`, `:430-473`, `:592-613`, `:711-802`, `:835-900` | Several circuits emit no constraints; membership always passes; the balance "commitment" is a tautology with no public commitment; age, voter eligibility, whitelist membership, and authority-root binding are missing or simulated. A valid Groth16 proof therefore proves a weak or empty statement. | CONFIRMED; SEV-1 wherever relied upon for authorization |
| ZK-005 | SEV-2 | Feature-gated verification | `Protocol_Core/qanto-node/src/interoperability.rs:1007-1044`; `Protocol_Core/qanto-node/src/zkp_stub.rs:29-57`; `Protocol_Core/qanto-node/src/lib.rs:113-121` | Without `real-zk-verifier`, interoperability ZK verification fails closed. With the feature enabled, it only checks length and hash length and indexes `proof.data[..8]` before its minimum-length check. The `zkp_stub` verifier also fails closed, which is correct. | CONFIRMED; enabled pseudo-verifier is unsafe |
| ZK-006 | SEV-2 | No-ZK/all-feature build matrix | `Protocol_Core/qanto-node/src/decentralization.rs:1112-1120`; `Protocol_Core/qanto-node/src/mock_traits.rs:239-259`; `Protocol_Core/qanto-zk-sdk/Cargo.toml:16-18` | `qanto --no-default-features` fails with 44 errors, including cfg-dependent API shape and an ungated impl of a gated type. Concurrent remediation fixed the ZK SDK all-feature failure; `cargo test -p qanto-zk-sdk --all-features --locked` now passes five tests. | PARTIALLY_FIXED, node no-ZK build remains broken |

## 3.B What is genuinely implemented

- Default ML-DSA-65 key generation, signing, and verification are real library calls:
  - `QantoPQPrivateKey::generate`: `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:608-614`.
  - `QantoPQPrivateKey::sign`: `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:589-598`.
  - `QantoPQPublicKey::verify`: `Protocol_Core/qanto-core/src/qanto_native_crypto.rs:533-541`.
- `PQSuite` delegates to those native primitives at `Protocol_Core/qanto-node/src/pq_suite.rs:45-64`.
- The internal ZK subsystem deserializes and verifies an Arkworks Groth16 proof at `Protocol_Core/qanto-node/src/zkp.rs:1915-1947`.
- The no-ZK stub rejects proofs deterministically at `Protocol_Core/qanto-node/src/zkp_stub.rs:39-41`.
- The bridge SDK now rejects every proof until a real verifier exists at `Protocol_Core/qanto-zk-sdk/src/lib.rs:240-250`, and block validation rejects `BridgeClaim` transactions while `BRIDGE_PROOF_VERIFIER_AVAILABLE` is false at `Protocol_Core/qanto-node/src/qantodag.rs:2938-2953`.
- The bridge relayer quorum signatures are real recoverable secp256k1 operations at `Protocol_Core/qanto-node/src/qantodag.rs:4092-4134`; this does not make the claimed source event valid.

These positive facts do not neutralize the missing identity binding, weak hash, backend divergence, or absence of a real bridge proof verifier.

## 3.C Known-answer and constant-time caveats

### Known-answer tests

The existing focused ML-DSA tests are self-round trips. They do not provide:

- NIST ML-DSA-65 ACVP/KAT vectors;
- malformed public-key/signature corpus coverage;
- cross-version or cross-language interoperability;
- deterministic seed-to-key/vector stability guarantees;
- CPU/CUDA/Metal/OpenCL QanHash equivalence vectors;
- big-endian or unaligned-memory portability vectors.

`cargo test -p my-blockchain --lib --locked` reports zero tests, so there is no executable consensus-backend equivalence contract.

### Constant-time assessment

No confirmed local secret-dependent equality branch was found. Ordinary equality at `transaction.rs:515` compares public keys, and UTXO/address comparisons also operate on public data. That does not establish constant-time ML-DSA signing or verification: those operations are delegated to `dilithium-rs = 0.2.0`, whose implementation and compiler output require an upstream/manual side-channel review. The local audit therefore assigns `NEEDS_EXTERNAL_REVIEW`, not a clean constant-time certification.

Private-key wrappers derive `Clone` and `Serialize` at `qanto_native_crypto.rs:158-160` and `:184-186`. Zeroization on drop protects the wrapper's final allocation, but cloned/serialized buffers can persist separately. Production key management should minimize cloning, prohibit general serialization of secret-key types, and use locked/protected memory where practical.

## 3.D Backend equivalence proof

The required parity proposition is:

```text
For every header H, nonce N, and epoch E:
CPU(H,N,E) == OpenCL(H,N,E) == CUDA(H,N,E) == Metal(H,N,E)
```

The proposition is false by source inspection:

| Backend | Rounds/mixer | Finalization | Epoch handling |
|---|---|---|---|
| CPU | 32 rounds of multiply-by-31, add DAG1, rotate by lane, XOR DAG2 | `qanto_hash(128-byte mix)` | `get_qdag(block_index)` |
| OpenCL | 32 rounds of vector rotations/XOR against separate vector DAG entries | First 32 raw private mix bytes | Q-DAG supplied by host |
| CUDA | 64 rounds of bytewise DAG XOR; Keccak every 8 rounds | Final Keccak, first 32 state bytes | Wrapper hard-codes epoch 0 |
| Metal | Same broad design as CUDA but with a different CUDA round constant | Final Keccak, first 32 state bytes | Wrapper hard-codes epoch 0 |

This is not a performance variance. It is a consensus-output variance.

## 3.E Targeted check results

| Command | Result | Interpretation |
|---|---|---|
| `cargo test -p qanto-core qanto_native_crypto::tests --locked` | PASS: 3 passed | Default Ed25519/ML-DSA local round trips work. |
| Same test with `--features pqcrypto-legacy` | FAIL: 1 passed, 2 failed | Legacy PQ public-key derivation panics at `qanto_native_crypto.rs:566`. |
| `cargo test -p qanto-zk-sdk --all-features --locked` | PASS: 5 passed | Concurrent fail-closed tests pass and the former undeclared legacy import is gone. This still does not provide a bridge verifier. |
| `cargo test -p my-blockchain --lib --locked` | PASS: 0 tests | No QanHash parity or KAT coverage exists. |
| `cargo check -p my-blockchain --features gpu --locked` | PASS with warning | OpenCL host code compiles; no GPU kernel was executed. |
| `cargo check -p my-blockchain --features cuda --locked` | FAIL: 5 errors | Missing/unresolved `cudarc`, `Arc`, and `get_qdag`. |
| `cargo check -p qanto --no-default-features --locked` | FAIL: 44 errors | Fail-closed no-ZK deployment is not buildable. |
| `cargo nextest run --workspace --locked --no-fail-fast --retries 0` | PASS: 207/207 passed; 1 skipped | Final full-workspace validation covers the current qanto-node suite and containment regressions. It does not close the failing feature/build matrices above. |

## 3.F Implemented fixes in the effective audit worktree

This document's author did not edit protocol source. Concurrent remediation in the shared audit worktree implemented the following containment fixes after the initial findings were reproduced:

| Fix | Evidence | Verification/state |
|---|---|---|
| Dormant `pq_crypto` verifier changed from accept-all to explicit error; regression test added. | `Protocol_Core/qanto-node/src/pq_crypto.rs:15-38` | Source verified; final workspace Nextest passed 207/207 tests with 1 skipped. |
| ZK SDK verifier changed from nonzero-shape acceptance to unconditional fail-closed; synthetic proof pipeline test now expects rejection. | `Protocol_Core/qanto-zk-sdk/src/lib.rs:240-250`, `:319-334`, `:373-394` | `cargo test -p qanto-zk-sdk --all-features --locked`: 5 passed. |
| Undeclared ZK SDK legacy PQ import removed; compatibility feature made an empty no-op. | `Protocol_Core/qanto-zk-sdk/Cargo.toml:16-18` | Same all-feature test passes. |
| Bridge claims disabled both at block-validation entry and inside the claim handler. | `Protocol_Core/qanto-node/src/qantodag.rs:89-91`, `:2938-2953`, `:4002-4014` | Source verified; final workspace Nextest passed 207/207 tests with 1 skipped. This containment keeps claims unauthorized; bridge functionality is unavailable. |
| Unsafe/native-endian CPU Q-DAG word casts replaced with explicit little-endian decoding. | `Protocol_Core/myblockchain/src/qanhash.rs:1396-1403`, `:1414-1420`, `:1884-1888` | `cargo test -p my-blockchain --lib --locked` compiles and passes, but still executes zero tests. Backend KAT/parity proof remains absent. |

Existing fail-closed behavior in `zkp_stub.rs:39-41` and default interoperability rejection at `interoperability.rs:1007-1014` pre-dated these fixes.

## 3.G Deferred remediation

### Safe containment that does not change consensus history

| Priority | Action | Estimated effort | Reason/status |
|---|---|---:|---|
| Immediate | Keep bridge claims disabled and additionally prevent relayer transmission/startup from advertising a usable bridge. | 0.5-1 day | Node/SDK containment is implemented; operator/RPC surface verification is still required. |
| Immediate | Disable OpenCL/CUDA/Metal mining and CPU-reverify every externally supplied GPU result. | 1-2 days | Prevents heterogeneous kernel output from entering blocks. |
| Completed containment | Keep `pq_crypto::verify_pq_signature` fail closed or remove the exported module. | Implemented | Regression test exists; final workspace Nextest passed 207/207 tests with 1 skipped. |
| Immediate | Make unsupported hybrid key generation return `Err`, never empty keys. | <0.5 day | Prevents ambiguous empty-key success. |
| P0 | Repair default/no-default/all-feature compilation. | 2-4 days | Makes fail-closed build modes enforceable in CI. |
| P0 | Add ML-DSA KATs, malformed-input tests, and backend parity fixtures. | 3-7 days | Establishes a reproducible cryptographic and consensus contract. |

### Protocol migrations requiring explicit authorization

| Priority | Change | Estimated effort | Why authorization/migration is required |
|---|---|---:|---|
| P0 | Version native addresses and validator identities as a domain-separated commitment to the PQ key, or require a bound Ed25519+ML-DSA hybrid authorization. | 2-6 weeks plus migration | Changes who is authorized to spend existing UTXOs and produce/finalize blocks. Existing addresses, validator records, wallets, genesis, RPCs, and fixtures must migrate together. |
| P0 | Replace/version `qanto_hash` with a standardized collision-resistant hash or a correctly padded, domain-separated construction. | 3-8 weeks | Changes transaction IDs, block IDs, Merkle roots, PoW, commitments, and persisted indexes. A silent in-place swap would fork the chain. |
| P0 | Define one byte-exact QanHash specification and derive all CPU/GPU backends from it. | 3-6 weeks plus hardware validation | Mining output is consensus data. Kernel replacement needs an activation height/version and golden vectors across epochs/endian cases. |
| P0 | Replace pseudo-P256 with standards-compliant P-256 ECDSA. | 1-3 weeks plus identity migration | Existing HAME public keys/signatures use a nonstandard representation and cannot be treated as real P-256 material. |
| P0 | Introduce an audited bridge proof format, canonical VK/CRS, verified source headers/finality, receipt inclusion, log-domain binding, and nullifier semantics. | 6-12+ weeks plus independent audit | Alters bridge authorization and requires coordinated relayer/node deployment and adversarial review. |

---

# Workstream 4 — Tokenomics and Emission Mathematical Proof

## 4.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| ECO-001 | INFO | Sealed constants | `Protocol_Core/qanto-node/src/lib.rs:164-187` | `Q_SCALE = 1e9`, max supply is 21B QNTO, allocations are 16.8B/3.15B/1.05B, and a compile-time assertion proves their sum equals max supply. | CONFIRMED, correct; unchanged |
| ECO-002 | INFO | Intended recurrence | `Protocol_Core/qanto-node/src/emission.rs:6-10`, `:71-109` | The recurrence is integer-only: each period uses checked `u128` multiplication followed by integer division. Given identical inputs and termination, it is platform deterministic. | CONFIRMED |
| ECO-003 | SEV-1 | Live consensus reward path | `Protocol_Core/qanto-node/src/qantodag.rs:2778-2788`, `:3122-3148`, `:3184-3192`; `Protocol_Core/qanto-node/src/saga.rs:3754-3855` | Candidate construction and validation call SAGA, not `Emission::calculate_reward`. SAGA depends on mutable miner credit, global threat state, environmental metrics, predictive market premium, active edict, and `f64` multiplication/casts. Different nodes can calculate different expected rewards. | CONFIRMED, launch blocker |
| ECO-004 | SEV-1 | Supply-cap enforcement | `Protocol_Core/qanto-node/src/qantodag.rs:2233-2242`; `Protocol_Core/qanto-node/src/emission.rs:96-129` | `update_supply` silently clamps an internal counter, but does not reject/reduce an already accepted ledger reward. Thus it does not enforce the cap on state transition. | CONFIRMED, launch blocker |
| ECO-005 | SEV-1 | Fees counted as issuance | `Protocol_Core/qanto-node/src/qantodag.rs:2782-2788`, `:3126-3143`, `:2233-2242` | `block.reward` equals SAGA subsidy plus fees, and the entire amount is passed to `update_supply`. Recycled transaction fees are therefore counted as newly emitted supply. | CONFIRMED |
| ECO-006 | SEV-1 | Supply persistence/genesis | `Protocol_Core/qanto-node/src/emission.rs:44-57`; `Protocol_Core/qanto-node/src/qantodag.rs:1001-1013`; `config/genesis.json:7-20`; `Protocol_Core/qanto-node/src/node.rs:394-458` | Every DAG starts its emission counter at zero. The shipped testnet genesis allocates the complete 21B supply to one address and loads it into balances/UTXOs, but that allocation is not reflected in the emission counter. Restart also reconstructs a zero counter. | CONFIRMED for the shipped testnet configuration |
| ECO-007 | SEV-2 | Community mining budget | `Protocol_Core/qanto-node/src/lib.rs:173-180`; `Protocol_Core/qanto-node/src/emission.rs:6-7`, `:60-68` | The intended emission engine is capped against 21B, not the sealed 16.8B community allocation. It can consume supply reserved for ecosystem/development and liquidity. | CONFIRMED |
| ECO-008 | SEV-3 | Reward floor/complexity/metrics | `Protocol_Core/qanto-node/src/emission.rs:76-109`, `:140-152` | Reward computation is `O(elapsed_periods)`, forces a minimum one atomic unit until the cap, and increments the halving metric repeatedly because `update_last_halving_period` has no production caller. | CONFIRMED |
| DOC-001 | INFO | Decimal documentation | `README.md:39-49`, `:55-65`; `SmartContracts/contracts/WQNTO.sol:11-15`; `Docs/bridge-wqnto.md:9-17`; `Docs/guide/airdrop_guide.md:7-18`; `OVERVIEW.md:309-310` | Canonical native and WQNTO decimals are 9 across the maintained README, contract, bridge guide, and airdrop guide. The only non-audit QNTO/WQNTO references to six decimals are explicit warnings not to use historical six-decimal examples; no non-audit document currently specifies six as the live precision. | FIXED_IN_WORKTREE |
| DOC-002 | SEV-2 | Reward documentation and generated UI examples | `README.md:39-53`; `SmartContracts/token-info.json:9-17`; `Docs/node-operator-guide.md:1-7`; `Docs/rpc-api.md:1-8`, `:74-82`, `:161-169`; `Frontend/website/assets/json/explorer-mock.json:1-15`, `:16-59`; `Frontend/website/assets/js/faucet.js:1-18`; `Frontend/website/explorer.html:641-642`; `Protocol_Core/qanto-node/src/emission.rs:6`; `Protocol_Core/qanto-node/src/saga.rs:3426-3434` | README now correctly marks the reward unresolved, and the maintained bridge/airdrop guides publish no reward. Residual token metadata, operator/RPC documentation, explorer/faucet mocks, and explorer UI still publish 2.5 QNTO, while both code bases specify 50 QNTO and the live SAGA path remains consensus-unsafe. | PARTIALLY_FIXED; confirmed specification delta |
| DOC-003 | SEV-2 | Security documentation | `Docs/external_review_package.md:50-54`; `Docs/threat_model.md:93-98`; `Protocol_Core/qanto-node/src/qantodag.rs:3138-3143` | Security documents claim forged rewards are checked against `emission.rs`; current consensus checks against SAGA instead. | CONFIRMED specification delta |

## 4.B Sealed constants verification

Current kernel constants at `Protocol_Core/qanto-node/src/lib.rs:164-180` are:

```text
Q_SCALE          = 1,000,000,000
MAX_TOTAL_SUPPLY = 21,000,000,000 * Q_SCALE
COMMUNITY_ALLOC  = 16,800,000,000 * Q_SCALE
ECO_DEV_ALLOC    =  3,150,000,000 * Q_SCALE
LIQUIDITY_ALLOC  =  1,050,000,000 * Q_SCALE
```

Arithmetic proof:

```text
16,800,000,000 + 3,150,000,000 + 1,050,000,000
= 21,000,000,000 QNTO

80% + 15% + 5% = 100%
```

The compile-time assertion at `lib.rs:182-187` enforces the equality. **No sealed constant was modified or proposed for modification.**

## 4.C Deterministic recurrence proof

For timestamp `t >= genesis_timestamp`, the intended engine computes:

```text
n = floor((t - genesis_timestamp) / 8,400,000)
r[0] = 50 * 1,000,000,000 atomic units
r[k+1] = floor(r[k] * 999,000,000 / 1,000,000,000)
per_chain = max(1, floor(r[n] / num_chains))
return min(per_chain, total_supply - current_supply), or 0 when no supply remains
```

All consensus-value operations in this function use fixed-width integers, checked multiplication, and integer division. There is no floating point. The maximum first-step product is `50,000,000,000 * 999,000,000`, far below `u128::MAX`, and later products decrease. The result is therefore deterministic across Rust targets if the loop completes.

This proof applies only to `Emission::calculate_reward`. It does **not** prove the current live reward path, because source search finds no production call to `calculate_reward`; live consensus uses `PalletSaga::calculate_dynamic_reward`.

## 4.D Model assumptions

The model below is a deterministic reproduction of the integer recurrence, not a forecast of the current SAGA implementation.

- Genesis timestamp: `1717250400` = `2024-06-01T14:00:00Z` from `config/genesis.json:4`.
- Zero transaction fees.
- Full uptime and regular production.
- One aggregate subsidy stream.
- Default scenario: one chain, one block per second, matching `Protocol_Core/qanto-node/src/config.rs:228-278`.
- High-performance scenario: four chains, one block per chain every 31ms, with the intended engine dividing reward by four, matching `config.rs:564-620`. Aggregate issuance is one full reward per 31ms slot. If only one chain produces in a slot, divide the 31ms gross column by four.
- One year = 365.25 days.
- `floor` is applied after every fixed-point multiplication exactly as in Rust.
- "Gross" ignores the cap so the shape can be assessed. "Cap-limited" applies the intended 21B clamp and assumes the counter begins at zero.

## 4.E Emission totals

| Horizon | Gross issuance at 1s | Gross aggregate issuance at 31ms | Intended cap-limited result at 1s | Intended cap-limited result at 31ms |
|---:|---:|---:|---:|---:|
| 10 years | 15,493,637,441.882064 QNTO | 499,794,756,153.029900056 QNTO | 15,493,637,441.882064 | 21,000,000,000 |
| 50 years | 71,959,818,718.51968 | 2,321,284,474,757.370358385 | 21,000,000,000 | 21,000,000,000 |
| 100 years | 131,590,559,869.97328 | 4,244,856,769,977.101128509 | 21,000,000,000 | 21,000,000,000 |
| 500 years | 355,871,653,541.5092 | 11,479,730,759,382.221272067 | 21,000,000,000 | 21,000,000,000 |

Uncapped integer-recurrence lifetime issuance is approximately:

- `419,999,923,688.7216 QNTO` at one block per second;
- `13,548,384,635,095.793548646 QNTO` at one aggregate reward per 31ms.

Thus the asymptotic schedule exceeds `COMMUNITY_ALLOC` by a very large margin unless a separate, correctly initialized community-budget cap is enforced.

## 4.F Reward thresholds

### Raw recurrence, ignoring supply exhaustion

| Threshold | Period | Reward | Offset from genesis | UTC timestamp | Height at 1s | Height at 31ms |
|---|---:|---:|---:|---|---:|---:|
| Aggregate/single-chain reward first below 1 QNTO | 3,911 | 0.999066156 QNTO | 32,852,400,000s | 3065-06-20T16:40:00Z | 32,852,400,000 | 1,059,754,838,709 |
| Four-chain per-chain reward first below 1 QNTO | 2,525 | 0.999465330 QNTO | 21,210,000,000s | 2696-07-14T16:40:00Z | 21,210,000,000 | 684,193,548,387 |
| Raw recurrence first becomes 0 atomic units | 18,296 | 0 | 153,686,400,000s | 6894-07-18T08:40:00Z | 153,686,400,000 | 4,957,625,806,451 |

Period 18,295 pays exactly one atomic unit. Because `calculate_reward` applies `.max(1)` at `emission.rs:91-94`, its returned reward never falls below one atomic unit while supply remains; it jumps to zero only when `max_remaining == 0`.

### Cap and allocation crossings

Under the zero-initial-supply assumption:

| Boundary | Production model | Approximate height | Time from genesis | UTC timestamp |
|---|---|---:|---:|---|
| 16.8B community allocation | 1s | 342,733,942 | 10.860583 years | 2035-04-12T09:52:22Z |
| 21B total supply | 1s | 430,649,029 | 13.646444 years | 2038-01-23T22:43:49Z |
| 16.8B community allocation | 31ms aggregate | 336,065,098 | 120.578912 days | 2024-09-30T03:53:38.038Z |
| 21B total supply | 31ms aggregate | 420,149,182 | 150.747970 days | 2024-10-30T07:57:04.642Z |

For the shipped genesis, however, all 21B are already allocated. A correctly initialized cap-aware engine would return zero subsidy from genesis for that configuration. The current engine starts its counter at zero, and live consensus bypasses it, so neither modeled clamp is actually enforced.

## 4.G Live economics divergence

The runtime sequence is:

```text
SAGA local state and f64 multipliers
          |
          v
calculate_dynamic_reward() ---- transaction fees
          |                         |
          +-----------+-------------+
                      v
              block.reward
                      |
          +-----------+-----------+
          |                       |
          v                       v
   coinbase/state mint     emission.update_supply()
                          silently clamps counter
```

Key consequences:

1. **Consensus nondeterminism:** `saga.rs:3760-3805` reads mutable local maps/services and multiplies `f64`s. Two honest nodes need not share those values.
2. **Wrong schedule:** elapsed time, `HALVING_PERIOD`, and `HALVING_FACTOR_FIXED` do not influence the live subsidy.
3. **Fee mis-accounting:** fees are transfers from users, not new issuance, but are included in the emission counter.
4. **Non-enforcing cap:** counter clamping does not undo or reject an accepted coinbase/state change.
5. **Restart reset:** reconstructed DAGs begin at zero issuance regardless of persisted supply.
6. **Allocation collision:** the engine's 21B limit does not reserve the sealed 15% ecosystem/development and 5% liquidity allocations.

The debug/test-only global supply assertion at `qantodag.rs:2307-2342` cannot protect release builds.

## 4.H Implemented fixes in this pass

**No monetary consensus implementation was changed by this track.** It was a read-only protocol/economics audit: the sealed constants, live reward path, genesis allocation, and state transition remain unchanged. Documentation containment and integer-emission regression tests were added elsewhere in the shared audit pass and are credited in the master/testing reports.

## 4.I Deferred remediation

### Safe containment and documentation corrections

| Priority | Action | Estimated effort | Notes |
|---|---|---:|---|
| Immediate | Treat the current build as economically non-authoritative; do not launch a value-bearing network. | Immediate | Required until consensus subsidy and supply state are repaired. |
| Completed documentation containment | Keep all maintained QNTO/WQNTO integration surfaces on nine decimals and reject historical six-decimal examples. | Implemented | No erroneous six-decimal QNTO/WQNTO value remains in non-audit documentation; the remaining mentions are explicit warnings. The unrelated USDT test helper intentionally uses six decimals. |
| P0 | Replace residual 2.5-QNTO metadata and UI examples with `UNRESOLVED_DO_NOT_INTEGRATE` until an authorized deterministic reward rule exists. | 0.5-1 day | Applies to `token-info.json`, operator/RPC docs, and website explorer/faucet mocks. Do not silently replace 2.5 with 50: the live reward path is not consensus-safe. |
| P0 | Add boundary/property tests for integer recurrence, multi-chain division, cap remainder, pre-genesis timestamp, extreme timestamp, and restart persistence. | 2-4 days | Tests should use exact atomic-unit vectors. |
| P1 | Change repeated period iteration to deterministic exponentiation-by-squaring or a bounded precomputed schedule after equivalence proof. | 2-5 days | Performance-only until activated; exact floor semantics must be preserved. |

### Consensus/economic changes requiring explicit authorization

| Priority | Change | Estimated effort | Why authorization is required |
|---|---|---:|---|
| P0 | Make `Emission::calculate_reward` or an equivalently specified fixed-point function the sole consensus subsidy source. Remove local SAGA telemetry and `f64` from block validation. | 2-4 weeks | Changes every future coinbase amount and therefore consensus. SAGA may recommend governance changes, but activated parameters must be finalized/versioned consensus data. |
| P0 | Persist/reconstruct issued supply from canonical state before accepting blocks, and reject a block whose subsidy exceeds the exact remaining budget. | 2-4 weeks | Alters validation and restart/replay behavior; requires migration tests and state-versioning. |
| P0 | Separate subsidy from transaction fees in issuance accounting. | 1-2 weeks | Changes supply metrics and potentially historical reconciliation. |
| P0 | Enforce mining rewards against the existing sealed `COMMUNITY_ALLOC` constant while separately accounting for `ECO_DEV_ALLOC` and `LIQUIDITY_ALLOC`. | 2-4 weeks | Does not modify sealed constants, but changes budget semantics and must be approved as a protocol rule. |
| P0 | Replace the shipped full-supply testnet prefund with an allocation/genesis policy consistent with the approved public-testnet economics, or explicitly disable mining for that genesis. | 1-2 weeks plus reset/migration plan | Existing testnet state cannot simultaneously represent a full 21B allocation and a fresh emission schedule. |

## 4.J Final workstream verdict

- Sealed constants: **correct and internally consistent**.
- Standalone integer emission function: **deterministic, but operationally unused and imperfectly bounded**.
- Live reward consensus: **not deterministic from canonical block state**.
- Supply cap: **not enforced on ledger state**.
- Public-testnet genesis/economics: **internally contradictory**.
- Decimal standard: **9 is canonical; erroneous six-decimal QNTO/WQNTO documentation is fixed, with historical values retained only as warnings**.

**Workstream 4 verdict: NOT READY.**

---

# Change and provenance statement

This document is the only file authored by this audit track. The author did not edit Rust, CUDA, Metal, OpenCL, Solidity, JSON genesis, Cargo manifests, or user-facing product documentation. During drafting, concurrent agents changed `pq_crypto.rs`, `qanto-zk-sdk`, `qantodag.rs`, and `qanhash.rs`; the effective-state containment fixes are explicitly recorded in section 3.F. Other concurrent changes in `transaction.rs`, `interoperability.rs`, `README.md`, the maintained bridge/airdrop guides, Hardhat/deployment metadata, and CEX metadata are outside this track. Findings marked unresolved were rechecked against the effective worktree after those concurrent edits.
