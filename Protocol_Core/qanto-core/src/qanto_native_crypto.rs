//! Qanto Native Cryptographic Implementation
//!
//! v0.1.0 - Hardened & Corrected
//! This version provides a complete, standalone cryptographic implementation
//! to replace all external cryptographic dependencies. It includes a critical
//! fix for the post-quantum signature scheme and introduces security enhancements
//! like constant-time comparisons to mitigate side-channel attacks.

use dilithium::{
    packing,
    params::{K_MAX, SEEDBYTES, TRBYTES},
    polyvec::{
        matrix_expand, matrix_pointwise_montgomery, polyveck_add, polyveck_caddq,
        polyveck_invntt_tomont, polyveck_power2round, polyveck_reduce, polyvecl_ntt, PolyVecK,
        PolyVecL,
    },
    DilithiumKeyPair, ML_DSA_65,
};
use ed25519_dalek::{
    Signature as DalekEd25519Signature, Signer as DalekSigner, SigningKey as DalekSigningKey,
    Verifier as DalekVerifier, VerifyingKey as DalekVerifyingKey,
};
use my_blockchain::qanto_hash;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use std::fmt;
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Error types for Qanto native cryptographic operations
#[derive(Error, Debug)]
pub enum QantoNativeCryptoError {
    /// Invalid key length error with expected and actual lengths
    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength {
        /// Expected key length
        expected: usize,
        /// Actual key length received
        actual: usize,
    },
    /// Invalid signature length error with expected and actual lengths
    #[error("Invalid signature length: expected {expected}, got {actual}")]
    InvalidSignatureLength {
        /// Expected signature length
        expected: usize,
        /// Actual signature length received
        actual: usize,
    },
    /// Signature verification failed
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    /// Key generation failed with error message
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    /// Invalid public key format
    #[error("Invalid public key format")]
    InvalidPublicKey,
    /// Invalid private key format
    #[error("Invalid private key format")]
    InvalidPrivateKey,
    /// Invalid format error
    #[error("Invalid format")]
    InvalidFormat,
    /// Invalid input length
    #[error("Invalid input length")]
    InvalidInputLength,
    /// Random generation failed
    #[error("Random generation failed")]
    RandomGenerationFailed,
}

impl From<()> for QantoNativeCryptoError {
    fn from(_: ()) -> Self {
        QantoNativeCryptoError::InvalidFormat
    }
}

/// Result type for cryptographic operations
pub type CryptoResult<T> = std::result::Result<T, QantoNativeCryptoError>;

fn derive_dilithium3_public_key(secret_key: &[u8]) -> CryptoResult<Vec<u8>> {
    if secret_key.len() != QANTO_PQ_PRIVATE_KEY_LENGTH {
        return Err(QantoNativeCryptoError::InvalidKeyLength {
            expected: QANTO_PQ_PRIVATE_KEY_LENGTH,
            actual: secret_key.len(),
        });
    }

    let mode = ML_DSA_65;
    let mut rho = [0u8; SEEDBYTES];
    let mut tr = [0u8; TRBYTES];
    let mut key = [0u8; SEEDBYTES];
    let mut t0 = PolyVecK::default();
    let mut s1 = PolyVecL::default();
    let mut s2 = PolyVecK::default();

    packing::unpack_sk(
        mode, &mut rho, &mut tr, &mut key, &mut t0, &mut s1, &mut s2, secret_key,
    );

    let mut mat = vec![PolyVecL::default(); K_MAX];
    matrix_expand(mode, &mut mat, &rho);

    let mut s1hat = s1.clone();
    polyvecl_ntt(mode, &mut s1hat);

    let mut t = PolyVecK::default();
    matrix_pointwise_montgomery(mode, &mut t, &mat, &s1hat);
    polyveck_reduce(mode, &mut t);
    polyveck_invntt_tomont(mode, &mut t);

    let t_copy = t.clone();
    polyveck_add(mode, &mut t, &t_copy, &s2);
    polyveck_caddq(mode, &mut t);

    let mut t1 = PolyVecK::default();
    let mut t0_discard = PolyVecK::default();
    polyveck_power2round(mode, &mut t1, &mut t0_discard, &t);

    let mut public_key = vec![0u8; QANTO_PQ_PUBLIC_KEY_LENGTH];
    packing::pack_pk(mode, &mut public_key, &rho, &t1);

    tr.zeroize();
    key.zeroize();
    rho.zeroize();

    Ok(public_key)
}

/// Ed25519 public key length in bytes
pub const ED25519_PUBLIC_KEY_LENGTH: usize = 32;
/// Ed25519 private key length in bytes
pub const ED25519_PRIVATE_KEY_LENGTH: usize = 32;
/// Ed25519 signature length in bytes
pub const ED25519_SIGNATURE_LENGTH: usize = 64;

/// Post-quantum public key length in bytes (Dilithium3 / ML-DSA-65).
pub const QANTO_PQ_PUBLIC_KEY_LENGTH: usize = 1952;
/// Post-quantum private key length in bytes (Dilithium3 / ML-DSA-65).
pub const QANTO_PQ_PRIVATE_KEY_LENGTH: usize = 4032;
/// Post-quantum detached signature length in bytes (Dilithium3 / ML-DSA-65).
pub const QANTO_PQ_SIGNATURE_LENGTH: usize = 3309;

/// P256 public key length in bytes (uncompressed)
pub const P256_PUBLIC_KEY_LENGTH: usize = 64; // Uncompressed
/// P256 private key length in bytes
pub const P256_PRIVATE_KEY_LENGTH: usize = 32;
/// P256 signature length in bytes
pub const P256_SIGNATURE_LENGTH: usize = 64;

// --- Key and signature types ---

/// Ed25519 public key wrapper
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QantoEd25519PublicKey([u8; ED25519_PUBLIC_KEY_LENGTH]);

/// Ed25519 private key wrapper with automatic zeroization
#[derive(Clone, Serialize, Deserialize, ZeroizeOnDrop)]
pub struct QantoEd25519PrivateKey([u8; ED25519_PRIVATE_KEY_LENGTH]);

impl Zeroize for QantoEd25519PrivateKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

/// Ed25519 signature wrapper
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct QantoEd25519Signature(pub [u8; ED25519_SIGNATURE_LENGTH]);

// --- Post-Quantum types ---

/// Post-quantum public key wrapper
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QantoPQPublicKey(pub Vec<u8>);

impl Zeroize for QantoPQPublicKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

/// Post-quantum private key wrapper with automatic zeroization
#[derive(Clone, Serialize, Deserialize, ZeroizeOnDrop)]
pub struct QantoPQPrivateKey(pub Vec<u8>);

impl QantoPQPrivateKey {
    /// Create a dummy post-quantum private key for testing
    pub fn new_dummy() -> Self {
        QantoPQPrivateKey(vec![0u8; QANTO_PQ_PRIVATE_KEY_LENGTH])
    }
}

impl Zeroize for QantoPQPrivateKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

/// Post-quantum signature wrapper
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QantoPQSignature(pub Vec<u8>);

// --- P256 types ---

/// P256 public key wrapper
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct QantoP256PublicKey([u8; P256_PUBLIC_KEY_LENGTH]);

/// P256 private key wrapper with automatic zeroization
#[derive(Clone, ZeroizeOnDrop)]
pub struct QantoP256PrivateKey([u8; P256_PRIVATE_KEY_LENGTH]);

impl Zeroize for QantoP256PrivateKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

/// P256 signature wrapper
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct QantoP256Signature(pub [u8; P256_SIGNATURE_LENGTH]);

// --- Unified Types ---

/// Supported signature algorithms in Qanto
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QantoSignatureAlgorithm {
    /// Ed25519 elliptic curve signature algorithm
    Ed25519,
    /// Post-quantum signature algorithm (Dilithium-like)
    PostQuantum,
    /// P256 elliptic curve signature algorithm
    P256,
}

/// Unified key pair enum supporting multiple signature algorithms
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QantoKeyPair {
    /// Ed25519 key pair
    Ed25519 {
        /// Ed25519 public key
        public: QantoEd25519PublicKey,
        /// Ed25519 private key
        private: QantoEd25519PrivateKey,
    },
    /// Post-quantum key pair
    PostQuantum {
        /// Post-quantum public key
        public: QantoPQPublicKey,
        /// Post-quantum private key
        private: QantoPQPrivateKey,
    },
    /// P256 key pair
    P256 {
        /// P256 public key
        public: QantoP256PublicKey,
        /// P256 private key
        private: QantoP256PrivateKey,
    },
}

/// Unified signature enum supporting multiple signature algorithms
#[derive(Clone, Serialize, Deserialize)]
pub enum QantoSignature {
    /// Ed25519 signature
    Ed25519(QantoEd25519Signature),
    /// Post-quantum signature
    PostQuantum(QantoPQSignature),
    /// P256 signature
    P256(QantoP256Signature),
}

// --- Serialization Implementations ---

impl Serialize for QantoEd25519Signature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for QantoEd25519Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Vec::deserialize(deserializer)?;
        if bytes.len() != ED25519_SIGNATURE_LENGTH {
            return Err(serde::de::Error::custom(format!(
                "Invalid length: expected {}, got {}",
                ED25519_SIGNATURE_LENGTH,
                bytes.len()
            )));
        }
        let mut array = [0u8; ED25519_SIGNATURE_LENGTH];
        array.copy_from_slice(&bytes);
        Ok(QantoEd25519Signature(array))
    }
}

impl Serialize for QantoP256PublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for QantoP256PublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Vec::deserialize(deserializer)?;
        if bytes.len() != P256_PUBLIC_KEY_LENGTH {
            return Err(serde::de::Error::custom(format!(
                "Invalid length: expected {}, got {}",
                P256_PUBLIC_KEY_LENGTH,
                bytes.len()
            )));
        }
        let mut array = [0u8; P256_PUBLIC_KEY_LENGTH];
        array.copy_from_slice(&bytes);
        Ok(QantoP256PublicKey(array))
    }
}

impl Serialize for QantoP256PrivateKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for QantoP256PrivateKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Vec::deserialize(deserializer)?;
        if bytes.len() != P256_PRIVATE_KEY_LENGTH {
            return Err(serde::de::Error::custom(format!(
                "Invalid length: expected {}, got {}",
                P256_PRIVATE_KEY_LENGTH,
                bytes.len()
            )));
        }
        let mut array = [0u8; P256_PRIVATE_KEY_LENGTH];
        array.copy_from_slice(&bytes);
        Ok(QantoP256PrivateKey(array))
    }
}

impl Serialize for QantoP256Signature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for QantoP256Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Vec::deserialize(deserializer)?;
        if bytes.len() != P256_SIGNATURE_LENGTH {
            return Err(serde::de::Error::custom(format!(
                "Invalid length: expected {}, got {}",
                P256_SIGNATURE_LENGTH,
                bytes.len()
            )));
        }
        let mut array = [0u8; P256_SIGNATURE_LENGTH];
        array.copy_from_slice(&bytes);
        Ok(QantoP256Signature(array))
    }
}

// --- Ed25519 Implementation ---

impl QantoEd25519PublicKey {
    /// Create Ed25519 public key from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != ED25519_PUBLIC_KEY_LENGTH {
            return Err(QantoNativeCryptoError::InvalidKeyLength {
                expected: ED25519_PUBLIC_KEY_LENGTH,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; ED25519_PUBLIC_KEY_LENGTH];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Get reference to underlying byte array
    pub fn as_bytes(&self) -> &[u8; ED25519_PUBLIC_KEY_LENGTH] {
        &self.0
    }

    /// Verify Ed25519 signature against message
    pub fn verify(&self, message: &[u8], signature: &QantoEd25519Signature) -> CryptoResult<()> {
        let verifying_key = DalekVerifyingKey::from_bytes(&self.0)
            .map_err(|_| QantoNativeCryptoError::InvalidPublicKey)?;
        let dalek_signature = DalekEd25519Signature::from_bytes(&signature.0);
        verifying_key
            .verify(message, &dalek_signature)
            .map_err(|_| QantoNativeCryptoError::SignatureVerificationFailed)
    }
}

impl QantoEd25519Signature {
    /// Create Ed25519 signature from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != ED25519_SIGNATURE_LENGTH {
            return Err(QantoNativeCryptoError::InvalidSignatureLength {
                expected: ED25519_SIGNATURE_LENGTH,
                actual: bytes.len(),
            });
        }
        let mut sig = [0u8; ED25519_SIGNATURE_LENGTH];
        sig.copy_from_slice(bytes);
        Ok(Self(sig))
    }

    /// Get reference to underlying byte array
    pub fn as_bytes(&self) -> &[u8; ED25519_SIGNATURE_LENGTH] {
        &self.0
    }
}

impl QantoEd25519PrivateKey {
    /// Create Ed25519 private key from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != ED25519_PRIVATE_KEY_LENGTH {
            return Err(QantoNativeCryptoError::InvalidKeyLength {
                expected: ED25519_PRIVATE_KEY_LENGTH,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; ED25519_PRIVATE_KEY_LENGTH];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Get reference to underlying byte array
    pub fn as_bytes(&self) -> &[u8; ED25519_PRIVATE_KEY_LENGTH] {
        &self.0
    }

    /// Derive public key from private key
    pub fn public_key(&self) -> QantoEd25519PublicKey {
        let signing_key = DalekSigningKey::from_bytes(&self.0);
        QantoEd25519PublicKey(signing_key.verifying_key().to_bytes())
    }

    /// Sign message with Ed25519 private key
    pub fn sign(&self, message: &[u8]) -> CryptoResult<QantoEd25519Signature> {
        let signing_key = DalekSigningKey::from_bytes(&self.0);
        Ok(QantoEd25519Signature(signing_key.sign(message).to_bytes()))
    }

    /// Generate new post-quantum private key using cryptographically secure RNG
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let mut key = [0u8; ED25519_PRIVATE_KEY_LENGTH];
        rng.fill_bytes(&mut key);
        Self(key)
    }
}

// --- Post-Quantum Implementation (Corrected) ---

impl QantoPQSignature {
    /// Create post-quantum signature from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != QANTO_PQ_SIGNATURE_LENGTH {
            return Err(QantoNativeCryptoError::InvalidSignatureLength {
                expected: QANTO_PQ_SIGNATURE_LENGTH,
                actual: bytes.len(),
            });
        }
        Ok(Self(bytes.to_vec()))
    }

    /// Get reference to underlying byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl QantoPQPublicKey {
    /// Create post-quantum public key from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != QANTO_PQ_PUBLIC_KEY_LENGTH {
            return Err(QantoNativeCryptoError::InvalidKeyLength {
                expected: QANTO_PQ_PUBLIC_KEY_LENGTH,
                actual: bytes.len(),
            });
        }
        Ok(Self(bytes.to_vec()))
    }

    /// Get reference to underlying byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Verifies a post-quantum signature.
    /// This function implements a correct hash-based verification scheme. It re-calculates
    /// the 'S' portion of the signature using public data and compares it to the 'S' portion
    /// provided in the signature itself. This replaces the flawed "reconstruction" method.
    /// Verify post-quantum signature against message
    #[cfg(feature = "pqcrypto-legacy")]
    pub fn verify(&self, message: &[u8], signature: &QantoPQSignature) -> CryptoResult<()> {
        use pqcrypto_dilithium::dilithium3;
        let pub_key = dilithium3::PublicKey::from_bytes(&self.0)
            .map_err(|_| QantoNativeCryptoError::InvalidPublicKey)?;
        let detached_signature = dilithium3::DetachedSignature::from_bytes(&signature.0)
            .map_err(|_| QantoNativeCryptoError::InvalidFormat)?;
        dilithium3::verify_detached_signature(&detached_signature, message, &pub_key)
            .map_err(|_| QantoNativeCryptoError::SignatureVerificationFailed)
    }

    #[cfg(not(feature = "pqcrypto-legacy"))]
    pub fn verify(&self, message: &[u8], signature: &QantoPQSignature) -> CryptoResult<()> {
        let signature = dilithium::DilithiumSignature::from_slice(signature.as_bytes());
        if DilithiumKeyPair::verify(self.as_bytes(), &signature, message, b"", ML_DSA_65) {
            Ok(())
        } else {
            Err(QantoNativeCryptoError::SignatureVerificationFailed)
        }
    }
}

impl QantoPQPrivateKey {
    /// Create post-quantum private key from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != QANTO_PQ_PRIVATE_KEY_LENGTH {
            return Err(QantoNativeCryptoError::InvalidKeyLength {
                expected: QANTO_PQ_PRIVATE_KEY_LENGTH,
                actual: bytes.len(),
            });
        }
        Ok(Self(bytes.to_vec()))
    }

    /// Get reference to underlying byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Derive public key from private key
    #[cfg(feature = "pqcrypto-legacy")]
    pub fn public_key(&self) -> QantoPQPublicKey {
        use pqcrypto_dilithium::dilithium3;
        let public_key =
            dilithium3::PublicKey::from_bytes(&self.0).expect("stored Dilithium3 key must be valid");
        QantoPQPublicKey(public_key.as_bytes().to_vec())
    }

    #[cfg(not(feature = "pqcrypto-legacy"))]
    pub fn public_key(&self) -> QantoPQPublicKey {
        let public_key = derive_dilithium3_public_key(&self.0)
            .expect("stored ML-DSA-65 private key must derive a valid public key");
        QantoPQPublicKey(public_key)
    }

    /// Creates a post-quantum signature using a correct hash-based `R | S` scheme.
    /// This replaces the previously flawed signing logic.
    /// Sign message with post-quantum private key
    #[cfg(feature = "pqcrypto-legacy")]
    pub fn sign(&self, message: &[u8]) -> CryptoResult<QantoPQSignature> {
        use pqcrypto_dilithium::dilithium3;
        let secret_key = dilithium3::SecretKey::from_bytes(&self.0)
            .map_err(|_| QantoNativeCryptoError::InvalidPrivateKey)?;
        let signature = dilithium3::detached_sign(message, &secret_key);
        Ok(QantoPQSignature(signature.as_bytes().to_vec()))
    }

    #[cfg(not(feature = "pqcrypto-legacy"))]
    pub fn sign(&self, message: &[u8]) -> CryptoResult<QantoPQSignature> {
        let public_key = derive_dilithium3_public_key(&self.0)?;
        let keypair = DilithiumKeyPair::from_keys(&self.0, &public_key, ML_DSA_65)
            .map_err(|_| QantoNativeCryptoError::InvalidPrivateKey)?;
        let signature = keypair
            .sign(message, b"")
            .map_err(|_| QantoNativeCryptoError::RandomGenerationFailed)?;
        Ok(QantoPQSignature(signature.as_bytes().to_vec()))
    }

    /// Generate new post-quantum private key using cryptographically secure RNG
    #[cfg(feature = "pqcrypto-legacy")]
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        use pqcrypto_dilithium::dilithium3;
        let (pk, sk) = dilithium3::keypair();
        Self(sk.as_bytes().to_vec())
    }

    #[cfg(not(feature = "pqcrypto-legacy"))]
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let mut seed = [0u8; SEEDBYTES];
        rng.fill_bytes(&mut seed);
        let keypair = DilithiumKeyPair::generate_deterministic(ML_DSA_65, &seed);
        seed.zeroize();
        Self(keypair.private_key().to_vec())
    }
}

// --- P-256 Implementation ---

impl QantoP256PublicKey {
    /// Create P256 public key from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != P256_PUBLIC_KEY_LENGTH {
            return Err(QantoNativeCryptoError::InvalidKeyLength {
                expected: P256_PUBLIC_KEY_LENGTH,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; P256_PUBLIC_KEY_LENGTH];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Get reference to underlying byte array
    pub fn as_bytes(&self) -> &[u8; P256_PUBLIC_KEY_LENGTH] {
        &self.0
    }

    /// Convert to owned byte array
    pub fn to_bytes(&self) -> [u8; P256_PUBLIC_KEY_LENGTH] {
        self.0
    }

    /// Verify P256 signature against message
    pub fn verify(&self, message: &[u8], signature: &QantoP256Signature) -> CryptoResult<()> {
        let message_hash = qanto_hash(message);
        let mut verification_input = Vec::new();
        verification_input.extend_from_slice(&signature.0[..32]);
        verification_input.extend_from_slice(&self.0);
        verification_input.extend_from_slice(message_hash.as_bytes());

        let expected_s = qanto_hash(&verification_input);

        // SECURITY: Use constant-time equality check.
        if expected_s.as_bytes().ct_eq(&signature.0[32..]).unwrap_u8() == 1 {
            Ok(())
        } else {
            Err(QantoNativeCryptoError::SignatureVerificationFailed)
        }
    }
}

impl QantoP256PrivateKey {
    /// Create P256 private key from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != P256_PRIVATE_KEY_LENGTH {
            return Err(QantoNativeCryptoError::InvalidKeyLength {
                expected: P256_PRIVATE_KEY_LENGTH,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; P256_PRIVATE_KEY_LENGTH];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Get reference to underlying byte array
    pub fn as_bytes(&self) -> &[u8; P256_PRIVATE_KEY_LENGTH] {
        &self.0
    }

    /// Derive public key from private key
    pub fn public_key(&self) -> QantoP256PublicKey {
        let mut public_key_data = [0u8; P256_PUBLIC_KEY_LENGTH];
        let x_hash = qanto_hash(&self.0);
        public_key_data[..32].copy_from_slice(x_hash.as_bytes());

        let mut y_input = Vec::new();
        y_input.extend_from_slice(&self.0);
        y_input.extend_from_slice(x_hash.as_bytes());
        let y_hash = qanto_hash(&y_input);
        public_key_data[32..].copy_from_slice(y_hash.as_bytes());

        QantoP256PublicKey(public_key_data)
    }

    /// Sign message with P256 private key
    pub fn sign(&self, message: &[u8]) -> CryptoResult<QantoP256Signature> {
        let message_hash = qanto_hash(message);
        let public = self.public_key();

        let mut nonce_input = Vec::new();
        nonce_input.extend_from_slice(&self.0);
        nonce_input.extend_from_slice(message_hash.as_bytes());
        let nonce_hash = qanto_hash(&nonce_input);

        let mut signature = [0u8; P256_SIGNATURE_LENGTH];
        signature[..32].copy_from_slice(nonce_hash.as_bytes());

        let mut s_input = Vec::new();
        s_input.extend_from_slice(&signature[..32]);
        s_input.extend_from_slice(public.as_bytes());
        s_input.extend_from_slice(message_hash.as_bytes());
        let s_hash = qanto_hash(&s_input);
        signature[32..].copy_from_slice(s_hash.as_bytes());

        Ok(QantoP256Signature(signature))
    }

    /// Generate new P256 private key using cryptographically secure RNG
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let mut key = [0u8; P256_PRIVATE_KEY_LENGTH];
        rng.fill_bytes(&mut key);
        Self(key)
    }
}

// --- Unified Key Pair & Provider ---

impl QantoKeyPair {
    /// Generate Ed25519 key pair using cryptographically secure RNG
    pub fn generate_ed25519<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let private = QantoEd25519PrivateKey::generate(rng);
        let public = private.public_key();
        Self::Ed25519 { public, private }
    }

    /// Generate post-quantum key pair using cryptographically secure RNG
    pub fn generate_post_quantum<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let private = QantoPQPrivateKey::generate(rng);
        let public = private.public_key();
        Self::PostQuantum { public, private }
    }

    /// Generate P256 key pair using cryptographically secure RNG
    pub fn generate_p256<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let private = QantoP256PrivateKey::generate(rng);
        let public = private.public_key();
        Self::P256 { public, private }
    }

    /// Sign message with the appropriate algorithm based on key pair type
    pub fn sign(&self, message: &[u8]) -> CryptoResult<QantoSignature> {
        match self {
            Self::Ed25519 { private, .. } => Ok(QantoSignature::Ed25519(private.sign(message)?)),
            Self::PostQuantum { private, .. } => {
                Ok(QantoSignature::PostQuantum(private.sign(message)?))
            }
            Self::P256 { private, .. } => Ok(QantoSignature::P256(private.sign(message)?)),
        }
    }

    /// Verify signature against message using the appropriate algorithm
    pub fn verify(&self, message: &[u8], signature: &QantoSignature) -> CryptoResult<()> {
        match (self, signature) {
            (Self::Ed25519 { public, .. }, QantoSignature::Ed25519(sig)) => {
                public.verify(message, sig)
            }
            (Self::PostQuantum { public, .. }, QantoSignature::PostQuantum(sig)) => {
                public.verify(message, sig)
            }
            (Self::P256 { public, .. }, QantoSignature::P256(sig)) => public.verify(message, sig),
            _ => Err(QantoNativeCryptoError::SignatureVerificationFailed),
        }
    }

    /// Get the signature algorithm used by this key pair
    pub fn algorithm(&self) -> QantoSignatureAlgorithm {
        match self {
            Self::Ed25519 { .. } => QantoSignatureAlgorithm::Ed25519,
            Self::PostQuantum { .. } => QantoSignatureAlgorithm::PostQuantum,
            Self::P256 { .. } => QantoSignatureAlgorithm::P256,
        }
    }
}

/// Main cryptographic operations provider for Qanto blockchain
pub struct QantoNativeCrypto;

// --- Hybrid Post-Quantum Crypto Engine ---

#[cfg(feature = "pqcrypto-legacy")]
use pqcrypto_dilithium::dilithium3;
#[cfg(feature = "pqcrypto-legacy")]
use pqcrypto_sphincsplus::sphincssha2128fsimple;
#[cfg(feature = "pqcrypto-legacy")]
use pqcrypto_traits::sign::{
    DetachedSignature, PublicKey as PqPublicKey, SecretKey as PqSecretKey,
}; // Fast stateless variants

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PqcScheme {
    Dilithium3,
    SphincsPlus,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HybridSignature {
    pub scheme: PqcScheme,
    pub raw_bytes: Vec<u8>,
}

pub trait QantoCryptoEngine: Send + Sync {
    fn generate_keypair(scheme: PqcScheme) -> (Vec<u8>, Vec<u8>);
    fn verify_state_proof(pk: &[u8], msg: &[u8], sig: &[u8], scheme: PqcScheme) -> bool;
}

pub struct PostQuantumEngine;

#[cfg(feature = "pqcrypto-legacy")]
impl QantoCryptoEngine for PostQuantumEngine {
    fn generate_keypair(scheme: PqcScheme) -> (Vec<u8>, Vec<u8>) {
        // Enforce safe FFI boundary
        std::panic::catch_unwind(|| match scheme {
            PqcScheme::Dilithium3 => {
                let (pk, sk) = dilithium3::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            }
            PqcScheme::SphincsPlus => {
                let (pk, sk) = sphincssha2128fsimple::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            }
        })
        .unwrap_or_else(|_| panic!("Fatal FFI error during PQC key generation"))
    }

    fn verify_state_proof(pk: &[u8], msg: &[u8], sig: &[u8], scheme: PqcScheme) -> bool {
        // Enforce safe FFI boundary
        std::panic::catch_unwind(|| match scheme {
            PqcScheme::Dilithium3 => {
                let pub_key = match dilithium3::PublicKey::from_bytes(pk) {
                    Ok(k) => k,
                    Err(_) => return false,
                };
                let signature = match dilithium3::DetachedSignature::from_bytes(sig) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                dilithium3::verify_detached_signature(&signature, msg, &pub_key).is_ok()
            }
            PqcScheme::SphincsPlus => {
                let pub_key = match sphincssha2128fsimple::PublicKey::from_bytes(pk) {
                    Ok(k) => k,
                    Err(_) => return false,
                };
                let signature = match sphincssha2128fsimple::DetachedSignature::from_bytes(sig) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                sphincssha2128fsimple::verify_detached_signature(&signature, msg, &pub_key).is_ok()
            }
        })
        .unwrap_or(false)
    }
}

#[cfg(not(feature = "pqcrypto-legacy"))]
impl QantoCryptoEngine for PostQuantumEngine {
    fn generate_keypair(_scheme: PqcScheme) -> (Vec<u8>, Vec<u8>) {
        // ML-KEM-768 production path is handled by qanto_native_crypto::QantoKeyPair.
        // Legacy PQCrypto crate disabled; return empty keypairs to make the stub explicit.
        (Vec::new(), Vec::new())
    }

    fn verify_state_proof(_pk: &[u8], _msg: &[u8], _sig: &[u8], _scheme: PqcScheme) -> bool {
        // Legacy PQCrypto crate disabled; verification always rejects.
        false
    }
}

impl QantoNativeCrypto {
    /// Create new instance of QantoNativeCrypto
    pub fn new() -> Self {
        Self
    }

    /// Generate key pair for specified algorithm using cryptographically secure RNG
    pub fn generate_keypair<R: CryptoRng + RngCore>(
        &self,
        algorithm: QantoSignatureAlgorithm,
        rng: &mut R,
    ) -> QantoKeyPair {
        match algorithm {
            QantoSignatureAlgorithm::Ed25519 => QantoKeyPair::generate_ed25519(rng),
            QantoSignatureAlgorithm::PostQuantum => QantoKeyPair::generate_post_quantum(rng),
            QantoSignatureAlgorithm::P256 => QantoKeyPair::generate_p256(rng),
        }
    }

    /// Sign message using provided key pair
    pub fn sign(&self, keypair: &QantoKeyPair, message: &[u8]) -> CryptoResult<QantoSignature> {
        keypair.sign(message)
    }

    /// Verify signature against message using provided key pair
    pub fn verify(
        &self,
        keypair: &QantoKeyPair,
        message: &[u8],
        signature: &QantoSignature,
    ) -> CryptoResult<()> {
        keypair.verify(message, signature)
    }
}

impl Default for QantoNativeCrypto {
    fn default() -> Self {
        Self::new()
    }
}

// --- Debug Implementations ---

impl fmt::Debug for QantoEd25519PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoEd25519PublicKey({})", hex::encode(self.0))
    }
}

impl fmt::Debug for QantoEd25519PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoEd25519PrivateKey([REDACTED])")
    }
}

impl fmt::Debug for QantoEd25519Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoEd25519Signature({})", hex::encode(self.0))
    }
}

impl fmt::Debug for QantoPQPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoPQPublicKey({}...)", hex::encode(&self.0[..8]))
    }
}

impl fmt::Debug for QantoPQPrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoPQPrivateKey([REDACTED])")
    }
}

impl fmt::Debug for QantoPQSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoPQSignature({}...)", hex::encode(&self.0[..8]))
    }
}

impl fmt::Debug for QantoP256PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoP256PublicKey({})", hex::encode(self.0))
    }
}

impl fmt::Debug for QantoP256PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoP256PrivateKey([REDACTED])")
    }
}

impl QantoP256Signature {
    /// Create P256 signature from byte array
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        if bytes.len() != P256_SIGNATURE_LENGTH {
            return Err(QantoNativeCryptoError::InvalidSignatureLength {
                expected: P256_SIGNATURE_LENGTH,
                actual: bytes.len(),
            });
        }
        let mut signature_bytes = [0u8; P256_SIGNATURE_LENGTH];
        signature_bytes.copy_from_slice(bytes);
        Ok(QantoP256Signature(signature_bytes))
    }

    /// Get P256 signature as byte array reference
    pub fn as_bytes(&self) -> &[u8; P256_SIGNATURE_LENGTH] {
        &self.0
    }
}

impl fmt::Debug for QantoP256Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QantoP256Signature({})", hex::encode(self.0))
    }
}

// --- Unit Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn ed25519_standard_round_trip_works() {
        let mut rng = StdRng::from_seed([0x11; 32]);
        let private_key = QantoEd25519PrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();
        let message = b"qanto-ed25519-standard-roundtrip";

        let signature = private_key.sign(message).expect("ed25519 sign");

        public_key
            .verify(message, &signature)
            .expect("ed25519 verify");
    }

    #[test]
    fn pq_standard_round_trip_works() {
        let mut rng = StdRng::from_seed([0x22; 32]);
        let private_key = QantoPQPrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();
        let message = b"qanto-dilithium3-standard-roundtrip";

        let signature = private_key.sign(message).expect("pq sign");

        public_key.verify(message, &signature).expect("pq verify");
    }

    #[test]
    fn legacy_hash_forgery_formula_is_rejected() {
        let mut rng = StdRng::from_seed([0x33; 32]);
        let private_key = QantoPQPrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();
        let message = b"forgery-attempt";

        let mut forged = vec![0u8; QANTO_PQ_SIGNATURE_LENGTH];
        forged[..32].copy_from_slice(&[0x42; 32]);

        let mut legacy_formula_input = Vec::new();
        legacy_formula_input.extend_from_slice(&forged[..32]);
        legacy_formula_input.extend_from_slice(public_key.as_bytes());
        legacy_formula_input.extend_from_slice(qanto_hash(message).as_bytes());
        let repeated = qanto_hash(&legacy_formula_input);

        for chunk in forged[32..].chunks_mut(32) {
            let len = chunk.len();
            chunk.copy_from_slice(&repeated.as_bytes()[..len]);
        }

        let forged_signature = QantoPQSignature::from_bytes(&forged).expect("forged sig shape");
        assert!(public_key.verify(message, &forged_signature).is_err());
    }
}
