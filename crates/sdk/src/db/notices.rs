use super::bind::Bind;
use super::rows::{NotificationRow, map_all};
use super::Db;
use crate::clock::{new_id, now_iso};
use ecclesia_domain::Notification;

impl Db {
    pub async fn notify(
        &self,
        user_id: &str,
        kind: &str,
        title: &str,
        body: &str,
        href: &str,
    ) -> anyhow::Result<()> {
        let id = new_id();
        let created = now_iso();
        self.execute(
            "INSERT INTO notifications (id, user_id, kind, title, body, href, read, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
            &[
                Bind::Text(&id),
                Bind::Text(user_id),
                Bind::Text(kind),
                Bind::Text(title),
                Bind::Text(body),
                Bind::Text(href),
                Bind::Text(&created),
            ],
        )
        .await
    }

    pub async fn notifications(&self, user_id: &str) -> anyhow::Result<Vec<Notification>> {
        Ok(map_all(
            self.fetch_all::<NotificationRow>(
                "SELECT * FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 50",
                &[Bind::Text(user_id)],
            )
            .await?,
        ))
    }

    pub async fn unread_count(&self, user_id: &str) -> anyhow::Result<i64> {
        self.fetch_scalar_i64(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read = 0",
            &[Bind::Text(user_id)],
        )
        .await
    }

    pub async fn mark_notifications_read(&self, user_id: &str) -> anyhow::Result<()> {
        self.execute(
            "UPDATE notifications SET read = 1 WHERE user_id = ?",
            &[Bind::Text(user_id)],
        )
        .await
    }
}
