use ahash::AHashMap as HashMap;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use sha3::{Digest, Keccak256};
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::mempool::Mempool;
use crate::p2p::P2PCommand;
use crate::persistence::{balance_key, decode_balance};
use crate::qantodag::QantoDAG;
use crate::types::UTXO;
use crate::websocket_server::BalanceEvent;
use qanto_rpc::server::generated as proto;

#[derive(Clone, Debug)]
pub enum RpcTransactionUpdate {
    Mempool {
        tx: crate::transaction::Transaction,
    },
    Confirmed {
        tx: crate::transaction::Transaction,
        block_id: String,
        block_height: u64,
        block_timestamp: u64,
    },
    Finalized {
        tx: crate::transaction::Transaction,
        block_id: String,
        block_height: u64,
        block_timestamp: u64,
    },
}

pub struct NodeRpcBackend {
    pub dag: Arc<QantoDAG>,
    pub utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub p2p_sender: mpsc::Sender<P2PCommand>,
    pub rpc_balance_sender: broadcast::Sender<proto::BalanceUpdate>,
    pub rpc_transaction_sender: broadcast::Sender<proto::AddressTransactionEntry>,
    pub balance_cache: Arc<RwLock<HashMap<String, u128>>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BridgeMintRequest {
    pub source_tx_hash: String,
    pub amount: String,
    pub recipient: String,
    pub source_chain: String,
    pub relayer_signatures: Vec<String>,
    pub merkle_proof: String,
    pub receipt_proof: String,
    pub block_header: String,
}

#[derive(Clone, Debug)]
pub struct BridgeClaimExecution {
    pub tx_id: String,
    pub recipient: String,
    pub amount: u128,
}

impl BridgeMintRequest {
    fn normalize_hex_field(field: &str, value: &str, expected_len: Option<usize>) -> Result<String, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(format!("{field} cannot be empty"));
        }

        let without_prefix = trimmed.trim_start_matches("0x").trim_start_matches("0X");
        if without_prefix.is_empty() || without_prefix.len() % 2 != 0 {
            return Err(format!("{field} must be valid hex data"));
        }
        if let Some(expected_len) = expected_len {
            if without_prefix.len() != expected_len {
                return Err(format!(
                    "{field} must be {expected_len} hex characters, got {}",
                    without_prefix.len()
                ));
            }
        }
        hex::decode(without_prefix).map_err(|_| format!("{field} must decode as hex bytes"))?;
        Ok(format!("0x{}", without_prefix.to_lowercase()))
    }

    fn normalize_recipient(&self) -> Result<String, String> {
        let trimmed = self.recipient.trim();
        if trimmed.is_empty() {
            return Err("recipient cannot be empty".to_string());
        }

        let without_prefix = trimmed.trim_start_matches("0x").trim_start_matches("0X");
        match without_prefix.len() {
            40 => {
                hex::decode(without_prefix)
                    .map_err(|_| "recipient must be a valid 20-byte EVM address".to_string())?;
                Ok(crate::transaction::pad_ethereum_address(trimmed))
            }
            64 => {
                hex::decode(without_prefix)
                    .map_err(|_| "recipient must be a valid padded 32-byte address".to_string())?;
                Ok(without_prefix.to_lowercase())
            }
            _ => Err("recipient must be 20-byte EVM hex or 32-byte padded hex".to_string()),
        }
    }

    fn normalized_signatures(&self) -> Result<Vec<String>, String> {
        if self.relayer_signatures.is_empty() {
            return Err("relayer_signatures cannot be empty".to_string());
        }

        self.relayer_signatures
            .iter()
            .map(|signature| {
                let normalized =
                    Self::normalize_hex_field("relayer_signatures[]", signature, Some(130))?;
                let sig_bytes = hex::decode(normalized.trim_start_matches("0x"))
                    .map_err(|_| "relayer_signatures[] must decode as 65 bytes".to_string())?;
                if sig_bytes.len() != 65 {
                    return Err("relayer_signatures[] must decode as 65 bytes".to_string());
                }
                Ok(normalized)
            })
            .collect()
    }

    fn build_transaction(
        &self,
    ) -> Result<(crate::transaction::Transaction, String, u128), String> {
        let source_tx_hash =
            Self::normalize_hex_field("source_tx_hash", &self.source_tx_hash, Some(64))?;
        let recipient = self.normalize_recipient()?;
        let amount = self
            .amount
            .trim()
            .parse::<u128>()
            .map_err(|_| "amount must be a base-unit u128 string".to_string())?;
        if amount == 0 {
            return Err("amount must be greater than zero".to_string());
        }
        let source_chain = self.source_chain.trim();
        if source_chain.is_empty() {
            return Err("source_chain cannot be empty".to_string());
        }
        let relayer_signatures = self.normalized_signatures()?;
        let merkle_proof =
            Self::normalize_hex_field("merkle_proof", &self.merkle_proof, None)?;
        let receipt_proof =
            Self::normalize_hex_field("receipt_proof", &self.receipt_proof, None)?;
        let block_header =
            Self::normalize_hex_field("block_header", &self.block_header, None)?;

        let mut metadata = HashMap::new();
        metadata.insert("bridge_source_tx_hash".to_string(), source_tx_hash.clone());
        metadata.insert("bridge_amount".to_string(), amount.to_string());
        metadata.insert("bridge_recipient".to_string(), recipient.clone());
        metadata.insert("bridge_source_chain".to_string(), source_chain.to_string());
        metadata.insert(
            "bridge_relayer_signatures".to_string(),
            relayer_signatures.join(","),
        );
        metadata.insert("bridge_merkle_proof".to_string(), merkle_proof.clone());
        metadata.insert("bridge_receipt_proof".to_string(), receipt_proof.clone());
        metadata.insert("bridge_block_header".to_string(), block_header.clone());

        let bridge_system_address =
            crate::transaction::pad_ethereum_address("0x9F00000000000000000000000000000000000013");
        let mut hasher = Keccak256::new();
        hasher.update(source_chain.as_bytes());
        hasher.update(source_tx_hash.as_bytes());
        hasher.update(recipient.as_bytes());
        hasher.update(amount.to_string().as_bytes());
        hasher.update(merkle_proof.as_bytes());
        hasher.update(receipt_proof.as_bytes());
        hasher.update(block_header.as_bytes());
        hasher.update(relayer_signatures.join(",").as_bytes());
        let tx_id = hex::encode(hasher.finalize());

        let tx = crate::transaction::Transaction {
            id: tx_id.clone(),
            sender: bridge_system_address.clone(),
            receiver: bridge_system_address,
            amount,
            fee: 0,
            gas_limit: 250_000,
            gas_used: 0,
            gas_price: 1,
            priority_fee: 0,
            inputs: Vec::new(),
            outputs: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            metadata,
            signature: crate::types::QuantumResistantSignature {
                signer_public_key: Vec::new(),
                signature: Vec::new(),
            },
            fee_breakdown: None,
            transaction_kind: crate::transaction::TransactionKind::BridgeClaim,
            chain_id: crate::transaction::GLOBAL_CHAIN_ID
                .load(std::sync::atomic::Ordering::Relaxed) as u32,
        };

        Ok((tx, recipient, amount))
    }
}

impl NodeRpcBackend {
    pub fn new(
        dag: Arc<QantoDAG>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        mempool: Arc<RwLock<Mempool>>,
        p2p_sender: mpsc::Sender<P2PCommand>,
        ws_balance_sender: broadcast::Sender<BalanceEvent>,
        transaction_event_sender: broadcast::Sender<RpcTransactionUpdate>,
    ) -> Self {
        let (rpc_balance_sender, _) = broadcast::channel(1000);
        let (rpc_transaction_sender, _) = broadcast::channel(4000);
        let balance_cache = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut ws_rx = ws_balance_sender.subscribe();
            let rpc_tx = rpc_balance_sender.clone();
            let cache = balance_cache.clone();
            tokio::spawn(async move {
                loop {
                    match ws_rx.recv().await {
                        Ok(ev) => {
                            // Update live cache
                            {
                                let mut cache_guard = cache.write().await;
                                cache_guard.insert(ev.address.clone(), ev.balance);
                            }
                            // Bridge to RPC subscribers
                            let update = proto::BalanceUpdate {
                                address: ev.address,
                                base_units: ev.balance.to_string(),
                                timestamp: ev.timestamp,
                            };
                            let _ = rpc_tx.send(update);
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }
        {
            let mut updates_rx = transaction_event_sender.subscribe();
            let rpc_tx = rpc_transaction_sender.clone();
            let dag = dag.clone();
            tokio::spawn(async move {
                loop {
                    match updates_rx.recv().await {
                        Ok(event) => {
                            let entry =
                                NodeRpcBackend::build_transaction_update_entry(&dag, event).await;
                            let _ = rpc_tx.send(entry);
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        Self {
            dag,
            utxos,
            mempool,
            p2p_sender,
            rpc_balance_sender,
            rpc_transaction_sender,
            balance_cache,
        }
    }

    fn status_code(in_mempool: bool, is_finalized: bool) -> i32 {
        if in_mempool {
            0
        } else if is_finalized {
            2
        } else {
            1
        }
    }

    fn build_proto_entry(
        tx: &crate::transaction::Transaction,
        block_id: String,
        block_height: u64,
        block_timestamp: u64,
        confirmations: u64,
        is_finalized: bool,
        in_mempool: bool,
        event_kind: i32,
    ) -> proto::AddressTransactionEntry {
        proto::AddressTransactionEntry {
            transaction: Some(convert_internal_tx_to_proto(tx)),
            block_id,
            block_height,
            block_timestamp,
            confirmations,
            is_finalized,
            in_mempool,
            status: Self::status_code(in_mempool, is_finalized),
            event_kind,
        }
    }

    async fn build_transaction_update_entry(
        dag: &QantoDAG,
        event: RpcTransactionUpdate,
    ) -> proto::AddressTransactionEntry {
        let latest_height = dag
            .get_latest_block()
            .await
            .map(|b| b.height)
            .unwrap_or_default();

        match event {
            RpcTransactionUpdate::Mempool { tx } => {
                Self::build_proto_entry(&tx, String::new(), 0, tx.timestamp, 0, false, true, 1)
            }
            RpcTransactionUpdate::Confirmed {
                tx,
                block_id,
                block_height,
                block_timestamp,
            } => {
                let confirmations = if latest_height >= block_height {
                    latest_height.saturating_sub(block_height).saturating_add(1)
                } else {
                    1
                };
                let is_finalized = dag
                    .finalized_blocks
                    .get(&block_id)
                    .map(|flag| *flag.value())
                    .unwrap_or(false);

                Self::build_proto_entry(
                    &tx,
                    block_id,
                    block_height,
                    block_timestamp,
                    confirmations,
                    is_finalized,
                    false,
                    2,
                )
            }
            RpcTransactionUpdate::Finalized {
                tx,
                block_id,
                block_height,
                block_timestamp,
            } => {
                let confirmations = if latest_height >= block_height {
                    latest_height.saturating_sub(block_height).saturating_add(1)
                } else {
                    1
                };

                Self::build_proto_entry(
                    &tx,
                    block_id,
                    block_height,
                    block_timestamp,
                    confirmations,
                    true,
                    false,
                    3,
                )
            }
        }
    }
}

pub(crate) async fn ingest_transaction_locally_and_broadcast(
    tx: crate::transaction::Transaction,
    mempool: &RwLock<Mempool>,
    utxos: &RwLock<HashMap<String, UTXO>>,
    dag: &QantoDAG,
    p2p_sender: &mpsc::Sender<P2PCommand>,
) -> Result<(), String> {
    let utxos_guard = utxos.read().await;
    {
        let mempool_guard = mempool.read().await;
        mempool_guard
            .add_transaction(tx.clone(), &utxos_guard, dag)
            .await
            .map_err(|e| format!("Failed to add transaction to local mempool: {e}"))?;
    }

    p2p_sender
        .send(P2PCommand::BroadcastTransaction(tx))
        .await
        .map_err(|e| format!("Failed to broadcast transaction: {e}"))?;

    Ok(())
}

use crate::p2p::{
    convert_internal_block_to_proto as convert_block_to_proto, convert_internal_tx_to_proto,
    convert_proto_tx,
};

#[async_trait]
impl qanto_rpc::RpcBackend for NodeRpcBackend {
    async fn submit_transaction(&self, tx: proto::Transaction) -> Result<(), String> {
        let parsed = convert_proto_tx(tx)?;

        parsed
            .verify_with_shared_utxos(&self.dag, &self.utxos)
            .await
            .map_err(|e| format!("Transaction verification failed: {e}"))?;

        if let Err(e) = parsed.validate_for_mempool() {
            return Err(format!("Mempool validation failed: {e}"));
        }

        ingest_transaction_locally_and_broadcast(
            parsed.clone(),
            &self.mempool,
            &self.utxos,
            &self.dag,
            &self.p2p_sender,
        )
        .await?;

        Ok(())
    }

    async fn get_wallet_balance(&self, address: String) -> Result<(u128, u128), String> {
        if address.len() != 64 || !address.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid address format".to_string());
        }

        // Confirmed balance: prefer live cache, fallback to UTXO scan, then storage
        let confirmed = {
            // Try live cache first
            if let Some(v) = { self.balance_cache.read().await.get(&address).cloned() } {
                v
            } else {
                // UTXO scan
                let utxos = self.utxos.read().await;
                let mut total: u128 = 0;
                for utxo in utxos.values() {
                    if utxo.address == address {
                        total = total.saturating_add(utxo.amount);
                    }
                }
                // If UTXO scan yielded a value, update cache and use it. Otherwise, fallback to storage.
                if total > 0 {
                    let mut cache = self.balance_cache.write().await;
                    cache.insert(address.clone(), total);
                    total
                } else {
                    let key = balance_key(&address);
                    match self.dag.db.get(&key) {
                        Ok(Some(bytes)) => match decode_balance(&bytes) {
                            Ok(v) => {
                                let mut cache = self.balance_cache.write().await;
                                cache.insert(address.clone(), v);
                                v
                            }
                            Err(_) => 0,
                        },
                        _ => 0,
                    }
                }
            }
        };

        // Unconfirmed delta: net effect from mempool transactions
        let mempool_map = {
            let mp = self.mempool.read().await;
            mp.get_transactions().await
        };
        let utxos = self.utxos.read().await;

        let mut incoming: u128 = 0;
        let mut outgoing: u128 = 0;

        for tx in mempool_map.values() {
            // Sum outputs directed to this address
            for o in &tx.outputs {
                if o.address == address {
                    incoming = incoming.saturating_add(o.amount);
                }
            }
            // Sum inputs that spend this address's UTXOs
            for i in &tx.inputs {
                let utxo_key = format!("{}_{}", i.tx_id, i.output_index);
                if let Some(u) = utxos.get(&utxo_key) {
                    if u.address == address {
                        outgoing = outgoing.saturating_add(u.amount);
                    }
                }
            }
        }

        let unconfirmed_delta = incoming.saturating_sub(outgoing);
        Ok((confirmed, unconfirmed_delta))
    }

    async fn get_balance(&self, address: String) -> Result<u128, String> {
        match self.get_wallet_balance(address).await {
            Ok((confirmed, _)) => Ok(confirmed),
            Err(e) => Err(e),
        }
    }

    async fn get_block(&self, block_id: String) -> Result<proto::QantoBlock, String> {
        // Fetch block from DAG and convert to typed proto
        match self.dag.get_block(&block_id).await {
            Some(block) => Ok(convert_block_to_proto(&block)),
            None => Err("Block not found".to_string()),
        }
    }

    async fn get_network_stats(&self) -> Result<qanto_rpc::NetworkStats, String> {
        // Pull core metrics from DAG performance metrics
        let metrics = &self.dag.performance_metrics;
        let tps = metrics.calculate_real_time_tps();
        let bps = metrics.get_bps();
        let finality_ms = metrics.get_finality_ms();
        // get_network_throughput() returns MB/s; convert to Mbps
        let network_throughput_mbps = (metrics.get_network_throughput() * 8) as f64;

        // Mempool stats
        let (mempool_tx_count, mempool_size_bytes) = {
            let mp = self.mempool.read().await;
            let count = mp.len().await as u64;
            let size = mp.get_current_size_bytes() as u64;
            (count, size)
        };

        // Connected peers via P2P oneshot
        let connected_peers = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.p2p_sender
                .send(P2PCommand::GetConnectedPeers {
                    response_sender: tx,
                })
                .await
                .map_err(|e| format!("Failed to request connected peers: {e}"))?;
            let peers = rx
                .await
                .map_err(|e| format!("Failed to receive peer list: {e}"))?;

            peers.len() as u64
        };

        // Block count from DAG
        let block_count = self.dag.get_block_count().await;

        Ok(qanto_rpc::NetworkStats {
            tps: tps as f64,
            bps: bps as f64,
            mempool_tx_count,
            mempool_size_bytes,
            connected_peers,
            block_count,
            finality_ms,
            network_throughput_mbps,
        })
    }

    fn balance_updates_receiver(&self) -> tokio::sync::broadcast::Receiver<proto::BalanceUpdate> {
        self.rpc_balance_sender.subscribe()
    }

    fn transaction_updates_receiver(
        &self,
    ) -> tokio::sync::broadcast::Receiver<proto::AddressTransactionEntry> {
        self.rpc_transaction_sender.subscribe()
    }

    async fn get_transactions_by_address(
        &self,
        address: String,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<proto::AddressTransactionEntry>, u64), String> {
        if address.len() != 64 || !address.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid address format".to_string());
        }

        let page = page.max(1);
        let page_size = page_size.clamp(1, 200);
        let latest_height = self
            .dag
            .get_latest_block()
            .await
            .map(|b| b.height)
            .unwrap_or_default();

        let mut entries: Vec<proto::AddressTransactionEntry> = Vec::new();
        let mut seen_tx_ids = HashSet::new();

        let mut confirmed_items = self.dag.get_address_history(&address);
        confirmed_items.sort_by(|a, b| {
            b.block_height
                .cmp(&a.block_height)
                .then_with(|| b.tx.timestamp.cmp(&a.tx.timestamp))
        });

        for it in confirmed_items {
            if !seen_tx_ids.insert(it.tx.id.clone()) {
                continue;
            }
            let block_timestamp = self
                .dag
                .blocks
                .get(&it.block_id)
                .map(|entry| entry.value().timestamp)
                .unwrap_or(it.tx.timestamp);
            let confirmations = if latest_height >= it.block_height {
                latest_height
                    .saturating_sub(it.block_height)
                    .saturating_add(1)
            } else {
                0
            };
            let is_finalized = self
                .dag
                .finalized_blocks
                .get(&it.block_id)
                .map(|flag| *flag.value())
                .unwrap_or(false);

            entries.push(Self::build_proto_entry(
                &it.tx,
                it.block_id.clone(),
                it.block_height,
                block_timestamp,
                confirmations,
                is_finalized,
                false,
                0,
            ));
        }

        let mempool_map = {
            let mp = self.mempool.read().await;
            mp.get_transactions().await
        };
        for tx in mempool_map.values() {
            let matches_sender = tx.sender == address;
            let matches_receiver = tx.receiver == address;
            let matches_outputs = tx.outputs.iter().any(|output| output.address == address);
            if !(matches_sender || matches_receiver || matches_outputs) {
                continue;
            }
            if !seen_tx_ids.insert(tx.id.clone()) {
                continue;
            }

            entries.push(Self::build_proto_entry(
                tx,
                String::new(),
                0,
                tx.timestamp,
                0,
                false,
                true,
                0,
            ));
        }

        entries.sort_by(|a, b| {
            b.in_mempool
                .cmp(&a.in_mempool)
                .then_with(|| b.block_timestamp.cmp(&a.block_timestamp))
                .then_with(|| b.block_height.cmp(&a.block_height))
        });

        let total = entries.len() as u64;
        let start = ((page - 1) as usize).saturating_mul(page_size as usize);
        let end = start.saturating_add(page_size as usize);

        let mut out = Vec::new();
        if start < entries.len() {
            out.extend(entries[start..entries.len().min(end)].iter().cloned());
        }

        Ok((out, total))
    }
}

pub async fn handle_request_faucet_funds(
    wallet: &crate::wallet::Wallet,
    utxos_lock: &tokio::sync::RwLock<HashMap<String, crate::types::UTXO>>,
    mempool_lock: &tokio::sync::RwLock<Mempool>,
    p2p_sender: &tokio::sync::mpsc::Sender<crate::p2p::P2PCommand>,
    _dag: &crate::qantodag::QantoDAG,
    receiver_address_hex: String,
) -> Result<String, String> {
    // 1. Get sender address and keys
    let sender = wallet.address();
    let (sk, pk) = wallet.get_keypair().map_err(|e| e.to_string())?;

    // 2. Select UTXOs to spend
    let target_amount = 100_000_000_000u128; // 100 QNTO
    let mut selected_utxos = Vec::new();
    let mut input_sum = 0u128;

    {
        let utxos = utxos_lock.read().await;
        for utxo in utxos.values() {
            if utxo.address == sender {
                selected_utxos.push(utxo.clone());
                input_sum += utxo.amount;
                if input_sum >= target_amount {
                    break;
                }
            }
        }
    }

    if input_sum < target_amount {
        return Err(format!(
            "Insufficient treasury funds: balance is {}, target is {}",
            input_sum, target_amount
        ));
    }

    // 3. Create inputs
    let inputs: Vec<crate::transaction::Input> = selected_utxos
        .iter()
        .map(|u| crate::transaction::Input {
            tx_id: u.tx_id.clone(),
            output_index: u.output_index,
        })
        .collect();

    // 4. Create outputs
    let pk_bytes = pk.as_bytes();

    // Clean and pad address to 64 chars
    let mut cleaned_addr = receiver_address_hex.trim_start_matches("0x").to_string();
    if cleaned_addr.len() < 64 {
        cleaned_addr = format!("{:0<64}", cleaned_addr);
    } else if cleaned_addr.len() > 64 {
        cleaned_addr = cleaned_addr[..64].to_string();
    }

    let mut outputs = vec![crate::transaction::Output {
        address: cleaned_addr.clone(),
        amount: target_amount,
        homomorphic_encrypted: crate::types::HomomorphicEncrypted::new(target_amount, pk_bytes),
    }];

    if input_sum > target_amount {
        let change = input_sum - target_amount;
        outputs.push(crate::transaction::Output {
            address: sender.clone(),
            amount: change,
            homomorphic_encrypted: crate::types::HomomorphicEncrypted::new(change, pk_bytes),
        });
    }

    // 5. Create transaction config and sign
    let cfg = crate::transaction::TransactionConfig {
        sender,
        receiver: cleaned_addr,
        amount: target_amount,
        fee: 0,
        gas_limit: 50_000,
        gas_price: 1,
        priority_fee: 0,
        inputs,
        outputs,
        metadata: None,
        tx_timestamps: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        chain_id: crate::transaction::GLOBAL_CHAIN_ID.load(std::sync::atomic::Ordering::Relaxed)
            as u32,
    };

    let tx = crate::transaction::Transaction::new(cfg, &sk)
        .await
        .map_err(|e| format!("Failed to create transaction: {e}"))?;

    let tx_id = tx.id.clone();

    // 6. Broadcast via P2P (which adds it to mempool automatically)
    ingest_transaction_locally_and_broadcast(tx, mempool_lock, utxos_lock, _dag, p2p_sender)
        .await?;

    Ok(tx_id)
}

pub async fn handle_bridge_claim_submission(
    dag: &crate::qantodag::QantoDAG,
    utxos_arc: &Arc<RwLock<HashMap<String, crate::types::UTXO>>>,
    request: BridgeMintRequest,
) -> Result<BridgeClaimExecution, String> {
    let (tx, recipient, amount) = request.build_transaction()?;
    dag.process_bridge_claim(&tx, utxos_arc)
        .await
        .map_err(|err| format!("Bridge claim processing failed: {err}"))?;

    Ok(BridgeClaimExecution {
        tx_id: tx.id.clone(),
        recipient,
        amount,
    })
}
