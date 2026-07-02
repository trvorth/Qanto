use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use qanto::mempool::{LockFreePqcMempool, SigVerificationTask};

fn bench_pqc_mempool_burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("pqc_mempool");
    group.sample_size(10);

    group.bench_function("burst_10k_txs", |b| {
        b.iter_batched(
            || {
                let mempool = LockFreePqcMempool::new();
                for _ in 0..10_000 {
                    // Simulating ~5KB payload
                    mempool.push_task(SigVerificationTask {
                        public_key: vec![0u8; 1952], // Approx Dilithium3 PK size
                        message: vec![0u8; 32],
                        signature: vec![0u8; 3293], // Approx Dilithium3 Sig size
                    });
                }
                mempool
            },
            |mempool| {
                mempool.process_batch_with_prefetch();
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

criterion_group!(benches, bench_pqc_mempool_burst);
criterion_main!(benches);
