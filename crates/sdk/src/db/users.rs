use super::bind::Bind;
use super::rows::UserRow;
use super::Db;
use ecclesia_domain::User;

impl Db {
    pub async fn user(&self, id: &str) -> anyhow::Result<Option<User>> {
        Ok(self
            .fetch_optional::<UserRow>("SELECT * FROM users WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(User::from))
    }

    pub async fn user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        Ok(self
            .fetch_optional::<UserRow>(
                "SELECT * FROM users WHERE lower(email) = lower(?)",
                &[Bind::Text(email)],
            )
            .await?
            .map(User::from))
    }

    pub async fn update_user(
        &self,
        id: &str,
        name: &str,
        city: &str,
        region: &str,
        bio: &str,
    ) -> anyhow::Result<()> {
        self.execute(
            "UPDATE users SET name = ?, city = ?, region = ?, bio = ? WHERE id = ?",
            &[
                Bind::Text(name.trim()),
                Bind::Text(city.trim()),
                Bind::Text(region.trim()),
                Bind::Text(bio.trim()),
                Bind::Text(id),
            ],
        )
        .await
    }
}
