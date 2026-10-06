use super::Db;
use super::bind::Bind;
use super::rows::{MembershipRow, UserRow};
use ecclesia_domain::{Membership, User};

impl Db {
    pub async fn user(&self, id: &str) -> anyhow::Result<Option<User>> {
        let Some(user) = self
            .fetch_optional::<UserRow>("SELECT * FROM users WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(User::from)
        else {
            return Ok(None);
        };
        Ok(Some(self.with_memberships(user).await?))
    }

    pub async fn user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        let Some(user) = self
            .fetch_optional::<UserRow>(
                "SELECT * FROM users WHERE lower(email) = lower(?)",
                &[Bind::Text(email)],
            )
            .await?
            .map(User::from)
        else {
            return Ok(None);
        };
        Ok(Some(self.with_memberships(user).await?))
    }

    async fn with_memberships(&self, mut user: User) -> anyhow::Result<User> {
        user.memberships = self.memberships_for(&user.id).await?;
        Ok(user)
    }

    pub async fn memberships_for(&self, user_id: &str) -> anyhow::Result<Vec<Membership>> {
        let rows = self
            .fetch_all::<MembershipRow>(
                "SELECT church_id, role, status FROM memberships_live WHERE user_id = ? ORDER BY church_id",
                &[Bind::Text(user_id)],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| Membership {
                church_id: row.church_id,
                role: row.role,
                status: row.status,
            })
            .collect())
    }
}
