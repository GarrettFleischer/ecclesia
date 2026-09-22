use super::bind::{placeholders, Bind};
use super::rows::{ChurchMemberRow, ChurchRow, CountRow, MembershipRow, map_all};
use super::Db;
use ecclesia_domain::{Church, ChurchMember, Membership};

impl Db {
    pub async fn churches_page(
        &self,
        after: Option<(&str, &str, &str)>,
    ) -> anyhow::Result<Vec<Church>> {
        match after {
            None => Ok(map_all(
                self.fetch_all::<ChurchRow>(
                    "SELECT * FROM churches ORDER BY city, name, id LIMIT 20",
                    &[],
                )
                .await?,
            )),
            Some((city, name, id)) => Ok(map_all(
                self.fetch_all::<ChurchRow>(
                    "SELECT * FROM churches
                     WHERE (city, name, id) > (?, ?, ?)
                     ORDER BY city, name, id
                     LIMIT 20",
                    &[Bind::Text(city), Bind::Text(name), Bind::Text(id)],
                )
                .await?,
            )),
        }
    }

    pub async fn church(&self, id: &str) -> anyhow::Result<Option<Church>> {
        Ok(self
            .fetch_optional::<ChurchRow>("SELECT * FROM churches WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(Church::from))
    }

    pub async fn church_by_invite(&self, code: &str) -> anyhow::Result<Option<Church>> {
        Ok(self
            .fetch_optional::<ChurchRow>(
                "SELECT * FROM churches WHERE lower(invite_code) = lower(?)",
                &[Bind::Text(code.trim())],
            )
            .await?
            .map(Church::from))
    }

    pub async fn memberships_for_user(&self, user_id: &str) -> anyhow::Result<Vec<Membership>> {
        Ok(map_all(
            self.fetch_all::<MembershipRow>(
                "SELECT * FROM memberships WHERE user_id = ? ORDER BY created_at",
                &[Bind::Text(user_id)],
            )
            .await?,
        ))
    }

    pub async fn churches_for_user(&self, user_id: &str) -> anyhow::Result<Vec<Church>> {
        Ok(map_all(
            self.fetch_all::<ChurchRow>(
                r#"
            SELECT c.* FROM churches c
            JOIN memberships m ON m.church_id = c.id
            WHERE m.user_id = ?
            ORDER BY c.name
            "#,
                &[Bind::Text(user_id)],
            )
            .await?,
        ))
    }

    pub async fn membership(&self, id: &str) -> anyhow::Result<Option<Membership>> {
        Ok(self
            .fetch_optional::<MembershipRow>("SELECT * FROM memberships WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(Membership::from))
    }

    pub async fn membership_pair(
        &self,
        church_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<Membership>> {
        Ok(self
            .fetch_optional::<MembershipRow>(
                "SELECT * FROM memberships WHERE church_id = ? AND user_id = ?",
                &[Bind::Text(church_id), Bind::Text(user_id)],
            )
            .await?
            .map(Membership::from))
    }

    pub async fn churches_with_ids(&self, ids: &[&str]) -> anyhow::Result<Vec<Church>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT * FROM churches WHERE id IN ({})",
            placeholders(ids.len())
        );
        let binds: Vec<Bind<'_>> = ids.iter().map(|id| Bind::Text(*id)).collect();
        Ok(map_all(self.fetch_all::<ChurchRow>(&sql, &binds).await?))
    }

    pub async fn church_members_page(
        &self,
        church_id: &str,
        after: Option<(&str, &str)>,
    ) -> anyhow::Result<Vec<ChurchMember>> {
        match after {
            None => Ok(map_all(
                self.fetch_all::<ChurchMemberRow>(
                    r#"
            SELECT m.id AS membership_id, u.id AS user_id, u.name, u.city, m.role, m.status
            FROM memberships m
            JOIN users u ON u.id = m.user_id
            WHERE m.church_id = ?
            ORDER BY u.name, u.id
            LIMIT 20
            "#,
                    &[Bind::Text(church_id)],
                )
                .await?,
            )),
            Some((name, id)) => Ok(map_all(
                self.fetch_all::<ChurchMemberRow>(
                    r#"
            SELECT m.id AS membership_id, u.id AS user_id, u.name, u.city, m.role, m.status
            FROM memberships m
            JOIN users u ON u.id = m.user_id
            WHERE m.church_id = ?
              AND (u.name, u.id) > (?, ?)
            ORDER BY u.name, u.id
            LIMIT 20
            "#,
                    &[Bind::Text(church_id), Bind::Text(name), Bind::Text(id)],
                )
                .await?,
            )),
        }
    }

    pub async fn church_members(&self, church_id: &str) -> anyhow::Result<Vec<ChurchMember>> {
        self.church_members_page(church_id, None).await
    }

    pub async fn governor_ids(&self, church_id: &str) -> anyhow::Result<Vec<String>> {
        self.fetch_strings(
            r#"
            SELECT u.id FROM users u
            JOIN memberships m ON m.user_id = u.id
            WHERE m.church_id = ? AND m.status = 'active' AND m.role IN ('owner', 'steward')
            "#,
            &[Bind::Text(church_id)],
        )
        .await
    }

    pub async fn counts_for_church_ids(
        &self,
        ids: &[&str],
    ) -> anyhow::Result<Vec<(String, i64, i64)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            r#"
            SELECT c.id,
                   (SELECT COUNT(*) FROM memberships m WHERE m.church_id = c.id AND m.status = 'active') AS members,
                   (SELECT COUNT(*) FROM needs n WHERE n.church_id = c.id AND n.status = 'open') AS needs
            FROM churches c
            WHERE c.id IN ({})
            "#,
            placeholders(ids.len())
        );
        let binds: Vec<Bind<'_>> = ids.iter().map(|id| Bind::Text(*id)).collect();
        let rows = self.fetch_all::<CountRow>(&sql, &binds).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.id, row.members, row.needs))
            .collect())
    }

    pub async fn counts_for_one_church(&self, church_id: &str) -> anyhow::Result<(i64, i64)> {
        let rows = self.counts_for_church_ids(&[church_id]).await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|(_, members, needs)| (members, needs))
            .unwrap_or((0, 0)))
    }
}
