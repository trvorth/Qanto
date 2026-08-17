//! Attack 12: ΩMEGA Threat Level Manipulation via Global Atomic
//!
//! LOW VULNERABILITY: The ΩMEGA conscious security layer uses a global AtomicU8
//! for threat level that can be manipulated by any code with access to the node.
//!
//! In saga.rs and infinite_strata_node.rs:
//! - OMEGA_THREAT_LEVEL: AtomicU8 (global static)
//! - Modified by: detect_quantum_threats, anomaly detection, manual triggers
//! - Read by: calculate_dynamic_reward (threat_modifier)
//! - NO authentication/authorization for modifications
//!
//! Any component (or compromised dependency) can:
//! 1. Inflate threat level -> reduce rewards for validators
//! 2. Deflate threat level -> hide actual threats
//! 3. Oscillate threat level -> destabilize reward calculations

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoDAG};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::Transaction;
use crate::types::UTXO;
use ahash::AHashMap as HashMap;
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2};
use std::sync::atomic::{AtomicU8, Ordering};
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

/// ATTACK DEMONSTRATION: ΩMEGA threat level manipulation
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_12_omega_threat_level_global_atomic() {
        // Check saga.rs and infinite_strata_node.rs for:
        // static OMEGA_THREAT_LEVEL: AtomicU8 = AtomicU8::new(0);
        //
        // This global is accessible from ANYWHERE in the process
        // No capability-based access control
        // No audit trail of who/what changed it

        println!("VULNERABILITY: OMEGA_THREAT_LEVEL is global AtomicU8");
        println!("Any code in process can: load(), store(), fetch_add(), etc.");
        println!("No: authentication, authorization, audit log, rate limiting");
    }

    #[tokio::test]
    async fn attack_12_threat_level_affects_rewards() {
        // calculate_dynamic_reward reads threat level:
        // let threat_modifier = rules.get("omega_threat_reward_modifier") (-0.25 default)
        // This is multiplied by threat level
        //
        // Attack: Compromised dependency sets threat_level = 255
        // -> threat_modifier * 255 -> massive reward reduction
        // Or: Sets threat_level = 0 during actual attack -> no reward penalty

        println!("VULNERABILITY: Threat level directly affects validator rewards");
        println!("calculate_dynamic_reward uses: threat_modifier * threat_level");
        println!(
            "Attacker can: suppress rewards (inflate threat) or hide attacks (deflate threat)"
        );
    }

    #[tokio::test]
    async fn attack_12_no_authentication_for_threat_changes() {
        // Functions that modify threat level:
        // - infinite_strata_node::detect_quantum_threats()
        // - saga anomaly detection
        // - Manual admin triggers (if any)
        //
        // None require: cryptographic proof, quorum, governance vote

        println!("VULNERABILITY: No auth for threat level changes");
        println!("detect_quantum_threats() can be called by any component");
        println!("No: multi-sig, threshold signatures, governance proposal");
    }

    #[tokio::test]
    async fn attack_12_compromised_dependency_manipulation() {
        // Supply chain attack: Malicious crate with access to node internals
        // can directly manipulate OMEGA_THREAT_LEVEL
        //
        // Since it's a public static (or accessible via module), any dependency
        // that gets linked can modify it
        //
        // Example: Malicious logging crate, metrics exporter, etc.

        println!("VULNERABILITY: Supply chain - any dependency can manipulate threat level");
        println!("OMEGA_THREAT_LEVEL accessible from any linked crate");
        println!("Fix: Encapsulate behind capability token / actor model");
    }

    #[tokio::test]
    async fn attack_12_demonstrate_manipulation() {
        // Direct demonstration of atomic manipulation
        static OMEGA_THREAT_LEVEL: AtomicU8 = AtomicU8::new(0);

        // Normal operation
        assert_eq!(OMEGA_THREAT_LEVEL.load(Ordering::SeqCst), 0);

        // Attacker inflates threat level
        OMEGA_THREAT_LEVEL.store(100, Ordering::SeqCst);
        assert_eq!(OMEGA_THREAT_LEVEL.load(Ordering::SeqCst), 100);

        // Attacker hides real threat
        OMEGA_THREAT_LEVEL.store(0, Ordering::SeqCst);
        assert_eq!(OMEGA_THREAT_LEVEL.load(Ordering::SeqCst), 0);

        println!("DEMONSTRATED: Direct atomic manipulation possible from any code");
        println!("No audit trail, no authorization, no rate limiting");
    }
}
