//! Offline, read-only canonical genesis commitment calculator.

use anyhow::{bail, Context, Result};
use qanto::absolute_genesis::{compute_genesis_commitments, GenesisDocument};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut arguments = std::env::args_os();
    let program = arguments
        .next()
        .unwrap_or_else(|| "genesis_manifest".into());
    let Some(path) = arguments.next() else {
        bail!("usage: {} <genesis.json>", PathBuf::from(program).display());
    };
    if arguments.next().is_some() {
        bail!("exactly one genesis manifest path is required");
    }

    let path = PathBuf::from(path);
    let raw = std::fs::read(&path)
        .with_context(|| format!("failed to read genesis manifest '{}'", path.display()))?;
    let document: GenesisDocument = serde_json::from_slice(&raw)
        .with_context(|| format!("failed to parse genesis manifest '{}'", path.display()))?;
    let (manifest_hash, allocation_state_root) = compute_genesis_commitments(&document)
        .with_context(|| format!("genesis manifest '{}' is invalid", path.display()))?;

    println!("genesis_manifest_hash={manifest_hash}");
    println!("allocation_state_root={allocation_state_root}");
    Ok(())
}
