// src/transaction.rs

//! --- Qanto Transaction ---
//! v0.1.0 - Initial Version
//!
//! This module defines the transaction data structure and related operations.
//!
//! # Features
//! - **Quantum-Resistant Signatures**: Utilizes quantum-resistant signature algorithms
//!   for secure transaction authentication.
//! - **Homomorphic Encryption**: Supports homomorphic encryption to enable secure
//!   computation on encrypted data.
//! - **Dynamic Fee Structure**: Implements a dynamic fee structure based on transaction
//!   complexity and network congestion.
//! - **Scalability**: Designed to handle a high volume of transactions per second (TPS)
//!   while maintaining low latency.
//! - **Privacy**: Ensures user privacy by keeping transaction details confidential.

use crate::gas_fee_model::{FeeBreakdown, GasFeeError, GasFeeModel, StorageDuration};
use crate::omega;
use crate::qantodag::QantoDAG;
use crate::types::{HomomorphicEncrypted, QuantumResistantSignature, UTXO};
use bincode::Options;
use my_blockchain::qanto_hash;
use rayon::prelude::*;

use ahash::AHashMap as HashMap;
use ahash::AHashSet as HashSet;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use zeroize::Zeroize;

use k256::ecdsa::{RecoveryId, Signature as K256Signature, VerifyingKey};
use sha3::{Digest, Keccak256};

// --- Performance & Fee Constants ---
/// The maximum number of transactions allowed per minute to align with the 10M+ TPS goal.
const MAX_TRANSACTIONS_PER_MINUTE: u64 = 600_000_000;
/// The minimum gas limit required for any transaction to prevent dust spam (C-H1).
pub const MIN_TX_GAS: u128 = 21_000;
/// The minimum fee required for any transaction to prevent dust spam (C-H1).
pub const MIN_TX_FEE: u128 = 100;
/// The maximum number of key-value pairs allowed in a transaction's metadata.
const MAX_METADATA_PAIRS: usize = 16;
/// The maximum length of a metadata key.
const MAX_METADATA_KEY_LEN: usize = 64;
/// The maximum length of a metadata value.
const MAX_METADATA_VALUE_LEN: usize = 256;

// --- New Dynamic Fee Structure ---
// Thresholds are defined in the smallest units of QAN for precision.
/// The number of decimals per QAN when formatting base units.
pub const DECIMALS_PER_QAN: usize = 9;
/// The smallest unit multiplier (1 QAN = 1,000,000,000 base units)
pub const SMALLEST_UNITS_PER_QAN: u128 = 1_000_000_000;
/// The threshold for the first fee tier (under 1,000,000 QAN).
pub const FEE_TIER1_THRESHOLD: u128 = 1_000_000 * crate::Q_SCALE;
/// The threshold for the second fee tier (1M to 10M QAN).
pub const FEE_TIER2_THRESHOLD: u128 = 10_000_000 * crate::Q_SCALE;
/// The threshold for the third fee tier (10M to 100M QAN).
pub const FEE_TIER3_THRESHOLD: u128 = 100_000_000 * crate::Q_SCALE;
/// The fee rate for the first tier (0%).
pub const FEE_RATE_TIER1_FIXED: u128 = 0;
/// The fee rate for the second tier (1%).
pub const FEE_RATE_TIER2_FIXED: u128 = 10_000_000;
/// The fee rate for the third tier (2%).
pub const FEE_RATE_TIER3_FIXED: u128 = 20_000_000;
/// The fee rate for the fourth tier (3%).
pub const FEE_RATE_TIER4_FIXED: u128 = 30_000_000;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("ΛΣ-ΩMEGA Protocol rejected the action as unstable")]
    OmegaRejection,
    #[error("Invalid address format")]
    InvalidAddress,
    #[error("Quantum-resistant signature verification failed")]
    QuantumSignatureVerification,
    #[error("Invalid transaction signature")]
    InvalidTransactionSignature,
    #[error("Insufficient funds")]
    InsufficientFunds,
    #[error("Invalid transaction structure: {0}")]
    InvalidStructure(String),
    #[error("Homomorphic encryption error: {0}")]
    HomomorphicError(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Anomaly detected: {0}")]
    AnomalyDetected(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Timestamp error")]
    TimestampError,
    #[error("Wallet error: {0}")]
    Wallet(#[from] crate::wallet::WalletError),
    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),
    #[error("Post-quantum crypto error: {0}")]
    PqCrypto(String),
    #[error("Gas fee error: {0}")]
    GasFee(#[from] GasFeeError),
    #[error("Legacy ECDSA schemes are deprecated. Please use a Hybrid Signature.")]
    LegacySchemeDeprecated,
}

#[derive(Clone, Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
pub struct Input {
    pub tx_id: String,
    pub output_index: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Output {
    pub address: String,
    pub amount: u128,
    pub homomorphic_encrypted: HomomorphicEncrypted,
}

pub struct TransactionConfig {
    pub sender: String,
    pub receiver: String,
    pub amount: u128,
    pub fee: u128,
    pub gas_limit: u128,
    pub gas_price: u128,
    pub priority_fee: u128,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub metadata: Option<HashMap<String, String>>,
    pub tx_timestamps: Arc<RwLock<HashMap<String, u64>>>,
    pub chain_id: u32,
}

#[derive(Debug, Serialize)]
struct TransactionIdPayload {
    sender: String,
    receiver: String,
    amount: u128,
    fee: u128,
    gas_limit: u128,
    gas_used: u128,
    gas_price: u128,
    priority_fee: u128,
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    metadata: BTreeMap<String, String>,
    timestamp: u64,
    transaction_kind: TransactionKind,
    chain_id: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct TransactionSigningPayload {
    pub(crate) id: String,
    pub(crate) sender: String,
    pub(crate) receiver: String,
    pub(crate) amount: u128,
    pub(crate) fee: u128,
    pub(crate) gas_limit: u128,
    pub(crate) gas_used: u128,
    pub(crate) gas_price: u128,
    pub(crate) priority_fee: u128,
    pub(crate) inputs: Vec<Input>,
    pub(crate) outputs: Vec<Output>,
    pub(crate) metadata: BTreeMap<String, String>,
    pub(crate) timestamp: u64,
    pub(crate) transaction_kind: TransactionKind,
    pub(crate) chain_id: u32,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionKind {
    Transfer,
    Stake,
    Unstake,
    Delegate,
    Vote,
    Proposal,
    BridgeLock,
    BridgeObserve,
    BridgeClaim,
    AirdropClaim,
}

impl Default for TransactionKind {
    fn default() -> Self {
        TransactionKind::Transfer
    }
}

fn metadata_is_empty(m: &HashMap<String, String>) -> bool {
    m.is_empty()
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Transaction {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub amount: u128,
    pub fee: u128,
    pub gas_limit: u128,
    pub gas_used: u128,
    pub gas_price: u128,
    pub priority_fee: u128,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "metadata_is_empty")]
    pub metadata: HashMap<String, String>,
    pub signature: QuantumResistantSignature,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_breakdown: Option<FeeBreakdown>,
    #[serde(default)]
    pub transaction_kind: TransactionKind,
    #[serde(default)]
    pub chain_id: u32,
}

/// Calculates the transaction fee based on the new tiered structure.
pub fn calculate_dynamic_fee(amount: u128) -> u128 {
    let rate = if amount < FEE_TIER1_THRESHOLD {
        FEE_RATE_TIER1_FIXED // 0% fee for amounts under 1,000,000 QAN
    } else if amount < FEE_TIER2_THRESHOLD {
        FEE_RATE_TIER2_FIXED // 1% fee for amounts between 1M and 10M QAN
    } else if amount < FEE_TIER3_THRESHOLD {
        FEE_RATE_TIER3_FIXED // 2% fee for amounts between 10M and 100M QAN
    } else {
        FEE_RATE_TIER4_FIXED // 3% for amounts over 100M QAN
    };

    // Calculate fee using fixed-point integer arithmetic (scale 1e9)
    // multiplication before division to maintain precision
    amount
        .checked_mul(rate)
        .and_then(|r| r.checked_div(crate::QANTO_SCALE))
        .unwrap_or(0)
}

impl Transaction {
    fn canonical_metadata(metadata: &HashMap<String, String>) -> BTreeMap<String, String> {
        metadata
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<BTreeMap<_, _>>()
    }

    fn canonical_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, TransactionError> {
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_little_endian()
            .serialize(value)
            .map_err(|e| {
                TransactionError::InvalidStructure(format!(
                    "canonical transaction serialization failed: {e}"
                ))
            })
    }

    fn id_payload(
        sender: String,
        receiver: String,
        amount: u128,
        fee: u128,
        gas_limit: u128,
        gas_used: u128,
        gas_price: u128,
        priority_fee: u128,
        inputs: Vec<Input>,
        outputs: Vec<Output>,
        metadata: HashMap<String, String>,
        timestamp: u64,
        transaction_kind: TransactionKind,
        chain_id: u32,
    ) -> TransactionIdPayload {
        TransactionIdPayload {
            sender,
            receiver,
            amount,
            fee,
            gas_limit,
            gas_used,
            gas_price,
            priority_fee,
            inputs,
            outputs,
            metadata: Self::canonical_metadata(&metadata),
            timestamp,
            transaction_kind,
            chain_id,
        }
    }

    fn derive_transaction_id(payload: &TransactionIdPayload) -> Result<String, TransactionError> {
        let bytes = Self::canonical_serialize(payload)?;
        Ok(hex::encode(qanto_hash(&bytes).as_bytes()))
    }

    fn signing_payload_from_id_payload(
        id: String,
        payload: TransactionIdPayload,
    ) -> TransactionSigningPayload {
        TransactionSigningPayload {
            id,
            sender: payload.sender,
            receiver: payload.receiver,
            amount: payload.amount,
            fee: payload.fee,
            gas_limit: payload.gas_limit,
            gas_used: payload.gas_used,
            gas_price: payload.gas_price,
            priority_fee: payload.priority_fee,
            inputs: payload.inputs,
            outputs: payload.outputs,
            metadata: payload.metadata,
            timestamp: payload.timestamp,
            transaction_kind: payload.transaction_kind,
            chain_id: payload.chain_id,
        }
    }

    pub(crate) fn signing_payload_for_transaction(&self) -> TransactionSigningPayload {
        let payload = Self::id_payload(
            self.sender.clone(),
            self.receiver.clone(),
            self.amount,
            self.fee,
            self.gas_limit,
            self.gas_used,
            self.gas_price,
            self.priority_fee,
            self.inputs.clone(),
            self.outputs.clone(),
            self.metadata.clone(),
            self.timestamp,
            self.transaction_kind,
            self.chain_id,
        );
        Self::signing_payload_from_id_payload(self.id.clone(), payload)
    }

    #[allow(dead_code)]
    pub(crate) fn rebuild_canonical_signature(
        &mut self,
        signing_key: &crate::qanto_native_crypto::QantoPQPrivateKey,
    ) -> Result<(), TransactionError> {
        let id_payload = Self::id_payload(
            self.sender.clone(),
            self.receiver.clone(),
            self.amount,
            self.fee,
            self.gas_limit,
            self.gas_used,
            self.gas_price,
            self.priority_fee,
            self.inputs.clone(),
            self.outputs.clone(),
            self.metadata.clone(),
            self.timestamp,
            self.transaction_kind,
            self.chain_id,
        );
        let tx_id = Self::derive_transaction_id(&id_payload)?;
        let signing_payload = Self::signing_payload_from_id_payload(tx_id.clone(), id_payload);
        let signature_data = Self::serialize_for_signing(&signing_payload)?;

        self.id = tx_id;
        self.signature = QuantumResistantSignature::sign(signing_key, &signature_data)
            .map_err(|_| TransactionError::InvalidTransactionSignature)?;
        Ok(())
    }

    /// Creates a new dummy transaction for testing purposes.
    pub fn new_dummy() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let dummy_id: [u8; 32] = rng.gen();
        let dummy_sender: [u8; 32] = rng.gen();
        let dummy_receiver: [u8; 32] = rng.gen();

        Transaction {
            id: hex::encode(dummy_id),
            sender: hex::encode(dummy_sender),
            receiver: hex::encode(dummy_receiver),
            amount: 100, // Dummy amount
            fee: 0,
            gas_limit: 50000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: vec![],
            outputs: vec![Output {
                address: "dummy_miner_address".to_string(),
                amount: 50_000_000_000, // Example mining reward (50 QAN)
                homomorphic_encrypted: HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            }],
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            metadata: HashMap::new(),
            signature: QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            fee_breakdown: None,
            transaction_kind: TransactionKind::Transfer,
            chain_id: 1234,
        }
    }

    /// Creates a new signed dummy transaction for testing purposes.
    pub fn new_dummy_signed(
        signing_key: &crate::qanto_native_crypto::QantoPQPrivateKey,
        _public_key: &crate::qanto_native_crypto::QantoPQPublicKey,
    ) -> Result<Self, TransactionError> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let dummy_sender: [u8; 32] = rng.gen();
        let dummy_receiver: [u8; 32] = rng.gen();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let sender_str = hex::encode(dummy_sender);
        let receiver_str = hex::encode(dummy_receiver);

        let id_payload = Self::id_payload(
            sender_str.clone(),
            receiver_str.clone(),
            100,
            0,
            50000,
            0,
            1,
            0,
            vec![],
            vec![],
            HashMap::new(),
            timestamp,
            TransactionKind::Transfer,
            1234,
        );
        let tx_id = Self::derive_transaction_id(&id_payload)?;
        let signing_payload = Self::signing_payload_from_id_payload(tx_id.clone(), id_payload);
        let signature_data = Self::serialize_for_signing(&signing_payload)?;

        let signature_obj = QuantumResistantSignature::sign(signing_key, &signature_data)
            .map_err(|_| TransactionError::InvalidTransactionSignature)?;

        Ok(Self {
            id: tx_id,
            sender: sender_str,
            receiver: receiver_str,
            amount: 100,
            fee: 0,
            gas_limit: 50000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: vec![],
            outputs: vec![Output {
                address: "dummy_miner_address".to_string(),
                amount: 50_000_000_000, // Example mining reward (50 QAN)
                homomorphic_encrypted: HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            }],
            timestamp,
            metadata: HashMap::new(),
            signature: signature_obj,
            fee_breakdown: None,
            transaction_kind: TransactionKind::Transfer,
            chain_id: 1234,
        })
    }

    /// Verifies the signature of the transaction (supports both post-quantum and EVM).
    pub fn verify_signature(
        &self,
        public_key: &crate::qanto_native_crypto::QantoPQPublicKey,
    ) -> Result<(), TransactionError> {
        let expected_chain_id = GLOBAL_CHAIN_ID.load(std::sync::atomic::Ordering::Relaxed) as u32;
        if self.chain_id != expected_chain_id {
            return Err(TransactionError::InvalidStructure(format!(
                "Chain ID mismatch: expected {}, got {}",
                expected_chain_id, self.chain_id
            )));
        }

        if self.id != self.compute_hash() {
            return Err(TransactionError::InvalidStructure(
                "transaction id does not match canonical payload".to_string(),
            ));
        }

        if self.signature.signer_public_key != public_key.as_bytes() {
            return Err(TransactionError::InvalidTransactionSignature);
        }

        if let Some(raw_tx_hex) = self.metadata.get("evm_raw_tx") {
            let raw_bytes = hex::decode(raw_tx_hex).map_err(|_| {
                TransactionError::InvalidStructure("Invalid evm_raw_tx hex".to_string())
            })?;
            let (recovered_sender, recovered_receiver, recovered_amount, _tx_data) =
                Self::recover_evm_sender(&raw_bytes).map_err(|e| {
                    TransactionError::InvalidStructure(format!("EVM recovery failed: {}", e))
                })?;

            if recovered_sender != self.sender {
                return Err(TransactionError::InvalidAddress);
            }
            if recovered_receiver != self.receiver {
                return Err(TransactionError::InvalidAddress);
            }
            if recovered_amount != self.amount {
                return Err(TransactionError::InvalidStructure(
                    "Amount mismatch".to_string(),
                ));
            }
            return Ok(());
        }

        let signature_data = Self::serialize_for_signing(&self.signing_payload_for_transaction())?;

        if !QuantumResistantSignature::verify(&self.signature, &signature_data) {
            return Err(TransactionError::InvalidTransactionSignature);
        }
        Ok(())
    }

    /// Creates a new transaction with the given configuration.
    pub async fn new(
        config: TransactionConfig,
        signing_key: &crate::qanto_native_crypto::QantoPQPrivateKey,
    ) -> Result<Self, TransactionError> {
        Self::validate_structure_pre_creation(
            config.sender.clone(),
            config.receiver.clone(),
            config.amount,
            config.inputs.clone(),
            config.metadata.clone(),
            config.fee,
            config.gas_limit,
        )?;
        Self::check_rate_limit(&config.tx_timestamps, MAX_TRANSACTIONS_PER_MINUTE).await?;
        Self::validate_addresses(
            config.sender.clone(),
            config.receiver.clone(),
            config.outputs.clone(),
        )?;
        let timestamp = Self::get_current_timestamp()?;
        let metadata = config.metadata.unwrap_or_default();
        let id_payload = Self::id_payload(
            config.sender.clone(),
            config.receiver.clone(),
            config.amount,
            config.fee,
            config.gas_limit,
            0,
            config.gas_price,
            config.priority_fee,
            config.inputs.clone(),
            config.outputs.clone(),
            metadata.clone(),
            timestamp,
            TransactionKind::Transfer,
            config.chain_id,
        );
        let tx_id = Self::derive_transaction_id(&id_payload)?;
        let signing_payload = Self::signing_payload_from_id_payload(tx_id.clone(), id_payload);
        let signature_data = Self::serialize_for_signing(&signing_payload)?;
        let hash_bytes = qanto_hash(&signature_data).as_bytes().to_vec();
        let action_hash: [u8; 32] = hash_bytes[..32.min(hash_bytes.len())]
            .try_into()
            .map_err(|_| TransactionError::InvalidStructure("Hash length mismatch".to_string()))?;

        #[cfg(test)]
        let result =
            omega::reflect_on_action_for_testing(crate::qanto_compat::sp_core::H256(action_hash))
                .await;
        #[cfg(not(test))]
        let result =
            omega::reflect_on_action(crate::qanto_compat::sp_core::H256(action_hash)).await;

        if !result {
            return Err(TransactionError::OmegaRejection);
        }

        let tx = Self {
            id: tx_id,
            sender: config.sender,
            receiver: config.receiver,
            amount: config.amount,
            fee: config.fee,
            gas_limit: config.gas_limit,
            gas_used: 0, // Will be set during execution
            gas_price: config.gas_price,
            priority_fee: config.priority_fee,
            inputs: config.inputs,
            outputs: config.outputs,
            timestamp,
            metadata,
            signature: QuantumResistantSignature::sign(signing_key, &signature_data)
                .map_err(|_| TransactionError::InvalidTransactionSignature)?,
            fee_breakdown: None, // Will be calculated by gas fee model
            transaction_kind: TransactionKind::Transfer,
            chain_id: config.chain_id,
        };
        let mut timestamps_guard = config.tx_timestamps.write().await;
        timestamps_guard.insert(tx.id.clone(), timestamp);
        if timestamps_guard.len() > (MAX_TRANSACTIONS_PER_MINUTE * 2) as usize {
            let current_time = Self::get_current_timestamp().unwrap_or(0);
            timestamps_guard
                .retain(|_, &mut stored_ts| current_time.saturating_sub(stored_ts) < 3600);
        }
        Ok(tx)
    }

    /// Creates a new coinbase transaction.
    pub(crate) fn new_coinbase(
        receiver: String,
        _reward: u128,
        outputs: Vec<Output>,
        signing_key: &crate::qanto_native_crypto::QantoPQPrivateKey,
        chain_id: u32,
    ) -> Result<Self, TransactionError> {
        let sender = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let timestamp = Self::get_current_timestamp()?;
        let metadata = HashMap::new();

        // Calculate the actual amount as sum of outputs
        let actual_amount = outputs.iter().map(|output| output.amount).sum::<u128>();

        let id_payload = Self::id_payload(
            sender.clone(),
            receiver.clone(),
            actual_amount,
            0,
            0,
            0,
            0,
            0,
            vec![],
            outputs.clone(),
            metadata.clone(),
            timestamp,
            TransactionKind::Transfer,
            chain_id,
        );
        let tx_id = Self::derive_transaction_id(&id_payload)?;
        let signing_payload = Self::signing_payload_from_id_payload(tx_id.clone(), id_payload);
        let signature_data = Self::serialize_for_signing(&signing_payload)?;

        let signature_obj = QuantumResistantSignature::sign(signing_key, &signature_data)
            .map_err(|_| TransactionError::InvalidTransactionSignature)?;

        let tx = Self {
            id: tx_id,
            sender,
            receiver,
            amount: actual_amount,
            fee: 0,
            gas_limit: 0, // Coinbase transactions don't use gas
            gas_used: 0,
            gas_price: 0,
            priority_fee: 0,
            inputs: vec![],
            outputs,
            timestamp,
            metadata,
            signature: signature_obj,
            fee_breakdown: None,
            transaction_kind: TransactionKind::Transfer,
            chain_id,
        };
        Ok(tx)
    }

    /// Checks if the transaction is a coinbase transaction.
    pub fn is_coinbase(&self) -> bool {
        self.inputs.is_empty()
    }

    /// Returns a reference to the transaction's metadata.
    pub fn get_metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Validates the basic structure of a transaction before creation.
    fn validate_structure_pre_creation(
        sender: String,
        receiver: String,
        amount: u128,
        inputs: Vec<Input>,
        metadata: Option<HashMap<String, String>>,
        fee: u128,
        gas_limit: u128,
    ) -> Result<(), TransactionError> {
        if sender.is_empty() {
            return Err(TransactionError::InvalidStructure(
                "Sender cannot be empty".to_string(),
            ));
        }
        if receiver.is_empty() {
            return Err(TransactionError::InvalidStructure(
                "Receiver cannot be empty".to_string(),
            ));
        }
        if amount == 0 && !inputs.is_empty() {
            return Err(TransactionError::InvalidStructure(
                "Amount cannot be zero for regular transactions".to_string(),
            ));
        }

        // --- CRITICAL: Enforce Min Fee and Gas to block dust attacks (C-H1) ---
        if fee < MIN_TX_FEE {
            return Err(TransactionError::InvalidStructure(format!(
                "Transaction fee {} is below minimum required fee {}",
                fee, MIN_TX_FEE
            )));
        }
        if gas_limit < MIN_TX_GAS {
            return Err(TransactionError::InvalidStructure(format!(
                "Gas limit {} is below minimum required gas {}",
                gas_limit, MIN_TX_GAS
            )));
        }

        if let Some(md) = metadata {
            if md.len() > MAX_METADATA_PAIRS {
                let mut error_msg = String::with_capacity(50);
                error_msg.push_str("Exceeded max metadata pairs limit of ");
                error_msg.push_str(&MAX_METADATA_PAIRS.to_string());
                return Err(TransactionError::InvalidMetadata(error_msg));
            }
            for (k, v) in md {
                if k.len() > MAX_METADATA_KEY_LEN || v.len() > MAX_METADATA_VALUE_LEN {
                    return Err(TransactionError::InvalidMetadata(
                        "Metadata key/value length exceeded".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Checks if the transaction rate limit has been exceeded.
    async fn check_rate_limit(
        tx_timestamps: &Arc<RwLock<HashMap<String, u64>>>,
        max_txs: u64,
    ) -> Result<(), TransactionError> {
        let now = Self::get_current_timestamp()?;
        let timestamps_guard = tx_timestamps.read().await;
        let recent_tx_count = timestamps_guard
            .values()
            .filter(|&&t| now.saturating_sub(t) < 60)
            .count() as u64;
        if recent_tx_count >= max_txs {
            return Err(TransactionError::RateLimitExceeded);
        }
        Ok(())
    }

    /// Validates the format of the sender, receiver, and output addresses.
    fn validate_addresses(
        sender: String,
        receiver: String,
        outputs: Vec<Output>,
    ) -> Result<(), TransactionError> {
        if !Self::is_valid_address(sender)
            || !Self::is_valid_address(receiver)
            || outputs
                .iter()
                .any(|o| !Self::is_valid_address(o.address.clone()))
        {
            Err(TransactionError::InvalidAddress)
        } else {
            Ok(())
        }
    }

    /// Checks if a given address string is valid.
    fn is_valid_address(address: String) -> bool {
        address.len() == 64 && hex::decode(address).is_ok()
    }

    /// Returns the current Unix timestamp.
    fn get_current_timestamp() -> Result<u64, TransactionError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|_| TransactionError::TimestampError)
    }

    /// Serializes the transaction data for signing.
    pub(crate) fn serialize_for_signing(
        payload: &TransactionSigningPayload,
    ) -> Result<Vec<u8>, TransactionError> {
        Self::canonical_serialize(payload)
    }

    /// Computes the hash of the transaction.
    pub(crate) fn compute_hash(&self) -> String {
        let payload = Self::id_payload(
            self.sender.clone(),
            self.receiver.clone(),
            self.amount,
            self.fee,
            self.gas_limit,
            self.gas_used,
            self.gas_price,
            self.priority_fee,
            self.inputs.clone(),
            self.outputs.clone(),
            self.metadata.clone(),
            self.timestamp,
            self.transaction_kind,
            self.chain_id,
        );
        Self::derive_transaction_id(&payload).unwrap_or_default()
    }

    /// Memory-optimized verification using a shared UTXO reference.
    pub async fn verify_with_shared_utxos(
        &self,
        _dag: &QantoDAG,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), TransactionError> {
        let public_key = crate::qanto_native_crypto::QantoPQPublicKey::from_bytes(
            &self.signature.signer_public_key,
        )
        .map_err(|_| TransactionError::InvalidTransactionSignature)?;
        self.verify_signature(&public_key)?;

        if self.is_coinbase() {
            if self.fee != 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase fee must be 0".to_string(),
                ));
            }
            let total_output: u128 = self.outputs.iter().map(|o| o.amount).sum();
            if total_output == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase output cannot be zero".to_string(),
                ));
            }
        } else {
            // MEMORY OPTIMIZATION: Only read the UTXO set once and validate all inputs.
            let utxos_guard = utxos_arc.read().await;
            let mut total_input_value = 0u128;

            for input in &self.inputs {
                let mut utxo_id = String::with_capacity(input.tx_id.len() + 10);
                utxo_id.push_str(&input.tx_id);
                utxo_id.push('_');
                utxo_id.push_str(&input.output_index.to_string());

                let utxo = utxos_guard.get(&utxo_id).ok_or_else(|| {
                    TransactionError::InvalidStructure(format!("UTXO not found: {utxo_id}"))
                })?;

                if utxo.address != self.sender {
                    return Err(TransactionError::InvalidStructure(
                        "UTXO address does not match sender".to_string(),
                    ));
                }

                total_input_value += utxo.amount;
            }

            let total_output_value: u128 = self.outputs.iter().map(|o| o.amount).sum();
            let expected_total = total_output_value + self.fee;

            if total_input_value != expected_total {
                return Err(TransactionError::InvalidStructure(format!(
                    "Input/output value mismatch: {total_input_value} != {expected_total}"
                )));
            }
        }

        Ok(())
    }

    /// Verifies the transaction against a given set of UTXOs.
    pub async fn verify(
        &self,
        _dag: &QantoDAG,
        utxos: &HashMap<String, UTXO>,
    ) -> Result<(), TransactionError> {
        let public_key = crate::qanto_native_crypto::QantoPQPublicKey::from_bytes(
            &self.signature.signer_public_key,
        )
        .map_err(|_| TransactionError::InvalidTransactionSignature)?;
        self.verify_signature(&public_key)?;

        if self.is_coinbase() {
            if self.fee != 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase fee must be 0".to_string(),
                ));
            }
            let total_output: u128 = self.outputs.iter().map(|o| o.amount).sum();
            if total_output == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase output cannot be zero".to_string(),
                ));
            }
        } else {
            let mut total_input_value = 0u128;
            for input in &self.inputs {
                let mut utxo_id = String::with_capacity(input.tx_id.len() + 10);
                utxo_id.push_str(&input.tx_id);
                utxo_id.push('_');
                utxo_id.push_str(&input.output_index.to_string());

                let utxo = utxos.get(&utxo_id).ok_or_else(|| {
                    let mut error_msg = String::with_capacity(15 + utxo_id.len());
                    error_msg.push_str("UTXO ");
                    error_msg.push_str(&utxo_id);
                    error_msg.push_str(" not found");
                    TransactionError::InvalidStructure(error_msg)
                })?;
                if utxo.address != self.sender {
                    let mut error_msg = String::with_capacity(45 + utxo_id.len());
                    error_msg.push_str("Input UTXO ");
                    error_msg.push_str(&utxo_id);
                    error_msg.push_str(" does not belong to sender");
                    return Err(TransactionError::InvalidStructure(error_msg));
                }
                total_input_value += utxo.amount;
            }
            let total_output_value: u128 = self.outputs.iter().map(|o| o.amount).sum();

            if total_input_value < total_output_value + self.fee {
                return Err(TransactionError::InsufficientFunds);
            }
        }
        Ok(())
    }

    /// Optimized batch verification for multiple transactions using parallel processing.
    pub fn verify_batch_parallel(
        transactions: &[Transaction],
        utxos_map: &HashMap<String, UTXO>,
        _verification_semaphore: &Arc<Semaphore>,
    ) -> Vec<Result<(), TransactionError>> {
        // Chunk the transactions to process in batches to reduce contention
        let chunk_size = transactions.len() / rayon::current_num_threads() + 1;
        transactions
            .par_chunks(chunk_size)
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .map(|tx| Self::verify_single_transaction(tx, utxos_map))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// A single transaction verification helper for batch processing.
    pub fn verify_single_transaction(
        tx: &Transaction,
        utxos: &HashMap<String, UTXO>,
    ) -> Result<(), TransactionError> {
        let public_key = crate::qanto_native_crypto::QantoPQPublicKey::from_bytes(
            &tx.signature.signer_public_key,
        )
        .map_err(|_| TransactionError::InvalidTransactionSignature)?;
        tx.verify_signature(&public_key)?;

        // --- CRITICAL: Enforce Min Fee and Gas in validation path (C-H1) ---
        if !tx.is_coinbase() {
            if tx.fee < MIN_TX_FEE {
                return Err(TransactionError::InvalidStructure(format!(
                    "Transaction fee {} is below minimum required fee {}",
                    tx.fee, MIN_TX_FEE
                )));
            }
            if tx.gas_limit < MIN_TX_GAS {
                return Err(TransactionError::InvalidStructure(format!(
                    "Gas limit {} is below minimum required gas {}",
                    tx.gas_limit, MIN_TX_GAS
                )));
            }
        }

        if tx.is_coinbase() {
            if tx.fee != 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase fee must be 0".to_string(),
                ));
            }
            let total_output: u128 = tx.outputs.iter().map(|o| o.amount).sum();
            if total_output == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase output cannot be zero".to_string(),
                ));
            }
        } else {
            let mut total_input_value = 0u128;
            for input in &tx.inputs {
                let mut utxo_id = String::with_capacity(input.tx_id.len() + 10);
                utxo_id.push_str(&input.tx_id);
                utxo_id.push('_');
                utxo_id.push_str(&input.output_index.to_string());

                let utxo = utxos.get(&utxo_id).ok_or_else(|| {
                    let mut error_msg = String::with_capacity(15 + utxo_id.len());
                    error_msg.push_str("UTXO ");
                    error_msg.push_str(&utxo_id);
                    error_msg.push_str(" not found");
                    TransactionError::InvalidStructure(error_msg)
                })?;
                if utxo.address != tx.sender {
                    let mut error_msg = String::with_capacity(45 + utxo_id.len());
                    error_msg.push_str("Input UTXO ");
                    error_msg.push_str(&utxo_id);
                    error_msg.push_str(" does not belong to sender");
                    return Err(TransactionError::InvalidStructure(error_msg));
                }
                total_input_value += utxo.amount;
            }
            let total_output_value: u128 = tx.outputs.iter().map(|o| o.amount).sum();

            if total_input_value < total_output_value + tx.fee {
                return Err(TransactionError::InsufficientFunds);
            }
        }
        Ok(())
    }

    /// Lightweight transaction validation for mempool admission.
    pub fn validate_for_mempool(&self) -> Result<(), TransactionError> {
        // DoS Protection: size limits check
        let serialized_len = serde_json::to_vec(self)
            .map_err(TransactionError::Serialization)?
            .len();
        if serialized_len > 128 * 1024 {
            return Err(TransactionError::InvalidStructure(format!(
                "Transaction size exceeds limit of 128KB: got {}",
                serialized_len
            )));
        }

        if let Some(proof) = self.metadata.get("bridge_merkle_proof") {
            if proof.len() > 64 * 1024 {
                return Err(TransactionError::InvalidStructure(format!(
                    "Bridge Merkle proof size exceeds limit of 64KB: got {}",
                    proof.len()
                )));
            }
        }
        if let Some(proof) = self.metadata.get("bridge_receipt_proof") {
            if proof.len() > 64 * 1024 {
                return Err(TransactionError::InvalidStructure(format!(
                    "Bridge receipt proof size exceeds limit of 64KB: got {}",
                    proof.len()
                )));
            }
        }

        if let Some(desc) = self.metadata.get("proposal_description") {
            if desc.len() > 16 * 1024 {
                return Err(TransactionError::InvalidStructure(format!(
                    "Proposal description size exceeds limit of 16KB: got {}",
                    desc.len()
                )));
            }
        }

        if let Some(cid) = self.metadata.get("proposal_cid") {
            if cid.len() > 256 {
                return Err(TransactionError::InvalidStructure(format!(
                    "Proposal CID size exceeds limit of 256 bytes: got {}",
                    cid.len()
                )));
            }
        }

        // Basic structure validation
        if self.id.is_empty() {
            return Err(TransactionError::InvalidStructure(
                "Transaction ID cannot be empty".to_string(),
            ));
        }

        // Check for empty inputs and outputs (except coinbase)
        if self.inputs.is_empty() && self.outputs.is_empty() {
            return Err(TransactionError::InvalidStructure(
                "Transaction cannot have both empty inputs and outputs".to_string(),
            ));
        }

        // Validate that the fee is reasonable (not zero for non-coinbase)
        if !self.inputs.is_empty() && self.fee == 0 {
            let network_load =
                crate::saga_ai::NETWORK_THROTTLE_BPS.load(std::sync::atomic::Ordering::Relaxed);
            if !crate::saga_ai::authorize_gasless_transaction(&self.sender, network_load) {
                return Err(TransactionError::InvalidStructure(
                    "Non-coinbase transaction must have a fee".to_string(),
                ));
            }
        }

        // Check for duplicate inputs
        let mut input_set = HashSet::new();
        for input in &self.inputs {
            let mut input_key = String::with_capacity(input.tx_id.len() + 10);
            input_key.push_str(&input.tx_id);
            input_key.push(':');
            input_key.push_str(&input.output_index.to_string());
            if !input_set.insert(input_key) {
                return Err(TransactionError::InvalidStructure(
                    "Duplicate input detected".to_string(),
                ));
            }
        }

        // Validate that output amounts are positive
        for output in &self.outputs {
            if output.amount == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Output amount cannot be zero".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Fast hash computation for transaction identification.
    pub fn compute_fast_hash(&self) -> String {
        let mut data = Vec::new();

        // Hash only essential fields for speed.
        data.extend_from_slice(self.id.as_bytes());
        data.extend_from_slice(&self.fee.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());

        // Hash inputs and outputs in parallel.
        let input_hashes: Vec<_> = self
            .inputs
            .par_iter()
            .map(|input| {
                let mut input_data = Vec::new();
                input_data.extend_from_slice(input.tx_id.as_bytes());
                input_data.extend_from_slice(&input.output_index.to_le_bytes());
                qanto_hash(&input_data).as_bytes().to_vec()
            })
            .collect();

        let output_hashes: Vec<_> = self
            .outputs
            .par_iter()
            .map(|output| {
                let mut output_data = Vec::new();
                output_data.extend_from_slice(output.address.as_bytes());
                output_data.extend_from_slice(&output.amount.to_le_bytes());
                qanto_hash(&output_data).as_bytes().to_vec()
            })
            .collect();

        for hash in input_hashes {
            data.extend_from_slice(&hash[..]);
        }
        for hash in output_hashes {
            data.extend_from_slice(&hash[..]);
        }

        let final_hash = qanto_hash(&data);
        hex::encode(final_hash.as_bytes())
    }

    /// Generates a UTXO from a transaction output.
    pub fn generate_utxo(&self, index: u32) -> UTXO {
        let output = &self.outputs[index as usize];
        let mut utxo_id = String::with_capacity(self.id.len() + 12); // tx_id + "_" + index (up to 10 digits)
        utxo_id.push_str(&self.id);
        utxo_id.push('_');
        utxo_id.push_str(&index.to_string());

        // Use a local explorer instead of an external service.
        let mut explorer_link = String::with_capacity(22 + utxo_id.len()); // base URL + utxo_id
        explorer_link.push_str("/explorer/utxo/");
        explorer_link.push_str(&utxo_id);

        UTXO {
            address: output.address.clone(),
            amount: output.amount,
            tx_id: self.id.clone(),
            output_index: index,
            explorer_link,
        }
    }

    /// Calculates and sets the fee breakdown using the gas fee model
    pub async fn calculate_gas_fee(
        &mut self,
        gas_fee_model: &GasFeeModel,
    ) -> Result<(), TransactionError> {
        let storage_duration = StorageDuration::ShortTerm; // Default to short-term storage

        let fee_breakdown = gas_fee_model
            .calculate_transaction_fee(self, self.gas_limit, self.priority_fee, storage_duration)
            .await?;

        self.fee = fee_breakdown.total_fee;
        self.fee_breakdown = Some(fee_breakdown);
        Ok(())
    }

    /// Updates gas usage after transaction execution
    pub fn update_gas_usage(&mut self, gas_used: u128) -> Result<(), TransactionError> {
        if gas_used > self.gas_limit {
            return Err(TransactionError::GasFee(
                crate::gas_fee_model::GasFeeError::GasLimitExceeded {
                    used: gas_used,
                    limit: self.gas_limit,
                },
            ));
        }
        self.gas_used = gas_used;
        Ok(())
    }

    /// Validates gas parameters before transaction execution
    pub fn validate_gas_parameters(&self) -> Result<(), TransactionError> {
        if self.gas_limit == 0 && !self.is_coinbase() {
            return Err(TransactionError::InvalidStructure(
                "Gas limit cannot be zero for non-coinbase transactions".to_string(),
            ));
        }

        if self.gas_price == 0 && !self.is_coinbase() {
            let network_load =
                crate::saga_ai::NETWORK_THROTTLE_BPS.load(std::sync::atomic::Ordering::Relaxed);
            if !crate::saga_ai::authorize_gasless_transaction(&self.sender, network_load) {
                return Err(TransactionError::InvalidStructure(
                    "Gas price cannot be zero for non-coinbase transactions".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Gets the effective fee rate (fee per gas unit)
    pub fn get_effective_fee_rate(&self) -> u128 {
        if self.gas_used == 0 {
            0
        } else {
            (self.fee * crate::Q_SCALE) / self.gas_used
        }
    }
}

impl Zeroize for Transaction {
    fn zeroize(&mut self) {
        self.sender.zeroize();
        self.receiver.zeroize();
        self.amount = 0;
        self.fee = 0;
        self.inputs.clear();
        self.outputs.clear();
        self.timestamp = 0;
        self.metadata.clear();
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        self.zeroize();
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
pub static GLOBAL_CHAIN_ID: AtomicU64 = AtomicU64::new(1234);

pub fn pad_ethereum_address(eth_addr_hex: &str) -> String {
    let mut cleaned = eth_addr_hex.trim_start_matches("0x").to_string();
    if cleaned.len() < 64 {
        cleaned = format!("{:0<64}", cleaned);
    } else if cleaned.len() > 64 {
        cleaned = cleaned[..64].to_string();
    }
    cleaned.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance_optimizations::QantoDAGOptimizations;
    use crate::post_quantum_crypto::generate_pq_keypair;
    use ahash::AHashMap as HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn sample_output() -> Output {
        Output {
            address: "b".repeat(64),
            amount: 42,
            homomorphic_encrypted: HomomorphicEncrypted::default(),
        }
    }

    #[test]
    fn canonical_serialization_is_deterministic_across_metadata_order() {
        let mut metadata_a = HashMap::new();
        metadata_a.insert("z".to_string(), "last".to_string());
        metadata_a.insert("a".to_string(), "first".to_string());

        let mut metadata_b = HashMap::new();
        metadata_b.insert("a".to_string(), "first".to_string());
        metadata_b.insert("z".to_string(), "last".to_string());

        let payload_a = Transaction::id_payload(
            "a".repeat(64),
            "b".repeat(64),
            42,
            0,
            50_000,
            0,
            1,
            0,
            vec![],
            vec![sample_output()],
            metadata_a,
            123456789,
            TransactionKind::Transfer,
            1234,
        );
        let payload_b = Transaction::id_payload(
            "a".repeat(64),
            "b".repeat(64),
            42,
            0,
            50_000,
            0,
            1,
            0,
            vec![],
            vec![sample_output()],
            metadata_b,
            123456789,
            TransactionKind::Transfer,
            1234,
        );

        let id_a = Transaction::derive_transaction_id(&payload_a).expect("id a");
        let id_b = Transaction::derive_transaction_id(&payload_b).expect("id b");
        let signing_a = Transaction::signing_payload_from_id_payload(id_a.clone(), payload_a);
        let signing_b = Transaction::signing_payload_from_id_payload(id_b.clone(), payload_b);

        let bytes_a = Transaction::serialize_for_signing(&signing_a).expect("bytes a");
        let bytes_b = Transaction::serialize_for_signing(&signing_b).expect("bytes b");

        assert_eq!(id_a, id_b);
        assert_eq!(bytes_a, bytes_b);
    }

    #[tokio::test]
    async fn verify_paths_fail_fast_on_invalid_signature() {
        let dag = QantoDAG::new_dummy_for_verification();
        let (_pk, sk) = generate_pq_keypair(None).expect("keypair");
        let mut tx = Transaction::new_dummy_signed(&sk, &sk.public_key()).expect("signed tx");
        tx.signature.signature[0] ^= 0x01;

        let shared_utxos = Arc::new(RwLock::new(HashMap::new()));
        let direct_utxos = HashMap::new();

        let shared_err = tx
            .verify_with_shared_utxos(&dag, &shared_utxos)
            .await
            .expect_err("shared verification should fail");
        let direct_err = tx
            .verify(&dag, &direct_utxos)
            .await
            .expect_err("direct verification should fail");

        assert!(matches!(
            shared_err,
            TransactionError::InvalidTransactionSignature
        ));
        assert!(matches!(
            direct_err,
            TransactionError::InvalidTransactionSignature
        ));
    }
}

impl Transaction {
    pub fn verify_signatures_batch_parallel(transactions: &[Transaction]) -> Vec<bool> {
        let expected_chain_id = GLOBAL_CHAIN_ID.load(Ordering::Relaxed) as u32;
        use rayon::prelude::*;
        transactions
            .par_iter()
            .map(|tx| {
                if tx.chain_id != expected_chain_id {
                    return false;
                }
                if let Some(raw_tx_hex) = tx.metadata.get("evm_raw_tx") {
                    if let Ok(raw_bytes) = hex::decode(raw_tx_hex) {
                        if let Ok((
                            recovered_sender,
                            recovered_receiver,
                            recovered_amount,
                            _tx_data,
                        )) = Self::recover_evm_sender(&raw_bytes)
                        {
                            return recovered_sender == tx.sender
                                && recovered_receiver == tx.receiver
                                && recovered_amount == tx.amount;
                        }
                    }
                    return false;
                }

                match Self::serialize_for_signing(&tx.signing_payload_for_transaction()) {
                    Ok(bytes) => QuantumResistantSignature::verify(&tx.signature, &bytes),
                    Err(_) => false,
                }
            })
            .collect()
    }

    pub fn recover_evm_sender(
        raw_tx_bytes: &[u8],
    ) -> Result<(String, String, u128, Vec<u8>), String> {
        if raw_tx_bytes.is_empty() {
            return Err("Empty transaction bytes".to_string());
        }

        let expected_chain_id = GLOBAL_CHAIN_ID.load(Ordering::Relaxed);

        let mut tx_to_hash: Vec<u8>;
        let rec_id: u8;
        let r_bytes: Vec<u8>;
        let s_bytes: Vec<u8>;
        let to_addr_bytes: Vec<u8>;
        let transfer_value: u128;
        let tx_data: Vec<u8>;

        if raw_tx_bytes[0] == 2 {
            // EIP-1559 (Type 2) transaction
            let payload = &raw_tx_bytes[1..];
            let rlp = rlp::Rlp::new(payload);
            if !rlp.is_list() {
                return Err("Invalid RLP payload for EIP-1559".to_string());
            }
            let count = rlp.item_count().unwrap_or(0);
            if count < 12 {
                return Err(format!(
                    "Insufficient items in EIP-1559 RLP: got {}, expected at least 12",
                    count
                ));
            }

            let chain_id: u64 = rlp.val_at(0).map_err(|e| e.to_string())?;
            if chain_id != expected_chain_id {
                return Err(format!(
                    "Chain ID mismatch: expected {}, got {}",
                    expected_chain_id, chain_id
                ));
            }

            let nonce: u64 = rlp.val_at(1).map_err(|e| e.to_string())?;
            let max_priority_fee: Vec<u8> = rlp.val_at(2).map_err(|e| e.to_string())?;
            let max_fee: Vec<u8> = rlp.val_at(3).map_err(|e| e.to_string())?;
            let gas_limit: Vec<u8> = rlp.val_at(4).map_err(|e| e.to_string())?;
            to_addr_bytes = rlp.val_at::<Vec<u8>>(5).map_err(|e| e.to_string())?;
            transfer_value = rlp.val_at::<u128>(6).map_err(|e| e.to_string())?;
            let data: Vec<u8> = rlp.val_at(7).map_err(|e| e.to_string())?;
            tx_data = data.clone();
            let access_list: rlp::Rlp = rlp.at(8).map_err(|e| e.to_string())?;
            let y_parity: u8 = rlp.val_at(9).map_err(|e| e.to_string())?;
            r_bytes = rlp.val_at(10).map_err(|e| e.to_string())?;
            s_bytes = rlp.val_at(11).map_err(|e| e.to_string())?;

            // Reconstruct signing payload
            let mut stream = rlp::RlpStream::new_list(9);
            stream.append(&chain_id);
            stream.append(&nonce);
            stream.append(&max_priority_fee);
            stream.append(&max_fee);
            stream.append(&gas_limit);
            stream.append(&to_addr_bytes);
            stream.append(&transfer_value);
            stream.append(&data);
            stream.append_raw(access_list.as_raw(), 1);

            tx_to_hash = vec![2u8];
            tx_to_hash.extend_from_slice(&stream.out());
            rec_id = y_parity;
        } else if raw_tx_bytes[0] >= 0xc0 {
            // Legacy transaction
            let rlp = rlp::Rlp::new(raw_tx_bytes);
            if !rlp.is_list() {
                return Err("Invalid RLP payload for Legacy transaction".to_string());
            }
            let count = rlp.item_count().unwrap_or(0);
            if count < 9 {
                return Err(format!(
                    "Insufficient items in Legacy RLP: got {}, expected at least 9",
                    count
                ));
            }

            let nonce: u64 = rlp.val_at(0).map_err(|e| e.to_string())?;
            let gas_price: Vec<u8> = rlp.val_at(1).map_err(|e| e.to_string())?;
            let gas_limit: Vec<u8> = rlp.val_at(2).map_err(|e| e.to_string())?;
            to_addr_bytes = rlp.val_at::<Vec<u8>>(3).map_err(|e| e.to_string())?;
            transfer_value = rlp.val_at::<u128>(4).map_err(|e| e.to_string())?;
            let data: Vec<u8> = rlp.val_at(5).map_err(|e| e.to_string())?;
            tx_data = data.clone();
            let v: u64 = rlp.val_at(6).map_err(|e| e.to_string())?;
            r_bytes = rlp.val_at(7).map_err(|e| e.to_string())?;
            s_bytes = rlp.val_at(8).map_err(|e| e.to_string())?;

            if v >= 37 {
                // EIP-155 replay protection
                let chain_id = (v - 35) / 2;
                if chain_id != expected_chain_id {
                    return Err(format!(
                        "Chain ID mismatch: expected {}, got {}",
                        expected_chain_id, chain_id
                    ));
                }
                rec_id = ((v - 35) % 2) as u8;

                // Reconstruct signing payload: [nonce, gas_price, gas_limit, to, value, data, chain_id, 0, 0]
                let mut stream = rlp::RlpStream::new_list(9);
                stream.append(&nonce);
                stream.append(&gas_price);
                stream.append(&gas_limit);
                stream.append(&to_addr_bytes);
                stream.append(&transfer_value);
                stream.append(&data);
                stream.append(&chain_id);
                stream.append(&0u8);
                stream.append(&0u8);
                tx_to_hash = stream.out().to_vec();
            } else if v == 27 || v == 28 {
                // No EIP-155 replay protection
                rec_id = (v - 27) as u8;

                // Reconstruct signing payload: [nonce, gas_price, gas_limit, to, value, data]
                let mut stream = rlp::RlpStream::new_list(6);
                stream.append(&nonce);
                stream.append(&gas_price);
                stream.append(&gas_limit);
                stream.append(&to_addr_bytes);
                stream.append(&transfer_value);
                stream.append(&data);
                tx_to_hash = stream.out().to_vec();
            } else {
                return Err(format!("Unsupported v value for Legacy transaction: {}", v));
            }
        } else {
            return Err(format!(
                "Unknown transaction type header: 0x{:02x}",
                raw_tx_bytes[0]
            ));
        }

        // Keccak256 hash of tx_to_hash
        let mut hasher = Keccak256::new();
        hasher.update(&tx_to_hash);
        let msg_hash = hasher.finalize();

        // Standardize r and s to 32 bytes
        let mut r_32 = [0u8; 32];
        let mut s_32 = [0u8; 32];
        if r_bytes.len() <= 32 {
            r_32[32 - r_bytes.len()..].copy_from_slice(&r_bytes);
        } else {
            return Err("Invalid r length".to_string());
        }
        if s_bytes.len() <= 32 {
            s_32[32 - s_bytes.len()..].copy_from_slice(&s_bytes);
        } else {
            return Err("Invalid s length".to_string());
        }

        // Canonical Low-S check to prevent malleability (EIP-2)
        let half_order = [
            0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46,
            0x68, 0x1b, 0x20, 0xa0,
        ];
        if s_32 > half_order {
            return Err("Non-canonical signature (high-S value rejected)".to_string());
        }

        // Recover public key
        let mut sig_bytes = [0u8; 64];
        sig_bytes[..32].copy_from_slice(&r_32);
        sig_bytes[32..].copy_from_slice(&s_32);

        let signature = K256Signature::from_slice(&sig_bytes).map_err(|e| e.to_string())?;
        let recovery_id = RecoveryId::try_from(rec_id).map_err(|e| e.to_string())?;

        let recovered_key =
            VerifyingKey::recover_from_prehash(msg_hash.as_ref(), &signature, recovery_id)
                .map_err(|e| e.to_string())?;

        // SEC1 compression verification
        let binding = recovered_key.to_encoded_point(false);
        let pub_bytes = binding.as_bytes();
        if pub_bytes.len() != 65 || pub_bytes[0] != 4 {
            return Err("Failed to obtain uncompressed public key".to_string());
        }

        // Keccak256 skip first byte
        let mut pub_hasher = Keccak256::new();
        pub_hasher.update(&pub_bytes[1..]);
        let pub_hash = pub_hasher.finalize();

        let eth_addr = &pub_hash[12..];
        let eth_addr_hex = format!("0x{}", hex::encode(eth_addr));

        let receiver_addr_hex = if to_addr_bytes.is_empty() {
            "0x0000000000000000000000000000000000000000".to_string()
        } else {
            format!("0x{}", hex::encode(&to_addr_bytes))
        };

        let padded_sender = pad_ethereum_address(&eth_addr_hex);
        let padded_receiver = pad_ethereum_address(&receiver_addr_hex);

        Ok((padded_sender, padded_receiver, transfer_value, tx_data))
    }
}
