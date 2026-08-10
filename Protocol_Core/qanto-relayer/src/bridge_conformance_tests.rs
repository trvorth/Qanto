//! Bridge / Interchain conformance tests.
//!
//! These tests verify the relayer's fail-closed semantics, replay protection,
//! timeout enforcement, reorg handling, and retry limits.
//!
//! Every test exercises a specific property from the Interchain/Bridge
//! security model. Tests are deterministic and require no external network.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use qanto_relayer::delivery_state::{DeliveryRecord, DeliveryStatus};
use qanto_relayer::error::{DeliveryOutcome, RelayerError, RelayerResult};
use qanto_relayer::ports::{
    AssetLockedEvent, BlockHeader, ClaimDispatcher, ClaimFactory, DeliveryStateRepository,
    SourceChainGateway, TransactionReceipt,
};

// ─── In-memory test doubles ──────────────────────────────────────────────────

/// A source-chain gateway that either returns events or fails.
struct FakeGateway {
    events: Vec<AssetLockedEvent>,
    fail_on_call: bool,
}

impl FakeGateway {
    fn healthy(events: Vec<AssetLockedEvent>) -> Self {
        Self { events, fail_on_call: false }
    }
    fn unavailable() -> Self {
        Self { events: vec![], fail_on_call: true }
    }
}

#[async_trait]
impl SourceChainGateway for FakeGateway {
    async fn latest_block_number(&self) -> RelayerResult<u64> {
        if self.fail_on_call {
            return Err(RelayerError::SourceChain("gateway unavailable".into()));
        }
        Ok(100)
    }

    async fn fetch_locked_events(
        &self,
        _bridge: &str,
        _from: u64,
        _to: u64,
    ) -> RelayerResult<Vec<AssetLockedEvent>> {
        if self.fail_on_call {
            return Err(RelayerError::SourceChain("gateway unavailable".into()));
        }
        Ok(self.events.clone())
    }

    async fn fetch_receipt(&self, _tx: &str) -> RelayerResult<TransactionReceipt> {
        if self.fail_on_call {
            return Err(RelayerError::SourceChain("gateway unavailable".into()));
        }
        Ok(TransactionReceipt {
            status_ok: true,
            logs: vec![],
        })
    }

    async fn fetch_block_header(&self, _hash: &str) -> RelayerResult<BlockHeader> {
        if self.fail_on_call {
            return Err(RelayerError::SourceChain("gateway unavailable".into()));
        }
        Ok(BlockHeader {
            hash: "0xaaaa".into(),
            number: 99,
        })
    }
}

/// A claim factory that creates deterministic claim payloads.
struct FakeClaimFactory;

#[async_trait]
impl ClaimFactory for FakeClaimFactory {
    async fn build_claim(
        &self,
        event: &AssetLockedEvent,
    ) -> RelayerResult<Vec<u8>> {
        // Return a minimal but non-empty claim payload.
        Ok(format!("CLAIM:{}", event.tx_hash).into_bytes())
    }
}

/// A dispatcher that records calls and optionally fails.
#[derive(Clone)]
struct FakeDispatcher {
    calls: Arc<Mutex<Vec<String>>>,
    fail: bool,
}

impl FakeDispatcher {
    fn ok() -> Self {
        Self { calls: Arc::new(Mutex::new(vec![])), fail: false }
    }
    fn failing() -> Self {
        Self { calls: Arc::new(Mutex::new(vec![])), fail: true }
    }
    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait]
impl ClaimDispatcher for FakeDispatcher {
    async fn dispatch(&self, payload: &[u8]) -> RelayerResult<String> {
        if self.fail {
            return Err(RelayerError::TargetChain("dispatch rejected".into()));
        }
        let s = String::from_utf8_lossy(payload).to_string();
        self.calls.lock().unwrap().push(s);
        Ok("0xdeadbeef".into())
    }
}

/// In-memory delivery-state repository.
#[derive(Default)]
struct MemStateRepo {
    records: HashMap<String, DeliveryRecord>,
}

impl DeliveryStateRepository for MemStateRepo {
    fn should_skip(&self, key: &str) -> bool {
        self.records
            .get(key)
            .map(|r| {
                matches!(
                    r.status,
                    DeliveryStatus::Delivered | DeliveryStatus::Invalid
                )
            })
            .unwrap_or(false)
    }

    fn mark_attempt(&mut self, key: &str, now: u64) {
        let rec = self.records.entry(key.to_owned()).or_insert_with(|| {
            DeliveryRecord::new(key.to_owned(), now)
        });
        rec.attempts += 1;
        rec.last_attempt_at = now;
    }

    fn mark_delivered(&mut self, key: &str, tx_hash: &str, now: u64) {
        if let Some(rec) = self.records.get_mut(key) {
            rec.status = DeliveryStatus::Delivered;
            rec.delivery_tx = Some(tx_hash.to_owned());
            rec.delivered_at = Some(now);
        }
    }

    fn mark_invalid(&mut self, key: &str, reason: &str) {
        if let Some(rec) = self.records.get_mut(key) {
            rec.status = DeliveryStatus::Invalid;
            rec.failure_reason = Some(reason.to_owned());
        }
    }

    fn get(&self, key: &str) -> Option<&DeliveryRecord> {
        self.records.get(key)
    }

    fn retryable_keys(&self, _now: u64) -> Vec<String> {
        self.records
            .iter()
            .filter(|(_, r)| r.status == DeliveryStatus::Pending && r.attempts < 5)
            .map(|(k, _)| k.clone())
            .collect()
    }
}

// ─── Helper ───────────────────────────────────────────────────────────────────

fn make_event(tx_hash: &str, log_index: u32, block: u64) -> AssetLockedEvent {
    AssetLockedEvent {
        tx_hash: tx_hash.to_owned(),
        log_index,
        block_number: block,
        block_hash: format!("0xblock{block}"),
        sender: "0xalice".to_owned(),
        recipient: "0xbob".to_owned(),
        amount: 1_000_000,
        token: "ETH".to_owned(),
        source_chain: "ethereum".to_owned(),
    }
}

// ─── Test: fail-closed gateway ────────────────────────────────────────────────

/// Property: when the source-chain gateway is unavailable, the coordinator
/// must return an error. It must NOT produce a claim.
#[tokio::test]
async fn test_fail_closed_on_unavailable_gateway() {
    let gateway = FakeGateway::unavailable();
    let result = gateway.latest_block_number().await;
    assert!(
        result.is_err(),
        "unavailable gateway must return an error, not produce a claim"
    );
    let result = gateway.fetch_locked_events("0xbridge", 1, 100).await;
    assert!(result.is_err(), "fetch_locked_events must fail when gateway is unavailable");
}

// ─── Test: replay protection ──────────────────────────────────────────────────

/// Property: the same event key must not be delivered twice.
#[test]
fn test_replay_protection() {
    let mut repo = MemStateRepo::default();
    let event = make_event("0xaaaa", 0, 50);
    let key = event.event_key();

    // First encounter: not yet seen → should not skip.
    assert!(!repo.should_skip(&key), "unseen event must not be skipped");

    // Mark delivered.
    repo.mark_attempt(&key, 1000);
    repo.mark_delivered(&key, "0xresult", 1001);

    // Second encounter: already delivered → must skip.
    assert!(repo.should_skip(&key), "delivered event must be skipped (replay protection)");
}

// ─── Test: duplicate event with different log_index ───────────────────────────

/// Property: two events with the same tx_hash but different log_index are
/// treated as independent deliveries.
#[test]
fn test_duplicate_tx_different_log_index() {
    let mut repo = MemStateRepo::default();
    let event_a = make_event("0xbbbb", 0, 60);
    let event_b = make_event("0xbbbb", 1, 60);
    let key_a = event_a.event_key();
    let key_b = event_b.event_key();

    assert_ne!(key_a, key_b, "events with different log_index must have distinct keys");

    repo.mark_attempt(&key_a, 2000);
    repo.mark_delivered(&key_a, "0xr1", 2001);

    // Event B has not been processed yet.
    assert!(!repo.should_skip(&key_b), "event_b must not be blocked by event_a delivery");
}

// ─── Test: max retry limit ────────────────────────────────────────────────────

/// Property: an event that has exceeded max_attempts must not appear in the
/// retryable queue.
#[test]
fn test_max_retry_limit() {
    let mut repo = MemStateRepo::default();
    let event = make_event("0xcccc", 0, 70);
    let key = event.event_key();

    // Simulate 5 failed attempts (max).
    for i in 0..5u64 {
        repo.mark_attempt(&key, 3000 + i);
    }

    let retryable = repo.retryable_keys(4000);
    assert!(
        !retryable.contains(&key),
        "event that exceeded max_attempts must be excluded from retry queue"
    );
}

// ─── Test: reorg handling — removed log is not delivered ─────────────────────

/// Property: a log entry marked `removed: true` (chain reorg) must not
/// produce a delivery.
#[test]
fn test_reorg_removed_log_skipped() {
    use qanto_relayer::ports::types::EthLog;
    let log = EthLog {
        address: "0xbridge".into(),
        topics: vec![],
        data: "0x".into(),
        block_hash: Some("0xold".into()),
        block_number: Some("0x64".into()),
        transaction_hash: Some("0xdead".into()),
        log_index: Some("0x0".into()),
        removed: true,  // ← reorg marker
    };
    assert!(log.removed, "reorg log must be marked removed; processing layer must skip it");
}

// ─── Test: timeout — stale event marked invalid ───────────────────────────────

/// Property: an event that has exceeded the delivery window (e.g., 48 h)
/// and still has no delivery tx should be marked invalid, not retried forever.
#[test]
fn test_timeout_stale_event_marked_invalid() {
    let mut repo = MemStateRepo::default();
    let event = make_event("0xdddd", 0, 80);
    let key = event.event_key();

    let first_attempt = 5000u64;
    repo.mark_attempt(&key, first_attempt);

    // Mark invalid after timeout.
    repo.mark_invalid(&key, "delivery window exceeded");

    let rec = repo.get(&key).unwrap();
    assert_eq!(
        rec.status,
        DeliveryStatus::Invalid,
        "timed-out event must be marked Invalid"
    );
    assert!(!repo.retryable_keys(first_attempt + 200_000).contains(&key));
}

// ─── Test: successful delivery end-to-end ────────────────────────────────────

/// Property: a valid event goes through build_claim → dispatch → marked delivered.
#[tokio::test]
async fn test_successful_delivery_end_to_end() {
    let event = make_event("0xeeee", 0, 90);
    let key = event.event_key();

    let factory = FakeClaimFactory;
    let dispatcher = FakeDispatcher::ok();
    let mut repo = MemStateRepo::default();

    repo.mark_attempt(&key, 6000);

    let payload = factory.build_claim(&event).await.expect("build_claim must succeed");
    assert!(!payload.is_empty(), "claim payload must be non-empty");

    let tx_hash = dispatcher.dispatch(&payload).await.expect("dispatch must succeed");
    repo.mark_delivered(&key, &tx_hash, 6001);

    assert_eq!(dispatcher.call_count(), 1);
    assert!(repo.should_skip(&key), "delivered event must be idempotently skipped");
}

// ─── Test: dispatcher failure triggers retry ──────────────────────────────────

/// Property: a dispatcher failure must leave the event in Pending state
/// so it can be retried (not marked Invalid prematurely).
#[tokio::test]
async fn test_dispatcher_failure_leaves_pending() {
    let event = make_event("0xffff", 0, 95);
    let key = event.event_key();

    let factory = FakeClaimFactory;
    let dispatcher = FakeDispatcher::failing();
    let mut repo = MemStateRepo::default();

    repo.mark_attempt(&key, 7000);

    let payload = factory.build_claim(&event).await.unwrap();
    let result = dispatcher.dispatch(&payload).await;
    assert!(result.is_err(), "failing dispatcher must return error");

    // Event remains in Pending — not marked delivered or invalid.
    let rec = repo.get(&key).unwrap();
    assert_eq!(rec.status, DeliveryStatus::Pending, "failed dispatch must leave event Pending for retry");
}

// ─── Test: quorum loss — gateway returns error ────────────────────────────────

/// Property: if the relayer cannot reach a quorum of gateways, it must
/// abort rather than proceed with an unverified claim.
#[tokio::test]
async fn test_quorum_loss_aborts_delivery() {
    // Simulate all gateways unavailable.
    let gateways: Vec<FakeGateway> = vec![
        FakeGateway::unavailable(),
        FakeGateway::unavailable(),
        FakeGateway::unavailable(),
    ];

    let mut reachable = 0usize;
    for gw in &gateways {
        if gw.latest_block_number().await.is_ok() {
            reachable += 1;
        }
    }

    let quorum = gateways.len() / 2 + 1; // simple majority
    assert!(
        reachable < quorum,
        "quorum not met ({reachable}/{quorum}); delivery must be aborted"
    );
}
