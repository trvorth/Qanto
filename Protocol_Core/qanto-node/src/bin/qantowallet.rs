use qanto::persistence::{balance_key, decode_balance, decode_utxo, utxos_prefix};
use qanto::qanto_storage::QantoStorage;
use qanto::qantowallet;

pub fn get_balance_storage(
    address: &str,
    storage: &QantoStorage,
) -> Result<u64, Box<dyn std::error::Error>> {
    // Try to get cached balance first
    let key = balance_key(address);
    if let Ok(Some(cached_balance)) = storage.get(&key) {
        if let Ok(parsed_balance) = decode_balance(&cached_balance) {
            return Ok(parsed_balance);
        }
    }

    // Fallback to scanning UTXOs
    let prefix = utxos_prefix();
    let keys = storage.keys_with_prefix(&prefix)?;

    let mut total_balance = 0u64;
    for k in keys {
        if let Ok(Some(utxo_data)) = storage.get(&k) {
            if let Ok(utxo) = decode_utxo(&utxo_data) {
                if utxo.address == address {
                    total_balance = total_balance.saturating_add(utxo.amount);
                }
            }
        }
    }

    Ok(total_balance)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    qantowallet::run().await
}
