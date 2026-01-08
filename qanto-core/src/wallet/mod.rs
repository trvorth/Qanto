use crate::balance_stream;
use tokio::sync::broadcast;

pub type WalletBalanceUpdate = balance_stream::BalanceUpdate;
pub type WalletBalanceStream = broadcast::Receiver<balance_stream::BalanceUpdate>;
pub type BalanceStream = WalletBalanceStream;

pub fn subscribe(broadcaster: &balance_stream::BalanceBroadcaster) -> WalletBalanceStream {
    broadcaster.subscribe()
}
