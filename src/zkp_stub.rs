// Lightweight ZKP stub used when the `zk` feature is disabled.
// Provides minimal types and async APIs so modules depending on `crate::zkp` can compile
// without pulling in Arkworks or heavy cryptographic dependencies.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ZKProofType {
    RangeProof,
    MembershipProof,
    BalanceProof,
    ComputationProof,
    IdentityProof,
    VotingProof,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZKProof {
    pub proof: Vec<u8>,
    pub public_inputs: Vec<Vec<u8>>,
    pub proof_type: ZKProofType,
    pub vk_hash: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Debug, Default)]
pub struct ZKProofSystem;

impl ZKProofSystem {
    pub fn new() -> Self {
        Self
    }

    pub async fn initialize(&self) -> Result<()> {
        // No-op in stub
        Ok(())
    }

    pub async fn verify_proof(&self, _proof: &ZKProof) -> Result<bool> {
        // Deterministically return false in stub to avoid accidental acceptance
        Ok(false)
    }

    pub async fn generate_proof(&self, proof_type: ZKProofType, data: &[u8]) -> Result<ZKProof> {
        // Produce a deterministic, placeholder proof
        let public_inputs = if data.is_empty() {
            vec![]
        } else {
            vec![data.to_vec()]
        };
        Ok(ZKProof {
            proof: vec![],
            public_inputs,
            proof_type,
            vk_hash: vec![],
            timestamp: 0,
        })
    }

    pub async fn generate_range_proof(&self, value: u64, min: u64, max: u64) -> Result<ZKProof> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&value.to_le_bytes());
        buf.extend_from_slice(&min.to_le_bytes());
        buf.extend_from_slice(&max.to_le_bytes());
        self.generate_proof(ZKProofType::RangeProof, &buf).await
    }

    pub async fn generate_membership_proof(&self, element: u64, _set: Vec<u64>) -> Result<ZKProof> {
        self.generate_proof(ZKProofType::MembershipProof, &element.to_le_bytes())
            .await
    }

    pub async fn generate_balance_proof(
        &self,
        inputs: Vec<u64>,
        outputs: Vec<u64>,
    ) -> Result<ZKProof> {
        let input_sum: u64 = inputs.iter().copied().sum();
        let output_sum: u64 = outputs.iter().copied().sum();
        if input_sum != output_sum {
            return Err(anyhow!("stub balance check failed: inputs != outputs"));
        }
        self.generate_proof(ZKProofType::BalanceProof, &input_sum.to_le_bytes())
            .await
    }
}
