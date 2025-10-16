// The primary modules that define the Qanto node and its behavior.
pub mod adaptive_mining;
pub mod analytics_dashboard;
pub mod config;
pub mod config_validator;
pub mod consensus;
pub mod decentralization;
pub mod deterministic_mining;
pub mod diagnostics;
pub mod elite_mempool;
pub mod emission;
pub mod gas_fee_model;
pub mod graphql_server;
pub mod hame;
pub mod interoperability;
pub mod keygen;
pub mod mempool;
pub mod metrics;
pub mod miner;
pub mod mining_celebration;
pub mod mining_metrics;
pub mod node;
pub mod omega;
pub mod omega_enhanced;
pub mod optimized_qdag;
pub mod p2p;
pub mod password_utils;
pub mod performance_monitoring;
pub mod performance_optimizations;
pub mod performance_validation;
pub mod post_quantum_crypto;
pub mod privacy;
pub mod resource_cleanup;
pub mod safe_interval;
pub mod shutdown;
pub mod telemetry;
pub mod timing;

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

pub mod memory_optimization;
pub use my_blockchain::qanhash;

pub mod qantowallet;

pub mod start_node;
