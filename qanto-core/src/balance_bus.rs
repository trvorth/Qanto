use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Clone, Serialize, Deserialize)]
pub struct BalanceUpdateWs { pub address: String, pub spendable: u64, pub total: u64, pub finalized: bool }

pub type BalanceBus = broadcast::Sender<BalanceUpdateWs>;
pub type BalanceBusRx = broadcast::Receiver<BalanceUpdateWs>;

pub fn emit_update(bus: &BalanceBus, update: BalanceUpdateWs) {
    let _ = bus.send(update);
}
