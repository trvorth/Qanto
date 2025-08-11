//! --- Qanto QantoDAG Ledger ---
//! v2.4.2 - Block Reward Calculation Fix
//! This version corrects the block reward calculation by ensuring that transaction
//! fees are properly calculated and included in the final reward amount.
//!
//! - FIX: The `create_candidate_block` function now calculates the total fees
//!   from mempool transactions *before* calling SAGA's reward calculation,
//!   ensuring the final reward includes both the base amount and fees.

use crate::emission::Emission;
use crate::mempool::Mempool;
use crate::miner::Miner;
use crate::saga::{
    CarbonOffsetCredential, GovernanceProposal, PalletSaga, ProposalStatus, ProposalType,
};
// This import is required for the `#[from]` attribute in QantoDAGError.
// The compiler may incorrectly flag it as unused, but it is necessary.
use crate::transaction::{Output, Transaction};
use crate::wallet::Wallet;
use hex;
use lru::LruCache;
use pqcrypto_mldsa::mldsa65 as dilithium5;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
use prometheus::{register_int_counter, IntCounter};
use rand::Rng;
use rayon::prelude::*;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Weak};
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::task;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// --- High-Throughput Constants ---
// To achieve ~10M TPS at 32 BPS, each block must hold ~312,500 transactions.
pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 320_000;
// Increased block size to accommodate the higher transaction count.
pub const MAX_BLOCK_SIZE: usize = 80_000_000; // 80 MB

// --- Network & Economic Constants ---
pub const DEV_ADDRESS: &str = "74fd2aae70ae8e0930b87a3dcb3b77f5b71d956659849f067360d3486604db41";
pub const INITIAL_BLOCK_REWARD: u64 = 50_000_000_000; // In smallest units (assuming 10^9 per QNTO)
const SMALLEST_UNITS_PER_QNTO: u64 = 1_000_000_000;
const FINALIZATION_DEPTH: u64 = 8;
const SHARD_THRESHOLD: u32 = 2;
const TEMPORAL_CONSENSUS_WINDOW: u64 = 600;
const MAX_BLOCKS_PER_MINUTE: u64 = 32 * 60;
const MIN_VALIDATOR_STAKE: u64 = 50;
const SLASHING_PENALTY: u64 = 30;
const CACHE_SIZE: usize = 1_000;
const ANOMALY_DETECTION_BASELINE_BLOCKS: usize = 100;
const ANOMALY_Z_SCORE_THRESHOLD: f64 = 3.5;
const INITIAL_DIFFICULTY: f64 = 1.0; // Difficulty is now a float managed by SAGA's PID controller

lazy_static::lazy_static! {
    static ref BLOCKS_PROCESSED: IntCounter = register_int_counter!("blocks_processed_total", "Total blocks processed").unwrap();
    static ref TRANSACTIONS_PROCESSED: IntCounter = register_int_counter!("transactions_processed_total", "Total transactions processed").unwrap();
    static ref ANOMALIES_DETECTED: IntCounter = register_int_counter!("anomalies_detected_total", "Total anomalies detected").unwrap();
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UTXO {
    pub address: String,
    pub amount: u64,
    pub tx_id: String,
    pub output_index: u32,
    pub explorer_link: String,
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
    QuantumSignature(String),
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
    RocksDB(#[from] rocksdb::Error),
    #[error("Miner error: {0}")]
    MinerError(String),
    #[error("Hex decoding error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Wallet error: {0}")]
    WalletError(String),
}

impl From<crate::wallet::WalletError> for QantoDAGError {
    fn from(e: crate::wallet::WalletError) -> Self {
        QantoDAGError::WalletError(e.to_string())
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

pub struct QantoBlockCreationData<'a> {
    pub chain_id: u32,
    pub parents: Vec<String>,
    pub transactions: Vec<Transaction>,
    pub difficulty: f64,
    pub validator: String,
    pub miner: String,
    pub qr_signing_key: &'a dilithium5::SecretKey,
    pub qr_public_key: &'a dilithium5::PublicKey,
    pub timestamp: u64,
    pub current_epoch: u64,
    pub height: u64,
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct QuantumResistantSignature {
    pub signer_public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

impl QuantumResistantSignature {
    pub fn sign(
        signing_key: &dilithium5::SecretKey,
        public_key: &dilithium5::PublicKey,
        message: &[u8],
    ) -> Result<Self, QantoDAGError> {
        let signed_message = dilithium5::sign(message, signing_key);
        Ok(Self {
            signer_public_key: public_key.as_bytes().to_vec(),
            signature: signed_message.as_bytes().to_vec(),
        })
    }

    pub fn verify(&self, message: &[u8]) -> bool {
        let Ok(pk) = dilithium5::PublicKey::from_bytes(&self.signer_public_key) else {
            return false;
        };
        let Ok(signed_message) = dilithium5::SignedMessage::from_bytes(&self.signature) else {
            return false;
        };
        match dilithium5::open(&signed_message, &pk) {
            Ok(recovered_message) => recovered_message == message,
            Err(_) => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct HomomorphicEncrypted {
    pub ciphertext: Vec<u8>,
}

impl HomomorphicEncrypted {
    #[instrument]
    pub fn new(amount: u64, _public_key_material: &[u8]) -> Self {
        Self {
            ciphertext: amount.to_be_bytes().to_vec(),
        }
    }

    pub fn decrypt(&self, _private_key_material: &[u8]) -> Result<u64, QantoDAGError> {
        if self.ciphertext.len() != 8 {
            return Err(QantoDAGError::HomomorphicError(
                "Invalid ciphertext length for decryption".to_string(),
            ));
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.ciphertext);
        Ok(u64::from_be_bytes(bytes))
    }

    pub fn add(&self, other: &Self) -> Result<Self, QantoDAGError> {
        let val1 = self.decrypt(&[])?;
        let val2 = other.decrypt(&[])?;
        Ok(Self::new(val1.saturating_add(val2), &[]))
    }
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
    #[instrument]
    pub fn execute(&mut self, input: &str, gas_limit: u64) -> Result<String, QantoDAGError> {
        let mut gas_used = 0;
        let mut charge_gas = |cost: u64| -> Result<(), QantoDAGError> {
            gas_used += cost;
            if gas_used > gas_limit || gas_used > self.gas_balance {
                Err(QantoDAGError::SmartContractError(format!(
                    "Out of gas. Limit: {gas_limit}, Used: {gas_used}"
                )))
            } else {
                Ok(())
            }
        };

        let result = if self.code.contains("echo") {
            charge_gas(10)?;
            charge_gas(input.len() as u64)?;
            self.storage
                .insert("last_input".to_string(), input.to_string());
            Ok(format!("echo: {input}"))
        } else if self.code.contains("increment_counter") {
            charge_gas(50)?;
            let counter_str = self
                .storage
                .entry("counter".to_string())
                .or_insert_with(|| "0".to_string());
            let current_val: u64 = counter_str.parse().unwrap_or(0);
            *counter_str = (current_val + 1).to_string();
            Ok(format!("counter updated to: {counter_str}"))
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
    pub qr_signature: QuantumResistantSignature,
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
        // FIX: Format the u64 reward into a decimal representation of QNTO.
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
    #[instrument(skip(data))]
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
        let id = hex::encode(Keccak256::digest(&pre_signature_data_for_id));

        let qr_signature = QuantumResistantSignature::sign(
            data.qr_signing_key,
            data.qr_public_key,
            &pre_signature_data_for_id,
        )?;

        let homomorphic_encrypted_data = data
            .transactions
            .iter()
            .map(|tx| HomomorphicEncrypted::new(tx.amount, data.qr_public_key.as_bytes()))
            .collect();

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
            qr_signature,
            cross_chain_swaps: vec![],
            homomorphic_encrypted: homomorphic_encrypted_data,
            smart_contracts: vec![],
            carbon_credentials: vec![],
            epoch: data.current_epoch,
        })
    }

    pub fn serialize_for_signing(data: &SigningData) -> Result<Vec<u8>, QantoDAGError> {
        let mut hasher = Keccak256::new();
        hasher.update(data.chain_id.to_le_bytes());
        hasher.update(data.merkle_root.as_bytes());
        for parent in data.parents {
            hasher.update(parent.as_bytes());
        }
        for tx in data.transactions {
            hasher.update(tx.id.as_bytes());
        }
        hasher.update(data.timestamp.to_be_bytes());
        hasher.update(data.difficulty.to_be_bytes());
        hasher.update(data.height.to_be_bytes());
        hasher.update(data.validator.as_bytes());
        hasher.update(data.miner.as_bytes());
        Ok(hasher.finalize().to_vec())
    }

    #[instrument]
    pub fn compute_merkle_root(transactions: &[Transaction]) -> Result<String, QantoDAGError> {
        if transactions.is_empty() {
            return Ok(hex::encode(Keccak256::digest([])));
        }
        let mut leaves: Vec<Vec<u8>> = transactions
            .par_iter()
            .map(|tx| Keccak256::digest(tx.id.as_bytes()).to_vec())
            .collect();

        while leaves.len() > 1 {
            if !leaves.len().is_multiple_of(2) {
                leaves.push(leaves.last().unwrap().clone());
            }
            leaves = leaves
                .par_chunks(2)
                .map(|chunk| {
                    let mut hasher = Keccak256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[1]);
                    hasher.finalize().to_vec()
                })
                .collect();
        }
        Ok(hex::encode(leaves.first().ok_or_else(|| {
            QantoDAGError::InvalidBlock("Merkle root computation failed".to_string())
        })?))
    }

    #[instrument]
    pub fn hash(&self) -> String {
        let mut hasher = Keccak256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(self.timestamp.to_be_bytes());
        hasher.update(self.nonce.to_le_bytes());
        hex::encode(hasher.finalize())
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
        let data_to_verify = QantoBlock::serialize_for_signing(&signing_data)?;

        Ok(self.qr_signature.verify(&data_to_verify))
    }
}

/// Configuration for creating a new QantoDAG instance.
pub struct QantoDagConfig<'a> {
    pub initial_validator: String,
    pub target_block_time: u64,
    pub num_chains: u32,
    pub qr_signing_key: &'a dilithium5::SecretKey,
    pub qr_public_key: &'a dilithium5::PublicKey,
}

#[derive(Debug)]
pub struct QantoDAG {
    pub blocks: Arc<RwLock<HashMap<String, QantoBlock>>>,
    pub tips: Arc<RwLock<HashMap<u32, HashSet<String>>>>,
    pub validators: Arc<RwLock<HashMap<String, u64>>>,
    pub target_block_time: u64,
    pub emission: Arc<RwLock<Emission>>,
    pub num_chains: Arc<RwLock<u32>>,
    pub finalized_blocks: Arc<RwLock<HashSet<String>>>,
    pub chain_loads: Arc<RwLock<HashMap<u32, u64>>>,
    pub difficulty_history: Arc<RwLock<Vec<(u64, u64)>>>,
    pub block_creation_timestamps: Arc<RwLock<HashMap<String, u64>>>,
    pub anomaly_history: Arc<RwLock<HashMap<String, u64>>>,
    pub cross_chain_swaps: Arc<RwLock<HashMap<String, CrossChainSwap>>>,
    pub smart_contracts: Arc<RwLock<HashMap<String, SmartContract>>>,
    pub cache: Arc<RwLock<LruCache<String, QantoBlock>>>,
    pub db: Arc<DB>,
    pub saga: Arc<PalletSaga>,
    pub self_arc: Weak<QantoDAG>,
    pub current_epoch: Arc<RwLock<u64>>,
}

impl QantoDAG {
    #[instrument(skip(config, saga, db))]
    pub fn new(
        config: QantoDagConfig,
        saga: Arc<PalletSaga>,
        db: DB,
    ) -> Result<Arc<Self>, QantoDAGError> {
        let mut blocks_map = HashMap::new();
        let mut tips_map = HashMap::new();
        let mut validators_map = HashMap::new();
        let genesis_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        for chain_id_val in 0..config.num_chains {
            let genesis_creation_data = QantoBlockCreationData {
                chain_id: chain_id_val,
                parents: vec![],
                transactions: vec![],
                difficulty: INITIAL_DIFFICULTY,
                validator: config.initial_validator.clone(),
                miner: config.initial_validator.clone(),
                qr_signing_key: config.qr_signing_key,
                qr_public_key: config.qr_public_key,
                timestamp: genesis_timestamp,
                current_epoch: 0,
                height: 0,
            };
            let mut genesis_block = QantoBlock::new(genesis_creation_data)?;
            genesis_block.reward = INITIAL_BLOCK_REWARD;
            let genesis_id = genesis_block.id.clone();

            blocks_map.insert(genesis_id.clone(), genesis_block);
            tips_map
                .entry(chain_id_val)
                .or_insert_with(HashSet::new)
                .insert(genesis_id);
        }
        validators_map.insert(
            config.initial_validator.clone(),
            MIN_VALIDATOR_STAKE * config.num_chains as u64 * 2,
        );

        let dag = Self {
            blocks: Arc::new(RwLock::new(blocks_map)),
            tips: Arc::new(RwLock::new(tips_map)),
            validators: Arc::new(RwLock::new(validators_map)),
            target_block_time: config.target_block_time,
            emission: Arc::new(RwLock::new(Emission::default_with_timestamp(
                genesis_timestamp,
                config.num_chains,
            ))),
            num_chains: Arc::new(RwLock::new(config.num_chains.max(1))),
            finalized_blocks: Arc::new(RwLock::new(HashSet::new())),
            chain_loads: Arc::new(RwLock::new(HashMap::new())),
            difficulty_history: Arc::new(RwLock::new(Vec::new())),
            block_creation_timestamps: Arc::new(RwLock::new(HashMap::new())),
            anomaly_history: Arc::new(RwLock::new(HashMap::new())),
            cross_chain_swaps: Arc::new(RwLock::new(HashMap::new())),
            smart_contracts: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(CACHE_SIZE.max(1)).unwrap(),
            ))),
            db: Arc::new(db),
            saga,
            self_arc: Weak::new(),
            current_epoch: Arc::new(RwLock::new(0)),
        };

        let arc_dag = Arc::new(dag);
        let weak_self = Arc::downgrade(&arc_dag);

        let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;

        unsafe {
            (*ptr).self_arc = weak_self;
        }

        Ok(arc_dag)
    }

    #[instrument(skip(self, wallet, mempool, utxos, miner))]
    pub async fn run_solo_miner(
        self: Arc<Self>,
        wallet: Arc<Wallet>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        miner: Arc<Miner>,
        mining_interval_secs: u64,
    ) {
        info!(
            "SOLO MINER: Starting proactive mining loop with an interval of {} seconds.",
            mining_interval_secs
        );

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(mining_interval_secs)).await;
            debug!("SOLO MINER: Waking up to attempt block creation.");

            let miner_address = wallet.address();
            let (signing_key, public_key) = match wallet.get_keypair() {
                Ok(pair) => pair,
                Err(e) => {
                    warn!(
                        "SOLO MINER: Could not get keypair, skipping cycle. Error: {}",
                        e
                    );
                    continue;
                }
            };

            let chain_id_to_mine: u32 = 0;

            let mut candidate_block = match self
                .create_candidate_block(
                    &signing_key,
                    &public_key,
                    &miner_address,
                    &mempool,
                    &utxos,
                    chain_id_to_mine,
                )
                .await
            {
                Ok(block) => {
                    debug!(
                        "SOLO MINER: Successfully created candidate block {}.",
                        block.id
                    );
                    block
                }
                Err(e) => {
                    warn!(
                        "SOLO MINER: Failed to create candidate block: {}. Retrying after delay.",
                        e
                    );
                    continue;
                }
            };

            debug!(
                "SOLO MINER: Starting proof-of-work for candidate block {}...",
                candidate_block.id
            );
            if let Err(e) = miner.solve_pow(&mut candidate_block) {
                warn!(
                    "SOLO MINER: Mining failed for the current candidate block: {}",
                    e
                );
                continue;
            }

            let mined_block = candidate_block;
            info!(
                "SOLO MINER: Successfully mined block with ID: {}",
                mined_block.id
            );
            println!("{mined_block}");

            match self.add_block(mined_block.clone(), &utxos).await {
                Ok(true) => {
                    info!("SOLO MINER: Successfully added new block to the QantoDAG.");
                    mempool
                        .read()
                        .await
                        .remove_transactions(&mined_block.transactions)
                        .await;
                }
                Ok(false) => {
                    warn!("SOLO MINER: Mined block was rejected by the DAG (already exists?).")
                }
                Err(e) => warn!("SOLO MINER: Failed to add mined block to DAG: {}", e),
            }
        }
    }

    #[instrument]
    pub async fn get_average_tx_per_block(&self) -> f64 {
        let blocks_guard = self.blocks.read().await;
        if blocks_guard.is_empty() {
            return 0.0;
        }
        let total_txs: usize = blocks_guard.values().map(|b| b.transactions.len()).sum();
        total_txs as f64 / blocks_guard.len() as f64
    }

    #[instrument(skip(self, block, utxos_arc))]
    pub async fn add_block(
        &self,
        block: QantoBlock,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<bool, QantoDAGError> {
        if self.blocks.read().await.contains_key(&block.id) {
            warn!("Attempted to add block {} which already exists.", block.id);
            return Ok(false);
        }

        if !self.is_valid_block(&block, utxos_arc).await? {
            return Err(QantoDAGError::InvalidBlock(format!(
                "Block {} failed validation in add_block",
                block.id
            )));
        }

        let mut blocks_write_guard = self.blocks.write().await;
        let mut utxos_write_guard = utxos_arc.write().await;
        let mut validators_guard = self.validators.write().await;
        let mut tips_guard = self.tips.write().await;

        if blocks_write_guard.contains_key(&block.id) {
            warn!(
                "Block {} already exists (double check after write lock).",
                block.id
            );
            return Ok(false);
        }

        let anomaly_score = self
            .detect_anomaly_internal(&blocks_write_guard, &block)
            .await?;
        if anomaly_score > 0.9 {
            if let Some(stake) = validators_guard.get_mut(&block.validator) {
                let penalty = (*stake * SLASHING_PENALTY) / 100;
                *stake = stake.saturating_sub(penalty);
                info!(
                    "Slashed validator {} by {} for anomaly (score: {})",
                    block.validator, penalty, anomaly_score
                );
            }
        }

        for tx in &block.transactions {
            for input in &tx.inputs {
                let utxo_id = format!("{}_{}", input.tx_id, input.output_index);
                utxos_write_guard.remove(&utxo_id);
            }
            for (index, output) in tx.outputs.iter().enumerate() {
                let utxo_id = format!("{}_{}", tx.id, index);
                utxos_write_guard.insert(
                    utxo_id.clone(),
                    UTXO {
                        address: output.address.clone(),
                        amount: output.amount,
                        tx_id: tx.id.clone(),
                        output_index: index as u32,
                        explorer_link: format!("https://qantoblockexplorer.org/utxo/{utxo_id}"),
                    },
                );
            }
        }

        let current_tips = tips_guard
            .entry(block.chain_id)
            .or_insert_with(HashSet::new);
        for parent_id in &block.parents {
            current_tips.remove(parent_id);
        }
        current_tips.insert(block.id.clone());

        let block_for_db = block.clone();
        blocks_write_guard.insert(block.id.clone(), block);

        drop(validators_guard);
        drop(tips_guard);
        drop(blocks_write_guard);
        drop(utxos_write_guard);

        let db_clone = self.db.clone();
        let id_bytes = block_for_db.id.as_bytes().to_vec();
        let block_bytes = serde_json::to_vec(&block_for_db)?;

        task::spawn_blocking(move || db_clone.put(id_bytes, block_bytes)).await??;

        let mut emission = self.emission.write().await;
        emission
            .update_supply(block_for_db.reward)
            .map_err(QantoDAGError::EmissionError)?;

        BLOCKS_PROCESSED.inc();
        TRANSACTIONS_PROCESSED.inc_by(block_for_db.transactions.len() as u64);
        Ok(true)
    }

    #[instrument]
    pub async fn get_id(&self) -> u32 {
        0
    }

    #[instrument]
    pub async fn get_tips(&self, chain_id: u32) -> Option<Vec<String>> {
        self.tips
            .read()
            .await
            .get(&chain_id)
            .map(|tips_set| tips_set.iter().cloned().collect())
    }

    #[instrument]
    pub async fn add_validator(&self, address: String, stake: u64) {
        let mut validators_guard = self.validators.write().await;
        validators_guard.insert(address, stake.max(MIN_VALIDATOR_STAKE));
    }

    #[instrument]
    pub async fn initiate_cross_chain_swap(
        &self,
        params: CrossChainSwapParams,
    ) -> Result<String, QantoDAGError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let swap_id = hex::encode(Keccak256::digest(
            format!(
                "swap_{}_{}_{}_{}",
                params.initiator, params.responder, params.amount, now
            )
            .as_bytes(),
        ));
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
        self.cross_chain_swaps
            .write()
            .await
            .insert(swap_id.clone(), swap);
        Ok(swap_id)
    }

    #[instrument]
    pub async fn redeem_cross_chain_swap(
        &self,
        swap_id: &str,
        secret: &str,
    ) -> Result<(), QantoDAGError> {
        let mut swaps = self.cross_chain_swaps.write().await;
        let swap = swaps.get_mut(swap_id).ok_or_else(|| {
            QantoDAGError::CrossChainSwapError(format!("Swap ID {swap_id} not found"))
        })?;

        let hash_of_provided_secret = hex::encode(Keccak256::digest(secret.as_bytes()));
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

    #[instrument]
    pub async fn deploy_smart_contract(
        &self,
        code: String,
        owner: String,
        initial_gas: u64,
    ) -> Result<String, QantoDAGError> {
        let contract_id = hex::encode(Keccak256::digest(code.as_bytes()));
        let contract = SmartContract {
            contract_id: contract_id.clone(),
            code,
            storage: HashMap::new(),
            owner,
            gas_balance: initial_gas,
        };
        self.smart_contracts
            .write()
            .await
            .insert(contract_id.clone(), contract);
        Ok(contract_id)
    }

    #[instrument(skip(self, qr_signing_key, qr_public_key, mempool_arc, _utxos_arc))]
    pub async fn create_candidate_block(
        &self,
        qr_signing_key: &dilithium5::SecretKey,
        qr_public_key: &dilithium5::PublicKey,
        validator_address: &str,
        mempool_arc: &Arc<RwLock<Mempool>>,
        _utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
        chain_id_val: u32,
    ) -> Result<QantoBlock, QantoDAGError> {
        {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let mut timestamps_guard = self.block_creation_timestamps.write().await;
            let recent_blocks = timestamps_guard
                .values()
                .filter(|&&t| now.saturating_sub(t) < 60)
                .count() as u64;
            if recent_blocks >= MAX_BLOCKS_PER_MINUTE {
                return Err(QantoDAGError::InvalidBlock(format!(
                    "Rate limit exceeded: {recent_blocks} blocks in last minute"
                )));
            }
            if timestamps_guard.len() > 1000 {
                timestamps_guard.retain(|_, t_val| now.saturating_sub(*t_val) < 3600);
            }
        }
        {
            let validators_guard = self.validators.read().await;
            let stake = validators_guard.get(validator_address).ok_or_else(|| {
                QantoDAGError::InvalidBlock(format!(
                    "Validator {validator_address} not found or no stake"
                ))
            })?;
            if *stake < MIN_VALIDATOR_STAKE {
                return Err(QantoDAGError::InvalidBlock(format!(
                    "Insufficient stake for validator {validator_address}: {stake} < {MIN_VALIDATOR_STAKE}"
                )));
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
            let blocks_guard = self.blocks.read().await;
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

        let epoch = *self.current_epoch.read().await;

        let current_difficulty = *self.saga.economy.current_difficulty.read().await;

        // FIX: Calculate total fees from selected transactions BEFORE calculating reward.
        let total_fees = selected_transactions.iter().map(|tx| tx.fee).sum::<u64>();

        // A temporary block is created with a placeholder transaction vec
        // to calculate the reward. The actual transactions will be added later.
        let temp_block_for_reward_calc = QantoBlock::new(QantoBlockCreationData {
            chain_id: chain_id_val,
            parents: parent_tips.clone(),
            transactions: vec![],
            difficulty: current_difficulty,
            validator: validator_address.to_string(),
            miner: validator_address.to_string(),
            qr_signing_key,
            qr_public_key,
            timestamp: new_timestamp,
            current_epoch: epoch,
            height,
        })?;

        let self_arc_strong = self
            .self_arc
            .upgrade()
            .ok_or(QantoDAGError::SelfReferenceNotInitialized)?;
        let reward = self
            .saga
            .calculate_dynamic_reward(&temp_block_for_reward_calc, &self_arc_strong, total_fees)
            .await?;

        // The block reward now goes entirely to the miner.
        let coinbase_outputs = vec![Output {
            address: validator_address.to_string(),
            amount: reward,
            homomorphic_encrypted: HomomorphicEncrypted::new(reward, &[]),
        }];

        let reward_tx = Transaction::new_coinbase(
            validator_address.to_string(),
            reward,
            qr_signing_key.as_bytes(),
            qr_public_key.as_bytes(),
            coinbase_outputs,
        )?;

        let mut transactions_for_block = vec![reward_tx];
        transactions_for_block.extend(selected_transactions);

        let mut cross_chain_references = vec![];
        let num_chains_val = *self.num_chains.read().await;
        if num_chains_val > 1 {
            let prev_chain = (chain_id_val + num_chains_val - 1) % num_chains_val;
            let tips_guard = self.tips.read().await;
            if let Some(prev_tips_set) = tips_guard.get(&prev_chain) {
                if let Some(tip_val) = prev_tips_set.iter().next() {
                    cross_chain_references.push((prev_chain, tip_val.clone()));
                }
            }
        }

        let mut block = QantoBlock::new(QantoBlockCreationData {
            chain_id: chain_id_val,
            parents: parent_tips,
            transactions: transactions_for_block,
            difficulty: current_difficulty,
            validator: validator_address.to_string(),
            miner: validator_address.to_string(),
            qr_signing_key,
            qr_public_key,
            timestamp: new_timestamp,
            current_epoch: epoch,
            height,
        })?;
        block.cross_chain_references = cross_chain_references;
        block.reward = reward;

        self.block_creation_timestamps
            .write()
            .await
            .insert(block.id.clone(), new_timestamp);
        *self
            .chain_loads
            .write()
            .await
            .entry(chain_id_val)
            .or_insert(0) += block.transactions.len() as u64;

        Ok(block)
    }

    #[instrument(skip(self, block, utxos_arc))]
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
            return Err(QantoDAGError::InvalidBlock(format!(
                "Block exceeds size limits: {} txns, {} bytes",
                block.transactions.len(),
                serialized_size
            )));
        }

        let expected_merkle_root = QantoBlock::compute_merkle_root(&block.transactions)?;
        if block.merkle_root != expected_merkle_root {
            return Err(QantoDAGError::MerkleRootMismatch);
        }
        if !block.verify_signature()? {
            return Err(QantoDAGError::QuantumSignature(
                "Quantum-resistant signature verification failed".to_string(),
            ));
        }

        let target_hash_bytes = Miner::calculate_target_from_difficulty(block.difficulty);
        let block_pow_hash = block.hash();
        if !Miner::hash_meets_target(&hex::decode(block_pow_hash).unwrap(), &target_hash_bytes) {
            return Err(QantoDAGError::InvalidBlock(
                "Proof-of-Work not satisfied".to_string(),
            ));
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if block.timestamp > now + TEMPORAL_CONSENSUS_WINDOW {
            return Err(QantoDAGError::InvalidBlock(format!(
                "Timestamp {} is too far in the future",
                block.timestamp
            )));
        }

        {
            let blocks_guard = self.blocks.read().await;
            let mut max_parent_height = 0;
            for parent_id in &block.parents {
                let parent_block = blocks_guard.get(parent_id).ok_or_else(|| {
                    QantoDAGError::InvalidParent(format!("Parent block {parent_id} not found"))
                })?;
                if parent_block.chain_id != block.chain_id {
                    return Err(QantoDAGError::InvalidParent(format!(
                        "Parent {} on chain {} but block {} on chain {}",
                        parent_id, parent_block.chain_id, block.id, block.chain_id
                    )));
                }
                if block.timestamp <= parent_block.timestamp {
                    return Err(QantoDAGError::InvalidBlock(format!(
                        "Block timestamp {} is not after parent timestamp {}",
                        block.timestamp, parent_block.timestamp
                    )));
                }
                if parent_block.height > max_parent_height {
                    max_parent_height = parent_block.height;
                }
            }
            if !block.parents.is_empty() && block.height != max_parent_height + 1 {
                return Err(QantoDAGError::InvalidBlock(format!(
                    "Invalid block height. Expected {}, got {}",
                    max_parent_height + 1,
                    block.height
                )));
            }

            for (_ref_chain_id, ref_block_id) in &block.cross_chain_references {
                if !blocks_guard.contains_key(ref_block_id) {
                    return Err(QantoDAGError::CrossChainReferenceError(format!(
                        "Reference block {ref_block_id} not found"
                    )));
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

        let blocks_guard = self.blocks.read().await;
        let anomaly_score = self.detect_anomaly_internal(&blocks_guard, block).await?;
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

    #[instrument]
    pub async fn validate_transaction(
        &self,
        tx: &Transaction,
        utxos_map: &HashMap<String, UTXO>,
    ) -> bool {
        tx.verify(self, utxos_map).await.is_ok()
    }

    #[instrument]
    pub async fn finalize_blocks(&self) -> Result<(), QantoDAGError> {
        let blocks_guard = self.blocks.read().await;
        let mut finalized_guard = self.finalized_blocks.write().await;
        let tips_guard = self.tips.read().await;
        let num_chains_val = *self.num_chains.read().await;

        for chain_id_val in 0..num_chains_val {
            if let Some(chain_tips) = tips_guard.get(&chain_id_val) {
                for tip_id in chain_tips {
                    let mut path_to_finalize = Vec::new();
                    let mut current_id = tip_id.clone();

                    for _depth in 0..FINALIZATION_DEPTH {
                        if finalized_guard.contains(&current_id) {
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
                            if finalized_guard.insert(id_to_finalize.clone()) {
                                log::debug!("Finalized block: {id_to_finalize}");
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[instrument]
    pub async fn dynamic_sharding(&self) -> Result<(), QantoDAGError> {
        let mut chain_loads_guard = self.chain_loads.write().await;
        let mut num_chains_guard = self.num_chains.write().await;
        if chain_loads_guard.is_empty() {
            return Ok(());
        }

        let total_load: u64 = chain_loads_guard.values().sum();
        if *num_chains_guard == 0 {
            return Ok(());
        }
        let avg_load = total_load / (*num_chains_guard as u64);
        let split_threshold = avg_load.saturating_mul(SHARD_THRESHOLD as u64);

        let chains_to_split: Vec<u32> = chain_loads_guard
            .iter()
            .filter(|(_, &load)| load > split_threshold)
            .map(|(&id, _)| id)
            .collect();

        if chains_to_split.is_empty() {
            return Ok(());
        }

        let initial_validator_placeholder = DEV_ADDRESS.to_string();
        let (placeholder_pk, placeholder_sk) = dilithium5::keypair();

        let mut tips_guard = self.tips.write().await;
        let mut blocks_guard = self.blocks.write().await;

        let epoch = *self.current_epoch.read().await;

        for chain_id_to_split in chains_to_split {
            if *num_chains_guard == u32::MAX {
                continue;
            }

            let new_chain_id = *num_chains_guard;
            *num_chains_guard += 1;

            let original_load = chain_loads_guard.remove(&chain_id_to_split).unwrap_or(0);
            let new_load_for_old = original_load / 2;
            let new_load_for_new = original_load - new_load_for_old;

            chain_loads_guard.insert(chain_id_to_split, new_load_for_old);
            chain_loads_guard.insert(new_chain_id, new_load_for_new);

            let new_genesis_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let parent_difficulty = *self.saga.economy.current_difficulty.read().await;

            let mut genesis_block = QantoBlock::new(QantoBlockCreationData {
                chain_id: new_chain_id,
                parents: vec![],
                transactions: vec![],
                difficulty: parent_difficulty,
                validator: initial_validator_placeholder.clone(),
                miner: initial_validator_placeholder.clone(),
                qr_signing_key: &placeholder_sk,
                qr_public_key: &placeholder_pk,
                timestamp: new_genesis_timestamp,
                current_epoch: epoch,
                height: 0,
            })?;
            genesis_block.reward = INITIAL_BLOCK_REWARD;
            let new_genesis_id = genesis_block.id.clone();

            blocks_guard.insert(new_genesis_id.clone(), genesis_block);
            let mut new_tips = HashSet::new();
            new_tips.insert(new_genesis_id);
            tips_guard.insert(new_chain_id, new_tips);

            info!(
                "SHARDING: High load on chain {chain_id_to_split} triggered split. New chain {new_chain_id} created."
            );
        }
        Ok(())
    }

    #[instrument(skip(self, proposer_address, rule_name, new_value))]
    pub async fn propose_governance(
        &self,
        proposer_address: String,
        rule_name: String,
        new_value: f64,
        _creation_epoch: u64,
    ) -> Result<String, QantoDAGError> {
        {
            let validators_guard = self.validators.read().await;
            let stake = validators_guard.get(&proposer_address).ok_or_else(|| {
                QantoDAGError::Governance("Proposer not found or has no stake".to_string())
            })?;
            if *stake < MIN_VALIDATOR_STAKE * 10 {
                return Err(QantoDAGError::Governance(
                    "Insufficient stake to create a governance proposal.".to_string(),
                ));
            }
        }

        let proposal_id_val = format!("saga-proposal-{}", Uuid::new_v4());
        let proposal_obj = GovernanceProposal {
            id: proposal_id_val.clone(),
            proposer: proposer_address,
            proposal_type: ProposalType::UpdateRule(rule_name, new_value),
            votes_for: 0.0,
            votes_against: 0.0,
            status: ProposalStatus::Voting,
            voters: vec![],
            creation_epoch: *self.current_epoch.read().await,
            justification: None,
        };

        // --- FIX (E0599): Interact with DashMap directly ---
        // No .write().await needed. DashMap handles concurrent access.
        self.saga
            .governance
            .proposals
            .insert(proposal_id_val.clone(), proposal_obj);

        info!("New governance proposal {} submitted.", proposal_id_val);
        Ok(proposal_id_val)
    }

    #[instrument]
    pub async fn vote_governance(
        &self,
        voter: String,
        proposal_id: String,
        vote_for: bool,
    ) -> Result<(), QantoDAGError> {
        let stake_val: u64;
        {
            let validators_guard = self.validators.read().await;
            stake_val = *validators_guard.get(&voter).ok_or_else(|| {
                QantoDAGError::Governance("Voter not found or no stake".to_string())
            })?;
        }

        // --- FIX (E0599): Interact with DashMap directly ---
        // Use .get_mut() for mutable access.
        let mut proposal_obj = self
            .saga
            .governance
            .proposals
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

    #[instrument]
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

    #[instrument]
    pub async fn select_validator(&self) -> Option<String> {
        let validators_guard = self.validators.read().await;
        if validators_guard.is_empty() {
            return None;
        }
        let total_stake_val: u64 = validators_guard.values().sum();
        if total_stake_val == 0 {
            let validator_keys: Vec<String> = validators_guard.keys().cloned().collect();
            if validator_keys.is_empty() {
                return None;
            }
            let index = rand::thread_rng().gen_range(0..validator_keys.len());
            return Some(validator_keys[index].clone());
        }
        let mut rand_num = rand::thread_rng().gen_range(0..total_stake_val);
        for (validator_addr, stake_val) in validators_guard.iter() {
            if rand_num < *stake_val {
                return Some(validator_addr.clone());
            }
            rand_num -= *stake_val;
        }
        validators_guard.keys().next().cloned()
    }

    #[instrument]
    pub async fn get_state_snapshot(
        &self,
        chain_id_val: u32,
    ) -> (HashMap<String, QantoBlock>, HashMap<String, UTXO>) {
        let blocks_guard = self.blocks.read().await;
        let mut chain_blocks_map = HashMap::new();
        let mut utxos_map_for_chain = HashMap::new();
        for (id_val, block_val) in blocks_guard.iter() {
            if block_val.chain_id == chain_id_val {
                chain_blocks_map.insert(id_val.clone(), block_val.clone());
                for tx_val in &block_val.transactions {
                    for (index_val, output_val) in tx_val.outputs.iter().enumerate() {
                        let utxo_id_val = format!("{}_{}", tx_val.id, index_val);
                        utxos_map_for_chain.insert(
                            utxo_id_val.clone(),
                            UTXO {
                                address: output_val.address.clone(),
                                amount: output_val.amount,
                                tx_id: tx_val.id.clone(),
                                output_index: index_val as u32,
                                explorer_link: format!(
                                    "https://qantoblockexplorer.org/utxo/{utxo_id_val}"
                                ),
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

        // Difficulty adjustment is now fully handled by SAGA during epoch evolution.
        if let Err(e) = self.finalize_blocks().await {
            warn!("Failed to finalize blocks during maintenance: {e}");
        }
        if let Err(e) = self.dynamic_sharding().await {
            warn!("Failed to run dynamic sharding during maintenance: {e}");
        }

        let mut epoch_guard = self.current_epoch.write().await;
        *epoch_guard += 1;
        let current_epoch = *epoch_guard;

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
        }
    }
}