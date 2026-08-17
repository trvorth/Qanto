//! Byte-exact, dependency-light building blocks for the inactive Qanto V2 protocol.
//!
//! This crate deliberately does not integrate with V1 block or transaction acceptance. It is the
//! portable correctness oracle for QIP-002 vectors while the V2 activation and migration remain
//! separate, reviewed protocol work.

#![forbid(unsafe_code)]

pub mod block;
pub mod block_v2;
pub mod cache;
pub mod consensus_parameters;
pub mod hash;
pub mod identity_v2;
pub mod merkle;
pub mod pow;
pub mod record;
pub mod state;
pub mod state_root_v2;
pub mod transaction;
pub mod transaction_v2;
pub mod v2_records;

pub use block::{Block, BlockError};
pub use block_v2::{
    build_tx_merkle_root_v2, compute_chain_work_v2, compute_pow_work_v2, verify_block_pow_v2,
    BlockCoreV2, BlockCoreV2Params,
};
pub use cache::LruCache;
pub use consensus_parameters::{
    is_v2_active, ConsensusParametersV2, ProtocolVersion, SchemaVersion, ACTIVATION_HEIGHT,
    CONSENSUS_PARAMETERS_SPECS, DOMAIN_REGISTRY,
};
pub use hash::{
    block_id_v2, encode_hash_frame_v2, hash_v2, transaction_id_v2, AccountIdV2, BlockIdV2,
    DomainV2, HashSuiteV2, HashV2, StateRootV2, TxIdV2, ValidatorIdV2, WorkHashV2,
    HASH_FRAME_HEADER_LEN, HASH_V2_MAGIC,
};
pub use merkle::{merkle_root, merkle_root_and_proof, verify_merkle_proof, MerkleError};
pub use pow::{qanhash_v2, verify_qanhash_v2, work_meets_difficulty_v2, work_target_v2, PowError};
pub use record::{
    CanonicalRecord, DecodedField, DecodedRecord, FieldSpec, RecordError, SequenceElements,
    SequenceSemantics, SequenceSpec, SequenceView, ValueError, WireType,
};
pub use state::{StateError, StateTrie};
pub use transaction::{Transaction, TransactionError};
pub use transaction_v2::{TransactionBodyV2, TransactionEnvelopeV2, TransactionSigningV2};

pub use identity_v2::{account_id_v2, identities_equal_constant_time, validator_id_v2};
pub use state_root_v2::state_root_v2;
#[cfg(test)]
pub use transaction_v2::sign_transaction_body_v2;
