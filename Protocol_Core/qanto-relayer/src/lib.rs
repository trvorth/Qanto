//! QANTO Cross-Chain Relayer Daemon
//!
//! Polls an EVM JSON-RPC endpoint for bridge lock events and forwards validated claim payloads.

#![allow(
    clippy::multiple_crate_versions,
    clippy::wildcard_imports,
    clippy::missing_errors_doc,
    clippy::doc_markdown,
    clippy::used_underscore_items,
    clippy::default_trait_access,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::use_self,
    clippy::option_if_let_else,
    clippy::single_match_else,
    clippy::module_name_repetitions
)]

mod adapters;
mod config;
mod coordinator;
mod error;
mod ports;
mod runtime;

pub use config::RelayerConfig;
pub use error::{RelayerError, RelayerResult};
pub use runtime::RuntimeBuilder;

/// Entry point for the relayer daemon.
pub async fn run() -> RelayerResult<()> {
    let config = RelayerConfig::from_env()?;

    let runtime = RuntimeBuilder::new(config).build().await?;
    runtime.run().await
}
