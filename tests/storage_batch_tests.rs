use qanto::qanto_storage::{LogEntry, QantoStorage, StorageConfig, WriteBatch};
use tempfile::TempDir;

#[test]
fn test_storage_batch_processing() {
    // Create temporary directory for test storage
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let storage_path = temp_dir.path().join("test_storage");

    // Initialize storage with proper config
    let storage_config = StorageConfig {
        data_dir: storage_path,
        max_file_size: 64 * 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024,
        compaction_threshold: 10,
        max_open_files: 100,
        ..StorageConfig::default()
    };
    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    // Create a batch with multiple operations
    let mut batch = WriteBatch::new();

    // Add some test operations
    batch.operations.push(LogEntry::Put {
        key: b"key1".to_vec(),
        value: b"value1".to_vec(),
    });
    batch.operations.push(LogEntry::Put {
        key: b"key2".to_vec(),
        value: b"value2".to_vec(),
    });
    batch.operations.push(LogEntry::Put {
        key: b"key3".to_vec(),
        value: b"value3".to_vec(),
    });

    // Execute the batch
    storage.write_batch(batch).expect("Failed to execute batch");

    // Verify all keys were stored correctly
    let value1 = storage.get(b"key1".as_ref()).expect("Failed to get key1");
    assert_eq!(value1, Some(b"value1".to_vec()));

    let value2 = storage.get(b"key2".as_ref()).expect("Failed to get key2");
    assert_eq!(value2, Some(b"value2".to_vec()));

    let value3 = storage.get(b"key3".as_ref()).expect("Failed to get key3");
    assert_eq!(value3, Some(b"value3".to_vec()));
}

#[test]
fn test_empty_batch_processing() {
    // Create temporary directory for test storage
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let storage_path = temp_dir.path().join("test_storage");

    // Initialize storage with proper config
    let storage_config = StorageConfig {
        data_dir: storage_path,
        max_file_size: 64 * 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024,
        compaction_threshold: 10,
        max_open_files: 100,
        ..StorageConfig::default()
    };
    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    // Create an empty batch
    let batch = WriteBatch::new();

    // Execute the empty batch (should not fail)
    storage
        .write_batch(batch)
        .expect("Failed to execute empty batch");
}

#[test]
fn test_batch_with_mixed_operations() {
    // Create temporary directory for test storage
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let storage_path = temp_dir.path().join("test_storage");

    // Initialize storage with proper config
    let storage_config = StorageConfig {
        data_dir: storage_path,
        max_file_size: 64 * 1024 * 1024,
        compression_enabled: false,
        encryption_enabled: false,
        wal_enabled: true,
        sync_writes: false,
        cache_size: 1024 * 1024,
        compaction_threshold: 10,
        max_open_files: 100,
        ..StorageConfig::default()
    };
    let storage = QantoStorage::new(storage_config).expect("Failed to create storage");

    // First, put some initial data
    storage
        .put(b"existing_key".to_vec(), b"existing_value".to_vec())
        .expect("Failed to put initial data");

    // Create a batch with mixed operations
    let mut batch = WriteBatch::new();
    batch.operations.push(LogEntry::Put {
        key: b"new_key".to_vec(),
        value: b"new_value".to_vec(),
    });
    batch.operations.push(LogEntry::Delete {
        key: b"existing_key".to_vec(),
    });
    batch.operations.push(LogEntry::Put {
        key: b"another_key".to_vec(),
        value: b"another_value".to_vec(),
    });

    // Execute the batch
    storage
        .write_batch(batch)
        .expect("Failed to execute mixed batch");

    // Verify the operations were applied correctly
    let new_value = storage
        .get(b"new_key".as_ref())
        .expect("Failed to get new_key");
    assert_eq!(new_value, Some(b"new_value".to_vec()));

    let deleted_value = storage
        .get(b"existing_key".as_ref())
        .expect("Failed to check deleted key");
    assert_eq!(deleted_value, None);

    let another_value = storage
        .get(b"another_key".as_ref())
        .expect("Failed to get another_key");
    assert_eq!(another_value, Some(b"another_value".to_vec()));
}
