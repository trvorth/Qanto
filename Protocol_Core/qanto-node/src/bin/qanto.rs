use qanto::qanto;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use ::qanto::pipeline::TokenProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize the token processor as an in-memory utility
    let script_path =
        std::env::var("RTK_SCRIPT_PATH").unwrap_or_else(|_| "scripts/filter.lua".to_string());
    if let Ok(_processor) = TokenProcessor::new(&script_path) {
        println!("RTK TokenProcessor initialized successfully");
        // Example integration: process a raw context stream
        // let _clean_output = _processor.process_stream("raw_context_stream_to_be_killed");
    } else {
        println!(
            "Warning: Failed to initialize RTK TokenProcessor. Check if {} exists.",
            script_path
        );
    }

    qanto::run().await
}
