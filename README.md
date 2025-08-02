[![Status](https://img.shields.io/badge/Status-Phase%201%3A%20Foundation%20(85%25)-orange?style=for-the-badge)](./docs/ROADMAP.md)
[![CI](https://img.shields.io/github/actions/workflow/status/trvorth/Qanto/rust.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/trvorth/Qanto/actions/workflows/rust.yml)
[![License](https://img.shields.io/badge/License-MIT-lightgrey?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/LICENSE)
[![Docs](https://img.shields.io/badge/Docs-Testnet%20Guide-blue?style=for-the-badge)](https://github.com/trvorth/Qanto/blob/main/docs/testnet-guide.md)

---

# Qanto: A Heterogeneous, Post-Quantum, AI-Governed DLT Framework with Hybrid Consensus and Dynamic Sharding

![Qanto Banner](https://placehold.co/1200x300/1a1a2e/e0e0e0?text=Qanto)

**Repository for the official Rust implementation of the Qanto Protocol**  

**Author**: trvorth | **License**: MIT | **Status**: Phase 1 \- Foundation (In Progress)

---

## **About Qanto**

**Website**: https://Qanto.live (coming soon)  

**Topics**: blockchain, ai, layer-0, dag, rust, post-quantum-cryptography, fintech, decentralized-finance.

Qanto is a modern Layer-0 blockchain designed for high performance and security. Its hybrid architecture combines Directed Acyclic Graphs (DAGs) for rapid, parallel transaction processing with traditional Proof-of-Work (PoW) and Proof-of-Stake (PoS) chains for robust security.

Key innovations include an AI-driven governance system, post-quantum security using lattice-based cryptography (inspired by CRYSTALS-Dilithium), and advanced privacy features like zk-SNARKs and homomorphic encryption.

As a foundational protocol, Qanto facilitates interoperability across its ecosystem, capable of hosting Layer-1-like chains and enabling Layer-2 scaling solutions such as rollups. Currently in its foundational development phase, the open-source (MIT) project welcomes community contributions to help build a future-proof, decentralized ecosystem.

For a comprehensive academic and technical overview, please refer to the official [**Qanto Whitepaper**](./docs/whitepaper/Qanto-whitepaper.pdf).

## **Performance Benchmarks** 📈

The following benchmarks were conducted on an Apple M-series CPU and an integrated GPU, demonstrating the performance of the core components.

| Benchmark                               | Time             | Throughput (approx.)      | Notes                                                              |
| --------------------------------------- | ---------------- | ------------------------- | ------------------------------------------------------------------ |
| **CPU Hashrate (1k Hashes)** | `~338 µs`        | **~2.96 MHash/s** | Measures the performance of the `qanhash` algorithm on a single CPU core. |
| **GPU Hashrate (1 Batch)** | `~2.42 ms`        | **~3.47 GHash/s** | Measures the performance of 8.3M hashes on an integrated GPU.        |
| **Execution Layer (3,125 txs)** | `~15.4 ms`       | **~202,000 TPS** | Time to process a full block payload (signature verification & Merkle root). |

These results validate the high-throughput design of the Qanto protocol, with transaction processing speed comfortably exceeding the **100,000 TPS** target.

## **Structure, Key Features & Innovations**

**Structure and Innovations**

The Qanto repository is a Cargo workspace containing several key components:

* **/src**: The main library crate containing the core logic.  
* `src/node.rs`: Main node orchestration, managing all services.
* `src/config.rs`: Configuration loading and validation.
* `src/qantodag.rs`: The core DAG ledger implementation natively supports both DAG-based shards and linear PoW/PoS chains within one interoperable ecosystem. A Directed Acyclic Graph structure that allows for parallel block processing, high throughput, and near-instant finality. **Heterogeneous Architecture**: Natively supports both DAG-based shards and linear PoW/PoS chains within one interoperable ecosystem. **Dynamic Sharding**: The network autonomously adjusts the number of active DAG shards based on real-time transactional load, ensuring scalability. **Post-Quantum Security**: Implements a lattice-based signature scheme (modeled after NIST standard CRYSTALS-Dilithium) for all validator attestations, ensuring long-term security.
* `src/consensus.rs`: Qanto utilizes a unique, multi-layered consensus model:
    * **Proof-of-Work (PoW):**
    * **Proof-of-Stake (PoS):**
    * **Proof-of-Sentiency (PoSe):** This is the top-level consensus layer, managed by the **SAGA** AI pallet.
* `src/p2p.rs`: The libp2p-based peer-to-peer networking layer.
* `src/miner.rs`: Proof-of-Work puzzle solving logic.
* `src/transaction.rs`: Transaction creation and validation logic.
* `src/wallet.rs`: Encrypted wallet management.
* `src/saga.rs`: The fully-integrated  AI governance and adaptive security pallet, which functions as the network's decentralized brain.
* `src/omega.rs`: The system's core identity and reflex protocol that provides a final layer of defense against unstable or dangerous system state transitions.
* `src/x_phyrus.rs`: The military-grade pre-boot security and diagnostics suite integrity checks and activates advanced operational protocols.
* `src/zk.rs`: (Feature-gated) ZK-proof circuit definitions.
* `src/infinite_strata_node.rs`: Proof-of-Sustained-Cloud-Presence (PoSCP) and cloud-adaptive mining logic.
* **/src/bin**: Executable crates for the node (start\_node.rs) and wallet (Qantowallet.rs).  
* **/docs**: Project documentation, including the whitepaper and launch plans.  
* **config.toml.example**: An example configuration file for the node.

**Features**

* **On-Chain Governance**: A decentralized, stake-weighted governance mechanism allows for protocol upgrades and parameter changes without contentious hard forks.  
* **Advanced Cryptography**: Includes specifications for Zero-Knowledge Proofs (zk-SNARKs) and Homomorphic Encryption for future privacy-preserving features.  
* **Advanced Security**: Features a novel on-chain Intrusion Detection System (IDS) that economically penalizes validators for anomalous behavior.

## **Developer & Research Materia**

* **Formal Specification (Whitepaper)**: [docs//whitepaper/Qanto-whitepaper.md](./docs/whitepaper/Qanto-whitepaper.md)
* **System Architecture Overview**: [Architecture.md](./Architecture.md)
* **API Documentation**: A complete specification for the public RPC and REST Application Programming Interfaces is slated for publication prior to the mainnet launch.
* **Command-Line Interface (CLI) Wallet**: The `Qantowallet` executable furnishes a command-line interface for all requisite wallet and cryptographic key management operations.

## **Procedural Guide for Local Instantiation**

The subsequent instructions delineate the procedures for obtaining and operating a functional instance of the project on a local machine for purposes of development and testing.

## **Testnet Participation**

For details on joining the public testnet, including hardware requirements, incentive programs, and bootnode addresses, please refer to the [**Testnet Launch Plan**](./docs/testnet-plan.md), [**Testnet Guide**](./docs/testnet-guide.md) and [**Qantowallet Guidance**](./docs/QANTOWALLET_GUIDE.md).

## **Security**

The security of the network is our highest priority. We have a formal plan for a comprehensive third-party audit. For more details, please see our [**Security Audit Plan**](./docs/security-audit-plan.md).

## **Contribution Protocol**

We welcome contributions from the community\! This project thrives on collaboration and outside feedback. Please read our [**Contribution Guidelines**](./CONTRIBUTING.md) to get started.  
All participants are expected to follow our [**Code of Conduct**](./CODE_OF_CONDUCT.md).

## **License**

This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.
