use qanto::qanto_compat::ed25519_dalek::Signer;
use qanto::qanto_compat::*;
use rand::thread_rng;

#[test]
fn test_ed25519_compatibility() {
    let mut rng = thread_rng();
    let signing_key = qanto::qanto_compat::ed25519_dalek::SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();

    let message = b"Test message";
    let signature = signing_key.sign(message);

    assert!(verifying_key.verify_strict(message, &signature).is_ok());
}

#[test]
fn test_pq_compatibility() {
    let (public_key, secret_key) = pqcrypto_compat::mldsa65::keypair();

    let message = b"Post-quantum test message";
    let signature = pqcrypto_compat::mldsa65::sign_detached(message, &secret_key);

    assert!(
        pqcrypto_compat::mldsa65::verify_detached_signature(message, &signature, &public_key)
            .is_ok()
    );
}

#[test]
fn test_libp2p_identity_compatibility() {
    let keypair = libp2p_identity::Keypair::generate_ed25519();
    let public_key = keypair.public();
    let peer_id = public_key.to_peer_id();

    // Test that peer ID is deterministic
    let peer_id2 = public_key.to_peer_id();
    assert_eq!(peer_id, peer_id2);
}
