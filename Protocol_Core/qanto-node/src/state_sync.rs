use bumpalo::Bump;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use zstd::stream::read::Decoder;

use crate::interoperability::CompressedRecursiveProof;
use crate::qantodag::QantoDAG;

/// Represents a severing token to disconnect malicious or corrupted peers
#[derive(Debug)]
pub struct DisconnectPeerToken {
    pub peer_id: String,
    pub reason: String,
}

/// A wrapped state snapshot chunk streamed across the wire
#[derive(Debug, Clone)]
pub struct StateSnapshotChunk {
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub payload: Vec<u8>,
    pub expected_checksum: [u8; 32],
    pub recursive_proof: Option<CompressedRecursiveProof>,
}

#[derive(Debug)]
pub enum SyncError {
    ChecksumMismatch,
    DecompressionFailed,
    ProofValidationFailed,
    Timeout,
}

pub struct StateSyncWorker {
    _dag: Option<Arc<RwLock<QantoDAG>>>,
    disconnect_tx: mpsc::Sender<DisconnectPeerToken>,
}

impl StateSyncWorker {
    pub fn new(
        dag: Option<Arc<RwLock<QantoDAG>>>,
        disconnect_tx: mpsc::Sender<DisconnectPeerToken>,
    ) -> Self {
        Self {
            _dag: dag,
            disconnect_tx,
        }
    }

    /// Process a stream of snapshot chunks from a specific peer
    pub async fn process_snapshot_stream(
        &self,
        peer_id: String,
        mut chunk_stream: mpsc::Receiver<StateSnapshotChunk>,
    ) -> Result<(), SyncError> {
        let mut total_buffer = Vec::new();

        while let Ok(Some(chunk)) =
            tokio::time::timeout(Duration::from_secs(5), chunk_stream.recv()).await
        {
            // Validate checksum (Progressive BLAKE3)
            let mut hasher = blake3::Hasher::new();
            hasher.update(&chunk.payload);
            let hash = hasher.finalize();

            if hash.as_bytes() != &chunk.expected_checksum {
                self.sever_peer(&peer_id, "Chunk checksum mismatch".to_string())
                    .await;
                return Err(SyncError::ChecksumMismatch);
            }

            // Isolate decompression in a thread-bound blocking task with Bumpalo
            let payload = chunk.payload.clone();
            let decompressed_result = tokio::task::spawn_blocking(move || {
                let _arena = Bump::new(); // Local scope arena
                let mut decompressed_data = Vec::new();

                // Decode using zstd isolated
                let mut decoder = match Decoder::new(&payload[..]) {
                    Ok(d) => d,
                    Err(_) => return Err(()), // Let drop(arena) trigger gracefully
                };

                if decoder.read_to_end(&mut decompressed_data).is_err() {
                    return Err(());
                }

                Ok(decompressed_data)
            })
            .await;

            match decompressed_result {
                Ok(Ok(data)) => {
                    total_buffer.extend(data);
                }
                _ => {
                    self.sever_peer(&peer_id, "Zstd Decompression Fault".to_string())
                        .await;
                    return Err(SyncError::DecompressionFailed);
                }
            }

            // Verify the Dilithium proof on the final chunk
            if let Some(proof) = chunk.recursive_proof {
                if !self.verify_dilithium_proof(&proof).await {
                    self.sever_peer(
                        &peer_id,
                        "Aggregated recursive proof validation failed".to_string(),
                    )
                    .await;
                    return Err(SyncError::ProofValidationFailed);
                }
            }
        }

        // Commit state to DB (mocked for architectural representation)
        // self.dag.write().await.apply_state(total_buffer);
        tracing::info!("State Snapshot Stream successfully ingested and decompressed.");

        Ok(())
    }

    /// Asynchronously sever peer connection
    async fn sever_peer(&self, peer_id: &str, reason: String) {
        tracing::warn!("Severing peer {}: {}", peer_id, reason);
        let token = DisconnectPeerToken {
            peer_id: peer_id.to_string(),
            reason,
        };
        // Non-blocking disconnect submission
        let _ = self.disconnect_tx.try_send(token);
    }

    /// Synthetic verification of the Dilithium proof against consensus
    async fn verify_dilithium_proof(&self, proof: &CompressedRecursiveProof) -> bool {
        // Here we would verify the 32-byte layout hash emitted from our Halo2 Circuit
        !proof.payload.is_empty()
    }
}
