//! Mining Celebration Module
//! Provides colorful, celebratory output when blocks are successfully mined.
//! Enhanced for high-performance blockchain with quantum-proof messaging
//! v0.1.0
// Local logging config to avoid cross-crate dependency

#[derive(Debug, Clone, Default)]
pub struct LoggingConfig {
    pub enable_block_celebrations: bool,
    pub celebration_log_level: String,
    pub celebration_throttle_per_min: Option<u32>,
}

impl LoggingConfig {
    pub fn enabled_default() -> Self {
        Self {
            enable_block_celebrations: true,
            celebration_log_level: "info".to_string(),
            celebration_throttle_per_min: None,
        }
    }
}
use chrono::{DateTime, Local};
use colored::*;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Throttling state for celebration logging
#[derive(Debug)]
struct ThrottleState {
    last_reset: Instant,
    count: u32,
}

impl Default for ThrottleState {
    fn default() -> Self {
        Self {
            last_reset: Instant::now(),
            count: 0,
        }
    }
}

/// Global throttle state for celebration logging
static THROTTLE_STATE: LazyLock<Mutex<ThrottleState>> = LazyLock::new(|| {
    Mutex::new(ThrottleState {
        last_reset: Instant::now(),
        count: 0,
    })
});

/// Structure to hold mining statistics
#[derive(Debug, Clone)]
pub struct MiningStats {
    pub block_height: u64,
    pub block_hash: String,
    pub nonce: u64,
    pub difficulty: f64,
    pub timestamp: DateTime<Local>,
    pub transactions_count: usize,
    pub mining_time: Duration,
    pub hash_rate: f64,
    pub total_blocks_mined: u64,
    pub chain_id: u32,
    pub effort: u64,       // Number of hashes tried
    pub block_reward: u64, // Block reward in smallest units
}

#[allow(clippy::too_many_arguments)]
impl MiningStats {
    /// Creates a new MiningStats instance
    pub fn new(
        block_height: u64,
        block_hash: String,
        nonce: u64,
        difficulty: f64,
        transactions_count: usize,
        mining_time: Duration,
        effort: u64,
        total_blocks_mined: u64,
        chain_id: u32,
        block_reward: u64,
    ) -> Self {
        let hash_rate = if mining_time.as_secs() > 0 {
            effort as f64 / mining_time.as_secs_f64()
        } else {
            effort as f64
        };

        Self {
            block_height,
            block_hash,
            nonce,
            difficulty,
            timestamp: Local::now(),
            transactions_count,
            mining_time,
            hash_rate,
            total_blocks_mined,
            chain_id,
            effort,
            block_reward,
        }
    }

    /// Displays a celebratory message for successful mining
    pub fn display_celebration(&self) {
        // Clear line and print enhanced celebration header
        println!("\n{}", "═".repeat(88).bright_cyan().bold());

        // Enhanced celebration with quantum-themed emojis
        let celebration_emojis = [
            "🎉", "⛏️", "💎", "✨", "🚀", "⚡", "🏆", "🌟", "🔮", "⚛️", "🌌", "💫",
        ];
        let emoji_line = celebration_emojis.join(" ");

        println!("{}", emoji_line.bright_yellow().bold());
        println!(
            "{}",
            "🎊 ⚡ QUANTUM BLOCK SUCCESSFULLY MINED! ⚡ 🎊"
                .bold()
                .bright_green()
                .on_bright_black()
        );
        println!("{}", emoji_line.bright_yellow().bold());

        println!("{}", "═".repeat(88).bright_cyan().bold());

        // Block details section with enhanced formatting
        println!(
            "\n{}",
            "📊 Block Details:".bold().bright_white().underline()
        );
        println!("  {} {}", "Height:".bright_blue().bold(), {
            let mut height_str = String::with_capacity(15);
            height_str.push('#');
            height_str.push_str(&self.format_number(self.block_height));
            height_str.bright_yellow().bold()
        });
        println!(
            "  {} 0x{}",
            "Hash:".bright_blue().bold(),
            self.format_hash_colorful()
        );
        println!(
            "  {} {}",
            "Nonce:".bright_blue().bold(),
            self.format_number(self.nonce).bright_green().bold()
        );
        println!(
            "  {} {}",
            "Chain ID:".bright_blue().bold(),
            self.chain_id.to_string().bright_magenta().bold()
        );
        println!("  {} {}", "Difficulty:".bright_blue().bold(), {
            let mut diff_str = String::with_capacity(12);
            diff_str.push_str(&format!("{:.6}", self.difficulty));
            diff_str.bright_red().bold()
        });
        println!(
            "  {} {}",
            "Timestamp:".bright_blue().bold(),
            self.timestamp
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string()
                .bright_cyan()
                .bold()
        );

        // Enhanced transaction metrics with TPS calculation
        println!(
            "\n{}",
            "🚀 Transaction Metrics:".bold().bright_white().underline()
        );
        println!(
            "  {} {} transactions",
            "Contains:".bright_blue().bold(),
            self.format_number(self.transactions_count as u64)
                .bright_white()
                .bold()
        );

        // Calculate theoretical TPS based on 31ms block time
        let theoretical_tps = if self.mining_time.as_millis() > 0 {
            (self.transactions_count as f64 * 1000.0) / self.mining_time.as_millis() as f64
        } else {
            0.0
        };

        println!(
            "  {} {} TPS",
            "Throughput:".bright_blue().bold(),
            self.format_tps(theoretical_tps).bright_green().bold()
        );

        // Mining performance section with enhanced metrics
        println!(
            "\n{}",
            "⚙️  Mining Performance:".bold().bright_white().underline()
        );
        println!(
            "  {} {}",
            "Mining Time:".bright_blue().bold(),
            self.format_duration().bright_yellow().bold()
        );

        // Calculate BPS (Blocks Per Second)
        let bps = if self.mining_time.as_secs_f64() > 0.0 {
            1.0 / self.mining_time.as_secs_f64()
        } else {
            0.0
        };

        println!(
            "  {} {} BPS",
            "Block Rate:".bright_blue().bold(),
            format!("{bps:.2}").bright_magenta().bold()
        );

        println!(
            "  {} {} hashes",
            "Effort:".bright_blue().bold(),
            self.format_number(self.effort).bright_green().bold()
        );
        println!(
            "  {} {}",
            "Hash Rate:".bright_blue().bold(),
            self.format_hash_rate().bright_magenta().bold()
        );

        // Enhanced block reward section
        println!("\n{}", "💰 Block Reward:".bold().bright_white().underline());
        let reward_qanto = self.block_reward as f64 / 1_000_000_000.0;
        println!(
            "  {} {} QANTO",
            "Reward:".bright_blue().bold(),
            format!("{reward_qanto:.6}")
                .bright_yellow()
                .bold()
                .on_bright_black()
        );

        // Add reward value indicator
        if reward_qanto >= 50.0 {
            println!(
                "  {} {}",
                "Status:".bright_blue().bold(),
                "✅ Optimized Reward".bright_green().bold()
            );
        }

        // Performance indicators
        println!(
            "\n{}",
            "📈 Performance Indicators:"
                .bold()
                .bright_white()
                .underline()
        );
        println!(
            "  {} {}",
            "Total Blocks:".bright_blue().bold(),
            self.format_number(self.total_blocks_mined)
                .bright_green()
                .bold()
        );

        // Performance status indicators
        if bps >= 30.0 {
            println!(
                "  {} {}",
                "BPS Status:".bright_blue().bold(),
                "🚀 High Performance (>30 BPS)".bright_green().bold()
            );
        }

        if theoretical_tps >= 10_000_000.0 {
            println!(
                "  {} {}",
                "TPS Status:".bright_blue().bold(),
                "⚡ Ultra High Throughput (>10M TPS)".bright_green().bold()
            );
        }

        if self.transactions_count >= 312_000 {
            println!(
                "  {} {}",
                "Batch Size:".bright_blue().bold(),
                "💎 Optimized Batch (>312K tx)".bright_green().bold()
            );
        }

        // Quantum-proof success message with enhanced encouragements
        let quantum_encouragements = [
            "Quantum-resistant mining achieved! 🔮⚛️",
            "Blockchain secured for the quantum era! 🌌💫",
            "Mining at light speed! ⚡🚀",
            "Transcending classical limits! 🌟💎",
            "Quantum supremacy in action! 🔮⚡",
            "Future-proof mining excellence! 🛡️✨",
            "Defying quantum threats! ⚛️🏆",
            "Next-gen blockchain mastery! 🚀🌈",
        ];

        let random_index = (self.nonce as usize) % quantum_encouragements.len();
        println!(
            "\n{}",
            quantum_encouragements[random_index]
                .bold()
                .bright_yellow()
                .italic()
                .on_bright_black()
        );

        println!("{}", "═".repeat(88).bright_cyan().bold());
        println!();
    }

    /// Formats the block hash with enhanced colorful segments
    fn format_hash_colorful(&self) -> String {
        let hash = &self.block_hash;
        if hash.len() >= 20 {
            {
                let mut result = String::with_capacity(30);
                result.push_str(&hash[0..6].bright_red().bold().to_string());
                result.push_str(&hash[6..12].bright_green().bold().to_string());
                result.push_str(&hash[12..18].bright_blue().bold().to_string());
                result.push_str(&hash[18..20].bright_magenta().bold().to_string());
                result.push_str("...");
                result
            }
        } else {
            hash.bright_white().bold().to_string()
        }
    }

    /// Formats TPS in a human-readable way
    fn format_tps(&self, tps: f64) -> String {
        if tps >= 1_000_000.0 {
            format!("{:.2}M", tps / 1_000_000.0)
        } else if tps >= 1_000.0 {
            format!("{:.2}K", tps / 1_000.0)
        } else {
            format!("{tps:.2}")
        }
    }

    /// Formats duration in a human-readable way
    fn format_duration(&self) -> String {
        let total_secs = self.mining_time.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        let millis = self.mining_time.subsec_millis();

        if hours > 0 {
            let mut duration_str = String::with_capacity(20);
            duration_str.push_str(&hours.to_string());
            duration_str.push('h');
            duration_str.push(' ');
            duration_str.push_str(&minutes.to_string());
            duration_str.push_str("m ");
            duration_str.push_str(&seconds.to_string());
            duration_str.push('s');
            duration_str
        } else if minutes > 0 {
            let mut duration_str = String::with_capacity(15);
            duration_str.push_str(&minutes.to_string());
            duration_str.push_str("m ");
            duration_str.push_str(&seconds.to_string());
            duration_str.push('.');
            duration_str.push_str(&millis.to_string());
            duration_str.push('s');
            duration_str
        } else {
            let mut duration_str = String::with_capacity(10);
            duration_str.push_str(&seconds.to_string());
            duration_str.push('.');
            duration_str.push_str(&millis.to_string());
            duration_str.push('s');
            duration_str
        }
    }

    /// Formats hash rate in a human-readable way
    fn format_hash_rate(&self) -> String {
        format_hash_rate(self.hash_rate)
    }

    /// Formats large numbers with thousand separators
    fn format_number(&self, num: u64) -> String {
        let num_str = num.to_string();
        let len = num_str.len();

        // Pre-calculate result capacity: original length + commas
        let comma_count = (len.saturating_sub(1)) / 3;
        let mut result = String::with_capacity(len + comma_count);

        // Insert commas from right to left without reversing
        for (i, ch) in num_str.chars().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(ch);
        }

        result
    }

    /// Displays a compact one-line celebration
    pub fn display_compact(&self) {
        let reward_qanto = self.block_reward as f64 / 1_000_000_000.0;
        println!(
            "🎉 Block #{} Mined! | Hash: 0x{}... | 💰 Reward: {:.3} QANTO",
            self.block_height,
            &self.block_hash[..8],
            reward_qanto
        );
    }
}

/// Helper function to create and display mining celebration
#[allow(clippy::too_many_arguments)]
#[derive(Debug)]
pub struct MiningCelebrationParams {
    pub block_height: u64,
    pub block_hash: String,
    pub nonce: u64,
    pub difficulty: f64,
    pub transactions_count: usize,
    pub mining_time: Duration,
    pub effort: u64,
    pub total_blocks_mined: u64,
    pub chain_id: u32,
    pub block_reward: u64,
    pub compact: bool,
}

/// Canonical hook for block mining celebration with configurable logging
pub fn on_block_mined(params: MiningCelebrationParams, config: &LoggingConfig) {
    // Check if celebrations are enabled
    if !config.enable_block_celebrations {
        return;
    }

    // Check throttling if configured
    if let Some(throttle_limit) = config.celebration_throttle_per_min {
        let mut state = THROTTLE_STATE.lock().unwrap();
        let now = Instant::now();

        // Reset counter if a minute has passed
        if now.duration_since(state.last_reset) >= Duration::from_secs(60) {
            state.count = 0;
            state.last_reset = now;
        }

        // Check if we've exceeded the throttle limit
        if state.count >= throttle_limit {
            return;
        }

        state.count += 1;
    }

    // Create mining stats for metadata
    let _stats = MiningStats::new(
        params.block_height,
        params.block_hash.clone(),
        params.nonce,
        params.difficulty,
        params.transactions_count,
        params.mining_time,
        params.effort,
        params.total_blocks_mined,
        params.chain_id,
        params.block_reward,
    );

    // Log celebration message based on configured level
    let message = format!(
        "🎉 Mined block #{}, id={}, time_ms={}, effort={}, txs={}",
        params.block_height,
        &params.block_hash[..8], // Show first 8 chars of hash
        params.mining_time.as_millis(),
        params.effort,
        params.transactions_count
    );

    match config.celebration_log_level.as_str() {
        "debug" => debug!("{}", message),
        "info" => info!("{}", message),
        "warn" => warn!("{}", message),
        _ => info!("{}", message), // Default to info
    }
}

/// Legacy function for backward compatibility - now uses configurable logging
pub fn celebrate_mining_success(params: MiningCelebrationParams) {
    // For backward compatibility, create a default config that enables celebrations
    let default_config = LoggingConfig::enabled_default();

    on_block_mined(params, &default_config);
}

// Provide a free function for hash rate formatting used by tests and other modules
pub fn format_hash_rate(rate_hps: f64) -> String {
    if rate_hps >= 1_000_000_000.0 {
        format!("{:.2} GH/s", rate_hps / 1_000_000_000.0)
    } else if rate_hps >= 1_000_000.0 {
        format!("{:.2} MH/s", rate_hps / 1_000_000.0)
    } else if rate_hps >= 1_000.0 {
        format!("{:.2} kH/s", rate_hps / 1_000.0)
    } else {
        format!("{rate_hps:.2} H/s")
    }
}

#[cfg(test)]
mod tests {
    use super::format_hash_rate;

    #[test]
    fn test_format_hash_rate_h() {
        let s = format_hash_rate(500.0);
        assert_eq!(s, "500.00 H/s");
    }

    #[test]
    fn test_format_hash_rate_kh() {
        let s = format_hash_rate(2_500.0);
        assert_eq!(s, "2.50 kH/s");
    }

    #[test]
    fn test_format_hash_rate_mh() {
        let s = format_hash_rate(5_000_000.0);
        assert_eq!(s, "5.00 MH/s");
    }

    #[test]
    fn test_format_hash_rate_gh() {
        let s = format_hash_rate(6_000_000_000.0);
        assert_eq!(s, "6.00 GH/s");
    }
}
