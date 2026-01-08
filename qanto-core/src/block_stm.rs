use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;

pub type VersionId = u64;

#[derive(Clone)]
pub struct MultiVersionState<K: Eq + Hash + Clone, V: Clone> {
    inner: HashMap<K, Vec<(VersionId, V)>>,
}

impl<K: Eq + Hash + Clone, V: Clone> MultiVersionState<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, key: &K, version: VersionId) -> Option<V> {
        self.inner.get(key).and_then(|vs| {
            vs.iter()
                .rev()
                .find(|(ver, _)| *ver <= version)
                .map(|(_, v)| v.clone())
        })
    }
    pub fn put(&mut self, key: K, version: VersionId, value: V) {
        let entry = self.inner.entry(key).or_default();
        entry.push((version, value));
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Default for MultiVersionState<K, V> {
    fn default() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ReadWriteSet<K: Eq + Hash + Clone> {
    pub reads: HashSet<K>,
    pub writes: HashSet<K>,
}

pub trait ExecutableTx<K: Eq + Hash + Clone, V: Clone + Send + Sync>: Send + Sync {
    fn id(&self) -> u64;
    fn version(&self) -> VersionId;
    fn read_write_set(&self) -> ReadWriteSet<K>;
    fn execute(&self, state: &MultiVersionState<K, V>) -> HashMap<K, V>;
}

pub struct ParallelExecutor;

impl ParallelExecutor {
    pub fn execute_block<K, V, T>(
        state: Arc<MultiVersionState<K, V>>,
        txs: Vec<T>,
    ) -> (Arc<MultiVersionState<K, V>>, Vec<u64>)
    where
        K: Eq + Hash + Clone + Send + Sync,
        V: Clone + Send + Sync,
        T: ExecutableTx<K, V> + Clone,
    {
        let workers = num_cpus::get().max(1);
        let outcomes: Vec<(u64, ReadWriteSet<K>, HashMap<K, V>)> = txs
            .par_chunks(std::cmp::max(1, txs.len() / workers))
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .map(|tx| {
                        let rws = tx.read_write_set();
                        let writes = tx.execute(&state);
                        (tx.id(), rws, writes)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let mut committed_ids = Vec::new();
        let mut new_state = (*state).clone();
        for (tx_id, _rws, writes) in outcomes.into_iter() {
            {
                for (k, v) in writes.into_iter() {
                    new_state.put(k, tx_id, v);
                }
                committed_ids.push(tx_id);
            }
        }

        (Arc::new(new_state), committed_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_version_state_default_and_new_equivalent() {
        let a: MultiVersionState<u64, u64> = MultiVersionState::default();
        let b: MultiVersionState<u64, u64> = MultiVersionState::new();
        assert_eq!(a.inner.len(), 0);
        assert_eq!(b.inner.len(), 0);
    }

    #[test]
    fn put_and_get_latest_not_exceeding_version() {
        let mut s: MultiVersionState<&str, i32> = MultiVersionState::new();
        s.put("k", 1, 10);
        s.put("k", 2, 20);
        s.put("k", 3, 30);

        assert_eq!(s.get(&"k", 0), None);
        assert_eq!(s.get(&"k", 1), Some(10));
        assert_eq!(s.get(&"k", 2), Some(20));
        assert_eq!(s.get(&"k", 3), Some(30));
        assert_eq!(s.get(&"k", 4), Some(30));
    }
}
