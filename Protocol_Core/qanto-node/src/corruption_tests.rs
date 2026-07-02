use qanto_core::qanto_storage::{QantoStorage, StorageConfig};
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use tempfile::TempDir;

#[test]
fn test_wal_tail_corruption_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    // 1. Create a config with WAL enabled and sync writes
    let config = StorageConfig {
        data_dir: path.to_path_buf(),
        wal_enabled: true,
        sync_writes: true,
        enable_async_io: false,
        ..StorageConfig::default()
    };

    // 2. Open storage and write some keys
    {
        let storage = QantoStorage::new(config.clone()).unwrap();
        storage.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        storage.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();
        // Close/sync is implicitly executed when we drop storage, but we can flush to be safe
        storage.sync().unwrap();
    }

    // 3. Corrupt the WAL by appending garbage bytes
    let wal_path = path.join("wal.log");
    assert!(wal_path.exists());
    {
        let mut file = OpenOptions::new().append(true).open(&wal_path).unwrap();
        file.write_all(b"GARBAGE_BYTES_AT_THE_END_OF_THE_FILE")
            .unwrap();
    }

    // 4. Reopen and verify that:
    //    - Recovery succeeds (doesn't crash or return error)
    //    - key1 and key2 are still read correctly
    let storage2 = QantoStorage::new(config.clone()).unwrap();
    assert_eq!(storage2.get(b"key1").unwrap(), Some(b"value1".to_vec()));
    assert_eq!(storage2.get(b"key2").unwrap(), Some(b"value2".to_vec()));
}

#[test]
fn test_wal_mid_record_corruption_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    let config = StorageConfig {
        data_dir: path.to_path_buf(),
        wal_enabled: true,
        sync_writes: true,
        enable_async_io: false,
        ..StorageConfig::default()
    };

    // Write keys
    {
        let storage = QantoStorage::new(config.clone()).unwrap();
        storage.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        storage.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();
        storage.sync().unwrap();
    }

    // Corrupt the WAL by truncating the last few bytes (simulating partial write/crash mid-record)
    let wal_path = path.join("wal.log");
    let metadata = fs::metadata(&wal_path).unwrap();
    let size = metadata.len();
    assert!(size > 10);
    // Truncate last 5 bytes
    let file = OpenOptions::new().write(true).open(&wal_path).unwrap();
    file.set_len(size - 5).unwrap();

    // Reopen. It should recover key1, key2 might be lost or partially recovered, but it must not crash!
    let _storage2 = QantoStorage::new(config.clone()).unwrap();
}

#[test]
fn test_segment_file_corruption_detection() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    let config = StorageConfig {
        data_dir: path.to_path_buf(),
        wal_enabled: true,
        enable_async_io: false,
        ..StorageConfig::default()
    };

    // Write keys and flush to segment file
    {
        let storage = QantoStorage::new(config.clone()).unwrap();
        storage.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        storage.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();
        storage.flush().unwrap(); // Flushes memtable to segment file
    }

    // Verify segment files exist
    let mut segment_file_path = None;
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.starts_with("segment_") && name.ends_with(".qdb") {
            segment_file_path = Some(entry.path());
            break;
        }
    }
    let segment_path = segment_file_path.expect("Segment file should have been flushed");

    // Overwrite the segment header with garbage (tamper with magic number)
    {
        let mut file = OpenOptions::new().write(true).open(&segment_path).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(b"BAD!").unwrap();
    }

    // Delete the WAL so it doesn't replay and restore the keys
    let wal_path = path.join("wal.log");
    if wal_path.exists() {
        fs::remove_file(wal_path).unwrap();
    }

    // Try to open storage. Under the hood, segment index load will print a warning and skip the corrupted segment.
    // Let's assert that the database still opens but keys from the corrupted segment are missing.
    let storage2 = QantoStorage::new(config.clone()).unwrap();
    assert_eq!(storage2.get(b"key1").unwrap(), None);
}

#[test]
fn test_crash_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    let config = StorageConfig {
        data_dir: path.to_path_buf(),
        wal_enabled: true,
        sync_writes: true,
        enable_async_io: false,
        ..StorageConfig::default()
    };

    // Write keys to memory/WAL without calling close() or normal checkpoint.
    // Drop the storage object abruptly.
    {
        let storage = QantoStorage::new(config.clone()).unwrap();
        storage
            .put(b"crash_key".to_vec(), b"crash_value".to_vec())
            .unwrap();
        // Do not flush! It remains in WAL and memtable.
        // Drop here.
    }

    // Reopen and check if WAL replayed the crash_key.
    let storage2 = QantoStorage::new(config.clone()).unwrap();
    assert_eq!(
        storage2.get(b"crash_key").unwrap(),
        Some(b"crash_value".to_vec())
    );
}
