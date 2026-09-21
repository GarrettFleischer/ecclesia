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
        sqlx::query(
            "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(endpoint) DO UPDATE SET user_id = excluded.user_id, p256dh = excluded.p256dh, auth = excluded.auth",
        )
        .bind(user_id)
        .bind(endpoint)
        .bind(p256dh)
        .bind(auth)
        .bind(crate::sdk::clock::now_iso())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_push_subscription(&self, endpoint: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ?")
            .bind(endpoint)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn push_subscriptions(&self, user_id: &str) -> anyhow::Result<Vec<PushSubscription>> {
        Ok(sqlx::query_as::<_, PushSubscription>(
            "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn push_devices(&self, user_id: &str) -> anyhow::Result<Vec<PushDevice>> {
        Ok(sqlx::query_as::<_, PushDevice>(
            "SELECT token, platform FROM push_devices WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn upsert_push_device(
        &self,
        user_id: &str,
        token: &str,
        platform: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO push_devices (user_id, token, platform, created_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(token) DO UPDATE SET user_id = excluded.user_id, platform = excluded.platform",
        )
        .bind(user_id)
        .bind(token)
        .bind(platform)
        .bind(crate::sdk::clock::now_iso())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
