//! V2 canonical block core, Merkle tree, chain work, and BlockIdV2 derivation.

use crate::{
    consensus_parameters::{ConsensusParametersV2, SchemaVersion},
    hash::{hash_v2, BlockIdV2, DomainV2, HashV2, TxIdV2, WorkHashV2},
    record::{CanonicalRecord, WireType},
    v2_records::{
        block_core_v2, pow_work_record, tx_merkle_leaf, tx_merkle_node, tx_merkle_root,
        BLOCK_CORE_SPECS,
    },
};

/// V2 BlockCore with canonical encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockCoreV2 {
    pub canonical_bytes: Vec<u8>,
    pub block_core_hash: HashV2,
}

/// Parameters for constructing a BlockCoreV2
#[derive(Debug, Clone)]
pub struct BlockCoreV2Params {
    pub version: u8,
    pub parent_block_id: BlockIdV2,
    pub state_root: HashV2,
    pub transactions_root: HashV2,
    pub timestamp: u64,
    pub difficulty: u64,
    pub chain_work: u128,
    pub validator_id: HashV2,
    pub miner_id: HashV2,
    pub height: u64,
    pub state_parent: BlockIdV2, // NEW PARAMETER: state parent for branch-scoped execution
}

impl BlockCoreV2 {
    pub fn new(params: BlockCoreV2Params) -> Self {
        let record = block_core_v2(
            params.version,
            params.parent_block_id,
            params.state_root,
            params.transactions_root,
            params.timestamp,
            params.difficulty,
            params.chain_work,
            params.validator_id,
            params.miner_id,
            params.height,
            params.state_parent,
        );
        let bytes = record.finish();
        let block_core_hash =
            HashV2::from_bytes(hash_v2(DomainV2::BlockCore, 0, 0, &bytes).into_bytes()); // domain for internal hash
        Self {
            canonical_bytes: bytes,
            block_core_hash,
        }
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn block_core_hash(&self) -> HashV2 {
        self.block_core_hash
    }
}

/// Build the V2 transaction Merkle root from a list of TxIdV2.
/// Uses typed domains: TxMerkleLeaf -> TxMerkleNode -> TxMerkleRoot.
pub fn build_tx_merkle_root_v2(tx_ids: &[TxIdV2]) -> HashV2 {
    if tx_ids.is_empty() {
        let empty = CanonicalRecord::new().finish();
        let empty_hash = hash_v2(DomainV2::TxMerkleEmpty, 0, 0, &empty);
        let root_rec = tx_merkle_root(TxIdV2::from_hash(empty_hash)).finish();
        return hash_v2(DomainV2::TxMerkleRoot, 0, 0, &root_rec);
    }

    // First level: typed leaves
    let mut level: Vec<TxIdV2> = tx_ids
        .iter()
        .map(|id| {
            let leaf_rec = tx_merkle_leaf(*id).finish();
            TxIdV2::from_hash(hash_v2(DomainV2::TxMerkleLeaf, 0, 0, &leaf_rec))
        })
        .collect();

    // Build tree with typed internal nodes
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for chunk in level.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() == 2 { chunk[1] } else { left }; // odd -> promote duplicate
            let node_rec = tx_merkle_node(left, right).finish();
            next.push(TxIdV2::from_hash(hash_v2(
                DomainV2::TxMerkleNode,
                0,
                0,
                &node_rec,
            )));
        }
        level = next;
    }

    // Final root typed as TxMerkleRoot
    let root_rec = tx_merkle_root(level[0]).finish();
    hash_v2(DomainV2::TxMerkleRoot, 0, 0, &root_rec)
}

/// Compute cumulative chain work (sum of block work) as u128.
/// Block work = floor((2^256 - 1) / difficulty) + 1  (approximate expected work)
/// For exact implementation, we use the integral formulation.
pub fn compute_chain_work_v2(parent_chain_work: u128, difficulty: u64) -> u128 {
    // Exact: work = (2^256) / difficulty
    // We compute using u256 via u128 pairs for 256-bit precision
    // For u128 accumulation, we use the approximation: max_work / difficulty
    const MAX_U128: u128 = u128::MAX;
    let block_work = MAX_U128 / u128::from(difficulty.max(1));
    parent_chain_work.saturating_add(block_work)
}

/// Compute the PoW work hash for a candidate block (CPU oracle).
pub fn compute_pow_work_v2(
    chain_id: u32,
    schema: u16,
    block_core: &BlockCoreV2,
    nonce: u64,
) -> WorkHashV2 {
    let work_rec = pow_work_record(block_core.block_core_hash, nonce);
    let work_bytes = work_rec.finish();
    WorkHashV2::from_hash(hash_v2(DomainV2::PowWork, chain_id, schema, &work_bytes))
}

/// Verify a complete block's PoW against the V2 target.
pub fn verify_block_pow_v2(
    chain_id: u32,
    schema: u16,
    block_core: &BlockCoreV2,
    nonce: u64,
    claimed_work: WorkHashV2,
    difficulty: u64,
) -> Result<bool, crate::pow::PowError> {
    let computed = compute_pow_work_v2(chain_id, schema, block_core, nonce);
    if computed != claimed_work {
        return Ok(false);
    }
    crate::pow::work_meets_difficulty_v2(computed, difficulty)
}

/// Derive BlockIdV2 from a validated BlockCoreV2.
pub fn block_id_v2(chain_id: u32, schema: u16, block_core: &BlockCoreV2) -> BlockIdV2 {
    BlockIdV2::from_hash(hash_v2(
        DomainV2::BlockId,
        chain_id,
        schema,
        block_core.canonical_bytes(),
    ))
}
