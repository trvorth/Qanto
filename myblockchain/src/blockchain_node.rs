use ed25519_dalek::SigningKey;

use my_blockchain::{
    difficulty_to_target, is_solution_valid, qanto_hash, Block, Blockchain, QantoHash,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Serialize, Deserialize)]
struct TxData {
    amount: u64, // in QNTO
}

fn calculate_reward(block_index: u64) -> f64 {
    let base = 50.0;
    base / (2.0f64).powi((block_index / 210_000) as i32)
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        env_logger::init();
        let keypair = SigningKey::generate(&mut rand::thread_rng());
        let blockchain = Arc::new(Blockchain::new("qanto_db").unwrap());

        // Start JSON-RPC server in background
        let _rpc_handle = blockchain.clone().start_jsonrpc(3030, 1);

        loop {
            let last_block = blockchain.get_last_block().await;
            let difficulty = 1u64; // Very low difficulty for debugging
            let target = difficulty_to_target(difficulty);
            let (tx_root, batches) = blockchain.execution_layer.create_block_payload();

            let state_root = blockchain
                .execute_block(&Block::new(
                    last_block.header.index + 1,
                    tx_root,
                    QantoHash::new([0; 32]),
                    last_block.get_hash(),
                    &keypair,
                    batches.clone(),
                ))
                .await;
            let header_hash = qanto_hash(&last_block.header.index.to_le_bytes());
            let mut nonce = rand::thread_rng().gen::<u64>();
            let mut nonce_bytes = [0u8; 32];
            nonce_bytes[0..8].copy_from_slice(&nonce.to_le_bytes());
            let mut input = header_hash.as_bytes().to_vec();
            input.extend_from_slice(&nonce_bytes);

            log::info!("Starting mining for block {}", last_block.header.index + 1);
            loop {
                log::info!("Trying nonce: {nonce}");
                let candidate = qanto_hash(&input);
                if is_solution_valid(candidate.as_bytes(), target) {
                    let block = Block::new(
                        last_block.header.index + 1,
                        tx_root,
                        state_root,
                        last_block.get_hash(),
                        &keypair,
                        batches.clone(),
                    );
                    match blockchain.add_block(block.clone()).await {
                        Ok(_) => {
                            let reward = calculate_reward(block.header.index);
                            log::info!(
                                "🎉 Block #{} Mined! | Hash: {:?} | 💰 Reward: {:.3} QANTO",
                                block.header.index,
                                block.get_hash(),
                                reward
                            );
                            break;
                        }
                        Err(e) => {
                            log::warn!("Failed to add block: {e:?}. Retrying with new state...");
                            break; // Break inner loop to recalculate state
                        }
                    }
                }
                nonce += 1;
                nonce_bytes[0..8].copy_from_slice(&nonce.to_le_bytes());
                input = header_hash.as_bytes().to_vec();
                input.extend_from_slice(&nonce_bytes);
                sleep(Duration::from_millis(1)).await;
            }
            log::info!("Block mined successfully");
        }
    });
}
