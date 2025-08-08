//! # Qanhash Algorithm Implementation
//!
//! This module provides the core implementation of the Qanhash proof-of-work
//! algorithm. It is designed to be quantum-resistant through the use of a
//! large, dynamically generated dataset (Q-DAG). It supports both CPU and
//! GPU (via OpenCL) hashing.
//!
//! ## Features:
//! - **Dynamic Q-DAG**: A large memory dataset that changes over time,
//!   requiring miners to constantly recalculate it, which favors GPUs and FPGAs
//!   with large memory bandwidth over simple ASICs.
//! - **Difficulty Adjustment**: A robust mechanism to keep block times
//!   consistent.
//! - **CPU & GPU Hashing**: Provides a reference CPU implementation and a
//!   high-performance GPU implementation (when the `gpu` feature is enabled).

use crate::qanto_standalone::{
    hash::{qanto_hash, QantoHash},
    parallel::ThreadPool,
};
use lazy_static::lazy_static;
use log::{info, warn};
use primitive_types::U256;
use std::sync::{mpsc, Arc, RwLock};

// --- Type Aliases ---
pub type Difficulty = u64;
pub type Target = [u8; 32];

// --- Constants ---
const TARGET_SLOT_TIME_SECS: u64 = 5;
pub const DIFFICULTY_ADJUSTMENT_WINDOW: usize = 100;
const DAMPING_FACTOR: u64 = 4;

// --- Q-DAG (Quantum-Dynamic Algorithmic Graph) Constants ---
type QDagCacheEntry = Option<(u64, Arc<Vec<[u8; MIX_BYTES]>>)>;

// The large dataset size is a core feature of the algorithm's security.
const DATASET_INIT_SIZE: usize = 1 << 24; // ~16.7M items, ~2GB

const DATASET_GROWTH_EPOCH: u64 = 10_000;
pub const MIX_BYTES: usize = 128;
const CACHE_SIZE: usize = 1 << 20;

// --- GPU Context (Conditional Compilation) ---
#[cfg(feature = "gpu")]
pub use gpu_impl::{hash_batch, GpuContext, GPU_CONTEXT};

#[cfg(feature = "gpu")]
mod gpu_impl {
    use super::*;
    use bytemuck;
    use opencl3::{
        command_queue::CommandQueue,
        context::Context,
        device::{get_all_devices, Device, CL_DEVICE_TYPE_GPU},
        kernel::{ExecuteKernel, Kernel},
        memory::{Buffer, CL_MEM_COPY_HOST_PTR, CL_MEM_READ_ONLY, CL_MEM_READ_WRITE},
        program::Program,
        types::{cl_bool, cl_ulong},
    };
    use std::ffi::c_void;
    use std::sync::Mutex;

    /// A struct to hold the OpenCL context, program, and queue.
    pub struct GpuContext {
        pub kernel: Kernel,
        pub queue: CommandQueue,
        pub context: Context,
        pub device: Device,
    }

    impl GpuContext {
        /// Initializes the OpenCL context, device, and compiles the kernel.
        fn new(kernel_src: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let device_id = get_all_devices(CL_DEVICE_TYPE_GPU)?
                .first()
                .ok_or("No GPU found")?
                .clone();
            let device = Device::new(device_id);
            let context = Context::from_device(&device)?;

            #[allow(deprecated)]
            let queue = unsafe { CommandQueue::create(&context, device_id, 0)? };

            let program = Program::create_and_build_from_source(&context, kernel_src, "")?;
            let kernel = Kernel::create(&program, "qanhash_kernel")?;

            info!(
                "[Qanhash-GPU] Initialized OpenCL context on device: {}",
                device.name()?
            );
            Ok(Self {
                kernel,
                queue,
                context,
                device,
            })
        }
    }

    lazy_static! {
        pub static ref GPU_CONTEXT: Mutex<GpuContext> = {
            info!("[Qanhash-GPU] Compiling OpenCL kernel...");
            const KERNEL_SRC: &str = include_str!("kernel.cl");
            Mutex::new(GpuContext::new(KERNEL_SRC).expect("Failed to initialize GPU context"))
        };
    }

    /// Hashes a batch of nonces on the GPU using the `opencl3` crate.
    pub fn hash_batch(
        header_hash: &QantoHash,
        start_nonce: u64,
        batch_size: usize,
        target: Target,
    ) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
        // All OpenCL calls are inherently unsafe as they interface with C APIs.
        unsafe {
            let gpu = GPU_CONTEXT.lock().unwrap();
            let block_index = u64::from_le_bytes(header_hash.as_bytes()[0..8].try_into().unwrap());
            let dag = get_qdag(block_index);
            let dag_len_mask = (dag.len() - 1) as cl_ulong;

            // Create buffers and copy data in one step using CL_MEM_COPY_HOST_PTR.
            let header_buffer = Buffer::<u8>::create(
                &gpu.context,
                CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
                32,
                header_hash.as_bytes().as_ptr() as *mut c_void,
            )?;

            let dag_as_bytes: &[[u8; MIX_BYTES]] = dag.as_slice();
            let dag_buffer = Buffer::<u8>::create(
                &gpu.context,
                CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
                dag.len() * MIX_BYTES,
                bytemuck::cast_slice::<[u8; MIX_BYTES], u8>(dag_as_bytes).as_ptr() as *mut c_void,
            )?;

            let target_buffer = Buffer::<u8>::create(
                &gpu.context,
                CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
                32,
                target.as_ptr() as *mut c_void,
            )?;

            // Create output buffers.
            let mut result_gid = [0xFFFFFFFFu32];
            let result_gid_buffer = Buffer::<u32>::create(
                &gpu.context,
                CL_MEM_READ_WRITE | CL_MEM_COPY_HOST_PTR,
                1,
                result_gid.as_mut_ptr() as *mut c_void,
            )?;

            let result_hash_buffer =
                Buffer::<u8>::create(&gpu.context, CL_MEM_READ_WRITE, 32, std::ptr::null_mut())?;

            let mut kernel_exec = ExecuteKernel::new(&gpu.kernel);
            kernel_exec
                .set_arg(&header_buffer)
                .set_arg(&start_nonce)
                .set_arg(&dag_buffer)
                .set_arg(&dag_len_mask)
                .set_arg(&target_buffer)
                .set_arg(&result_gid_buffer)
                .set_arg(&result_hash_buffer)
                .set_global_work_size(batch_size);

            kernel_exec.enqueue_nd_range(&gpu.queue)?.wait()?;

            gpu.queue
                .enqueue_read_buffer(&result_gid_buffer, true as cl_bool, 0, &mut result_gid, &[])?
                .wait()?;

            if result_gid[0] != 0xFFFFFFFF {
                let mut final_hash = [0u8; 32];
                gpu.queue
                    .enqueue_read_buffer(
                        &result_hash_buffer,
                        true as cl_bool,
                        0,
                        &mut final_hash,
                        &[],
                    )?
                    .wait()?;
                let winning_nonce = start_nonce + result_gid[0] as u64;
                return Ok(Some((winning_nonce, final_hash)));
            }

            Ok(None)
        }
    }
}

lazy_static! {
    static ref QDAG_CACHE: RwLock<QDagCacheEntry> = RwLock::new(None);
}

pub fn calculate_next_difficulty(last_difficulty: Difficulty, timestamps: &[i64]) -> Difficulty {
    if timestamps.len() < DIFFICULTY_ADJUSTMENT_WINDOW {
        return last_difficulty;
    }
    let actual_timespan_ns = timestamps.last().unwrap() - timestamps.first().unwrap();
    let target_timespan_ns =
        (TARGET_SLOT_TIME_SECS * 1_000_000_000 * (DIFFICULTY_ADJUSTMENT_WINDOW as u64 - 1)) as i64;
    if actual_timespan_ns <= 0 {
        warn!("[Qanhash] Invalid timespan, increasing difficulty significantly.");
        return last_difficulty.saturating_mul(2);
    }
    let adjustment =
        (target_timespan_ns as f64 / actual_timespan_ns as f64 - 1.0) / DAMPING_FACTOR as f64 + 1.0;
    let next_difficulty = (last_difficulty as f64 * adjustment).max(1.0) as Difficulty;

    // FIX: Replaced separate format arguments with inline variables.
    info!("[Qanhash] Difficulty adjusted from {last_difficulty} to {next_difficulty}");
    next_difficulty
}

pub fn difficulty_to_target(difficulty: Difficulty) -> Target {
    if difficulty == 0 {
        return [0xff; 32];
    }
    let diff_u256 = U256::from(difficulty);
    let target_u256 = U256::MAX / diff_u256;
    let mut target = [0u8; 32];
    target_u256.to_big_endian(&mut target);
    target
}

pub fn is_solution_valid(hash: &[u8; 32], target: Target) -> bool {
    U256::from_big_endian(hash) <= U256::from_big_endian(&target)
}

pub fn get_qdag(block_index: u64) -> Arc<Vec<[u8; MIX_BYTES]>> {
    let epoch = block_index / DATASET_GROWTH_EPOCH;
    if let Some((cached_epoch, dag)) = &*QDAG_CACHE.read().unwrap() {
        if *cached_epoch == epoch {
            return dag.clone();
        }
    }
    let mut write_cache = QDAG_CACHE.write().unwrap();
    if let Some((cached_epoch, dag)) = &*write_cache {
        if *cached_epoch == epoch {
            return dag.clone();
        }
    }
    // FIX: Replaced separate format arguments with inline variables.
    info!("[Qanhash] Generating new Q-DAG for epoch {epoch}");
    let seed = qanto_hash(&epoch.to_le_bytes());
    let new_dag = generate_qdag(seed.as_bytes(), epoch);
    *write_cache = Some((epoch, new_dag.clone()));
    new_dag
}

fn generate_qdag(seed: &[u8; 32], epoch: u64) -> Arc<Vec<[u8; MIX_BYTES]>> {
    let base_size = DATASET_INIT_SIZE + (epoch.min(1000) as usize * 128);
    let dataset_size = base_size.next_power_of_two();
    // FIX: Replaced separate format arguments with inline variables.
    info!("[Qanhash] Generating DAG with size {dataset_size} for epoch {epoch}");
    let mut cache = Vec::with_capacity(CACHE_SIZE);
    let mut item_hash = qanto_hash(seed).as_bytes().to_vec();
    for _ in 0..CACHE_SIZE {
        item_hash = qanto_hash(&item_hash).as_bytes().to_vec();
        cache.push(item_hash.clone());
    }
    let mut dataset = vec![[0u8; MIX_BYTES]; dataset_size];
    let pool = ThreadPool::new(num_cpus::get());
    let (tx, rx) = mpsc::channel();
    let arc_cache = Arc::new(cache);
    for i in 0..dataset_size {
        let tx_clone = tx.clone();
        let cache_clone = Arc::clone(&arc_cache);
        pool.execute(move || {
            let mut item_seed = qanto_hash(&i.to_le_bytes()).as_bytes().to_vec();
            let mut final_item_data = vec![0u8; MIX_BYTES];
            for _ in 0..16 {
                let cache_index =
                    u32::from_le_bytes(item_seed[0..4].try_into().unwrap()) as usize % CACHE_SIZE;
                let cache_item = &cache_clone[cache_index];
                for k in 0..MIX_BYTES {
                    final_item_data[k] ^= cache_item[k % 32];
                }
                item_seed = qanto_hash(&item_seed).as_bytes().to_vec();
            }
            let mut slice = [0u8; MIX_BYTES];
            slice.copy_from_slice(&final_item_data);
            tx_clone.send((i, slice)).unwrap();
        });
    }
    drop(tx);
    for (i, slice) in rx {
        dataset[i] = slice;
    }
    Arc::new(dataset)
}

pub fn hash(header_hash: &QantoHash, nonce: u64) -> [u8; 32] {
    let block_index = u64::from_le_bytes(header_hash.as_bytes()[0..8].try_into().unwrap());
    let dag = get_qdag(block_index);
    let dag_len_mask = dag.len() - 1;
    let mut mix = [0u64; MIX_BYTES / 8];
    // FIX: Replaced needless range loop with a more idiomatic iterator-based approach.
    for (i, chunk) in header_hash.as_bytes().chunks(8).take(4).enumerate() {
        mix[i] = u64::from_le_bytes(chunk.try_into().unwrap());
    }
    mix[4] = nonce;
    for _ in 0..32 {
        let p_index = mix[0].wrapping_add(mix[1]);
        let dag_entry1: &[u64; 16] =
            unsafe { &*(dag[p_index as usize & dag_len_mask].as_ptr() as *const [u64; 16]) };
        let dag_entry2: &[u64; 16] = unsafe {
            &*(dag[(p_index.wrapping_add(1)) as usize & dag_len_mask].as_ptr() as *const [u64; 16])
        };
        for i in 0..16 {
            let val = mix[i].wrapping_mul(31).wrapping_add(dag_entry1[i]);
            mix[i] = val.rotate_left(((i % 8) + 1) as u32) ^ dag_entry2[i];
        }
    }
    let mut final_hash_bytes = [0u8; 128];
    for i in 0..16 {
        final_hash_bytes[i * 8..(i + 1) * 8].copy_from_slice(&mix[i].to_le_bytes());
    }
    *qanto_hash(&final_hash_bytes).as_bytes()
}
