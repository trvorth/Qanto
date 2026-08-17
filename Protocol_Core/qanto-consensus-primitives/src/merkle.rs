use crate::{hash_v2, DomainV2, HashV2};
use core::convert::TryFrom;

/// Error returned when Merkle tree operations fail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MerkleError {
    /// The leaf count is zero.
    ZeroLeaves,
    /// The provided leaves do not form a valid Merkle tree.
    InvalidLeaves,
    /// Hash computation failed.
    HashError,
}

impl core::fmt::Display for MerkleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MerkleError::ZeroLeaves => write!(f, "Merkle tree requires at least one leaf"),
            MerkleError::InvalidLeaves => write!(f, "Invalid leaves for Merkle tree construction"),
            MerkleError::HashError => write!(f, "Hash computation failed"),
        }
    }
}

impl std::error::Error for MerkleError {}

/// Computes the Merkle root of a slice of leaf hashes.
///
/// # Arguments
/// * `leaves` - A slice of leaf hashes
///
/// # Returns
/// * `Ok(HashV2)` containing the Merkle root
/// * `Err(MerkleError)` if the operation fails
pub fn merkle_root(leaves: &[HashV2]) -> Result<HashV2, MerkleError> {
    if leaves.is_empty() {
        return Err(MerkleError::ZeroLeaves);
    }

    // Special case: single leaf
    if leaves.len() == 1 {
        return Ok(leaves[0]);
    }

    // Build the Merkle tree level by level
    let mut current_level = leaves.to_vec();

    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));

        // Process pairs of nodes
        let mut i = 0;
        while i < current_level.len() {
            if i + 1 < current_level.len() {
                // Hash the pair
                let combined = combine_hashes(&current_level[i], &current_level[i + 1]);
                next_level.push(combined);
                i += 2;
            } else {
                // Odd number of nodes, duplicate the last one
                let combined = combine_hashes(&current_level[i], &current_level[i]);
                next_level.push(combined);
                i += 1;
            }
        }

        current_level = next_level;
    }

    Ok(current_level[0])
}

/// Combines two hash values by concatenating them and hashing the result.
///
/// # Arguments
/// * `left` - Left hash
/// * `right` - Right hash
///
/// # Returns
/// * The combined hash
fn combine_hashes(left: &HashV2, right: &HashV2) -> HashV2 {
    // Create a temporary record to hold the concatenated hashes
    let mut combined = Vec::with_capacity(64);
    combined.extend_from_slice(left.as_bytes());
    combined.extend_from_slice(right.as_bytes());

    // Hash the combined value using the MERKLE_NODE domain
    hash_v2(DomainV2::TxMerkleNode, 1_234, 2, &combined)
}

/// Computes the Merkle root and generates a proof for a specific leaf.
///
/// # Arguments
/// * `leaves` - A slice of leaf hashes
/// * `leaf_index` - The index of the leaf to generate a proof for
///
/// # Returns
/// * `Ok((root, proof))` where root is the Merkle root and proof is the Merkle proof
/// * `Err(MerkleError)` if the operation fails
pub fn merkle_root_and_proof(
    leaves: &[HashV2],
    leaf_index: usize,
) -> Result<(HashV2, Vec<HashV2>), MerkleError> {
    if leaves.is_empty() {
        return Err(MerkleError::ZeroLeaves);
    }

    if leaf_index >= leaves.len() {
        return Err(MerkleError::InvalidLeaves);
    }

    // Special case: single leaf
    if leaves.len() == 1 {
        return Ok((leaves[0], Vec::new()));
    }

    let mut current_level = leaves.to_vec();
    let mut proof = Vec::new();
    let mut index = leaf_index;

    // Build the Merkle tree level by level, collecting proof nodes
    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));

        // Process pairs of nodes
        let mut i = 0;
        while i < current_level.len() {
            if i + 1 < current_level.len() {
                // Hash the pair
                let combined = combine_hashes(&current_level[i], &current_level[i + 1]);
                next_level.push(combined);

                // If this chunk contains our target leaf, add the sibling to the proof
                if index == i || index == i + 1 {
                    // Add the sibling hash to the proof
                    let sibling_index = if index == i { i + 1 } else { i };
                    proof.push(current_level[sibling_index]);
                }
                i += 2;
            } else {
                // Odd number of nodes, duplicate the last one
                let combined = combine_hashes(&current_level[i], &current_level[i]);
                next_level.push(combined);

                // If this is our target leaf, add the sibling (which is itself) to the proof
                if index == i {
                    proof.push(current_level[i]);
                }
                i += 1;
            }
        }

        // Update index for next level
        index /= 2;
        current_level = next_level;
    }

    Ok((current_level[0], proof))
}

/// Verifies a Merkle proof for a leaf against a root hash.
///
/// # Arguments
/// * `leaf` - The leaf hash to verify
/// * `leaf_index` - The index of the leaf in the tree
/// * `proof` - The Merkle proof (sibling hashes)
/// * `root` - The expected Merkle root
///
/// # Returns
/// * `Ok(true)` if the proof is valid
/// * `Ok(false)` if the proof is invalid
/// * `Err(MerkleError)` if the proof is malformed
pub fn verify_merkle_proof(
    leaf: &HashV2,
    leaf_index: usize,
    proof: &[HashV2],
    root: &HashV2,
) -> Result<bool, MerkleError> {
    if proof.is_empty() && leaf_index == 0 {
        // Special case: single leaf tree
        return Ok(leaf == root);
    }

    let mut computed = *leaf;
    let mut index = leaf_index;

    // Walk up the tree using the proof
    for sibling in proof {
        if index.is_multiple_of(2) {
            // Left child, hash with right sibling
            computed = combine_hashes(&computed, sibling);
        } else {
            // Right child, hash with left sibling
            computed = combine_hashes(sibling, &computed);
        }
        index /= 2;
    }

    Ok(computed == *root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_v2;
    use crate::{DomainV2, HashV2};

    const TEST_CHAIN_ID: u32 = 1_234;
    const TEST_SCHEMA: u16 = 2;

    #[test]
    fn test_empty_merkle_tree() {
        let leaves: [HashV2; 0] = [];
        assert!(matches!(merkle_root(&leaves), Err(MerkleError::ZeroLeaves)));
    }

    #[test]
    fn test_single_leaf_merkle_tree() {
        let leaf = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        let leaves = [leaf];
        let root = merkle_root(&leaves).unwrap();
        assert_eq!(root, leaf);
    }

    #[test]
    fn test_two_leaf_merkle_tree() {
        let leaf1 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        let leaf2 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x02]);
        let leaves = [leaf1, leaf2];

        let root = merkle_root(&leaves).unwrap();
        let expected = combine_hashes(&leaf1, &leaf2);
        assert_eq!(root, expected);
    }

    #[test]
    fn test_three_leaf_merkle_tree() {
        let leaf1 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        let leaf2 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x02]);
        let leaf3 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x03]);
        let leaves = [leaf1, leaf2, leaf3];

        let root = merkle_root(&leaves).unwrap();
        // For 3 leaves (standard Merkle): hash(leaf1, leaf2) -> temp1
        // hash(leaf3, leaf3) -> temp2 (duplicate odd leaf)
        // hash(temp1, temp2) -> root
        let temp1 = combine_hashes(&leaf1, &leaf2);
        let temp2 = combine_hashes(&leaf3, &leaf3);
        let expected = combine_hashes(&temp1, &temp2);
        eprintln!("root: {}", hex::encode(root.as_bytes()));
        eprintln!("expected: {}", hex::encode(expected.as_bytes()));
        assert_eq!(root, expected);
    }

    #[test]
    fn test_merkle_proof() {
        let leaf1 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        let leaf2 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x02]);
        let leaf3 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x03]);
        let leaf4 = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x04]);
        let leaves = [leaf1, leaf2, leaf3, leaf4];

        // Get Merkle root and proof for leaf2 (index 1)
        let (root, proof) = merkle_root_and_proof(&leaves, 1).unwrap();

        // Verify the proof
        assert!(verify_merkle_proof(&leaf2, 1, &proof, &root).unwrap());

        // Verify that tampering is detected
        let mut tampered_proof = proof.clone();
        if !tampered_proof.is_empty() {
            tampered_proof[0] = HashV2::ZERO;
        }
        assert!(!verify_merkle_proof(&leaf2, 1, &tampered_proof, &root).unwrap());
    }

    #[test]
    fn test_merkle_proof_single_leaf() {
        let leaf = hash_v2(DomainV2::TxBody, TEST_CHAIN_ID, TEST_SCHEMA, &[0x01]);
        let leaves = [leaf];

        let (root, proof) = merkle_root_and_proof(&leaves, 0).unwrap();
        assert_eq!(root, leaf);
        assert!(proof.is_empty());

        assert!(verify_merkle_proof(&leaf, 0, &proof, &root).unwrap());
    }
}
