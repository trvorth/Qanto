//! Fast transaction tests using mocks
//! This module provides accelerated transaction tests using mock implementations

use anyhow::Result;
use qanto::storage_traits::{CryptoOperations, StorageOperations};
use qanto::transaction::{Input, Output, Transaction};
use qanto::types::{HomomorphicEncrypted, QuantumResistantSignature};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

mod mock_test_helpers;
use mock_test_helpers::{create_in_memory_mock_storage, create_mock_crypto, MockTestConfig};

const FAST_TEST_TIMEOUT_SECS: u64 = 5; // Much faster timeout for mocked tests
const LARGE_BATCH_SIZE: usize = 10000;

/// Create a test transaction using mock crypto
async fn create_fast_transaction(id: usize, crypto: &impl CryptoOperations) -> Result<Transaction> {
    let message = format!("tx_{id}");
    let (private_key, public_key) = crypto
        .generate_keypair()
        .await
        .map_err(|e| anyhow::anyhow!("Keypair generation failed: {e}"))?;
    let signature = crypto
        .sign(message.as_bytes(), &private_key)
        .await
        .map_err(|e| anyhow::anyhow!("Signing failed: {e}"))?;

    Ok(Transaction {
        id: format!("tx_{id}"),
        sender: "test_sender".to_string(),
        receiver: "test_receiver".to_string(),
        amount: 1000,
        fee: 10,
        gas_limit: 21000,
        gas_used: 0,
        gas_price: 1,
        priority_fee: 0,
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
        signature: QuantumResistantSignature {
            signer_public_key: public_key,
            signature,
        },
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metadata: std::collections::HashMap::new(),
        fee_breakdown: None,
    })
}

/// Fast transaction verification using mocks
async fn verify_transaction_fast(tx: &Transaction, crypto: &impl CryptoOperations) -> Result<bool> {
    let message = tx.id.as_bytes();
    let verified = crypto
        .verify_signature(
            message,
            &tx.signature.signature,
            &tx.signature.signer_public_key,
        )
        .await;

    Ok(verified && tx.amount > 0 && tx.fee > 0)
}

#[tokio::test]
async fn test_fast_transaction_creation() -> Result<()> {
    let test_future = async {
        let crypto = create_mock_crypto();

        let start = Instant::now();
        let tx = create_fast_transaction(1, &crypto).await?;
        let creation_time = start.elapsed();

        // Verify transaction structure
        assert_eq!(tx.id, "tx_1");
        assert_eq!(tx.amount, 1000);
        assert_eq!(tx.fee, 10);
        assert!(!tx.signature.signature.is_empty());
        assert!(!tx.signature.signer_public_key.is_empty());

        println!("Fast transaction creation took: {creation_time:?}");
        assert!(creation_time < Duration::from_millis(10)); // Should be very fast with mocks

        Ok::<(), anyhow::Error>(())
    };

    timeout(Duration::from_secs(FAST_TEST_TIMEOUT_SECS), test_future).await?
}

#[tokio::test]
async fn test_fast_batch_verification() -> Result<()> {
    let test_future = async {
        let crypto = create_mock_crypto();

        let start = Instant::now();

        // Create a large batch of transactions
        let mut transactions = Vec::with_capacity(LARGE_BATCH_SIZE);
        for i in 0..LARGE_BATCH_SIZE {
            let tx = create_fast_transaction(i, &crypto).await?;
            transactions.push(tx);
        }

        let creation_time = start.elapsed();
        println!("Created {LARGE_BATCH_SIZE} transactions in {creation_time:?}");

        // Verify all transactions
        let verification_start = Instant::now();
        let mut verified_count = 0;

        for tx in &transactions {
            if verify_transaction_fast(tx, &crypto).await? {
                verified_count += 1;
            }
        }

        let verification_time = verification_start.elapsed();
        println!("Verified {verified_count} transactions in {verification_time:?}");

        assert_eq!(verified_count, LARGE_BATCH_SIZE);
        assert!(verification_time < Duration::from_secs(1)); // Should be very fast with mocks

        Ok::<(), anyhow::Error>(())
    };

    timeout(Duration::from_secs(FAST_TEST_TIMEOUT_SECS), test_future).await?
}

#[tokio::test]
async fn test_fast_storage_operations() -> Result<()> {
    let test_future = async {
        let storage = create_in_memory_mock_storage();

        let start = Instant::now();

        // Store transactions
        for i in 0..1000 {
            let key = format!("tx_{i}").into_bytes();
            let value = format!("transaction_data_{i}").into_bytes();
            storage.put(key, value).await?;
        }

        let storage_time = start.elapsed();
        println!("Stored 1000 transactions in {storage_time:?}");

        // Retrieve transactions
        let retrieval_start = Instant::now();
        let mut retrieved_count = 0;

        for i in 0..1000 {
            let key = format!("tx_{i}").into_bytes();
            if storage.get(&key).await?.is_some() {
                retrieved_count += 1;
            }
        }

        let retrieval_time = retrieval_start.elapsed();
        println!("Retrieved {retrieved_count} transactions in {retrieval_time:?}",);

        assert_eq!(retrieved_count, 1000);
        assert!(storage_time < Duration::from_millis(100)); // Should be very fast with mocks
        assert!(retrieval_time < Duration::from_millis(100));

        Ok::<(), anyhow::Error>(())
    };

    timeout(Duration::from_secs(FAST_TEST_TIMEOUT_SECS), test_future).await?
}

#[tokio::test]
async fn test_fast_crypto_operations() -> Result<()> {
    let test_future = async {
        let crypto = create_mock_crypto();

        let start = Instant::now();

        // Test hash operations
        let mut hashes = Vec::new();
        for i in 0..1000 {
            let data = format!("data_{i}").into_bytes();
            let hash = crypto.hash(&data).await;
            hashes.push(hash);
        }

        let hash_time = start.elapsed();
        println!("Generated 1000 hashes in {hash_time:?}");

        // Test signature operations
        let sig_start = Instant::now();
        let (private_key, public_key) = crypto
            .generate_keypair()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        let mut signatures = Vec::new();
        for i in 0..100 {
            let message = format!("message_{i}").into_bytes();
            let signature = crypto
                .sign(&message, &private_key)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            signatures.push((message, signature));
        }

        let sig_time = sig_start.elapsed();
        println!("Generated 100 signatures in {sig_time:?}");

        // Verify signatures
        let verify_start = Instant::now();
        let mut verified_count = 0;

        for (message, signature) in &signatures {
            if crypto
                .verify_signature(message, signature, &public_key)
                .await
            {
                verified_count += 1;
            }
        }

        let verify_time = verify_start.elapsed();
        println!("Verified {verified_count} signatures in {verify_time:?}");

        assert_eq!(hashes.len(), 1000);
        assert_eq!(signatures.len(), 100);
        assert_eq!(verified_count, 100);

        // All operations should be very fast with mocks
        assert!(hash_time < Duration::from_millis(50));
        assert!(sig_time < Duration::from_millis(50));
        assert!(verify_time < Duration::from_millis(50));

        Ok::<(), anyhow::Error>(())
    };

    timeout(Duration::from_secs(FAST_TEST_TIMEOUT_SECS), test_future).await?
}

#[tokio::test]
async fn test_integrated_fast_workflow() -> Result<()> {
    let test_future = async {
        let _config = MockTestConfig::with_in_memory_storage();
        let crypto = create_mock_crypto();
        let storage = create_in_memory_mock_storage();

        let start = Instant::now();

        // Create, sign, and store transactions
        for i in 0..500 {
            let tx = create_fast_transaction(i, &crypto).await?;

            // Verify transaction
            assert!(verify_transaction_fast(&tx, &crypto).await?);

            // Store transaction
            let key = format!("tx_{i}").into_bytes();
            let value = serde_json::to_vec(&tx).unwrap();
            storage.put(key, value).await?;
        }

        let workflow_time = start.elapsed();
        println!("Completed integrated workflow for 500 transactions in {workflow_time:?}");

        // Verify all transactions are stored
        let mut stored_count = 0;
        for i in 0..500 {
            let key = format!("tx_{i}").into_bytes();
            if storage.get(&key).await?.is_some() {
                stored_count += 1;
            }
        }

        assert_eq!(stored_count, 500);
        assert!(workflow_time < Duration::from_secs(1)); // Should be very fast with mocks

        Ok::<(), anyhow::Error>(())
    };

    timeout(Duration::from_secs(FAST_TEST_TIMEOUT_SECS), test_future).await?
}

#[tokio::test]
async fn test_performance_comparison() -> Result<()> {
    let test_future = async {
        let crypto = create_mock_crypto();

        // Test with different batch sizes to show scaling
        let batch_sizes = vec![10, 100, 1000, 5000];

        for &batch_size in &batch_sizes {
            let start = Instant::now();

            for i in 0..batch_size {
                let tx = create_fast_transaction(i, &crypto).await?;
                assert!(verify_transaction_fast(&tx, &crypto).await?);
            }

            let elapsed = start.elapsed();
            let tx_per_sec = batch_size as f64 / elapsed.as_secs_f64();

            println!("Batch size {batch_size}: {elapsed:?} ({tx_per_sec:.0} tx/sec)");

            // With mocks, we should achieve very high throughput
            assert!(tx_per_sec > 1000.0); // At least 1000 transactions per second
        }

        Ok::<(), anyhow::Error>(())
    };

    timeout(Duration::from_secs(FAST_TEST_TIMEOUT_SECS), test_future).await?
}
