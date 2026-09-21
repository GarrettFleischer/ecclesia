use super::Db;
use crate::domain::{new_id, now_iso};
use crate::leaf::Notification;

impl Db {
    pub async fn notify(
        &self,
        user_id: &str,
        kind: &str,
        title: &str,
        body: &str,
        href: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO notifications (id, user_id, kind, title, body, href, read, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(new_id())
        .bind(user_id)
        .bind(kind)
        .bind(title)
        .bind(body)
        .bind(href)
        .bind(now_iso())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn notifications(&self, user_id: &str) -> anyhow::Result<Vec<Notification>> {
        Ok(sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 50",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn unread_count(&self, user_id: &str) -> anyhow::Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read = 0")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn mark_notifications_read(&self, user_id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE notifications SET read = 1 WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
