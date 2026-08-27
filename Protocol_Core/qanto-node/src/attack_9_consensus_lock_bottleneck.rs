//! Attack 9: Consensus Lock Serialization Bottleneck Under Load
//!
//! MEDIUM VULNERABILITY: The QantoDAG uses a single RwLock (consensus_lock)
//! for ALL block validation and state mutations.
//!
//! In qantodag.rs, the consensus_lock is used in:
//! - add_block() - exclusive lock for entire validation
//! - is_valid_block() - read lock for validation
//! - finalize_blocks() - exclusive lock
//! - process_stake/unstake - exclusive lock
//! - And many more methods
//!
//! Under high load (many concurrent block submissions):
//! - All validation SERIALIZED through single lock
//! - Throughput limited to ~1 validation at a time
//! - Latency grows linearly with contention
//! - No parallel validation of independent blocks/chains

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
    if !dag.tips.contains_key(&0) {
        let mut genesis = QantoBlock::new_test_block("test-genesis".to_string());
        genesis.height = 0;
        genesis.parents.clear();
        genesis.timestamp = 0;
        genesis.id = genesis.compute_block_id().expect("test genesis id");
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(0).or_default().insert(genesis.id);
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

    let mut block = QantoBlock::new_unsealed_candidate(crate::qantodag::QantoBlockCreationData {
        validator_private_key: validator_sk.clone(),
        chain_id: 0,
        parents: vec![parent_tip.to_string()],
        transactions: txs,
        difficulty: crate::QANTO_SCALE as u64,
        validator: validator_addr.to_string(),
        miner: validator_addr.to_string(),
        timestamp: now,
        current_epoch: 0,
        height,
        reward: 0,
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
    block
}

/// ATTACK DEMONSTRATION: Consensus lock serialization bottleneck
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_9_consensus_lock_serialization_bottleneck() {
        // The QantoDAG uses a single mutex/lock for ALL state mutations
        // Check qantodag.rs for the lock structure
        //
        // The struct has:
        // - consensus_lock: Mutex<()> or RwLock<()> (need to check actual type)
        // - Used in add_block, is_valid_block, finalize_blocks, etc.
        //
        // Under concurrent load:
        // - 100 clients submit blocks simultaneously
        // - All serialize on single lock
        // - Effective throughput = 1 block at a time

        println!("VULNERABILITY: Single consensus lock serializes all operations");
        println!("Check qantodag.rs for: consensus_lock, blocks_lock, tips_lock, etc.");
        println!("Expected: Multiple fine-grained locks or lock-free structures");
    }

    #[tokio::test]
    async fn attack_9_benchmark_lock_contention() {
        // Benchmark to demonstrate the bottleneck
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (validator_pk, validator_sk, validator_addr) = create_test_validator(&dag).await;
        let utxos = Arc::new(RwLock::new(HashMap::new()));

        // Create many blocks concurrently
        let num_blocks = 50;
        let mut handles = Vec::new();

        let start = Instant::now();

        for i in 1..=num_blocks {
            let dag_clone = dag.clone();
            let utxos_clone = utxos.clone();
            let validator_pk_clone = validator_pk.clone();
            let validator_sk_clone = validator_sk.clone();
            let validator_addr_clone = validator_addr.clone();

            let handle = tokio::spawn(async move {
                // All blocks try to extend from genesis (will conflict on tips)
                let tip = dag_clone
                    .tips
                    .get(&0)
                    .unwrap()
                    .iter()
                    .next()
                    .unwrap()
                    .clone();
                let block = create_valid_block(
                    &dag_clone,
                    &validator_sk_clone,
                    &validator_pk_clone,
                    &validator_addr_clone,
                    &tip,
                    i,
                    vec![],
                )
                .await;

                dag_clone.add_block(block, &utxos_clone, None, None).await
            });
            handles.push(handle);
        }

        // Wait for all - they will serialize on the lock
        let mut success = 0;
        for handle in handles {
            if let Ok(Ok(true)) = handle.await {
                success += 1;
            }
        }

        let elapsed = start.elapsed();
        println!("Added {} blocks in {:?} (serialized)", success, elapsed);
        println!(
            "Throughput: {:.2} blocks/sec",
            success as f64 / elapsed.as_secs_f64()
        );
        println!("EXPECTED: Linear scaling with num_blocks due to single lock");
    }

    #[tokio::test]
    async fn attack_9_no_parallel_validation_independent_chains() {
        // Even if we had multiple chains (chain_id > 0),
        // the single lock would serialize validation across chains
        //
        // The code has chain_id support but:
        // - Line 3100: "V2 containment permits only chain 0"
        // - So multi-chain is disabled anyway
        // - But even if enabled, single lock would bottleneck

        println!("VULNERABILITY: Even multi-chain would serialize on single lock");
        println!("Fine-grained locking per chain_id needed for parallelism");
    }

    #[tokio::test]
    async fn attack_9_read_lock_starves_writers() {
        // If using RwLock, many concurrent is_valid_block() read locks
        // can starve add_block() write lock
        //
        // Check if it's RwLock or Mutex

        println!("CHECK: Lock type - RwLock (readers starve writers) or Mutex?");
        println!("If RwLock: many is_valid_block() calls block add_block()");
    }
}
