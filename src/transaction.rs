// src/transaction.rs

//! --- Qanto Transaction ---
//! v3.0.1 - Trait Scoping Resolved

use crate::omega;
use crate::qantodag::{HomomorphicEncrypted, QantoDAG, QuantumResistantSignature, UTXO};
use hex;
use pqcrypto_mldsa::mldsa65::{PublicKey, SecretKey};
use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak512};
use sp_core::H256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::instrument;
use zeroize::Zeroize;

// --- Performance & Fee Constants ---
// Increased rate limit to align with 10M+ TPS goal.
const MAX_TRANSACTIONS_PER_MINUTE: u64 = 600_000_000;
const MAX_METADATA_PAIRS: usize = 16;
const MAX_METADATA_KEY_LEN: usize = 64;
const MAX_METADATA_VALUE_LEN: usize = 256;

// --- New Dynamic Fee Structure ---
// Represents the smallest unit of currency (e.g., satoshis, wei).
// Assuming 1,000,000 units = $1 USD for this example.
const FEE_TIER1_THRESHOLD: u64 = 1_000_000;
const FEE_TIER2_THRESHOLD: u64 = 10_000_000;
const FEE_TIER3_THRESHOLD: u64 = 100_000_000;
const FEE_RATE_TIER1: f64 = 0.00; // 0%
const FEE_RATE_TIER2: f64 = 0.01; // 1%
const FEE_RATE_TIER3: f64 = 0.02; // 2%
const FEE_RATE_TIER4: f64 = 0.03; // 3%

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
}

#[derive(Clone, Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
pub struct Input {
    pub tx_id: String,
    pub output_index: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Output {
    pub address: String,
    pub amount: u64,
    pub homomorphic_encrypted: HomomorphicEncrypted,
}

pub struct TransactionConfig<'a> {
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub fee: u64,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub metadata: Option<HashMap<String, String>>,
    pub signing_key_bytes: &'a [u8],
    pub public_key_bytes: &'a [u8],
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub fee: u64,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub qr_signature: QuantumResistantSignature,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Calculates the transaction fee based on the new tiered structure.
pub fn calculate_dynamic_fee(amount: u64) -> u64 {
    let rate = if amount < FEE_TIER1_THRESHOLD {
        FEE_RATE_TIER1 // 0% fee for amounts under $1
    } else if amount < FEE_TIER2_THRESHOLD {
        FEE_RATE_TIER2 // 1% fee for amounts between $1 and $10
    } else if amount < FEE_TIER3_THRESHOLD {
        FEE_RATE_TIER3 // 2% fee for amounts between $10 and $100
    } else {
        FEE_RATE_TIER4 // 3% for amounts over $100
    };
    (amount as f64 * rate).round() as u64
}

impl Transaction {
    #[instrument(skip(config))]
    pub async fn new(config: TransactionConfig<'_>) -> Result<Self, TransactionError> {
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
        let action_hash =
            H256::from_slice(Keccak512::digest(&signature_data).as_slice()[..32].as_ref());
        if !omega::reflect_on_action(action_hash).await {
            return Err(TransactionError::OmegaRejection);
        }

        // --- Corrected Key Creation ---
        // Now that the traits are in scope, these calls will compile correctly.
        let sk = SecretKey::from_bytes(config.signing_key_bytes)
            .map_err(|e| TransactionError::PqCrypto(e.to_string()))?;
        let pk = PublicKey::from_bytes(config.public_key_bytes)
            .map_err(|e| TransactionError::PqCrypto(e.to_string()))?;
        let signature_obj = QuantumResistantSignature::sign(&sk, &pk, &signature_data)
            .map_err(|_| TransactionError::QuantumSignatureVerification)?;

        let mut tx = Self {
            id: String::new(),
            sender: config.sender,
            receiver: config.receiver,
            amount: config.amount,
            fee: config.fee,
            inputs: config.inputs,
            outputs: config.outputs,
            qr_signature: signature_obj,
            timestamp,
            metadata,
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

    pub(crate) fn new_coinbase(
        receiver: String,
        reward: u64,
        signing_key_bytes: &[u8],
        public_key_bytes: &[u8],
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

        // --- Corrected Key Creation ---
        let sk = SecretKey::from_bytes(signing_key_bytes)
            .map_err(|e| TransactionError::PqCrypto(e.to_string()))?;
        let pk = PublicKey::from_bytes(public_key_bytes)
            .map_err(|e| TransactionError::PqCrypto(e.to_string()))?;

        let signature_obj = QuantumResistantSignature::sign(&sk, &pk, &signature_data)
            .map_err(|_| TransactionError::QuantumSignatureVerification)?;

        let mut tx = Self {
            id: String::new(),
            sender,
            receiver,
            amount: reward,
            fee: 0,
            inputs: vec![],
            outputs,
            qr_signature: signature_obj,
            timestamp,
            metadata,
        };
        tx.id = tx.compute_hash();
        Ok(tx)
    }

    pub fn is_coinbase(&self) -> bool {
        self.inputs.is_empty()
    }

    pub fn get_metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

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
                return Err(TransactionError::InvalidMetadata(format!(
                    "Exceeded max metadata pairs limit of {MAX_METADATA_PAIRS}"
                )));
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

    fn is_valid_address(address: &str) -> bool {
        address.len() == 64 && hex::decode(address).is_ok()
    }

    fn get_current_timestamp() -> Result<u64, TransactionError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|_| TransactionError::TimestampError)
    }

    fn serialize_for_signing(
        payload: &TransactionSigningPayload,
    ) -> Result<Vec<u8>, TransactionError> {
        let mut hasher = Keccak512::new();
        hasher.update(payload.sender.as_bytes());
        hasher.update(payload.receiver.as_bytes());
        hasher.update(payload.amount.to_be_bytes());
        hasher.update(payload.fee.to_be_bytes());
        payload.inputs.iter().for_each(|i| {
            hasher.update(i.tx_id.as_bytes());
            hasher.update(i.output_index.to_be_bytes());
        });
        payload.outputs.iter().for_each(|o| {
            hasher.update(o.address.as_bytes());
            hasher.update(o.amount.to_be_bytes());
        });
        let mut sorted_metadata: Vec<_> = payload.metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);
        sorted_metadata.iter().for_each(|(k, v)| {
            hasher.update(k.as_bytes());
            hasher.update(v.as_bytes());
        });
        hasher.update(payload.timestamp.to_be_bytes());
        Ok(hasher.finalize().to_vec())
    }

    fn compute_hash(&self) -> String {
        let mut hasher = Keccak512::new();
        hasher.update(self.sender.as_bytes());
        hasher.update(self.receiver.as_bytes());
        hasher.update(self.amount.to_be_bytes());
        hasher.update(self.fee.to_be_bytes());
        self.inputs.iter().for_each(|i| {
            hasher.update(i.tx_id.as_bytes());
            hasher.update(i.output_index.to_be_bytes());
        });
        self.outputs.iter().for_each(|o| {
            hasher.update(o.address.as_bytes());
            hasher.update(o.amount.to_be_bytes());
        });
        let mut sorted_metadata: Vec<_> = self.metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);
        sorted_metadata.iter().for_each(|(k, v)| {
            hasher.update(k.as_bytes());
            hasher.update(v.as_bytes());
        });
        hasher.update(self.timestamp.to_be_bytes());
        hex::encode(&hasher.finalize()[..32])
    }

    #[instrument(skip(self, _dag, utxos))]
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
        let data_to_verify = Self::serialize_for_signing(&signing_payload)?;

        if !self.qr_signature.verify(&data_to_verify) {
            return Err(TransactionError::QuantumSignatureVerification);
        }

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
                let utxo_id = format!("{}_{}", input.tx_id, input.output_index);
                let utxo = utxos.get(&utxo_id).ok_or_else(|| {
                    TransactionError::InvalidStructure(format!("UTXO {utxo_id} not found"))
                })?;
                if utxo.address != self.sender {
                    return Err(TransactionError::InvalidStructure(format!(
                        "Input UTXO {utxo_id} does not belong to sender"
                    )));
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

    #[instrument]
    pub fn generate_utxo(&self, index: u32) -> UTXO {
        let output = &self.outputs[index as usize];
        let utxo_id = format!("{}_{}", self.id, index);
        UTXO {
            address: output.address.clone(),
            amount: output.amount,
            tx_id: self.id.clone(),
            output_index: index,
            explorer_link: format!("https://qantoblockexplorer.org/utxo/{utxo_id}"),
        }
    }
}

impl Zeroize for Transaction {
    fn zeroize(&mut self) {
        self.id.zeroize();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omega::{self, identity::set_threat_level, identity::ThreatLevel, OmegaState};
    use crate::qantodag::{QantoDAG, QantoDagConfig};
    use crate::saga::PalletSaga;
    use crate::wallet::Wallet;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_transaction_creation_and_verification() -> Result<(), Box<dyn std::error::Error>>
    {
        {
            let mut state = omega::OMEGA_STATE.lock().await;
            *state = OmegaState::new();
            set_threat_level(ThreatLevel::Nominal);
        }

        let db_path = "qantodag_db_test";
        if std::path::Path::new(db_path).exists() {
            std::fs::remove_dir_all(db_path)?;
        }

        let wallet = Arc::new(Wallet::new()?);
        let (qr_secret_key, qr_public_key) = wallet.get_keypair()?;
        let sender_address = wallet.address();
        let amount_to_receiver = 50;

        // Use the new dynamic fee calculation
        let fee = calculate_dynamic_fee(amount_to_receiver);

        let dev_fee_on_transfer = (amount_to_receiver as f64 * 0.0304).round() as u64;

        let mut initial_utxos_map = HashMap::new();
        let input_utxo_amount = amount_to_receiver + fee + dev_fee_on_transfer + 10;
        let genesis_utxo_for_test = UTXO {
            address: sender_address.clone(),
            amount: input_utxo_amount,
            tx_id: "genesis_tx_id_for_test_0".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };
        initial_utxos_map.insert(
            "genesis_tx_id_for_test_0_0".to_string(),
            genesis_utxo_for_test,
        );

        let inputs_for_tx = vec![Input {
            tx_id: "genesis_tx_id_for_test_0".to_string(),
            output_index: 0,
        }];

        let change_amount = input_utxo_amount - amount_to_receiver - fee - dev_fee_on_transfer;

        let he_public_key_dalek = wallet.get_signing_key()?.verifying_key();
        let he_pub_key_material_slice: &[u8] = he_public_key_dalek.as_bytes();

        let mut outputs_for_tx = vec![Output {
            address: "0000000000000000000000000000000000000000000000000000000000000001".to_string(),
            amount: amount_to_receiver,
            homomorphic_encrypted: HomomorphicEncrypted::new(
                amount_to_receiver,
                he_pub_key_material_slice,
            ),
        }];
        if dev_fee_on_transfer > 0 {
            outputs_for_tx.push(Output {
                address: "74fd2aae70ae8e0930b87a3dcb3b77f5b71d956659849f067360d3486604db41"
                    .to_string(), // DEV_ADDRESS
                amount: dev_fee_on_transfer,
                homomorphic_encrypted: HomomorphicEncrypted::new(
                    dev_fee_on_transfer,
                    he_pub_key_material_slice,
                ),
            });
        }
        if change_amount > 0 {
            outputs_for_tx.push(Output {
                address: sender_address.clone(),
                amount: change_amount,
                homomorphic_encrypted: HomomorphicEncrypted::new(
                    change_amount,
                    he_pub_key_material_slice,
                ),
            });
        }

        let mut metadata = HashMap::new();
        metadata.insert("memo".to_string(), "Test transaction".to_string());

        let tx_timestamps_map = Arc::new(RwLock::new(HashMap::new()));

        let tx_config = TransactionConfig {
            sender: sender_address.clone(),
            receiver: "0000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            amount: amount_to_receiver,
            fee,
            inputs: inputs_for_tx.clone(),
            outputs: outputs_for_tx.clone(),
            metadata: Some(metadata),
            signing_key_bytes: qr_secret_key.as_bytes(),
            public_key_bytes: qr_public_key.as_bytes(),
            tx_timestamps: tx_timestamps_map.clone(),
        };

        let tx = Transaction::new(tx_config).await?;

        let saga_pallet = Arc::new(PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ));

        let dag_config = QantoDagConfig {
            initial_validator: sender_address.clone(),
            target_block_time: 60000,
            num_chains: 1,
            qr_signing_key: &qr_secret_key,
            qr_public_key: &qr_public_key,
        };
        let dag_arc = QantoDAG::new(
            dag_config,
            saga_pallet,
            rocksdb::DB::open_default(db_path).unwrap(),
        )?;

        let utxos_arc_for_test = Arc::new(RwLock::new(initial_utxos_map));
        let utxos_read_guard = utxos_arc_for_test.read().await;
        tx.verify(&dag_arc, &utxos_read_guard)
            .await
            .map_err(|e| format!("TX verification error: {e:?}"))?;

        let generated_utxo_instance = tx.generate_utxo(0);
        assert_eq!(generated_utxo_instance.tx_id, tx.id);
        assert_eq!(generated_utxo_instance.amount, amount_to_receiver);

        Ok(())
    }
}
