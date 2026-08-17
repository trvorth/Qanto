# Duplication Map

The codebase contains numerous experimental and overlapping implementations. Below is a map of responsibilities and their various implementations.

## Mempool
- `qanto-node/src/mempool.rs` (Likely the canonical implementation)
- `qanto-node/src/elite_mempool.rs` (Experimental/Alternative)
- `qanto-core/src/dag_aware_mempool.rs` (Core level alternative)

## Block Production & Mining
- `qanto-node/src/block_producer.rs`
- `qanto-node/src/decoupled_producer.rs`
- `qanto-node/src/optimized_decoupled_producer.rs`
- `qanto-node/src/miner.rs`
- `qanto-node/src/deterministic_mining.rs`
- `qanto-node/src/adaptive_mining.rs`

## Consensus
- `qanto-node/src/consensus.rs`
- `qanto-node/src/cosmic_consensus.rs`
- `qanto-node/src/neural_consensus.rs`

## DAG Structure
- `qanto-node/src/qantodag.rs`
- `qanto-node/src/optimized_qdag.rs`
- `qanto-node/src/qantodag_testnet.rs`

## P2P Networking
- `qanto-node/src/p2p.rs`
- `qanto-node/src/p2p_mesh.rs`

## Wallets
- `qanto-node/src/wallet.rs`
- `qanto-node/src/qantowallet.rs`
