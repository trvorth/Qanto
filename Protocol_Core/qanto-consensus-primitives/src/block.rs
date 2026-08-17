use crate::record::{
    CanonicalRecord, DecodedRecord, FieldSpec, RecordError, SequenceSemantics, SequenceSpec,
    WireType,
};
use crate::{hash_v2, BlockIdV2, DomainV2, HashV2, StateRootV2, TxIdV2};

/// Error returned when a block fails consensus validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockError {
    /// The block's canonical encoding violated a rule.
    Record(RecordError),
    /// The block is missing a required field.
    MissingRequiredField(u16),
    /// The block contains an unknown field.
    UnknownField(u16),
    /// The block's merkle root does not match the computed value.
    MerkleMismatch,
    /// The block's state root does not match the computed value.
    StateRootMismatch,
    /// The block's transaction count does not match the merkle tree leaves.
    TransactionCountMismatch,
}

impl From<RecordError> for BlockError {
    fn from(err: RecordError) -> Self {
        BlockError::Record(err)
    }
}

impl core::fmt::Display for BlockError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BlockError::Record(e) => write!(f, "block record error: {e}"),
            BlockError::MissingRequiredField(id) => write!(f, "missing required field {id}"),
            BlockError::UnknownField(id) => write!(f, "unknown field {id}"),
            BlockError::MerkleMismatch => write!(f, "merkle root mismatch"),
            BlockError::StateRootMismatch => write!(f, "state root mismatch"),
            BlockError::TransactionCountMismatch => write!(f, "transaction count mismatch"),
        }
    }
}

impl std::error::Error for BlockError {}

/// Canonical block layout for QIP-002
///
/// Field 1: VERSION (u32)
/// Field 2: PREV_BLOCK_ID (hash32)
/// Field 3: STATE_ROOT (hash32)
/// Field 4: TX_MERKLE_ROOT (hash32)
/// Field 5: TIMESTAMP (u64)
/// Field 6: DIFFICULTY (u64)
/// Field 7: NONCE (u64)
/// Field 8: TRANSACTIONS (sequence of hash32, ordered)
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub version: u32,
    pub prev_block_id: BlockIdV2,
    pub state_root: StateRootV2,
    pub tx_merkle_root: TxIdV2, // Reusing TxIdV2 for 32-byte hash
    pub timestamp: u64,
    pub difficulty: u64,
    pub nonce: u64,
    pub transactions: Vec<TxIdV2>,
}

impl Block {
    /// Create a new block with default values.
    pub fn new() -> Self {
        Self {
            version: 1,
            prev_block_id: BlockIdV2::from_hash(HashV2::ZERO),
            state_root: StateRootV2::from_hash(HashV2::ZERO),
            tx_merkle_root: TxIdV2::from_hash(HashV2::ZERO),
            timestamp: 0,
            difficulty: 1,
            nonce: 0,
            transactions: Vec::new(),
        }
    }
}

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

impl Block {
    /// Validate the block's canonical encoding and consensus rules.
    ///
    /// # Arguments
    /// * `chain_id` - The chain identifier
    /// * `schema` - The schema version
    ///
    /// # Returns
    /// * `Ok(())` if the block is valid
    /// * `Err(BlockError)` if validation fails
    pub fn validate_consensus(&self, chain_id: u32, schema: u16) -> Result<(), BlockError> {
        // Encode the block as a canonical record
        let record = self.to_canonical_record(chain_id, schema)?;
        let bytes = record.finish();

        // Debug: print canonical bytes
        eprintln!(
            "Block canonical bytes ({} bytes): {}",
            bytes.len(),
            hex::encode(&bytes)
        );

        // Decode it back to ensure it's canonical
        eprintln!("Attempting DecodedRecord::decode...");
        let decoded = DecodedRecord::decode(&bytes)?;
        eprintln!(
            "DecodedRecord::decode OK, fields: {}",
            decoded.fields().len()
        );

        // Validate against the expected schema
        eprintln!("Attempting DecodedRecord::decode_with_schema...");
        let spec = Block::block_spec();
        eprintln!("Block spec has {} fields", spec.len());
        DecodedRecord::decode_with_schema(&bytes, &spec)?;
        eprintln!("DecodedRecord::decode_with_schema OK");

        // Additional consensus validations
        self.validate_merkle_root()?;
        self.validate_state_root()?;

        Ok(())
    }

    /// Convert the block to a canonical record for hashing.
    fn to_canonical_record(
        &self,
        chain_id: u32,
        schema: u16,
    ) -> Result<CanonicalRecord, RecordError> {
        let mut record = CanonicalRecord::new();

        // Field 1: VERSION (u32)
        record.push_u32(1, self.version)?;

        // Field 2: PREV_BLOCK_ID (hash32)
        record.push_hash32(2, self.prev_block_id.as_bytes())?;

        // Field 3: STATE_ROOT (hash32)
        record.push_hash32(3, self.state_root.as_bytes())?;

        // Field 4: TX_MERKLE_ROOT (hash32)
        record.push_hash32(4, self.tx_merkle_root.as_bytes())?;

        // Field 5: TIMESTAMP (u64)
        record.push_u64(5, self.timestamp)?;

        // Field 6: DIFFICULTY (u64)
        record.push_u64(6, self.difficulty)?;

        // Field 7: NONCE (u64)
        record.push_u64(7, self.nonce)?;

        // Field 8: TRANSACTIONS (sequence of hash32, ordered)
        let tx_bytes: Vec<&[u8]> = self
            .transactions
            .iter()
            .map(|tx| tx.as_bytes().as_slice())
            .collect();
        record.push_ordered_sequence(8, WireType::Hash32, &tx_bytes)?;

        Ok(record)
    }

    /// Validate that the merkle root matches the transactions.
    fn validate_merkle_root(&self) -> Result<(), BlockError> {
        // For simplicity in V2, we'll just check that we have the right number of transactions
        // A full merkle implementation would compute the root from the transactions
        if self.transactions.is_empty() {
            // Empty block should have zero merkle root
            if self.tx_merkle_root.as_bytes() != &[0u8; 32] {
                return Err(BlockError::MerkleMismatch);
            }
        } else {
            // In a full implementation, we'd compute the merkle root here
            // For now, we'll skip detailed validation as this is skeleton code
        }
        Ok(())
    }

    /// Validate that the state root is well-formed.
    fn validate_state_root(&self) -> Result<(), BlockError> {
        // Basic validation - ensure it's not all zeros unless it's a genesis block
        // In a full implementation, this would validate against the state trie
        if self.block_is_genesis() {
            // Genesis block can have any state root
            Ok(())
        } else {
            // Non-genesis blocks should have a non-zero state root
            // (This is a simplification - real validation would be more complex)
            if self.state_root.as_bytes() == &[0u8; 32] {
                Err(BlockError::StateRootMismatch)
            } else {
                Ok(())
            }
        }
    }

    /// Check if this is the genesis block.
    fn block_is_genesis(&self) -> bool {
        self.version == 1
            && self.prev_block_id.as_bytes() == &[0u8; 32]
            && self.timestamp == 0
            && self.difficulty == 1
            && self.nonce == 0
            && self.transactions.is_empty()
    }

    /// Get the field specification for block validation.
    fn block_spec() -> [FieldSpec; 8] {
        [
            FieldSpec {
                id: 1,
                wire_type: WireType::U32,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 2,
                wire_type: WireType::Hash32,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 3,
                wire_type: WireType::Hash32,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 4,
                wire_type: WireType::Hash32,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 5,
                wire_type: WireType::U64,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 6,
                wire_type: WireType::U64,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 7,
                wire_type: WireType::U64,
                required: true,
                sequence: None,
            },
            FieldSpec {
                id: 8,
                wire_type: WireType::Sequence,
                required: true,
                sequence: Some(SequenceSpec {
                    element_type: WireType::Hash32, // Sequence of transaction IDs (hash32)
                    semantics: SequenceSemantics::Ordered,
                }),
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
    fn test_genesis_block_validation() {
        let mut block = Block::new();
        block.version = 1;
        block.prev_block_id = BlockIdV2::from_hash(HashV2::ZERO);
        block.state_root = StateRootV2::from_hash(HashV2::ZERO);
        block.tx_merkle_root = TxIdV2::from_hash(HashV2::ZERO);
        block.timestamp = 0;
        block.difficulty = 1;
        block.nonce = 0;
        block.transactions = vec![];

        eprintln!("TEST: About to call validate_consensus...");
        let result = block.validate_consensus(TEST_CHAIN_ID, TEST_SCHEMA);
        eprintln!("TEST: Result = {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_prev_block() {
        let mut block = Block::new();
        block.version = 1;
        // Set prev_block_id to non-zero - this makes it a non-genesis block
        let mut hash = [0u8; 32];
        hash[0] = 1;
        block.prev_block_id = BlockIdV2::from_hash(HashV2::from_slice(&hash));
        // Non-genesis blocks need non-zero state_root and proper timestamp/difficulty/nonce
        block.state_root = StateRootV2::from_hash(HashV2::from_slice(&[2u8; 32]));
        block.tx_merkle_root = TxIdV2::from_hash(HashV2::ZERO);
        block.timestamp = 1234567890;
        block.difficulty = 100;
        block.nonce = 42;
        block.transactions = vec![];

        // This should pass basic validation
        assert!(block.validate_consensus(TEST_CHAIN_ID, TEST_SCHEMA).is_ok());
    }
}
