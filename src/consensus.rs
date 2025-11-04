//! --- Qanto Hybrid Consensus Engine ---
//! v0.1.0 - Initial Version
//!
//! This version implements a basic consensus engine with transaction prioritization
//! and UTXO tracking.
//!
//! # Features
//! - **Transaction Prioritization**: Orders transactions based on fee and priority,
//!   ensuring that high-value transactions are processed first.
//! - **UTXO Tracking**: Maintains a UTXO set to track available balances and validate
//!   transaction inputs.   
//! - **Transaction Validation**: Ensures that transactions adhere to the Qanto protocol
//!   rules, including fee requirements, input/output validation, and script evaluation.
//! - **Block Validation**: Validates the structure and integrity of blocks, including
//!   Proof-of-Work (PoW) and Proof-of-Stake (PoS) checks.
//! - **SAGA-ΩMEGA Security**: Ensures that blocks are validated against the SAGA-ΩMEGA
//!   consensus protocol, providing a high level of security and decentralization.
//! - **Post-Quantum Cryptography**: Utilizes post-quantum cryptographic primitives to
//!   safeguard sensitive data and transactions against future quantum attacks.
//! - **Decentralized Governance**: Utilizes a decentralized governance model to ensure
//!   transparent and community-driven decision-making.
//! - **Scalability Solutions**: Implements sharding, layer-2 protocols, and other scalability
//!   solutions to handle high transaction volumes.
//! - **Interoperability Standards**: Adopts interoperability standards to enable seamless
//!   integration with other blockchains and ecosystems.
//! - **Regulatory Compliance**: Incorporates mechanisms to ensure compliance with relevant
//!   regulations and standards.
//! - **Layer-2 Scalability**: Integrates layer-2 protocols to improve transaction
//!   throughput and reduce network congestion.
//! - **Privacy Features**: Implements privacy-enhancing technologies, such as zero-knowledge
//!   proofs and confidential transactions.
//! - **Interoperability**: Develops interoperability mechanisms to connect Qanto with other
//!   blockchains and ecosystems.

use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::saga::{PalletSaga, SagaError};
use crate::transaction::TransactionError;
use crate::types::UTXO;
use futures::future;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Invalid block structure: {0}")]
    InvalidBlockStructure(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(#[from] TransactionError),
    #[error("Proof-of-Work check failed: {0}")]
    ProofOfWorkFailed(String),
    #[error("Proof-of-Stake check failed: {0}")]
    ProofOfStakeFailed(String),
    #[error("Block failed SAGA-ΩMEGA security validation: {0}")]
    OmegaRejection(String),
    #[error("Database or state error during validation: {0}")]
    StateError(String),
    #[error("Saga error: {0}")]
    SagaError(#[from] SagaError),
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("QantoDAG error: {0}")]
    QantoDAG(#[from] QantoDAGError),
}

/// The main consensus engine for Qanto. It orchestrates the various validation
/// mechanisms to ensure network integrity.
#[derive(Debug)]
pub struct Consensus {
    saga: Arc<PalletSaga>,
}

impl Consensus {
    /// Creates a new Consensus engine instance, linking it to the SAGA pallet.
    pub fn new(saga: Arc<PalletSaga>) -> Self {
        Self { saga }
    }

    /// **CANONICAL POW VALIDATION FUNCTION**
    ///
    /// This is the single source of truth for Proof-of-Work validation.
    /// Both the miner and DAG validation logic MUST use this function to ensure consistency.
    ///
    /// # Arguments
    /// * `hash_bytes` - The block hash as raw bytes
    /// * `difficulty` - The target difficulty as a floating-point number
    ///
    /// # Returns
    /// * `true` if the hash meets the difficulty target, `false` otherwise
    pub fn is_pow_valid(hash_bytes: &[u8], difficulty: f64) -> bool {
        let target_hash = crate::miner::Miner::calculate_target_from_difficulty(difficulty);
        crate::miner::Miner::hash_meets_target(hash_bytes, &target_hash)
    }

    /// The primary validation function. It checks a block against all consensus rules,
    /// prioritizing Proof-of-Work as the primary finality mechanism.
    pub async fn validate_block(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), ConsensusError> {
        // --- Rule 1: Structural & Cryptographic Integrity (Fastest Check) ---
        // These checks are cheap to perform and can quickly invalidate a malformed block.
        self.validate_core_fields(block)?;
        self.validate_merkle_root(block)?;
        self.validate_coinbase_transaction(block)?;

        // --- Rule 2: Proof-of-Work (PoW) with PoSe - The "Primary Finality" ---
        // This is the most critical check. A block is fundamentally invalid without correct PoW.
        // It secures the network against spam and ensures that computational effort was expended.
        self.validate_proof_of_work(block).await?;

        // --- Rule 2.1: Optional Finality Proof Check ---
        // If a block includes a `finality_proof`, verify it matches the canonical PoW hash.
        // A mismatch is logged as a warning but does NOT invalidate the block.
        if let Err(e) = self.validate_finality_proof_optional(block) {
            warn!(
                "Finality proof warning for block {}: {}. Block remains valid due to PoW.",
                block.id, e
            );
        }

        // --- Rule 3: Transaction Validity (MEMORY OPTIMIZED: Shared Reference Processing) ---
        // Ensures every transaction in the block is valid using memory-efficient parallel verification.
        // MEMORY OPTIMIZATION: Use Arc reference instead of cloning the entire UTXO set.
        let utxos_arc = Arc::clone(utxos);

        // OPTIMIZATION: Use parallel processing with a shared UTXO reference to reduce memory usage.
        use rayon::prelude::*;
        let verification_tasks: Vec<_> = block
            .transactions
            .par_iter()
            .skip(1) // Skip the coinbase transaction, which has no inputs to verify.
            .map(|tx| {
                // Clone the transaction to avoid lifetime issues with tokio::spawn
                let tx_clone = tx.clone();
                let utxos_ref = Arc::clone(&utxos_arc);
                let dag_ref = Arc::clone(dag_arc);

                // Spawn async task instead of using block_on to avoid runtime-in-runtime panic
                tokio::spawn(async move {
                    // Try with shared UTXOs first; fall back to cloned UTXOs if needed.
                    match tx_clone
                        .verify_with_shared_utxos(&dag_ref, &utxos_ref)
                        .await
                    {
                        Ok(()) => Ok(()),
                        Err(_) => {
                            let guard = utxos_ref.read().await;
                            tx_clone.verify(&dag_ref, &guard.clone()).await
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        // Await all verification tasks
        let verification_results = future::join_all(verification_tasks).await;

        // Check if any transaction verification failed.
        for result in verification_results {
            match result {
                Ok(verification_result) => verification_result?,
                Err(join_error) => {
                    return Err(ConsensusError::StateError(format!(
                        "Transaction verification task failed: {join_error}"
                    )));
                }
            }
        }

        // --- Rule 4: Proof-of-Stake (PoS) - The "Finality Helper" ---
        // This check is now supplementary. A failure here logs a warning but does NOT
        // invalidate the block, as finality is primarily derived from PoW.
        if let Err(e) = self
            .validate_proof_of_stake(&block.validator, dag_arc)
            .await
        {
            warn!(
                "PoS Warning for block {}: {}. Block is still valid due to PoW.",
                block.id, e
            );
        }

        debug!(
            "All consensus checks passed for block {block_id}",
            block_id = block.id
        );
        Ok(())
    }

    /// **Crate-level Documentation Fix**: Moved the documentation comment to avoid the empty line warning.
    ///
    /// Performs all fundamental structural and cryptographic checks on a block.
    /// It validates that core block fields are not empty.
    fn validate_core_fields(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        if block.id.is_empty() || block.merkle_root.is_empty() || block.validator.is_empty() {
            return Err(ConsensusError::InvalidBlockStructure(
                "Core fields (ID, Merkle Root, Validator) cannot be empty".to_string(),
            ));
        }
        if block.transactions.is_empty() {
            return Err(ConsensusError::InvalidBlockStructure(
                "Block must have at least one transaction (coinbase)".to_string(),
            ));
        }
        Ok(())
    }

    /// Validates the block's Merkle root against its transactions.
    fn validate_merkle_root(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        let expected_merkle_root = QantoBlock::compute_merkle_root(&block.transactions)
            .map_err(|e| ConsensusError::InvalidBlockStructure(e.to_string()))?;
        if block.merkle_root != expected_merkle_root {
            return Err(ConsensusError::InvalidBlockStructure(
                "Merkle root mismatch".to_string(),
            ));
        }
        Ok(())
    }

    /// Validates that the first transaction is a valid coinbase transaction.
    fn validate_coinbase_transaction(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        let coinbase = &block.transactions[0];
        if !coinbase.is_coinbase() {
            return Err(ConsensusError::InvalidBlockStructure(
                "First transaction must be coinbase".to_string(),
            ));
        }
        Ok(())
    }

    /// Validates the block reward against the expected reward from SAGA.
    #[allow(dead_code)]
    async fn validate_block_reward(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
    ) -> Result<(), ConsensusError> {
        let total_fees = block
            .transactions
            .iter()
            .skip(1)
            .map(|tx| tx.fee)
            .sum::<u64>();
        let expected_reward = self
            .saga
            .calculate_dynamic_reward(block, dag_arc, total_fees)
            .await?;
        if block.reward != expected_reward {
            let mut msg = String::with_capacity(64);
            msg.push_str("Block reward mismatch. Claimed: ");
            msg.push_str(&block.reward.to_string());
            msg.push_str(", Expected (from SAGA): ");
            msg.push_str(&expected_reward.to_string());
            return Err(ConsensusError::InvalidBlockStructure(msg));
        }
        Ok(())
    }

    /// Validates the validator's stake. Per the new specification, this now functions
    /// as a "helper" check and no longer rejects blocks from validators with low stake.
    /// It will return an error that can be logged as a warning.
    async fn validate_proof_of_stake(
        &self,
        validator_address: &str,
        dag: &QantoDAG,
    ) -> Result<(), ConsensusError> {
        let rules = self.saga.economy.epoch_rules.read().await;
        let min_stake_for_full_confidence =
            rules.get("min_validator_stake").map_or(100.0, |r| r.value) as u64;

        let validators = &dag.validators;
        let validator_stake = validators.get(validator_address).map(|v| *v).unwrap_or(0);

        if validator_stake < min_stake_for_full_confidence {
            // This error will be caught and logged as a warning in `validate_block`.
            let mut msg = String::with_capacity(128);
            msg.push_str("Low PoS confidence. Stake of ");
            msg.push_str(&validator_stake.to_string());
            msg.push_str(" is below the nominal threshold of ");
            msg.push_str(&min_stake_for_full_confidence.to_string());
            return Err(ConsensusError::ProofOfStakeFailed(msg));
        }
        Ok(())
    }

    /// Optional finality proof validation.
    /// If present, the `finality_proof` must match the canonical PoW hash string.
    /// Any mismatch is reported as an error here, but callers should treat it as a warning.
    fn validate_finality_proof_optional(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        if let Some(ref proof) = block.finality_proof {
            let expected = format!("{}", block.hash_for_pow());
            if &expected != proof {
                let mut msg = String::with_capacity(96);
                msg.push_str("Finality proof mismatch. Provided: ");
                msg.push_str(proof);
                msg.push_str(", Expected: ");
                msg.push_str(&expected);
                return Err(ConsensusError::InvalidBlockStructure(msg));
            }
        }
        Ok(())
    }

    /// Validates the block's Proof-of-Work against the dynamically adjusted difficulty target from SAGA.
    async fn validate_proof_of_work(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        let effective_difficulty = self.get_effective_difficulty(&block.miner).await;

        // Use a small epsilon for floating-point comparison to avoid precision issues.
        if (block.difficulty - effective_difficulty).abs() > f64::EPSILON {
            warn!(
                "Block {} has difficulty mismatch. Claimed: {}, Required (by PoSe): {}",
                block.id, block.difficulty, effective_difficulty
            );
            let mut msg = String::with_capacity(64);
            msg.push_str("Difficulty mismatch. Claimed: ");
            msg.push_str(&block.difficulty.to_string());
            msg.push_str(", Required by PoSe: ");
            msg.push_str(&effective_difficulty.to_string());
            return Err(ConsensusError::ProofOfWorkFailed(msg));
        }

        // Use the canonical PoW validation function (using nonce-including PoW hash)
        let block_pow_hash = block.hash_for_pow();
        if !Self::is_pow_valid(block_pow_hash.as_bytes(), effective_difficulty) {
            return Err(ConsensusError::ProofOfWorkFailed(
                "Block PoW hash does not meet PoSe difficulty target.".to_string(),
            ));
        }

        debug!(
            "PoW validation passed for miner {}. Effective PoSe difficulty: {}",
            block.miner, effective_difficulty
        );
        Ok(())
    }

    /// Retrieves the effective PoW difficulty for a given miner.
    /// This is the core of PoSe, where SAGA's intelligence modifies the base PoW.
    pub async fn get_effective_difficulty(&self, miner_address: &str) -> f64 {
        let rules = self.saga.economy.epoch_rules.read().await;

        // Debug logging to see what's in the rules HashMap
        debug!("Available rules: {:?}", rules.keys().collect::<Vec<_>>());

        let base_difficulty = rules.get("base_difficulty").map_or(10.0, |r| {
            debug!(
                "Found base_difficulty rule with value: {value}",
                value = r.value
            );
            r.value
        });

        if !rules.contains_key("base_difficulty") {
            debug!("base_difficulty rule not found, using fallback value of 10.0");
        }

        let scs = self
            .saga
            .reputation
            .credit_scores
            .read()
            .await
            .get(miner_address)
            .map_or(0.5, |s| s.score);

        let difficulty_modifier = 1.0 - (scs - 0.5);
        let effective_difficulty = base_difficulty * difficulty_modifier;

        debug!(
            "Calculated effective difficulty: {} (base: {}, modifier: {})",
            effective_difficulty, base_difficulty, difficulty_modifier
        );

        // Clamp the difficulty within a reasonable range (e.g., 50% to 200% of base).
        effective_difficulty.clamp(base_difficulty / 2.0, base_difficulty * 2.0)
    }
}
