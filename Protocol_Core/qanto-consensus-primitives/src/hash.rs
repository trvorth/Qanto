use core::fmt;
use sha3::{Digest, Sha3_256};

/// Fixed eight-byte discriminator for every V2 hash frame.
pub const HASH_V2_MAGIC: [u8; 8] = *b"QANTO-V2";

/// Bytes before the payload in a V2 hash frame.
pub const HASH_FRAME_HEADER_LEN: usize = 8 + 2 + 2 + 4 + 2 + 8;

/// The only hash suite accepted by this version of the primitives crate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum HashSuiteV2 {
    /// SHA3-256, encoded as `0x0001` in the frame.
    Sha3_256 = 0x0001,
}

/// Append-only QIP-002 domain registry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum DomainV2 {
    TxBody = 0x0001,
    TxSigning = 0x0002,
    TxEnvelope = 0x0003,
    UtxoOutpoint = 0x0004,
    BlockCore = 0x0010,
    PowWork = 0x0011,
    BlockSigning = 0x0012,
    BlockId = 0x0013,
    TxMerkleEmpty = 0x0020,
    TxMerkleLeaf = 0x0021,
    TxMerkleNode = 0x0022,
    TxMerkleRoot = 0x0023,
    StateKey = 0x0030,
    StateLeaf = 0x0031,
    StateNode = 0x0032,
    StateRoot = 0x0033,
    AccountId = 0x0040,
    ValidatorId = 0x0041,
    PeerId = 0x0042,
    FinalityVote = 0x0050,
    FinalityCertificate = 0x0051,
    ValidatorSet = 0x0052,
    BridgeClaim = 0x0060,
    BridgeNullifier = 0x0061,
    ZkVerifyingKey = 0x0062,
    P2pEnvelope = 0x0070,
    SnapshotManifest = 0x0071,
    GenesisManifest = 0x0080,
    ConsensusParameters = 0x0081,
    DomainRegistry = 0x0082,
}

impl DomainV2 {
    /// Return the stable on-wire registry value.
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }
}

impl TryFrom<u16> for DomainV2 {
    type Error = UnknownDomainV2;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let domain = match value {
            0x0001 => Self::TxBody,
            0x0002 => Self::TxSigning,
            0x0003 => Self::TxEnvelope,
            0x0004 => Self::UtxoOutpoint,
            0x0010 => Self::BlockCore,
            0x0011 => Self::PowWork,
            0x0012 => Self::BlockSigning,
            0x0013 => Self::BlockId,
            0x0020 => Self::TxMerkleEmpty,
            0x0021 => Self::TxMerkleLeaf,
            0x0022 => Self::TxMerkleNode,
            0x0023 => Self::TxMerkleRoot,
            0x0030 => Self::StateKey,
            0x0031 => Self::StateLeaf,
            0x0032 => Self::StateNode,
            0x0033 => Self::StateRoot,
            0x0040 => Self::AccountId,
            0x0041 => Self::ValidatorId,
            0x0042 => Self::PeerId,
            0x0050 => Self::FinalityVote,
            0x0051 => Self::FinalityCertificate,
            0x0052 => Self::ValidatorSet,
            0x0060 => Self::BridgeClaim,
            0x0061 => Self::BridgeNullifier,
            0x0062 => Self::ZkVerifyingKey,
            0x0070 => Self::P2pEnvelope,
            0x0071 => Self::SnapshotManifest,
            0x0080 => Self::GenesisManifest,
            0x0081 => Self::ConsensusParameters,
            0x0082 => Self::DomainRegistry,
            _ => return Err(UnknownDomainV2(value)),
        };
        Ok(domain)
    }
}

/// Error returned when a numeric domain is not in the append-only registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownDomainV2(pub u16);

impl fmt::Display for UnknownDomainV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown Qanto V2 domain 0x{:04x}", self.0)
    }
}

impl std::error::Error for UnknownDomainV2 {}

/// Untyped 32-byte output of the framed V2 hash construction.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HashV2([u8; 32]);

impl HashV2 {
    /// All-zero hash value, useful for explicit sentinel and test inputs.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct from exact bytes without reinterpretation.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Construct from a byte slice.
    ///
    /// # Panics
    ///
    /// Panics if the slice is not exactly 32 bytes long.
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Self {
        let bytes = <[u8; 32]>::try_from(slice)
            .expect("HashV2::from_slice called with slice that is not 32 bytes long");
        Self::from_bytes(bytes)
    }

    /// Borrow the canonical big-endian byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consume the value and return its exact bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl AsRef<[u8]> for HashV2 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for HashV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lower_hex(formatter, &self.0)
    }
}

impl fmt::Debug for HashV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HashV2(")?;
        write_lower_hex(formatter, &self.0)?;
        formatter.write_str(")")
    }
}

fn write_lower_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

macro_rules! typed_hash {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(HashV2);

        impl $name {
            #[must_use]
            pub const fn from_hash(hash: HashV2) -> Self {
                Self(hash)
            }

            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(HashV2::from_bytes(bytes))
            }

            #[must_use]
            pub const fn as_hash(&self) -> &HashV2 {
                &self.0
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                self.0.as_bytes()
            }

            #[must_use]
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0.into_bytes()
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, formatter)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "("))?;
                write_lower_hex(formatter, self.as_bytes())?;
                formatter.write_str(")")
            }
        }
    };
}

typed_hash!(TxIdV2);
typed_hash!(BlockIdV2);

impl BlockIdV2 {
    /// All-zero BlockIdV2 constant.
    pub const ZERO: Self = Self(HashV2::ZERO);
}

impl AccountIdV2 {
    /// All-zero AccountIdV2 constant (coinbase receiver).
    pub const ZERO: Self = Self(HashV2::ZERO);
}

typed_hash!(StateRootV2);
typed_hash!(WorkHashV2);
typed_hash!(AccountIdV2);
typed_hash!(ValidatorIdV2);

/// Build the byte-exact input to SHA3-256.
///
/// Layout: `QANTO-V2 || suite:u16 || domain:u16 || chain:u32 || schema:u16 ||
/// payload_length:u64 || payload`. Every integer is big-endian.
#[must_use]
pub fn encode_hash_frame_v2(
    domain: DomainV2,
    chain_id: u32,
    schema: u16,
    payload: &[u8],
) -> Vec<u8> {
    let payload_length = payload.len() as u64;
    let mut frame = Vec::with_capacity(HASH_FRAME_HEADER_LEN.saturating_add(payload.len()));
    frame.extend_from_slice(&HASH_V2_MAGIC);
    frame.extend_from_slice(&(HashSuiteV2::Sha3_256 as u16).to_be_bytes());
    frame.extend_from_slice(&domain.code().to_be_bytes());
    frame.extend_from_slice(&chain_id.to_be_bytes());
    frame.extend_from_slice(&schema.to_be_bytes());
    frame.extend_from_slice(&payload_length.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Hash a canonical payload with the byte-exact V2 SHA3-256 frame.
#[must_use]
pub fn hash_v2(domain: DomainV2, chain_id: u32, schema: u16, payload: &[u8]) -> HashV2 {
    let payload_length = payload.len() as u64;
    let mut hasher = Sha3_256::new();
    hasher.update(HASH_V2_MAGIC);
    hasher.update((HashSuiteV2::Sha3_256 as u16).to_be_bytes());
    hasher.update(domain.code().to_be_bytes());
    hasher.update(chain_id.to_be_bytes());
    hasher.update(schema.to_be_bytes());
    hasher.update(payload_length.to_be_bytes());
    hasher.update(payload);
    HashV2(hasher.finalize().into())
}

/// Derive a typed V2 transaction ID from a validated canonical transaction body record.
#[must_use]
pub fn transaction_id_v2(chain_id: u32, schema: u16, transaction_body: &[u8]) -> TxIdV2 {
    TxIdV2::from_hash(hash_v2(
        DomainV2::TxBody,
        chain_id,
        schema,
        transaction_body,
    ))
}

/// Derive a typed V2 block ID from a validated canonical block-ID record.
#[must_use]
pub fn block_id_v2(chain_id: u32, schema: u16, block_id_record: &[u8]) -> BlockIdV2 {
    BlockIdV2::from_hash(hash_v2(
        DomainV2::BlockId,
        chain_id,
        schema,
        block_id_record,
    ))
}

/// Derive a typed V2 state root from a validated canonical state-root record.
#[must_use]
pub fn state_root_v2(chain_id: u32, schema: u16, state_root_record: &[u8]) -> StateRootV2 {
    StateRootV2::from_hash(hash_v2(
        DomainV2::StateRoot,
        chain_id,
        schema,
        state_root_record,
    ))
}
