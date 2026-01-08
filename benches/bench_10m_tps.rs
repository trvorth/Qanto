use criterion::{criterion_group, criterion_main, Criterion};
use qanto_core::block_stm::{
    ExecutableTx, MultiVersionState, ParallelExecutor, ReadWriteSet, VersionId,
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
struct Tx(u64);

impl ExecutableTx<String, u64> for Tx {
    fn id(&self) -> u64 {
        self.0
    }
    fn version(&self) -> VersionId {
        self.0
    }
    fn read_write_set(&self) -> ReadWriteSet<String> {
        let mut rws = ReadWriteSet::default();
        rws.reads.insert("k".to_string());
        rws.writes.insert("k".to_string());
        rws
    }
    fn execute(&self, _state: &MultiVersionState<String, u64>) -> HashMap<String, u64> {
        let mut m = HashMap::new();
        m.insert("k".to_string(), self.0);
        m
    }
}

fn bench_parallel_executor(c: &mut Criterion) {
    let state = Arc::new(MultiVersionState::<String, u64>::new());
    let n = std::env::var("QANTO_BENCH_TXS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1_000_000);
    let txs: Vec<Tx> = (0..n as u64).map(Tx).collect();
    c.bench_function("parallel_execute_n", |b| {
        b.iter(|| {
            let _ = ParallelExecutor::execute_block(state.clone(), txs.clone());
        })
    });
}

criterion_group!(benches, bench_parallel_executor);
criterion_main!(benches);
