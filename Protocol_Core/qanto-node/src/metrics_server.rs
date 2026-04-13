// src/metrics_server.rs

use axum::{extract::State, response::Json, routing::get, Router};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::metrics::{get_global_metrics, QantoMetrics};

/// Shared state for the metrics server
#[derive(Clone)]
struct MetricsState {
    metrics: Arc<QantoMetrics>,
}

/// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Prometheus metrics endpoint
async fn metrics_endpoint(State(state): State<MetricsState>) -> String {
    state.metrics.export_prometheus()
}

/// Performance metrics endpoint
async fn performance_endpoint(State(state): State<MetricsState>) -> Json<serde_json::Value> {
    let metrics = &state.metrics;
    Json(json!({
        "tps": metrics.get_tps(),
        "bps": metrics.get_bps(),
        "memory_mb": metrics.get_memory_utilization(),
        "cpu_percent": metrics.get_cpu_utilization(),
        "network_mbps": metrics.get_network_throughput(),
        "latency_ms": metrics.get_average_latency(),
        "cache_hit_ratio": metrics.get_cache_hit_ratio(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Real-time TPS endpoint
async fn tps_endpoint(State(state): State<MetricsState>) -> Json<serde_json::Value> {
    let metrics = &state.metrics;
    Json(json!({
        "tps_real_time": metrics.calculate_real_time_tps(),
        "tps_configured": metrics.get_tps(),
        "bps": metrics.get_bps(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// System resources endpoint
async fn resources_endpoint(State(state): State<MetricsState>) -> Json<serde_json::Value> {
    let metrics = &state.metrics;
    Json(json!({
        "memory_utilization_percent": metrics.get_memory_utilization(),
        "cpu_utilization_percent": metrics.get_cpu_utilization(),
        "network_throughput_mbps": metrics.get_network_throughput(),
        "average_latency_ms": metrics.get_average_latency(),
        "cache_hit_ratio": metrics.get_cache_hit_ratio(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Create shared state
    let state = MetricsState {
        metrics: get_global_metrics(),
    };

    // Create router with all endpoints
    let app = Router::new()
        .route("/metrics", get(metrics_endpoint))
        .route("/performance", get(performance_endpoint))
        .route("/health", get(health_check))
        .route("/tps", get(tps_endpoint))
        .route("/resources", get(resources_endpoint))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Bind to address
    let listener = TcpListener::bind("0.0.0.0:9090").await?;

    println!("Metrics server running on http://0.0.0.0:9090");
    println!("Available endpoints:");
    println!("  - /metrics (Prometheus format)");
    println!("  - /performance (JSON summary)");
    println!("  - /tps (Real-time TPS)");
    println!("  - /resources (System resources)");
    println!("  - /health (Health check)");

    // Start the server
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // Placeholder test to prevent test runner warnings
    }
}
