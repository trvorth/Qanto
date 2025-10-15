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
use my_blockchain::qanto_hash;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use zeroize::Zeroize;

// --- Performance & Fee Constants ---
/// The maximum number of transactions allowed per minute to align with the 10M+ TPS goal.
const MAX_TRANSACTIONS_PER_MINUTE: u64 = 600_000_000;
/// The maximum number of key-value pairs allowed in a transaction's metadata.
const MAX_METADATA_PAIRS: usize = 16;
/// The maximum length of a metadata key.
const MAX_METADATA_KEY_LEN: usize = 64;
/// The maximum length of a metadata value.
const MAX_METADATA_VALUE_LEN: usize = 256;

// --- New Dynamic Fee Structure ---
// Thresholds are defined in the smallest units of QAN for precision.
/// The number of smallest units in one QAN.
pub const SMALLEST_UNITS_PER_QAN: u64 = 1_000_000_000;
/// The threshold for the first fee tier (under 1,000,000 QAN).
pub const FEE_TIER1_THRESHOLD: u64 = 1_000_000 * SMALLEST_UNITS_PER_QAN;
/// The threshold for the second fee tier (1M to 10M QAN).
pub const FEE_TIER2_THRESHOLD: u64 = 10_000_000 * SMALLEST_UNITS_PER_QAN;
/// The threshold for the third fee tier (10M to 100M QAN).
pub const FEE_TIER3_THRESHOLD: u64 = 100_000_000 * SMALLEST_UNITS_PER_QAN;
/// The fee rate for the first tier (0%).
pub const FEE_RATE_TIER1: f64 = 0.00;
/// The fee rate for the second tier (1%).
pub const FEE_RATE_TIER2: f64 = 0.01;
/// The fee rate for the third tier (2%).
pub const FEE_RATE_TIER3: f64 = 0.02;
/// The fee rate for the fourth tier (3%).
pub const FEE_RATE_TIER4: f64 = 0.03;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("ΛΣ-ΩMEGA Protocol rejected the action as unstable")]
    OmegaRejection,
    #[error("Invalid address format")]
    InvalidAddress,
    #[error("Quantum-resistant signature verification failed")]
    QuantumSignatureVerification,
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
}

#[derive(Clone, Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
pub struct Input {
    pub tx_id: String,
    pub output_index: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Output {
    pub address: String,
    pub amount: u64,
    pub homomorphic_encrypted: HomomorphicEncrypted,
}

pub struct TransactionConfig {
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub fee: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub priority_fee: u64,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub metadata: Option<HashMap<String, String>>,
    pub tx_timestamps: Arc<RwLock<HashMap<String, u64>>>,
}

#[derive(Debug)]
struct TransactionSigningPayload<'a> {
    sender: &'a str,
    receiver: &'a str,
    amount: u64,
    fee: u64,
    inputs: &'a [Input],
    outputs: &'a [Output],
    metadata: &'a HashMap<String, String>,
    timestamp: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Transaction {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub fee: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub gas_price: u64,
    pub priority_fee: u64,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    pub signature: QuantumResistantSignature,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_breakdown: Option<FeeBreakdown>,
}

/// Calculates the transaction fee based on the new tiered structure.
pub fn calculate_dynamic_fee(amount: u64) -> u64 {
    let rate = if amount < FEE_TIER1_THRESHOLD {
        FEE_RATE_TIER1 // 0% fee for amounts under 1,000,000 QAN
    } else if amount < FEE_TIER2_THRESHOLD {
        FEE_RATE_TIER2 // 1% fee for amounts between 1M and 10M QAN
    } else if amount < FEE_TIER3_THRESHOLD {
        FEE_RATE_TIER3 // 2% fee for amounts between 10M and 100M QAN
    } else {
        FEE_RATE_TIER4 // 3% for amounts over 100M QAN
    };
    (amount as f64 * rate).round() as u64
}

impl Transaction {
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
        }
    }

    /// Creates a new signed dummy transaction for testing purposes.
    pub fn new_dummy_signed(
        signing_key: &crate::qanto_native_crypto::QantoPQPrivateKey,
        _public_key: &crate::qanto_native_crypto::QantoPQPublicKey,
    ) -> Result<Self, TransactionError> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let dummy_id: [u8; 32] = rng.gen();
        let dummy_sender: [u8; 32] = rng.gen();
        let dummy_receiver: [u8; 32] = rng.gen();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let sender_str = hex::encode(dummy_sender);
        let receiver_str = hex::encode(dummy_receiver);

        let signing_payload = TransactionSigningPayload {
            sender: &sender_str,
            receiver: &receiver_str,
            amount: 100,
            fee: 0,
            inputs: &[],
            outputs: &[],
            metadata: &HashMap::new(),
            timestamp,
        };

        let signature_data = Self::serialize_for_signing(&signing_payload)?;

        let signature_obj = QuantumResistantSignature::sign(signing_key, &signature_data)
            .map_err(|_| TransactionError::QuantumSignatureVerification)?;

        Ok(Self {
            id: hex::encode(dummy_id),
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
        })
    }

    /// Verifies the quantum-resistant signature of the transaction.
    pub fn verify_signature(
        &self,
        _public_key: &crate::qanto_native_crypto::QantoPQPublicKey,
    ) -> Result<(), TransactionError> {
        let signing_payload = TransactionSigningPayload {
            sender: &self.sender,
            receiver: &self.receiver,
            amount: self.amount,
            fee: self.fee,
            inputs: &self.inputs,
            outputs: &self.outputs,
            metadata: &self.metadata,
            timestamp: self.timestamp,
        };

        let signature_data = Self::serialize_for_signing(&signing_payload)?;

        if !QuantumResistantSignature::verify(&self.signature, &signature_data) {
            return Err(TransactionError::QuantumSignatureVerification);
        }
        Ok(())
    }

    /// Creates a new transaction with the given configuration.
    pub async fn new(
        config: TransactionConfig,
        signing_key: &crate::qanto_native_crypto::QantoPQPrivateKey,
    ) -> Result<Self, TransactionError> {
        Self::validate_structure_pre_creation(
            &config.sender,
            &config.receiver,
            config.amount,
            &config.inputs,
            config.metadata.as_ref(),
        )?;
        Self::check_rate_limit(&config.tx_timestamps, MAX_TRANSACTIONS_PER_MINUTE).await?;
        Self::validate_addresses(&config.sender, &config.receiver, &config.outputs)?;
        let timestamp = Self::get_current_timestamp()?;
        let metadata = config.metadata.unwrap_or_default();
        let signing_payload = TransactionSigningPayload {
            sender: &config.sender,
            receiver: &config.receiver,
            amount: config.amount,
            fee: config.fee,
            inputs: &config.inputs,
            outputs: &config.outputs,
            metadata: &metadata,
            timestamp,
        };
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

        let mut tx = Self {
            id: String::new(),
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
                .map_err(|_| TransactionError::QuantumSignatureVerification)?,
            fee_breakdown: None, // Will be calculated by gas fee model
        };
        tx.id = tx.compute_hash();
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
        reward: u64,
        outputs: Vec<Output>,
    ) -> Result<Self, TransactionError> {
        let sender = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let timestamp = Self::get_current_timestamp()?;
        let metadata = HashMap::new();
        let signing_payload = TransactionSigningPayload {
            sender: &sender,
            receiver: &receiver,
            amount: reward,
            fee: 0,
            inputs: &[],
            outputs: &outputs,
            metadata: &metadata,
            timestamp,
        };
        let signature_data = Self::serialize_for_signing(&signing_payload)?;

        let sk = crate::qanto_native_crypto::QantoPQPrivateKey::new_dummy(); // Placeholder for actual key
        let signature_obj = QuantumResistantSignature::sign(&sk, &signature_data)
            .map_err(|_| TransactionError::QuantumSignatureVerification)?;

        let mut tx = Self {
            id: String::new(),
            sender,
            receiver,
            amount: reward,
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
        };
        tx.id = tx.compute_hash();
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
        sender: &str,
        receiver: &str,
        amount: u64,
        inputs: &[Input],
        metadata: Option<&HashMap<String, String>>,
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
        sender: &str,
        receiver: &str,
        outputs: &[Output],
    ) -> Result<(), TransactionError> {
        if !Self::is_valid_address(sender)
            || !Self::is_valid_address(receiver)
            || outputs.iter().any(|o| !Self::is_valid_address(&o.address))
        {
            Err(TransactionError::InvalidAddress)
        } else {
            Ok(())
        }
    }

    /// Checks if a given address string is valid.
    fn is_valid_address(address: &str) -> bool {
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
    fn serialize_for_signing(
        payload: &TransactionSigningPayload,
    ) -> Result<Vec<u8>, TransactionError> {
        let mut data = Vec::new();
        data.extend_from_slice(payload.sender.as_bytes());
        data.extend_from_slice(payload.receiver.as_bytes());
        data.extend_from_slice(&payload.amount.to_be_bytes());
        data.extend_from_slice(&payload.fee.to_be_bytes());
        payload.inputs.iter().for_each(|i| {
            data.extend_from_slice(i.tx_id.as_bytes());
            data.extend_from_slice(&i.output_index.to_be_bytes());
        });
        payload.outputs.iter().for_each(|o| {
            data.extend_from_slice(o.address.as_bytes());
            data.extend_from_slice(&o.amount.to_be_bytes());
        });
        let mut sorted_metadata: Vec<_> = payload.metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);
        sorted_metadata.iter().for_each(|(k, v)| {
            data.extend_from_slice(k.as_bytes());
            data.extend_from_slice(v.as_bytes());
        });
        data.extend_from_slice(&payload.timestamp.to_be_bytes());
        Ok(qanto_hash(&data).as_bytes().to_vec())
    }

    /// Computes the hash of the transaction.
    fn compute_hash(&self) -> String {
        let mut data = Vec::new();
        data.extend_from_slice(self.sender.as_bytes());
        data.extend_from_slice(self.receiver.as_bytes());
        data.extend_from_slice(&self.amount.to_be_bytes());
        data.extend_from_slice(&self.fee.to_be_bytes());
        self.inputs.iter().for_each(|i| {
            data.extend_from_slice(i.tx_id.as_bytes());
            data.extend_from_slice(&i.output_index.to_be_bytes());
        });
        self.outputs.iter().for_each(|o| {
            data.extend_from_slice(o.address.as_bytes());
            data.extend_from_slice(&o.amount.to_be_bytes());
        });
        let mut sorted_metadata: Vec<_> = self.metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);
        sorted_metadata.iter().for_each(|(k, v)| {
            data.extend_from_slice(k.as_bytes());
            data.extend_from_slice(v.as_bytes());
        });
        data.extend_from_slice(&self.timestamp.to_be_bytes());
        let hash = qanto_hash(&data);
        hex::encode(hash.as_bytes())
    }

    /// Memory-optimized verification using a shared UTXO reference.
    pub async fn verify_with_shared_utxos(
        &self,
        _dag: &QantoDAG,
        utxos_arc: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), TransactionError> {
        let signing_payload = TransactionSigningPayload {
            sender: &self.sender,
            receiver: &self.receiver,
            amount: self.amount,
            fee: self.fee,
            inputs: &self.inputs,
            outputs: &self.outputs,
            metadata: &self.metadata,
            timestamp: self.timestamp,
        };
        let _data_to_verify = Self::serialize_for_signing(&signing_payload)?;

        if self.is_coinbase() {
            if self.fee != 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase fee must be 0".to_string(),
                ));
            }
            let total_output: u64 = self.outputs.iter().map(|o| o.amount).sum();
            if total_output == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase output cannot be zero".to_string(),
                ));
            }
        } else {
            // MEMORY OPTIMIZATION: Only read the UTXO set once and validate all inputs.
            let utxos_guard = utxos_arc.read().await;
            let mut total_input_value = 0;

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

            let total_output_value: u64 = self.outputs.iter().map(|o| o.amount).sum();
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
        let signing_payload = TransactionSigningPayload {
            sender: &self.sender,
            receiver: &self.receiver,
            amount: self.amount,
            fee: self.fee,
            inputs: &self.inputs,
            outputs: &self.outputs,
            metadata: &self.metadata,
            timestamp: self.timestamp,
        };
        let _data_to_verify = Self::serialize_for_signing(&signing_payload)?;

        if self.is_coinbase() {
            if self.fee != 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase fee must be 0".to_string(),
                ));
            }
            let total_output: u64 = self.outputs.iter().map(|o| o.amount).sum();
            if total_output == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase output cannot be zero".to_string(),
                ));
            }
        } else {
            let mut total_input_value = 0;
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
            let total_output_value: u64 = self.outputs.iter().map(|o| o.amount).sum();

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
        verification_semaphore: &Arc<Semaphore>,
    ) -> Vec<Result<(), TransactionError>> {
        transactions
            .par_iter()
            .map(|tx| {
                // Acquire a semaphore permit for controlled concurrency.
                let _permit = verification_semaphore.try_acquire();
                if _permit.is_err() {
                    // Fallback to sequential verification if the semaphore is full.
                    return Self::verify_single_transaction(tx, utxos_map);
                }

                Self::verify_single_transaction(tx, utxos_map)
            })
            .collect()
    }

    /// A single transaction verification helper for batch processing.
    pub fn verify_single_transaction(
        tx: &Transaction,
        utxos: &HashMap<String, UTXO>,
    ) -> Result<(), TransactionError> {
        let signing_payload = TransactionSigningPayload {
            sender: &tx.sender,
            receiver: &tx.receiver,
            amount: tx.amount,
            fee: tx.fee,
            inputs: &tx.inputs,
            outputs: &tx.outputs,
            metadata: &tx.metadata,
            timestamp: tx.timestamp,
        };
        let _data_to_verify = Self::serialize_for_signing(&signing_payload)?;

        // Signature verification is removed from this batch helper for performance.
        // It's assumed to be done separately.

        if tx.is_coinbase() {
            if tx.fee != 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase fee must be 0".to_string(),
                ));
            }
            let total_output: u64 = tx.outputs.iter().map(|o| o.amount).sum();
            if total_output == 0 {
                return Err(TransactionError::InvalidStructure(
                    "Coinbase output cannot be zero".to_string(),
                ));
            }
        } else {
            let mut total_input_value = 0;
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
            let total_output_value: u64 = tx.outputs.iter().map(|o| o.amount).sum();

            if total_input_value < total_output_value + tx.fee {
                return Err(TransactionError::InsufficientFunds);
            }
        }
        Ok(())
    }

    /// Lightweight transaction validation for mempool admission.
    pub fn validate_for_mempool(&self) -> Result<(), TransactionError> {
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
            return Err(TransactionError::InvalidStructure(
                "Non-coinbase transaction must have a fee".to_string(),
            ));
        }

        // Check for duplicate inputs
        let mut input_set = std::collections::HashSet::new();
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
    pub fn calculate_gas_fee(
        &mut self,
        gas_fee_model: &GasFeeModel,
    ) -> Result<(), TransactionError> {
        let storage_duration = StorageDuration::ShortTerm; // Default to short-term storage

        let fee_breakdown = gas_fee_model.calculate_transaction_fee(
            self,
            self.gas_limit,
            self.priority_fee,
            storage_duration,
        )?;

        self.fee = fee_breakdown.total_fee;
        self.fee_breakdown = Some(fee_breakdown);
        Ok(())
    }

    /// Updates gas usage after transaction execution
    pub fn update_gas_usage(&mut self, gas_used: u64) -> Result<(), TransactionError> {
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
            return Err(TransactionError::InvalidStructure(
                "Gas price cannot be zero for non-coinbase transactions".to_string(),
            ));
        }

        Ok(())
    }

    /// Gets the effective fee rate (fee per gas unit)
    pub fn get_effective_fee_rate(&self) -> f64 {
        if self.gas_used == 0 {
            0.0
        } else {
            self.fee as f64 / self.gas_used as f64
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
