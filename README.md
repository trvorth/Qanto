[![Status](https://img.shields.io/badge/Status-Phase%201%3A%20Foundation%20(85%25)-orange?style=for-the-badge)](./docs/ROADMAP.md)
[![CI](https://img.shields.io/github/actions/workflow/status/trvorth/Qanto/rust.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/trvorth/Qanto/actions/workflows/rust.yml)
[![License](https://img.shields.io/badge/License-MIT-lightgrey?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/LICENSE)
[![Docs](https://img.shields.io/badge/Docs-Testnet%20Guide-blue?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/docs/testnet-guide.md)

---

# Qanto: A Heterogeneous, Post-Quantum, AI-Governed DLT Framework with Hybrid Consensus and Dynamic Sharding

![Qanto Banner](https://placehold.co/1200x300/1a1a2e/e0e0e0?text=Qanto)

**Repository for the official Rust implementation of the Qanto Protocol**  

**Author**: trvorth

---

## **About Qanto**

**Website**: https://Qanto.live (coming soon)  

**Topics**: blockchain, ai, layer-0, rust, post-quantum-cryptography, high-throughput, decentralized-finance.

Qanto is a modern Layer-0 blockchain designed for extreme performance, decentralization, and security. Its evolved architecture utilizes a Hybrid Proof-of-Work and Delegated Proof-of-Stake (PoW+DPoS) consensus model. This innovative approach uses PoW for permissionless leader election, ensuring the network remains open and secure, while DPoS is used for high-speed block production, enabling massive transaction throughput.

Key innovations include an AI-driven governance system (SAGA), post-quantum security using lattice-based cryptography (inspired by CRYSTALS-Dilithium), and a sophisticated, self-regulating difficulty adjustment algorithm.

As a foundational protocol, Qanto facilitates interoperability across its ecosystem, capable of hosting Layer-1-like chains and enabling Layer-2 scaling solutions such as rollups. As a foundational protocol, Qanto is engineered to be a standalone, production-grade system capable of supporting a high-throughput environment, the open-source (MIT) project welcomes community contributions to help build a future-proof, decentralized ecosystem.

For a comprehensive academic and technical overview, please refer to the official [**Qanto Whitepaper**](./docs/whitepaper/Qanto-whitepaper.pdf).

## **Performance Benchmarks** 📈

The following benchmarks were conducted on an Apple M-series CPU and an integrated GPU, demonstrating the performance of the core components.

| Benchmark                               | Time             | Throughput (approx.)      | Notes                                                              |
| --------------------------------------- | ---------------- | ------------------------- | ------------------------------------------------------------------ |
| **CPU Hashrate (1k Hashes)** | `~281 µs`        | **~3.55 MHash/s** | Measures the performance of the `qanhash` algorithm on a single CPU core. |
| **GPU Hashrate (1 Batch)** | `~3.67 ms`        | **~17.85 MHash/s** | Measures the performance of 65,536 hashes on an integrated GPU. |
| **Execution Layer (16,000 txs)** | `~79.7 ms`       | **~200,580 TPS** | Time to process a full block payload (signature verification & Merkle root). |
| **Hyperscale Execution (1.6M txs)** | `~62.8 ms`       | **~25,447,000 TPS** | Peak raw throughput of the sharded execution model. |

These results validate the high-throughput design of the Qanto protocol, with transaction processing speed comfortably exceeding the **10,000,000 TPS** target.

## **Structure, Key Features & Innovations**

### **Structure**

The Qanto repository is a Cargo workspace containing several key components:

* **/src:** The main `qanto` application crate which integrates the `my-blockchain` library and provides higher-level services like the SAGA AI and the main CLI.  
* `src/node.rs`: Main node orchestration, managing all services.
* `src/config.rs`: Configuration loading and validation.
* `src/qantodag.rs`: The core DAG ledger implementation natively supports both DAG-based shards and linear PoW/PoS chains within one interoperable ecosystem. A Directed Acyclic Graph structure that allows for parallel block processing, high throughput, and near-instant finality. **Heterogeneous Architecture** natively supports both DAG-based shards and linear PoW/PoS chains within one interoperable ecosystem. **Dynamic Sharding**, the network autonomously adjusts the number of active DAG shards based on real-time transactional load, ensuring scalability. **Post-Quantum Security** implements a lattice-based signature scheme (modeled after NIST standard CRYSTALS-Dilithium) for all validator attestations, ensuring long-term security.
* `src/consensus.rs`: Qanto utilizes a unique, multi-layered consensus model:
    * **Proof-of-Work (PoW):**
    * **Delegated Proof-of-Stake (DPoS):**
    * **Proof-of-Sentiency (PoSe):** This is the top-level consensus layer, managed by the **SAGA** AI pallet.
* `src/p2p.rs`: The libp2p-based peer-to-peer networking layer.
* `src/miner.rs`: Proof-of-Work puzzle solving logic.
* `src/transaction.rs`: Transaction creation and validation logic.
* `src/wallet.rs`: Encrypted wallet management.
* `src/saga.rs`: The fully-integrated  AI governance and adaptive security pallet, which functions as the network's decentralized brain.
* `src/hame.rs`: The **Hybrid Autonomous Meta-Economy (H.A.M.E.)** protocol. This advanced economic layer manages Sovereign Identities, a Meta-Reflexive Value Engine (MRVE) for tracking reputation, and a Reality-Tied Asset Web (RTAW) for tokenizing real-world events.
* `src/omega.rs`: The system's core identity and reflex protocol that provides a final layer of defense against unstable or dangerous system state transitions.
* `src/x_phyrus.rs`: The military-grade pre-boot security and diagnostics suite integrity checks and activates advanced operational protocols.
* `src/zk.rs`: (Feature-gated) ZK-proof circuit definitions.
* `src/infinite_strata_node.rs`: Proof-of-Sustained-Cloud-Presence (PoSCP) and cloud-adaptive mining logic.
* **/src/bin**: Executable crates for the node (start\_node.rs) and wallet (Qantowallet.rs).  
* **/docs**: Project documentation, including the whitepaper and launch plans.  
* **config.toml.example**: An example configuration file for the node.

The core blockchain logic is now consolidated into the `my-blockchain` package for clarity and performance.

* `myblockchain/src/lib.rs`: The central library crate containing the complete, consolidated runtime logic for the blockchain. It includes inline modules for:
    **P2P Networking** (`p2p`): A lean `libp2p` implementation for decentralized peer discovery and communication.
    **DPoS** (`dpos`): Data structures for the Delegated Proof-of-Stake system.
    **Miner** (`miner`): The Proof-of-Work engine responsible for competing in leader elections.
    **Execution Layer:** The high-performance engine for transaction processing, featuring batch signature verification.
    **Blockchain:** The core data structures and logic for managing the chain, state, and consensus rules.
* `myblockchain/src/qanhash.rs`: A dedicated module for the core **Qanhash** Proof-of-Work algorithm. It contains the CPU and GPU (OpenCL) hashing implementations and the sophisticated **Exponential Moving Average (EMA) difficulty adjustment algorithm**.
* `myblockchain/src/kernel.cl`: The high-performance **OpenCL kernel** for the `qanhash` algorithm. This code runs directly on the GPU and is heavily optimized for parallel processing using vector types and atomic operations to maximize hashing efficiency.
* `myblockchain/src/qanhash32x.rs`: A standalone, production-grade **post-quantum cryptographic kernel**. It provides a suite of quantum-resistant functions, including a custom Key Encapsulation Mechanism (KEM), a high-throughput hash function, and a memory-hard identity generation function.
    
### **Key Features & Innovations**

* **Hybrid PoW+DPoS Consensus:** A novel two-layer system that provides the decentralization and permissionless nature of PoW for leader election, while leveraging the speed of DPoS for block production to achieve 32 BPS.

* **Sophisticated Difficulty Adjustment:** A modern EMA algorithm ensures the PoW leader election rate is stable and responsive to changes in network hash rate, a critical feature for a standalone, production-grade system.

* **High-Throughput Execution Layer:** Designed from the ground up for performance, using concurrent data structures and batch transaction verification to process massive transaction volumes.

* **Standalone & Decentralized:** The entire system is self-contained. The `libp2p` networking layer and permissionless PoW leader election ensure that the network can operate and grow without any central points of failure.

* **Post-Quantum Security:** Implements a lattice-based signature scheme (modeled after NIST standard CRYSTALS-Dilithium) for all validator attestations, ensuring long-term security against quantum threats.

## **Developer & Research Materia**

* **Formal Specification (Whitepaper)**: [docs//whitepaper/Qanto-whitepaper.md](./docs/whitepaper/Qanto-whitepaper.md)
* **System Architecture Overview**: [Architecture.md](./Architecture.md)
* **API Documentation**: A complete specification for the public RPC and REST Application Programming Interfaces is slated for publication prior to the mainnet launch.
* **Command-Line Interface (CLI) Wallet**: The `Qantowallet` executable furnishes a command-line interface for all requisite wallet and cryptographic key management operations.

## **Testnet Participation**

For details on joining the public testnet, including hardware requirements, incentive programs, and bootnode addresses, please refer to the [**Testnet Launch Plan**](./docs/testnet-plan.md), [**Testnet Guide**](./docs/testnet-guide.md) and [**Qantowallet Guidance**](./docs/QANTOWALLET_GUIDE.md).

## **Security**

The security of the network is our highest priority. We have a formal plan for a comprehensive third-party audit. For more details, please see our [**Security Audit Plan**](./docs/security-audit-plan.md).

## **Contribution Protocol**

We welcome contributions from the community\! This project thrives on collaboration and outside feedback. Please read our [**Contribution Guidelines**](./CONTRIBUTING.md) to get started.  
All participants are expected to follow our [**Code of Conduct**](./CODE_OF_CONDUCT.md).

## **License**

This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.
