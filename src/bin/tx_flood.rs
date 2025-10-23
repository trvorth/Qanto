// tx_flood: High-concurrency transaction flooder for Qanto
// Minimal but production-ready scaffold with core logic and comments

use clap::Parser;
use rayon::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
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
    version = "0.1.0",
    about = "Flood the Qanto node with valid transactions and report metrics"
)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    endpoint: String,
    #[arg(long, default_value_t = 10_000_000)]
    tps: u64,
    #[arg(long, default_value_t = 60)]
    duration: u64,
    #[arg(long, default_value_t = 5000)]
    concurrency: usize,
    #[arg(long, default_value_t = 100)]
    batch_size: usize,
    #[arg(
        long,
        help = "Prefunded genesis address to source UTXOs",
        required = true
    )]
    genesis_address: String,
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
struct SubmitResponse {
    ok: bool,
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

fn address_from_pk(pk_bytes: &[u8]) -> String {
    hex::encode(qanto_hash(pk_bytes).as_bytes())
}

fn make_output(addr: String, amount: u64, pk_bytes: &[u8]) -> Output {
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

async fn submit_tx_batch(
    client: &Client,
    endpoint: &str,
    txs: &[Transaction],
) -> Result<BatchSubmitResponse, Box<dyn std::error::Error>> {
    let url = format!("{endpoint}/submit-transactions");
    let resp = client.post(url).json(&txs).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let rejected = txs
            .iter()
            .map(|t| RejectedItem {
                id: t.id.clone(),
                error: format!("HTTP {status}"),
            })
            .collect();
        return Ok(BatchSubmitResponse {
            accepted: vec![],
            rejected,
        });
    }
    match resp.json::<BatchSubmitResponse>().await {
        Ok(br) => Ok(br),
        Err(e) => {
            let rejected = txs
                .iter()
                .map(|t| RejectedItem {
                    id: t.id.clone(),
                    error: format!("ParseError: {e}"),
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
    tx_timestamps: Arc<RwLock<HashMap<String, u64>>>,
) -> Result<Transaction, TransactionError> {
    let pk = master_sk.public_key();
    let pk_bytes = pk.as_bytes();
    let sender = address_from_pk(pk_bytes);

    // Evenly split the input value into many outputs; account for fee
    let mut fee = calculate_dynamic_fee(funding.amount);
    if fee > funding.amount {
        fee = 0;
    }
    let spendable = funding.amount.saturating_sub(fee);

    // Prevent zero-value outputs: cap output count by spendable amount and ensure all outputs > 0
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
        // Derive temporary address from master key material + index
        let mut seed = Vec::with_capacity(pk_bytes.len() + 8);
        seed.extend_from_slice(pk_bytes);
        seed.extend_from_slice(&(i as u64).to_be_bytes());
        let addr = hex::encode(qanto_hash(&seed).as_bytes());
        let extra = if (i as u64) < remainder { 1 } else { 0 };
        let amt = base + extra; // ensures strictly positive when effective_outputs <= spendable
        outputs.push(make_output(addr, amt, pk_bytes));
    }

    let inputs = vec![Input {
        tx_id: funding.tx_id.clone(),
        output_index: funding.output_index,
    }];
    let cfg = TransactionConfig {
        sender,
        receiver: "0".repeat(64),
        amount: spendable, // non-zero per validation; semantic not enforced
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

async fn build_spend_tx(
    utxo: &UTXO,
    master_sk: &QantoPQPrivateKey,
    tx_timestamps: Arc<RwLock<HashMap<String, u64>>>,
) -> Result<Transaction, TransactionError> {
    let pk = master_sk.public_key();
    let pk_bytes = pk.as_bytes();
    let sender = address_from_pk(pk_bytes);

    // Derive new destination address for uniqueness
    let mut seed = Vec::with_capacity(pk_bytes.len() + 8);
    seed.extend_from_slice(pk_bytes);
    seed.extend_from_slice(qanto_hash(utxo.tx_id.as_bytes()).as_bytes());
    let dst_addr = hex::encode(qanto_hash(&seed).as_bytes());

    let fee = calculate_dynamic_fee(utxo.amount);
    let send_amt = utxo.amount.saturating_sub(fee);

    let outputs = vec![make_output(dst_addr.clone(), send_amt, pk_bytes)];
    let inputs = vec![Input {
        tx_id: utxo.tx_id.clone(),
        output_index: utxo.output_index,
    }];
    let cfg = TransactionConfig {
        sender,
        receiver: dst_addr,
        amount: send_amt,
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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let client = Client::builder().build()?;

    // Metrics
    let submitted = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let total_latency_ms = Arc::new(AtomicU64::new(0));

    // Global token bucket to approximate target TPS
    let tokens = Arc::new(AtomicU64::new(0));
    let refill = {
        let tokens = tokens.clone();
        let tps = args.tps;
        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(1));
            loop {
                tick.tick().await;
                tokens.store(tps, Ordering::Relaxed);
            }
        })
    };

    // Master PQ keypair
    let mut rng = rand::rngs::OsRng;
    let master_sk = QantoPQPrivateKey::generate(&mut rng);

    // Fetch funding UTXOs from genesis address and pre-populate spendable outputs
    let genesis_utxos =
        get_utxos_for_address(&client, &args.endpoint, &args.genesis_address).await?;
    let funding = choose_best_funding_utxo(genesis_utxos)
        .ok_or_else(|| "No funding UTXOs for genesis address".to_string())?;
    let tx_timestamps = Arc::new(RwLock::new(HashMap::new()));
    let funding_tx = build_funding_tx(
        &funding,
        &master_sk,
        args.concurrency * 10,
        tx_timestamps.clone(),
    )
    .await?;
    // Submit funding outputs via batch endpoint (single element)
    let _ = submit_tx_batch(&client, &args.endpoint, std::slice::from_ref(&funding_tx)).await?;

    let start_info = get_dag_info(&client, &args.endpoint)
        .await
        .unwrap_or(DagInfo {
            block_count: 0,
            tip_count: 0,
            current_difficulty: 0.0,
            target_block_time: 0,
            validator_count: 0,
            num_chains: 0,
            latest_block_timestamp: 0,
        });

    // Locally track the outputs we created for spending during the flood
    let mut initial_utxos = Vec::new();
    for (i, o) in funding_tx.outputs.iter().enumerate() {
        initial_utxos.push(UTXO {
            address: o.address.clone(),
            amount: o.amount,
            tx_id: funding_tx.id.clone(),
            output_index: i as u32,
            explorer_link: String::new(),
        });
    }
    let utxo_pool = Arc::new(Mutex::new(initial_utxos));

    // Spawn workers
    let stop_at = Instant::now() + Duration::from_secs(args.duration);
    let batch_size = args.batch_size;
    let mut set = JoinSet::new();
    for _ in 0..args.concurrency {
        let client = client.clone();
        let endpoint = args.endpoint.clone();
        let submitted = submitted.clone();
        let failed = failed.clone();
        let total_latency_ms = total_latency_ms.clone();
        let tokens = tokens.clone();
        let utxo_pool = utxo_pool.clone();
        let tx_timestamps = tx_timestamps.clone();
        let master_sk = master_sk.clone();

        set.spawn(async move {
            while Instant::now() < stop_at {
                // Build up to batch_size transactions, each requiring one token and one UTXO
                let mut utxos_to_spend: Vec<UTXO> = Vec::with_capacity(batch_size);
                
                // Collect UTXOs and tokens in parallel
                while utxos_to_spend.len() < batch_size {
                    let utxo_opt = {
                        let mut guard = utxo_pool.lock().await;
                        guard.pop()
                    };
                    if let Some(utxo) = utxo_opt {
                        // Acquire a token; if none available, push UTXO back and back off briefly
                        let got = tokens.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |t| {
                            if t > 0 {
                                Some(t - 1)
                            } else {
                                None
                            }
                        });
                        if got.is_err() {
                            // return UTXO to pool
                            let mut guard = utxo_pool.lock().await;
                            guard.push(utxo);
                            sleep(Duration::from_millis(1)).await;
                            break;
                        }
                        utxos_to_spend.push(utxo);
                    } else {
                        // No UTXO available; small pause
                        sleep(Duration::from_millis(1)).await;
                        break;
                    }
                }

                if utxos_to_spend.is_empty() {
                    continue;
                }

                // Use Rayon to build transactions in parallel
                let tx_timestamps_clone = tx_timestamps.clone();
                let master_sk_clone = master_sk.clone();
                let pairs: Vec<(Transaction, UTXO)> = tokio::task::spawn_blocking(move || {
                    utxos_to_spend
                        .into_par_iter()
                        .filter_map(|utxo| {
                            let rt = tokio::runtime::Handle::current();
                            match rt.block_on(build_spend_tx(&utxo, &master_sk_clone, tx_timestamps_clone.clone())) {
                                Ok(tx) => Some((tx, utxo)),
                                Err(_) => None,
                            }
                        })
                        .collect()
                }).await.unwrap_or_default();

                // Count failed transactions
                let failed_count = batch_size - pairs.len();
                if failed_count > 0 {
                    failed.fetch_add(failed_count as u64, Ordering::Relaxed);
                }

                if pairs.is_empty() {
                    continue;
                }

                // Submit as batch (single or multiple)
                let t0 = Instant::now();
                let txs: Vec<Transaction> = pairs.iter().map(|(tx, _)| tx.clone()).collect();
                let resp = submit_tx_batch(&client, &endpoint, &txs).await.unwrap_or(
                    BatchSubmitResponse {
                        accepted: vec![],
                        rejected: vec![],
                    },
                );
                let dt = t0.elapsed().as_millis() as u64;
                let accepted_set: HashSet<String> = resp.accepted.into_iter().collect();
                let per_ms = dt.saturating_div(accepted_set.len().max(1) as u64);
                for (tx, _orig_utxo) in &pairs {
                    if accepted_set.contains(&tx.id) {
                        submitted.fetch_add(1, Ordering::Relaxed);
                        total_latency_ms.fetch_add(per_ms, Ordering::Relaxed);
                        if let Some(o) = tx.outputs.first() {
                            let new_u = UTXO {
                                address: o.address.clone(),
                                amount: o.amount,
                                tx_id: tx.id.clone(),
                                output_index: 0,
                                explorer_link: String::new(),
                            };
                            let mut guard = utxo_pool.lock().await;
                            guard.push(new_u);
                        }
                    } else {
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    }

    while set.join_next().await.is_some() {}
    drop(refill);

    // Final report
    let end_info = get_dag_info(&client, &args.endpoint)
        .await
        .unwrap_or(DagInfo {
            block_count: 0,
            tip_count: 0,
            current_difficulty: 0.0,
            target_block_time: 0,
            validator_count: 0,
            num_chains: 0,
            latest_block_timestamp: 0,
        });
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

    println!("=== tx_flood Report ===");
    let duration = args.duration;
    let tps = args.tps;
    println!("Duration: {duration}s");
    println!("Target TPS: {tps} | Actual Submitted TPS: {actual_tps}");
    println!("Submitted: {total} | Failed: {failed_n}");
    println!("Average Submission Latency: {avg_lat} ms");
    println!("Estimated Blocks Per Second: {confirmed_bps} (Δ blocks: {confirmed_blocks})");

    Ok(())
}
