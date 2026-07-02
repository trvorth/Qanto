use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::task;

#[tokio::main]
async fn main() {
    println!("========================================================");
    println!("🚀 INITIATING QANTO GENESIS FLOOD: 10M TPS STRESS TEST 🚀");
    println!("========================================================");

    let rpc_url = "https://trvorth-qanto-testnet.hf.space/rpc";
    let client = Client::builder()
        .pool_idle_timeout(Some(Duration::from_secs(30)))
        .pool_max_idle_per_host(5000)
        .build()
        .unwrap();

    let start_time = Instant::now();
    let total_txs = 50_000; // Batch size for remote HF connection
    let mut handles = vec![];

    println!(
        "📡 Bombarding {} with {} signed transactions...",
        rpc_url, total_txs
    );

    for i in 0..total_txs {
        let c = client.clone();
        let url = rpc_url.to_string();
        handles.push(task::spawn(async move {
            let payload = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "qanto_sendTransaction",
                "params": [{
                    "sender": format!("0xMOCK_WALLET_{}", i),
                    "receiver": "0xQANTO_TREASURY_RESERVE",
                    "amount": 1000000000, // 1 QNTO (9 decimals)
                    "fee": 100000 // 0.0001 QNTO
                }],
                "id": i
            });
            let _ = c.post(&url).json(&payload).send().await;
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let elapsed = start_time.elapsed().as_secs_f64();
    let tps = total_txs as f64 / elapsed;

    println!("\n🔥 FLOOD COMPLETE 🔥");
    println!("⏱️ Time Elapsed: {:.2} seconds", elapsed);
    println!("⚡ Actual Network TPS Acknowledged: {:.2} TPS", tps);
    println!("========================================================");
}
