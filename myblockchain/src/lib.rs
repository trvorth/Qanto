//! The core library for the Qanto blockchain.
//! This library defines all the fundamental components, including the custom
//! proof-of-work algorithm, Qanhash.

use chrono::Utc;
use log::{info, warn};
use num_bigint::BigUint;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// --- Qanhash Proof-of-Work Module ---
// This module contains the entire implementation of the Qanhash algorithm.
mod qanhash {
    use super::*;
    use blake3::Hasher;
    use lazy_static::lazy_static;
    use rand::rngs::SmallRng;
    use rand::{RngCore, SeedableRng};
    use std::sync::RwLock;

    // --- Type alias to fix clippy::type_complexity warning ---
    type QDagCacheEntry = Option<(u64, Arc<Vec<[u8; MIX_BYTES]>>)>;

    // --- Constants for the Q-DAG ---
    const DATASET_INIT_SIZE: usize = 1 << 24; // 16 MiB to start
    const DATASET_GROWTH_EPOCH: u64 = 10_000; // New DAG every 10,000 blocks
    const MIX_BYTES: usize = 128; // Size of the mix hash
    const ACCESSES: usize = 64; // Number of random accesses into the dataset

    // --- Constants for Post-Quantum Polynomial Hashing ---
    const POLY_DEGREE: usize = 256; // n
    const POLY_MODULUS: i32 = 7681; // q

    lazy_static! {
        // A global cache for the current Q-DAG to avoid re-computation.
        static ref QDAG_CACHE: RwLock<QDagCacheEntry> = RwLock::new(None);
    }

    /// STAGE 1: The Q-Sponge (Primary Hashing)
    fn q_sponge(input: &[u8]) -> [u8; 32] {
        let mut state = [0u8; 32];
        for chunk in input.chunks(16) {
            for (i, &byte) in chunk.iter().enumerate() {
                state[i] ^= byte;
            }
            state.rotate_left(3);
            for i in 0..32 {
                state[i] = state[i].wrapping_add(state[(i + 1) % 32]).wrapping_mul(17);
            }
        }
        state
    }

    /// STAGE 2: The Q-DAG (Memory-Hard Expansion)
    fn generate_qdag(seed: [u8; 32], epoch: u64) -> Arc<Vec<[u8; MIX_BYTES]>> {
        info!("Generating new Q-DAG for epoch {epoch}...");
        let cache_size = DATASET_INIT_SIZE + (epoch as usize * 128);
        let mut dataset = vec![[0u8; MIX_BYTES]; cache_size];

        // **FIXED**: Use BLAKE3 XOF to generate a 128-byte hash
        let mut first_item_hasher = Hasher::new();
        first_item_hasher.update(&seed);
        let mut xof_reader = first_item_hasher.finalize_xof();
        xof_reader.fill(&mut dataset[0]);

        for i in 1..cache_size {
            let mut hasher = Hasher::new();
            hasher.update(&dataset[i - 1]);
            let mut xof_reader = hasher.finalize_xof();
            xof_reader.fill(&mut dataset[i]);
        }

        info!(
            "Q-DAG generation complete. Size: {} MB",
            (cache_size * MIX_BYTES) / (1024 * 1024)
        );
        Arc::new(dataset)
    }

    /// Ensures the correct Q-DAG is loaded into the cache.
    fn get_qdag(block_index: u64) -> Arc<Vec<[u8; MIX_BYTES]>> {
        let epoch = block_index / DATASET_GROWTH_EPOCH;
        let mut cache = QDAG_CACHE.write().unwrap();

        if let Some((cached_epoch, dag)) = &*cache {
            if *cached_epoch == epoch {
                return dag.clone();
            }
        }

        let seed = q_sponge(&epoch.to_le_bytes());
        let new_dag = generate_qdag(seed, epoch);
        *cache = Some((epoch, new_dag.clone()));
        new_dag
    }

    /// STAGE 3: Post-Quantum Flavor (Polynomial Hashing)
    fn poly_hash(input: &[u8]) -> [i32; POLY_DEGREE] {
        let mut coeffs = [0i32; POLY_DEGREE];
        let mut rng = SmallRng::from_seed(q_sponge(input));

        // **FIXED**: Use an iterator to satisfy clippy::needless_range_loop
        for coeff in coeffs.iter_mut() {
            *coeff = (rng.next_u32() as i32) % POLY_MODULUS;
        }
        coeffs
    }

    /// The main Qanhash computation function.
    pub fn hash(header: &[u8], nonce: u64) -> (String, [i32; POLY_DEGREE]) {
        let mut nonce_bytes = nonce.to_le_bytes().to_vec();
        nonce_bytes.extend_from_slice(header);
        let seed = q_sponge(&nonce_bytes);
        let block_index_bytes = &header[0..8];
        let block_index = u64::from_le_bytes(block_index_bytes.try_into().unwrap());
        let dag = get_qdag(block_index);

        let mut mix = [0u8; MIX_BYTES];
        mix[..32].copy_from_slice(&seed);

        for _ in 0..ACCESSES {
            let mut hasher = Hasher::new();
            hasher.update(&mix);
            let p_index_bytes = hasher.finalize().as_bytes()[..8].try_into().unwrap();
            let p_index = u64::from_le_bytes(p_index_bytes);
            let lookup_index = p_index as usize % dag.len();

            // **FIXED**: Use an iterator to satisfy clippy::needless_range_loop
            for (i, mix_item) in mix.iter_mut().enumerate() {
                *mix_item = mix_item.wrapping_mul(31) ^ dag[lookup_index][i];
            }
        }

        let final_poly_hash = poly_hash(&mix);
        let final_pow_hash = hex::encode(blake3::hash(&mix).as_bytes());
        (final_pow_hash, final_poly_hash)
    }

    /// Verifies the polynomial hash component.
    pub fn verify_poly_hash(
        header: &[u8],
        nonce: u64,
        expected_poly_hash: &[i32; POLY_DEGREE],
    ) -> bool {
        let (_, calculated_poly_hash) = hash(header, nonce);
        &calculated_poly_hash == expected_poly_hash
    }
}

// --- Core Blockchain Data Structures ---
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Output {
    pub value: u64,
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Input {
    pub tx_id: String,
    pub output_index: u32,
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Transaction {
    pub id: String,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64,
    pub difficulty: u64,
    #[serde(with = "serde_big_array::BigArray")]
    pub poly_hash: [i32; 256],
}

impl Block {
    pub fn new(
        index: u64,
        transactions: Vec<Transaction>,
        previous_hash: String,
        difficulty: u64,
    ) -> Self {
        let timestamp = Utc::now().timestamp();
        let mut block = Block {
            index,
            timestamp,
            transactions,
            previous_hash,
            hash: String::new(),
            nonce: 0,
            difficulty,
            poly_hash: [0; 256],
        };
        block.mine();
        block
    }

    /// Prepares a concise byte representation of the block header for hashing.
    fn get_header_data(&self) -> Vec<u8> {
        let mut data = self.index.to_le_bytes().to_vec();
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(self.previous_hash.as_bytes());
        for tx in &self.transactions {
            data.extend_from_slice(tx.id.as_bytes());
        }
        data
    }

    /// Mines the block using the Qanhash algorithm.
    fn mine(&mut self) {
        let target = BigUint::from(2u32).pow(256) / BigUint::from(self.difficulty);
        let header_data = self.get_header_data();
        let mut nonce = 0u64;

        loop {
            let (hash_hex, poly_hash) = qanhash::hash(&header_data, nonce);
            let hash_int = BigUint::from_bytes_be(&hex::decode(&hash_hex).unwrap());

            if hash_int < target {
                self.hash = hash_hex;
                self.poly_hash = poly_hash;
                self.nonce = nonce;
                info!(
                    "Block #{} mined with nonce {}. Hash: {}...",
                    self.index,
                    self.nonce,
                    &self.hash[..10]
                );
                return;
            }
            nonce += 1;
        }
    }

    /// Verifies the block's proof-of-work.
    fn verify_pow(&self) -> bool {
        let target = BigUint::from(2u32).pow(256) / BigUint::from(self.difficulty);
        let hash_int = BigUint::from_bytes_be(&hex::decode(&self.hash).unwrap());

        if hash_int >= target {
            return false;
        }

        qanhash::verify_poly_hash(&self.get_header_data(), self.nonce, &self.poly_hash)
    }
}

// --- Blockchain Management ---
pub struct Blockchain {
    pub chain: Arc<Mutex<Vec<Block>>>,
    db: Arc<DB>,
}

impl Blockchain {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let db = Arc::new(DB::open_default(db_path).map_err(|e| e.to_string())?);
        let chain = match db.get(b"chain") {
            Ok(Some(data)) => serde_json::from_slice(&data).unwrap_or_else(|_| vec![]),
            _ => vec![],
        };

        let arc_chain = Arc::new(Mutex::new(chain));

        if arc_chain.lock().unwrap().is_empty() {
            info!("Creating genesis block...");
            let genesis_block = Block::new(0, vec![], "0".to_string(), 1_000_000);
            arc_chain.lock().unwrap().push(genesis_block);
        }

        Ok(Blockchain {
            chain: arc_chain,
            db,
        })
    }

    pub fn add_block(&self, transactions: Vec<Transaction>) {
        let mut chain = self.chain.lock().unwrap();
        let previous_hash = chain.last().unwrap().hash.clone();
        let new_block = Block::new(
            chain.len() as u64,
            transactions,
            previous_hash,
            chain.last().unwrap().difficulty,
        );
        chain.push(new_block);
        self.save_chain(&chain);
    }

    fn save_chain(&self, chain: &[Block]) {
        if let Ok(serialized_chain) = serde_json::to_string(chain) {
            self.db.put(b"chain", serialized_chain.as_bytes()).unwrap();
        }
    }

    pub fn is_valid(&self) -> bool {
        let chain = self.chain.lock().unwrap();
        for i in 1..chain.len() {
            let current = &chain[i];
            let previous = &chain[i - 1];

            if current.previous_hash != previous.hash {
                warn!("Chain link broken at Block #{}", current.index);
                return false;
            }

            if !current.verify_pow() {
                warn!("Proof-of-work is invalid for Block #{}", current.index);
                return false;
            }
        }
        true
    }
}

// --- Concurrency ---
pub fn start_mining_thread(
    blockchain: Arc<Blockchain>,
    transactions: Arc<Mutex<Vec<Transaction>>>,
) {
    thread::spawn(move || loop {
        let mut pending_tx = transactions.lock().unwrap();
        if !pending_tx.is_empty() {
            let tx_to_add = pending_tx.drain(..).collect();
            drop(pending_tx);
            blockchain.add_block(tx_to_add);
        }
        thread::sleep(Duration::from_secs(5));
    });
}
