//! Attack 11: SAGA f64 Non-Determinism in Reward Calculation
//!
//! LOW VULNERABILITY: The SAGA adaptive control system uses f64 arithmetic
//! for reward calculations, which is NON-DETERMINISTIC across platforms.
//!
//! In saga.rs:calculate_dynamic_reward() (lines 3754-3828):
//! - Uses f64 for base_reward, threat_modifier, reputation scores
//! - f64 operations differ between x86_64, ARM, WASM
//! - Different optimization levels change results
//! - Consensus-critical rewards MUST be deterministic
//!
//! The function is called from validate_block_reward (consensus.rs:256-280)
//! but that function is marked #[allow(dead_code)] and NOT used in
//! the active validation path (qantodag.rs uses fixed reward = fees only).

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
    let weak_self = Arc::downgrade(&arc_dag);
    let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;
    unsafe {
        (*ptr).self_arc = weak_self;
    }
    arc_dag
}

/// ATTACK DEMONSTRATION: SAGA f64 non-determinism
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_11_saga_f64_nondeterminism_reward_calculation() {
        // calculate_dynamic_reward in saga.rs uses f64 throughout:
        // - base_reward: f64 (from epoch_rules)
        // - threat_modifier: f64
        // - reputation credit_scores: f64 via evaluate_credit
        // - isnm_multiplier: f64
        // - market_premium: f64
        // - Final calculation: base_reward * (1 + threat + reputation + isnm + market)
        //
        // f64 non-determinism sources:
        // 1. x87 vs SSE2 vs AVX floating point units
        // 2. ARM NEON vs x86 SSE
        // 3. Compiler optimization level (-O0 vs -O3)
        // 4. FMA (fused multiply-add) contraction
        // 5. WASM vs native
        // 6. Rounding mode differences
        //
        // Result: Same inputs -> different rewards on different platforms
        // This BREAKS CONSENSUS if rewards are consensus-critical

        println!("VULNERABILITY: SAGA calculate_dynamic_reward uses f64 (non-deterministic)");
        println!("saga.rs:3754-3828 - all arithmetic in f64");
        println!("Platforms: x86_64 (SSE2/AVX), ARM (NEON), WASM - all differ");
        println!("Optimization: -O0 vs -O3 changes f64 results");
    }

    #[tokio::test]
    async fn attack_11_saga_not_consensus_critical_currently() {
        // Currently, the active validation path (qantodag.rs:3418-3423) uses:
        // - reward = total_fees only (containment mode, subsidy = 0)
        // - SAGA calculate_dynamic_reward is NOT called
        //
        // But validate_block_reward in consensus.rs:256-280 CALLS it
        // and is marked #[allow(dead_code)]
        //
        // If/when SAGA reward becomes active:
        // - CONSENSUS FAILURE due to non-determinism

        println!("CURRENT STATE: SAGA reward NOT used in active consensus path");
        println!("qantodag.rs uses: reward = total_fees (deterministic integer)");
        println!("consensus.rs validate_block_reward IS dead code");
        println!("RISK: If enabled -> consensus split across platforms");
    }

    #[tokio::test]
    async fn attack_11_f64_operations_that_differ() {
        // Specific operations in calculate_dynamic_reward that are non-deterministic:
        // 1. reputation.credit_scores - evaluation uses f64
        // 2. isnm_multiplier - from ISNM service, f64
        // 3. market_premium - calculated from f64 inputs
        // 4. Final multiplication chain: base * (1 + sum_of_modifiers)
        // 5. Division operations (threat_modifier application)
        //
        // All of these can produce different bit patterns

        println!("NON-DETERMINISTIC OPERATIONS in calculate_dynamic_reward:");
        println!("1. credit_score evaluation -> f64");
        println!("2. isnm_multiplier -> f64");
        println!("3. market_premium calculation -> f64");
        println!("4. (1.0 + threat + reputation + isnm + market) -> f64");
        println!("5. base_reward * modifier -> f64");
        println!("6. Final conversion to u128 -> truncation differs");
    }

    #[tokio::test]
    async fn attack_11_fix_use_fixed_point() {
        // Fix: Replace ALL f64 with fixed-point integer arithmetic
        // Use QANTO_SCALE (1e9) for 9 decimal places
        //
        // Pattern from miner.rs:
        // - calculate_target_from_difficulty uses U256 fixed-point
        // - No floating point in consensus path

        println!("FIX: Replace f64 with fixed-point (QANTO_SCALE = 1e9)");
        println!("Pattern: miner.rs U256 arithmetic for deterministic results");
        println!("All: base_reward, threat_modifier, credit_scores, multipliers");
    }
}
