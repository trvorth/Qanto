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
        rws.reads.insert("a".to_string());
        rws.writes.insert("a".to_string());
        rws
    }
    fn execute(&self, _state: &MultiVersionState<String, u64>) -> HashMap<String, u64> {
        let mut m = HashMap::new();
        m.insert("a".to_string(), self.0);
        m
    }
}

#[test]
fn executes_without_conflict_panic() {
    let state = Arc::new(MultiVersionState::<String, u64>::new());
    let txs: Vec<Tx> = (0..1000u64).map(Tx).collect();
    let (_state2, committed) = ParallelExecutor::execute_block(state, txs);
    assert!(!committed.is_empty());
}
