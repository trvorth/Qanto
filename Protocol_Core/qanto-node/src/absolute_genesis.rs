use crate::config::Config;
use crate::persistence::{
    balance_key, decode_utxo, encode_balance, encode_utxo, utxo_key, utxos_prefix,
};
use crate::qanto_storage::{QantoStorage, WriteBatch};
use crate::transaction::{DECIMALS_PER_QAN, SMALLEST_UNITS_PER_QAN};
use crate::types::UTXO;
use ahash::AHashMap as HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const DEFAULT_GENESIS_PATH: &str = "config/genesis.json";
const GENESIS_TX_ID_PREFIX: &str = "genesis_allocation_tx";

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct GenesisAllocation {
    pub address: String,
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
pub struct GenesisDocument {
    #[serde(rename = "networkId")]
    pub network_id: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "genesisTimestamp")]
    pub genesis_timestamp: u64,
    #[serde(rename = "genesisValidator")]
    pub genesis_validator: String,
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
}

fn resolve_genesis_path() -> PathBuf {
    if let Ok(path) = std::env::var("QANTO_GENESIS_FILE") {
        return PathBuf::from(path);
    }
    PathBuf::from(DEFAULT_GENESIS_PATH)
}

fn ensure_hex_address(address: &str, field: &str) -> Result<String, GenesisLoaderError> {
    let normalized = address.trim().to_lowercase();
    if normalized.len() != 64 || hex::decode(&normalized).is_err() {
        return Err(GenesisLoaderError::Validation(format!(
            "{field} must be a 64-character hex address: {address}"
        )));
    }
    Ok(normalized)
}

fn parse_u128_field(value: &str, field: &str) -> Result<u128, GenesisLoaderError> {
    value.parse::<u128>().map_err(|_| {
        GenesisLoaderError::Validation(format!(
            "Field '{field}' contains an invalid u128 value: {value}"
        ))
    })
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

    if let Some(contract_address) = &document.contract_address {
        let _ = ensure_hex_address(contract_address, "contractAddress")?;
    }

    let max_supply_qanto =
        parse_u128_field(&document.tokenomics.max_supply, "tokenomics.maxSupply")?;
    if max_supply_qanto != 21_000_000_000u128 {
        return Err(GenesisLoaderError::Validation(format!(
            "tokenomics.maxSupply must equal 21000000000, got {}",
            max_supply_qanto
        )));
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
    if let Some(base_units) = &document.tokenomics.max_supply_base_units {
        let parsed = parse_u128_field(base_units, "tokenomics.maxSupplyBaseUnits")?;
        if parsed != expected_max_supply_base_units {
            return Err(GenesisLoaderError::Validation(format!(
                "tokenomics.maxSupplyBaseUnits must equal {}, got {}",
                expected_max_supply_base_units, parsed
            )));
        }
    }

    Ok(())
}

fn build_state_from_document(
    path: PathBuf,
    document: GenesisDocument,
) -> Result<NativeGenesisState, GenesisLoaderError> {
    let mut utxos = HashMap::with_capacity(document.allocations.len());
    let mut balances = HashMap::with_capacity(document.allocations.len());
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

        let amount_base_units = match (&allocation.amount_base_units, &allocation.amount_qanto) {
            (Some(base_units), Some(amount_qanto)) => {
                let parsed_base_units =
                    parse_u128_field(base_units, "allocations.amountBaseUnits")?;
                let parsed_amount_qanto =
                    parse_u128_field(amount_qanto, "allocations.amountQanto")?;
                let derived_base_units = parsed_amount_qanto
                    .checked_mul(smallest_units)
                    .ok_or_else(|| {
                        GenesisLoaderError::Validation(format!(
                            "allocation for address {} overflows when converting amountQanto to base units",
                            address
                        ))
                    })?;
                if parsed_base_units != derived_base_units {
                    return Err(GenesisLoaderError::Validation(format!(
                        "allocation for address {} has mismatched amountQanto ({}) and amountBaseUnits ({})",
                        address, parsed_amount_qanto, parsed_base_units
                    )));
                }
                parsed_base_units
            }
            (Some(base_units), None) => {
                parse_u128_field(base_units, "allocations.amountBaseUnits")?
            }
            (None, Some(amount_qanto)) => {
                parse_u128_field(amount_qanto, "allocations.amountQanto")?
                    .checked_mul(smallest_units)
                    .ok_or_else(|| {
                        GenesisLoaderError::Validation(format!(
                    "allocation for address {} overflows when converting amountQanto to base units",
                    address
                ))
                    })?
            }
            (None, None) => {
                return Err(GenesisLoaderError::MissingField {
                    path: path.display().to_string(),
                    field: "allocations.amountBaseUnits|allocations.amountQanto",
                });
            }
        };

        if amount_base_units == 0 {
            return Err(GenesisLoaderError::Validation(format!(
                "allocation for address {} must be greater than zero",
                address
            )));
        }

        total_allocated_base_units = total_allocated_base_units
            .checked_add(amount_base_units)
            .ok_or_else(|| {
                GenesisLoaderError::Validation(
                    "total allocated genesis amount overflowed u128".to_string(),
                )
            })?;

        let utxo_id = format!("genesis_alloc_{}_{}", index, address);
        let tx_id = format!("{}_{}", GENESIS_TX_ID_PREFIX, index);
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
        balances
            .entry(address)
            .and_modify(|existing: &mut u128| {
                *existing = existing.saturating_add(amount_base_units)
            })
            .or_insert(amount_base_units);
    }

    if total_allocated_base_units > max_supply_base_units {
        return Err(GenesisLoaderError::Validation(format!(
            "total allocated genesis amount {} exceeds tokenomics.maxSupplyBaseUnits {}",
            total_allocated_base_units, max_supply_base_units
        )));
    }

    Ok(NativeGenesisState {
        path,
        document,
        utxos,
        balances,
        total_allocated_base_units,
    })
}

pub fn load_validated_genesis(config: &Config) -> Result<NativeGenesisState, GenesisLoaderError> {
    let (path, document) = read_genesis_document()?;
    validate_genesis_document(&path, &document, config)?;
    build_state_from_document(path, document)
}

pub fn persist_genesis_state(
    db: &QantoStorage,
    state: &NativeGenesisState,
) -> Result<(), GenesisLoaderError> {
    let mut batch = WriteBatch::with_capacity(state.utxos.len() + state.balances.len());
    for (utxo_id, utxo) in &state.utxos {
        let encoded =
            encode_utxo(utxo).map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
        batch.put(utxo_key(utxo_id), encoded);
    }
    for (address, balance) in &state.balances {
        batch.put(balance_key(address), encode_balance(*balance));
    }

    db.write_batch(batch)
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
    db.flush()
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
    db.sync()
        .map_err(|e| GenesisLoaderError::Persistence(e.to_string()))?;
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
        let utxo_id = String::from_utf8_lossy(&key[prefix.len()..]).to_string();
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
        utxos.insert(utxo_id, utxo);
    }

    Ok(utxos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_config() -> Config {
        let mut config = Config::default();
        config.network_id = "qanto-testnet".to_string();
        config.chain_id = Some(1234);
        config.genesis_validator =
            "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3".to_string();
        config
    }

    #[test]
    fn loads_valid_genesis_file_into_utxo_state() {
        let temp_dir = tempdir().expect("tempdir");
        let genesis_path = temp_dir.path().join("genesis.json");
        fs::write(
            &genesis_path,
            r#"{
                "networkId": "qanto-testnet",
                "chainId": 1234,
                "genesisTimestamp": 1717250400,
                "genesisValidator": "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3",
                "tokenomics": {
                    "maxSupply": "21000000000",
                    "tokenDecimals": 9,
                    "smallestUnitsPerQanto": "1000000000",
                    "maxSupplyBaseUnits": "21000000000000000000"
                },
                "allocations": [
                    {
                        "address": "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3",
                        "amountBaseUnits": "21000000000000000000"
                    }
                ]
            }"#,
        )
        .expect("write genesis");

        std::env::set_var("QANTO_GENESIS_FILE", &genesis_path);
        let state = load_validated_genesis(&sample_config()).expect("load genesis");
        std::env::remove_var("QANTO_GENESIS_FILE");

        assert_eq!(state.utxos.len(), 1);
        assert_eq!(state.total_allocated_base_units, crate::MAX_TOTAL_SUPPLY);
        assert_eq!(
            state
                .balances
                .get("ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3"),
            Some(&crate::MAX_TOTAL_SUPPLY)
        );
    }

    #[test]
    fn rejects_genesis_with_invalid_tokenomics() {
        let temp_dir = tempdir().expect("tempdir");
        let genesis_path = temp_dir.path().join("genesis.json");
        fs::write(
            &genesis_path,
            r#"{
                "networkId": "qanto-testnet",
                "chainId": 1234,
                "genesisTimestamp": 1717250400,
                "genesisValidator": "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3",
                "tokenomics": {
                    "maxSupply": "42",
                    "tokenDecimals": 9,
                    "smallestUnitsPerQanto": "1000000000"
                },
                "allocations": [
                    {
                        "address": "ae527b01ffcb3baae0106fbb954acd184e02cb379a3319ff66d3cdfb4a63f9d3",
                        "amountBaseUnits": "1"
                    }
                ]
            }"#,
        )
        .expect("write genesis");

        std::env::set_var("QANTO_GENESIS_FILE", &genesis_path);
        let err = load_validated_genesis(&sample_config()).expect_err("invalid tokenomics");
        std::env::remove_var("QANTO_GENESIS_FILE");

        assert!(err.to_string().contains("21000000000"));
    }
}
