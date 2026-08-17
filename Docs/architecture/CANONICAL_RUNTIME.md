# Canonical Runtime

Based on Phase 2 codebase mapping, the canonical runtime components for production are as follows.

## CLI & Entrypoint
- `qanto node start` (currently `start_node.rs` which configures `node.rs`).

## Consensus & State
- **DAG:** `qantodag.rs` (Primary implementation).
- **Consensus Rules:** `consensus.rs` (Primary deterministic rules).

## Networking
- **P2P Transport:** `p2p.rs` (Standard libp2p with Kademlia/Gossipsub).

## Transaction Processing
- **Mempool:** `mempool.rs`.
- **Transactions:** `transaction.rs`.

## Identity and Cryptography
- **Node Identity (P2P):** Needs to be split out properly (Phase 3 task).
- **Wallets:** `wallet.rs` and `qantowallet.rs` (Needs to be untangled from node startup).

## Mining (Disabled by Default)
- **Miner:** `miner.rs`.
