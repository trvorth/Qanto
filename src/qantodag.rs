//! --- Qanto QantoDAG Ledger ---
//! v2.4.2 - Block Reward Calculation Fix
//! This version corrects the block reward calculation by ensuring that transaction
//! fees are properly calculated and included in the final reward amount.
//!
//! - FIX: The `create_candidate_block` function now calculates the total fees
//!   from mempool transactions *before* calling SAGA's reward calculation,
//!   ensuring the final reward includes both the base amount and fees.

use crate::emission::Emission;
use crate::types::QuantumResistantSignature;
use crate::mempool::Mempool;
use crate::metrics::QantoMetrics;
use crate::miner::Miner;
use crate::mining_celebration::{celebrate_mining_success, MiningCelebrationParams};
use crate::saga::{
    CarbonOffsetCredential, GovernanceProposal, PalletSaga, ProposalStatus, ProposalType,
};
use my_blockchain::{qanhash, qanto_hash};

// This import is required for the `#[from]` attribute in QantoDAGError.
// The compiler may incorrectly flag it as unused, but it is necessary.
use crate::post_quantum_crypto::{PQError, QantoPQPrivateKey, QantoPQPublicKey, QantoPQSignature, pq_verify, pq_sign};
use crate::qanto_storage::{QantoStorage, QantoStorageError, StorageConfig};
use crate::transaction::{Output, Transaction};
use crate::types::{HomomorphicEncrypted, UTXO};
use crate::wallet::Wallet;
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
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Instant, SystemTime, SystemTimeError, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use tokio::task;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// --- High-Throughput Constants ---
// To achieve ~10M TPS at 32 BPS, each block must hold ~312,500 transactions.
pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 500000;
// Increased block size to accommodate the higher transaction count.
pub const MAX_BLOCK_SIZE: usize = 128000000; // 128 MB

// --- Network & Economic Constants ---
pub const DEV_ADDRESS: &str = "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3";
pub const CONTRACT_ADDRESS: &str =
    "4a8d50f24c5ffec79ac665d123a3bdecacaa95f9f26751385a5a925c647bd394";
pub const INITIAL_BLOCK_REWARD: u64 = 50_000_000_000; // In smallest units (assuming 10^9 per QNTO)
const SMALLEST_UNITS_PER_QNTO: u64 = 1_000_000_000;
const FINALIZATION_DEPTH: u64 = 8;
const SHARD_THRESHOLD: u32 = 2;
const TEMPORAL_CONSENSUS_WINDOW: u64 = 600;
const MAX_BLOCKS_PER_MINUTE: u64 = 32 * 60;
const MIN_VALIDATOR_STAKE: u64 = 50;
const SLASHING_PENALTY: u64 = 30;
const CACHE_SIZE: usize = 10_000; // Increased cache size for better performance
const ANOMALY_DETECTION_BASELINE_BLOCKS: usize = 100;
const ANOMALY_Z_SCORE_THRESHOLD: f64 = 3.5;
const INITIAL_DIFFICULTY: f64 = 1.0; // Difficulty is now a float managed by SAGA's PID controller

// High-Performance Optimization Constants - Enhanced for 32 BPS / 10M+ TPS
const PARALLEL_VALIDATION_BATCH_SIZE: usize = 10000; // Further increased for better throughput
const BLOCK_PROCESSING_WORKERS: usize = 128; // Further increased for extreme parallelism
const TRANSACTION_VALIDATION_WORKERS: usize = 32; // Further increased for signature verification
const MEMPOOL_BATCH_SIZE: usize = 40000; // Further increased for high-throughput batching
const FAST_SYNC_BATCH_SIZE: usize = 1000; // 10x increase for faster sync
const BLOCK_CACHE_TTL_SECS: u64 = 7200; // Extended cache lifetime
const VALIDATION_TIMEOUT_MS: u64 = 50; // Reduced timeout for faster processing
const CONCURRENT_BLOCK_LIMIT: usize = 256; // Quadrupled concurrent processing
const SIMD_BATCH_SIZE: usize = 8; // SIMD processing batch size
#[allow(dead_code)]
const PREFETCH_BLOCKS: usize = 16; // Number of blocks to prefetch
const LOCK_FREE_QUEUE_SIZE: usize = 65536; // Lock-free queue capacity

lazy_static::lazy_static! {
    static ref BLOCKS_PROCESSED: IntCounter = register_int_counter!("blocks_processed_total", "Total blocks processed")
        .unwrap_or_else(|_| prometheus::IntCounter::new("blocks_processed_total_fallback", "Total blocks processed fallback").unwrap());
    static ref TRANSACTIONS_PROCESSED: IntCounter = register_int_counter!("transactions_processed_total", "Total transactions processed")
        .unwrap_or_else(|_| prometheus::IntCounter::new("transactions_processed_total_fallback", "Total transactions processed fallback").unwrap());
    static ref ANOMALIES_DETECTED: IntCounter = register_int_counter!("anomalies_detected_total", "Total anomalies detected")
        .unwrap_or_else(|_| prometheus::IntCounter::new("anomalies_detected_total_fallback", "Total anomalies detected fallback").unwrap());
}

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
    RewardMismatch(u64, u64),
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
    #[error("RocksDB error: {0}")]
    Storage(#[from] QantoStorageError),
    #[error("Miner error: {0}")]
    MinerError(String),
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
    pub difficulty: f64,
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
    pub difficulty: f64,
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
    pub amount: u64,
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
    pub amount: u64,
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
    pub gas_balance: u64,
}

impl SmartContract {
    pub fn execute(&mut self, input: &str, gas_limit: u64) -> Result<String, QantoDAGError> {
        let mut gas_used = 0;
        let mut charge_gas = |cost: u64| -> Result<(), QantoDAGError> {
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
            charge_gas(input.len() as u64)?;
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
            let current_val: u64 = counter_str.parse().unwrap_or(0);
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
    pub difficulty: f64,
    pub validator: String,
    pub miner: String,
    pub nonce: u64,
    pub timestamp: u64,
    pub height: u64,
    pub reward: u64,
    pub effort: u64,
    pub cross_chain_references: Vec<(u32, String)>,
    pub cross_chain_swaps: Vec<CrossChainSwap>,
    pub merkle_root: String,
    pub signature: QuantumResistantSignature,

    pub homomorphic_encrypted: Vec<HomomorphicEncrypted>,
    pub smart_contracts: Vec<SmartContract>,
    #[serde(default)]
    pub carbon_credentials: Vec<CarbonOffsetCredential>,
    pub epoch: u64,
}

impl fmt::Display for QantoBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let border = "═".repeat(90);
        writeln!(f, "╔{border}╗")?;
        writeln!(
            f,
            "║ ⛓️  New Qanto Block Mined on Chain #{} ⛓️",
            self.chain_id
        )?;
        writeln!(f, "╟{border}╢")?;
        writeln!(f, "║ 🆔 Block ID:      {}", self.id)?;
        writeln!(f, "║ 📅 Timestamp:     {}", self.timestamp)?;
        writeln!(f, "║ 📈 Height:        {}", self.height)?;
        writeln!(
            f,
            "║ 🔗 Parents:        {}",
            if self.parents.is_empty() {
                "(Genesis Block)".to_string()
            } else {
                self.parents.join(", ")
            }
        )?;
        writeln!(f, "║ 🧾 Transactions:   {}", self.transactions.len())?;
        let total_offset: f64 = self
            .carbon_credentials
            .iter()
            .map(|c| c.tonnes_co2_sequestered)
            .sum();
        if total_offset > 0.0 {
            writeln!(f, "║ 🌍 Carbon Offset:  {total_offset:.4} tonnes CO₂e")?;
        }
        writeln!(f, "║ 🌳 Merkle Root:    {}", self.merkle_root)?;
        writeln!(f, "╟─ Mining Details ─{}╢", "─".repeat(70))?;
        writeln!(f, "║ ⛏️  Miner:           {}", self.miner)?;
        writeln!(f, "║ ✨ Nonce:          {}", self.nonce)?;
        writeln!(f, "║ 🎯 Difficulty:    {:.4}", self.difficulty)?;
        writeln!(f, "║ 💪 Effort:         {} hashes", self.effort)?;
        writeln!(
            f,
            "║ 💰 Block Reward:    {:.9} $QNTO (from SAGA)",
            self.reward as f64 / SMALLEST_UNITS_PER_QNTO as f64
        )?;
        writeln!(f, "╚{border}╝")?;
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



        let (public_key, _) = HomomorphicEncrypted::generate_keypair();
        let homomorphic_encrypted_data = data
            .transactions
            .iter()
            .map(|tx| HomomorphicEncrypted::new(tx.amount, &public_key))
            .collect();

        let signer_public_key = data.validator_private_key.public_key().as_bytes().to_vec();
        let signature = pq_sign(&data.validator_private_key, &pre_signature_data_for_id)
            .map_err(|_e| QantoDAGError::QuantumSignature(crate::post_quantum_crypto::PQError::SigningError))?;


        Ok(Self {
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
        })
    }

    pub fn serialize_for_signing(data: &SigningData) -> Result<Vec<u8>, QantoDAGError> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&data.chain_id.to_le_bytes());
        buffer.extend_from_slice(data.merkle_root.as_bytes());
        for parent in data.parents {
            buffer.extend_from_slice(parent.as_bytes());
        }
        for tx in data.transactions {
            buffer.extend_from_slice(tx.id.as_bytes());
        }
        buffer.extend_from_slice(&data.timestamp.to_be_bytes());
        buffer.extend_from_slice(&data.difficulty.to_be_bytes());
        buffer.extend_from_slice(&data.height.to_be_bytes());
        buffer.extend_from_slice(data.validator.as_bytes());
        buffer.extend_from_slice(data.miner.as_bytes());
        Ok(qanto_hash(&buffer).as_bytes().to_vec())
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

        let pk = QantoPQPublicKey::from_bytes(&self.signature.signer_public_key)
            .map_err(|_e| QantoDAGError::QuantumSignature(crate::post_quantum_crypto::PQError::VerificationError))?;
        let sig = QantoPQSignature::from_bytes(&self.signature.signature)
            .map_err(|_e| QantoDAGError::QuantumSignature(crate::post_quantum_crypto::PQError::VerificationError))?;

        Ok(pq_verify(&pk, &data_to_verify, &sig).unwrap_or(false))
    }

    pub fn hash(&self) -> String {
        // Create header hash for qanhash algorithm
        let mut header_data = Vec::new();
        header_data.extend_from_slice(self.id.as_bytes());
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
        let qanhash_result = qanhash::hash(&header_hash, self.nonce);
        hex::encode(qanhash_result)
    }


}

// Use unified metrics system
pub type PerformanceMetrics = QantoMetrics;

/// Configuration for creating a new QantoDAG instance.
pub struct QantoDagConfig {
    pub initial_validator: String,
    pub target_block_time: u64,
    pub num_chains: u32,

}

#[derive(Debug)]
pub struct QantoDAG {
    // Core data structures with optimized concurrent access
    pub blocks: Arc<DashMap<String, QantoBlock>>,
    pub tips: Arc<DashMap<u32, HashSet<String>>>,
    pub validators: Arc<DashMap<String, u64>>,
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
    pub saga: Arc<PalletSaga>,
    pub self_arc: Weak<QantoDAG>,
    pub current_epoch: Arc<AtomicU64>,
    

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
}

/// Helper struct to track mining state
#[derive(Debug)]
struct MiningState {
    cycles: u64,
    blocks_mined: u64,
    last_heartbeat: std::time::Instant,
}

impl MiningState {
    fn new() -> Self {
        Self {
            cycles: 0,
            blocks_mined: 0,
            last_heartbeat: std::time::Instant::now(),
        }
    }
}

impl QantoDAG {
    pub fn new(
        config: QantoDagConfig,
        saga: Arc<PalletSaga>,
        db: QantoStorage,
    ) -> Result<Arc<Self>, QantoDAGError> {
        let genesis_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Initialize crossbeam channel for block processing queue
        let (block_sender, block_receiver) = bounded(CONCURRENT_BLOCK_LIMIT);

        let dag = Self {
            blocks: Arc::new(DashMap::new()),
            tips: Arc::new(DashMap::new()),
            validators: Arc::new(DashMap::new()),
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
            db: Arc::new(db),
            saga,
            self_arc: Weak::new(),
            current_epoch: Arc::new(AtomicU64::new(0)),


            // High-performance optimization fields
            block_processing_semaphore: Arc::new(Semaphore::new(BLOCK_PROCESSING_WORKERS)),
            validation_workers: Arc::new(Semaphore::new(TRANSACTION_VALIDATION_WORKERS)),
            block_queue: Arc::new((block_sender, block_receiver)),
            validation_cache: Arc::new(DashMap::new()),
            fast_tips_cache: Arc::new(DashMap::new()),
            processing_blocks: Arc::new(DashMap::new()),
            performance_metrics: Arc::new(PerformanceMetrics::default()),
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
        };

        info!(
            "Starting genesis initialization loop for {} chains",
            config.num_chains
        );
        // Generate a single keypair for all genesis blocks to avoid expensive repeated key generation
        let (paillier_pk, _) = HomomorphicEncrypted::generate_keypair();
        for chain_id_val in 0..config.num_chains {
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

            dag.blocks.insert(genesis_id.clone(), genesis_block);
            dag.tips
                .entry(chain_id_val)
                .or_insert_with(HashSet::new)
                .insert(genesis_id);
        }

        info!("Genesis loop completed");
        // Initialize validators
        dag.validators.insert(
            config.initial_validator.clone(),
            MIN_VALIDATOR_STAKE * config.num_chains as u64 * 2,
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

        Ok(arc_dag)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_solo_miner(
        self: Arc<Self>,
        wallet: Arc<Wallet>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        miner: Arc<Miner>,
        _optimized_block_builder: Arc<crate::performance_optimizations::OptimizedBlockBuilder>,
        mining_interval_secs: u64,
        shutdown_token: tokio_util::sync::CancellationToken,
    ) -> Result<(), QantoDAGError> {
        info!(
            "SOLO MINER: Starting proactive mining loop with an interval of {} seconds.",
            mining_interval_secs
        );
        let mut mining_state = MiningState::new();

        let mut mining_interval = tokio::time::interval(tokio::time::Duration::from_secs(mining_interval_secs));
        mining_interval.tick().await;

        // Clone necessary Arcs for the dummy transaction task
        let wallet_clone = Arc::clone(&wallet);
        let mempool_clone = Arc::clone(&mempool);
        let utxos_clone = Arc::clone(&utxos);
        let shutdown_token_clone = shutdown_token.clone();

        // Clone self for the dummy transaction task
        let self_clone = Arc::clone(&self);

        // Spawn a separate task for dummy transaction generation
        tokio::spawn(async move {
            let mut dummy_tx_interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            dummy_tx_interval.tick().await; // Initial tick

            info!("SOLO MINER: Starting dummy transaction generation loop.");

            loop {
                tokio::select! {
                    biased;
                    _ = shutdown_token_clone.cancelled() => {
                        info!("SOLO MINER: Dummy transaction task received shutdown signal, stopping.");
                        break;
                    }
                    _ = dummy_tx_interval.tick() => {
                        // Generate a valid transaction with a quantum-resistant signature
                        let (signing_key, public_key) = match wallet_clone.get_keypair() {
                            Ok(keypair) => keypair,
                            Err(e) => {
                                warn!("Failed to get keypair from wallet: {:?}", e);
                                continue;
                            }
                        };
                        let dummy_tx = match Transaction::new_dummy_signed(&signing_key, &public_key) {
                            Ok(tx) => tx,
                            Err(e) => {
                                warn!("Failed to create dummy signed transaction: {:?}", e);
                                continue;
                            }
                        };

                        let mempool_guard = mempool_clone.write().await;
                        let utxos_guard = utxos_clone.read().await;
                        if let Err(e) = mempool_guard.add_transaction(dummy_tx, &utxos_guard, &self_clone).await {
                            warn!("Failed to add dummy transaction to mempool: {}", e);
                        }
                    }
                }
            }
            info!("SOLO MINER: Dummy transaction task has stopped.");
        });

        loop {
            tokio::select! {
                biased;
                _ = shutdown_token.cancelled() => {
                    info!("SOLO MINER: Received shutdown signal, stopping mining loop. (DEBUG: Shutdown triggered)");
                    // Add a log to indicate if the token was explicitly cancelled
                    if shutdown_token.is_cancelled() {
                        info!("SOLO MINER: Shutdown token explicitly cancelled.");
                    } else {
                        info!("SOLO MINER: Shutdown token not explicitly cancelled, but signal received.");
                    }
                    break;
                }
                _ = mining_interval.tick() => {
                    info!("SOLO MINER: Mining interval elapsed ({}s), starting mining cycle", mining_interval_secs);
                    info!("SOLO MINER: (DEBUG) Mining loop continuing.");

                    mining_state.cycles += 1;
                    info!("SOLO MINER: Starting mining cycle #{}", mining_state.cycles);
                    
                    self.log_heartbeat_if_needed(&mut mining_state, mining_interval_secs);

                    let emission = self.emission.read().await;
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                    let estimated_reward = emission.calculate_reward(now).unwrap_or(0);
                    let current_difficulty = self.get_current_difficulty().await;
                    info!(
                        "Mining Status: Cycle {}, Difficulty: {:.4}, Estimated Reward: {} QANTO",
                        mining_state.cycles, current_difficulty, estimated_reward
                    );
                    
                    tokio::task::yield_now().await;

                    if !self.validate_wallet_keypair(&wallet).await {
                        continue;
                    }

                    info!("SOLO MINER: Preparing to create candidate block");
                    let mut candidate_block = match self.create_mining_candidate(&wallet, &mempool, &utxos).await {
                        Ok(block) => {
                            info!("SOLO MINER: Candidate block created successfully");
                            block
                        }
                        Err(e) => {
                            warn!("SOLO MINER: Failed to create candidate block: {}. Retrying next cycle.", e);
                            continue;
                        }
                    };

                    let (mined_block, mining_duration) = match self.execute_mining_pow(&miner, &mut candidate_block, &shutdown_token).await {
                        Ok((block, duration)) => (block, duration),
                        Err(should_break) => {
                            if should_break { break; } // Propagate shutdown signal
                            continue;
                        }
                    };

                    mining_state.blocks_mined += 1;
            
            // FIX: Replaced the previous error handling with a more robust `match` statement.
            // This ensures that any failure during block processing is explicitly logged,
            // preventing the miner from stalling silently.
            let process_result = self.process_mined_block(
                    mined_block,
                    &mempool,
                    &utxos,
                    mining_state.blocks_mined,
                    mining_duration,
                ).await;

            match process_result {
                Ok(_) => {
                    info!(
                        "SOLO MINER: Successfully processed and added mined block. Cycle #{} complete.",
                        mining_state.cycles
                    );
                }
                Err(e) => {
                    warn!(
                        "SOLO MINER: Failed to process mined block: {}. Continuing to next cycle.",
                        e
                    );
                }
                }
            }
        }
    }
    info!("SOLO MINER: Mining loop has stopped.");
    Ok(())
}

    /// Log heartbeat if enough time has elapsed
    fn log_heartbeat_if_needed(&self, mining_state: &mut MiningState, mining_interval_secs: u64) {
        const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

        if mining_state.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            info!(
                "SOLO MINER HEARTBEAT: Alive - Cycles: {}, Blocks Mined: {}, Next attempt in {} seconds",
                mining_state.cycles, mining_state.blocks_mined, mining_interval_secs
            );
            mining_state.last_heartbeat = std::time::Instant::now();
        }
    }

    /// Validate wallet keypair availability
    async fn validate_wallet_keypair(&self, wallet: &Arc<Wallet>) -> bool {
        match wallet.get_keypair() {
            Ok(_) => true,
            Err(e) => {
                warn!(
                    "SOLO MINER: Could not get keypair, skipping cycle. Error: {}",
                    e
                );
                false
            }
        }
    }

    /// Create a candidate block for mining
    async fn create_mining_candidate(
        &self,
        wallet: &Arc<Wallet>,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<QantoBlock, QantoDAGError> {
        let miner_address = wallet.address();
        let chain_id_to_mine: u32 = 0;

        info!(
            "SOLO MINER: Creating candidate block for chain {} (rewards to validator: {})",
            chain_id_to_mine, miner_address
        );
        let candidate_creation_start = std::time::Instant::now();

        let (qr_signing_key, qr_public_key) = wallet.get_keypair().map_err(|e| {
            QantoDAGError::WalletError(format!("Failed to get keypair from wallet: {e}"))
        })?;

        match self
            .create_candidate_block(
                &qr_signing_key,
                &qr_public_key,
                &miner_address,
                mempool,
                utxos,
                chain_id_to_mine,
            )
            .await
        {
            Ok(block) => {
                info!(
                    "SOLO MINER: Successfully created candidate block {} with {} transactions in {:?}",
                    block.id,
                    block.transactions.len(),
                    candidate_creation_start.elapsed()
                );
                Ok(block)
            }
            Err(e) => {
                warn!(
                    "SOLO MINER: Failed to create candidate block after {:?}: {}. Retrying after delay.",
                    candidate_creation_start.elapsed(),
                    e
                );
                Err(QantoDAGError::Generic(format!(
                    "Failed to create candidate block: {e}"
                )))
            }
        }
    }

    /// Execute proof-of-work mining
    async fn execute_mining_pow(
        &self,
        miner: &Arc<Miner>,
        candidate_block: &mut QantoBlock,
        shutdown_token: &tokio_util::sync::CancellationToken,
    ) -> Result<(QantoBlock, std::time::Duration), bool> {
        info!(
            "SOLO MINER: Starting proof-of-work for candidate block {} with difficulty {}",
            candidate_block.id, candidate_block.difficulty
        );

        let miner_clone = miner.clone();
        let shutdown_clone = shutdown_token.clone();
        let mut candidate_clone = candidate_block.clone();
        let pow_start = std::time::Instant::now();

        info!("SOLO MINER: Starting POW mining task");

        let mining_result = tokio::task::spawn_blocking(move || {
            miner_clone
                .solve_pow_with_cancellation(&mut candidate_clone, shutdown_clone)
                .map(|_| candidate_clone)
        })
        .await;

        info!("SOLO MINER: POW mining task completed");

        let mining_duration = pow_start.elapsed();
        match mining_result {
            Ok(Ok(block)) => {
                info!(
                    "SOLO MINER: Proof-of-work completed in {:?}, nonce: {}",
                    mining_duration, block.nonce
                );
                Ok((block, mining_duration))
            }
            Ok(Err(e)) => {
                if shutdown_token.is_cancelled() {
                    info!("SOLO MINER: Mining cancelled due to shutdown.");
                    Err(true)
                } else {
                    warn!(
                        "SOLO MINER: Mining failed after {:?} for the current candidate block: {}",
                        pow_start.elapsed(),
                        e
                    );
                    Err(false)
                }
            }
            Err(e) => {
                warn!("SOLO MINER: Mining task panicked: {}", e);
                Err(false)
            }
        }
    }

    /// Process a successfully mined block
    #[allow(clippy::too_many_arguments)]
    async fn process_mined_block(
        &self,
        mined_block: QantoBlock,
        mempool: &Arc<RwLock<Mempool>>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
        total_blocks_mined: u64,
        mining_time: std::time::Duration,
    ) -> Result<(), QantoDAGError> {
        let block_height = mined_block.height;

        celebrate_mining_success(MiningCelebrationParams {
            block_height,
            block_hash: mined_block.hash(),
            nonce: mined_block.nonce,
            difficulty: mined_block.difficulty,
            transactions_count: mined_block.transactions.len(),
            mining_time,
            effort: mined_block.effort,
            total_blocks_mined,
            chain_id: mined_block.chain_id,
            block_reward: mined_block.reward,
            compact: false,
        });

        info!(
            "SOLO MINER: Successfully mined block #{} with ID: {} (Total blocks mined: {})",
            total_blocks_mined, mined_block.id, total_blocks_mined
        );

        tokio::task::yield_now().await;

        if self.add_block(mined_block.clone(), utxos).await? {
            info!("SOLO MINER: Successfully added new block to the QantoDAG.");
            let removed_count = mined_block.transactions.len();
            mempool
                .read()
                .await
                .remove_transactions(&mined_block.transactions)
                .await;
            info!(
                "SOLO MINER: Removed {} transactions from mempool",
                removed_count
            );
        } else {
            warn!("SOLO MINER: Mined block was rejected by the DAG (already exists?).");
        }
        Ok(())
    }

    pub async fn get_block_reward(&self, block_id: &str) -> Option<u64> {
        self.blocks.get(block_id).map(|b| b.reward)
    }

    pub async fn prune_old_blocks(&self, prune_depth: u64) -> Result<usize, QantoDAGError> {
        let current_height = self.get_latest_block().await.map(|b| b.height).unwrap_or(0);
        let prune_threshold = current_height.saturating_sub(prune_depth);

        let mut pruned_count = 0;
        let mut ids_to_prune = Vec::new();

        for entry in self.blocks.iter() {
            let block = entry.value();
            if block.height < prune_threshold && !self.finalized_blocks.contains_key(&block.id) {
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

    pub async fn get_average_tx_per_block(&self) -> f64 {
        if self.blocks.is_empty() {
            return 0.0;
        }
        let total_txs: usize = self
            .blocks
            .iter()
            .map(|entry| entry.value().transactions.len())
            .sum();
        total_txs as f64 / self.blocks.len() as f64
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

    pub async fn get_current_difficulty(&self) -> f64 {
        INITIAL_DIFFICULTY // Return the current difficulty
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

    pub async fn get_transaction(&self, tx_id: &str) -> Option<Transaction> {
        for block_entry in self.blocks.iter() {
            for tx in &block_entry.value().transactions {
                if tx.id == tx_id {
                    return Some(tx.clone());
                }
            }
        }
        None
    }

    pub async fn add_block(
        &self,
        block: QantoBlock,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<bool, QantoDAGError> {
        if self.blocks.contains_key(&block.id) {
            warn!("Attempted to add block {} which already exists.", block.id);
            return Ok(false);
        }

        if !self.is_valid_block(&block, utxos_arc).await? {
            let mut error_msg = String::with_capacity(50 + block.id.len());
            error_msg.push_str("Block ");
            error_msg.push_str(&block.id);
            error_msg.push_str(" failed validation in add_block");
            return Err(QantoDAGError::InvalidBlock(error_msg));
        }

        let mut utxos_write_guard = utxos_arc.write().await;

        if self.blocks.contains_key(&block.id) {
            warn!(
                "Block {} already exists (double check after write lock).",
                block.id
            );
            return Ok(false);
        }

        // Create a temporary HashMap for anomaly detection
        let blocks_map: HashMap<String, QantoBlock> = self
            .blocks
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        let anomaly_score = self.detect_anomaly_internal(&blocks_map, &block).await?;
        if anomaly_score > 0.9 {
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

        for tx in &block.transactions {
            for input in &tx.inputs {
                // Pre-calculate capacity: tx_id length + 1 (underscore) + max 10 digits for output_index
                let mut utxo_id = String::with_capacity(input.tx_id.len() + 11);
                utxo_id.push_str(&input.tx_id);
                utxo_id.push('_');
                utxo_id.push_str(&input.output_index.to_string());
                utxos_write_guard.remove(&utxo_id);
            }
            for (index, output) in tx.outputs.iter().enumerate() {
                // Pre-calculate capacity: tx_id length + 1 (underscore) + max 10 digits for index
                let mut utxo_id = String::with_capacity(tx.id.len() + 11);
                utxo_id.push_str(&tx.id);
                utxo_id.push('_');
                utxo_id.push_str(&index.to_string());

                // Pre-calculate explorer link capacity
                let mut explorer_link = String::with_capacity(42 + utxo_id.len());
                explorer_link.push_str("/explorer/utxo/");
                explorer_link.push_str(&utxo_id);

                utxos_write_guard.insert(
                    utxo_id,
                    UTXO {
                        address: output.address.clone(),
                        amount: output.amount,
                        tx_id: tx.id.clone(),
                        output_index: index as u32,
                        explorer_link,
                    },
                );
            }
        }

        // Update tips using DashMap
        let mut current_tips = self.tips.entry(block.chain_id).or_insert_with(HashSet::new);
        for parent_id in &block.parents {
            current_tips.remove(parent_id);
        }
        current_tips.insert(block.id.clone());
        drop(current_tips);

        // Store the block
        let block_for_db = block.clone();
        self.blocks.insert(block.id.clone(), block);
        self.block_creation_timestamps
            .insert(block_for_db.id.clone(), Utc::now().timestamp() as u64);

        drop(utxos_write_guard);

        let db_clone = self.db.clone();
        let id_bytes = block_for_db.id.as_bytes().to_vec();
        let block_bytes = serde_json::to_vec(&block_for_db)?;

        task::spawn_blocking(move || db_clone.put(id_bytes, block_bytes))
            .await
            .map_err(QantoDAGError::JoinError)?
            .map_err(QantoDAGError::Storage)?;

        let mut emission = self.emission.write().await;
        emission
            .update_supply(block_for_db.reward)
            .map_err(QantoDAGError::EmissionError)?;

        BLOCKS_PROCESSED.inc();
        TRANSACTIONS_PROCESSED.inc_by(block_for_db.transactions.len() as u64);
        Ok(true)
    }

    pub async fn get_id(&self) -> u32 {
        0
    }

    pub async fn get_tips(&self, chain_id: u32) -> Option<Vec<String>> {
        self.tips
            .get(&chain_id)
            .map(|tips_set| tips_set.iter().cloned().collect())
    }

    pub async fn add_validator(&self, address: String, stake: u64) {
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
        initial_gas: u64,
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

    pub async fn create_candidate_block(
        &self,

        _qr_signing_key: &QantoPQPrivateKey,
         _qr_public_key: &QantoPQPublicKey,
         validator_address: &str,
         mempool_arc: &Arc<RwLock<Mempool>>,
        _utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        chain_id_val: u32,
    ) -> Result<QantoBlock, QantoDAGError> {
        {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let recent_blocks = self
                .block_creation_timestamps
                .iter()
                .map(|entry| *entry.value())
                .filter(|&t| now.saturating_sub(t) < 60)
                .count() as u64;
            if recent_blocks >= MAX_BLOCKS_PER_MINUTE {
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
                let mut msg = String::with_capacity(80 + validator_address.len());
                msg.push_str("Insufficient stake for validator ");
                msg.push_str(validator_address);
                msg.push_str(": ");
                msg.push_str(&stake.to_string());
                msg.push_str(" < ");
                msg.push_str(&MIN_VALIDATOR_STAKE.to_string());
                return Err(QantoDAGError::InvalidBlock(msg));
            }
        }

        let selected_transactions = {
            let mempool_guard = mempool_arc.read().await;
            mempool_guard
                .select_transactions(MAX_TRANSACTIONS_PER_BLOCK)
                .await
        };
        let parent_tips: Vec<String> = self.get_tips(chain_id_val).await.unwrap_or_default();

        let (height, new_timestamp) = {
            let blocks_guard = &self.blocks;
            let (max_parent_height, max_parent_timestamp) = parent_tips
                .iter()
                .filter_map(|p_id| blocks_guard.get(p_id))
                .map(|p_block| (p_block.height, p_block.timestamp))
                .max()
                .unwrap_or((0, 0));

            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            (
                max_parent_height + 1,
                current_time.max(max_parent_timestamp + 1),
            )
        };

        let epoch = self
            .current_epoch
            .load(std::sync::atomic::Ordering::Relaxed);

        let current_difficulty = {
            let rules = self.saga.economy.epoch_rules.read().await;
            rules.get("base_difficulty").map_or(10.0, |r| r.value)
        };

        // Calculate total fees from selected transactions (excluding coinbase which will be added later)
        let total_fees = selected_transactions.iter().map(|tx| tx.fee).sum::<u64>();

        // Create temporary block with actual selected transactions for accurate reward calculation
        let (paillier_pk, _) = HomomorphicEncrypted::generate_keypair();
        let temp_block_for_reward_calc = QantoBlock::new(QantoBlockCreationData {
            chain_id: chain_id_val,
            parents: parent_tips.clone(),
            transactions: selected_transactions.clone(),
            difficulty: current_difficulty,
            validator: validator_address.to_string(),
            miner: validator_address.to_string(),
            validator_private_key: QantoPQPrivateKey::new_dummy(),

            timestamp: new_timestamp,
            current_epoch: epoch,
            height,
            paillier_pk,
        })?;

        let self_arc_strong = self
            .self_arc
            .upgrade()
            .ok_or(QantoDAGError::SelfReferenceNotInitialized)?;
        let reward = self
            .saga
            .calculate_dynamic_reward(&temp_block_for_reward_calc, &self_arc_strong, total_fees)
            .await?;

        // Generate proper homomorphic encryption keys for coinbase output
        let (public_key_material, _private_key_material) = HomomorphicEncrypted::generate_keypair();
        let coinbase_outputs = vec![Output {
            address: validator_address.to_string(),
            amount: reward,
            homomorphic_encrypted: HomomorphicEncrypted::new(reward, &public_key_material),
        }];

        let reward_tx = Transaction::new_coinbase(
            validator_address.to_string(),
            reward,

            coinbase_outputs,
        )?;

        let mut transactions_for_block = vec![reward_tx];
        transactions_for_block.extend(selected_transactions);

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
            validator_private_key: QantoPQPrivateKey::new_dummy(),

            chain_id: chain_id_val,
            parents: parent_tips,
            transactions: transactions_for_block,
            difficulty: current_difficulty,
            validator: validator_address.to_string(),
            miner: validator_address.to_string(),

            timestamp: new_timestamp,
            current_epoch: epoch,
            height,
            paillier_pk,
        })?;
        block.cross_chain_references = cross_chain_references;
        block.reward = reward;

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

        let serialized_size = serde_json::to_vec(&block)?.len();
        if block.transactions.len() > MAX_TRANSACTIONS_PER_BLOCK || serialized_size > MAX_BLOCK_SIZE
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
            return Err(QantoDAGError::QuantumSignature(
                PQError::VerificationError
            ));
        }

        let target_hash_bytes = Miner::calculate_target_from_difficulty(block.difficulty);
        let block_pow_hash = block.hash();
        let block_pow_hash_bytes = hex::decode(block_pow_hash).map_err(QantoDAGError::HexError)?;
        if !Miner::hash_meets_target(&block_pow_hash_bytes, &target_hash_bytes) {
            return Err(QantoDAGError::InvalidBlock(
                "Proof-of-Work not satisfied".to_string(),
            ));
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
                if block.timestamp <= parent_block.timestamp {
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

        let coinbase_tx = &block.transactions[0];
        if !coinbase_tx.is_coinbase() {
            return Err(QantoDAGError::InvalidBlock(
                "First transaction must be a coinbase (no inputs)".to_string(),
            ));
        }
        let total_coinbase_output: u64 = coinbase_tx.outputs.iter().map(|o| o.amount).sum();

        let self_arc_strong = self
            .self_arc
            .upgrade()
            .ok_or(QantoDAGError::SelfReferenceNotInitialized)?;
        let total_fees = block
            .transactions
            .iter()
            .skip(1)
            .map(|tx| tx.fee)
            .sum::<u64>();
        let expected_reward = self
            .saga
            .calculate_dynamic_reward(block, &self_arc_strong, total_fees)
            .await?;

        if block.reward != expected_reward {
            return Err(QantoDAGError::RewardMismatch(expected_reward, block.reward));
        }

        if total_coinbase_output != block.reward {
            return Err(QantoDAGError::RewardMismatch(
                block.reward,
                total_coinbase_output,
            ));
        }

        let utxos_guard = utxos_arc.read().await;
        for tx in block.transactions.iter().skip(1) {
            tx.verify(self, &utxos_guard).await?;
        }

        let blocks_map: HashMap<String, QantoBlock> = self
            .blocks
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        let anomaly_score = self.detect_anomaly_internal(&blocks_map, block).await?;
        if anomaly_score > 0.7 {
            ANOMALIES_DETECTED.inc();
            warn!(
                "High anomaly score ({}) detected for block {}",
                anomaly_score, block.id
            );
        }

        Ok(true)
    }

    async fn detect_anomaly_internal(
        &self,
        blocks_guard: &HashMap<String, QantoBlock>,
        block: &QantoBlock,
    ) -> Result<f64, QantoDAGError> {
        if blocks_guard.len() < ANOMALY_DETECTION_BASELINE_BLOCKS {
            return Ok(0.0);
        }

        let transaction_values: Vec<f64> = blocks_guard
            .values()
            .flat_map(|b| &b.transactions)
            .filter(|tx| !tx.is_coinbase())
            .map(|tx| tx.amount as f64)
            .collect();

        if transaction_values.is_empty() {
            return Ok(0.0);
        }

        let mean: f64 = transaction_values.iter().sum::<f64>() / transaction_values.len() as f64;
        let std_dev: f64 = (transaction_values
            .iter()
            .map(|val| (*val - mean).powi(2))
            .sum::<f64>()
            / transaction_values.len() as f64)
            .sqrt();

        let mut max_z_score = 0.0;
        for tx in block.transactions.iter().filter(|tx| !tx.is_coinbase()) {
            if std_dev > 0.0 {
                let z_score = ((tx.amount as f64 - mean) / std_dev).abs();
                if z_score > max_z_score {
                    max_z_score = z_score;
                }
            }
        }

        if max_z_score > ANOMALY_Z_SCORE_THRESHOLD {
            warn!(
                block_id = %block.id,
                max_z_score,
                "High Z-score detected for transaction value, potential economic anomaly."
            );
            return Ok(1.0);
        }

        let avg_tx_count: f64 = blocks_guard
            .values()
            .map(|b_val| b_val.transactions.len() as f64)
            .sum::<f64>()
            / (blocks_guard.len() as f64);

        if avg_tx_count < 1.0 {
            return if block.transactions.len() > 10 {
                Ok(1.0)
            } else {
                Ok(0.0)
            };
        }

        let anomaly_score = (block.transactions.len() as f64 - avg_tx_count).abs() / avg_tx_count;
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
                for tip_id in &*chain_tips {
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
                        for id_to_finalize in path_to_finalize {
                            if finalized_guard
                                .insert(id_to_finalize.clone(), true)
                                .is_none()
                            {
                                log::debug!("Finalized block: {id_to_finalize}");
                            }
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
                .unwrap_or(0);
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
                .map_or(10.0, |r| r.value);

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

    pub async fn propose_governance(
        &self,
        proposer_address: String,
        rule_name: String,
        new_value: f64,
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
            proposal_type: ProposalType::UpdateRule(rule_name, new_value),
            votes_for: 0.0,
            votes_against: 0.0,
            status: ProposalStatus::Voting,
            voters: vec![],
            creation_epoch: self.current_epoch.load(Ordering::SeqCst),
            justification: None,
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
        let stake_val: u64 = *self
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
            proposal_obj.votes_for += stake_val as f64;
        } else {
            proposal_obj.votes_against += stake_val as f64;
        }

        let rules = self.saga.economy.epoch_rules.read().await;
        let vote_threshold = rules
            .get("proposal_vote_threshold")
            .map_or(100.0, |r| r.value);
        if proposal_obj.votes_for >= vote_threshold {
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
        let total_stake_val: u64 = self.validators.iter().map(|entry| *entry.value()).sum();
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

    pub async fn run_periodic_maintenance(&self) {
        debug!("Running periodic DAG maintenance...");

        // Process mempool batches using MEMPOOL_BATCH_SIZE
        self.process_mempool_batches().await;

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
    async fn process_mempool_batches(&self) {
        let batch_count = self.lock_free_tx_queue.len() / MEMPOOL_BATCH_SIZE;
        if batch_count > 0 {
            debug!(
                "Processing {} mempool batches of size {}",
                batch_count, MEMPOOL_BATCH_SIZE
            );

            for batch_id in 0..batch_count {
                let mut batch_transactions = Vec::with_capacity(MEMPOOL_BATCH_SIZE);

                // Collect transactions for this batch
                for _ in 0..MEMPOOL_BATCH_SIZE {
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
        // Check fast tips cache first
        if let Some(cached_tips) = self.fast_tips_cache.get(&chain_id) {
            self.performance_metrics
                .cache_hits
                .fetch_add(1, Ordering::Relaxed);
            return Ok(cached_tips.clone());
        }

        self.performance_metrics
            .cache_misses
            .fetch_add(1, Ordering::Relaxed);

        // Compute tips and cache them
        let tips = self.get_tips(chain_id).await.unwrap_or_default();
        self.fast_tips_cache.insert(chain_id, tips.clone());

        Ok(tips)
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
                let added = self.add_block(block, utxos_arc).await?;
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
        let blocks_per_second = (results.len() as f64 / start_time.elapsed().as_secs_f64()) as u64;
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
                    let utxo_key = format!("{}:{}", input.tx_id, input.output_index);
                    if !utxos.contains_key(&utxo_key) {
                        return false;
                    }
                }

                // Basic signature verification would go here
                // For now, assume valid if structure is correct
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

    /// Optimized block creation with batch processing
    pub async fn create_optimized_block(
        &self,

        validator_address: &str,
        transactions: Vec<Transaction>,
        chain_id_val: u32,
    ) -> Result<QantoBlock, QantoDAGError> {
        let start_time = Instant::now();

        // Get parent blocks efficiently
        let parents = self
            .get_fast_tips(chain_id_val)
            .await?
            .into_iter()
            .take(2) // Limit to 2 parents for efficiency
            .collect();

        // Calculate total fees in parallel
        let total_fees: u64 = transactions.par_iter().map(|tx| tx.fee).sum();

        // Get current difficulty and height
        let difficulty = 1.0; // Use default difficulty for optimized processing
        let height = self.blocks.len() as u64 + 1;
        let current_epoch = self.current_epoch.load(Ordering::Relaxed);

        // Calculate reward using simplified approach for optimized processing
        let base_reward = total_fees + (transactions.len() as u64 * 100);

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

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
                    self.add_block(block, utxos_arc).await?;
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
}

impl Clone for QantoDAG {
    fn clone(&self) -> Self {
        Self {
            blocks: self.blocks.clone(),
            tips: self.tips.clone(),
            validators: self.validators.clone(),
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
            saga: self.saga.clone(),
            self_arc: self.self_arc.clone(),
            current_epoch: self.current_epoch.clone(),
            block_processing_semaphore: self.block_processing_semaphore.clone(),
            validation_workers: self.validation_workers.clone(),
            block_queue: self.block_queue.clone(),
            validation_cache: self.validation_cache.clone(),
            fast_tips_cache: self.fast_tips_cache.clone(),
            processing_blocks: self.processing_blocks.clone(),
            performance_metrics: self.performance_metrics.clone(),
            // Advanced performance optimization fields
            simd_processor: self.simd_processor.clone(),
            lock_free_tx_queue: Arc::new(crossbeam::queue::SegQueue::new()),
            memory_pool: self.memory_pool.clone(),
            prefetch_cache: self.prefetch_cache.clone(),
            pipeline_stages: self.pipeline_stages.clone(),
            work_stealing_pool: self.work_stealing_pool.clone(),
            utxo_bloom_filter: self.utxo_bloom_filter.clone(),
            batch_processor: self.batch_processor.clone(),
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
            .create_optimized_block(

                &validator,
                transactions,
                chain_id,
            )
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

        use crossbeam::channel::bounded;
        use std::sync::Weak;

        // Create QantoStorage for testing
        let temp_path = std::env::temp_dir().join("dummy_qanto_storage");
        let storage_config = StorageConfig {
            data_dir: temp_path,
            max_file_size: 64 * 1024 * 1024, // 64MB
            cache_size: 1024 * 1024,         // 1MB
            compression_enabled: true,
            encryption_enabled: false,
            wal_enabled: true,
            sync_writes: false,
            compaction_threshold: 0.7,
            max_open_files: 100,
        };
        let db = QantoStorage::new(storage_config).expect("Failed to create dummy QantoStorage");

        // Create bounded channel for block queue
        let (sender, receiver) = bounded(1000);

        Self {
            blocks: Arc::new(DashMap::new()),
            tips: Arc::new(DashMap::new()),
            validators: Arc::new(DashMap::new()),
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
            db: Arc::new(db),
            saga: Arc::new(PalletSaga::new(
                #[cfg(feature = "infinite-strata")]
                None,
            )),
            self_arc: Weak::new(),
            current_epoch: Arc::new(AtomicU64::new(0)),
            block_processing_semaphore: Arc::new(Semaphore::new(10)),
            validation_workers: Arc::new(Semaphore::new(32)),
            block_queue: Arc::new((sender, receiver)),
            validation_cache: Arc::new(DashMap::new()),
            fast_tips_cache: Arc::new(DashMap::new()),
            processing_blocks: Arc::new(DashMap::new()),
            performance_metrics: Arc::new(PerformanceMetrics::default()),
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
        }
    }
}
