use std::sync::Arc;
use std::thread;

use crate::increment_metric;
use crate::qanto_storage::{QantoStorage, QantoStorageError, WriteBatch};
use crate::set_metric;
use crossbeam::channel::{bounded, Receiver, Sender, TryRecvError};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

/// Genesis keys and helpers
pub const GENESIS_BLOCK_ID_KEY: &[u8] = b"genesis_block_id";
pub const GENESIS_BLOCK_ID_PREFIX: &str = "genesis_block_id:";

pub const TIPS_KEY_PREFIX: &str = "tips:";

// New: balances key prefix and helpers
pub const BALANCES_KEY_PREFIX: &str = "balance:";

pub fn balance_key(address: &str) -> Vec<u8> {
    let mut key = String::with_capacity(BALANCES_KEY_PREFIX.len() + address.len());
    key.push_str(BALANCES_KEY_PREFIX);
    key.push_str(address);
    key.into_bytes()
}

pub fn encode_balance(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

pub fn decode_balance(bytes: &[u8]) -> Result<u64, String> {
    if bytes.len() != 8 {
        return Err(format!("Invalid balance value length: {}", bytes.len()));
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(arr))
}

pub fn tips_prefix(chain_id: u32) -> Vec<u8> {
    let mut prefix = String::with_capacity(TIPS_KEY_PREFIX.len() + 11);
    prefix.push_str(TIPS_KEY_PREFIX);
    prefix.push_str(&chain_id.to_string());
    prefix.push(':');
    prefix.into_bytes()
}

pub fn tip_key(chain_id: u32, block_id: &str) -> Vec<u8> {
    let mut key = String::with_capacity(TIPS_KEY_PREFIX.len() + 11 + block_id.len());
    key.push_str(TIPS_KEY_PREFIX);
    key.push_str(&chain_id.to_string());
    key.push(':');
    key.push_str(block_id);
    key.into_bytes()
}

pub fn genesis_id_key(chain_id: u32) -> Vec<u8> {
    let mut key = String::with_capacity(GENESIS_BLOCK_ID_PREFIX.len() + 10);
    key.push_str(GENESIS_BLOCK_ID_PREFIX);
    key.push_str(&chain_id.to_string());
    key.into_bytes()
}

pub fn has_genesis_block(db: &QantoStorage) -> Result<bool, QantoStorageError> {
    // Check legacy global key first for backward compatibility
    if db.contains_key(GENESIS_BLOCK_ID_KEY)? {
        return Ok(true);
    }
    // Also check the chain-specific genesis marker for chain 0
    if db.contains_key(&genesis_id_key(0))? {
        return Ok(true);
    }
    Ok(false)
}

// --- New: UTXO persistence helpers ---
use crate::types::UTXO;

pub const UTXO_KEY_PREFIX: &str = "utxo:";

pub fn utxo_key(utxo_id: &str) -> Vec<u8> {
    let mut key = String::with_capacity(UTXO_KEY_PREFIX.len() + utxo_id.len());
    key.push_str(UTXO_KEY_PREFIX);
    key.push_str(utxo_id);
    key.into_bytes()
}

pub fn utxos_prefix() -> Vec<u8> {
    UTXO_KEY_PREFIX.as_bytes().to_vec()
}

pub fn encode_utxo(utxo: &UTXO) -> Result<Vec<u8>, String> {
    serde_json::to_vec(utxo).map_err(|e| format!("Failed to encode UTXO: {e}"))
}

pub fn decode_utxo(bytes: &[u8]) -> Result<UTXO, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("Failed to decode UTXO: {e}"))
}

/// A job representing a persistence operation to the storage engine.
pub enum PersistenceJob {
    /// Write a single key-value pair to storage.
    Put(Vec<u8>, Vec<u8>),
    /// Delete a single key from storage.
    Delete(Vec<u8>),
    /// Request graceful shutdown. The worker will flush pending jobs and exit,
    /// then acknowledge via the provided channel.
    Shutdown(oneshot::Sender<()>),
}

/// Asynchronous persistence writer that decouples storage I/O from hot paths.
#[derive(Debug)]
pub struct PersistenceWriter {
    sender: Sender<PersistenceJob>,
}

impl PersistenceWriter {
    /// Create a new persistence writer with a bounded queue and spawn a background thread.
    pub fn new(db: Arc<QantoStorage>, capacity: usize) -> Self {
        let (tx, rx): (Sender<PersistenceJob>, Receiver<PersistenceJob>) = bounded(capacity);

        // Spawn a dedicated background thread to process persistence jobs.
        let db_clone = db.clone();
        thread::Builder::new()
            .name("qanto-persistence-writer".to_string())
            .spawn(move || {
                info!("Persistence writer thread started");

                // Batching heuristics: avoid bundling very large values to reduce memory pressure.
                const BATCH_MAX_JOBS: usize = 64; // conservative to limit WAL transaction size
                const BATCH_MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MB cap for batch payloads
                const LARGE_VALUE_THRESHOLD: usize = 1024 * 1024; // 1 MB considered large

                // Main worker loop
                'worker: loop {
                    // Block until at least one job arrives
                    let first_job = match rx.recv() {
                        Ok(job) => job,
                        Err(_) => {
                            warn!("Persistence writer thread exiting: receiver disconnected");
                            break;
                        }
                    };

                    // Handle shutdown request: drain any remaining jobs, flush, acknowledge, and exit
                    if let PersistenceJob::Shutdown(ack_tx) = first_job {
                        // Drain any remaining jobs and process them in constrained batches
                        let mut batch = WriteBatch::new();
                        let mut batch_bytes = 0usize;
                        let mut ops = 0usize;

                        // Drain immediately available jobs
                        loop {
                            match rx.try_recv() {
                                Ok(job) => {
                                    match job {
                                        PersistenceJob::Put(key, value) => {
                                            let job_bytes = key.len() + value.len();
                                            batch.put(key, value);
                                            batch_bytes += job_bytes;
                                            ops += 1;
                                        }
                                        PersistenceJob::Delete(key) => {
                                            let job_bytes = key.len();
                                            batch.delete(key);
                                            batch_bytes += job_bytes;
                                            ops += 1;
                                        }
                                        PersistenceJob::Shutdown(_) => {
                                            // Ignore nested shutdowns; we'll exit after flushing
                                        }
                                    }
                                    // If batch is getting large, write it and start a new one
                                    if ops >= 64 || batch_bytes >= 8 * 1024 * 1024 {
                                        let _ = db_clone.write_batch(batch);
                                        batch = WriteBatch::new();
                                        batch_bytes = 0;
                                        ops = 0;
                                    }
                                }
                                Err(TryRecvError::Empty) => break,
                                Err(TryRecvError::Disconnected) => break,
                            }
                        }

                        // Flush remaining batch
                        if ops > 0 {
                            let _ = db_clone.write_batch(batch);
                        }
                        // Best-effort flush and sync to ensure durability
                        if let Err(e) = db_clone.flush() {
                            warn!("Persistence flush on shutdown failed: {}", e);
                        }
                        if let Err(e) = db_clone.sync() {
                            warn!("Persistence fsync on shutdown failed: {}", e);
                        }

                        // Acknowledge shutdown
                        let _ = ack_tx.send(());
                        info!("Persistence writer thread shutting down gracefully");
                        break;
                    }

                    // If the first job is large, write it individually and continue
                    match &first_job {
                        PersistenceJob::Put(key, value) if value.len() >= LARGE_VALUE_THRESHOLD => {
                            if let Err(e) = db_clone.put(key.clone(), value.clone()) {
                                error!("Persistence write failed: {}", e);
                            } else {
                                debug!("Persistence write succeeded (single large value)");
                            }
                            continue;
                        }
                        &PersistenceJob::Shutdown(_) => {
                            // Should have been handled above; keep exhaustive match
                            unreachable!("Shutdown already handled prior to large-value pre-check");
                        }
                        _ => {}
                    }

                    // Prepare a batch
                    let mut batch = WriteBatch::new();
                    let mut batch_bytes = 0usize;
                    let mut ops = 0usize;

                    // Add the first job
                    match &first_job {
                        PersistenceJob::Put(key, value) => {
                            batch.put(key.clone(), value.clone());
                            batch_bytes += key.len() + value.len();
                            ops += 1;
                        }
                        PersistenceJob::Delete(key) => {
                            batch.delete(key.clone());
                            batch_bytes += key.len();
                            ops += 1;
                        }
                        &PersistenceJob::Shutdown(_) => {
                            // Should have been handled above; keep exhaustive match
                            unreachable!("Shutdown already handled prior to batch assembly");
                        }
                    }

                    // Overflow buffer for jobs that exceed limits mid-batch
                    let mut overflow_job: Option<PersistenceJob> = None;

                    // Try to fill the batch up to limits
                    loop {
                        match rx.try_recv() {
                            Ok(job) => {
                                // If this job is a shutdown, acknowledge and exit immediately
                                if let PersistenceJob::Shutdown(ack_tx) = job {
                                    let _ = ack_tx.send(());
                                    info!("Persistence writer thread shutting down during batch fill");
                                    break 'worker;
                                }

                                // Estimate size impact
                                let job_bytes = match &job {
                                    PersistenceJob::Put(k, v) => k.len() + v.len(),
                                    PersistenceJob::Delete(k) => k.len(),
                                    _ => 0,
                                };

                                // Respect batch limits
                                if ops + 1 > BATCH_MAX_JOBS || batch_bytes + job_bytes > BATCH_MAX_BYTES {
                                    // Defer this job and process after current batch
                                    overflow_job = Some(job);
                                    break;
                                }

                                // Add job to batch
                                match &job {
                                    PersistenceJob::Put(key, value) => {
                                        batch.put(key.clone(), value.clone());
                                        batch_bytes += job_bytes;
                                        ops += 1;
                                    }
                                    PersistenceJob::Delete(key) => {
                                        batch.delete(key.clone());
                                        batch_bytes += job_bytes;
                                        ops += 1;
                                    }
                                    _ => {}
                                }
                            }
                            Err(TryRecvError::Empty) => {
                                // No more jobs immediately available; execute batch
                                break;
                            }
                            Err(TryRecvError::Disconnected) => {
                                warn!("Persistence writer receiver disconnected; flushing batch and exiting");
                                break;
                            }
                        }
                    }

                    // Execute batch
                    if ops == 1 {
                        // Single-operation optimization
                        match first_job {
                            PersistenceJob::Put(key, value) => {
                                if let Err(e) = db_clone.put(key.clone(), value.clone()) {
                                    error!("Persistence write failed: {}", e);
                                } else {
                                    debug!("Persistence write succeeded (single)");
                                }
                            }
                            PersistenceJob::Delete(key) => {
                                if let Err(e) = db_clone.delete(&key) {
                                    error!("Persistence delete failed: {}", e);
                                } else {
                                    debug!("Persistence delete succeeded (single)");
                                }
                            }
                            PersistenceJob::Shutdown(_) => {
                                // Should have been handled above; keep exhaustive match
                                unreachable!("Shutdown already handled prior to single-op path");
                            }
                        }
                        // Update batch metrics for single-op batch
                        increment_metric!(persistence_batches);
                        set_metric!(persistence_last_batch_ops, 1);
                        set_metric!(persistence_last_batch_bytes, batch_bytes as u64);
                    } else {
                        // Write the batch atomically
                        let ops_count = batch.len();
                        match db_clone.write_batch(batch) {
                            Ok(()) => debug!("Persistence batch write succeeded: {} ops, ~{} bytes", ops_count, batch_bytes),
                            Err(e) => error!("Persistence batch write failed: {}", e),
                        }
                        increment_metric!(persistence_batches);
                        set_metric!(persistence_last_batch_ops, ops_count as u64);
                        set_metric!(persistence_last_batch_bytes, batch_bytes as u64);
                    }

                    // Process any deferred overflow job independently
                    if let Some(job) = overflow_job.take() {
                        match job {
                            PersistenceJob::Put(key, value) => {
                                if let Err(e) = db_clone.put(key.clone(), value.clone()) {
                                    error!("Persistence overflow write failed: {}", e);
                                } else {
                                    debug!("Persistence overflow write succeeded");
                                }
                            }
                            PersistenceJob::Delete(key) => {
                                if let Err(e) = db_clone.delete(&key) {
                                    error!("Persistence overflow delete failed: {}", e);
                                } else {
                                    debug!("Persistence overflow delete succeeded");
                                }
                            }
                            PersistenceJob::Shutdown(ack_tx) => {
                                let _ = ack_tx.send(());
                                info!("Persistence writer thread shutting down while processing overflow job");
                                break 'worker;
                            }
                        }
                        increment_metric!(persistence_overflows);
                    }
                }
            })
            .expect("Failed to start persistence writer thread");

        Self { sender: tx }
    }

    /// Enqueue a put operation.
    pub fn enqueue_put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), String> {
        self.sender
            .send(PersistenceJob::Put(key, value))
            .map_err(|e| format!("Failed to enqueue put: {e}"))
    }

    /// Enqueue a delete operation.
    pub fn enqueue_delete(&self, key: Vec<u8>) -> Result<(), String> {
        self.sender
            .send(PersistenceJob::Delete(key))
            .map_err(|e| format!("Failed to enqueue delete: {e}"))
    }

    /// Request graceful shutdown and wait for acknowledgement with a timeout.
    pub async fn shutdown(&self) -> Result<(), String> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.sender
            .send(PersistenceJob::Shutdown(ack_tx))
            .map_err(|e| format!("Failed to enqueue shutdown: {e}"))?;

        // Wait up to 5 seconds for the persistence thread to flush and exit
        match tokio::time::timeout(std::time::Duration::from_secs(5), ack_rx).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err("Persistence shutdown acknowledgement channel closed".to_string()),
            Err(_) => Err("Persistence shutdown timed out".to_string()),
        }
    }
}
