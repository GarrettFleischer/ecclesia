//! Claim and finish outbox rows. The worker talks to phones.

use chrono::{Duration, SecondsFormat, TimeZone, Utc};

use super::bind::Bind;
use super::dialect::Driver;
use super::Db;
use crate::clock::now_iso;

const MAX_ATTEMPTS: i64 = 8;
const LEASE_MINUTES: i64 = 15;
const BACKOFF_SECS: [i64; 7] = [10, 30, 120, 600, 1800, 7200, 21600];

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OutboxRow {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub attempts: i64,
    pub available_at: String,
    pub status: String,
    pub dead_at: Option<String>,
}

pub enum OutboxFinish {
    Success,
    Failed,
}

impl Db {
    pub async fn claim_outbox(&self, now: &str) -> anyhow::Result<Vec<OutboxRow>> {
        self.reclaim_stale(now).await?;
        match self.driver() {
            Driver::Sqlite => claim_sqlite(self, now).await,
            Driver::Postgres => claim_postgres(self, now).await,
        }
    }

    pub async fn finish_outbox(&self, id: &str, finish: OutboxFinish) -> anyhow::Result<()> {
        match finish {
            OutboxFinish::Success => {
                self.execute(
                    "UPDATE outbox SET status = 'done' WHERE id = ?",
                    &[Bind::Text(id)],
                )
                .await
            }
            OutboxFinish::Failed => fail_outbox(self, id).await,
        }
    }

    pub async fn outbox_lag_seconds(&self, now: &str) -> anyhow::Result<Option<i64>> {
        let oldest: Option<String> = self
            .fetch_optional::<OldestRow>(
                "SELECT MIN(available_at) AS oldest FROM outbox WHERE status IN ('pending', 'working')",
                &[],
            )
            .await?
            .and_then(|row| row.oldest);
        let Some(oldest) = oldest else {
            return Ok(None);
        };
        Ok(age_seconds(&oldest, now))
    }

    pub async fn outbox_row(&self, id: &str) -> anyhow::Result<Option<OutboxRow>> {
        self.fetch_optional("SELECT * FROM outbox WHERE id = ?", &[Bind::Text(id)])
            .await
    }

    pub async fn pending_outbox(&self) -> anyhow::Result<Vec<OutboxRow>> {
        self.fetch_all(
            "SELECT * FROM outbox WHERE status = 'pending' ORDER BY available_at",
            &[],
        )
        .await
    }

    async fn reclaim_stale(&self, now: &str) -> anyhow::Result<()> {
        let cutoff = shift_iso(now, -LEASE_MINUTES * 60);
        self.execute(
            "UPDATE outbox SET status = 'pending' WHERE status = 'working' AND available_at <= ?",
            &[Bind::Text(&cutoff)],
        )
        .await
    }
}

#[derive(sqlx::FromRow)]
struct OldestRow {
    oldest: Option<String>,
}

async fn claim_sqlite(db: &Db, now: &str) -> anyhow::Result<Vec<OutboxRow>> {
    let super::Inner::Sqlite(pool) = &db.inner else {
        anyhow::bail!("sqlite claim on a postgres store");
    };
    let mut conn = pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await?;
    let claimed = async {
        let rows = sqlx::query_as::<sqlx::Sqlite, OutboxRow>(
            "SELECT * FROM outbox
             WHERE status = 'pending' AND available_at <= ?
             ORDER BY available_at
             LIMIT 10",
        )
        .bind(now)
        .fetch_all(&mut *conn)
        .await?;
        for row in &rows {
            sqlx::query("UPDATE outbox SET status = 'working', available_at = ? WHERE id = ?")
                .bind(now)
                .bind(&row.id)
                .execute(&mut *conn)
                .await?;
        }
        Ok::<_, anyhow::Error>(rows)
    }
    .await;
    match claimed {
        Ok(rows) => {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            Ok(rows)
        }
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(error)
        }
    }
}

async fn claim_postgres(db: &Db, now: &str) -> anyhow::Result<Vec<OutboxRow>> {
    db.execute(
        "UPDATE outbox SET status = 'working', available_at = ?
         WHERE id IN (
            SELECT id FROM outbox
            WHERE status = 'pending' AND available_at <= ?
            ORDER BY available_at
            LIMIT 10
            FOR UPDATE SKIP LOCKED
         )",
        &[Bind::Text(now), Bind::Text(now)],
    )
    .await?;
    db.fetch_all(
        "SELECT * FROM outbox WHERE status = 'working' AND available_at = ? ORDER BY id",
        &[Bind::Text(now)],
    )
    .await
}

async fn fail_outbox(db: &Db, id: &str) -> anyhow::Result<()> {
    let Some(row) = db.outbox_row(id).await? else {
        return Ok(());
    };
    let attempts = row.attempts + 1;
    if attempts >= MAX_ATTEMPTS {
        let dead_at = now_iso();
        return db
            .execute(
                "UPDATE outbox SET status = 'dead', attempts = ?, dead_at = ? WHERE id = ?",
                &[
                    Bind::I64(attempts),
                    Bind::Text(&dead_at),
                    Bind::Text(id),
                ],
            )
            .await;
    }
    let delay = BACKOFF_SECS[(attempts as usize - 1).min(BACKOFF_SECS.len() - 1)];
    let available = shift_iso(&now_iso(), delay);
    db.execute(
        "UPDATE outbox SET status = 'pending', attempts = ?, available_at = ? WHERE id = ?",
        &[
            Bind::I64(attempts),
            Bind::Text(&available),
            Bind::Text(id),
        ],
    )
    .await
}

fn shift_iso(now: &str, seconds: i64) -> String {
    match parse_iso(now) {
        Some(when) => (when + Duration::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Secs, true),
        None => now.to_string(),
    }
}

fn age_seconds(oldest: &str, now: &str) -> Option<i64> {
    let oldest = parse_iso(oldest)?;
    let now = parse_iso(now)?;
    Some(now.signed_duration_since(oldest).num_seconds())
}

fn parse_iso(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|when| when.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%SZ")
                .ok()
                .map(|naive| Utc.from_utc_datetime(&naive))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    async fn fresh_db() -> Db {
        let path = std::env::temp_dir().join(format!(
            "ecclesia-outbox-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Db::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("test database")
    }

    #[tokio::test]
    async fn us_outbox_02_apply_writes_a_pending_push_row() {
        let db = fresh_db().await;
        let effect = ecclesia_domain::Effect {
            writes: Vec::new(),
            notices: vec![ecclesia_domain::NoticeDraft {
                user_id: "user_miriam".into(),
                kind: "join_request",
                title: std::sync::Arc::from("Hello"),
                body: "A notice",
                href: "/home".into(),
            }],
        };
        db.apply(&effect).await.unwrap();
        let pending = db.pending_outbox().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, "push");
        assert_eq!(pending[0].status, "pending");
    }

    #[tokio::test]
    async fn us_outbox_05_eight_failures_mark_the_row_dead() {
        let db = fresh_db().await;
        let now = now_iso();
        db.execute(
            "INSERT INTO outbox (id, kind, payload, attempts, available_at, status, dead_at)
             VALUES ('ob1', 'push', '{}', 0, ?, 'pending', NULL)",
            &[Bind::Text(&now)],
        )
        .await
        .unwrap();
        let later = "9999-12-31T00:00:00Z";
        for _ in 0..8 {
            let claimed = db.claim_outbox(later).await.unwrap();
            assert_eq!(claimed.len(), 1);
            db.finish_outbox(&claimed[0].id, OutboxFinish::Failed)
                .await
                .unwrap();
        }
        let row = db.outbox_row("ob1").await.unwrap().expect("row");
        assert_eq!(row.status, "dead");
        assert!(row.dead_at.is_some());
        assert!(db.claim_outbox(later).await.unwrap().is_empty());
    }
}
