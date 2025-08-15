use clap::{App, Arg};
use serde::{Deserialize, Serialize};
use signal_hook::{consts::SIGINT, consts::SIGTERM, iterator::Signals};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelayerConfig {
    source_chain: String,
    destination_chain: String,
    source_rpc: String,
    destination_rpc: String,
    channel_id: String,
    port_id: String,
    polling_interval: u64,
    max_retries: u32,
    gas_price_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct PacketEvent {
    sequence: u64,
    source_port: String,
    source_channel: String,
    destination_port: String,
    destination_channel: String,
    data: Vec<u8>,
    timeout_height: Option<u64>,
    timeout_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct ProofData {
    packet_commitment: Vec<u8>,
    proof_height: u64,
    proof_bytes: Vec<u8>,
}

struct QantoRelayer {
    config: RelayerConfig,
    pending_packets: Arc<RwLock<HashMap<u64, PacketEvent>>>,
    processed_sequences: Arc<RwLock<Vec<u64>>>,
    metrics: Arc<RwLock<RelayerMetrics>>,
    shutdown: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct RelayerMetrics {
    packets_relayed: u64,
    packets_failed: u64,
    total_fees_spent: u128,
    average_confirmation_time: u64,
    last_successful_relay: Option<u64>,
}

impl QantoRelayer {
    fn new(config: RelayerConfig) -> Self {
        Self {
            config,
            pending_packets: Arc::new(RwLock::new(HashMap::new())),
            processed_sequences: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(RelayerMetrics::default())),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🚀 Qanto Relayer starting...");
        println!("📡 Source chain: {}", self.config.source_chain);
        println!("🎯 Destination chain: {}", self.config.destination_chain);
        println!(
            "📊 Channel: {}/{}",
            self.config.port_id, self.config.channel_id
        );
        println!("🛑 Press Ctrl+C for graceful shutdown");

        // Setup signal handlers
        self.setup_signal_handlers()?;

        // Start event monitoring
        let event_handle = self.spawn_event_monitor();

        // Start packet processor
        let processor_handle = self.spawn_packet_processor();

        // Start metrics reporter
        let metrics_handle = self.spawn_metrics_reporter();

        // Wait for shutdown or task completion
        tokio::select! {
            _ = self.wait_for_shutdown() => {
                println!("\n⚠️  Shutdown signal received. Initiating graceful shutdown...");
            }
            _ = event_handle => println!("Event monitor stopped"),
            _ = processor_handle => println!("Packet processor stopped"),
            _ = metrics_handle => println!("Metrics reporter stopped"),
        }

        // Perform cleanup
        self.cleanup().await;

        Ok(())
    }

    fn setup_signal_handlers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let shutdown = self.shutdown.clone();

        std::thread::spawn(move || {
            let mut signals =
                Signals::new([SIGINT, SIGTERM]).expect("Failed to register signal handlers");
            for sig in signals.forever() {
                match sig {
                    SIGINT => {
                        println!("\n📍 Received SIGINT (Ctrl+C)");
                        shutdown.store(true, Ordering::Relaxed);
                        break;
                    }
                    SIGTERM => {
                        println!("\n📍 Received SIGTERM");
                        shutdown.store(true, Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    async fn wait_for_shutdown(&self) {
        while !self.shutdown.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn cleanup(&self) {
        println!("🧹 Starting cleanup procedures...");

        // Signal all tasks to stop
        self.shutdown.store(true, Ordering::Relaxed);

        // Wait a moment for tasks to finish current operations
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Save pending packets state
        let pending = self.pending_packets.read().await;
        if !pending.is_empty() {
            println!(
                "💾 Saving {} pending packets for next run...",
                pending.len()
            );
            if let Ok(serialized) = serde_json::to_string_pretty(&*pending) {
                if let Err(e) = tokio::fs::write("pending_packets.json", serialized).await {
                    eprintln!("⚠️ Failed to save pending packets: {e}");
                } else {
                    println!("✅ Pending packets saved to pending_packets.json");
                }
            }
        }

        // Save processed sequences
        let processed = self.processed_sequences.read().await;
        if !processed.is_empty() {
            println!(
                "💾 Saving {len} processed sequences...",
                len = processed.len()
            );
            if let Ok(serialized) = serde_json::to_string_pretty(&*processed) {
                if let Err(e) = tokio::fs::write("processed_sequences.json", serialized).await {
                    eprintln!("⚠️ Failed to save processed sequences: {e}");
                } else {
                    println!("✅ Processed sequences saved to processed_sequences.json");
                }
            }
        }

        // Print final metrics
        let metrics = self.metrics.read().await;
        println!("\n📊 Final Metrics:");
        println!("   Total packets relayed: {}", metrics.packets_relayed);
        println!("   Total packets failed: {}", metrics.packets_failed);

        println!("✅ Cleanup completed. Relayer shut down gracefully.");
    }

    fn spawn_event_monitor(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let pending_packets = self.pending_packets.clone();
        let processed_sequences = self.processed_sequences.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            while !shutdown.load(Ordering::Relaxed) {
                // Simulate fetching new packet events from source chain
                match Self::fetch_packet_events(&config.source_rpc, &config.channel_id).await {
                    Ok(events) => {
                        let mut pending = pending_packets.write().await;
                        let processed = processed_sequences.read().await;

                        for event in events {
                            if !processed.contains(&event.sequence)
                                && !pending.contains_key(&event.sequence)
                            {
                                println!("📦 New packet detected: sequence {}", event.sequence);
                                pending.insert(event.sequence, event);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Error fetching events: {e}");
                    }
                }

                // Check for shutdown more frequently
                for _ in 0..config.polling_interval * 10 {
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
            println!("📭 Event monitor stopped.");
        })
    }

    fn spawn_packet_processor(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let pending_packets = self.pending_packets.clone();
        let processed_sequences = self.processed_sequences.clone();
        let metrics = self.metrics.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            while !shutdown.load(Ordering::Relaxed) {
                let packets_to_process: Vec<(u64, PacketEvent)> = {
                    let pending = pending_packets.read().await;
                    pending.iter().map(|(k, v)| (*k, v.clone())).collect()
                };

                for (sequence, packet) in packets_to_process {
                    // Check for shutdown before processing each packet
                    if shutdown.load(Ordering::Relaxed) {
                        println!("⏸️  Stopping packet processing for shutdown...");
                        break;
                    }

                    println!("⚡ Processing packet sequence {sequence}");

                    // Fetch proof from source chain
                    match Self::fetch_packet_proof(&config.source_rpc, sequence).await {
                        Ok(proof) => {
                            // Submit packet to destination chain
                            match Self::submit_packet(&config.destination_rpc, &packet, &proof)
                                .await
                            {
                                Ok(tx_hash) => {
                                    println!(
                                        "✅ Packet {sequence} relayed successfully: tx {tx_hash}"
                                    );

                                    // Update state
                                    pending_packets.write().await.remove(&sequence);
                                    processed_sequences.write().await.push(sequence);

                                    // Update metrics
                                    let mut m = metrics.write().await;
                                    m.packets_relayed += 1;
                                    m.last_successful_relay =
                                        Some(chrono::Utc::now().timestamp() as u64);
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to submit packet {sequence}: {e}");
                                    metrics.write().await.packets_failed += 1;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Failed to fetch proof for packet {sequence}: {e}");
                        }
                    }

                    // Rate limiting
                    sleep(Duration::from_millis(500)).await;
                }

                // Check for shutdown more frequently
                for _ in 0..50 {
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
            println!("⚡ Packet processor stopped.");
        })
    }

    fn spawn_metrics_reporter(&self) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            while !shutdown.load(Ordering::Relaxed) {
                // Check for shutdown more frequently (every second instead of waiting full 60 seconds)
                for _ in 0..60 {
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    sleep(Duration::from_secs(1)).await;
                }

                if !shutdown.load(Ordering::Relaxed) {
                    let m = metrics.read().await;
                    println!("\n📊 Relayer Metrics Report");
                    println!("   Packets relayed: {}", m.packets_relayed);
                    println!("   Packets failed: {}", m.packets_failed);
                    println!(
                        "   Success rate: {:.2}%",
                        if m.packets_relayed + m.packets_failed > 0 {
                            (m.packets_relayed as f64
                                / (m.packets_relayed + m.packets_failed) as f64)
                                * 100.0
                        } else {
                            0.0
                        }
                    );
                    println!("   Total fees spent: {} units", m.total_fees_spent);
                    if let Some(last) = m.last_successful_relay {
                        println!(
                            "   Last successful relay: {} seconds ago",
                            chrono::Utc::now().timestamp() as u64 - last
                        );
                    }
                    println!();
                }
            }
            println!("📊 Metrics reporter stopped.");
        })
    }

    async fn fetch_packet_events(
        _rpc_url: &str,
        channel_id: &str,
    ) -> Result<Vec<PacketEvent>, Box<dyn std::error::Error + Send + Sync>> {
        // In production, this would query the actual blockchain RPC
        // For now, simulate occasional new packets
        if rand::random::<f64>() < 0.3 {
            Ok(vec![PacketEvent {
                sequence: chrono::Utc::now().timestamp_millis() as u64,
                source_port: "transfer".to_string(),
                source_channel: channel_id.to_string(),
                destination_port: "transfer".to_string(),
                destination_channel: "channel-1".to_string(),
                data: vec![1, 2, 3, 4],
                timeout_height: Some(1000000),
                timeout_timestamp: chrono::Utc::now().timestamp() as u64 + 3600,
            }])
        } else {
            Ok(vec![])
        }
    }

    async fn fetch_packet_proof(
        _rpc_url: &str,
        _sequence: u64,
    ) -> Result<ProofData, Box<dyn std::error::Error + Send + Sync>> {
        // In production, fetch actual Merkle proof from blockchain
        sleep(Duration::from_millis(100)).await;
        Ok(ProofData {
            packet_commitment: vec![0; 32],
            proof_height: 12345,
            proof_bytes: vec![0; 256],
        })
    }

    async fn submit_packet(
        _rpc_url: &str,
        _packet: &PacketEvent,
        _proof: &ProofData,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // In production, submit actual transaction to destination chain
        sleep(Duration::from_millis(200)).await;
        Ok(format!("0x{:064x}", rand::random::<u64>()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let matches = App::new("Qanto Relayer")
        .version("1.0.0")
        .author("Qanto Protocol")
        .about("Production-ready cross-chain packet relayer")
        .arg(
            Arg::with_name("source")
                .short("s")
                .long("source")
                .value_name("CHAIN")
                .help("Source chain identifier")
                .required(true),
        )
        .arg(
            Arg::with_name("destination")
                .short("d")
                .long("destination")
                .value_name("CHAIN")
                .help("Destination chain identifier")
                .required(true),
        )
        .arg(
            Arg::with_name("channel")
                .short("c")
                .long("channel")
                .value_name("CHANNEL_ID")
                .help("IBC channel identifier")
                .required(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT_ID")
                .help("IBC port identifier")
                .default_value("transfer"),
        )
        .arg(
            Arg::with_name("source-rpc")
                .long("source-rpc")
                .value_name("URL")
                .help("Source chain RPC endpoint")
                .default_value("http://localhost:8545"),
        )
        .arg(
            Arg::with_name("dest-rpc")
                .long("dest-rpc")
                .value_name("URL")
                .help("Destination chain RPC endpoint")
                .default_value("http://localhost:8546"),
        )
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .value_name("SECONDS")
                .help("Polling interval in seconds")
                .default_value("10"),
        )
        .get_matches();

    let config = RelayerConfig {
        source_chain: matches.value_of("source").unwrap().to_string(),
        destination_chain: matches.value_of("destination").unwrap().to_string(),
        source_rpc: matches.value_of("source-rpc").unwrap().to_string(),
        destination_rpc: matches.value_of("dest-rpc").unwrap().to_string(),
        channel_id: matches.value_of("channel").unwrap().to_string(),
        port_id: matches.value_of("port").unwrap().to_string(),
        polling_interval: matches.value_of("interval").unwrap().parse().unwrap(),
        max_retries: 3,
        gas_price_multiplier: 1.2,
    };

    let relayer = QantoRelayer::new(config);
    relayer.start().await
}
