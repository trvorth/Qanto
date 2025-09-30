//! # Qanto Performance Test Binary
//!
//! This binary runs comprehensive performance validation tests to verify
//! that Qanto meets its performance targets of 32 BPS and 10M+ TPS.

use colored::*;
use std::env;
use std::process;
use std::time::Instant;
use tokio::time::Duration;
use tracing::{error, info, warn};

use qanto::performance_validation::validate_performance_targets;

#[tokio::main]
async fn main() {
    // Initialize logging with proper async support
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

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

    // Start timing for overall test duration
    let test_start = Instant::now();

    // Pre-warm the system with a short async delay
    info!("Pre-warming system components...");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Run performance validation with proper async timing
    info!("Starting concurrent performance validation...");
    match validate_performance_targets(duration_secs).await {
        Ok(results) => {
            let total_test_time = test_start.elapsed();
            println!();
            info!(
                "Performance validation completed in {:.2}s (target: {}s)",
                total_test_time.as_secs_f64(),
                duration_secs
            );

            // Log detailed performance metrics
            info!(
                "Achieved Performance - BPS: {:.2}, TPS: {:.0}, Avg Block Time: {:.2}ms",
                results.bps_achieved, results.tps_achieved, results.avg_block_time_ms
            );

            // Exit with appropriate code based on results
            if results.bps_target_met && results.tps_target_met {
                println!(
                    "{}",
                    "🎉 All performance targets achieved!".bright_green().bold()
                );
                info!("✅ BPS: {:.2} >= 32", results.bps_achieved);
                info!("✅ TPS: {:.0} >= 10M", results.tps_achieved);
                process::exit(0);
            } else {
                println!(
                    "{}",
                    "⚠️  Some performance targets not met"
                        .bright_yellow()
                        .bold()
                );
                if !results.bps_target_met {
                    warn!("❌ BPS target missed: {:.2} < 32", results.bps_achieved);
                }
                if !results.tps_target_met {
                    warn!("❌ TPS target missed: {:.0} < 10M", results.tps_achieved);
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
