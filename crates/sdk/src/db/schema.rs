use super::Db;
use super::bind::Bind;

impl Db {
    pub(crate) async fn migrate(&self) -> anyhow::Result<()> {
        for statement in SCHEMA {
            self.execute(statement, &[]).await?;
        }
        add_endorsement_skill_column(self).await?;
        backfill_endorsement_skills(self).await?;
        add_password_hash_column(self).await?;
        add_auth_tables(self).await?;
        add_session_api_columns(self).await?;
        reshape_account_place(self).await?;
        move_church_columns_to_memberships(self).await?;
        ensure_account_indexes(self).await?;
        // Offer text stays on `applications`. Copying it into `need_replies`
        // would show a private offer to the need's audience.
        add_church_registration(self).await?;
        add_need_archive(self).await?;
        add_need_praise(self).await?;
        install_soft_delete(self).await?;
        self.archive_met_needs().await?;
        self.ensure_need_shares().await?;
        add_media_conversations(self).await?;
        Ok(())
    }
}

async fn add_endorsement_skill_column(db: &Db) -> anyhow::Result<()> {
    if let Err(error) = db
        .execute(
            "ALTER TABLE endorsements ADD COLUMN skill TEXT NOT NULL DEFAULT ''",
            &[],
        )
        .await
    {
        let message = error.to_string();
        if !message.contains("duplicate column") && !message.contains("already exists") {
            return Err(error);
        }
    }
    Ok(())
}

async fn backfill_endorsement_skills(db: &Db) -> anyhow::Result<()> {
    db.execute(
        "UPDATE endorsements
         SET skill = (SELECT name FROM gifts WHERE gifts.id = endorsements.gift_id)
         WHERE skill = '' AND gift_id != ''",
        &[] as &[Bind<'_>],
    )
    .await?;
    Ok(())
}

const SCHEMA: &[&str] = &[
    r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                first_name TEXT NOT NULL,
                last_name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                bio TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                password_hash TEXT
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS memberships (
                user_id TEXT NOT NULL,
                church_id TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                deleted_at TEXT,
                PRIMARY KEY (user_id, church_id)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS churches (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                address TEXT NOT NULL,
                latitude REAL NOT NULL,
                longitude REAL NOT NULL,
                country TEXT NOT NULL DEFAULT 'US',
                description TEXT NOT NULL,
                gathering TEXT NOT NULL DEFAULT '',
                ein TEXT NOT NULL DEFAULT '',
                registry_state TEXT NOT NULL DEFAULT '',
                registry_number TEXT NOT NULL DEFAULT '',
                owner_id TEXT NOT NULL,
                invite_code TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                deleted_at TEXT
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS gifts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                category TEXT NOT NULL
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS member_gifts (
                user_id TEXT NOT NULL,
                gift_id TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                deleted_at TEXT,
                PRIMARY KEY (user_id, gift_id)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS needs (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                gift_id TEXT,
                scope TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                closed_at TEXT,
                praise TEXT,
                archived INTEGER NOT NULL DEFAULT 0
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS shares (
                code TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                target_id TEXT NOT NULL,
                expires_at TEXT,
                deleted_at TEXT,
                UNIQUE (kind, target_id)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS need_replies (
                id TEXT PRIMARY KEY,
                need_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                kind TEXT NOT NULL DEFAULT 'message' CHECK (kind IN ('message', 'completion'))
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS prayers (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                author_id TEXT,
                body TEXT NOT NULL,
                status TEXT NOT NULL,
                praise TEXT,
                manage_hash TEXT,
                created_at TEXT NOT NULL,
                answered_at TEXT
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS prayer_marks (
                user_id TEXT NOT NULL,
                prayer_id TEXT NOT NULL,
                day TEXT NOT NULL,
                kind TEXT NOT NULL,
                PRIMARY KEY (user_id, prayer_id, day)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS applications (
                id TEXT PRIMARY KEY,
                need_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                message TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(need_id, user_id)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS endorsements (
                id TEXT PRIMARY KEY,
                from_user_id TEXT NOT NULL,
                to_user_id TEXT NOT NULL,
                gift_id TEXT NOT NULL DEFAULT '',
                skill TEXT NOT NULL DEFAULT '',
                note TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                href TEXT NOT NULL DEFAULT '/',
                read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS push_subscriptions (
                endpoint TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                p256dh TEXT NOT NULL,
                auth TEXT NOT NULL,
                created_at TEXT NOT NULL,
                deleted_at TEXT
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS push_devices (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                platform TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS outbox (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                payload TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                available_at TEXT NOT NULL,
                status TEXT NOT NULL,
                dead_at TEXT
            )
            "#,
    "CREATE INDEX IF NOT EXISTS idx_needs_status_created ON needs (status, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_needs_church_status ON needs (church_id, status)",
    "CREATE INDEX IF NOT EXISTS idx_need_replies_need ON need_replies (need_id, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_prayers_church_status ON prayers (church_id, status)",
    "CREATE INDEX IF NOT EXISTS idx_prayer_marks_user_day ON prayer_marks (user_id, day)",
    "CREATE INDEX IF NOT EXISTS idx_memberships_church_status ON memberships (church_id, status)",
    "CREATE INDEX IF NOT EXISTS idx_memberships_user ON memberships (user_id)",
    "CREATE INDEX IF NOT EXISTS idx_churches_name_id ON churches (name, id)",
    "CREATE INDEX IF NOT EXISTS idx_notifications_user_created ON notifications (user_id, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_outbox_available ON outbox (available_at) WHERE status != 'done'",
];

async fn add_church_registration(db: &Db) -> anyhow::Result<()> {
    for sql in [
        "ALTER TABLE churches ADD COLUMN ein TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE churches ADD COLUMN registry_state TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE churches ADD COLUMN registry_number TEXT NOT NULL DEFAULT ''",
    ] {
        add_column(db, sql).await?;
    }
    Ok(())
}

async fn add_need_praise(db: &Db) -> anyhow::Result<()> {
    add_column(db, "ALTER TABLE needs ADD COLUMN praise TEXT").await
}

const MEDIA_TABLES: &[&str] = &[
    r#"
            CREATE TABLE IF NOT EXISTS media_assets (
                id TEXT PRIMARY KEY,
                owner_id TEXT NOT NULL REFERENCES users (id),
                state TEXT NOT NULL CHECK (state IN ('staged', 'attached', 'deleting')),
                staging_full_key TEXT NOT NULL,
                staging_thumb_key TEXT NOT NULL,
                full_key TEXT,
                thumb_key TEXT,
                width INTEGER NOT NULL CHECK (width > 0),
                height INTEGER NOT NULL CHECK (height > 0),
                full_bytes INTEGER NOT NULL CHECK (full_bytes > 0),
                thumb_bytes INTEGER NOT NULL CHECK (thumb_bytes > 0),
                created_at TEXT NOT NULL,
                attached_at TEXT,
                deleted_at TEXT
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS need_media (
                need_id TEXT NOT NULL REFERENCES needs (id),
                media_id TEXT NOT NULL REFERENCES media_assets (id),
                position INTEGER NOT NULL CHECK (position >= 0 AND position <= 4),
                description TEXT CHECK (description IS NULL OR length(description) <= 300),
                PRIMARY KEY (need_id, media_id)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS reply_media (
                reply_id TEXT NOT NULL REFERENCES need_replies (id),
                media_id TEXT NOT NULL REFERENCES media_assets (id),
                position INTEGER NOT NULL CHECK (position >= 0 AND position <= 4),
                description TEXT CHECK (description IS NULL OR length(description) <= 300),
                PRIMARY KEY (reply_id, media_id)
            )
            "#,
    "CREATE INDEX IF NOT EXISTS idx_media_assets_owner_state_created ON media_assets (owner_id, state, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_media_assets_state_created ON media_assets (state, created_at) WHERE deleted_at IS NULL",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_need_media_position ON need_media (need_id, position)",
    "CREATE INDEX IF NOT EXISTS idx_need_media_media ON need_media (media_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_reply_media_position ON reply_media (reply_id, position)",
    "CREATE INDEX IF NOT EXISTS idx_reply_media_media ON reply_media (media_id)",
];

const MEDIA_COLUMNS: &[&str] = &[
    "ALTER TABLE users ADD COLUMN avatar_media_id TEXT REFERENCES media_assets (id)",
    "ALTER TABLE need_replies ADD COLUMN kind TEXT NOT NULL DEFAULT 'message' CHECK (kind IN ('message', 'completion'))",
    "ALTER TABLE needs ADD COLUMN closing_reply_id TEXT REFERENCES need_replies (id)",
];

const MEDIA_COLUMN_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_users_avatar_media ON users (avatar_media_id) WHERE avatar_media_id IS NOT NULL",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_needs_closing_reply ON needs (closing_reply_id) WHERE closing_reply_id IS NOT NULL",
];

/// Private photos and the conversation columns that point at them.
///
/// Runs after account reshape so a legacy `DROP TABLE users` still succeeds.
/// New foreign keys are column clauses on `ADD COLUMN` or on new tables.
/// `CREATE TABLE IF NOT EXISTS` cannot change a table that already exists.
/// `needs.praise` is not copied or rewritten.
async fn add_media_conversations(db: &Db) -> anyhow::Result<()> {
    for statement in MEDIA_TABLES {
        db.execute(statement, &[]).await?;
    }
    for statement in MEDIA_COLUMNS {
        add_column(db, statement).await?;
    }
    for statement in MEDIA_COLUMN_INDEXES {
        db.execute(statement, &[]).await?;
    }
    Ok(())
}

async fn add_need_archive(db: &Db) -> anyhow::Result<()> {
    add_column(db, "ALTER TABLE needs ADD COLUMN closed_at TEXT").await?;
    add_column(
        db,
        "ALTER TABLE needs ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    add_column(db, "ALTER TABLE shares ADD COLUMN expires_at TEXT").await?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_needs_listed ON needs (church_id, created_at, id) WHERE archived = 0",
        &[],
    )
    .await?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_needs_author_open ON needs (author_id, church_id) WHERE status = 'open' AND archived = 0",
        &[],
    )
    .await?;
    db.execute(
        "UPDATE needs SET closed_at = created_at WHERE status = 'closed' AND closed_at IS NULL",
        &[] as &[Bind<'_>],
    )
    .await?;
    Ok(())
}

async fn add_column(db: &Db, sql: &str) -> anyhow::Result<()> {
    if let Err(error) = db.execute(sql, &[]).await {
        let message = error.to_string();
        if !message.contains("duplicate column") && !message.contains("already exists") {
            return Err(error);
        }
    }
    Ok(())
}

async fn add_password_hash_column(db: &Db) -> anyhow::Result<()> {
    if let Err(error) = db
        .execute("ALTER TABLE users ADD COLUMN password_hash TEXT", &[])
        .await
    {
        let message = error.to_string();
        if !message.contains("duplicate column") && !message.contains("already exists") {
            return Err(error);
        }
    }
    Ok(())
}

async fn add_session_api_columns(db: &Db) -> anyhow::Result<()> {
    for sql in [
        "ALTER TABLE sessions ADD COLUMN transport TEXT NOT NULL DEFAULT 'cookie'",
        "ALTER TABLE sessions ADD COLUMN refresh_token_hash TEXT",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_refresh_hash ON sessions (refresh_token_hash) WHERE refresh_token_hash IS NOT NULL",
    ] {
        if let Err(error) = db.execute(sql, &[]).await {
            let message = error.to_string();
            if message.contains("duplicate column")
                || message.contains("already exists")
                || message.contains("duplicate key")
            {
                continue;
            }
            return Err(error);
        }
    }
    Ok(())
}

async fn add_auth_tables(db: &Db) -> anyhow::Result<()> {
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                csrf TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                user_agent TEXT NOT NULL DEFAULT '',
                ip TEXT NOT NULL DEFAULT '',
                transport TEXT NOT NULL DEFAULT 'cookie',
                refresh_token_hash TEXT,
                deleted_at TEXT
            )"#,
        "CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id)",
        r#"CREATE TABLE IF NOT EXISTS magic_links (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                consumed_at TEXT,
                created_at TEXT NOT NULL
            )"#,
        r#"CREATE TABLE IF NOT EXISTS password_resets (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                consumed_at TEXT,
                created_at TEXT NOT NULL
            )"#,
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_magic_live ON magic_links (user_id) WHERE consumed_at IS NULL",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_reset_live ON password_resets (user_id) WHERE consumed_at IS NULL",
        "CREATE INDEX IF NOT EXISTS idx_magic_user ON magic_links (user_id)",
        "CREATE INDEX IF NOT EXISTS idx_reset_user ON password_resets (user_id)",
    ] {
        db.execute(statement, &[]).await?;
    }
    Ok(())
}

async fn reshape_account_place(db: &Db) -> anyhow::Result<()> {
    let legacy_users = count_sql(
        db,
        if db.driver().is_postgres() {
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'name'"
        } else {
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'name'"
        },
    )
    .await?;
    if !legacy_users {
        return Ok(());
    }
    db.execute("DROP TABLE IF EXISTS memberships", &[]).await?;
    db.execute("DROP TABLE IF EXISTS users", &[]).await?;
    db.execute("DROP TABLE IF EXISTS churches", &[]).await?;
    for statement in USERS_AND_CHURCHES {
        db.execute(statement, &[]).await?;
    }
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_churches_name_id ON churches (name, id)",
        &[],
    )
    .await?;
    Ok(())
}

async fn ensure_account_indexes(db: &Db) -> anyhow::Result<()> {
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_churches_name_id ON churches (name, id)",
        &[],
    )
    .await?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_memberships_church_status ON memberships (church_id, status)",
        &[],
    )
    .await?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_memberships_user ON memberships (user_id)",
        &[],
    )
    .await?;
    Ok(())
}

async fn move_church_columns_to_memberships(db: &Db) -> anyhow::Result<()> {
    if !column_exists(db, "users", "church_id").await? {
        return Ok(());
    }
    db.execute(
        "INSERT INTO memberships (user_id, church_id, role, status, created_at)
         SELECT id, church_id, COALESCE(church_role, 'member'), COALESCE(church_status, 'active'), created_at
         FROM users
         WHERE church_id IS NOT NULL
           AND NOT EXISTS (
               SELECT 1 FROM memberships m
               WHERE m.user_id = users.id AND m.church_id = users.church_id
           )",
        &[],
    )
    .await?;
    let _ = db
        .execute("DROP INDEX IF EXISTS idx_users_church", &[])
        .await;
    for column in ["church_role", "church_status", "church_id"] {
        if column_exists(db, "users", column).await? {
            db.execute(&format!("ALTER TABLE users DROP COLUMN {column}"), &[])
                .await?;
        }
    }
    Ok(())
}

async fn column_exists(db: &Db, table: &str, column: &str) -> anyhow::Result<bool> {
    let sql = if db.driver().is_postgres() {
        format!(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = '{table}' AND column_name = '{column}'"
        )
    } else {
        format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'")
    };
    Ok(db.fetch_scalar_i64(&sql, &[]).await? > 0)
}

const USERS_AND_CHURCHES: &[&str] = &[
    r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                first_name TEXT NOT NULL,
                last_name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                bio TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                password_hash TEXT
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS memberships (
                user_id TEXT NOT NULL,
                church_id TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                deleted_at TEXT,
                PRIMARY KEY (user_id, church_id)
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS churches (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                address TEXT NOT NULL,
                latitude REAL NOT NULL,
                longitude REAL NOT NULL,
                country TEXT NOT NULL DEFAULT 'US',
                description TEXT NOT NULL,
                gathering TEXT NOT NULL DEFAULT '',
                ein TEXT NOT NULL DEFAULT '',
                registry_state TEXT NOT NULL DEFAULT '',
                registry_number TEXT NOT NULL DEFAULT '',
                owner_id TEXT NOT NULL,
                invite_code TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                deleted_at TEXT
            )
            "#,
];

/// Live reads use the `*_live` views. Partial indexes hold only rows with `deleted_at IS NULL`.
async fn install_soft_delete(db: &Db) -> anyhow::Result<()> {
    for sql in [
        "ALTER TABLE churches ADD COLUMN deleted_at TEXT",
        "ALTER TABLE memberships ADD COLUMN deleted_at TEXT",
        "ALTER TABLE member_gifts ADD COLUMN deleted_at TEXT",
        "ALTER TABLE push_subscriptions ADD COLUMN deleted_at TEXT",
        "ALTER TABLE sessions ADD COLUMN deleted_at TEXT",
        "ALTER TABLE shares ADD COLUMN deleted_at TEXT",
    ] {
        add_column(db, sql).await?;
    }
    // One connection, so a drop is visible to the create that follows it.
    let mut statements = Vec::with_capacity(LIVE_INDEXES.len() + LIVE_VIEWS.len());
    statements.extend_from_slice(LIVE_INDEXES);
    statements.extend_from_slice(LIVE_VIEWS);
    execute_schema(db, &statements).await
}

async fn execute_schema(db: &Db, statements: &[&str]) -> anyhow::Result<()> {
    match &db.inner {
        super::Inner::Sqlite(pool) => {
            let mut conn = pool.acquire().await?;
            for sql in statements {
                sqlx::query(sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| anyhow::anyhow!("{sql}: {error}"))?;
            }
        }
        super::Inner::Postgres(pool) => {
            let mut conn = pool.acquire().await?;
            for sql in statements {
                let sql = super::Driver::Postgres.sql(sql);
                sqlx::query(&sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| anyhow::anyhow!("{sql}: {error}"))?;
            }
        }
    }
    Ok(())
}

const LIVE_INDEXES: &[&str] = &[
    "DROP INDEX IF EXISTS idx_churches_name_id",
    "CREATE INDEX IF NOT EXISTS idx_churches_name_id ON churches (name, id) WHERE deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_sessions_user",
    "CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id) WHERE deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_sessions_refresh_hash",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_refresh_hash ON sessions (refresh_token_hash) WHERE refresh_token_hash IS NOT NULL AND deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_member_gifts_live",
    "CREATE INDEX IF NOT EXISTS idx_member_gifts_live ON member_gifts (user_id) WHERE deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_push_live",
    "CREATE INDEX IF NOT EXISTS idx_push_live ON push_subscriptions (user_id) WHERE deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_memberships_church_status",
    "CREATE INDEX IF NOT EXISTS idx_memberships_church_status ON memberships (church_id, status) WHERE deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_memberships_user",
    "CREATE INDEX IF NOT EXISTS idx_memberships_user ON memberships (user_id) WHERE deleted_at IS NULL",
    "DROP INDEX IF EXISTS idx_shares_live_target",
    "CREATE INDEX IF NOT EXISTS idx_shares_live_target ON shares (kind, target_id) WHERE deleted_at IS NULL",
];

const LIVE_VIEWS: &[&str] = &[
    "DROP VIEW IF EXISTS churches_live",
    "CREATE VIEW churches_live AS
        SELECT id, name, address, latitude, longitude, country, description, gathering,
               ein, registry_state, registry_number, owner_id, invite_code, created_at
        FROM churches
        WHERE deleted_at IS NULL",
    "DROP VIEW IF EXISTS sessions_live",
    "CREATE VIEW sessions_live AS
        SELECT id, user_id, csrf, created_at, last_seen_at, user_agent, ip, transport, refresh_token_hash
        FROM sessions
        WHERE deleted_at IS NULL",
    "DROP VIEW IF EXISTS member_gifts_live",
    "CREATE VIEW member_gifts_live AS
        SELECT user_id, gift_id, note
        FROM member_gifts
        WHERE deleted_at IS NULL",
    "DROP VIEW IF EXISTS push_subscriptions_live",
    "CREATE VIEW push_subscriptions_live AS
        SELECT endpoint, user_id, p256dh, auth, created_at
        FROM push_subscriptions
        WHERE deleted_at IS NULL",
    "DROP VIEW IF EXISTS memberships_live",
    "CREATE VIEW memberships_live AS
        SELECT user_id, church_id, role, status, created_at
        FROM memberships
        WHERE deleted_at IS NULL",
    "DROP VIEW IF EXISTS shares_live",
    "CREATE VIEW shares_live AS
        SELECT code, kind, target_id, expires_at
        FROM shares
        WHERE deleted_at IS NULL",
];

async fn count_sql(db: &Db, sql: &str) -> anyhow::Result<bool> {
    Ok(db.fetch_scalar_i64(sql, &[]).await? > 0)
}

#[cfg(test)]
mod tests {
    use super::Bind;
    use super::Db;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    const LEGACY: &[&str] = &[
        r#"CREATE TABLE users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                city TEXT NOT NULL,
                region TEXT NOT NULL,
                bio TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                password_hash TEXT
            )"#,
        r#"CREATE TABLE churches (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                city TEXT NOT NULL,
                region TEXT NOT NULL,
                country TEXT NOT NULL DEFAULT 'US',
                description TEXT NOT NULL,
                gathering TEXT NOT NULL DEFAULT '',
                owner_id TEXT NOT NULL,
                invite_code TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            )"#,
        r#"CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                csrf TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                user_agent TEXT NOT NULL DEFAULT '',
                ip TEXT NOT NULL DEFAULT ''
            )"#,
        r#"CREATE TABLE memberships (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(church_id, user_id)
            )"#,
        "CREATE INDEX idx_sessions_user ON sessions (user_id)",
    ];

    fn temp_sqlite_url() -> String {
        let path = std::env::temp_dir().join(format!(
            "ecclesia-legacy-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        format!("sqlite://{}", path.display())
    }

    async fn has_column(db: &Db, table: &str, name: &str) -> bool {
        let sql =
            format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{name}'");
        db.fetch_scalar_i64(&sql, &[]).await.unwrap() > 0
    }

    #[tokio::test]
    async fn legacy_sqlite_migrates_to_account_without_a_church() {
        let url = temp_sqlite_url();
        let options = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        for sql in LEGACY {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        pool.close().await;

        let db = Db::connect(&url).await.expect("legacy database migrates");
        assert!(has_column(&db, "users", "first_name").await);
        assert!(has_column(&db, "users", "last_name").await);
        assert!(!has_column(&db, "users", "name").await);
        assert!(!has_column(&db, "users", "city").await);
        assert!(has_column(&db, "sessions", "refresh_token_hash").await);
        assert!(has_column(&db, "sessions", "transport").await);
        assert!(has_column(&db, "churches", "address").await);
        assert!(has_column(&db, "churches", "latitude").await);
        assert!(has_column(&db, "churches", "longitude").await);
        assert!(has_column(&db, "churches", "ein").await);
        assert!(has_column(&db, "churches", "registry_state").await);
        assert!(has_column(&db, "churches", "registry_number").await);
        assert!(!has_column(&db, "churches", "city").await);
        let memberships = db
            .fetch_scalar_i64(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'memberships'",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(memberships, 1);
        assert!(!has_column(&db, "users", "church_id").await);
        assert!(has_column(&db, "users", "avatar_media_id").await);
        assert!(has_column(&db, "needs", "praise").await);
        assert!(has_column(&db, "needs", "closing_reply_id").await);
        assert!(has_column(&db, "need_replies", "kind").await);
    }

    #[tokio::test]
    async fn fresh_sqlite_media_schema_migrates_twice_and_enforces_links() {
        let db = Db::connect(&temp_sqlite_url())
            .await
            .expect("fresh database");
        assert_media_shape(&db).await;
        db.migrate().await.expect("second migrate");
        assert_media_shape(&db).await;

        insert_user(&db, "ada", "ada@example.test").await;
        insert_open_need(&db, "need_1", "ada").await;
        insert_open_need(&db, "need_2", "ada").await;
        insert_reply(&db, "reply_1", "need_1", "ada").await;
        assert_eq!(
            text(
                &db,
                "SELECT kind FROM need_replies WHERE id = ?",
                &[Bind::Text("reply_1")],
            )
            .await,
            "message"
        );

        insert_staged(&db, "m2", "ada").await;
        insert_staged(&db, "m0", "ada").await;
        insert_staged(&db, "m1", "ada").await;
        attach_need(&db, "need_1", "m2", 2, None).await;
        attach_need(&db, "need_1", "m0", 0, Some("west slope")).await;
        attach_need(&db, "need_1", "m1", 1, None).await;
        assert_eq!(
            db.fetch_strings(
                "SELECT media_id FROM need_media WHERE need_id = ? ORDER BY position",
                &[Bind::Text("need_1")],
            )
            .await
            .unwrap(),
            vec!["m0", "m1", "m2"]
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM need_media WHERE media_id = ? AND description IS NULL",
                &[Bind::Text("m1")],
            )
            .await,
            1
        );

        let fitted = "a".repeat(300);
        let too_long = "a".repeat(301);
        insert_staged(&db, "m3", "ada").await;
        expect_refused(
            &db,
            "INSERT INTO need_media (need_id, media_id, position, description) VALUES (?, ?, ?, ?)",
            &[
                Bind::Text("need_1"),
                Bind::Text("m3"),
                Bind::I64(3),
                Bind::Text(&too_long),
            ],
        )
        .await;
        attach_need(&db, "need_1", "m3", 3, Some(&fitted)).await;
        insert_staged(&db, "m4", "ada").await;
        attach_need(&db, "need_1", "m4", 4, None).await;
        insert_staged(&db, "m5", "ada").await;
        expect_refused(
            &db,
            "INSERT INTO need_media (need_id, media_id, position, description) VALUES (?, ?, ?, NULL)",
            &[Bind::Text("need_1"), Bind::Text("m5"), Bind::I64(5)],
        )
        .await;
        expect_refused(
            &db,
            "INSERT INTO need_media (need_id, media_id, position, description) VALUES (?, ?, ?, NULL)",
            &[Bind::Text("need_1"), Bind::Text("m5"), Bind::I64(0)],
        )
        .await;
        expect_refused(
            &db,
            "INSERT INTO need_media (need_id, media_id, position, description) VALUES (?, ?, ?, NULL)",
            &[Bind::Text("need_1"), Bind::Text("m0"), Bind::I64(4)],
        )
        .await;
        insert_staged(&db, "m_other", "ada").await;
        attach_need(&db, "need_2", "m_other", 0, None).await;

        insert_staged(&db, "r1", "ada").await;
        insert_staged(&db, "r0", "ada").await;
        attach_reply(&db, "reply_1", "r1", 1).await;
        attach_reply(&db, "reply_1", "r0", 0).await;
        assert_eq!(
            db.fetch_strings(
                "SELECT media_id FROM reply_media WHERE reply_id = ? ORDER BY position",
                &[Bind::Text("reply_1")],
            )
            .await
            .unwrap(),
            vec!["r0", "r1"]
        );
        assert_eq!(
            text(
                &db,
                "SELECT reply_id FROM reply_media WHERE media_id = ?",
                &[Bind::Text("r0")],
            )
            .await,
            "reply_1"
        );
        assert_eq!(
            text(
                &db,
                "SELECT need_id FROM need_media WHERE media_id = ?",
                &[Bind::Text("m0")],
            )
            .await,
            "need_1"
        );

        expect_refused(
            &db,
            "INSERT INTO media_assets (
                id, owner_id, state, staging_full_key, staging_thumb_key,
                width, height, full_bytes, thumb_bytes, created_at
             ) VALUES ('ghost', 'missing_user', 'staged', 'stage/full/ghost', 'stage/thumb/ghost', 800, 600, 1000, 400, '2026-10-07T12:00:00Z')",
            &[],
        )
        .await;
        expect_refused(
            &db,
            "INSERT INTO media_assets (
                id, owner_id, state, staging_full_key, staging_thumb_key,
                width, height, full_bytes, thumb_bytes, created_at
             ) VALUES ('bad_state', 'ada', 'deleted', 'stage/full/bad', 'stage/thumb/bad', 800, 600, 1000, 400, '2026-10-07T12:00:00Z')",
            &[],
        )
        .await;
        expect_refused(
            &db,
            "INSERT INTO media_assets (
                id, owner_id, state, staging_full_key, staging_thumb_key,
                width, height, full_bytes, thumb_bytes, created_at
             ) VALUES ('flat', 'ada', 'staged', 'stage/full/flat', 'stage/thumb/flat', 0, 600, 1000, 400, '2026-10-07T12:00:00Z')",
            &[],
        )
        .await;
        expect_refused(
            &db,
            "DELETE FROM media_assets WHERE id = ?",
            &[Bind::Text("m0")],
        )
        .await;

        db.execute(
            "UPDATE media_assets
             SET state = 'attached', full_key = ?, thumb_key = ?, attached_at = ?
             WHERE id = ?",
            &[
                Bind::Text("full/m0"),
                Bind::Text("thumb/m0"),
                Bind::Text("2026-10-07T12:05:00Z"),
                Bind::Text("m0"),
            ],
        )
        .await
        .unwrap();
        db.execute(
            "UPDATE users SET avatar_media_id = ? WHERE id = ?",
            &[Bind::Text("m0"), Bind::Text("ada")],
        )
        .await
        .unwrap();
        expect_refused(
            &db,
            "UPDATE users SET avatar_media_id = ? WHERE id = ?",
            &[Bind::Text("missing_media"), Bind::Text("ada")],
        )
        .await;
        db.execute(
            "UPDATE need_replies SET kind = 'completion' WHERE id = ?",
            &[Bind::Text("reply_1")],
        )
        .await
        .unwrap();
        expect_refused(
            &db,
            "UPDATE need_replies SET kind = 'note' WHERE id = ?",
            &[Bind::Text("reply_1")],
        )
        .await;
        db.execute(
            "UPDATE needs SET closing_reply_id = ? WHERE id = ?",
            &[Bind::Text("reply_1"), Bind::Text("need_1")],
        )
        .await
        .unwrap();
        expect_refused(
            &db,
            "UPDATE needs SET closing_reply_id = ? WHERE id = ?",
            &[Bind::Text("missing_reply"), Bind::Text("need_2")],
        )
        .await;

        db.migrate().await.expect("migrate with rows");
        assert_eq!(
            text(
                &db,
                "SELECT kind FROM need_replies WHERE id = ?",
                &[Bind::Text("reply_1")],
            )
            .await,
            "completion"
        );
        assert_eq!(
            text(
                &db,
                "SELECT avatar_media_id FROM users WHERE id = ?",
                &[Bind::Text("ada")],
            )
            .await,
            "m0"
        );
        assert_eq!(
            text(
                &db,
                "SELECT closing_reply_id FROM needs WHERE id = ?",
                &[Bind::Text("need_1")],
            )
            .await,
            "reply_1"
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM need_replies WHERE need_id = ?",
                &[Bind::Text("need_1")],
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn legacy_praise_survives_and_existing_replies_default_to_message() {
        let url = temp_sqlite_url();
        let options = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE needs (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                gift_id TEXT,
                scope TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                closed_at TEXT,
                praise TEXT,
                archived INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO needs (
                id, church_id, author_id, title, body, gift_id, scope, status,
                created_at, closed_at, praise, archived
             ) VALUES (
                'need_old', 'church_old', 'author_old', 'Roof', 'It leaked.', NULL,
                'church', 'closed', '2019-01-01T00:00:00Z', '2019-06-01T00:00:00Z',
                'The roof is dry.', 0
             )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE need_replies (
                id TEXT PRIMARY KEY,
                need_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO need_replies (id, need_id, author_id, body, created_at)
             VALUES ('reply_old', 'need_old', 'author_old', 'I can help Saturday.', '2019-02-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let db = Db::connect(&url).await.expect("legacy need migrates");
        assert_legacy_conversation(&db).await;
        db.migrate().await.expect("second migrate");
        assert_legacy_conversation(&db).await;
    }

    async fn assert_legacy_conversation(db: &Db) {
        assert_eq!(
            text(
                db,
                "SELECT praise FROM needs WHERE id = ?",
                &[Bind::Text("need_old")],
            )
            .await,
            "The roof is dry."
        );
        assert_eq!(
            text(
                db,
                "SELECT kind FROM need_replies WHERE id = ?",
                &[Bind::Text("reply_old")],
            )
            .await,
            "message"
        );
        assert_eq!(
            count(
                db,
                "SELECT COUNT(*) FROM needs WHERE id = ? AND closing_reply_id IS NULL",
                &[Bind::Text("need_old")],
            )
            .await,
            1
        );
        assert_eq!(
            count(
                db,
                "SELECT COUNT(*) FROM need_replies WHERE need_id = ?",
                &[Bind::Text("need_old")],
            )
            .await,
            1
        );
        assert_eq!(
            count(
                db,
                "SELECT COUNT(*) FROM need_replies WHERE body = ?",
                &[Bind::Text("The roof is dry.")],
            )
            .await,
            0
        );
    }

    async fn assert_media_shape(db: &Db) {
        assert_columns(
            db,
            "media_assets",
            &[
                "owner_id",
                "state",
                "staging_full_key",
                "staging_thumb_key",
                "full_key",
                "thumb_key",
                "width",
                "height",
                "full_bytes",
                "thumb_bytes",
                "created_at",
                "attached_at",
                "deleted_at",
            ],
        )
        .await;
        assert_columns(
            db,
            "need_media",
            &["need_id", "media_id", "position", "description"],
        )
        .await;
        assert_columns(
            db,
            "reply_media",
            &["reply_id", "media_id", "position", "description"],
        )
        .await;
        assert!(has_column(db, "users", "avatar_media_id").await);
        assert!(has_column(db, "need_replies", "kind").await);
        assert!(has_column(db, "needs", "closing_reply_id").await);
        assert!(has_column(db, "needs", "praise").await);
        for name in [
            "idx_media_assets_owner_state_created",
            "idx_media_assets_state_created",
            "idx_need_media_position",
            "idx_need_media_media",
            "idx_reply_media_position",
            "idx_reply_media_media",
            "idx_users_avatar_media",
            "idx_needs_closing_reply",
        ] {
            assert!(has_index(db, name).await, "{name}");
        }
        let staged = index_sql(db, "idx_media_assets_state_created").await;
        assert!(staged.contains("deleted_at IS NULL"));
        let avatar = index_sql(db, "idx_users_avatar_media").await;
        assert!(avatar.contains("avatar_media_id IS NOT NULL"));
        let closing = index_sql(db, "idx_needs_closing_reply").await;
        assert!(closing.to_ascii_lowercase().contains("unique"));
        assert!(closing.contains("closing_reply_id IS NOT NULL"));
    }

    async fn assert_columns(db: &Db, table: &str, names: &[&str]) {
        for name in names {
            assert!(has_column(db, table, name).await, "{table}.{name}");
        }
    }

    async fn has_index(db: &Db, name: &str) -> bool {
        let sql =
            format!("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = '{name}'");
        db.fetch_scalar_i64(&sql, &[]).await.unwrap() > 0
    }

    async fn index_sql(db: &Db, name: &str) -> String {
        let sql = format!("SELECT sql FROM sqlite_master WHERE type = 'index' AND name = '{name}'");
        text(db, &sql, &[]).await
    }

    async fn text(db: &Db, sql: &str, binds: &[Bind<'_>]) -> String {
        let mut rows = db.fetch_strings(sql, binds).await.unwrap();
        assert_eq!(rows.len(), 1, "{sql}");
        rows.pop().unwrap()
    }

    async fn count(db: &Db, sql: &str, binds: &[Bind<'_>]) -> i64 {
        db.fetch_scalar_i64(sql, binds).await.unwrap()
    }

    async fn expect_refused(db: &Db, sql: &str, binds: &[Bind<'_>]) {
        let error = db.execute(sql, binds).await.expect_err(sql);
        let message = error.to_string().to_ascii_lowercase();
        assert!(message.contains("constraint"), "{sql} returned {message}");
    }

    async fn insert_user(db: &Db, id: &str, email: &str) {
        db.execute(
            "INSERT INTO users (id, first_name, last_name, email, bio, created_at) VALUES (?, ?, ?, ?, '', ?)",
            &[
                Bind::Text(id),
                Bind::Text("Ada"),
                Bind::Text("Lovelace"),
                Bind::Text(email),
                Bind::Text("2026-10-07T12:00:00Z"),
            ],
        )
        .await
        .unwrap();
    }

    async fn insert_open_need(db: &Db, id: &str, author: &str) {
        db.execute(
            "INSERT INTO needs (
                id, church_id, author_id, title, body, gift_id, scope, status,
                created_at, closed_at, praise, archived
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(id),
                Bind::Text("church_1"),
                Bind::Text(author),
                Bind::Text("Roof"),
                Bind::Text("It leaks."),
                Bind::OptText(None),
                Bind::Text("church"),
                Bind::Text("open"),
                Bind::Text("2026-10-07T12:00:00Z"),
                Bind::OptText(None),
                Bind::OptText(None),
                Bind::I64(0),
            ],
        )
        .await
        .unwrap();
    }

    async fn insert_reply(db: &Db, id: &str, need: &str, author: &str) {
        db.execute(
            "INSERT INTO need_replies (id, need_id, author_id, body, created_at) VALUES (?, ?, ?, ?, ?)",
            &[
                Bind::Text(id),
                Bind::Text(need),
                Bind::Text(author),
                Bind::Text("I can bring a ladder."),
                Bind::Text("2026-10-07T12:00:00Z"),
            ],
        )
        .await
        .unwrap();
    }

    async fn insert_staged(db: &Db, id: &str, owner: &str) {
        let full = format!("stage/full/{id}");
        let thumb = format!("stage/thumb/{id}");
        db.execute(
            "INSERT INTO media_assets (
                id, owner_id, state, staging_full_key, staging_thumb_key,
                width, height, full_bytes, thumb_bytes, created_at
             ) VALUES (?, ?, 'staged', ?, ?, 1600, 1200, 20000, 4000, '2026-10-07T12:00:00Z')",
            &[
                Bind::Text(id),
                Bind::Text(owner),
                Bind::Text(&full),
                Bind::Text(&thumb),
            ],
        )
        .await
        .unwrap();
    }

    async fn attach_need(
        db: &Db,
        need: &str,
        media: &str,
        position: i64,
        description: Option<&str>,
    ) {
        db.execute(
            "INSERT INTO need_media (need_id, media_id, position, description) VALUES (?, ?, ?, ?)",
            &[
                Bind::Text(need),
                Bind::Text(media),
                Bind::I64(position),
                Bind::OptText(description),
            ],
        )
        .await
        .unwrap();
    }

    async fn attach_reply(db: &Db, reply: &str, media: &str, position: i64) {
        db.execute(
            "INSERT INTO reply_media (reply_id, media_id, position, description) VALUES (?, ?, ?, NULL)",
            &[Bind::Text(reply), Bind::Text(media), Bind::I64(position)],
        )
        .await
        .unwrap();
    }
}
