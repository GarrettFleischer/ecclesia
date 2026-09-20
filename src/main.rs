#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ecclesia::http::serve().await
}
