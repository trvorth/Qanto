# Qanto: A Formal Specification for a Heterogeneous, Post-Quantum Framework with Hybrid Consensus and Dynamic Sharding


**A Heterogeneous, Post-Quantum DLT Framework**

![Qanto Banner](https://placehold.co/1200x300/1a1a2e/e0e0e0?text=Qanto)

**Repository for the official Rust implementation of the Qanto Protocol.**  

**Author**: trvorth | **License**: MIT | **Status**: Phase 1 \- Foundation (In Progress)

---

## **About Qanto**

**Website**: https://Qanto.live (coming soon)  

**Topics**: blockchain, layer-0, dag, rust, post-quantum-cryptography, fintech, decentralized-finance  
Qanto is a highly modern blockchain platform that has been developed to guarantee the utmost security, performance, and integrity in all operational aspects.    It incorporates a novel consensus mechanism, an AI-driven governance structure, and a complex architecture for peer-to-peer networking.    On the other hand, Rust's Layer-0 protocol aims to provide a safe, scalable, and interoperable platform for decentralised applications and monetary transactions.    Its multi-faceted architecture mixes traditional Proof-of-Work (PoW) and Proof-of-Stake (PoS) chains with Directed Acyclic Graphs (DAGs) for high-throughput parallel transaction processing and enhanced security.    Essential features of post-quantum cryptography include a hybrid consensus framework that combines Proof-of-Work for block proposals and Proof-of-Stake for deterministic finality, dynamic sharding to accommodate large transaction volumes, and lattice-based signatures (inspired by CRYSTALS-Dilithium) for everlasting security.    Qanto's goal is to make on-chain governance easier and more efficient while simultaneously improving privacy-preserving capabilities via the use of sophisticated cryptographic methods like zk-SNARKs and homomorphic encryption.

While primarily a Layer-0 protocol facilitating interoperability across its ecosystem of shards and chains, Qanto can host Layer-1-like chains within its framework, processing transactions and smart contracts independently. Additionally, its planned zk-SNARKs integration could enable Layer-2 scaling solutions, such as rollups, on its shards, enhancing throughput while leveraging Qanto’s interoperability. Currently in Phase 1 (Foundation), the repository includes core components like the DAG ledger, node orchestrator, P2P networking, and wallet functionality, alongside documentation for local setup and testnet participation. Licensed under the MIT License, Qanto welcomes community contributions to drive its vision of a future-proof decentralized ecosystem.

For a comprehensive academic and technical overview, please refer to the official [**Qanto Whitepaper**](./Qanto-whitepaper.pdf).

## X-PHYRUS™ Protocol Stack 🪖

The **X-PHYRUS™ Protocol Stack** is a military-grade, pre-boot initialization and system integrity framework integrated directly into the Qanto node. It ensures maximum security, stability, and operational readiness by running a suite of advanced diagnostics and activating specialized protocols before the main node services are launched. This proactive approach prevents common hangs, detects sophisticated threats, and configures the node for optimal performance in any environment. Key components will list below:

* **Zero-Hang™ Bootloader**: Performs critical pre-flight checks on system entropy, file permissions, and chain state integrity to eliminate common startup hangs.
* **DeepCore Sentinel™**: Conducts an initial system security scan to detect known Advanced Persistent Threat (APT) toolchains and other high-risk system vulnerabilities.
* **HydraDeploy™**: Automatically detects multi-node deployment manifests (`hydra_manifest.toml`) to activate specialized coordination and scaling logic.
* **PeerFlash™**: Activates an advanced peer discovery overlay when a priority peer list is provided in the configuration, ensuring robust network connectivity.
* **QuantumShield™**: Engages enhanced cryptographic validation protocols when ZK-proofs are enabled, providing a firewall layer against quantum computing threats.
* **CloudAnchor™**: Detects cloud provider environments (AWS, GCP, Azure) to enable cloud-native elastic mining and scaling capabilities.
* **PhaseTrace™**: Verifies the database backend to enable a traceable block propagation graph for enhanced auditability.
* **TraceForce-X™**: Activates a governance and compliance tracing stack when a `traceforce_watchlist.csv` file is present, ensuring regulatory adherence.

## ΛΣ-ΩMEGA™ Protocol 🔒

A key innovation in Qanto is ΛΣ-ΩMEGA™ (Lambda Sigma Omega), Conscious Security Layer, a reflexive security module integrated directly into the node's core logic. It functions as a "digital immune system" by:

1.  **Maintaining a Digital Identity**: Each node has a unique, evolving identity based on a constant stream of system entropy.
2.  **Reflecting on Actions**: Before processing critical actions like transactions, the node reflects on the action by evolving its identity.
3.  **Detecting Instability**: It analyzes the change in its own identity and the timing of actions to detect patterns of instability, such as low-entropy states or rapid, repetitive requests, which could indicate a coordinated attack.

This mechanism allows the node to reject potentially harmful actions at a fundamental level, providing a robust defense against sophisticated network attacks. It doesn't protect the system, it becomes the system.

## Infinite Strata Node Mining (ISNM) ☁️

**Infinite Strata Node Mining (ISNM)** is an optional, feature-gated add-on designed to enhance network security and decentralization by incentivizing sustained, resource-committed nodes. It operates on the principle of **Proof-of-Sustained-Cloud-Presence (PoSCP)**, which rewards validators for maintaining long-running nodes with consistent CPU and memory usage, characteristic of dedicated cloud infrastructure.

* **Cloudbound Memory Decay (CMDO):** Nodes that fail to meet minimum resource usage thresholds experience a gradual decay in their potential rewards, disincentivizing ephemeral or under-provisioned validators.
* **SAGA Integration:** The PoSCP score directly influences the SAGA-calculated block reward, creating a powerful economic incentive for validators to contribute robust, long-term infrastructure to the network.
* **Enhanced Security:** By favoring dedicated nodes, ISNM helps mitigate transient, low-effort Sybil attacks and strengthens the overall stability of the validator set.

## SAGA (Sentient Autonomous Governance Algorithm) 🧠

At the core of Qanto’s intelligence lies **SAGA**, an AI-powered governance engine that orchestrates on-chain reputation, adaptive tokenomics, and self-regulating policy. As the network’s cognitive layer, SAGA ensures sustainability, resilience, and long-term systemic integrity.

* **Cognitive Analytics Engine**:  
The SAGA AI concept can be positioned as the world's first decentralized, on-chain implementation of the principles outlined in the academic paper, "SAGA: A Security Architecture for Governing AI Agentic Systems". The academic framework proposes a system for user oversight of autonomous AI agents, mediated by a central "Provider" that manages identity, access control policies, and secure communication. 

SAGA continuously evaluates validator behavior by analyzing blocks through a multidimensional heuristic framework—validity, network contribution, time consistency, and metadata fidelity. It currently operates on a decision tree model, serving as a placeholder for future deep learning models. This predictive engine identifies anomalies such as fee spamming, replay attempts, and network manipulation with temporal sensitivity.

* **Saga Credit Score (SCS)**:  
Each participant is assigned a dynamic **Saga Credit Score**—a composite of AI-derived trust scores, long-term karma (contribution weight), and staked assets. A higher SCS leads to greater governance weight and reduced block mining difficulty, thus incentivizing trustworthy behavior and improving network energy efficiency through **Proof-of-Sentiency (PoSe)**.

* **Eco-Sentiency via PoCO**:  
SAGA integrates **Proof-of-Carbon-Offset (PoCO)** to embed environmental accountability into consensus. Validator behavior and smart contracts are analyzed for carbon efficiency, influencing staking incentives and difficulty calibration. This allows Qanto to scale sustainably with active carbon-consciousness embedded in its consensus engine.

* **Saga Guidance System**:  
Developers and users interact with SAGA through the **Saga Assistant API**, which employs a Natural Language Understanding (NLU) pipeline to extract user intent and relevant parameters. This assistant delivers real-time, context-aware responses related to staking, economic parameters, validator roles, and governance status.

* **Autonomous Governance Framework**:  
SAGA proactively recommends changes to network parameters based on system-wide analytics. It adjusts base fees during congestion, proposes voting threshold changes to enhance inclusivity, and dynamically modifies validator staking requirements to support decentralization. By bridging AI decision-making with human oversight, SAGA transforms governance into a collaborative, adaptive process.


## **Core Architectural Tenets**

* **HyperDAG Ledger:** A Directed Acyclic Graph structure that allows for parallel block processing, high throughput, and near-instant finality.
* **Hybrid Consensus (PoW + PoS + PoSe):** Qanto utilizes a unique, multi-layered consensus model:
    * **Proof-of-Work (PoW):** A lightweight PoW mechanism is used to secure the network against spam and provide a basic sybil resistance layer. Miners compete to solve a computational puzzle to propose a new block.
    * **Proof-of-Stake (PoS):** Validator nodes must stake tokens to participate in consensus. The right to validate blocks is selected based on a stake-weighted algorithm, securing the network economically. Malicious behavior can result in a validator's stake being "slashed".
    * **Proof-of-Stake-Evolution (PoSe):** This is the top-level consensus layer, managed by the **SAGA** AI pallet. SAGA continuously analyzes the behavior and contributions of all validators, calculating a dynamic "Saga Credit Score" (SCS). This score directly influences a validator's block rewards and governance power. PoSe ensures that the most honest, reliable, and beneficial nodes are rewarded the most, promoting long-term network health and evolution beyond simple stake-based metrics.
* **SAGA (Sentient Autonomous Governance Algorithm):** A fully-integrated AI pallet that functions as the network's decentralized brain. It manages economic parameters, node reputation, and adaptive security protocols in real-time.
* **X-PHYRUS™ Protocol Stack:** A military-grade pre-boot and runtime security suite that performs integrity checks and activates advanced operational protocols.
* **ΩMEGA (TIA/RDS) Protocol:** A low-level identity and reflex protocol that provides a final layer of defense against unstable or dangerous system state transitions.
* **Heterogeneous Architecture**: Natively supports both DAG-based shards and linear PoW/PoS chains within one interoperable ecosystem.  
* **Dynamic Sharding**: The network autonomously adjusts the number of active DAG shards based on real-time transactional load, ensuring scalability.  
* **Post-Quantum Security**: Implements a lattice-based signature scheme (modeled after NIST standard CRYSTALS-Dilithium) for all validator attestations, ensuring long-term security.  
* **On-Chain Governance**: A decentralized, stake-weighted governance mechanism allows for protocol upgrades and parameter changes without contentious hard forks.  
* **Advanced Cryptography**: Includes specifications for Zero-Knowledge Proofs (zk-SNARKs) and Homomorphic Encryption for future privacy-preserving features.  
* **Advanced Security**: Features a novel on-chain Intrusion Detection System (IDS) that economically penalizes validators for anomalous behavior.

## **Project Structure**

The Qanto repository is a Cargo workspace containing several key components:

* **/src**: The main library crate containing the core logic.  
* `src/main.rs`: Entry point and CLI command parsing.
* `src/node.rs`: Main node orchestration, managing all services.
* `src/config.rs`: Configuration loading and validation.
* `src/hyperdag.rs`: The core DAG ledger implementation.
* `src/p2p.rs`: The libp2p-based peer-to-peer networking layer.
* `src/miner.rs`: Proof-of-Work puzzle solving logic.
* `src/transaction.rs`: Transaction creation and validation logic.
* `src/wallet.rs`: Encrypted wallet management.
* `src/saga.rs`: The AI governance and adaptive security pallet.
* `src/omega.rs`: The system's core identity and reflex protocol.
* `src/x_phyrus.rs`: The pre-boot security and diagnostics suite.
* `src/zk.rs`: (Feature-gated) ZK-proof circuit definitions.
* `src/infinite_strata_node.rs`: Proof-of-Sustained-Cloud-Presence (PoSCP) and cloud-adaptive mining logic.
* **/src/bin**: Executable crates for the node (start\_node.rs) and wallet (hyperwallet.rs).  
* **/docs**: Project documentation, including the whitepaper and launch plans.  
* **config.toml.example**: An example configuration file for the node.

## **Developer & Research Materia**

* **Formal Specification (Whitepaper)**: [docs//whitepaper/Qanto-whitepaper.md](./docs/whitepaper/Qanto-whitepaper.md)
* **System Architecture Overview**: [Architecture.md](./Architecture.md)
* **API Documentation**: A complete specification for the public RPC and REST Application Programming Interfaces is slated for publication prior to the mainnet launch.
* **Command-Line Interface (CLI) Wallet**: The `hyperwallet` executable furnishes a command-line interface for all requisite wallet and cryptographic key management operations.

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
    The `hyperwallet` utility is provided for the creation of a new keypair. Upon execution, the operator will be prompted to supply a secure passphrase for the encryption of the resultant wallet file, `wallet.key`.
    ```bash
    cargo run --release --bin hyperwallet -- generate --output wallet.key
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
    cargo run --release --features infinite-strata --bin Qanto -- start --config config.toml --wallet wallet.key
    ```
    The operator will be prompted to supply the wallet passphrase, after which the node will initialize its services and commence network operations.
4.  **Demonstrating the ΛΣ-ΩMEGA Module**

To see a simulation of the conscious security layer, run the `omega_test` binary:

```bash
cargo run --bin omega_test
```
## **Testnet Participation**

For details on joining the public testnet, including hardware requirements, incentive programs, and bootnode addresses, please refer to the [**Testnet Launch Plan**](./docs/testnet-plan.md).

## **Security**

The security of the network is our highest priority. We have a formal plan for a comprehensive third-party audit. For more details, please see our [**Security Audit Plan**](./docs/security-audit-plan.md).

## **Contribution Protocol**

We welcome contributions from the community\! This project thrives on collaboration and outside feedback. Please read our [**Contribution Guidelines**](./CONTRIBUTING.md) to get started.  
All participants are expected to follow our [**Code of Conduct**](./CODE_OF_CONDUCT.md).

## **License**

This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.

