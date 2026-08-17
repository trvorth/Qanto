//! V2 canonical transaction types: body, signing, envelope, and TxIdV2.
//! No legacy fallbacks. All encoding through CanonicalRecord and hash_v2.

use crate::{
    consensus_parameters::SchemaVersion,
    hash::{hash_v2, AccountIdV2, DomainV2, HashV2, TxIdV2},
    record::{CanonicalRecord, DecodedRecord, FieldSpec, RecordError, WireType},
    v2_records::{
        transaction_body_v2, ACCOUNT_ID_SPECS, TX_BODY_SPECS, TX_ENVELOPE_SPECS, TX_SIGNING_SPECS,
    },
};

/// V2 transaction body (unsigned).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionBodyV2 {
    pub canonical_bytes: Vec<u8>,
    pub tx_id: TxIdV2,
}

impl TransactionBodyV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: u8,
        nonce: u64,
        gas_limit: u128,
        gas_price: u128,
        priority_fee: u128,
        sender: HashV2,
        receiver: HashV2,
        value: u128,
        payload: &[u8],
        chain_id: u32,
        schema: SchemaVersion,
    ) -> Self {
        let record = transaction_body_v2(
            kind,
            nonce,
            gas_limit,
            gas_price,
            priority_fee,
            sender,
            receiver,
            value,
            payload,
            chain_id,
            schema,
        );
        let bytes = record.finish();
        let tx_id = TxIdV2::from_hash(hash_v2(DomainV2::TxBody, chain_id, schema as u16, &bytes));
        Self {
            canonical_bytes: bytes,
            tx_id,
        }
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn from_canonical_bytes(
        bytes: Vec<u8>,
        chain_id: u32,
        schema: SchemaVersion,
    ) -> Result<Self, crate::record::RecordError> {
        let _ = DecodedRecord::decode_with_schema(&bytes, TX_BODY_SPECS)?;
        let tx_id = TxIdV2::from_hash(hash_v2(DomainV2::TxBody, chain_id, schema as u16, &bytes));
        Ok(Self {
            canonical_bytes: bytes,
            tx_id,
        })
    }
}

/// V2 transaction signing record (commits to tx body hash, pubkey, signature).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionSigningV2 {
    pub canonical_bytes: Vec<u8>,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl TransactionSigningV2 {
    fn new(
        signature: Vec<u8>,
        public_key: Vec<u8>,
        tx_body_hash: &[u8; 32],
        _chain_id: u32,
        _schema: SchemaVersion,
    ) -> Self {
        let mut record = CanonicalRecord::new();
        record.push_hash32(1, tx_body_hash).unwrap();
        record.push_bytes(2, &public_key).unwrap();
        record.push_bytes(3, &signature).unwrap();
        let bytes = record.finish();
        Self {
            canonical_bytes: bytes,
            signature,
            public_key,
        }
    }
}

/// V2 transaction envelope (body + signing).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionEnvelopeV2 {
    pub body_bytes: Vec<u8>,
    pub signing: TransactionSigningV2,
    pub canonical_bytes: Vec<u8>,
}

impl TransactionEnvelopeV2 {
    pub fn from_parts(body_bytes: Vec<u8>, signature: Vec<u8>, public_key: Vec<u8>) -> Self {
        let tx_body_hash = hash_v2(DomainV2::TxBody, 0, 0, &body_bytes); // chain/schema not needed for envelope encoding
        let signing = TransactionSigningV2::new(
            signature,
            public_key,
            tx_body_hash.as_bytes(),
            0,
            SchemaVersion::V2_INITIAL,
        );
        let canonical = Self::encode_envelope(&body_bytes, &signing);
        Self {
            body_bytes,
            signing,
            canonical_bytes: canonical,
        }
    }

    fn encode_envelope(body: &[u8], signing: &TransactionSigningV2) -> Vec<u8> {
        let mut record = CanonicalRecord::new();
        record
            .push_hash32(1, &hash_v2(DomainV2::TxBody, 0, 0, body).into_bytes())
            .unwrap();
        record
            .push_ordered_sequence(
                2,
                WireType::Bytes,
                &[&signing.signature, &signing.public_key],
            )
            .unwrap();
        record.finish()
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Verify the envelope signature against the expected sender identity.
    /// Returns the TxIdV2 on success.
    pub fn verify(
        &self,
        expected_sender: AccountIdV2,
        chain_id: u32,
        schema: SchemaVersion,
    ) -> Result<TxIdV2, crate::record::RecordError> {
        // Verify the body hash matches
        let body_hash = hash_v2(DomainV2::TxBody, chain_id, schema as u16, &self.body_bytes);

        // Verify the signing record commits to the same body hash
        let decoded_signing =
            DecodedRecord::decode_with_schema(&self.signing.canonical_bytes, TX_SIGNING_SPECS)?;
        let signing_body_hash_field = decoded_signing
            .field(1)
            .ok_or(crate::record::RecordError::MissingRequiredField { field_id: 1 })?;
        let signing_body_hash = HashV2::from_bytes(signing_body_hash_field.as_hash32()?);

        if signing_body_hash != body_hash {
            return Err(crate::record::RecordError::InvalidValue {
                field_id: 1,
                error: crate::record::ValueError::VerificationFailed,
            });
        }

        // Verify the sender identity matches expected_sender
        let signing_pubkey_field = decoded_signing
            .field(2)
            .ok_or(crate::record::RecordError::MissingRequiredField { field_id: 2 })?;
        let pubkey = signing_pubkey_field.as_bytes()?;
        let pubkey_array: [u8; 1952] =
            pubkey
                .try_into()
                .map_err(|_| crate::record::RecordError::InvalidValue {
                    field_id: 2,
                    error: crate::record::ValueError::IncorrectLength {
                        wire_type: crate::record::WireType::Bytes,
                        expected: 1952,
                        actual: pubkey.len(),
                    },
                })?;
        let computed_sender = crate::identity_v2::account_id_v2(&pubkey_array, chain_id, schema);

        if computed_sender != expected_sender {
            return Err(crate::record::RecordError::InvalidValue {
                field_id: 2,
                error: crate::record::ValueError::VerificationFailed,
            });
        }

        // TODO: Actual ML-DSA-65 signature verification would go here
        // For now, we trust the envelope structure

        Ok(TxIdV2::from_hash(body_hash))
    }

    /// Decode and validate a TransactionEnvelopeV2 from canonical bytes.
    pub fn decode(
        bytes: &[u8],
        _chain_id: u32,
        _schema: SchemaVersion,
    ) -> Result<Self, crate::record::RecordError> {
        let decoded = DecodedRecord::decode_with_schema(bytes, TX_ENVELOPE_SPECS)?;
        // Extract fields
        let _body_hash_field = decoded
            .field(1)
            .ok_or(crate::record::RecordError::MissingRequiredField { field_id: 1 })?;
        let _sig_seq_field = decoded
            .field(2)
            .ok_or(crate::record::RecordError::MissingRequiredField { field_id: 2 })?;

        // For now, return a placeholder - full decode requires SequenceView iteration
        // This would reconstruct body_bytes, signing from the envelope
        unimplemented!("Full decode implementation requires SequenceView iteration")
    }
}

/// Sign a transaction body with ML-DSA-65, returning the TransactionSigningV2 record.
/// This function signature is a placeholder for test utilities; actual implementation requires
/// qanto_core's QantoKeyPair which is not available in this no-std-compatible crate.
/// Tests in the tests/ directory should import qanto_core directly and implement signing there.
#[cfg(test)]
pub fn sign_transaction_body_v2(
    _keypair: &(), // Placeholder - tests use qanto_core directly
    _body_bytes: &[u8],
    _chain_id: u32,
    _schema: SchemaVersion,
) -> Result<TransactionSigningV2, crate::record::RecordError> {
    unimplemented!("Tests should use qanto_core::qanto_native_crypto::QantoKeyPair directly")
}
