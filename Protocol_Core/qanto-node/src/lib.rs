#![recursion_limit = "512"]
// The primary modules that define the Qanto node and its behavior.
pub mod adaptive_mining;
pub mod analytics_dashboard;
pub mod block_producer;
pub mod config;
pub mod config_validator;
pub mod consensus;
pub mod decentralization;
pub mod decoupled_producer;
pub mod deterministic_mining;
pub mod diagnostics;
pub mod elite_mempool;
pub mod emission;
pub mod formal_verifier;
pub mod gas_fee_model;
pub mod graphql_api;
pub mod graphql_server;
pub mod hame;
pub mod heritage_archive;
pub mod holo_mesh;
pub mod interoperability;
pub mod interplanetary;
pub mod kernel_boot;
pub mod keygen;
pub mod mempool;
pub mod metrics;
pub mod miner;
pub mod mining_celebration;
pub mod mining_metrics;
pub mod node;
pub mod omega;
pub mod omega_enhanced;
pub mod optimized_decoupled_producer;
pub mod optimized_qdag;
pub mod p2p;
pub mod p2p_mesh;
pub mod password_utils;
pub mod performance_monitoring;
pub mod performance_optimizations;
pub mod performance_validation;
pub mod persistence;
pub mod post_quantum_crypto;
pub mod pq_crypto;
pub mod pq_suite;
pub mod privacy;
pub mod resource_cleanup;
pub mod safe_interval;
pub mod saga_ai;
pub mod shutdown;
pub mod storage_mesh;
pub mod telemetry;
pub mod timing;

// --- Added unreferenced modules ---
pub mod a2a_material;
pub mod absolute_genesis;
pub mod absolute_singularity;
pub mod config_validation;
pub mod cosmic_consensus;
pub mod entropy_shield;
pub mod eternal_existence;
pub mod final_transcendence;
pub mod futarchy_market;
pub mod global_synthesis;
pub mod inference_mesh;
pub mod infinite_omnipresence;
pub mod mesh_finality;
pub mod mesh_optimizer;
pub mod mesh_recovery;
pub mod neural_arbitrage;
pub mod neural_consensus;
pub mod neural_link;
pub mod neural_mirror;
pub mod qantodag_testnet;
pub mod quantum_bridge;
pub mod recursive_evolution;
pub mod self_sovereign_code;
pub mod sharding;
pub mod swarm_orchestrator;
pub mod truth_engine;
pub mod ubi_credits;
pub mod zk;

pub mod derive_genesis_key;
pub mod generate_wallet;
pub mod get_address;
pub mod import_wallet;
pub mod metrics_server;
pub mod mock_traits;
pub mod monitor;
pub mod qanto;
pub mod qanto_ai_metrics;
// Re-export core modules from the `qanto-core` crate instead of duplicating them.
pub use qanto_core::{
    qanto_compat, qanto_native_crypto, qanto_net, qanto_p2p, qanto_serde, qanto_storage,
    storage_adapter, storage_traits,
};
pub mod qantodag;
pub mod rpc_backend;
pub mod saga;
pub mod transaction;
pub mod types;
pub mod wallet;
pub mod websocket_server;
pub mod x_phyrus;

// Conditionally compiled modules for extended features.
#[cfg(feature = "infinite-strata")]
pub mod infinite_strata_node;

// Use the full zkp module when the `zk` feature is enabled
#[cfg(feature = "zk")]
pub mod zkp;

// Provide a lightweight stub for the `zkp` module when the `zk` feature is disabled,
// so dependent modules can compile without pulling in heavy Arkworks dependencies.
#[cfg(not(feature = "zk"))]
#[path = "zkp_stub.rs"]
pub mod zkp;

pub mod zk_sequencer;

pub mod memory_optimization;
pub use my_blockchain::qanhash;

pub mod qantowallet;
pub mod qds;

#[cfg(test)]
pub mod byzantine_tests;
#[cfg(test)]
pub mod corruption_tests;
#[cfg(test)]
pub mod economic_attack_tests;
#[cfg(test)]
pub mod partition_tests;
#[cfg(test)]
pub mod security_tests;
pub mod start_node;
pub mod state_sync;

// Test-safe tracing initialization to avoid double-init panics in tests
use std::sync::Once;
static INIT_TRACING: Once = Once::new();

/// Initialize tracing subscriber once for tests.
/// Uses `RUST_LOG` via `EnvFilter` when set; otherwise defaults to `info`.
pub fn init_test_tracing() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    INIT_TRACING.call_once(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        // Try init; ignore error if already initialized by other harness
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .try_init();
    });
}
pub mod ipc_server;

pub const Q_SCALE: u128 = 1_000_000_000;
pub const QANTO_SCALE: u128 = Q_SCALE;
pub const SCORE_SCALE: i128 = 1_000_000_000;

// ═══════════════════════════════════════════════════════════════════════════════
// CANONICAL TOKENOMICS — MATHEMATICALLY SEALED AT THE KERNEL LEVEL
// These constants are the single source of truth for QNTO supply distribution.
// Any module that needs supply constants MUST reference these or emission::TOTAL_SUPPLY.
// ═══════════════════════════════════════════════════════════════════════════════
/// 21 Billion QNTO total supply (9-decimal fixed-point)
pub const MAX_TOTAL_SUPPLY: u128 = 21_000_000_000 * Q_SCALE;
/// 80% → Community (mining, public rewards, fair launch distribution)
pub const COMMUNITY_ALLOC: u128 = 16_800_000_000 * Q_SCALE;
/// 15% → Ecosystem & Development Fund (2-year vest, 1-year cliff)
pub const ECO_DEV_ALLOC: u128 = 3_150_000_000 * Q_SCALE;
/// 5% → Public Liquidity (DEX bootstrapping)
pub const LIQUIDITY_ALLOC: u128 = 1_050_000_000 * Q_SCALE;

// Compile-time proof: allocations MUST sum exactly to MAX_TOTAL_SUPPLY.
// If this assertion fails, the crate will not compile.
const _: () = assert!(
    COMMUNITY_ALLOC + ECO_DEV_ALLOC + LIQUIDITY_ALLOC == MAX_TOTAL_SUPPLY,
    "FATAL: Tokenomics allocations do not sum to MAX_TOTAL_SUPPLY (21B QNTO)"
);

/// QAmount: Fixed-point u128 representing a decimal quantity (scale 1e9).
/// Used for balances, rewards, fees, and stakes.
pub type QAmount = u128;

/// QScore: Fixed-point i128 representing a signed score (scale 1e9).
/// Used for Saga scoring and reputation.
pub type QScore = i128;

/// QDifficulty: Mining difficulty ONLY.
pub type QDifficulty = u64;

pub mod math;
