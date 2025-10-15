use qanto::qanto;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    qanto::run().await
}
