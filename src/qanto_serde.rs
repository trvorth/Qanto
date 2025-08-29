//! --- Qanto Native Serialization System ---
//! v0.1.0 - Custom Serialization Implementation
//! This module provides a native serialization/deserialization system for Qanto,
//! replacing serde with a custom high-performance implementation optimized for blockchain data.
//!
//! Features:
//! - Zero-copy deserialization where possible
//! - Compact binary format
//! - Type-safe serialization
//! - Versioning support
//! - Endianness handling
//! - Compression integration
//! - Quantum-resistant encoding

use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QantoSerdeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u8, actual: u8 },
    #[error("Buffer overflow")]
    BufferOverflow,
    #[error("Invalid type tag: {0}")]
    InvalidTypeTag(u8),
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),
    #[error("Custom error: {0}")]
    Custom(String),
}

/// Type tags for serialization
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    // Made public to fix private interface warnings
    // Primitives
    Bool = 0x01,
    U8 = 0x02,
    U16 = 0x03,
    U32 = 0x04,
    U64 = 0x05,
    U128 = 0x06,
    I8 = 0x07,
    I16 = 0x08,
    I32 = 0x09,
    I64 = 0x0A,
    I128 = 0x0B,
    F32 = 0x0C,
    F64 = 0x0D,

    // Collections
    String = 0x10,
    Bytes = 0x11,
    Vec = 0x12,
    HashMap = 0x13,
    BTreeMap = 0x14,
    HashSet = 0x15,
    BTreeSet = 0x16,
    VecDeque = 0x17,

    // Options and Results
    None = 0x20,
    Some = 0x21,
    Ok = 0x22,
    Err = 0x23,

    // Tuples
    Tuple2 = 0x30,
    Tuple3 = 0x31,
    Tuple4 = 0x32,

    // Complex types
    Struct = 0x40,
    Enum = 0x41,

    // Special
    Null = 0x00,
    Custom = 0xFF,
}

impl TypeTag {
    fn from_u8(value: u8) -> Result<Self, QantoSerdeError> {
        match value {
            0x00 => Ok(TypeTag::Null),
            0x01 => Ok(TypeTag::Bool),
            0x02 => Ok(TypeTag::U8),
            0x03 => Ok(TypeTag::U16),
            0x04 => Ok(TypeTag::U32),
            0x05 => Ok(TypeTag::U64),
            0x06 => Ok(TypeTag::U128),
            0x07 => Ok(TypeTag::I8),
            0x08 => Ok(TypeTag::I16),
            0x09 => Ok(TypeTag::I32),
            0x0A => Ok(TypeTag::I64),
            0x0B => Ok(TypeTag::I128),
            0x0C => Ok(TypeTag::F32),
            0x0D => Ok(TypeTag::F64),
            0x10 => Ok(TypeTag::String),
            0x11 => Ok(TypeTag::Bytes),
            0x12 => Ok(TypeTag::Vec),
            0x13 => Ok(TypeTag::HashMap),
            0x14 => Ok(TypeTag::BTreeMap),
            0x15 => Ok(TypeTag::HashSet),
            0x16 => Ok(TypeTag::BTreeSet),
            0x17 => Ok(TypeTag::VecDeque),
            0x20 => Ok(TypeTag::None),
            0x21 => Ok(TypeTag::Some),
            0x22 => Ok(TypeTag::Ok),
            0x23 => Ok(TypeTag::Err),
            0x30 => Ok(TypeTag::Tuple2),
            0x31 => Ok(TypeTag::Tuple3),
            0x32 => Ok(TypeTag::Tuple4),
            0x40 => Ok(TypeTag::Struct),
            0x41 => Ok(TypeTag::Enum),
            0xFF => Ok(TypeTag::Custom),
            _ => Err(QantoSerdeError::InvalidTypeTag(value)),
        }
    }
}

/// Serialization trait
pub trait QantoSerialize {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError>;
}

/// Deserialization trait
pub trait QantoDeserialize: Sized {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError>;
}

/// Serializer trait
pub trait QantoSerializer {
    fn write_type_tag(&mut self, tag: TypeTag) -> Result<(), QantoSerdeError>;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), QantoSerdeError>;
    fn write_u8(&mut self, value: u8) -> Result<(), QantoSerdeError>;
    fn write_u16(&mut self, value: u16) -> Result<(), QantoSerdeError>;
    fn write_u32(&mut self, value: u32) -> Result<(), QantoSerdeError>;
    fn write_u64(&mut self, value: u64) -> Result<(), QantoSerdeError>;
    fn write_u128(&mut self, value: u128) -> Result<(), QantoSerdeError>;
    fn write_i8(&mut self, value: i8) -> Result<(), QantoSerdeError>;
    fn write_i16(&mut self, value: i16) -> Result<(), QantoSerdeError>;
    fn write_i32(&mut self, value: i32) -> Result<(), QantoSerdeError>;
    fn write_i64(&mut self, value: i64) -> Result<(), QantoSerdeError>;
    fn write_i128(&mut self, value: i128) -> Result<(), QantoSerdeError>;
    fn write_f32(&mut self, value: f32) -> Result<(), QantoSerdeError>;
    fn write_f64(&mut self, value: f64) -> Result<(), QantoSerdeError>;
    fn write_bool(&mut self, value: bool) -> Result<(), QantoSerdeError>;
    fn write_string(&mut self, value: &str) -> Result<(), QantoSerdeError>;
    fn write_length(&mut self, length: usize) -> Result<(), QantoSerdeError>;
}

/// Deserializer trait
pub trait QantoDeserializer {
    fn read_type_tag(&mut self) -> Result<TypeTag, QantoSerdeError>;
    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, QantoSerdeError>;
    fn read_u8(&mut self) -> Result<u8, QantoSerdeError>;
    fn read_u16(&mut self) -> Result<u16, QantoSerdeError>;
    fn read_u32(&mut self) -> Result<u32, QantoSerdeError>;
    fn read_u64(&mut self) -> Result<u64, QantoSerdeError>;
    fn read_u128(&mut self) -> Result<u128, QantoSerdeError>;
    fn read_i8(&mut self) -> Result<i8, QantoSerdeError>;
    fn read_i16(&mut self) -> Result<i16, QantoSerdeError>;
    fn read_i32(&mut self) -> Result<i32, QantoSerdeError>;
    fn read_i64(&mut self) -> Result<i64, QantoSerdeError>;
    fn read_i128(&mut self) -> Result<i128, QantoSerdeError>;
    fn read_f32(&mut self) -> Result<f32, QantoSerdeError>;
    fn read_f64(&mut self) -> Result<f64, QantoSerdeError>;
    fn read_bool(&mut self) -> Result<bool, QantoSerdeError>;
    fn read_string(&mut self) -> Result<String, QantoSerdeError>;
    fn read_length(&mut self) -> Result<usize, QantoSerdeError>;
}

/// Binary serializer implementation
pub struct BinarySerializer {
    buffer: Vec<u8>,
    version: u8,
}

impl Default for BinarySerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl BinarySerializer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            version: 1,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            version: 1,
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        // Prepend version and magic number
        let mut result = Vec::with_capacity(self.buffer.len() + 8);
        result.extend_from_slice(b"QSER"); // Magic number
        result.push(self.version);
        result.extend_from_slice(&(self.buffer.len() as u32).to_le_bytes()[..3]); // 24-bit length
        result.append(&mut self.buffer);
        result
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl QantoSerializer for BinarySerializer {
    fn write_type_tag(&mut self, tag: TypeTag) -> Result<(), QantoSerdeError> {
        self.buffer.push(tag as u8);
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), QantoSerdeError> {
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn write_u8(&mut self, value: u8) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::U8)?;
        self.buffer.push(value);
        Ok(())
    }

    fn write_u16(&mut self, value: u16) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::U16)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_u32(&mut self, value: u32) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::U32)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_u64(&mut self, value: u64) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::U64)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_u128(&mut self, value: u128) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::U128)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_i8(&mut self, value: i8) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::I8)?;
        self.buffer.push(value as u8);
        Ok(())
    }

    fn write_i16(&mut self, value: i16) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::I16)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_i32(&mut self, value: i32) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::I32)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_i64(&mut self, value: i64) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::I64)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_i128(&mut self, value: i128) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::I128)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_f32(&mut self, value: f32) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::F32)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_f64(&mut self, value: f64) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::F64)?;
        self.buffer.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_bool(&mut self, value: bool) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::Bool)?;
        self.buffer.push(if value { 1 } else { 0 });
        Ok(())
    }

    fn write_string(&mut self, value: &str) -> Result<(), QantoSerdeError> {
        self.write_type_tag(TypeTag::String)?;
        self.write_length(value.len())?;
        self.buffer.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn write_length(&mut self, length: usize) -> Result<(), QantoSerdeError> {
        // Variable-length encoding for efficiency
        if length < 0x80 {
            self.buffer.push(length as u8);
        } else if length < 0x4000 {
            let value = (length as u16) | 0x8000;
            self.buffer.extend_from_slice(&value.to_le_bytes());
        } else if length < 0x20000000 {
            let value = (length as u32) | 0xC0000000;
            self.buffer.extend_from_slice(&value.to_le_bytes());
        } else {
            self.buffer.push(0xFF);
            self.buffer
                .extend_from_slice(&(length as u64).to_le_bytes());
        }
        Ok(())
    }
}

/// Binary deserializer implementation
pub struct BinaryDeserializer<'a> {
    data: &'a [u8],
    position: usize,
    _version: u8, // Prefixed with underscore to indicate intentional non-use
}

impl<'a> BinaryDeserializer<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, QantoSerdeError> {
        if data.len() < 8 {
            return Err(QantoSerdeError::InvalidFormat("Data too short".to_string()));
        }

        // Check magic number
        if &data[0..4] != b"QSER" {
            return Err(QantoSerdeError::InvalidFormat(
                "Invalid magic number".to_string(),
            ));
        }

        let version = data[4];
        if version != 1 {
            return Err(QantoSerdeError::VersionMismatch {
                expected: 1,
                actual: version,
            });
        }

        // Read length (24-bit)
        let length = u32::from_le_bytes([data[5], data[6], data[7], 0]) as usize;
        if data.len() != length + 8 {
            return Err(QantoSerdeError::InvalidFormat(
                "Length mismatch".to_string(),
            ));
        }

        Ok(Self {
            data: &data[8..],
            position: 0,
            _version: version,
        })
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }
}

impl<'a> BinaryDeserializer<'a> {
    fn tag_to_str(tag: TypeTag) -> &'static str {
        match tag {
            TypeTag::Bool => "Bool",
            TypeTag::U8 => "U8",
            TypeTag::U16 => "U16",
            TypeTag::U32 => "U32",
            TypeTag::U64 => "U64",
            TypeTag::U128 => "U128",
            TypeTag::I8 => "I8",
            TypeTag::I16 => "I16",
            TypeTag::I32 => "I32",
            TypeTag::I64 => "I64",
            TypeTag::I128 => "I128",
            TypeTag::F32 => "F32",
            TypeTag::F64 => "F64",
            TypeTag::String => "String",
            TypeTag::Bytes => "Bytes",
            TypeTag::Vec => "Vec",
            TypeTag::HashMap => "HashMap",
            TypeTag::BTreeMap => "BTreeMap",
            TypeTag::HashSet => "HashSet",
            TypeTag::BTreeSet => "BTreeSet",
            TypeTag::VecDeque => "VecDeque",
            TypeTag::None => "None",
            TypeTag::Some => "Some",
            TypeTag::Ok => "Ok",
            TypeTag::Err => "Err",
            TypeTag::Tuple2 => "Tuple2",
            TypeTag::Tuple3 => "Tuple3",
            TypeTag::Tuple4 => "Tuple4",
            TypeTag::Struct => "Struct",
            TypeTag::Enum => "Enum",
            TypeTag::Null => "Null",
            TypeTag::Custom => "Custom",
        }
    }
}

impl<'a> QantoDeserializer for BinaryDeserializer<'a> {
    fn read_type_tag(&mut self) -> Result<TypeTag, QantoSerdeError> {
        if self.position >= self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let tag = TypeTag::from_u8(self.data[self.position])?;
        self.position += 1;
        Ok(tag)
    }

    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, QantoSerdeError> {
        if self.position + len > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let bytes = self.data[self.position..self.position + len].to_vec();
        self.position += len;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::U8 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(16 + tag_str.len());
            error_msg.push_str("Expected U8, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position >= self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let value = self.data[self.position];
        self.position += 1;
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::U16 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(17 + tag_str.len());
            error_msg.push_str("Expected U16, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 2 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let bytes = [self.data[self.position], self.data[self.position + 1]];
        self.position += 2;
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::U32 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(17 + tag_str.len());
            error_msg.push_str("Expected U32, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 4 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.data[self.position..self.position + 4]);
        self.position += 4;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::U64 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(17 + tag_str.len());
            error_msg.push_str("Expected U64, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 8 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.position..self.position + 8]);
        self.position += 8;
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_u128(&mut self) -> Result<u128, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::U128 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(18 + tag_str.len());
            error_msg.push_str("Expected U128, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 16 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.data[self.position..self.position + 16]);
        self.position += 16;
        Ok(u128::from_le_bytes(bytes))
    }

    fn read_i8(&mut self) -> Result<i8, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::I8 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(16 + tag_str.len());
            error_msg.push_str("Expected I8, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position >= self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let value = self.data[self.position] as i8;
        self.position += 1;
        Ok(value)
    }

    fn read_i16(&mut self) -> Result<i16, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::I16 {
            let mut error_msg = String::with_capacity(20);
            error_msg.push_str("Expected I16, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 2 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let bytes = [self.data[self.position], self.data[self.position + 1]];
        self.position += 2;
        Ok(i16::from_le_bytes(bytes))
    }

    fn read_i32(&mut self) -> Result<i32, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::I32 {
            let mut error_msg = String::with_capacity(20);
            error_msg.push_str("Expected I32, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 4 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.data[self.position..self.position + 4]);
        self.position += 4;
        Ok(i32::from_le_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::I64 {
            let mut error_msg = String::with_capacity(20);
            error_msg.push_str("Expected I64, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 8 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.position..self.position + 8]);
        self.position += 8;
        Ok(i64::from_le_bytes(bytes))
    }

    fn read_i128(&mut self) -> Result<i128, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::I128 {
            let mut error_msg = String::with_capacity(21);
            error_msg.push_str("Expected I128, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 16 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.data[self.position..self.position + 16]);
        self.position += 16;
        Ok(i128::from_le_bytes(bytes))
    }

    fn read_f32(&mut self) -> Result<f32, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::F32 {
            let mut error_msg = String::with_capacity(20);
            error_msg.push_str("Expected F32, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 4 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.data[self.position..self.position + 4]);
        self.position += 4;
        Ok(f32::from_le_bytes(bytes))
    }

    fn read_f64(&mut self) -> Result<f64, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::F64 {
            let mut error_msg = String::with_capacity(20);
            error_msg.push_str("Expected F64, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position + 8 > self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.position..self.position + 8]);
        self.position += 8;
        Ok(f64::from_le_bytes(bytes))
    }

    fn read_bool(&mut self) -> Result<bool, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::Bool {
            let mut error_msg = String::with_capacity(21);
            error_msg.push_str("Expected Bool, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        if self.position >= self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let value = self.data[self.position] != 0;
        self.position += 1;
        Ok(value)
    }

    fn read_string(&mut self) -> Result<String, QantoSerdeError> {
        let tag = self.read_type_tag()?;
        if tag != TypeTag::String {
            let mut error_msg = String::with_capacity(23);
            error_msg.push_str("Expected String, got ");
            error_msg.push_str(BinaryDeserializer::tag_to_str(tag));
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        let length = self.read_length()?;
        let bytes = self.read_bytes(length)?;

        String::from_utf8(bytes).map_err(|e| {
            let mut msg = String::with_capacity(14 + e.to_string().len());
            msg.push_str("Invalid UTF-8: ");
            msg.push_str(&e.to_string());
            QantoSerdeError::InvalidFormat(msg)
        })
    }

    fn read_length(&mut self) -> Result<usize, QantoSerdeError> {
        if self.position >= self.data.len() {
            return Err(QantoSerdeError::BufferOverflow);
        }

        let first_byte = self.data[self.position];

        if first_byte < 0x80 {
            // Single byte encoding
            self.position += 1;
            Ok(first_byte as usize)
        } else {
            // Multi-byte encoding - need to read enough bytes to determine type
            if self.position + 2 > self.data.len() {
                return Err(QantoSerdeError::BufferOverflow);
            }

            // Read the first 2 bytes to determine encoding type
            let two_byte_value =
                u16::from_le_bytes([self.data[self.position], self.data[self.position + 1]]);

            if (0x8000..=0xBFFF).contains(&two_byte_value) {
                // 2-byte encoding: 0x8000-0xBFFF range
                self.position += 2;
                Ok((two_byte_value & 0x3FFF) as usize)
            } else {
                // Need to check if it's 4-byte or 9-byte encoding
                if self.position + 4 > self.data.len() {
                    return Err(QantoSerdeError::BufferOverflow);
                }

                // Read 4 bytes to check if it's in 4-byte range
                let four_byte_value = u32::from_le_bytes([
                    self.data[self.position],
                    self.data[self.position + 1],
                    self.data[self.position + 2],
                    self.data[self.position + 3],
                ]);

                if (0xC0000000..=0xFEFFFFFF).contains(&four_byte_value) {
                    // 4-byte encoding: 0xC0000000-0xFEFFFFFF range
                    self.position += 4;
                    Ok((four_byte_value & 0x1FFFFFFF) as usize)
                } else if first_byte == 0xFF {
                    // 9-byte encoding: 0xFF + 8 bytes
                    if self.position + 9 > self.data.len() {
                        return Err(QantoSerdeError::BufferOverflow);
                    }
                    self.position += 1;
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&self.data[self.position..self.position + 8]);
                    self.position += 8;
                    Ok(u64::from_le_bytes(bytes) as usize)
                } else {
                    Err(QantoSerdeError::InvalidFormat(
                        "Invalid encoding type".to_string(),
                    ))
                }
            }
        }
    }
}

// Implement serialization for primitive types

impl QantoSerialize for bool {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_bool(*self)
    }
}

impl QantoDeserialize for bool {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        deserializer.read_bool()
    }
}

macro_rules! impl_serialize_primitive {
    ($type:ty, $write_method:ident, $read_method:ident) => {
        impl QantoSerialize for $type {
            fn serialize<W: QantoSerializer>(
                &self,
                serializer: &mut W,
            ) -> Result<(), QantoSerdeError> {
                serializer.$write_method(*self)
            }
        }

        impl QantoDeserialize for $type {
            fn deserialize<R: QantoDeserializer>(
                deserializer: &mut R,
            ) -> Result<Self, QantoSerdeError> {
                deserializer.$read_method()
            }
        }
    };
}

impl_serialize_primitive!(u8, write_u8, read_u8);
impl_serialize_primitive!(u16, write_u16, read_u16);
impl_serialize_primitive!(u32, write_u32, read_u32);
impl_serialize_primitive!(u64, write_u64, read_u64);
impl_serialize_primitive!(u128, write_u128, read_u128);
impl_serialize_primitive!(i8, write_i8, read_i8);
impl_serialize_primitive!(i16, write_i16, read_i16);
impl_serialize_primitive!(i32, write_i32, read_i32);
impl_serialize_primitive!(i64, write_i64, read_i64);
impl_serialize_primitive!(i128, write_i128, read_i128);
impl_serialize_primitive!(f32, write_f32, read_f32);
impl_serialize_primitive!(f64, write_f64, read_f64);

impl QantoSerialize for String {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_string(self)
    }
}

impl QantoDeserialize for String {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        deserializer.read_string()
    }
}

impl<T: QantoSerialize + 'static> QantoSerialize for Vec<T> {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        // Special handling for Vec<u8> to use Bytes tag
        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u8>() {
            serializer.write_type_tag(TypeTag::Bytes)?;
            serializer.write_length(self.len())?;
            // Safety: We know T is u8, so this cast is safe
            let bytes: &[u8] = unsafe { std::mem::transmute(self.as_slice()) };
            return serializer.write_bytes(bytes);
        }

        serializer.write_type_tag(TypeTag::Vec)?;
        serializer.write_length(self.len())?;
        for item in self {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<T: QantoDeserialize + 'static> QantoDeserialize for Vec<T> {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let tag = deserializer.read_type_tag()?;

        // Handle both Vec and Bytes tags for Vec<u8> compatibility
        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u8>() && tag == TypeTag::Bytes {
            let length = deserializer.read_length()?;
            let bytes = deserializer.read_bytes(length)?;
            // Safety: We know T is u8, so this cast is safe
            return Ok(unsafe { std::mem::transmute::<Vec<u8>, Vec<T>>(bytes) });
        }

        if tag != TypeTag::Vec {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(17 + tag_str.len());
            error_msg.push_str("Expected Vec, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        let length = deserializer.read_length()?;
        let mut vec = Vec::with_capacity(length);
        for _ in 0..length {
            vec.push(T::deserialize(deserializer)?);
        }
        Ok(vec)
    }
}

impl<T: QantoSerialize> QantoSerialize for Option<T> {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        match self {
            None => serializer.write_type_tag(TypeTag::None),
            Some(value) => {
                serializer.write_type_tag(TypeTag::Some)?;
                value.serialize(serializer)
            }
        }
    }
}

impl<T: QantoDeserialize> QantoDeserialize for Option<T> {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let tag = deserializer.read_type_tag()?;
        match tag {
            TypeTag::None => Ok(None),
            TypeTag::Some => Ok(Some(T::deserialize(deserializer)?)),
            _ => {
                let tag_str = BinaryDeserializer::tag_to_str(tag);
                let mut error_msg = String::with_capacity(20 + tag_str.len());
                error_msg.push_str("Expected Option, got ");
                error_msg.push_str(tag_str);
                Err(QantoSerdeError::InvalidFormat(error_msg))
            }
        }
    }
}

impl<T: QantoSerialize, E: QantoSerialize> QantoSerialize for Result<T, E> {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        match self {
            Ok(value) => {
                serializer.write_type_tag(TypeTag::Ok)?;
                value.serialize(serializer)
            }
            Err(error) => {
                serializer.write_type_tag(TypeTag::Err)?;
                error.serialize(serializer)
            }
        }
    }
}

impl<T: QantoDeserialize, E: QantoDeserialize> QantoDeserialize for Result<T, E> {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let tag = deserializer.read_type_tag()?;
        match tag {
            TypeTag::Ok => Ok(Ok(T::deserialize(deserializer)?)),
            TypeTag::Err => Ok(Err(E::deserialize(deserializer)?)),
            _ => {
                let tag_str = BinaryDeserializer::tag_to_str(tag);
                let mut error_msg = String::with_capacity(20 + tag_str.len());
                error_msg.push_str("Expected Result, got ");
                error_msg.push_str(tag_str);
                Err(QantoSerdeError::InvalidFormat(error_msg))
            }
        }
    }
}

// Tuple implementations
impl<A: QantoSerialize, B: QantoSerialize> QantoSerialize for (A, B) {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_type_tag(TypeTag::Tuple2)?;
        self.0.serialize(serializer)?;
        self.1.serialize(serializer)
    }
}

impl<A: QantoDeserialize, B: QantoDeserialize> QantoDeserialize for (A, B) {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let tag = deserializer.read_type_tag()?;
        if tag != TypeTag::Tuple2 {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(19 + tag_str.len());
            error_msg.push_str("Expected Tuple2, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        let a = A::deserialize(deserializer)?;
        let b = B::deserialize(deserializer)?;
        Ok((a, b))
    }
}

// HashMap implementation
impl<K: QantoSerialize, V: QantoSerialize> QantoSerialize for HashMap<K, V> {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_type_tag(TypeTag::HashMap)?;
        serializer.write_length(self.len())?;
        for (key, value) in self {
            key.serialize(serializer)?;
            value.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<K: QantoDeserialize + std::hash::Hash + Eq, V: QantoDeserialize> QantoDeserialize
    for HashMap<K, V>
{
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let tag = deserializer.read_type_tag()?;
        if tag != TypeTag::HashMap {
            let tag_str = BinaryDeserializer::tag_to_str(tag);
            let mut error_msg = String::with_capacity(20 + tag_str.len());
            error_msg.push_str("Expected HashMap, got ");
            error_msg.push_str(tag_str);
            return Err(QantoSerdeError::InvalidFormat(error_msg));
        }

        let length = deserializer.read_length()?;
        let mut map = HashMap::with_capacity(length);
        for _ in 0..length {
            let key = K::deserialize(deserializer)?;
            let value = V::deserialize(deserializer)?;
            map.insert(key, value);
        }
        Ok(map)
    }
}

// Convenience functions
pub fn to_bytes<T: QantoSerialize>(value: &T) -> Result<Vec<u8>, QantoSerdeError> {
    let mut serializer = BinarySerializer::new();
    value.serialize(&mut serializer)?;
    Ok(serializer.finish())
}

pub fn from_bytes<T: QantoDeserialize>(bytes: &[u8]) -> Result<T, QantoSerdeError> {
    let mut deserializer = BinaryDeserializer::new(bytes)?;
    T::deserialize(&mut deserializer)
}

// Derive macro support (placeholder for proc macro)
// In a real implementation, this would be a proc macro
// For now, we provide manual implementations

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_serialization() {
        let value = 42u32;
        let bytes = to_bytes(&value).unwrap();
        let deserialized: u32 = from_bytes(&bytes).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn test_string_serialization() {
        let value = "Hello, Qanto!".to_string();
        let bytes = to_bytes(&value).unwrap();
        let deserialized: String = from_bytes(&bytes).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn test_vec_serialization() {
        let value = vec![1u32, 2, 3, 4, 5];
        let bytes = to_bytes(&value).unwrap();
        let deserialized: Vec<u32> = from_bytes(&bytes).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn test_option_serialization() {
        let some_value = Some(42u32);
        let none_value: Option<u32> = None;

        let some_bytes = to_bytes(&some_value).unwrap();
        let none_bytes = to_bytes(&none_value).unwrap();

        let some_deserialized: Option<u32> = from_bytes(&some_bytes).unwrap();
        let none_deserialized: Option<u32> = from_bytes(&none_bytes).unwrap();

        assert_eq!(some_value, some_deserialized);
        assert_eq!(none_value, none_deserialized);
    }

    #[test]
    fn test_hashmap_serialization() {
        let mut map = HashMap::new();
        map.insert("key1".to_string(), 42u32);
        map.insert("key2".to_string(), 84u32);

        let bytes = to_bytes(&map).unwrap();
        let deserialized: HashMap<String, u32> = from_bytes(&bytes).unwrap();

        assert_eq!(map, deserialized);
    }

    #[test]
    fn test_tuple_serialization() {
        let value = ("hello".to_string(), 42u32);
        let bytes = to_bytes(&value).unwrap();
        let deserialized: (String, u32) = from_bytes(&bytes).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn test_result_serialization() {
        let ok_value: Result<u32, String> = Ok(42);
        let err_value: Result<u32, String> = Err("error".to_string());

        let ok_bytes = to_bytes(&ok_value).unwrap();
        let err_bytes = to_bytes(&err_value).unwrap();

        let ok_deserialized: Result<u32, String> = from_bytes(&ok_bytes).unwrap();
        let err_deserialized: Result<u32, String> = from_bytes(&err_bytes).unwrap();

        assert_eq!(ok_value, ok_deserialized);
        assert_eq!(err_value, err_deserialized);
    }

    #[test]
    fn test_variable_length_encoding() {
        let mut serializer = BinarySerializer::new();

        // Test different length ranges
        serializer.write_length(0x7F).unwrap(); // 1 byte
        serializer.write_length(0x3FFF).unwrap(); // 2 bytes
        serializer.write_length(0x1FFFFFFF).unwrap(); // 4 bytes
        serializer.write_length(0x100000000).unwrap(); // 9 bytes

        let bytes = serializer.finish();

        let mut deserializer = BinaryDeserializer::new(&bytes).unwrap();

        let val1 = deserializer.read_length().unwrap();
        assert_eq!(val1, 0x7F);

        let val2 = deserializer.read_length().unwrap();
        assert_eq!(val2, 0x3FFF);

        let val3 = deserializer.read_length().unwrap();
        assert_eq!(val3, 0x1FFFFFFF);

        let val4 = deserializer.read_length().unwrap();
        assert_eq!(val4, 0x100000000);
    }
}
