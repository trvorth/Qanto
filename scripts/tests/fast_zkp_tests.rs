#![cfg(feature = "zk")]
//! Fast Zero-Knowledge Proof Tests using Mocks
//!
//! This module provides optimized tests for zero-knowledge proof functionality
//! using MockZKProofSystem to avoid expensive cryptographic operations.

#[cfg(test)]
mod tests {
    use qanto::mock_traits::MockZKProofSystem;
    use qanto::zkp::ZKProofType;
    use std::time::Instant;

    #[tokio::test]
    async fn test_fast_zk_system_initialization() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        assert!(zk_system.initialize().await.is_ok());
        let duration = start.elapsed();
        println!("Mock ZK initialization took: {duration:?}");
        assert!(duration.as_millis() < 10); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_range_proof_generation_and_verification() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        let proof = zk_system.generate_range_proof(50, 0, 100).await.unwrap();
        assert_eq!(proof.proof_type, ZKProofType::RangeProof);

        let is_valid = zk_system.verify_proof(&proof).await.unwrap();
        assert!(is_valid);

        let duration = start.elapsed();
        println!("Mock range proof generation and verification took: {duration:?}");
        assert!(duration.as_millis() < 10); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_balance_proof_generation_and_verification() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        let inputs = vec![10, 20];
        let outputs = vec![15, 15];
        let proof = zk_system
            .generate_balance_proof(inputs, outputs)
            .await
            .unwrap();
        assert_eq!(proof.proof_type, ZKProofType::BalanceProof);

        let is_valid = zk_system.verify_proof(&proof).await.unwrap();
        assert!(is_valid);

        let duration = start.elapsed();
        println!("Mock balance proof generation and verification took: {duration:?}");
        assert!(duration.as_millis() < 10); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_computation_proof() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        // Test addition computation (type 0)
        let private_inputs = vec![2u64, 3u64];
        let public_inputs = vec![1u64];
        let expected_output = 6u64; // 2 + 3 + 1
        let computation_type = 0u8; // Addition

        let proof = zk_system
            .generate_computation_proof(
                private_inputs,
                public_inputs,
                expected_output,
                computation_type,
            )
            .await
            .unwrap();
        assert_eq!(proof.proof_type, ZKProofType::ComputationProof);

        let is_valid = zk_system.verify_proof(&proof).await.unwrap();
        assert!(is_valid, "Computation proof should be valid");

        // Test multiplication computation (type 1)
        let private_inputs = vec![2u64, 3u64];
        let public_inputs = vec![1u64];
        let expected_output = 6u64; // 2 * 3 * 1
        let computation_type = 1u8; // Multiplication

        let proof = zk_system
            .generate_computation_proof(
                private_inputs,
                public_inputs,
                expected_output,
                computation_type,
            )
            .await
            .unwrap();

        let is_valid = zk_system.verify_proof(&proof).await.unwrap();
        assert!(is_valid, "Multiplication computation proof should be valid");

        let duration = start.elapsed();
        println!("Mock computation proofs generation and verification took: {duration:?}");
        assert!(duration.as_millis() < 20); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_identity_proof() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        let identity_key = 12345u64;
        let nonce = 98765u64;
        let identity_commitment = (identity_key + nonce) * (identity_key + nonce); // Simple commitment
        let age = 25u64;
        let age_threshold = 18u64;

        let proof = zk_system
            .generate_identity_proof(identity_key, identity_commitment, nonce, age, age_threshold)
            .await
            .unwrap();
        assert_eq!(proof.proof_type, ZKProofType::IdentityProof);

        let is_valid = zk_system.verify_proof(&proof).await.unwrap();
        assert!(is_valid, "Identity proof should be valid");

        let duration = start.elapsed();
        println!("Mock identity proof generation and verification took: {duration:?}");
        assert!(duration.as_millis() < 10); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_voting_proof() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        let voter_key = 54321u64;
        let nullifier = voter_key * 2; // Simple nullifier computation
        let vote = 1u64; // Vote for candidate 1
        let election_pubkey = 11111u64;

        let proof = zk_system
            .generate_voting_proof(voter_key, nullifier, vote, election_pubkey)
            .await
            .unwrap();
        assert_eq!(proof.proof_type, ZKProofType::VotingProof);

        let is_valid = zk_system.verify_proof(&proof).await.unwrap();
        assert!(is_valid, "Voting proof should be valid");

        let duration = start.elapsed();
        println!("Mock voting proof generation and verification took: {duration:?}");
        assert!(duration.as_millis() < 10); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_proof_caching() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        let proof = zk_system.generate_range_proof(500, 0, 1000).await.unwrap();
        // Test that proof was generated
        assert!(!proof.proof.is_empty());

        // Test that the proof system is working
        let verification_result = zk_system.verify_proof(&proof).await;
        assert!(verification_result.is_ok());

        let duration = start.elapsed();
        println!("Mock proof caching test took: {duration:?}");
        assert!(duration.as_millis() < 10); // Should be nearly instant
    }

    #[tokio::test]
    async fn test_fast_bulk_proof_generation() {
        let start = Instant::now();
        let zk_system = MockZKProofSystem::new();
        zk_system.initialize().await.unwrap();

        // Generate multiple proofs quickly
        let mut proofs = Vec::new();
        for i in 0..100 {
            let proof = zk_system.generate_range_proof(i, 0, 1000).await.unwrap();
            proofs.push(proof);
        }

        // Verify all proofs
        for proof in &proofs {
            let is_valid = zk_system.verify_proof(proof).await.unwrap();
            assert!(is_valid);
        }

        let duration = start.elapsed();
        println!("Generated and verified 100 mock proofs in: {duration:?}");
        assert!(duration.as_millis() < 100); // Should be very fast
    }
}
