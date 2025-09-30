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
    let base = 150.0;
    base / (2.0f64).powi((block_index / 210_000) as i32)
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        env_logger::init();
        println!("🚀 Starting Qanto Blockchain Node...");
        log::info!("Initializing blockchain node");

        let keypair = SigningKey::generate(&mut rand::thread_rng());
        let blockchain = Arc::new(Blockchain::new("qanto_db").unwrap());

        println!("✅ Blockchain initialized");
        log::info!("Blockchain database initialized");

        // Start JSON-RPC server in background with configurable port (default 8545)
        let rpc_port = std::env::var("RPC_PORT")
            .unwrap_or_else(|_| "8545".to_string())
            .parse::<u16>()
            .unwrap_or(8545);

        println!("🌐 Starting JSON-RPC server on port {}...", rpc_port);
        let _rpc_handle = tokio::spawn({
            let blockchain = blockchain.clone();
            async move {
                blockchain.start_jsonrpc(rpc_port, 1).await
            }
        });
        println!("✅ JSON-RPC server started");

        println!("⛏️  Starting mining loop...");
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

            println!("🔨 Starting mining for block {}", last_block.header.index + 1);
            let mut attempts = 0;
            loop {
                attempts += 1;
                if attempts % 10000 == 0 {
                    println!("⚡ Mining attempt #{}: nonce {}", attempts, nonce);
                }
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
                            println!(
                                "🎉 Block #{} Mined! | Hash: {:?} | 💰 Reward: {:.3} QANTO | Attempts: {}",
                                block.header.index,
                                block.get_hash(),
                                reward,
                                attempts
                            );
                            break;
                        }
                        Err(e) => {
                            println!("❌ Failed to add block: {e:?}. Retrying with new state...");
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
            println!("✅ Block mined successfully, restarting mining loop...");
        }
    });
}
