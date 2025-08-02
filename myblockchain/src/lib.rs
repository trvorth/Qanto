//! The core library for the Qanto blockchain.
//! This library defines all the fundamental components, including the custom
//! proof-of-work algorithm, Qanhash, and is architected to evolve
//! towards a high-throughput system of 32 BPS and 100k+ TPS.

use atomic_shim::AtomicU64;
use bincode;
use blake3::Hasher;
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use log::info;
use num_bigint::BigUint;
use num_cpus;
use rayon::prelude::*;
use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// GPU-specific imports
#[cfg(feature = "gpu")]
use ocl::{builders::ProgramBuilder, flags, Buffer, Context, Device, Kernel, Platform, Queue};

// --- System Constants ---
const TARGET_BLOCK_TIME_NS: i64 = 1_000_000_000; // 1 second
const DIFFICULTY_ADJUSTMENT_WINDOW: usize = 64;
const TXS_PER_BLOCK: usize = 3125;

// --- Qanhash Proof-of-Work Module (Unchanged) ---
pub mod qanhash {
    use super::*;
    use lazy_static::lazy_static;
    use std::sync::RwLock;

    type QDagCacheEntry = Option<(u64, Arc<Vec<[u8; MIX_BYTES]>>)>;

    const DATASET_INIT_SIZE: usize = 1 << 24;
    const DATASET_GROWTH_EPOCH: u64 = 10_000;
    pub const MIX_BYTES: usize = 128;

    #[cfg(feature = "gpu")]
    pub struct GpuContext {
        pub context: Context,
        pub queue: Queue,
        pub program: ocl::Program,
    }

    #[cfg(feature = "gpu")]
    lazy_static! {
        pub static ref GPU_CONTEXT: Mutex<GpuContext> = {
            info!("Performing one-time initialization of GPU context...");
            let platform = Platform::default();
            let device = Device::first(platform).expect("No OpenCL device found");
            info!("Using GPU: {}", device.name().unwrap_or_default());
            let context = Context::builder().devices(device).build().unwrap();
            let queue = Queue::new(&context, device, None).unwrap();
            let kernel_src = r#"
                ulong8 mix_round(ulong8 val, ulong8 dag_val) {
                    val = val * 31 + dag_val;
                    val = rotate(val, (ulong8)(11, 7, 3, 1, 13, 5, 2, 9));
                    return val ^ dag_val;
                }
                __kernel void qanhash_kernel(
                    __global const uchar* header_hash, ulong start_nonce, __global const ulong8* q_dag,
                    ulong dag_len_mask, __global const uchar* target,
                    __global volatile uint* result_gid, __global uchar* result_hash
                ) {
                    uint gid = get_global_id(0);
                    ulong nonce = start_nonce + gid;
                    if (result_gid[0] != 0xFFFFFFFF) return;
                    ulong8 mix[2];
                    mix[0] = (ulong8)(
                        (ulong)header_hash[0] | (ulong)header_hash[1] << 8 | (ulong)header_hash[2] << 16 | (ulong)header_hash[3] << 24 | (ulong)header_hash[4] << 32 | (ulong)header_hash[5] << 40 | (ulong)header_hash[6] << 48 | (ulong)header_hash[7] << 56,
                        (ulong)header_hash[8] | (ulong)header_hash[9] << 8 | (ulong)header_hash[10] << 16 | (ulong)header_hash[11] << 24 | (ulong)header_hash[12] << 32 | (ulong)header_hash[13] << 40 | (ulong)header_hash[14] << 48 | (ulong)header_hash[15] << 56,
                        (ulong)header_hash[16] | (ulong)header_hash[17] << 8 | (ulong)header_hash[18] << 16 | (ulong)header_hash[19] << 24 | (ulong)header_hash[20] << 32 | (ulong)header_hash[21] << 40 | (ulong)header_hash[22] << 48 | (ulong)header_hash[23] << 56,
                        (ulong)header_hash[24] | (ulong)header_hash[25] << 8 | (ulong)header_hash[26] << 16 | (ulong)header_hash[27] << 24 | (ulong)header_hash[28] << 32 | (ulong)header_hash[29] << 40 | (ulong)header_hash[30] << 48 | (ulong)header_hash[31] << 56,
                        nonce, 0, 0, 0
                    );
                    mix[1] = (ulong8)(0);
                    __local ulong8 local_dag_cache[512];
                    for (int i = 0; i < 64; ++i) {
                        ulong p_index = mix[0].s0 * 11400714819323198485UL;
                        ulong lookup_base = (p_index & (dag_len_mask >> 1)) * 2;
                        barrier(CLK_LOCAL_MEM_FENCE);
                        event_t e = async_work_group_copy(local_dag_cache, &q_dag[lookup_base], 512, 0);
                        wait_group_events(1, &e);
                        barrier(CLK_LOCAL_MEM_FENCE);
                        ulong lookup_index_local = (p_index & 511);
                        mix[0] = mix_round(mix[0], local_dag_cache[lookup_index_local]);
                        mix[1] = mix_round(mix[1], local_dag_cache[lookup_index_local + 1]);
                    }
                    __private uchar* final_hash_ptr = (__private uchar*)mix;
                    for (int i = 31; i >= 0; --i) {
                        if (final_hash_ptr[i] < target[i]) {
                            if (atom_cmpxchg(result_gid, 0xFFFFFFFF, gid) == 0xFFFFFFFF) {
                                for(int j = 0; j < 32; ++j) result_hash[j] = final_hash_ptr[j];
                            }
                            return;
                        }
                        if (final_hash_ptr[i] > target[i]) return;
                    }
                    if (atom_cmpxchg(result_gid, 0xFFFFFFFF, gid) == 0xFFFFFFFF) {
                        for(int j = 0; j < 32; ++j) result_hash[j] = final_hash_ptr[j];
                    }
                }
            "#;
            let program = ProgramBuilder::new()
                .source(kernel_src)
                .build(&context)
                .unwrap();
            info!("GPU context and kernel compiled successfully.");
            Mutex::new(GpuContext {
                context,
                queue,
                program,
            })
        };
    }

    lazy_static! {
        static ref QDAG_CACHE: RwLock<QDagCacheEntry> = RwLock::new(None);
    }

    pub fn get_qdag(block_index: u64) -> Arc<Vec<[u8; MIX_BYTES]>> {
        let epoch = block_index / DATASET_GROWTH_EPOCH;
        let read_cache = QDAG_CACHE.read().expect("Q-DAG cache read lock poisoned");
        if let Some((cached_epoch, dag)) = &*read_cache {
            if *cached_epoch == epoch {
                return dag.clone();
            }
        }
        drop(read_cache);
        let mut write_cache = QDAG_CACHE.write().expect("Q-DAG cache write lock poisoned");
        if let Some((cached_epoch, dag)) = &*write_cache {
            if *cached_epoch == epoch {
                return dag.clone();
            }
        }
        let seed = blake3::hash(&epoch.to_le_bytes());
        let new_dag = generate_qdag(seed.as_bytes(), epoch);
        *write_cache = Some((epoch, new_dag.clone()));
        new_dag
    }

    fn generate_qdag(seed: &[u8; 32], epoch: u64) -> Arc<Vec<[u8; MIX_BYTES]>> {
        let cache_size = DATASET_INIT_SIZE + (epoch.min(1000) as usize * 128);
        let mut dataset = vec![[0u8; MIX_BYTES]; cache_size];
        let mut hasher = Hasher::new();
        hasher.update(seed);
        hasher.finalize_xof().fill(&mut dataset[0]);
        for i in 1..cache_size {
            let (prev_slice, current_slice) = dataset.split_at_mut(i);
            let prev_item = &prev_slice[i - 1];
            let current_item = &mut current_slice[0];
            let mut item_hasher = Hasher::new();
            item_hasher.update(prev_item);
            item_hasher.finalize_xof().fill(current_item);
        }
        Arc::new(dataset)
    }

    pub fn hash(header_hash: &blake3::Hash, nonce: u64) -> [u8; 32] {
        let block_index = u64::from_le_bytes(header_hash.as_bytes()[0..8].try_into().unwrap());
        let dag = get_qdag(block_index);
        let dag_len = dag.len();
        let mut mix = [0u64; MIX_BYTES / 8];
        let header_u64s: [u64; 4] = unsafe { std::mem::transmute(*header_hash.as_bytes()) };
        mix[0..4].copy_from_slice(&header_u64s);
        mix[4] = nonce;
        for _ in 0..64 {
            let p_index = mix[0].wrapping_mul(11400714819323198485);
            let lookup_index = p_index as usize % dag_len;
            let dag_entry: &[u64; 16] =
                unsafe { &*(dag[lookup_index].as_ptr() as *const [u64; 16]) };
            for i in 0..16 {
                let val = mix[i].wrapping_mul(31).wrapping_add(dag_entry[i]);
                mix[i] = val.rotate_left(((i % 8) + 1) as u32) ^ dag_entry[i];
            }
        }
        let mix_bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(mix.as_ptr() as *const u8, MIX_BYTES) };
        mix_bytes[..32]
            .try_into()
            .expect("Final hash slice has incorrect length")
    }
}

// --- Core Blockchain Data Structures ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: blake3::Hash,
    pub message: Vec<u8>,
    pub public_key: VerifyingKey,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransactionProof {
    pub root_hash: blake3::Hash,
    pub transaction_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transaction_proof: TransactionProof,
    pub previous_hash: blake3::Hash,
    pub hash: blake3::Hash,
    pub nonce: u64,
    pub difficulty: u64,
}

impl Block {
    pub fn new(
        index: u64,
        proof: TransactionProof,
        previous_hash: blake3::Hash,
        difficulty: u64,
    ) -> Self {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let mut block = Block {
            index,
            timestamp,
            transaction_proof: proof,
            previous_hash,
            hash: blake3::Hash::from_bytes([0; 32]),
            nonce: 0,
            difficulty,
        };
        block.mine();
        block
    }

    pub fn get_header_hash(&self) -> blake3::Hash {
        let mut hasher = Hasher::new();
        hasher.update(&self.index.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(self.previous_hash.as_bytes());
        hasher.update(self.transaction_proof.root_hash.as_bytes());
        hasher.finalize()
    }

    fn mine(&mut self) {
        #[cfg(not(feature = "gpu"))]
        self.mine_cpu();
        #[cfg(feature = "gpu")]
        self.mine_gpu();
    }

    // FIX: Acknowledge that this function is unused when compiling with the `gpu` feature.
    #[allow(dead_code)]
    fn mine_cpu(&mut self) {
        let start = Instant::now();
        let target_u256 = BigUint::from(2u32).pow(256) / BigUint::from(self.difficulty);
        let mut target_bytes = [0u8; 32];
        let be_bytes = target_u256.to_bytes_be();
        target_bytes[(32 - be_bytes.len())..].copy_from_slice(&be_bytes);

        let header_hash_arc = Arc::new(self.get_header_hash());
        let found_nonce = Arc::new(AtomicU64::new(u64::MAX));
        let found_hash_bytes = Arc::new(Mutex::new([0u8; 32]));
        let solution_found = Arc::new(AtomicBool::new(false));

        (0..num_cpus::get() as u64).into_par_iter().for_each(|i| {
            let mut nonce = i;
            while !solution_found.load(Ordering::Relaxed) {
                let hash_bytes = qanhash::hash(&header_hash_arc, nonce);
                if hash_bytes < target_bytes {
                    if found_nonce
                        .compare_exchange(u64::MAX, nonce, Ordering::SeqCst, Ordering::Relaxed)
                        .is_ok()
                    {
                        *found_hash_bytes.lock().unwrap() = hash_bytes;
                        solution_found.store(true, Ordering::Relaxed);
                    }
                    return;
                }
                nonce += num_cpus::get() as u64;
            }
        });

        self.nonce = found_nonce.load(Ordering::Relaxed);
        self.hash = blake3::Hash::from(*found_hash_bytes.lock().unwrap());
        info!(
            "CPU mining found solution in {:?}. Nonce: {}",
            start.elapsed(),
            self.nonce
        );
    }

    #[cfg(feature = "gpu")]
    fn mine_gpu(&mut self) {
        let start = Instant::now();
        let gpu = qanhash::GPU_CONTEXT.lock().unwrap();
        let target_u256 = BigUint::from(2u32).pow(256) / BigUint::from(self.difficulty);
        let mut target_bytes = [0u8; 32];
        let be_bytes = target_u256.to_bytes_be();
        target_bytes[(32 - be_bytes.len())..].copy_from_slice(&be_bytes);

        let header_hash = self.get_header_hash();
        let dag = qanhash::get_qdag(self.index);
        let dag_len_mask = (dag.len() as u64 / 2).next_power_of_two() - 1;
        let dag_flat: &[u8] = unsafe { dag.align_to::<u8>().1 };

        let header_buffer = Buffer::builder()
            .queue(gpu.queue.clone())
            .flags(flags::MEM_COPY_HOST_PTR)
            .len(32)
            .copy_host_slice(header_hash.as_bytes())
            .build()
            .unwrap();
        let dag_buffer = Buffer::builder()
            .queue(gpu.queue.clone())
            .flags(flags::MEM_COPY_HOST_PTR)
            .len(dag_flat.len())
            .copy_host_slice(dag_flat)
            .build()
            .unwrap();
        let target_buffer = Buffer::builder()
            .queue(gpu.queue.clone())
            .flags(flags::MEM_COPY_HOST_PTR)
            .len(32)
            .copy_host_slice(&target_bytes)
            .build()
            .unwrap();
        let result_gid_buffer: Buffer<u32> = Buffer::builder()
            .queue(gpu.queue.clone())
            .flags(flags::MEM_READ_WRITE)
            .len(1)
            .build()
            .unwrap();
        let result_hash_buffer: Buffer<u8> = Buffer::builder()
            .queue(gpu.queue.clone())
            .flags(flags::MEM_WRITE_ONLY)
            .len(32)
            .build()
            .unwrap();

        let mut start_nonce = 0u64;
        let batch_size: u64 = 1_048_576 * 8;
        const WORK_GROUP_SIZE: u64 = 256;

        loop {
            result_gid_buffer
                .write(&[u32::MAX] as &[u32])
                .enq()
                .unwrap();
            let kernel = Kernel::builder()
                .program(&gpu.program)
                .name("qanhash_kernel")
                .queue(gpu.queue.clone())
                .global_work_size((batch_size,))
                .local_work_size((WORK_GROUP_SIZE,))
                .arg(&header_buffer)
                .arg(start_nonce)
                .arg(&dag_buffer)
                .arg(dag_len_mask)
                .arg(&target_buffer)
                .arg(&result_gid_buffer)
                .arg(&result_hash_buffer)
                .build()
                .unwrap();
            unsafe {
                kernel.enq().unwrap();
            }
            gpu.queue.finish().unwrap();
            let mut result_gid = [0u32; 1];
            result_gid_buffer.read(&mut result_gid[..]).enq().unwrap();
            if result_gid[0] != u32::MAX {
                let winning_nonce = start_nonce + result_gid[0] as u64;
                let mut winning_hash = [0u8; 32];
                result_hash_buffer
                    .read(&mut winning_hash[..])
                    .enq()
                    .unwrap();
                self.nonce = winning_nonce;
                self.hash = blake3::Hash::from(winning_hash);
                info!(
                    "GPU mining found solution in {:?}. Nonce: {}",
                    start.elapsed(),
                    self.nonce
                );
                break;
            }
            start_nonce += batch_size;
        }
    }
}

pub struct ExecutionLayer {
    mempool: Arc<Mutex<VecDeque<Transaction>>>,
}

impl ExecutionLayer {
    pub fn new() -> Self {
        Self {
            mempool: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn add_transaction(&self, tx: Transaction) {
        self.mempool.lock().unwrap().push_back(tx);
    }

    pub fn create_block_payload(&self) -> Option<TransactionProof> {
        let mut mempool = self.mempool.lock().unwrap();
        if mempool.is_empty() {
            return None;
        }

        let batch_size = TXS_PER_BLOCK.min(mempool.len());
        let transactions: Vec<Transaction> = mempool.drain(0..batch_size).collect();
        drop(mempool);

        let verification_results: Vec<_> = transactions
            .par_iter()
            .map(|tx| tx.public_key.verify(&tx.message, &tx.signature).is_ok())
            .collect();

        let valid_txs: Vec<_> = transactions
            .into_iter()
            .zip(verification_results.into_iter())
            .filter_map(|(tx, is_valid)| if is_valid { Some(tx) } else { None })
            .collect();

        if valid_txs.is_empty() {
            return None;
        }

        let tx_hashes: Vec<_> = valid_txs.par_iter().map(|tx| tx.id).collect();

        let mut root_hasher = Hasher::new();
        for hash in &tx_hashes {
            root_hasher.update(hash.as_bytes());
        }
        let root_hash = root_hasher.finalize();

        Some(TransactionProof {
            root_hash,
            transaction_count: valid_txs.len() as u32,
        })
    }
}

pub struct Blockchain {
    pub chain: Arc<Mutex<Vec<Block>>>,
    db: Arc<DB>,
    pub execution_layer: Arc<ExecutionLayer>,
    block_timestamps: Arc<Mutex<VecDeque<i64>>>,
}

impl Blockchain {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        let db = Arc::new(DB::open(&db_opts, db_path).map_err(|e| e.to_string())?);

        let genesis_proof = TransactionProof {
            root_hash: blake3::Hash::from_bytes([0; 32]),
            transaction_count: 0,
        };
        let genesis_block =
            Block::new(0, genesis_proof, blake3::Hash::from_bytes([0; 32]), 100_000);

        let mut timestamps = VecDeque::with_capacity(DIFFICULTY_ADJUSTMENT_WINDOW);
        timestamps.push_back(genesis_block.timestamp);

        let blockchain = Blockchain {
            chain: Arc::new(Mutex::new(vec![genesis_block.clone()])),
            db,
            execution_layer: Arc::new(ExecutionLayer::new()),
            block_timestamps: Arc::new(Mutex::new(timestamps)),
        };
        blockchain.save_block(&genesis_block)?;
        Ok(blockchain)
    }

    fn get_next_difficulty(&self, current_difficulty: u64, current_index: u64) -> u64 {
        if current_index < DIFFICULTY_ADJUSTMENT_WINDOW as u64 {
            return current_difficulty;
        }

        let timestamps = self.block_timestamps.lock().unwrap();
        let actual_time_ns = (timestamps.back().unwrap() - timestamps.front().unwrap()) as u128;
        let target_time_ns =
            (TARGET_BLOCK_TIME_NS as u128) * (DIFFICULTY_ADJUSTMENT_WINDOW as u128 - 1);

        if actual_time_ns == 0 {
            return current_difficulty.saturating_mul(2);
        }

        let current_difficulty_bi = BigUint::from(current_difficulty);
        let new_difficulty_bi = current_difficulty_bi * target_time_ns / actual_time_ns;

        let new_difficulty = new_difficulty_bi.try_into().unwrap_or(u64::MAX);

        let min_diff = current_difficulty.saturating_div(2);
        let max_diff = current_difficulty.saturating_mul(2);
        new_difficulty.clamp(min_diff, max_diff)
    }

    pub fn create_new_block(&self) {
        let payload = self
            .execution_layer
            .create_block_payload()
            .unwrap_or_else(|| TransactionProof {
                root_hash: blake3::Hash::from_bytes([0; 32]),
                transaction_count: 0,
            });

        let last_block = self.chain.lock().unwrap().last().unwrap().clone();

        let new_difficulty = self.get_next_difficulty(last_block.difficulty, last_block.index);
        let new_block = Block::new(
            last_block.index + 1,
            payload,
            last_block.hash,
            new_difficulty,
        );

        let mut timestamps = self.block_timestamps.lock().unwrap();
        if timestamps.len() == DIFFICULTY_ADJUSTMENT_WINDOW {
            timestamps.pop_front();
        }
        timestamps.push_back(new_block.timestamp);
        drop(timestamps);

        self.save_block(&new_block).expect("Failed to save block");
        self.chain.lock().unwrap().push(new_block);
    }

    fn save_block(&self, block: &Block) -> Result<(), rocksdb::Error> {
        let key = block.index.to_be_bytes();
        let value = bincode::serialize(block).expect("Failed to serialize block");
        self.db.put(key, value)
    }
}
