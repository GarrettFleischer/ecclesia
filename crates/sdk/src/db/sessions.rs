use super::bind::Bind;
use super::extras::{SessionRow, TokenRow};
use super::Db;

impl Db {
    pub async fn session(&self, id: &str) -> anyhow::Result<Option<SessionRow>> {
        self.fetch_optional::<SessionRow>("SELECT * FROM sessions WHERE id = ?", &[Bind::Text(id)])
            .await
    }

    pub async fn sessions_for_user(&self, user_id: &str) -> anyhow::Result<Vec<SessionRow>> {
        self.fetch_all::<SessionRow>(
            "SELECT * FROM sessions WHERE user_id = ? ORDER BY last_seen_at DESC",
            &[Bind::Text(user_id)],
        )
        .await
    }

    pub async fn touch_session(&self, id: &str, last_seen_at: &str) -> anyhow::Result<()> {
        self.execute(
            "UPDATE sessions SET last_seen_at = ? WHERE id = ?",
            &[Bind::Text(last_seen_at), Bind::Text(id)],
        )
        .await
    }

    pub async fn user_password_hash(&self, user_id: &str) -> anyhow::Result<Option<String>> {
        #[derive(sqlx::FromRow)]
        struct HashRow {
            password_hash: Option<String>,
        }
        Ok(self
            .fetch_optional::<HashRow>(
                "SELECT password_hash FROM users WHERE id = ?",
                &[Bind::Text(user_id)],
            )
            .await?
            .and_then(|row| row.password_hash)
            .filter(|hash| !hash.is_empty()))
    }

    pub async fn live_magic(&self, id: &str) -> anyhow::Result<Option<TokenRow>> {
        self.fetch_optional::<TokenRow>(
            "SELECT * FROM magic_links WHERE id = ?",
            &[Bind::Text(id)],
        )
        .await
    }

    pub async fn live_reset(&self, id: &str) -> anyhow::Result<Option<TokenRow>> {
        self.fetch_optional::<TokenRow>(
            "SELECT * FROM password_resets WHERE id = ?",
            &[Bind::Text(id)],
        )
        .await
    }
}
