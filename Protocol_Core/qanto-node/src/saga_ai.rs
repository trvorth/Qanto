//! SAGA AI: Synaptic Autonomous Governance Algorithm
//! Applies conservative testnet guardrails so governance heuristics do not overclock the network.

use crate::telemetry::get_live_metrics;
use std::sync::atomic::{AtomicU64, Ordering};

const TESTNET_BASE_BATCH_THRESHOLD: u64 = 50_000;
const TESTNET_MIN_BATCH_THRESHOLD: u64 = 10_000;
const TESTNET_MAX_BATCH_THRESHOLD: u64 = 75_000;
const TESTNET_BASE_BPS: u64 = 8;
const TESTNET_MIN_BPS: u64 = 4;
const TESTNET_MAX_BPS: u64 = 12;
const TESTNET_GASLESS_BPS_LIMIT: u64 = 10;

lazy_static::lazy_static! {
    pub static ref DYNAMIC_BATCH_THRESHOLD: AtomicU64 = AtomicU64::new(TESTNET_BASE_BATCH_THRESHOLD);
    pub static ref NETWORK_THROTTLE_BPS: AtomicU64 = AtomicU64::new(TESTNET_BASE_BPS);
}

pub struct SagaDecision {
    pub adjusted_batch_size: u64,
    pub target_bps: u64,
    pub ai_confidence_score: f64,
    pub action_taken: String,
}

/// The SAGA Engine evaluates current telemetry and dynamically scales consensus.
pub fn evaluate_network_state() -> SagaDecision {
    let metrics = get_live_metrics();

    let current_batch = DYNAMIC_BATCH_THRESHOLD.load(Ordering::Relaxed);
    let current_bps = NETWORK_THROTTLE_BPS.load(Ordering::Relaxed);
    let mut new_batch_size =
        current_batch.clamp(TESTNET_MIN_BATCH_THRESHOLD, TESTNET_MAX_BATCH_THRESHOLD);
    let mut target_bps = current_bps.clamp(TESTNET_MIN_BPS, TESTNET_MAX_BPS);
    let mut action =
        String::from("SAGA: Testnet stable. Holding conservative throughput guardrails.");
    let mut confidence = 0.92;

    if metrics.cpu_usage >= 85.0
        || metrics.mem_usage_mb >= 4096
        || metrics.synaptic_latency_ms >= 250.0
    {
        new_batch_size = ((new_batch_size as f64) * 0.8) as u64;
        new_batch_size =
            new_batch_size.clamp(TESTNET_MIN_BATCH_THRESHOLD, TESTNET_MAX_BATCH_THRESHOLD);
        target_bps = TESTNET_MIN_BPS;
        action = String::from(
            "SAGA OVERRIDE: Testnet pressure detected. Reducing batch size and BPS to protect node stability.",
        );
        confidence = 0.97;
    } else if metrics.cpu_usage <= 45.0
        && metrics.mem_usage_mb <= 2048
        && metrics.current_tps >= 25_000
    {
        new_batch_size = ((new_batch_size as f64) * 1.1) as u64;
        new_batch_size =
            new_batch_size.clamp(TESTNET_MIN_BATCH_THRESHOLD, TESTNET_MAX_BATCH_THRESHOLD);
        target_bps = (target_bps + 1).clamp(TESTNET_MIN_BPS, TESTNET_MAX_BPS);
        action = String::from(
            "SAGA OVERRIDE: Sustained testnet demand detected. Applying a bounded throughput increase.",
        );
        confidence = 0.95;
    } else {
        new_batch_size = TESTNET_BASE_BATCH_THRESHOLD;
        target_bps = TESTNET_BASE_BPS;
    }

    // Apply changes atomically to the lock-free mempool configuration
    DYNAMIC_BATCH_THRESHOLD.store(new_batch_size, Ordering::SeqCst);
    NETWORK_THROTTLE_BPS.store(target_bps, Ordering::SeqCst);

    broadcast_to_discord(&action);

    SagaDecision {
        adjusted_batch_size: new_batch_size,
        target_bps,
        ai_confidence_score: confidence,
        action_taken: action,
    }
}

pub fn broadcast_to_discord(action_message: &str) {
    let webhook_url = std::env::var("DISCORD_SAGA_WEBHOOK").unwrap_or_default();
    if webhook_url.is_empty() {
        return;
    }

    let message = action_message.to_string();
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "username": "SAGA Neural Core",
            "embeds": [{
                "title": "🧠 SAGA AI Governance Decision",
                "description": message,
                "color": 9133270,
            }]
        });
        let _ = client.post(&webhook_url).json(&payload).send().await;
    });
}

pub fn predictive_resonance_tuning(current_tps: u64, active_waves: usize) -> String {
    let mut predicted_action = "SAGA: Testnet resonance stable.";

    if current_tps > 25_000 && active_waves > 8_000 {
        predicted_action =
            "🔮 SAGA PRECOGNITION: Testnet congestion building. Prefer smaller bounded batches.";
    }

    predicted_action.to_string()
}

pub fn authorize_gasless_transaction(user_address: &str, network_load_bps: u64) -> bool {
    if network_load_bps <= TESTNET_GASLESS_BPS_LIMIT {
        println!(
            "🧠 SAGA PAYMASTER: Transact fee sponsored for {}",
            user_address
        );
        return true;
    }
    false
}
