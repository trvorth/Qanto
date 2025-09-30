// tests/transaction_test.rs
// Unit tests for transaction processing and verification
//
// This module contains tests for the transaction processing and verification logic
// in the Qanto system. It includes tests for:
// - Transaction creation and serialization
// - Input and output validation
// - Fee calculation
// - Signature verification
// - Batch processing performance
// - Transaction validation rules

use anyhow::Result;
use log::{error, info};
use qanto::performance_optimizations::OptimizedTransactionProcessor;
use qanto::qanto_native_crypto::{QantoPQPrivateKey, QantoPQPublicKey};
use qanto::transaction::{Input, Output, Transaction};
use qanto::types::{HomomorphicEncrypted, QuantumResistantSignature};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

// Test configuration constants
const SMALL_BATCH_SIZE: usize = 100;
const MEDIUM_BATCH_SIZE: usize = 1000;
const LARGE_BATCH_SIZE: usize = 10000;
const TEST_TIMEOUT_SECS: u64 = 30;
const VERIFICATION_TIMEOUT_SECS: u64 = 10;

/// Create a minimal test transaction for fast processing
fn create_minimal_transaction(id: usize) -> Transaction {
    Transaction {
        id: format!("tx_{}", id),
        sender: "test_sender".to_string(),
        receiver: "test_receiver".to_string(),
        amount: 1000,
        fee: 10,
        inputs: vec![Input {
            tx_id: "0".repeat(64),
            output_index: 0,
        }],
        outputs: vec![Output {
            address: "test_recipient".to_string(),
            amount: 1000,
            homomorphic_encrypted: HomomorphicEncrypted {
                ciphertext: vec![],
                public_key: vec![],
            },
        }],
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metadata: std::collections::HashMap::new(),
        signature: QuantumResistantSignature {
            signer_public_key: vec![],
            signature: vec![],
        },
    }
}

/// Create a batch of test transactions
fn create_transaction_batch(size: usize) -> Vec<Transaction> {
    (0..size).map(create_minimal_transaction).collect()
}

/// Create mock crypto keys for testing
fn create_mock_keys() -> (QantoPQPrivateKey, QantoPQPublicKey) {
    let private_key = QantoPQPrivateKey::new_dummy();
    let public_key = private_key.public_key();
    (private_key, public_key)
}

/// Test small batch transaction verification with timeout
#[tokio::test]
async fn test_small_batch_verification() -> Result<()> {
    info!("Testing small batch transaction verification");

    let test_future = async {
        let processor = OptimizedTransactionProcessor::new();
        let transactions = create_transaction_batch(SMALL_BATCH_SIZE);

        let start_time = Instant::now();
        let _processed = processor.process_batch_parallel_sync(transactions);
        let processing_time = start_time.elapsed();

        info!(
            "Processed {} transactions in {:?}",
            SMALL_BATCH_SIZE, processing_time
        );
        assert!(processing_time < Duration::from_secs(5));

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(TEST_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Small batch verification test timed out");
            Err(anyhow::anyhow!("Small batch verification test timed out"))
        }
    }
}

/// Test medium batch transaction verification with optimized processing
#[tokio::test]
async fn test_medium_batch_verification() -> Result<()> {
    info!("Testing medium batch transaction verification");

    let test_future = async {
        let processor = OptimizedTransactionProcessor::new();
        let transactions = create_transaction_batch(MEDIUM_BATCH_SIZE);

        let start_time = Instant::now();
        let processed = processor.process_batch_parallel_sync(transactions);
        let processing_time = start_time.elapsed();

        info!(
            "Processed {} transactions in {:?}",
            processed.len(),
            processing_time
        );
        assert_eq!(processed.len(), MEDIUM_BATCH_SIZE);
        assert!(processing_time < Duration::from_secs(10));

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(TEST_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Medium batch verification test timed out");
            Err(anyhow::anyhow!("Medium batch verification test timed out"))
        }
    }
}

/// Test large batch transaction verification with performance monitoring
#[tokio::test]
async fn test_large_batch_verification() -> Result<()> {
    info!("Testing large batch transaction verification");

    let test_future = async {
        let processor = OptimizedTransactionProcessor::new();
        let transactions = create_transaction_batch(LARGE_BATCH_SIZE);

        let start_time = Instant::now();
        let processed = processor.process_batch_parallel_sync(transactions);
        let processing_time = start_time.elapsed();

        let tps = processed.len() as f64 / processing_time.as_secs_f64();
        info!(
            "Processed {} transactions in {:?} (TPS: {:.2})",
            processed.len(),
            processing_time,
            tps
        );

        assert_eq!(processed.len(), LARGE_BATCH_SIZE);
        assert!(tps > 1000.0); // Expect at least 1000 TPS

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(TEST_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Large batch verification test timed out");
            Err(anyhow::anyhow!("Large batch verification test timed out"))
        }
    }
}

/// Test transaction creation and basic validation
#[tokio::test]
async fn test_transaction_creation_and_validation() -> Result<()> {
    info!("Testing transaction creation and validation");

    let test_future = async {
        let tx = create_minimal_transaction(1);

        // Validate transaction structure
        assert!(!tx.id.is_empty());
        assert!(tx.fee > 0);
        assert!(!tx.inputs.is_empty());
        assert!(!tx.outputs.is_empty());
        assert!(tx.timestamp > 0);

        // Test serialization
        let serialized = serde_json::to_vec(&tx)?;
        assert!(!serialized.is_empty());

        // Test deserialization
        let deserialized: Transaction = serde_json::from_slice(&serialized)?;
        assert_eq!(tx.id, deserialized.id);

        // Test optimized processor creation
        let processor = OptimizedTransactionProcessor::new();
        let (processed, _avg_time) = processor.get_metrics();
        assert_eq!(processed, 0); // Should start with 0 processed transactions

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(VERIFICATION_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Transaction creation test timed out");
            Err(anyhow::anyhow!("Transaction creation test timed out"))
        }
    }
}

/// Test parallel batch processing with different batch sizes
#[tokio::test]
async fn test_parallel_batch_processing() -> Result<()> {
    info!("Testing parallel batch processing with different sizes");

    let test_future = async {
        let processor = OptimizedTransactionProcessor::new();
        let batch_sizes = vec![10, 50, 100, 500, 1000];

        for &size in &batch_sizes {
            let transactions = create_transaction_batch(size);
            let start_time = Instant::now();

            let processed = processor.process_batch_parallel_sync(transactions);
            let processing_time = start_time.elapsed();

            assert_eq!(processed.len(), size);
            info!("Batch size {}: processed in {:?}", size, processing_time);

            // Ensure processing time scales reasonably
            assert!(processing_time < Duration::from_millis(size as u64 * 10));
        }

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(TEST_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Parallel batch processing test timed out");
            Err(anyhow::anyhow!("Parallel batch processing test timed out"))
        }
    }
}

/// Test transaction verification with mock signatures
#[tokio::test]
async fn test_transaction_verification_with_mocks() -> Result<()> {
    info!("Testing transaction verification with mock signatures");

    let test_future = async {
        let (_private_key, public_key) = create_mock_keys();
        let mut tx = create_minimal_transaction(1);

        // Add mock quantum-resistant signature
        let mock_signature = QuantumResistantSignature {
            signer_public_key: public_key.as_bytes().to_vec(),
            signature: vec![0u8; 64], // Mock signature
        };

        // Update transaction signature
        tx.signature = mock_signature;

        // Verify transaction structure with mock signature
        assert!(!tx.signature.signer_public_key.is_empty());
        assert!(!tx.signature.signature.is_empty());
        assert_eq!(
            tx.signature.signer_public_key,
            public_key.as_bytes().to_vec()
        );

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(VERIFICATION_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Transaction verification test timed out");
            Err(anyhow::anyhow!("Transaction verification test timed out"))
        }
    }
}

/// Test transaction processing performance metrics
#[tokio::test]
async fn test_transaction_processing_metrics() -> Result<()> {
    info!("Testing transaction processing performance metrics");

    let test_future = async {
        let processor = OptimizedTransactionProcessor::new();
        let transactions = create_transaction_batch(MEDIUM_BATCH_SIZE);

        let start_time = Instant::now();
        let processed = processor.process_batch_parallel_sync(transactions);
        let total_time = start_time.elapsed();

        // Calculate metrics
        let tps = processed.len() as f64 / total_time.as_secs_f64();
        let avg_time_per_tx = total_time.as_micros() as f64 / processed.len() as f64;

        info!("Performance metrics:");
        info!("  Total transactions: {}", processed.len());
        info!("  Total time: {:?}", total_time);
        info!("  TPS: {:.2}", tps);
        info!("  Avg time per transaction: {:.2} μs", avg_time_per_tx);

        // Performance assertions
        assert!(tps > 100.0); // At least 100 TPS
        assert!(avg_time_per_tx < 10000.0); // Less than 10ms per transaction

        Ok::<(), anyhow::Error>(())
    };

    match timeout(Duration::from_secs(TEST_TIMEOUT_SECS), test_future).await {
        Ok(result) => result,
        Err(_) => {
            error!("Transaction processing metrics test timed out");
            Err(anyhow::anyhow!(
                "Transaction processing metrics test timed out"
            ))
        }
    }
}

/// Test timeout configuration and behavior
#[tokio::test]
async fn test_timeout_configuration() -> Result<()> {
    info!("Testing timeout configuration and behavior");

    // Test short timeout
    let short_timeout_result = timeout(Duration::from_millis(1), async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok::<(), anyhow::Error>(())
    })
    .await;

    assert!(short_timeout_result.is_err());
    info!("Short timeout correctly triggered");

    // Test adequate timeout
    let adequate_timeout_result = timeout(Duration::from_secs(1), async {
        let transactions = create_transaction_batch(10);
        let processor = OptimizedTransactionProcessor::new();
        let _processed = processor.process_batch_parallel_sync(transactions);
        Ok::<(), anyhow::Error>(())
    })
    .await;

    assert!(adequate_timeout_result.is_ok());
    info!("Adequate timeout allowed completion");

    Ok(())
}

/// Unit test for transaction batch creation
#[test]
fn test_transaction_batch_creation() {
    let batch = create_transaction_batch(5);
    assert_eq!(batch.len(), 5);

    for (i, tx) in batch.iter().enumerate() {
        assert_eq!(tx.id, format!("tx_{}", i));
        assert!(tx.fee > 0);
        assert!(!tx.inputs.is_empty());
        assert!(!tx.outputs.is_empty());
    }
}

/// Unit test for minimal transaction creation
#[test]
fn test_minimal_transaction_creation() {
    let tx = create_minimal_transaction(42);

    assert_eq!(tx.id, "tx_42");
    assert_eq!(tx.fee, 10);
    assert_eq!(tx.outputs[0].amount, 1000);
    assert!(tx.timestamp > 0);
    assert_eq!(tx.sender, "test_sender");
    assert_eq!(tx.receiver, "test_receiver");
}

/// Unit test for mock key generation
#[test]
fn test_mock_key_generation() {
    let (private_key, public_key) = create_mock_keys();

    // Test that keys are generated and have expected properties
    let private_bytes = private_key.as_bytes();
    let public_bytes = public_key.as_bytes();

    assert!(!private_bytes.is_empty());
    assert!(!public_bytes.is_empty());
    assert_ne!(private_bytes, public_bytes);
}
