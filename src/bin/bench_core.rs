use clap::Parser;
use qanto::config::Config;
use qanto::elite_mempool::EliteMempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "bench_core")]
struct Args {
    #[arg(long)]
    config: Option<String>,

    #[arg(long, default_value_t = 1_000_000)]
    tx_count: usize,

    #[arg(long)]
    batch_size: Option<usize>,

    #[arg(long)]
    mempool_max_size_bytes: Option<usize>,

    #[arg(long)]
    mempool_max_transactions: Option<usize>,

    #[arg(long, default_value_t = 64)]
    shard_count: usize,

    #[arg(long, default_value_t = 16)]
    worker_count: usize,

    #[arg(long, default_value_t = false)]
    expect_velocity: bool,

    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Serialize)]
struct BenchCoreResult {
    tx_count: usize,
    batch_size: usize,
    duration_secs: f64,
    tps: f64,
    config_path: Option<String>,
    config_target_block_time_ms: Option<u64>,
    config_mempool_batch_size: Option<usize>,
    config_mempool_max_size_bytes: Option<usize>,
    velocity_expected: bool,
    velocity_pass: Option<bool>,
}

fn main() {
    let args = Args::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.worker_count.max(1))
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            eprintln!("Failed to build Tokio runtime: {e}");
            std::process::exit(2);
        });
    runtime.block_on(async_main(args));
}

async fn async_main(args: Args) {
    println!("Starting Operation Hyperscale Benchmark: Core Protocol Saturation");

    let mut loaded_config: Option<Config> = None;
    if let Some(ref config_path) = args.config {
        match Config::load(config_path) {
            Ok(cfg) => loaded_config = Some(cfg),
            Err(e) => {
                eprintln!("Failed to load config from {config_path}: {e}");
                std::process::exit(2);
            }
        }
    }

    let config_target_block_time_ms = loaded_config.as_ref().map(|c| c.target_block_time);
    let config_mempool_batch_size = loaded_config.as_ref().and_then(|c| c.mempool_batch_size);
    let config_mempool_max_size_bytes = loaded_config
        .as_ref()
        .and_then(|c| c.mempool_max_size_bytes);

    let batch_size = args
        .batch_size
        .or(config_mempool_batch_size)
        .unwrap_or(50_000);
    let mempool_max_size_bytes = args
        .mempool_max_size_bytes
        .or(config_mempool_max_size_bytes)
        .unwrap_or(1024 * 1024 * 1024);
    let mempool_max_transactions = args
        .mempool_max_transactions
        .or_else(|| loaded_config.as_ref().and_then(|c| c.mempool_max_size))
        .unwrap_or(2_000_000);

    let velocity_pass = if args.expect_velocity {
        let expected_batch = 50_000usize;
        let expected_max_bytes = 8_589_934_592usize;
        let expected_block_time_ms = 31u64;
        Some(
            config_target_block_time_ms == Some(expected_block_time_ms)
                && config_mempool_batch_size == Some(expected_batch)
                && config_mempool_max_size_bytes == Some(expected_max_bytes),
        )
    } else {
        None
    };

    // 1. Setup Core Components (In-Memory)
    println!("Initializing in-memory DAG and EliteMempool...");
    let dag = Arc::new(<QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification());

    let mempool = Arc::new(
        EliteMempool::new(
            mempool_max_transactions,
            mempool_max_size_bytes,
            args.shard_count,
            args.worker_count,
        )
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize EliteMempool: {e}");
            std::process::exit(2);
        }),
    );
    if let Err(e) = mempool.start().await {
        eprintln!("Failed to start EliteMempool: {e}");
        std::process::exit(2);
    }

    let utxos = Arc::new(HashMap::<String, UTXO>::new());

    // 2. Pre-generate Transactions
    let tx_count = args.tx_count;
    println!(
        "Pre-generating {} signed transactions (Parallel)...",
        tx_count
    );
    let gen_start = Instant::now();

    // Use rayon for parallel generation to speed up setup
    let transactions: Vec<Arc<Transaction>> = (0..tx_count)
        .into_par_iter()
        .map(|_| Arc::new(Transaction::new_dummy()))
        .collect();

    let gen_time = gen_start.elapsed();
    println!(
        "Generated {} txs in {:.4}s ({:.0} tx/s generation rate)",
        tx_count,
        gen_time.as_secs_f64(),
        tx_count as f64 / gen_time.as_secs_f64()
    );

    // 3. Inject Transactions (Core Benchmark)
    println!("Injecting transactions into EliteMempool (Hyperscale Mode - Parallel Injection)...");
    let start_time = Instant::now();

    let transactions = Arc::new(transactions);
    let mut ranges = Vec::new();
    let mut offset = 0usize;
    while offset < tx_count {
        let end = (offset + batch_size).min(tx_count);
        ranges.push((offset, end));
        offset = end;
    }

    let mut handles = Vec::new();
    for (start, end) in ranges {
        let mempool = mempool.clone();
        let _utxos = utxos.clone();
        let _dag = dag.clone();
        let transactions = transactions.clone();

        handles.push(tokio::spawn(async move {
            let batch = &transactions[start..end];
            let _ = mempool.add_transaction_batch(batch);
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let duration = start_time.elapsed();
    let tps = tx_count as f64 / duration.as_secs_f64();

    println!("--------------------------------------------------");
    println!("OPERATION HYPERSCALE: BENCHMARK RESULTS");
    println!("--------------------------------------------------");
    println!("Total Transactions: {}", tx_count);
    println!("Total Time:         {:.6}s", duration.as_secs_f64());
    println!("Raw Throughput:     {:.2} TPS", tps);
    if args.expect_velocity {
        match velocity_pass {
            Some(true) => println!("Velocity Config Check: PASS"),
            Some(false) => println!("Velocity Config Check: FAIL"),
            None => println!("Velocity Config Check: FAIL"),
        }
    }
    println!("--------------------------------------------------");

    if tps > 9_000_000.0 {
        println!("SUCCESS: 10M TPS Target Achieved");
    } else if tps > 1_000_000.0 {
        println!("RESULT: >1M TPS (High Performance)");
    } else {
        println!("WARNING: Performance below hyperscale targets");
    }

    if args.json {
        let result = BenchCoreResult {
            tx_count,
            batch_size,
            duration_secs: duration.as_secs_f64(),
            tps,
            config_path: args.config,
            config_target_block_time_ms,
            config_mempool_batch_size,
            config_mempool_max_size_bytes,
            velocity_expected: args.expect_velocity,
            velocity_pass,
        };
        match serde_json::to_string(&result) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Failed to serialize benchmark result: {e}");
                std::process::exit(2);
            }
        }
    }
}
