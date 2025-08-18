//! # Qanto Performance Test Binary
//!
//! This binary runs comprehensive performance validation tests to verify
//! that Qanto meets its performance targets of 32 BPS and 10M+ TPS.

use colored::*;
use std::env;
use std::process;
use tracing::{error, info};

use qanto::performance_validation::validate_performance_targets;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!(
        "{}",
        "🚀 Qanto Performance Validation Suite".bright_cyan().bold()
    );
    println!("{}", "====================================".bright_cyan());
    println!();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let duration_secs = if args.len() > 1 {
        match args[1].parse::<u64>() {
            Ok(duration) if duration > 0 => duration,
            _ => {
                eprintln!(
                    "{}",
                    "Error: Duration must be a positive integer (seconds)".bright_red()
                );
                eprintln!("Usage: {} [duration_seconds]", args[0]);
                eprintln!("Example: {} 30", args[0]);
                process::exit(1);
            }
        }
    } else {
        30 // Default to 30 seconds
    };

    info!(
        "Starting performance validation for {} seconds...",
        duration_secs
    );
    println!(
        "⏱️  Test Duration: {} seconds",
        duration_secs.to_string().bright_yellow()
    );
    println!("🎯 Targets: 32 BPS | 10M+ TPS");
    println!();

    // Run performance validation
    match validate_performance_targets(duration_secs).await {
        Ok(results) => {
            println!();
            info!("Performance validation completed successfully!");

            // Exit with appropriate code based on results
            if results.bps_target_met && results.tps_target_met {
                println!(
                    "{}",
                    "🎉 All performance targets achieved!".bright_green().bold()
                );
                process::exit(0);
            } else {
                println!(
                    "{}",
                    "⚠️  Some performance targets not met"
                        .bright_yellow()
                        .bold()
                );
                if !results.bps_target_met {
                    println!("   • BPS target missed: {:.2} < 32", results.bps_achieved);
                }
                if !results.tps_target_met {
                    println!("   • TPS target missed: {:.0} < 10M", results.tps_achieved);
                }
                process::exit(1);
            }
        }
        Err(e) => {
            error!("Performance validation failed: {}", e);
            println!(
                "{}",
                format!("❌ Validation Error: {e}").bright_red().bold()
            );
            process::exit(2);
        }
    }
}
