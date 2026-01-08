use std::sync::Arc;
use std::time::Duration;

use qanto::metrics::get_global_metrics;
use qanto::persistence::tip_key;
use qanto::persistence::PersistenceWriter;
use qanto::persistence::BALANCES_KEY_PREFIX;
use qanto::qanto_storage::{QantoStorage, StorageConfig, WriteBatch};

/// Verify that enqueue_batch writes all operations atomically and updates metrics.
#[test]
fn persistence_batch_job_writes_and_metrics_update() {
    // Create a temporary storage directory for this test
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("qanto_persistence_batch_{unique}"));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    // Minimal storage configuration suitable for tests
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 64 * 1024 * 1024,
        cache_size: 8 * 1024 * 1024,
        compression_enabled: true,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        compaction_threshold: 10,
        max_open_files: 128,
        ..StorageConfig::default()
    };

    // Important: share the SAME storage instance with the writer via Arc.
    // QantoStorage::clone() creates a fresh instance, so using Arc avoids divergence.
    let db =
        Arc::new(QantoStorage::new(storage_config).expect("Failed to initialize QantoStorage"));
    let writer = PersistenceWriter::new(Arc::clone(&db), 4096);

    // Reset metrics baseline
    let metrics = get_global_metrics();
    metrics.reset();
    let base_batches = metrics
        .persistence_batches
        .load(std::sync::atomic::Ordering::Relaxed);

    // Pre-insert a key that the batch will delete to validate both put+delete are applied
    let old_tip = tip_key(0, "old-parent");
    db.put(old_tip.clone(), b"1".to_vec())
        .expect("Failed to pre-insert old tip");

    // Prepare a batch with 3 operations:
    // - delete old tip
    // - put new tip
    // - put a dummy balance key to simulate additional DAG-index writes
    let mut batch = WriteBatch::new();
    batch.delete(old_tip.clone());
    let new_tip = tip_key(0, "new-block");
    batch.put(new_tip.clone(), b"1".to_vec());
    let balance_key = {
        let mut k = String::with_capacity(BALANCES_KEY_PREFIX.len() + 8);
        k.push_str(BALANCES_KEY_PREFIX);
        k.push_str("deadbeef");
        k.into_bytes()
    };
    batch.put(balance_key.clone(), 1234u64.to_le_bytes().to_vec());

    writer
        .enqueue_batch(batch)
        .expect("Failed to enqueue batch job");

    // Gracefully shut down the writer to ensure the batch is flushed
    // Use a small Tokio runtime for the async shutdown
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("Failed to build tokio runtime");
    rt.block_on(async {
        writer
            .shutdown()
            .await
            .expect("Persistence shutdown failed");
    });

    // Verify storage reflects batch application
    assert!(!db
        .contains_key(&old_tip)
        .expect("contains_key failed for old_tip"));
    assert!(db
        .contains_key(&new_tip)
        .expect("contains_key failed for new_tip"));
    assert!(db
        .contains_key(&balance_key)
        .expect("contains_key failed for balance_key"));

    // Verify metrics updated for batch execution
    let batches = metrics
        .persistence_batches
        .load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        batches > base_batches,
        "Expected persistence_batches to increase: {batches} <= {base_batches}"
    );

    // Cleanup: drop writer/db and remove temp dir
    drop(writer);
    drop(db);
    std::thread::sleep(Duration::from_millis(50));
    let _ = std::fs::remove_dir_all(&temp_dir);
}
