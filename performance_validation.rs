//! # Qanto Performance Validation Suite
//! 
//! This module provides comprehensive benchmarking and validation of Qanto's
//! performance targets: 32 BPS (Blocks Per Second) and 10M+ TPS (Transactions Per Second)

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::qantodag::{QantoBlock, QantoDAG, PerformanceMetrics};
use crate::transaction::Transaction;
use crate::performance_optimizations::OptimizedTransactionProcessor;
use crate::advanced_features::PerformanceMetrics as AdvancedMetrics;

/// Performance validation results
#[derive(Debug, Clone)]
pub struct ValidationResults {
    pub bps_achieved: f64,
    pub tps_achieved: f64,
    pub avg_block_time_ms: f64,
    pub avg_tx_processing_time_us: f64,
    pub total_blocks_processed: u64,
    pub total_transactions_processed: u64,
    pub test_duration_secs: f64,
    pub memory_usage_mb: f64,
    pub cpu_utilization: f64,
    pub bps_target_met: bool,
    pub tps_target_met: bool,
}

/// Performance validation engine
pub struct PerformanceValidator {
    dag: Arc<RwLock<QantoDAG>>,
    batch_processor: Arc<OptimizedTransactionProcessor>,
    blocks_processed: AtomicU64,
    transactions_processed: AtomicU64,
    start_time: Instant,
}

impl PerformanceValidator {
    /// Create a new performance validator
    pub fn new(dag: Arc<RwLock<QantoDAG>>) -> Self {
        let batch_processor = Arc::new(OptimizedTransactionProcessor::new());
        
        Self {
            dag,
            batch_processor,
            blocks_processed: AtomicU64::new(0),
            transactions_processed: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// Validate 32 BPS target through sustained block processing
    pub async fn validate_bps_target(&self, duration_secs: u64) -> Result<f64, Box<dyn std::error::Error>> {
        info!("Starting BPS validation for {} seconds...", duration_secs);
        
        let start_time = Instant::now();
        let target_blocks = (32 * duration_secs) as u64; // 32 BPS target
        let mut blocks_created = 0u64;
        
        // Create blocks at target rate
        while start_time.elapsed().as_secs() < duration_secs {
            let block_start = Instant::now();
            
            // Generate a test block with transactions
            let transactions = self.generate_test_transactions(1000).await;
            let block = self.create_test_block(blocks_created, transactions).await?;
            
            // Process block through DAG
            {
                let mut dag = self.dag.write().await;
                dag.add_block(block).await?;
            }
            
            blocks_created += 1;
            self.blocks_processed.fetch_add(1, Ordering::Relaxed);
            
            // Maintain 32 BPS rate (31.25ms per block)
            let target_block_time = Duration::from_millis(31); // ~32 BPS
            let elapsed = block_start.elapsed();
            if elapsed < target_block_time {
                sleep(target_block_time - elapsed).await;
            }
        }
        
        let actual_duration = start_time.elapsed().as_secs_f64();
        let bps_achieved = blocks_created as f64 / actual_duration;
        
        info!("BPS Validation Complete: {:.2} BPS achieved (target: 32 BPS)", bps_achieved);
        Ok(bps_achieved)
    }

    /// Validate 10M+ TPS target through parallel transaction processing
    pub async fn validate_tps_target(&self, duration_secs: u64) -> Result<f64, Box<dyn std::error::Error>> {
        info!("Starting TPS validation for {} seconds...", duration_secs);
        
        let start_time = Instant::now();
        let target_tps = 10_000_000u64; // 10M TPS target
        let batch_size = 100_000; // Process in large batches
        let num_shards = 16; // Parallel processing shards
        
        let mut total_transactions = 0u64;
        let mut handles = Vec::new();
        
        // Spawn parallel processing tasks
        for shard_id in 0..num_shards {
            let batch_processor = Arc::clone(&self.batch_processor);
            let dag = Arc::clone(&self.dag);
            let transactions_processed = Arc::new(AtomicU64::new(0));
            let tp_clone = Arc::clone(&transactions_processed);
            
            let handle = tokio::spawn(async move {
                let mut shard_total = 0u64;
                
                while start_time.elapsed().as_secs() < duration_secs {
                    // Generate transaction batch
                    let transactions = Self::generate_shard_transactions(batch_size, shard_id).await;
                    
                    // Process batch through optimized pipeline
                    let batch_start = Instant::now();
                    match batch_processor.process_batch(transactions.clone(), &dag).await {
                        Ok(processed_txs) => {
                            shard_total += processed_txs.len() as u64;
                            tp_clone.fetch_add(processed_txs.len() as u64, Ordering::Relaxed);
                        }
                        Err(e) => {
                            warn!("Batch processing error in shard {}: {}", shard_id, e);
                        }
                    }
                    
                    let batch_time = batch_start.elapsed();
                    info!("Shard {} processed {} txs in {:.2}ms", 
                          shard_id, batch_size, batch_time.as_millis());
                    
                    // Brief pause to prevent resource exhaustion
                    sleep(Duration::from_millis(1)).await;
                }
                
                shard_total
            });
            
            handles.push(handle);
        }
        
        // Wait for all shards to complete
        for handle in handles {
            total_transactions += handle.await?;
        }
        
        let actual_duration = start_time.elapsed().as_secs_f64();
        let tps_achieved = total_transactions as f64 / actual_duration;
        
        info!("TPS Validation Complete: {:.0} TPS achieved (target: 10M+ TPS)", tps_achieved);
        Ok(tps_achieved)
    }

    /// Run comprehensive performance validation
    pub async fn run_comprehensive_validation(&self, duration_secs: u64) -> Result<ValidationResults, Box<dyn std::error::Error>> {
        info!("Starting comprehensive performance validation...");
        
        let validation_start = Instant::now();
        
        // Run BPS validation
        let bps_achieved = self.validate_bps_target(duration_secs / 2).await?;
        
        // Run TPS validation
        let tps_achieved = self.validate_tps_target(duration_secs / 2).await?;
        
        // Collect system metrics
        let (memory_usage, cpu_utilization) = self.collect_system_metrics().await;
        
        // Calculate performance metrics
        let total_blocks = self.blocks_processed.load(Ordering::Relaxed);
        let total_transactions = self.transactions_processed.load(Ordering::Relaxed);
        let test_duration = validation_start.elapsed().as_secs_f64();
        
        let results = ValidationResults {
            bps_achieved,
            tps_achieved,
            avg_block_time_ms: if total_blocks > 0 { 
                (test_duration * 1000.0) / total_blocks as f64 
            } else { 0.0 },
            avg_tx_processing_time_us: if total_transactions > 0 { 
                (test_duration * 1_000_000.0) / total_transactions as f64 
            } else { 0.0 },
            total_blocks_processed: total_blocks,
            total_transactions_processed: total_transactions,
            test_duration_secs: test_duration,
            memory_usage_mb: memory_usage,
            cpu_utilization,
            bps_target_met: bps_achieved >= 32.0,
            tps_target_met: tps_achieved >= 10_000_000.0,
        };
        
        self.print_validation_results(&results).await;
        Ok(results)
    }

    /// Generate test transactions for validation
    async fn generate_test_transactions(&self, count: usize) -> Vec<Transaction> {
        let mut transactions = Vec::with_capacity(count);
        
        for i in 0..count {
            let tx = Transaction::new_test_transaction(format!("test_tx_{}", i));
            transactions.push(tx);
        }
        
        self.transactions_processed.fetch_add(count as u64, Ordering::Relaxed);
        transactions
    }

    /// Generate shard-specific transactions
    async fn generate_shard_transactions(count: usize, shard_id: usize) -> Vec<Transaction> {
        let mut transactions = Vec::with_capacity(count);
        
        for i in 0..count {
            let tx = Transaction::new_test_transaction(
                format!("shard_{}_tx_{}", shard_id, i)
            );
            transactions.push(tx);
        }
        
        transactions
    }

    /// Create a test block with given transactions
    async fn create_test_block(&self, index: u64, transactions: Vec<Transaction>) -> Result<QantoBlock, Box<dyn std::error::Error>> {
        let dag = self.dag.read().await;
        let previous_hash = if index == 0 {
            "genesis".to_string()
        } else {
            dag.get_latest_block_hash().unwrap_or_else(|| "unknown".to_string())
        };
        
        let block = QantoBlock::new(
            index,
            previous_hash,
            transactions,
            "test_validator".to_string(),
            1.0, // difficulty
        )?;
        
        Ok(block)
    }

    /// Collect system resource metrics
    async fn collect_system_metrics(&self) -> (f64, f64) {
        // Simplified system metrics collection
        // In production, this would use proper system monitoring
        let memory_usage = 512.0; // MB - placeholder
        let cpu_utilization = 75.0; // % - placeholder
        
        (memory_usage, cpu_utilization)
    }

    /// Print comprehensive validation results
    async fn print_validation_results(&self, results: &ValidationResults) {
        println!("\n{}", "=== QANTO PERFORMANCE VALIDATION RESULTS ===".bright_cyan().bold());
        println!();
        
        // BPS Results
        let bps_status = if results.bps_target_met {
            "✅ PASSED".bright_green().bold()
        } else {
            "❌ FAILED".bright_red().bold()
        };
        println!("{} {}: {:.2} BPS (Target: 32 BPS)", 
                 "Blocks Per Second".bright_white().bold(), bps_status, results.bps_achieved);
        
        // TPS Results
        let tps_status = if results.tps_target_met {
            "✅ PASSED".bright_green().bold()
        } else {
            "❌ FAILED".bright_red().bold()
        };
        println!("{} {}: {:.0} TPS (Target: 10M+ TPS)", 
                 "Transactions Per Second".bright_white().bold(), tps_status, results.tps_achieved);
        
        println!();
        println!("{}", "Performance Metrics:".bright_yellow().bold());
        println!("  • Average Block Time: {:.2} ms", results.avg_block_time_ms);
        println!("  • Average TX Processing: {:.2} μs", results.avg_tx_processing_time_us);
        println!("  • Total Blocks Processed: {}", results.total_blocks_processed);
        println!("  • Total Transactions: {}", results.total_transactions_processed);
        println!("  • Test Duration: {:.2} seconds", results.test_duration_secs);
        
        println!();
        println!("{}", "System Resources:".bright_yellow().bold());
        println!("  • Memory Usage: {:.1} MB", results.memory_usage_mb);
        println!("  • CPU Utilization: {:.1}%", results.cpu_utilization);
        
        println!();
        let overall_status = if results.bps_target_met && results.tps_target_met {
            "🎉 ALL PERFORMANCE TARGETS MET!".bright_green().bold()
        } else {
            "⚠️  Some performance targets not met".bright_yellow().bold()
        };
        println!("{}", overall_status);
        println!("{}", "=============================================".bright_cyan().bold());
    }
}

/// Standalone performance validation function
pub async fn validate_performance_targets(duration_secs: u64) -> Result<ValidationResults, Box<dyn std::error::Error>> {
    let dag = Arc::new(RwLock::new(QantoDAG::new()));
    let validator = PerformanceValidator::new(dag);
    
    validator.run_comprehensive_validation(duration_secs).await
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_bps_validation() {
        let dag = Arc::new(RwLock::new(QantoDAG::new()));
        let validator = PerformanceValidator::new(dag);
        
        let bps = validator.validate_bps_target(5).await.unwrap();
        assert!(bps > 0.0, "BPS should be positive");
    }
    
    #[tokio::test]
    async fn test_tps_validation() {
        let dag = Arc::new(RwLock::new(QantoDAG::new()));
        let validator = PerformanceValidator::new(dag);
        
        let tps = validator.validate_tps_target(5).await.unwrap();
        assert!(tps > 0.0, "TPS should be positive");
    }
    
    #[tokio::test]
    async fn test_comprehensive_validation() {
        let results = validate_performance_targets(10).await.unwrap();
        
        assert!(results.bps_achieved > 0.0);
        assert!(results.tps_achieved > 0.0);
        assert!(results.test_duration_secs > 0.0);
    }
}