//! One query text for SQLite and Postgres.

use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    Sqlite,
    Postgres,
}

impl Driver {
    pub fn from_url(url: &str) -> anyhow::Result<Self> {
        let scheme = url.split(':').next().unwrap_or("");
        match scheme {
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "sqlite" => Ok(Self::Sqlite),
            other => anyhow::bail!("unsupported DATABASE_URL scheme: {other}"),
        }
    }

    pub fn is_postgres(self) -> bool {
        matches!(self, Self::Postgres)
    }

    pub fn sql(self, text: &str) -> Cow<'_, str> {
        match self {
            Self::Sqlite => Cow::Borrowed(text),
            Self::Postgres => Cow::Owned(rewrite_placeholders(text)),
        }
    }

    pub fn pool_size(self) -> u32 {
        match self {
            Self::Sqlite => 8,
            Self::Postgres => 16,
        }
    }
}

pub fn rewrite_placeholders(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = 0u32;
    let mut chars = sql.chars().peekable();
    let mut in_single = false;
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            out.push(ch);
            if in_single {
                if chars.peek() == Some(&'\'') {
                    out.push(chars.next().expect("escaped quote"));
                } else {
                    in_single = false;
                }
            } else {
                in_single = true;
            }
            continue;
        }
        if ch == '?' && !in_single {
            n += 1;
            out.push('$');
            out.push_str(&n.to_string());
            continue;
        }
        out.push(ch);
    }
    out
}

pub fn require_public_database_url(url: Option<&str>) -> anyhow::Result<&str> {
    let url = url.ok_or_else(|| anyhow::anyhow!("DATABASE_URL is required on a public host"))?;
    match Driver::from_url(url) {
        Ok(Driver::Postgres) => Ok(url),
        Ok(Driver::Sqlite) => anyhow::bail!("public host needs a postgres DATABASE_URL"),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_store_01_rewrites_question_marks_for_postgres() {
        let sqlite = "SELECT * FROM needs WHERE id = ? AND church_id = ?";
        assert_eq!(
            Driver::Postgres.sql(sqlite).as_ref(),
            "SELECT * FROM needs WHERE id = $1 AND church_id = $2"
        );
        assert_eq!(Driver::Sqlite.sql(sqlite).as_ref(), sqlite);
        assert_eq!(
            rewrite_placeholders("SELECT '?' FROM needs WHERE id = ?"),
            "SELECT '?' FROM needs WHERE id = $1"
        );
    }

    #[test]
    fn us_store_02_public_host_refuses_a_missing_or_sqlite_url() {
        assert!(require_public_database_url(None).is_err());
        assert!(require_public_database_url(Some("sqlite://ecclesia.db")).is_err());
        assert_eq!(
            require_public_database_url(Some("postgres://neon.example/ecclesia")).unwrap(),
            "postgres://neon.example/ecclesia"
        );
        assert_eq!(
            require_public_database_url(Some("postgresql://neon.example/ecclesia")).unwrap(),
            "postgresql://neon.example/ecclesia"
        );
    }
}
