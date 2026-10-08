use super::Db;
use super::bind::{Bind, placeholders};
use super::distance::haversine_km_sql;
use super::rows::{ChurchMemberRow, ChurchRow, CountRow, map_all};
use ecclesia_domain::{Church, ChurchMember, MemberRelease, User, nearby_km};

const NEARBY_CHURCH_LIMIT: usize = 20;

impl Db {
    pub async fn churches_for_lookup(&self) -> anyhow::Result<Vec<Church>> {
        Ok(map_all(
            self.fetch_all::<ChurchRow>("SELECT * FROM churches_live ORDER BY name, id", &[])
                .await?,
        ))
    }

    pub async fn churches_page(&self, after: Option<(&str, &str)>) -> anyhow::Result<Vec<Church>> {
        match after {
            None => Ok(map_all(
                self.fetch_all::<ChurchRow>(
                    "SELECT * FROM churches_live ORDER BY name, id LIMIT 20",
                    &[],
                )
                .await?,
            )),
            Some((name, id)) => Ok(map_all(
                self.fetch_all::<ChurchRow>(
                    "SELECT * FROM churches_live
                     WHERE (name, id) > (?, ?)
                     ORDER BY name, id
                     LIMIT 20",
                    &[Bind::Text(name), Bind::Text(id)],
                )
                .await?,
            )),
        }
    }

    pub async fn churches_near(
        &self,
        latitude: f64,
        longitude: f64,
        except_id: Option<&str>,
    ) -> anyhow::Result<Vec<Church>> {
        if !latitude.is_finite() || !longitude.is_finite() {
            anyhow::bail!("coordinates are not finite");
        }
        let distance = haversine_km_sql(
            &sql_number(latitude),
            &sql_number(longitude),
            "latitude",
            "longitude",
        );
        let sql = format!(
            "SELECT * FROM churches_live WHERE id != ? AND {distance} <= {cutoff} ORDER BY {distance}, name, id LIMIT {NEARBY_CHURCH_LIMIT}",
            cutoff = nearby_km(),
        );
        Ok(map_all(
            self.fetch_all::<ChurchRow>(&sql, &[Bind::Text(except_id.unwrap_or(""))])
                .await?,
        ))
    }

    pub async fn church(&self, id: &str) -> anyhow::Result<Option<Church>> {
        Ok(self
            .fetch_optional::<ChurchRow>(
                "SELECT * FROM churches_live WHERE id = ?",
                &[Bind::Text(id)],
            )
            .await?
            .map(Church::from))
    }

    /// One church row, including a closed church.
    pub async fn stored_church(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<(Church, Option<String>)>> {
        Ok(self
            .fetch_optional::<StoredChurchRow>(
                "SELECT id, name, address, latitude, longitude, country, description, gathering, ein, registry_state, registry_number, owner_id, invite_code, created_at, deleted_at FROM churches WHERE id = ?",
                &[Bind::Text(id)],
            )
            .await?
            .map(|row| {
                let deleted_at = row.deleted_at.clone();
                (Church::from(row), deleted_at)
            }))
    }

    pub async fn church_by_invite(&self, code: &str) -> anyhow::Result<Option<Church>> {
        let code = code.trim().to_lowercase();
        if code.is_empty() {
            return Ok(None);
        }
        Ok(self
            .fetch_optional::<ChurchRow>(
                "SELECT * FROM churches_live WHERE lower(invite_code) = ?",
                &[Bind::Text(&code)],
            )
            .await?
            .map(Church::from))
    }

    pub async fn churches_for_memberships(&self, user: &User) -> anyhow::Result<Vec<Church>> {
        let ids: Vec<&str> = user
            .memberships
            .iter()
            .map(|link| link.church_id.as_str())
            .collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.churches_with_ids(&ids).await
    }

    pub async fn churches_with_ids(&self, ids: &[&str]) -> anyhow::Result<Vec<Church>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT * FROM churches_live WHERE id IN ({})",
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
            SELECT u.id AS user_id, u.first_name, u.last_name, m.role AS role, m.status AS status
            FROM memberships_live m
            JOIN users u ON u.id = m.user_id
            WHERE m.church_id = ?
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
            SELECT u.id AS user_id, u.first_name, u.last_name, m.role AS role, m.status AS status
            FROM memberships_live m
            JOIN users u ON u.id = m.user_id
            WHERE m.church_id = ?
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

    pub async fn member_releases(&self, church_id: &str) -> anyhow::Result<Vec<MemberRelease>> {
        let rows = self
            .fetch_all::<ReleaseRow>(
                r#"
                SELECT m.user_id AS user_id,
                       (
                         SELECT m2.church_id FROM memberships_live m2
                         WHERE m2.user_id = m.user_id AND m2.church_id != m.church_id
                         LIMIT 1
                       ) AS other_church_id
                FROM memberships_live m
                WHERE m.church_id = ?
                ORDER BY m.user_id
                "#,
                &[Bind::Text(church_id)],
            )
            .await?;
        Ok(rows.into_iter().map(release_from).collect())
    }

    pub async fn governor_ids(&self, church_id: &str) -> anyhow::Result<Vec<String>> {
        self.fetch_strings(
            r#"
            SELECT user_id FROM memberships_live
            WHERE church_id = ? AND status = 'active' AND role IN ('owner', 'steward')
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
                   (SELECT COUNT(*) FROM memberships_live m WHERE m.church_id = c.id AND m.status = 'active') AS members,
                   (SELECT COUNT(*) FROM needs n WHERE n.church_id = c.id AND n.archived = 0 AND n.status = 'open') AS needs
            FROM churches_live c
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

#[derive(sqlx::FromRow)]
struct StoredChurchRow {
    pub id: String,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: String,
    pub description: String,
    pub gathering: String,
    pub ein: String,
    pub registry_state: String,
    pub registry_number: String,
    pub owner_id: String,
    pub invite_code: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

impl From<StoredChurchRow> for Church {
    fn from(row: StoredChurchRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            address: row.address,
            latitude: row.latitude,
            longitude: row.longitude,
            country: row.country,
            description: row.description,
            gathering: row.gathering,
            ein: row.ein,
            registry_state: row.registry_state,
            registry_number: row.registry_number,
            owner_id: row.owner_id,
            invite_code: row.invite_code,
            created_at: row.created_at,
        }
    }
}

fn sql_number(value: f64) -> String {
    format!("{value:.6}")
}

#[derive(sqlx::FromRow)]
struct ReleaseRow {
    user_id: String,
    other_church_id: Option<String>,
}

fn release_from(row: ReleaseRow) -> MemberRelease {
    match row.other_church_id {
        Some(_) => MemberRelease::AlsoElsewhere(row.user_id),
        None => MemberRelease::OnlyHere(row.user_id),
    }
}
