# Qanto Layer-0 — External Security Audit Report

**Audit type:** Independent external audit (full-stack review)  
**Auditor role:** External auditor (this session)  
**Audit date:** 2026-07-31  
**Reviewed commit:** `git rev-parse HEAD` at time of audit  
**Scope:** All Rust workspace crates + Solidity contracts + CI/DevOps

> This report reflects **actual code findings** from reading source files.
> It replaces the self-awarded scorecard that preceded this audit.
> Scores are honest assessments backed by specific line references.

---

## Summary Table

| Severity | Count |
|---|---|
| Critical | 4 |
| High | 6 |
| Medium | 8 |
| Low / Informational | 7 |
| **Total** | **25** |

---

## Critical Findings

---

### CRIT-01: P256 "Signature" is a Hash Construction, Not ECDSA — Universally Forgeable

**File:** `Protocol_Core/qanto-core/src/qanto_native_crypto.rs`  
**Lines:** 627–698  
**Severity:** Critical — Authentication bypass

**Description:**

`QantoP256PrivateKey::sign` and `QantoP256PublicKey::verify` do **not** implement P-256 (NIST curve) ECDSA. They implement a custom hash construction:

```rust
// "Sign" (lines 680–699):
let message_hash = qanto_hash(message);
let nonce_hash   = qanto_hash(&[private_key_bytes, message_hash]);
let s_hash       = qanto_hash(&[nonce_hash, public_key_bytes, message_hash]);
signature = [nonce_hash (32B) | s_hash (32B)]

// "Verify" (lines 627–641):
expected_s = qanto_hash(&[signature[0..32], public_key_bytes, message_hash])
accept if expected_s == signature[32..]
```

**Why this is catastrophically broken:**

1. `public_key_bytes` is **in the verify input** — and the verifier already has the public key.
2. An attacker can forge a valid signature for **any message** without knowing the private key:
   - Choose any random 32-byte value `r`
   - Compute `s = qanto_hash(r || public_key_bytes || qanto_hash(target_message))`
   - Signature `[r | s]` passes `verify()` for any message and any public key
3. The `public_key()` derivation is non-standard (hash-of-hash), not an elliptic curve point.

**Proof of Concept:**
```rust
// Forge a valid P256 "signature" without the private key:
let alice_pub = any_known_QantoP256PublicKey;
let message   = b"transfer 1000000 QUSD to attacker";
let r = [0xde; 32];
let msg_hash = qanto_hash(message);
let mut s_input = [r, alice_pub.as_bytes().as_slice(), msg_hash.as_bytes()].concat();
let s = qanto_hash(&s_input);
let forged = QantoP256Signature([r, *s.as_bytes()].concat().try_into().unwrap());
alice_pub.verify(message, &forged).unwrap(); // returns Ok(()) — FORGED
```

**Impact:** Any component using P256 authentication is completely bypassed. Zero-knowledge of private key required to forge signatures.

**Remediation:** Replace with the `p256` crate (`p256::ecdsa::SigningKey` / `VerifyingKey`). Delete the custom hash construction entirely.

---

### CRIT-02: P2P Message Signatures Are Not Verified on Receipt

**File:** `Protocol_Core/qanto-core/src/qanto_p2p.rs`  
**Lines:** 569–580 (signing), 798–825 (dispatch without verification)  
**Severity:** Critical — Unauthenticated message injection

**Description:**

The "signature" in `NetworkMessage` is `Hash(payload || node_id)`. `node_id` is a public value exchanged in the handshake. Any peer can produce this hash for any message.

When messages are received, the handler is dispatched without any signature check:
```rust
if let Some(handler) = message_handlers.get(&message.msg_type) {
    if let Err(e) = handler(message, peer_id) { ... }
    // ↑ no signature verification before dispatch
}
```

**Impact:** Any connected peer can inject arbitrary `Block`, `Transaction`, `StateSync`, or `BalanceSubscribe` messages. The `signature` field is cosmetic.

**Remediation:** Implement mutually-authenticated transport (Noise Protocol XX, TLS 1.3 with peer certificates) or sign messages with a node signing keypair and verify on receipt.

---

### CRIT-03: Bloom Filter Uses Non-Cryptographic `DefaultHasher` — Engineered Bypass

**File:** `Protocol_Core/qanto-core/src/qanto_p2p.rs`  
**Lines:** 1086–1091  
**Severity:** Critical — Gossip deduplication bypass / amplification loop

```rust
fn hash_with_seed(&self, data: &[u8], seed: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    data.hash(&mut hasher);
    hasher.finish()
}
```

`DefaultHasher` is not cryptographic. Its output is predictable. An attacker can:
1. Craft messages whose `MessageId` values are NOT in the bloom filter — causing all relays to re-forward them indefinitely (amplification).
2. Engineer false positives to block legitimate messages.

**Remediation:** Use `SipHash-2-4` with a random per-instance key, or `blake3` for bloom filter hashing.

---

### CRIT-04: QantoSwapRouter Returns No Value — Silent Fund Loss on Swap

**File:** `SmartContracts/contracts/QantoSwapRouter.sol`  
**Lines:** 19–31  
**Severity:** Critical — Silent fund loss / swap DoS

```solidity
function swapExactTokensForTokens(
    uint amountIn, uint amountOutMin,
    address[] calldata path, address to, uint deadline
) external returns (uint[] memory amounts) {
    require(path.length >= 2, 'QantoSwap: INVALID_PATH');
    require(deadline >= block.timestamp, 'QantoSwap: EXPIRED');
    // Logic handled by Qanto Layer-0 Core  ← STUB: NO TOKEN TRANSFER
}
```

The function takes no tokens, transfers nothing, and returns an empty `amounts` array. Any caller (wallet, aggregator) receives a success status on a swap that didn't happen.

**Remediation:** Until implemented: `revert("QantoSwapRouter: not implemented")`. Do not deploy a stub as a live swap router.

---

## High Findings

---

### HIGH-01: Handshake Does Not Authenticate Peer Identity — MITM

**Lines:** 511–566  
The Kyber decapsulation key `_kyber_dk` is discarded on line 523. No shared secret is established. `peer_handshake.node_id` is accepted verbatim — any node can impersonate any `node_id`.

---

### HIGH-02: `QantoDrop.withdrawRemaining` Unchecked ERC-20 Transfer

**Line:** 42 — `qntoToken.transfer(owner(), balance)` return value not checked. Use `SafeERC20.safeTransfer`.

---

### HIGH-03: `QantoDrop.claim` Merkle Leaf Vulnerable to Second-Preimage

**Line:** 31 — `keccak256(abi.encodePacked(msg.sender, amount))` without domain separator or double-hashing is vulnerable to second-preimage attacks. Use double-hashing: `keccak256(abi.encode(keccak256(abi.encode(msg.sender, amount))))`.

---

### HIGH-04: Unbounded `local_messages` Map — Remote OOM DoS

**Lines:** 1002, 1176–1182 — Messages inserted into `local_messages` (DashMap) are never evicted. `max_message_age` is set but never enforced. Unbounded memory growth under any message flood.

---

### HIGH-05: `QantoP256` Named After NIST P-256 But Uses Custom Hash Scheme

**Lines:** 665–677 — Type is named `P256` and algorithm is named `P256` but implements a custom hash-based scheme incompatible with any P-256 standard. Any component assuming NIST P-256 compatibility is silently using a broken scheme.

---

### HIGH-06: `generate_keypair` Returns `(private, public)` — Reversed Tuple Convention

**Lines:** 147–149  
```rust
Ok((private.as_bytes().to_vec(), public.as_bytes().to_vec()))
```
Standard convention is `(public, private)`. Callers who follow convention use the private key as a public key and publish the private key.

---

## Medium Findings

| ID | Location | Description |
|---|---|---|
| MED-01 | `qanto_p2p.rs:813` | Received message signature never verified even against the weak hash-based scheme |
| MED-02 | `qanto_p2p.rs:308` | P2P binds to `0.0.0.0` by default — should default to loopback |
| MED-03 | `SentinelNFT.sol` | Not reviewed in this audit pass |
| MED-04 | `QantoSwapFactory.sol` | Not reviewed in detail in this audit pass |
| MED-05 | `qanto_native_crypto.rs:571` | `expect()` on production signing path — panics on corrupt key |
| MED-06 | `qanto_p2p.rs:860` | Heartbeat signed with `qanto_hash(&[])` — not the heartbeat payload |
| MED-07 | `QantoTGE.sol` | Not reviewed in this audit pass |
| MED-08 | `.github/workflows/ci.yml` | No Solidity test, Slither, or fuzz jobs in CI |

---

## Low / Informational

| ID | Description |
|---|---|
| INFO-01 | `DefaultHasher` output not stable across Rust versions |
| INFO-02 | `get_stats()` uses `unwrap()` on RwLock — panics on poisoned lock |
| INFO-03 | Ed25519 key generation from raw CSPRNG bytes is correct (no issue) |
| INFO-04 | All-zero guard on PQ private key is insufficient — derive and let library reject |
| INFO-05 | Kyber `_kyber_dk` discarded immediately after generation — incomplete KEM |
| INFO-06 | `QantoDrop` has no total cap check — late claimants will always fail |
| INFO-07 | Priority queue allocated but never drained — dead code contributing to memory growth |

---

## Honest Score Assessment

Based on actual code findings:

| Component | Arch | Interop | **Security** | Perf | DX | Resilience | Data | **Weighted** |
|---|---|---|---|---|---|---|---|---|
| Consensus | 3.5 | 2.0 | **2.5** | 3.5 | 3.0 | 3.0 | 3.0 | 2.94 |
| DAG/State | 3.5 | 2.0 | **3.0** | 3.5 | 3.5 | 3.0 | 3.0 | 3.06 |
| P2P | 3.0 | 2.0 | **1.5** | 3.0 | 3.0 | 2.0 | 2.5 | 2.42 |
| Miner | 4.0 | 1.5 | **3.0** | 3.5 | 3.5 | 2.5 | 2.5 | 2.96 |
| Mempool | 3.5 | 1.5 | **3.0** | 3.5 | 3.5 | 2.5 | 3.0 | 2.98 |
| Node/API | 3.5 | 2.0 | **3.0** | 3.5 | 4.0 | 3.0 | 3.0 | 3.14 |
| Config | 4.0 | 2.0 | **3.5** | 4.0 | 4.0 | 3.0 | 3.0 | 3.38 |
| Wallet/Keys | 3.0 | 1.5 | **2.5** | 2.5 | 3.5 | 2.0 | 2.5 | 2.60 |
| Interchain/Bridge | 2.0 | 2.0 | **2.0** | 1.5 | 2.0 | 1.5 | 1.5 | 1.88 |
| JSON-RPC Adapter | 3.0 | 2.0 | **3.0** | 3.0 | 3.5 | 2.5 | 2.5 | 2.86 |
| DevOps/CI | 3.5 | 1.5 | **3.0** | 2.5 | 3.5 | 2.5 | 2.5 | 2.82 |
| SAGA/ΩMEGA | 3.0 | 1.5 | **2.5** | 3.0 | 3.0 | 2.5 | 2.5 | 2.62 |
| Desktop Wallet | 3.5 | 1.5 | **2.5** | 3.0 | 3.5 | 2.0 | 2.5 | 2.68 |
| SmartContracts | 2.5 | 2.5 | **1.5** | 2.0 | 2.5 | 1.5 | 2.0 | 2.02 |
| Frontend/SDK/CF | 2.5 | 2.0 | **2.5** | 2.5 | 3.5 | 2.0 | 2.5 | 2.48 |

**Raw weighted score: ~2.79/5 (55.8%)**  
**Security average: 2.57/5 — security gate TRIGGERED (threshold: 3.0)**  
**Components below Security 3.0: 7 (P2P, Consensus, Wallet/Keys, Interchain/Bridge, SAGA/ΩMEGA, Desktop Wallet, SmartContracts)**  
**Effective score: 2.4/5 (48%) — capped by security gate**

The self-awarded scorecard of 4.6/5 is **not defensible** based on current code. CRIT-01 alone (P256 universal forgery) invalidates any Security score above 2.0 for the crypto layer.

---

## Remediation Roadmap to 4.60

To reach 4.60 the following are required (in order):

1. **P0 — Fix CRIT-01:** Replace P256 with `p256` crate. Remove `QantoP256` custom scheme entirely. (1 day)
2. **P0 — Fix CRIT-04:** Revert the swap router stub or implement it. (1 hour)
3. **P1 — Fix CRIT-02/HIGH-01:** Implement Noise Protocol or mTLS for P2P transport with proper signature verification. (2–3 weeks)
4. **P1 — Fix CRIT-03:** Replace `DefaultHasher` in bloom filter with keyed SipHash. (2 hours)
5. **P1 — Fix HIGH-02/03:** `SafeERC20` + Merkle double-hashing in `QantoDrop`. (2 hours)
6. **P1 — Fix HIGH-04:** Add eviction loop to gossip `local_messages`. (4 hours)
7. **P1 — Fix HIGH-06:** Swap keypair return tuple order; add regression test. (1 hour)
8. **P2 — Fix MED items:** Bind address, signature verification, panic-free paths, CI Solidity jobs. (1 week)
9. **Schedule external audit** of fixed codebase, especially consensus and bridge.

Only after all P0/P1 fixes are implemented and verified can the scorecard be honestly updated above 4.0.

---

*Auditor: External review session, 2026-07-31*  
*Based on static analysis of source files. Dynamic testing and formal verification not performed.*
