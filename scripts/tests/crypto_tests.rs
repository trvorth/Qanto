//! Cryptographic Tests
//!
//! This module provides comprehensive tests for all cryptographic functionality
//! including Ed25519, P-256, and post-quantum cryptography.

#[cfg(test)]
mod tests {
    use qanto::qanto_native_crypto::*;
    use rand::thread_rng;

    #[test]
    fn test_ed25519_sign_verify() {
        let mut rng = thread_rng();
        let crypto = QantoNativeCrypto::new();
        let keypair = crypto.generate_keypair(QantoSignatureAlgorithm::Ed25519, &mut rng);

        let message = b"Hello, Qanto!";
        let signature = crypto.sign(&keypair, message).unwrap();

        assert!(crypto.verify(&keypair, message, &signature).is_ok());

        let wrong_message = b"Wrong message";
        assert!(crypto.verify(&keypair, wrong_message, &signature).is_err());
    }

    #[test]
    fn test_post_quantum_sign_verify_corrected() {
        let mut rng = thread_rng();
        let crypto = QantoNativeCrypto::new();
        let keypair = crypto.generate_keypair(QantoSignatureAlgorithm::PostQuantum, &mut rng);

        let message = b"Post-quantum test message with corrected logic";
        let signature = crypto.sign(&keypair, message).unwrap();

        assert!(
            crypto.verify(&keypair, message, &signature).is_ok(),
            "Post-quantum verification failed with the corrected logic"
        );

        let wrong_message = b"Different message";
        assert!(
            crypto.verify(&keypair, wrong_message, &signature).is_err(),
            "Post-quantum verification succeeded with incorrect message"
        );
    }

    #[test]
    fn test_p256_sign_verify() {
        let mut rng = thread_rng();
        let crypto = QantoNativeCrypto::new();
        let keypair = crypto.generate_keypair(QantoSignatureAlgorithm::P256, &mut rng);

        let message = b"P-256 test message";
        let signature = crypto.sign(&keypair, message).unwrap();

        assert!(crypto.verify(&keypair, message, &signature).is_ok());
    }

    #[test]
    fn test_cross_algorithm_verification_fails() {
        let mut rng = thread_rng();
        let crypto = QantoNativeCrypto::new();

        let ed25519_keypair = crypto.generate_keypair(QantoSignatureAlgorithm::Ed25519, &mut rng);
        let pq_keypair = crypto.generate_keypair(QantoSignatureAlgorithm::PostQuantum, &mut rng);

        let message = b"Cross-algorithm test";
        let ed25519_signature = crypto.sign(&ed25519_keypair, message).unwrap();

        assert!(crypto
            .verify(&pq_keypair, message, &ed25519_signature)
            .is_err());
    }
}
