#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Agg 15m service placeholder starting...");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
