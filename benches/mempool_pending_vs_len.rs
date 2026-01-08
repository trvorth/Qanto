//! Criterion benchmark comparing Mempool::pending_len_sync (atomic, lock-free)
//! versus Mempool::len() (async, RwLock read) under a pre-populated pool.
//!
//! Executor context: uses a manual Tokio runtime for async calls.
//! We pre-populate with coinbase dummy transactions to avoid UTXO checks.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use qanto::mempool::Mempool;
use qanto::performance_optimizations::QantoDAGOptimizations;
use qanto::qantodag::QantoDAG;
use qanto::transaction::Transaction;
use qanto::types::UTXO;
use std::collections::HashMap;

fn bench_pending_len_vs_len(c: &mut Criterion) {
    // Config chosen to avoid mempool-full rejections during prepopulation.
    let max_age_secs: u64 = 3600; // 1h retention
    let max_size_bytes: usize = 256 * 1024 * 1024; // 256 MiB
    let max_transactions: usize = 1_000_000; // soft cap, not enforced strictly here

    let mempool = Mempool::new(max_age_secs, max_size_bytes, max_transactions);
    let dag = <QantoDAG as QantoDAGOptimizations>::new_dummy_for_verification();
    let utxos: HashMap<String, UTXO> = HashMap::new();

    // Pre-populate with coinbase-like dummy transactions.
    // These have empty inputs and non-zero outputs; they bypass UTXO checks.
    let prepopulate_count: usize = 100_000;
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        for _ in 0..prepopulate_count {
            let tx = Transaction::new_dummy();
            // NOTE: This path validates mempool rules and updates the atomic pending count
            // on successful insertion.
            mempool
                .add_transaction(tx, &utxos, &dag)
                .await
                .expect("dummy tx admitted to mempool");
        }
    });

    let mut group = c.benchmark_group("mempool_pending_vs_len");

    // Hot-path: atomic fetch without awaiting.
    group.bench_function("pending_len_sync", |b| {
        b.iter(|| black_box(mempool.pending_len_sync()));
    });

    // Baseline: async read lock on transactions map (driven via manual runtime).
    group.bench_function("len_async", |b| {
        b.iter(|| black_box(rt.block_on(mempool.len())));
    });

    group.finish();
}

criterion_group!(benches, bench_pending_len_vs_len);
criterion_main!(benches);
