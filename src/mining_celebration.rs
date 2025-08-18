//! Mining Celebration Module
//! Provides colorful, celebratory output when blocks are successfully mined.

use chrono::{DateTime, Local};
use colored::*;
use std::time::Duration;

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
        // Clear line and print celebration header
        println!("\n{}", "=".repeat(80).bright_cyan());

        // Main celebration with animated emojis
        let celebration_emojis = ["🎉", "⛏️", "💎", "✨", "🚀", "⚡", "🏆", "🌟"];
        let emoji_line = celebration_emojis.join(" ");

        println!("{}", emoji_line.bright_yellow());
        println!(
            "{}",
            "🎊 BLOCK SUCCESSFULLY MINED! 🎊"
                .bold()
                .bright_green()
                .on_bright_black()
        );
        println!("{}", emoji_line.bright_yellow());

        println!("{}", "=".repeat(80).bright_cyan());

        // Block details section
        println!("\n{}", "📊 Block Details:".bold().bright_white());
        println!("  {} {}", "Height:".bright_blue(), {
            let mut height_str = String::with_capacity(10);
            height_str.push('#');
            height_str.push_str(&self.block_height.to_string());
            height_str.bright_yellow()
        });
        println!(
            "  {} 0x{}",
            "Hash:".bright_blue(),
            self.format_hash_colorful()
        );
        println!(
            "  {} {}",
            "Nonce:".bright_blue(),
            self.nonce.to_string().bright_green()
        );
        println!(
            "  {} {}",
            "Chain ID:".bright_blue(),
            self.chain_id.to_string().bright_magenta()
        );
        println!("  {} {}", "Difficulty:".bright_blue(), {
            let mut diff_str = String::with_capacity(12);
            diff_str.push_str(&format!("{:.6}", self.difficulty)); // Keep format! for floating-point precision
            diff_str.bright_red()
        });
        println!(
            "  {} {}",
            "Timestamp:".bright_blue(),
            self.timestamp
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .bright_cyan()
        );
        println!(
            "  {} {} transaction(s)",
            "Contains:".bright_blue(),
            self.transactions_count.to_string().bright_white()
        );

        // Mining performance section
        println!("\n{}", "⚙️  Mining Performance:".bold().bright_white());
        println!(
            "  {} {}",
            "Mining Time:".bright_blue(),
            self.format_duration().bright_yellow()
        );
        println!(
            "  {} {} hashes",
            "Effort:".bright_blue(),
            self.format_number(self.effort).bright_green()
        );
        println!(
            "  {} {} H/s",
            "Hash Rate:".bright_blue(),
            self.format_hash_rate().bright_magenta()
        );

        // Block reward section
        println!("\n{}", "💰 Block Reward:".bold().bright_white());
        let reward_qanto = self.block_reward as f64 / 1_000_000.0; // Convert from smallest units to QANTO
        println!(
            "  {} {} QANTO",
            "Reward:".bright_blue(),
            format!("{reward_qanto:.6}").bright_yellow().bold() // Keep format! for floating-point precision
        );

        // Overall statistics
        println!("\n{}", "📈 Overall Statistics:".bold().bright_white());
        println!(
            "  {} {}",
            "Total Blocks Mined:".bright_blue(),
            self.total_blocks_mined.to_string().bright_green()
        );

        // Success message with random encouragement
        let encouragements = [
            "Keep up the great work! 💪",
            "You're on fire! 🔥",
            "Mining like a champion! 🏅",
            "Excellent performance! ⭐",
            "You're crushing it! 💥",
            "Outstanding work! 👏",
            "Mining master at work! 🎯",
            "Impressive results! 🌈",
        ];

        let random_index = (self.nonce as usize) % encouragements.len();
        println!(
            "\n{}",
            encouragements[random_index].bold().bright_yellow().italic()
        );

        println!("{}", "=".repeat(80).bright_cyan());
        println!();
    }

    /// Formats the block hash with colorful segments
    fn format_hash_colorful(&self) -> String {
        let hash = &self.block_hash;
        if hash.len() >= 16 {
            {
                let mut result = String::with_capacity(20);
                result.push_str(&hash[0..6].bright_red().to_string());
                result.push_str(&hash[6..12].bright_green().to_string());
                result.push_str(&hash[12..16].bright_blue().to_string());
                result.push_str("...");
                result
            }
        } else {
            hash.bright_white().to_string()
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
        if self.hash_rate >= 1_000_000_000.0 {
            let mut rate_str = String::with_capacity(10);
            rate_str.push_str(&format!("{:.2}", self.hash_rate / 1_000_000_000.0)); // Keep format! for floating-point precision
            rate_str.push_str(" GH");
            rate_str
        } else if self.hash_rate >= 1_000_000.0 {
            let mut rate_str = String::with_capacity(10);
            rate_str.push_str(&format!("{:.2}", self.hash_rate / 1_000_000.0)); // Keep format! for floating-point precision
            rate_str.push_str(" MH");
            rate_str
        } else if self.hash_rate >= 1_000.0 {
            let mut rate_str = String::with_capacity(10);
            rate_str.push_str(&format!("{:.2}", self.hash_rate / 1_000.0)); // Keep format! for floating-point precision
            rate_str.push_str(" KH");
            rate_str
        } else {
            format!("{:.2}", self.hash_rate) // Keep format! for floating-point precision
        }
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

pub fn celebrate_mining_success(params: MiningCelebrationParams) {
    let MiningCelebrationParams {
        block_height,
        block_hash,
        nonce,
        difficulty,
        transactions_count,
        mining_time,
        effort,
        total_blocks_mined,
        chain_id,
        block_reward,
        compact,
    } = params;
    let stats = MiningStats::new(
        block_height,
        block_hash,
        nonce,
        difficulty,
        transactions_count,
        mining_time,
        effort,
        total_blocks_mined,
        chain_id,
        block_reward,
    );

    if compact {
        stats.display_compact();
    } else {
        stats.display_celebration();
    }
}
