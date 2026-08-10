use qanto_consensus_primitives::{
    encode_hash_frame_v2, hash_v2, qanhash_v2, verify_qanhash_v2, work_meets_difficulty_v2,
    work_target_v2, CanonicalRecord, DecodedRecord, DomainV2, FieldSpec, HashV2, PowError,
    RecordError, SequenceSemantics, SequenceSpec, ValueError, WireType,
};

const CHAIN_ID: u32 = 1_234;
const SCHEMA: u16 = 2;

#[test]
fn empty_record_hash_golden_vectors() {
    let payload = CanonicalRecord::new().finish();
    assert_eq!(payload, [0, 0]);
    assert_eq!(
        to_hex(&encode_hash_frame_v2(
            DomainV2::TxBody,
            CHAIN_ID,
            SCHEMA,
            &payload,
        )),
        "51414e544f2d563200010001000004d2000200000000000000020000"
    );
    assert_eq!(
        hash_v2(DomainV2::TxBody, CHAIN_ID, SCHEMA, &payload).to_string(),
        "7853f3384d74b12d27ef701c49f703fbacbc0aea22aabe1abacfd208a6e41c81"
    );
    assert_eq!(
        hash_v2(DomainV2::TxSigning, CHAIN_ID, SCHEMA, &payload).to_string(),
        // The transmitted vector omitted its final nibble (`0`); the complete independently
        // recomputed 32-byte digest was approved before this value was frozen.
        "819d940f5563eb4c5eb0f4b00f25e7617a8d2b5c10124dc996c48d3b337900c0"
    );
}

#[test]
fn trailing_zero_is_committed() {
    let mut one = CanonicalRecord::new();
    one.push_bytes(1, &[0x01]).unwrap();
    let one = one.finish();
    let mut one_zero = CanonicalRecord::new();
    one_zero.push_bytes(1, &[0x01, 0x00]).unwrap();
    let one_zero = one_zero.finish();

    let one_hash = hash_v2(DomainV2::TxBody, CHAIN_ID, SCHEMA, &one);
    let one_zero_hash = hash_v2(DomainV2::TxBody, CHAIN_ID, SCHEMA, &one_zero);

    // The initially transmitted approval vector accidentally omitted its final byte (`cf`). The
    // independently recomputed full 32-byte SHA3-256 value below was explicitly approved.
    assert_eq!(
        one_hash.to_string(),
        "f886884ec49e8eafae284f34bf7f67a3e4e1565a70277c47005fb3cda2fc15cf"
    );
    assert_eq!(
        one_zero_hash.to_string(),
        "1759d4f590d93ccd44dccb7c29b454c65aa4488db50b96b20ee71239bcba4bad"
    );
    assert_ne!(one_hash, one_zero_hash);
}

#[test]
fn domain_chain_and_schema_are_separated() {
    let payload = CanonicalRecord::new().finish();
    let base = hash_v2(DomainV2::TxBody, CHAIN_ID, SCHEMA, &payload);
    assert_ne!(
        base,
        hash_v2(DomainV2::TxSigning, CHAIN_ID, SCHEMA, &payload)
    );
    assert_ne!(
        base,
        hash_v2(DomainV2::TxBody, CHAIN_ID + 1, SCHEMA, &payload)
    );
    assert_ne!(
        base,
        hash_v2(DomainV2::TxBody, CHAIN_ID, SCHEMA + 1, &payload)
    );
}

#[test]
fn malformed_records_fail_closed() {
    let duplicate_fields = [
        0x00, 0x02, // count
        0x00, 0x01, 0x01, 0, 0, 0, 1, 0xaa, // field 1
        0x00, 0x01, 0x01, 0, 0, 0, 1, 0xbb, // duplicate field 1
    ];
    assert!(matches!(
        DecodedRecord::decode(&duplicate_fields),
        Err(RecordError::FieldOutOfOrder {
            previous: 1,
            next: 1
        })
    ));

    let unknown_wire = [0x00, 0x01, 0x00, 0x01, 0xff, 0, 0, 0, 0];
    assert!(matches!(
        DecodedRecord::decode(&unknown_wire),
        Err(RecordError::UnknownWireType {
            field_id: 1,
            wire_type: 0xff
        })
    ));

    let bad_u64_length = [0x00, 0x01, 0x00, 0x01, 0x04, 0, 0, 0, 1, 0];
    assert!(matches!(
        DecodedRecord::decode(&bad_u64_length),
        Err(RecordError::InvalidValue {
            field_id: 1,
            error: ValueError::IncorrectLength { .. }
        })
    ));

    let trailing = [0x00, 0x00, 0xff];
    assert!(matches!(
        DecodedRecord::decode(&trailing),
        Err(RecordError::TrailingBytes {
            offset: 2,
            total: 3
        })
    ));
}

#[test]
fn schema_rejects_unknown_fields_and_noncanonical_sets() {
    let mut unknown = CanonicalRecord::new();
    unknown.push_u8(2, 7).unwrap();
    let schema = [FieldSpec {
        id: 1,
        wire_type: WireType::U8,
        required: false,
        sequence: None,
    }];
    assert!(matches!(
        DecodedRecord::decode_with_schema(&unknown.finish(), &schema),
        Err(RecordError::UnknownField { field_id: 2 })
    ));

    let unsorted_set = [
        0x00, 0x01, // record count
        0x00, 0x01, 0x08, 0, 0, 0, 15, // sequence field
        0x01, 0, 0, 0, 2, // u8, two elements
        0, 0, 0, 1, 2, // first
        0, 0, 0, 1, 1, // second (out of order)
    ];
    let set_schema = [FieldSpec {
        id: 1,
        wire_type: WireType::Sequence,
        required: true,
        sequence: Some(SequenceSpec {
            element_type: WireType::U8,
            semantics: SequenceSemantics::Set,
        }),
    }];
    assert!(matches!(
        DecodedRecord::decode_with_schema(&unsorted_set, &set_schema),
        Err(RecordError::InvalidValue {
            field_id: 1,
            error: ValueError::NonCanonicalSet { index: 1 }
        })
    ));
}

#[test]
fn qanhash_nonce_and_difficulty_golden_vector() {
    let core_hash = HashV2::ZERO;
    let nonce_zero = qanhash_v2(CHAIN_ID, SCHEMA, core_hash, 0);
    let nonce_one = qanhash_v2(CHAIN_ID, SCHEMA, core_hash, 1);
    assert_eq!(
        nonce_zero.to_string(),
        "85fdfd24319c64084ebff75e2d49ae7e7a30992dddaf4a5bceff6c5d01a790db"
    );
    assert_eq!(
        nonce_one.to_string(),
        "21a53926d2bce27d5f543ed3762183e57830cfcb634bd45277d849a2a49e3339"
    );

    let target = work_target_v2(2).unwrap();
    assert_eq!(target[0], 0x7f);
    assert!(target[1..].iter().all(|byte| *byte == 0xff));
    assert!(!work_meets_difficulty_v2(nonce_zero, 2).unwrap());
    assert!(work_meets_difficulty_v2(nonce_one, 2).unwrap());
    assert!(verify_qanhash_v2(CHAIN_ID, SCHEMA, core_hash, 1, nonce_one, 2).unwrap());
    assert_eq!(work_target_v2(0), Err(PowError::ZeroDifficulty));
    assert_eq!(
        work_meets_difficulty_v2(nonce_one, 0),
        Err(PowError::ZeroDifficulty)
    );
}

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
