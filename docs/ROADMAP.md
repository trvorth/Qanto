# Qanto Project Roadmap

This document outlines the strategic phases for the development, launch, and expansion of the Qanto network.

---

## **Phase 1: FOUNDATION (Development & Testing)**

**Goal**: Build a robust, secure, and functional blockchain.
**Status**: In Progress (Approximately 85% complete)

### **Key Milestones**:

-   **✅ Core Runtime Developed**: The console logs confirm that a custom runtime (hyperdag) is compiled and functional on a local machine. It can initialize nodes, manage a DAG structure, and start a mining process. The successful run demonstrates a working event loop and inter-module communication.
-   **✅ Genesis Block Configuration Created**: The node successfully creates genesis blocks for its chains upon startup, as evidenced by the successful mining of the first few blocks which have a parent hash pointing back to the initial state. The configuration for this is handled within hyperdag.rs and node.rs.
-   **🟡 Whitepaper & Tokenomics Finalized**: The code (saga.rs) contains concrete, governable tokenomic parameters (e.g., base_reward, tiered transaction fees, etc.). SAGA's rule-based system acts as a "live" whitepaper for economic policy. However, a formal, human-readable whitepaper document that explains this complex system to a non-technical audience is a critical missing piece for community building and investor relations. The foundation is there, but it needs to be documented externally.
-   **✅ Local Testnet Operational**: The logs show the node running successfully in a simulated single-node mode. The p2p logic (p2p.rs) and configuration for multiple peers exist, indicating that a local multi-node testnet is likely functional. However, the provided logs only show a single instance, so we can't fully confirm distributed consensus, block propagation, and state synchronization between multiple local nodes.
-   **⬜ Successful Public Testnet Launch**: The node currently runs in a local environment. A public testnet, requiring deployment to cloud servers with public IP addresses, seed nodes, and community participation, has not yet been launched. This is the most critical next step in this phase.
-   **⬜ Security Audit**: This has not been started. A third-party audit is essential to identify vulnerabilities, especially given the complexity of the SAGA AI and ISNM modules, before any mainnet launch or significant value is held on the network.

---

## **Phase 2: IGNITION (Mainnet Launch)**

**Goal**: Bring the network to life and create the native coin.
**Status**: Not Started

### **Key Milestones**:

-   **⬜ Infrastructure Deployed for Public Access**: While the code has the necessary components (p2p.rs), public-facing infrastructure like seed nodes, bootnodes, and persistent RPC endpoints are not yet deployed. This is a prerequisite for a public testnet and mainnet.
-   **⬜ MAINNET GENESIS EVENT**: The mainnet has not been launched.
-   **⬜ COIN IS LIVE & Initial Distribution Complete**: The native coin ($HCN) only exists on the local testnet. It is not yet a publicly tradable asset.
-   **⬜ Public RPC Endpoints Operational**: No public endpoints are available for users or dApps to interact with the network.
-   **⬜ Official Block Explorer is Live**: A block explorer is crucial for transparency and user experience. It has not yet been developed.

---

## **Phase 3: EXPANSION (Ecosystem & Decentralization)**

**Goal**: Prove the project is viable, decentralized, and has an active community.
**Status**: Not Started

### **Key Milestones**:

-   **⬜ Official Wallets Integration is Live**: No user-facing wallets (GUI, browser extension, or mobile) have been developed or integrated.
-   **⬜ On-chain Governance is Active**: The foundational logic for on-chain governance exists within SAGA (saga.rs), with rules for proposals and council actions. However, without a public network and active participants, this governance is not yet active.
-   **🟡 Core Code Supports Independent Validators**: The configuration files (config.toml) and P2P logic are designed to support a list of peers and validators. This strongly indicates support for independent nodes, but it can only be proven on a public network where community members successfully connect and participate in consensus.
-   **⬜ First Independent, Community-run Validator Nodes Join**: This milestone is dependent on the public testnet launch.
-   **⬜ Developer Grant Program Launched**: No ecosystem development programs are in place.
-   **⬜ First dApp or Project Commits to Building on the Chain**: No external projects are building on Qanto yet.

---

## **Phase 4: MARKET ENTRY (Liquidity & Listing)**

**Goal**: Achieve tradable volume and listing criteria.
**Status**: Not Started

All milestones in this phase are not applicable yet, as they depend on the successful completion of Phases 1, 2, and 3.

---

### **Strategic Outlook & Next Steps**:

1.  **Finalize Phase 1**: The immediate priority is to move from a local simulation to a Public Testnet. This involves deploying at least one stable seed node, providing clear instructions for community members to join, and monitoring network stability and performance.
2.  **Documentation**: Concurrently, develop a comprehensive Whitepaper and developer documentation. Explain the unique value proposition of the SAGA AI, the ISNM/PoSe model, and the tiered fee structure. This is non-negotiable for attracting a community and validators.
3.  **Community Building**: Start building a community around the project now. Use the whitepaper and the promise of the upcoming public testnet to generate interest on platforms like Discord, Twitter, and Telegram.
4.  **Security Audit**: Once the testnet is stable, engage a reputable security firm to audit the codebase. This will be a major catalyst for building trust.
