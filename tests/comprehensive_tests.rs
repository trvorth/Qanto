//! Comprehensive Tests
//!
//! This module consolidates all remaining test functionality from various src modules
//! including config, metrics, diagnostics, storage, and other core components.

#[cfg(test)]
mod config_tests {
    use qanto::config::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(!config.network_id.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config.network_id, deserialized.network_id);
    }
}

#[cfg(test)]
mod wallet_tests {
    use qanto::wallet::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new().unwrap();
        assert!(!wallet.address().is_empty());
    }

    #[test]
    fn test_wallet_address() {
        let wallet = Wallet::new().unwrap();
        let address = wallet.address();
        assert!(!address.is_empty());
    }
}

#[cfg(test)]
mod transaction_basic_tests {
    use qanto::transaction::*;
    use qanto::types::*;
    use std::collections::HashMap;

    #[test]
    fn test_transaction_creation() {
        let tx = Transaction {
            id: "test_tx".to_string(),
            sender: "sender".to_string(),
            receiver: "receiver".to_string(),
            amount: 100,
            fee: 10,
            gas_limit: 21000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: vec![],
            outputs: vec![],
            timestamp: 0,
            metadata: HashMap::new(),
            signature: QuantumResistantSignature {
                signer_public_key: vec![],
                signature: vec![],
            },
            fee_breakdown: None,
        };
        assert_eq!(tx.amount, 100);
        assert_eq!(tx.fee, 10);
    }
}

#[cfg(test)]
mod types_tests {
    use qanto::types::*;

    #[test]
    fn test_quantum_resistant_signature() {
        let sig = QuantumResistantSignature {
            signer_public_key: vec![1, 2, 3],
            signature: vec![4, 5, 6],
        };
        assert_eq!(sig.signer_public_key.len(), 3);
        assert_eq!(sig.signature.len(), 3);
    }

    #[test]
    fn test_homomorphic_encrypted() {
        let encrypted = HomomorphicEncrypted {
            ciphertext: vec![1, 2, 3],
            public_key: vec![4, 5, 6],
        };
        assert_eq!(encrypted.ciphertext.len(), 3);
        assert_eq!(encrypted.public_key.len(), 3);
    }
}
