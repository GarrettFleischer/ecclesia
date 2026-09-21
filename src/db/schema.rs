use super::Db;

impl Db {
    pub(crate) async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(SCHEMA).execute(&self.pool).await?;
        Ok(())
    }
}

const SCHEMA: &str = r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                city TEXT NOT NULL,
                region TEXT NOT NULL,
                bio TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            );
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
            );
            CREATE TABLE IF NOT EXISTS memberships (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(church_id, user_id)
            );
            CREATE TABLE IF NOT EXISTS gifts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                category TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS member_gifts (
                user_id TEXT NOT NULL,
                gift_id TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (user_id, gift_id)
            );
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
            );
            CREATE TABLE IF NOT EXISTS applications (
                id TEXT PRIMARY KEY,
                need_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                message TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(need_id, user_id)
            );
            CREATE TABLE IF NOT EXISTS endorsements (
                id TEXT PRIMARY KEY,
                from_user_id TEXT NOT NULL,
                to_user_id TEXT NOT NULL,
                gift_id TEXT NOT NULL,
                note TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                href TEXT NOT NULL DEFAULT '/',
                read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            "#;
