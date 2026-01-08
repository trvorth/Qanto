//! UTXO snapshot utilities
//! Provides deterministic snapshot_id computation for a set of UTXOs.
//!
//! Determinism rules:
//! - Canonical ordering: sort entries by `utxo_id` ascending.
//! - Canonical serialization: concatenate fields in a fixed order with binary encodings
//!   for numeric fields (`output_index` and `amount`) using little-endian.
//! - Hash function: use QanHash via `qanto_core::qanto_native_crypto::qanto_hash` and hex-encode bytes.
//!
//! NOTE: This module is intentionally minimal and does not depend on storage traits.
//! It can be used anywhere a snapshot_id is required (mempool, DAG, tests).

use crate::types::UTXO;
use std::collections::HashMap;

/// Compute a deterministic `snapshot_id` for a UTXO set.
///
/// Executor context: synchronous utility; does not require Tokio.
///
/// Canonical input and encoding:
/// - Sort by `utxo_id` (key of the map) ascending to enforce determinism
/// - For each entry, append bytes in this order:
///   - `utxo_id` (UTF-8)
///   - 0x00 separator
///   - `tx_id` (UTF-8)
///   - 0x00 separator
///   - `output_index` (u32 little-endian)
///   - `address` (UTF-8)
///   - 0x00 separator
///   - `amount` (u128 little-endian)
///
/// Returns: lowercase hex string of QanHash digest bytes
pub fn compute_utxo_snapshot_id(utxos: &HashMap<String, UTXO>) -> String {
    // Pre-size buffer: rough heuristic to avoid frequent reallocations
    let approx_capacity = utxos.len() * 96;
    let mut buf = Vec::with_capacity(approx_capacity);

    let mut keys: Vec<&String> = utxos.keys().collect();
    keys.sort();

    for key in keys {
        if let Some(u) = utxos.get(key) {
            // utxo_id
            buf.extend_from_slice(key.as_bytes());
            buf.push(0);
            // tx_id
            buf.extend_from_slice(u.tx_id.as_bytes());
            buf.push(0);
            // output_index
            buf.extend_from_slice(&u.output_index.to_le_bytes());
            // address
            buf.extend_from_slice(u.address.as_bytes());
            buf.push(0);
            // amount
            buf.extend_from_slice(&u.amount.to_le_bytes());
        }
    }

    let digest = qanto_core::qanto_native_crypto::qanto_hash(&buf);
    hex::encode(digest.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_id_is_deterministic_over_ordering() {
        let mut a = HashMap::new();
        let mut b = HashMap::new();

        let u1 = UTXO {
            address: "A1".to_string(),
            amount: 100u64,
            tx_id: "tx-1".to_string(),
            output_index: 0,
            explorer_link: String::new(),
        };
        let u2 = UTXO {
            address: "B2".to_string(),
            amount: 250u64,
            tx_id: "tx-2".to_string(),
            output_index: 1,
            explorer_link: String::new(),
        };

        a.insert("utxo-1".to_string(), u1.clone());
        a.insert("utxo-2".to_string(), u2.clone());

        // Different insertion order
        b.insert("utxo-2".to_string(), u2);
        b.insert("utxo-1".to_string(), u1);

        let id_a = compute_utxo_snapshot_id(&a);
        let id_b = compute_utxo_snapshot_id(&b);
        assert_eq!(id_a, id_b);
    }
}
