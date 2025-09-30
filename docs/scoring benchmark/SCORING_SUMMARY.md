# Qanto Component Scores (Rubric Step 6)

This summary captures the per-component scores against the Layer-0 rubric, notes production-blocking issues, and highlights exemplary implementations.

Artifacts
- CSV grid: docs/scoring_matrix.csv (import into Sheets/Excel)
- JSON: docs/scoring_matrix.json (programmatic use, includes heat map scale)

Heat map guidance
- Color scale: 0–1 red, 2–2.9 orange, 3–3.4 yellow, 3.5–4.4 light green, 4.5–5 dark green
- Draw a red border around any cell where Security Infrastructure < 3.0 to reflect gating risk

Top-level observations
- Security gating risk: Some components score < 3.0 in security (notably P2P and Wallet/Keys). Overall maturity should be capped at Pre-alpha until remediated and at least one external audit is completed for consensus/DAG/P2P core.
- Interoperability is early/experimental. Bridge/light client functionality is not integrated into the node; interchain components live in auxiliary crate and are not trust-minimized.
- Performance claims are unverified at the published constants (MAX_BLOCK_SIZE=80MB, MAX_TRANSACTIONS_PER_BLOCK=320k). Provide a reproducible benchmark harness and publish p95/p99 metrics.
- Resilience: Add chaos/DR drills, remove mDNS in production, introduce operator diversity tracking.
- Data/state: Implement pruning modes, snapshot/warp sync, light-client proofs; document state growth targets.

Exemplary implementations to preserve
- Deterministic PoW target using integer U256 arithmetic and fixed-point conversion (src/miner.rs)
- Consensus validation pipeline with PoW-first, PoS advisory, and comprehensive block structure checks (src/consensus.rs)
- DAG parallel Merkle computation and reward reconciliation via SAGA (src/qantodag.rs)
- Node topological sort for sync, P2P startup backoff, API readiness and rate limiting (src/node.rs)
- Mempool eviction/pruning and current-size accounting fix (src/mempool.rs)
- P2P topic partitioning and per-message-type rate limiting; PQ signatures + HMAC on messages (src/p2p.rs)

Blocking issues (must-fix for production)
- Enforce non-default HMAC secret; fail startup when unset or default (src/p2p.rs)
- Introduce secrets management (rotation, storage) and optionally HSM/KMS for keys
- Expand CI with fuzzing/property tests, perf harness, and multi-region runs; publish SLOs
- Commission at least one independent audit on consensus, DAG, and P2P
- Scope interop or implement trust-minimized verification with defined failure modes

How to visualize (suggested)
- Use the CSV grid with conditional formatting using the heat map colors above
- Add a red border rule to Security Infrastructure < 3.0 cells
- Optionally compute a weighted per-component score using rubric weights

For drill-down, see the code references in the evaluation response or search the referenced files and line ranges.

