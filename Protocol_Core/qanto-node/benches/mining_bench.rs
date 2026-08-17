//! Mining & Hot-Path Benchmarks for Qanto
//!
//! Run with: `cargo bench --bench mining_bench`
//!
//! Covers:
//! - Nonce testing hot path (lock-free worker)
//! - Header serialization (compact block relay)
//! - Short ID computation (blake3)
//! - Peer scoring (P1-D)
//! - Block creation candidate (P1-C)

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::hint::black_box;
use std::time::Duration;

use libp2p::PeerId;

// Import publicly exported types from the qanto library crate
use qanto::{
    protocol::compact_block::ShortId, protocol::peer_scoring::PeerScoring, qantodag::QantoBlock,
};

// Simple synchronous hash for nonce testing benchmark
fn simple_hash(data: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

/// Benchmark: Lock-free nonce testing (core hot path)
fn bench_nonce_testing(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_testing");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    group.bench_function("test_nonce_optimized", |b| {
        b.iter(|| {
            let header_hash = [0xA5u8; 32];
            let nonce = 123456789u64;
            let height = 1000u64;

            let mut input = Vec::with_capacity(48);
            input.extend_from_slice(&header_hash);
            input.extend_from_slice(&nonce.to_be_bytes());
            input.extend_from_slice(&height.to_be_bytes());

            let _result = simple_hash(&input);
            black_box(_result);
        });
    });

    group.finish();
}

/// Benchmark: Header hash computation baseline
fn bench_solve_pow_production(c: &mut Criterion) {
    let mut group = c.benchmark_group("solve_pow");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20);

    let block = create_test_block();

    group.bench_function("solve_pow_production_single", |b| {
        b.iter_batched(
            || block.clone(),
            |blk| {
                let header_hash = compute_header_hash(&blk);
                black_box(header_hash);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: Header serialization for compact block relay
fn bench_header_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("header_serialization");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Bytes(280));

    let block = create_test_block();

    group.bench_function("serialize_canonical_header", |b| {
        b.iter(|| {
            let mut header = Vec::with_capacity(512);
            header.extend_from_slice(&block.timestamp.to_be_bytes());
            header.extend_from_slice(&block.chain_id.to_be_bytes());
            header.extend_from_slice(&block.height.to_be_bytes());
            header.extend_from_slice(&block.difficulty.to_be_bytes());
            header.extend_from_slice(block.merkle_root.as_bytes());
            header.extend_from_slice(block.validator.as_bytes());
            header.extend_from_slice(block.miner.as_bytes());
            for parent in &block.parents {
                header.extend_from_slice(parent.as_bytes());
            }
            black_box(header);
        });
    });

    group.finish();
}

/// Benchmark: Short ID computation for compact blocks (blake3)
fn bench_short_id_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("short_id");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    let txids: Vec<String> = (0..1000).map(|i| format!("tx_{:08x}", i)).collect();

    group.bench_function("blake3_short_id_1000", |b| {
        b.iter(|| {
            for txid in &txids {
                let _id = ShortId::from_txid(txid);
            }
        });
    });

    group.bench_function("blake3_short_id_10000", |b| {
        let txids_large: Vec<String> = (0..10000).map(|i| format!("tx_{:08x}", i)).collect();
        b.iter(|| {
            for txid in &txids_large {
                let _id = ShortId::from_txid(txid);
            }
        });
    });

    group.finish();
}

/// Benchmark: Peer scoring & selection (P1-D)
fn bench_peer_scoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("peer_scoring");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    let peers: Vec<PeerId> = (0..50).map(|_| PeerId::random()).collect();

    group.bench_function("select_best_peers_50", |b| {
        let mut scoring = PeerScoring::new(0.01, 10);
        for peer in &peers {
            scoring.update_rtt(peer, 50.0);
            scoring.record_success(peer);
        }

        b.iter(|| {
            let _best = scoring.select_best_peers();
        });
    });

    group.bench_function("update_rtt_50_peers", |b| {
        let mut scoring = PeerScoring::new(0.01, 10);
        b.iter(|| {
            for peer in &peers {
                scoring.update_rtt(peer, 50.0);
            }
        });
    });

    group.bench_function("record_success_failure_50", |b| {
        let mut scoring = PeerScoring::new(0.01, 10);
        b.iter(|| {
            for peer in &peers {
                scoring.record_success(peer);
                scoring.record_failure(peer);
            }
        });
    });

    group.finish();
}

/// Benchmark: Block candidate creation (sync path without actual mining)
fn bench_block_candidate_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_candidate_creation");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50);

    let block = create_test_block();

    group.bench_function("compute_header_hash", |b| {
        b.iter_batched(
            || block.clone(),
            |blk| {
                let header_hash = compute_header_hash(&blk);
                black_box(header_hash);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("serialize_block", |b| {
        b.iter_batched(
            || block.clone(),
            |blk| {
                let serialized = serde_json::to_vec(&blk).expect("serialize");
                black_box(serialized);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark: Chain work computation (U256 arithmetic)
fn bench_chain_work_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_work");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    group.bench_function("add_work_u256", |b| {
        use primitive_types::U256;

        let work = U256::from(1000000u64);
        let addend = U256::from(42u64);

        b.iter(|| {
            let _result = work.saturating_add(addend);
            black_box(_result);
        });
    });

    group.bench_function("mul_div_u256", |b| {
        use primitive_types::U256;

        let work = U256::from(1000000u64);
        let multiplier = 12345u64;

        b.iter(|| {
            let _result = work * multiplier;
            black_box(_result);
        });
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

fn create_test_block() -> QantoBlock {
    QantoBlock {
        protocol_version: qanto::qantodag::QANTO_BLOCK_PROTOCOL_VERSION_V2,
        chain_id: 1,
        id: "test_block_bench_001".to_string(),
        parents: vec![], // no parents to avoid length issues
        transactions: vec![],
        difficulty: 100_000,
        validator: format!("{:0>64}", "val"),
        miner: format!("{:0>64}", "miner"),
        nonce: 0,
        timestamp: 1_700_000_000,
        height: 100,
        reward: 50_000_000_000,
        effort: 0,
        chain_work: qanto::miner::U256::from(1000u64),
        cross_chain_references: vec![],
        cross_chain_swaps: vec![],
        merkle_root: format!("{:0>64}", "merkle"),
        signature: Default::default(),
        homomorphic_encrypted: vec![],
        smart_contracts: vec![],
        carbon_credentials: vec![],
        epoch: 0,
        finality_proof: None,
        reservation_miner_id: None,
        state_parent: String::new(),
        state_root: String::new(),
    }
}

fn compute_header_hash(block: &QantoBlock) -> [u8; 32] {
    let mut header = Vec::with_capacity(512);
    header.extend_from_slice(&block.timestamp.to_be_bytes());
    header.extend_from_slice(&block.chain_id.to_be_bytes());
    header.extend_from_slice(&block.height.to_be_bytes());
    header.extend_from_slice(&block.difficulty.to_be_bytes());
    header.extend_from_slice(block.merkle_root.as_bytes());
    header.extend_from_slice(block.validator.as_bytes());
    header.extend_from_slice(block.miner.as_bytes());
    for parent in &block.parents {
        header.extend_from_slice(parent.as_bytes());
    }
    simple_hash(&header)
}

criterion_group!(
    mining_benches,
    bench_nonce_testing,
    bench_solve_pow_production,
    bench_header_serialization,
    bench_short_id_computation,
    bench_peer_scoring,
    bench_block_candidate_creation,
    bench_chain_work_update,
);
criterion_main!(mining_benches);
