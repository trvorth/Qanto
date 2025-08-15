# Qanto Architecture

This document provides a comprehensive technical overview of the Qanto blockchain architecture, component structure, and development guidelines for contributors.

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Components](#core-components)
3. [Project Structure](#project-structure)
4. [Consensus Architecture](#consensus-architecture)
5. [Key Innovations](#key-innovations)
6. [Performance & Scalability](#performance--scalability)
7. [Interoperability & Privacy](#interoperability--privacy)
8. [Development Structure](#development-structure)
9. [Contributing Guidelines](#contributing-guidelines)

## System Overview

Qanto is a production-ready Layer-0 blockchain protocol engineered for quantum resistance, infinite scalability, and seamless cross-chain interoperability. Built with Rust for maximum performance, security, and reliability. The architecture utilizes a hybrid consensus model combining Proof-of-Work (PoW) for permissionless leader election with Delegated Proof-of-Stake (DPoS) for high-speed block production.

## Core Components

The Qanto repository is a Cargo workspace containing several key components:

### Main Application Crate (`/src`)

The main `qanto` application crate integrates the `my-blockchain` library and provides higher-level services:

* **`src/node.rs`**: Main node orchestration, managing all services
* **`src/config.rs`**: Configuration loading and validation
* **`src/qantodag.rs`**: Core DAG ledger implementation with:
  - Heterogeneous architecture supporting both DAG-based shards and linear PoW/PoS chains
  - Dynamic sharding that autonomously adjusts based on transactional load
  - Post-quantum security with lattice-based signature schemes (CRYSTALS-Dilithium)
* **`src/consensus.rs`**: Multi-layered consensus implementation
* **`src/p2p.rs`**: libp2p-based peer-to-peer networking layer
* **`src/miner.rs`**: Proof-of-Work puzzle solving logic
* **`src/transaction.rs`**: Transaction creation and validation logic
* **`src/wallet.rs`**: Encrypted wallet management
* **`src/saga.rs`**: AI governance and adaptive security pallet (network's decentralized brain)
* **`src/hame.rs`**: Hybrid Autonomous Meta-Economy (H.A.M.E.) protocol
  - Sovereign Identities management
  - Meta-Reflexive Value Engine (MRVE) for reputation tracking
  - Reality-Tied Asset Web (RTAW) for real-world event tokenization
* **`src/omega.rs`**: Core identity and reflex protocol for system state protection
* **`src/x_phyrus.rs`**: Military-grade pre-boot security and diagnostics suite
* **`src/zk.rs`**: Feature-gated ZK-proof circuit definitions
* **`src/infinite_strata_node.rs`**: Proof-of-Sustained-Cloud-Presence (PoSCP) and cloud-adaptive mining
* **`/src/bin`**: Executable crates for node (`start_node.rs`) and wallet (`Qantowallet.rs`)

### Blockchain Core Library (`myblockchain/`)

The core blockchain logic is consolidated into the `my-blockchain` package:

* **`myblockchain/src/lib.rs`**: Central library crate with inline modules:
  - **P2P Networking** (`p2p`): Lean libp2p implementation for peer discovery
  - **DPoS** (`dpos`): Delegated Proof-of-Stake system data structures
  - **Miner** (`miner`): PoW engine for leader election competition
  - **Execution Layer**: High-performance transaction processing with batch signature verification
  - **Blockchain**: Core data structures and consensus rule logic

* **`myblockchain/src/qanhash.rs`**: Core Qanhash PoW algorithm module:
  - CPU and GPU (OpenCL) hashing implementations
  - Sophisticated Exponential Moving Average (EMA) difficulty adjustment

* **`myblockchain/src/kernel.cl`**: High-performance OpenCL kernel:
  - GPU-optimized qanhash algorithm
  - Vector types and atomic operations for parallel processing

* **`myblockchain/src/qanhash32x.rs`**: Post-quantum cryptographic kernel:
  - Custom Key Encapsulation Mechanism (KEM)
  - High-throughput hash function
  - Memory-hard identity generation function

## Project Structure

```
qanto/
├── src/                    # Main application crate
│   ├── bin/               # Executable binaries
│   │   ├── start_node.rs  # Main node entry point
│   │   └── Qantowallet.rs # CLI wallet application
│   ├── node.rs            # Main node orchestration
│   ├── config.rs          # Configuration loading and validation
│   ├── qantodag.rs        # Core DAG ledger implementation
│   ├── consensus.rs       # Multi-layered consensus implementation
│   ├── p2p.rs             # libp2p-based networking layer
│   ├── miner.rs           # Proof-of-Work puzzle solving logic
│   ├── transaction.rs     # Transaction creation and validation
│   ├── wallet.rs          # Encrypted wallet management
│   ├── saga.rs            # AI governance and adaptive security
│   ├── hame.rs            # Hybrid Autonomous Meta-Economy protocol
│   ├── omega.rs           # Core identity and reflex protocol
│   ├── x_phyrus.rs        # Military-grade pre-boot security
│   ├── zk.rs              # ZK-proof circuit definitions
│   └── infinite_strata_node.rs # PoSCP and cloud-adaptive mining
├── myblockchain/          # Core blockchain library
│   ├── src/
│   │   ├── lib.rs         # Central library with inline modules
│   │   ├── qanhash.rs     # Core Qanhash PoW algorithm
│   │   ├── kernel.cl      # OpenCL kernel for GPU mining
│   │   ├── qanhash32x.rs  # Post-quantum cryptographic kernel
│   │   ├── jsonrpc_server.rs # JSON-RPC server implementation
│   │   └── main.rs        # Blockchain node main entry point
│   ├── Cargo.toml         # Blockchain library dependencies
│   └── benches/           # Performance benchmarks
├── tools/
│   └── relayer/           # Cross-chain relayer implementation
├── docs/                  # Project documentation
│   ├── Architecture.md    # This architecture document
│   ├── whitepaper/        # Technical whitepaper
│   └── testnet-guide.md   # Testnet participation guide
├── config/                # Configuration files
│   ├── config.toml.example # Example node configuration
│   └── config.production.toml # Production configuration
├── qanto-docs/            # Documentation portal
└── tests/                 # Integration tests
```

### Key Directory Descriptions

- **`src/`**: Main application crate integrating the blockchain library with higher-level services
- **`myblockchain/`**: Core blockchain logic consolidated for clarity and performance
- **`tools/relayer/`**: Cross-chain interoperability relayer for IBC protocol
- **`docs/`**: Comprehensive documentation including whitepapers and guides
- **`qanto-docs/`**: Docusaurus-based documentation portal for public access
- **`config/`**: Node configuration templates and production settings

## Consensus Architecture

Qanto utilizes a unique, multi-layered consensus model:

### 1. Proof-of-Work (PoW)
- Permissionless leader election
- ASIC-resistant Qanhash algorithm
- GPU-optimized mining with quantum resistance

### 2. Delegated Proof-of-Stake (DPoS)
- High-speed block production (32 BPS)
- Validator set management
- Byzantine fault tolerance

### 3. Proof-of-Sentiency (PoSe)
- Top-level consensus layer
- Managed by SAGA AI pallet
- Adaptive security and governance

## Key Innovations

### Hybrid PoW+DPoS Consensus
A novel two-layer system providing:
- Decentralization and permissionless nature of PoW for leader election
- Speed of DPoS for block production (32 BPS)
- Best of both consensus mechanisms

### Sophisticated Difficulty Adjustment
- Modern EMA algorithm for stable PoW leader election rate
- Responsive to network hash rate changes
- Critical for standalone, production-grade operation

### High-Throughput Execution Layer
- Ground-up performance design
- Concurrent data structures
- Batch transaction verification
- Massive transaction volume processing

### Standalone & Decentralized Architecture
- Self-contained system design
- libp2p networking layer
- Permissionless PoW leader election
- No central points of failure

### Post-Quantum Security
- CRYSTALS-Dilithium digital signatures
- Kyber KEM for key encapsulation
- Quantum-resistant cryptographic primitives
- Future-proof security model

## Performance & Scalability

### Ultra-High Throughput
- **25,447,000 TPS** (hyperscale configuration)
- **200,580 TPS** (execution layer baseline)
- **320,000 transactions per block** with DAG-based parallel processing
- **32 BPS** (Blocks Per Second) target with DPoS consensus

### Finality & Confirmation
- **Sub-100ms finality** for lightning-fast transaction confirmation
- **Deterministic ordering** in DAG structure prevents double-spending
- **Byzantine fault tolerance** with 2/3+ honest validator assumption
- **Instant settlement** for cross-shard atomic transactions

### Mining & Resource Optimization
- **GPU-Optimized Mining** with Qanhash algorithm
- **ASIC resistance** through memory-hard functions
- **Dynamic difficulty adjustment** using Exponential Moving Average (EMA)
- **Energy-efficient** hybrid consensus reducing overall power consumption

### AI-Powered Network Optimization
- **Machine learning** for network performance tuning via SAGA AI
- **Adaptive sharding** based on real-time transactional load
- **Predictive scaling** to handle traffic spikes
- **Self-healing network** with automated issue detection and resolution

## Interoperability & Privacy

### Cross-Chain Infrastructure
- **Native bridge support** for Ethereum, Bitcoin, and major blockchains
- **IBC Protocol** (Inter-Blockchain Communication) with light client verification
- **Atomic swaps** for trustless cross-chain asset exchanges using HTLC
- **Universal asset representation** across connected chains

### Privacy & Confidentiality
- **Zero-Knowledge Privacy** using ZK-SNARKs for confidential transactions
- **Selective disclosure** allowing users to control information visibility
- **Ring signatures** for transaction unlinkability
- **Stealth addresses** for enhanced recipient privacy

### Interchain Security
- **Shared security model** extending Qanto's security to connected chains
- **Cross-chain validation** with cryptographic proofs
- **Fraud detection** mechanisms for bridge operations
- **Economic incentives** for honest relayer behavior

### Developer Experience
- **Multi-language SDKs** (JavaScript, Python, Rust, Go)
- **Unified APIs** for cross-chain development
- **Smart contract portability** across supported chains
- **Comprehensive tooling** for dApp development and deployment

## Development Structure

### Project Layout
```
qanto/
├── src/                    # Main application crate
│   ├── bin/               # Executable binaries
│   └── *.rs              # Core application modules
├── myblockchain/          # Core blockchain library
│   └── src/              # Library implementation
├── docs/                  # Documentation
├── config/               # Configuration files
└── tests/                # Integration tests
```

### Build System
- Cargo workspace configuration
- Release optimization profiles
- Feature-gated components
- Cross-platform compatibility

### Testing Strategy
- Unit tests for individual components
- Integration tests for system interactions
- Performance benchmarks
- Security audits

## Contributing Guidelines

### Development Environment
1. **Rust**: 1.75+ with Cargo
2. **System**: Linux/macOS/Windows
3. **Memory**: 16GB+ RAM recommended
4. **Storage**: 500GB+ SSD for full development

### Code Standards
- Follow Rust idioms and best practices
- Comprehensive documentation
- Unit test coverage
- Performance considerations
- Security-first development

### Contribution Process
1. Fork the repository
2. Create feature branch
3. Implement changes with tests
4. Submit pull request
5. Code review and integration

### Architecture Decisions
- Document significant architectural changes
- Consider performance implications
- Maintain backward compatibility
- Security impact assessment

For specific implementation details and API documentation, refer to the inline code documentation and the [Qanto Whitepaper](./docs/whitepaper/Qanto-whitepaper.pdf).