use super::bind::Bind;
use super::Db;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PushSubscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PushDevice {
    pub token: String,
    pub platform: String,
}

impl Db {
    pub async fn upsert_push_subscription(
        &self,
        user_id: &str,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
    ) -> anyhow::Result<()> {
        let created = crate::clock::now_iso();
        self.execute(
            "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(endpoint) DO UPDATE SET
                p256dh = excluded.p256dh,
                auth = excluded.auth
             WHERE push_subscriptions.user_id = excluded.user_id",
            &[
                Bind::Text(user_id),
                Bind::Text(endpoint),
                Bind::Text(p256dh),
                Bind::Text(auth),
                Bind::Text(&created),
            ],
        )
        .await
    }

    pub async fn remove_push_subscription(
        &self,
        user_id: &str,
        endpoint: &str,
    ) -> anyhow::Result<()> {
        self.execute(
            "DELETE FROM push_subscriptions WHERE endpoint = ? AND user_id = ?",
            &[Bind::Text(endpoint), Bind::Text(user_id)],
        )
        .await
    }

    pub async fn forget_push_subscription(&self, endpoint: &str) -> anyhow::Result<()> {
        self.execute(
            "DELETE FROM push_subscriptions WHERE endpoint = ?",
            &[Bind::Text(endpoint)],
        )
        .await
    }

    pub async fn push_subscriptions(&self, user_id: &str) -> anyhow::Result<Vec<PushSubscription>> {
        self.fetch_all(
            "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = ?",
            &[Bind::Text(user_id)],
        )
        .await
    }

    pub async fn push_devices(&self, user_id: &str) -> anyhow::Result<Vec<PushDevice>> {
        self.fetch_all(
            "SELECT token, platform FROM push_devices WHERE user_id = ?",
            &[Bind::Text(user_id)],
        )
        .await
    }

    pub async fn upsert_push_device(
        &self,
        user_id: &str,
        token: &str,
        platform: &str,
    ) -> anyhow::Result<()> {
        let created = crate::clock::now_iso();
        self.execute(
            "INSERT INTO push_devices (user_id, token, platform, created_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(token) DO UPDATE SET platform = excluded.platform
             WHERE push_devices.user_id = excluded.user_id",
            &[
                Bind::Text(user_id),
                Bind::Text(token),
                Bind::Text(platform),
                Bind::Text(&created),
            ],
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Db;

    async fn fresh_db() -> Db {
        let path = std::env::temp_dir().join(format!(
            "ecclesia-push-{}-{}.db",
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
    async fn us_sec_07_unsubscribe_stays_with_the_owner() {
        let db = fresh_db().await;
        db.upsert_push_subscription("user_miriam", "https://push.example/m1", "abc", "def")
            .await
            .unwrap();
        db.remove_push_subscription("user_peter", "https://push.example/m1")
            .await
            .unwrap();
        assert_eq!(db.push_subscriptions("user_miriam").await.unwrap().len(), 1);
        db.remove_push_subscription("user_miriam", "https://push.example/m1")
            .await
            .unwrap();
        assert!(
            db.push_subscriptions("user_miriam")
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn us_sec_07_subscribe_does_not_steal() {
        let db = fresh_db().await;
        db.upsert_push_subscription("user_miriam", "https://push.example/m1", "abc", "def")
            .await
            .unwrap();
        db.upsert_push_subscription("user_peter", "https://push.example/m1", "zzz", "yyy")
            .await
            .unwrap();
        let mine = db.push_subscriptions("user_miriam").await.unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].p256dh, "abc");
        assert!(
            db.push_subscriptions("user_peter")
                .await
                .unwrap()
                .is_empty()
        );
        db.upsert_push_device("user_miriam", "fcm-1", "android")
            .await
            .unwrap();
        db.upsert_push_device("user_peter", "fcm-1", "ios")
            .await
            .unwrap();
        let devices = db.push_devices("user_miriam").await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].platform, "android");
        assert!(db.push_devices("user_peter").await.unwrap().is_empty());
    }
}
