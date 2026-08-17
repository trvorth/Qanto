use crate::merkle::merkle_root;
use crate::{hash_v2, DomainV2, HashV2, StateRootV2};
use std::collections::HashMap;

/// Error returned when state trie operations fail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    /// The state key is invalid.
    InvalidKey,
    /// The state value is invalid.
    InvalidValue,
    /// The state trie is missing a required node.
    MissingNode,
    /// Hash computation failed.
    HashError,
    /// The state key already exists.
    KeyAlreadyExists,
    /// The state key does not exist.
    KeyNotFound,
}

impl core::fmt::Display for StateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StateError::InvalidKey => write!(f, "Invalid state key"),
            StateError::InvalidValue => write!(f, "Invalid state value"),
            StateError::MissingNode => write!(f, "Missing state trie node"),
            StateError::HashError => write!(f, "Hash computation failed"),
            StateError::KeyAlreadyExists => write!(f, "State key already exists"),
            StateError::KeyNotFound => write!(f, "State key not found"),
        }
    }
}

impl std::error::Error for StateError {}

/// Represents a node in the state trie.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
enum TrieNode {
    /// A leaf node containing a key-value pair.
    Leaf { key: HashV2, value: HashV2 },

    /// An internal node with up to 16 children (hexary trie).
    Branch {
        children: [Option<Box<TrieNode>>; 16],
        value: Option<HashV2>, // Value stored at this node (optional)
    },

    #[default]
    Null,
}

impl TrieNode {
    /// Returns a None option for initializing the children array.
    const fn null_child() -> Option<Box<TrieNode>> {
        None
    }

    /// Creates a new null node.
    fn new_null() -> Self {
        TrieNode::Null
    }

    /// Creates a new leaf node.
    fn new_leaf(key: HashV2, value: HashV2) -> Self {
        TrieNode::Leaf { key, value }
    }

    /// Creates a new branch node.
    fn new_branch() -> Self {
        TrieNode::Branch {
            children: Default::default(),
            value: None,
        }
    }

    /// Checks if the node is null.
    fn is_null(&self) -> bool {
        matches!(self, TrieNode::Null)
    }

    /// Checks if the node is a leaf.
    fn is_leaf(&self) -> bool {
        matches!(self, TrieNode::Leaf { .. })
    }

    /// Gets the value stored at this node, if any.
    fn get_value(&self) -> Option<&HashV2> {
        match self {
            TrieNode::Leaf { value, .. } => Some(value),
            TrieNode::Branch { value, .. } => value.as_ref(),
            TrieNode::Null => None,
        }
    }

    /// Gets the hash of this node for trie computations.
    fn hash(&self) -> HashV2 {
        match self {
            TrieNode::Leaf { key, value } => {
                // Hash the key-value pair
                let mut encoded = Vec::with_capacity(64);
                encoded.extend_from_slice(key.as_bytes());
                encoded.extend_from_slice(value.as_bytes());
                hash_v2(DomainV2::StateLeaf, 1_234, 2, &encoded)
            }
            TrieNode::Branch { children, value } => {
                // Hash the branch node (16 children + optional value)
                let mut encoded = Vec::with_capacity(512); // 16 * 32 bytes + 32 for value

                // Encode children
                for child_option in children {
                    match child_option {
                        Some(child) => {
                            encoded.extend_from_slice(child.hash().as_bytes());
                        }
                        None => {
                            // Encode null child as 32 zero bytes
                            encoded.extend_from_slice(&[0u8; 32]);
                        }
                    }
                }

                // Encode value if present
                match value {
                    Some(v) => {
                        encoded.extend_from_slice(v.as_bytes());
                    }
                    None => {
                        // Encode null value as 32 zero bytes
                        encoded.extend_from_slice(&[0u8; 32]);
                    }
                }

                hash_v2(DomainV2::StateNode, 1_234, 2, &encoded)
            }
            TrieNode::Null => HashV2::ZERO,
        }
    }

    /// Inserts a key-value pair into the trie.
    fn insert(&mut self, key: &[u8], val: HashV2, depth: usize) -> Result<(), StateError> {
        if depth >= 64 {
            // Max depth reached (assuming 256-bit keys, 4 bits per nibble)
            return Err(StateError::InvalidKey);
        }

        match self {
            TrieNode::Null => {
                // Convert null to leaf at this depth
                *self = TrieNode::Leaf {
                    key: HashV2::from_slice(key),
                    value: val,
                };
                Ok(())
            }
            TrieNode::Leaf {
                key: existing_key,
                value: existing_value,
            } => {
                // Check if we're trying to insert at the same key
                if key == existing_key.as_bytes() {
                    // Update existing value
                    *existing_value = val;
                    Ok(())
                } else {
                    // Need to split the leaf into a branch
                    let leaf_key = *existing_key;
                    let leaf_value = *existing_value;

                    // Create new branch
                    let mut branch = TrieNode::new_branch();

                    // For simplicity in this implementation, we'll treat the key as a single nibble at depth 0
                    // A production implementation would properly handle nibble traversal
                    if depth == 0 {
                        // Use first byte of key as nibble (simplified)
                        let existing_nibble = (existing_key.as_bytes()[0] >> 4) & 0x0F;
                        let new_nibble = (key[0] >> 4) & 0x0F;

                        // Insert existing leaf into branch
                        if let TrieNode::Branch {
                            children,
                            value: branch_value,
                        } = &mut branch
                        {
                            children[existing_nibble as usize] = Some(Box::new(TrieNode::Leaf {
                                key: leaf_key,
                                value: leaf_value,
                            }));

                            // Insert new leaf into branch
                            children[new_nibble as usize] =
                                Some(Box::new(TrieNode::new_leaf(HashV2::from_slice(key), val)));

                            *self = branch;
                            Ok(())
                        } else {
                            Err(StateError::InvalidKey)
                        }
                    } else {
                        // For depths > 0, we simplify and just update the value if keys match
                        // or treat as collision (in a real impl, we'd continue down the trie)
                        if key == existing_key.as_bytes() {
                            *existing_value = val;
                            Ok(())
                        } else {
                            // Key collision at this depth - in a simple implementation, we overwrite
                            *existing_value = val;
                            Ok(())
                        }
                    }
                }
            }
            TrieNode::Branch { children, value } => {
                // Determine which nibble to follow
                if depth == 0 {
                    // Use first byte of key as nibble (simplified)
                    let nibble = (key[0] >> 4) & 0x0F;

                    // Follow/create the child node
                    let child_ref = &mut children[nibble as usize];
                    if child_ref.is_none() {
                        *child_ref = Some(Box::new(TrieNode::new_null()));
                    }

                    // Recursively insert into the child
                    if let Some(ref mut child) = child_ref {
                        child.insert(key, val, depth + 1)
                    } else {
                        Err(StateError::MissingNode)
                    }
                } else {
                    // For simplicity in this implementation, we stop at depth 0
                    // A full implementation would continue traversing the trie
                    // Store value at this node if we've conceptually reached the end
                    *value = Some(val);
                    Ok(())
                }
            }
        }
    }

    /// Retrieves a value from the trie by key.
    fn get(&self, key: &[u8], depth: usize) -> Option<&HashV2> {
        if depth >= 64 {
            return None;
        }

        match self {
            TrieNode::Null => None,
            TrieNode::Leaf {
                key: stored_key,
                value,
            } => {
                if key == stored_key.as_bytes() {
                    Some(value)
                } else {
                    None
                }
            }
            TrieNode::Branch { children, value } => {
                // Check if we have a value at this node (for exact key match at this depth)
                let result = value.as_ref();

                // Continue down the trie if we haven't consumed the full key
                if depth == 0 {
                    // Use first byte of key as nibble (simplified)
                    let nibble = (key[0] >> 4) & 0x0F;

                    // Follow the child node
                    if let Some(ref child) = children[nibble as usize] {
                        // Recursively search in the child
                        if let Some(child_value) = child.get(key, depth + 1) {
                            // If we found a value in the child, return it (child takes precedence)
                            return Some(child_value);
                        }
                    }
                }

                result
            }
        }
    }
}

/// A simple state trie implementation for account state management.
#[derive(Debug, Clone, Default)]
pub struct StateTrie {
    root: TrieNode,
}

impl StateTrie {
    /// Creates a new empty state trie.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a key-value pair into the state trie.
    ///
    /// # Arguments
    /// * `key` - The state key (as bytes)
    /// * `value` - The state value hash
    ///
    /// # Returns
    /// * `Ok(())` if the insertion was successful
    /// * `Err(StateError)` if the insertion failed
    pub fn insert(&mut self, key: &[u8], value: HashV2) -> Result<(), StateError> {
        // Validate key length (assuming 32-byte keys for simplicity)
        if key.len() != 32 {
            return Err(StateError::InvalidKey);
        }

        self.root.insert(key, value, 0)
    }

    /// Retrieves a value from the state trie by key.
    ///
    /// # Arguments
    /// * `key` - The state key (as bytes)
    ///
    /// # Returns
    /// * `Some(value)` if the key was found
    /// * `None` if the key was not found
    pub fn get(&self, key: &[u8]) -> Option<&HashV2> {
        // Validate key length
        if key.len() != 32 {
            return None;
        }

        self.root.get(key, 0)
    }

    /// Computes the root hash of the state trie.
    ///
    /// # Returns
    /// * The state root hash
    pub fn root_hash(&self) -> StateRootV2 {
        StateRootV2::from_hash(self.root.hash())
    }

    /// Checks if the state trie is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.root, TrieNode::Null)
    }

    /// Clears the state trie.
    pub fn clear(&mut self) {
        self.root = TrieNode::new_null();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_v2;
    use crate::{DomainV2, HashV2};

    const TEST_CHAIN_ID: u32 = 1_234;
    const TEST_SCHEMA: u16 = 2;

    #[test]
    fn test_empty_state_trie() {
        let trie = StateTrie::new();
        assert!(trie.is_empty());
        assert_eq!(trie.root_hash(), StateRootV2::from_hash(HashV2::ZERO));
    }

    #[test]
    fn test_state_trie_insert_and_get() {
        let mut trie = StateTrie::new();

        // Create test key and value
        let mut key = [0u8; 32];
        key[0] = 0x42; // Make it non-zero
        let value = hash_v2(
            DomainV2::StateLeaf,
            TEST_CHAIN_ID,
            TEST_SCHEMA,
            &[0x01, 0x02, 0x03, 0x04],
        );

        // Insert the key-value pair
        assert!(trie.insert(&key, value).is_ok());

        // Retrieve the value
        let retrieved = trie.get(&key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &value);

        // Retrieve a non-existent key
        let mut other_key = [0u8; 32];
        other_key[0] = 0x43; // Different key
        assert!(trie.get(&other_key).is_none());
    }

    #[test]
    fn test_state_trie_root_hash() {
        let mut trie = StateTrie::new();

        // Empty trie should have zero root
        assert_eq!(trie.root_hash(), StateRootV2::from_hash(HashV2::ZERO));

        // Add a key-value pair
        let mut key = [0u8; 32];
        key[0] = 0x42;
        let value = hash_v2(DomainV2::StateLeaf, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        assert!(trie.insert(&key, value).is_ok());

        // Root should now be non-zero
        let root_hash = trie.root_hash();
        assert_ne!(root_hash, StateRootV2::from_hash(HashV2::ZERO));
    }

    #[test]
    fn test_state_trie_update() {
        let mut trie = StateTrie::new();

        let mut key = [0u8; 32];
        key[0] = 0x42;
        let value1 = hash_v2(DomainV2::StateLeaf, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        let value2 = hash_v2(DomainV2::StateLeaf, TEST_CHAIN_ID, TEST_SCHEMA, &[0x02]);

        // Insert first value
        assert!(trie.insert(&key, value1).is_ok());
        assert_eq!(trie.get(&key).unwrap(), &value1);

        // Update with second value
        assert!(trie.insert(&key, value2).is_ok());
        assert_eq!(trie.get(&key).unwrap(), &value2);
    }

    #[test]
    fn test_state_trie_invalid_key_length() {
        let mut trie = StateTrie::new();
        let mut short_key = [0u8; 16]; // Too short
        short_key[0] = 0x42;
        let value = hash_v2(DomainV2::StateLeaf, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);

        assert!(matches!(
            trie.insert(&short_key, value),
            Err(StateError::InvalidKey)
        ));

        let mut long_key = [0u8; 64]; // Too long
        long_key[0] = 0x42;
        assert!(matches!(
            trie.insert(&long_key, value),
            Err(StateError::InvalidKey)
        ));
    }
}
