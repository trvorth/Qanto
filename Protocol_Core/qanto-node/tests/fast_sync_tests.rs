// tests/fast_sync_tests.rs

use tokio::sync::mpsc;

use qanto::interoperability::CompressedRecursiveProof;
use qanto::state_sync::{DisconnectPeerToken, StateSnapshotChunk, StateSyncWorker};

#[tokio::test]
async fn test_state_sync_corrupted_payload_severing() {
    let (disconnect_tx, mut disconnect_rx) = mpsc::channel::<DisconnectPeerToken>(10);

    let worker = StateSyncWorker::new(None, disconnect_tx);

    let (chunk_tx, chunk_rx) = mpsc::channel::<StateSnapshotChunk>(10);

    // Create a chunk with a valid checksum but malformed zstd compression payload
    let malformed_payload = vec![0xFF, 0x00, 0xBA, 0xAD, 0xF0, 0x0D]; // Not valid zstd bytes

    let mut hasher = blake3::Hasher::new();
    hasher.update(&malformed_payload);
    let expected_checksum: [u8; 32] = hasher.finalize().into();

    let chunk = StateSnapshotChunk {
        chunk_index: 0,
        total_chunks: 1,
        payload: malformed_payload,
        expected_checksum,
        recursive_proof: Some(CompressedRecursiveProof {
            payload: vec![1, 2, 3], // Synthetic payload
            num_signatures: 1,
            generation_time_ms: 100,
        }),
    };

    // Send chunk to worker
    chunk_tx.send(chunk).await.unwrap();
    drop(chunk_tx); // close stream

    let peer_id = "malicious_peer_123".to_string();

    // Process stream (should fail due to zstd decompression fault)
    let result = worker
        .process_snapshot_stream(peer_id.clone(), chunk_rx)
        .await;

    assert!(result.is_err(), "Expected decompression error");

    // Assert that the worker emitted a disconnect token back to the central P2P mesh
    let token = disconnect_rx
        .recv()
        .await
        .expect("Disconnect token not received");

    assert_eq!(token.peer_id, peer_id);
    assert_eq!(token.reason, "Zstd Decompression Fault");
}
