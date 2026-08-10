use crate::{hash_v2, DomainV2, HashV2, WorkHashV2};
use core::fmt;

/// Proof-of-work validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowError {
    /// Difficulty zero has no mathematical target and is consensus-invalid.
    ZeroDifficulty,
}

impl fmt::Display for PowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QanHashV2 difficulty must be non-zero")
    }
}

impl std::error::Error for PowError {}

/// Normative portable CPU oracle for inactive QanHashV2.
///
/// The payload is a canonical two-field record: field 1 is the 32-byte `BLOCK_CORE` hash and
/// field 2 is the big-endian `u64` nonce. The resulting record is hashed in the `POW_WORK`
/// domain. Accelerator output must be rechecked against this function before it can be trusted.
#[must_use]
pub fn qanhash_v2(chain_id: u32, schema: u16, block_core_hash: HashV2, nonce: u64) -> WorkHashV2 {
    let mut payload = Vec::with_capacity(56);
    payload.extend_from_slice(&2u16.to_be_bytes());
    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.push(0x07); // hash32
    payload.extend_from_slice(&32u32.to_be_bytes());
    payload.extend_from_slice(block_core_hash.as_bytes());
    payload.extend_from_slice(&2u16.to_be_bytes());
    payload.push(0x04); // u64
    payload.extend_from_slice(&8u32.to_be_bytes());
    payload.extend_from_slice(&nonce.to_be_bytes());
    debug_assert_eq!(payload.len(), 56);
    WorkHashV2::from_hash(hash_v2(DomainV2::PowWork, chain_id, schema, &payload))
}

/// Compute `floor((2^256 - 1) / difficulty)` as exactly 32 big-endian bytes.
pub fn work_target_v2(difficulty: u64) -> Result<[u8; 32], PowError> {
    if difficulty == 0 {
        return Err(PowError::ZeroDifficulty);
    }
    let divisor = u128::from(difficulty);
    let mut remainder = 0u128;
    let mut target = [0u8; 32];
    for output in &mut target {
        let dividend = (remainder << 8) | u128::from(u8::MAX);
        *output = u8::try_from(dividend / divisor)
            .expect("base-256 long division quotient digit always fits in u8");
        remainder = dividend % divisor;
    }
    Ok(target)
}

/// Check a work hash against the inclusive V2 target using big-endian integer ordering.
pub fn work_meets_difficulty_v2(work_hash: WorkHashV2, difficulty: u64) -> Result<bool, PowError> {
    let target = work_target_v2(difficulty)?;
    Ok(work_hash.as_bytes() <= &target)
}

/// CPU-recompute a claimed work hash, then apply the inclusive target check.
pub fn verify_qanhash_v2(
    chain_id: u32,
    schema: u16,
    block_core_hash: HashV2,
    nonce: u64,
    claimed_work_hash: WorkHashV2,
    difficulty: u64,
) -> Result<bool, PowError> {
    let computed = qanhash_v2(chain_id, schema, block_core_hash, nonce);
    if computed != claimed_work_hash {
        return Ok(false);
    }
    work_meets_difficulty_v2(computed, difficulty)
}
