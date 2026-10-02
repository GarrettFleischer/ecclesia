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

pub const LOCAL_SQLITE_DEFAULT: &str = "sqlite://ecclesia.db";

/// On a non-public host, pick a store URL that cannot accidentally target production.
pub fn local_database_url(env_url: Option<String>) -> String {
    if allow_remote_database_url() {
        return env_url.unwrap_or_else(|| LOCAL_SQLITE_DEFAULT.into());
    }
    let Some(url) = env_url.filter(|value| !value.is_empty()) else {
        return LOCAL_SQLITE_DEFAULT.into();
    };
    if is_local_database_url(&url) {
        return url;
    }
    tracing::warn!(
        ignored = %redact_database_url(&url),
        fallback = LOCAL_SQLITE_DEFAULT,
        "Ignoring remote DATABASE_URL on a local host"
    );
    LOCAL_SQLITE_DEFAULT.into()
}

pub fn allow_remote_database_url() -> bool {
    std::env::var("ECCLESIA_ALLOW_REMOTE_DATABASE")
        .ok()
        .as_deref()
        == Some("1")
}

pub fn is_local_database_url(url: &str) -> bool {
    match Driver::from_url(url) {
        Ok(Driver::Sqlite) => true,
        Ok(Driver::Postgres) => postgres_host_is_local(url),
        Err(_) => false,
    }
}

fn postgres_host_is_local(url: &str) -> bool {
    match postgres_authority_host(url) {
        Some(host) => matches!(
            host,
            "localhost" | "127.0.0.1" | "::1" | "host.docker.internal"
        ),
        None => false,
    }
}

fn postgres_authority_host(url: &str) -> Option<&str> {
    let rest = url.split("://").nth(1)?;
    let authority = rest.split('/').next()?;
    let host_port = authority.split('@').last()?;
    host_port.split(':').next()
}

fn redact_database_url(url: &str) -> String {
    let Some(rest) = url.split("://").nth(1) else {
        return "<invalid>".into();
    };
    let scheme = url.split("://").next().unwrap_or("?");
    if let Some((_, after)) = rest.split_once('@') {
        return format!("{scheme}://***@{after}");
    }
    format!("{scheme}://{rest}")
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
    fn us_store_03_local_host_ignores_remote_postgres_url() {
        assert!(is_local_database_url("sqlite://ecclesia.db"));
        assert!(is_local_database_url("postgres://ecclesia:ecclesia@127.0.0.1:5432/ecclesia"));
        assert!(!is_local_database_url(
            "postgres://user:pass@ep-cool-pooler.us-east-1.aws.neon.tech/neondb?sslmode=require"
        ));
        let picked = local_database_url(Some(
            "postgres://user:pass@ep-cool-pooler.us-east-1.aws.neon.tech/neondb".into(),
        ));
        assert_eq!(picked, LOCAL_SQLITE_DEFAULT);
        let kept = local_database_url(Some(
            "postgres://ecclesia:ecclesia@localhost:5432/ecclesia".into(),
        ));
        assert_eq!(kept, "postgres://ecclesia:ecclesia@localhost:5432/ecclesia");
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
