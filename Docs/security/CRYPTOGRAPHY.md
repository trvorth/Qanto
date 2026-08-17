# Cryptographic Review

**Qanto Layer-0 Blockchain**  
**Branch:** main

## Crypto Stack

| Component | Implementation | Status |
|-----------|---------------|--------|
| Native signatures | `qanto_native_crypto.rs` | Reviewed |
| QanHash | Custom PoW hash | Reviewed |
| ML-KEM (post-quantum) | Kyber-1024 | Reviewed |
| Ed25519 | Legacy compat (`qanto_compat.rs`) | Reviewed |
| P256 | Removed (unused imports) | Cleaned |
| Noise XX | libp2p handshake | Canonical |
| Wallet derivation | BIP-39 / BIP-44 compatible | Reviewed |
| Mnemonic import | BIP-39 | Reviewed |
| Private key import | Ed25519 + ML-KSA | Reviewed |
| RNG | `rand::rngs::OsRng` | Verified |
| Domain separation | Per-operation context strings | Verified |
| Chain binding | Chain ID in block/transaction signatures | Verified |

## Key Findings (Fixed)

1. **Unused crypto imports** (`P256Signer`, `P256Verifier`) – Removed from `qanto_native_crypto.rs`
2. **Signature verification Result handling** – Fixed `.expect()` calls to proper error handling in test code
3. **No shared authentication secrets** – Each connection uses fresh Noise handshake
4. **No secret-material logging** – All key operations avoid tracing private data

## Known-Answer Tests

- Native crypto: sign/verify round-trips
- QanHash: known input/output pairs
- ML-KEM: encapsulate/decapsulate round-trip
- Wallet derivation: BIP-44 derivation path consistency
- Mnemonic: BIP-39 test vectors

## Pending

- Miri verification of crypto operations
- Formal verification of ML-KEM integration
- Side-channel analysis of QaneHash