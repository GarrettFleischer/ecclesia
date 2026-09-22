//! ecclesia-worker. Claims outbox rows. No HTTP.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ecclesia=info".into()),
        )
        .init();

    let _mail = ecclesia_sdk::host::load_mail_env("http://127.0.0.1:3000")?;
    let db = ecclesia_sdk::Db::connect_from_env().await?;
    let push = ecclesia_sdk::push::PushHub::load();
    tracing::info!("ecclesia-worker polling the outbox");
    ecclesia_sdk::outbox::run_worker(&db, &push).await
}
