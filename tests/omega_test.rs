// tests/omega_test.rs
use qanto::omega;
use qanto::qanto_compat::sp_core::H256;

#[tokio::test]
async fn test_main() {
    env_logger::try_init().ok();
    println!("--- Running ΛΣ-ΩMEGA Standalone Test ---");

    // Await the now-async simulation function
    omega::simulation::run_simulation().await;

    println!("\n--- Direct Action Reflection Test ---");
    let sample_hash = H256::random();
    println!("Reflecting on action with hash: {sample_hash:?}");

    // Await the async function call to get the bool result
    let result = omega::reflect_on_action(sample_hash).await;

    println!(
        "Security reflex result: {}",
        if result { "Approved" } else { "Rejected" }
    );

    println!("\n--- Running Ultra-Throughput Simulation ---");
    let throughput_results = omega::simulation::run_ultra_throughput_simulation().await;
    println!("Throughput Results: {:?}", throughput_results);
}
