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
        copy_offers_into_replies(self).await?;
        add_church_registration(self).await?;
        add_need_archive(self).await?;
        add_need_praise(self).await?;
        install_soft_delete(self).await?;
        self.archive_met_needs().await?;
        self.ensure_need_shares().await?;
        Ok(())
    }
}

async fn copy_offers_into_replies(db: &Db) -> anyhow::Result<()> {
    db.execute(
        "INSERT INTO need_replies (id, need_id, author_id, body, created_at)
         SELECT a.id, a.need_id, a.user_id, a.message, a.created_at
         FROM applications a
         WHERE NOT EXISTS (SELECT 1 FROM need_replies r WHERE r.id = a.id)",
        &[] as &[Bind<'_>],
    )
    .await?;
    Ok(())
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
                created_at TEXT NOT NULL
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
    }
}
