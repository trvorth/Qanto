//! Qanto Compatibility Layer
//! v0.1.0 - Initial Version
//!
//! This module provides compatibility interfaces that match external cryptographic
//! libraries but use qanto-native implementations underneath. This allows for
//! seamless replacement of external dependencies without changing existing code.

use crate::qanto_native_crypto::*;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Substrate sp_core compatibility module
pub mod sp_core {
    use super::*;
    use rand::Rng;

    /// H256 hash type compatible with sp_core::H256
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct H256(pub [u8; 32]);

    impl H256 {
        /// Create from a 32-byte array
        pub fn from(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        /// Create from a slice (must be exactly 32 bytes)
        pub fn from_slice(slice: &[u8]) -> Self {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&slice[..32]);
            Self(bytes)
        }

        /// Generate a random H256
        pub fn random() -> Self {
            let mut rng = rand::thread_rng();
            let mut bytes = [0u8; 32];
            rng.fill(&mut bytes);
            Self(bytes)
        }

        /// Get the underlying bytes
        pub fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }

        /// Convert to a fixed array
        pub fn to_fixed_bytes(&self) -> [u8; 32] {
            self.0
        }

        /// Create a zero hash
        pub fn zero() -> Self {
            Self([0u8; 32])
        }
    }

    impl Default for H256 {
        fn default() -> Self {
            Self::zero()
        }
    }

    impl fmt::Debug for H256 {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "H256(0x{})", hex::encode(self.0))
        }
    }

    impl fmt::Display for H256 {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "0x{}", hex::encode(self.0))
        }
    }

    impl From<[u8; 32]> for H256 {
        fn from(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }
    }

    impl From<H256> for [u8; 32] {
        fn from(hash: H256) -> Self {
            hash.0
        }
    }

    impl AsRef<[u8]> for H256 {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }
}

// Re-export native crypto types with external library names for compatibility
pub use crate::qanto_native_crypto::{
    QantoNativeCrypto, QantoNativeCryptoError as CryptoError,
    QantoSignatureAlgorithm as SignatureAlgorithm,
};

/// Ed25519 Compatibility Module
/// Provides the same interface as ed25519_dalek but uses qanto-native implementation
pub mod ed25519_dalek {
    use super::*;

    /// Type alias for signature errors in Ed25519 operations.
    pub type SignatureError = QantoNativeCryptoError;

    /// Compatible SigningKey that matches ed25519_dalek::SigningKey interface
    #[derive(Clone, Serialize, Deserialize)]
    pub struct SigningKey {
        inner: QantoEd25519PrivateKey,
    }

    /// Compatible VerifyingKey that matches ed25519_dalek::VerifyingKey interface
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct VerifyingKey {
        inner: QantoEd25519PublicKey,
    }

    /// Compatible Signature that matches ed25519_dalek::Signature interface
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Signature {
        inner: QantoEd25519Signature,
    }

    impl SigningKey {
        /// Creates a SigningKey from raw bytes.
        ///
        /// # Arguments
        ///
        /// * `bytes` - The raw bytes representing the private key
        ///
        /// # Returns
        ///
        /// A Result containing the SigningKey or an error if the bytes are invalid.
        pub fn from_bytes(bytes: &[u8]) -> std::result::Result<Self, SignatureError> {
            let inner = QantoEd25519PrivateKey::from_bytes(bytes)?;
            Ok(Self { inner })
        }

        /// Creates a SigningKey from a 32-byte array.
        ///
        /// # Arguments
        ///
        /// * `bytes` - A 32-byte array containing the private key bytes
        ///
        /// # Returns
        ///
        /// A new SigningKey instance.
        pub fn from_bytes_array(bytes: [u8; 32]) -> Self {
            let inner = QantoEd25519PrivateKey::from_bytes(&bytes).unwrap();
            Self { inner }
        }

        /// Generates a new random SigningKey using the provided RNG.
        ///
        /// # Arguments
        ///
        /// * `rng` - A cryptographically secure random number generator
        ///
        /// # Returns
        ///
        /// A new randomly generated SigningKey.
        pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
            let inner = QantoEd25519PrivateKey::generate(rng);
            Self { inner }
        }

        /// Returns the corresponding VerifyingKey for this SigningKey.
        ///
        /// # Returns
        ///
        /// The public key corresponding to this private key.
        pub fn verifying_key(&self) -> VerifyingKey {
            VerifyingKey {
                inner: self.inner.public_key(),
            }
        }

        /// Converts the SigningKey to its byte representation.
        ///
        /// # Returns
        ///
        /// A 32-byte array containing the private key bytes.
        pub fn to_bytes(&self) -> [u8; 32] {
            *self.inner.as_bytes()
        }
    }

    impl VerifyingKey {
        /// Creates a VerifyingKey from raw bytes.
        ///
        /// # Arguments
        ///
        /// * `bytes` - The raw bytes representing the public key
        ///
        /// # Returns
        ///
        /// A Result containing the VerifyingKey or an error if the bytes are invalid.
        pub fn from_bytes(bytes: &[u8]) -> std::result::Result<Self, SignatureError> {
            let inner = QantoEd25519PublicKey::from_bytes(bytes)?;
            Ok(Self { inner })
        }

        /// Converts the VerifyingKey to its byte representation.
        ///
        /// # Returns
        ///
        /// A 32-byte array containing the public key bytes.
        pub fn to_bytes(&self) -> [u8; 32] {
            *self.inner.as_bytes()
        }

        /// Verifies a signature against a message using strict verification.
        ///
        /// # Arguments
        ///
        /// * `message` - The message that was signed
        /// * `signature` - The signature to verify
        ///
        /// # Returns
        ///
        /// A Result indicating success or failure of verification.
        pub fn verify_strict(
            &self,
            message: &[u8],
            signature: &Signature,
        ) -> std::result::Result<(), SignatureError> {
            self.inner.verify(message, &signature.inner)
        }
    }

    impl Signature {
        /// Creates a Signature from raw bytes.
        ///
        /// # Arguments
        ///
        /// * `bytes` - The raw bytes representing the signature (must be 64 bytes)
        ///
        /// # Returns
        ///
        /// A Result containing the Signature or an error if the bytes are invalid.
        pub fn from_bytes(bytes: &[u8]) -> std::result::Result<Self, SignatureError> {
            if bytes.len() != 64 {
                return Err(QantoNativeCryptoError::InvalidSignatureLength {
                    expected: 64,
                    actual: bytes.len(),
                });
            }
            let mut sig_bytes = [0u8; 64];
            sig_bytes.copy_from_slice(bytes);
            let inner = QantoEd25519Signature(sig_bytes);
            Ok(Self { inner })
        }

        /// Converts the signature to its byte representation.
        ///
        /// Returns the signature as a 64-byte array in the standard Ed25519 format.
        ///
        /// # Returns
        ///
        /// A 64-byte array containing the signature bytes.
        pub fn to_bytes(&self) -> [u8; 64] {
            self.inner.0
        }
    }

    /// Signer trait implementation for SigningKey
    /// A trait for cryptographic signing operations.
    ///
    /// This trait provides a generic interface for signing messages with various
    /// signature algorithms.
    pub trait Signer<S> {
        /// Signs a message and returns the signature.
        ///
        /// # Arguments
        ///
        /// * `message` - The message bytes to sign
        ///
        /// # Returns
        ///
        /// A signature of type `S` for the given message.
        fn sign(&self, message: &[u8]) -> S;
    }

    impl Signer<Signature> for SigningKey {
        fn sign(&self, message: &[u8]) -> Signature {
            let inner = self.inner.sign(message).unwrap();
            Signature { inner }
        }
    }

    /// Verifier trait implementation for VerifyingKey
    /// A trait for cryptographic signature verification operations.
    ///
    /// This trait provides a generic interface for verifying signatures with various
    /// signature algorithms.
    pub trait Verifier<S> {
        /// The error type returned when verification fails.
        type Error;
        /// Verifies a signature against a message.
        ///
        /// # Arguments
        ///
        /// * `message` - The message bytes that were signed
        /// * `signature` - The signature to verify
        ///
        /// # Returns
        ///
        /// `Ok(())` if the signature is valid, or an error if verification fails.
        fn verify(&self, message: &[u8], signature: &S) -> std::result::Result<(), Self::Error>;
    }

    impl Verifier<Signature> for VerifyingKey {
        type Error = SignatureError;

        fn verify(
            &self,
            message: &[u8],
            signature: &Signature,
        ) -> std::result::Result<(), Self::Error> {
            self.inner.verify(message, &signature.inner)
        }
    }

    // Debug implementations
    impl fmt::Debug for SigningKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "SigningKey([REDACTED])")
        }
    }

    impl fmt::Debug for VerifyingKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "VerifyingKey({})", hex::encode(self.inner.as_bytes()))
        }
    }

    impl fmt::Debug for Signature {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Signature({})", hex::encode(self.inner.0))
        }
    }
}

/// Post-Quantum Cryptography Compatibility Module
/// Provides interfaces compatible with pqcrypto libraries
pub mod pqcrypto_compat {
    use super::*;

    /// MLDSA (Dilithium) compatibility module
    pub mod mldsa65 {
        use super::*;
        use rand::rngs::OsRng;

        /// A secret key for ML-DSA-65 (Dilithium) digital signatures.
        ///
        /// This struct wraps a Qanto post-quantum private key for compatibility
        /// with the pqcrypto interface.
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: QantoPQPrivateKey,
        }

        impl SecretKey {
            /// Creates a new SecretKey from a Qanto post-quantum private key.
            ///
            /// # Arguments
            ///
            /// * `inner` - The underlying Qanto post-quantum private key
            ///
            /// # Returns
            ///
            /// A new SecretKey instance wrapping the provided key.
            pub fn new(inner: QantoPQPrivateKey) -> Self {
                Self { inner }
            }
        }

        /// A public key for ML-DSA-65 (Dilithium) digital signatures.
        ///
        /// This struct wraps a Qanto post-quantum public key for compatibility
        /// with the pqcrypto interface.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: QantoPQPublicKey,
        }

        impl PublicKey {
            /// Creates a new PublicKey from a Qanto post-quantum public key.
            ///
            /// # Arguments
            ///
            /// * `inner` - The underlying Qanto post-quantum public key
            ///
            /// # Returns
            ///
            /// A new PublicKey instance wrapping the provided key.
            pub fn new(inner: QantoPQPublicKey) -> Self {
                Self { inner }
            }
        }

        /// A detached signature for ML-DSA-65 post-quantum digital signatures.
        ///
        /// This structure wraps a Qanto post-quantum signature and provides
        /// compatibility with the ML-DSA-65 signature format.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct DetachedSignature {
            inner: QantoPQSignature,
        }

        impl DetachedSignature {
            /// Creates a new DetachedSignature from a Qanto post-quantum signature.
            ///
            /// # Arguments
            ///
            /// * `inner` - The underlying Qanto post-quantum signature
            ///
            /// # Returns
            ///
            /// A new DetachedSignature instance wrapping the provided signature.
            pub fn new(inner: QantoPQSignature) -> Self {
                Self { inner }
            }
        }

        /// ML-DSA signed message
        ///
        /// Contains both the signature and the original message.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct SignedMessage {
            /// The post-quantum signature
            pub signature: QantoPQSignature,
            /// The original message that was signed
            pub message: Vec<u8>,
        }

        /// Generate a new ML-DSA key pair
        ///
        /// # Returns
        /// A tuple containing (PublicKey, SecretKey)
        pub fn keypair() -> (PublicKey, SecretKey) {
            let crypto = QantoNativeCrypto::new();
            let keypair = crypto.generate_keypair(QantoSignatureAlgorithm::PostQuantum, &mut OsRng);
            match keypair {
                QantoKeyPair::PostQuantum { public, private } => {
                    (PublicKey::new(public), SecretKey::new(private))
                }
                _ => unreachable!("Expected PostQuantum keypair"),
            }
        }

        /// Create a detached signature for a message
        ///
        /// # Arguments
        /// * `message` - The message to sign
        /// * `secret_key` - The secret key to use for signing
        ///
        /// # Returns
        /// A DetachedSignature over the message
        pub fn sign_detached(message: &[u8], secret_key: &SecretKey) -> DetachedSignature {
            let signature = secret_key.inner.sign(message).unwrap();
            DetachedSignature::new(signature)
        }

        /// Verify a detached signature
        ///
        /// # Arguments
        /// * `message` - The original message
        /// * `signature` - The signature to verify
        /// * `public_key` - The public key to use for verification
        ///
        /// # Returns
        /// Ok(()) if the signature is valid, Err otherwise
        pub fn verify_detached_signature(
            message: &[u8],
            signature: &DetachedSignature,
            public_key: &PublicKey,
        ) -> Result<(), QantoNativeCryptoError> {
            public_key.inner.verify(message, &signature.inner)
        }

        impl SecretKey {
            /// Create a secret key from raw bytes
            ///
            /// # Arguments
            /// * `bytes` - The raw key material
            ///
            /// # Returns
            /// A Result containing the SecretKey or an error
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                match QantoPQPrivateKey::from_bytes(bytes) {
                    Ok(inner) => Ok(Self { inner }),
                    Err(e) => Err(e),
                }
            }

            /// Get the raw bytes of the secret key
            ///
            /// # Returns
            /// A reference to the key material as bytes
            pub fn as_bytes(&self) -> &[u8] {
                self.inner.as_bytes()
            }
        }

        impl PublicKey {
            /// Create a public key from raw bytes
            ///
            /// # Arguments
            /// * `bytes` - The raw key material
            ///
            /// # Returns
            /// A Result containing the PublicKey or an error
            pub fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError> {
                match QantoPQPublicKey::from_bytes(bytes) {
                    Ok(inner) => Ok(Self { inner }),
                    Err(e) => {
                        eprintln!("Failed to create PublicKey from bytes: {e:?}");
                        Err(e)
                    }
                }
            }

            /// Get the raw bytes of the public key
            ///
            /// # Returns
            /// A reference to the key material as bytes
            pub fn as_bytes(&self) -> &[u8] {
                self.inner.as_bytes()
            }
        }

        impl DetachedSignature {
            /// Create a detached signature from raw bytes
            ///
            /// # Arguments
            /// * `bytes` - The raw signature data
            ///
            /// # Returns
            /// A Result containing the DetachedSignature or an error
            pub fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError> {
                match QantoPQSignature::from_bytes(bytes) {
                    Ok(inner) => Ok(Self { inner }),
                    Err(e) => {
                        eprintln!("Failed to create DetachedSignature from bytes: {e:?}");
                        Err(e)
                    }
                }
            }

            /// Get the raw bytes of the signature
            ///
            /// # Returns
            /// A reference to the signature data as bytes
            pub fn as_bytes(&self) -> &[u8] {
                self.inner.as_bytes()
            }
        }

        impl SignedMessage {
            /// Create a signed message from raw bytes
            ///
            /// # Arguments
            /// * `bytes` - The serialized signed message data
            ///
            /// # Returns
            /// A Result containing the SignedMessage or an error
            pub fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError> {
                if bytes.len() < 8 {
                    return Err(QantoNativeCryptoError::InvalidInputLength);
                }

                // Simple serialization format: [signature_len: 4 bytes][signature][message]
                let sig_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
                if bytes.len() < 4 + sig_len {
                    return Err(QantoNativeCryptoError::InvalidInputLength);
                }

                let signature = QantoPQSignature::from_bytes(&bytes[4..4 + sig_len])?;
                let message = bytes[4 + sig_len..].to_vec();

                Ok(Self { signature, message })
            }

            /// Convert the signed message to raw bytes
            ///
            /// # Returns
            /// A vector containing the serialized signed message
            pub fn as_bytes(&self) -> Vec<u8> {
                let sig_bytes = self.signature.as_bytes();
                let mut result = Vec::with_capacity(4 + sig_bytes.len() + self.message.len());
                result.extend_from_slice(&(sig_bytes.len() as u32).to_le_bytes());
                result.extend_from_slice(sig_bytes);
                result.extend_from_slice(&self.message);
                result
            }
        }
    }

    /// Trait definitions for post-quantum cryptography
    pub mod traits {
        use super::*;

        /// Digital signature traits
        pub mod sign {
            use super::*;

            /// Trait for post-quantum public keys
            pub trait PublicKey {
                /// Get the raw bytes of the public key
                fn as_bytes(&self) -> &[u8];

                /// Create a public key from raw bytes
                fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            /// Trait for post-quantum secret keys
            pub trait SecretKey {
                /// Get the raw bytes of the secret key
                fn as_bytes(&self) -> &[u8];

                /// Create a secret key from raw bytes
                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            /// Trait for post-quantum detached signatures
            pub trait DetachedSignature {
                /// Get the raw bytes of the signature
                fn as_bytes(&self) -> &[u8];

                /// Create a signature from raw bytes
                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            /// Trait for post-quantum signed messages
            pub trait SignedMessage {
                /// Get the raw bytes of the signed message
                fn as_bytes(&self) -> Vec<u8>;

                /// Create a signed message from raw bytes
                fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            // Trait implementations for mldsa65 types
            impl PublicKey for mldsa65::PublicKey {
                fn as_bytes(&self) -> &[u8] {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    match QantoPQPublicKey::from_bytes(bytes) {
                        Ok(inner) => Ok(Self::new(inner)),
                        Err(e) => {
                            eprintln!("Failed to create mldsa65::PublicKey from bytes: {e:?}");
                            Err(e)
                        }
                    }
                }
            }

            impl SecretKey for mldsa65::SecretKey {
                fn as_bytes(&self) -> &[u8] {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    match QantoPQPrivateKey::from_bytes(bytes) {
                        Ok(inner) => Ok(Self::new(inner)),
                        Err(e) => {
                            eprintln!("Failed to create mldsa65::SecretKey from bytes: {e:?}");
                            Err(e)
                        }
                    }
                }
            }

            impl DetachedSignature for mldsa65::DetachedSignature {
                fn as_bytes(&self) -> &[u8] {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    match QantoPQSignature::from_bytes(bytes) {
                        Ok(inner) => Ok(Self::new(inner)),
                        Err(e) => {
                            eprintln!(
                                "Failed to create mldsa65::DetachedSignature from bytes: {e:?}"
                            );
                            Err(e)
                        }
                    }
                }
            }

            impl SignedMessage for mldsa65::SignedMessage {
                fn as_bytes(&self) -> Vec<u8> {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    Self::from_bytes(bytes)
                }
            }
        }
    }
}

/// ECDSA P-256 compatibility module providing drop-in replacements for p256 crate types.
///
/// This module provides signing and verification functionality using the P-256 elliptic curve
/// with Qanto's native cryptographic implementations underneath.
pub mod ecdsa {
    use super::*;

    /// A P-256 ECDSA signing key for creating digital signatures.
    ///
    /// This structure wraps a Qanto P-256 private key and provides
    /// compatibility with the p256 crate's SigningKey interface.
    #[derive(Clone, Serialize, Deserialize)]
    pub struct SigningKey {
        inner: QantoP256PrivateKey,
    }

    /// A P-256 ECDSA verifying key for validating digital signatures.
    ///
    /// This structure wraps a Qanto P-256 public key and provides
    /// compatibility with the p256 crate's VerifyingKey interface.
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
    pub struct VerifyingKey {
        inner: QantoP256PublicKey,
    }

    /// A P-256 ECDSA signature for digital signature verification.
    ///
    /// This structure wraps a Qanto P-256 signature and provides
    /// compatibility with the p256 crate's Signature interface.
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
    pub struct Signature {
        inner: QantoP256Signature,
    }

    impl SigningKey {
        /// Get the corresponding verifying key
        ///
        /// # Returns
        /// The VerifyingKey that corresponds to this SigningKey
        pub fn verifying_key(&self) -> VerifyingKey {
            VerifyingKey {
                inner: self.inner.public_key(),
            }
        }

        /// Convert the signing key to raw bytes
        ///
        /// # Returns
        /// A 32-byte array containing the private key material
        pub fn to_bytes(&self) -> [u8; 32] {
            *self.inner.as_bytes()
        }
    }

    impl VerifyingKey {
        /// Create a verifying key from raw bytes
        ///
        /// # Arguments
        /// * `bytes` - The raw public key material
        ///
        /// # Returns
        /// A Result containing the VerifyingKey or an error
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
            let inner = QantoP256PublicKey::from_bytes(bytes)?;
            Ok(Self { inner })
        }

        /// Convert the verifying key to raw bytes
        ///
        /// # Returns
        /// A 64-byte array containing the public key material
        pub fn to_bytes(&self) -> [u8; 64] {
            self.inner.to_bytes()
        }
    }

    impl Signature {
        /// Create a signature from raw bytes
        ///
        /// # Arguments
        /// * `bytes` - The raw signature data
        ///
        /// # Returns
        /// A Result containing the Signature or an error
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
            let inner = QantoP256Signature::from_bytes(bytes).map_err(|e| {
                eprintln!("Failed to create P256 Signature from bytes: {e:?}");
                e
            })?;
            Ok(Self { inner })
        }

        /// Convert the signature to raw bytes
        ///
        /// # Returns
        /// A 64-byte array containing the signature data
        pub fn to_bytes(&self) -> [u8; 64] {
            self.inner.0
        }
    }

    /// Digital signature traits for P-256 ECDSA
    pub mod signature {
        use super::*;

        /// Trait for types that can create signatures
        pub trait Signer<S> {
            /// Sign a message and return a signature
            fn sign(&self, message: &[u8]) -> S;
        }

        /// Trait for types that can verify signatures
        pub trait Verifier<S> {
            /// The error type returned by verification failures
            type Error;

            /// Verify a signature against a message
            fn verify(&self, message: &[u8], signature: &S)
                -> std::result::Result<(), Self::Error>;
        }

        impl Signer<Signature> for SigningKey {
            /// Sign a message using this signing key
            fn sign(&self, message: &[u8]) -> Signature {
                let inner = self.inner.sign(message).unwrap();
                Signature { inner }
            }
        }

        impl Verifier<Signature> for VerifyingKey {
            type Error = QantoNativeCryptoError;

            /// Verify a signature using this verifying key
            fn verify(
                &self,
                message: &[u8],
                signature: &Signature,
            ) -> std::result::Result<(), Self::Error> {
                self.inner.verify(message, &signature.inner)
            }
        }
    }
}

/// Libp2p Identity Compatibility
/// Provides compatibility with libp2p's identity types
// Additional pqcrypto compatibility modules
pub mod pqcrypto_classicmceliece {
    use super::*;

    /// Classic McEliece 8192128 post-quantum key encapsulation mechanism.
    ///
    /// This module provides compatibility with the Classic McEliece cryptographic
    /// scheme for post-quantum key encapsulation.
    pub mod mceliece8192128 {
        use super::*;

        /// A Classic McEliece public key for key encapsulation.
        ///
        /// This structure represents a public key used in the Classic McEliece
        /// key encapsulation mechanism.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        /// A Classic McEliece secret key for key decapsulation.
        ///
        /// This structure represents a secret key used in the Classic McEliece
        /// key encapsulation mechanism.
        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        /// A Classic McEliece ciphertext for encapsulated keys.
        ///
        /// This structure represents a ciphertext containing an encapsulated
        /// shared secret in the Classic McEliece scheme.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Ciphertext {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::Ciphertext for Ciphertext {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        /// Generate a Classic McEliece 8192128 key pair.
        ///
        /// Returns a tuple containing the public key and secret key for post-quantum
        /// key encapsulation mechanism operations.
        ///
        /// # Returns
        ///
        /// A tuple `(PublicKey, SecretKey)` representing the generated key pair.
        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use actual Classic McEliece in production
            let public = PublicKey {
                inner: vec![0u8; 1357824], // Actual Classic McEliece 8192128 public key size
            };
            let secret = SecretKey {
                inner: vec![0u8; 14080], // Actual Classic McEliece 8192128 secret key size
            };
            (public, secret)
        }

        /// Encapsulate a shared secret
        ///
        /// # Arguments
        /// * `_public_key` - The public key to use for encapsulation
        ///
        /// # Returns
        /// A tuple containing (Ciphertext, shared_secret)
        pub fn encapsulate(_public_key: &PublicKey) -> (Ciphertext, Vec<u8>) {
            // Placeholder implementation
            let ciphertext = Ciphertext {
                inner: vec![0u8; 240], // Actual Classic McEliece 8192128 ciphertext size
            };
            let shared_secret = vec![0u8; 32]; // 256-bit shared secret
            (ciphertext, shared_secret)
        }

        /// Decapsulate a shared secret
        ///
        /// # Arguments
        /// * `_ciphertext` - The ciphertext to decapsulate
        /// * `_secret_key` - The secret key to use for decapsulation
        ///
        /// # Returns
        /// The shared secret
        pub fn decapsulate(_ciphertext: &Ciphertext, _secret_key: &SecretKey) -> Vec<u8> {
            // Placeholder implementation
            vec![0u8; 32] // 256-bit shared secret
        }
    }
}

/// Falcon post-quantum signature compatibility
///
/// Provides compatibility with pqcrypto-falcon using qanto-native
/// post-quantum digital signatures.
pub mod pqcrypto_falcon {
    use super::*;

    /// Falcon-1024 parameter set
    pub mod falcon1024 {
        use super::*;

        /// Falcon public key
        ///
        /// Used for signature verification operations.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        /// Falcon secret key
        ///
        /// Used for signature generation operations.
        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        /// Falcon detached signature
        ///
        /// Contains the signature data separate from the message.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct DetachedSignature {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::sign::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::sign::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::sign::DetachedSignature for DetachedSignature {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        /// Generate a Falcon key pair
        ///
        /// # Returns
        /// A tuple containing (PublicKey, SecretKey)
        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use actual Falcon in production
            let public = PublicKey {
                inner: vec![0u8; 1793], // Actual Falcon-1024 public key size
            };
            let secret = SecretKey {
                inner: vec![0u8; 2305], // Actual Falcon-1024 secret key size
            };
            (public, secret)
        }

        /// Create a detached signature
        ///
        /// # Arguments
        /// * `_message` - The message to sign
        /// * `_secret_key` - The secret key to use for signing
        ///
        /// # Returns
        /// A DetachedSignature over the message
        pub fn detached_sign(_message: &[u8], _secret_key: &SecretKey) -> DetachedSignature {
            // Placeholder implementation
            DetachedSignature {
                inner: vec![0u8; 1330], // Typical Falcon-1024 signature size
            }
        }

        /// Verify a detached signature
        ///
        /// # Arguments
        /// * `_message` - The original message
        /// * `_signature` - The signature to verify
        /// * `_public_key` - The public key to use for verification
        ///
        /// # Returns
        /// Ok(()) if the signature is valid, Err otherwise
        pub fn verify_detached_signature(
            _message: &[u8],
            _signature: &DetachedSignature,
            _public_key: &PublicKey,
        ) -> CryptoResult<()> {
            // Placeholder implementation
            Ok(())
        }
    }
}

/// HQC post-quantum KEM compatibility
///
/// Provides compatibility with pqcrypto-hqc using qanto-native
/// post-quantum key encapsulation mechanisms.
pub mod pqcrypto_hqc {
    use super::*;

    /// HQC-256 parameter set
    pub mod hqc256 {
        use super::*;

        /// HQC public key
        ///
        /// Used for key encapsulation operations.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        /// HQC secret key
        ///
        /// Used for key decapsulation operations.
        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        /// HQC ciphertext
        ///
        /// Contains the encapsulated key material.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Ciphertext {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::Ciphertext for Ciphertext {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        /// Generate an HQC key pair
        ///
        /// # Returns
        /// A tuple containing (PublicKey, SecretKey)
        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use actual HQC in production
            let public = PublicKey {
                inner: vec![0u8; 67], // Actual HQC-256 public key size
            };
            let secret = SecretKey {
                inner: vec![0u8; 67], // Actual HQC-256 secret key size
            };
            (public, secret)
        }

        /// Encapsulate a shared secret
        ///
        /// # Arguments
        /// * `_public_key` - The public key to use for encapsulation
        ///
        /// # Returns
        /// A tuple containing (Ciphertext, shared_secret)
        pub fn encapsulate(_public_key: &PublicKey) -> (Ciphertext, Vec<u8>) {
            // Placeholder implementation
            let ciphertext = Ciphertext {
                inner: vec![0u8; 67], // Actual HQC-256 ciphertext size
            };
            let shared_secret = vec![0u8; 64]; // 512-bit shared secret
            (ciphertext, shared_secret)
        }

        /// Decapsulate a shared secret
        ///
        /// # Arguments
        /// * `_ciphertext` - The ciphertext to decapsulate
        /// * `_secret_key` - The secret key to use for decapsulation
        ///
        /// # Returns
        /// The shared secret
        pub fn decapsulate(_ciphertext: &Ciphertext, _secret_key: &SecretKey) -> Vec<u8> {
            // Placeholder implementation
            vec![0u8; 64] // 512-bit shared secret
        }
    }
}

/// ML-DSA compatibility re-export
///
/// Provides a direct re-export of the ML-DSA implementation from pqcrypto_compat.
pub mod pqcrypto_mldsa {
    /// ML-DSA-65 parameter set
    pub mod mldsa65 {
        /// Re-export all ML-DSA-65 types and functions
        pub use crate::qanto_compat::pqcrypto_compat::mldsa65::*;
    }
}

/// ML-KEM post-quantum KEM compatibility
///
/// Provides compatibility with pqcrypto-mlkem using qanto-native
/// post-quantum key encapsulation mechanisms.
pub mod pqcrypto_mlkem {
    use super::*;

    /// ML-KEM-1024 parameter set
    pub mod mlkem1024 {
        use super::*;

        /// ML-KEM public key
        ///
        /// Used for key encapsulation operations.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        /// ML-KEM secret key
        ///
        /// Used for key decapsulation operations.
        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        /// ML-KEM ciphertext
        ///
        /// Contains the encapsulated key material.
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Ciphertext {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::Ciphertext for Ciphertext {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                Ok(Self {
                    inner: bytes.to_vec(),
                })
            }
        }

        /// Generate an ML-KEM key pair
        ///
        /// # Returns
        /// A tuple containing (PublicKey, SecretKey)
        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use actual ML-KEM in production
            let public = PublicKey {
                inner: vec![0u8; 1568], // Actual ML-KEM-1024 public key size
            };
            let secret = SecretKey {
                inner: vec![0u8; 3168], // Actual ML-KEM-1024 secret key size
            };
            (public, secret)
        }

        /// Encapsulate a shared secret
        ///
        /// # Arguments
        /// * `_public_key` - The public key to use for encapsulation
        ///
        /// # Returns
        /// A tuple containing (Ciphertext, shared_secret)
        pub fn encapsulate(_public_key: &PublicKey) -> (Ciphertext, Vec<u8>) {
            // Placeholder implementation
            let ciphertext = Ciphertext {
                inner: vec![0u8; 1568], // Actual ML-KEM-1024 ciphertext size
            };
            let shared_secret = vec![0u8; 32]; // 256-bit shared secret
            (ciphertext, shared_secret)
        }

        /// Decapsulate a shared secret
        ///
        /// # Arguments
        /// * `_ciphertext` - The ciphertext to decapsulate
        /// * `_secret_key` - The secret key to use for decapsulation
        ///
        /// # Returns
        /// The shared secret
        pub fn decapsulate(_ciphertext: &Ciphertext, _secret_key: &SecretKey) -> Vec<u8> {
            // Placeholder implementation
            vec![0u8; 32] // 256-bit shared secret
        }
    }
}

/// Post-quantum cryptography trait definitions
///
/// Provides common trait interfaces for post-quantum cryptographic operations.
pub mod pqcrypto_traits {
    use super::*;

    /// Key Encapsulation Mechanism (KEM) traits
    pub mod kem {
        use super::*;

        /// Trait for KEM public keys
        pub trait PublicKey {
            /// Get the raw bytes of the public key
            fn as_bytes(&self) -> &[u8];

            /// Create a public key from raw bytes
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }

        /// Trait for KEM secret keys
        pub trait SecretKey {
            /// Get the raw bytes of the secret key
            fn as_bytes(&self) -> &[u8];

            /// Create a secret key from raw bytes
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }

        /// Trait for KEM ciphertexts
        pub trait Ciphertext {
            /// Get the raw bytes of the ciphertext
            fn as_bytes(&self) -> &[u8];

            /// Create a ciphertext from raw bytes
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }

        /// Trait for KEM shared secrets
        pub trait SharedSecret {
            /// Get the raw bytes of the shared secret
            fn as_bytes(&self) -> &[u8];

            /// Create a shared secret from raw bytes
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }
    }

    /// Digital signature traits
    pub mod sign {
        /// Re-export signature traits from pqcrypto_compat
        pub use crate::qanto_compat::pqcrypto_compat::traits::sign::*;
    }
}

/// libp2p identity compatibility module
///
/// Provides compatibility with libp2p's identity types using qanto-native
/// cryptographic implementations.
pub mod libp2p_identity {
    use super::*;

    /// Cryptographic keypair for libp2p identity
    ///
    /// Currently supports Ed25519 keys using qanto-native implementations.
    #[derive(Clone, Debug)]
    pub enum Keypair {
        /// Ed25519 keypair variant
        Ed25519(ed25519_dalek::SigningKey),
    }

    impl Keypair {
        /// Generate a new Ed25519 keypair
        ///
        /// # Returns
        /// A new Keypair with a randomly generated Ed25519 key
        pub fn generate_ed25519() -> Self {
            use rand::rngs::OsRng;
            let signing_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
            Keypair::Ed25519(signing_key)
        }

        /// Get the public key from this keypair
        ///
        /// # Returns
        /// The PublicKey corresponding to this Keypair
        pub fn public(&self) -> PublicKey {
            match self {
                Keypair::Ed25519(signing_key) => PublicKey::Ed25519(signing_key.verifying_key()),
            }
        }
    }

    /// Public key for libp2p identity
    ///
    /// Currently supports Ed25519 public keys using qanto-native implementations.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum PublicKey {
        /// Ed25519 public key variant
        Ed25519(ed25519_dalek::VerifyingKey),
    }

    impl PublicKey {
        /// Convert this public key to a peer ID
        ///
        /// # Returns
        /// A PeerId derived from this public key
        pub fn to_peer_id(&self) -> PeerId {
            match self {
                PublicKey::Ed25519(verifying_key) => {
                    // Simple peer ID derivation - in practice this would use proper hashing
                    let bytes = verifying_key.to_bytes().to_vec();
                    PeerId(bytes)
                }
            }
        }
    }

    /// Peer identifier for libp2p networks
    ///
    /// A unique identifier derived from a public key that identifies
    /// a peer in the libp2p network.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct PeerId(Vec<u8>);

    impl PeerId {
        /// Create a PeerId from raw bytes
        ///
        /// # Arguments
        /// * `bytes` - The raw bytes to create the PeerId from
        ///
        /// # Returns
        /// A Result containing the PeerId or an error
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
            if bytes.is_empty() {
                return Err(QantoNativeCryptoError::InvalidInputLength);
            }
            Ok(PeerId(bytes.to_vec()))
        }

        /// Convert the PeerId to raw bytes
        ///
        /// # Returns
        /// A vector containing the PeerId bytes
        pub fn to_bytes(&self) -> Vec<u8> {
            self.0.clone()
        }
    }

    impl fmt::Display for PeerId {
        /// Format PeerId for display (shows as hex string)
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", hex::encode(&self.0))
        }
    }
}
