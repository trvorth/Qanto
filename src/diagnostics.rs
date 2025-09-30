//! Qanto Diagnostics Module
//!
//! Provides comprehensive debugging and diagnostic capabilities for the Qanto testnet,
//! including mining loop monitoring, difficulty tracking, and strata state analysis.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Mining attempt diagnostic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningAttempt {
    pub attempt_id: u64,
    pub timestamp: u64,
    pub difficulty: u64,
    pub target_block_time: u64,
    pub actual_time_ms: u64,
    pub nonce: u64,
    pub hash_rate: f64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Strata state diagnostic information for infinite-strata feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrataState {
    pub layer_count: u32,
    pub current_layer: u32,
    pub vdf_iterations: u64,
    pub vdf_difficulty: u64,
    pub proof_verification_time_ms: u64,
    pub quantum_state_valid: bool,
    pub threshold_signatures_count: u32,
}

/// Network diagnostic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDiagnostics {
    pub peer_count: u32,
    pub rpc_port: u16,
    pub websocket_port: u16,
    pub last_block_received: u64,
    pub sync_status: String,
    pub connection_errors: u32,
}

/// Comprehensive diagnostic data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSnapshot {
    pub timestamp: u64,
    pub block_height: u64,
    pub mining_attempts: Vec<MiningAttempt>,
    pub strata_state: Option<StrataState>,
    pub network_diagnostics: NetworkDiagnostics,
    pub config_validation_errors: Vec<String>,
    pub wallet_status: String,
}

#[derive(Debug, Clone)]
pub struct MiningAttemptParams {
    pub attempt_id: u64,
    pub difficulty: u64,
    pub target_block_time: u64,
    pub start_time: SystemTime,
    pub nonce: u64,
    pub hash_rate: f64,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StrataStateParams {
    pub layer_count: u32,
    pub current_layer: u32,
    pub vdf_iterations: u64,
    pub vdf_difficulty: u64,
    pub proof_verification_time_ms: u64,
    pub quantum_state_valid: bool,
    pub threshold_signatures_count: u32,
}

/// Main diagnostics engine
pub struct DiagnosticsEngine {
    debug_mode: bool,
    snapshots: Arc<RwLock<Vec<DiagnosticSnapshot>>>,
    mining_attempts: Arc<RwLock<Vec<MiningAttempt>>>,
    max_snapshots: usize,
    last_snapshot_time: Arc<RwLock<u64>>,
}

impl DiagnosticsEngine {
    /// Create a new diagnostics engine
    pub fn new(debug_mode: bool) -> Self {
        Self {
            debug_mode,
            snapshots: Arc::new(RwLock::new(Vec::new())),
            mining_attempts: Arc::new(RwLock::new(Vec::new())),
            max_snapshots: 100, // Keep last 100 snapshots
            last_snapshot_time: Arc::new(RwLock::new(0)),
        }
    }

    /// Record a mining attempt with detailed diagnostics
    pub async fn record_mining_attempt(&self, params: MiningAttemptParams) -> Result<()> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let actual_time_ms = params.start_time.elapsed()?.as_millis() as u64;

        let attempt = MiningAttempt {
            attempt_id: params.attempt_id,
            timestamp,
            difficulty: params.difficulty,
            target_block_time: params.target_block_time,
            actual_time_ms,
            nonce: params.nonce,
            hash_rate: params.hash_rate,
            success: params.success,
            error_message: params.error_message.clone(),
        };

        if self.debug_mode {
            if params.success {
                info!(
                    "🎯 Mining Success - Attempt #{}: difficulty={}, time={} ms, hash_rate={:.2} H/s, nonce={}",
                    params.attempt_id, params.difficulty, actual_time_ms, params.hash_rate, params.nonce
                );
            } else {
                warn!(
                    "⛏️  Mining Attempt #{}: difficulty={}, time={} ms, hash_rate={:.2} H/s, error={:?}",
                    params.attempt_id, params.difficulty, actual_time_ms, params.hash_rate, params.error_message
                );
            }

            // Log difficulty analysis
            if actual_time_ms > params.target_block_time * 2 {
                warn!(
                    "🐌 Slow mining detected: {} ms > {} ms target ({}x slower)",
                    actual_time_ms,
                    params.target_block_time,
                    actual_time_ms / params.target_block_time.max(1)
                );
            }
        }

        let mut attempts = self.mining_attempts.write().await;
        attempts.push(attempt);

        // Keep only recent attempts
        if attempts.len() > 1000 {
            attempts.drain(0..500); // Remove oldest 500
        }

        Ok(())
    }

    ///Record strata state for infinite-strata debugging
    pub async fn record_strata_state(&self, params: StrataStateParams) -> Result<()> {
        if self.debug_mode {
            info!(
                "🔮 Strata State - Layer {}/{}: VDF iterations={}, difficulty={}, proof_time={} ms, quantum_valid={}, sigs={}",
                params.current_layer, params.layer_count, params.vdf_iterations, params.vdf_difficulty,
                params.proof_verification_time_ms, params.quantum_state_valid, params.threshold_signatures_count
            );
        }

        // Warn about potential VDF stalls
        if params.proof_verification_time_ms > 10000 {
            warn!(
                "🚨 VDF verification taking too long: {} ms (potential stall)",
                params.proof_verification_time_ms
            );
        }

        if !params.quantum_state_valid {
            error!("❌ Quantum state verification failed - potential security issue");
        }

        Ok(())
    }

    /// Create a comprehensive diagnostic snapshot
    pub async fn create_snapshot(
        &self,
        block_height: u64,
        network_diagnostics: NetworkDiagnostics,
        config_validation_errors: Vec<String>,
        wallet_status: String,
        strata_state: Option<StrataState>,
    ) -> Result<()> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Check if enough time has passed since last snapshot (avoid spam)
        {
            let last_time = *self.last_snapshot_time.read().await;
            if timestamp - last_time < 30 {
                return Ok(()); // Skip if less than 30 seconds
            }
        }

        let attempts = self.mining_attempts.read().await.clone();

        let snapshot = DiagnosticSnapshot {
            timestamp,
            block_height,
            mining_attempts: attempts,
            strata_state,
            network_diagnostics: network_diagnostics.clone(),
            config_validation_errors: config_validation_errors.clone(),
            wallet_status,
        };

        if self.debug_mode {
            info!(
                "📊 Diagnostic Snapshot - Block #{}: {} mining attempts, {} peers, RPC port {}",
                block_height,
                snapshot.mining_attempts.len(),
                network_diagnostics.peer_count,
                network_diagnostics.rpc_port
            );

            if !config_validation_errors.is_empty() {
                error!(
                    "⚠️  Config validation errors: {:?}",
                    config_validation_errors
                );
            }
        }

        let mut snapshots = self.snapshots.write().await;
        snapshots.push(snapshot);

        // Maintain max snapshots limit
        if snapshots.len() > self.max_snapshots {
            snapshots.drain(0..50); // Remove oldest 50
        }

        *self.last_snapshot_time.write().await = timestamp;

        Ok(())
    }

    /// Get recent mining statistics
    pub async fn get_mining_stats(&self) -> Result<MiningStats> {
        let attempts = self.mining_attempts.read().await;
        let recent_attempts: Vec<_> = attempts.iter().rev().take(100).cloned().collect();

        let total_attempts = recent_attempts.len();
        let successful_attempts = recent_attempts.iter().filter(|a| a.success).count();
        let success_rate = if total_attempts > 0 {
            (successful_attempts as f64) / (total_attempts as f64) * 100.0
        } else {
            0.0
        };

        let avg_time = if !recent_attempts.is_empty() {
            recent_attempts
                .iter()
                .map(|a| a.actual_time_ms)
                .sum::<u64>()
                / recent_attempts.len() as u64
        } else {
            0
        };

        let avg_hash_rate = if !recent_attempts.is_empty() {
            recent_attempts.iter().map(|a| a.hash_rate).sum::<f64>() / recent_attempts.len() as f64
        } else {
            0.0
        };

        let current_difficulty = recent_attempts.first().map(|a| a.difficulty).unwrap_or(0);

        Ok(MiningStats {
            total_attempts,
            successful_attempts,
            success_rate,
            avg_time_ms: avg_time,
            avg_hash_rate,
            current_difficulty,
        })
    }

    /// Export diagnostics to JSON for analysis
    pub async fn export_diagnostics(&self) -> Result<String> {
        let snapshots = self.snapshots.read().await;
        let export_data = ExportData {
            snapshots: snapshots.clone(),
            export_timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        };

        Ok(serde_json::to_string_pretty(&export_data)?)
    }

    /// Analyze potential issues and provide recommendations
    pub async fn analyze_issues(&self) -> Result<Vec<DiagnosticIssue>> {
        let mut issues = Vec::new();
        let stats = self.get_mining_stats().await?;

        // Check mining performance
        if stats.success_rate < 10.0 && stats.total_attempts > 10 {
            issues.push(DiagnosticIssue {
                severity: IssueSeverity::Critical,
                category: "Mining".to_string(),
                description: format!(
                    "Very low mining success rate: {:.1}% (expected >50%)",
                    stats.success_rate
                ),
                recommendation: "Consider reducing difficulty or checking mining configuration"
                    .to_string(),
            });
        }

        if stats.avg_time_ms > 30000 {
            issues.push(DiagnosticIssue {
                severity: IssueSeverity::Warning,
                category: "Performance".to_string(),
                description: format!(
                    "Mining attempts taking too long: {} ms average",
                    stats.avg_time_ms
                ),
                recommendation: "Consider enabling adaptive difficulty or checking system resources".to_string(),
            });
        }

        if stats.current_difficulty > 1000000 {
            issues.push(DiagnosticIssue {
                severity: IssueSeverity::Warning,
                category: "Configuration".to_string(),
                description: format!(
                    "Very high difficulty detected: {}",
                    stats.current_difficulty
                ),
                recommendation: "For testnet, consider using lower difficulty values".to_string(),
            });
        }

        Ok(issues)
    }

    /// Check if debug mode is enabled
    pub fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }
}

/// Mining statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningStats {
    pub total_attempts: usize,
    pub successful_attempts: usize,
    pub success_rate: f64,
    pub avg_time_ms: u64,
    pub avg_hash_rate: f64,
    pub current_difficulty: u64,
}

/// Export data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub snapshots: Vec<DiagnosticSnapshot>,
    pub export_timestamp: u64,
}

/// Diagnostic issue identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticIssue {
    pub severity: IssueSeverity,
    pub category: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Critical,
}

/// CLI integration helper functions
pub mod cli {
    use super::*;
    use clap::Args;

    /// CLI arguments for diagnostics
    #[derive(Args, Debug)]
    pub struct DiagnosticsArgs {
        /// Export diagnostics to file
        #[arg(long, help = "Export diagnostics data to specified file ")]
        pub export_diagnostics: Option<String>,

        /// Show mining statistics
        #[arg(long, help = "Display current mining statistics ")]
        pub show_mining_stats: bool,

        /// Analyze potential issues
        #[arg(long, help = "Analyze and report potential issues ")]
        pub analyze_issues: bool,
    }

    /// Process diagnostics CLI commands
    pub async fn process_diagnostics_commands(
        args: &DiagnosticsArgs,
        engine: &DiagnosticsEngine,
    ) -> Result<()> {
        if args.show_mining_stats {
            let stats = engine.get_mining_stats().await?;
            println!("📊 Mining Statistics:");
            println!("  Total Attempts: {}", stats.total_attempts);
            println!("  Successful: {}", stats.successful_attempts);
            println!("  Success Rate: {:.1}%", stats.success_rate);
            println!("  Average Time: {} ms", stats.avg_time_ms);
            println!("  Average Hash Rate: {:.2} H/s ", stats.avg_hash_rate);
            println!("  Current Difficulty: {}", stats.current_difficulty);
        }

        if args.analyze_issues {
            let issues = engine.analyze_issues().await?;
            if issues.is_empty() {
                println!("✅ No issues detected ");
            } else {
                println!("⚠️  Detected Issues:");
                for issue in issues {
                    println!(
                        "  [{:?}] {}: {}",
                        issue.severity, issue.category, issue.description
                    );
                    println!("    💡 {}", issue.recommendation);
                }
            }
        }

        if let Some(export_path) = &args.export_diagnostics {
            let data = engine.export_diagnostics().await?;
            tokio::fs::write(export_path, data).await?;
            println!("📁 Diagnostics exported to: {export_path}");
        }

        Ok(())
    }
}
