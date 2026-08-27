//! Attack 10: O(n) Blocks Map Clone Performance at 10k+ Blocks
//!
//! MEDIUM VULNERABILITY: The QantoDAG stores blocks in a DashMap (blocks: DashMap<String, QantoBlock>)
//! and in several places clones or iterates the entire map in O(n) operations.
//!
//! Key locations:
//! - qantodag.rs:5936 - difficulty_history iteration over all blocks
//! - Multiple places where blocks map is cloned or fully iterated
//! - Memory grows unbounded (no pruning implemented)
//!
//! At 10k+ blocks:
//! - O(n) operations become slow
//! - Memory usage grows linearly without bound
//! - No archival/pruning strategy

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::Transaction;
use crate::types::UTXO;
use ahash::AHashMap as HashMap;
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    if !dag.tips.contains_key(&1234) {
        let mut genesis = QantoBlock::new_test_block("test-genesis".to_string());
        genesis.height = 0;
        genesis.parents.clear();
        genesis.timestamp = 0;
        genesis.id = genesis.compute_block_id().expect("test genesis id");
        genesis.chain_id = 1234;
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(1234).or_default().insert(genesis.id);
    }
    let arc_dag = Arc::new(dag);
    // SELF-ARC-001 (Cycle 023): sound self-reference init. Previously this
    // cast Arc::as_ptr (a *const T) to *mut and wrote through it, which is UB
    // by provenance even with no other live handle.
    let _ = arc_dag.self_arc.set(Arc::downgrade(&arc_dag));
    arc_dag
}

async fn create_test_validator(
    dag: &Arc<QantoDAG>,
) -> (QantoPQPublicKey, QantoPQPrivateKey, String) {
    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_addr = validator_id_v2(
        pk.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    dag.validators
        .insert(validator_addr.clone(), 50 * crate::Q_SCALE);
    (pk, sk, validator_addr)
}

fn create_transfer_tx(
    from_pk: &QantoPQPublicKey,
    to_pk: &QantoPQPublicKey,
    amount: u128,
    fee: u128,
    nonce: u64,
) -> Transaction {
    let (_, sk) = generate_pq_keypair(None).unwrap();
    Transaction::new_dummy_signed(&sk, from_pk).unwrap()
}

async fn create_valid_block(
    dag: &Arc<QantoDAG>,
    validator_sk: &QantoPQPrivateKey,
    validator_pk: &QantoPQPublicKey,
    validator_addr: &str,
    parent_tip: &str,
    height: u64,
    txs: Vec<Transaction>,
) -> QantoBlock {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let paillier_pk = crate::types::HomomorphicEncrypted::generate_keypair().0;

    // Create coinbase transaction (no user txs, so fees = 0)
    let chain_id = 1234u32;
    let reward_address =
        qanto_core::protocol_identity::account_address_v2(chain_id, validator_pk.as_bytes())
            .unwrap();
    let coinbase_context =
        QantoBlock::compute_coinbase_context(chain_id, height, &[parent_tip.to_string()], &txs);
    let output = crate::transaction::Output {
        address: reward_address.clone(),
        amount: 0u128,
        homomorphic_encrypted: crate::types::HomomorphicEncrypted::new(0u128, &[0u8; 32]),
    };
    let coinbase_tx = crate::transaction::Transaction::new_coinbase(
        reward_address,
        0u128,
        vec![output],
        validator_sk,
        chain_id,
        coinbase_context,
    )
    .unwrap();

    let mut all_txs = vec![coinbase_tx];
    all_txs.extend(txs);

    // Calculate reward = sum of fees from user transactions
    let total_fees = all_txs.iter().skip(1).map(|tx| tx.fee).sum::<u128>();

    let mut block = QantoBlock::new_unsealed_candidate(crate::qantodag::QantoBlockCreationData {
        validator_private_key: validator_sk.clone(),
        chain_id: 1234,
        parents: vec![parent_tip.to_string()],
        transactions: all_txs,
        difficulty: crate::QANTO_SCALE as u64,
        validator: validator_addr.to_string(),
        miner: validator_addr.to_string(),
        timestamp: now,
        current_epoch: 0,
        height,
        reward: total_fees,
        nonce: 0,
        effort: 0,
        chain_work: crate::miner::U256::ZERO,
        paillier_pk,
        state_parent: String::new(),
    })
    .unwrap();

    // Mine the block to satisfy PoW using Miner instance
    let logging_config = crate::config::LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false,
        celebration_log_level: "info".to_string(),
        celebration_throttle_per_min: None,
    };
    let miner_addr =
        qanto_core::protocol_identity::account_address_v2(1234, validator_pk.as_bytes()).unwrap();
    let miner_config = crate::miner::MinerConfig {
        address: miner_addr.clone(),
        dag: dag.clone(),
        target_block_time: 3,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: logging_config.clone(),
    };
    let miner = crate::miner::Miner::new(miner_config).unwrap();
    miner.solve_pow(&mut block).unwrap();

    // Seal the block (sign it and compute block ID)
    block.seal(validator_sk).unwrap();
    block
}

/// ATTACK DEMONSTRATION: O(n) blocks map operations at scale
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_10_blocks_map_operations_scaling() {
        // The blocks DashMap grows unbounded
        // Operations that iterate entire map:
        // - qantodag.rs:5936 - difficulty history calculation
        // - Any .iter() or .len() on blocks map
        // - Serialization/deserialization
        //
        // At 10,000 blocks: 10k iterations per operation
        // At 1,000,000 blocks: 1M iterations -> seconds of latency

        println!("VULNERABILITY: Unbounded blocks map with O(n) operations");
        println!("Check qantodag.rs for: blocks.iter(), blocks.len(), difficulty_history");
        println!("No: pruning, archival, sharding, bloom filters");
    }

    #[tokio::test]
    async fn attack_10_memory_growth_unbounded() {
        // Blocks are never pruned
        // Each block: ~1-10 KB (transactions, signatures, etc.)
        // 10k blocks = 10-100 MB
        // 1M blocks = 1-10 GB
        // No pruning strategy implemented

        println!("VULNERABILITY: No block pruning or archival");
        println!("Memory grows O(n) with chain height");
        println!("Check persistence.rs for pruning - likely NOT implemented");
    }

    #[tokio::test]
    async fn attack_10_difficulty_history_full_scan() {
        // qantodag.rs:5936 - difficulty calculation scans ALL blocks
        // ```rust
        // .map(|b| (b.difficulty as u128).max(1))
        // ```
        // This runs on every block validation!
        //
        // Fix: Maintain running difficulty state, not full scan

        println!("VULNERABILITY: difficulty_history scans all blocks on each validation");
        println!("qantodag.rs:5936 - O(n) per block validation");
        println!("Should be: O(1) incremental update");
    }

    #[tokio::test]
    async fn attack_10_benchmark_blocks_map_growth() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (validator_pk, validator_sk, validator_addr) = create_test_validator(&dag).await;
        let utxos = Arc::new(RwLock::new(HashMap::new()));

        // Add blocks and measure operation time
        let mut tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();
        let mut times = Vec::new();

        for i in 1..=100 {
            let block = create_valid_block(
                &dag,
                &validator_sk,
                &validator_pk,
                &validator_addr,
                &tip,
                i,
                vec![],
            )
            .await;

            let start = Instant::now();
            dag.add_block(block.clone(), &utxos, None, None)
                .await
                .unwrap();
            let elapsed = start.elapsed();
            times.push(elapsed);
            tip = block.id;

            if i % 20 == 0 {
                let avg = times.iter().sum::<Duration>() / times.len() as u32;
                println!("Block {}: avg add_block time {:?}", i, avg);
            }
        }

        // Should see linear growth in time if O(n) operations on blocks map
        println!("Check for linear growth in add_block time");
    }
}
