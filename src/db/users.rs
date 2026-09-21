use super::Db;
use crate::leaf::User;

impl Db {
    pub async fn user(&self, id: &str) -> anyhow::Result<Option<User>> {
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE lower(email) = lower(?)")
                .bind(email)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn demo_users(&self) -> anyhow::Result<Vec<User>> {
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE id LIKE 'user_%' ORDER BY name")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn update_user(
        &self,
        id: &str,
        name: &str,
        city: &str,
        region: &str,
        bio: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET name = ?, city = ?, region = ?, bio = ? WHERE id = ?")
            .bind(name.trim())
            .bind(city.trim())
            .bind(region.trim())
            .bind(bio.trim())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
