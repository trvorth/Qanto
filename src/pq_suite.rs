// pq_suite.rs
// ΛΣ-ΩMEGA™ Post-Quantum Cryptography Suite — core API surface
// Provides a thin, stable abstraction over Qanto's native PQ primitives
// without altering existing data types.

use crate::post_quantum_crypto::{
    generate_pq_keypair, KeyRotationPolicy, QantoPQPrivateKey, QantoPQPublicKey,
};
use crate::types::QuantumResistantSignature;
use std::env;

/// Signature algorithm selector for the PQ suite.
/// Currently only `Native` is implemented (backed by Qanto's Dilithium-like primitives).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SignatureAlgo {
    #[default]
    Native,
}

/// Core PQ Suite configuration and helpers.
#[derive(Clone, Debug, Default)]
pub struct PQSuite {
    pub algo: SignatureAlgo,
    pub key_rotation: KeyRotationPolicy,
}

impl PQSuite {
    /// Initialize from environment.
    /// Uses `PQ_SIGNATURE_ALGO` env var, defaults to `native`.
    pub fn from_env() -> Self {
        let algo = match env::var("PQ_SIGNATURE_ALGO")
            .unwrap_or_else(|_| "native".to_string())
            .to_lowercase()
            .as_str()
        {
            "native" | "dilithium" => SignatureAlgo::Native,
            _ => SignatureAlgo::Native,
        };
        Self {
            algo,
            ..Default::default()
        }
    }

    /// Generate a PQ signature keypair (public, private).
    pub fn generate_keypair(
        &self,
        seed: Option<[u8; 32]>,
    ) -> Result<(QantoPQPublicKey, QantoPQPrivateKey), String> {
        generate_pq_keypair(seed).map_err(|_| "PQ key generation failed".to_string())
    }

    /// Sign a message producing a QuantumResistantSignature wrapper.
    pub fn sign(
        &self,
        private_key: &QantoPQPrivateKey,
        message: &[u8],
    ) -> Result<QuantumResistantSignature, String> {
        QuantumResistantSignature::sign(private_key, message)
    }

    /// Verify a QuantumResistantSignature against a message.
    pub fn verify(&self, signature: &QuantumResistantSignature, message: &[u8]) -> bool {
        signature.verify(message)
    }

    /// Check if current policy indicates key rotation.
    pub fn should_rotate(&self) -> bool {
        self.key_rotation.needs_rotation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let suite = PQSuite::default();
        let (_pk, sk) = suite.generate_keypair(None).expect("keygen");

        let msg = b"hello pq suite";
        let sig = suite.sign(&sk, msg).expect("sign");
        assert!(suite.verify(&sig, msg));
    }
}
