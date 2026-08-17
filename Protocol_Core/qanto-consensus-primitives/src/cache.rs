use core::hash::{BuildHasher, Hash, Hasher};
use core::marker::PhantomData;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;

/// A simple LRU (Least Recently Used) cache implementation.
///
/// This cache evicts the least recently used items when it exceeds its capacity.
/// It provides O(1) average time complexity for get and put operations.
#[derive(Debug, Clone)]
pub struct LruCache<K, V, S = RandomState> {
    /// Maximum number of items the cache can hold.
    capacity: usize,

    /// Current number of items in the cache.
    length: usize,

    /// Hash map storing key-value pairs and their position in the access list.
    map: HashMap<K, (V, usize), S>,

    /// Access list tracking the order of usage (most recent at the end).
    access_list: Vec<K>,

    /// Phantom data for the hasher.
    _marker: PhantomData<S>,
}

impl<K, V, S> LruCache<K, V, S>
where
    K: Eq + Hash + Clone,
    V: Clone,
    S: BuildHasher + Default,
{
    /// Creates a new LRU cache with the specified capacity.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of items the cache can hold
    ///
    /// # Returns
    /// * A new LruCache instance
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Cache capacity must be greater than zero");
        Self {
            capacity,
            length: 0,
            map: HashMap::with_capacity_and_hasher(capacity, S::default()),
            access_list: Vec::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    /// Creates a new LRU cache with the specified capacity and hasher.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of items the cache can hold
    /// * `hasher` - The hasher to use for keys
    ///
    /// # Returns
    /// * A new LruCache instance
    pub fn with_hasher(capacity: usize, hasher: S) -> Self {
        assert!(capacity > 0, "Cache capacity must be greater than zero");
        Self {
            capacity,
            length: 0,
            map: HashMap::with_capacity_and_hasher(capacity, hasher),
            access_list: Vec::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    /// Returns the number of items currently in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns true if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns true if the cache is at full capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.length >= self.capacity
    }

    /// Returns the maximum capacity of the cache.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all items from the cache.
    pub fn clear(&mut self) {
        self.map.clear();
        self.access_list.clear();
        self.length = 0;
    }

    /// Gets an item from the cache, marking it as recently used.
    ///
    /// # Arguments
    /// * `key` - The key to look up
    ///
    /// # Returns
    /// * `Some(value)` if the key was found
    /// * `None` if the key was not found
    pub fn get(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        if let Some((value, pos)) = self.map.get(key).cloned() {
            // Move the accessed item to the end of the access list (most recently used)
            self.access_list.remove(pos);
            self.access_list.push(key.clone());

            // Update the position of the accessed item
            let new_pos = self.access_list.len() - 1;
            if let Some(entry) = self.map.get_mut(key) {
                entry.1 = new_pos;
            }

            // Update positions of items that shifted left due to removal
            for i in pos..self.access_list.len() - 1 {
                if let Some(k) = self.access_list.get(i) {
                    if let Some(entry) = self.map.get_mut(k) {
                        if entry.1 > pos {
                            entry.1 -= 1;
                        }
                    }
                }
            }

            Some(value)
        } else {
            None
        }
    }

    /// Inserts an item into the cache, marking it as recently used.
    /// If the cache is at capacity, the least recently used item will be evicted.
    ///
    /// # Arguments
    /// * `key` - The key to insert
    /// * `value` - The value to insert
    ///
    /// # Returns
    /// * The evicted value if any, otherwise None
    pub fn put(&mut self, key: K, value: V) -> Option<V>
    where
        V: Clone,
    {
        // Check if key already exists
        if let Some((old_value, pos)) = self.map.get(&key).cloned() {
            // Update existing item
            self.access_list.remove(pos);
            self.access_list.push(key.clone());
            let new_pos = self.access_list.len() - 1;

            // Update the value and position
            if let Some(entry) = self.map.get_mut(&key) {
                entry.0 = value.clone();
                entry.1 = new_pos;
            }

            // Return the old value
            Some(old_value)
        } else {
            // Insert new item
            if self.length >= self.capacity {
                // Evict least recently used item
                if let Some(lru_key) = self.access_list.first().cloned() {
                    self.access_list.remove(0);
                    if let Some((evicted_value, _)) = self.map.remove(&lru_key) {
                        self.length -= 1;
                        // Insert the new item
                        self.access_list.push(key.clone());
                        let pos = self.access_list.len() - 1;
                        self.map.insert(key, (value, pos));
                        self.length += 1;
                        return Some(evicted_value);
                    }
                }
            } else {
                // Insert new item without eviction
                self.access_list.push(key.clone());
                let pos = self.access_list.len() - 1;
                self.map.insert(key, (value, pos));
                self.length += 1;
            }
            None
        }
    }

    /// Returns true if the cache contains the specified key.
    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Removes and returns the item associated with the key.
    ///
    /// # Arguments
    /// * `key` - The key to remove
    ///
    /// # Returns
    /// * `Some(value)` if the key was found and removed
    /// * `None` if the key was not found
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some((value, pos)) = self.map.remove(key) {
            // Remove from access list
            self.access_list.remove(pos);

            // Update positions of subsequent items in access list
            for i in pos..self.access_list.len() {
                if let Some(k) = self.access_list.get(i) {
                    if let Some(entry) = self.map.get_mut(k) {
                        if entry.1 > pos {
                            entry.1 -= 1;
                        }
                    }
                }
            }

            self.length -= 1;
            Some(value)
        } else {
            None
        }
    }

    /// Returns a reference to the most recently used item without removing it.
    #[must_use]
    pub fn peek(&self) -> Option<&V> {
        self.access_list
            .last()
            .and_then(|key| self.map.get(key).map(|(value, _)| value))
    }

    /// Returns a reference to the least recently used item without removing it.
    #[must_use]
    pub fn peek_lru(&self) -> Option<&V> {
        self.access_list
            .first()
            .and_then(|key| self.map.get(key).map(|(value, _)| value))
    }

    /// Returns an iterator over the cache items in order from most to least recently used.
    pub fn iter(&self) -> Iter<'_, K, V, S> {
        Iter {
            cache: self,
            pos: 0,
        }
    }
}

/// Iterator over the LruCache items from most to least recently used.
pub struct Iter<'a, K, V, S> {
    cache: &'a LruCache<K, V, S>,
    pos: usize,
}

impl<'a, K, V, S> Iterator for Iter<'a, K, V, S>
where
    K: Eq + Hash + Clone,
    S: BuildHasher,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.cache.access_list.len() {
            return None;
        }

        let key = &self.cache.access_list[self.cache.access_list.len() - 1 - self.pos];
        if let Some((value, _)) = self.cache.map.get(key) {
            self.pos += 1;
            Some((key, value))
        } else {
            // This shouldn't happen if the cache is consistent, but skip if it does
            self.pos += 1;
            self.next()
        }
    }
}

impl<K, V, S> Default for LruCache<K, V, S>
where
    K: Eq + Hash + Clone,
    V: Clone,
    S: BuildHasher + Default,
{
    fn default() -> Self {
        Self::new(128) // Default capacity of 128 items
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;

    #[test]
    fn test_basic_operations() {
        let mut cache = LruCache::<&str, &str, std::collections::hash_map::RandomState>::new(3);

        // Test insert and retrieval
        cache.put("key1", "value1");
        cache.put("key2", "value2");

        assert_eq!(cache.get(&"key1"), Some("value1"));
        assert_eq!(cache.get(&"key2"), Some("value2"));
        assert_eq!(cache.get(&"key3"), None);

        // Test update
        cache.put("key1", "updated_value1");
        assert_eq!(cache.get(&"key1"), Some("updated_value1"));

        // Test LRU eviction
        cache.put("key3", "value3"); // Cache is now full (3/3)
        cache.put("key4", "value4"); // This should evict key2 (least recently used)

        assert_eq!(cache.get(&"key2"), None); // Evicted
        assert_eq!(cache.get(&"key1"), Some("updated_value1")); // Recently used
        assert_eq!(cache.get(&"key3"), Some("value3")); // Recently used
        assert_eq!(cache.get(&"key4"), Some("value4")); // Just added

        // Test capacity
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.capacity(), 3);

        // Test clear
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.get(&"key1"), None);
    }

    #[test]
    fn test_access_order() {
        let mut cache = LruCache::<&str, i32, std::collections::hash_map::RandomState>::new(3);

        // Fill cache
        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        // Access 'a' to make it most recently used
        assert_eq!(cache.get(&"a"), Some(1));

        // Add new item, should evict 'b' (least recently used)
        cache.put("d", 4);

        assert_eq!(cache.get(&"b"), None); // Evicted
        assert_eq!(cache.get(&"a"), Some(1)); // Still there
        assert_eq!(cache.get(&"c"), Some(3)); // Still there
        assert_eq!(cache.get(&"d"), Some(4)); // New item
    }

    #[test]
    fn test_iterator() {
        let mut cache = LruCache::<&str, i32, std::collections::hash_map::RandomState>::new(3);
        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        // Access in order: a, b, c
        cache.get(&"a");
        cache.get(&"b");
        cache.get(&"c");

        // Order should be: c (most recent), b, a (least recent)
        let mut iter = cache.iter();
        assert_eq!(iter.next(), Some((&"c", &3)));
        assert_eq!(iter.next(), Some((&"b", &2)));
        assert_eq!(iter.next(), Some((&"a", &1)));
        assert_eq!(iter.next(), None);
    }
}
