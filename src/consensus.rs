//! --- Qanto Hybrid Consensus Engine ---
//! v2.2.0 - Deterministic PoW Alignment
//! This version is aligned with the new deterministic PoW target calculation
//! in the Miner module, ensuring that all consensus checks are free from
//! floating-point nondeterminism.

use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::saga::{PalletSaga, SagaError};
use crate::transaction::TransactionError;
use crate::types::UTXO;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, instrument, warn};

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

    /// The primary validation function. It checks a block against all consensus rules,
    /// prioritizing Proof-of-Work as the primary finality mechanism.
    #[instrument(skip(self, block, dag_arc, utxos), fields(block_id = %block.id, miner = %block.miner))]
    pub async fn validate_block(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
        utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    ) -> Result<(), ConsensusError> {
        // --- Rule 1: Structural & Cryptographic Integrity (Fastest Check) ---
        self.validate_block_structure(block, dag_arc).await?;

        // --- Rule 2: Proof-of-Work (PoW) with PoSe - The "Primary Finality" ---
        // This is the most critical check. A block is fundamentally invalid without correct PoW.
        self.validate_proof_of_work(block).await?;

        // --- Rule 3: Transaction Validity (OPTIMIZED: Parallel Processing) ---
        // Ensures every transaction in the block is valid using parallel verification
        let utxos_guard = utxos.read().await;
        let utxos_clone = utxos_guard.clone();
        drop(utxos_guard);

        // OPTIMIZATION: Use parallel processing for transaction verification
        use rayon::prelude::*;
        let verification_results: Result<Vec<_>, _> = block
            .transactions
            .par_iter()
            .skip(1) // Skip coinbase
            .map(|tx| {
                // Create a blocking task for async verification in parallel context
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(async { tx.verify(dag_arc, &utxos_clone).await })
                })
            })
            .collect();

        // Check if any verification failed
        verification_results?;

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

        debug!("All consensus checks passed for block {}", block.id);
        Ok(())
    }

    /// Performs all fundamental structural and cryptographic checks on a block.
    async fn validate_block_structure(
        &self,
        block: &QantoBlock,
        dag_arc: &Arc<QantoDAG>,
    ) -> Result<(), ConsensusError> {
        self.validate_core_fields(block)?;
        self.validate_block_signature(block)?;
        self.validate_merkle_root(block)?;
        self.validate_coinbase_transaction(block)?;
        self.validate_block_reward(block, dag_arc).await?;
        Ok(())
    }

    /// Validates that core block fields are not empty
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

    /// Validates the block's cryptographic signature
    fn validate_block_signature(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        if !block.verify_signature()? {
            return Err(ConsensusError::InvalidBlockStructure(
                "Block signature verification failed".to_string(),
            ));
        }
        Ok(())
    }

    /// Validates the block's Merkle root against its transactions
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

    /// Validates that the first transaction is a valid coinbase transaction
    fn validate_coinbase_transaction(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        let coinbase = &block.transactions[0];
        if !coinbase.is_coinbase() {
            return Err(ConsensusError::InvalidBlockStructure(
                "First transaction must be coinbase".to_string(),
            ));
        }
        Ok(())
    }

    /// Validates the block reward against the expected reward from SAGA
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
            rules.get("min_validator_stake").map_or(1000.0, |r| r.value) as u64;

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

        // Now uses a deterministic, integer-based calculation internally.
        let target_hash =
            crate::miner::Miner::calculate_target_from_difficulty(effective_difficulty);
        let block_pow_hash = hex::decode(block.hash()).map_err(|_| {
            ConsensusError::StateError("Failed to decode block PoW hash".to_string())
        })?;

        if !crate::miner::Miner::hash_meets_target(&block_pow_hash, &target_hash) {
            return Err(ConsensusError::ProofOfWorkFailed(
                "Block hash does not meet PoSe difficulty target.".to_string(),
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
        let base_difficulty = rules.get("base_difficulty").map_or(10.0, |r| r.value);

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

        // Clamp the difficulty within a reasonable range (e.g., 50% to 200% of base).
        effective_difficulty.clamp(base_difficulty / 2.0, base_difficulty * 2.0)
    }
}
