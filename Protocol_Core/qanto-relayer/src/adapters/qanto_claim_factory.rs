//! Qanto claim factory adapter - builds signed bridge claims.

use async_trait::async_trait;
use k256::ecdsa::{signature::hazmat::PrehashSigner, RecoveryId, Signature, SigningKey};
use qanto_zk_sdk::{
    bridge_claim_signing_hash, build_bridge_claim_request, generate_bridge_proof_bundle,
    BridgeEventWitness,
};
use sha3::{Digest, Keccak256};

use crate::error::{RelayerError, RelayerResult};
use crate::ports::{AssetLockedEvent, BridgeClaimRequest, ClaimFactory, RelayerSigner};

/// Adapter that builds signed BridgeClaimRequest from validated events.
pub struct QantoClaimFactory;

#[async_trait]
impl ClaimFactory for QantoClaimFactory {
    async fn build_claim(
        &self,
        event: &AssetLockedEvent,
        source_chain: &str,
        relayer_signer: &dyn RelayerSigner,
    ) -> RelayerResult<BridgeClaimRequest> {
        let witness = BridgeEventWitness {
            source_chain: source_chain.to_string(),
            source_tx_hash: event.transaction_hash.clone(),
            recipient: event.user.clone(),
            amount: event.amount.to_string(),
            token_address: event.token.clone(),
            block_hash: event.block_hash.clone(),
            block_number: event.block_number,
            log_index: event.log_index,
        };

        let proof_bundle = generate_bridge_proof_bundle(&witness)
            .map_err(|e| RelayerError::ClaimConstruction(format!("proof generation: {}", e)))?;

        let signing_hash = bridge_claim_signing_hash(
            &witness.source_chain,
            &witness.source_tx_hash,
            &witness.amount,
            &witness.recipient,
        );

        let signature = relayer_signer
            .sign_prehash(&signing_hash)
            .map_err(|e| RelayerError::Signing(e.to_string()))?;

        let sdk_claim = build_bridge_claim_request(&witness, vec![signature], &proof_bundle)
            .map_err(|e| RelayerError::ClaimConstruction(e.to_string()))?;

        // Serialize the SDK claim into our opaque payload
        let payload = serde_json::to_vec(&sdk_claim)
            .map_err(|e| RelayerError::ClaimConstruction(format!("serialize: {}", e)))?;

        Ok(BridgeClaimRequest { payload })
    }
}

/// Relayer signer implementation using k256.
pub struct K256Signer {
    signing_key: SigningKey,
    address: String,
}

impl K256Signer {
    pub fn new(signing_key: SigningKey) -> Self {
        let address = Self::derive_address(&signing_key);
        Self {
            signing_key,
            address,
        }
    }

    fn derive_address(signing_key: &SigningKey) -> String {
        use k256::ecdsa::VerifyingKey;
        let verifying_key = VerifyingKey::from(signing_key);
        let binding = verifying_key.to_encoded_point(false);
        let pub_bytes = binding.as_bytes();
        let mut hasher = Keccak256::new();
        hasher.update(&pub_bytes[1..]);
        let digest = hasher.finalize();
        format!("0x{}", hex::encode(&digest[12..]))
    }
}

impl RelayerSigner for K256Signer {
    fn address(&self) -> &str {
        &self.address
    }

    fn sign_prehash(&self, prehash: &[u8; 32]) -> RelayerResult<String> {
        let (signature, recovery_id): (Signature, RecoveryId) = self
            .signing_key
            .sign_prehash(prehash)
            .map_err(|e| RelayerError::Signing(e.to_string()))?;

        let mut sig_bytes = signature.to_bytes().to_vec();
        sig_bytes.push(recovery_id.to_byte() + 27);
        Ok(format!("0x{}", hex::encode(sig_bytes)))
    }
}
