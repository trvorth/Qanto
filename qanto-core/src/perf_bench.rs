//! Micro-benchmarks for qanto-core fast paths (Display + balance stream)
//!
//! These are lightweight, production-usable tests that measure throughput
//! without external dependencies. They print results for operator review.
//! Targets: 32 BPS and 10,000,000+ TPS (extrapolated).

use crate::{balance_stream::BalanceBroadcaster, qanto_net::QantoBlock};

#[cfg(test)]
mod benches {
    use super::*;

    #[test]
    fn bench_display_throughput() {
        // Construct a representative block and measure fmt throughput.
        let blk = QantoBlock {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            data: vec![],
            chain_id: 99,
            height: 1_234_567,
            timestamp: 1_700_000_000,
            parents: vec!["parent0123456789abcdef".to_string()],
            transactions_len: 10_000,
        };

        let iterations = 1_000_000; // 1M formats for quick signal
        let start = std::time::Instant::now();
        let mut sink = 0usize;
        for _ in 0..iterations {
            // Force formatting work without heap capture
            let s = format!("{}", blk);
            sink ^= s.len();
        }
        let elapsed = start.elapsed();
        let ops_per_sec = (iterations as f64) / elapsed.as_secs_f64();
        println!(
            "[bench] Display throughput: {:.2} ops/sec (sink={})",
            ops_per_sec, sink
        );
        // No hard assertion; environment-dependent. This prints a useful metric.
    }

    #[test]
    fn bench_balance_broadcast_throughput() {
        // Measure update and broadcast cost; single subscriber to avoid contention.
        let broadcaster = BalanceBroadcaster::new(1 << 16);
        let mut rx = broadcaster.subscribe();

        let iterations = 100_000; // keep small to avoid CI timeouts
        let start = std::time::Instant::now();
        for i in 0..iterations {
            broadcaster.set_balance("bench-addr", i as u128);
            // Drain one message per iteration where possible
            match rx.try_recv() {
                Ok(_update) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
            }
        }
        let elapsed = start.elapsed();
        let ops_per_sec = (iterations as f64) / elapsed.as_secs_f64();
        println!(
            "[bench] Balance broadcast throughput: {:.2} ops/sec",
            ops_per_sec
        );
        // No hard assertion; prints the observed rate.
    }
}
