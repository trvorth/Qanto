# Qanto Codebase Map

## Workspace Members
1. **`Protocol_Core/qanto-node`**
   - **Role:** Main node executable and runtime.
   - **Binaries:** `qanto`, `start_node`, `gen_wallet` (implied from src/bin).
   - **Key Modules:** `node.rs`, `p2p.rs`, `mempool.rs`, `transaction.rs`, `wallet.rs`, `qantodag.rs`, `config.rs`.
   
2. **`Protocol_Core/qanto-core`**
   - **Role:** Core primitives and shared libraries.
   - **Key Modules:** `qanto_native_crypto.rs`, `qanto_net.rs`, `qanto_storage.rs`, `qanto_p2p.rs`, `dag_aware_mempool.rs`.

3. **`Protocol_Core/qanto-rpc`**
   - **Role:** RPC / API definitions.

4. **`Protocol_Core/myblockchain`**
   - **Role:** Legacy / specific blockchain logic, contains `qanhash.cu`.

5. **`Protocol_Core/qanto-zk-sdk`**
   - **Role:** Zero-Knowledge proofs SDK.

## Components & Boundaries
- **Consensus & DAG:** Handled in `qanto-node/src/qantodag.rs` and `consensus.rs`.
- **P2P:** Implemented in `qanto-node/src/p2p.rs` (libp2p).
- **Wallet/Keys:** `qanto-node/src/wallet.rs`, `qantowallet.rs`, `password_utils.rs`.
- **Configuration:** `qanto-node/src/config.rs`.
