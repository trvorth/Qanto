// tx_flood: High-concurrency transaction flooder for Qanto
// Enhanced for 10M+ TPS with lock-free UTXO pools and optimized parallel processing

use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use clap::Parser;
use crossbeam::queue::SegQueue;
use rayon::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::{interval, sleep};

use my_blockchain::qanto_hash;
use qanto::transaction::{
    calculate_dynamic_fee, Input, Output, Transaction, TransactionConfig, TransactionError,
};
use qanto::types::{HomomorphicEncrypted, UTXO};
use qanto_core::qanto_native_crypto::QantoPQPrivateKey;

#[derive(Parser, Debug)]
#[command(
    name = "tx_flood",
    version = "0.2.0",
    about = "Flood the Qanto node with valid transactions and report metrics - Enhanced for 10M+ TPS"
)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    endpoint: String,
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
    #[arg(long, help = "Path to wallet key file to load private key from")]
    wallet: Option<std::path::PathBuf>,
    #[arg(long, help = "Password for wallet file")]
    password: Option<String>,
    #[arg(long, default_value_t = 1234, help = "Global Chain ID")]
    chain_id: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct DagInfo {
    block_count: usize,
    tip_count: usize,
    current_difficulty: f64,
    target_block_time: u64,
    validator_count: usize,
    num_chains: u32,
    latest_block_timestamp: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct StatsInfo {
    block_count: usize,
    mempool_size: usize,
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

fn make_output(addr: String, amount: u128, pk_bytes: &[u8]) -> Output {
    Output {
        address: addr,
        amount,
        homomorphic_encrypted: HomomorphicEncrypted::new(amount, pk_bytes),
    }
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

fn choose_best_funding_utxo(mut utxos: Vec<UTXO>) -> Option<UTXO> {
    utxos.sort_by_key(|u| std::cmp::Reverse(u.amount));
    utxos.into_iter().next()
}

fn infer_genesis_utxo(address: &str) -> UTXO {
    UTXO {
        address: address.to_string(),
        amount: 21_000_000_000u128 * qanto::transaction::SMALLEST_UNITS_PER_QAN,
        tx_id: "genesis_total_supply_tx".to_string(),
        output_index: 0,
        explorer_link: String::new(),
    }
}

async fn get_dag_info(
    client: &Client,
    endpoint: &str,
) -> Result<DagInfo, Box<dyn std::error::Error>> {
    let url = format!("{endpoint}/dag");
    let info = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<DagInfo>()
        .await?;
    Ok(info)
}

async fn get_stats_info(
    client: &Client,
    endpoint: &str,
) -> Result<StatsInfo, Box<dyn std::error::Error>> {
    let url = format!("{endpoint}/stats");
    let info = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<StatsInfo>()
        .await?;
    Ok(info)
}

async fn submit_tx_batch(
    client: &Client,
    endpoint: &str,
    txs: &[Transaction],
) -> Result<(BatchSubmitResponse, usize), Box<dyn std::error::Error>> {
    let url = format!("{endpoint}/submit-transactions");
    let payload = serde_json::to_vec(txs).unwrap_or_default();
    let payload_len = payload.len();
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(payload)
        .send()
        .await?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .unwrap_or_else(|e| format!("<body read error: {e}>"));

    if !status.is_success() {
        // Include full response body to aid debugging of submission failures
        let rejected = txs
            .iter()
            .map(|t| RejectedItem {
                id: t.id.clone(),
                error: format!("HTTP {status}: {body}"),
            })
            .collect();
        return Ok((
            BatchSubmitResponse {
                accepted: vec![],
                rejected,
            },
            payload_len,
        ));
    }

    // Try to parse the JSON response; if it fails, return the raw body for visibility
    match serde_json::from_str::<BatchSubmitResponse>(&body) {
        Ok(br) => Ok((br, payload_len)),
        Err(e) => {
            let rejected = txs
                .iter()
                .map(|t| RejectedItem {
                    id: t.id.clone(),
                    error: format!("ParseError: {e}; raw_body={body}"),
                })
                .collect();
            Ok((
                BatchSubmitResponse {
                    accepted: vec![],
                    rejected,
                },
                payload_len,
            ))
        }
    }
}

async fn build_funding_tx(
    funding: &UTXO,
    master_sk: &QantoPQPrivateKey,
    outputs_count: usize,
    tx_timestamps: Arc<RwLock<HashMap<String, u64>>>,
) -> Result<Transaction, TransactionError> {
    let pk = master_sk.public_key();
    let pk_bytes = pk.as_bytes();
    let sender = funding.address.clone();

    // Cap genesis spending to 5B QAN to prevent overflow during proto conversions
    let cap = 5_000_000_000u128 * qanto::transaction::SMALLEST_UNITS_PER_QAN;
    let (split_amount, change_amount) = if funding.amount > cap {
        (cap, funding.amount - cap)
    } else {
        (funding.amount, 0u128)
    };

    // Evenly split the input value into many outputs; account for fee
    let mut fee = calculate_dynamic_fee(split_amount);
    if fee > split_amount {
        fee = 0;
    }
    let spendable = split_amount.saturating_sub(fee);

    // Prevent zero-value outputs: cap output count by spendable amount and ensure all outputs > 0
    let effective_outputs = outputs_count.max(1).min(spendable as usize);
    let base = if effective_outputs > 0 {
        spendable / effective_outputs as u128
    } else {
        0
    };
    let remainder = if effective_outputs > 0 {
        spendable % effective_outputs as u128
    } else {
        0
    };

    let mut outputs = Vec::with_capacity(effective_outputs + if change_amount > 0 { 1 } else { 0 });
    for i in 0..effective_outputs {
        // Derive temporary address from master key material + index
        let mut seed = Vec::with_capacity(pk_bytes.len() + 8);
        seed.extend_from_slice(pk_bytes);
        seed.extend_from_slice(&(i as u64).to_be_bytes());
        let addr = hex::encode(qanto_hash(&seed).as_bytes());
        let extra = if (i as u128) < remainder { 1 } else { 0 };
        let amt = base + extra; // ensures strictly positive when effective_outputs <= spendable
        outputs.push(make_output(addr, amt, pk_bytes));
    }

    if change_amount > 0 {
        outputs.push(make_output(sender.clone(), change_amount, pk_bytes));
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
        chain_id: qanto::transaction::GLOBAL_CHAIN_ID.load(Ordering::Relaxed) as u32,
    };
    Transaction::new(cfg, master_sk).await
}

// Lock-free UTXO pool using crossbeam SegQueue for better concurrency
struct LockFreeUtxoPool {
    pools: Vec<SegQueue<UTXO>>,
    pool_count: AtomicUsize,
}

impl LockFreeUtxoPool {
    fn new(pool_count: usize) -> Self {
        let mut pools = Vec::with_capacity(pool_count);
        for _ in 0..pool_count {
            pools.push(SegQueue::new());
        }
        Self {
            pools,
            pool_count: AtomicUsize::new(0),
        }
    }

    fn bulk_pop(&self, count: usize) -> Vec<UTXO> {
        let mut utxos = Vec::with_capacity(count);
        let start_idx = self.pool_count.fetch_add(1, Ordering::Relaxed) % self.pools.len();

        // Fair round-robin: take at most one from each pool per cycle
        let mut idx = start_idx;
        loop {
            let mut made_progress = false;
            for _ in 0..self.pools.len() {
                if utxos.len() >= count {
                    break;
                }
                let pool = &self.pools[idx];
                if let Some(utxo) = pool.pop() {
                    utxos.push(utxo);
                    made_progress = true;
                }
                idx = (idx + 1) % self.pools.len();
            }
            if utxos.len() >= count || !made_progress {
                break;
            }
        }
        utxos
    }

    fn bulk_push(&self, utxos: Vec<UTXO>) {
        // Move UTXOs into pools without cloning, round-robin distribution
        for (i, utxo) in utxos.into_iter().enumerate() {
            let pool_idx = i % self.pools.len();
            self.pools[pool_idx].push(utxo);
        }
    }
}

// Enhanced token bucket with burst capacity
struct EnhancedTokenBucket {
    tokens: AtomicU64,
    burst_capacity: u64,
    refill_rate_per_tick: u64,
}

impl EnhancedTokenBucket {
    fn new(refill_rate_per_sec: u64) -> Self {
        let burst_capacity = refill_rate_per_sec * 2; // Allow 2x burst
                                                      // Smooth refills: 100 ticks per second (~10ms each)
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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Create multiple HTTP clients for better connection pooling
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
    let clients = Arc::new(clients);

    // Metrics
    let submitted = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let total_latency_ms = Arc::new(AtomicU64::new(0));
    let total_bytes_sent = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(crossbeam::queue::SegQueue::<u64>::new());

    // RSS and resource tracking
    let peak_rss_bytes = Arc::new(AtomicU64::new(0));
    let current_rss_bytes = Arc::new(AtomicU64::new(0));
    let rss_monitor = {
        let peak_rss = peak_rss_bytes.clone();
        let current_rss = current_rss_bytes.clone();
        tokio::spawn(async move {
            let mut sys = System::new();
            let pid = sysinfo::get_current_pid().ok();
            let mut interval = interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                if let Some(p) = pid {
                    sys.refresh_process(p);
                    if let Some(proc) = sys.process(p) {
                        let rss = proc.memory();
                        current_rss.store(rss, Ordering::Relaxed);
                        peak_rss.fetch_max(rss, Ordering::Relaxed);
                    }
                }
            }
        })
    };

    // Enhanced token bucket with burst capacity
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

    // Set global chain ID for transaction creation/validation
    qanto::transaction::GLOBAL_CHAIN_ID.store(args.chain_id as u64, Ordering::SeqCst);

    // Master PQ keypair
    let master_sk = if let Some(ref wallet_path) = args.wallet {
        let password = secrecy::SecretString::new(args.password.clone().unwrap_or_default());
        let wallet = qanto::wallet::Wallet::from_file(wallet_path, &password)?;
        let (sk, _pk) = wallet
            .get_keypair()
            .map_err(|e| format!("Failed to get keypair from wallet: {}", e))?;
        sk
    } else {
        let mut rng = rand::rngs::OsRng;
        QantoPQPrivateKey::generate(&mut rng)
    };

    // Fetch or assume funding UTXO
    let genesis_utxos = if args.assume_genesis_utxo {
        vec![infer_genesis_utxo(&args.genesis_address)]
    } else {
        match get_utxos_for_address(&clients[0], &args.endpoint, &args.genesis_address).await {
            Ok(list) if !list.is_empty() => list,
            Ok(_empty) => {
                println!(
                    "No UTXOs returned for genesis address; falling back to assumed genesis UTXO."
                );
                vec![infer_genesis_utxo(&args.genesis_address)]
            }
            Err(e) => {
                println!(
                    "Failed to fetch UTXOs for genesis address: {e}; falling back to assumed genesis UTXO."
                );
                vec![infer_genesis_utxo(&args.genesis_address)]
            }
        }
    };
    // Log initial UTXO fetch details to verify funding source correctness
    if !genesis_utxos.is_empty() {
        let top = choose_best_funding_utxo(genesis_utxos.clone());
        if let Some(best) = top.as_ref() {
            println!(
                "Fetched {} UTXOs for genesis address; top funding amount={} id={} index={}",
                genesis_utxos.len(),
                best.amount,
                best.tx_id,
                best.output_index
            );
        } else {
            println!(
                "Fetched {} UTXOs for genesis address; no suitable funding UTXO found",
                genesis_utxos.len()
            );
        }
    } else {
        println!("No UTXOs fetched for genesis address; verify node state and API.");
    }
    let funding = choose_best_funding_utxo(genesis_utxos)
        .ok_or_else(|| "No funding UTXOs for genesis address".to_string())?;
    let tx_timestamps = Arc::new(RwLock::new(HashMap::new()));

    // Create more initial UTXOs for better parallelism using a dynamic multi-stage split
    let target_utxos = (args.tps * 10).max(100).min(10000) as usize;
    println!("Target UTXO pool size: {target_utxos}");

    let mut current_utxos = vec![funding];
    let mut stage = 1;

    while current_utxos.len() < target_utxos {
        let current_len = current_utxos.len();
        // Calculate factor: how much to split each UTXO (factor <= 10 to fit in txn limits)
        let factor = ((target_utxos + current_len - 1) / current_len).min(10);
        if factor <= 1 {
            break;
        }

        println!(
            "Stage {}: Splitting {} UTXOs into {} outputs each (target: {})...",
            stage,
            current_len,
            factor,
            current_len * factor
        );

        let mut stage_txs = Vec::with_capacity(current_len);
        for utxo in &current_utxos {
            let tx = build_funding_tx(utxo, &master_sk, factor, tx_timestamps.clone()).await?;
            stage_txs.push(tx);
        }

        // Fetch starting block count to wait for confirmation
        let start_stats = get_stats_info(&clients[0], &args.endpoint)
            .await
            .unwrap_or(StatsInfo {
                block_count: 0,
                mempool_size: 0,
            });

        // Submit in batches of 10 to avoid huge HTTP payloads, with retries and spacing
        for chunk in stage_txs.chunks(10) {
            let mut attempts = 0;
            loop {
                match submit_tx_batch(&clients[0], &args.endpoint, chunk).await {
                    Ok(_) => break,
                    Err(e) => {
                        attempts += 1;
                        if attempts >= 5 {
                            println!(
                                "Error: Failed to submit pre-split batch after 5 attempts: {e}"
                            );
                            return Err(e);
                        }
                        println!("Warning: Pre-split batch submission failed (attempt {attempts}/5): {e}. Retrying in 3s...");
                        sleep(Duration::from_secs(3)).await;
                    }
                }
            }
            // Add a small delay between batches to avoid overloading the node's HTTP server
            sleep(Duration::from_millis(100)).await;
        }

        // Wait until block height advances and mempool is empty (confirmation gate)
        println!("Submitted Stage {stage} funding transactions; waiting for confirmation...");
        let wait_start = Instant::now();
        let timeout = Duration::from_secs(30);
        loop {
            sleep(Duration::from_millis(500)).await;
            if let Ok(curr_stats) = get_stats_info(&clients[0], &args.endpoint).await {
                if curr_stats.block_count > start_stats.block_count && curr_stats.mempool_size == 0
                {
                    println!(
                        "Stage {stage} confirmed (block height advanced from {} to {}, mempool empty).",
                        start_stats.block_count, curr_stats.block_count
                    );
                    break;
                }
            }
            if wait_start.elapsed() > timeout {
                println!(
                    "Warning: Confirmation gate timed out for Stage {stage} after {}s. Proceeding.",
                    timeout.as_secs()
                );
                break;
            }
        }

        let mut new_utxos = Vec::with_capacity(stage_txs.len() * factor);
        for tx in &stage_txs {
            for (i, o) in tx.outputs.iter().enumerate() {
                new_utxos.push(UTXO {
                    address: o.address.clone(),
                    amount: o.amount,
                    tx_id: tx.id.clone(),
                    output_index: i as u32,
                    explorer_link: String::new(),
                });
            }
        }
        current_utxos = new_utxos;
        stage += 1;
    }

    let start_info = if args.skip_dag_info {
        DagInfo {
            block_count: 0,
            tip_count: 0,
            current_difficulty: 0.0,
            target_block_time: 0,
            validator_count: 0,
            num_chains: 0,
            latest_block_timestamp: 0,
        }
    } else {
        get_dag_info(&clients[0], &args.endpoint)
            .await
            .unwrap_or(DagInfo {
                block_count: 0,
                tip_count: 0,
                current_difficulty: 0.0,
                target_block_time: 0,
                validator_count: 0,
                num_chains: 0,
                latest_block_timestamp: 0,
            })
    };

    // Initialize lock-free UTXO pools
    let utxo_pool = Arc::new(LockFreeUtxoPool::new(args.utxo_pools));
    utxo_pool.bulk_push(current_utxos);

    // Spawn enhanced workers with optimized batch processing
    let stop_at = Instant::now() + Duration::from_secs(args.duration);
    let batch_size = args.batch_size;
    let mut set = JoinSet::new();

    for worker_id in 0..args.concurrency {
        let client = clients[worker_id % clients.len()].clone();
        let endpoint = args.endpoint.clone();
        let submitted = submitted.clone();
        let failed = failed.clone();
        let total_latency_ms = total_latency_ms.clone();
        let token_bucket = token_bucket.clone();
        let utxo_pool = utxo_pool.clone();
        let tx_timestamps = tx_timestamps.clone();
        let master_sk = master_sk.clone();
        let total_bytes_sent = total_bytes_sent.clone();
        let latencies = latencies.clone();

        set.spawn(async move {
            while Instant::now() < stop_at {
                // Bulk collect UTXOs for better efficiency
                let utxos_to_spend = utxo_pool.bulk_pop(batch_size);

                if utxos_to_spend.is_empty() {
                    sleep(Duration::from_micros(100)).await; // Shorter sleep
                    continue;
                }

                // Check token availability for the entire batch
                let batch_tokens = utxos_to_spend.len() as u64;
                if !token_bucket.try_consume(batch_tokens) {
                    // Return UTXOs to pool and back off
                    utxo_pool.bulk_push(utxos_to_spend);
                    sleep(Duration::from_micros(500)).await;
                    continue;
                }

                // Enhanced parallel transaction building with Rayon
                let tx_timestamps_clone = tx_timestamps.clone();
                let master_sk_clone = master_sk.clone();
                let rt = tokio::runtime::Handle::current();

                let pairs: Vec<(Transaction, UTXO)> = tokio::task::spawn_blocking(move || {
                    // Use Rayon's thread pool more efficiently
                    utxos_to_spend
                        .into_par_iter()
                        .with_min_len(10) // Minimum work per thread
                        .filter_map(|utxo| {
                            // Pre-build transaction data to avoid async in parallel context
                            let pk = master_sk_clone.public_key();
                            let pk_bytes = pk.as_bytes();
                            let sender = utxo.address.clone();

                            // Create a simple spend transaction
                            let fee = calculate_dynamic_fee(utxo.amount);
                            let spendable = utxo.amount.saturating_sub(fee);

                            if spendable == 0 {
                                return None;
                            }

                            // Generate deterministic recipient address
                            let mut seed = Vec::with_capacity(pk_bytes.len() + utxo.tx_id.len());
                            seed.extend_from_slice(pk_bytes);
                            seed.extend_from_slice(utxo.tx_id.as_bytes());
                            let recipient = hex::encode(qanto_hash(&seed).as_bytes());

                            let output = make_output(recipient, spendable, pk_bytes);
                            let inputs = vec![Input {
                                tx_id: utxo.tx_id.clone(),
                                output_index: utxo.output_index,
                            }];

                            let cfg = TransactionConfig {
                                sender,
                                receiver: output.address.clone(),
                                amount: spendable,
                                fee,
                                gas_limit: 50_000,
                                gas_price: 1,
                                priority_fee: 0,
                                inputs,
                                outputs: vec![output],
                                metadata: None,
                                tx_timestamps: tx_timestamps_clone.clone(),
                                chain_id: qanto::transaction::GLOBAL_CHAIN_ID
                                    .load(Ordering::Relaxed)
                                    as u32,
                            };

                            // Build transaction synchronously where possible
                            match rt.block_on(Transaction::new(cfg, &master_sk_clone)) {
                                Ok(tx) => Some((tx, utxo)),
                                Err(_) => None,
                            }
                        })
                        .collect()
                })
                .await
                .unwrap_or_default();

                // Count failed transactions
                let failed_count = batch_tokens as usize - pairs.len();
                if failed_count > 0 {
                    failed.fetch_add(failed_count as u64, Ordering::Relaxed);
                }

                if pairs.is_empty() {
                    continue;
                }

                // Submit as batch with timing
                let t0 = Instant::now();
                let txs: Vec<Transaction> = pairs.iter().map(|(tx, _)| tx.clone()).collect();
                let (resp, batch_bytes) = match submit_tx_batch(&client, &endpoint, &txs).await {
                    Ok(res) => res,
                    Err(_) => (
                        BatchSubmitResponse {
                            accepted: vec![],
                            rejected: vec![],
                        },
                        0,
                    ),
                };
                let dt_us = t0.elapsed().as_micros() as u64;
                let dt = dt_us / 1000;
                latencies.push(dt_us);
                total_bytes_sent.fetch_add(batch_bytes as u64, Ordering::Relaxed);

                // Log rejected reasons for visibility (sample up to 5 per batch)
                if !resp.rejected.is_empty() {
                    for r in resp.rejected.iter().take(5) {
                        println!("Rejected tx {}: {}", r.id, r.error);
                    }
                }

                // Process results and recycle UTXOs
                let accepted_set: HashSet<String> = resp.accepted.into_iter().collect();
                let per_ms = dt.saturating_div(accepted_set.len().max(1) as u64);
                let mut new_utxos = Vec::new();

                for (tx, orig_utxo) in &pairs {
                    if accepted_set.contains(&tx.id) {
                        submitted.fetch_add(1, Ordering::Relaxed);
                        total_latency_ms.fetch_add(per_ms, Ordering::Relaxed);

                        // Recycle the first output as a new UTXO
                        if let Some(o) = tx.outputs.first() {
                            new_utxos.push(UTXO {
                                address: o.address.clone(),
                                amount: o.amount,
                                tx_id: tx.id.clone(),
                                output_index: 0,
                                explorer_link: String::new(),
                            });
                        }
                    } else {
                        failed.fetch_add(1, Ordering::Relaxed);
                        // Recycle original input UTXO only if the transaction was actually rejected or never acknowledged
                        new_utxos.push(orig_utxo.clone());
                    }
                }

                // Bulk add new UTXOs back to pool
                if !new_utxos.is_empty() {
                    utxo_pool.bulk_push(new_utxos);
                }
            }
        });
    }

    while set.join_next().await.is_some() {}
    drop(refill);
    rss_monitor.abort();

    // Final report
    let end_info = if args.skip_dag_info {
        DagInfo {
            block_count: 0,
            tip_count: 0,
            current_difficulty: 0.0,
            target_block_time: 0,
            validator_count: 0,
            num_chains: 0,
            latest_block_timestamp: 0,
        }
    } else {
        get_dag_info(&clients[0], &args.endpoint)
            .await
            .unwrap_or(DagInfo {
                block_count: 0,
                tip_count: 0,
                current_difficulty: 0.0,
                target_block_time: 0,
                validator_count: 0,
                num_chains: 0,
                latest_block_timestamp: 0,
            })
    };

    let total = submitted.load(Ordering::Relaxed);
    let failed_n = failed.load(Ordering::Relaxed);
    let avg_lat = if total > 0 {
        total_latency_ms.load(Ordering::Relaxed) / total
    } else {
        0
    };
    let actual_tps = if args.duration > 0 {
        total / args.duration
    } else {
        0
    };
    let confirmed_blocks = end_info.block_count.saturating_sub(start_info.block_count);
    let confirmed_bps = if args.duration > 0 {
        confirmed_blocks as u64 / args.duration
    } else {
        0
    };

    // Calculate latency percentiles
    let mut lat_vec = Vec::new();
    while let Some(l) = latencies.pop() {
        lat_vec.push(l);
    }
    lat_vec.sort();

    let (p50, p90, p99) = if !lat_vec.is_empty() {
        let len = lat_vec.len();
        let p50 = lat_vec[len * 50 / 100] as f64 / 1000.0;
        let p90 = lat_vec[len * 90 / 100] as f64 / 1000.0;
        let p99 = lat_vec[len * 99 / 100] as f64 / 1000.0;
        (p50, p90, p99)
    } else {
        (0.0, 0.0, 0.0)
    };

    // RSS Memory info
    let peak_rss_mb = peak_rss_bytes.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
    let final_rss_mb = current_rss_bytes.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);

    // Throughput info
    let total_bytes = total_bytes_sent.load(Ordering::Relaxed);
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let total_mbit = (total_bytes * 8) as f64 / (1000.0 * 1000.0);
    let mb_per_sec = if args.duration > 0 {
        total_mb / args.duration as f64
    } else {
        0.0
    };
    let mbit_per_sec = if args.duration > 0 {
        total_mbit / args.duration as f64
    } else {
        0.0
    };

    println!("=== tx_flood Enhanced Report ===");
    let duration = args.duration;
    let tps = args.tps;
    println!("Duration: {duration}s");
    println!("Target TPS: {tps} | Actual Submitted TPS: {actual_tps}");
    println!("Submitted: {total} | Failed: {failed_n}");
    println!("Average Submission Latency: {avg_lat} ms");
    println!("P50 Latency: {p50:.2} ms | P90 Latency: {p90:.2} ms | P99 Latency: {p99:.2} ms");
    println!("Write Throughput: {mb_per_sec:.2} MB/s ({mbit_per_sec:.2} Mb/s) | Total Sent: {total_mb:.2} MB");
    println!("Peak RSS Memory: {peak_rss_mb:.2} MB | Final RSS Memory: {final_rss_mb:.2} MB");
    println!("Estimated Blocks Per Second: {confirmed_bps} (Δ blocks: {confirmed_blocks})");
    println!(
        "Concurrency: {} workers | Batch size: {} | UTXO pools: {}",
        args.concurrency, args.batch_size, args.utxo_pools
    );
    println!(
        "HTTP clients: {} | Success rate: {:.2}%",
        args.http_clients,
        if total + failed_n > 0 {
            (total as f64 / (total + failed_n) as f64) * 100.0
        } else {
            0.0
        }
    );

    Ok(())
}
