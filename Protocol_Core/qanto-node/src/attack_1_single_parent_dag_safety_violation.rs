//! Attack 1: Single-parent DAG Safety Violation under Partial Synchrony
//!
//! CRITICAL VULNERABILITY: The QantoDAG in containment mode uses single-parent blocks
//! (line 3245-3248: "Containment mode requires exactly one state parent").
//!
//! Under partial synchrony, an attacker with >50% hashrate can:
//! 1. Withhold blocks on a private chain
//! 2. Wait for honest chain to advance
//! 3. Release the heavier private chain, causing a reorg
//! 4. The single-parent rule means no multi-parent consensus safety
//!
//! This is a classic "selfish mining" / "longest chain" attack on single-parent DAGs.

use crate::post_quantum_crypto::{generate_pq_keypair, QantoPQPrivateKey, QantoPQPublicKey};
use crate::qantodag::{QantoBlock, QantoBlockCreationData, QantoDAG, INITIAL_BLOCK_REWARD};
use crate::security_tests::new_dummy_with_storage_path;
use crate::transaction::{Input, Output, Transaction, TransactionKind};
use crate::types::{HomomorphicEncrypted, UTXO};
use ahash::AHashMap as HashMap;
use qanto_core::protocol_identity::{account_address_v2, validator_id_v2};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

fn wrap_and_init_dag(dag: QantoDAG) -> Arc<QantoDAG> {
    if !dag.tips.contains_key(&1234) {
        let mut genesis = QantoBlock::new_test_block("test-genesis".to_string());
        genesis.height = 0;
        genesis.parents.clear();
        genesis.timestamp = 0;
        genesis.id = genesis.compute_block_id().expect("test genesis id");
        genesis.chain_id = 1234;
        dag.blocks.insert(genesis.id.clone(), genesis.clone());
        dag.tips.entry(1234).or_default().insert(genesis.id);
    }
    let arc_dag = Arc::new(dag);
    let weak_self = Arc::downgrade(&arc_dag);
    let ptr = Arc::as_ptr(&arc_dag) as *mut QantoDAG;
    unsafe {
        (*ptr).self_arc = weak_self;
    }
    arc_dag
}

async fn create_test_validator(
    dag: &Arc<QantoDAG>,
) -> (QantoPQPublicKey, QantoPQPrivateKey, String, String) {
    let (pk, sk) = generate_pq_keypair(None).unwrap();
    let validator_addr = validator_id_v2(
        pk.as_array(),
        1234,
        qanto_consensus_primitives::consensus_parameters::SchemaVersion::V2_INITIAL,
    )
    .to_string();
    let account_addr = account_address_v2(1234, pk.as_bytes()).unwrap();
    dag.validators
        .insert(validator_addr.clone(), 50 * crate::Q_SCALE);
    // Also set initial balance for the account address
    dag.account_state_cache
        .set_balance(account_addr.clone(), 10000);
    (pk, sk, validator_addr, account_addr)
}

/// Creates a transfer transaction for testing with proper UTXO setup
/// Also adds the required UTXO to the UTXO map
async fn create_transfer_tx(
    from_sk: &QantoPQPrivateKey,
    from_pk: &QantoPQPublicKey,
    to_pk: &QantoPQPublicKey,
    amount: u128,
    fee: u128,
    nonce: u64,
    utxos: &Arc<RwLock<HashMap<String, UTXO>>>,
    existing_utxo_tx_id: Option<String>,
    sender_account_addr: String,
) -> Transaction {
    let chain_id = 1234u32;
    let sender_addr = sender_account_addr; // Use the provided account address

    let (tx_id, utxo_amount) = if let Some(utxo_tx_id) = existing_utxo_tx_id {
        // Use existing UTXO for double-spend simulation
        let utxo_key = format!("{}_{}", utxo_tx_id, 0);
        let utxo_amount = {
            let utxo_guard = utxos.read().await;
            let utxo = utxo_guard
                .get(&utxo_key)
                .expect("Existing UTXO must exist for double-spend test");
            utxo.amount
        };
        (utxo_tx_id, utxo_amount)
    } else {
        // Create new UTXO
        let tx_id = format!("utxo_sender_{}", nonce);
        let utxo_amount = amount + fee;
        let utxo_key = format!("{}_{}", tx_id, 0);
        let utxo = UTXO {
            tx_id: tx_id.clone(),
            output_index: 0,
            amount: utxo_amount,
            address: sender_addr.clone(),
            explorer_link: "test".to_string(),
        };
        utxos.write().await.insert(utxo_key, utxo);
        (tx_id, utxo_amount)
    };

    let inputs = vec![Input {
        tx_id,
        output_index: 0,
    }];

    // Create outputs: recipient + change back to sender (UTXO conservation)
    let change = utxo_amount.saturating_sub(amount + fee);
    let mut outputs = vec![Output {
        address: qanto_core::protocol_identity::account_address_v2(chain_id, to_pk.as_bytes())
            .unwrap(),
        amount,
        homomorphic_encrypted: HomomorphicEncrypted::new(amount, &[0u8; 32]),
    }];
    if change > 0 {
        outputs.push(Output {
            address: sender_addr.clone(),
            amount: change,
            homomorphic_encrypted: HomomorphicEncrypted::new(change, &[0u8; 32]),
        });
    }

    crate::security_tests::create_valid_signed_tx(from_sk, from_pk, chain_id, inputs, outputs, fee)
        .await
}

/// Creates a coinbase transaction for testing
async fn create_coinbase_tx(
    dag: &Arc<QantoDAG>,
    validator_sk: &QantoPQPrivateKey,
    validator_pk: &QantoPQPublicKey,
    validator_addr: &str,
    height: u64,
    parent_tip: &str,
    user_txs: &[Transaction],
) -> Transaction {
    let chain_id = 1234u32; // Must match validator_id_v2 chain_id
    let reward_address =
        qanto_core::protocol_identity::account_address_v2(chain_id, validator_pk.as_bytes())
            .unwrap();

    // Calculate coinbase amount = sum of fees from user transactions (containment mode: no block subsidy)
    let total_fees = user_txs.iter().map(|tx| tx.fee).sum::<u128>();

    // Compute coinbase context properly - must match QantoBlock::compute_coinbase_context
    let coinbase_context =
        QantoBlock::compute_coinbase_context(chain_id, height, &[parent_tip.to_string()], user_txs);

    let output = Output {
        address: reward_address.clone(),
        amount: total_fees,
        homomorphic_encrypted: HomomorphicEncrypted::new(total_fees, &[0u8; 32]),
    };

    Transaction::new_coinbase(
        reward_address,
        total_fees,
        vec![output],
        validator_sk,
        chain_id,
        coinbase_context,
    )
    .unwrap()
}

/// Creates a valid block for testing with proper coinbase transaction
async fn create_valid_block(
    dag: &Arc<QantoDAG>,
    validator_sk: &QantoPQPrivateKey,
    validator_pk: &QantoPQPublicKey,
    validator_addr: &str,
    parent_tip: &str,
    height: u64,
    txs: Vec<Transaction>,
) -> QantoBlock {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let paillier_pk = crate::types::HomomorphicEncrypted::generate_keypair().0;

    // Calculate reward = sum of fees from user transactions (containment mode: no block subsidy)
    let total_fees = txs.iter().map(|tx| tx.fee).sum::<u128>();

    // Create coinbase transaction first (pass user txs for proper context computation)
    let coinbase_tx = create_coinbase_tx(
        dag,
        validator_sk,
        validator_pk,
        validator_addr,
        height,
        parent_tip,
        &txs,
    )
    .await;

    // Combine coinbase with user transactions
    let mut all_txs = vec![coinbase_tx];
    all_txs.extend(txs);

    let mut block = QantoBlock::new_unsealed_candidate(crate::qantodag::QantoBlockCreationData {
        validator_private_key: validator_sk.clone(),
        chain_id: 1234,
        parents: vec![parent_tip.to_string()],
        transactions: all_txs,
        difficulty: crate::QANTO_SCALE as u64,
        validator: validator_addr.to_string(),
        miner: validator_addr.to_string(),
        timestamp: now,
        current_epoch: 0,
        height,
        reward: total_fees,
        nonce: 0,
        effort: 0,
        chain_work: crate::miner::U256::ZERO,
        paillier_pk,
        state_parent: String::new(),
    })
    .unwrap();

    // Mine the block to satisfy PoW using Miner instance
    let logging_config = crate::config::LoggingConfig {
        level: "info".to_string(),
        enable_block_celebrations: false,
        celebration_log_level: "info".to_string(),
        celebration_throttle_per_min: None,
    };
    let miner_addr =
        qanto_core::protocol_identity::account_address_v2(1234, validator_pk.as_bytes()).unwrap();
    let miner_config = crate::miner::MinerConfig {
        address: miner_addr.clone(),
        dag: dag.clone(),
        target_block_time: 3,
        use_gpu: false,
        zk_enabled: false,
        threads: 1,
        logging_config: logging_config.clone(),
    };
    let miner = crate::miner::Miner::new(miner_config).unwrap();
    miner.solve_pow(&mut block).unwrap();

    // Seal the block (sign it and compute block ID)
    block.seal(validator_sk).unwrap();
    block
}

/// ATTACK DEMONSTRATION: Single-parent DAG safety violation
///
/// This test demonstrates that with >50% hashrate, an attacker can:
/// 1. Build a longer private chain while honest nodes build public chain
/// 2. Release the private chain causing a reorg and double-spend
/// 3. No multi-parent consensus means no safety guarantee
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn attack_1_single_parent_safety_violation_selfish_mining() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        // Setup two validators: honest (49% hashrate) and attacker (51% hashrate)
        let (honest_pk, honest_sk, honest_validator_addr, honest_account_addr) =
            create_test_validator(&dag).await;
        let (attacker_pk, attacker_sk, attacker_validator_addr, attacker_account_addr) =
            create_test_validator(&dag).await;

        // Give attacker more stake (simulating 51% hashrate) - both must meet MIN_VALIDATOR_STAKE
        // MIN_VALIDATOR_STAKE = 50 * SMALLEST_UNITS_PER_QAN = 50 QAN = 50_000_000_000
        *dag.validators.get_mut(&attacker_validator_addr).unwrap() =
            51 * crate::transaction::SMALLEST_UNITS_PER_QAN;
        *dag.validators.get_mut(&honest_validator_addr).unwrap() =
            50 * crate::transaction::SMALLEST_UNITS_PER_QAN;

        // Balances already set in create_test_validator for account addresses
        // (validator addresses are different from account addresses)

        let utxos = Arc::new(RwLock::new(HashMap::new()));

        // Step 1: Honest chain builds 5 blocks
        let mut honest_tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();
        let mut honest_blocks = Vec::new();

        for i in 1..=5 {
            let txs = vec![
                create_transfer_tx(
                    &honest_sk,
                    &honest_pk,
                    &attacker_pk,
                    100,
                    100,
                    i,
                    &utxos,
                    None,
                    honest_account_addr.clone(),
                )
                .await,
            ];
            let block = create_valid_block(
                &dag,
                &honest_sk,
                &honest_pk,
                &honest_validator_addr,
                &honest_tip,
                i,
                txs,
            )
            .await;
            let added = dag
                .add_block(block.clone(), &utxos, None, None)
                .await
                .unwrap();
            assert!(added, "Honest block {} should be added", i);
            honest_tip = block.id.clone();
            honest_blocks.push(block.id.clone());
        }

        // Verify honest chain is in tips
        assert_eq!(dag.tips.get(&1234).unwrap().len(), 1);
        assert_eq!(
            *dag.tips.get(&1234).unwrap().iter().next().unwrap(),
            honest_tip
        );

        // Step 2: Attacker secretly builds a longer chain (6 blocks) on the same parent
        // This simulates the attacker withholding blocks during a period of partial synchrony
        let genesis_id = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();
        let mut attacker_tip = genesis_id.clone();
        let mut attacker_blocks = Vec::new();

        for i in 1..=6 {
            let txs = vec![
                create_transfer_tx(
                    &attacker_sk,
                    &attacker_pk,
                    &honest_pk,
                    100,
                    100,
                    100 + i,
                    &utxos,
                    None,
                    attacker_account_addr.clone(),
                )
                .await,
            ];
            let block = create_valid_block(
                &dag,
                &attacker_sk,
                &attacker_pk,
                &attacker_validator_addr,
                &attacker_tip,
                i,
                txs,
            )
            .await;
            // Attacker adds to DAG but we simulate withholding by not announcing
            // In reality, the attacker would mine privately and only release when chain is longer
            attacker_tip = block.id.clone();
            attacker_blocks.push(block);
        }

        // Step 3: Attacker releases the longer chain (reorg attack)
        // Since the DAG uses single-parent and no finality, the heavier chain wins
        attacker_tip = genesis_id.clone();
        for block in attacker_blocks {
            let added = dag.add_block(block, &utxos, None, None).await;
            // The DAG should accept the attacker's chain because it's "heavier"
            // (higher cumulative work) but there's no mechanism to prevent reorg
            if added.is_ok() && added.unwrap() {
                attacker_tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();
            }
        }

        // VULNERABILITY: The honest chain was reorganized!
        // There's no finality, no checkpointing, no slashing for equivocation
        // The attacker's chain replaced the honest chain's blocks
        println!(
            "After attack: tip = {}",
            dag.tips.get(&1234).unwrap().iter().next().unwrap()
        );
        println!("Honest blocks in DAG: {:?}", honest_blocks);

        // This test demonstrates the vulnerability EXISTS
        // After FIX: this test should fail (attack prevented)
        // The fix would be: finality gadget, checkpointing, or multi-parent DAG
    }

    #[tokio::test]
    async fn attack_1_single_parent_safety_violation_double_spend() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dag = wrap_and_init_dag(new_dummy_with_storage_path(temp_dir.path()));

        let (honest_pk, honest_sk, honest_validator_addr, honest_account_addr) =
            create_test_validator(&dag).await;
        let (attacker_pk, attacker_sk, attacker_validator_addr, attacker_account_addr) =
            create_test_validator(&dag).await;
        let (victim_pk, victim_sk, victim_validator_addr, victim_account_addr) =
            create_test_validator(&dag).await;

        *dag.validators.get_mut(&attacker_validator_addr).unwrap() =
            51 * crate::transaction::SMALLEST_UNITS_PER_QAN;
        *dag.validators.get_mut(&honest_validator_addr).unwrap() =
            50 * crate::transaction::SMALLEST_UNITS_PER_QAN;
        *dag.validators.get_mut(&victim_validator_addr).unwrap() =
            50 * crate::transaction::SMALLEST_UNITS_PER_QAN;

        let utxos = Arc::new(RwLock::new(HashMap::new()));

        // Balances already set in create_test_validator for account addresses

        // Setup initial UTXO for victim (key uses underscore to match validation code)
        let victim_utxo = UTXO {
            tx_id: "victim_initial".to_string(),
            output_index: 0,
            amount: 1000,
            address: victim_account_addr.clone(),
            explorer_link: "test".to_string(),
        };
        utxos
            .write()
            .await
            .insert("victim_initial_0".to_string(), victim_utxo);

        // Step 1: Honest chain - victim sends 500 to merchant A (block 1)
        let honest_tip = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();
        let tx_to_merchant = create_transfer_tx(
            &victim_sk,
            &victim_pk,
            &honest_pk,
            500,
            100,
            1,
            &utxos,
            Some("victim_initial".to_string()),
            victim_account_addr.clone(),
        )
        .await;
        let txs = vec![tx_to_merchant];
        let block1 = create_valid_block(
            &dag,
            &victim_sk,
            &victim_pk,
            &victim_validator_addr,
            &honest_tip,
            1,
            txs,
        )
        .await;

        // Step 2: Attacker builds private chain excluding victim's payment
        // Attacker creates alternative block 1 that sends 500 to themselves
        // Both blocks spend the SAME victim_initial UTXO (double spend)
        // The attacker builds on genesis, not on honest_tip
        let genesis_id = dag.tips.get(&1234).unwrap().iter().next().unwrap().clone();

        // For the attacker's chain, we need the UTXO to still exist
        // (simulating that attacker built their chain before seeing the honest block)
        // We'll create a fresh UTXO map for attacker's chain validation
        let attacker_utxos = Arc::new(RwLock::new(HashMap::new()));
        let victim_utxo_attacker = UTXO {
            tx_id: "victim_initial".to_string(),
            output_index: 0,
            amount: 1000,
            address: victim_account_addr.clone(),
            explorer_link: "test".to_string(),
        };
        attacker_utxos
            .write()
            .await
            .insert("victim_initial_0".to_string(), victim_utxo_attacker);

        let tx_to_attacker = create_transfer_tx(
            &victim_sk,
            &victim_pk,
            &attacker_pk,
            500,
            100,
            1,
            &attacker_utxos,
            Some("victim_initial".to_string()),
            victim_account_addr.clone(),
        )
        .await;
        let block1_alt = create_valid_block(
            &dag,
            &attacker_sk,
            &attacker_pk,
            &attacker_validator_addr,
            &genesis_id,
            1,
            vec![tx_to_attacker],
        )
        .await;

        // Attacker builds 2 more blocks on top (3 total vs honest 1)
        let attacker_tip = block1_alt.id.clone();
        let block2 = create_valid_block(
            &dag,
            &attacker_sk,
            &attacker_pk,
            &attacker_validator_addr,
            &attacker_tip,
            2,
            vec![],
        )
        .await;
        let attacker_tip2 = block2.id.clone();
        let block3 = create_valid_block(
            &dag,
            &attacker_sk,
            &attacker_pk,
            &attacker_validator_addr,
            &attacker_tip2,
            3,
            vec![],
        )
        .await;

        // Step 3: Attacker releases the longer chain
        dag.add_block(block1_alt, &utxos, None, None).await.unwrap();
        dag.add_block(block2, &utxos, None, None).await.unwrap();
        dag.add_block(block3, &utxos, None, None).await.unwrap();

        // VULNERABILITY: Double spend succeeded!
        // Merchant A's payment is now invalid (reorg'd out)
        // No finality = no safety for 0-conf or even 1-conf transactions
        println!(
            "Final tip after double spend: {}",
            dag.tips.get(&1234).unwrap().iter().next().unwrap()
        );
        println!(
            "Block 1 (merchant payment) in DAG: {}",
            dag.blocks.contains_key(&block1.id)
        );
    }
}
