# Qanto: A Heterogeneous, Post-Quantum, AI-Governed DLT Framework with Hybrid Consensus and Dynamic Sharding

![Qanto Banner](https://placehold.co/1200x300/1a1a2e/e0e0e0?text=Qanto)

**Repository for the official Rust implementation of the Qanto Protocol, A Heterogeneous, Post-Quantum DLT Framework.**  

**Author**: trvorth | **License**: MIT | **Status**: Phase 1 \- Foundation (In Progress)

---
![Release](https://img.shields.io/badge/release-v2.3.4-blue.svg?style=for-the-badge)
![Docs](https://img.shields.io/badge/docs-passing-brightgreen.svg?style=for-the-badge)

![Status](https://img.shields.io/badge/status-Phase%201%3A%20Foundation-orange.svg?style=for-the-badge)
![CI](https://img.shields.io/github/actions/workflow/status/trvorth/Qanto/rust.yml?branch=main&label=ci&style=for-the-badge)
![License](https://img.shields.io/badge/license-MIT-lightgrey.svg?style=for-the-badge)
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

## Procedural Guide for Local Instantiation

The subsequent instructions delineate the procedures for obtaining and operating a functional instance of the project on a local machine for purposes of development and testing.

## **Getting Started: Running a Local Node**

These instructions will get you a copy of the project up and running on your local machine for development and testing purposes.

### **Prerequisites**

To build and run a Qanto node, you will need to have the following installed on your system:

* **Rust Toolchain**: The latest stable release of the Rust programming language and its associated Cargo build system must be installed via `rustup`.
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
    ```
* **Git**: The Git version control system is required for cloning the source code repository.
* **Build-System Dependencies**: A C++ compiler toolchain and the RocksDB library constitute essential build dependencies. The requisite installation procedures vary by operating system.

### **Build Instructions (Linux & macOS)**

1. **Install Build Essentials**:  
    * For Debian-based distributions (e.g., Ubuntu):
        ```bash
        sudo apt-get update && sudo apt-get install build-essential clang librocksdb-dev
        ```
    * For macOS systems utilizing the Homebrew package manager:
        ```bash
        xcode-select --install && brew install rocksdb
        ```
    * For Fedora, CentOS, or RHEL-based distributions:
        ```bash
        sudo dnf groupinstall "Development Tools" && sudo dnf install rocksdb-devel
        ```
 
2. **Clone and Compile**:  
    ```bash
    git clone [https://github.com/trvorth/Qanto.git](https://github.com/trvorth/Qanto.git)
    cd Qanto
    cargo build --release
    ```
    Upon successful compilation, the resultant binaries will be situated in the `target/release/` directory.

### **Build Instructions (Windows)**

Building on Windows requires the MSVC C++ toolchain and manual installation of RocksDB via vcpkg.

1.  **Installation of Microsoft C++ Build Tools**:
    * It is required to download the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
    * The installer must be executed, and the **"C++ build tools"** workload is to be selected for installation. It should be ensured that the latest Windows SDK and the English language pack are included.

2.  **Installation and Configuration of `vcpkg`**:
    * An instance of the PowerShell terminal is to be opened for the purpose of cloning the `vcpkg` repository.
        ```powershell
        git clone [https://github.com/Microsoft/vcpkg.git](https://github.com/Microsoft/vcpkg.git)
        cd vcpkg
        ./bootstrap-vcpkg.bat
        ./vcpkg integrate install
        ```

3.  **Installation of RocksDB via `vcpkg`**:
    * The `vcpkg` utility shall be used to install the 64-bit version of the RocksDB library. This operation may require a considerable amount of time to complete.
        ```powershell
        ./vcpkg.exe install rocksdb:x64-windows
        ```

4.  **Configuration of Environment Variables**:
    * An environment variable must be established to inform the Cargo build system of the location of the RocksDB library files. A PowerShell terminal with administrative privileges must be utilized to execute the following command, with the file path adjusted to correspond to the `vcpkg` installation directory:
        ```powershell
        [System.Environment]::SetEnvironmentVariable('ROCKSDB_LIB_DIR', 'C:\path\to\vcpkg\installed\x64-windows\lib', [System.EnvironmentVariableTarget]::Machine)
        ```
    * **Note Bene**: A restart of the terminal or Integrated Development Environment (IDE) is mandatory for this environment variable modification to take effect.

5.  **Repository Cloning and Compilation**:
    * A new terminal instance must be opened.
    ```bash
    git clone [https://github.com/trvorth/Qanto.git](https://github.com/trvorth/Qanto.git)
    cd Qanto
    cargo build --release
    ```
    The compiled binaries will be located at `target/release/`.

### **Operational Quick Start**

1.  **Wallet Credential Generation**:
    The `Qantowallet` utility is provided for the creation of a new keypair. Upon execution, the operator will be prompted to supply a secure passphrase for the encryption of the resultant wallet file, `wallet.key`.
    ```bash
    cargo run --release --bin qantowallet -- generate --output wallet.key
    ```
        (You will be prompted for a secure password).
    
   **Critical Notice**: The `Public Address` emitted by this operation must be copied. Furthermore, the associated `Mnemonic Phrase` must be transcribed and stored in a secure, offline medium for recovery purposes.

2.  **Node Configuration**:
    An exemplary configuration file is provided within the repository. A local copy must be created for operational use.
    ```bash
    cp config.toml.example config.toml
    ```
    The newly created `config.toml` file must be edited to substitute the placeholder value of the `initial_validators` field with the public address generated in the preceding step.

3.  **Node Instantiation**:
    The Qanto node may be initiated by executing the `start_node` binary. The system will automatically load the configuration and wallet files.
    ```bash
    # You will be prompted for the wallet password
    cargo run --release --features infinite-strata --bin qanto -- start --config config.toml --wallet wallet.key --clean
    ```
    The operator will be prompted to supply the wallet passphrase, after which the node will initialize its services and commence network operations.
4.  **Demonstrating the ΛΣ-ΩMEGA Module**

To see a simulation of the conscious security layer, run the `omega_test` binary:

```bash
cargo run --bin omega_test
```
## **Testnet Participation**

For details on joining the public testnet, including hardware requirements, incentive programs, and bootnode addresses, please refer to the [**Testnet Launch Plan**](./docs/testnet-plan.md), [**Testnet Guide**](./docs/testnet-guide.md) and [**Qantowallet Guidance**](./docs/QANTOWALLET_GUIDE.md).

## **Security**

The security of the network is our highest priority. We have a formal plan for a comprehensive third-party audit. For more details, please see our [**Security Audit Plan**](./docs/security-audit-plan.md).

## **Contribution Protocol**

We welcome contributions from the community\! This project thrives on collaboration and outside feedback. Please read our [**Contribution Guidelines**](./CONTRIBUTING.md) to get started.  
All participants are expected to follow our [**Code of Conduct**](./CODE_OF_CONDUCT.md).

## **License**

This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.
