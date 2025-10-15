use qanto::qantowallet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    qantowallet::run().await
}
