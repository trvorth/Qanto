// post_quantum_crypto.rs
// Qanto's post-quantum cryptography module using native qanhash implementation

pub use crate::qanto_native_crypto::{
    QantoPQPrivateKey, QantoPQPublicKey, QantoPQSignature, QantoSignatureAlgorithm,
};
use rand::rngs::OsRng;
use rand::RngCore;
use rand::SeedableRng;
use std::error::Error;
use std::time::{Duration, SystemTime};

/// Error type for post-quantum operations
#[derive(Debug)]
pub enum PQError {
    KeyGenerationError,
    SigningError,
    VerificationError,
}

impl std::fmt::Display for PQError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PQError::KeyGenerationError => write!(f, "Failed to generate key pair"),
            PQError::SigningError => write!(f, "Failed to sign message"),
            PQError::VerificationError => write!(f, "Signature verification failed"),
        }
    }
}

impl Error for PQError {}

pub const QANTO_PQ_PUBLIC_KEY_BYTES: usize = 1952;
pub const QANTO_PQ_PRIVATE_KEY_BYTES: usize = 4000;

/// Generate a post-quantum key pair using native Qanto implementation
pub fn generate_pq_keypair(
    seed: Option<[u8; 32]>,
) -> Result<(QantoPQPublicKey, QantoPQPrivateKey), PQError> {
    let mut rng = if let Some(s) = seed {
        rand::rngs::StdRng::from_seed(s)
    } else {
        let mut s = [0u8; 32];
        OsRng.fill_bytes(&mut s);
        rand::rngs::StdRng::from_seed(s)
    };
    let private_key = QantoPQPrivateKey::generate(&mut rng);
    let public_key = private_key.public_key();
    Ok((public_key, private_key))
}

/// Sign a message using post-quantum private key
pub fn pq_sign(
    private_key: &QantoPQPrivateKey,
    message: &[u8],
) -> Result<QantoPQSignature, PQError> {
    private_key.sign(message).map_err(|_| PQError::SigningError)
}

/// Verify a post-quantum signature
pub fn pq_verify(
    public_key: &QantoPQPublicKey,
    message: &[u8],
    signature: &QantoPQSignature,
) -> Result<bool, PQError> {
    public_key
        .verify(message, signature)
        .map(|_| true)
        .map_err(|_| PQError::VerificationError)
}

/// Post-Quantum Signature Key Pair
#[derive(Clone, Debug)]
pub struct PQSignatureKeyPair {
    pub public: QantoPQPublicKey,
    pub private: QantoPQPrivateKey,
}

impl PQSignatureKeyPair {
    pub fn generate() -> Result<Self, PQError> {
        let (public, private) = generate_pq_keypair(None)?;
        Ok(Self { public, private })
    }
}

/// Key rotation policy structure
#[derive(Clone, Debug)]
pub struct KeyRotationPolicy {
    pub rotation_interval: Duration,
    pub last_rotation: SystemTime,
    pub usage_count: u64,
    pub max_usage: u64,
}

impl Default for KeyRotationPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyRotationPolicy {
    pub fn new() -> Self {
        KeyRotationPolicy {
            rotation_interval: Duration::from_secs(60 * 60 * 24 * 30), // 30 days
            last_rotation: SystemTime::now(),
            usage_count: 0,
            max_usage: 100000,
        }
    }

    pub fn needs_rotation(&self) -> bool {
        let elapsed = self.last_rotation.elapsed().unwrap_or(Duration::ZERO);
        elapsed > self.rotation_interval || self.usage_count > self.max_usage
    }
}
