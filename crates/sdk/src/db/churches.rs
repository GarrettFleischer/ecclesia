use super::bind::{placeholders, Bind};
use super::distance::haversine_km_sql;
use super::rows::{ChurchMemberRow, ChurchRow, CountRow, map_all};
use super::Db;
use ecclesia_domain::{Church, ChurchMember};

impl Db {
    pub async fn churches_for_lookup(&self) -> anyhow::Result<Vec<Church>> {
        Ok(map_all(
            self.fetch_all::<ChurchRow>("SELECT * FROM churches ORDER BY name, id", &[])
                .await?,
        ))
    }

    pub async fn churches_page(
        &self,
        after: Option<(&str, &str)>,
    ) -> anyhow::Result<Vec<Church>> {
        match after {
            None => Ok(map_all(
                self.fetch_all::<ChurchRow>(
                    "SELECT * FROM churches ORDER BY name, id LIMIT 20",
                    &[],
                )
                .await?,
            )),
            Some((name, id)) => Ok(map_all(
                self.fetch_all::<ChurchRow>(
                    "SELECT * FROM churches
                     WHERE (name, id) > (?, ?)
                     ORDER BY name, id
                     LIMIT 20",
                    &[Bind::Text(name), Bind::Text(id)],
                )
                .await?,
            )),
        }
    }

    pub async fn nearest_church(
        &self,
        latitude: f64,
        longitude: f64,
        except_id: Option<&str>,
    ) -> anyhow::Result<Option<Church>> {
        let distance = haversine_km_sql("?", "?", "latitude", "longitude");
        let sql = format!(
            "SELECT * FROM churches WHERE id != ? ORDER BY {distance} LIMIT 1"
        );
        Ok(self
            .fetch_optional::<ChurchRow>(
                &sql,
                &[
                    Bind::Text(except_id.unwrap_or("")),
                    Bind::F64(latitude),
                    Bind::F64(longitude),
                ],
            )
            .await?
            .map(Church::from))
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
        after: Option<(&str, &str, &str)>,
    ) -> anyhow::Result<Vec<ChurchMember>> {
        match after {
            None => Ok(map_all(
                self.fetch_all::<ChurchMemberRow>(
                    r#"
            SELECT u.id AS user_id, u.first_name, u.last_name, u.church_role AS role, u.church_status AS status
            FROM users u
            WHERE u.church_id = ?
            ORDER BY u.first_name, u.last_name, u.id
            LIMIT 20
            "#,
                    &[Bind::Text(church_id)],
                )
                .await?,
            )),
            Some((first, last, id)) => Ok(map_all(
                self.fetch_all::<ChurchMemberRow>(
                    r#"
            SELECT u.id AS user_id, u.first_name, u.last_name, u.church_role AS role, u.church_status AS status
            FROM users u
            WHERE u.church_id = ?
              AND (u.first_name, u.last_name, u.id) > (?, ?, ?)
            ORDER BY u.first_name, u.last_name, u.id
            LIMIT 20
            "#,
                    &[
                        Bind::Text(church_id),
                        Bind::Text(first),
                        Bind::Text(last),
                        Bind::Text(id),
                    ],
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
            SELECT id FROM users
            WHERE church_id = ? AND church_status = 'active' AND church_role IN ('owner', 'steward')
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
                   (SELECT COUNT(*) FROM users u WHERE u.church_id = c.id AND u.church_status = 'active') AS members,
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
