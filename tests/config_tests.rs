use std::process;

fn main() {
    match qanto::config::Config::load("config.toml") {
        Ok(config) => {
            println!("✅ Config loaded successfully!");
            println!("   - data_dir: {}", config.data_dir);
            println!("   - p2p_address: {}", config.p2p_address);
            println!("   - target_block_time: {}ms", config.target_block_time);
            println!("   - difficulty: {}", config.difficulty);
            println!("   - mining_threads: {}", config.mining_threads);
        }
        Err(e) => {
            println!("❌ Config validation failed: {}", e);
            process::exit(1);
        }
    }
}
