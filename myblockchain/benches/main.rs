use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ed25519_dalek::{Signer, SigningKey};
use my_blockchain::{qanhash, ExecutionLayer, Transaction};
use rand::rngs::OsRng;
use rand::Rng;

#[cfg(feature = "gpu")]
use criterion::BatchSize;

/// This benchmark measures the performance of a single Qanhash operation on the CPU.
fn qanhash_op_benchmark(c: &mut Criterion) {
    let mut header_data = [0u8; 128];
    rand::thread_rng().fill(&mut header_data);
    let block_index: u64 = 123_456;
    header_data[0..8].copy_from_slice(&block_index.to_le_bytes());
    let mut hasher = blake3::Hasher::new();
    hasher.update(&header_data);
    let header_hash = hasher.finalize();
    let start_nonce = 1_000_000u64;
    let mut group = c.benchmark_group("Qanhash");
    group.bench_function("Qanhash single operation", |b| {
        b.iter(|| qanhash::hash(black_box(&header_hash), black_box(start_nonce)))
    });
    group.finish();
}

/// This benchmark measures the CPU hash rate by running a fixed number of hashes.
fn mine_cpu_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Block Mining");
    let mut header_data = [0u8; 32];
    rand::thread_rng().fill(&mut header_data);
    let header_hash = blake3::hash(&header_data);
    let start_nonce = 1_000_000u64;
    const HASHES_PER_ITERATION: u64 = 1000;
    group.bench_function("CPU Hash Rate (1k Hashes)", |b| {
        b.iter(|| {
            let mut volatile_sum: u8 = 0;
            for i in 0..HASHES_PER_ITERATION {
                let result = qanhash::hash(black_box(&header_hash), black_box(start_nonce + i));
                volatile_sum = volatile_sum.wrapping_add(result[0]);
            }
            black_box(volatile_sum);
        })
    });
    group.finish();
}

/// This benchmark measures the throughput of the redesigned Execution Layer.
fn execution_layer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Execution Layer");
    group.sample_size(10);

    // FIX: Perform the expensive transaction generation only ONCE.
    let mut transactions = Vec::with_capacity(3125);
    let mut csprng = OsRng;
    for _ in 0..3125 {
        let signing_key = SigningKey::generate(&mut csprng);
        let message = b"This is a test transaction";
        let signature = signing_key.sign(message);
        transactions.push(Transaction {
            id: blake3::hash(message),
            message: message.to_vec(),
            public_key: signing_key.verifying_key(),
            signature,
        });
    }

    group.bench_function("Create Block Payload (3125 txs)", |b| {
        // Use iter_batched to reset the state for each measurement.
        b.iter_batched(
            // Setup: Quickly clone the pre-generated transactions into a new layer.
            // This is much faster and more stable than generating new keys every time.
            || {
                let exec_layer = ExecutionLayer::new();
                for tx in &transactions {
                    exec_layer.add_transaction(tx.clone());
                }
                exec_layer
            },
            // Routine: Take the fresh exec_layer and run the function.
            // Explicitly use black_box on the result for maximum stability.
            |exec_layer| {
                black_box(exec_layer.create_block_payload());
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// This benchmark measures the GPU hash rate.
#[cfg(feature = "gpu")]
fn gpu_hashrate_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Block Mining");
    group.sample_size(10);
    let gpu = my_blockchain::qanhash::GPU_CONTEXT.lock().unwrap();
    let index: u64 = 1;
    let mut header_data = [0u8; 32];
    header_data[0..8].copy_from_slice(&index.to_le_bytes());
    let header_hash = blake3::hash(&header_data);
    let dag = my_blockchain::qanhash::get_qdag(index);
    let dag_len_mask = (dag.len() as u64 / 2).next_power_of_two() - 1;
    let dag_flat: &[u8] = unsafe { dag.align_to::<u8>().1 };
    let header_buffer = ocl::Buffer::builder()
        .queue(gpu.queue.clone())
        .len(32)
        .copy_host_slice(header_hash.as_bytes())
        .build()
        .unwrap();
    let dag_buffer = ocl::Buffer::builder()
        .queue(gpu.queue.clone())
        .len(dag_flat.len())
        .copy_host_slice(dag_flat)
        .build()
        .unwrap();
    let target_buffer = ocl::Buffer::builder()
        .queue(gpu.queue.clone())
        .len(32)
        .copy_host_slice(&[0u8; 32])
        .build()
        .unwrap();
    let result_gid_buffer: ocl::Buffer<u32> = ocl::Buffer::builder()
        .queue(gpu.queue.clone())
        .len(1)
        .build()
        .unwrap();
    let result_hash_buffer: ocl::Buffer<u8> = ocl::Buffer::builder()
        .queue(gpu.queue.clone())
        .len(32)
        .build()
        .unwrap();
    let batch_size: u64 = 1_048_576 * 8;
    group.bench_function("GPU Hash Rate (1 Batch)", |b| {
        b.iter_batched(
            || {
                ocl::Kernel::builder()
                    .program(&gpu.program)
                    .name("qanhash_kernel")
                    .queue(gpu.queue.clone())
                    .global_work_size((batch_size,))
                    .arg(&header_buffer)
                    .arg(0u64)
                    .arg(&dag_buffer)
                    .arg(dag_len_mask)
                    .arg(&target_buffer)
                    .arg(&result_gid_buffer)
                    .arg(&result_hash_buffer)
                    .build()
                    .unwrap()
            },
            |kernel| {
                unsafe {
                    kernel.enq().unwrap();
                }
                gpu.queue.finish().unwrap();
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

// Conditionally register the correct set of benchmarks.
#[cfg(feature = "gpu")]
criterion_group!(
    benches,
    qanhash_op_benchmark,
    mine_cpu_benchmark,
    execution_layer_benchmark,
    gpu_hashrate_benchmark
);

#[cfg(not(feature = "gpu"))]
criterion_group!(
    benches,
    qanhash_op_benchmark,
    mine_cpu_benchmark,
    execution_layer_benchmark
);

criterion_main!(benches);
