//! Service implementations for the Qanto Runtime Manager.
//!
//! # Reference Services (Phase 2.2)
//!
//! Lightweight services used to validate the Runtime Manager architecture:
//! - `metrics` — Heartbeat-only reference (NOT production)
//! - `telemetry` — Depends-on-metrics reference
//! - `snapshot` — Depends-on-telemetry reference
//! - `diagnostics` — Independent reference
//! - `failing` — Configurable failure injection for validation
//!
//! # Production Services (Phase 3)
//!
//! Real services that replace code in `node.rs`:
//! - `production_metrics` — Wraps QantoMetrics global singleton with lifecycle
//! - `production_diagnostics` — Wraps DiagnosticsEngine with lifecycle
//! - `production_telemetry` — Wraps telemetry functions with lifecycle (event-driven)
//! - `production_snapshot` — Wraps QantoStorage checkpoint logic with lifecycle
//! - `production_state_pruning` — Wraps state pruning logic with lifecycle
//! - `production_rpc` — Wraps gRPC RPC server with lifecycle
//! - `production_api` — Wraps Axum HTTP server (REST + GraphQL + WebSocket routes) with lifecycle
//! - `production_ipc` — Wraps IPC server (Unix socket / TCP) with lifecycle
//! - `production_p2p` — Wraps libp2p P2PServer event loop with lifecycle (monolithic swarm)

pub mod diagnostics;
pub mod failing;
pub mod metrics;
pub mod production_api;
pub mod production_diagnostics;
pub mod production_ipc;
pub mod production_metrics;
pub mod production_p2p;
pub mod production_rpc;
pub mod production_snapshot;
pub mod production_state_pruning;
pub mod production_telemetry;
pub mod snapshot;
pub mod telemetry;
