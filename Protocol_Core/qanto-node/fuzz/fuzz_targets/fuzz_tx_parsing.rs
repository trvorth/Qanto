#![no_main]
use libfuzzer_sys::fuzz_target;
use qanto::transaction::Transaction;

fuzz_target!(|data: &[u8]| {
    // Fuzz recovery of EVM sender and related transaction parsing
    let _ = Transaction::recover_evm_sender(data);
});
