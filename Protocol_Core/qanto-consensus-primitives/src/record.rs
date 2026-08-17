use core::fmt;

/// Canonical record value types and their stable wire codes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum WireType {
    U8 = 0x01,
    U16 = 0x02,
    U32 = 0x03,
    U64 = 0x04,
    U128 = 0x05,
    Bytes = 0x06,
    Hash32 = 0x07,
    Sequence = 0x08,
    Bool = 0x09,
}

impl WireType {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    const fn fixed_length(self) -> Option<usize> {
        match self {
            Self::U8 | Self::Bool => Some(1),
            Self::U16 => Some(2),
            Self::U32 => Some(4),
            Self::U64 => Some(8),
            Self::U128 => Some(16),
            Self::Hash32 => Some(32),
            Self::Bytes | Self::Sequence => None,
        }
    }
}

impl TryFrom<u8> for WireType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::U8),
            0x02 => Ok(Self::U16),
            0x03 => Ok(Self::U32),
            0x04 => Ok(Self::U64),
            0x05 => Ok(Self::U128),
            0x06 => Ok(Self::Bytes),
            0x07 => Ok(Self::Hash32),
            0x08 => Ok(Self::Sequence),
            0x09 => Ok(Self::Bool),
            other => Err(other),
        }
    }
}

/// Why an individual field or sequence element is not canonically encoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueError {
    IncorrectLength {
        wire_type: WireType,
        expected: usize,
        actual: usize,
    },
    InvalidBoolean(u8),
    SequenceNestingTooDeep,
    SequenceUnexpectedEof {
        offset: usize,
        needed: usize,
    },
    UnknownSequenceElementType(u8),
    SequenceTrailingBytes {
        offset: usize,
        total: usize,
    },
    NonCanonicalSet {
        index: u32,
    },
    /// Cryptographic verification failed (signature, hash mismatch, etc.)
    VerificationFailed,
}

/// Canonical record construction or decoding error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordError {
    TooManyFields,
    ValueTooLong {
        field_id: u16,
        length: usize,
    },
    TooManySequenceElements {
        field_id: u16,
        count: usize,
    },
    FieldOutOfOrder {
        previous: u16,
        next: u16,
    },
    UnexpectedEof {
        offset: usize,
        needed: usize,
    },
    UnknownWireType {
        field_id: u16,
        wire_type: u8,
    },
    InvalidValue {
        field_id: u16,
        error: ValueError,
    },
    TrailingBytes {
        offset: usize,
        total: usize,
    },
    SchemaOutOfOrder {
        previous: u16,
        next: u16,
    },
    InvalidSequenceSchema {
        field_id: u16,
    },
    UnknownField {
        field_id: u16,
    },
    MissingRequiredField {
        field_id: u16,
    },
    WireTypeMismatch {
        field_id: u16,
        expected: WireType,
        actual: WireType,
    },
    SequenceElementTypeMismatch {
        field_id: u16,
        expected: WireType,
        actual: WireType,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "non-canonical Qanto V2 record: {self:?}")
    }
}

impl std::error::Error for RecordError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedField {
    id: u16,
    wire_type: WireType,
    value: Vec<u8>,
}

/// Safe builder that only emits strictly increasing, unique field IDs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalRecord {
    fields: Vec<OwnedField>,
}

impl CanonicalRecord {
    #[must_use]
    pub const fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn push_u8(&mut self, field_id: u16, value: u8) -> Result<(), RecordError> {
        self.push(field_id, WireType::U8, vec![value])
    }

    pub fn push_u16(&mut self, field_id: u16, value: u16) -> Result<(), RecordError> {
        self.push(field_id, WireType::U16, value.to_be_bytes().to_vec())
    }

    pub fn push_u32(&mut self, field_id: u16, value: u32) -> Result<(), RecordError> {
        self.push(field_id, WireType::U32, value.to_be_bytes().to_vec())
    }

    pub fn push_u64(&mut self, field_id: u16, value: u64) -> Result<(), RecordError> {
        self.push(field_id, WireType::U64, value.to_be_bytes().to_vec())
    }

    pub fn push_u128(&mut self, field_id: u16, value: u128) -> Result<(), RecordError> {
        self.push(field_id, WireType::U128, value.to_be_bytes().to_vec())
    }

    pub fn push_bytes(&mut self, field_id: u16, value: &[u8]) -> Result<(), RecordError> {
        self.push(field_id, WireType::Bytes, value.to_vec())
    }

    pub fn push_hash32(&mut self, field_id: u16, value: &[u8; 32]) -> Result<(), RecordError> {
        self.push(field_id, WireType::Hash32, value.to_vec())
    }

    pub fn push_bool(&mut self, field_id: u16, value: bool) -> Result<(), RecordError> {
        self.push(field_id, WireType::Bool, vec![u8::from(value)])
    }

    /// Encode an order-sensitive sequence exactly as supplied.
    pub fn push_ordered_sequence(
        &mut self,
        field_id: u16,
        element_type: WireType,
        elements: &[&[u8]],
    ) -> Result<(), RecordError> {
        let value = encode_sequence(field_id, element_type, elements, false)?;
        self.push(field_id, WireType::Sequence, value)
    }

    /// Encode a set by sorting complete encoded elements and rejecting duplicates.
    pub fn push_set_sequence(
        &mut self,
        field_id: u16,
        element_type: WireType,
        elements: &[&[u8]],
    ) -> Result<(), RecordError> {
        let value = encode_sequence(field_id, element_type, elements, true)?;
        self.push(field_id, WireType::Sequence, value)
    }

    fn push(
        &mut self,
        field_id: u16,
        wire_type: WireType,
        value: Vec<u8>,
    ) -> Result<(), RecordError> {
        if self.fields.len() == usize::from(u16::MAX) {
            return Err(RecordError::TooManyFields);
        }
        if let Some(previous) = self.fields.last().map(|field| field.id) {
            if field_id <= previous {
                return Err(RecordError::FieldOutOfOrder {
                    previous,
                    next: field_id,
                });
            }
        }
        if u32::try_from(value.len()).is_err() {
            return Err(RecordError::ValueTooLong {
                field_id,
                length: value.len(),
            });
        }
        validate_wire_value(wire_type, &value, 0, false)
            .map_err(|error| RecordError::InvalidValue { field_id, error })?;
        self.fields.push(OwnedField {
            id: field_id,
            wire_type,
            value,
        });
        Ok(())
    }

    /// Finish the canonical record. The empty record is exactly `0x0000`.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        let mut output = Vec::new();
        let field_count = u16::try_from(self.fields.len())
            .expect("CanonicalRecord prevents more than u16::MAX fields");
        output.extend_from_slice(&field_count.to_be_bytes());
        for field in self.fields {
            let value_length = u32::try_from(field.value.len())
                .expect("CanonicalRecord rejects values longer than u32::MAX");
            output.extend_from_slice(&field.id.to_be_bytes());
            output.push(field.wire_type.code());
            output.extend_from_slice(&value_length.to_be_bytes());
            output.extend_from_slice(&field.value);
        }
        output
    }
}

fn encode_sequence(
    field_id: u16,
    element_type: WireType,
    elements: &[&[u8]],
    set_semantics: bool,
) -> Result<Vec<u8>, RecordError> {
    let count =
        u32::try_from(elements.len()).map_err(|_| RecordError::TooManySequenceElements {
            field_id,
            count: elements.len(),
        })?;
    let mut encoded_elements = Vec::with_capacity(elements.len());
    for element in elements {
        validate_wire_value(element_type, element, 1, false)
            .map_err(|error| RecordError::InvalidValue { field_id, error })?;
        let length = u32::try_from(element.len()).map_err(|_| RecordError::ValueTooLong {
            field_id,
            length: element.len(),
        })?;
        let mut encoded = Vec::with_capacity(4usize.saturating_add(element.len()));
        encoded.extend_from_slice(&length.to_be_bytes());
        encoded.extend_from_slice(element);
        encoded_elements.push(encoded);
    }
    if set_semantics {
        encoded_elements.sort_unstable();
        if let Some(index) = encoded_elements
            .windows(2)
            .position(|pair| pair[0] == pair[1])
        {
            return Err(RecordError::InvalidValue {
                field_id,
                error: ValueError::NonCanonicalSet {
                    index: u32::try_from(index + 1).unwrap_or(u32::MAX),
                },
            });
        }
    }
    let mut value = Vec::new();
    value.push(element_type.code());
    value.extend_from_slice(&count.to_be_bytes());
    for element in encoded_elements {
        value.extend_from_slice(&element);
    }
    Ok(value)
}

/// Whether a sequence field is order-sensitive or a canonical set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceSemantics {
    Ordered,
    Set,
}

/// Sequence constraints carried by an object schema, not by the sequence bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceSpec {
    pub element_type: WireType,
    pub semantics: SequenceSemantics,
}

/// Allowed field definition for schema-aware strict record decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldSpec {
    pub id: u16,
    pub wire_type: WireType,
    pub required: bool,
    pub sequence: Option<SequenceSpec>,
}

/// Borrowed, validated canonical field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedField<'a> {
    pub id: u16,
    pub wire_type: WireType,
    pub value: &'a [u8],
}

impl<'a> DecodedField<'a> {
    pub fn as_sequence(self) -> Result<SequenceView<'a>, RecordError> {
        if self.wire_type != WireType::Sequence {
            return Err(RecordError::WireTypeMismatch {
                field_id: self.id,
                expected: WireType::Sequence,
                actual: self.wire_type,
            });
        }
        SequenceView::decode(self.value).map_err(|error| RecordError::InvalidValue {
            field_id: self.id,
            error,
        })
    }

    /// Extract the raw bytes of a Hash32 field as a [u8; 32].
    pub fn as_hash32(self) -> Result<[u8; 32], RecordError> {
        if self.wire_type != WireType::Hash32 {
            return Err(RecordError::WireTypeMismatch {
                field_id: self.id,
                expected: WireType::Hash32,
                actual: self.wire_type,
            });
        }
        self.value
            .try_into()
            .map_err(|_| RecordError::InvalidValue {
                field_id: self.id,
                error: ValueError::IncorrectLength {
                    wire_type: WireType::Hash32,
                    expected: 32,
                    actual: self.value.len(),
                },
            })
    }

    /// Extract the raw bytes of a Bytes field.
    pub fn as_bytes(self) -> Result<&'a [u8], RecordError> {
        if self.wire_type != WireType::Bytes {
            return Err(RecordError::WireTypeMismatch {
                field_id: self.id,
                expected: WireType::Bytes,
                actual: self.wire_type,
            });
        }
        Ok(self.value)
    }
}

/// Borrowed, validated canonical record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRecord<'a> {
    fields: Vec<DecodedField<'a>>,
}

impl<'a> DecodedRecord<'a> {
    /// Decode structural canonical form. Object-specific unknown fields require
    /// [`Self::decode_with_schema`].
    pub fn decode(bytes: &'a [u8]) -> Result<Self, RecordError> {
        let mut offset = 0usize;
        let field_count = usize::from(read_u16(bytes, &mut offset)?);
        let mut fields = Vec::with_capacity(field_count);
        let mut previous = None;
        for _ in 0..field_count {
            let field_id = read_u16(bytes, &mut offset)?;
            if let Some(previous_id) = previous {
                if field_id <= previous_id {
                    return Err(RecordError::FieldOutOfOrder {
                        previous: previous_id,
                        next: field_id,
                    });
                }
            }
            previous = Some(field_id);
            let wire_code = read_u8(bytes, &mut offset)?;
            let wire_type = WireType::try_from(wire_code).map_err(|wire_type| {
                RecordError::UnknownWireType {
                    field_id,
                    wire_type,
                }
            })?;
            let length = usize::try_from(read_u32(bytes, &mut offset)?).map_err(|_| {
                RecordError::UnexpectedEof {
                    offset,
                    needed: usize::MAX,
                }
            })?;
            let value = take(bytes, &mut offset, length)?;
            validate_wire_value(wire_type, value, 0, false)
                .map_err(|error| RecordError::InvalidValue { field_id, error })?;
            fields.push(DecodedField {
                id: field_id,
                wire_type,
                value,
            });
        }
        if offset != bytes.len() {
            return Err(RecordError::TrailingBytes {
                offset,
                total: bytes.len(),
            });
        }
        Ok(Self { fields })
    }

    /// Decode and reject every field not explicitly present in `schema`.
    pub fn decode_with_schema(bytes: &'a [u8], schema: &[FieldSpec]) -> Result<Self, RecordError> {
        validate_schema(schema)?;
        let decoded = Self::decode(bytes)?;
        for field in &decoded.fields {
            let spec = schema
                .binary_search_by_key(&field.id, |candidate| candidate.id)
                .ok()
                .map(|index| schema[index])
                .ok_or(RecordError::UnknownField { field_id: field.id })?;
            if field.wire_type != spec.wire_type {
                return Err(RecordError::WireTypeMismatch {
                    field_id: field.id,
                    expected: spec.wire_type,
                    actual: field.wire_type,
                });
            }
            if let Some(sequence_spec) = spec.sequence {
                let view = SequenceView::decode(field.value).map_err(|error| {
                    RecordError::InvalidValue {
                        field_id: field.id,
                        error,
                    }
                })?;
                if view.element_type != sequence_spec.element_type {
                    return Err(RecordError::SequenceElementTypeMismatch {
                        field_id: field.id,
                        expected: sequence_spec.element_type,
                        actual: view.element_type,
                    });
                }
                if sequence_spec.semantics == SequenceSemantics::Set {
                    validate_wire_value(WireType::Sequence, field.value, 0, true).map_err(
                        |error| RecordError::InvalidValue {
                            field_id: field.id,
                            error,
                        },
                    )?;
                }
            }
        }
        for spec in schema {
            if spec.required && decoded.field(spec.id).is_none() {
                return Err(RecordError::MissingRequiredField { field_id: spec.id });
            }
        }
        Ok(decoded)
    }

    #[must_use]
    pub fn fields(&self) -> &[DecodedField<'a>] {
        &self.fields
    }

    #[must_use]
    pub fn field(&self, field_id: u16) -> Option<DecodedField<'a>> {
        self.fields
            .binary_search_by_key(&field_id, |field| field.id)
            .ok()
            .map(|index| self.fields[index])
    }
}

fn validate_schema(schema: &[FieldSpec]) -> Result<(), RecordError> {
    let mut previous = None;
    for spec in schema {
        if let Some(previous_id) = previous {
            if spec.id <= previous_id {
                return Err(RecordError::SchemaOutOfOrder {
                    previous: previous_id,
                    next: spec.id,
                });
            }
        }
        previous = Some(spec.id);
        if (spec.wire_type == WireType::Sequence) != spec.sequence.is_some() {
            return Err(RecordError::InvalidSequenceSchema { field_id: spec.id });
        }
    }
    Ok(())
}

const MAX_SEQUENCE_NESTING: usize = 16;

fn validate_wire_value(
    wire_type: WireType,
    value: &[u8],
    nesting: usize,
    set_semantics: bool,
) -> Result<(), ValueError> {
    if let Some(expected) = wire_type.fixed_length() {
        if value.len() != expected {
            return Err(ValueError::IncorrectLength {
                wire_type,
                expected,
                actual: value.len(),
            });
        }
    }
    if wire_type == WireType::Bool && value[0] > 1 {
        return Err(ValueError::InvalidBoolean(value[0]));
    }
    if wire_type == WireType::Sequence {
        validate_sequence(value, nesting, set_semantics)?;
    }
    Ok(())
}

fn validate_sequence(value: &[u8], nesting: usize, set_semantics: bool) -> Result<(), ValueError> {
    if nesting >= MAX_SEQUENCE_NESTING {
        return Err(ValueError::SequenceNestingTooDeep);
    }
    let mut offset = 0usize;
    let element_code = sequence_read_u8(value, &mut offset)?;
    let element_type =
        WireType::try_from(element_code).map_err(ValueError::UnknownSequenceElementType)?;
    let count = sequence_read_u32(value, &mut offset)?;
    let mut previous_encoded: Option<&[u8]> = None;
    for index in 0..count {
        let encoded_start = offset;
        let length = usize::try_from(sequence_read_u32(value, &mut offset)?).map_err(|_| {
            ValueError::SequenceUnexpectedEof {
                offset,
                needed: usize::MAX,
            }
        })?;
        let element = sequence_take(value, &mut offset, length)?;
        validate_wire_value(element_type, element, nesting + 1, false)?;
        if set_semantics {
            let encoded = &value[encoded_start..offset];
            if previous_encoded.is_some_and(|previous| previous >= encoded) {
                return Err(ValueError::NonCanonicalSet { index });
            }
            previous_encoded = Some(encoded);
        }
    }
    if offset != value.len() {
        return Err(ValueError::SequenceTrailingBytes {
            offset,
            total: value.len(),
        });
    }
    Ok(())
}

/// Validated view over one canonical sequence value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceView<'a> {
    pub element_type: WireType,
    pub count: u32,
    encoded_elements: &'a [u8],
}

impl<'a> SequenceView<'a> {
    fn decode(value: &'a [u8]) -> Result<Self, ValueError> {
        validate_sequence(value, 0, false)?;
        let element_type =
            WireType::try_from(value[0]).map_err(ValueError::UnknownSequenceElementType)?;
        let count = u32::from_be_bytes([value[1], value[2], value[3], value[4]]);
        Ok(Self {
            element_type,
            count,
            encoded_elements: &value[5..],
        })
    }

    #[must_use]
    pub fn elements(self) -> SequenceElements<'a> {
        SequenceElements {
            remaining: self.count,
            encoded: self.encoded_elements,
            offset: 0,
        }
    }
}

/// Exact-size iterator over already validated sequence element values.
#[derive(Clone, Debug)]
pub struct SequenceElements<'a> {
    remaining: u32,
    encoded: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for SequenceElements<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let end_length = self.offset.checked_add(4)?;
        let length_bytes: [u8; 4] = self.encoded.get(self.offset..end_length)?.try_into().ok()?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes)).ok()?;
        let start = end_length;
        let end = start.checked_add(length)?;
        let element = self.encoded.get(start..end)?;
        self.offset = end;
        self.remaining -= 1;
        Some(element)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SequenceElements<'_> {}

fn take<'a>(bytes: &'a [u8], offset: &mut usize, length: usize) -> Result<&'a [u8], RecordError> {
    let end = offset
        .checked_add(length)
        .ok_or(RecordError::UnexpectedEof {
            offset: *offset,
            needed: length,
        })?;
    let result = bytes.get(*offset..end).ok_or(RecordError::UnexpectedEof {
        offset: *offset,
        needed: length,
    })?;
    *offset = end;
    Ok(result)
}

fn read_u8(bytes: &[u8], offset: &mut usize) -> Result<u8, RecordError> {
    Ok(take(bytes, offset, 1)?[0])
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> Result<u16, RecordError> {
    let bytes: [u8; 2] = take(bytes, offset, 2)?
        .try_into()
        .expect("take returned exactly two bytes");
    Ok(u16::from_be_bytes(bytes))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, RecordError> {
    let bytes: [u8; 4] = take(bytes, offset, 4)?
        .try_into()
        .expect("take returned exactly four bytes");
    Ok(u32::from_be_bytes(bytes))
}

fn sequence_take<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    length: usize,
) -> Result<&'a [u8], ValueError> {
    let end = offset
        .checked_add(length)
        .ok_or(ValueError::SequenceUnexpectedEof {
            offset: *offset,
            needed: length,
        })?;
    let result = bytes
        .get(*offset..end)
        .ok_or(ValueError::SequenceUnexpectedEof {
            offset: *offset,
            needed: length,
        })?;
    *offset = end;
    Ok(result)
}

fn sequence_read_u8(bytes: &[u8], offset: &mut usize) -> Result<u8, ValueError> {
    Ok(sequence_take(bytes, offset, 1)?[0])
}

fn sequence_read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, ValueError> {
    let bytes: [u8; 4] = sequence_take(bytes, offset, 4)?
        .try_into()
        .expect("sequence_take returned exactly four bytes");
    Ok(u32::from_be_bytes(bytes))
}
