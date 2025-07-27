// src/bin/saga_cli.rs

use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tracing::error;

/// Prints a line of text and then pauses for a specified duration.
fn print_with_delay(text: &str, delay_ms: u64) {
    println!("{text}");
    io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(delay_ms));
}

/// Simulates the sequential output of the security analysis tasks.
fn run_analysis(risk_type: &str) {
    match risk_type {
        "centralization" => {
            print_with_delay("+ Initializing security analysis protocol...", 100);
            print_with_delay("-- Accessing DAG state for validator distribution...", 200);
            print_with_delay(
                "-- Fetching block production data for last 10 epochs...",
                200,
            );
            print_with_delay(
                "! SecurityMonitor: Calculating Herfindahl-Hirschman Index (HHI)...",
                350,
            );
            print_with_delay(
                "HHI calculated: 1850. Moderate concentration detected.",
                100,
            );
            print_with_delay("+ Formulating insight based on HHI score...", 150);
            print_with_delay("✓ Analysis complete. Centralization risk is moderate but within nominal parameters.", 100);
        }
        "spam" => {
            print_with_delay("+ Initializing security analysis protocol...", 100);
            print_with_delay(
                "-- Analyzing transaction metadata integrity for the last 200 blocks...",
                250,
            );
            print_with_delay("-- Calculating zero-fee transaction ratio...", 250);
            print_with_delay(
                "! SecurityMonitor: Zero-fee ratio at 12%. Shannon entropy of metadata is nominal.",
                350,
            );
            print_with_delay(
                "✓ Analysis complete. No indicators of a coordinated spam attack detected.",
                100,
            );
        }
        _ => {
            println!("Unknown analysis type. Try 'centralization' or 'spam'.");
        }
    }
}

/// Simulates the Sense-Think-Act cycle for autonomous governance.
fn run_simulation() {
    print_with_delay(
        "+ Starting Sense-Think-Act loop for epoch governance...",
        100,
    );
    print_with_delay("[Sense] Analyzing long-term network trends...", 200);
    print_with_delay(
        "-- Trend detected: Sustained network congestion (avg > 0.85 for 3 epochs).",
        250,
    );
    print_with_delay(
        "-- Trend detected: High proposal rejection rate (78%).",
        250,
    );
    print_with_delay("[Think] Formulating strategic proposals...", 200);
    print_with_delay(
        "! Proposal Drafted: Increase 'base_tx_fee_min' by 15% to mitigate spam.",
        350,
    );
    print_with_delay("! Proposal Drafted: Lower 'proposal_vote_threshold' by 10% to encourage governance participation.", 350);
    print_with_delay("[Act] Submitting proposals to governance queue...", 150);
    print_with_delay(
        "✓ Autonomous governance cycle complete. 2 new proposals submitted.",
        100,
    );
}

/// Processes the user's command input and prints the appropriate response.
fn process_command(input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts.first().unwrap_or(&"");
    let args = &parts[1..];

    match *command {
        "/help" => {
            println!("Available commands:");
            println!("  /ask [topic]       - Get info (e.g., /ask about scs)");
            println!(
                "  /analyze [risk]    - Run security analysis (e.g., /analyze centralization)"
            );
            println!("  /simulate cycle    - Run a mock Sense-Think-Act loop");
            println!("  /status            - View detailed network status");
            println!("  /clear             - Clear the terminal screen");
            println!("  exit               - Quit the console");
        }
        "/status" => {
            println!("+--------------------------------+");
            println!("| Qanto Network Status           |");
            println!("+--------------------------------+");
            println!("| Epoch: 142 (mocked)            |");
            println!("| Network State: NOMINAL         |");
            println!("| Omega Threat Level: GUARDED    |");
            println!("| Avg. Block Time: 3.2s          |");
            println!("| Validators: 87                 |");
            println!("| SCS Mean: 0.78                 |");
            println!("+--------------------------------+");
        }
        "/ask" => {
            let topic = if let Some(pos) = args.iter().position(|&r| r == "about") {
                args.get(pos + 1)
            } else {
                args.first()
            };

            match topic.map(|s| s.to_lowercase()).as_deref() {
                Some("scs") => {
                    println!("\nYour Saga Credit Score (SCS) is your on-chain reputation (0.0 to 1.0). It's a weighted average of your trust score (from block analysis), Karma (long-term contributions), total stake, and environmental contributions (from PoCO). A higher SCS leads to greater block rewards and governance influence.");
                }
                Some("poco") => {
                    println!("\nProof-of-Carbon-Offset (PoCO) is Qanto's innovative mechanism for integrating real-world environmental action into the blockchain consensus. Validators include verifiable `CarbonOffsetCredential` data in their blocks. SAGA's Cognitive Engine analyzes these credentials, and miners who include valid, high-quality credentials receive a boost to their 'environmental_contribution' score. This improves their overall Saga Credit Score (SCS), leading to higher block rewards.");
                }
                Some("sense-think-act") => {
                    println!("\nThe Sense-Think-Act loop is the core of SAGA's autonomous operation:\n  [Sense]: SAGA continuously ingests on-chain data, security metrics, and economic indicators.\n  [Think]: It analyzes long-term trends, predicts future states (like congestion), and formulates strategic responses.\n  [Act]: Based on its conclusions, it autonomously issues edicts or proposes governance changes to optimize the network.");
                }
                _ => {
                    println!("Topic not found or not specified. Usage: /ask [topic]\nAvailable topics: scs, poco, sense-think-act");
                }
            }
        }
        "/analyze" => {
            if let Some(risk_type) = args.first() {
                run_analysis(risk_type);
            } else {
                println!("Usage: /analyze [risk_type]. Try 'centralization' or 'spam'.");
            }
        }
        "/simulate" => {
            if let Some(&"cycle") = args.first() {
                run_simulation();
            } else {
                println!("Usage: /simulate cycle");
            }
        }
        "/clear" => {
            // Use ANSI escape codes to clear the terminal screen
            print!("\x1B[2J\x1B[1;1H");
            io::stdout().flush().unwrap();
        }
        _ => {
            if command.starts_with('/') {
                println!("Command not found: {command}. Type '/help' for available commands.");
            } else {
                println!(
                    "Invalid input. All commands must start with a forward slash (e.g., /help)."
                );
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("--- SAGA Sentient Console v8.0 ---");
    println!("Initializing SAGA pallet...");
    println!("✓ SAGA is online. Type '/help' for available commands or 'exit' to quit.");

    loop {
        print!("\nSAGA >> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            error!("Failed to read line from stdin");
            continue;
        }

        let trimmed_input = input.trim();
        if trimmed_input.is_empty() {
            continue;
        }

        if trimmed_input == "exit" {
            println!("Deactivating SAGA. Goodbye.");
            break;
        }

        process_command(trimmed_input);
    }
}
