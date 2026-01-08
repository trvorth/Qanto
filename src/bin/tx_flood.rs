// tx_flood: High-concurrency transaction flooder for Qanto
// Enhanced for 10M+ TPS with lock-free UTXO pools and optimized parallel processing

use clap::{Parser, ValueEnum};
use crossbeam::queue::SegQueue;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::net::IpAddr;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::{interval, sleep};
use url::Url;

use qanto::transaction::{Input, Output, Transaction, TransactionConfig, TransactionError};
use qanto::types::{HomomorphicEncrypted, UTXO};
use qanto_core::qanto_native_crypto::qanto_hash;
use qanto_core::qanto_native_crypto::QantoPQPrivateKey;

#[derive(Clone, ValueEnum, Debug)]
enum Mode {
    Setup,
    Flood,
    Generate,
}

#[derive(Parser, Debug)]
#[command(
    name = "tx_flood",
    version = "0.3.0",
    about = "Flood the Qanto node with valid transactions and report metrics - Phase 3 Pre-Mine Strategy"
)]
struct Args {
    #[arg(long, value_enum, default_value_t = Mode::Flood)]
    mode: Mode,

    #[arg(long, alias = "target", default_value = "http://127.0.0.1:8080")]
    endpoint: String,

    #[arg(
        long,
        alias = "metrics_endpoint",
        help = "Optional Prometheus metrics endpoint to poll"
    )]
    metrics_endpoint: Option<String>,

    #[arg(long, default_value_t = 2000)]
    propagation_ms_threshold: u64,

    #[arg(long, default_value_t = 10_000_000)]
    tps: u64,

    #[arg(long, default_value_t = 60)]
    duration: u64,

    #[arg(long, default_value_t = 8000)]
    concurrency: usize,

    #[arg(long, default_value_t = 1000)]
    batch_size: usize,

    #[arg(
        long,
        alias = "genesis_address",
        help = "Prefunded genesis address to source UTXOs",
        required = true
    )]
    genesis_address: String,

    #[arg(long, default_value_t = 16)]
    utxo_pools: usize,

    #[arg(long, default_value_t = 4)]
    http_clients: usize,

    #[arg(
        long,
        alias = "allow_public_endpoint",
        default_value_t = false,
        help = "Allow running against non-private endpoints"
    )]
    allow_public_endpoint: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Assume known genesis UTXO instead of fetching via HTTP"
    )]
    assume_genesis_utxo: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Skip querying DAG info endpoints to avoid HTTP timeouts"
    )]
    skip_dag_info: bool,

    #[arg(
        long,
        default_value_t = 0.001,
        help = "Minimum /info difficulty seen within first 10s (0 disables)"
    )]
    min_difficulty_first10s: f64,

    #[arg(
        long,
        default_value = "armed_utxos.json",
        help = "File to save/load confirmed UTXOs"
    )]
    utxo_file: String,

    #[arg(
        long,
        help = "File to load genesis key from (for setup) or save to (for generate)"
    )]
    genesis_key_file: Option<String>,

    #[arg(
        long,
        default_value_t = 20000,
        help = "Number of addresses to pre-fund in setup mode"
    )]
    setup_count: usize,

    #[arg(long, help = "Explicit funding UTXO ID (avoids fetching all UTXOs)")]
    funding_utxo_id: Option<String>,

    #[arg(long, default_value_t = 0, help = "Explicit funding UTXO index")]
    funding_utxo_index: u32,

    #[arg(long, default_value_t = 0, help = "Explicit funding UTXO amount")]
    funding_utxo_amount: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct RejectedItem {
    id: String,
    error: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct BatchSubmitResponse {
    accepted: Vec<String>,
    rejected: Vec<RejectedItem>,
}

// Struct to store pre-armed UTXO data along with the private key to spend it
#[derive(Serialize, Deserialize, Clone)]
struct ArmedUtxo {
    utxo: UTXO,
    private_key_bytes: Vec<u8>,
}

fn make_output(addr: String, amount: u64, pk_bytes: &[u8]) -> Output {
    Output {
        address: addr,
        amount,
        homomorphic_encrypted: HomomorphicEncrypted::new(amount, pk_bytes),
    }
}

fn derive_address(pk_bytes: &[u8], index: u64) -> String {
    let mut seed = Vec::with_capacity(pk_bytes.len() + 8);
    seed.extend_from_slice(pk_bytes);
    seed.extend_from_slice(&index.to_be_bytes());
    hex::encode(qanto_hash(&seed).as_bytes())
}

async fn get_utxos_for_address(
    client: &Client,
    endpoint: &str,
    address: &str,
) -> Result<Vec<UTXO>, Box<dyn std::error::Error>> {
    let url = format!("{endpoint}/utxos/{address}");
    let utxos_map = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<HashMap<String, UTXO>>()
        .await?;
    Ok(utxos_map.into_values().collect())
}

fn infer_genesis_utxo(address: &str) -> UTXO {
    UTXO {
        address: address.to_string(),
        amount: 21_000_000_000u64 * qanto::transaction::SMALLEST_UNITS_PER_QAN,
        tx_id: "genesis_total_supply_tx".to_string(),
        output_index: 0,
        explorer_link: String::new(),
    }
}

fn enforce_endpoint_safety(endpoint: &str, allow_public_endpoint: bool) -> Result<(), String> {
    let url = match Url::parse(endpoint) {
        Ok(u) => u,
        Err(_) if !endpoint.contains("://") => Url::parse(&format!("http://{endpoint}"))
            .map_err(|e| format!("Invalid endpoint URL: {e}"))?,
        Err(e) => return Err(format!("Invalid endpoint URL: {e}")),
    };
    let host = url
        .host_str()
        .ok_or_else(|| "Endpoint URL must include a host".to_string())?;

    if allow_public_endpoint {
        return Ok(());
    }

    if host.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }

    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            if ip.is_loopback() || ip.is_private() || ip.is_link_local() {
                return Ok(());
            }
        }
        Ok(IpAddr::V6(ip)) => {
            if ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local() {
                return Ok(());
            }
        }
        Err(_) => {}
    }

    Err(format!(
        "Refusing to run against non-private endpoint host '{host}'. Pass --allow-public-endpoint to override."
    ))
}

async fn submit_tx_batch(
    client: &Client,
    endpoint: &str,
    txs: &[Transaction],
) -> Result<BatchSubmitResponse, Box<dyn std::error::Error>> {
    let url = format!("{endpoint}/submit-transactions");
    let resp = client.post(url).json(&txs).send().await?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .unwrap_or_else(|e| format!("<body read error: {e}>"));

    if !status.is_success() {
        let rejected = txs
            .iter()
            .map(|t| RejectedItem {
                id: t.id.clone(),
                error: format!("HTTP {status}: {body}"),
            })
            .collect();
        return Ok(BatchSubmitResponse {
            accepted: vec![],
            rejected,
        });
    }

    match serde_json::from_str::<BatchSubmitResponse>(&body) {
        Ok(br) => Ok(br),
        Err(e) => {
            let rejected = txs
                .iter()
                .map(|t| RejectedItem {
                    id: t.id.clone(),
                    error: format!("ParseError: {e}; raw_body={body}"),
                })
                .collect();
            Ok(BatchSubmitResponse {
                accepted: vec![],
                rejected,
            })
        }
    }
}

async fn build_funding_tx(
    funding: &UTXO,
    master_sk: &QantoPQPrivateKey,
    outputs_count: usize,
    start_index: usize,
    pk_bytes: &[u8],
    tx_timestamps: Arc<RwLock<HashMap<String, u64>>>,
) -> Result<Transaction, TransactionError> {
    let sender = funding.address.clone();

    // Evenly split the input value into many outputs; account for fee
    let fee = 1_000_000;
    let spendable = funding.amount.saturating_sub(fee);
    println!(
        "Debug: Building tx with fee: {}, spendable: {}",
        fee, spendable
    );

    let effective_outputs = outputs_count.max(1).min(spendable as usize);
    let base = if effective_outputs > 0 {
        spendable / effective_outputs as u64
    } else {
        0
    };
    let remainder = if effective_outputs > 0 {
        spendable % effective_outputs as u64
    } else {
        0
    };

    let mut outputs = Vec::with_capacity(effective_outputs);
    for i in 0..effective_outputs {
        // Derive unique addresses for each output to prevent collisions
        let addr = derive_address(pk_bytes, (start_index + i) as u64);
        let extra = if (i as u64) < remainder { 1 } else { 0 };
        let amt = base + extra;
        outputs.push(make_output(addr, amt, pk_bytes));
    }

    let inputs = vec![Input {
        tx_id: funding.tx_id.clone(),
        output_index: funding.output_index,
    }];
    let cfg = TransactionConfig {
        sender,
        receiver: "0".repeat(64),
        amount: spendable,
        fee,
        gas_limit: 50_000,
        gas_price: 1,
        priority_fee: 0,
        inputs,
        outputs,
        metadata: None,
        tx_timestamps,
    };
    Transaction::new(cfg, master_sk).await
}

struct EnhancedTokenBucket {
    tokens: AtomicU64,
    burst_capacity: u64,
    refill_rate_per_tick: u64,
}

impl EnhancedTokenBucket {
    fn new(refill_rate_per_sec: u64) -> Self {
        let burst_capacity = refill_rate_per_sec * 2;
        let per_tick = (refill_rate_per_sec / 100).max(1);
        Self {
            tokens: AtomicU64::new(burst_capacity),
            burst_capacity,
            refill_rate_per_tick: per_tick,
        }
    }

    fn try_consume(&self, count: u64) -> bool {
        self.tokens
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                if current >= count {
                    Some(current - count)
                } else {
                    None
                }
            })
            .is_ok()
    }

    fn refill(&self) {
        let current = self.tokens.load(Ordering::Relaxed);
        if current < self.burst_capacity {
            let new_tokens = (current + self.refill_rate_per_tick).min(self.burst_capacity);
            self.tokens.store(new_tokens, Ordering::Relaxed);
        }
    }
}

async fn run_generate_mode(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Generate Key Mode ===");
    let mut rng = rand::rngs::OsRng;
    let sk = QantoPQPrivateKey::generate(&mut rng);
    let pk = sk.public_key();
    let pk_bytes = pk.as_bytes();
    let addr = derive_address(pk_bytes, 0);

    println!("Generated Qanto PQ Keypair");
    println!("Address: {}", addr);

    let sk_bytes = sk.as_bytes();
    let hex_sk = hex::encode(sk_bytes);

    if let Some(path) = args.genesis_key_file {
        std::fs::write(&path, &hex_sk)?;
        println!("Saved private key to {}", path);
    } else {
        println!("Private Key (hex): {}", hex_sk);
    }

    Ok(())
}

// --- SETUP MODE ---

async fn run_setup_mode(args: Args, client: Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Setup Mode ===");
    println!("Loading genesis key...");

    let master_sk = if let Some(path) = &args.genesis_key_file {
        let hex_sk = std::fs::read_to_string(path)?;
        let bytes = hex::decode(hex_sk.trim())?;
        QantoPQPrivateKey::from_bytes(&bytes).map_err(|_| "Invalid private key bytes")?
    } else {
        println!("No genesis key file provided! Generating a random one (likely won't match genesis UTXO)...");
        let mut rng = rand::rngs::OsRng;
        QantoPQPrivateKey::generate(&mut rng)
    };

    let pk_bytes = master_sk.public_key().as_bytes().to_vec();
    let derived_genesis_addr = derive_address(&pk_bytes, 0);
    println!(
        "Derived genesis address (index 0) from key: {}",
        derived_genesis_addr
    );

    let funding_utxo = if let Some(tx_id) = &args.funding_utxo_id {
        println!("Using explicit funding UTXO: {}", tx_id);
        UTXO {
            address: args.genesis_address.clone(),
            amount: args.funding_utxo_amount,
            tx_id: tx_id.clone(),
            output_index: args.funding_utxo_index,
            explorer_link: String::new(),
        }
    } else {
        println!("Fetching genesis UTXOs for {}...", args.genesis_address);
        let genesis_utxos = if args.assume_genesis_utxo {
            vec![infer_genesis_utxo(&args.genesis_address)]
        } else {
            let list =
                get_utxos_for_address(&client, &args.endpoint, &args.genesis_address).await?;
            if list.is_empty() {
                return Err(format!(
                    "No UTXOs found for genesis address {} on {}",
                    args.genesis_address, args.endpoint
                )
                .into());
            }
            list
        };

        if genesis_utxos.is_empty() {
            return Err("No genesis UTXOs found".into());
        }

        // Pick a random UTXO to avoid contention or stuck spent UTXOs
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        genesis_utxos
            .choose(&mut rng)
            .ok_or("No genesis UTXOs found")?
            .clone()
    };

    println!(
        "Using funding UTXO: {} (amount: {})",
        funding_utxo.tx_id, funding_utxo.amount
    );

    let tx_timestamps = Arc::new(RwLock::new(HashMap::new()));

    // We want to create args.setup_count UTXOs
    // We can do this in batches. One tx can have ~200 outputs.
    // So we need setup_count / 200 transactions.
    // But we need to chain them if we only have 1 input.
    // Actually, we can just split the genesis UTXO into N outputs in a tree or just 1-to-many.
    // Qanto max outputs per tx is not strictly limited but let's be safe with 100.

    let outputs_per_tx = 100;
    let num_txs = args.setup_count.div_ceil(outputs_per_tx);

    println!(
        "Target: {} armed UTXOs. Will broadcast ~{} split transactions.",
        args.setup_count, num_txs
    );

    // First, we might need to split the genesis UTXO into `num_txs` intermediate UTXOs,
    // and then each of those splits into `outputs_per_tx` final UTXOs.
    // To keep it simple:
    // 1. Genesis -> [Intermediate 1, Intermediate 2, ... Intermediate M]
    // 2. Intermediate 1 -> [Final 1..100]
    // 3. Intermediate 2 -> [Final 101..200]
    // ...

    let intermediate_count = num_txs;
    println!(
        "Phase 1: Splitting Genesis into {} intermediate UTXOs...",
        intermediate_count
    );

    let split_tx = build_funding_tx(
        &funding_utxo,
        &master_sk,
        intermediate_count,
        0,
        &pk_bytes,
        tx_timestamps.clone(),
    )
    .await?;

    let resp = submit_tx_batch(&client, &args.endpoint, std::slice::from_ref(&split_tx)).await?;
    if !resp.accepted.contains(&split_tx.id) {
        return Err(format!("Phase 1 split transaction rejected: {:?}", resp.rejected).into());
    }

    println!(
        "Phase 1 TX submitted: {}. Waiting for confirmation...",
        split_tx.id
    );

    // Wait for confirmation of intermediate UTXOs
    let mut intermediate_utxos = Vec::new();
    let start_wait = Instant::now();
    loop {
        if start_wait.elapsed() > Duration::from_secs(60) {
            return Err("Timed out waiting for Phase 1 confirmation".into());
        }
        sleep(Duration::from_secs(1)).await;

        // Check if the first output address has UTXOs.
        // Note: build_funding_tx derives addresses based on index.
        // We need to check all intermediate addresses?
        // Or better, just wait until we see UTXOs for the derived addresses.

        let mut confirmed_count = 0;

        // Sample a few addresses to check progress
        for i in 0..intermediate_count.min(5) {
            let addr = derive_address(&pk_bytes, i as u64);
            if let Ok(utxos) = get_utxos_for_address(&client, &args.endpoint, &addr).await {
                if !utxos.is_empty() {
                    confirmed_count += 1;
                }
            }
        }

        if confirmed_count == intermediate_count.min(5) {
            // Assume all are ready if sample is ready, but let's be thorough for Phase 2
            println!("Phase 1 confirmed (sample). Proceeding to Phase 2...");

            // Reconstruct UTXOs from the tx outputs to avoid polling 100s of addresses
            for (i, output) in split_tx.outputs.iter().enumerate() {
                intermediate_utxos.push(UTXO {
                    tx_id: split_tx.id.clone(),
                    output_index: i as u32,
                    address: output.address.clone(),
                    amount: output.amount,
                    explorer_link: String::new(),
                });
            }
            break;
        }
    }

    println!(
        "Phase 2: Fanning out to {} final UTXOs...",
        args.setup_count
    );

    let mut armed_utxos = Vec::new();
    let mut batch_txs = Vec::new();
    let mut final_addresses = Vec::new();

    for (i, input_utxo) in intermediate_utxos.iter().enumerate() {
        let start_addr_idx = i * outputs_per_tx;
        let final_tx = build_funding_tx(
            input_utxo,
            &master_sk,
            outputs_per_tx,
            start_addr_idx,
            &pk_bytes,
            tx_timestamps.clone(),
        )
        .await?;

        // Save the expected UTXOs
        for (out_idx, output) in final_tx.outputs.iter().enumerate() {
            armed_utxos.push(ArmedUtxo {
                utxo: UTXO {
                    tx_id: final_tx.id.clone(),
                    output_index: out_idx as u32,
                    address: output.address.clone(),
                    amount: output.amount,
                    explorer_link: String::new(),
                },
                private_key_bytes: master_sk.as_bytes().to_vec(), // Reusing master key for simplicity as all addrs derived from it
            });
            final_addresses.push(output.address.clone());
        }

        batch_txs.push(final_tx);
    }

    // Submit in chunks
    let chunk_size = 20;
    for chunk in batch_txs.chunks(chunk_size) {
        let resp = submit_tx_batch(&client, &args.endpoint, chunk).await?;
        println!(
            "Submitted batch of {} txs. Accepted: {}, Rejected: {}",
            chunk.len(),
            resp.accepted.len(),
            resp.rejected.len()
        );
        if !resp.rejected.is_empty() {
            println!("Warning: some setup txs rejected: {:?}", resp.rejected[0]);
        }
        // Sleep longer to let mempool drain
        sleep(Duration::from_millis(200)).await;
    }

    println!(
        "Waiting for final confirmation of {} UTXOs...",
        armed_utxos.len()
    );
    // Poll a random sample of final addresses to confirm they exist on-chain
    let sample_size = 20;
    let start_wait = Instant::now();
    loop {
        if start_wait.elapsed() > Duration::from_secs(120) {
            println!("Warning: Timeout waiting for full confirmation. Saving what we have...");
            break;
        }

        let mut confirmed = 0;
        let mut rng = rand::thread_rng();
        use rand::seq::SliceRandom;

        let sample: Vec<_> = final_addresses
            .choose_multiple(&mut rng, sample_size)
            .collect();

        for addr in sample {
            if let Ok(utxos) = get_utxos_for_address(&client, &args.endpoint, addr).await {
                if !utxos.is_empty() {
                    confirmed += 1;
                }
            }
        }

        if confirmed >= (sample_size * 8 / 10) {
            // 80% threshold for "good enough"
            println!(
                "Sample confirmed ({}/{})! Saving state.",
                confirmed, sample_size
            );
            break;
        }

        println!(
            "Waiting... {}/{} sample addresses confirmed.",
            confirmed, sample_size
        );
        sleep(Duration::from_secs(2)).await;
    }

    let file = File::create(&args.utxo_file)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, &armed_utxos)?;

    println!(
        "Saved {} armed UTXOs to {}",
        armed_utxos.len(),
        args.utxo_file
    );
    Ok(())
}

// --- FLOOD MODE ---

async fn run_flood_mode(
    args: Args,
    clients: Arc<Vec<Client>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Flood Mode ===");
    println!("Loading UTXOs from {}...", args.utxo_file);

    let file = File::open(&args.utxo_file)?;
    let reader = BufReader::new(file);
    let armed_utxos: Vec<ArmedUtxo> = serde_json::from_reader(reader)?;

    if armed_utxos.is_empty() {
        return Err("No UTXOs in file".into());
    }

    println!("Loaded {} UTXOs. Initializing flood...", armed_utxos.len());

    // Distribute UTXOs to concurrent workers
    // We use a SegQueue for workers to pick up UTXOs.
    // BUT: To support chaining, when a worker spends a UTXO, it puts the *new* UTXO back into the queue.
    // The "ArmedUtxo" struct needs to hold the key to spend it.

    let utxo_queue = Arc::new(SegQueue::new());
    for u in armed_utxos {
        utxo_queue.push(u);
    }

    let token_bucket = Arc::new(EnhancedTokenBucket::new(args.tps));
    let refill = {
        let token_bucket = token_bucket.clone();
        tokio::spawn(async move {
            let mut tick = interval(Duration::from_millis(10));
            loop {
                tick.tick().await;
                token_bucket.refill();
            }
        })
    };

    let submitted = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let stop_at = Instant::now() + Duration::from_secs(args.duration);
    let tx_timestamps = Arc::new(RwLock::new(HashMap::new()));

    let mut set = JoinSet::new();

    // Monitor loop
    {
        let submitted = submitted.clone();
        let failed = failed.clone();
        tokio::spawn(async move {
            let mut last_submitted = 0;
            let mut start = Instant::now();
            loop {
                sleep(Duration::from_secs(1)).await;
                if Instant::now() > stop_at {
                    break;
                }

                let current = submitted.load(Ordering::Relaxed);
                let fail = failed.load(Ordering::Relaxed);
                let delta = current - last_submitted;
                let tps = delta as f64 / start.elapsed().as_secs_f64();

                println!("TPS: {:.0} | Total: {} | Failed: {}", tps, current, fail);
                last_submitted = current;
                start = Instant::now();
            }
        });
    }

    for i in 0..args.concurrency {
        let client = clients[i % clients.len()].clone();
        let endpoint = args.endpoint.clone();
        let queue = utxo_queue.clone();
        let bucket = token_bucket.clone();
        let submitted = submitted.clone();
        let failed = failed.clone();
        let tx_timestamps = tx_timestamps.clone();

        set.spawn(async move {
            // Each worker grabs a batch of UTXOs, spends them, and pushes new ones back
            let batch_size = args.batch_size;

            while Instant::now() < stop_at {
                if !bucket.try_consume(batch_size as u64) {
                    sleep(Duration::from_millis(1)).await;
                    continue;
                }

                let mut batch_utxos = Vec::with_capacity(batch_size);
                for _ in 0..batch_size {
                    if let Some(u) = queue.pop() {
                        batch_utxos.push(u);
                    } else {
                        break;
                    }
                }

                if batch_utxos.is_empty() {
                    sleep(Duration::from_millis(10)).await;
                    continue;
                }

                // Build transactions
                let mut txs = Vec::with_capacity(batch_utxos.len());
                let mut next_utxos = Vec::with_capacity(batch_utxos.len());
                let mut source_utxos = Vec::with_capacity(batch_utxos.len());

                for armed in &batch_utxos {
                    let master_sk =
                        QantoPQPrivateKey::from_bytes(&armed.private_key_bytes).unwrap();
                    let pk = master_sk.public_key();
                    let pk_bytes = pk.as_bytes();

                    let fee = 10000; // minimal fee
                    if armed.utxo.amount <= fee {
                        // Dust, discard
                        continue;
                    }
                    let amount = armed.utxo.amount - fee;

                    // Send back to self to keep the loop going
                    let receiver = armed.utxo.address.clone();
                    let output = make_output(receiver.clone(), amount, pk_bytes);

                    let input = Input {
                        tx_id: armed.utxo.tx_id.clone(),
                        output_index: armed.utxo.output_index,
                    };

                    let cfg = TransactionConfig {
                        sender: armed.utxo.address.clone(),
                        receiver,
                        amount,
                        fee,
                        gas_limit: 50_000,
                        gas_price: 1,
                        priority_fee: 0,
                        inputs: vec![input],
                        outputs: vec![output.clone()],
                        metadata: None,
                        tx_timestamps: tx_timestamps.clone(),
                    };

                    if let Ok(tx) = Transaction::new(cfg, &master_sk).await {
                        // Predict the next UTXO
                        let next_utxo = ArmedUtxo {
                            utxo: UTXO {
                                tx_id: tx.id.clone(),
                                output_index: 0, // We only have 1 output
                                address: output.address,
                                amount: output.amount,
                                explorer_link: String::new(),
                            },
                            private_key_bytes: armed.private_key_bytes.clone(),
                        };

                        txs.push(tx);
                        next_utxos.push(next_utxo);
                        source_utxos.push(armed.clone());
                    }
                }

                if txs.is_empty() {
                    continue;
                }

                // Submit batch
                match submit_tx_batch(&client, &endpoint, &txs).await {
                    Ok(resp) => {
                        let accepted_count = resp.accepted.len();
                        submitted.fetch_add(accepted_count as u64, Ordering::Relaxed);
                        failed.fetch_add(resp.rejected.len() as u64, Ordering::Relaxed);

                        use std::collections::HashSet;
                        let accepted_set: HashSet<&String> = resp.accepted.iter().collect();

                        // Print first rejection error for debugging
                        if !resp.rejected.is_empty() {
                            static FIRST_ERROR_LOGGED: std::sync::atomic::AtomicBool =
                                std::sync::atomic::AtomicBool::new(false);
                            if !FIRST_ERROR_LOGGED.swap(true, Ordering::Relaxed) {
                                eprintln!("DEBUG: First rejection error: {:?}", resp.rejected[0]);
                            }
                        }

                        for (i, tx) in txs.iter().enumerate() {
                            if accepted_set.contains(&tx.id) {
                                // Success: Push next UTXO
                                if i < next_utxos.len() {
                                    queue.push(next_utxos[i].clone());
                                }
                            } else {
                                // Rejected: Retry original UTXO
                                if i < source_utxos.len() {
                                    queue.push(source_utxos[i].clone());
                                }
                            }
                        }
                    }
                    Err(_) => {
                        failed.fetch_add(txs.len() as u64, Ordering::Relaxed);
                        // Network error: Retry all sources
                        for u in source_utxos {
                            queue.push(u);
                        }
                    }
                }
            }
        });
    }

    while set.join_next().await.is_some() {}
    drop(refill);

    let total = submitted.load(Ordering::Relaxed);
    println!("=== Flood Complete ===");
    println!("Total Submitted: {}", total);
    println!("Total Failed: {}", failed.load(Ordering::Relaxed));
    println!("Duration: {}s", args.duration);
    println!("Avg TPS: {:.2}", total as f64 / args.duration as f64);

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    enforce_endpoint_safety(&args.endpoint, args.allow_public_endpoint)
        .map_err(std::io::Error::other)?;

    // Create clients
    let mut clients = Vec::with_capacity(args.http_clients);
    for _ in 0..args.http_clients {
        clients.push(
            Client::builder()
                .pool_max_idle_per_host(100)
                .pool_idle_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(30))
                .build()?,
        );
    }
    let clients_arc = Arc::new(clients.clone());

    match args.mode {
        Mode::Setup => run_setup_mode(args, clients[0].clone()).await,
        Mode::Flood => run_flood_mode(args, clients_arc).await,
        Mode::Generate => run_generate_mode(args).await,
    }
}
