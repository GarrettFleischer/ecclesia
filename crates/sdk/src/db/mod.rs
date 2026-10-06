//! Store skin. Reads the world and applies Domain effects.

mod apply;
mod bind;
mod churches;
mod dialect;
mod distance;
mod extras;
mod gifts;
mod needs;
mod notices;
mod outbox;
mod prayers;
mod push;
mod query;
mod rows;
mod schema;
mod seed;
mod seed_data;
mod sessions;
mod shares;
mod users;

use anyhow::Context;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{PgPool, SqlitePool};
use std::str::FromStr;

use ecclesia_domain::Effect;

use crate::host::is_public_host;

pub use dialect::{
    Driver, LOCAL_SQLITE_DEFAULT, allow_remote_database_url, is_local_database_url,
    local_database_url, require_public_database_url, rewrite_placeholders,
};
pub use extras::{
    MailWrite, PasswordHashWrite, SessionRow, SessionTransport, SessionWrite, StoryExtras,
    TokenRow, TokenWrite,
};
pub use needs::ClosedNeedGroup;
pub use outbox::{OutboxFinish, OutboxRow};
pub use push::{PushDevice, PushSubscription};

use dialect::Driver as StoreDriver;

#[derive(Clone)]
pub struct Db {
    inner: Inner,
}

#[derive(Clone)]
pub(crate) enum Inner {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

impl Db {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        match StoreDriver::from_url(url)? {
            StoreDriver::Sqlite => connect_sqlite(url).await,
            StoreDriver::Postgres => connect_postgres(url).await,
        }
    }

    pub async fn connect_from_env() -> anyhow::Result<Self> {
        let url = std::env::var("DATABASE_URL").ok();
        if is_public_host() {
            let url = require_public_database_url(url.as_deref())?;
            return Db::connect(url).await;
        }
        let url = local_database_url(url);
        Db::connect(&url).await
    }

    pub async fn apply(&self, effect: &Effect) -> anyhow::Result<Vec<String>> {
        self.apply_with(effect, &StoryExtras::default()).await
    }

    pub async fn apply_with(
        &self,
        effect: &Effect,
        extras: &StoryExtras,
    ) -> anyhow::Result<Vec<String>> {
        apply::apply_with(self, effect, extras).await
    }
}

async fn connect_sqlite(url: &str) -> anyhow::Result<Db> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    // Schema changes on a pooled SQLite connection are invisible to the
    // connection that still holds the old schema, so DROP INDEX then CREATE
    // INDEX of the same name fails with "already exists". Migrate on one
    // connection, then open the pool the rest of the process uses.
    let migrate_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .with_context(|| format!("connecting to {url}"))?;
    let migrator = Db {
        inner: Inner::Sqlite(migrate_pool.clone()),
    };
    migrator.migrate().await?;
    migrator.seed_if_empty().await?;
    migrate_pool.close().await;
    let pool = SqlitePoolOptions::new()
        .max_connections(StoreDriver::Sqlite.pool_size())
        .connect_with(options)
        .await
        .with_context(|| format!("connecting to {url}"))?;
    Ok(Db {
        inner: Inner::Sqlite(pool),
    })
}

async fn connect_postgres(url: &str) -> anyhow::Result<Db> {
    let options = PgConnectOptions::from_str(url)?;
    let pool = PgPoolOptions::new()
        .max_connections(StoreDriver::Postgres.pool_size())
        .connect_with(options)
        .await
        .with_context(|| format!("connecting to {url}"))?;
    let db = Db {
        inner: Inner::Postgres(pool),
    };
    db.migrate().await?;
    db.seed_if_empty().await?;
    Ok(db)
}
