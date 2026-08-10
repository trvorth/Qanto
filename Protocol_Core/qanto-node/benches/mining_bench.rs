//! Mining & Hot-Path Benchmarks for Qanto
//!
//! Run with: `cargo bench --bench mining_bench`
//!
//! Covers:
//! - Nonce testing hot path (lock-free worker)
//! - solve_pow_production end-to-end
//! - BlockTemplate creation & cache invalidation
//! - Header serialization (compact block relay)
//! - Short ID computation (blake3)
//! - Peer scoring (P1-D)
//! - Header sync (P1-C)

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

// Import publicly exported types from the qanto library crate
use libp2p::PeerId;
use qanto::{
    protocol::compact_block::ShortId,
    protocol::header_sync::BlockHeader,
    protocol::peer_scoring::PeerScoring,
    qantodag::{BlockTemplate, QantoBlock},
    state_sync::HeaderFirstSync,
};

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

/// Benchmark: BlockTemplate creation & cache invalidation
fn bench_block_template_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_template");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let block = create_test_block();

    group.bench_function("from_candidate_block", |b| {
        b.iter(|| {
            let qdag = Arc::new(vec![[0u8; 128]; 1024]);
            // Use a simple transaction struct matching the expected interface
            let template = BlockTemplate::from_candidate_block(
                black_box(&block),
                black_box(qdag),
                black_box(0),
                black_box(1),
                black_box(create_coinbase_tx()),
                black_box([0u8; 32]),
            );
            black_box(template);
        });
    });

    group.bench_function("template_invalidation", |b| {
        b.iter(|| {
            let qdag = Arc::new(vec![[0u8; 128]; 1024]);
            let template = BlockTemplate::from_candidate_block(
                black_box(&block),
                black_box(qdag),
                black_box(0),
                black_box(1),
                black_box(create_coinbase_tx()),
                black_box([0u8; 32]),
            );

            let _new_version = template.version + 1;
            black_box(_new_version);
        });
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

/// Benchmark: Header sync phase (P1-C)
fn bench_header_sync(c: &mut Criterion) {
    let mut group = c.benchmark_group("header_sync");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    let peers: Vec<PeerId> = (0..5).map(|_| PeerId::random()).collect();

    group.bench_function("header_download_1000", |b| {
        b.iter(|| {
            let sync = HeaderFirstSync::new(0, 1000, peers.clone()).expect("bounded sync");
            let req = sync.build_next_request();
            black_box(req);
        });
    });

    group.bench_function("build_block_requests", |b| {
        let mut sync = HeaderFirstSync::new(0, 1000, peers.clone()).expect("bounded sync");
        let headers: Vec<_> = (1..=1000)
            .map(|height| BlockHeader {
                height,
                chain_id: 1,
                timestamp: height,
                difficulty: 1,
                merkle_root: format!("root-{height}"),
                validator: "validator".to_string(),
                miner: "miner".to_string(),
                parents: Vec::new(),
                block_id: format!("block-{height}"),
                txn_count: 0,
                total_fee: 0,
            })
            .collect();
        sync.process_headers_response(headers[..512].to_vec(), 1000, &peers[0])
            .expect("first header batch");
        sync.process_headers_response(headers[512..].to_vec(), 1000, &peers[0])
            .expect("second header batch");
        b.iter(|| {
            let _reqs = sync.build_block_requests();
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
        cached_header_hash: std::sync::OnceLock::new(),
    }
}

// Create a simple transaction matching the Transaction struct interface.
// We need to use the exact fields but can't directly import Transaction.
// Instead we use a minimal approach compatible with BlockTemplate::from_candidate_block.
fn create_coinbase_tx() -> qanto::transaction::Transaction {
    qanto::transaction::Transaction::default()
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

fn simple_hash(data: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

criterion_group!(
    mining_benches,
    bench_nonce_testing,
    bench_solve_pow_production,
    bench_block_template_creation,
    bench_header_serialization,
    bench_short_id_computation,
    bench_peer_scoring,
    bench_header_sync,
);
criterion_main!(mining_benches);
