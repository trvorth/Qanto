use qanto::metrics_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    metrics_server::run().await
}
