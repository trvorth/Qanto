use crate::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info};

/// Gas-based fee model errors
#[derive(Error, Debug)]
pub enum GasFeeError {
    #[error("Gas limit exceeded: used {used}, limit {limit}")]
    GasLimitExceeded { used: u64, limit: u64 },
    #[error("Insufficient gas balance: required {required}, available {available}")]
    InsufficientGasBalance { required: u64, available: u64 },
    #[error("Invalid gas price: {price}")]
    InvalidGasPrice { price: u64 },
    #[error("Fee calculation error: {reason}")]
    FeeCalculationError { reason: String },
}

/// Gas fee model constants (in smallest units - microQAN)
pub const BASE_FEE_MICRO_QAN: u64 = 100; // 0.0001 QAN = 100 microQAN
pub const TARGET_TPS: u64 = 10_000_000;
pub const CONGESTION_ADJUSTMENT_FACTOR: f64 = 0.5;

/// Storage rates per byte (in microQAN)
pub const STORAGE_RATE_TEMPORARY: u64 = 1; // 0.000001 QAN/byte
pub const STORAGE_RATE_SHORT_TERM: u64 = 5; // 0.000005 QAN/byte
pub const STORAGE_RATE_PERMANENT: u64 = 10; // 0.00001 QAN/byte

/// Gas costs for different operations (compatible with Ethereum)
pub const GAS_COST_TRANSFER: u64 = 21_000;
pub const GAS_COST_CONTRACT_CALL: u64 = 25_000;
pub const GAS_COST_CONTRACT_DEPLOY: u64 = 53_000;
pub const GAS_COST_STORAGE_SET: u64 = 20_000;
pub const GAS_COST_STORAGE_RESET: u64 = 5_000;
pub const GAS_COST_LOG: u64 = 375;
pub const GAS_COST_COPY: u64 = 3; // per byte

/// Complexity multipliers for different transaction types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionComplexity {
    SimpleTransfer = 100,     // 1.0x
    MultiSignature = 120,     // 1.2x per additional signature
    SmartContract = 150,      // 1.5x base
    CrossChain = 200,         // 2.0x
    PrivacyTransaction = 300, // 3.0x
}

/// Storage duration categories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StorageDuration {
    Temporary, // < 1 block
    ShortTerm, // < 1000 blocks
    Permanent, // permanent storage
}

/// Fee breakdown structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeBreakdown {
    pub base_fee: u64,
    pub complexity_fee: u64,
    pub storage_fee: u64,
    pub gas_fee: u64,
    pub priority_fee: u64,
    pub congestion_multiplier: f64,
    pub total_fee: u64,
    pub gas_used: u64,
    pub gas_price: u64,
}

/// Network congestion metrics
#[derive(Debug, Clone)]
pub struct CongestionMetrics {
    pub current_tps: Arc<AtomicU64>,
    pub average_block_time: Arc<AtomicU64>,
    pub mempool_size: Arc<AtomicU64>,
    pub last_update: Arc<AtomicU64>,
}

impl Default for CongestionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionMetrics {
    pub fn new() -> Self {
        Self {
            current_tps: Arc::new(AtomicU64::new(0)),
            average_block_time: Arc::new(AtomicU64::new(1000)), // 1 second default
            mempool_size: Arc::new(AtomicU64::new(0)),
            last_update: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn update_tps(&self, tps: u64) {
        self.current_tps.store(tps, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_update.store(now, Ordering::Relaxed);
    }

    pub fn get_current_tps(&self) -> u64 {
        self.current_tps.load(Ordering::Relaxed)
    }
}

/// Enhanced gas-based fee model
#[derive(Debug, Clone)]
pub struct GasFeeModel {
    pub congestion_metrics: CongestionMetrics,
    pub gas_price_oracle: Arc<AtomicU64>,
    pub fee_history: Arc<std::sync::Mutex<Vec<FeeBreakdown>>>,
}

impl Default for GasFeeModel {
    fn default() -> Self {
        Self::new()
    }
}

impl GasFeeModel {
    pub fn new() -> Self {
        Self {
            congestion_metrics: CongestionMetrics::new(),
            gas_price_oracle: Arc::new(AtomicU64::new(1)), // 1 microQAN per gas unit
            fee_history: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Calculate comprehensive transaction fee with gas model
    pub fn calculate_transaction_fee(
        &self,
        transaction: &Transaction,
        gas_limit: u64,
        priority_fee: u64,
        storage_duration: StorageDuration,
    ) -> Result<FeeBreakdown, GasFeeError> {
        // 1. Base fee
        let base_fee = BASE_FEE_MICRO_QAN;

        // 2. Determine transaction complexity
        let complexity = self.determine_transaction_complexity(transaction);
        let complexity_multiplier = complexity as u64 as f64 / 100.0;
        let complexity_fee = (base_fee as f64 * (complexity_multiplier - 1.0)) as u64;

        // 3. Calculate storage fee
        let transaction_size = self.calculate_transaction_size(transaction);
        let storage_rate = match storage_duration {
            StorageDuration::Temporary => STORAGE_RATE_TEMPORARY,
            StorageDuration::ShortTerm => STORAGE_RATE_SHORT_TERM,
            StorageDuration::Permanent => STORAGE_RATE_PERMANENT,
        };
        let storage_fee = transaction_size * storage_rate;

        // 4. Calculate gas fee
        let gas_used = self.estimate_gas_usage(transaction, complexity);
        if gas_used > gas_limit {
            return Err(GasFeeError::GasLimitExceeded {
                used: gas_used,
                limit: gas_limit,
            });
        }

        let gas_price = self.gas_price_oracle.load(Ordering::Relaxed);
        let gas_fee = gas_used * gas_price;

        // 5. Calculate congestion multiplier
        let congestion_multiplier = self.calculate_congestion_multiplier();

        // 6. Apply congestion multiplier to base components
        let base_components = base_fee + complexity_fee + storage_fee + gas_fee;
        let adjusted_fee = (base_components as f64 * congestion_multiplier) as u64;

        // 7. Add priority fee (not affected by congestion)
        let total_fee = adjusted_fee + priority_fee;

        let breakdown = FeeBreakdown {
            base_fee,
            complexity_fee,
            storage_fee,
            gas_fee,
            priority_fee,
            congestion_multiplier,
            total_fee,
            gas_used,
            gas_price,
        };

        // Store in fee history for analytics
        if let Ok(mut history) = self.fee_history.lock() {
            history.push(breakdown.clone());
            // Keep only last 1000 entries
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        debug!(
            tx_id = %transaction.id,
            total_fee = total_fee,
            gas_used = gas_used,
            congestion_multiplier = congestion_multiplier,
            "Calculated transaction fee"
        );

        Ok(breakdown)
    }

    /// Determine transaction complexity based on transaction properties
    fn determine_transaction_complexity(&self, transaction: &Transaction) -> TransactionComplexity {
        // Check metadata for transaction type hints
        let metadata = transaction.get_metadata();

        if metadata.contains_key("contract_call") || metadata.contains_key("contract_deployment") {
            TransactionComplexity::SmartContract
        } else if metadata.contains_key("cross_chain") {
            TransactionComplexity::CrossChain
        } else if metadata.contains_key("privacy") || metadata.contains_key("zk_proof") {
            TransactionComplexity::PrivacyTransaction
        } else if transaction.inputs.len() > 1 || transaction.outputs.len() > 2 {
            // Multi-signature or complex transaction
            TransactionComplexity::MultiSignature
        } else {
            TransactionComplexity::SimpleTransfer
        }
    }

    /// Calculate transaction size in bytes
    fn calculate_transaction_size(&self, transaction: &Transaction) -> u64 {
        // Estimate serialized transaction size
        let base_size = 32 + 8 + 8; // id + amount + fee
        let inputs_size = transaction.inputs.len() * 64; // 64 bytes per input
        let outputs_size = transaction.outputs.len() * 64; // 64 bytes per output
        let signature_size = 64; // Standard signature size
        let metadata_size = transaction.get_metadata().len() * 32; // Estimate metadata size

        (base_size + inputs_size + outputs_size + signature_size + metadata_size) as u64
    }

    /// Estimate gas usage for transaction
    fn estimate_gas_usage(
        &self,
        transaction: &Transaction,
        complexity: TransactionComplexity,
    ) -> u64 {
        let base_gas = match complexity {
            TransactionComplexity::SimpleTransfer => GAS_COST_TRANSFER,
            TransactionComplexity::MultiSignature => {
                GAS_COST_TRANSFER + (transaction.inputs.len() as u64 * 5000)
            }
            TransactionComplexity::SmartContract => {
                let metadata = transaction.get_metadata();
                if metadata.contains_key("contract_deployment") {
                    GAS_COST_CONTRACT_DEPLOY
                } else {
                    GAS_COST_CONTRACT_CALL
                }
            }
            TransactionComplexity::CrossChain => GAS_COST_TRANSFER * 2,
            TransactionComplexity::PrivacyTransaction => GAS_COST_TRANSFER * 3,
        };

        // Add gas for data/storage operations
        let data_gas = self.calculate_transaction_size(transaction) * GAS_COST_COPY;

        base_gas + data_gas
    }

    /// Calculate congestion multiplier based on current network state
    fn calculate_congestion_multiplier(&self) -> f64 {
        let current_tps = self.congestion_metrics.get_current_tps();

        if current_tps <= (TARGET_TPS as f64 * 0.7) as u64 {
            1.0 // Normal fees
        } else if current_tps <= TARGET_TPS {
            let utilization =
                (current_tps as f64 - TARGET_TPS as f64 * 0.7) / (TARGET_TPS as f64 * 0.3);
            1.0 + utilization * CONGESTION_ADJUSTMENT_FACTOR
        } else {
            // Exponential increase for overload conditions
            let overload_factor = current_tps as f64 / TARGET_TPS as f64;
            1.5 * overload_factor.powi(2)
        }
    }

    /// Update gas price based on network conditions
    pub fn update_gas_price(&self, new_price: u64) -> Result<(), GasFeeError> {
        if new_price == 0 {
            return Err(GasFeeError::InvalidGasPrice { price: new_price });
        }

        self.gas_price_oracle.store(new_price, Ordering::Relaxed);
        info!(gas_price = new_price, "Updated gas price");
        Ok(())
    }

    /// Get current gas price
    pub fn get_gas_price(&self) -> u64 {
        self.gas_price_oracle.load(Ordering::Relaxed)
    }

    /// Estimate fee for a transaction
    pub fn estimate_fee(
        &self,
        transaction: &Transaction,
        gas_limit: u64,
        priority_fee: u64,
    ) -> Result<FeeBreakdown, GasFeeError> {
        // Use short-term storage as default for estimation
        self.calculate_transaction_fee(
            transaction,
            gas_limit,
            priority_fee,
            StorageDuration::ShortTerm,
        )
    }

    /// Get fee statistics from history
    pub fn get_fee_statistics(&self) -> HashMap<String, f64> {
        let mut stats = HashMap::new();

        if let Ok(history) = self.fee_history.lock() {
            if history.is_empty() {
                return stats;
            }

            let total_fees: u64 = history.iter().map(|f| f.total_fee).sum();
            let total_gas: u64 = history.iter().map(|f| f.gas_used).sum();
            let count = history.len() as f64;

            stats.insert("average_fee".to_string(), total_fees as f64 / count);
            stats.insert("average_gas_used".to_string(), total_gas as f64 / count);
            stats.insert("current_gas_price".to_string(), self.get_gas_price() as f64);
            stats.insert(
                "congestion_multiplier".to_string(),
                self.calculate_congestion_multiplier(),
            );
            stats.insert(
                "current_tps".to_string(),
                self.congestion_metrics.get_current_tps() as f64,
            );
        }

        stats
    }

    /// Batch fee calculation for multiple transactions
    pub fn calculate_batch_fees(
        &self,
        transactions: &[Transaction],
        gas_limits: &[u64],
        priority_fees: &[u64],
        storage_duration: StorageDuration,
    ) -> Result<Vec<FeeBreakdown>, GasFeeError> {
        if transactions.len() != gas_limits.len() || transactions.len() != priority_fees.len() {
            return Err(GasFeeError::FeeCalculationError {
                reason: "Mismatched array lengths for batch calculation".to_string(),
            });
        }

        let mut results = Vec::with_capacity(transactions.len());

        for (i, transaction) in transactions.iter().enumerate() {
            let breakdown = self.calculate_transaction_fee(
                transaction,
                gas_limits[i],
                priority_fees[i],
                storage_duration,
            )?;
            results.push(breakdown);
        }

        // Apply batch discount (5% for 10+ transactions, 10% for 50+ transactions)
        if transactions.len() >= 50 {
            for breakdown in &mut results {
                breakdown.total_fee = (breakdown.total_fee as f64 * 0.9) as u64;
            }
        } else if transactions.len() >= 10 {
            for breakdown in &mut results {
                breakdown.total_fee = (breakdown.total_fee as f64 * 0.95) as u64;
            }
        }

        Ok(results)
    }

    /// Update network congestion metrics
    pub fn update_congestion_metrics(&self, tps: u64, block_time_ms: u64, mempool_size: u64) {
        self.congestion_metrics.update_tps(tps);
        self.congestion_metrics
            .average_block_time
            .store(block_time_ms, Ordering::Relaxed);
        self.congestion_metrics
            .mempool_size
            .store(mempool_size, Ordering::Relaxed);
    }

    /// Check if transaction can afford the fee
    pub fn validate_transaction_fee(
        &self,
        transaction: &Transaction,
        gas_limit: u64,
        priority_fee: u64,
        available_balance: u64,
    ) -> Result<FeeBreakdown, GasFeeError> {
        let breakdown = self.estimate_fee(transaction, gas_limit, priority_fee)?;

        if breakdown.total_fee > available_balance {
            return Err(GasFeeError::InsufficientGasBalance {
                required: breakdown.total_fee,
                available: available_balance,
            });
        }

        Ok(breakdown)
    }
}

/// Fee estimation API for external use
pub struct FeeEstimator {
    gas_model: GasFeeModel,
}

impl FeeEstimator {
    pub fn new(gas_model: GasFeeModel) -> Self {
        Self { gas_model }
    }

    /// Get current fee estimates for different transaction types
    pub fn get_fee_estimates(&self) -> HashMap<String, u64> {
        let mut estimates = HashMap::new();
        let base_gas_price = self.gas_model.get_gas_price();
        let congestion_multiplier = self.gas_model.calculate_congestion_multiplier();

        // Simple transfer
        let simple_fee = (BASE_FEE_MICRO_QAN + GAS_COST_TRANSFER * base_gas_price) as f64
            * congestion_multiplier;
        estimates.insert("simple_transfer".to_string(), simple_fee as u64);

        // Smart contract call
        let contract_fee = (BASE_FEE_MICRO_QAN + GAS_COST_CONTRACT_CALL * base_gas_price) as f64
            * congestion_multiplier
            * 1.5;
        estimates.insert("contract_call".to_string(), contract_fee as u64);

        // Contract deployment
        let deploy_fee = (BASE_FEE_MICRO_QAN + GAS_COST_CONTRACT_DEPLOY * base_gas_price) as f64
            * congestion_multiplier
            * 3.0;
        estimates.insert("contract_deployment".to_string(), deploy_fee as u64);

        estimates
    }

    /// Predict fee for next N blocks
    pub fn predict_fees(&self, blocks_ahead: u32) -> Vec<HashMap<String, u64>> {
        let mut predictions = Vec::new();

        for i in 1..=blocks_ahead {
            // Simple prediction: assume linear TPS change
            let current_tps = self.gas_model.congestion_metrics.get_current_tps();
            let predicted_tps = current_tps + (i as u64 * 1000); // Assume 1k TPS increase per block

            // Temporarily update TPS for prediction
            let original_tps = current_tps;
            self.gas_model.congestion_metrics.update_tps(predicted_tps);

            let estimates = self.get_fee_estimates();
            predictions.push(estimates);

            // Restore original TPS
            self.gas_model.congestion_metrics.update_tps(original_tps);
        }

        predictions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{Input, Output, Transaction};
    use crate::types::{HomomorphicEncrypted, QuantumResistantSignature};

    fn create_test_transaction() -> Transaction {
        Transaction {
            id: "test_tx_001".to_string(),
            sender: "test_sender".to_string(),
            receiver: "test_receiver".to_string(),
            amount: 1000000, // 1 QAN
            fee: 1000,
            gas_limit: 21000,
            gas_used: 0,
            gas_price: 1000,
            priority_fee: 0,
            inputs: vec![Input {
                tx_id: "prev_hash".to_string(),
                output_index: 0,
            }],
            outputs: vec![Output {
                address: "test_recipient".to_string(),
                amount: 1000000, // 1 QAN
                homomorphic_encrypted: HomomorphicEncrypted::default(),
            }],
            timestamp: 1640995200,
            metadata: std::collections::HashMap::new(),
            signature: QuantumResistantSignature::default(),
            fee_breakdown: None,
        }
    }

    #[test]
    fn test_simple_transfer_fee_calculation() {
        let gas_model = GasFeeModel::new();
        let transaction = create_test_transaction();

        let result = gas_model.calculate_transaction_fee(
            &transaction,
            50000, // gas limit
            0,     // priority fee
            StorageDuration::ShortTerm,
        );

        assert!(result.is_ok());
        let breakdown = result.unwrap();
        assert_eq!(breakdown.base_fee, BASE_FEE_MICRO_QAN);
        assert!(breakdown.total_fee > 0);
    }

    #[test]
    fn test_congestion_multiplier() {
        let gas_model = GasFeeModel::new();

        // Test normal conditions
        gas_model.congestion_metrics.update_tps(5_000_000); // 50% of target
        let multiplier = gas_model.calculate_congestion_multiplier();
        assert_eq!(multiplier, 1.0);

        // Test high congestion
        gas_model.congestion_metrics.update_tps(15_000_000); // 150% of target
        let multiplier = gas_model.calculate_congestion_multiplier();
        assert!(multiplier > 1.5);
    }

    #[test]
    fn test_batch_fee_calculation() {
        let gas_model = GasFeeModel::new();
        let transactions = vec![create_test_transaction(); 15]; // 15 transactions for batch discount
        let gas_limits = vec![50000; 15];
        let priority_fees = vec![0; 15];

        let result = gas_model.calculate_batch_fees(
            &transactions,
            &gas_limits,
            &priority_fees,
            StorageDuration::ShortTerm,
        );

        assert!(result.is_ok());
        let breakdowns = result.unwrap();
        assert_eq!(breakdowns.len(), 15);

        // Check that batch discount was applied (5% for 10+ transactions)
        for breakdown in &breakdowns {
            assert!(breakdown.total_fee > 0);
        }
    }

    #[test]
    fn test_gas_limit_exceeded() {
        let gas_model = GasFeeModel::new();
        let transaction = create_test_transaction();

        let result = gas_model.calculate_transaction_fee(
            &transaction,
            1000, // Very low gas limit
            0,
            StorageDuration::ShortTerm,
        );

        assert!(matches!(result, Err(GasFeeError::GasLimitExceeded { .. })));
    }

    #[test]
    fn test_fee_estimator() {
        let gas_model = GasFeeModel::new();
        let estimator = FeeEstimator::new(gas_model);

        let estimates = estimator.get_fee_estimates();
        assert!(estimates.contains_key("simple_transfer"));
        assert!(estimates.contains_key("contract_call"));
        assert!(estimates.contains_key("contract_deployment"));

        // Contract calls should be more expensive than simple transfers
        assert!(estimates["contract_call"] > estimates["simple_transfer"]);
        assert!(estimates["contract_deployment"] > estimates["contract_call"]);
    }
}
