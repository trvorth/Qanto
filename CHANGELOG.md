# Changelog

All notable changes to the Qanto Layer-0 Protocol will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - "Genesis Block" - 2026-04-06

### Added
- **Initial Protocol Launch**: Release of the Qanto Layer-0 foundation.
- **Quantum-Resistant Cryptography**: Implementation of post-quantum signature schemes and key encapsulation.
- **Hybrid Consensus**: Integration of Proof-of-Work (PoW) and Directed Acyclic Graph (DAG) for maximum throughput and security.
- **ZK-Proof System**: Native support for Groth16 and PLONK circuits including Range, Membership, and Balance proofs.
- **Enterprise Explorer**: Real-time blockchain visualization and telemetry.
- **Genesis Smart Contracts**: Deployment of the core QNTO token contracts on Mainnet.

### Fixed
- **RPC Stability**: Hardened JSON-RPC server with Ethers.js v6 compatibility.
- **Gas Metrics**: Resolved transaction fee estimation inconsistencies and fixed `effectiveGasPrice` parsing.
- **Workspace Recovery**: Restored critical orchestrators and entry-points after the Phase 30 Git index purge.

### Security
- **Institutional Hardening**: Comprehensive `.gitignore` and `.gitleaks.toml` configuration to protect private keys and environmental tokens.
- **Silent Build Standard**: Zero-warning Rust compiler policy for all production releases.

## [0.9.0] - Phase 1 to 29 (Development)

### Added
- Feature-rich ZK-circuits (Voting, Identity).
- Peer-to-Peer (P2P) networking with adaptive mining.
- Multi-threaded block production and transaction verification.
- Comprehensive telemetry tracking for block height, TPS, and gas prices.

### Changed
- Refactored core logging from `println!` to structured `log` macros for improved observability.
- Upgraded website build system to Vite 5.x for improved loading performance.

---
[1.0.0]: https://github.com/trvorth/Qanto/releases/tag/v1.0.0
[0.9.0]: https://github.com/trvorth/Qanto/compare/v0.1.0...v0.9.0
