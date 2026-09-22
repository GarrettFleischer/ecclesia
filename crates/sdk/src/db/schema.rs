use super::bind::Bind;
use super::Db;

impl Db {
    pub(crate) async fn migrate(&self) -> anyhow::Result<()> {
        for statement in SCHEMA {
            self.execute(statement, &[]).await?;
        }
        add_endorsement_skill_column(self).await?;
        backfill_endorsement_skills(self).await?;
        add_password_hash_column(self).await?;
        add_auth_tables(self).await?;
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
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                city TEXT NOT NULL,
                region TEXT NOT NULL,
                bio TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS churches (
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
            )
            "#,
    r#"
            CREATE TABLE IF NOT EXISTS memberships (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(church_id, user_id)
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
                created_at TEXT NOT NULL
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
                created_at TEXT NOT NULL
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
    "CREATE INDEX IF NOT EXISTS idx_churches_region_city ON churches (region, city)",
    "CREATE INDEX IF NOT EXISTS idx_churches_city_name_id ON churches (city, name, id)",
    "CREATE INDEX IF NOT EXISTS idx_memberships_user ON memberships (user_id)",
    "CREATE INDEX IF NOT EXISTS idx_memberships_church_status ON memberships (church_id, status)",
    "CREATE INDEX IF NOT EXISTS idx_notifications_user_created ON notifications (user_id, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_outbox_available ON outbox (available_at) WHERE status != 'done'",
];

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

async fn add_auth_tables(db: &Db) -> anyhow::Result<()> {
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                csrf TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                user_agent TEXT NOT NULL DEFAULT '',
                ip TEXT NOT NULL DEFAULT ''
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
