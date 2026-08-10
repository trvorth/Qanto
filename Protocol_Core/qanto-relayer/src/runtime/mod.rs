//! Relayer runtime - daemon loop and orchestration.

use std::time::Instant;
use tokio::time::sleep;

use crate::adapters::qanto_claim_factory::K256Signer;
use crate::adapters::{
    EvmRpcGateway, FileDeliveryStateRepo, QantoClaimFactory, QantoRpcDispatcher,
};
use crate::config::RelayerConfig;
use crate::coordinator::RelayerCoordinator;
use crate::error::RelayerResult;
use crate::ports::{DeliveryStateRepository, RelayerSigner};

/// Relayer runtime configuration and main loop.
pub struct RelayerRuntime<
    S: crate::ports::SourceChainGateway,
    F: crate::ports::ClaimFactory,
    D: crate::ports::ClaimDispatcher,
    R: crate::ports::DeliveryStateRepository,
> {
    coordinator: RelayerCoordinator<S, F, D, R>,
    config: RelayerConfig,
    last_processed_block: u64,
}

impl<
        S: crate::ports::SourceChainGateway,
        F: crate::ports::ClaimFactory,
        D: crate::ports::ClaimDispatcher,
        R: crate::ports::DeliveryStateRepository,
    > RelayerRuntime<S, F, D, R>
{
    pub async fn new(
        coordinator: RelayerCoordinator<S, F, D, R>,
        config: RelayerConfig,
    ) -> RelayerResult<Self> {
        let latest_block = coordinator.source_gateway().latest_block_number().await?;
        let last_processed_block = config
            .start_block
            .map(|b| b.saturating_sub(1))
            .unwrap_or(latest_block);

        Ok(Self {
            coordinator,
            config,
            last_processed_block,
        })
    }

    /// Run one tick of the relayer loop.
    pub async fn tick(&mut self) -> RelayerResult<()> {
        let now = Self::now_unix_secs();

        // Process retries
        let retry_keys = self
            .coordinator
            .state_repo_mut()
            .pending_retry_keys(now, self.config.retry_backoff.as_secs());
        for event_key in retry_keys
            .into_iter()
            .take(self.config.max_retries_per_poll)
        {
            if self.coordinator.state_repo_mut().should_skip(&event_key) {
                continue;
            }
            if !self.coordinator.state_repo_mut().should_retry(
                &event_key,
                now,
                self.config.retry_backoff.as_secs(),
            ) {
                continue;
            }

            if let Err(e) = self.coordinator.process_retry(&event_key, now).await {
                eprintln!("Retry processing failed for {}: {}", event_key, e);
            }
        }

        // Fetch latest block
        let latest_block = match self
            .coordinator
            .source_gateway()
            .latest_block_number()
            .await
        {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Failed to fetch latest block: {}", e);
                sleep(self.config.poll_interval).await;
                return Ok(());
            }
        };

        let safe_latest = latest_block.saturating_sub(self.config.confirmations.saturating_sub(1));

        if safe_latest <= self.last_processed_block {
            sleep(self.config.poll_interval).await;
            return Ok(());
        }

        let from_block = self.last_processed_block.saturating_add(1);
        let to_block = safe_latest;

        println!(
            "[*] Scanning bridge logs from block {} to {}",
            from_block, to_block
        );

        let delivered = self
            .coordinator
            .scan_and_deliver(from_block, to_block, &self.config.bridge_address)
            .await?;

        if delivered > 0 {
            println!("[+] Delivered {} claims", delivered);
        }

        self.last_processed_block = to_block;

        // Prune old state
        let now = Self::now_unix_secs();
        if self
            .coordinator
            .state_repo_mut()
            .prune_completed(now, self.config.state_retention.as_secs())
        {
            self.coordinator.state_repo_mut().save().await?;
        }

        sleep(self.config.poll_interval).await;
        Ok(())
    }

    /// Run the relayer loop indefinitely.
    pub async fn run(mut self) -> RelayerResult<()> {
        println!("========================================================");
        println!("IGNITING QANTO CROSS-CHAIN RELAYER DAEMON");
        println!("========================================================");
        println!("[+] Source chain: {}", self.config.source_chain);
        println!("[+] Source RPC: {}", self.config.eth_rpc_url);
        println!("[+] QANTO RPC: {}", self.config.qanto_rpc_url);
        println!("[+] Bridge contract: {}", self.config.bridge_address);
        println!(
            "[+] Relayer signer: {}",
            self.coordinator.signer().address()
        );
        println!(
            "[+] Poll interval: {}s",
            self.config.poll_interval.as_secs()
        );
        println!("[+] Confirmations: {}", self.config.confirmations);
        println!("[+] State file: {}", self.config.state_file);

        println!(
            "[*] Starting EVM log poller from block {}",
            self.last_processed_block.saturating_add(1)
        );

        loop {
            let start = Instant::now();
            if let Err(e) = self.tick().await {
                eprintln!("Tick error: {}", e);
            }
            let elapsed = start.elapsed();
            if elapsed < self.config.poll_interval {
                sleep(self.config.poll_interval - elapsed).await;
            }
        }
    }

    fn now_unix_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Builder for creating the runtime with all dependencies wired up.
pub struct RuntimeBuilder {
    config: RelayerConfig,
}

impl RuntimeBuilder {
    pub fn new(config: RelayerConfig) -> Self {
        Self { config }
    }

    pub async fn build(
        self,
    ) -> RelayerResult<
        RelayerRuntime<EvmRpcGateway, QantoClaimFactory, QantoRpcDispatcher, FileDeliveryStateRepo>,
    > {
        let client = reqwest::Client::new();

        let source_gateway = EvmRpcGateway::new(
            client.clone(),
            self.config.eth_rpc_url.clone(),
            self.config.bridge_address.clone(),
        );

        let claim_factory = QantoClaimFactory;
        let dispatcher = QantoRpcDispatcher::new(client, self.config.qanto_rpc_url.clone());
        let mut state_repo = FileDeliveryStateRepo::new(&self.config.state_file);
        state_repo.load().await?;

        let signer = std::sync::Arc::new(K256Signer::new(self.config.relayer_signing_key.clone()));

        let coordinator = RelayerCoordinator::new(
            source_gateway,
            claim_factory,
            dispatcher,
            state_repo,
            self.config.max_attempts,
        )
        .with_signer(signer)
        .with_source_chain(self.config.source_chain.clone());

        RelayerRuntime::new(coordinator, self.config).await
    }
}
