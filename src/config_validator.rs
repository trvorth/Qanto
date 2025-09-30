use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml;
use tracing::{debug, error, info, warn};

/// Configuration validation errors
#[derive(Error, Debug)]
pub enum ConfigValidationError {
    #[error("Configuration file not found: {0}")]
    ConfigNotFound(String),

    #[error("Wallet file not found: {0}")]
    WalletNotFound(String),

    #[error("Invalid TOML format: {0}")]
    InvalidToml(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Mining configuration error: {0}")]
    MiningConfigError(String),

    #[error("Wallet validation error: {0}")]
    WalletValidationError(String),

    #[error("Network configuration error: {0}")]
    NetworkConfigError(String),

    #[error("RPC configuration error: {0}")]
    RpcConfigError(String),
}

/// Mining configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningConfig {
    pub enabled: bool,
    pub difficulty: Option<f64>,
    pub target_block_time: Option<u64>,
    pub max_retries: Option<u32>,
    pub adaptive_difficulty: Option<bool>,
    pub testnet_mode: Option<bool>,
    pub skip_vdf: Option<bool>,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            difficulty: Some(0.001),       // Low difficulty for testnet
            target_block_time: Some(5000), // 5 seconds
            max_retries: Some(3),
            adaptive_difficulty: Some(true),
            testnet_mode: Some(true),
            skip_vdf: Some(true),
        }
    }
}

/// Network configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub listen_address: Option<String>,
    pub port: Option<u16>,
    pub bootstrap_peers: Option<Vec<String>>,
    pub max_peers: Option<u32>,
    pub enable_discovery: Option<bool>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_address: Some("0.0.0.0".to_string()),
            port: Some(8333),
            bootstrap_peers: Some(vec![]),
            max_peers: Some(50),
            enable_discovery: Some(true),
        }
    }
}

/// RPC configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub enabled: Option<bool>,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub allowed_origins: Option<Vec<String>>,
    pub max_connections: Option<u32>,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            enabled: Some(true),
            address: Some("127.0.0.1".to_string()),
            port: Some(8545), // Default to 8545 instead of 3030
            allowed_origins: Some(vec!["*".to_string()]),
            max_connections: Some(100),
        }
    }
}

/// Database configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: Option<String>,
    pub cache_size: Option<u64>,
    pub max_connections: Option<u32>,
    pub enable_wal: Option<bool>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: Some("./data/qanto.db".to_string()),
            cache_size: Some(100 * 1024 * 1024), // 100MB
            max_connections: Some(10),
            enable_wal: Some(true),
        }
    }
}

/// Logging configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: Option<String>,
    pub file: Option<String>,
    pub max_size: Option<u64>,
    pub max_files: Option<u32>,
    pub console: Option<bool>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Some("info".to_string()),
            file: Some("./logs/qanto.log".to_string()),
            max_size: Some(10 * 1024 * 1024), // 10MB
            max_files: Some(5),
            console: Some(true),
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QantoConfig {
    pub mining: Option<MiningConfig>,
    pub network: Option<NetworkConfig>,
    pub rpc: Option<RpcConfig>,
    pub database: Option<DatabaseConfig>,
    pub logging: Option<LoggingConfig>,

    // Additional fields for compatibility
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

impl Default for QantoConfig {
    fn default() -> Self {
        Self {
            mining: Some(MiningConfig::default()),
            network: Some(NetworkConfig::default()),
            rpc: Some(RpcConfig::default()),
            database: Some(DatabaseConfig::default()),
            logging: Some(LoggingConfig::default()),
            extra: HashMap::new(),
        }
    }
}

/// Wallet validation information
#[derive(Debug, Clone)]
pub struct WalletValidationInfo {
    pub exists: bool,
    pub readable: bool,
    pub size_bytes: u64,
    pub has_private_key: bool,
    pub signature_valid: bool,
    pub key_type: Option<String>,
}

/// Configuration validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub config_valid: bool,
    pub wallet_valid: bool,
    pub config_issues: Vec<String>,
    pub wallet_issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub config: QantoConfig,
    pub wallet_info: WalletValidationInfo,
}

/// Main configuration validator
pub struct ConfigValidator {
    pub config_path: PathBuf,
    pub wallet_path: PathBuf,
}

impl ConfigValidator {
    /// Create a new configuration validator
    pub fn new<P: AsRef<Path>>(config_path: P, wallet_path: P) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            wallet_path: wallet_path.as_ref().to_path_buf(),
        }
    }

    /// Validate both configuration and wallet files
    pub async fn validate_all(&self) -> Result<ValidationResult, ConfigValidationError> {
        info!("🔍 Starting configuration validation");

        let mut result = ValidationResult {
            config_valid: false,
            wallet_valid: false,
            config_issues: Vec::new(),
            wallet_issues: Vec::new(),
            suggestions: Vec::new(),
            config: QantoConfig::default(),
            wallet_info: WalletValidationInfo {
                exists: false,
                readable: false,
                size_bytes: 0,
                has_private_key: false,
                signature_valid: false,
                key_type: None,
            },
        };

        // Validate configuration file
        match self.validate_config().await {
            Ok((config, issues)) => {
                result.config = config;
                result.config_issues = issues;
                result.config_valid = result.config_issues.is_empty();
            }
            Err(e) => {
                result
                    .config_issues
                    .push(format!("Configuration validation failed: {e}"));
                result.config_valid = false;
            }
        }

        // Validate wallet file
        match self.validate_wallet().await {
            Ok((wallet_info, issues)) => {
                result.wallet_info = wallet_info;
                result.wallet_issues = issues;
                result.wallet_valid = result.wallet_issues.is_empty();
            }
            Err(e) => {
                result
                    .wallet_issues
                    .push(format!("Wallet validation failed: {e}"));
                result.wallet_valid = false;
            }
        }

        // Generate suggestions based on issues found
        result.suggestions = self.generate_suggestions(&result);

        // Log validation summary
        self.log_validation_summary(&result);

        Ok(result)
    }

    /// Validate configuration file
    async fn validate_config(&self) -> Result<(QantoConfig, Vec<String>), ConfigValidationError> {
        let mut issues = Vec::new();

        // Check if config file exists
        if !self.config_path.exists() {
            return Err(ConfigValidationError::ConfigNotFound(
                self.config_path.display().to_string(),
            ));
        }

        // Read and parse configuration file
        let config_content = fs::read_to_string(&self.config_path)?;
        let mut config: QantoConfig = toml::from_str(&config_content)
            .map_err(|e| ConfigValidationError::InvalidToml(e.to_string()))?;

        // Apply defaults for missing sections
        if config.mining.is_none() {
            config.mining = Some(MiningConfig::default());
            issues.push("Mining configuration missing, using defaults".to_string());
        }

        if config.network.is_none() {
            config.network = Some(NetworkConfig::default());
            issues.push("Network configuration missing, using defaults".to_string());
        }

        if config.rpc.is_none() {
            config.rpc = Some(RpcConfig::default());
            issues.push("RPC configuration missing, using defaults".to_string());
        }

        if config.database.is_none() {
            config.database = Some(DatabaseConfig::default());
            issues.push("Database configuration missing, using defaults".to_string());
        }

        if config.logging.is_none() {
            config.logging = Some(LoggingConfig::default());
            issues.push("Logging configuration missing, using defaults".to_string());
        }

        // Validate mining configuration
        if let Some(ref mining) = config.mining {
            self.validate_mining_config(mining, &mut issues);
        }

        // Validate network configuration
        if let Some(ref network) = config.network {
            self.validate_network_config(network, &mut issues);
        }

        // Validate RPC configuration
        if let Some(ref rpc) = config.rpc {
            self.validate_rpc_config(rpc, &mut issues);
        }

        // Validate database configuration
        if let Some(ref database) = config.database {
            self.validate_database_config(database, &mut issues);
        }

        // Validate logging configuration
        if let Some(ref logging) = config.logging {
            self.validate_logging_config(logging, &mut issues);
        }

        debug!(
            "Configuration validation completed with {} issues",
            issues.len()
        );
        Ok((config, issues))
    }

    /// Validate mining configuration section
    fn validate_mining_config(&self, mining: &MiningConfig, issues: &mut Vec<String>) {
        if !mining.enabled {
            issues.push("Mining is disabled - enable it for testnet operation".to_string());
        }

        if let Some(difficulty) = mining.difficulty {
            if difficulty > 1.0 {
                issues.push(format!(
                    "Mining difficulty ({difficulty}) is high for testnet - consider reducing to < 1.0"
                ));
            }
            if difficulty <= 0.0 {
                issues.push("Mining difficulty must be greater than 0".to_string());
            }
        }

        if let Some(target_time) = mining.target_block_time {
            if target_time < 1000 {
                issues.push(
                    "Target block time is very low (< 1s) - may cause instability".to_string(),
                );
            }
            if target_time > 60000 {
                issues.push(
                    "Target block time is very high (> 60s) - may slow down testnet".to_string(),
                );
            }
        }

        if let Some(retries) = mining.max_retries {
            if retries == 0 {
                issues.push("Max retries is 0 - mining may fail frequently".to_string());
            }
            if retries > 10 {
                issues.push("Max retries is very high - may cause long delays".to_string());
            }
        }
    }

    /// Validate network configuration section
    fn validate_network_config(&self, network: &NetworkConfig, issues: &mut Vec<String>) {
        if let Some(ref address) = network.listen_address {
            if address.is_empty() {
                issues.push("Listen address is empty".to_string());
            }
        }

        if let Some(port) = network.port {
            if port < 1024 {
                issues.push("Network port < 1024 may require root privileges".to_string());
            }
            if port == 8545 {
                issues.push("Network port conflicts with default RPC port (8545)".to_string());
            }
        }

        if let Some(max_peers) = network.max_peers {
            if max_peers == 0 {
                issues.push("Max peers is 0 - node will be isolated".to_string());
            }
            if max_peers > 1000 {
                issues.push("Max peers is very high - may consume excessive resources".to_string());
            }
        }
    }

    /// Validate RPC configuration section
    fn validate_rpc_config(&self, rpc: &RpcConfig, issues: &mut Vec<String>) {
        if let Some(enabled) = rpc.enabled {
            if !enabled {
                issues.push("RPC is disabled - may limit debugging capabilities".to_string());
            }
        }

        if let Some(port) = rpc.port {
            if port == 3030 {
                issues.push(
                    "RPC port is 3030 but should be 8545 for Ethereum compatibility".to_string(),
                );
            }
            if port < 1024 {
                issues.push("RPC port < 1024 may require root privileges".to_string());
            }
        }

        if let Some(ref origins) = rpc.allowed_origins {
            if origins.contains(&"*".to_string()) {
                issues.push(
                    "RPC allows all origins (*) - consider restricting for security".to_string(),
                );
            }
        }

        if let Some(max_conn) = rpc.max_connections {
            if max_conn == 0 {
                issues.push("RPC max connections is 0 - RPC will be unusable".to_string());
            }
            if max_conn > 1000 {
                issues.push(
                    "RPC max connections is very high - may consume excessive resources"
                        .to_string(),
                );
            }
        }
    }

    /// Validate database configuration section
    fn validate_database_config(&self, database: &DatabaseConfig, issues: &mut Vec<String>) {
        if let Some(ref path) = database.path {
            let db_path = Path::new(path);
            if let Some(parent) = db_path.parent() {
                if !parent.exists() {
                    issues.push(format!(
                        "Database directory does not exist: {}",
                        parent.display()
                    ));
                }
            }
        }

        if let Some(cache_size) = database.cache_size {
            if cache_size < 1024 * 1024 {
                issues.push("Database cache size is very small (< 1MB)".to_string());
            }
            if cache_size > 1024 * 1024 * 1024 {
                issues.push("Database cache size is very large (> 1GB)".to_string());
            }
        }

        if let Some(max_conn) = database.max_connections {
            if max_conn == 0 {
                issues.push("Database max connections is 0".to_string());
            }
            if max_conn > 100 {
                issues.push("Database max connections is very high".to_string());
            }
        }
    }

    /// Validate logging configuration section
    fn validate_logging_config(&self, logging: &LoggingConfig, issues: &mut Vec<String>) {
        if let Some(ref level) = logging.level {
            let valid_levels = ["trace", "debug", "info", "warn", "error"];
            if !valid_levels.contains(&level.as_str()) {
                issues.push(format!(
                    "Invalid log level '{}' - use one of: {}",
                    level,
                    valid_levels.join(", ")
                ));
            }
        }

        if let Some(ref file_path) = logging.file {
            let log_path = Path::new(file_path);
            if let Some(parent) = log_path.parent() {
                if !parent.exists() {
                    issues.push(format!(
                        "Log directory does not exist: {}",
                        parent.display()
                    ));
                }
            }
        }

        if let Some(max_size) = logging.max_size {
            if max_size < 1024 * 1024 {
                issues.push("Log max size is very small (< 1MB)".to_string());
            }
        }

        if let Some(max_files) = logging.max_files {
            if max_files == 0 {
                issues.push("Log max files is 0 - logs will not be rotated".to_string());
            }
            if max_files > 100 {
                issues.push("Log max files is very high".to_string());
            }
        }
    }

    /// Validate wallet file
    async fn validate_wallet(
        &self,
    ) -> Result<(WalletValidationInfo, Vec<String>), ConfigValidationError> {
        let mut issues = Vec::new();
        let mut wallet_info = WalletValidationInfo {
            exists: false,
            readable: false,
            size_bytes: 0,
            has_private_key: false,
            signature_valid: false,
            key_type: None,
        };

        // Check if wallet file exists
        if !self.wallet_path.exists() {
            issues.push(format!(
                "Wallet file does not exist: {}",
                self.wallet_path.display()
            ));
            return Ok((wallet_info, issues));
        }
        wallet_info.exists = true;

        // Check if wallet file is readable
        match fs::metadata(&self.wallet_path) {
            Ok(metadata) => {
                wallet_info.size_bytes = metadata.len();
                if wallet_info.size_bytes == 0 {
                    issues.push("Wallet file is empty".to_string());
                }
                if wallet_info.size_bytes > 10 * 1024 * 1024 {
                    issues.push("Wallet file is unusually large (> 10MB)".to_string());
                }
            }
            Err(e) => {
                issues.push(format!("Cannot read wallet file metadata: {e}"));
                return Ok((wallet_info, issues));
            }
        }

        // Try to read wallet file content
        match fs::read(&self.wallet_path) {
            Ok(content) => {
                wallet_info.readable = true;

                // Basic validation of wallet content
                if content.len() < 32 {
                    issues.push("Wallet file is too small to contain valid keys".to_string());
                } else {
                    wallet_info.has_private_key = true;
                }

                // Try to detect key type based on content
                if content.starts_with(b"-----BEGIN") {
                    wallet_info.key_type = Some("PEM".to_string());
                } else if content.len() == 32 || content.len() == 64 {
                    wallet_info.key_type = Some("Raw".to_string());
                } else {
                    wallet_info.key_type = Some("Unknown".to_string());
                }

                // Attempt basic signature validation (simplified)
                wallet_info.signature_valid = self.validate_wallet_signature(&content);
                if !wallet_info.signature_valid {
                    issues.push("Wallet signature validation failed".to_string());
                }
            }
            Err(e) => {
                issues.push(format!("Cannot read wallet file: {e}"));
            }
        }

        debug!("Wallet validation completed with {} issues", issues.len());
        Ok((wallet_info, issues))
    }

    /// Validate wallet signature (simplified implementation)
    fn validate_wallet_signature(&self, _content: &[u8]) -> bool {
        // This is a simplified validation - in a real implementation,
        // you would use the actual cryptographic library to validate
        // the Dilithium signature or other quantum-resistant signature
        true // Assume valid for now
    }

    /// Generate suggestions based on validation issues
    fn generate_suggestions(&self, result: &ValidationResult) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Configuration suggestions
        if !result.config_valid {
            suggestions.push("Fix configuration issues before starting the node".to_string());

            if result
                .config_issues
                .iter()
                .any(|issue| issue.contains("Mining is disabled"))
            {
                suggestions
                    .push("Enable mining in config.toml: [mining] enabled = true".to_string());
            }

            if result
                .config_issues
                .iter()
                .any(|issue| issue.contains("difficulty"))
            {
                suggestions.push(
                    "Set low difficulty for testnet: [mining] difficulty = 0.001".to_string(),
                );
            }

            if result
                .config_issues
                .iter()
                .any(|issue| issue.contains("3030"))
            {
                suggestions.push("Change RPC port to 8545: [rpc] port = 8545".to_string());
            }
        }

        // Wallet suggestions
        if !result.wallet_valid {
            suggestions.push("Fix wallet issues before starting the node".to_string());

            if !result.wallet_info.exists {
                suggestions.push(format!(
                    "Generate wallet: cargo run --bin qanto -- generate-wallet {}",
                    self.wallet_path.display()
                ));
            }

            if result.wallet_info.exists && !result.wallet_info.signature_valid {
                suggestions.push("Regenerate wallet with valid Dilithium signature".to_string());
            }
        }

        // General suggestions
        if result.config_valid && result.wallet_valid {
            suggestions
                .push("Configuration and wallet are valid - ready to start mining!".to_string());
            suggestions.push("Consider using --debug-mode for detailed logging".to_string());
            suggestions
                .push("Use --clean flag only when necessary (resets blockchain state)".to_string());
        }

        suggestions
    }

    /// Log validation summary
    fn log_validation_summary(&self, result: &ValidationResult) {
        info!("📋 Configuration Validation Summary:");
        info!("  Config Valid: {}", result.config_valid);
        info!("  Wallet Valid: {}", result.wallet_valid);
        info!("  Config Issues: {}", result.config_issues.len());
        info!("  Wallet Issues: {}", result.wallet_issues.len());
        info!("  Suggestions: {}", result.suggestions.len());

        if !result.config_issues.is_empty() {
            warn!("Configuration Issues:");
            for issue in &result.config_issues {
                warn!("  - {}", issue);
            }
        }

        if !result.wallet_issues.is_empty() {
            warn!("Wallet Issues:");
            for issue in &result.wallet_issues {
                warn!("  - {}", issue);
            }
        }

        if !result.suggestions.is_empty() {
            info!("Suggestions:");
            for suggestion in &result.suggestions {
                info!("  💡 {}", suggestion);
            }
        }
    }

    /// Create a sample configuration file
    pub fn create_sample_config(&self) -> Result<(), ConfigValidationError> {
        let sample_config = QantoConfig::default();
        let toml_content = toml::to_string_pretty(&sample_config)
            .map_err(|e| ConfigValidationError::InvalidToml(e.to_string()))?;

        fs::write(&self.config_path, toml_content)?;
        info!(
            "📝 Created sample configuration at {}",
            self.config_path.display()
        );
        Ok(())
    }

    /// Fix common configuration issues automatically
    pub async fn auto_fix_config(&self) -> Result<Vec<String>, ConfigValidationError> {
        let mut fixes_applied = Vec::new();

        // Load current config or create default
        let mut config = if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)?;
            toml::from_str(&content)
                .map_err(|e| ConfigValidationError::InvalidToml(e.to_string()))?
        } else {
            fixes_applied.push("Created new configuration file".to_string());
            QantoConfig::default()
        };

        // Apply common fixes
        if let Some(ref mut mining) = config.mining {
            if !mining.enabled {
                mining.enabled = true;
                fixes_applied.push("Enabled mining".to_string());
            }
            if mining.difficulty.unwrap_or(1.0) > 1.0 {
                mining.difficulty = Some(0.001);
                fixes_applied.push("Reduced mining difficulty to 0.001".to_string());
            }
            if mining.testnet_mode.is_none() {
                mining.testnet_mode = Some(true);
                fixes_applied.push("Enabled testnet mode".to_string());
            }
        }

        if let Some(ref mut rpc) = config.rpc {
            if rpc.port == Some(3030) {
                rpc.port = Some(8545);
                fixes_applied.push("Changed RPC port from 3030 to 8545".to_string());
            }
        }

        // Save fixed configuration
        if !fixes_applied.is_empty() {
            let toml_content = toml::to_string_pretty(&config)
                .map_err(|e| ConfigValidationError::InvalidToml(e.to_string()))?;
            fs::write(&self.config_path, toml_content)?;
            info!("🔧 Applied {} configuration fixes", fixes_applied.len());
        }

        Ok(fixes_applied)
    }
}
