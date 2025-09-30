# Qanto Dependency Migration Guide

This guide outlines how to replace external cryptographic dependencies with qanto-native implementations using the compatibility layer.

## Overview

The Qanto project is transitioning from external dependencies to native implementations to enhance decentralization and reduce attack surface. The `qanto_compat` module provides drop-in replacements for external cryptographic libraries.

## Migration Steps

### 1. Ed25519 (ed25519_dalek)

**Before:**
```rust
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
```

**After:**
```rust
use crate::qanto_compat::ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
```

**Example Migration:**
```rust
// Old code
use ed25519_dalek::{SigningKey, Signer};
let signing_key = SigningKey::generate(&mut rng);
let signature = signing_key.sign(message);

// New code (same interface, native implementation)
use crate::qanto_compat::ed25519_dalek::{SigningKey, Signer};
let signing_key = SigningKey::generate(&mut rng);
let signature = signing_key.sign(message);
```

### 2. Post-Quantum Cryptography (pqcrypto-*)

**Before:**
```rust
use pqcrypto_dilithium::dilithium5::*;
use pqcrypto_traits::sign::*;
```

**After:**
```rust
use crate::qanto_compat::pqcrypto_compat::mldsa65::*;
use crate::qanto_compat::pqcrypto_compat::traits::sign::*;
```

**Example Migration:**
```rust
// Old code
use pqcrypto_dilithium::dilithium5::{keypair, sign_detached, verify_detached_signature};
let (pk, sk) = keypair();
let sig = sign_detached(message, &sk);
verify_detached_signature(message, &sig, &pk).unwrap();

// New code (same interface, native implementation)
use crate::qanto_compat::pqcrypto_compat::mldsa65::{keypair, sign_detached, verify_detached_signature};
let (pk, sk) = keypair();
let sig = sign_detached(message, &sk);
verify_detached_signature(message, &sig, &pk).unwrap();
```

### 3. P-256 ECDSA (p256)

**Before:**
```rust
use p256::ecdsa::{SigningKey, VerifyingKey, Signature};
use p256::ecdsa::signature::{Signer, Verifier};
```

**After:**
```rust
use crate::qanto_compat::p256_compat::ecdsa::{SigningKey, VerifyingKey, Signature};
use crate::qanto_compat::p256_compat::ecdsa::signature::{Signer, Verifier};
```

### 4. Libp2p Identity (libp2p-identity)

**Before:**
```rust
use libp2p_identity::{Keypair, PublicKey, PeerId};
```

**After:**
```rust
use crate::qanto_compat::libp2p_identity::{Keypair, PublicKey, PeerId};
```

## Files to Migrate

Based on the codebase analysis, the following files need migration:

### High Priority (Core Functionality)
1. `src/wallet.rs` - Wallet operations with Ed25519 + Dilithium
2. `src/transaction.rs` - Transaction signing and verification
3. `src/consensus.rs` - Consensus mechanism signatures
4. `src/post_quantum_crypto.rs` - Post-quantum implementations
5. `src/privacy.rs` - Privacy features with ring signatures

### Medium Priority (Network and P2P)
1. `src/qanto_p2p.rs` - P2P networking with libp2p
2. `src/qanto_net.rs` - Network message handling
3. `src/node.rs` - Node operations and peer management

### Lower Priority (Utilities and Tests)
1. `src/crypto.rs` - General cryptographic utilities
2. `tests/` - Test files using external crypto
3. `examples/` - Example code

## Migration Process

### Step 1: Update Imports
Replace external crate imports with compatibility layer imports:

```bash
# Find all ed25519_dalek imports
grep -r "use ed25519_dalek" src/

# Replace with compatibility imports
sed -i 's/use ed25519_dalek/use crate::qanto_compat::ed25519_dalek/g' src/*.rs
```

### Step 2: Update Cargo.toml
Comment out or remove external dependencies:

```toml
# [dependencies]
# ed25519-dalek = { version = "2.1", features = ["rand_core"] }  # Temporarily restored for compatibility
# pqcrypto-dilithium = "0.5"  # Temporarily restored for compatibility
# p256 = { version = "0.13", features = ["ecdsa"] }  # Temporarily restored for compatibility
```

### Step 3: Test Migration
Run tests to ensure compatibility:

```bash
cargo test --all-features
cargo clippy --all-targets --all-features
```

### Step 4: Verify Functionality
Ensure all cryptographic operations work correctly:

```bash
# Test wallet operations
cargo run --bin wallet -- generate

# Test node operations
cargo run --bin node -- --test-crypto

# Run performance tests
cargo run --bin performance_test
```

## Benefits of Migration

1. **Decentralization**: Removes dependency on external cryptographic libraries
2. **Security**: Reduces attack surface by eliminating external code
3. **Control**: Full control over cryptographic implementations
4. **Auditability**: All cryptographic code is within the project
5. **Consistency**: Unified approach to cryptographic operations
6. **Performance**: Optimized for Qanto's specific use cases

## Rollback Plan

If issues arise during migration:

1. Revert import changes:
   ```bash
   git checkout -- src/
   ```

2. Restore external dependencies in `Cargo.toml`

3. Run tests to ensure functionality:
   ```bash
   cargo test --all-features
   ```

## Validation Checklist

- [ ] All imports updated to use compatibility layer
- [ ] External dependencies removed from Cargo.toml
- [ ] All tests pass
- [ ] Clippy warnings resolved
- [ ] Wallet operations work correctly
- [ ] Node can start and connect to peers
- [ ] Transaction signing/verification works
- [ ] Consensus mechanism functions properly
- [ ] Performance benchmarks show acceptable results

## Notes

- The compatibility layer maintains the same API as external libraries
- All cryptographic operations use qanto-native implementations
- Error types are mapped to maintain compatibility
- Serialization formats are preserved for network compatibility
- Performance characteristics may differ slightly but should be acceptable

## Support

If you encounter issues during migration:

1. Check that all imports are correctly updated
2. Verify that the compatibility layer exports match your usage
3. Run `cargo clean && cargo build` to ensure clean compilation
4. Check for any remaining references to external crates

The migration should be transparent to application logic while providing the benefits of native implementations.