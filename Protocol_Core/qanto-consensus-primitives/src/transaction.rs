use crate::record::{
    CanonicalRecord, DecodedRecord, FieldSpec, RecordError, SequenceSemantics, SequenceSpec,
    WireType,
};
use crate::{hash_v2, DomainV2, HashV2, TxIdV2};

/// Error returned when a transaction fails consensus validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionError {
    /// The transaction's canonical encoding violated a rule.
    Record(RecordError),
    /// The transaction is missing a required field.
    MissingRequiredField(u16),
    /// The transaction contains an unknown field.
    UnknownField(u16),
    /// The transaction has invalid coinbase parameters.
    InvalidCoinbase,
    /// The transaction has an invalid signature.
    InvalidSignature,
    /// The transaction has insufficient funds for fees.
    InsufficientFees,
    /// The transaction has an invalid nonce.
    InvalidNonce,
}

impl From<RecordError> for TransactionError {
    fn from(err: RecordError) -> Self {
        TransactionError::Record(err)
    }
}

impl core::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TransactionError::Record(e) => write!(f, "transaction record error: {e}"),
            TransactionError::MissingRequiredField(id) => write!(f, "missing required field {id}"),
            TransactionError::UnknownField(id) => write!(f, "unknown field {id}"),
            TransactionError::InvalidCoinbase => write!(f, "invalid coinbase transaction"),
            TransactionError::InvalidSignature => write!(f, "invalid transaction signature"),
            TransactionError::InsufficientFees => write!(f, "insufficient fees"),
            TransactionError::InvalidNonce => write!(f, "invalid transaction nonce"),
        }
    }
}

impl std::error::Error for TransactionError {}

/// Canonical transaction layout for QIP-002
///
/// Field 1: VERSION (u32)
/// Field 2: NONCE (u64)
/// Field 3: GAS_PRICE (u64)
/// Field 4: GAS_LIMIT (u64)
/// Field 5: TO (address - bytes20)
/// Field 6: VALUE (u128)
/// Field 7: DATA (bytes)
/// Field 8: SIGNATURE (bytes - 65 bytes for secp256k1)
/// Field 9: IS_COINBASE (bool)
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transaction {
    pub version: u32,
    pub nonce: u64,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub to: [u8; 20],
    pub value: u128,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
    pub is_coinbase: bool,
}

impl Transaction {
    /// Create a new transaction with default values.
    pub fn new() -> Self {
        Self {
            version: 1,
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,
            to: [0u8; 20],
            value: 0,
            data: Vec::new(),
            signature: Vec::new(),
            is_coinbase: false,
        }
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

impl Transaction {
    /// Create a new coinbase transaction.
    pub fn new_coinbase(block_reward: u128, data: Vec<u8>) -> Self {
        Self {
            version: 1,
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,
            to: [0u8; 20], // Empty address for coinbase
            value: block_reward,
            data,
            signature: Vec::new(),
            is_coinbase: true,
        }
    }

    /// Validate the transaction's canonical encoding and consensus rules.
    ///
    /// # Arguments
    /// * `chain_id` - The chain identifier
    /// * `schema` - The schema version
    ///
    /// # Returns
    /// * `Ok(())` if the transaction is valid
    /// * `Err(TransactionError)` if validation fails
    pub fn validate_consensus(&self, chain_id: u32, schema: u16) -> Result<(), TransactionError> {
        // Encode the transaction as a canonical record
        let record = self.to_canonical_record(chain_id, schema)?;
        let bytes = record.finish();

        // Decode it back to ensure it's canonical
        let decoded = DecodedRecord::decode(&bytes)?;

        // Validate against the expected schema
        let spec = Transaction::transaction_spec();
        DecodedRecord::decode_with_schema(&bytes, &spec)?;

        // Additional consensus validations
        self.validate_coinbase_rules()?;
        self.validate_signature()?;
        self.validate_nonce()?;
        self.validate_gas_limits()?;
        self.validate_value_non_negative()?;

        Ok(())
    }

    /// Convert the transaction to a canonical record for hashing.
    fn to_canonical_record(
        &self,
        _chain_id: u32,
        _schema: u16,
    ) -> Result<CanonicalRecord, RecordError> {
        let mut record = CanonicalRecord::new();

        // Field 1: VERSION (u32)
        record.push_u32(1, self.version)?;

        // Field 2: NONCE (u64)
        record.push_u64(2, self.nonce)?;

        // Field 3: GAS_PRICE (u64)
        record.push_u64(3, self.gas_price)?;

        // Field 4: GAS_LIMIT (u64)
        record.push_u64(4, self.gas_limit)?;

        // Field 5: TO (address - bytes20)
        record.push_bytes(5, &self.to)?;

        // Field 6: VALUE (u128)
        record.push_u128(6, self.value)?;

        // Field 7: DATA (bytes)
        record.push_bytes(7, &self.data)?;

        // Field 8: SIGNATURE (bytes)
        record.push_bytes(8, &self.signature)?;

        // Field 9: IS_COINBASE (bool)
        record.push_bool(9, self.is_coinbase)?;

        Ok(record)
    }

    /// Validate coinbase-specific rules.
    fn validate_coinbase_rules(&self) -> Result<(), TransactionError> {
        if self.is_coinbase {
            // Coinbase transaction must have empty 'to' address
            if self.to != [0u8; 20] {
                return Err(TransactionError::InvalidCoinbase);
            }
            // Coinbase transaction typically has no signature in many implementations
            // but we'll allow it for flexibility
        } else {
            // Regular transaction must not be coinbase
            // Additional validation would go here (signatures, nonce, etc.)
        }
        Ok(())
    }

    /// Validate the transaction signature.
    fn validate_signature(&self) -> Result<(), TransactionError> {
        // In a full implementation, we would verify the cryptographic signature
        // For now, we'll do basic validation
        if !self.is_coinbase && self.signature.is_empty() {
            return Err(TransactionError::InvalidSignature);
        }
        // Signature should be 65 bytes for secp256k1 (R + S + V)
        if !self.signature.is_empty() && self.signature.len() != 65 {
            return Err(TransactionError::InvalidSignature);
        }
        Ok(())
    }

    /// Validate the transaction nonce.
    fn validate_nonce(&self) -> Result<(), TransactionError> {
        // Nonce validation would typically check against account nonce
        // For now, we'll just ensure it's not negative (always true for u64)
        Ok(())
    }

    /// Validate gas limits.
    fn validate_gas_limits(&self) -> Result<(), TransactionError> {
        // Coinbase transactions don't have gas limits
        if self.is_coinbase {
            return Ok(());
        }
        // Gas limit must be greater than gas used
        // For simplicity, we'll just check that gas_limit is reasonable
        if self.gas_limit == 0 {
            return Err(TransactionError::InsufficientFees);
        }
        // In a real implementation, we'd check gas_price * gas_limit against account balance
        Ok(())
    }

    /// Validate that value is non-negative (always true for u128, but kept for completeness)
    fn validate_value_non_negative(&self) -> Result<(), TransactionError> {
        // u128 is always non-negative
        Ok(())
    }

    /// Get the field specification for transaction validation.
    fn transaction_spec() -> [FieldSpec; 9] {
        [
            FieldSpec {
                id: 1,
                wire_type: WireType::U32,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 2,
                wire_type: WireType::U64,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 3,
                wire_type: WireType::U64,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 4,
                wire_type: WireType::U64,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 5,
                wire_type: WireType::Bytes,
                required: true,
                sequence: None,
                // Note: We're not validating the exact length (20) here for simplicity
                // A full implementation would add custom validation
            },
            FieldSpec {
                id: 6,
                wire_type: WireType::U128,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 7,
                wire_type: WireType::Bytes,
                required: false, // Data is optional
                sequence: None,
            },
            FieldSpec {
                id: 8,
                wire_type: WireType::Bytes,
                required: false, // Signature is optional for some tx types
                sequence: None,
            },
            FieldSpec {
                id: 9,
                wire_type: WireType::Bool,
                required: true,
                sequence: None,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DomainV2, HashV2};

    const TEST_CHAIN_ID: u32 = 1_234;
    const TEST_SCHEMA: u16 = 2;

    #[test]
    fn test_regular_transaction_validation() {
        let mut tx = Transaction::new();
        tx.version = 1;
        tx.nonce = 1;
        tx.gas_price = 20_000_000_000; // 20 gwei
        tx.gas_limit = 100_000;
        // Set a dummy address
        tx.to[0] = 0xab;
        tx.value = 1_000_000_000_000_000_000; // 1 QANTO
        tx.data = vec![0xde, 0xad, 0xbe, 0xef];
        // Dummy signature (65 bytes)
        tx.signature = vec![0xab; 65];
        tx.is_coinbase = false;

        assert!(tx.validate_consensus(TEST_CHAIN_ID, TEST_SCHEMA).is_ok());
    }

    #[test]
    fn test_coinbase_transaction_validation() {
        let tx = Transaction::new_coinbase(
            50_000_000_000_000_000_000,   // 50 QANTO block reward
            vec![0x00, 0x01, 0x02, 0x03], // Some coinbase data
        );

        let result = tx.validate_consensus(TEST_CHAIN_ID, TEST_SCHEMA);
        eprintln!("Coinbase validation result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_coinbase_with_nonempty_to() {
        let mut tx = Transaction::new_coinbase(50_000_000_000_000_000_000, vec![]);
        // Make it invalid by setting non-empty 'to' address
        tx.to[0] = 0xFF;

        assert!(matches!(
            tx.validate_consensus(TEST_CHAIN_ID, TEST_SCHEMA),
            Err(TransactionError::InvalidCoinbase)
        ));
    }

    #[test]
    fn test_transaction_with_empty_signature_when_not_coinbase() {
        let mut tx = Transaction::new();
        tx.version = 1;
        tx.nonce = 1;
        tx.gas_price = 20_000_000_000;
        tx.gas_limit = 100_000;
        tx.to[0] = 0xab;
        tx.value = 1_000_000_000_000_000_000;
        tx.data = vec![];
        tx.signature = vec![]; // Empty signature
        tx.is_coinbase = false;

        assert!(matches!(
            tx.validate_consensus(TEST_CHAIN_ID, TEST_SCHEMA),
            Err(TransactionError::InvalidSignature)
        ));
    }
}
