// src/bin/saga_cli.rs

use qanto::omega;
use qanto::saga::{NetworkState, PalletSaga};
use std::io::{self, Write};
use tracing::error;

/// A mock representation of the main QantoDAG for CLI purposes.
/// In a real integration, this would be the actual running node.
struct MockQantoNode {
    saga: PalletSaga,
    network_state: NetworkState,
    omega_threat: omega::identity::ThreatLevel,
}

impl Default for MockQantoNode {
    fn default() -> Self {
        Self {
            saga: PalletSaga::new(),
            network_state: NetworkState::Nominal,
            omega_threat: omega::identity::ThreatLevel::Nominal,
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing for logging, but keep it minimal for a clean CLI
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("--- SAGA Sentient Console v7.0 ---");
    println!("Initializing SAGA pallet...");

    let node = MockQantoNode::default();
    let saga_guidance = node.saga.guidance_system.clone();

    println!("✓ SAGA is online. Type 'help' for available commands or 'exit' to quit.");

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

        // Use the SagaGuidanceSystem to process the query
        match saga_guidance
            .get_guidance_response(
                trimmed_input,
                node.network_state,
                node.omega_threat,
                None, // No proactive insights in this mock CLI
            )
            .await
        {
            Ok(response) => {
                // Clean up the response for better terminal readability
                let clean_response = response
                    .replace("**", "") // Remove markdown bold
                    .replace("*", ""); // Remove markdown italics
                println!("{}", clean_response);
            }
            Err(e) => {
                println!("SAGA Error: {}", e);
            }
        }
    }
}
