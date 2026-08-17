# Audit Scope and Build Guide

**Qanto Layer-0 Blockchain**  
**Branch:** main
**HEAD:** 716c457

## Audit Scope

This security audit covers:

### In Scope
- `Protocol_Core/qanto-core/` – Core crate: crypto, serde, types, storage traits
- `Protocol_Core/qanto-node/` – Node crate: consensus, DAG, mining, P2P, tx, state, mempool, wallet, RPC, IPC, governance (SAGA), metrics
- `Protocol_Core/qanto-rpc/` – RPC crate
- `Protocol_Core/qanto-zk-sdk/` – ZK SDK crate
- `Protocol_Core/qanto-contracts/` – Smart contract crate

### Out of Scope
- Frontend/web components
- Third-party SD-K implementations
- AWS infrastructure configuration (infrastructure security is separate)
- Hardware security module integration

## Reproducible Build

### Prerequisites

- Rust `/rustup-toolchain.toml` (channel defined in repository)
- Platform: Linux x86_64-musl (primary), macOS aarch64 (development)

### Build Commands

```bash
# Install toolchain from rustup-toolchain.toml
rustup show

# Build workspace
cargo build --workspace --all-targets --locked

# Check
cargo check --workspace --all-targets --locked

# Lint (warnings-denied)
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Test
cargo test --workspace --all-targets --locked

# Format check
cargo fmt --all --check

# Dependency audit
cargo audit

# Vendor (for offline builds)
cargo vendor
```

### Verified Toolchain Versions

| Tool | Version |
|------|---------|
| rustc | 1.88.0 (from rustup-toolchain.toml) |
| cargo | 1.88.0 |
| cargo-audit | 0.21.2 |
| cargo-fuzz | 0.12.0 |
| rustfmt | nightly-2025-01-01 |

### Artifact Hashes

To be generated at release time.

## Security Test Commands

```bash
# Dependency audit
cargo audit

# Dependency license and deny checks
cargo deny check

# Secret scan
gitleaks detect --source .

# Fuzzing
cd Protocol_Core/qanto-node
cargo fuzz run fuzz_tx_parsing -- -max_total_time=300
cargo fuzz run fuzz_bridge_proofs -- -max_total_time=300
```