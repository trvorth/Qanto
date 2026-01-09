//! Node-side adaptive mining adapter
//!
//! This module provides `NodeMiningAdapter`, an implementation of the core
//! `MiningAdapter` trait that bridges Qanto node types (DAG, Wallet, Miner,
//! Mempool, UTXO, metrics, diagnostics) with the adaptive mining loop defined
//! in `qanto-core`.

use crate::diagnostics::DiagnosticsEngine;
use crate::mempool::Mempool;
use crate::miner::Miner;
use crate::mining_celebration::{on_block_mined, MiningCelebrationParams};
use crate::mining_metrics::{MiningFailureType, MiningMetrics};
use crate::node_keystore::Wallet;
use crate::p2p::P2PCommand;
use crate::qantodag::{QantoBlock, QantoDAG, QantoDAGError};
use crate::types::UTXO;
use qanto_core::adaptive_mining::{MiningAdapter, MiningAttemptReport};
use qanto_core::pow::{solve_pow, MiningResult as CoreMiningResult, PowConfig};
use qanto_core::qanto_native_crypto::qanto_hash;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

/// Node-side adapter that bridges core adaptive mining with concrete Qanto types.
pub struct NodeMiningAdapter {
    dag: Arc<QantoDAG>,
    wallet: Arc<Wallet>,
    miner: Arc<Miner>,
    mempool: Arc<RwLock<Mempool>>,
    utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    metrics: Arc<MiningMetrics>,
    diagnostics: Option<Arc<DiagnosticsEngine>>,
    p2p_command_sender: Option<mpsc::Sender<P2PCommand>>,
}

impl NodeMiningAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dag: Arc<QantoDAG>,
        wallet: Arc<Wallet>,
        miner: Arc<Miner>,
        mempool: Arc<RwLock<Mempool>>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        metrics: Arc<MiningMetrics>,
        diagnostics: Option<Arc<DiagnosticsEngine>>,
        p2p_command_sender: Option<mpsc::Sender<P2PCommand>>,
    ) -> Self {
        Self {
            dag,
            wallet,
            miner,
            mempool,
            utxos,
            metrics,
            diagnostics,
            p2p_command_sender,
        }
    }
}

#[async_trait::async_trait]
impl MiningAdapter for NodeMiningAdapter {
    type Block = QantoBlock;
    type Error = QantoDAGError;

    async fn pending_transactions_len(&self) -> usize {
        // Opportunistically prune stale transactions before reporting count.
        // This keeps adapter state aligned with mempool TTL semantics in tests.
        let guard = self.mempool.write().await;
        guard.prune_old_transactions().await;
        guard.pending_len_sync()
    }

    async fn create_candidate_block(&self, difficulty: f64) -> Result<Self::Block, Self::Error> {
        let miner_address = self.wallet.address();
        let chain_id: u32 = 0;

        let private_key = self.dag.get_private_key().await?;
        let public_key = private_key.public_key();

        let mut block = self
            .dag
            .create_candidate_block(
                &private_key,
                &public_key,
                &miner_address,
                &self.mempool,
                &self.utxos,
                chain_id,
                &self.miner,
                None, // homomorphic_public_key
                None, // parents_override
            )
            .await?;

        if self.startup_mode() {
            block.difficulty = difficulty;
        } else {
            let consensus_difficulty = self.dag.get_current_difficulty().await;
            let is_dev_mode = std::env::var("QANTO_DEV_MODE")
                .map(|v| v == "1")
                .unwrap_or(false);
            let diff_ratio = if consensus_difficulty > 0.0 {
                ((difficulty - consensus_difficulty).abs()) / consensus_difficulty
            } else {
                0.0
            };
            if diff_ratio > 0.10 && !is_dev_mode {
                warn!(
                    "Overriding suggested difficulty ({:.6}) with ASERT consensus ({:.6})",
                    difficulty, consensus_difficulty
                );
                block.difficulty = consensus_difficulty;
            } else if is_dev_mode {
                block.difficulty = difficulty;
            } else {
                block.difficulty = consensus_difficulty;
            }
        }
        Ok(block)
    }

    fn set_block_difficulty(&self, block: &mut Self::Block, difficulty: f64) {
        block.difficulty = difficulty;
    }

    async fn mine_block(
        &self,
        block: &mut Self::Block,
        shutdown: &CancellationToken,
    ) -> Result<Self::Block, Self::Error> {
        let mut candidate = block.clone();

        let target_hash = self.get_block_target(&candidate);
        let header_hash = self.get_block_header_hash(&candidate);
        let height = self.block_height(&candidate);

        let config = PowConfig {
            target_block_time_sec: self.miner.target_block_time(),
            use_gpu: self.miner.use_gpu(),
            threads: self.miner.threads(),
            nonce_start: None,
        };

        let qanhash_miner = self.miner.qanhash_miner();
        let token = shutdown.clone();
        let start_time = std::time::Instant::now();

        // Execute mining directly (solve_pow handles blocking tasks internally)
        let res = solve_pow(
            &qanhash_miner,
            header_hash,
            target_hash,
            height,
            &config,
            token,
        )
        .await
        .map_err(|e| QantoDAGError::MinerError(e.to_string()))?;

        match res {
            CoreMiningResult::Found {
                nonce,
                hash,
                hashes_tried,
            } => {
                // Record success metrics with actual mining duration.
                let duration = start_time.elapsed();
                self.metrics.record_success(duration, 0).await;

                self.set_block_nonce(&mut candidate, nonce, hashes_tried);

                // --- Celebration Logic ---
                let effort = u64::from_be_bytes([
                    hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
                ]);

                // Force print the block to stdout for user visibility (ASCII Art)
                println!("{}", candidate);

                let celebration_params = MiningCelebrationParams {
                    block_height: candidate.height,
                    block_hash: hex::encode(&hash[..8]),
                    nonce,
                    difficulty: candidate.difficulty,
                    transactions_count: candidate.transactions.len(),
                    mining_time: duration,
                    effort,
                    total_blocks_mined: 1,
                    chain_id: 0,
                    block_reward: 2_500_000,
                    compact: false,
                };

                on_block_mined(celebration_params, self.miner.logging_config());
                // -------------------------

                Ok(candidate)
            }
            CoreMiningResult::Cancelled { .. } => {
                Err(QantoDAGError::MinerError("Mining cancelled".into()))
            }
            CoreMiningResult::Timeout { .. } => {
                Err(QantoDAGError::MinerError("Mining timed out".into()))
            }
        }
    }

    async fn add_block(&self, block: &Self::Block) -> Result<bool, Self::Error> {
        self.dag
            .add_block(block.clone(), &self.utxos, Some(&self.mempool), None)
            .await
    }

    async fn post_add_block(&self, block: &Self::Block) -> Result<(), Self::Error> {
        let tx_count = block.transactions.len();
        // Remove processed txs from mempool.
        {
            let mempool_guard = self.mempool.write().await;
            mempool_guard.remove_transactions(&block.transactions).await;
        }

        // Update UTXO set (simplified model; inputs spent, outputs created).
        const MAX_UTXO_RETRIES: u32 = 3;
        let mut retries = 0;
        while retries < MAX_UTXO_RETRIES {
            let mut utxos_guard = self.utxos.write().await;
            let update_result = async {
                for tx in &block.transactions {
                    for input in &tx.inputs {
                        utxos_guard.remove(&input.tx_id);
                    }
                    for (index, output) in tx.outputs.iter().enumerate() {
                        let utxo_id = format!("{}:{}", tx.id, index);
                        let utxo = UTXO {
                            address: output.address.clone(),
                            amount: output.amount,
                            tx_id: tx.id.clone(),
                            output_index: index as u32,
                            explorer_link: format!("https://explorer.qanto.org/tx/{}", tx.id),
                        };
                        utxos_guard.insert(utxo_id, utxo);
                    }
                }
                Ok::<(), String>(())
            }
            .await;

            match update_result {
                Ok(_) => {
                    debug!("✅ Updated UTXO set for block #{}", block.height);
                    break;
                }
                Err(e) => {
                    retries += 1;
                    if retries < MAX_UTXO_RETRIES {
                        warn!(
                            "Failed to update UTXO set (attempt {}/{}) for block #{}: {}. Retrying...",
                            retries,
                            MAX_UTXO_RETRIES,
                            block.height,
                            e
                        );
                        drop(utxos_guard);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    } else {
                        warn!(
                            "Failed to update UTXO set after {} attempts for block #{}: {}",
                            MAX_UTXO_RETRIES, block.height, e
                        );
                        break;
                    }
                }
            }
        }

        // NOTE: Success metrics are recorded in `mine_block` with precise duration.
        // Avoid double counting here.
        debug!("✅ Removed {} transactions from mempool", tx_count);
        Ok(())
    }

    async fn record_attempt(&self, report: MiningAttemptReport) {
        if let Some(diag) = &self.diagnostics {
            let _ = diag
                .record_mining_attempt(crate::diagnostics::MiningAttemptParams {
                    attempt_id: report.attempt_id,
                    difficulty: report.difficulty,
                    target_block_time: report.target_block_time_ms,
                    start_time: report.start_time,
                    nonce: report.nonce,
                    hash_rate: report.hash_rate,
                    success: report.success,
                    error_message: report.error_message.clone(),
                })
                .await;
        }
    }

    async fn record_success_metrics(&self, mining_duration: Duration, retries: u32) {
        self.metrics.record_success(mining_duration, retries).await;
    }

    async fn record_failure_metrics(&self, _failure_label: &'static str) {
        self.metrics
            .record_failure(MiningFailureType::MiningError)
            .await;
    }

    fn block_height(&self, block: &Self::Block) -> u64 {
        block.height
    }

    fn block_id(&self, block: &Self::Block) -> String {
        block.id.clone()
    }

    fn tx_count(&self, block: &Self::Block) -> usize {
        block.transactions.len()
    }

    async fn connected_peers_len(&self) -> usize {
        if let Some(sender) = &self.p2p_command_sender {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let cmd = P2PCommand::GetConnectedPeers {
                response_sender: tx,
            };
            if sender.send(cmd).await.is_ok() {
                if let Ok(peers) = rx.await {
                    return peers.len();
                }
            }
        }
        0
    }

    fn use_gpu(&self) -> bool {
        self.miner.use_gpu()
    }

    fn mining_threads(&self) -> usize {
        self.miner.threads()
    }

    fn get_block_target(&self, block: &Self::Block) -> [u8; 32] {
        let t = block.target.as_ref().expect("Missing header target");
        let v = hex::decode(t).expect("Invalid target encoding");
        let mut arr = [0u8; 32];
        let copy_len = 32.min(v.len());
        arr[32 - copy_len..].copy_from_slice(&v[v.len() - copy_len..]);
        arr
    }

    fn get_block_header_hash(&self, block: &Self::Block) -> [u8; 32] {
        let mut header_data = Vec::with_capacity(280 + (block.parents.len() * 64));
        header_data.extend_from_slice(&block.timestamp.to_be_bytes());
        header_data.extend_from_slice(&block.chain_id.to_be_bytes());
        header_data.extend_from_slice(&block.height.to_be_bytes());
        header_data.extend_from_slice(&block.difficulty.to_be_bytes());
        header_data.extend_from_slice(block.merkle_root.as_bytes());
        header_data.extend_from_slice(block.validator.as_bytes());
        header_data.extend_from_slice(block.miner.as_bytes());
        for parent in &block.parents {
            header_data.extend_from_slice(parent.as_bytes());
        }

        let header_hash_qanto = qanto_hash(&header_data);
        *header_hash_qanto.as_bytes()
    }

    fn set_block_nonce(&self, block: &mut Self::Block, nonce: u64, effort: u64) {
        block.nonce = nonce;
        block.effort = effort;
    }

    async fn get_current_difficulty(&self) -> Option<f64> {
        Some(self.dag.get_current_difficulty().await)
    }

    fn startup_mode(&self) -> bool {
        false
    }
}
