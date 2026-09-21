use sqlx::Row;

use super::Db;
use crate::leaf::{Church, ChurchMember, Membership, User};

impl Db {
    pub async fn churches(&self) -> anyhow::Result<Vec<Church>> {
        Ok(
            sqlx::query_as::<_, Church>("SELECT * FROM churches ORDER BY city, name")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn church(&self, id: &str) -> anyhow::Result<Option<Church>> {
        Ok(
            sqlx::query_as::<_, Church>("SELECT * FROM churches WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn church_by_invite(&self, code: &str) -> anyhow::Result<Option<Church>> {
        Ok(sqlx::query_as::<_, Church>(
            "SELECT * FROM churches WHERE lower(invite_code) = lower(?)",
        )
        .bind(code.trim())
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn memberships_for_user(&self, user_id: &str) -> anyhow::Result<Vec<Membership>> {
        Ok(sqlx::query_as::<_, Membership>(
            "SELECT * FROM memberships WHERE user_id = ? ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn churches_for_user(&self, user_id: &str) -> anyhow::Result<Vec<Church>> {
        Ok(sqlx::query_as::<_, Church>(
            r#"
            SELECT c.* FROM churches c
            JOIN memberships m ON m.church_id = c.id
            WHERE m.user_id = ?
            ORDER BY c.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn membership(&self, id: &str) -> anyhow::Result<Option<Membership>> {
        Ok(
            sqlx::query_as::<_, Membership>("SELECT * FROM memberships WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn membership_pair(
        &self,
        church_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<Membership>> {
        Ok(sqlx::query_as::<_, Membership>(
            "SELECT * FROM memberships WHERE church_id = ? AND user_id = ?",
        )
        .bind(church_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn set_membership_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE memberships SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn church_members(&self, church_id: &str) -> anyhow::Result<Vec<ChurchMember>> {
        Ok(sqlx::query_as::<_, ChurchMember>(
            r#"
            SELECT m.id AS membership_id, u.id AS user_id, u.name, u.city, m.role, m.status
            FROM memberships m
            JOIN users u ON u.id = m.user_id
            WHERE m.church_id = ?
            ORDER BY
                CASE m.status WHEN 'pending_request' THEN 0 WHEN 'pending_invite' THEN 1 WHEN 'active' THEN 2 ELSE 3 END,
                CASE m.role WHEN 'owner' THEN 0 WHEN 'steward' THEN 1 ELSE 2 END,
                u.name
            "#,
        )
        .bind(church_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn governors(&self, church_id: &str) -> anyhow::Result<Vec<User>> {
        Ok(sqlx::query_as::<_, User>(
            r#"
            SELECT u.* FROM users u
            JOIN memberships m ON m.user_id = u.id
            WHERE m.church_id = ? AND m.status = 'active' AND m.role IN ('owner', 'steward')
            "#,
        )
        .bind(church_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn counts_for_churches(&self) -> anyhow::Result<Vec<(String, i64, i64)>> {
        let rows = sqlx::query(
            r#"
            SELECT c.id,
                   (SELECT COUNT(*) FROM memberships m WHERE m.church_id = c.id AND m.status = 'active') AS members,
                   (SELECT COUNT(*) FROM needs n WHERE n.church_id = c.id AND n.status = 'open') AS needs
            FROM churches c
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(count_rows(rows))
    }
}

fn count_rows(rows: Vec<sqlx::sqlite::SqliteRow>) -> Vec<(String, i64, i64)> {
    rows.into_iter().map(count_row).collect()
}

fn count_row(row: sqlx::sqlite::SqliteRow) -> (String, i64, i64) {
    (
        row.get::<String, _>("id"),
        row.get::<i64, _>("members"),
        row.get::<i64, _>("needs"),
    )
}
