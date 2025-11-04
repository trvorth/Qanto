use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::mempool::Mempool;
use crate::p2p::P2PCommand;
use crate::persistence::{balance_key, decode_balance};
use crate::qantodag::QantoDAG;
use crate::transaction::Transaction;
use crate::types::UTXO;
use crate::websocket_server::BalanceEvent;
use qanto_rpc::server::generated as proto;

pub struct NodeRpcBackend {
    pub dag: Arc<QantoDAG>,
    pub utxos: Arc<RwLock<HashMap<String, UTXO>>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub p2p_sender: mpsc::Sender<P2PCommand>,
    pub rpc_balance_sender: broadcast::Sender<proto::BalanceUpdate>,
    pub balance_cache: Arc<RwLock<HashMap<String, u64>>>,
}

impl NodeRpcBackend {
    pub fn new(
        dag: Arc<QantoDAG>,
        utxos: Arc<RwLock<HashMap<String, UTXO>>>,
        mempool: Arc<RwLock<Mempool>>,
        p2p_sender: mpsc::Sender<P2PCommand>,
        ws_balance_sender: broadcast::Sender<BalanceEvent>,
    ) -> Self {
        let (rpc_balance_sender, _) = broadcast::channel(1000);
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
                                base_units: ev.balance,
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

        Self {
            dag,
            utxos,
            mempool,
            p2p_sender,
            rpc_balance_sender,
            balance_cache,
        }
    }
}

fn convert_proto_tx(ptx: proto::Transaction) -> Result<Transaction, String> {
    let inputs = ptx
        .inputs
        .into_iter()
        .map(|i| crate::transaction::Input {
            tx_id: i.tx_id,
            output_index: i.output_index,
        })
        .collect::<Vec<_>>();

    let outputs = ptx
        .outputs
        .into_iter()
        .map(|o| crate::transaction::Output {
            address: o.address,
            amount: o.amount,
            homomorphic_encrypted: match o.homomorphic_encrypted {
                Some(he) => crate::types::HomomorphicEncrypted {
                    ciphertext: he.ciphertext,
                    public_key: he.public_key,
                },
                None => crate::types::HomomorphicEncrypted {
                    ciphertext: vec![],
                    public_key: vec![],
                },
            },
        })
        .collect::<Vec<_>>();

    let signature = match ptx.signature {
        Some(sig) => crate::types::QuantumResistantSignature {
            signer_public_key: sig.signer_public_key,
            signature: sig.signature,
        },
        None => return Err("Missing transaction signature".to_string()),
    };

    let fee_breakdown = ptx
        .fee_breakdown
        .map(|fb| crate::gas_fee_model::FeeBreakdown {
            base_fee: fb.base_fee,
            complexity_fee: fb.complexity_fee,
            storage_fee: fb.storage_fee,
            gas_fee: fb.gas_fee,
            priority_fee: fb.priority_fee,
            congestion_multiplier: fb.congestion_multiplier,
            total_fee: fb.total_fee,
            gas_used: fb.gas_used,
            gas_price: fb.gas_price,
        });

    Ok(Transaction {
        id: ptx.id,
        sender: ptx.sender,
        receiver: ptx.receiver,
        amount: ptx.amount,
        fee: ptx.fee,
        gas_limit: ptx.gas_limit,
        gas_used: ptx.gas_used,
        gas_price: ptx.gas_price,
        priority_fee: ptx.priority_fee,
        inputs,
        outputs,
        timestamp: ptx.timestamp,
        metadata: ptx.metadata,
        signature,
        fee_breakdown,
    })
}

fn convert_internal_tx_to_proto(tx: &crate::transaction::Transaction) -> proto::Transaction {
    let inputs = tx
        .inputs
        .iter()
        .map(|i| proto::Input {
            tx_id: i.tx_id.clone(),
            output_index: i.output_index,
        })
        .collect::<Vec<_>>();

    let outputs = tx
        .outputs
        .iter()
        .map(|o| proto::Output {
            address: o.address.clone(),
            amount: o.amount,
            homomorphic_encrypted: Some(proto::HomomorphicEncrypted {
                ciphertext: o.homomorphic_encrypted.ciphertext.clone(),
                public_key: o.homomorphic_encrypted.public_key.clone(),
            }),
        })
        .collect::<Vec<_>>();

    let signature = Some(proto::QuantumResistantSignature {
        signer_public_key: tx.signature.signer_public_key.clone(),
        signature: tx.signature.signature.clone(),
    });

    let fee_breakdown = tx.fee_breakdown.as_ref().map(|fb| proto::FeeBreakdown {
        base_fee: fb.base_fee,
        complexity_fee: fb.complexity_fee,
        storage_fee: fb.storage_fee,
        gas_fee: fb.gas_fee,
        priority_fee: fb.priority_fee,
        congestion_multiplier: fb.congestion_multiplier,
        total_fee: fb.total_fee,
        gas_used: fb.gas_used,
        gas_price: fb.gas_price,
    });

    proto::Transaction {
        id: tx.id.clone(),
        sender: tx.sender.clone(),
        receiver: tx.receiver.clone(),
        amount: tx.amount,
        fee: tx.fee,
        gas_limit: tx.gas_limit,
        gas_used: tx.gas_used,
        gas_price: tx.gas_price,
        priority_fee: tx.priority_fee,
        inputs,
        outputs,
        timestamp: tx.timestamp,
        metadata: tx.metadata.clone(),
        signature,
        fee_breakdown,
    }
}

fn convert_block_to_proto(b: &crate::qantodag::QantoBlock) -> proto::QantoBlock {
    let transactions = b
        .transactions
        .iter()
        .map(convert_internal_tx_to_proto)
        .collect::<Vec<_>>();

    let cross_chain_references = b
        .cross_chain_references
        .iter()
        .map(|(cid, bid)| proto::CrossChainReference {
            chain_id: *cid,
            block_id: bid.clone(),
        })
        .collect::<Vec<_>>();

    let cross_chain_swaps = b
        .cross_chain_swaps
        .iter()
        .map(|s| proto::CrossChainSwap {
            swap_id: s.swap_id.clone(),
            source_chain: s.source_chain,
            target_chain: s.target_chain,
            amount: s.amount,
            initiator: s.initiator.clone(),
            responder: s.responder.clone(),
            timelock: s.timelock,
            state: match s.state {
                crate::qantodag::SwapState::Initiated => proto::SwapState::Initiated as i32,
                crate::qantodag::SwapState::Redeemed => proto::SwapState::Redeemed as i32,
                crate::qantodag::SwapState::Refunded => proto::SwapState::Refunded as i32,
            },
            secret_hash: s.secret_hash.clone(),
            secret: s.secret.clone(),
        })
        .collect::<Vec<_>>();

    let signature = Some(proto::QuantumResistantSignature {
        signer_public_key: b.signature.signer_public_key.clone(),
        signature: b.signature.signature.clone(),
    });

    let homomorphic_encrypted = b
        .homomorphic_encrypted
        .iter()
        .map(|he| proto::HomomorphicEncrypted {
            ciphertext: he.ciphertext.clone(),
            public_key: he.public_key.clone(),
        })
        .collect::<Vec<_>>();

    let smart_contracts = b
        .smart_contracts
        .iter()
        .map(|sc| proto::SmartContract {
            contract_id: sc.contract_id.clone(),
            code: sc.code.clone(),
            storage: sc.storage.clone(),
            owner: sc.owner.clone(),
            gas_balance: sc.gas_balance,
        })
        .collect::<Vec<_>>();

    let carbon_credentials = b
        .carbon_credentials
        .iter()
        .map(|cc| proto::CarbonOffsetCredential {
            id: cc.id.clone(),
            issuer_id: cc.issuer_id.clone(),
            beneficiary_node: cc.beneficiary_node.clone(),
            tonnes_co2_sequestered: cc.tonnes_co2_sequestered,
            project_id: cc.project_id.clone(),
            vintage_year: cc.vintage_year,
            verification_signature: cc.verification_signature.clone(),
            additionality_proof_hash: cc.additionality_proof_hash.clone(),
            issuer_reputation_score: cc.issuer_reputation_score,
            geospatial_consistency_score: cc.geospatial_consistency_score,
        })
        .collect::<Vec<_>>();

    proto::QantoBlock {
        chain_id: b.chain_id,
        id: b.id.clone(),
        parents: b.parents.clone(),
        transactions,
        difficulty: b.difficulty,
        validator: b.validator.clone(),
        miner: b.miner.clone(),
        nonce: b.nonce,
        timestamp: b.timestamp,
        height: b.height,
        reward: b.reward,
        effort: b.effort,
        cross_chain_references,
        cross_chain_swaps,
        merkle_root: b.merkle_root.clone(),
        signature,
        homomorphic_encrypted,
        smart_contracts,
        carbon_credentials,
        epoch: b.epoch,
    }
}

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

        self.p2p_sender
            .send(P2PCommand::BroadcastTransaction(parsed.clone()))
            .await
            .map_err(|e| format!("Failed to broadcast transaction: {e}"))?;

        Ok(())
    }

    async fn get_wallet_balance(&self, address: String) -> Result<(u64, u64), String> {
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
                let mut total: u64 = 0;
                for (_k, utxo) in utxos.iter() {
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

        let mut incoming: u64 = 0;
        let mut outgoing: u64 = 0;

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

    async fn get_balance(&self, address: String) -> Result<u64, String> {
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
        let network_throughput_mbps = metrics.get_network_throughput() * 8.0;

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
            tps,
            bps,
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
}
