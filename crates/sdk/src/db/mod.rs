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
pub use rows::{AttachmentRow, UnknownReplyKind, reply_kind_from_column};

pub(crate) use bind::Bind;

/// A profile photo reference. `None` from the loader means the user row is missing.
///
/// # Notes
/// [`AvatarReference::Initials`] is a profile with no photo. The media asset
/// itself is not loaded here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvatarReference {
    Initials,
    Photo(String),
}

/// The reply that closed a need, if one is stored.
///
/// # Notes
/// [`ClosingReplyReference::Unset`] is how a legacy met need keeps its praise
/// without a completion reply. `None` from the loader means the need row is missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosingReplyReference {
    Unset,
    Reply(String),
}
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

    /// Photos attached to a need, in `position` order.
    ///
    /// # Parameters
    /// - `need_id`: the need whose links to load.
    ///
    /// # Returns
    /// Attachment rows. An empty vec means the need has no photos.
    pub async fn need_attachments(&self, need_id: &str) -> anyhow::Result<Vec<AttachmentRow>> {
        self.attachment_rows(
            "SELECT media_id, position, description FROM need_media WHERE need_id = ? ORDER BY position ASC, media_id ASC",
            need_id,
        )
        .await
    }

    /// Photos attached to a reply, in `position` order.
    ///
    /// # Parameters
    /// - `reply_id`: the reply whose links to load.
    ///
    /// # Returns
    /// Attachment rows. An empty vec means the reply has no photos.
    pub async fn reply_attachments(&self, reply_id: &str) -> anyhow::Result<Vec<AttachmentRow>> {
        self.attachment_rows(
            "SELECT media_id, position, description FROM reply_media WHERE reply_id = ? ORDER BY position ASC, media_id ASC",
            reply_id,
        )
        .await
    }

    /// The profile photo pointer for a user.
    ///
    /// # Returns
    /// `None` when the user does not exist. [`AvatarReference::Initials`] when
    /// the column is null.
    pub async fn avatar_reference(&self, user_id: &str) -> anyhow::Result<Option<AvatarReference>> {
        let Some(row) = self
            .fetch_optional::<rows::OptionalTextRow>(
                "SELECT avatar_media_id AS value FROM users WHERE id = ?",
                &[Bind::Text(user_id)],
            )
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(reference_from_column(row.value)))
    }

    /// The closing reply pointer for a need.
    ///
    /// # Returns
    /// `None` when the need does not exist. [`ClosingReplyReference::Unset`]
    /// when the column is null.
    pub async fn closing_reply_reference(
        &self,
        need_id: &str,
    ) -> anyhow::Result<Option<ClosingReplyReference>> {
        let Some(row) = self
            .fetch_optional::<rows::OptionalTextRow>(
                "SELECT closing_reply_id AS value FROM needs WHERE id = ?",
                &[Bind::Text(need_id)],
            )
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(closing_from_column(row.value)))
    }

    /// One reply, including its kind.
    ///
    /// # Returns
    /// `None` when the reply does not exist. An unknown nonempty kind is an error.
    pub async fn need_reply(&self, id: &str) -> anyhow::Result<Option<ecclesia_domain::NeedReply>> {
        let Some(row) = self
            .fetch_optional::<rows::NeedReplyRow>(
                "SELECT id, need_id, author_id, body, created_at, kind FROM need_replies WHERE id = ?",
                &[Bind::Text(id)],
            )
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(rows::need_reply_from_row(row)?))
    }

    async fn attachment_rows(
        &self,
        sql: &str,
        owner_id: &str,
    ) -> anyhow::Result<Vec<AttachmentRow>> {
        self.fetch_all(sql, &[Bind::Text(owner_id)]).await
    }
}

fn reference_from_column(value: Option<String>) -> AvatarReference {
    match value {
        Some(media_id) => AvatarReference::Photo(media_id),
        None => AvatarReference::Initials,
    }
}

fn closing_from_column(value: Option<String>) -> ClosingReplyReference {
    match value {
        Some(reply_id) => ClosingReplyReference::Reply(reply_id),
        None => ClosingReplyReference::Unset,
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
