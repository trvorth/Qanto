use crate::config::Config;
use crate::persistence::{
    balance_key, decode_utxo, encode_balance, encode_utxo, utxo_key, utxos_prefix,
};
use crate::qanto_storage::{QantoStorage, WriteBatch};
use crate::transaction::{DECIMALS_PER_QAN, SMALLEST_UNITS_PER_QAN};
use crate::types::UTXO;
use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use qanto_consensus_primitives::{hash_v2, state_root_v2, CanonicalRecord, DomainV2, WireType};
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2, IDENTITY_VERSION_V2};
use qanto_core::qanto_native_crypto::QANTO_PQ_PUBLIC_KEY_LENGTH;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const DEFAULT_GENESIS_PATH: &str = "config/genesis.json";
const GENESIS_TX_ID_PREFIX: &str = "genesis_allocation_tx";
const GENESIS_PROTOCOL_VERSION: u16 = 2;
const GENESIS_MANIFEST_SCHEMA: u16 = 1;
const GENESIS_STATE_SCHEMA: u16 = 1;
const GENESIS_MANIFEST_KEY: &[u8] = b"consensus:genesis:manifest:v2";
const GENESIS_STATE_ROOT_KEY: &[u8] = b"consensus:genesis:state-root:v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisTokenomics {
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(rename = "maxSupply")]
    pub max_supply: String,
    #[serde(rename = "tokenDecimals")]
    pub token_decimals: u32,
    #[serde(rename = "smallestUnitsPerQanto")]
    pub smallest_units_per_qanto: String,
    #[serde(rename = "maxSupplyBaseUnits")]
    pub max_supply_base_units: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisAllocation {
    pub address: String,
    #[serde(rename = "accountPublicKey")]
    pub account_public_key: Option<String>,
    #[serde(rename = "amountQanto")]
    pub amount_qanto: Option<String>,
    #[serde(rename = "amountBaseUnits")]
    pub amount_base_units: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub properties: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisDocument {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: Option<u16>,
    #[serde(rename = "networkId")]
    pub network_id: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "genesisTimestamp")]
    pub genesis_timestamp: u64,
    #[serde(rename = "genesisValidator")]
    pub genesis_validator: String,
    #[serde(rename = "genesisValidatorPublicKey")]
    pub genesis_validator_public_key: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    pub tokenomics: GenesisTokenomics,
    #[serde(default)]
    pub allocations: Vec<GenesisAllocation>,
    #[serde(default)]
    pub properties: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct NativeGenesisState {
    pub path: PathBuf,
    pub document: GenesisDocument,
    pub utxos: HashMap<String, UTXO>,
    pub balances: HashMap<String, u128>,
    pub total_allocated_base_units: u128,
    pub manifest_hash: String,
    pub allocation_state_root: String,
}

#[derive(Error, Debug)]
pub enum GenesisLoaderError {
    #[error("Genesis file not found at '{0}'")]
    MissingFile(String),
    #[error("Failed to read genesis file '{path}': {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse genesis file '{path}': {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("Genesis file '{path}' is missing required field '{field}'")]
    MissingField { path: String, field: &'static str },
    #[error("Genesis validation failed: {0}")]
    Validation(String),
    #[error("Failed to persist genesis state: {0}")]
    Persistence(String),
    #[error("Persisted UTXO state is missing or inconsistent: {0}")]
    InconsistentStorage(String),
    #[error("Genesis manifest commitment failed: {0}")]
    Commitment(String),
}

fn resolve_genesis_path() -> PathBuf {
    if let Ok(path) = std::env::var("QANTO_GENESIS_FILE") {
        return PathBuf::from(path);
    }
    PathBuf::from(DEFAULT_GENESIS_PATH)
}

fn ensure_hex_address(address: &str, field: &str) -> Result<String, GenesisLoaderError> {
    if address.len() != 64
        || !address
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || hex::decode(address).is_err()
    {
        return Err(GenesisLoaderError::Validation(format!(
            "{field} must be canonical 64-character lowercase hexadecimal: {address}"
        )));
    }
    Ok(address.to_string())
}

fn decode_hex_field(
    value: &str,
    field: &str,
    expected_bytes: usize,
) -> Result<Vec<u8>, GenesisLoaderError> {
    let expected_characters = expected_bytes.checked_mul(2).ok_or_else(|| {
        GenesisLoaderError::Validation(format!("{field} expected length overflowed usize"))
    })?;
    if value.len() != expected_characters
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(GenesisLoaderError::Validation(format!(
            "{field} must be canonical lowercase hexadecimal encoding exactly {expected_bytes} bytes"
        )));
    }
    hex::decode(value).map_err(|_| {
        GenesisLoaderError::Validation(format!(
            "{field} is not a valid {expected_bytes}-byte hexadecimal value"
        ))
    })
}

fn decode_hash32(value: &str, field: &str) -> Result<[u8; 32], GenesisLoaderError> {
    let decoded = decode_hex_field(value, field, 32)?;
    decoded.try_into().map_err(|_| {
        GenesisLoaderError::Validation(format!("{field} must decode to exactly 32 bytes"))
    })
}

fn required_public_key<'a>(
    value: Option<&'a str>,
    field: &'static str,
) -> Result<&'a str, GenesisLoaderError> {
    value.ok_or_else(|| GenesisLoaderError::Validation(format!("missing required {field}")))
}

fn canonical_network_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    let edge = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    !bytes.is_empty()
        && bytes.len() <= 64
        && edge(bytes[0])
        && edge(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn canonical_role(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn record_error(context: &str, error: impl std::fmt::Display) -> GenesisLoaderError {
    GenesisLoaderError::Commitment(format!("{context}: {error}"))
}

fn parse_u128_field(value: &str, field: &str) -> Result<u128, GenesisLoaderError> {
    value.parse::<u128>().map_err(|_| {
        GenesisLoaderError::Validation(format!(
            "Field '{field}' contains an invalid u128 value: {value}"
        ))
    })
}

fn allocation_amount_base_units(
    allocation: &GenesisAllocation,
    smallest_units: u128,
) -> Result<u128, GenesisLoaderError> {
    let address = &allocation.address;
    let amount = match (&allocation.amount_base_units, &allocation.amount_qanto) {
        (Some(base_units), Some(amount_qanto)) => {
            let parsed_base_units =
                parse_u128_field(base_units, "allocations.amountBaseUnits")?;
            let parsed_amount_qanto = parse_u128_field(amount_qanto, "allocations.amountQanto")?;
            let derived_base_units = parsed_amount_qanto.checked_mul(smallest_units).ok_or_else(
                || {
                    GenesisLoaderError::Validation(format!(
                        "allocation for address {address} overflows when converting amountQanto to base units"
                    ))
                },
            )?;
            if parsed_base_units != derived_base_units {
                return Err(GenesisLoaderError::Validation(format!(
                    "allocation for address {address} has mismatched amountQanto ({parsed_amount_qanto}) and amountBaseUnits ({parsed_base_units})"
                )));
            }
            parsed_base_units
        }
        (Some(base_units), None) => {
            parse_u128_field(base_units, "allocations.amountBaseUnits")?
        }
        (None, Some(amount_qanto)) => parse_u128_field(
            amount_qanto,
            "allocations.amountQanto",
        )?
        .checked_mul(smallest_units)
        .ok_or_else(|| {
            GenesisLoaderError::Validation(format!(
                "allocation for address {address} overflows when converting amountQanto to base units"
            ))
        })?,
        (None, None) => {
            return Err(GenesisLoaderError::Validation(format!(
                "allocation for address {address} must declare amountBaseUnits or amountQanto"
            )));
        }
    };

    if amount == 0 {
        return Err(GenesisLoaderError::Validation(format!(
            "allocation for address {address} must be greater than zero"
        )));
    }
    Ok(amount)
}

fn canonical_allocation_record(
    allocation: &GenesisAllocation,
    amount_base_units: u128,
    network_chain_id: u32,
) -> Result<Vec<u8>, GenesisLoaderError> {
    if !allocation.properties.is_empty() {
        return Err(GenesisLoaderError::Validation(format!(
            "allocation for {} contains non-canonical properties",
            allocation.address
        )));
    }
    let role = allocation.role.as_deref().ok_or_else(|| {
        GenesisLoaderError::Validation(format!(
            "allocation for {} is missing canonical role",
            allocation.address
        ))
    })?;
    if !canonical_role(role) {
        return Err(GenesisLoaderError::Validation(format!(
            "allocation role '{role}' must be 1-64 lowercase ASCII letters, digits, or hyphens"
        )));
    }

    let address = ensure_hex_address(&allocation.address, "allocations.address")?;
    let public_key_hex = required_public_key(
        allocation.account_public_key.as_deref(),
        "allocations.accountPublicKey",
    )?;
    let public_key = decode_hex_field(
        public_key_hex,
        "allocations.accountPublicKey",
        QANTO_PQ_PUBLIC_KEY_LENGTH,
    )?;
    let derived_address = account_address_v2(network_chain_id, &public_key)
        .map_err(|error| GenesisLoaderError::Validation(error.to_string()))?;
    if address != derived_address {
        return Err(GenesisLoaderError::Validation(format!(
            "allocation address {address} is not derived from its accountPublicKey for chain {network_chain_id}"
        )));
    }

    let address_bytes = decode_hash32(&address, "allocations.address")?;
    let mut record = CanonicalRecord::new();
    record
        .push_hash32(1, &address_bytes)
        .map_err(|error| record_error("allocation address", error))?;
    record
        .push_bytes(2, &public_key)
        .map_err(|error| record_error("allocation public key", error))?;
    record
        .push_u128(3, amount_base_units)
        .map_err(|error| record_error("allocation amount", error))?;
    record
        .push_bytes(4, role.as_bytes())
        .map_err(|error| record_error("allocation role", error))?;
    Ok(record.finish())
}

/// Compute the byte-exact manifest commitment and allocation state root.
///
/// The returned hashes commit semantic values, not JSON whitespace or object-key
/// order. Every variable-length value is encoded by `CanonicalRecord`, and the
/// outer hashes use distinct QIP-002 domains.
pub fn compute_genesis_commitments(
    document: &GenesisDocument,
) -> Result<(String, String), GenesisLoaderError> {
    let protocol_version = document
        .protocol_version
        .ok_or_else(|| GenesisLoaderError::Validation("protocolVersion is required".to_string()))?;
    if protocol_version != GENESIS_PROTOCOL_VERSION {
        return Err(GenesisLoaderError::Validation(format!(
            "unsupported genesis protocol version {protocol_version}"
        )));
    }
    if !canonical_network_id(&document.network_id) {
        return Err(GenesisLoaderError::Validation(
            "networkId is not canonical".to_string(),
        ));
    }
    if document.genesis_timestamp == 0 {
        return Err(GenesisLoaderError::Validation(
            "genesisTimestamp must be nonzero".to_string(),
        ));
    }
    if !document.properties.is_empty() {
        return Err(GenesisLoaderError::Validation(
            "top-level genesis properties are not part of the canonical schema".to_string(),
        ));
    }
    if document.allocations.is_empty() {
        return Err(GenesisLoaderError::Validation(
            "at least one genesis allocation is required".to_string(),
        ));
    }
    let network_chain_id = u32::try_from(document.chain_id).map_err(|_| {
        GenesisLoaderError::Validation("genesis chainId exceeds the V2 u32 domain".to_string())
    })?;
    let smallest_units = parse_u128_field(
        &document.tokenomics.smallest_units_per_qanto,
        "tokenomics.smallestUnitsPerQanto",
    )?;
    if smallest_units != SMALLEST_UNITS_PER_QAN {
        return Err(GenesisLoaderError::Validation(format!(
            "tokenomics.smallestUnitsPerQanto must equal {SMALLEST_UNITS_PER_QAN}"
        )));
    }
    if document.tokenomics.symbol.as_deref() != Some("QNTO")
        || document.tokenomics.token_decimals != DECIMALS_PER_QAN as u32
    {
        return Err(GenesisLoaderError::Validation(
            "tokenomics symbol/decimals must equal QNTO/9".to_string(),
        ));
    }

    let mut allocation_records = Vec::with_capacity(document.allocations.len());
    let mut total = 0_u128;
    let mut seen_addresses = HashSet::with_capacity(document.allocations.len());
    let mut previous_address: Option<&str> = None;
    for allocation in &document.allocations {
        let address = ensure_hex_address(&allocation.address, "allocations.address")?;
        if !seen_addresses.insert(address.clone()) {
            return Err(GenesisLoaderError::Validation(format!(
                "duplicate genesis allocation address: {address}"
            )));
        }
        if previous_address.is_some_and(|previous| previous >= address.as_str()) {
            return Err(GenesisLoaderError::Validation(
                "genesis allocations must be strictly sorted by canonical address".to_string(),
            ));
        }
        previous_address = Some(allocation.address.as_str());
        let amount = allocation_amount_base_units(allocation, smallest_units)?;
        total = total.checked_add(amount).ok_or_else(|| {
            GenesisLoaderError::Validation(
                "total allocated genesis amount overflowed u128".to_string(),
            )
        })?;
        allocation_records.push(canonical_allocation_record(
            allocation,
            amount,
            network_chain_id,
        )?);
    }
    if total != crate::MAX_TOTAL_SUPPLY {
        return Err(GenesisLoaderError::Validation(format!(
            "canonical allocation total {total} does not equal sealed maximum {}",
            crate::MAX_TOTAL_SUPPLY
        )));
    }

    let allocation_slices: Vec<&[u8]> = allocation_records.iter().map(Vec::as_slice).collect();
    let mut state_record = CanonicalRecord::new();
    state_record
        .push_u16(1, protocol_version)
        .map_err(|error| record_error("state protocol version", error))?;
    state_record
        .push_ordered_sequence(2, WireType::Bytes, &allocation_slices)
        .map_err(|error| record_error("state allocations", error))?;
    state_record
        .push_u128(3, total)
        .map_err(|error| record_error("state total", error))?;
    let allocation_state_root = state_root_v2(
        network_chain_id,
        GENESIS_STATE_SCHEMA,
        &state_record.finish(),
    );

    let validator_id = decode_hash32(&document.genesis_validator, "genesisValidator")?;
    let validator_public_key_hex = required_public_key(
        document.genesis_validator_public_key.as_deref(),
        "genesisValidatorPublicKey",
    )?;
    let validator_public_key = decode_hex_field(
        validator_public_key_hex,
        "genesisValidatorPublicKey",
        QANTO_PQ_PUBLIC_KEY_LENGTH,
    )?;
    let derived_validator = validator_id_v2(network_chain_id, &validator_public_key)
        .map_err(|error| GenesisLoaderError::Validation(error.to_string()))?;
    if derived_validator != document.genesis_validator {
        return Err(GenesisLoaderError::Validation(
            "genesisValidator is not derived from genesisValidatorPublicKey for this chain"
                .to_string(),
        ));
    }
    let contract_address = document
        .contract_address
        .as_deref()
        .ok_or_else(|| GenesisLoaderError::Validation("contractAddress is required".to_string()))?;
    let contract_address = decode_hash32(contract_address, "contractAddress")?;
    if contract_address.iter().all(|byte| *byte == 0) {
        return Err(GenesisLoaderError::Validation(
            "contractAddress cannot be the all-zero placeholder".to_string(),
        ));
    }
    let max_supply = parse_u128_field(&document.tokenomics.max_supply, "tokenomics.maxSupply")?;
    let max_supply_base_units = parse_u128_field(
        document
            .tokenomics
            .max_supply_base_units
            .as_deref()
            .ok_or_else(|| {
                GenesisLoaderError::Validation(
                    "tokenomics.maxSupplyBaseUnits is required".to_string(),
                )
            })?,
        "tokenomics.maxSupplyBaseUnits",
    )?;
    let symbol = document.tokenomics.symbol.as_deref().ok_or_else(|| {
        GenesisLoaderError::Validation("tokenomics.symbol is required".to_string())
    })?;
    if max_supply != 21_000_000_000 || max_supply_base_units != crate::MAX_TOTAL_SUPPLY {
        return Err(GenesisLoaderError::Validation(
            "tokenomics supply fields do not equal the sealed maximum".to_string(),
        ));
    }

    let mut manifest_record = CanonicalRecord::new();
    manifest_record
        .push_u16(1, protocol_version)
        .map_err(|error| record_error("manifest protocol version", error))?;
    manifest_record
        .push_bytes(2, document.network_id.as_bytes())
        .map_err(|error| record_error("manifest network", error))?;
    manifest_record
        .push_u32(3, network_chain_id)
        .map_err(|error| record_error("manifest chain", error))?;
    manifest_record
        .push_u64(4, document.genesis_timestamp)
        .map_err(|error| record_error("manifest timestamp", error))?;
    manifest_record
        .push_hash32(5, &validator_id)
        .map_err(|error| record_error("manifest validator", error))?;
    manifest_record
        .push_bytes(6, &validator_public_key)
        .map_err(|error| record_error("manifest validator public key", error))?;
    manifest_record
        .push_hash32(7, &contract_address)
        .map_err(|error| record_error("manifest contract", error))?;
    manifest_record
        .push_u128(8, max_supply)
        .map_err(|error| record_error("manifest max supply", error))?;
    manifest_record
        .push_u32(9, document.tokenomics.token_decimals)
        .map_err(|error| record_error("manifest decimals", error))?;
    manifest_record
        .push_u128(10, smallest_units)
        .map_err(|error| record_error("manifest smallest units", error))?;
    manifest_record
        .push_u128(11, max_supply_base_units)
        .map_err(|error| record_error("manifest base-unit supply", error))?;
    manifest_record
        .push_bytes(12, symbol.as_bytes())
        .map_err(|error| record_error("manifest symbol", error))?;
    manifest_record
        .push_hash32(13, allocation_state_root.as_bytes())
        .map_err(|error| record_error("manifest state root", error))?;
    manifest_record
        .push_u16(14, IDENTITY_VERSION_V2)
        .map_err(|error| record_error("manifest identity version", error))?;

    let manifest_hash = hash_v2(
        DomainV2::GenesisManifest,
        network_chain_id,
        GENESIS_MANIFEST_SCHEMA,
        &manifest_record.finish(),
    );
    Ok((manifest_hash.to_string(), allocation_state_root.to_string()))
}

fn read_genesis_document() -> Result<(PathBuf, GenesisDocument), GenesisLoaderError> {
    let path = resolve_genesis_path();
    if !path.exists() {
        return Err(GenesisLoaderError::MissingFile(path.display().to_string()));
    }

    let raw = fs::read_to_string(&path).map_err(|source| GenesisLoaderError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let document = serde_json::from_str::<GenesisDocument>(&raw).map_err(|source| {
        GenesisLoaderError::Parse {
            path: path.display().to_string(),
            source,
        }
    })?;

    Ok((path, document))
}

fn validate_genesis_document(
    path: &Path,
    document: &GenesisDocument,
    config: &Config,
) -> Result<(), GenesisLoaderError> {
    if document.protocol_version != Some(GENESIS_PROTOCOL_VERSION) {
        return Err(GenesisLoaderError::Validation(format!(
            "protocolVersion must equal {GENESIS_PROTOCOL_VERSION}; legacy uncommitted genesis documents are rejected"
        )));
    }
    if !canonical_network_id(&document.network_id) {
        return Err(GenesisLoaderError::Validation(
            "networkId must be 1-64 canonical lowercase ASCII letters, digits, or interior hyphens"
                .to_string(),
        ));
    }
    if document.genesis_timestamp == 0 {
        return Err(GenesisLoaderError::Validation(
            "genesisTimestamp must be nonzero".to_string(),
        ));
    }
    if !document.properties.is_empty() {
        return Err(GenesisLoaderError::Validation(
            "top-level genesis properties are not part of the canonical schema".to_string(),
        ));
    }
    if document.allocations.is_empty() {
        return Err(GenesisLoaderError::MissingField {
            path: path.display().to_string(),
            field: "allocations",
        });
    }

    if config.network_id != document.network_id {
        return Err(GenesisLoaderError::Validation(format!(
            "genesis networkId '{}' does not match config.network_id '{}'",
            document.network_id, config.network_id
        )));
    }

    let network_chain_id = u32::try_from(document.chain_id).map_err(|_| {
        GenesisLoaderError::Validation("genesis chainId exceeds the V2 u32 domain".to_string())
    })?;
    if document.chain_id != config.chain_id.unwrap_or(1234) {
        return Err(GenesisLoaderError::Validation(format!(
            "genesis chainId '{}' does not match config.chain_id '{}'",
            document.chain_id,
            config.chain_id.unwrap_or(1234)
        )));
    }

    let genesis_validator = ensure_hex_address(&document.genesis_validator, "genesisValidator")?;
    let config_validator =
        ensure_hex_address(&config.genesis_validator, "config.genesis_validator")?;
    if genesis_validator != config_validator {
        return Err(GenesisLoaderError::Validation(format!(
            "genesisValidator '{}' does not match config.genesis_validator '{}'",
            genesis_validator, config_validator
        )));
    }

    let validator_public_key_hex = required_public_key(
        document.genesis_validator_public_key.as_deref(),
        "genesisValidatorPublicKey",
    )?;
    let validator_public_key = decode_hex_field(
        validator_public_key_hex,
        "genesisValidatorPublicKey",
        QANTO_PQ_PUBLIC_KEY_LENGTH,
    )?;
    let derived_validator = validator_id_v2(network_chain_id, &validator_public_key)
        .map_err(|error| GenesisLoaderError::Validation(error.to_string()))?;
    if derived_validator != genesis_validator {
        return Err(GenesisLoaderError::Validation(
            "genesisValidator is not derived from genesisValidatorPublicKey for this chain"
                .to_string(),
        ));
    }

    let contract_address =
        document
            .contract_address
            .as_deref()
            .ok_or(GenesisLoaderError::MissingField {
                path: path.display().to_string(),
                field: "contractAddress",
            })?;
    let _ = ensure_hex_address(contract_address, "contractAddress")?;

    let max_supply_qanto =
        parse_u128_field(&document.tokenomics.max_supply, "tokenomics.maxSupply")?;
    if max_supply_qanto != 21_000_000_000u128 {
        return Err(GenesisLoaderError::Validation(format!(
            "tokenomics.maxSupply must equal 21000000000, got {}",
            max_supply_qanto
        )));
    }

    if document.tokenomics.symbol.as_deref() != Some("QNTO") {
        return Err(GenesisLoaderError::Validation(
            "tokenomics.symbol must equal canonical value QNTO".to_string(),
        ));
    }

    if document.tokenomics.token_decimals != DECIMALS_PER_QAN as u32 {
        return Err(GenesisLoaderError::Validation(format!(
            "tokenomics.tokenDecimals must equal {}, got {}",
            DECIMALS_PER_QAN, document.tokenomics.token_decimals
        )));
    }

    let smallest_units = parse_u128_field(
        &document.tokenomics.smallest_units_per_qanto,
        "tokenomics.smallestUnitsPerQanto",
    )?;
    if smallest_units != SMALLEST_UNITS_PER_QAN {
        return Err(GenesisLoaderError::Validation(format!(
            "tokenomics.smallestUnitsPerQanto must equal {}, got {}",
            SMALLEST_UNITS_PER_QAN, smallest_units
        )));
    }

    let expected_max_supply_base_units = crate::MAX_TOTAL_SUPPLY;
    let base_units = document.tokenomics.max_supply_base_units.as_deref().ok_or(
        GenesisLoaderError::MissingField {
            path: path.display().to_string(),
            field: "tokenomics.maxSupplyBaseUnits",
        },
    )?;
    let parsed = parse_u128_field(base_units, "tokenomics.maxSupplyBaseUnits")?;
    if parsed != expected_max_supply_base_units {
        return Err(GenesisLoaderError::Validation(format!(
            "tokenomics.maxSupplyBaseUnits must equal {}, got {}",
            expected_max_supply_base_units, parsed
        )));
    }

    Ok(())
}

fn build_state_from_document(
    path: PathBuf,
    document: GenesisDocument,
) -> Result<NativeGenesisState, GenesisLoaderError> {
    let mut utxos = HashMap::with_capacity(document.allocations.len());
    let mut balances = HashMap::with_capacity(document.allocations.len());
    let mut seen_addresses = HashSet::with_capacity(document.allocations.len());
    let mut previous_address: Option<String> = None;
    let max_supply_base_units = document
        .tokenomics
        .max_supply_base_units
        .as_deref()
        .map(|v| parse_u128_field(v, "tokenomics.maxSupplyBaseUnits"))
        .transpose()?
        .unwrap_or(crate::MAX_TOTAL_SUPPLY);

    let smallest_units = parse_u128_field(
        &document.tokenomics.smallest_units_per_qanto,
        "tokenomics.smallestUnitsPerQanto",
    )?;

    let mut total_allocated_base_units = 0u128;

    for (index, allocation) in document.allocations.iter().enumerate() {
        let address = ensure_hex_address(&allocation.address, "allocations.address")?;
        if !seen_addresses.insert(address.clone()) {
            return Err(GenesisLoaderError::Validation(format!(
                "duplicate genesis allocation address: {address}"
            )));
        }
        if previous_address
            .as_ref()
            .is_some_and(|previous| previous >= &address)
        {
            return Err(GenesisLoaderError::Validation(
                "genesis allocations must be strictly sorted by canonical address".to_string(),
            ));
        }
        previous_address = Some(address.clone());

        let amount_base_units = allocation_amount_base_units(allocation, smallest_units)?;

        total_allocated_base_units = total_allocated_base_units
            .checked_add(amount_base_units)
            .ok_or_else(|| {
                GenesisLoaderError::Validation(
                    "total allocated genesis amount overflowed u128".to_string(),
                )
            })?;

        let tx_id = format!("{}_{}", GENESIS_TX_ID_PREFIX, index);
        // The transaction and mempool engines resolve outpoints as
        // `<tx_id>_<output_index>`. Genesis must use the identical key format
        // or every prefunded output is permanently unspendable.
        let utxo_id = format!("{tx_id}_0");
        utxos.insert(
            utxo_id.clone(),
            UTXO {
                address: address.clone(),
                amount: amount_base_units,
                tx_id,
                output_index: 0,
                explorer_link: format!("/explorer/utxo/{}", utxo_id),
            },
        );
        balances.insert(address, amount_base_units);
    }

    if total_allocated_base_units != max_supply_base_units {
        return Err(GenesisLoaderError::Validation(format!(
            "fee-only V1 genesis must allocate exactly tokenomics.maxSupplyBaseUnits: {} != {}",
            total_allocated_base_units, max_supply_base_units
        )));
    }

    let (manifest_hash, allocation_state_root) = compute_genesis_commitments(&document)?;

    Ok(NativeGenesisState {
        path,
        document,
        utxos,
        balances,
        total_allocated_base_units,
        manifest_hash,
        allocation_state_root,
    })
}

pub fn load_validated_genesis(config: &Config) -> Result<NativeGenesisState, GenesisLoaderError> {
    let (path, document) = read_genesis_document()?;
    validate_genesis_document(&path, &document, config)?;
    let state = build_state_from_document(path, document)?;
    let expected_manifest_hash = config.genesis_manifest_hash.as_deref().ok_or_else(|| {
        GenesisLoaderError::Validation(
            "config.genesis_manifest_hash is required; refusing an unpinned genesis manifest"
                .to_string(),
        )
    })?;
    let _ = decode_hash32(expected_manifest_hash, "config.genesis_manifest_hash")?;
    if expected_manifest_hash.bytes().all(|byte| byte == b'0') {
        return Err(GenesisLoaderError::Validation(
            "config.genesis_manifest_hash cannot be the all-zero placeholder".to_string(),
        ));
    }
    if expected_manifest_hash != state.manifest_hash {
        return Err(GenesisLoaderError::Validation(format!(
            "genesis manifest hash mismatch: configured {expected_manifest_hash}, computed {}",
            state.manifest_hash
        )));
    }
    Ok(state)
}

pub fn persist_genesis_state(
    db: &QantoStorage,
    state: &NativeGenesisState,
) -> Result<(), GenesisLoaderError> {
    let mut batch = WriteBatch::with_capacity(state.utxos.len() + state.balances.len() + 2);
    let mut utxos: Vec<_> = state.utxos.iter().collect();
    utxos.sort_unstable_by(|left, right| left.0.cmp(right.0));
    for (utxo_id, utxo) in utxos {
        let encoded =
            encode_utxo(utxo).map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
        batch.put(utxo_key(utxo_id), encoded);
    }
    let mut balances: Vec<_> = state.balances.iter().collect();
    balances.sort_unstable_by(|left, right| left.0.cmp(right.0));
    for (address, balance) in balances {
        batch.put(balance_key(address), encode_balance(*balance));
    }
    batch.put(
        GENESIS_MANIFEST_KEY.to_vec(),
        state.manifest_hash.as_bytes().to_vec(),
    );
    batch.put(
        GENESIS_STATE_ROOT_KEY.to_vec(),
        state.allocation_state_root.as_bytes().to_vec(),
    );

    db.write_batch(batch)
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
    db.flush()
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
    db.sync()
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
    Ok(())
}

pub fn verify_persisted_genesis_commitment(
    db: &QantoStorage,
    state: &NativeGenesisState,
) -> Result<(), GenesisLoaderError> {
    for (key, expected, label) in [
        (
            GENESIS_MANIFEST_KEY,
            state.manifest_hash.as_str(),
            "manifest hash",
        ),
        (
            GENESIS_STATE_ROOT_KEY,
            state.allocation_state_root.as_str(),
            "allocation state root",
        ),
    ] {
        let persisted = db
            .get(key)
            .map_err(|error| GenesisLoaderError::Persistence(error.to_string()))?
            .ok_or_else(|| {
                GenesisLoaderError::InconsistentStorage(format!(
                    "persisted genesis is missing canonical {label}; use a new data directory and replay the pinned manifest"
                ))
            })?;
        if persisted.as_slice() != expected.as_bytes() {
            return Err(GenesisLoaderError::InconsistentStorage(format!(
                "persisted genesis {label} does not match the pinned manifest"
            )));
        }
    }
    Ok(())
}

pub fn load_persisted_utxos(
    db: &QantoStorage,
) -> Result<HashMap<String, UTXO>, GenesisLoaderError> {
    let prefix = utxos_prefix();
    let keys = db
        .keys_with_prefix(&prefix)
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;

    if keys.is_empty() {
        return Err(GenesisLoaderError::InconsistentStorage(
            "genesis marker exists but no persisted UTXO entries were found".to_string(),
        ));
    }

    let mut utxos = HashMap::with_capacity(keys.len());
    for key in keys {
        let utxo_id = String::from_utf8(key[prefix.len()..].to_vec()).map_err(|_| {
            GenesisLoaderError::InconsistentStorage(
                "persisted UTXO key suffix is not canonical UTF-8".to_string(),
            )
        })?;
        let raw = db
            .get(&key)
            .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?
            .ok_or_else(|| {
                GenesisLoaderError::InconsistentStorage(format!(
                    "UTXO key '{}' exists but value is missing",
                    utxo_id
                ))
            })?;
        let utxo = decode_utxo(&raw).map_err(GenesisLoaderError::InconsistentStorage)?;
        if utxos.insert(utxo_id.clone(), utxo).is_some() {
            return Err(GenesisLoaderError::InconsistentStorage(format!(
                "duplicate persisted UTXO identifier {utxo_id}"
            )));
        }
    }

    Ok(utxos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qanto_storage::StorageConfig;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // Mutex to serialize tests that use QANTO_GENESIS_FILE environment variable
    static GENESIS_TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn sample_document() -> GenesisDocument {
        let validator_public_key = vec![7_u8; QANTO_PQ_PUBLIC_KEY_LENGTH];
        let account_public_key = vec![9_u8; QANTO_PQ_PUBLIC_KEY_LENGTH];
        GenesisDocument {
            protocol_version: Some(GENESIS_PROTOCOL_VERSION),
            network_id: "qanto-testnet".to_string(),
            chain_id: 1234,
            genesis_timestamp: 1_717_250_400,
            genesis_validator: validator_id_v2(1234, &validator_public_key).unwrap(),
            genesis_validator_public_key: Some(hex::encode(validator_public_key)),
            contract_address: Some("1".repeat(64)),
            tokenomics: GenesisTokenomics {
                symbol: Some("QNTO".to_string()),
                max_supply: "21000000000".to_string(),
                token_decimals: 9,
                smallest_units_per_qanto: "1000000000".to_string(),
                max_supply_base_units: Some("21000000000000000000".to_string()),
            },
            allocations: vec![GenesisAllocation {
                address: account_address_v2(1234, &account_public_key).unwrap(),
                account_public_key: Some(hex::encode(account_public_key)),
                amount_qanto: Some("21000000000".to_string()),
                amount_base_units: Some("21000000000000000000".to_string()),
                role: Some("community-treasury".to_string()),
                properties: HashMap::default(),
            }],
            properties: HashMap::default(),
        }
    }

    fn sample_config(document: &GenesisDocument) -> Config {
        Config {
            network_id: document.network_id.clone(),
            chain_id: Some(document.chain_id),
            genesis_validator: document.genesis_validator.clone(),
            genesis_manifest_hash: Some(compute_genesis_commitments(document).unwrap().0),
            ..Default::default()
        }
    }

    fn write_document(path: &Path, document: &GenesisDocument) {
        fs::write(path, serde_json::to_vec_pretty(document).unwrap()).unwrap();
    }

    #[test]
    fn loads_valid_genesis_file_into_utxo_state() {
        let _lock = GENESIS_TEST_MUTEX.lock().unwrap();
        let temp_dir = tempdir().expect("tempdir");
        let genesis_path = temp_dir.path().join("genesis.json");
        let document = sample_document();
        write_document(&genesis_path, &document);

        std::env::set_var("QANTO_GENESIS_FILE", &genesis_path);
        let state = load_validated_genesis(&sample_config(&document)).expect("load genesis");
        std::env::remove_var("QANTO_GENESIS_FILE");

        assert_eq!(state.utxos.len(), 1);
        let genesis_outpoint = state
            .utxos
            .get("genesis_allocation_tx_0_0")
            .expect("genesis outpoint uses canonical transaction key format");
        assert_eq!(genesis_outpoint.tx_id, "genesis_allocation_tx_0");
        assert_eq!(genesis_outpoint.output_index, 0);
        assert_eq!(state.total_allocated_base_units, crate::MAX_TOTAL_SUPPLY);
        assert_eq!(
            state.balances.get(&document.allocations[0].address),
            Some(&crate::MAX_TOTAL_SUPPLY)
        );
        assert_eq!(state.manifest_hash.len(), 64);
        assert_eq!(state.allocation_state_root.len(), 64);
    }

    #[test]
    fn rejects_genesis_with_invalid_tokenomics() {
        let _lock = GENESIS_TEST_MUTEX.lock().unwrap();
        let temp_dir = tempdir().expect("tempdir");
        let genesis_path = temp_dir.path().join("genesis.json");
        let mut document = sample_document();
        let config = sample_config(&document);
        document.tokenomics.max_supply = "42".to_string();
        write_document(&genesis_path, &document);

        std::env::set_var("QANTO_GENESIS_FILE", &genesis_path);
        let err = load_validated_genesis(&config).expect_err("invalid tokenomics");
        std::env::remove_var("QANTO_GENESIS_FILE");

        assert!(err.to_string().contains("21000000000"));
    }

    #[test]
    fn commitments_are_stable_and_domain_separated() {
        let document = sample_document();
        let first = compute_genesis_commitments(&document).unwrap();
        let second = compute_genesis_commitments(&document).unwrap();

        assert_eq!(first, second);
        assert_ne!(first.0, first.1);
        assert_eq!(
            first.0,
            "f41fd016cacdbf388c70aa37d00daaa9a2c5194fa449cba2a21b59ef2bc41331"
        );
        assert_eq!(
            first.1,
            "3c576a1ba02764c0e14a20b1f8380ddb0b82ec816997ba8ea625d9238afb52ad"
        );
    }

    #[test]
    fn key_or_manifest_tampering_is_rejected() {
        let mut document = sample_document();
        document.allocations[0].account_public_key =
            Some(hex::encode(vec![10_u8; QANTO_PQ_PUBLIC_KEY_LENGTH]));
        assert!(compute_genesis_commitments(&document)
            .unwrap_err()
            .to_string()
            .contains("not derived"));

        let document = sample_document();
        let mut config = sample_config(&document);
        config.genesis_manifest_hash = Some("f".repeat(64));
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("genesis.json");
        write_document(&path, &document);
        let _lock = GENESIS_TEST_MUTEX.lock().unwrap();
        std::env::set_var("QANTO_GENESIS_FILE", &path);
        let error = load_validated_genesis(&config).unwrap_err();
        std::env::remove_var("QANTO_GENESIS_FILE");
        assert!(error.to_string().contains("hash mismatch"));
    }

    #[test]
    fn persisted_commitments_are_verified_before_reuse() {
        let document = sample_document();
        let state = build_state_from_document(PathBuf::from("fixture.json"), document).unwrap();
        let temp_dir = tempdir().unwrap();
        let storage_config = qanto_core::qanto_storage::StorageConfig {
            data_dir: temp_dir.path().join("db"),
            encryption_enabled: false,
            enable_async_io: false,
            ..Default::default()
        };
        let storage = QantoStorage::new(storage_config).unwrap();

        persist_genesis_state(&storage, &state).unwrap();
        verify_persisted_genesis_commitment(&storage, &state).unwrap();

        let mut tampered = state.clone();
        tampered.manifest_hash = "f".repeat(64);
        assert!(verify_persisted_genesis_commitment(&storage, &tampered).is_err());
    }
}
