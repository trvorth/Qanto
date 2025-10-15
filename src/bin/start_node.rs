use qanto::start_node;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    start_node::run().await
}
