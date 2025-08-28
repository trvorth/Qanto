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
        pub fn from_bytes(bytes: &[u8]) -> std::result::Result<Self, SignatureError> {
            let inner = QantoEd25519PrivateKey::from_bytes(bytes)?;
            Ok(Self { inner })
        }

        pub fn from_bytes_array(bytes: [u8; 32]) -> Self {
            let inner = QantoEd25519PrivateKey::from_bytes(&bytes).unwrap();
            Self { inner }
        }

        pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
            let inner = QantoEd25519PrivateKey::generate(rng);
            Self { inner }
        }

        pub fn verifying_key(&self) -> VerifyingKey {
            VerifyingKey {
                inner: self.inner.public_key(),
            }
        }

        pub fn to_bytes(&self) -> [u8; 32] {
            *self.inner.as_bytes()
        }
    }

    impl VerifyingKey {
        pub fn from_bytes(bytes: &[u8]) -> std::result::Result<Self, SignatureError> {
            let inner = QantoEd25519PublicKey::from_bytes(bytes)?;
            Ok(Self { inner })
        }

        pub fn to_bytes(&self) -> [u8; 32] {
            *self.inner.as_bytes()
        }

        pub fn verify_strict(
            &self,
            message: &[u8],
            signature: &Signature,
        ) -> std::result::Result<(), SignatureError> {
            self.inner.verify(message, &signature.inner)
        }
    }

    impl Signature {
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

        pub fn to_bytes(&self) -> [u8; 64] {
            self.inner.0
        }
    }

    /// Signer trait implementation for SigningKey
    pub trait Signer<S> {
        fn sign(&self, message: &[u8]) -> S;
    }

    impl Signer<Signature> for SigningKey {
        fn sign(&self, message: &[u8]) -> Signature {
            let inner = self.inner.sign(message).unwrap();
            Signature { inner }
        }
    }

    /// Verifier trait implementation for VerifyingKey
    pub trait Verifier<S> {
        type Error;
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

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: QantoPQPrivateKey,
        }

        impl SecretKey {
            pub fn new(inner: QantoPQPrivateKey) -> Self {
                Self { inner }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: QantoPQPublicKey,
        }

        impl PublicKey {
            pub fn new(inner: QantoPQPublicKey) -> Self {
                Self { inner }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct DetachedSignature {
            inner: QantoPQSignature,
        }

        impl DetachedSignature {
            pub fn new(inner: QantoPQSignature) -> Self {
                Self { inner }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct SignedMessage {
            pub signature: QantoPQSignature,
            pub message: Vec<u8>,
        }

        pub fn keypair() -> (PublicKey, SecretKey) {
            let sk = QantoPQPrivateKey::generate(&mut OsRng);
            let pk = sk.public_key();
            (PublicKey { inner: pk }, SecretKey { inner: sk })
        }

        pub fn sign_detached(message: &[u8], secret_key: &SecretKey) -> DetachedSignature {
            let sig = secret_key.inner.sign(message).unwrap();
            DetachedSignature { inner: sig }
        }

        pub fn verify_detached_signature(
            message: &[u8],
            signature: &DetachedSignature,
            public_key: &PublicKey,
        ) -> Result<(), QantoNativeCryptoError> {
            public_key.inner.verify(message, &signature.inner)
        }

        impl SecretKey {
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.len() != QANTO_PQ_PRIVATE_KEY_LENGTH {
                    return Err(QantoNativeCryptoError::InvalidKeyLength {
                        expected: QANTO_PQ_PRIVATE_KEY_LENGTH,
                        actual: bytes.len(),
                    });
                }
                let inner = QantoPQPrivateKey(bytes.to_vec());
                Ok(Self { inner })
            }

            pub fn as_bytes(&self) -> &[u8] {
                self.inner.as_bytes()
            }
        }

        impl PublicKey {
            pub fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                if bytes.len() != QANTO_PQ_PUBLIC_KEY_LENGTH {
                    return Err(QantoNativeCryptoError::InvalidKeyLength {
                        expected: QANTO_PQ_PUBLIC_KEY_LENGTH,
                        actual: bytes.len(),
                    });
                }
                let inner = QantoPQPublicKey(bytes.to_vec());
                Ok(Self { inner })
            }

            pub fn as_bytes(&self) -> &[u8] {
                self.inner.as_bytes()
            }
        }

        impl DetachedSignature {
            pub fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                if bytes.len() != QANTO_PQ_SIGNATURE_LENGTH {
                    return Err(QantoNativeCryptoError::InvalidSignatureLength {
                        expected: QANTO_PQ_SIGNATURE_LENGTH,
                        actual: bytes.len(),
                    });
                }
                let inner = QantoPQSignature(bytes.to_vec());
                Ok(Self { inner })
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.inner.0
            }
        }

        impl SignedMessage {
            pub fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                // Simple format: signature_length (4 bytes) + signature + message
                if bytes.len() < 4 {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }

                let sig_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
                if bytes.len() < 4 + sig_len {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }

                let signature = QantoPQSignature(bytes[4..4 + sig_len].to_vec());
                let message = bytes[4 + sig_len..].to_vec();

                Ok(Self { signature, message })
            }

            pub fn as_bytes(&self) -> Vec<u8> {
                let mut result = Vec::new();
                result.extend_from_slice(&(self.signature.0.len() as u32).to_le_bytes());
                result.extend_from_slice(&self.signature.0);
                result.extend_from_slice(&self.message);
                result
            }
        }
    }

    /// Traits for compatibility with pqcrypto_traits
    pub mod traits {
        use super::*;

        pub mod sign {
            use super::*;

            pub trait PublicKey {
                fn as_bytes(&self) -> &[u8];
                fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            pub trait SecretKey {
                fn as_bytes(&self) -> &[u8];
                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            pub trait DetachedSignature {
                fn as_bytes(&self) -> &[u8];
                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            pub trait SignedMessage {
                fn as_bytes(&self) -> Vec<u8>;
                fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
                where
                    Self: Sized;
            }

            // Implement traits for our types
            impl PublicKey for mldsa65::PublicKey {
                fn as_bytes(&self) -> &[u8] {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    if bytes.len() != 1952 {
                        return Err(QantoNativeCryptoError::InvalidKeyLength {
                            expected: 1952,
                            actual: bytes.len(),
                        });
                    }
                    let mut arr = [0u8; 1952];
                    arr.copy_from_slice(bytes);
                    Ok(mldsa65::PublicKey::new(QantoPQPublicKey(arr.to_vec())))
                }
            }

            impl SecretKey for mldsa65::SecretKey {
                fn as_bytes(&self) -> &[u8] {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    if bytes.len() != 4032 {
                        return Err(QantoNativeCryptoError::InvalidKeyLength {
                            expected: 4032,
                            actual: bytes.len(),
                        });
                    }
                    let mut arr = [0u8; 4032];
                    arr.copy_from_slice(bytes);
                    Ok(mldsa65::SecretKey::new(QantoPQPrivateKey(arr.to_vec())))
                }
            }

            impl DetachedSignature for mldsa65::DetachedSignature {
                fn as_bytes(&self) -> &[u8] {
                    self.as_bytes()
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    if bytes.len() != 3309 {
                        return Err(QantoNativeCryptoError::InvalidSignatureLength {
                            expected: 3309,
                            actual: bytes.len(),
                        });
                    }
                    let mut arr = [0u8; 3309];
                    arr.copy_from_slice(bytes);
                    Ok(mldsa65::DetachedSignature::new(QantoPQSignature(
                        arr.to_vec(),
                    )))
                }
            }

            impl SignedMessage for mldsa65::SignedMessage {
                fn as_bytes(&self) -> Vec<u8> {
                    let mut result = Vec::new();
                    result.extend_from_slice(&(self.signature.0.len() as u32).to_le_bytes());
                    result.extend_from_slice(&self.signature.0);
                    result.extend_from_slice(&self.message);
                    result
                }

                fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                    if bytes.len() < 4 {
                        return Err(QantoNativeCryptoError::InvalidFormat);
                    }
                    let sig_len =
                        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
                    if bytes.len() < 4 + sig_len {
                        return Err(QantoNativeCryptoError::InvalidFormat);
                    }
                    let signature = QantoPQSignature(bytes[4..4 + sig_len].to_vec());
                    let message = bytes[4 + sig_len..].to_vec();
                    Ok(Self { signature, message })
                }
            }
        }
    }
}

/// P-256 ECDSA Compatibility Module
/// Provides interfaces compatible with p256 crate
pub mod p256_compat {
    use super::*;

    pub mod ecdsa {
        use super::*;

        #[derive(Clone, Serialize, Deserialize)]
        pub struct SigningKey {
            inner: QantoP256PrivateKey,
        }

        #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
        pub struct VerifyingKey {
            inner: QantoP256PublicKey,
        }

        #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
        pub struct Signature {
            inner: QantoP256Signature,
        }

        impl SigningKey {
            pub fn verifying_key(&self) -> VerifyingKey {
                VerifyingKey {
                    inner: self.inner.public_key(),
                }
            }

            pub fn to_bytes(&self) -> [u8; 32] {
                *self.inner.as_bytes()
            }
        }

        impl VerifyingKey {
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                let inner = QantoP256PublicKey::from_bytes(bytes)?;
                Ok(Self { inner })
            }

            pub fn to_bytes(&self) -> [u8; 64] {
                *self.inner.as_bytes()
            }
        }

        impl Signature {
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.len() != 64 {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                let mut sig_bytes = [0u8; 64];
                sig_bytes.copy_from_slice(bytes);
                let inner = QantoP256Signature(sig_bytes);
                Ok(Self { inner })
            }

            pub fn to_bytes(&self) -> [u8; 64] {
                self.inner.0
            }
        }

        /// Signature trait for P-256
        pub mod signature {
            use super::*;

            pub trait Signer<S> {
                fn sign(&self, message: &[u8]) -> S;
            }

            pub trait Verifier<S> {
                type Error;
                fn verify(
                    &self,
                    message: &[u8],
                    signature: &S,
                ) -> std::result::Result<(), Self::Error>;
            }

            impl Signer<Signature> for SigningKey {
                fn sign(&self, message: &[u8]) -> Signature {
                    let inner = self.inner.sign(message).unwrap();
                    Signature { inner }
                }
            }

            impl Verifier<Signature> for VerifyingKey {
                type Error = QantoNativeCryptoError;

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
}

/// Libp2p Identity Compatibility
/// Provides compatibility with libp2p identity types
// Additional pqcrypto compatibility modules
pub mod pqcrypto_classicmceliece {
    use super::*;

    pub mod mceliece8192128 {
        use super::*;

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Ciphertext {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(PublicKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(SecretKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::Ciphertext for Ciphertext {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(Ciphertext {
                    inner: bytes.to_vec(),
                })
            }
        }

        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use qanto-native McEliece
            let pk = PublicKey {
                inner: vec![0u8; 1357824],
            };
            let sk = SecretKey {
                inner: vec![0u8; 13892],
            };
            (pk, sk)
        }

        pub fn encapsulate(_public_key: &PublicKey) -> (Ciphertext, Vec<u8>) {
            // Placeholder implementation
            let ct = Ciphertext {
                inner: vec![0u8; 240],
            };
            let ss = vec![0u8; 32];
            (ct, ss)
        }

        pub fn decapsulate(_ciphertext: &Ciphertext, _secret_key: &SecretKey) -> Vec<u8> {
            // Placeholder implementation
            vec![0u8; 32]
        }
    }
}

pub mod pqcrypto_falcon {
    use super::*;

    pub mod falcon1024 {
        use super::*;

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct DetachedSignature {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::sign::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(PublicKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::sign::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(SecretKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::sign::DetachedSignature for DetachedSignature {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(DetachedSignature {
                    inner: bytes.to_vec(),
                })
            }
        }

        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use qanto-native Falcon
            let pk = PublicKey {
                inner: vec![0u8; 1793],
            };
            let sk = SecretKey {
                inner: vec![0u8; 2305],
            };
            (pk, sk)
        }

        pub fn detached_sign(_message: &[u8], _secret_key: &SecretKey) -> DetachedSignature {
            // Placeholder implementation
            DetachedSignature {
                inner: vec![0u8; 1330],
            }
        }

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

pub mod pqcrypto_hqc {
    use super::*;

    pub mod hqc256 {
        use super::*;

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Ciphertext {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(PublicKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(SecretKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::Ciphertext for Ciphertext {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(Ciphertext {
                    inner: bytes.to_vec(),
                })
            }
        }

        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use qanto-native HQC
            let pk = PublicKey {
                inner: vec![0u8; 7245],
            };
            let sk = SecretKey {
                inner: vec![0u8; 7285],
            };
            (pk, sk)
        }

        pub fn encapsulate(_public_key: &PublicKey) -> (Ciphertext, Vec<u8>) {
            // Placeholder implementation
            let ct = Ciphertext {
                inner: vec![0u8; 14469],
            };
            let ss = vec![0u8; 64];
            (ct, ss)
        }

        pub fn decapsulate(_ciphertext: &Ciphertext, _secret_key: &SecretKey) -> Vec<u8> {
            // Placeholder implementation
            vec![0u8; 64]
        }
    }
}

pub mod pqcrypto_mldsa {

    pub mod mldsa65 {
        pub use crate::qanto_compat::pqcrypto_compat::mldsa65::*;
    }
}

pub mod pqcrypto_mlkem {
    use super::*;

    pub mod mlkem1024 {
        use super::*;

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PublicKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct SecretKey {
            inner: Vec<u8>,
        }

        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Ciphertext {
            inner: Vec<u8>,
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::PublicKey for PublicKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(PublicKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::SecretKey for SecretKey {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(SecretKey {
                    inner: bytes.to_vec(),
                })
            }
        }

        impl crate::qanto_compat::pqcrypto_traits::kem::Ciphertext for Ciphertext {
            fn as_bytes(&self) -> &[u8] {
                &self.inner
            }

            fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
                if bytes.is_empty() {
                    return Err(QantoNativeCryptoError::InvalidFormat);
                }
                Ok(Ciphertext {
                    inner: bytes.to_vec(),
                })
            }
        }

        pub fn keypair() -> (PublicKey, SecretKey) {
            // Placeholder implementation - would use qanto-native ML-KEM
            let pk = PublicKey {
                inner: vec![0u8; 1568],
            };
            let sk = SecretKey {
                inner: vec![0u8; 3168],
            };
            (pk, sk)
        }

        pub fn encapsulate(_public_key: &PublicKey) -> (Ciphertext, Vec<u8>) {
            // Placeholder implementation
            let ct = Ciphertext {
                inner: vec![0u8; 1568],
            };
            let ss = vec![0u8; 32];
            (ct, ss)
        }

        pub fn decapsulate(_ciphertext: &Ciphertext, _secret_key: &SecretKey) -> Vec<u8> {
            // Placeholder implementation
            vec![0u8; 32]
        }
    }
}

pub mod pqcrypto_traits {
    use super::*;

    pub mod kem {
        use super::*;

        pub trait PublicKey {
            fn as_bytes(&self) -> &[u8];
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }

        pub trait SecretKey {
            fn as_bytes(&self) -> &[u8];
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }

        pub trait Ciphertext {
            fn as_bytes(&self) -> &[u8];
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }

        pub trait SharedSecret {
            fn as_bytes(&self) -> &[u8];
            fn from_bytes(bytes: &[u8]) -> ::std::result::Result<Self, QantoNativeCryptoError>
            where
                Self: Sized;
        }
    }

    pub mod sign {
        pub use crate::qanto_compat::pqcrypto_compat::traits::sign::*;
    }
}

pub mod libp2p_identity {
    use super::*;

    #[derive(Clone, Debug)]
    pub enum Keypair {
        Ed25519(ed25519_dalek::SigningKey),
    }

    impl Keypair {
        pub fn generate_ed25519() -> Self {
            let mut rng = rand::thread_rng();
            let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
            Self::Ed25519(signing_key)
        }

        pub fn public(&self) -> PublicKey {
            match self {
                Self::Ed25519(key) => PublicKey::Ed25519(key.verifying_key()),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum PublicKey {
        Ed25519(ed25519_dalek::VerifyingKey),
    }

    impl PublicKey {
        pub fn to_peer_id(&self) -> PeerId {
            match self {
                Self::Ed25519(key) => {
                    let key_bytes = key.to_bytes();
                    let hash = my_blockchain::qanto_standalone::hash::qanto_hash(&key_bytes);
                    PeerId(hash.as_bytes().to_vec())
                }
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct PeerId(Vec<u8>);

    impl PeerId {
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, QantoNativeCryptoError> {
            if bytes.is_empty() {
                return Err(QantoNativeCryptoError::InvalidFormat);
            }
            Ok(Self(bytes.to_vec()))
        }

        pub fn to_bytes(&self) -> Vec<u8> {
            self.0.clone()
        }
    }

    impl fmt::Display for PeerId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "PeerId({})", hex::encode(&self.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qanto_compat::ed25519_dalek::Signer;
    use rand::thread_rng;

    #[test]
    fn test_ed25519_compatibility() {
        let mut rng = thread_rng();
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
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

        assert!(pqcrypto_compat::mldsa65::verify_detached_signature(
            message,
            &signature,
            &public_key
        )
        .is_ok());
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
}
