// Test that our new modules can be imported
#[allow(unused_imports)]
use crate::{
    consensus_parameters::{ACTIVATION_HEIGHT, ConsensusParametersV2, ProtocolVersion, SchemaVersion, is_v2_active},
    v2_records::*,
    transaction_v2::*,
    block_v2::*,
    identity_v2::*,
    state_root_v2::*,
};

#[test]
fn test_imports() {
    // Just verify we can reference the types
    let _ = ProtocolVersion::V1;
    let _ = SchemaVersion::V2_INITIAL;
    let _ = ACTIVATION_HEIGHT;
}