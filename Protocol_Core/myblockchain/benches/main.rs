//! # Qanto Blockchain Benchmark Suite
//!
//! This file contains the Criterion benchmarks for the core components of the
//! `my-blockchain` library, allowing for performance analysis and regression testing.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use futures::future::join_all;
use my_blockchain::{
    execution_shard::ExecutionShard,
    qanhash, qanhash32x,
    qanto_standalone::hash::{qanto_hash, QantoHash},
    ExecutionLayer, Transaction,
};
use rand::rngs::OsRng;
use rand::Rng;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};

/// Benchmarks the core Qanhash hashing operations.
fn qanhash_op_benchmark(c: &mut Criterion) {
    let mut header_data = [0u8; 32];
    rand::thread_rng().fill(&mut header_data);
    let block_index: u64 = 123_456;
    header_data[0..8].copy_from_slice(&block_index.to_le_bytes());
    let header_hash = QantoHash::new(header_data);
    let start_nonce = 1_000_000u64;

    let mut group = c.benchmark_group("Qanhash");
    let quick = std::env::var("QANTO_QUICK_BENCH").is_ok();
    group.sampling_mode(criterion::SamplingMode::Flat);
    group.sample_size(if quick { 100 } else { 1000 });
    group.measurement_time(Duration::from_secs(if quick { 5 } else { 15 }));

    group.bench_function("CPU Qanhash single operation", |b| {
        b.iter_with_large_drop(|| {
            let mut res = [0u8; 32];
            for i in 0..10000 {
                res = qanhash::hash(
                    black_box(&header_hash),
                    black_box(start_nonce + i),
                    black_box(i as u64),
                );
            }
            res
        })
    });

    group.bench_function("Qanhash32x post-quantum operation", |b| {
        b.iter_with_large_drop(|| {
            let mut res = [0u8; 32];
            for i in 0..100 {
                header_data[0] = i as u8;
                res = qanhash32x::qanhash32x(black_box(&header_data));
            }
            res
        })
    });

    group.finish();
}

/// Benchmarks the CPU hash rate.
fn mine_cpu_benchmark(c: &mut Criterion) {
    let mut header_data = [0u8; 32];
    rand::thread_rng().fill(&mut header_data);
    let header_hash = QantoHash::new(header_data);
    let mut nonce = 0u64;
    qanhash::difficulty_to_target(1_000_000); // Calculate target but don't store unused result

    c.bench_function("Block Mining/CPU Hash Rate (1k Hashes)", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(qanhash::hash(&header_hash, nonce, nonce));
                nonce = nonce.wrapping_add(1);
            }
        })
    });
}

/// Benchmarks the "realistic" throughput of the full execution layer.
fn execution_layer_benchmark(c: &mut Criterion) {
    let mut csprng = OsRng;
    let exec_layer = Arc::new(ExecutionLayer::new());

    let quick = std::env::var("QANTO_QUICK_BENCH").is_ok();
    let num_concurrent_batches: usize = if quick { 8 } else { 16 };
    let txs_per_batch: usize = if quick { 500 } else { 1_000 };
    let total_txs: usize = num_concurrent_batches * txs_per_batch;

    const TX_POOL_SIZE: usize = 1000;
    let tx_pool: Vec<Transaction> = (0..TX_POOL_SIZE)
        .map(|i| {
            let keypair = SigningKey::generate(&mut csprng);
            let message = format!("Pooled Tx {i}").into_bytes();
            let signature = keypair.sign(&message);
            Transaction {
                id: qanto_hash(&message),
                message,
                public_key: keypair.verifying_key(),
                signature,
            }
        })
        .collect();

    let all_batches: Arc<Vec<Vec<Transaction>>> = Arc::new(
        (0..num_concurrent_batches)
            .map(|_| {
                (0..txs_per_batch)
                    .map(|i| tx_pool[i % TX_POOL_SIZE].clone())
                    .collect()
            })
            .collect(),
    );

    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("Execution Layer");
    group.throughput(criterion::Throughput::Elements(total_txs as u64));

    group.bench_function(
        format!("Add {num_concurrent_batches} Concurrent Batches ({total_txs} txs total)"),
        |b| {
            b.to_async(&rt).iter_batched(
                || (Arc::clone(&all_batches), Arc::clone(&exec_layer)),
                |(batches, local_exec_layer)| async move {
                    let futures = batches
                        .iter()
                        .map(|batch| local_exec_layer.add_transaction_batch(batch.clone()));
                    join_all(futures).await;
                },
                BatchSize::SmallInput,
            )
        },
    );
    group.finish();
}

/// Benchmarks the raw parallel processing power of the sharded execution model to test peak TPS.
fn hyperscale_tps_benchmark(c: &mut Criterion) {
    let mut csprng = OsRng;

    let quick = std::env::var("QANTO_QUICK_BENCH").is_ok();
    let num_shards: usize = if quick { 8 } else { 16 };
    let txs_per_batch: usize = if quick { 10_000 } else { 100_000 };
    let total_txs: usize = num_shards * txs_per_batch;

    const TX_POOL_SIZE: usize = 1000;
    let tx_pool: Vec<Transaction> = (0..TX_POOL_SIZE)
        .map(|i| {
            let keypair = SigningKey::generate(&mut csprng);
            let message = format!("Pooled Tx {i}").into_bytes();
            let signature = keypair.sign(&message);
            Transaction {
                id: qanto_hash(&message),
                message,
                public_key: keypair.verifying_key(),
                signature,
            }
        })
        .collect();

    let all_batches: Arc<Vec<Vec<Transaction>>> = Arc::new(
        (0..num_shards)
            .map(|_| {
                (0..txs_per_batch)
                    .map(|i| tx_pool[i % TX_POOL_SIZE].clone())
                    .collect()
            })
            .collect(),
    );

    let shards: Arc<Vec<_>> = Arc::new(
        (0..num_shards)
            .map(|_| Mutex::new(ExecutionShard::new()))
            .collect(),
    );

    let mut group = c.benchmark_group("Hyperscale Execution");
    group.throughput(criterion::Throughput::Elements(total_txs as u64));

    group.bench_function(
        format!("Raw Sharded Execution ({total_txs} txs total)"),
        |b| {
            b.iter_batched(
                || (Arc::clone(&shards), Arc::clone(&all_batches)),
                |(shards, batches)| {
                    batches.par_iter().enumerate().for_each(|(i, batch)| {
                        let mut shard = shards[i].lock().unwrap();
                        black_box(shard.process_batch(batch));
                    });
                },
                BatchSize::SmallInput,
            )
        },
    );
    group.finish();
}

/// Benchmarks the GPU hash rate (only if 'gpu' feature is enabled).
#[cfg(feature = "gpu")]
fn gpu_hashrate_benchmark(c: &mut Criterion) {
    use bytemuck;
    use my_blockchain::qanhash::GPU_CONTEXT;
    use opencl3::{
        kernel::ExecuteKernel,
        memory::{Buffer, CL_MEM_COPY_HOST_PTR, CL_MEM_READ_ONLY, CL_MEM_READ_WRITE},
        types::cl_ulong,
    };
    use std::ffi::c_void;

    println!("Initializing GPU context for benchmark...");
    let gpu = GPU_CONTEXT.lock().unwrap();
    println!("GPU context initialized.");

    let mut header_data = [0u8; 32];
    rand::thread_rng().fill(&mut header_data);
    let block_index: u64 = 123_456;
    header_data[0..8].copy_from_slice(&block_index.to_le_bytes());
    let header_hash = QantoHash::new(header_data);
    let target = qanhash::difficulty_to_target(50_000_000);

    println!("Generating Q-DAG for benchmark... This may take a moment.");
    let dag = qanhash::get_qdag(block_index);
    let dag_len_mask = (dag.len() - 1) as cl_ulong;
    println!("Q-DAG generated. Creating GPU buffers...");

    // SAFETY: OpenCL buffer creation is safe because:
    // 1. GPU context is valid and properly initialized
    // 2. Buffer sizes are calculated correctly and match data lengths
    // 3. Source pointers (header_hash, dag, target) are valid and live for buffer creation
    // 4. CL_MEM_COPY_HOST_PTR ensures data is copied, not referenced
    // 5. Error handling with unwrap() is acceptable in benchmark context
    unsafe {
        // Create buffers and copy data in one step using CL_MEM_COPY_HOST_PTR for setup.
        let header_buffer = Buffer::<u8>::create(
            &gpu.context,
            CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
            32,
            header_hash.as_bytes().as_ptr() as *mut c_void,
        )
        .unwrap();

        // FIX: Explicitly specify the types for bytemuck::cast_slice to resolve the compiler error.
        let dag_as_bytes: &[[u8; qanhash::MIX_BYTES]] = dag.as_slice();
        let dag_buffer = Buffer::<u8>::create(
            &gpu.context,
            CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
            dag.len() * my_blockchain::qanhash::MIX_BYTES,
            bytemuck::cast_slice::<[u8; qanhash::MIX_BYTES], u8>(dag_as_bytes).as_ptr()
                as *mut c_void,
        )
        .unwrap();

        let target_buffer = Buffer::<u8>::create(
            &gpu.context,
            CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
            32,
            target.as_ptr() as *mut c_void,
        )
        .unwrap();
        println!("GPU buffers created.");

        let mut group = c.benchmark_group("Block Mining");
        group.sample_size(10);
        let batch_size: usize = 1 << 16;
        group.throughput(criterion::Throughput::Elements(batch_size as u64));
        let mut current_nonce = 0u64;

        group.bench_function("GPU Hash Rate", |b| {
            b.iter_batched(
                || {
                    // Setup for each iteration: create result buffers
                    let result_gid_buffer = Buffer::<u32>::create(
                        &gpu.context,
                        CL_MEM_READ_WRITE | CL_MEM_COPY_HOST_PTR,
                        1,
                        [0xFFFFFFFFu32].as_ptr() as *mut c_void,
                    )
                    .unwrap();
                    let result_hash_buffer = Buffer::<u8>::create(
                        &gpu.context,
                        CL_MEM_READ_WRITE,
                        32,
                        std::ptr::null_mut(),
                    )
                    .unwrap();
                    (result_gid_buffer, result_hash_buffer)
                },
                |(mut result_gid_buffer, mut result_hash_buffer)| {
                    // Routine being measured
                    let mut kernel_exec = ExecuteKernel::new(&gpu.kernel);
                    kernel_exec
                        .set_arg(&header_buffer)
                        .set_arg(&current_nonce)
                        .set_arg(&dag_buffer)
                        .set_arg(&dag_len_mask)
                        .set_arg(&target_buffer)
                        .set_arg(&mut result_gid_buffer)
                        .set_arg(&mut result_hash_buffer)
                        .set_global_work_size(batch_size);

                    kernel_exec
                        .enqueue_nd_range(&gpu.queue)
                        .unwrap()
                        .wait()
                        .unwrap();
                    current_nonce = current_nonce.wrapping_add(batch_size as u64);
                },
                BatchSize::SmallInput,
            );
        });
        group.finish();
    }
}

// --- Criterion Group Definitions ---
#[cfg(feature = "gpu")]
criterion_group!(
    name = benches;
    config = Criterion::default().with_plots();
    targets =
        qanhash_op_benchmark,
        mine_cpu_benchmark,
        execution_layer_benchmark,
        hyperscale_tps_benchmark,
        gpu_hashrate_benchmark
);

#[cfg(not(feature = "gpu"))]
criterion_group!(
    name = benches;
    config = Criterion::default().with_plots();
    targets =
        qanhash_op_benchmark,
        mine_cpu_benchmark,
        execution_layer_benchmark,
        hyperscale_tps_benchmark
);

criterion_main!(benches);
