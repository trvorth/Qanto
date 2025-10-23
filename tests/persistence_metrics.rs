use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use qanto::metrics::get_global_metrics;
use qanto::persistence::PersistenceWriter;
use qanto::qanto_storage::{QantoStorage, StorageConfig};

// Integration test: enqueue jobs to PersistenceWriter, then verify Prometheus metrics reflect updates.
#[test]
fn persistence_metrics_update_and_prometheus_export() {
    // Create a temporary storage directory for the test
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("qanto_persistence_test_{unique}"));
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    // Minimal storage configuration; values chosen to be reasonable for tests
    let storage_config = StorageConfig {
        data_dir: temp_dir.clone(),
        max_file_size: 64 * 1024 * 1024, // 64 MB
        cache_size: 8 * 1024 * 1024,     // 8 MB
        compression_enabled: true,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        compaction_threshold: 0.7,
        max_open_files: 128,
    };

    let db = QantoStorage::new(storage_config).expect("Failed to initialize QantoStorage");
    let writer = PersistenceWriter::new(Arc::new(db), 4096);

    // Reset global metrics so we have a clean baseline
    let metrics = get_global_metrics();
    metrics.reset();

    let base_batches = metrics.persistence_batches.load(Ordering::Relaxed);
    let base_overflows = metrics.persistence_overflows.load(Ordering::Relaxed);

    // Attempt to trigger an overflow by job count (65 small jobs).
    // Due to concurrency, we retry a few times to increase determinism.
    let mut observed_overflow = false;
    for attempt in 0..5 {
        for i in 0..65u32 {
            let key = format!("test-key-{attempt}-{i}").into_bytes();
            let value = vec![0u8; 16];
            writer
                .enqueue_put(key, value)
                .expect("Failed to enqueue put job");
        }

        // Poll metrics until at least one new batch is observed or timeout
        let start = Instant::now();
        loop {
            let batches = metrics.persistence_batches.load(Ordering::Relaxed);
            let overflows = metrics.persistence_overflows.load(Ordering::Relaxed);
            let last_ops = metrics.persistence_last_batch_ops.load(Ordering::Relaxed);

            // Expect at least one batch and a 64-op batch in this round
            if batches > base_batches && last_ops == 64 {
                if overflows > base_overflows {
                    observed_overflow = true;
                }
                break;
            }

            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Timeout waiting for batch in attempt {attempt}: batches={batches}, overflows={overflows}, last_ops={last_ops}"
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        if observed_overflow {
            break;
        }
    }

    // If not observed yet, try to trigger overflow by bytes: enqueue many ~900KB values
    if !observed_overflow {
        let chunk = vec![42u8; 900 * 1024]; // below LARGE_VALUE_THRESHOLD (1MB)
        for i in 0..10u32 {
            // >= ~9MB payload across jobs, should overflow BATCH_MAX_BYTES (8MB)
            let key = format!("fat-key-{i}").into_bytes();
            writer
                .enqueue_put(key, chunk.clone())
                .expect("Failed to enqueue put job (fat chunk)");
        }

        let start = Instant::now();
        loop {
            let overflows = metrics.persistence_overflows.load(Ordering::Relaxed);
            if overflows > base_overflows {
                observed_overflow = true;
                break;
            }
            if start.elapsed() > Duration::from_secs(10) {
                panic!(
                    "Timeout waiting for byte-overflow: overflows did not increase (base={base_overflows})"
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    assert!(
        observed_overflow,
        "Expected at least one overflow to be observed"
    );

    // "Scrape" the Prometheus metrics export and assert expected values
    let prom = metrics.export_prometheus();

    fn find_gauge(prom: &str, name: &str) -> Option<u64> {
        for line in prom.lines() {
            if line.starts_with(name) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(v) = parts[1].parse::<u64>() {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    let m_batches = find_gauge(&prom, "qanto_persistence_batches_total")
        .expect("Missing qanto_persistence_batches_total in Prometheus export");
    let m_overflows = find_gauge(&prom, "qanto_persistence_overflows_total")
        .expect("Missing qanto_persistence_overflows_total in Prometheus export");
    let m_last_ops = find_gauge(&prom, "qanto_persistence_last_batch_ops")
        .expect("Missing qanto_persistence_last_batch_ops in Prometheus export");

    // We expect at least one batch, and overflows increased compared to baseline
    assert!(
        m_batches > base_batches,
        "Expected at least one new batch, got {m_batches} (base={base_batches})"
    );
    assert!(
        m_overflows > base_overflows,
        "Expected overflows to increase, got {m_overflows} (base={base_overflows})"
    );
    assert!(
        m_last_ops == 64 || m_last_ops == 1,
        "Expected last batch ops to be 64 (full) or 1 (single), got {m_last_ops}"
    );

    // Cleanup: drop writer and remove temp dir
    drop(writer);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
