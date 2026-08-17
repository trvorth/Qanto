use qanto_consensus_primitives::{
    hash::{BlockIdV2, HashV2},
    record::{DecodedRecord, WireType},
    v2_records::{block_core_v2, BLOCK_CORE_SPECS},
};

const CHAIN_ID: u32 = 0x5141_4E54; // "QANT" as u32
const SCHEMA: u16 = 1; // V2_INITIAL

#[test]
fn test_block_core_v2_includes_state_parent_and_state_root() {
    let state_root = HashV2::from_bytes([1u8; 32]);
    let state_parent = BlockIdV2::from_bytes([2u8; 32]);
    let parent_block_id = BlockIdV2::from_bytes([3u8; 32]);
    let transactions_root = HashV2::from_bytes([4u8; 32]);
    let validator_id = HashV2::from_bytes([5u8; 32]);
    let miner_id = HashV2::from_bytes([6u8; 32]);

    // Current block_core_v2 has 10 parameters (no state_parent)
    // After modification it should have 11 parameters
    let record = block_core_v2(
        2, // version
        parent_block_id,
        state_root,
        transactions_root,
        1234567890,    // timestamp
        1_000_000_000, // difficulty
        1000,          // chain_work
        validator_id,
        miner_id,
        10,           // height
        state_parent, // NEW PARAMETER
    );
    let bytes = record.finish();
    let decoded = DecodedRecord::decode_with_schema(&bytes, BLOCK_CORE_SPECS).unwrap();

    // field 3 = state_root (already exists)
    assert_eq!(
        decoded.field(3).unwrap().as_hash32().unwrap().as_ref(),
        state_root.as_bytes()
    );
    // field 11 = state_parent (new field)
    assert_eq!(
        decoded.field(11).unwrap().as_hash32().unwrap().as_ref(),
        state_parent.as_bytes()
    );
}

#[test]
fn test_block_core_specs_has_state_parent_field() {
    // Verify BLOCK_CORE_SPECS has 11 fields with field 11 being state_parent
    assert_eq!(BLOCK_CORE_SPECS.len(), 11);

    // Field 11 should be Hash32 for state_parent
    let field_11 = &BLOCK_CORE_SPECS[10]; // 0-indexed
    assert_eq!(field_11.id, 11);
    assert_eq!(field_11.wire_type, WireType::Hash32);
    assert!(field_11.required);
}
