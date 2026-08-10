//! Byte-exact, dependency-light building blocks for the inactive Qanto V2 protocol.
//!
//! This crate deliberately does not integrate with V1 block or transaction acceptance. It is the
//! portable correctness oracle for QIP-002 vectors while the V2 activation and migration remain
//! separate, reviewed protocol work.

#![forbid(unsafe_code)]

mod hash;
mod pow;
mod record;

pub use hash::{
    block_id_v2, encode_hash_frame_v2, hash_v2, state_root_v2, transaction_id_v2, BlockIdV2,
    DomainV2, HashSuiteV2, HashV2, StateRootV2, TxIdV2, WorkHashV2, HASH_FRAME_HEADER_LEN,
    HASH_V2_MAGIC,
};
pub use pow::{qanhash_v2, verify_qanhash_v2, work_meets_difficulty_v2, work_target_v2, PowError};
pub use record::{
    CanonicalRecord, DecodedField, DecodedRecord, FieldSpec, RecordError, SequenceElements,
    SequenceSemantics, SequenceSpec, SequenceView, ValueError, WireType,
};
