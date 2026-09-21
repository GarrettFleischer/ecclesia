//! SQLite skin. Reads the world and applies leaf effects.

mod apply;
mod churches;
mod gifts;
mod needs;
mod notices;
mod schema;
mod seed;
mod users;

use anyhow::Context;
use sqlx::{
    sqlite::SqliteConnectOptions, sqlite::SqliteJournalMode, sqlite::SqlitePoolOptions, SqlitePool,
};
use std::str::FromStr;

use crate::leaf::Effect;

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await
            .with_context(|| format!("connecting to {url}"))?;
        let db = Self { pool };
        db.migrate().await?;
        db.seed_if_empty().await?;
        Ok(db)
    }

    pub async fn apply(&self, effect: &Effect) -> anyhow::Result<()> {
        apply::apply(&self.pool, effect).await
    }
}
