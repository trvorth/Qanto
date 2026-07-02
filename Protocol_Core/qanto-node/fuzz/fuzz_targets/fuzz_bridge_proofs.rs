#![no_main]
use libfuzzer_sys::fuzz_target;
use qanto::post_quantum_crypto::{QantoPQPublicKey, QantoPQSignature};

#[derive(serde::Serialize, serde::Deserialize)]
struct FuzzBridgeProof {
    validators: Vec<String>,
    signatures: Vec<Vec<u8>>,
    message: Vec<u8>,
}

fuzz_target!(|data: &[u8]| {
    // Attempt to deserialize our fuzzing payload using bincode
    if let Ok(proof) = bincode::deserialize::<FuzzBridgeProof>(data) {
        for (i, signature_bytes) in proof.signatures.iter().enumerate() {
            if i >= proof.validators.len() {
                break;
            }

            // Fuzz hex decoding of validator public key
            let validator_pubkey_hex = &proof.validators[i];
            if let Ok(validator_pubkey) = hex::decode(validator_pubkey_hex) {
                // Fuzz post-quantum public key construction
                if let Ok(public_key) = QantoPQPublicKey::from_bytes(&validator_pubkey) {
                    // Fuzz post-quantum signature construction
                    if let Ok(pq_signature) = QantoPQSignature::from_bytes(signature_bytes) {
                        // Fuzz verification logic
                        let _ = public_key.verify(&proof.message, &pq_signature);
                    }
                }
            }
        }
    }
});
