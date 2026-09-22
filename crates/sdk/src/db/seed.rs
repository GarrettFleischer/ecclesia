use super::bind::Bind;
use super::seed_data::GIFTS;
use super::Db;

impl Db {
    pub(crate) async fn seed_if_empty(&self) -> anyhow::Result<()> {
        if self.gift_count().await? > 0 {
            return Ok(());
        }
        insert_gift_rows(self, GIFTS).await?;
        tracing::info!("seeded gift catalog");
        Ok(())
    }

    async fn gift_count(&self) -> anyhow::Result<i64> {
        self.fetch_scalar_i64("SELECT COUNT(*) FROM gifts", &[]).await
    }
}

async fn insert_gift_rows(db: &Db, rows: &[(&str, &str, &str)]) -> anyhow::Result<()> {
    for row in rows {
        let (id, name, category) = *row;
        db.execute(
            "INSERT INTO gifts (id, name, category) VALUES (?, ?, ?)",
            &[Bind::Text(id), Bind::Text(name), Bind::Text(category)],
        )
        .await?;
    }
    Ok(())
}
