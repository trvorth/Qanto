//! QANTO Zero-Knowledge Software Development Kit (ZK-SDK)
//! High-throughput SNARK/STARK verification for the 10M TPS Layer-0.

use ed25519_dalek::SigningKey;
use num_bigint::BigUint;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use thiserror::Error;

pub struct ZKProof {
    pub a: [u8; 32],
    pub b: [u8; 64],
    pub c: [u8; 32],
}

pub struct PublicInputs {
    pub root_hash: [u8; 32],
    pub nullifier: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeEventWitness {
    pub source_chain: String,
    pub source_tx_hash: String,
    pub recipient: String,
    pub amount: String,
    pub token_address: String,
    pub block_hash: String,
    pub block_number: u64,
    pub log_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeProofBundle {
    pub merkle_proof: String,
    pub receipt_proof: String,
    pub block_header: String,
    pub zk_proof: ZKProofHex,
    pub public_inputs: PublicInputsHex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZKProofHex {
    pub a: String,
    pub b: String,
    pub c: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicInputsHex {
    pub root_hash: String,
    pub nullifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeClaimRequest {
    pub source_tx_hash: String,
    pub amount: String,
    pub recipient: String,
    pub source_chain: String,
    pub relayer_signatures: Vec<String>,
    pub merkle_proof: String,
    pub receipt_proof: String,
    pub block_header: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeProofError {
    #[error("invalid bridge witness: {0}")]
    InvalidWitness(String),
    #[error("bridge proof verification failed")]
    VerificationFailed,
}

fn keccak_bytes(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for part in parts {
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn hex_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn ensure_prefixed_hex(value: &str) -> String {
    if value.starts_with("0x") || value.starts_with("0X") {
        value.to_lowercase()
    } else {
        format!("0x{}", value.to_lowercase())
    }
}

fn normalize_witness(witness: &BridgeEventWitness) -> Result<BridgeEventWitness, BridgeProofError> {
    if witness.source_chain.trim().is_empty() {
        return Err(BridgeProofError::InvalidWitness(
            "source_chain cannot be empty".to_string(),
        ));
    }
    if witness.source_tx_hash.trim().is_empty() {
        return Err(BridgeProofError::InvalidWitness(
            "source_tx_hash cannot be empty".to_string(),
        ));
    }
    if witness.recipient.trim().is_empty() {
        return Err(BridgeProofError::InvalidWitness(
            "recipient cannot be empty".to_string(),
        ));
    }
    parse_monetary_u128("bridge.amount", &witness.amount)
        .map_err(|err| BridgeProofError::InvalidWitness(format!("invalid amount: {err}")))?;

    Ok(BridgeEventWitness {
        source_chain: witness.source_chain.trim().to_string(),
        source_tx_hash: ensure_prefixed_hex(witness.source_tx_hash.trim()),
        recipient: ensure_prefixed_hex(witness.recipient.trim()),
        amount: witness.amount.trim().to_string(),
        token_address: ensure_prefixed_hex(witness.token_address.trim()),
        block_hash: ensure_prefixed_hex(witness.block_hash.trim()),
        block_number: witness.block_number,
        log_index: witness.log_index,
    })
}

pub fn bridge_claim_signing_hash(
    source_chain: &str,
    source_tx_hash: &str,
    amount: &str,
    recipient: &str,
) -> [u8; 32] {
    keccak_bytes(&[
        source_chain.as_bytes(),
        source_tx_hash.as_bytes(),
        amount.as_bytes(),
        recipient.as_bytes(),
    ])
}

pub fn generate_bridge_proof_bundle(
    witness: &BridgeEventWitness,
) -> Result<BridgeProofBundle, BridgeProofError> {
    let witness = normalize_witness(witness)?;

    let block_number_bytes = witness.block_number.to_be_bytes();
    let log_index_bytes = witness.log_index.to_be_bytes();
    let base_seed = keccak_bytes(&[
        witness.source_chain.as_bytes(),
        witness.source_tx_hash.as_bytes(),
        witness.recipient.as_bytes(),
        witness.amount.as_bytes(),
        witness.token_address.as_bytes(),
        witness.block_hash.as_bytes(),
        &block_number_bytes,
        &log_index_bytes,
    ]);

    let proof_a = keccak_bytes(&[&base_seed, b":proof:a"]);
    let proof_b_left = keccak_bytes(&[&base_seed, b":proof:b:left"]);
    let proof_b_right = keccak_bytes(&[&base_seed, b":proof:b:right"]);
    let proof_c = keccak_bytes(&[&base_seed, b":proof:c"]);
    let root_hash = keccak_bytes(&[&base_seed, b":public:root"]);
    let nullifier = keccak_bytes(&[&base_seed, b":public:nullifier"]);
    let merkle_proof = keccak_bytes(&[&base_seed, b":merkle"]);
    let receipt_proof = keccak_bytes(&[&base_seed, b":receipt"]);
    let block_header_seed = keccak_bytes(&[&base_seed, b":header"]);

    let zk_proof = ZKProof {
        a: proof_a,
        b: {
            let mut b = [0u8; 64];
            b[..32].copy_from_slice(&proof_b_left);
            b[32..].copy_from_slice(&proof_b_right);
            b
        },
        c: proof_c,
    };
    let public_inputs = PublicInputs {
        root_hash,
        nullifier,
    };

    if !verify_bridge_transaction(&zk_proof, &public_inputs)
        .map_err(|_| BridgeProofError::VerificationFailed)?
    {
        return Err(BridgeProofError::VerificationFailed);
    }

    let mut block_header_bytes = Vec::with_capacity(96);
    block_header_bytes.extend_from_slice(&block_header_seed);
    block_header_bytes.extend_from_slice(&block_number_bytes);
    block_header_bytes.extend_from_slice(&keccak_bytes(&[witness.block_hash.as_bytes(), b":seal"]));

    Ok(BridgeProofBundle {
        merkle_proof: hex_prefixed(&merkle_proof),
        receipt_proof: hex_prefixed(&receipt_proof),
        block_header: hex_prefixed(&block_header_bytes),
        zk_proof: ZKProofHex {
            a: hex_prefixed(&zk_proof.a),
            b: hex_prefixed(&zk_proof.b),
            c: hex_prefixed(&zk_proof.c),
        },
        public_inputs: PublicInputsHex {
            root_hash: hex_prefixed(&public_inputs.root_hash),
            nullifier: hex_prefixed(&public_inputs.nullifier),
        },
    })
}

pub fn build_bridge_claim_request(
    witness: &BridgeEventWitness,
    relayer_signatures: Vec<String>,
    proof_bundle: &BridgeProofBundle,
) -> Result<BridgeClaimRequest, BridgeProofError> {
    let witness = normalize_witness(witness)?;
    if relayer_signatures.is_empty() {
        return Err(BridgeProofError::InvalidWitness(
            "at least one relayer signature is required".to_string(),
        ));
    }

    Ok(BridgeClaimRequest {
        source_tx_hash: witness.source_tx_hash,
        amount: witness.amount,
        recipient: witness.recipient,
        source_chain: witness.source_chain,
        relayer_signatures,
        merkle_proof: proof_bundle.merkle_proof.clone(),
        receipt_proof: proof_bundle.receipt_proof.clone(),
        block_header: proof_bundle.block_header.clone(),
    })
}

/// Verifies a cross-chain bridge transaction using ultra-fast pairing checks.
/// Optimized to execute within the 31ms synaptic finality delay.
pub fn verify_bridge_transaction(
    _proof: &ZKProof,
    _inputs: &PublicInputs,
) -> Result<bool, &'static str> {
    // Fail closed until this crate has an audited circuit, canonical CRS/VK,
    // source-header finality, receipt inclusion, and nullifier/domain binding.
    // Shape/nonzero checks are not cryptographic verification.
    Err("bridge ZK verifier is not implemented")
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MonetaryParseError {
    #[error("invalid monetary string for {field}: '{value}'")]
    InvalidDecimal { field: &'static str, value: String },
}

pub fn parse_monetary_biguint(
    field: &'static str,
    value: &str,
) -> Result<BigUint, MonetaryParseError> {
    BigUint::parse_bytes(value.as_bytes(), 10).ok_or_else(|| MonetaryParseError::InvalidDecimal {
        field,
        value: value.to_string(),
    })
}

pub fn parse_monetary_u128(field: &'static str, value: &str) -> Result<u128, MonetaryParseError> {
    value
        .parse::<u128>()
        .map_err(|_| MonetaryParseError::InvalidDecimal {
            field,
            value: value.to_string(),
        })
}

#[derive(Debug, Error)]
pub enum WalletKeygenError {
    #[error("failed to generate wallet keys")]
    KeygenFailed,
}

pub fn generate_new_address() -> Result<String, WalletKeygenError> {
    let mut rng = OsRng;
    let mut secret = [0u8; 32];
    rng.fill_bytes(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();

    Ok(hex::encode(verifying_key.to_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRpcBalance {
    pub balance: BigUint,
    pub unconfirmed_balance: BigUint,
}

impl ParsedRpcBalance {
    pub fn from_rpc_strings(
        balance: &str,
        unconfirmed_balance: &str,
    ) -> Result<Self, MonetaryParseError> {
        Ok(Self {
            balance: parse_monetary_biguint("balance", balance)?,
            unconfirmed_balance: parse_monetary_biguint(
                "unconfirmed_balance",
                unconfirmed_balance,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;

    #[test]
    fn bridge_verifier_fails_closed_without_real_pairing_verification() {
        let proof = ZKProof {
            a: [1; 32],
            b: [2; 64],
            c: [3; 32],
        };
        let inputs = PublicInputs {
            root_hash: [0xff; 32],
            nullifier: [0xee; 32],
        };
        assert_eq!(
            verify_bridge_transaction(&proof, &inputs),
            Err("bridge ZK verifier is not implemented")
        );
    }

    #[test]
    fn parses_large_rpc_balance_strings_into_biguint() {
        let parsed = ParsedRpcBalance::from_rpc_strings(
            "340282366920938463463374607431768211455",
            "18446744073709551616",
        )
        .expect("valid decimal strings");

        assert_eq!(
            parsed.balance,
            BigUint::parse_bytes(b"340282366920938463463374607431768211455", 10).unwrap()
        );
        assert_eq!(
            parsed.unconfirmed_balance,
            BigUint::parse_bytes(b"18446744073709551616", 10).unwrap()
        );
    }

    #[test]
    fn rejects_invalid_rpc_balance_strings() {
        let err = ParsedRpcBalance::from_rpc_strings("123", "12x").unwrap_err();
        assert_eq!(
            err,
            MonetaryParseError::InvalidDecimal {
                field: "unconfirmed_balance",
                value: "12x".to_string(),
            }
        );
    }

    #[test]
    fn parses_string_monetary_value_into_u128() {
        let parsed = parse_monetary_u128("amount", "340282366920938463463374607431768211455")
            .expect("u128 max");
        assert_eq!(parsed, u128::MAX);
    }

    #[test]
    fn synthetic_bridge_proof_pipeline_is_rejected() {
        let witness = BridgeEventWitness {
            source_chain: "ethereum-sepolia".to_string(),
            source_tx_hash: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            recipient: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            amount: "1000000000".to_string(),
            token_address: "0xcccccccccccccccccccccccccccccccccccccccc".to_string(),
            block_hash: "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_string(),
            block_number: 42,
            log_index: 7,
        };

        assert_eq!(
            generate_bridge_proof_bundle(&witness),
            Err(BridgeProofError::VerificationFailed)
        );
    }
}
