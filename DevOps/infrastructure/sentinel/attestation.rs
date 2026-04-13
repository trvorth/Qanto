//! QANTO Hardware Attestation (QSA) Module
//! v1.0.0 - Phase 47

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareQuote {
    pub provider: String, // SGX, SEV, Nitro
    pub version: String,
    pub report_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub enclave_measurements: [u8; 32],
}

pub enum SentinelTier {
    Mobile,
    Industrial,
    VacuumReady, // Orbital Satellites
}

pub struct AttestationEngine {
    pub root_of_trust_pubkey: Vec<u8>,
}

impl AttestationEngine {
    pub fn new(root_of_trust_pubkey: Vec<u8>) -> Self {
        Self {
            root_of_trust_pubkey,
        }
    }

    /// Verifies a hardware quote against the Root of Trust.
    /// Orbital Tier: Requires radiation-hardened TEE signatures.
    pub fn verify_quote(&self, quote: &HardwareQuote, tier: SentinelTier) -> bool {
        // Mock verification logic
        let base_valid = !quote.signature.is_empty() && quote.enclave_measurements != [0u8; 32];
        match tier {
            SentinelTier::VacuumReady => base_valid && quote.version == "RadHard-v1",
            _ => base_valid,
        }
    }

    /// Calculates the PUAI reward multiplier based on attestation status.
    /// Orbital Sentinels (VacuumReady) = 10.0x Multiplier for cosmic stability.
    pub fn get_reward_multiplier(&self, is_attested: bool, tier: SentinelTier) -> f64 {
        if !is_attested { return 1.0; }
        match tier {
            SentinelTier::VacuumReady => 10.0,
            SentinelTier::Industrial => 2.5,
            SentinelTier::Mobile => 1.5,
        }
    }
}
