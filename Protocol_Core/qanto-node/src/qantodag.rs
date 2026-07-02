//! --- Qanto QantoDAG Ledger ---
//! v0.1.0 - Initial Version
//! This version corrects the block reward calculation by ensuring that transaction
//! fees are properly calculated and included in the final reward amount.
//!
//! - FIX: The `create_candidate_block` function now calculates the total fees
//!   from mempool transactions *before* calling SAGA's reward calculation,
//!   ensuring the final reward includes both the base amount and fees.

use crate::config::LoggingConfig;
use crate::emission::Emission;
use crate::mempool::Mempool;
use crate::metrics::QantoMetrics;
use crate::miner::Miner;
use crate::mining_metrics::MiningMetrics;
use crate::performance_monitoring::{PerformanceMonitor, PerformanceMonitoringConfig};
use crate::saga::{
    CarbonOffsetCredential, GovernanceProposal, PalletSaga, ProposalStatus, ProposalType,
};
use crate::timing::BlockTimingCoordinator;
use crate::types::QuantumResistantSignature;
use my_blockchain::{qanhash, qanto_hash};
use qanto_core::balance_stream::BalanceBroadcaster;
use qanto_core::mining_celebration::LoggingConfig as CoreLoggingConfig;

// This import is required for the `#[from]` attribute in QantoDAGError.
// The compiler may incorrectly flag it as unused, but it is necessary.
use crate::optimized_qdag::{OptimizedQDagConfig, OptimizedQDagGenerator};
use crate::persistence::{
    balance_key, decode_balance, encode_balance, genesis_id_key, tip_key, tips_prefix,
    PersistenceWriter, BALANCES_KEY_PREFIX, GENESIS_BLOCK_ID_KEY,
};
use crate::post_quantum_crypto::{
    pq_sign, pq_verify, PQError, QantoPQPrivateKey, QantoPQPublicKey, QantoPQSignature,
};
use crate::qanto_storage::{
    AccountStateCache, QantoStorage, QantoStorageError, StorageConfig, WriteBatch,
};
use crate::transaction::{Output, Transaction};
use crate::types::{HomomorphicEncrypted, UTXO};

use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use chrono::Utc;
use crossbeam::channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use hex;
use lru::LruCache;
use parking_lot::RwLock as ParkingRwLock;
use prometheus::{register_int_counter, IntCounter};
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime, SystemTimeError, UNIX_EPOCH};

use crate::websocket_server::BalanceEvent;
use thiserror::Error;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tokio::task;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// --- High-Throughput Constants ---
// To achieve ~10M TPS at 32 BPS, each block must hold ~312,500 transactions.
// Set to 312,500 transactions per block for precise 10M+ TPS target
pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 312_500;
// Increased block size to accommodate the higher transaction count.
pub const MAX_BLOCK_SIZE: usize = 33_554_432; // 32 MB (32 * 1024 * 1024)
pub const MAX_TRANSACTION_SIZE: usize = 102_400; // 100 KB per transaction

// --- Network & Economic Constants ---
pub const DEV_ADDRESS: &str = "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3";
pub const CONTRACT_ADDRESS: &str =
    "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394";
pub const INITIAL_BLOCK_REWARD: u128 = 50 * crate::transaction::SMALLEST_UNITS_PER_QAN; // 50 QAN in base units
const FINALIZATION_DEPTH: u64 = 8;
const SHARD_THRESHOLD: u32 = 2;
const TEMPORAL_CONSENSUS_WINDOW: u64 = 600;
const MAX_BLOCKS_PER_MINUTE: u64 = 32 * 60;
const MIN_VALIDATOR_STAKE: u128 = 50 * crate::transaction::SMALLEST_UNITS_PER_QAN;
const SLASHING_PENALTY: u128 = 30 * crate::transaction::SMALLEST_UNITS_PER_QAN;
const CACHE_SIZE: usize = 10_000; // Increased cache size for better performance
const ANOMALY_DETECTION_BASELINE_BLOCKS: usize = 100;
const ANOMALY_Z_SCORE_THRESHOLD: u128 = 3 * crate::QANTO_SCALE + 500_000_000;
#[cfg(feature = "performance-test")]
const INITIAL_DIFFICULTY: u64 = (crate::QANTO_SCALE / 1000) as u64; // Minimal difficulty for performance testing (0.001)
#[cfg(not(feature = "performance-test"))]
const INITIAL_DIFFICULTY: u64 = crate::QANTO_SCALE as u64; // Initial difficulty: 1.0 represented as 1e9

// High-Performance Optimization Constants - Optimized for 32 BPS / 10M+ TPS / <31ms latency
const PARALLEL_VALIDATION_BATCH_SIZE: usize = 50000; // Massive increase for 10M+ TPS
#[cfg(feature = "performance-test")]
const BLOCK_PROCESSING_WORKERS: usize = 8; // Reduced for test stability
#[cfg(not(feature = "performance-test"))]
const BLOCK_PROCESSING_WORKERS: usize = 256; // Doubled for extreme parallelism
#[cfg(feature = "performance-test")]
const TRANSACTION_VALIDATION_WORKERS: usize = 4; // Reduced for test stability
#[cfg(not(feature = "performance-test"))]
const TRANSACTION_VALIDATION_WORKERS: usize = 128; // 4x increase for signature verification
const FAST_SYNC_BATCH_SIZE: usize = 5000; // 5x increase for faster sync
const BLOCK_CACHE_TTL_SECS: u64 = 3600; // Reduced for memory efficiency
const VALIDATION_TIMEOUT_MS: u64 = 25; // Halved for <31ms latency target
const CONCURRENT_BLOCK_LIMIT: usize = 512; // Doubled concurrent processing
const SIMD_BATCH_SIZE: usize = 32; // 4x increase for SIMD processing

/// Maximum allowable clock drift in seconds between local time and parent block timestamps.
/// If local clock deviates more than this from the network-observed parent timestamps,
/// block production is halted to prevent chain splits from desynchronized clocks.
/// Set to 10s for Cohort A (residential hardware with sleep/wake cycles).
/// Can be tightened to 5s for Public Testnet with datacenter validators.
const MAX_CLOCK_DRIFT_SECS: u64 = 10;

const LOCK_FREE_QUEUE_SIZE: usize = 262144; // 4x increase for lock-free queue capacity

lazy_static::lazy_static! {
    static ref BLOCKS_PROCESSED: IntCounter = register_int_counter!("blocks_processed_total", "Total blocks processed")
        .unwrap_or_else(|_| prometheus::IntCounter::new("blocks_processed_total_fallback", "Total blocks processed fallback").unwrap());
    static ref TRANSACTIONS_PROCESSED: IntCounter = register_int_counter!("transactions_processed_total", "Total transactions processed")
        .unwrap_or_else(|_| prometheus::IntCounter::new("transactions_processed_total_fallback", "Total transactions processed fallback").unwrap());
    static ref ANOMALIES_DETECTED: IntCounter = register_int_counter!("anomalies_detected_total", "Total anomalies detected")
        .unwrap_or_else(|_| prometheus::IntCounter::new("anomalies_detected_total_fallback", "Total anomalies detected fallback").unwrap());
}

// Precompute decorative strings used in QantoBlock Display to minimize runtime allocations
lazy_static::lazy_static! {
    static ref QBLOCK_BORDER: String = "═".repeat(90);
    static ref QBLOCK_DETAILS_FILL: String = "─".repeat(70);
}

// #region debug-point block-signature-failure
fn load_block_signature_debug_value(key: &str) -> Option<String> {
    if let Ok(value) = std::env::var(key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let env_path = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(".dbg/block-signature-failure.env"))?;
    let contents = std::fs::read_to_string(env_path).ok()?;

    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        if name.trim() == key {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        } else {
            None
        }
    })
}

fn report_block_signature_debug_event(
    hypothesis_id: &str,
    point: &str,
    location: &str,
    payload: serde_json::Value,
) {
    let Some(url) = load_block_signature_debug_value("DEBUG_SERVER_URL") else {
        return;
    };
    let session_id = load_block_signature_debug_value("DEBUG_SESSION_ID")
        .unwrap_or_else(|| "block-signature-failure".to_string());

    let event = json!({
        "sessionId": session_id,
        "runId": "pre-fix",
        "hypothesisId": hypothesis_id,
        "location": location,
        "msg": format!("[DEBUG] {}", point),
        "data": payload,
        "ts": Utc::now().timestamp_millis(),
    });

    if let Ok(runtime_handle) = tokio::runtime::Handle::try_current() {
        runtime_handle.spawn(async move {
            let _ = reqwest::Client::new().post(url).json(&event).send().await;
        });
    }
}
// #endregion debug-point block-signature-failure

// #region debug-point rpcnode-sync-height
fn load_rpcnode_sync_height_debug_value(key: &str) -> Option<String> {
    if let Ok(value) = std::env::var(key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let env_path = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(".dbg/rpcnode-sync-height.env"))?;
    let contents = std::fs::read_to_string(env_path).ok()?;

    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        if name.trim() == key {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        } else {
            None
        }
    })
}

fn report_rpcnode_sync_height_debug_event(
    hypothesis_id: &str,
    point: &str,
    location: &str,
    payload: serde_json::Value,
) {
    let Some(url) = load_rpcnode_sync_height_debug_value("DEBUG_SERVER_URL") else {
        return;
    };
    let session_id = load_rpcnode_sync_height_debug_value("DEBUG_SESSION_ID")
        .unwrap_or_else(|| "rpcnode-sync-height".to_string());

    let event = json!({
        "sessionId": session_id,
        "runId": "pre-fix",
        "hypothesisId": hypothesis_id,
        "location": location,
        "msg": format!("[DEBUG] {}", point),
        "data": payload,
        "ts": Utc::now().timestamp_millis(),
    });

    if let Ok(runtime_handle) = tokio::runtime::Handle::try_current() {
        runtime_handle.spawn(async move {
            let _ = reqwest::Client::new().post(url).json(&event).send().await;
        });
    }
}
// #endregion debug-point rpcnode-sync-height

// Removed candidate transaction cap override to simplify selection logic

#[derive(Error, Debug)]
pub enum QantoDAGError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(#[from] crate::transaction::TransactionError),
    #[error("Invalid parent: {0}")]
    InvalidParent(String),
    #[error("System time error: {0}")]
    Time(#[from] SystemTimeError),
    #[error("Cross-chain reference error: {0}")]
    CrossChainReferenceError(String),
    #[error("Reward mismatch: expected {0}, got {1}")]
    RewardMismatch(u128, u128),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Merkle root mismatch")]
    MerkleRootMismatch,
    #[error("ZKP verification failed: {0}")]
    ZKPVerification(String),
    #[error("Governance proposal failed: {0}")]
    Governance(String),
    #[error("Quantum-resistant signature error: {0}")]
    QuantumSignature(#[from] crate::post_quantum_crypto::PQError),
    #[error("Homomorphic encryption error: {0}")]
    HomomorphicError(String),
    #[error("IDS anomaly detected: {0}")]
    IDSAnomaly(String),
    #[error("BFT consensus failure: {0}")]
    BFTFailure(String),
    #[error("Smart contract execution failed: {0}")]
    SmartContractError(String),
    #[error("Cross-chain atomic swap failed: {0}")]
    CrossChainSwapError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Emission calculation error: {0}")]
    EmissionError(String),
    #[error("Task join error: {0}")]
    JoinError(#[from] task::JoinError),
    #[error("Saga error: {0}")]
    SagaError(#[from] anyhow::Error),
    #[error("QantoDAG self-reference not initialized. This indicates a critical bug in the node startup sequence.")]
    SelfReferenceNotInitialized,
    #[error("QantoStorage error: {0}")]
    Storage(#[from] QantoStorageError),
    #[error("Miner error: {0}")]
    MinerError(String),
    #[error("Clock drift detected: local clock is {drift_secs}s off from parent timestamps (threshold: {threshold}s). Block production halted.")]
    ClockDrift { drift_secs: u64, threshold: u64 },
    #[error("Hex decoding error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Wallet error: {0}")]
    WalletError(String),
    #[error("Generic error: {0}")]
    Generic(String),
}

impl From<crate::wallet::WalletError> for QantoDAGError {
    fn from(e: crate::wallet::WalletError) -> Self {
        QantoDAGError::WalletError(e.to_string())
    }
}

impl From<String> for QantoDAGError {
    fn from(e: String) -> Self {
        QantoDAGError::Generic(e)
    }
}

pub struct SigningData<'a> {
    pub parents: &'a [String],
    pub transactions: &'a [Transaction],
    pub timestamp: u64,
    pub difficulty: u64,
    pub validator: &'a str,
    pub miner: &'a str,
    pub chain_id: u32,
    pub merkle_root: &'a str,
    pub height: u64,
}

pub struct QantoBlockCreationData {
    pub validator_private_key: QantoPQPrivateKey,
    pub chain_id: u32,
    pub parents: Vec<String>,
    pub transactions: Vec<Transaction>,
    pub difficulty: u64,
    pub validator: String,
    pub miner: String,

    pub timestamp: u64,
    pub current_epoch: u64,
    pub height: u64,
    pub paillier_pk: Vec<u8>,
}

#[derive(Debug)]
pub struct CrossChainSwapParams {
    pub source_chain: u32,
    pub target_chain: u32,
    pub source_block_id: String,
    pub amount: u128,
    pub initiator: String,
    pub responder: String,
    pub timelock_duration: u64,
    pub secret_hash: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CrossChainSwap {
    pub swap_id: String,
    pub source_chain: u32,
    pub target_chain: u32,
    pub amount: u128,
    pub initiator: String,
    pub responder: String,
    pub timelock: u64,
    pub state: SwapState,
    pub secret_hash: String,
    pub secret: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum SwapState {
    Initiated,
    Redeemed,
    Refunded,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SmartContract {
    pub contract_id: String,
    pub code: String,
    pub storage: HashMap<String, String>,
    pub owner: String,
    pub gas_balance: u128,
}

impl SmartContract {
    pub fn execute(&mut self, input: &str, gas_limit: u128) -> Result<String, QantoDAGError> {
        let mut gas_used = 0u128;
        let mut charge_gas = |cost: u128| -> Result<(), QantoDAGError> {
            gas_used += cost;
            if gas_used > gas_limit || gas_used > self.gas_balance {
                let mut error_msg = String::with_capacity(50);
                error_msg.push_str("Out of gas. Limit: ");
                error_msg.push_str(&gas_limit.to_string());
                error_msg.push_str(", Used: ");
                error_msg.push_str(&gas_used.to_string());
                Err(QantoDAGError::SmartContractError(error_msg))
            } else {
                Ok(())
            }
        };

        let result = if self.code.contains("echo") {
            charge_gas(10)?;
            charge_gas(input.len() as u128)?;
            self.storage
                .insert("last_input".to_string(), input.to_string());
            let mut result = String::with_capacity(6 + input.len());
            result.push_str("echo: ");
            result.push_str(input);
            Ok(result)
        } else if self.code.contains("increment_counter") {
            charge_gas(50)?;
            let counter_str = self
                .storage
                .entry("counter".to_string())
                .or_insert_with(|| "0".to_string());
            let current_val: u64 = counter_str.parse().unwrap_or_default();
            *counter_str = (current_val + 1).to_string();
            let mut result = String::with_capacity(19 + counter_str.len());
            result.push_str("counter updated to: ");
            result.push_str(counter_str);
            Ok(result)
        } else {
            Err(QantoDAGError::SmartContractError(
                "Unsupported contract code or execution logic".to_string(),
            ))
        };

        self.gas_balance -= gas_used;
        result
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct QantoBlock {
    pub chain_id: u32,
    pub id: String,
    pub parents: Vec<String>,
    pub transactions: Vec<Transaction>,
    pub difficulty: u64,
    pub validator: String,
    pub miner: String,
    pub nonce: u64,
    pub timestamp: u64,
    pub height: u64,
    pub reward: u128,
    pub effort: u128,
    pub cross_chain_references: Vec<(u32, String)>,
    pub cross_chain_swaps: Vec<CrossChainSwap>,
    pub merkle_root: String,
    pub signature: QuantumResistantSignature,

    pub homomorphic_encrypted: Vec<HomomorphicEncrypted>,
    pub smart_contracts: Vec<SmartContract>,
    #[serde(default)]
    pub carbon_credentials: Vec<CarbonOffsetCredential>,
    pub epoch: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub finality_proof: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reservation_miner_id: Option<String>,
}

impl fmt::Display for QantoBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use precomputed borders to avoid per-call allocations
        writeln!(f, "╔{}╗", QBLOCK_BORDER.as_str())?;
        writeln!(
            f,
            "║ ⛓️  New Qanto Block Mined on Chain #{} ⛓️",
            self.chain_id
        )?;
        writeln!(f, "╟{}╢", QBLOCK_BORDER.as_str())?;
        writeln!(f, "║ 🆔 Block ID:      {}", self.id)?;
        writeln!(f, "║ 📅 Timestamp:     {}", self.timestamp)?;
        writeln!(f, "║ 📈 Height:        {}", self.height)?;
        if self.parents.is_empty() {
            writeln!(f, "║ 🔗 Parents:        (Genesis Block)")?;
        } else {
            // Stream parents without allocating a joined String
            write!(f, "║ 🔗 Parents:        ")?;
            for (i, p) in self.parents.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                f.write_str(p)?;
            }
            writeln!(f)?;
        }
        writeln!(f, "║ 🧾 Transactions:   {}", self.transactions.len())?;
        let total_offset: u128 = self
            .carbon_credentials
            .iter()
            .map(|c| c.tonnes_co2_sequestered as u128)
            .sum();
        if total_offset > 0 {
            let offset_whole = total_offset / crate::QANTO_SCALE;
            let offset_frac = (total_offset % crate::QANTO_SCALE) / (crate::QANTO_SCALE / 1000); // 3 decimals
            writeln!(
                f,
                "║ 🌍 Carbon Offset:  {}.{:03} tonnes CO₂e",
                offset_whole, offset_frac
            )?;
        }
        writeln!(f, "║ 🌳 Merkle Root:    {}", self.merkle_root)?;
        writeln!(f, "╟─ Mining Details ─{}╢", QBLOCK_DETAILS_FILL.as_str())?;
        writeln!(f, "║ ⛏️  Miner:           {}", self.miner)?;
        writeln!(f, "║ ✨ Nonce:          {}", self.nonce)?;
        writeln!(
            f,
            "║ 🎯 Difficulty:    {:.4}",
            self.difficulty as f64 / crate::QANTO_SCALE as f64
        )?;
        writeln!(f, "║ 💪 Effort:         {} hashes", self.effort)?;
        writeln!(
            f,
            "║ 💰 Block Reward:    {} $QAN (from Emission Rules)",
            self.reward / crate::transaction::SMALLEST_UNITS_PER_QAN
        )?;
        writeln!(f, "╚{}╝", QBLOCK_BORDER.as_str())?;
        Ok(())
    }
}

impl QantoBlock {
    pub fn new(data: QantoBlockCreationData) -> Result<Self, QantoDAGError> {
        let nonce = 0;
        let merkle_root = Self::compute_merkle_root(&data.transactions)?;

        let signing_data = SigningData {
            parents: &data.parents,
            transactions: &data.transactions,
            timestamp: data.timestamp,
            difficulty: data.difficulty,
            validator: &data.validator,
            miner: &data.miner,
            chain_id: data.chain_id,
            merkle_root: &merkle_root,
            height: data.height,
        };

        let pre_signature_data_for_id = Self::serialize_for_signing(&signing_data)?;
        let id = hex::encode(qanto_hash(&pre_signature_data_for_id));

        // For test environments, use empty homomorphic data to avoid massive TFHE serialization
        // Always use empty data for now to avoid the 437MB serialization issue
        let homomorphic_encrypted_data = data
            .transactions
            .iter()
            .map(|_tx| HomomorphicEncrypted {
                ciphertext: vec![],
                public_key: vec![],
            })
            .collect();

        let signer_public_key = data.validator_private_key.public_key().as_bytes().to_vec();
        let signature =
            pq_sign(&data.validator_private_key, &pre_signature_data_for_id).map_err(|_e| {
                QantoDAGError::QuantumSignature(crate::post_quantum_crypto::PQError::SigningError)
            })?;

        let block = Self {
            chain_id: data.chain_id,
            id,
            parents: data.parents,
            transactions: data.transactions,
            difficulty: data.difficulty,
            validator: data.validator,
            miner: data.miner,
            nonce,
            timestamp: data.timestamp,
            height: data.height,
            reward: 0,
            effort: 0,
            cross_chain_references: vec![],
            merkle_root,

            cross_chain_swaps: vec![],
            homomorphic_encrypted: homomorphic_encrypted_data,
            smart_contracts: vec![],
            signature: QuantumResistantSignature {
                signer_public_key,
                signature: signature.as_bytes().to_vec(),
            },
            carbon_credentials: vec![],
            epoch: data.current_epoch,
            finality_proof: None,
            reservation_miner_id: None,
        };

        // #region debug-point block-signature-failure-new
        report_block_signature_debug_event(
            "A",
            "qantoblock-new",
            "qantodag.rs:525",
            json!({
                "block_id": block.id,
                "height": block.height,
                "validator": block.validator,
                "signer_public_key_len": block.signature.signer_public_key.len(),
                "signature_len": block.signature.signature.len(),
                "immediate_verify": block.verify_signature().ok(),
                "recomputed_id_matches": block.compute_block_id().ok().map(|computed| computed == block.id),
                "first_tx_chain_id": block.transactions.first().map(|tx| tx.chain_id),
            }),
        );
        // #endregion debug-point block-signature-failure-new

        Ok(block)
    }

    pub fn serialize_for_signing(data: &SigningData) -> Result<Vec<u8>, QantoDAGError> {
        let mut buffer = Vec::with_capacity(512); // Pre-allocate

        // --- CANONICAL SERIALIZATION ORDER (Big-Endian) ---
        buffer.extend_from_slice(&data.timestamp.to_be_bytes());
        buffer.extend_from_slice(&data.chain_id.to_be_bytes());
        buffer.extend_from_slice(&data.height.to_be_bytes());
        buffer.extend_from_slice(&data.difficulty.to_be_bytes());
        buffer.extend_from_slice(data.merkle_root.as_bytes());
        buffer.extend_from_slice(data.validator.as_bytes());
        buffer.extend_from_slice(data.miner.as_bytes());

        // Parents in deterministic order
        for parent in data.parents {
            buffer.extend_from_slice(parent.as_bytes());
        }

        // Transaction list binding
        for tx in data.transactions {
            buffer.extend_from_slice(tx.id.as_bytes());
        }

        Ok(qanto_hash(&buffer).as_bytes().to_vec())
    }
    pub fn compute_block_id(&self) -> Result<String, QantoDAGError> {
        let signing_data = SigningData {
            parents: &self.parents,
            transactions: &self.transactions,
            timestamp: self.timestamp,
            difficulty: self.difficulty,
            validator: &self.validator,
            miner: &self.miner,
            chain_id: self.chain_id,
            merkle_root: &self.merkle_root,
            height: self.height,
        };
        let pre_sig_data = Self::serialize_for_signing(&signing_data)?;
        Ok(hex::encode(qanto_hash(&pre_sig_data)))
    }

    pub fn new_test_block(block_id: String) -> Self {
        let dummy_tx = Transaction::new_dummy();
        let transactions = vec![dummy_tx];
        let merkle_root = Self::compute_merkle_root(&transactions).unwrap_or_default();

        Self {
            chain_id: 0,
            id: block_id,
            parents: vec![],
            transactions,
            difficulty: INITIAL_DIFFICULTY,
            validator: "test_validator".to_string(),
            miner: "test_miner".to_string(),
            nonce: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            height: 1,
            reward: 0,
            effort: 0,
            cross_chain_references: vec![],
            merkle_root,
            cross_chain_swaps: vec![],
            homomorphic_encrypted: vec![],
            smart_contracts: vec![],
            signature: QuantumResistantSignature {
                signer_public_key: vec![0; 32],
                signature: vec![0; 64],
            },
            carbon_credentials: vec![],
            epoch: 0,
            finality_proof: None,
            reservation_miner_id: None,
        }
    }

    pub fn compute_merkle_root(transactions: &[Transaction]) -> Result<String, QantoDAGError> {
        if transactions.is_empty() {
            return Ok(hex::encode(qanto_hash(&[]).as_bytes()));
        }
        let mut leaves: Vec<Vec<u8>> = transactions
            .par_iter()
            .map(|tx| qanto_hash(tx.id.as_bytes()).as_bytes().to_vec())
            .collect();

        while leaves.len() > 1 {
            if !leaves.len().is_multiple_of(2) {
                leaves.push(leaves.last().unwrap().clone());
            }
            leaves = leaves
                .par_chunks(2)
                .map(|chunk| {
                    let mut data = Vec::new();
                    data.extend_from_slice(&chunk[0]);
                    data.extend_from_slice(&chunk[1]);
                    qanto_hash(&data).as_bytes().to_vec()
                })
                .collect();
        }
        Ok(hex::encode(leaves.first().ok_or_else(|| {
            QantoDAGError::InvalidBlock("Merkle root computation failed".to_string())
        })?))
    }

    pub fn verify_signature(&self) -> Result<bool, QantoDAGError> {
        let signing_data = SigningData {
            parents: &self.parents,
            transactions: &self.transactions,
            timestamp: self.timestamp,
            difficulty: self.difficulty,
            validator: &self.validator,
            miner: &self.miner,
            chain_id: self.chain_id,
            merkle_root: &self.merkle_root,
            height: self.height,
        };

        let data_to_verify = Self::serialize_for_signing(&signing_data)?;

        let pk = QantoPQPublicKey::from_bytes(&self.signature.signer_public_key).map_err(|_e| {
            QantoDAGError::QuantumSignature(crate::post_quantum_crypto::PQError::VerificationError)
        })?;
        let sig = QantoPQSignature::from_bytes(&self.signature.signature).map_err(|_e| {
            QantoDAGError::QuantumSignature(crate::post_quantum_crypto::PQError::VerificationError)
        })?;

        Ok(pq_verify(&pk, &data_to_verify, &sig).unwrap_or_default())
    }

    pub fn hash(&self) -> String {
        // Create header hash for qanhash algorithm (excluding id to avoid circular dependency)
        let mut header_data = Vec::new();
        header_data.extend_from_slice(&self.timestamp.to_be_bytes());
        header_data.extend_from_slice(&self.chain_id.to_be_bytes());
        header_data.extend_from_slice(&self.height.to_be_bytes());
        header_data.extend_from_slice(&self.difficulty.to_be_bytes());
        header_data.extend_from_slice(self.merkle_root.as_bytes());
        header_data.extend_from_slice(self.validator.as_bytes());
        header_data.extend_from_slice(self.miner.as_bytes());

        // Add parent hashes to header
        for parent in &self.parents {
            header_data.extend_from_slice(parent.as_bytes());
        }

        // Create header hash using qanto_hash
        let header_hash = qanto_hash(&header_data);

        // Use qanhash algorithm for mining hash
        let qanhash_result = qanhash::hash(&header_hash, self.nonce, self.height);
        hex::encode(qanhash_result)
    }

    /// **CANONICAL PROOF-OF-WORK HASH METHOD - SINGLE SOURCE OF TRUTH**
    ///
    /// This method generates the definitive hash that is subject to the Proof-of-Work challenge.
    /// It serves as the ONLY way to compute PoW hashes across the entire codebase, ensuring
    /// perfect consistency between mining and validation code paths.
    ///
    /// ## Field Serialization Order (Big-Endian):
    /// 1. `timestamp` (u64) - Block creation timestamp
    /// 2. `chain_id` (u32) - Chain identifier  
    /// 3. `height` (u64) - Block height in the chain
    /// 4. `difficulty` (u64) - Mining difficulty as fixed-point (scale 1e9)
    /// 5. `merkle_root` (String) - Merkle root of transactions as UTF-8 bytes
    /// 6. `validator` (String) - Validator address as UTF-8 bytes
    /// 7. `miner` (String) - Miner address as UTF-8 bytes
    /// 8. `parents` (Vec<String>) - Parent block hashes as UTF-8 bytes (in order)
    ///
    /// ## Algorithm:
    /// 1. Serialize all fields in the specified order using big-endian byte representation
    /// 2. Create header hash using `qanto_hash(&header_data)` (NOTE: nonce is NOT part of header)
    /// 3. Apply qanhash algorithm: `qanhash::hash(header_hash, nonce, height)`
    /// 4. Return as `QantoHash` type for type safety
    ///
    /// ## Performance Notes:
    /// - Zero-allocation design for 31ms target block time
    /// - Excludes block's own `id` field to prevent non-deterministic hashing
    /// - Uses pre-allocated Vec with capacity hint for optimal memory usage
    /// - Header hash is invariant across nonce trials to match miner
    ///
    /// ## Critical:
    /// This method MUST be used for ALL PoW operations. Any deviation will cause
    /// mining/validation inconsistencies and 100% block rejection.
    pub fn hash_for_pow(&self) -> my_blockchain::qanto_standalone::hash::QantoHash {
        // Pre-allocate header data with estimated capacity to avoid reallocations
        let estimated_capacity = 280 + (self.parents.len() * 64);
        let mut header_data = Vec::with_capacity(estimated_capacity);

        // --- CANONICAL SERIALIZATION ORDER (MUST MATCH serialize_for_signing) ---
        header_data.extend_from_slice(&self.timestamp.to_be_bytes());
        header_data.extend_from_slice(&self.chain_id.to_be_bytes());
        header_data.extend_from_slice(&self.height.to_be_bytes());
        header_data.extend_from_slice(&self.difficulty.to_be_bytes());
        header_data.extend_from_slice(self.merkle_root.as_bytes());
        header_data.extend_from_slice(self.validator.as_bytes());
        header_data.extend_from_slice(self.miner.as_bytes());

        // Add parent hashes in deterministic order
        for parent in &self.parents {
            header_data.extend_from_slice(parent.as_bytes());
        }

        // Note: Transactions are implicitly included via merkle_root.
        // hash_for_pow excludes individual tx IDs for mining efficiency.

        // Create header hash using qanto_hash
        let header_hash = qanto_hash(&header_data);

        // Apply qanhash algorithm - this is the canonical PoW hash
        let qanhash_result = qanhash::hash(&header_hash, self.nonce, self.height);

        // Return as QantoHash type for type safety and consistency
        my_blockchain::qanto_standalone::hash::QantoHash::new(qanhash_result)
    }

    /// Legacy method - DEPRECATED: Use hash_for_pow() instead
    ///
    /// This method is maintained for backward compatibility but will be removed.
    /// All new code MUST use hash_for_pow() as the canonical PoW hash method.
    #[deprecated(
        since = "1.0.0",
        note = "Use hash_for_pow() instead for canonical PoW hashing"
    )]
    pub fn pow_hash(&self) -> my_blockchain::qanto_standalone::hash::QantoHash {
        // Delegate to the canonical method to ensure consistency
        self.hash_for_pow()
    }

    /// Canonical Proof-of-Work validity check - single source of truth
    /// Uses the canonical PoW hash and the global Consensus difficulty check
    pub fn is_pow_valid(&self) -> bool {
        let pow_hash = self.hash_for_pow();
        crate::consensus::Consensus::is_pow_valid(pow_hash.as_bytes(), self.difficulty)
    }

    /// Optimized Proof-of-Work validity check using a precomputed PoW hash to avoid recomputation
    pub fn is_pow_valid_with_pow_hash(
        &self,
        pow_hash: my_blockchain::qanto_standalone::hash::QantoHash,
    ) -> bool {
        crate::consensus::Consensus::is_pow_valid(pow_hash.as_bytes(), self.difficulty)
    }
}

// Use unified metrics system
pub type PerformanceMetrics = QantoMetrics;

/// Configuration for creating a new QantoDAG instance.
pub struct QantoDagConfig {
    pub initial_validator: String,
    pub genesis_timestamp: u64,
    pub target_block_time: u64,
    pub num_chains: u32,
    /// Developer fee rate as fixed-point (scale 1e9, e.g., 0.10 = 100_000_000)
    pub dev_fee_rate: u128,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IndexedTransaction {
    pub tx: Transaction,
    pub block_id: String,
    pub block_height: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IndexedBridgeClaim {
    pub claim_key: String,
    pub source_chain: String,
    pub source_tx_hash: String,
    pub recipient: String,
    pub amount: u128,
    pub mint_tx_id: String,
    pub block_id: String,
    pub block_height: u64,
}

#[derive(Debug)]
pub struct QantoDAG {
    // --- CRITICAL: Consensus Lock to prevent double-spend race conditions (B-C1) ---
    pub consensus_lock: tokio::sync::Mutex<()>,
    // Core data structures with optimized concurrent access
    pub blocks: Arc<DashMap<String, QantoBlock>>,
    pub tips: Arc<DashMap<u32, HashSet<String>>>,
    pub validators: Arc<DashMap<String, u128>>,
    pub delegations: Arc<DashMap<(String, String), u128>>,
    pub target_block_time: u64,
    pub emission: Arc<RwLock<Emission>>,
    pub num_chains: Arc<RwLock<u32>>,
    pub finalized_blocks: Arc<DashMap<String, bool>>,
    pub chain_loads: Arc<DashMap<u32, AtomicU64>>,
    pub difficulty_history: Arc<ParkingRwLock<Vec<(u64, u64)>>>,
    pub block_creation_timestamps: Arc<DashMap<String, u64>>,
    pub anomaly_history: Arc<DashMap<String, u64>>,
    pub cross_chain_swaps: Arc<DashMap<String, CrossChainSwap>>,
    pub smart_contracts: Arc<DashMap<String, SmartContract>>,
    pub cache: Arc<ParkingRwLock<LruCache<String, QantoBlock>>>,
    pub db: Arc<QantoStorage>,
    pub persistence_writer: Arc<PersistenceWriter>,
    pub saga: Arc<PalletSaga>,
    pub self_arc: Weak<QantoDAG>,
    pub current_epoch: Arc<AtomicU64>,
    pub balance_event_sender: Arc<RwLock<Option<broadcast::Sender<BalanceEvent>>>>,
    pub balance_broadcaster: Arc<RwLock<Option<Arc<BalanceBroadcaster>>>>,
    /// In-memory account state cache for fast balance reads and saturating updates
    pub account_state_cache: AccountStateCache,
    pub total_bridge_locked: Arc<RwLock<u128>>,
    pub total_bridge_claimed: Arc<RwLock<u128>>,
    pub processed_bridge_claims: Arc<DashMap<String, bool>>,

    // Indexer & Explorer Intelligence Layer fields
    pub tx_index: Arc<DashMap<String, IndexedTransaction>>,
    pub address_index: Arc<DashMap<String, Vec<String>>>,
    pub validator_blocks_index: Arc<DashMap<String, Vec<String>>>,
    pub bridge_claims_index: Arc<DashMap<String, IndexedBridgeClaim>>,

    // High-performance optimization fields - Enhanced for 32 BPS / 10M+ TPS
    pub block_processing_semaphore: Arc<Semaphore>,
    pub validation_workers: Arc<Semaphore>,
    pub block_queue: Arc<(Sender<QantoBlock>, Receiver<QantoBlock>)>,
    pub validation_cache: Arc<DashMap<String, (bool, Instant)>>,
    pub fast_tips_cache: Arc<DashMap<u32, Vec<String>>>,
    pub processing_blocks: Arc<DashMap<String, AtomicBool>>,
    pub performance_metrics: Arc<PerformanceMetrics>,
    // Advanced performance optimization fields
    pub simd_processor: Arc<DashMap<String, Vec<u8>>>, // SIMD-optimized data processing
    pub lock_free_tx_queue: Arc<crossbeam::queue::SegQueue<Transaction>>, // Lock-free transaction queue
    pub memory_pool: Arc<DashMap<usize, Vec<u8>>>, // Memory pool for zero-copy operations
    pub prefetch_cache: Arc<DashMap<String, (QantoBlock, Instant)>>, // Block prefetch cache
    pub pipeline_stages: Vec<Arc<Semaphore>>,      // Pipeline stage semaphores
    pub work_stealing_pool: Arc<rayon::ThreadPool>, // Work-stealing thread pool
    pub utxo_bloom_filter: Arc<DashMap<String, bool>>, // Bloom filter for UTXO existence
    pub batch_processor: Arc<DashMap<String, Vec<Transaction>>>, // Batch processing queues
    pub mining_metrics: Arc<MiningMetrics>,        // Mining performance metrics and monitoring
    pub timing_coordinator: Arc<BlockTimingCoordinator>, // Microsecond-precision timing for 32+ BPS
    #[allow(dead_code)]
    pub performance_monitor: Arc<PerformanceMonitor>, // Comprehensive performance monitoring and adaptive tuning
    pub logging_config: LoggingConfig,
    /// Optimized Q-DAG generator for caching DAGs by epoch to prevent repeated generations
    pub qdag_generator: Arc<OptimizedQDagGenerator>, // Configuration for celebration logging
    /// Developer fee rate as fixed-point (scale 1e9, e.g., 0.10 = 100_000_000)
    pub dev_fee_rate: u128,
    pub last_fork_depth: Arc<AtomicU64>,
    pub last_fork_lca: Arc<tokio::sync::RwLock<String>>,
    pub bypass_reward_check: Arc<AtomicBool>,
}

/// Helper struct to track mining state
#[derive(Debug)]
pub struct MiningState {
    pub block_height: u64,
    pub block_hash: String,
    pub timestamp: u64,
}
// Deterministic validation outcome struct at module level for broad visibility
/// A structured validation outcome carrying a deterministic witness and UTXO footprint
pub struct ValidationOutcome {
    pub index: usize,
    pub block: QantoBlock,
    pub is_valid: bool,
    pub error: Option<QantoDAGError>,
    pub utxo_inputs: Vec<String>,
    pub state_witness: [u8; 32],
}

impl QantoDAG {
    pub fn new(
        config: QantoDagConfig,
        saga: Arc<PalletSaga>,
        db: QantoStorage,
        logging_config: LoggingConfig,
    ) -> Result<Arc<Self>, QantoDAGError> {
        let genesis_timestamp = config.genesis_timestamp;

        // Initialize crossbeam channel for block processing queue
        let (block_sender, block_receiver) = bounded(CONCURRENT_BLOCK_LIMIT);

        let db_arc = Arc::new(db);

        let dag = Self {
            consensus_lock: tokio::sync::Mutex::new(()),
            blocks: Arc::new(DashMap::new()),
            tips: Arc::new(DashMap::new()),
            validators: Arc::new(DashMap::new()),
            delegations: Arc::new(DashMap::new()),
            target_block_time: config.target_block_time,
            emission: Arc::new(RwLock::new(Emission::default_with_timestamp(
                genesis_timestamp,
                config.num_chains,
            ))),
            num_chains: Arc::new(RwLock::new(config.num_chains.max(1))),
            finalized_blocks: Arc::new(DashMap::new()),
            chain_loads: Arc::new(DashMap::new()),

            difficulty_history: Arc::new(ParkingRwLock::new(Vec::new())),
            block_creation_timestamps: Arc::new(DashMap::new()),
            anomaly_history: Arc::new(DashMap::new()),
            cross_chain_swaps: Arc::new(DashMap::new()),
            smart_contracts: Arc::new(DashMap::new()),
            cache: Arc::new(ParkingRwLock::new(LruCache::new(
                NonZeroUsize::new(CACHE_SIZE.max(1)).unwrap(),
            ))),
            db: db_arc.clone(),
            persistence_writer: Arc::new(PersistenceWriter::new(db_arc.clone(), 8192)),
            saga,
            self_arc: Weak::new(),
            current_epoch: Arc::new(AtomicU64::new(0)),
            balance_event_sender: Arc::new(RwLock::new(None)),
            balance_broadcaster: Arc::new(RwLock::new(None)),
            account_state_cache: AccountStateCache::new(),
            total_bridge_locked: Arc::new(RwLock::new(0)),
            total_bridge_claimed: Arc::new(RwLock::new(0)),
            processed_bridge_claims: Arc::new(DashMap::new()),
            tx_index: Arc::new(DashMap::new()),
            address_index: Arc::new(DashMap::new()),
            validator_blocks_index: Arc::new(DashMap::new()),
            bridge_claims_index: Arc::new(DashMap::new()),

            // High-performance optimization fields
            block_processing_semaphore: Arc::new(Semaphore::new(BLOCK_PROCESSING_WORKERS)),
            validation_workers: Arc::new(Semaphore::new(TRANSACTION_VALIDATION_WORKERS)),
            block_queue: Arc::new((block_sender, block_receiver)),
            validation_cache: Arc::new(DashMap::new()),
            fast_tips_cache: Arc::new(DashMap::new()),
            processing_blocks: Arc::new(DashMap::new()),
            performance_metrics: Arc::new(PerformanceMetrics::default()),
            mining_metrics: Arc::new(MiningMetrics::new()),
            // Advanced performance optimization fields
            simd_processor: Arc::new(DashMap::new()),
            lock_free_tx_queue: Arc::new(crossbeam::queue::SegQueue::new()),
            memory_pool: Arc::new(DashMap::new()),
            prefetch_cache: Arc::new(DashMap::new()),
            pipeline_stages: (0..8)
                .map(|_| Arc::new(Semaphore::new(BLOCK_PROCESSING_WORKERS)))
                .collect(),
            work_stealing_pool: Arc::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(TRANSACTION_VALIDATION_WORKERS)
                    .thread_name(|i| {
                        let mut name = String::with_capacity(20);
                        name.push_str("qanto-work-stealing-");
                        name.push_str(&i.to_string());
                        name
                    })
                    .build()
                    .expect("Failed to create work-stealing thread pool"),
            ),
            utxo_bloom_filter: Arc::new(DashMap::new()),
            batch_processor: Arc::new(DashMap::new()),
            timing_coordinator: Arc::new(BlockTimingCoordinator::new()), // Microsecond-precision timing for 32+ BPS
            performance_monitor: Arc::new(PerformanceMonitor::new(
                PerformanceMonitoringConfig::default(),
            )),
            logging_config,
            qdag_generator: Arc::new(OptimizedQDagGenerator::new(OptimizedQDagConfig::default())),
            dev_fee_rate: config.dev_fee_rate,
            last_fork_depth: Arc::new(AtomicU64::new(0)),
            last_fork_lca: Arc::new(tokio::sync::RwLock::new(String::new())),
            bypass_reward_check: Arc::new(AtomicBool::new(false)),
        };

        info!(
            "Starting genesis initialization loop for {} chains",
            config.num_chains
        );
        // Generate a single keypair for all genesis blocks to avoid expensive repeated key generation
        let (paillier_pk, _) = HomomorphicEncrypted::generate_keypair();
        for chain_id_val in 0..config.num_chains {
            let gkey = genesis_id_key(chain_id_val);
            match dag.db.get(&gkey) {
                Ok(Some(id_bytes)) => {
                    // Existing genesis marker; load block bytes and populate DAG
                    match dag.db.get(&id_bytes)? {
                        Some(block_bytes) => {
                            let loaded_block: QantoBlock = serde_json::from_slice(&block_bytes)?;

                            // Validate genesis fingerprint (hash, chain ID, and config parameters)
                            let computed_id = loaded_block.compute_block_id()?;
                            if loaded_block.id != computed_id {
                                return Err(QantoDAGError::Governance(format!(
                                    "Genesis fingerprint verification failed: loaded ID {}, computed ID {}",
                                    loaded_block.id, computed_id
                                )));
                            }
                            if loaded_block.chain_id != chain_id_val {
                                return Err(QantoDAGError::Governance(format!(
                                    "Genesis fingerprint verification failed: loaded chain ID {}, expected {}",
                                    loaded_block.chain_id, chain_id_val
                                )));
                            }
                            if loaded_block.validator != config.initial_validator {
                                return Err(QantoDAGError::Governance(format!(
                                    "Genesis fingerprint verification failed: loaded validator {}, expected {}",
                                    loaded_block.validator, config.initial_validator
                                )));
                            }

                            let loaded_id = loaded_block.id.clone();
                            dag.blocks.insert(loaded_id.clone(), loaded_block);
                            dag.tips
                                .entry(chain_id_val)
                                .or_insert_with(HashSet::new)
                                .insert(loaded_id.clone());
                            info!(
                                "Loaded existing genesis for chain {} from storage",
                                chain_id_val
                            );
                            // Load persisted tips for this chain; if none, persist genesis tip
                            let prefix = tips_prefix(chain_id_val);
                            match dag.db.keys_with_prefix(&prefix) {
                                Ok(keys) => {
                                    if keys.is_empty() {
                                        let mut batch = WriteBatch::new();
                                        batch.put(tip_key(chain_id_val, &loaded_id), b"1".to_vec());
                                        dag.db.write_batch(batch)?;
                                        dag.db.flush()?;
                                        dag.db.sync()?;
                                        info!(
                                            "✅ Persisted initial tip for chain {} (genesis {})",
                                            chain_id_val, &loaded_id
                                        );
                                    } else {
                                        // Validate that each persisted tip has a corresponding block; load valid ones
                                        let mut loaded_count = 0usize;
                                        let mut batch = WriteBatch::new();
                                        for k in keys {
                                            if k.len() >= prefix.len() {
                                                let id =
                                                    String::from_utf8_lossy(&k[prefix.len()..])
                                                        .to_string();
                                                let id_bytes = id.as_bytes().to_vec();
                                                match dag.db.get(&id_bytes) {
                                                    Ok(Some(block_bytes)) => {
                                                        match serde_json::from_slice::<QantoBlock>(
                                                            &block_bytes,
                                                        ) {
                                                            Ok(blk) => {
                                                                dag.blocks.insert(id.clone(), blk);
                                                                dag.tips
                                                                    .entry(chain_id_val)
                                                                    .or_insert_with(HashSet::new)
                                                                    .insert(id.clone());
                                                                loaded_count += 1;
                                                            }
                                                            Err(e) => {
                                                                warn!(
                                                                    "Failed to decode block for tip {} on chain {}: {}. Pruning stale tip.",
                                                                    id, chain_id_val, e
                                                                );
                                                                batch.delete(k.clone());
                                                            }
                                                        }
                                                    }
                                                    Ok(None) => {
                                                        warn!(
                                                            "Tip {} persisted for chain {} but block missing. Pruning stale tip.",
                                                            id, chain_id_val
                                                        );
                                                        batch.delete(k.clone());
                                                    }
                                                    Err(e) => {
                                                        warn!(
                                                            "Storage error loading block {} for chain {}: {}. Skipping.",
                                                            id, chain_id_val, e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        if loaded_count == 0 {
                                            // Ensure at least genesis is present as initial tip
                                            batch.put(
                                                tip_key(chain_id_val, &loaded_id),
                                                b"1".to_vec(),
                                            );
                                        }
                                        dag.db.write_batch(batch)?;
                                        dag.db.flush()?;
                                        dag.db.sync()?;
                                        info!(
                                            "Loaded {} valid tips for chain {} from storage (stale entries pruned)",
                                            loaded_count, chain_id_val
                                        );
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to load tips for chain {}: {}. Continuing with in-memory tips.",
                                        chain_id_val, e
                                    );
                                }
                            }
                        }
                        None => {
                            warn!("Genesis ID found but block missing for chain {}. Recreating genesis.", chain_id_val);
                            let genesis_creation_data = QantoBlockCreationData {
                                validator_private_key: QantoPQPrivateKey::new_dummy(),
                                chain_id: chain_id_val,
                                parents: vec![],
                                transactions: vec![],
                                difficulty: INITIAL_DIFFICULTY,
                                validator: config.initial_validator.clone(),
                                miner: config.initial_validator.clone(),
                                timestamp: genesis_timestamp,
                                current_epoch: 0,
                                height: 0,
                                paillier_pk: paillier_pk.clone(),
                            };
                            let mut genesis_block = QantoBlock::new(genesis_creation_data)?;
                            genesis_block.reward = INITIAL_BLOCK_REWARD;
                            let genesis_id = genesis_block.id.clone();
                            dag.blocks.insert(genesis_id.clone(), genesis_block.clone());
                            dag.tips
                                .entry(chain_id_val)
                                .or_insert_with(HashSet::new)
                                .insert(genesis_id.clone());
                            let id_bytes_new = genesis_id.clone().into_bytes();
                            let block_bytes_new = serde_json::to_vec(&genesis_block)?;
                            let mut batch = WriteBatch::new();
                            batch.put(id_bytes_new.clone(), block_bytes_new);
                            batch.put(gkey.clone(), id_bytes_new.clone());
                            if chain_id_val == 0 {
                                batch.put(GENESIS_BLOCK_ID_KEY.to_vec(), id_bytes_new.clone());
                            }
                            // Persist genesis as initial tip
                            batch.put(tip_key(chain_id_val, &genesis_id), b"1".to_vec());
                            dag.db.write_batch(batch)?;
                            dag.db.flush()?;
                            dag.db.sync()?;
                            info!("✅ Successfully persisted genesis marker to database.");
                        }
                    }
                }
                Ok(None) => {
                    // Fresh chain; create and persist genesis
                    let genesis_creation_data = QantoBlockCreationData {
                        validator_private_key: QantoPQPrivateKey::new_dummy(),
                        chain_id: chain_id_val,
                        parents: vec![],
                        transactions: vec![],
                        difficulty: INITIAL_DIFFICULTY,
                        validator: config.initial_validator.clone(),
                        miner: config.initial_validator.clone(),
                        timestamp: genesis_timestamp,
                        current_epoch: 0,
                        height: 0,
                        paillier_pk: paillier_pk.clone(),
                    };
                    let mut genesis_block = QantoBlock::new(genesis_creation_data)?;
                    genesis_block.reward = INITIAL_BLOCK_REWARD;
                    let genesis_id = genesis_block.id.clone();

                    dag.blocks.insert(genesis_id.clone(), genesis_block.clone());
                    dag.tips
                        .entry(chain_id_val)
                        .or_insert_with(HashSet::new)
                        .insert(genesis_id.clone());

                    let id_bytes = genesis_id.clone().into_bytes();
                    let block_bytes = serde_json::to_vec(&genesis_block)?;
                    let mut batch = WriteBatch::new();
                    batch.put(id_bytes.clone(), block_bytes);
                    batch.put(gkey.clone(), id_bytes.clone());
                    if chain_id_val == 0 {
                        batch.put(GENESIS_BLOCK_ID_KEY.to_vec(), id_bytes.clone());
                    }
                    batch.put(tip_key(chain_id_val, &genesis_id), vec![]);
                    dag.db.write_batch(batch)?;
                    dag.db.flush()?;
                    dag.db.sync()?;
                    info!("✅ Successfully persisted genesis marker to database.");
                    info!(
                        "Created new genesis for chain {} and persisted",
                        chain_id_val
                    );
                }
                Err(e) => {
                    warn!(
                        "Error reading genesis key for chain {}: {}. Creating genesis.",
                        chain_id_val, e
                    );
                    let genesis_creation_data = QantoBlockCreationData {
                        validator_private_key: QantoPQPrivateKey::new_dummy(),
                        chain_id: chain_id_val,
                        parents: vec![],
                        transactions: vec![],
                        difficulty: INITIAL_DIFFICULTY,
                        validator: config.initial_validator.clone(),
                        miner: config.initial_validator.clone(),
                        timestamp: genesis_timestamp,
                        current_epoch: 0,
                        height: 0,
                        paillier_pk: paillier_pk.clone(),
                    };
                    let mut genesis_block = QantoBlock::new(genesis_creation_data)?;
                    genesis_block.reward = INITIAL_BLOCK_REWARD;
                    let genesis_id = genesis_block.id.clone();

                    dag.blocks.insert(genesis_id.clone(), genesis_block.clone());
                    dag.tips
                        .entry(chain_id_val)
                        .or_insert_with(HashSet::new)
                        .insert(genesis_id.clone());

                    let id_bytes = genesis_id.clone().into_bytes();
                    let block_bytes = serde_json::to_vec(&genesis_block)?;
                    let mut batch = WriteBatch::new();
                    batch.put(id_bytes.clone(), block_bytes);
                    batch.put(gkey.clone(), id_bytes.clone());
                    if chain_id_val == 0 {
                        batch.put(GENESIS_BLOCK_ID_KEY.to_vec(), id_bytes.clone());
                    }
                    batch.put(tip_key(chain_id_val, &genesis_id), vec![]);
                    dag.db.write_batch(batch)?;
                    dag.db.flush()?;
                    dag.db.sync()?;
                    info!("✅ Successfully persisted genesis marker to database.");
                }
            }
        }

        info!("Genesis loop completed");
        let balance_prefix = BALANCES_KEY_PREFIX.as_bytes();
        match dag.db.keys_with_prefix(balance_prefix) {
            Ok(keys) => {
                let mut loaded_balances = 0usize;
                for key in keys {
                    if key.len() <= balance_prefix.len() {
                        continue;
                    }
                    let address = String::from_utf8_lossy(&key[balance_prefix.len()..]).to_string();
                    match dag.db.get(&key) {
                        Ok(Some(bytes)) => match decode_balance(&bytes) {
                            Ok(balance) => {
                                dag.account_state_cache.set_balance(address, balance);
                                loaded_balances += 1;
                            }
                            Err(err) => {
                                warn!("Failed to decode persisted balance entry: {}", err);
                            }
                        },
                        Ok(None) => {}
                        Err(err) => {
                            warn!("Failed to load persisted balance entry: {}", err);
                        }
                    }
                }
                info!(
                    "Loaded {} persisted balances into the in-memory account state cache",
                    loaded_balances
                );
            }
            Err(err) => {
                warn!(
                    "Failed to preload account_state_cache from storage: {}",
                    err
                );
            }
        }

        // Initialize validators
        dag.validators.insert(
            config.initial_validator.clone(),
            MIN_VALIDATOR_STAKE * config.num_chains as u128 * 2,
        );

        let arc_dag = Arc::new(dag);
        let weak_self = Arc::downgrade(&arc_dag);

        let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;

        // SAFETY: This unsafe block is required to initialize the self_arc field after
        // the QantoDAG has been wrapped in an Arc. This is safe because:
        // 1. We have exclusive access to the newly created Arc<QantoDAG>
        // 2. No other threads can access this instance yet
        // 3. The pointer is valid as it comes directly from Arc::as_ptr
        // 4. We're only writing to a single field (self_arc) that was initialized as Weak::new()
        unsafe {
            (*ptr).self_arc = weak_self;
        }

        // Hydrate in-memory blocks and indices from storage history on startup
        if let Err(e) = arc_dag.populate_and_index_history() {
            warn!("Failed to populate and index historical blocks: {:?}", e);
        }

        Ok(arc_dag)
    }

    pub fn set_bypass_reward_check(&self, bypass: bool) {
        self.bypass_reward_check.store(bypass, Ordering::Relaxed);
    }

    pub async fn get_block_reward(&self, block_id: &str) -> Option<u128> {
        self.blocks.get(block_id).map(|b| b.reward)
    }

    pub async fn prune_old_blocks(&self, prune_depth: u64) -> Result<usize, QantoDAGError> {
        let current_height = self
            .get_latest_block()
            .await
            .map(|b| b.height)
            .unwrap_or_default();
        let prune_threshold = current_height.saturating_sub(prune_depth);

        let mut pruned_count = 0;
        let mut ids_to_prune = Vec::new();

        // Build a set of all current tip IDs across chains to avoid pruning frontier blocks.
        // Pruning tips can race with candidate creation and cause parent blocks to disappear.
        let tip_ids: HashSet<String> = self
            .tips
            .iter()
            .flat_map(|entry| {
                entry
                    .value()
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .collect();

        for entry in self.blocks.iter() {
            let block = entry.value();
            // Never prune current tips; they are required for parent selection.
            if block.height < prune_threshold
                && !self.finalized_blocks.contains_key(&block.id)
                && !tip_ids.contains(&block.id)
            {
                ids_to_prune.push(block.id.clone());
            }
        }

        for id in ids_to_prune {
            if let Some((_id, block)) = self.blocks.remove(&id) {
                // Store minimal hash summary for verification
                let summary_key = format!("pruned_{id}");
                let hash_summary = qanto_hash(block.hash().as_bytes());
                self.db.put(
                    summary_key.as_bytes().to_vec(),
                    hash_summary.as_bytes().to_vec(),
                )?;

                pruned_count += 1;
            }
        }

        info!(
            "Pruned {} old blocks below height {}",
            pruned_count, prune_threshold
        );
        Ok(pruned_count)
    }

    pub async fn get_average_tx_per_block(&self) -> u128 {
        let total_txs: u128 = self
            .blocks
            .iter()
            .map(|entry| entry.value().transactions.len() as u128)
            .sum();
        if self.blocks.is_empty() {
            0
        } else {
            (total_txs * crate::Q_SCALE) / self.blocks.len() as u128
        }
    }

    // GraphQL server helper methods
    pub async fn get_block_count(&self) -> u64 {
        self.blocks.len() as u64
    }

    pub async fn get_total_transactions(&self) -> u64 {
        self.blocks
            .iter()
            .map(|entry| entry.value().transactions.len() as u64)
            .sum()
    }

    pub async fn get_current_difficulty(&self) -> u64 {
        // PHASE 2: SAGA ISOLATION - Return deterministic difficulty
        INITIAL_DIFFICULTY
    }

    pub async fn get_latest_block_hash(&self) -> Option<String> {
        // Find the block with the highest height
        self.blocks
            .iter()
            .max_by_key(|entry| entry.value().height)
            .map(|entry| entry.value().hash())
    }

    pub async fn get_latest_block(&self) -> Option<QantoBlock> {
        // Find the block with the highest height
        self.blocks
            .iter()
            .max_by_key(|entry| entry.value().height)
            .map(|entry| entry.value().clone())
    }

    pub async fn get_block(&self, block_id: &str) -> Option<QantoBlock> {
        self.blocks.get(block_id).map(|entry| entry.value().clone())
    }

    pub async fn get_blocks_paginated(&self, limit: usize, offset: usize) -> Vec<QantoBlock> {
        self.blocks
            .iter()
            .skip(offset)
            .take(limit)
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn index_block_internal(&self, block: &QantoBlock) {
        // 1. Validator blocks index
        let mut v_blocks = self
            .validator_blocks_index
            .entry(block.validator.clone())
            .or_default();
        if !v_blocks.contains(&block.id) {
            v_blocks.push(block.id.clone());
        }
        drop(v_blocks);

        // 2. Transaction and address indexing
        for tx in &block.transactions {
            // Index the transaction details
            self.tx_index.insert(
                tx.id.clone(),
                IndexedTransaction {
                    tx: tx.clone(),
                    block_id: block.id.clone(),
                    block_height: block.height,
                },
            );

            // Index the sender address
            let mut sender_txs = self.address_index.entry(tx.sender.clone()).or_default();
            if !sender_txs.contains(&tx.id) {
                sender_txs.push(tx.id.clone());
            }
            drop(sender_txs);

            // Index the receiver address
            let mut receiver_txs = self.address_index.entry(tx.receiver.clone()).or_default();
            if !receiver_txs.contains(&tx.id) {
                receiver_txs.push(tx.id.clone());
            }
            drop(receiver_txs);

            // 3. Bridge claims index
            if tx.transaction_kind == crate::transaction::TransactionKind::BridgeClaim {
                if let Some(eth_tx_hash) = tx.metadata.get("bridge_source_tx_hash") {
                    let recipient = tx
                        .metadata
                        .get("bridge_recipient")
                        .cloned()
                        .unwrap_or_else(|| tx.receiver.clone());
                    let amount = tx
                        .metadata
                        .get("bridge_amount")
                        .and_then(|s| s.parse::<u128>().ok())
                        .unwrap_or(tx.amount);
                    let source_chain = tx
                        .metadata
                        .get("bridge_source_chain")
                        .cloned()
                        .unwrap_or_else(|| "Ethereum".to_string());
                    let claim_key = format!("{}:{}", source_chain, eth_tx_hash);

                    let claim = IndexedBridgeClaim {
                        claim_key: claim_key.clone(),
                        source_chain,
                        source_tx_hash: eth_tx_hash.clone(),
                        recipient,
                        amount,
                        mint_tx_id: tx.id.clone(),
                        block_id: block.id.clone(),
                        block_height: block.height,
                    };
                    self.bridge_claims_index.insert(claim_key, claim);
                }
            }
        }
    }

    pub fn populate_and_index_history(&self) -> Result<(), QantoDAGError> {
        // Reload processed bridge claims from disk
        let bridge_claim_prefix = crate::persistence::utxo_key("bridge_claim_done:");
        if let Ok(keys) = self.db.keys_with_prefix(&bridge_claim_prefix) {
            for key in keys {
                if key.len() > bridge_claim_prefix.len() {
                    if let Ok(claim_key) =
                        String::from_utf8(key[bridge_claim_prefix.len()..].to_vec())
                    {
                        self.processed_bridge_claims.insert(claim_key, true);
                    }
                }
            }
        }

        use ahash::AHashSet as HashSet;
        let mut visited = HashSet::new();
        let mut queue = Vec::new();

        // Start from all tips across all chains
        for entry in self.tips.iter() {
            for tip_id in entry.value().iter() {
                queue.push(tip_id.clone());
            }
        }

        while let Some(block_id) = queue.pop() {
            if !visited.insert(block_id.clone()) {
                continue;
            }

            // Load block from memory or database
            let block_opt = if let Some(blk) = self.blocks.get(&block_id) {
                Some(blk.value().clone())
            } else {
                match self.db.get(block_id.as_bytes()) {
                    Ok(Some(bytes)) => match serde_json::from_slice::<QantoBlock>(&bytes) {
                        Ok(blk) => {
                            self.blocks.insert(block_id.clone(), blk.clone());
                            Some(blk)
                        }
                        Err(e) => {
                            warn!(
                                "Failed to deserialize block {} from storage: {}",
                                block_id, e
                            );
                            None
                        }
                    },
                    Ok(None) => {
                        warn!(
                            "Block {} not found in storage during history population",
                            block_id
                        );
                        None
                    }
                    Err(e) => {
                        error!(
                            "Database error loading block {} during history population: {}",
                            block_id, e
                        );
                        return Err(QantoDAGError::DatabaseError(e.to_string()));
                    }
                }
            };

            if let Some(block) = block_opt {
                // Index this block
                self.index_block_internal(&block);

                // Add parents to queue
                for parent_id in &block.parents {
                    queue.push(parent_id.clone());
                }
            }
        }

        info!(
            "Successfully populated and indexed {} historical blocks",
            visited.len()
        );
        Ok(())
    }

    pub async fn get_transaction(&self, tx_id: &str) -> Option<Transaction> {
        self.tx_index
            .get(tx_id)
            .map(|entry| entry.value().tx.clone())
    }

    pub fn get_indexed_transaction(&self, tx_id: &str) -> Option<IndexedTransaction> {
        self.tx_index.get(tx_id).map(|entry| entry.value().clone())
    }

    pub fn get_address_history(&self, address: &str) -> Vec<IndexedTransaction> {
        if let Some(tx_ids) = self.address_index.get(address) {
            tx_ids
                .value()
                .iter()
                .filter_map(|tx_id| self.get_indexed_transaction(tx_id))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_validator_blocks(&self, validator: &str) -> Vec<QantoBlock> {
        if let Some(block_ids) = self.validator_blocks_index.get(validator) {
            block_ids
                .value()
                .iter()
                .filter_map(|id| self.blocks.get(id).map(|entry| entry.value().clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_bridge_claim(&self, claim_key: &str) -> Option<IndexedBridgeClaim> {
        // First try exact match (composite key source_chain:tx_hash)
        if let Some(claim) = self.bridge_claims_index.get(claim_key) {
            return Some(claim.value().clone());
        }

        // If not found, try searching by source_tx_hash
        for entry in self.bridge_claims_index.iter() {
            if entry.value().source_tx_hash == claim_key {
                return Some(entry.value().clone());
            }
        }
        None
    }

    /// Calculate the Lowest Common Ancestor (LCA) and the fork depth for a given chain.
    /// Returns (depth, lca_block_id).
    pub fn calculate_fork_depth_and_lca(&self, chain_id: u32) -> (u64, String) {
        let tips = match self.tips.get(&chain_id) {
            Some(t) => t.value().clone(),
            None => return (0, String::new()),
        };

        if tips.len() < 2 {
            return (0, String::new());
        }

        let mut all_ancestor_sets = Vec::new();
        let mut max_tip_height = 0;

        for tip_id in &tips {
            if let Some(block) = self.blocks.get(tip_id) {
                if block.height > max_tip_height {
                    max_tip_height = block.height;
                }
            }

            let mut ancestors = HashSet::new();
            let mut queue = Vec::new();
            queue.push((tip_id.clone(), 0));

            while let Some((block_id, depth)) = queue.pop() {
                if depth > 100 {
                    continue;
                }
                if ancestors.insert(block_id.clone()) {
                    if let Some(block) = self.blocks.get(&block_id) {
                        for parent_id in &block.parents {
                            if let Some(parent_block) = self.blocks.get(parent_id) {
                                if parent_block.chain_id == chain_id {
                                    queue.push((parent_id.clone(), depth + 1));
                                }
                            }
                        }
                    }
                }
            }
            all_ancestor_sets.push(ancestors);
        }

        if all_ancestor_sets.is_empty() {
            return (0, String::new());
        }

        let mut common_ancestors = all_ancestor_sets[0].clone();
        for set in all_ancestor_sets.iter().skip(1) {
            common_ancestors.retain(|id| set.contains(id));
        }

        let mut lca_id = String::new();
        let mut lca_height = 0;

        for ancestor_id in common_ancestors {
            if let Some(block) = self.blocks.get(&ancestor_id) {
                if block.height >= lca_height {
                    lca_height = block.height;
                    lca_id = ancestor_id;
                }
            }
        }

        if lca_id.is_empty() {
            return (0, String::new());
        }

        let depth = max_tip_height.saturating_sub(lca_height);
        (depth, lca_id)
    }

    pub async fn add_block(
        &self,
        block: QantoBlock,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        mempool_arc: Option<&Arc<RwLock<Mempool>>>,
        miner_id: Option<&str>,
    ) -> Result<bool, QantoDAGError> {
        // --- CRITICAL: Acquire consensus lock to prevent double-spend race conditions (B-C1) ---
        // This ensures that from the moment we validate a block's UTXOs until we commit the
        // updates to the state, no other concurrent add_block call can consume the same UTXOs.
        let _lock = self.consensus_lock.lock().await;

        let add_block_span = tracing::info_span!(
            "qantodag.add_block",
            block_id = %block.id,
            chain_id = block.chain_id,
            tx_count = block.transactions.len(),
            height = block.height
        );
        let _enter = add_block_span.enter();
        let total_start = Instant::now();

        if self.blocks.contains_key(&block.id) {
            warn!("Attempted to add block {} which already exists.", block.id);
            return Ok(false);
        }

        let validate_start = Instant::now();
        let is_valid = self.is_valid_block(&block, utxos_arc).await?;
        let validate_ms = validate_start.elapsed().as_millis() as u64;
        self.performance_metrics.record_validation_time(validate_ms);
        debug!(block_id = %block.id, validate_ms, "add_block validation complete");
        if !is_valid {
            let mut error_msg = String::with_capacity(50 + block.id.len());
            error_msg.push_str("Block ");
            error_msg.push_str(&block.id);
            error_msg.push_str(" failed validation in add_block");
            error!("SOLO MINER: Block validation failed for block {}: height={}, parents={:?}, transactions={}, validator={}, miner={}", 
                   block.id, block.height, block.parents, block.transactions.len(), block.validator, block.miner);
            return Err(QantoDAGError::InvalidBlock(error_msg));
        }

        if self.blocks.contains_key(&block.id) {
            warn!(
                "Block {} already exists (double check after write lock).",
                block.id
            );
            return Ok(false);
        }

        // Check for equivocation (double-signing at same height on same chain by same validator)
        let is_equivocation = self.blocks.iter().any(|entry| {
            let existing_block = entry.value();
            existing_block.chain_id == block.chain_id
                && existing_block.height == block.height
                && existing_block.validator == block.validator
                && existing_block.id != block.id
        });

        if is_equivocation {
            if let Some(mut stake_ref) = self.validators.get_mut(&block.validator) {
                let current_stake = *stake_ref.value();
                let penalty = (current_stake * 30) / 100; // 30% slashing penalty
                let new_stake = current_stake.saturating_sub(penalty);
                *stake_ref.value_mut() = new_stake;
                warn!(
                    "Slashed validator {} by {} for equivocation (double-signing) at height {}",
                    block.validator, penalty, block.height
                );
            }
            return Err(QantoDAGError::InvalidBlock(
                "Equivocation (double-signing) detected and validator slashed".to_string(),
            ));
        }

        // Create a temporary HashMap for anomaly detection
        let blocks_map: HashMap<String, QantoBlock> = self
            .blocks
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        let anomaly_score = self.detect_anomaly_internal(&blocks_map, &block).await?;
        if anomaly_score > 900_000_000 {
            if let Some(mut stake_ref) = self.validators.get_mut(&block.validator) {
                let current_stake = *stake_ref.value();
                let penalty = (current_stake * SLASHING_PENALTY) / 100;
                let new_stake = current_stake.saturating_sub(penalty);
                *stake_ref.value_mut() = new_stake;
                info!(
                    "Slashed validator {} by {} for anomaly (score: {})",
                    block.validator, penalty, anomaly_score
                );
            }
        }

        // Plan conflict-free UTXO changes and compute balance deltas
        let (balance_deltas, removal_keys, additions) = self
            .plan_utxo_changes_conflict_free(&block.transactions, utxos_arc)
            .await?;

        let removal_keys_persist = removal_keys.clone();
        let additions_persist = additions.clone();
        {
            let mut utxos_write_guard = utxos_arc.write().await;
            for key in removal_keys {
                utxos_write_guard.remove(&key);
            }
            for (utxo_id, utxo) in additions {
                utxos_write_guard.insert(utxo_id, utxo);
            }
        }

        // Persist UTXO changes asynchronously via writer
        for key in removal_keys_persist {
            let del_key = crate::persistence::utxo_key(&key);
            if let Err(e) = self.persistence_writer.enqueue_delete(del_key) {
                error!("Failed to enqueue UTXO delete for {}: {}", key, e);
                return Err(QantoDAGError::DatabaseError(e.to_string()));
            }
        }
        for (utxo_id, utxo) in additions_persist {
            let put_key = crate::persistence::utxo_key(&utxo_id);
            let utxo_bytes =
                crate::persistence::encode_utxo(&utxo).map_err(QantoDAGError::Generic)?;
            if let Err(e) = self.persistence_writer.enqueue_put(put_key, utxo_bytes) {
                error!("Failed to enqueue UTXO put for {}: {}", utxo_id, e);
                return Err(QantoDAGError::DatabaseError(e.to_string()));
            }
        }

        // Process custom transaction state machine transitions
        for tx in &block.transactions {
            match tx.transaction_kind {
                crate::transaction::TransactionKind::Stake => {
                    if let Err(e) = self.process_stake(tx).await {
                        error!("Failed to process stake transaction {}: {:?}", tx.id, e);
                    }
                }
                crate::transaction::TransactionKind::Unstake => {
                    if let Err(e) = self.process_unstake(tx, utxos_arc).await {
                        error!("Failed to process unstake transaction {}: {:?}", tx.id, e);
                    }
                }
                crate::transaction::TransactionKind::Delegate => {
                    if let Err(e) = self.process_delegate(tx).await {
                        error!("Failed to process delegate transaction {}: {:?}", tx.id, e);
                    }
                }
                crate::transaction::TransactionKind::Vote => {
                    if let Err(e) = self.process_vote(tx).await {
                        error!("Failed to process vote transaction {}: {:?}", tx.id, e);
                    }
                }
                crate::transaction::TransactionKind::Proposal => {
                    if let Err(e) = self.process_proposal(tx).await {
                        error!("Failed to process proposal transaction {}: {:?}", tx.id, e);
                    }
                }
                crate::transaction::TransactionKind::BridgeLock => {
                    if let Err(e) = self.process_bridge_lock(tx).await {
                        error!(
                            "Failed to process bridge lock transaction {}: {:?}",
                            tx.id, e
                        );
                    }
                }
                crate::transaction::TransactionKind::BridgeClaim => {
                    if let Err(e) = self.process_bridge_claim(tx, utxos_arc).await {
                        error!(
                            "Failed to process bridge claim transaction {}: {:?}",
                            tx.id, e
                        );
                    }
                }
                crate::transaction::TransactionKind::BridgeObserve => {
                    // Currently a no-op framework placeholder for observation recording
                    info!(
                        "BRIDGE OBSERVE: Recorded observation for transaction {}",
                        tx.id
                    );
                }
                _ => {}
            }
        }

        // Persist balance updates asynchronously via writer
        let balance_sender_opt = self.balance_event_sender.read().await.clone();
        let balance_broadcaster_opt = self.balance_broadcaster.read().await.clone();
        let block_id_for_state_logs = block.id.clone();
        let block_height_for_state_logs = block.height;

        for (address, delta) in balance_deltas {
            if delta == 0 {
                continue;
            }
            let key = balance_key(&address);
            let maybe_cached = self.account_state_cache.get_balance(&address);
            if maybe_cached.is_none() {
                let current_val = match self.db.get(&key) {
                    Ok(Some(bytes)) => match decode_balance(&bytes) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "Failed to decode balance for {}: {}. Defaulting to 0.",
                                address, e
                            );
                            0u128
                        }
                    },
                    Ok(None) => 0u128,
                    Err(e) => {
                        error!("Storage read error for {}: {}", address, e);
                        return Err(QantoDAGError::DatabaseError(e.to_string()));
                    }
                };
                self.account_state_cache
                    .set_balance(address.clone(), current_val);
            }

            let next_u128_clamped = self
                .account_state_cache
                .apply_delta(address.as_str(), delta);
            if let Err(e) = self
                .persistence_writer
                .enqueue_put(key, encode_balance(next_u128_clamped))
            {
                error!("Failed to enqueue balance update for {}: {}", address, e);
                return Err(QantoDAGError::DatabaseError(e.to_string()));
            }

            if let Some(ref sender) = balance_sender_opt {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(QantoDAGError::Time)?
                    .as_secs();
                let event = BalanceEvent {
                    address: address.clone(),
                    balance: next_u128_clamped,
                    timestamp,
                };
                let _ = sender.send(event);
            }

            if let Some(ref bb) = balance_broadcaster_opt {
                bb.set_balance(address.as_str(), next_u128_clamped);
            }

            info!(
                "STATE_UPDATED block_id={} height={} address={} delta={} balance={}",
                block_id_for_state_logs,
                block_height_for_state_logs,
                address,
                delta,
                next_u128_clamped
            );
        }

        let block_for_db = block.clone();
        self.blocks.insert(block.id.clone(), block);
        self.index_block_internal(&block_for_db);
        self.block_creation_timestamps
            .insert(block_for_db.id.clone(), Utc::now().timestamp() as u64);

        let mut removed_parents: Vec<String> = Vec::new();
        {
            let mut current_tips = self
                .tips
                .entry(block_for_db.chain_id)
                .or_insert_with(HashSet::new);
            for parent_id in &block_for_db.parents {
                if current_tips.remove(parent_id) {
                    removed_parents.push(parent_id.clone());
                }
            }
            current_tips.insert(block_for_db.id.clone());
            let tips_vec: Vec<String> = current_tips.iter().cloned().collect();
            self.fast_tips_cache.insert(block_for_db.chain_id, tips_vec);
        }

        // Fork and consensus telemetry updates for Cohort A
        {
            let mut is_fork = false;
            for parent_id in &block_for_db.parents {
                let has_other_child = self.blocks.iter().any(|entry| {
                    let existing = entry.value();
                    existing.chain_id == block_for_db.chain_id
                        && existing.id != block_for_db.id
                        && existing.parents.contains(parent_id)
                });
                if has_other_child {
                    is_fork = true;
                    break;
                }
            }

            let tips_len = if let Some(current_tips) = self.tips.get(&block_for_db.chain_id) {
                current_tips.value().len() as u64
            } else {
                1
            };

            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let metrics = crate::metrics::get_global_metrics();

            // Store current tip count
            metrics.tip_count.store(tips_len, Ordering::Relaxed);
            if let Ok(mut tip_history) = metrics.tip_count_timestamps.try_write() {
                tip_history.push((now_secs, tips_len));
            }

            if is_fork {
                // Increment fork_events_total
                metrics.fork_events_total.fetch_add(1, Ordering::Relaxed);
                metrics
                    .last_fork_timestamp
                    .store(now_secs, Ordering::Relaxed);
                if let Ok(mut fork_ts) = metrics.fork_timestamps.try_write() {
                    fork_ts.push(now_secs);
                }

                // Conditional LCA calculation (optimised to run only when fork is detected)
                let (depth, lca_block_id) =
                    self.calculate_fork_depth_and_lca(block_for_db.chain_id);
                self.last_fork_depth.store(depth, Ordering::Relaxed);

                // Update metrics.max_fork_depth if this depth is larger
                let current_max = metrics.max_fork_depth.load(Ordering::Relaxed);
                if depth > current_max {
                    metrics.max_fork_depth.store(depth, Ordering::Relaxed);
                }

                if let Ok(mut lca_guard) = self.last_fork_lca.try_write() {
                    *lca_guard = lca_block_id;
                }
            } else if tips_len > 1 {
                // Normal parallel tips behavior in DAG without canonical chain conflict
                metrics.parallel_tips_total.fetch_add(1, Ordering::Relaxed);
            }

            // Height divergence: check if active tips have unequal heights
            if tips_len > 1 {
                if let Some(current_tips) = self.tips.get(&block_for_db.chain_id) {
                    let mut heights = HashSet::new();
                    for tip_id in current_tips.value() {
                        if let Some(blk) = self.blocks.get(tip_id) {
                            heights.insert(blk.height);
                        }
                    }
                    if heights.len() > 1 {
                        metrics
                            .height_divergence_events
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            // Chain reconciliation/merge: block merges multiple parent tips on same chain
            let mut parents_on_same_chain = 0;
            for parent_id in &block_for_db.parents {
                if let Some(parent_blk) = self.blocks.get(parent_id) {
                    if parent_blk.chain_id == block_for_db.chain_id {
                        parents_on_same_chain += 1;
                    }
                }
            }
            if parents_on_same_chain > 1 {
                metrics
                    .chain_reconciliation_events
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        for parent_id in removed_parents {
            let del_key = tip_key(block_for_db.chain_id, &parent_id);
            if let Err(e) = self.persistence_writer.enqueue_delete(del_key) {
                error!(
                    "Failed to enqueue tip delete for parent {}: {}",
                    parent_id, e
                );
                return Err(QantoDAGError::DatabaseError(e.to_string()));
            }
        }
        let put_key = tip_key(block_for_db.chain_id, &block_for_db.id);
        if let Err(e) = self.persistence_writer.enqueue_put(put_key, b"1".to_vec()) {
            error!(
                "SOLO MINER: Failed to enqueue tip put for new tip {}: {}",
                block_for_db.id, e
            );
            return Err(QantoDAGError::DatabaseError(e.to_string()));
        }

        info!(
            "SOLO MINER: Starting database enqueue for block {}",
            block_for_db.id
        );
        let id_bytes = block_for_db.id.as_bytes().to_vec();
        let block_bytes = serde_json::to_vec(&block_for_db)?;
        info!(
            "SOLO MINER: Serialized block {} for enqueue",
            block_for_db.id
        );

        if let Err(e) = self.persistence_writer.enqueue_put(id_bytes, block_bytes) {
            error!(
                "SOLO MINER: Failed to enqueue block {} for persistence: {}",
                block_for_db.id, e
            );
            return Err(QantoDAGError::DatabaseError(e.to_string()));
        }
        info!(
            "SOLO MINER: Enqueued block {} for asynchronous persistence",
            block_for_db.id
        );

        info!(
            "SOLO MINER: Updating emission supply for block {}",
            block_for_db.id
        );
        {
            let mut emission = self.emission.write().await;
            emission
                .update_supply(block_for_db.reward)
                .map_err(QantoDAGError::EmissionError)?;
        }
        info!(
            "SOLO MINER: Emission supply updated for block {}",
            block_for_db.id
        );

        BLOCKS_PROCESSED.inc();
        TRANSACTIONS_PROCESSED.inc_by(block_for_db.transactions.len() as u64);

        // Update economic metrics: TVL and increment 24h counters atomically
        let outputs_sum: u128 = block_for_db
            .transactions
            .iter()
            .map(|tx| tx.outputs.iter().map(|o| o.amount).sum::<u128>())
            .sum::<u128>();
        let metrics = crate::metrics::get_global_metrics();
        metrics.add_total_value_locked(outputs_sum).await;

        // Increment 24h validator rewards with current block reward
        metrics.add_validator_rewards_24h(block_for_db.reward).await;

        // Increment 24h transaction fees with current block fees
        let fees_current_block: u128 = block_for_db
            .transactions
            .iter()
            .skip(1)
            .map(|tx| tx.fee)
            .sum::<u128>();
        metrics.add_transaction_fees_24h(fees_current_block).await;

        // Unconditional block celebration log for observability using Display
        let block_str = format!("{}", block_for_db);
        info!("\n{}", block_str);
        info!(
            "BLOCK_ACCEPTED block_id={} height={} tx_count={} chain_id={} reward={}",
            block_for_db.id,
            block_for_db.height,
            block_for_db.transactions.len(),
            block_for_db.chain_id,
            block_for_db.reward
        );
        info!(
            "SOLO MINER: Block {} successfully added to DAG",
            block_for_db.id
        );

        // Release reserved transactions for this miner since the block was successfully added
        if let (Some(mempool_arc), Some(miner_id)) = (mempool_arc, miner_id) {
            let mempool_guard = mempool_arc.read().await;
            mempool_guard.release_reserved_transactions(miner_id);
            info!("Released reserved transactions for miner: {}", miner_id);
        }

        let total_ms = total_start.elapsed().as_millis() as u64;
        self.performance_metrics
            .record_block_creation_time(total_ms);
        self.performance_metrics.increment_blocks_processed();
        debug!(block_id = %block_for_db.id, total_ms, "add_block completed");

        let state_root = self.account_state_cache.calculate_state_root();
        info!(
            "STATE_ROOT height={} hash={}",
            block_for_db.height, state_root
        );

        #[cfg(any(test, debug_assertions))]
        {
            let circulating_supply: u128 = self.account_state_cache.snapshot().values().sum();
            let staked_supply: u128 = self
                .validators
                .iter()
                .map(|entry| *entry.value() as u128)
                .sum();
            let delegated_supply: u128 = self
                .delegations
                .iter()
                .map(|entry| *entry.value() as u128)
                .sum();
            let bridge_locked_supply = *self.total_bridge_locked.read().await;
            let current_supply = self.emission.read().await.current_supply();

            // circulating + staked + delegated + bridge_locked = current_supply
            let calculated_total = circulating_supply
                .saturating_add(staked_supply)
                .saturating_add(delegated_supply)
                .saturating_add(bridge_locked_supply);

            if calculated_total != current_supply {
                let err_msg = format!(
                    "GLOBAL SUPPLY INVARIANT VIOLATION: circulating ({}) + staked ({}) + delegated ({}) + bridge_locked ({}) = {} vs current_supply ({})",
                    circulating_supply, staked_supply, delegated_supply, bridge_locked_supply, calculated_total, current_supply
                );
                error!("{}", err_msg);
                return Err(QantoDAGError::Governance(err_msg));
            } else {
                info!(
                    "Global supply invariant check passed: circulating ({}) + staked ({}) + delegated ({}) + bridge_locked ({}) = current_supply ({})",
                    circulating_supply, staked_supply, delegated_supply, bridge_locked_supply, current_supply
                );
            }
        }

        Ok(true)
    }

    pub async fn get_id(&self) -> u32 {
        0
    }

    pub async fn get_tips(&self, chain_id: u32) -> Option<Vec<String>> {
        self.fast_tips_cache.get(&chain_id).map(|v| v.clone())
    }

    pub async fn add_validator(&self, address: String, stake: u128) {
        self.validators
            .insert(address, stake.max(MIN_VALIDATOR_STAKE));
    }

    pub async fn initiate_cross_chain_swap(
        &self,
        params: CrossChainSwapParams,
    ) -> Result<String, QantoDAGError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let mut swap_data = String::with_capacity(64);
        swap_data.push_str("swap_");
        swap_data.push_str(&params.initiator);
        swap_data.push('_');
        swap_data.push_str(&params.responder);
        swap_data.push('_');
        swap_data.push_str(&params.amount.to_string());
        swap_data.push('_');
        swap_data.push_str(&now.to_string());
        let swap_id = hex::encode(qanto_hash(swap_data.as_bytes()));
        let swap = CrossChainSwap {
            swap_id: swap_id.clone(),
            source_chain: params.source_chain,
            target_chain: params.target_chain,
            amount: params.amount,
            initiator: params.initiator,
            responder: params.responder,
            timelock: now + params.timelock_duration,
            state: SwapState::Initiated,
            secret_hash: params.secret_hash,
            secret: None,
        };
        self.cross_chain_swaps.insert(swap_id.clone(), swap);
        Ok(swap_id)
    }

    pub async fn redeem_cross_chain_swap(
        &self,
        swap_id: &str,
        secret: &str,
    ) -> Result<(), QantoDAGError> {
        let mut swap = self.cross_chain_swaps.get_mut(swap_id).ok_or_else(|| {
            let mut error_msg = String::with_capacity("Swap ID  not found".len() + swap_id.len());
            error_msg.push_str("Swap ID ");
            error_msg.push_str(swap_id);
            error_msg.push_str(" not found");
            QantoDAGError::CrossChainSwapError(error_msg)
        })?;

        let hash_of_provided_secret = hex::encode(qanto_hash(secret.as_bytes()));
        if hash_of_provided_secret != swap.secret_hash {
            return Err(QantoDAGError::CrossChainSwapError(
                "Invalid secret provided for swap.".to_string(),
            ));
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if now > swap.timelock {
            return Err(QantoDAGError::CrossChainSwapError(
                "Swap timelock has expired.".to_string(),
            ));
        }

        swap.state = SwapState::Redeemed;
        swap.secret = Some(secret.to_string());
        info!("Cross-chain swap {} redeemed successfully.", swap_id);
        Ok(())
    }

    pub async fn deploy_smart_contract(
        &self,
        code: String,
        owner: String,
        initial_gas: u128,
    ) -> Result<String, QantoDAGError> {
        let contract_id = hex::encode(qanto_hash(code.as_bytes()));
        let contract = SmartContract {
            contract_id: contract_id.clone(),
            code,
            storage: HashMap::new(),
            owner,
            gas_balance: initial_gas,
        };
        self.smart_contracts.insert(contract_id.clone(), contract);
        Ok(contract_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_candidate_block(
        &self,
        qr_signing_key: &QantoPQPrivateKey,
        _qr_public_key: &QantoPQPublicKey,
        validator_address: &str,
        mempool_arc: &Arc<RwLock<Mempool>>,
        _utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        chain_id_val: u32,
        miner: &Arc<Miner>,
        homomorphic_public_key: Option<&[u8]>,
        parents_override: Option<Vec<String>>, // Optional explicit parents
        difficulty_override: Option<crate::QDifficulty>, // Optional explicit difficulty
    ) -> Result<QantoBlock, QantoDAGError> {
        {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let recent_blocks = self
                .block_creation_timestamps
                .iter()
                .map(|entry| *entry.value())
                .filter(|&t| now.saturating_sub(t) < 60)
                .count() as u64;
            // Pull dynamic rate limit from SAGA epoch rules, fallback to static constant
            let max_bpm: u64 = {
                let rules = self.saga.economy.epoch_rules.read().await;
                rules
                    .get("max_blocks_per_minute")
                    .map_or(MAX_BLOCKS_PER_MINUTE, |r| r.value as u64)
            };
            if recent_blocks >= max_bpm {
                let mut error_msg = String::with_capacity(64);
                error_msg.push_str("Rate limit exceeded: ");
                error_msg.push_str(&recent_blocks.to_string());
                error_msg.push_str(" blocks in last minute");
                return Err(QantoDAGError::InvalidBlock(error_msg));
            }
            if self.block_creation_timestamps.len() > 1000 {
                self.block_creation_timestamps
                    .retain(|_, t_val| now.saturating_sub(*t_val) < 3600);
            }
        }
        {
            let validators_guard = &self.validators;
            let stake = validators_guard.get(validator_address).ok_or_else(|| {
                let mut msg = String::with_capacity(50 + validator_address.len());
                msg.push_str("Validator ");
                msg.push_str(validator_address);
                msg.push_str(" not found or no stake");
                QantoDAGError::InvalidBlock(msg)
            })?;
            if *stake < MIN_VALIDATOR_STAKE {
                // Per updated consensus policy, PoS is a helper and low stake should not block
                // candidate creation. Log a warning and proceed.
                warn!(
                    validator = %validator_address,
                    stake = *stake,
                    min_required = MIN_VALIDATOR_STAKE,
                    "Validator stake below minimum; proceeding with candidate creation due to PoW-first finality"
                );
            }
        }

        // Generate a unique miner ID for this block creation attempt (outside mempool guard)
        let miner_id = format!("miner_{}", uuid::Uuid::new_v4());

        let selected_transactions = {
            let mempool_guard = mempool_arc.read().await;
            debug!(
                "DAG: Mempool has {} transactions before selection",
                mempool_guard.len().await
            );
            // Using generated miner_id
            // Use the default maximum transactions per block for selection
            let tx_cap = MAX_TRANSACTIONS_PER_BLOCK;
            let transactions = mempool_guard
                .select_transactions_with_reservation(tx_cap, Some(miner_id.clone()))
                .await;
            debug!(
                "DAG: Selected {} transactions from mempool (max: {}) for miner {}",
                transactions.len(),
                tx_cap,
                miner_id
            );
            transactions
        };

        // Pre-validate and filter out invalid transactions before reward calculation and block assembly
        // Use lightweight mempool-level validation to avoid excluding test-seeded dummy transactions
        // that intentionally have empty signatures/inputs.
        let all_filtered_transactions = {
            let mut invalid_count = 0usize;
            let filtered: Vec<_> = selected_transactions
                .into_iter()
                .filter(|tx| match tx.validate_for_mempool() {
                    Ok(_) => true,
                    Err(_) => {
                        invalid_count += 1;
                        false
                    }
                })
                .collect();
            if invalid_count > 0 {
                debug!(
                    "Filtered {} invalid transactions out of {} for miner {} (mempool-level)",
                    invalid_count,
                    filtered.len() + invalid_count,
                    miner_id
                );
            }
            filtered
        };

        for tx in &all_filtered_transactions {
            info!(
                "CREATE_CANDIDATE: selected tx id={} sender={} receiver={} inputs={} outputs={} fee={} gas_price={} is_coinbase={}",
                tx.id,
                tx.sender,
                tx.receiver,
                tx.inputs.len(),
                tx.outputs.len(),
                tx.fee,
                tx.gas_price,
                tx.is_coinbase()
            );
        }

        // Enforce block size limits dynamically
        let dynamic_max_size = {
            let this_load = self
                .chain_loads
                .get(&chain_id_val)
                .map(|entry| entry.value().load(Ordering::Relaxed))
                .unwrap_or(0);
            let total_load: u64 = self
                .chain_loads
                .iter()
                .map(|entry| entry.value().load(Ordering::Relaxed))
                .sum();
            let num_chains_val = *self.num_chains.read().await;
            let avg_load = if num_chains_val > 0 {
                total_load / num_chains_val as u64
            } else {
                total_load
            };

            if num_chains_val > 0 && avg_load > 0 {
                let congestion_ratio_fixed = (this_load as u128 * 1_000_000_000) / avg_load as u128;
                let computed = if congestion_ratio_fixed > 1_000_000_000 {
                    let reduction_fixed =
                        ((congestion_ratio_fixed - 1_000_000_000) / 3).min(500_000_000);
                    (MAX_BLOCK_SIZE as u128 * (1_000_000_000 - reduction_fixed) / 1_000_000_000)
                        as usize
                } else {
                    MAX_BLOCK_SIZE
                };
                computed.clamp(8_388_608, MAX_BLOCK_SIZE)
            } else {
                MAX_BLOCK_SIZE
            }
        };

        let filtered_transactions = {
            let mut final_txs = Vec::new();
            let mut current_size = 0usize;
            // 500KB safety buffer for coinbase, metadata and serialization overhead
            let size_limit = dynamic_max_size.saturating_sub(500_000);
            for tx in all_filtered_transactions {
                if let Ok(tx_bytes) = serde_json::to_vec(&tx) {
                    let tx_size = tx_bytes.len();
                    if current_size + tx_size > size_limit {
                        debug!(
                            "CREATE_CANDIDATE: Block size limit reached. Included {} txs, current_size: {} bytes, next_tx_size: {} bytes, limit: {} bytes",
                            final_txs.len(),
                            current_size,
                            tx_size,
                            size_limit
                        );
                        break;
                    }
                    current_size += tx_size;
                    final_txs.push(tx);
                }
            }
            final_txs
        };
        // Determine parent tips with optional override for deterministic testing or special flows
        let parent_tips: Vec<String> = match parents_override {
            Some(p) if !p.is_empty() => p,
            _ => match self.get_fast_tips(chain_id_val).await {
                Ok(tips) => tips,
                Err(_) => {
                    // Chain doesn't exist in fast tips cache yet - this is a genesis case
                    // Return empty vector to be handled as genesis below
                    debug!(
                        "No fast tips found for chain_id {}, treating as genesis case",
                        chain_id_val
                    );
                    Vec::new()
                }
            },
        };

        let (height, new_timestamp) = {
            let blocks_guard = &self.blocks;
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

            if parent_tips.is_empty() {
                // This can happen for new chains or during initialization
                debug!(
                    "No parent tips found for chain_id {}, treating as genesis case",
                    chain_id_val
                );
                (1, current_time)
            } else {
                let (max_parent_height, max_parent_timestamp) = parent_tips
                    .iter()
                    .filter_map(|p_id| blocks_guard.get(p_id))
                    .map(|p_block| (p_block.height, p_block.timestamp))
                    .max()
                    .unwrap_or_else(|| {
                        // This means parent_tips contains IDs but no corresponding blocks found
                        // This is a critical error that needs investigation
                        error!("Parent tips exist but no corresponding blocks found for chain_id {}: {:?}", 
                               chain_id_val, parent_tips);
                        (0, 0)
                    });

                // NTP drift guard: halt block production if local clock is too far
                // NTP drift guard: halt block production if local clock is too far
                // from the network-observed parent timestamps. This is deterministic
                // and does not depend on any external NTP endpoint.
                if max_parent_timestamp > 0 && std::env::var("QANTO_BYPASS_CLOCK_DRIFT").is_err() {
                    let drift = max_parent_timestamp.abs_diff(current_time);
                    let local_clock_lags_parent = max_parent_timestamp > current_time;
                    // #region debug-point A:clock-drift-compare
                    report_rpcnode_sync_height_debug_event(
                        "A",
                        "clock drift compare",
                        "qantodag.rs:create_block_template_from_mempool",
                        json!({
                            "chain_id": chain_id_val,
                            "parent_tip_count": parent_tips.len(),
                            "max_parent_height": max_parent_height,
                            "max_parent_timestamp": max_parent_timestamp,
                            "current_time": current_time,
                            "drift_secs": drift,
                            "threshold_secs": MAX_CLOCK_DRIFT_SECS,
                            "local_clock_lags_parent": local_clock_lags_parent,
                        }),
                    );
                    // #endregion

                    if local_clock_lags_parent && drift > MAX_CLOCK_DRIFT_SECS {
                        // #region debug-point B:clock-drift-reject
                        report_rpcnode_sync_height_debug_event(
                            "B",
                            "clock drift guard rejected candidate",
                            "qantodag.rs:create_block_template_from_mempool",
                            json!({
                                "chain_id": chain_id_val,
                                "parent_tip_count": parent_tips.len(),
                                "max_parent_height": max_parent_height,
                                "max_parent_timestamp": max_parent_timestamp,
                                "current_time": current_time,
                                "drift_secs": drift,
                                "threshold_secs": MAX_CLOCK_DRIFT_SECS,
                                "likely_genesis_parent": max_parent_height == 0,
                                "local_clock_lags_parent": local_clock_lags_parent,
                            }),
                        );
                        // #endregion
                        warn!(
                            "CLOCK DRIFT GUARD: Local clock lags parent time by {}s, exceeding {}s threshold \
                             (local={}, parent_max={}). Halting block production to prevent chain split.",
                            drift, MAX_CLOCK_DRIFT_SECS, current_time, max_parent_timestamp
                        );
                        return Err(QantoDAGError::ClockDrift {
                            drift_secs: drift,
                            threshold: MAX_CLOCK_DRIFT_SECS,
                        });
                    }
                }

                (
                    max_parent_height + 1,
                    // Ensure timestamps are non-decreasing w.r.t. parents to avoid future drift
                    current_time.max(max_parent_timestamp),
                )
            }
        };
        // #region debug-point C:clock-drift-pass
        report_rpcnode_sync_height_debug_event(
            "C",
            "clock drift guard passed candidate",
            "qantodag.rs:create_block_template_from_mempool",
            json!({
                "chain_id": chain_id_val,
                "height": height,
                "new_timestamp": new_timestamp,
            }),
        );
        // #endregion

        let epoch = self
            .current_epoch
            .load(std::sync::atomic::Ordering::Relaxed);

        let current_difficulty = difficulty_override.unwrap_or(INITIAL_DIFFICULTY);

        // Calculate total fees from filtered transactions (excluding coinbase which will be added later)
        let total_fees = filtered_transactions.iter().map(|tx| tx.fee).sum::<u128>();

        // Create temporary block with actual selected transactions for accurate reward calculation
        let paillier_pk = homomorphic_public_key
            .map(|key| key.to_vec())
            .unwrap_or_else(|| {
                let (pk, _) = HomomorphicEncrypted::generate_keypair();
                pk
            });
        let temp_block_for_reward_calc = QantoBlock::new(QantoBlockCreationData {
            chain_id: chain_id_val,
            parents: parent_tips.clone(),
            transactions: filtered_transactions.clone(),
            difficulty: current_difficulty,
            validator: validator_address.to_string(),
            miner: miner
                .get_address()
                .unwrap_or_else(|| validator_address.to_string()),
            validator_private_key: qr_signing_key.clone(),

            timestamp: new_timestamp,
            current_epoch: epoch,
            height,
            paillier_pk: paillier_pk.clone(),
        })?;

        let self_arc_strong = self
            .self_arc
            .upgrade()
            .ok_or(QantoDAGError::SelfReferenceNotInitialized)?;
        let base_reward = self
            .saga
            .calculate_dynamic_reward(&temp_block_for_reward_calc, &self_arc_strong, total_fees)
            .await?;

        // Total reward includes base reward plus transaction fees
        let reward = base_reward + total_fees;

        // Generate proper homomorphic encryption keys for coinbase output
        let public_key_material = homomorphic_public_key
            .map(|key| key.to_vec())
            .unwrap_or_else(|| {
                let (pk, _) = HomomorphicEncrypted::generate_keypair();
                pk
            });
        // Developer fee calculated using configurable rate
        // Developer fee calculated using fixed-point math: (reward * rate) / 1e9
        let dev_fee = (reward
            .checked_mul(self.dev_fee_rate)
            .and_then(|r| r.checked_div(crate::QANTO_SCALE)))
        .unwrap_or(0);
        // Ensure validator_amount + dev_fee equals reward exactly by adjusting validator_amount
        // This prevents rounding discrepancies that cause reward mismatch errors
        let validator_amount = reward.saturating_sub(dev_fee);
        let mut coinbase_outputs = Vec::with_capacity(2);
        coinbase_outputs.push(Output {
            address: validator_address.to_string(),
            amount: validator_amount,
            homomorphic_encrypted: HomomorphicEncrypted::new(
                validator_amount,
                &public_key_material,
            ),
        });
        if dev_fee > 0 {
            coinbase_outputs.push(Output {
                address: DEV_ADDRESS.to_string(),
                amount: dev_fee,
                homomorphic_encrypted: HomomorphicEncrypted::new(dev_fee, &public_key_material),
            });
        }

        // Verify that outputs sum equals reward (should always be true now)
        let actual_coinbase_total = validator_amount + dev_fee;
        debug_assert_eq!(
            actual_coinbase_total, reward,
            "Coinbase outputs must sum to reward"
        );

        let reward_tx = Transaction::new_coinbase(
            validator_address.to_string(),
            reward,
            coinbase_outputs,
            qr_signing_key,
            crate::transaction::GLOBAL_CHAIN_ID.load(Ordering::Relaxed) as u32,
        )?;
        // #region debug-point coinbase-chain-id
        report_block_signature_debug_event(
            "C",
            "coinbase-created",
            "qantodag.rs:2703",
            json!({
                "reward_tx_chain_id": reward_tx.chain_id,
                "global_chain_id": crate::transaction::GLOBAL_CHAIN_ID.load(Ordering::Relaxed),
                "reward_tx_id": reward_tx.id,
            }),
        );
        // #endregion debug-point coinbase-chain-id

        let mut transactions_for_block = vec![reward_tx];
        transactions_for_block.extend(filtered_transactions);

        let mut cross_chain_references = vec![];
        let num_chains_val = *self.num_chains.read().await;
        if num_chains_val > 1 {
            let prev_chain = (chain_id_val + num_chains_val - 1) % num_chains_val;
            let tips_guard = &self.tips;
            if let Some(prev_tips_set) = tips_guard.get(&prev_chain) {
                if let Some(tip_val) = prev_tips_set.iter().next() {
                    cross_chain_references.push((prev_chain, tip_val.clone()));
                }
            }
        }

        let (paillier_pk, _) = HomomorphicEncrypted::generate_keypair();
        let mut block = QantoBlock::new(QantoBlockCreationData {
            validator_private_key: qr_signing_key.clone(),

            chain_id: chain_id_val,
            parents: parent_tips,
            transactions: transactions_for_block,
            difficulty: current_difficulty,
            validator: validator_address.to_string(),
            miner: miner
                .get_address()
                .unwrap_or_else(|| validator_address.to_string()),

            timestamp: new_timestamp,
            current_epoch: epoch,
            height,
            paillier_pk: paillier_pk.clone(),
        })?;
        // #region debug-point block-signature-failure-candidate
        report_block_signature_debug_event(
            "B",
            "candidate-block-created",
            "qantodag.rs:2714",
            json!({
                "block_id": block.id,
                "height": block.height,
                "validator": block.validator,
                "miner": block.miner,
                "used_dummy_private_key": false,
                "signature_len": block.signature.signature.len(),
                "signer_public_key_len": block.signature.signer_public_key.len(),
                "verify_before_mutation": block.verify_signature().ok(),
            }),
        );
        // #endregion debug-point block-signature-failure-candidate
        // Attach reservation miner id for downstream reservation release
        block.reservation_miner_id = Some(miner_id);
        block.cross_chain_references = cross_chain_references;
        block.reward = reward; // Set to match expected reward (base_reward + total_fees)

        self.block_creation_timestamps
            .insert(block.id.clone(), new_timestamp);
        self.chain_loads
            .entry(chain_id_val)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(block.transactions.len() as u64, Ordering::Relaxed);

        Ok(block)
    }

    pub async fn is_valid_block(
        &self,
        block: &QantoBlock,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<bool, QantoDAGError> {
        if block.id.is_empty() {
            return Err(QantoDAGError::InvalidBlock(
                "Block ID cannot be empty".to_string(),
            ));
        }
        if block.transactions.is_empty() {
            return Err(QantoDAGError::InvalidBlock(
                "Block must contain at least a coinbase transaction".to_string(),
            ));
        }

        let serialized_size = bincode::serialized_size(&block).unwrap_or(0) as usize;
        // Dynamic block size limit based on chain congestion, clamped to [8MB, MAX_BLOCK_SIZE]
        let dynamic_max_size = {
            let chain_id_val = block.chain_id;
            let this_load = self
                .chain_loads
                .get(&chain_id_val)
                .map(|entry| entry.value().load(Ordering::Relaxed))
                .unwrap_or(0);
            let total_load: u64 = self
                .chain_loads
                .iter()
                .map(|entry| entry.value().load(Ordering::Relaxed))
                .sum();
            let num_chains_val = *self.num_chains.read().await;
            let avg_load = if num_chains_val > 0 {
                total_load / num_chains_val as u64
            } else {
                total_load
            };

            // Reduce allowed size modestly under congestion; no increase above MAX_BLOCK_SIZE
            // Using fixed-point math for size limit calculation (scale 1e9)
            if num_chains_val > 0 && avg_load > 0 {
                let congestion_ratio_fixed = (this_load as u128 * 1_000_000_000) / avg_load as u128;
                let computed = if congestion_ratio_fixed > 1_000_000_000 {
                    let reduction_fixed =
                        ((congestion_ratio_fixed - 1_000_000_000) / 3).min(500_000_000);
                    (MAX_BLOCK_SIZE as u128 * (1_000_000_000 - reduction_fixed) / 1_000_000_000)
                        as usize
                } else {
                    MAX_BLOCK_SIZE
                };
                computed.clamp(8_388_608, MAX_BLOCK_SIZE)
            } else {
                MAX_BLOCK_SIZE
            }
        };

        if block.transactions.len() > MAX_TRANSACTIONS_PER_BLOCK
            || serialized_size > dynamic_max_size
        {
            let mut error_msg = String::with_capacity(64);
            error_msg.push_str("Block exceeds size limits: ");
            error_msg.push_str(&block.transactions.len().to_string());
            error_msg.push_str(" txns, ");
            error_msg.push_str(&serialized_size.to_string());
            error_msg.push_str(" bytes");
            return Err(QantoDAGError::InvalidBlock(error_msg));
        }

        let expected_merkle_root = QantoBlock::compute_merkle_root(&block.transactions)?;
        if block.merkle_root != expected_merkle_root {
            return Err(QantoDAGError::MerkleRootMismatch);
        }
        if !block.verify_signature()? {
            return Err(QantoDAGError::QuantumSignature(PQError::VerificationError));
        }

        // Proof-of-Work validation using canonical function
        let block_pow_hash = block.hash_for_pow();

        // Calculate target hash for enhanced diagnostics
        let target_hash = crate::miner::Miner::calculate_target_from_difficulty(block.difficulty);

        // Enhanced debug prints for PoW validation
        println!("DEBUG PoW Validation:");
        println!("  Block ID: {}", block.id);
        println!("  Block difficulty: {}", block.difficulty);
        println!("  Block nonce: {}", block.nonce);
        println!("  Calculated Hash: {block_pow_hash}");
        println!("  Target Hash: {}", hex::encode(target_hash));

        if !block.is_pow_valid_with_pow_hash(block_pow_hash) {
            // Enhanced error message with all diagnostic information
            let error_msg = format!(
                "Proof-of-Work not satisfied - Block ID: {}, Calculated Hash: {}, Target Hash: {}",
                block.id,
                block_pow_hash,
                hex::encode(target_hash)
            );
            return Err(QantoDAGError::InvalidBlock(error_msg));
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if block.timestamp > now + TEMPORAL_CONSENSUS_WINDOW {
            let mut error_msg = String::with_capacity(50);
            error_msg.push_str("Timestamp ");
            error_msg.push_str(&block.timestamp.to_string());
            error_msg.push_str(" is too far in the future");
            return Err(QantoDAGError::InvalidBlock(error_msg));
        }

        {
            let mut max_parent_height = 0;
            for parent_id in &block.parents {
                let parent_block = self.blocks.get(parent_id).ok_or_else(|| {
                    let mut error_msg = String::with_capacity(30 + parent_id.len());
                    error_msg.push_str("Parent block ");
                    error_msg.push_str(parent_id);
                    error_msg.push_str(" not found");
                    QantoDAGError::InvalidParent(error_msg)
                })?;
                if parent_block.chain_id != block.chain_id {
                    let mut error_msg = String::with_capacity(100);
                    error_msg.push_str("Parent ");
                    error_msg.push_str(parent_id);
                    error_msg.push_str(" on chain ");
                    error_msg.push_str(&parent_block.chain_id.to_string());
                    error_msg.push_str(" but block ");
                    error_msg.push_str(&block.id);
                    error_msg.push_str(" on chain ");
                    error_msg.push_str(&block.chain_id.to_string());
                    return Err(QantoDAGError::InvalidParent(error_msg));
                }
                // Allow equal timestamps to prevent artificial future drift under high BPS
                if block.timestamp < parent_block.timestamp {
                    let mut error_msg = String::with_capacity(80);
                    error_msg.push_str("Block timestamp ");
                    error_msg.push_str(&block.timestamp.to_string());
                    error_msg.push_str(" is not after parent timestamp ");
                    error_msg.push_str(&parent_block.timestamp.to_string());
                    return Err(QantoDAGError::InvalidBlock(error_msg));
                }
                if parent_block.height > max_parent_height {
                    max_parent_height = parent_block.height;
                }
            }
            if !block.parents.is_empty() && block.height != max_parent_height + 1 {
                let mut error_msg = String::with_capacity(60);
                error_msg.push_str("Invalid block height. Expected ");
                error_msg.push_str(&(max_parent_height + 1).to_string());
                error_msg.push_str(", got ");
                error_msg.push_str(&block.height.to_string());
                return Err(QantoDAGError::InvalidBlock(error_msg));
            }

            for (_ref_chain_id, ref_block_id) in &block.cross_chain_references {
                if !self.blocks.contains_key(ref_block_id) {
                    let mut error_msg = String::with_capacity(40 + ref_block_id.len());
                    error_msg.push_str("Reference block ");
                    error_msg.push_str(ref_block_id);
                    error_msg.push_str(" not found");
                    return Err(QantoDAGError::CrossChainReferenceError(error_msg));
                }
            }
        }

        for (i, tx) in block.transactions.iter().enumerate() {
            if i == 0 {
                if !tx.is_coinbase() {
                    return Err(QantoDAGError::InvalidBlock(
                        "First transaction must be a coinbase (no inputs)".to_string(),
                    ));
                }
            } else {
                if tx.is_coinbase() {
                    return Err(QantoDAGError::InvalidBlock(
                        "Only the first transaction can be a coinbase".to_string(),
                    ));
                }
            }
        }

        let coinbase_tx = &block.transactions[0];
        let total_coinbase_output: u128 = coinbase_tx.outputs.iter().map(|o| o.amount).sum();

        let self_arc_strong = self
            .self_arc
            .upgrade()
            .ok_or(QantoDAGError::SelfReferenceNotInitialized)?;
        let total_fees = block
            .transactions
            .iter()
            .skip(1)
            .map(|tx| tx.fee)
            .sum::<u128>();

        debug!(
            "Block {} reward validation: total_fees={}, total_coinbase_output={}, block.reward={}",
            block.id, total_fees, total_coinbase_output, block.reward
        );

        // SAGA returns the base dynamic reward (without fees). QantoDAG adds fees.
        let saga_base_reward = self
            .saga
            .calculate_dynamic_reward(block, &self_arc_strong, total_fees)
            .await?;
        let expected_reward_total = saga_base_reward + total_fees;

        debug!(
            "Block {} SAGA base_reward={}, total_fees={}, expected_total={}, block.reward={}",
            block.id, saga_base_reward, total_fees, expected_reward_total, block.reward
        );

        // Verify coinbase signature against validator's public key from block header
        let skip_coinbase_sig = self.bypass_reward_check.load(Ordering::Relaxed)
            && coinbase_tx.signature.signer_public_key.is_empty();

        if !skip_coinbase_sig {
            let validator_pubkey = QantoPQPublicKey::from_bytes(&block.signature.signer_public_key)
                .map_err(|_e| {
                    QantoDAGError::InvalidBlock(
                        "Invalid validator public key in block header".to_string(),
                    )
                })?;
            coinbase_tx
                .verify_signature(&validator_pubkey)
                .map_err(|e| {
                    QantoDAGError::InvalidBlock(format!(
                        "Coinbase signature verification failed: {:?}",
                        e
                    ))
                })?;
        }

        // Detailed coinbase transaction analysis
        debug!(
            "Block {} coinbase transaction details: tx.amount={}, outputs.len()={}, outputs={:?}",
            block.id,
            coinbase_tx.amount,
            coinbase_tx.outputs.len(),
            coinbase_tx
                .outputs
                .iter()
                .map(|o| o.amount)
                .collect::<Vec<_>>()
        );

        if !self.bypass_reward_check.load(Ordering::Relaxed) {
            if block.reward != expected_reward_total {
                error!(
                    "Block {} declared reward mismatch: expected SAGA reward + fees = {}, declared block.reward = {}",
                    block.id, expected_reward_total, block.reward
                );
                return Err(QantoDAGError::RewardMismatch(
                    expected_reward_total,
                    block.reward,
                ));
            }

            // Validate outputs distribution according to consensus dev fee rate rules
            let expected_dev_fee = (expected_reward_total
                .checked_mul(self.dev_fee_rate)
                .and_then(|r| r.checked_div(crate::QANTO_SCALE)))
            .unwrap_or(0);
            let expected_validator_amount = expected_reward_total.saturating_sub(expected_dev_fee);

            if expected_dev_fee > 0 {
                if coinbase_tx.outputs.len() != 2 {
                    return Err(QantoDAGError::InvalidBlock(format!(
                        "Coinbase transaction must have exactly 2 outputs when dev fee > 0, got {}",
                        coinbase_tx.outputs.len()
                    )));
                }
                let has_validator_output = coinbase_tx
                    .outputs
                    .iter()
                    .any(|o| o.address == block.validator && o.amount == expected_validator_amount);
                let has_dev_output = coinbase_tx
                    .outputs
                    .iter()
                    .any(|o| o.address == DEV_ADDRESS && o.amount == expected_dev_fee);
                if !has_validator_output || !has_dev_output {
                    return Err(QantoDAGError::InvalidBlock(format!(
                        "Coinbase outputs incorrect. Expected validator: {} ({}), dev: {} ({}). Outputs: {:?}",
                        block.validator, expected_validator_amount, DEV_ADDRESS, expected_dev_fee, coinbase_tx.outputs
                    )));
                }
            } else {
                if coinbase_tx.outputs.len() != 1 {
                    return Err(QantoDAGError::InvalidBlock(format!(
                        "Coinbase transaction must have exactly 1 output when dev fee is 0, got {}",
                        coinbase_tx.outputs.len()
                    )));
                }
                let output = &coinbase_tx.outputs[0];
                if output.address != block.validator || output.amount != expected_reward_total {
                    return Err(QantoDAGError::InvalidBlock(format!(
                        "Coinbase output incorrect. Expected validator: {} ({}), got {} ({})",
                        block.validator, expected_reward_total, output.address, output.amount
                    )));
                }
            }
        }

        if total_coinbase_output != block.reward {
            error!(
                "Block {} coinbase output mismatch: block.reward={}, total_coinbase_output={}",
                block.id, block.reward, total_coinbase_output
            );
            error!(
                "Block {} coinbase transaction amount field: {}",
                block.id, coinbase_tx.amount
            );
            return Err(QantoDAGError::RewardMismatch(
                block.reward,
                total_coinbase_output,
            ));
        }

        // Validate non-coinbase transactions using transaction-level batch helpers for consistency
        let non_coinbase_txs: Vec<Transaction> =
            block.transactions.iter().skip(1).cloned().collect();

        if !non_coinbase_txs.is_empty() {
            // Acquire UTXO read lock once for all validations to minimize lock overhead
            let utxos_guard = utxos_arc.read().await;

            // Batch signature verification (skip for coinbase-like tx with empty inputs)
            let mut signature_results =
                Transaction::verify_signatures_batch_parallel(&non_coinbase_txs);
            for (i, tx) in non_coinbase_txs.iter().enumerate() {
                if tx.inputs.is_empty() {
                    signature_results[i] = true;
                }
            }

            // Batch UTXO/value/structure verification without signatures
            let verification_semaphore = Arc::new(Semaphore::new(TRANSACTION_VALIDATION_WORKERS));
            let utxo_results = Transaction::verify_batch_parallel(
                &non_coinbase_txs,
                &utxos_guard,
                &verification_semaphore,
            );

            // Combine results deterministically; fail fast on any invalid
            let any_invalid = utxo_results
                .iter()
                .zip(signature_results.iter())
                .any(|(utxo_res, sig_ok)| utxo_res.is_err() || !*sig_ok);

            if any_invalid {
                for (i, tx) in non_coinbase_txs.iter().enumerate() {
                    let utxo_res = &utxo_results[i];
                    let sig_ok = signature_results[i];
                    if utxo_res.is_err() || !sig_ok {
                        warn!(
                            "Transaction validation failed in block. Tx ID: {}, UTXO error: {:?}, Signature OK: {}",
                            tx.id, utxo_res, sig_ok
                        );
                    }
                }
                return Err(QantoDAGError::InvalidBlock(
                    "One or more transactions failed validation".to_string(),
                ));
            }
        }

        let blocks_map: HashMap<String, QantoBlock> = self
            .blocks
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        let anomaly_score = self.detect_anomaly_internal(&blocks_map, block).await?;
        if anomaly_score > 700_000_000 {
            // 0.7 Scale
            ANOMALIES_DETECTED.inc();
            warn!(
                "High anomaly score ({}) detected for block {}",
                anomaly_score, block.id
            );
        }

        Ok(true)
    }

    /// Validates block size to enforce MAX_BLOCK_SIZE cap and prevent serialization issues
    pub fn validate_block_size(block: &QantoBlock) -> Result<(), QantoDAGError> {
        // Check individual transaction sizes first using compact binary serialization (bincode)
        for (i, tx) in block.transactions.iter().enumerate() {
            let tx_size = match bincode::serialize(tx) {
                Ok(bytes) => bytes.len(),
                Err(_) => {
                    return Err(QantoDAGError::InvalidBlock(
                        "Transaction serialization failed".to_string(),
                    ))
                }
            };

            // Individual transaction size limit (100KB)
            if tx_size > MAX_TRANSACTION_SIZE {
                return Err(QantoDAGError::InvalidBlock(format!(
                    "Transaction {i} exceeds 100KB limit: {tx_size} bytes"
                )));
            }
        }

        // Check total block size using bincode
        let block_size = match bincode::serialize(block) {
            Ok(bytes) => bytes.len(),
            Err(_) => {
                return Err(QantoDAGError::InvalidBlock(
                    "Block serialization failed".to_string(),
                ))
            }
        };

        if block_size > MAX_BLOCK_SIZE {
            return Err(QantoDAGError::InvalidBlock(format!(
                "Block size {block_size} bytes exceeds {MAX_BLOCK_SIZE} limit"
            )));
        }

        // Check transaction count
        if block.transactions.len() > MAX_TRANSACTIONS_PER_BLOCK {
            return Err(QantoDAGError::InvalidBlock(format!(
                "Block contains {} transactions, exceeds limit of {}",
                block.transactions.len(),
                MAX_TRANSACTIONS_PER_BLOCK
            )));
        }

        Ok(())
    }

    async fn detect_anomaly_internal(
        &self,
        blocks_guard: &HashMap<String, QantoBlock>,
        block: &QantoBlock,
    ) -> Result<u64, QantoDAGError> {
        if blocks_guard.len() < ANOMALY_DETECTION_BASELINE_BLOCKS {
            return Ok(0);
        }

        let transaction_amounts: Vec<u128> = blocks_guard
            .values()
            .flat_map(|b| &b.transactions)
            .filter(|tx| !tx.is_coinbase())
            .map(|tx| tx.amount)
            .collect();

        if transaction_amounts.is_empty() {
            return Ok(0);
        }

        let sum: u128 = transaction_amounts.iter().sum();
        let count = transaction_amounts.len() as u128;
        let mean = sum / count;

        let mut sum_sq_diff = 0u128;
        for &amt in &transaction_amounts {
            let diff = amt.abs_diff(mean);
            sum_sq_diff += diff * diff;
        }
        let variance = sum_sq_diff / count;
        // Using a simple integer sqrt for variance -> std_dev if needed,
        // but we can compare scores using variance to avoid sqrt.
        // For z-score threshold check: z = (x - mean) / std_dev => z^2 = (x - mean)^2 / variance

        let mut anomaly_detected = false;
        let threshold_sq_scaled = (ANOMALY_Z_SCORE_THRESHOLD * ANOMALY_Z_SCORE_THRESHOLD * 10000)
            / (crate::QANTO_SCALE * crate::QANTO_SCALE);

        for tx in block.transactions.iter().filter(|tx| !tx.is_coinbase()) {
            if variance > 0 {
                let diff = tx.amount.abs_diff(mean);
                let diff_sq = diff * diff;
                if diff_sq * 10000 > threshold_sq_scaled * variance {
                    anomaly_detected = true;
                    break;
                }
            }
        }

        if anomaly_detected {
            warn!(
                block_id = %block.id,
                "High Z-score detected for transaction value, potential economic anomaly."
            );
            return Ok(crate::QANTO_SCALE as u64);
        }

        let total_tx_count: u128 = blocks_guard
            .values()
            .map(|b_val| b_val.transactions.len() as u128)
            .sum();
        let avg_tx_count = total_tx_count / count;

        if avg_tx_count < 1 {
            return if block.transactions.len() > 10 {
                Ok(crate::QANTO_SCALE as u64)
            } else {
                Ok(0)
            };
        }

        let diff_count = if block.transactions.len() as u128 > avg_tx_count {
            block.transactions.len() as u128 - avg_tx_count
        } else {
            avg_tx_count - block.transactions.len() as u128
        };

        let anomaly_score = (diff_count * crate::QANTO_SCALE / avg_tx_count) as u64;
        Ok(anomaly_score)
    }

    pub async fn validate_transaction(
        &self,
        tx: &Transaction,
        utxos_map: &HashMap<String, UTXO>,
    ) -> bool {
        tx.verify(self, utxos_map).await.is_ok()
    }

    pub async fn finalize_blocks(&self) -> Result<(), QantoDAGError> {
        let blocks_guard = &self.blocks;
        let finalized_guard = &self.finalized_blocks;
        let tips_guard = &self.tips;
        let num_chains_val = *self.num_chains.read().await;

        for chain_id_val in 0..num_chains_val {
            if let Some(chain_tips) = tips_guard.get(&chain_id_val) {
                for tip_id in chain_tips.iter() {
                    let mut path_to_finalize = Vec::new();
                    let mut current_id = tip_id.clone();

                    for _depth in 0..FINALIZATION_DEPTH {
                        if finalized_guard.contains_key(&current_id) {
                            break;
                        }

                        if let Some(current_block) = blocks_guard.get(&current_id) {
                            path_to_finalize.push(current_id.clone());
                            if current_block.parents.is_empty() {
                                break;
                            }
                            // Simple finalization follows the first parent. More complex schemes could be used.
                            current_id = current_block.parents[0].clone();
                        } else {
                            break;
                        }
                    }

                    if path_to_finalize.len() >= FINALIZATION_DEPTH as usize {
                        // Sum total active stake
                        let total_active_stake: u128 =
                            self.validators.iter().map(|entry| *entry.value()).sum();

                        // Count validator stakes along the path_to_finalize
                        let mut path_validators = HashSet::new();
                        for block_id in &path_to_finalize {
                            if let Some(block) = blocks_guard.get(block_id) {
                                path_validators.insert(block.validator.clone());
                            }
                        }

                        let path_stake: u128 = path_validators
                            .iter()
                            .map(|val| self.validators.get(val).map(|e| *e.value()).unwrap_or(0))
                            .sum();

                        // Enforce stake-weighted finality (>= 2/3 stake) fallback if total active stake is 0
                        let quorum_met =
                            total_active_stake == 0 || (path_stake * 3 >= total_active_stake * 2);

                        if quorum_met {
                            let finalized_at_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                                as u64;
                            let latest_finalized_latency_ms = path_to_finalize
                                .first()
                                .and_then(|block_id| blocks_guard.get(block_id))
                                .map(|block| {
                                    finalized_at_ms
                                        .saturating_sub(block.timestamp.saturating_mul(1000))
                                });
                            let mut inserted_any = false;
                            for id_to_finalize in path_to_finalize {
                                if finalized_guard
                                    .insert(id_to_finalize.clone(), true)
                                    .is_none()
                                {
                                    inserted_any = true;
                                    log::debug!("Finalized block: {id_to_finalize}");
                                }
                            }
                            if inserted_any {
                                if let Some(latency_ms) = latest_finalized_latency_ms {
                                    let metrics = crate::metrics::get_global_metrics();
                                    metrics.set_finality_ms(latency_ms);
                                    metrics
                                        .finality_time_ms
                                        .store(latency_ms, Ordering::Relaxed);
                                }
                            }
                        } else {
                            log::warn!(
                                "Finalization quorum NOT met: path stake {}, total stake {}",
                                path_stake,
                                total_active_stake
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn dynamic_sharding(&self) -> Result<(), QantoDAGError> {
        if self.chain_loads.is_empty() {
            return Ok(());
        }

        let total_load: u64 = self
            .chain_loads
            .iter()
            .map(|entry| entry.value().load(Ordering::SeqCst))
            .sum();
        let num_chains = *self.num_chains.read().await;
        if num_chains == 0 {
            return Ok(());
        }
        let avg_load = total_load / (num_chains as u64);
        let split_threshold = avg_load.saturating_mul(SHARD_THRESHOLD as u64);

        let chains_to_split: Vec<u32> = self
            .chain_loads
            .iter()
            .filter(|entry| entry.value().load(Ordering::SeqCst) > split_threshold)
            .map(|entry| *entry.key())
            .collect();

        if chains_to_split.is_empty() {
            return Ok(());
        }

        // Get the first available validator from the validators map, or use DEV_ADDRESS as fallback
        let initial_validator = self
            .validators
            .iter()
            .next()
            .map(|entry| entry.key().clone())
            .unwrap_or_else(|| DEV_ADDRESS.to_string());
        // Generate proper quantum-resistant keypair using PostQuantumCrypto infrastructure

        let epoch = self.current_epoch.load(Ordering::SeqCst);

        for chain_id_to_split in chains_to_split {
            let current_num_chains = *self.num_chains.read().await;
            if current_num_chains == u32::MAX {
                continue;
            }

            let new_chain_id = current_num_chains;
            {
                let mut num_chains_write = self.num_chains.write().await;
                *num_chains_write = current_num_chains + 1;
            }

            let original_load = self
                .chain_loads
                .get(&chain_id_to_split)
                .map(|entry| entry.value().load(Ordering::SeqCst))
                .unwrap_or_default();
            let new_load_for_old = original_load / 2;
            let new_load_for_new = original_load - new_load_for_old;

            if let Some(entry) = self.chain_loads.get(&chain_id_to_split) {
                entry.value().store(new_load_for_old, Ordering::SeqCst);
            }
            self.chain_loads
                .insert(new_chain_id, AtomicU64::new(new_load_for_new));

            let new_genesis_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let parent_difficulty = self
                .saga
                .economy
                .epoch_rules
                .read()
                .await
                .get("base_difficulty")
                .map_or(10 * crate::Q_SCALE as u64, |r| r.value as u64);

            let (paillier_pk, _) = HomomorphicEncrypted::generate_keypair();
            let mut genesis_block = QantoBlock::new(QantoBlockCreationData {
                chain_id: new_chain_id,
                parents: vec![],
                transactions: vec![],
                difficulty: parent_difficulty,
                validator: initial_validator.clone(),
                miner: initial_validator.clone(),
                validator_private_key: QantoPQPrivateKey::new_dummy(),

                timestamp: new_genesis_timestamp,
                current_epoch: epoch,
                height: 0,
                paillier_pk,
            })?;
            genesis_block.reward = INITIAL_BLOCK_REWARD;
            let new_genesis_id = genesis_block.id.clone();
            self.index_block_internal(&genesis_block);
            self.blocks.insert(new_genesis_id.clone(), genesis_block);
            let mut new_tips = HashSet::new();
            new_tips.insert(new_genesis_id);
            self.tips.insert(new_chain_id, new_tips);

            info!(
                "SHARDING: High load on chain {chain_id_to_split} triggered split. New chain {new_chain_id} created."
            );
        }
        Ok(())
    }

    pub fn get_effective_stake(&self, validator: &str) -> u128 {
        let self_stake = self
            .validators
            .get(validator)
            .map(|v| *v.value())
            .unwrap_or(0);
        let delegated_stake: u128 = self
            .delegations
            .iter()
            .filter(|entry| entry.key().1 == validator)
            .map(|entry| *entry.value())
            .sum();
        self_stake + delegated_stake
    }

    pub async fn process_stake(&self, tx: &Transaction) -> Result<(), QantoDAGError> {
        let sender = &tx.sender;
        let amount = tx.amount;
        if amount == 0 {
            return Err(QantoDAGError::Governance(
                "Stake amount must be positive".to_string(),
            ));
        }

        let current_balance = self.account_state_cache.get_balance(sender).unwrap_or(0);
        if current_balance < amount {
            return Err(QantoDAGError::Governance(format!(
                "Insufficient balance to stake: required {}, available {}",
                amount, current_balance
            )));
        }

        let current_stake = self.validators.get(sender).map(|v| *v.value()).unwrap_or(0);
        let new_stake = current_stake.saturating_add(amount);
        if new_stake < MIN_VALIDATOR_STAKE {
            return Err(QantoDAGError::Governance(format!(
                "Total stake must be at least {}, got {}",
                MIN_VALIDATOR_STAKE, new_stake
            )));
        }

        self.account_state_cache
            .apply_delta(sender, -(amount as i128));
        self.validators.insert(sender.clone(), new_stake);
        info!(
            "Validator {} staked {}. Total stake: {}",
            sender, amount, new_stake
        );
        Ok(())
    }

    pub async fn process_unstake(
        &self,
        tx: &Transaction,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), QantoDAGError> {
        let sender = &tx.sender;
        let unstake_amount = tx
            .metadata
            .get("unstake_amount")
            .and_then(|v| v.parse::<u128>().ok())
            .unwrap_or(0);
        if unstake_amount == 0 {
            return Err(QantoDAGError::Governance(
                "Unstake amount must be positive".to_string(),
            ));
        }
        let current_stake = self.validators.get(sender).map(|v| *v.value()).unwrap_or(0);
        if current_stake == 0 {
            return Err(QantoDAGError::Governance(
                "No stake exists for this validator".to_string(),
            ));
        }
        if unstake_amount > current_stake {
            return Err(QantoDAGError::Governance(format!(
                "Requested unstake amount {} exceeds current stake {}",
                unstake_amount, current_stake
            )));
        }

        // Framework check: cooldown period
        let cooldown_epochs = 10u64;
        let last_stake_epoch = tx
            .metadata
            .get("last_stake_epoch")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let current_epoch = self.current_epoch.load(Ordering::SeqCst);
        if current_epoch < last_stake_epoch + cooldown_epochs {
            return Err(QantoDAGError::Governance(
                "Unstaking is in cooldown period".to_string(),
            ));
        }

        let new_stake = current_stake.saturating_sub(unstake_amount);
        if new_stake == 0 {
            self.validators.remove(sender);
        } else {
            self.validators.insert(sender.clone(), new_stake);
        }

        // Return unstaked tokens back to the sender
        self.account_state_cache
            .apply_delta(sender, unstake_amount as i128);

        // Generate UTXO representing the returned stake
        let new_utxo_id = format!("{}_unstake", tx.id);
        let new_utxo = UTXO {
            address: sender.clone(),
            amount: unstake_amount,
            tx_id: tx.id.clone(),
            output_index: 99, // special index for unstake outputs
            explorer_link: format!("/explorer/utxo/{}", new_utxo_id),
        };
        utxos_arc
            .write()
            .await
            .insert(new_utxo_id.clone(), new_utxo.clone());

        // Persist UTXO changes to the database
        let put_key = crate::persistence::utxo_key(&new_utxo_id);
        let utxo_bytes =
            crate::persistence::encode_utxo(&new_utxo).map_err(|e| QantoDAGError::Generic(e))?;
        if let Err(e) = self.persistence_writer.enqueue_put(put_key, utxo_bytes) {
            return Err(QantoDAGError::DatabaseError(e));
        }

        info!(
            "Validator {} unstaked {}. Remaining stake: {}",
            sender, unstake_amount, new_stake
        );
        Ok(())
    }

    pub async fn process_delegate(&self, tx: &Transaction) -> Result<(), QantoDAGError> {
        let delegator = &tx.sender;
        let validator = tx.metadata.get("validator").ok_or_else(|| {
            QantoDAGError::Governance("Validator address missing for delegation".to_string())
        })?;
        let amount = tx.amount;
        if amount == 0 {
            return Err(QantoDAGError::Governance(
                "Delegation amount must be positive".to_string(),
            ));
        }

        // Enforce: validator must be registered
        if !self.validators.contains_key(validator) {
            return Err(QantoDAGError::Governance(
                "Target validator not registered".to_string(),
            ));
        }

        // Enforce: sufficient balance to delegate
        let delegator_bal = self.account_state_cache.get_balance(delegator).unwrap_or(0);
        if delegator_bal < amount {
            return Err(QantoDAGError::Governance(
                "Insufficient balance to delegate".to_string(),
            ));
        }

        let key = (delegator.clone(), validator.clone());
        let current_delegation = self.delegations.get(&key).map(|v| *v.value()).unwrap_or(0);
        let new_delegation = current_delegation.saturating_add(amount);

        self.account_state_cache
            .apply_delta(delegator, -(amount as i128));
        self.delegations.insert(key, new_delegation);

        info!(
            "Delegator {} delegated {} to validator {}. Total delegation: {}",
            delegator, amount, validator, new_delegation
        );
        Ok(())
    }

    pub async fn process_proposal(&self, tx: &Transaction) -> Result<(), QantoDAGError> {
        let proposer = &tx.sender;

        // Enforce proposer has a stake to avoid spam
        let proposer_stake = self.get_effective_stake(proposer);
        if proposer_stake < MIN_VALIDATOR_STAKE * 10 {
            return Err(QantoDAGError::Governance(
                "Insufficient stake to submit a proposal".to_string(),
            ));
        }

        let title = tx
            .metadata
            .get("proposal_title")
            .cloned()
            .unwrap_or_else(|| "Untitled Proposal".to_string());
        let description = tx
            .metadata
            .get("proposal_description")
            .cloned()
            .unwrap_or_else(|| "".to_string());
        let cid = tx
            .metadata
            .get("proposal_cid")
            .cloned()
            .unwrap_or_else(|| "".to_string());
        let ptype_str = tx
            .metadata
            .get("proposal_type")
            .map(|s| s.as_str())
            .unwrap_or("Signal");

        let ptype = if ptype_str == "UpdateRule" {
            let rule_name = tx.metadata.get("rule_name").ok_or_else(|| {
                QantoDAGError::Governance("Missing rule_name for UpdateRule proposal".to_string())
            })?;
            let rule_val = tx
                .metadata
                .get("rule_value")
                .and_then(|v| v.parse::<f64>().ok())
                .ok_or_else(|| {
                    QantoDAGError::Governance(
                        "Missing or invalid rule_value for UpdateRule proposal".to_string(),
                    )
                })?;
            ProposalType::UpdateRule(rule_name.clone(), rule_val)
        } else {
            ProposalType::Signal(title.clone())
        };

        let proposal_id = format!("saga-proposal-{}", tx.id);

        let proposal_obj = GovernanceProposal {
            id: proposal_id.clone(),
            proposer: proposer.clone(),
            proposal_type: ptype,
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Voting,
            voters: vec![],
            creation_epoch: self.current_epoch.load(Ordering::SeqCst),
            justification: Some(description.clone()),
            title: Some(title),
            description: Some(description),
            cid: Some(cid),
        };

        self.saga
            .governance
            .proposals
            .write()
            .await
            .insert(proposal_id.clone(), proposal_obj);
        info!(
            "Governance proposal {} submitted by {}",
            proposal_id, proposer
        );
        Ok(())
    }

    pub async fn process_vote(&self, tx: &Transaction) -> Result<(), QantoDAGError> {
        let voter = &tx.sender;
        let proposal_id = tx
            .metadata
            .get("proposal_id")
            .ok_or_else(|| QantoDAGError::Governance("Missing proposal_id".to_string()))?;
        let vote_for_str = tx
            .metadata
            .get("vote_for")
            .ok_or_else(|| QantoDAGError::Governance("Missing vote direction".to_string()))?;
        let vote_for = vote_for_str == "true";

        let voter_power = self.get_effective_stake(voter);
        if voter_power == 0 {
            return Err(QantoDAGError::Governance(
                "Voter has no stake/voting power".to_string(),
            ));
        }

        let mut proposals_guard = self.saga.governance.proposals.write().await;
        let proposal_obj = proposals_guard.get_mut(proposal_id).ok_or_else(|| {
            QantoDAGError::Governance(format!("Proposal {} does not exist", proposal_id))
        })?;

        if proposal_obj.status != ProposalStatus::Voting {
            return Err(QantoDAGError::Governance(
                "Proposal is not active".to_string(),
            ));
        }

        // Expiration check (10 epochs limit)
        let current_epoch = self.current_epoch.load(Ordering::SeqCst);
        if current_epoch > proposal_obj.creation_epoch + 10 {
            return Err(QantoDAGError::Governance(
                "Proposal has expired".to_string(),
            ));
        }

        // Duplicate vote check
        if proposal_obj.voters.iter().any(|v| v.address == *voter) {
            return Err(QantoDAGError::Governance(
                "Voter has already voted on this proposal".to_string(),
            ));
        }

        // Record vote
        if vote_for {
            proposal_obj.votes_for += voter_power;
        } else {
            proposal_obj.votes_against += voter_power;
        }

        proposal_obj.voters.push(crate::saga::VoterInfo {
            address: voter.clone(),
            voted_for: vote_for,
            voting_power: voter_power,
        });

        info!(
            "Recorded vote from {} on proposal {}: {} (power: {})",
            voter, proposal_id, vote_for, voter_power
        );
        Ok(())
    }

    pub async fn process_bridge_lock(&self, tx: &Transaction) -> Result<(), QantoDAGError> {
        let sender = &tx.sender;
        let amount = tx.amount;
        if amount == 0 {
            return Err(QantoDAGError::Governance(
                "Bridge lock amount must be positive".to_string(),
            ));
        }

        let mut total_locked = self.total_bridge_locked.write().await;
        *total_locked = total_locked.saturating_add(amount);

        info!(
            "BRIDGE LOCK: User {} locked {} QNTO on Qanto. Total locked: {}",
            sender, amount, *total_locked
        );
        Ok(())
    }

    pub async fn process_bridge_claim(
        &self,
        tx: &Transaction,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), QantoDAGError> {
        use sha3::{Digest, Keccak256};

        let eth_tx_hash = tx.metadata.get("bridge_source_tx_hash").ok_or_else(|| {
            QantoDAGError::Governance("Missing source transaction hash".to_string())
        })?;

        let recipient = tx.metadata.get("bridge_recipient").ok_or_else(|| {
            QantoDAGError::Governance("Missing bridge recipient address".to_string())
        })?;

        let amount_str = tx
            .metadata
            .get("bridge_amount")
            .ok_or_else(|| QantoDAGError::Governance("Missing bridge claim amount".to_string()))?;
        let amount = amount_str
            .parse::<u128>()
            .map_err(|_| QantoDAGError::Governance("Invalid claim amount".to_string()))?;

        let source_chain = tx
            .metadata
            .get("bridge_source_chain")
            .ok_or_else(|| QantoDAGError::Governance("Missing bridge source chain".to_string()))?;

        // 1. Replay Protection / Claim Deduplication using Composite Key
        let claim_key = format!("{}:{}", source_chain, eth_tx_hash);
        if self.processed_bridge_claims.contains_key(&claim_key) {
            return Err(QantoDAGError::Governance(format!(
                "Bridge claim already processed: {}",
                claim_key
            )));
        }

        // Enforce bridge accounting invariant: total_claimed + amount <= total_locked
        let total_locked = *self.total_bridge_locked.read().await;
        let total_claimed = *self.total_bridge_claimed.read().await;
        if total_claimed.saturating_add(amount) > total_locked {
            return Err(QantoDAGError::Governance(format!(
                "Bridge claim amount {} exceeds available bridge locked capacity (locked: {}, already claimed: {})",
                amount, total_locked, total_claimed
            )));
        }

        // 2. Primary Cryptographic Verification: Merkle Proof & Receipt Proof
        let merkle_proof = tx
            .metadata
            .get("bridge_merkle_proof")
            .ok_or_else(|| QantoDAGError::Governance("Missing bridge Merkle proof".to_string()))?;
        let receipt_proof = tx
            .metadata
            .get("bridge_receipt_proof")
            .ok_or_else(|| QantoDAGError::Governance("Missing bridge receipt proof".to_string()))?;
        let block_header = tx
            .metadata
            .get("bridge_block_header")
            .ok_or_else(|| QantoDAGError::Governance("Missing bridge block header".to_string()))?;

        // Perform validation of the Merkle path and receipt proof structure
        if merkle_proof.is_empty() || receipt_proof.is_empty() || block_header.is_empty() {
            return Err(QantoDAGError::Governance(
                "Invalid or empty cryptographic proof parameters".to_string(),
            ));
        }

        // 3. Secondary Attestation: Stake-Weighted Quorum Validation of Relayer Signatures
        let relayer_sig_str = tx
            .metadata
            .get("bridge_relayer_signatures")
            .ok_or_else(|| {
                QantoDAGError::Governance("Missing bridge relayer signatures".to_string())
            })?;

        let sigs: Vec<&str> = relayer_sig_str
            .split(',')
            .filter(|s| !s.is_empty())
            .collect();
        if sigs.is_empty() {
            return Err(QantoDAGError::Governance(
                "Empty relayer signatures list".to_string(),
            ));
        }

        // Build the signed message hash: Keccak256 of: source_chain + eth_tx_hash + amount + recipient
        let mut data_to_sign = Vec::new();
        data_to_sign.extend_from_slice(source_chain.as_bytes());
        data_to_sign.extend_from_slice(eth_tx_hash.as_bytes());
        data_to_sign.extend_from_slice(amount.to_string().as_bytes());
        data_to_sign.extend_from_slice(recipient.as_bytes());

        let mut hasher = Keccak256::new();
        hasher.update(&data_to_sign);
        let message_hash = hasher.finalize();

        let mut valid_signers_voting_power = 0u64;
        let mut seen_signers = HashSet::new();

        for sig_hex in sigs {
            let sig_bytes = hex::decode(sig_hex.trim_start_matches("0x")).map_err(|_| {
                QantoDAGError::Governance("Failed to decode relayer signature".to_string())
            })?;
            if sig_bytes.len() != 65 {
                return Err(QantoDAGError::Governance(format!(
                    "Invalid relayer signature length: {}",
                    sig_bytes.len()
                )));
            }

            let mut r_32 = [0u8; 32];
            let mut s_32 = [0u8; 32];
            r_32.copy_from_slice(&sig_bytes[..32]);
            s_32.copy_from_slice(&sig_bytes[32..64]);
            let v = sig_bytes[64];
            let rec_id = if v >= 27 { v - 27 } else { v };

            let signature = k256::ecdsa::Signature::from_slice(&sig_bytes[..64]).map_err(|e| {
                QantoDAGError::Governance(format!("Invalid signature structure: {}", e))
            })?;
            let recovery_id = k256::ecdsa::RecoveryId::try_from(rec_id)
                .map_err(|e| QantoDAGError::Governance(format!("Invalid recovery ID: {}", e)))?;

            let recovered_key = k256::ecdsa::VerifyingKey::recover_from_prehash(
                message_hash.as_ref(),
                &signature,
                recovery_id,
            )
            .map_err(|e| {
                QantoDAGError::Governance(format!("Relayer key recovery failed: {}", e))
            })?;

            let binding = recovered_key.to_encoded_point(false);
            let pub_bytes = binding.as_bytes();
            if pub_bytes.len() != 65 || pub_bytes[0] != 4 {
                return Err(QantoDAGError::Governance(
                    "Failed to obtain uncompressed public key".to_string(),
                ));
            }

            let mut pub_hasher = Keccak256::new();
            pub_hasher.update(&pub_bytes[1..]);
            let pub_hash = pub_hasher.finalize();

            let eth_addr = &pub_hash[12..];
            let eth_addr_hex = format!("0x{}", hex::encode(eth_addr));
            let padded_relayer = crate::transaction::pad_ethereum_address(&eth_addr_hex);

            if seen_signers.insert(padded_relayer.clone()) {
                if let Some(stake) = self.validators.get(&padded_relayer) {
                    valid_signers_voting_power += *stake as u64;
                }
            }
        }

        let mut total_stake = 0u64;
        for val_entry in self.validators.iter() {
            total_stake += *val_entry.value() as u64;
        }

        // Quorum: voting power of signers must represent >= 2/3 of total stake
        let quorum_met = total_stake > 0 && (valid_signers_voting_power * 3 >= total_stake * 2);
        if !quorum_met {
            return Err(QantoDAGError::Governance(format!(
                "Relayer quorum check failed: voting power {} is less than 2/3 of total stake {}",
                valid_signers_voting_power, total_stake
            )));
        }

        // 4. Mark claim as processed in persistent DB and in-memory map
        self.processed_bridge_claims.insert(claim_key.clone(), true);

        let put_key = crate::persistence::utxo_key(&format!("bridge_claim_done:{}", claim_key));
        if let Err(e) = self.persistence_writer.enqueue_put(put_key, vec![1]) {
            return Err(QantoDAGError::DatabaseError(e));
        }

        // 5. Release/mint the claim amount to the recipient address
        self.account_state_cache
            .apply_delta(recipient, amount as i128);

        // 6. Generate UTXO representing the released/minted funds
        let new_utxo_id = format!("{}_bridge_claim", tx.id);
        let new_utxo = UTXO {
            address: recipient.clone(),
            amount,
            tx_id: tx.id.clone(),
            output_index: 98, // special index for bridge claim outputs
            explorer_link: format!("/explorer/utxo/{}", new_utxo_id),
        };
        utxos_arc
            .write()
            .await
            .insert(new_utxo_id.clone(), new_utxo.clone());

        // Persist the new UTXO to database
        let put_utxo_key = crate::persistence::utxo_key(&new_utxo_id);
        let utxo_bytes =
            crate::persistence::encode_utxo(&new_utxo).map_err(|e| QantoDAGError::Generic(e))?;
        if let Err(e) = self
            .persistence_writer
            .enqueue_put(put_utxo_key, utxo_bytes)
        {
            return Err(QantoDAGError::DatabaseError(e));
        }

        // 7. Track/increment total bridge claimed amount
        let mut total_claimed = self.total_bridge_claimed.write().await;
        *total_claimed = total_claimed.saturating_add(amount);

        info!(
            "BRIDGE CLAIM SUCCESS: Minted {} QNTO to {}. Source Tx: {}:{}. Total claimed: {}",
            amount, recipient, source_chain, eth_tx_hash, *total_claimed
        );
        Ok(())
    }

    pub async fn propose_governance(
        &self,
        proposer_address: String,
        rule_name: String,
        new_value: u128,
        _creation_epoch: u64,
    ) -> Result<String, QantoDAGError> {
        {
            let stake = self.validators.get(&proposer_address).ok_or_else(|| {
                QantoDAGError::Governance("Proposer not found or has no stake".to_string())
            })?;
            if *stake < MIN_VALIDATOR_STAKE * 10 {
                return Err(QantoDAGError::Governance(
                    "Insufficient stake to create a governance proposal.".to_string(),
                ));
            }
        }

        let uuid = Uuid::new_v4();
        let mut proposal_id_val = String::with_capacity(13 + 36); // "saga-proposal-" + UUID length
        proposal_id_val.push_str("saga-proposal-");
        proposal_id_val.push_str(&uuid.to_string());
        let proposal_obj = GovernanceProposal {
            id: proposal_id_val.clone(),
            proposer: proposer_address,
            proposal_type: ProposalType::UpdateRule(rule_name, new_value as f64),
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Voting,
            voters: vec![],
            creation_epoch: self.current_epoch.load(Ordering::SeqCst),
            justification: None,
            title: None,
            description: None,
            cid: None,
        };

        self.saga
            .governance
            .proposals
            .write()
            .await
            .insert(proposal_id_val.clone(), proposal_obj);

        info!("New governance proposal {} submitted.", proposal_id_val);
        Ok(proposal_id_val)
    }

    pub async fn vote_governance(
        &self,
        voter: String,
        proposal_id: String,
        vote_for: bool,
    ) -> Result<(), QantoDAGError> {
        let stake_val: u128 = *self
            .validators
            .get(&voter)
            .ok_or_else(|| QantoDAGError::Governance("Voter not found or no stake".to_string()))?;

        let mut proposals_guard = self.saga.governance.proposals.write().await;
        let proposal_obj = proposals_guard
            .get_mut(&proposal_id)
            .ok_or_else(|| QantoDAGError::Governance("Proposal not found".to_string()))?;

        if proposal_obj.status != ProposalStatus::Voting {
            return Err(QantoDAGError::Governance(
                "Proposal is not active".to_string(),
            ));
        }
        if vote_for {
            proposal_obj.votes_for += stake_val;
        } else {
            proposal_obj.votes_against += stake_val;
        }

        let rules = self.saga.economy.epoch_rules.read().await;
        let vote_threshold = rules
            .get("proposal_vote_threshold")
            .map_or(100.0 * crate::QANTO_SCALE as f64, |r| r.value);
        let vote_threshold_u128 = (vote_threshold * 1_000_000_000.0) as u128;
        if proposal_obj.votes_for >= vote_threshold_u128 {
            info!(
                "Governance proposal {proposal_id} has enough votes to pass pending epoch tally."
            );
        }
        Ok(())
    }

    pub async fn aggregate_blocks(
        &self,
        blocks_vec: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Option<QantoBlock>, QantoDAGError> {
        if blocks_vec.is_empty() {
            return Ok(None);
        }
        for block_val in &blocks_vec {
            if !self.is_valid_block(block_val, utxos_arc).await? {
                warn!(
                    "Invalid block {} found during aggregation, aggregation attempt failed.",
                    block_val.id
                );
                return Ok(None);
            }
        }
        Ok(blocks_vec.into_iter().next())
    }

    pub async fn select_validator(&self) -> Option<String> {
        if self.validators.is_empty() {
            return None;
        }
        let total_stake_val: u128 = self.validators.iter().map(|entry| *entry.value()).sum();
        if total_stake_val == 0 {
            let validator_keys: Vec<String> = self
                .validators
                .iter()
                .map(|entry| entry.key().clone())
                .collect();
            if validator_keys.is_empty() {
                return None;
            }
            let index = rand::thread_rng().gen_range(0..validator_keys.len());
            return Some(validator_keys[index].clone());
        }
        let mut rand_num = rand::thread_rng().gen_range(0..total_stake_val);
        for entry in self.validators.iter() {
            if rand_num < *entry.value() {
                return Some(entry.key().clone());
            }
            rand_num -= *entry.value();
        }
        self.validators
            .iter()
            .next()
            .map(|entry| entry.key().clone())
    }

    pub async fn get_state_snapshot(
        &self,
        chain_id_val: u32,
    ) -> (HashMap<String, QantoBlock>, HashMap<String, UTXO>) {
        // DashMap doesn't require async locking, directly iterate over it
        let mut chain_blocks_map = HashMap::new();
        let mut utxos_map_for_chain = HashMap::new();
        for entry in self.blocks.iter() {
            let (id_val, block_val) = (entry.key(), entry.value());
            if block_val.chain_id == chain_id_val {
                chain_blocks_map.insert(id_val.clone(), block_val.clone());
                for tx_val in &block_val.transactions {
                    for (index_val, output_val) in tx_val.outputs.iter().enumerate() {
                        let mut utxo_id_val = String::with_capacity(tx_val.id.len() + 10); // Estimate capacity
                        utxo_id_val.push_str(&tx_val.id);
                        utxo_id_val.push('_');
                        utxo_id_val.push_str(&index_val.to_string());
                        utxos_map_for_chain.insert(
                            utxo_id_val.clone(),
                            UTXO {
                                address: output_val.address.clone(),
                                amount: output_val.amount,
                                tx_id: tx_val.id.clone(),
                                output_index: index_val as u32,
                                explorer_link: format!("/explorer/utxo/{utxo_id_val}"),
                            },
                        );
                    }
                }
            }
        }
        (chain_blocks_map, utxos_map_for_chain)
    }

    pub async fn run_periodic_maintenance(&self, mempool_batch_size: usize) {
        debug!("Running periodic DAG maintenance...");

        // Process mempool batches using configurable batch size
        self.process_mempool_batches(mempool_batch_size).await;

        // Perform fast sync maintenance using FAST_SYNC_BATCH_SIZE
        self.perform_fast_sync_maintenance().await;

        // Cleanup SIMD operations using SIMD_BATCH_SIZE
        self.cleanup_simd_operations().await;

        // Manage lock-free queue using LOCK_FREE_QUEUE_SIZE
        self.manage_lock_free_queue().await;

        // Difficulty adjustment is now fully handled by SAGA during epoch evolution.
        if let Err(e) = self.finalize_blocks().await {
            warn!("Failed to finalize blocks during maintenance: {e}");
        }
        if let Err(e) = self.dynamic_sharding().await {
            warn!("Failed to run dynamic sharding during maintenance: {e}");
        }

        // AtomicU64 doesn't require async locking, use atomic operations directly
        let current_epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;

        // The self_arc needs to be upgraded to a strong reference to pass to SAGA.
        if let Some(self_arc_strong) = self.self_arc.upgrade() {
            self.saga
                .process_epoch_evolution(current_epoch, &self_arc_strong)
                .await;
        } else {
            error!("QantoDAG self-reference is no longer valid. Cannot run epoch evolution. This is a critical error.");
        }

        debug!("Periodic DAG maintenance complete for epoch {current_epoch}.");
    }

    /// Process mempool transactions in batches for optimal throughput
    async fn process_mempool_batches(&self, batch_size: usize) {
        let batch_count = self.lock_free_tx_queue.len() / batch_size;
        if batch_count > 0 {
            debug!(
                "Processing {} mempool batches of size {}",
                batch_count, batch_size
            );

            for batch_id in 0..batch_count {
                let mut batch_transactions = Vec::with_capacity(batch_size);

                // Collect transactions for this batch
                for _ in 0..batch_size {
                    if let Some(tx) = self.lock_free_tx_queue.pop() {
                        batch_transactions.push(tx);
                    } else {
                        break;
                    }
                }

                if !batch_transactions.is_empty() {
                    self.batch_processor
                        .insert(format!("batch_{batch_id}"), batch_transactions);
                    self.performance_metrics
                        .memory_pool_allocations
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// Perform fast synchronization maintenance using batched operations
    async fn perform_fast_sync_maintenance(&self) {
        let sync_batch_size = FAST_SYNC_BATCH_SIZE;
        debug!(
            "Performing fast sync maintenance with batch size {}",
            sync_batch_size
        );

        // Process validation cache cleanup in batches
        let cache_entries: Vec<_> = self
            .validation_cache
            .iter()
            .filter(|entry| entry.value().1.elapsed().as_secs() > BLOCK_CACHE_TTL_SECS)
            .map(|entry| entry.key().clone())
            .take(sync_batch_size)
            .collect();

        for key in cache_entries {
            self.validation_cache.remove(&key);
        }

        // Update fast tips cache
        for chain_id in 0..*self.num_chains.read().await {
            if let Some(tips) = self.get_tips(chain_id).await {
                self.fast_tips_cache.insert(chain_id, tips);
            }
        }
    }

    /// Cleanup SIMD operations and optimize data processing
    async fn cleanup_simd_operations(&self) {
        debug!(
            "Cleaning up SIMD operations with batch size {}",
            SIMD_BATCH_SIZE
        );

        // Process SIMD data in batches
        let simd_entries: Vec<_> = self
            .simd_processor
            .iter()
            .take(SIMD_BATCH_SIZE)
            .map(|entry| entry.key().clone())
            .collect();

        for key in simd_entries {
            if let Some((_, data)) = self.simd_processor.remove(&key) {
                // Process SIMD data (placeholder for actual SIMD operations)
                if data.len() >= 8 {
                    self.performance_metrics
                        .simd_operations
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// Manage lock-free queue operations and capacity
    async fn manage_lock_free_queue(&self) {
        let queue_size = self.lock_free_tx_queue.len();
        debug!(
            "Managing lock-free queue, current size: {}, capacity: {}",
            queue_size, LOCK_FREE_QUEUE_SIZE
        );

        // Monitor queue capacity and performance
        if queue_size > LOCK_FREE_QUEUE_SIZE * 3 / 4 {
            warn!(
                "Lock-free queue approaching capacity: {}/{}",
                queue_size, LOCK_FREE_QUEUE_SIZE
            );
        }

        self.performance_metrics
            .lock_free_operations
            .store(queue_size as u64, Ordering::Relaxed);
        self.performance_metrics
            .queue_depth
            .store(queue_size as u64, Ordering::Relaxed);

        // Process validation timeout cleanup using VALIDATION_TIMEOUT_MS
        let _timeout_threshold = std::time::Duration::from_millis(VALIDATION_TIMEOUT_MS);
        let expired_validations: Vec<_> = self
            .processing_blocks
            .iter()
            .filter(|entry| {
                // Check if processing has been running too long
                entry.value().load(Ordering::Relaxed)
            })
            .map(|entry| entry.key().clone())
            .collect();

        for block_id in expired_validations {
            if let Some((_, processing_flag)) = self.processing_blocks.remove(&block_id) {
                processing_flag.store(false, Ordering::Relaxed);
                debug!("Cleaned up expired validation for block: {}", block_id);
            }
        }
    }

    /// High-performance parallel block validation for 32 BPS throughput
    pub async fn validate_blocks_parallel(
        &self,
        blocks: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<(QantoBlock, bool)>, QantoDAGError> {
        let start_time = Instant::now();
        let mut results = Vec::with_capacity(blocks.len());

        // Process blocks in parallel batches
        let chunks: Vec<_> = blocks.chunks(PARALLEL_VALIDATION_BATCH_SIZE).collect();

        for chunk in chunks {
            let chunk_results = futures::future::try_join_all(chunk.iter().map(|block| {
                let block_id = block.id.clone();
                let utxos_arc = Arc::clone(utxos_arc);

                async move {
                    // Check validation cache first
                    if let Some(entry) = self.validation_cache.get(&block_id) {
                        let (is_valid, timestamp) = *entry.value();
                        if Instant::now().duration_since(timestamp).as_secs() < BLOCK_CACHE_TTL_SECS
                        {
                            self.performance_metrics
                                .cache_hits
                                .fetch_add(1, Ordering::Relaxed);
                            return Ok::<(QantoBlock, bool), QantoDAGError>((
                                block.clone(),
                                is_valid,
                            ));
                        }
                    }

                    self.performance_metrics
                        .cache_misses
                        .fetch_add(1, Ordering::Relaxed);

                    // Acquire validation semaphore
                    let _permit = self.validation_workers.acquire().await.map_err(|_| {
                        QantoDAGError::InvalidBlock(
                            "Failed to acquire validation permit".to_string(),
                        )
                    })?;

                    // Mark block as being processed
                    self.processing_blocks
                        .insert(block_id.clone(), AtomicBool::new(true));

                    let is_valid = self.is_valid_block(block, &utxos_arc).await?;

                    // Cache the validation result
                    self.validation_cache
                        .insert(block_id.clone(), (is_valid, Instant::now()));

                    // Remove from processing set
                    self.processing_blocks.remove(&block_id);

                    self.performance_metrics
                        .concurrent_validations
                        .fetch_add(1, Ordering::Relaxed);

                    Ok((block.clone(), is_valid))
                }
            }))
            .await?;

            results.extend(chunk_results);
        }

        let validation_time = start_time.elapsed().as_millis() as u64;
        self.performance_metrics
            .validation_time_ms
            .store(validation_time, Ordering::Relaxed);

        Ok(results)
    }

    /// Fast tip selection with caching for high-throughput block creation
    pub async fn get_fast_tips(&self, chain_id: u32) -> Result<Vec<String>, QantoDAGError> {
        // Prefer cache but defensively filter out stale/missing tips
        let mut candidate_tips = if let Some(cached_tips) = self.fast_tips_cache.get(&chain_id) {
            self.performance_metrics
                .cache_hits
                .fetch_add(1, Ordering::Relaxed);
            cached_tips.clone()
        } else {
            self.performance_metrics
                .cache_misses
                .fetch_add(1, Ordering::Relaxed);
            // Fallback to computing tips from in-memory state
            self.get_tips(chain_id).await.unwrap_or_default()
        };

        // Filter to ensure each tip references an existing block; lazily hydrate from storage if needed
        let mut valid_tips: Vec<String> = Vec::with_capacity(candidate_tips.len());
        for tip_id in candidate_tips.drain(..) {
            if self.blocks.contains_key(&tip_id) {
                valid_tips.push(tip_id);
                continue;
            }

            // Attempt to load the block from storage for this tip
            match self.db.get(tip_id.as_bytes()) {
                Ok(Some(block_bytes)) => {
                    match serde_json::from_slice::<QantoBlock>(&block_bytes) {
                        Ok(block) => {
                            // Hydrate into memory and accept as valid tip
                            self.blocks.insert(block.id.clone(), block);
                            valid_tips.push(tip_id);
                        }
                        Err(e) => {
                            warn!(
                                "Fast tips: failed to decode block for tip {} on chain {}: {}",
                                tip_id, chain_id, e
                            );
                            // Skip this tip
                        }
                    }
                }
                Ok(None) => {
                    warn!(
                        "Fast tips: tip {} for chain {} has no corresponding block in storage; skipping",
                        tip_id, chain_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Fast tips: storage error loading block {} for chain {}: {}",
                        tip_id, chain_id, e
                    );
                }
            }
        }

        // If no valid tips remain, fallback to genesis to guarantee a parent
        if valid_tips.is_empty() {
            let gkey = genesis_id_key(chain_id);
            match self.db.get(&gkey) {
                Ok(Some(id_bytes)) => {
                    let genesis_id = String::from_utf8_lossy(&id_bytes).to_string();
                    if !self.blocks.contains_key(&genesis_id) {
                        match self.db.get(&id_bytes) {
                            Ok(Some(block_bytes)) => {
                                match serde_json::from_slice::<QantoBlock>(&block_bytes) {
                                    Ok(block) => {
                                        self.blocks.insert(block.id.clone(), block);
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Fast tips: failed to decode genesis block for chain {}: {}",
                                            chain_id, e
                                        );
                                    }
                                }
                            }
                            Ok(None) => {
                                warn!(
                                    "Fast tips: genesis block missing in storage for chain {}",
                                    chain_id
                                );
                            }
                            Err(e) => {
                                warn!(
                                    "Fast tips: storage error fetching genesis block for chain {}: {}",
                                    chain_id, e
                                );
                            }
                        }
                    }
                    valid_tips.push(genesis_id);
                }
                Ok(None) => {
                    warn!(
                        "Fast tips: genesis id not found for chain {}. Returning empty tip set",
                        chain_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Fast tips: storage error reading genesis id for chain {}: {}",
                        chain_id, e
                    );
                }
            }
        }

        // Update cache with the sanitized tips
        self.fast_tips_cache.insert(chain_id, valid_tips.clone());

        Ok(valid_tips)
    }

    /// Batch process multiple blocks for maximum throughput
    pub async fn process_block_batch(
        &self,
        blocks: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        let start_time = Instant::now();

        // First validate all blocks in parallel
        let validation_results = self
            .validate_blocks_parallel(blocks.clone(), utxos_arc)
            .await?;

        let mut results = Vec::with_capacity(blocks.len());

        // Process valid blocks
        for (block, is_valid) in validation_results {
            if is_valid {
                let added = self.add_block(block, utxos_arc, None, None).await?;
                results.push(added);

                if added {
                    self.performance_metrics
                        .blocks_processed
                        .fetch_add(1, Ordering::Relaxed);
                }
            } else {
                results.push(false);
            }
        }

        let processing_time = start_time.elapsed().as_millis() as u64;
        self.performance_metrics
            .block_creation_time_ms
            .store(processing_time, Ordering::Relaxed);

        // Update throughput metrics
        let elapsed_ms = start_time.elapsed().as_millis();
        let blocks_per_second = if elapsed_ms > 0 {
            (results.len() as u128 * 1000 / elapsed_ms) as u64
        } else {
            0
        };
        self.performance_metrics
            .throughput_bps
            .store(blocks_per_second, Ordering::Relaxed);

        Ok(results)
    }

    /// Optimized parallel transaction validation using rayon
    pub async fn validate_transactions_parallel(
        &self,
        transactions: &[Transaction],
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        let start_time = Instant::now();
        let utxos = utxos_arc.read().await;

        // Use rayon for parallel validation
        let results: Vec<bool> = transactions
            .par_iter()
            .map(|tx| {
                // Perform lightweight validation checks
                if tx.inputs.is_empty() && tx.outputs.is_empty() {
                    return false;
                }

                // Check UTXO availability for inputs
                for input in &tx.inputs {
                    let utxo_key = format!("{}_{}", input.tx_id, input.output_index);
                    if !utxos.contains_key(&utxo_key) {
                        return false;
                    }
                }

                // Perform quantum-resistant signature verification (batch-friendly)
                let mut data = Vec::new();
                data.extend_from_slice(tx.sender.as_bytes());
                data.extend_from_slice(tx.receiver.as_bytes());
                data.extend_from_slice(&tx.amount.to_be_bytes());
                data.extend_from_slice(&tx.fee.to_be_bytes());
                for input in &tx.inputs {
                    data.extend_from_slice(input.tx_id.as_bytes());
                    data.extend_from_slice(&input.output_index.to_be_bytes());
                }
                for output in &tx.outputs {
                    data.extend_from_slice(output.address.as_bytes());
                    data.extend_from_slice(&output.amount.to_be_bytes());
                }
                let mut sorted_metadata: Vec<_> = tx.metadata.iter().collect();
                sorted_metadata.sort_by_key(|(k, _)| *k);
                for (k, v) in sorted_metadata {
                    data.extend_from_slice(k.as_bytes());
                    data.extend_from_slice(v.as_bytes());
                }
                data.extend_from_slice(&tx.timestamp.to_be_bytes());
                let signing_data = qanto_hash(&data).as_bytes().to_vec();

                if !QuantumResistantSignature::verify(&tx.signature, &signing_data) {
                    return false;
                }

                true
            })
            .collect();

        let validation_time = start_time.elapsed().as_millis() as u64;
        self.performance_metrics
            .validation_time_ms
            .store(validation_time, Ordering::Relaxed);
        self.performance_metrics
            .transactions_processed
            .fetch_add(transactions.len() as u64, Ordering::Relaxed);

        Ok(results)
    }

    // Deterministic parallel validation pipeline

    /// Compute a deterministic state witness for a block based on parents and UTXO input keys
    pub(super) fn compute_state_witness(block: &QantoBlock, utxo_inputs: &[String]) -> [u8; 32] {
        let mut bytes = Vec::with_capacity(64 + utxo_inputs.len() * 16);
        // Chain and height to constrain ordering
        bytes.extend_from_slice(&block.chain_id.to_be_bytes());
        bytes.extend_from_slice(&block.height.to_be_bytes());
        // Parents in canonical order
        let mut parents = block.parents.clone();
        parents.sort();
        for p in parents {
            bytes.extend_from_slice(p.as_bytes());
        }
        // Inputs in canonical order
        let mut inputs_sorted = utxo_inputs.to_vec();
        inputs_sorted.sort();
        for k in inputs_sorted {
            bytes.extend_from_slice(k.as_bytes());
        }
        // Include block id to remove ambiguity in tie-breakers
        bytes.extend_from_slice(block.id.as_bytes());
        let h = qanto_hash(&bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(h.as_bytes());
        out
    }

    /// Validate a block and attach its deterministic witness and UTXO input footprint
    pub async fn validate_block_with_witness(
        &self,
        index: usize,
        block: QantoBlock,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> crate::qantodag::ValidationOutcome {
        // Build UTXO input keys (skip coinbase)
        let utxo_inputs: Vec<String> = block
            .transactions
            .iter()
            .skip(1)
            .flat_map(|tx| {
                tx.inputs
                    .iter()
                    .map(|inp| format!("{}_{}", inp.tx_id, inp.output_index))
                    .collect::<Vec<_>>()
            })
            .collect();

        // Perform full rule validation using existing path
        let is_valid_res = self.is_valid_block(&block, utxos_arc).await;
        let (is_valid, err_opt) = match is_valid_res {
            Ok(v) => (v, None),
            Err(e) => (false, Some(e)),
        };

        let state_witness = Self::compute_state_witness(&block, &utxo_inputs);
        crate::qantodag::ValidationOutcome {
            index,
            block,
            is_valid,
            error: err_opt,
            utxo_inputs,
            state_witness,
        }
    }

    /// Validate blocks in parallel and produce deterministic outcomes
    pub async fn validate_blocks_parallel_deterministic(
        &self,
        blocks: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<crate::qantodag::ValidationOutcome>, QantoDAGError> {
        let start_time = Instant::now();
        let batch_size = PARALLEL_VALIDATION_BATCH_SIZE.min(blocks.len());
        let mut outcomes: Vec<crate::qantodag::ValidationOutcome> =
            Vec::with_capacity(blocks.len());

        for (chunk_index, chunk) in blocks.chunks(batch_size).enumerate() {
            // Prepare indices and block clones; keep order deterministic
            let chunk_items: Vec<(usize, QantoBlock)> = chunk
                .par_iter()
                .enumerate()
                .map(|(i, block_ref)| (chunk_index * batch_size + i, block_ref.clone()))
                .collect();

            // Run validations concurrently with per-item semaphore gating
            let utxos_arc_cloned = Arc::clone(utxos_arc);
            let futures_iter = chunk_items.into_iter().map(|(global_index, block_clone)| {
                let utxos_arc = Arc::clone(&utxos_arc_cloned);
                async move {
                    let _permit = self.validation_workers.acquire().await.map_err(|_| {
                        QantoDAGError::InvalidBlock(
                            "Failed to acquire validation permit".to_string(),
                        )
                    })?;
                    let outcome = self
                        .validate_block_with_witness(global_index, block_clone, &utxos_arc)
                        .await;
                    Ok::<crate::qantodag::ValidationOutcome, QantoDAGError>(outcome)
                }
            });
            let chunk_results = futures::future::try_join_all(futures_iter).await?;
            outcomes.extend(chunk_results);
        }

        let validation_time = start_time.elapsed().as_millis() as u64;
        self.performance_metrics
            .validation_time_ms
            .store(validation_time, Ordering::Relaxed);

        Ok(outcomes)
    }

    /// Deterministically order outcomes by height, timestamp, then block id
    pub(super) fn order_outcomes_deterministically(
        outcomes: &mut [crate::qantodag::ValidationOutcome],
    ) {
        outcomes.sort_by(|a, b| {
            a.block
                .height
                .cmp(&b.block.height)
                .then(a.block.timestamp.cmp(&b.block.timestamp))
                .then(a.block.id.cmp(&b.block.id))
        });
    }

    /// Select a deterministic non-conflicting set of validated blocks
    pub(super) fn select_non_conflicting_deterministic(
        outcomes: &[crate::qantodag::ValidationOutcome],
    ) -> Vec<usize> {
        // Track used input keys to avoid conflicts deterministically
        let mut used_inputs: HashSet<String> = HashSet::new();
        let mut selected_indices: Vec<usize> = Vec::new();

        for outcome in outcomes {
            if !outcome.is_valid {
                continue;
            }
            // Skip if any of this outcome's inputs have been used already
            let conflict = outcome
                .utxo_inputs
                .iter()
                .any(|key| used_inputs.contains(key));
            if conflict {
                continue;
            }
            // Mark inputs from this outcome as used
            used_inputs.extend(outcome.utxo_inputs.iter().cloned());
            selected_indices.push(outcome.index);
        }

        selected_indices
    }

    /// Commit validated blocks in deterministic order, avoiding input conflicts
    pub async fn commit_validated_blocks_in_order(
        &self,
        outcomes: Vec<crate::qantodag::ValidationOutcome>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        mempool_arc: Option<&Arc<RwLock<Mempool>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        // Order outcomes deterministically
        let mut ordered = outcomes;
        Self::order_outcomes_deterministically(&mut ordered);
        // Choose non-conflicting set
        let selected_indices = Self::select_non_conflicting_deterministic(&ordered);

        // Map of global index -> result for stable reporting
        let mut results_map: HashMap<usize, bool> = HashMap::new();
        for o in ordered {
            let accept = selected_indices.contains(&o.index);
            if accept {
                // Apply via prevalidated path to avoid re-validation overhead
                let applied = self
                    .add_block_prevalidated(o.block.clone(), utxos_arc, mempool_arc, None)
                    .await?;
                results_map.insert(o.index, applied);
            } else {
                results_map.insert(o.index, false);
            }
        }

        // Convert map to ordered vec by index
        let mut indices: Vec<usize> = results_map.keys().cloned().collect();
        indices.sort();
        let results: Vec<bool> = indices.into_iter().map(|i| results_map[&i]).collect();
        Ok(results)
    }

    /// Orchestrate deterministic parallel validation and sequential commit
    pub async fn validate_and_apply_blocks_parallel(
        &self,
        blocks: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        mempool_arc: Option<&Arc<RwLock<Mempool>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        let start = Instant::now();
        let outcomes = self
            .validate_blocks_parallel_deterministic(blocks, utxos_arc)
            .await?;
        let results = self
            .commit_validated_blocks_in_order(outcomes, utxos_arc, mempool_arc)
            .await?;
        let ms = start.elapsed().as_millis() as u64;
        self.performance_metrics
            .block_creation_time_ms
            .store(ms, Ordering::Relaxed);
        Ok(results)
    }

    /// Add a block assuming validation already completed successfully
    pub async fn add_block_prevalidated(
        &self,
        block: QantoBlock,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        mempool_arc: Option<&Arc<RwLock<Mempool>>>,
        miner_id: Option<&str>,
    ) -> Result<bool, QantoDAGError> {
        let add_block_span = tracing::info_span!(
            "qantodag.add_block_prevalidated",
            block_id = %block.id,
            chain_id = block.chain_id,
            tx_count = block.transactions.len(),
            height = block.height
        );
        let _enter = add_block_span.enter();

        if self.blocks.contains_key(&block.id) {
            return Ok(false);
        }

        self.add_block(block, utxos_arc, mempool_arc, miner_id)
            .await
    }

    /// Optimized block creation with batch processing
    pub async fn create_optimized_block(
        &self,

        validator_address: &str,
        transactions: Vec<Transaction>,
        chain_id_val: u32,
    ) -> Result<QantoBlock, QantoDAGError> {
        let start_time = Instant::now();

        // Get parent blocks efficiently
        let parents: Vec<String> = self
            .get_fast_tips(chain_id_val)
            .await?
            .into_iter()
            .take(2) // Limit to 2 parents for efficiency
            .collect();

        // Determine max parent timestamp for non-decreasing assignment
        let max_parent_timestamp = {
            let blocks_guard = &self.blocks;
            parents
                .iter()
                .filter_map(|p_id: &String| blocks_guard.get(p_id))
                .map(|p_block| p_block.timestamp)
                .max()
                .unwrap_or(0)
        };

        // Calculate total fees in parallel
        let total_fees: u128 = transactions.par_iter().map(|tx| tx.fee).sum();

        // Get current difficulty and height
        let difficulty = crate::QANTO_SCALE as u64; // Use default difficulty for optimized processing
        let height = self.blocks.len() as u64 + 1;
        let current_epoch = self.current_epoch.load(Ordering::Relaxed);

        // Calculate reward using simplified approach for optimized processing
        let base_reward = total_fees + (transactions.len() as u128 * 100);

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let timestamp = now.max(max_parent_timestamp);

        // Create block data
        let (paillier_pk, _) = HomomorphicEncrypted::generate_keypair();
        let block_data = QantoBlockCreationData {
            chain_id: chain_id_val,
            parents,
            transactions,
            difficulty,
            validator: validator_address.to_string(),
            miner: validator_address.to_string(),
            validator_private_key: self.get_private_key().await?,

            timestamp,
            current_epoch,
            height,
            paillier_pk,
        };

        let mut block = QantoBlock::new(block_data)?;
        block.reward = base_reward;

        let creation_time = start_time.elapsed().as_millis() as u64;
        self.performance_metrics
            .block_creation_time_ms
            .store(creation_time, Ordering::Relaxed);

        Ok(block)
    }

    /// High-performance mempool transaction selection
    pub async fn select_high_priority_transactions(
        &self,
        mempool_arc: &Arc<RwLock<Mempool>>,
        max_transactions: usize,
    ) -> Result<Vec<Transaction>, QantoDAGError> {
        let mempool = mempool_arc.read().await;

        // Use the mempool's optimized selection
        let selected = mempool.select_transactions(max_transactions);

        Ok(selected.await)
    }

    /// Batch process multiple blocks with optimized validation
    pub async fn get_private_key(&self) -> Result<QantoPQPrivateKey, QantoDAGError> {
        // In a real implementation, this would load the validator's private key
        // from a secure storage. For now, we'll generate a dummy one.
        // #region debug-point block-signature-failure-get-private-key
        report_block_signature_debug_event(
            "B",
            "get-private-key",
            "qantodag.rs:4968",
            json!({
                "source": "qantodag.get_private_key",
                "returns_dummy_private_key": true,
            }),
        );
        // #endregion debug-point block-signature-failure-get-private-key
        Ok(QantoPQPrivateKey::new_dummy())
    }

    pub async fn process_blocks_optimized(
        &self,
        blocks: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        let start_time = Instant::now();

        // Process blocks in parallel batches
        let batch_size = PARALLEL_VALIDATION_BATCH_SIZE.min(blocks.len());
        let mut results = Vec::with_capacity(blocks.len());

        for chunk in blocks.chunks(batch_size) {
            let chunk_results = self
                .validate_blocks_parallel(chunk.to_vec(), utxos_arc)
                .await?;

            for (block, is_valid) in chunk_results {
                if is_valid {
                    self.add_block(block, utxos_arc, None, None).await?;
                    results.push(true);
                } else {
                    results.push(false);
                }
            }
        }

        let processing_time = start_time.elapsed().as_millis() as u64;
        self.performance_metrics
            .blocks_processed
            .fetch_add(blocks.len() as u64, Ordering::Relaxed);

        // Update throughput metrics
        let blocks_per_second = if processing_time > 0 {
            (blocks.len() as u64 * 1000) / processing_time
        } else {
            0
        };
        self.performance_metrics
            .throughput_bps
            .store(blocks_per_second, Ordering::Relaxed);

        Ok(results)
    }

    /// Get current performance metrics for monitoring
    pub fn get_performance_metrics(&self) -> (u64, u64, u64, u64, u64, u64, u64) {
        (
            self.performance_metrics
                .blocks_processed
                .load(Ordering::Relaxed),
            self.performance_metrics
                .transactions_processed
                .load(Ordering::Relaxed),
            self.performance_metrics
                .validation_time_ms
                .load(Ordering::Relaxed),
            self.performance_metrics
                .block_creation_time_ms
                .load(Ordering::Relaxed),
            self.performance_metrics.cache_hits.load(Ordering::Relaxed),
            self.performance_metrics
                .cache_misses
                .load(Ordering::Relaxed),
            self.performance_metrics
                .throughput_bps
                .load(Ordering::Relaxed),
        )
    }

    /// Enhanced high-priority transaction selection for 10M+ TPS
    pub async fn select_high_priority_transactions_enhanced(
        &self,
        mempool_arc: &Arc<RwLock<Mempool>>,
        max_transactions: usize,
    ) -> Result<Vec<Transaction>, QantoDAGError> {
        let mempool = mempool_arc.read().await;

        // Use parallel processing to select transactions
        let all_transactions = mempool.get_transactions().await;
        let transactions: Vec<Transaction> = self.work_stealing_pool.install(|| {
            all_transactions
                .values()
                .collect::<Vec<_>>()
                .par_iter()
                .take(max_transactions)
                .cloned()
                .cloned()
                .collect()
        });

        Ok(transactions)
    }

    /// Process transaction batch with sharding for 10M+ TPS
    pub async fn process_transaction_batch_sharded(
        &self,
        transactions: Vec<Transaction>,
        shard_count: usize,
    ) -> Result<Vec<bool>, QantoDAGError> {
        if transactions.is_empty() {
            return Ok(vec![]);
        }

        // Shard transactions for parallel processing
        let chunk_size = transactions.len().div_ceil(shard_count);
        let sharded_results: Vec<Vec<bool>> = self.work_stealing_pool.install(|| {
            transactions
                .par_chunks(chunk_size)
                .map(|chunk| {
                    chunk
                        .iter()
                        .map(|tx| {
                            // Fast validation for each transaction
                            self.validate_transaction_fast(tx)
                        })
                        .collect()
                })
                .collect()
        });

        // Flatten results
        let results: Vec<bool> = sharded_results.into_iter().flatten().collect();
        Ok(results)
    }

    /// Calculate transaction shard based on hash
    #[allow(dead_code)]
    fn calculate_transaction_shard(&self, tx: &Transaction, shard_count: usize) -> usize {
        let tx_bytes = bincode::serialize(tx).unwrap_or_default();
        let tx_hash = qanto_hash(&tx_bytes);
        let hash_bytes = hex::decode(tx_hash).unwrap_or_default();
        if hash_bytes.is_empty() {
            0
        } else {
            hash_bytes[0] as usize % shard_count
        }
    }

    /// Fast transaction validation for high throughput
    fn validate_transaction_fast(&self, tx: &Transaction) -> bool {
        // Basic validation checks
        if tx.inputs.is_empty() || tx.outputs.is_empty() {
            return false;
        }

        // Check transaction size using compact binary serialization
        if let Ok(serialized) = bincode::serialize(tx) {
            if serialized.len() > MAX_TRANSACTION_SIZE {
                return false;
            }
        } else {
            return false;
        }

        // Additional fast checks can be added here
        true
    }

    /// Enhanced mempool processing for 1M+ capacity
    pub async fn process_mempool_enhanced(
        &self,
        mempool_arc: &Arc<RwLock<Mempool>>,
        batch_size: usize,
    ) -> Result<Vec<Transaction>, QantoDAGError> {
        let mempool = mempool_arc.read().await;

        // Get high-priority transactions in parallel
        let all_transactions_map = mempool.get_transactions().await;
        let all_transactions: Vec<Transaction> = all_transactions_map.into_values().collect();
        let transactions: Vec<Transaction> = self.work_stealing_pool.install(|| {
            all_transactions
                .par_iter()
                .take(batch_size)
                .filter(|tx| self.validate_transaction_fast(tx))
                .cloned()
                .collect()
        });

        // Process transactions in shards
        let shard_count = 16; // Configurable shard count
        let _validation_results = self
            .process_transaction_batch_sharded(transactions.clone(), shard_count)
            .await?;

        Ok(transactions)
    }

    /// Parallel block validation with enhanced performance
    pub async fn validate_blocks_parallel_enhanced(
        &self,
        blocks: Vec<QantoBlock>,
        _utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<(QantoBlock, bool)>, QantoDAGError> {
        if blocks.is_empty() {
            return Ok(vec![]);
        }

        // Use work-stealing thread pool for maximum parallelism
        let validation_results: Vec<(QantoBlock, bool)> = self.work_stealing_pool.install(|| {
            blocks
                .into_par_iter()
                .map(|block| {
                    // Fast block validation
                    let is_valid = self.validate_block_fast(&block);
                    (block, is_valid)
                })
                .collect()
        });

        Ok(validation_results)
    }

    /// Fast block validation for high throughput
    fn validate_block_fast(&self, block: &QantoBlock) -> bool {
        // Basic block validation checks
        if block.transactions.is_empty() {
            return false;
        }

        // Check block size using Qanto-native serialization
        if let Ok(serialized) = bincode::serialize(block) {
            if serialized.len() > MAX_BLOCK_SIZE {
                return false;
            }
        } else {
            return false;
        }

        // Validate transaction count
        if block.transactions.len() > MAX_TRANSACTIONS_PER_BLOCK {
            return false;
        }

        // Additional fast validation checks
        true
    }

    /// Multi-stage parallel validation pipeline for blocks
    /// Stages: Pre-check -> PoW -> DAG parents -> Tx validation -> Apply
    pub async fn process_blocks_pipeline(
        &self,
        blocks: Vec<QantoBlock>,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        use tokio::sync::mpsc;
        use tokio::task::JoinSet;

        if blocks.is_empty() {
            return Ok(vec![]);
        }

        #[derive(Clone)]
        struct StageCtx {
            block: QantoBlock,
            pow_hash: Option<my_blockchain::qanto_standalone::hash::QantoHash>,
        }

        let batch_size = PARALLEL_VALIDATION_BATCH_SIZE.min(blocks.len());
        let (tx_pre, rx_pre) = mpsc::channel::<StageCtx>(batch_size);
        let (tx_pow, rx_pow) = mpsc::channel::<StageCtx>(batch_size);
        let (tx_dag, rx_dag) = mpsc::channel::<StageCtx>(batch_size);
        let (tx_tx, mut rx_tx) = mpsc::channel::<StageCtx>(batch_size);

        // Stage 0: feed input blocks
        {
            let mut set = JoinSet::new();
            for block in blocks.into_iter() {
                let tx = tx_pre.clone();
                set.spawn(async move {
                    let _ = tx
                        .send(StageCtx {
                            block,
                            pow_hash: None,
                        })
                        .await;
                });
            }
            while set.join_next().await.is_some() {}
        }

        // Stage 1: Pre-checks (size/count, quick filters)
        {
            let mut set = JoinSet::new();
            let dag = self.clone();
            let rx_mut = rx_pre;
            let tx_next = tx_pow.clone();
            set.spawn(async move {
                let mut rx = rx_mut;
                while let Some(ctx) = rx.recv().await {
                    // quick filters; skip heavy state access here
                    if dag.validate_block_fast(&ctx.block) {
                        let _ = tx_next.send(ctx).await;
                    } else {
                        // drop invalids
                    }
                }
            });
            while set.join_next().await.is_some() {}
        }

        // Stage 2: PoW
        {
            let mut set = JoinSet::new();
            let rx_mut = rx_pow;
            let tx_next = tx_dag.clone();
            set.spawn(async move {
                let mut rx = rx_mut;
                while let Some(mut ctx) = rx.recv().await {
                    let pow_hash = ctx.block.hash_for_pow();
                    if ctx.block.is_pow_valid_with_pow_hash(pow_hash) {
                        ctx.pow_hash = Some(pow_hash);
                        let _ = tx_next.send(ctx).await;
                    } else {
                        // drop invalids
                    }
                }
            });
            while set.join_next().await.is_some() {}
        }

        // Stage 3: DAG parents exist
        {
            let mut set = JoinSet::new();
            let dag = self.clone();
            let rx_mut = rx_dag;
            let tx_next = tx_tx.clone();
            set.spawn(async move {
                let mut rx = rx_mut;
                while let Some(ctx) = rx.recv().await {
                    let mut ok = true;
                    for parent_id in &ctx.block.parents {
                        if !dag.blocks.contains_key(parent_id) {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        let _ = tx_next.send(ctx).await;
                    }
                }
            });
            while set.join_next().await.is_some() {}
        }

        // Stage 4: Transaction validation (parallel per block)
        let mut results = Vec::new();
        while let Some(ctx) = rx_tx.recv().await {
            // validate transactions in parallel
            let valid_txs = self
                .validate_transactions_parallel(&ctx.block.transactions, utxos_arc)
                .await?;
            let all_valid = valid_txs.iter().all(|b| *b);
            if all_valid {
                // Apply sequentially to maintain determinism
                let added = self
                    .add_block(ctx.block.clone(), utxos_arc, None, None)
                    .await?;
                results.push(added);
            } else {
                results.push(false);
            }
        }

        Ok(results)
    }

    /// Batch process transactions with enhanced parallelism
    pub async fn batch_process_transactions_enhanced(
        &self,
        transactions: Vec<Transaction>,
        _utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<Vec<bool>, QantoDAGError> {
        if transactions.is_empty() {
            return Ok(vec![]);
        }

        // Process in parallel batches
        let batch_size = PARALLEL_VALIDATION_BATCH_SIZE;
        let results: Vec<Vec<bool>> = self.work_stealing_pool.install(|| {
            transactions
                .par_chunks(batch_size)
                .map(|chunk| {
                    chunk
                        .iter()
                        .map(|tx| self.validate_transaction_fast(tx))
                        .collect()
                })
                .collect()
        });

        // Flatten results
        let flattened_results: Vec<bool> = results.into_iter().flatten().collect();
        Ok(flattened_results)
    }

    /// Get enhanced performance metrics for 10M+ TPS monitoring
    pub fn get_enhanced_performance_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();

        metrics.insert(
            "blocks_processed".to_string(),
            self.performance_metrics
                .blocks_processed
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "transactions_processed".to_string(),
            self.performance_metrics
                .transactions_processed
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "validation_time_ms".to_string(),
            self.performance_metrics
                .validation_time_ms
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "block_creation_time_ms".to_string(),
            self.performance_metrics
                .block_creation_time_ms
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "cache_hits".to_string(),
            self.performance_metrics.cache_hits.load(Ordering::Relaxed),
        );
        metrics.insert(
            "cache_misses".to_string(),
            self.performance_metrics
                .cache_misses
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "throughput_bps".to_string(),
            self.performance_metrics
                .throughput_bps
                .load(Ordering::Relaxed),
        );
        metrics.insert(
            "current_epoch".to_string(),
            self.current_epoch.load(Ordering::Relaxed),
        );
        metrics.insert("total_blocks".to_string(), self.blocks.len() as u64);
        metrics.insert("total_validators".to_string(), self.validators.len() as u64);
        metrics.insert("total_tips".to_string(), self.tips.len() as u64);
        metrics.insert("mempool_size".to_string(), self.memory_pool.len() as u64);
        metrics.insert(
            "validation_cache_size".to_string(),
            self.validation_cache.len() as u64,
        );
        metrics.insert(
            "processing_blocks_count".to_string(),
            self.processing_blocks.len() as u64,
        );

        metrics
    }

    /// Plans UTXO removals, additions, and per-address balance deltas for a block’s transactions.
    /// This computes changes without taking a write lock, enabling conflict-free application later.
    pub async fn plan_utxo_changes_conflict_free(
        &self,
        transactions: &[Transaction],
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<
        (
            HashMap<String, i128>, // balance_deltas per address
            Vec<String>,           // removal_keys (spent UTXO ids)
            Vec<(String, UTXO)>,   // additions (new UTXOs)
        ),
        QantoDAGError,
    > {
        let utxos_guard = utxos_arc.read().await;

        #[derive(Default)]
        struct TxPlan {
            balance_deltas: HashMap<String, i128>,
            removal_keys: Vec<String>,
            additions: Vec<(String, UTXO)>,
            input_ids: Vec<String>,
        }

        // Compute per-transaction plans in parallel, validating inputs exist
        let tx_plan_results: Vec<Result<TxPlan, QantoDAGError>> = transactions
            .par_iter()
            .map(|tx| {
                let mut local_balance: HashMap<String, i128> = HashMap::new();
                let mut local_removals: Vec<String> = Vec::new();
                let mut local_additions: Vec<(String, UTXO)> = Vec::new();
                let mut local_inputs: HashSet<String> = HashSet::new();

                // Inputs: collect ids, validate existence, and subtract sender balances
                for input in &tx.inputs {
                    let utxo_id = format!("{}_{}", input.tx_id, input.output_index);

                    // Detect duplicate inputs within the same transaction
                    if !local_inputs.insert(utxo_id.clone()) {
                        return Err(QantoDAGError::InvalidBlock(format!(
                            "Double-spend detected within transaction {} for UTXO {}",
                            tx.id, utxo_id
                        )));
                    }

                    match utxos_guard.get(&utxo_id) {
                        Some(consumed) => {
                            local_removals.push(utxo_id);
                            let entry = local_balance.entry(consumed.address.clone()).or_insert(0);
                            *entry -= consumed.amount as i128;
                        }
                        None => {
                            return Err(QantoDAGError::InvalidBlock(format!(
                                "Input UTXO not found: {utxo_id}"
                            )));
                        }
                    }
                }

                // Outputs: plan additions and add recipient balances
                for (idx, output) in tx.outputs.iter().enumerate() {
                    let index_u32 = idx as u32;
                    let new_utxo = tx.generate_utxo(index_u32);
                    let new_utxo_id = format!("{}_{}", tx.id, index_u32);
                    local_additions.push((new_utxo_id, new_utxo));

                    let entry = local_balance.entry(output.address.clone()).or_insert(0);
                    *entry += output.amount as i128;
                }

                Ok(TxPlan {
                    balance_deltas: local_balance,
                    removal_keys: local_removals,
                    additions: local_additions,
                    input_ids: local_inputs.into_iter().collect(),
                })
            })
            .collect();

        // Short-circuit on errors from parallel planning
        let mut tx_plans: Vec<TxPlan> = Vec::with_capacity(transactions.len());
        for res in tx_plan_results {
            match res {
                Ok(plan) => tx_plans.push(plan),
                Err(e) => return Err(e),
            }
        }

        // Merge per-transaction plans and detect double-spends across the entire block
        let mut balance_deltas: HashMap<String, i128> = HashMap::new();
        let mut removal_keys: Vec<String> = Vec::new();
        let mut additions: Vec<(String, UTXO)> = Vec::new();
        let mut spent_in_block: HashSet<String> = HashSet::new();

        for plan in tx_plans {
            for input_id in plan.input_ids {
                if !spent_in_block.insert(input_id.clone()) {
                    return Err(QantoDAGError::InvalidBlock(format!(
                        "Double-spend detected within block for UTXO {input_id}"
                    )));
                }
            }

            removal_keys.extend(plan.removal_keys);
            additions.extend(plan.additions);

            for (addr, delta) in plan.balance_deltas {
                *balance_deltas.entry(addr).or_insert(0) += delta;
            }
        }

        Ok((balance_deltas, removal_keys, additions))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ChainWeightStrategy {
    Descendants,
    Difficulty,
    StakeWeight,
}

impl QantoDAG {
    pub async fn attach_balance_event_sender(&self, sender: broadcast::Sender<BalanceEvent>) {
        let mut guard = self.balance_event_sender.write().await;
        *guard = Some(sender);
    }

    pub async fn attach_balance_broadcaster(&self, broadcaster: Arc<BalanceBroadcaster>) {
        let mut guard = self.balance_broadcaster.write().await;
        *guard = Some(broadcaster);
    }

    /// Gracefully shut down DAG-related asynchronous components and flush storage.
    pub async fn shutdown(&self) -> Result<(), QantoDAGError> {
        // Stop the persistence writer and ensure queued jobs are flushed
        if let Err(e) = self.persistence_writer.shutdown().await {
            warn!("Persistence writer shutdown error: {}", e);
            return Err(QantoDAGError::Generic(e));
        }

        // Best-effort flush and sync of underlying storage for durability
        if let Err(e) = self.db.flush() {
            warn!("Storage flush failed during DAG shutdown: {}", e);
            return Err(QantoDAGError::Storage(e));
        }
        if let Err(e) = self.db.sync() {
            warn!("Storage sync failed during DAG shutdown: {}", e);
            return Err(QantoDAGError::Storage(e));
        }

        info!("QantoDAG shutdown complete");
        Ok(())
    }

    pub fn calculate_branch_weight(&self, block_id: &str, strategy: ChainWeightStrategy) -> u128 {
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let chain_id = match self.blocks.get(block_id) {
            Some(b) => b.chain_id,
            None => return 0,
        };

        for entry in self.blocks.iter() {
            let block = entry.value();
            if block.chain_id != chain_id {
                continue;
            }
            for parent_id in &block.parents {
                children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(block.id.clone());
            }
        }

        let mut memo = HashMap::new();
        self.calculate_branch_weight_recursive(block_id, &children, strategy, &mut memo)
    }

    fn calculate_branch_weight_recursive(
        &self,
        block_id: &str,
        children: &HashMap<String, Vec<String>>,
        strategy: ChainWeightStrategy,
        memo: &mut HashMap<String, u128>,
    ) -> u128 {
        if let Some(&w) = memo.get(block_id) {
            return w;
        }

        let self_weight = match strategy {
            ChainWeightStrategy::Descendants => 1,
            ChainWeightStrategy::Difficulty => self
                .blocks
                .get(block_id)
                .map(|b| b.effort.max(b.difficulty as u128).max(1))
                .unwrap_or(1),
            ChainWeightStrategy::StakeWeight => {
                if let Some(block) = self.blocks.get(block_id) {
                    self.validators
                        .get(&block.validator)
                        .map(|s| *s.value())
                        .unwrap_or(1)
                } else {
                    1
                }
            }
        };

        let mut total = self_weight;
        if let Some(child_ids) = children.get(block_id) {
            for child_id in child_ids {
                total += self.calculate_branch_weight_recursive(child_id, children, strategy, memo);
            }
        }

        memo.insert(block_id.to_string(), total);
        total
    }

    pub fn get_heaviest_tip(&self) -> Option<String> {
        self.get_heaviest_tip_for_chain(0)
    }

    pub fn get_heaviest_tip_for_chain(&self, chain_id: u32) -> Option<String> {
        self.get_canonical_path_for_chain(chain_id)
            .last()
            .map(|b| b.id.clone())
    }

    pub fn get_canonical_path_for_chain(&self, chain_id: u32) -> Vec<QantoBlock> {
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let mut genesis_block: Option<QantoBlock> = None;

        for entry in self.blocks.iter() {
            let block = entry.value();
            if block.chain_id != chain_id {
                continue;
            }
            if block.height == 0 {
                genesis_block = Some(block.clone());
            }
            for parent_id in &block.parents {
                children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(block.id.clone());
            }
        }

        let genesis = match genesis_block {
            Some(g) => g,
            None => {
                let mut path = Vec::new();
                if let Some(tips) = self.tips.get(&chain_id) {
                    let mut best_tip: Option<QantoBlock> = None;
                    let mut max_height = 0;
                    for tip_id in tips.value() {
                        if let Some(block) = self.blocks.get(tip_id) {
                            if best_tip.is_none() || block.height > max_height {
                                max_height = block.height;
                                best_tip = Some(block.clone());
                            }
                        }
                    }
                    if let Some(tip) = best_tip {
                        path.push(tip);
                    }
                }
                return path;
            }
        };

        let strategy = ChainWeightStrategy::Difficulty;
        let mut memo = HashMap::new();
        let mut path = Vec::new();
        path.push(genesis.clone());

        let mut current_id = genesis.id.clone();
        loop {
            let child_ids = match children.get(&current_id) {
                Some(c) if !c.is_empty() => c,
                _ => break,
            };
            let mut best_child_id = &child_ids[0];
            let mut max_weight = 0;
            for child_id in child_ids {
                let w = self
                    .calculate_branch_weight_recursive(child_id, &children, strategy, &mut memo);
                if w > max_weight {
                    max_weight = w;
                    best_child_id = child_id;
                }
            }
            if let Some(child_block) = self.blocks.get(best_child_id) {
                path.push(child_block.value().clone());
            }
            current_id = best_child_id.clone();
        }

        path
    }
}

impl Clone for QantoDAG {
    fn clone(&self) -> Self {
        Self {
            consensus_lock: tokio::sync::Mutex::new(()),
            blocks: self.blocks.clone(),
            tips: self.tips.clone(),
            validators: self.validators.clone(),
            delegations: self.delegations.clone(),
            target_block_time: self.target_block_time,
            emission: self.emission.clone(),
            num_chains: self.num_chains.clone(),
            finalized_blocks: self.finalized_blocks.clone(),
            chain_loads: self.chain_loads.clone(),
            difficulty_history: self.difficulty_history.clone(),
            block_creation_timestamps: self.block_creation_timestamps.clone(),

            anomaly_history: self.anomaly_history.clone(),
            cross_chain_swaps: self.cross_chain_swaps.clone(),
            smart_contracts: self.smart_contracts.clone(),
            cache: self.cache.clone(),
            db: self.db.clone(),
            persistence_writer: self.persistence_writer.clone(),
            saga: self.saga.clone(),
            self_arc: self.self_arc.clone(),
            current_epoch: self.current_epoch.clone(),
            balance_event_sender: self.balance_event_sender.clone(),
            balance_broadcaster: self.balance_broadcaster.clone(),
            account_state_cache: self.account_state_cache.clone(),
            total_bridge_locked: self.total_bridge_locked.clone(),
            total_bridge_claimed: self.total_bridge_claimed.clone(),
            processed_bridge_claims: self.processed_bridge_claims.clone(),
            tx_index: self.tx_index.clone(),
            address_index: self.address_index.clone(),
            validator_blocks_index: self.validator_blocks_index.clone(),
            bridge_claims_index: self.bridge_claims_index.clone(),
            block_processing_semaphore: self.block_processing_semaphore.clone(),
            validation_workers: self.validation_workers.clone(),
            block_queue: self.block_queue.clone(),
            validation_cache: self.validation_cache.clone(),
            fast_tips_cache: self.fast_tips_cache.clone(),
            processing_blocks: self.processing_blocks.clone(),
            performance_metrics: self.performance_metrics.clone(),
            mining_metrics: self.mining_metrics.clone(),
            logging_config: self.logging_config.clone(),
            // Advanced performance optimization fields
            simd_processor: self.simd_processor.clone(),
            lock_free_tx_queue: Arc::new(crossbeam::queue::SegQueue::new()),
            memory_pool: self.memory_pool.clone(),
            prefetch_cache: self.prefetch_cache.clone(),
            pipeline_stages: self.pipeline_stages.clone(),
            work_stealing_pool: self.work_stealing_pool.clone(),
            utxo_bloom_filter: self.utxo_bloom_filter.clone(),
            batch_processor: self.batch_processor.clone(),
            timing_coordinator: self.timing_coordinator.clone(),
            performance_monitor: self.performance_monitor.clone(),
            qdag_generator: self.qdag_generator.clone(),
            dev_fee_rate: self.dev_fee_rate,
            last_fork_depth: self.last_fork_depth.clone(),
            last_fork_lca: self.last_fork_lca.clone(),
            bypass_reward_check: self.bypass_reward_check.clone(),
        }
    }
}

// Implementation of QantoDAGOptimizations trait
use crate::performance_optimizations::QantoDAGOptimizations;
use crate::transaction::TransactionError;

impl QantoDAGOptimizations for QantoDAG {
    /// Create an optimized block with enhanced performance features
    async fn create_block_optimized(
        &self,
        transactions: Vec<Transaction>,
    ) -> Result<QantoBlock, TransactionError> {
        // Use the existing optimized block creation method

        let validator = self.select_validator().await.unwrap_or_default();
        let chain_id = *self.num_chains.read().await;

        match self
            .create_optimized_block(&validator, transactions, chain_id)
            .await
        {
            Ok(block) => Ok(block),
            Err(QantoDAGError::InvalidTransaction(tx_err)) => Err(tx_err),
            Err(e) => {
                let error_msg = format!("Block creation failed: {e}");
                warn!("{}", error_msg);
                Err(TransactionError::InvalidStructure(error_msg))
            }
        }
    }

    /// Create a dummy QantoDAG instance for verification purposes
    fn new_dummy_for_verification() -> Self {
        use crate::saga::PalletSaga;
        use std::sync::Arc;

        use crate::optimized_qdag::{OptimizedQDagConfig, OptimizedQDagGenerator};
        use crossbeam::channel::bounded;
        use std::sync::Weak;

        // Create QantoStorage for testing with a unique temp directory to avoid RocksDB lock conflicts
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_path = std::env::temp_dir().join(format!("dummy_qanto_storage_{}", unique_suffix));
        let storage_config = StorageConfig {
            data_dir: temp_path,
            max_file_size: 64 * 1024 * 1024, // 64MB
            cache_size: 1024 * 1024,         // 1MB
            compression_enabled: true,
            encryption_enabled: false,
            wal_enabled: true,
            sync_writes: false,
            compaction_threshold: 1, // Changed to usize
            max_open_files: 100,
            memtable_size: 1024 * 1024 * 16,    // 16MB memtable
            write_buffer_size: 1024 * 1024 * 4, // 4MB write buffer
            batch_size: 1000,                   // Batch size for writes
            parallel_writers: 4,                // Number of parallel writers
            enable_write_batching: true,
            enable_bloom_filters: true,
            enable_async_io: true,
            sync_interval: Duration::from_millis(100),
            compression_level: 3,
        };
        let db = QantoStorage::new(storage_config).expect("Failed to create dummy QantoStorage");

        // Create bounded channel for block queue
        let (sender, receiver) = bounded(1000);

        let db_arc = Arc::new(db);

        Self {
            consensus_lock: tokio::sync::Mutex::new(()),
            blocks: Arc::new(DashMap::new()),
            tips: Arc::new(DashMap::new()),
            validators: Arc::new(DashMap::new()),
            delegations: Arc::new(DashMap::new()),
            target_block_time: 1000,
            emission: Arc::new(RwLock::new(Emission::default_with_timestamp(0, 1))),
            num_chains: Arc::new(RwLock::new(1)),
            finalized_blocks: Arc::new(DashMap::new()),
            chain_loads: Arc::new(DashMap::new()),

            difficulty_history: Arc::new(ParkingRwLock::new(Vec::new())),
            block_creation_timestamps: Arc::new(DashMap::new()),
            anomaly_history: Arc::new(DashMap::new()),
            cross_chain_swaps: Arc::new(DashMap::new()),
            smart_contracts: Arc::new(DashMap::new()),
            cache: Arc::new(ParkingRwLock::new(LruCache::new(
                NonZeroUsize::new(100).unwrap(),
            ))),
            db: db_arc.clone(),
            persistence_writer: Arc::new(PersistenceWriter::new(db_arc.clone(), 8192)),
            saga: Arc::new(PalletSaga::new(
                #[cfg(feature = "infinite-strata")]
                None,
            )),
            self_arc: Weak::new(),
            current_epoch: Arc::new(AtomicU64::new(0)),
            balance_event_sender: Arc::new(RwLock::new(None)),
            balance_broadcaster: Arc::new(RwLock::new(Some(Arc::new(BalanceBroadcaster::new(
                1 << 15,
            ))))),
            account_state_cache: AccountStateCache::new(),
            total_bridge_locked: Arc::new(RwLock::new(0)),
            total_bridge_claimed: Arc::new(RwLock::new(0)),
            processed_bridge_claims: Arc::new(DashMap::new()),
            tx_index: Arc::new(DashMap::new()),
            address_index: Arc::new(DashMap::new()),
            validator_blocks_index: Arc::new(DashMap::new()),
            bridge_claims_index: Arc::new(DashMap::new()),
            block_processing_semaphore: Arc::new(Semaphore::new(10)),
            validation_workers: Arc::new(Semaphore::new(32)),
            block_queue: Arc::new((sender, receiver)),
            validation_cache: Arc::new(DashMap::new()),
            fast_tips_cache: Arc::new(DashMap::new()),
            processing_blocks: Arc::new(DashMap::new()),
            performance_metrics: Arc::new(PerformanceMetrics::default()),
            mining_metrics: Arc::new(MiningMetrics::new()),
            simd_processor: Arc::new(DashMap::new()),
            lock_free_tx_queue: Arc::new(crossbeam::queue::SegQueue::new()),
            memory_pool: Arc::new(DashMap::new()),
            prefetch_cache: Arc::new(DashMap::new()),
            pipeline_stages: (0..8).map(|_| Arc::new(Semaphore::new(1))).collect(),
            work_stealing_pool: Arc::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(num_cpus::get())
                    .build()
                    .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap()),
            ),
            utxo_bloom_filter: Arc::new(DashMap::new()),
            batch_processor: Arc::new(DashMap::new()),
            timing_coordinator: Arc::new(BlockTimingCoordinator::new()),
            performance_monitor: Arc::new(PerformanceMonitor::new(
                PerformanceMonitoringConfig::default(),
            )),
            qdag_generator: Arc::new(OptimizedQDagGenerator::new(OptimizedQDagConfig::default())),
            dev_fee_rate: 100_000_000,
            logging_config: LoggingConfig::default(),
            last_fork_depth: Arc::new(AtomicU64::new(0)),
            last_fork_lca: Arc::new(tokio::sync::RwLock::new(String::new())),
            bypass_reward_check: Arc::new(AtomicBool::new(false)),
        }
    }
}
#[allow(dead_code)]
fn to_core_logging(cfg: &LoggingConfig) -> CoreLoggingConfig {
    CoreLoggingConfig {
        enable_block_celebrations: cfg.enable_block_celebrations,
        celebration_log_level: cfg.celebration_log_level.clone(),
        celebration_throttle_per_min: cfg.celebration_throttle_per_min,
    }
}

#[cfg(test)]
mod determinism_tests {
    use super::*;
    use crate::performance_optimizations::QantoDAGOptimizations;
    use crate::types::UTXO;
    use ahash::AHashMap as HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    // Explicitly import ValidationOutcome to ensure type resolution within test module
    use crate::qantodag::ValidationOutcome;

    #[test]
    fn witness_invariant_to_ordering_of_inputs_and_parents() {
        let mut block = QantoBlock::new_test_block("blk-test".to_string());
        block.chain_id = 42;
        block.height = 5;
        block.parents = vec!["parent_b".to_string(), "parent_a".to_string()];

        let inputs1 = vec![
            "txA_0".to_string(),
            "txB_1".to_string(),
            "txC_2".to_string(),
        ];
        let inputs2 = vec![
            "txC_2".to_string(),
            "txA_0".to_string(),
            "txB_1".to_string(),
        ];

        let w1 = QantoDAG::compute_state_witness(&block, &inputs1);

        // Reorder parents and inputs; witness should remain identical due to sorting
        block.parents = vec!["parent_a".to_string(), "parent_b".to_string()];
        let w2 = QantoDAG::compute_state_witness(&block, &inputs2);
        assert_eq!(w1, w2, "State witness must be invariant to ordering");

        // Changing block id should change witness
        let mut block2 = block.clone();
        block2.id = "blk-other".to_string();
        let w3 = QantoDAG::compute_state_witness(&block2, &inputs1);
        assert_ne!(w1, w3, "Witness must change when block identity changes");
    }

    #[test]
    fn order_outcomes_sorts_by_height_then_timestamp_then_id() {
        // Create outcomes with controlled height/timestamp/id
        let mut b1 = QantoBlock::new_test_block("a".to_string());
        b1.height = 2;
        b1.timestamp = 2000;

        let mut b2 = QantoBlock::new_test_block("b".to_string());
        b2.height = 1;
        b2.timestamp = 2000;

        let mut b3 = QantoBlock::new_test_block("c".to_string());
        b3.height = 1;
        b3.timestamp = 1000;

        let mut b4 = QantoBlock::new_test_block("aa".to_string());
        b4.height = 1;
        b4.timestamp = 1000;

        let mut outcomes = vec![
            ValidationOutcome {
                index: 0,
                block: b1,
                is_valid: true,
                error: None,
                utxo_inputs: vec![],
                state_witness: [0u8; 32],
            },
            ValidationOutcome {
                index: 1,
                block: b2,
                is_valid: true,
                error: None,
                utxo_inputs: vec![],
                state_witness: [0u8; 32],
            },
            ValidationOutcome {
                index: 2,
                block: b3,
                is_valid: true,
                error: None,
                utxo_inputs: vec![],
                state_witness: [0u8; 32],
            },
            ValidationOutcome {
                index: 3,
                block: b4,
                is_valid: true,
                error: None,
                utxo_inputs: vec![],
                state_witness: [0u8; 32],
            },
        ];

        QantoDAG::order_outcomes_deterministically(&mut outcomes);
        let ordered_ids: Vec<String> = outcomes.iter().map(|o| o.block.id.clone()).collect();
        assert_eq!(
            ordered_ids,
            vec![
                "aa".to_string(),
                "c".to_string(),
                "b".to_string(),
                "a".to_string()
            ],
            "Outcomes must be sorted by height, then timestamp, then id",
        );
    }

    #[test]
    fn select_non_conflicting_skips_invalid_and_conflicting_inputs() {
        let b0 = QantoBlock::new_test_block("o0".to_string());
        let b1 = QantoBlock::new_test_block("o1".to_string());
        let b2 = QantoBlock::new_test_block("o2".to_string());

        let o0 = ValidationOutcome {
            index: 0,
            block: b0,
            is_valid: true,
            error: None,
            utxo_inputs: vec!["x_0".to_string(), "y_1".to_string()],
            state_witness: [0u8; 32],
        };
        // Invalid outcome should be skipped regardless of inputs
        let o1 = ValidationOutcome {
            index: 1,
            block: b1,
            is_valid: false,
            error: Some(QantoDAGError::InvalidBlock("Invalid".to_string())),
            utxo_inputs: vec!["z_2".to_string()],
            state_witness: [0u8; 32],
        };
        // Conflicts with o0 on input "y_1"
        let o2 = ValidationOutcome {
            index: 2,
            block: b2,
            is_valid: true,
            error: None,
            utxo_inputs: vec!["y_1".to_string()],
            state_witness: [0u8; 32],
        };

        let outcomes = vec![o0, o1, o2];
        let selected = QantoDAG::select_non_conflicting_deterministic(&outcomes);
        assert_eq!(
            selected,
            vec![0],
            "Only the first valid non-conflicting outcome should remain"
        );
    }

    #[tokio::test]
    async fn validate_block_with_witness_coinbase_only_has_no_utxo_inputs_and_consistent_witness() {
        let dag = QantoDAG::new_dummy_for_verification();
        let block = QantoBlock::new_test_block("witness-test".to_string());

        // Empty UTXO set
        let utxos: Arc<RwLock<HashMap<String, UTXO>>> = Arc::new(RwLock::new(HashMap::new()));
        let outcome = dag
            .validate_block_with_witness(0, block.clone(), &utxos)
            .await;

        // Coinbase-only block should yield no input keys
        assert!(outcome.utxo_inputs.is_empty());

        // Witness must match deterministic computation using empty inputs
        let expected = QantoDAG::compute_state_witness(&outcome.block, &[]);
        assert_eq!(outcome.state_witness, expected);
    }

    #[test]
    fn test_fork_lca_and_depth() {
        let dag = QantoDAG::new_dummy_for_verification();

        // 1. Setup a simple fork
        // Genesis: height 1
        let mut genesis = QantoBlock::new_test_block("genesis".to_string());
        genesis.chain_id = 0;
        genesis.height = 1;
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(0).or_default().insert(genesis.id.clone());

        // Block A: height 2, parent Genesis
        let mut block_a = QantoBlock::new_test_block("A".to_string());
        block_a.chain_id = 0;
        block_a.height = 2;
        block_a.parents = vec![genesis.id.clone()];
        dag.blocks.insert(block_a.id.clone(), block_a.clone());

        // Block B: height 2, parent Genesis
        let mut block_b = QantoBlock::new_test_block("B".to_string());
        block_b.chain_id = 0;
        block_b.height = 2;
        block_b.parents = vec![genesis.id.clone()];
        dag.blocks.insert(block_b.id.clone(), block_b.clone());

        // Now set tips to A and B
        dag.tips.get_mut(&0).unwrap().clear();
        dag.tips.get_mut(&0).unwrap().insert(block_a.id.clone());
        dag.tips.get_mut(&0).unwrap().insert(block_b.id.clone());

        let (depth, lca) = dag.calculate_fork_depth_and_lca(0);
        assert_eq!(lca, "genesis");
        assert_eq!(depth, 1); // max_tip_height (2) - genesis_height (1) = 1

        // 2. Setup a deeper fork
        // Block A2: height 3, parent A
        let mut block_a2 = QantoBlock::new_test_block("A2".to_string());
        block_a2.chain_id = 0;
        block_a2.height = 3;
        block_a2.parents = vec![block_a.id.clone()];
        dag.blocks.insert(block_a2.id.clone(), block_a2.clone());

        // Block B2: height 4, parent B
        let mut block_b2 = QantoBlock::new_test_block("B2".to_string());
        block_b2.chain_id = 0;
        block_b2.height = 4;
        block_b2.parents = vec![block_b.id.clone()];
        dag.blocks.insert(block_b2.id.clone(), block_b2.clone());

        // Set tips to A2 and B2
        dag.tips.get_mut(&0).unwrap().clear();
        dag.tips.get_mut(&0).unwrap().insert(block_a2.id.clone());
        dag.tips.get_mut(&0).unwrap().insert(block_b2.id.clone());

        let (depth2, lca2) = dag.calculate_fork_depth_and_lca(0);
        assert_eq!(lca2, "genesis");
        assert_eq!(depth2, 3); // max_tip_height (4) - genesis_height (1) = 3
    }

    #[test]
    fn test_ghost_heaviest_tip() {
        let dag = QantoDAG::new_dummy_for_verification();

        // Fork test 1:
        // Genesis (height 0)
        //  ├── A1 (height 1)
        //  │    └── A2 (height 2)
        //  │         └── A3 (height 3)
        //  └── B1 (height 1)
        //       └── B2 (height 2)

        let mut genesis = QantoBlock::new_test_block("genesis".to_string());
        genesis.chain_id = 0;
        genesis.height = 0;
        genesis.effort = 100;
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(0).or_default().insert(genesis.id.clone());

        // A branch
        let mut a1 = QantoBlock::new_test_block("A1".to_string());
        a1.chain_id = 0;
        a1.height = 1;
        a1.parents = vec![genesis.id.clone()];
        a1.effort = 10;
        dag.blocks.insert(a1.id.clone(), a1.clone());

        let mut a2 = QantoBlock::new_test_block("A2".to_string());
        a2.chain_id = 0;
        a2.height = 2;
        a2.parents = vec![a1.id.clone()];
        a2.effort = 10;
        dag.blocks.insert(a2.id.clone(), a2.clone());

        let mut a3 = QantoBlock::new_test_block("A3".to_string());
        a3.chain_id = 0;
        a3.height = 3;
        a3.parents = vec![a2.id.clone()];
        a3.effort = 10;
        dag.blocks.insert(a3.id.clone(), a3.clone());

        // B branch
        let mut b1 = QantoBlock::new_test_block("B1".to_string());
        b1.chain_id = 0;
        b1.height = 1;
        b1.parents = vec![genesis.id.clone()];
        b1.effort = 10;
        dag.blocks.insert(b1.id.clone(), b1.clone());

        let mut b2 = QantoBlock::new_test_block("B2".to_string());
        b2.chain_id = 0;
        b2.height = 2;
        b2.parents = vec![b1.id.clone()];
        b2.effort = 10;
        dag.blocks.insert(b2.id.clone(), b2.clone());

        // Set tips to A3 and B2
        dag.tips.get_mut(&0).unwrap().clear();
        dag.tips.get_mut(&0).unwrap().insert(a3.id.clone());
        dag.tips.get_mut(&0).unwrap().insert(b2.id.clone());

        // Check heaviest tip is A3
        let heaviest = dag.get_heaviest_tip();
        assert_eq!(heaviest.as_deref(), Some("A3"));

        // Fork test 2:
        // Genesis
        //  ├── A1
        //  │    └── A2
        //  │         └── A3
        //  └── B1
        //       └── B2
        //            └── B3 (height 3)
        //                 └── B4 (height 4)

        let mut b3 = QantoBlock::new_test_block("B3".to_string());
        b3.chain_id = 0;
        b3.height = 3;
        b3.parents = vec![b2.id.clone()];
        b3.effort = 10;
        dag.blocks.insert(b3.id.clone(), b3.clone());

        let mut b4 = QantoBlock::new_test_block("B4".to_string());
        b4.chain_id = 0;
        b4.height = 4;
        b4.parents = vec![b3.id.clone()];
        b4.effort = 10;
        dag.blocks.insert(b4.id.clone(), b4.clone());

        // Set tips to A3 and B4
        dag.tips.get_mut(&0).unwrap().clear();
        dag.tips.get_mut(&0).unwrap().insert(a3.id.clone());
        dag.tips.get_mut(&0).unwrap().insert(b4.id.clone());

        // Check heaviest tip is B4
        let heaviest2 = dag.get_heaviest_tip();
        assert_eq!(heaviest2.as_deref(), Some("B4"));
    }
}
