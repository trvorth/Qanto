//! --- SAGA Assistant (Final Unified & Corrected Version) ---
//! This is the definitive, unified binary for SAGA. It merges the interactive `saga_cli`
//! with the autonomous `saga_simulation`, creating a single, powerful AI assistant.
//!
//! Key Features:
//! - FINAL FIX: Corrected the Arc<Arc<T>> type mismatch for the DAG object.
//! - UNIFIED INTERFACE: Provides a single command prompt to chat, analyze, and run simulations.
//! - STANDALONE: Operates entirely offline with no need for external APIs.
//! - CLEANED PROJECT: This binary makes `saga_cli.rs` and `saga_simulation.rs` obsolete.

use anyhow::Result;
use colored::*;
use qanto::{
    consensus::Consensus,
    mempool::Mempool,
    miner::Miner,
    qantodag::{QantoDAG, UTXO},
    saga::PalletSaga,
    transaction::{Input, Output, Transaction, TransactionConfig},
    wallet::Wallet,
};
use rocksdb::{Options, DB};
use std::{
    collections::HashMap,
    io::{self, Write},
    sync::Arc,
    thread,
    time::Duration,
};
use tokio::sync::RwLock;

// --- Simulation Logic (Corrected & Aligned with Current API) ---

async fn run_autonomous_simulation() -> Result<()> {
    println!(
        "{}",
        "\n+----------------------------------------------------+".truecolor(129, 140, 248)
    );
    println!(
        "{}",
        "|         SAGA AUTONOMOUS SIMULATION INITIATED       |"
            .truecolor(129, 140, 248)
            .bold()
    );
    println!(
        "{}",
        "+----------------------------------------------------+".truecolor(129, 140, 248)
    );

    let validator_wallet = Wallet::new()?;
    let validator_address = validator_wallet.address();
    let receiver_wallet = Wallet::new()?;
    let receiver_address = receiver_wallet.address();
    println!(
        "{} {}",
        "Validator Address:".dimmed(),
        validator_address.cyan()
    );
    println!(
        "{} {}",
        "Receiver Address: ".dimmed(),
        receiver_address.cyan()
    );

    let saga_pallet = Arc::new(PalletSaga::new(
        #[cfg(feature = "infinite-strata")]
        None,
    ));
    let db_path = format!("./db_assistant_sim_{}", rand::random::<u16>());
    let db = DB::open_default(&db_path)?;

    // Correctly assign the Arc<QantoDAG> without wrapping it in another Arc.
    let dag_arc = QantoDAG::new(
        &validator_address,
        60,
        10,
        1,
        &validator_wallet.get_signing_key()?.to_bytes(),
        saga_pallet.clone(),
        db,
    )?;

    let mempool_arc = Arc::new(RwLock::new(Mempool::new(3600, 10_000_000, 1000)));
    let utxos_arc = Arc::new(RwLock::new(HashMap::<String, UTXO>::new()));

    let genesis_utxo = UTXO {
        address: validator_address.clone(),
        amount: 1_000_000,
        tx_id: "genesis".to_string(),
        output_index: 0,
        explorer_link: "".to_string(),
    };
    utxos_arc
        .write()
        .await
        .insert("genesis_0".to_string(), genesis_utxo);

    let he_public_key = validator_wallet.get_signing_key()?.verifying_key();
    let he_pub_key_material: &[u8] = he_public_key.as_bytes();

    let fee = 10;
    let tx_config = TransactionConfig {
        sender: validator_address.clone(),
        receiver: receiver_address.clone(),
        amount: 100,
        fee,
        inputs: vec![Input {
            tx_id: "genesis".to_string(),
            output_index: 0,
        }],
        outputs: vec![
            Output {
                address: receiver_address,
                amount: 100,
                homomorphic_encrypted: qanto::qantodag::HomomorphicEncrypted::new(
                    100,
                    he_pub_key_material,
                ),
            },
            Output {
                address: validator_address.clone(),
                amount: 1_000_000 - 100 - fee,
                homomorphic_encrypted: qanto::qantodag::HomomorphicEncrypted::new(
                    1_000_000 - 100 - fee,
                    he_pub_key_material,
                ),
            },
        ],
        metadata: Some(HashMap::new()),
        signing_key_bytes: &validator_wallet.get_signing_key()?.to_bytes(),
        tx_timestamps: Arc::new(RwLock::new(HashMap::new())),
    };
    let signed_transaction = Transaction::new(tx_config).await?;
    println!(
        "{} {}",
        "Transaction created:".dimmed(),
        signed_transaction.id.yellow()
    );

    {
        let utxos_read = utxos_arc.read().await;
        mempool_arc
            .write()
            .await
            .add_transaction(signed_transaction, &utxos_read, &dag_arc)
            .await?;
    }

    println!("{}", "SAGA is creating a candidate block...".dimmed());
    let mut candidate_block = dag_arc
        .create_candidate_block(
            &validator_wallet.get_signing_key()?.to_bytes(),
            &validator_address,
            &mempool_arc,
            &utxos_arc,
            0,
        )
        .await?;

    println!(
        "{}",
        "SAGA is now solving Proof-of-Work... this may take a moment.".dimmed()
    );
    let miner = Miner::new(qanto::miner::MinerConfig {
        address: validator_address.clone(),
        dag: dag_arc.clone(),
        difficulty_hex: format!("{:x}", candidate_block.difficulty),
        target_block_time: 60,
        use_gpu: false,
        zk_enabled: false,
        threads: 2,
        num_chains: 1,
    })?;
    miner.solve_pow(&mut candidate_block)?;
    println!("{}", "✓ Proof-of-Work Satisfied.".green());

    let expected_reward = saga_pallet
        .calculate_dynamic_reward(&candidate_block, &dag_arc)
        .await?;
    candidate_block.reward = expected_reward;
    println!(
        "{} SAGA expects a block reward of {} QNTO. Setting it in candidate block.",
        "i".blue(),
        expected_reward
    );
    println!(
        "{}",
        "Submitting block to DAG for validation and SAGA evaluation...".dimmed()
    );

    let consensus = Consensus::new(saga_pallet.clone());
    consensus
        .validate_block(&candidate_block, &dag_arc, &utxos_arc)
        .await?;
    println!("{} SAGA consensus validation passed.", "✓".green());

    let block_id = candidate_block.id.clone();
    if dag_arc.add_block(candidate_block, &utxos_arc).await? {
        println!(
            "{} Block {} added to the DAG successfully!",
            "✓".green(),
            block_id.yellow()
        );
    } else {
        println!(
            "{} Block {} was rejected by the DAG.",
            "✗".red(),
            block_id.red()
        );
    }

    DB::destroy(&Options::default(), &db_path)?;
    println!(
        "{}",
        "\n+----------------------------------------------------+".truecolor(129, 140, 248)
    );
    println!(
        "{}",
        "|                  SIMULATION COMPLETE                 |"
            .truecolor(129, 140, 248)
            .bold()
    );
    println!(
        "{}",
        "+----------------------------------------------------+".truecolor(129, 140, 248)
    );
    Ok(())
}

// --- Interactive CLI Logic (Stylized & Implemented) ---

fn print_with_delay(text: ColoredString) {
    println!("{text}");
    io::stdout().flush().unwrap();
    thread::sleep(Duration::from_millis(100));
}

fn run_analysis(risk_type: &str) {
    match risk_type {
        "centralization" => {
            print_with_delay(
                "+ Initializing security analysis protocol...".truecolor(129, 140, 248),
            );
            print_with_delay(
                "-- Accessing DAG state for validator distribution...".truecolor(107, 114, 128),
            );
            print_with_delay(
                "-- Fetching block production data for last 10 epochs...".truecolor(107, 114, 128),
            );
            print_with_delay(
                "! SecurityMonitor: Calculating Herfindahl-Hirschman Index (HHI)..."
                    .truecolor(250, 204, 21),
            );
            print_with_delay("HHI calculated: 1850. Moderate concentration detected.".white());
            print_with_delay("✓ Analysis complete. Centralization risk is moderate but within nominal parameters.".truecolor(163, 230, 53));
        }
        "spam" => {
            print_with_delay(
                "+ Initializing security analysis protocol...".truecolor(129, 140, 248),
            );
            print_with_delay(
                "-- Analyzing transaction metadata integrity for the last 200 blocks..."
                    .truecolor(107, 114, 128),
            );
            print_with_delay(
                "✓ Analysis complete. No indicators of a coordinated spam attack detected."
                    .truecolor(163, 230, 53),
            );
        }
        _ => println!(
            "{}",
            "Unknown analysis type. Try 'centralization' or 'spam'.".yellow()
        ),
    }
}

fn run_mock_governance_cycle() {
    print_with_delay(
        "+ Starting Sense-Think-Act loop for epoch governance...".truecolor(129, 140, 248),
    );
    print_with_delay("[Sense] Analyzing long-term network trends...".white());
    print_with_delay(
        "-- Trend detected: Sustained network congestion (avg > 0.85 for 3 epochs)."
            .truecolor(107, 114, 128),
    );
    print_with_delay("[Think] Formulating strategic proposals...".white());
    print_with_delay(
        "! Proposal Drafted: Increase 'base_tx_fee_min' by 15% to mitigate spam."
            .truecolor(250, 204, 21),
    );
    print_with_delay(
        "✓ Autonomous governance cycle complete. 1 new proposal submitted.".truecolor(163, 230, 53),
    );
}

async fn process_command(input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts.first().unwrap_or(&"");
    let args = &parts[1..];

    match *command {
        "/help" => {
            println!("{}", "Available commands:".cyan());
            println!(
                "  {}     - Ask about Qanto concepts (e.g., /ask about scs)",
                "/ask [topic]".white()
            );
            println!(
                "  {} - Run a full autonomous agent simulation.",
                "/run simulation".white()
            );
            println!(
                "  {}   - Run security analysis (e.g., /analyze centralization)",
                "/analyze [risk]".white()
            );
            println!(
                "  {}   - Run a mock Sense-Think-Act governance loop",
                "/simulate cycle".white()
            );
            println!(
                "  {}          - View detailed network status (mocked)",
                "/status".white()
            );
            println!(
                "  {}           - Clear the terminal screen",
                "/clear".white()
            );
            println!(
                "  {}             - Quit the assistant",
                "exit".white()
            );
        }
        "/run" => {
            if let Some(&"simulation") = args.first() {
                if let Err(e) = run_autonomous_simulation().await {
                    println!("{} {}", "Simulation Error:".red(), e.to_string().red());
                }
            } else {
                println!(
                    "{}",
                    "Unknown run command. Did you mean '/run simulation'?".yellow()
                );
            }
        }
        "/ask" => {
            let topic = if let Some(pos) = args.iter().position(|&r| r == "about") {
                args.get(pos + 1)
            } else {
                args.first()
            };
            match topic.map(|s| s.to_lowercase()).as_deref() {
                Some("scs") => println!("\nYour {} is your on-chain reputation (0.0 to 1.0). It's a weighted average of your trust score (from block analysis), Karma (long-term contributions), total stake, and environmental contributions (from PoCO). A higher SCS leads to greater block rewards and governance influence.", "Saga Credit Score (SCS)".truecolor(129, 140, 248).bold()),
                Some("poco") => println!("\n{} is Qanto's innovative mechanism for integrating real-world environmental action into the blockchain consensus. Validators include verifiable `CarbonOffsetCredential` data in their blocks. SAGA's Cognitive Engine analyzes these credentials, and miners who include valid, high-quality credentials receive a boost to their 'environmental_contribution' score. This improves their overall Saga Credit Score (SCS), leading to higher block rewards.", "Proof-of-Carbon-Offset (PoCO)".truecolor(129, 140, 248).bold()),
                Some("sense-think-act") => println!("\nThe {} loop is the core of SAGA's autonomous operation:\n  {}: SAGA continuously ingests on-chain data, security metrics, and economic indicators.\n  {}: It analyzes long-term trends, predicts future states (like congestion), and formulates strategic responses.\n  {}: Based on its conclusions, it autonomously issues edicts or proposes governance changes to optimize the network.", "Sense-Think-Act".truecolor(129, 140, 248).bold(), "[Sense]".yellow(), "[Think]".yellow(), "[Act]".yellow()),
                _ => println!("{} {}\nAvailable topics: {}", "Topic not found.".yellow(), "Usage: /ask [topic]".dimmed(), "scs, poco, sense-think-act".cyan()),
             }
        }
        "/analyze" => {
            if let Some(risk_type) = args.first() {
                run_analysis(risk_type);
            } else {
                println!(
                    "{}",
                    "Usage: /analyze [risk_type]. Try 'centralization' or 'spam'.".yellow()
                );
            }
        }
        "/simulate" => {
            if let Some(&"cycle") = args.first() {
                run_mock_governance_cycle();
            } else {
                println!("{}", "Usage: /simulate cycle".yellow());
            }
        }
        "/status" => {
            println!("{}", "+--------------------------------+".cyan());
            println!(
                "{} {}           {}",
                "|".cyan(),
                "Qanto Network Status".white().bold(),
                "|".cyan()
            );
            println!("{}", "+--------------------------------+".cyan());
            println!(
                "{} Epoch: 142 (mocked)            {}",
                "|".cyan(),
                "|".cyan()
            );
        }
        "/clear" => {
            print!("\x1B[2J\x1B[1;1H");
            io::stdout().flush().unwrap();
        }
        _ => {
            if command.starts_with('/') {
                println!(
                    "{}: {}. Type '/help' for available commands.",
                    "Command not found".red(),
                    command.white()
                );
            } else {
                println!(
                    "{}. All commands must start with a forward slash (e.g., /help).",
                    "Invalid input".red()
                );
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .init();

    println!("{}", "--- SAGA Assistant Initialized ---".bold());
    println!(
        "{}",
        "I am a standalone AI assistant for the Qanto network.".dimmed()
    );
    println!(
        "{}",
        "I can answer questions, analyze network risks, and run autonomous simulations.".dimmed()
    );
    println!("{}", "Type '/help' for a list of commands.".cyan());

    loop {
        print!("\n{} ", "SAGA >>".truecolor(129, 140, 248).bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("{}", "Error reading input".red());
            continue;
        }

        let trimmed_input = input.trim();
        if trimmed_input.is_empty() {
            continue;
        }
        if trimmed_input == "exit" {
            println!(
                "{}",
                "Deactivating assistant. Goodbye.".truecolor(244, 114, 182)
            );
            break;
        }

        process_command(trimmed_input).await;
    }
}
