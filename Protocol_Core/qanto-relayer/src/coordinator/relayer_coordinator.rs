//! Coordinator orchestrates the delivery flow.

use crate::adapters::qanto_claim_factory::K256Signer;
use crate::error::{DeliveryOutcome, RelayerResult};
use crate::ports::{
    AssetLockedEvent, ClaimDispatcher, ClaimFactory, DeliveryStateRepository, SourceChainGateway,
};

/// Coordinator handles the full delivery pipeline for a single event.
pub struct RelayerCoordinator<S, F, D, R>
where
    S: SourceChainGateway,
    F: ClaimFactory,
    D: ClaimDispatcher,
    R: DeliveryStateRepository,
{
    pub source_gateway: S,
    claim_factory: F,
    dispatcher: D,
    pub state_repo: R,
    max_attempts: u32,
    signer: std::sync::Arc<K256Signer>,
    source_chain: String,
}

impl<S, F, D, R> RelayerCoordinator<S, F, D, R>
where
    S: SourceChainGateway,
    F: ClaimFactory,
    D: ClaimDispatcher,
    R: DeliveryStateRepository,
{
    pub fn new(
        source_gateway: S,
        claim_factory: F,
        dispatcher: D,
        state_repo: R,
        max_attempts: u32,
    ) -> Self {
        let dummy_key = k256::ecdsa::SigningKey::from_slice(&[1u8; 32]).unwrap();
        let signer = std::sync::Arc::new(K256Signer::new(dummy_key));

        Self {
            source_gateway,
            claim_factory,
            dispatcher,
            state_repo,
            max_attempts,
            source_chain: "ethereum".to_string(),
            signer,
        }
    }

    pub fn with_signer(mut self, signer: std::sync::Arc<K256Signer>) -> Self {
        self.signer = signer;
        self
    }

    pub fn with_source_chain(mut self, chain: String) -> Self {
        self.source_chain = chain;
        self
    }

    pub fn signer(&self) -> &K256Signer {
        &self.signer
    }

    /// Process a retryable event key.
    pub async fn process_retry(
        &mut self,
        event_key: &str,
        now_unix_secs: u64,
    ) -> RelayerResult<DeliveryOutcome> {
        self.state_repo.mark_attempt(event_key, now_unix_secs);

        let outcome = self.deliver_from_key(event_key, now_unix_secs).await;

        self.apply_outcome(event_key, now_unix_secs, &outcome)
            .await?;

        Ok(outcome)
    }

    /// Process a newly discovered event from a log.
    pub async fn process_new_event(
        &mut self,
        event: &AssetLockedEvent,
        now_unix_secs: u64,
    ) -> RelayerResult<DeliveryOutcome> {
        let event_key = event.event_key();

        if self.state_repo.should_skip(&event_key) {
            return Ok(DeliveryOutcome::Invalid {
                reason: "already processed".into(),
            });
        }

        self.state_repo.mark_attempt(&event_key, now_unix_secs);

        let outcome = self.deliver_event(event, now_unix_secs).await;

        self.apply_outcome(&event_key, now_unix_secs, &outcome)
            .await?;

        Ok(outcome)
    }

    /// Scan and process new bridge events in a block range.
    pub async fn scan_and_deliver(
        &mut self,
        from_block: u64,
        to_block: u64,
        bridge_address: &str,
    ) -> RelayerResult<usize> {
        let now = Self::now_unix_secs();

        let events = self
            .source_gateway
            .fetch_locked_events(bridge_address, from_block, to_block)
            .await?;

        let mut delivered = 0;
        for event in events {
            let event_key = event.event_key();

            if self.state_repo.should_skip(&event_key) {
                continue;
            }

            if !self.state_repo.should_retry(&event_key, now, 10) {
                continue;
            }

            self.state_repo.mark_attempt(&event_key, now);
            self.state_repo.save().await?;

            let outcome = self.deliver_event(&event, now).await;
            self.apply_outcome(&event_key, now, &outcome).await?;

            if outcome.is_delivered() {
                delivered += 1;
            }
        }

        Ok(delivered)
    }

    async fn deliver_event(
        &self,
        event: &AssetLockedEvent,
        _now_unix_secs: u64,
    ) -> DeliveryOutcome {
        // Build claim
        let claim = match self
            .claim_factory
            .build_claim(event, &self.source_chain, self.signer.as_ref())
            .await
        {
            Ok(c) => c,
            Err(e) => {
                return DeliveryOutcome::Invalid {
                    reason: format!("claim build: {}", e),
                }
            }
        };

        // Dispatch
        match self.dispatcher.dispatch(&claim).await {
            Ok(tx_hash) => DeliveryOutcome::Delivered {
                claim_tx_hash: tx_hash,
            },
            Err(e) => crate::error::classify_qanto_error(&e.to_string()),
        }
    }

    async fn deliver_from_key(&self, _event_key: &str, _now_unix_secs: u64) -> DeliveryOutcome {
        // For retry, we need to re-fetch the event from source chain
        // This is a simplified implementation
        DeliveryOutcome::Retryable {
            reason: "refetch required".into(),
        }
    }

    async fn apply_outcome(
        &mut self,
        event_key: &str,
        now_unix_secs: u64,
        outcome: &DeliveryOutcome,
    ) -> RelayerResult<()> {
        match outcome {
            DeliveryOutcome::Delivered { claim_tx_hash } => {
                self.state_repo
                    .mark_delivered(event_key, now_unix_secs, claim_tx_hash.clone());
            }
            DeliveryOutcome::Invalid { reason } => {
                self.state_repo
                    .mark_invalid(event_key, now_unix_secs, reason.clone());
            }
            DeliveryOutcome::Retryable { reason } => {
                self.state_repo.mark_retryable(
                    event_key,
                    now_unix_secs,
                    self.max_attempts,
                    reason.clone(),
                );
            }
            DeliveryOutcome::Exhausted { error } => {
                self.state_repo
                    .mark_invalid(event_key, now_unix_secs, error.clone());
            }
            DeliveryOutcome::Duplicate { claim_tx_hash } => {
                self.state_repo
                    .mark_delivered(event_key, now_unix_secs, claim_tx_hash.clone());
            }
        }
        self.state_repo.save().await
    }

    pub fn state_repo_mut(&mut self) -> &mut R {
        &mut self.state_repo
    }

    pub fn source_gateway(&self) -> &S {
        &self.source_gateway
    }

    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_type_exists() {
        // Compile-time test: verify the coordinator type is generic over its ports
        fn _assert_types<
            S: SourceChainGateway,
            F: ClaimFactory,
            D: ClaimDispatcher,
            R: DeliveryStateRepository,
        >() {
            let _ = std::marker::PhantomData::<RelayerCoordinator<S, F, D, R>>;
        }
    }
}
