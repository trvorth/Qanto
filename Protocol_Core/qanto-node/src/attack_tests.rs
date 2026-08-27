//! --- Consensus Safety Attack Tests ---
//!
//! This module demonstrates CRITICAL vulnerabilities in the Qanto consensus layer
//! as identified during the ANALYZE phase of the A-Z engineering audit.
//!
//! Each test demonstrates a concrete attack vector that violates safety or liveness.
//! These tests MUST FAIL (demonstrating the vulnerability exists) before fixes are applied,
//! and MUST PASS (vulnerability fixed) after remediation.
//!
//! Vulnerabilities demonstrated:
//! 1. Single-parent DAG safety violation under partial synchrony (CRITICAL)
//! 2. Liveness failure with fixed difficulty under hashrate drop (CRITICAL)
//! 3. Finality absence - reorg any depth, no checkpointing (CRITICAL)
//! 4. EVM raw tx path bypassing PQ verification (HIGH)
//! 5. BridgeClaim with forged SignatureProof using user-provided validators (HIGH)
//! 6. RPC/WS unauthenticated attacks: faucet drain, GraphQL DoS, gRPC submit_tx (HIGH)
//! 7. P2P eclipse attack on fixed gossipsub topics (MEDIUM)
//! 8. Miner reservation griefing - reserve all mempool capacity (MEDIUM)
//! 9. Consensus lock serialization bottleneck under load (MEDIUM)
//! 10. O(n) blocks map clone performance at 10k+ blocks (MEDIUM)
//! 11. SAGA f64 non-determinism in reward calculation (LOW)
//! 12. ΩMEGA threat level manipulation via global atomic (LOW)

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::{Transaction, TransactionKind};
use crate::types::UTXO;
use ahash::AHashMap as HashMap;
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
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

// Import the attack modules from the crate root (defined in lib.rs under #[cfg(test)])
#[cfg(test)]
use crate::attack_10_blocks_map_performance;
#[cfg(test)]
use crate::attack_11_saga_f64_nondeterminism;
#[cfg(test)]
use crate::attack_12_omega_threat_manipulation;
#[cfg(test)]
use crate::attack_1_single_parent_dag_safety_violation;
#[cfg(test)]
use crate::attack_2_liveness_failure_fixed_difficulty;
#[cfg(test)]
use crate::attack_3_finality_absence_reorg_any_depth;
#[cfg(test)]
use crate::attack_4_evm_raw_tx_bypass_pq;
#[cfg(test)]
use crate::attack_5_bridge_claim_forged_signature;
#[cfg(test)]
use crate::attack_6_rpc_ws_unauthenticated;
#[cfg(test)]
use crate::attack_7_p2p_eclipse_fixed_topics;
#[cfg(test)]
use crate::attack_8_miner_reservation_griefing;
#[cfg(test)]
use crate::attack_9_consensus_lock_bottleneck;

// Re-export the attack test modules
#[cfg(test)]
pub use attack_10_blocks_map_performance::*;
#[cfg(test)]
pub use attack_11_saga_f64_nondeterminism::*;
#[cfg(test)]
pub use attack_12_omega_threat_manipulation::*;
#[cfg(test)]
pub use attack_1_single_parent_dag_safety_violation::*;
#[cfg(test)]
pub use attack_2_liveness_failure_fixed_difficulty::*;
#[cfg(test)]
pub use attack_3_finality_absence_reorg_any_depth::*;
#[cfg(test)]
pub use attack_4_evm_raw_tx_bypass_pq::*;
#[cfg(test)]
pub use attack_5_bridge_claim_forged_signature::*;
#[cfg(test)]
pub use attack_6_rpc_ws_unauthenticated::*;
#[cfg(test)]
pub use attack_7_p2p_eclipse_fixed_topics::*;
#[cfg(test)]
pub use attack_8_miner_reservation_griefing::*;
#[cfg(test)]
pub use attack_9_consensus_lock_bottleneck::*;
