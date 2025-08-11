//! --- Qanto Hybrid Consensus Engine ---
//! v2.1.0 - User-Specified Finality Model
//!
//! This module implements the core consensus rules for the Qanto network.
//! It uses a hybrid model where finality is determined by a combination of PoW and PoS,
//! with PoW as the primary mechanism.
//!
//! ### Finality Model (As per Specification)
//!
//! 1.  **Proof-of-Work (PoW): The "Primary Finality"**. This is the fundamental security
//!     and default finality layer. Every valid block MUST contain a valid Proof-of-Work
//!     solution. The cumulative work on a chain of blocks is the primary determinant of its finality,
//!     making history computationally expensive to rewrite.
//!
//! 2.  **Proof-of-Stake (PoS): The "Finality Helper"**. This mechanism helps accelerate the
//!     appearance of finality and signals validator confidence. It is **NOT** a strict requirement
//!     for a block to be valid. A block with valid PoW but low stake is still considered
//!     valid, though it may be flagged as having lower PoS-backed confidence. Validators
//!     are **NO LONGER REQUIRED** to have a minimum stake to produce blocks.
//!
//! 3.  **Proof-of-Sentiency (PoSe): The "Intelligence Layer"**. Powered by the SAGA
//!     pallet, PoSe makes the primary PoW mechanism "smarter" by dynamically adjusting
//!     the PoW difficulty for each miner based on their reputation (Saga Credit Score - SCS).
//!     This makes the network more efficient and secure without replacing PoW.

use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError, UTXO};
use crate::saga::{PalletSaga, SagaError};
use crate::transaction::TransactionError;
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

        // --- Rule 3: Transaction Validity ---
        // Ensures every transaction in the block is valid.
        let utxos_guard = utxos.read().await;
        for tx in block.transactions.iter().skip(1) {
            // Skip coinbase
            tx.verify(dag_arc, &utxos_guard).await?;
        }
        drop(utxos_guard);

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

        if !block.verify_signature()? {
            return Err(ConsensusError::InvalidBlockStructure(
                "Block signature verification failed".to_string(),
            ));
        }

        let expected_merkle_root = QantoBlock::compute_merkle_root(&block.transactions)
            .map_err(|e| ConsensusError::InvalidBlockStructure(e.to_string()))?;
        if block.merkle_root != expected_merkle_root {
            return Err(ConsensusError::InvalidBlockStructure(
                "Merkle root mismatch".to_string(),
            ));
        }

        let coinbase = &block.transactions[0];
        if !coinbase.is_coinbase() {
            return Err(ConsensusError::InvalidBlockStructure(
                "First transaction must be coinbase".to_string(),
            ));
        }

        let expected_reward = self.saga.calculate_dynamic_reward(block, dag_arc).await?;
        if block.reward != expected_reward {
            return Err(ConsensusError::InvalidBlockStructure(format!(
                "Block reward mismatch. Claimed: {}, Expected (from SAGA): {}",
                block.reward, expected_reward
            )));
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
        // --- NEW LOGIC: NO MINIMUM STAKE REQUIRED ---
        // The check for a minimum stake (e.g., 1,000 QNTO) has been removed.
        // A validator NO LONGER NEEDS to have the required minimum stake.
        // PoW is now the primary determinant of finality. This function can still be
        // used to check for stake and issue warnings, but it will not cause validation to fail.
        let rules = self.saga.economy.epoch_rules.read().await;
        let min_stake_for_full_confidence =
            rules.get("min_validator_stake").map_or(1000.0, |r| r.value) as u64;

        let validators = dag.validators.read().await;
        let validator_stake = validators.get(validator_address).copied().unwrap_or(0);

        if validator_stake < min_stake_for_full_confidence {
            // This error will be caught and logged as a warning in `validate_block`.
            return Err(ConsensusError::ProofOfStakeFailed(format!(
                "Low PoS confidence. Stake of {validator_stake} is below the nominal threshold of {min_stake_for_full_confidence}"
            )));
        }
        Ok(())
    }

    /// Validates the block's Proof-of-Work against the dynamically adjusted difficulty target from SAGA.
    async fn validate_proof_of_work(&self, block: &QantoBlock) -> Result<(), ConsensusError> {
        let effective_difficulty = self.get_effective_difficulty(&block.miner).await;

        if block.difficulty != effective_difficulty {
            warn!(
                "Block {} has difficulty mismatch. Claimed: {}, Required (by PoSe): {}",
                block.id, block.difficulty, effective_difficulty
            );
            return Err(ConsensusError::ProofOfWorkFailed(format!(
                "Difficulty mismatch. Claimed: {}, Required by PoSe: {}",
                block.difficulty, effective_difficulty
            )));
        }

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
    pub async fn get_effective_difficulty(&self, miner_address: &str) -> u64 {
        let rules = self.saga.economy.epoch_rules.read().await;
        let base_difficulty = rules.get("base_difficulty").map_or(10.0, |r| r.value) as u64;

        let scs = self
            .saga
            .reputation
            .credit_scores
            .read()
            .await
            .get(miner_address)
            .map_or(0.5, |s| s.score);

        let difficulty_modifier = 1.0 - (scs - 0.5);
        let effective_difficulty = (base_difficulty as f64 * difficulty_modifier).round() as u64;

        effective_difficulty.clamp(
            base_difficulty.saturating_div(2),
            base_difficulty.saturating_mul(2),
        )
    }
}
