use super::Db;
use super::bind::{Bind, placeholders};
use super::distance::haversine_km_sql;
use super::rows::{
    ApplicationRow, NeedCardRow, NeedReplyCardRow, NeedRow, map_all, map_reply_cards,
};
use ecclesia_domain::{Application, Need, NeedCard, NeedReplyCard, Viewer, nearby_km};

const NEED_CARD_SELECT: &str = r#"
        SELECT n.id, n.church_id, c.name AS church_name, c.address AS church_address,
               n.author_id, u.first_name AS author_first, u.last_name AS author_last,
               n.title, n.body, n.gift_id, g.name AS gift_name,
               n.scope, n.status, n.created_at, n.praise, n.archived
        FROM needs n
        JOIN churches_live c ON c.id = n.church_id
        JOIN users u ON u.id = n.author_id
        LEFT JOIN gifts g ON g.id = n.gift_id
"#;

impl Db {
    pub async fn need(&self, id: &str) -> anyhow::Result<Option<Need>> {
        Ok(self
            .fetch_optional::<NeedRow>(
                "SELECT n.* FROM needs n JOIN churches_live c ON c.id = n.church_id WHERE n.id = ?",
                &[Bind::Text(id)],
            )
            .await?
            .map(Need::from))
    }

    pub async fn open_needs_from_closed_churches(
        &self,
        author_id: &str,
    ) -> anyhow::Result<Vec<ClosedNeedGroup>> {
        let rows = self
            .fetch_all::<ClosedNeedRow>(
                r#"
                SELECT c.id AS church_id, c.name AS church_name, n.title AS title
                FROM needs n
                JOIN churches c ON c.id = n.church_id
                WHERE n.author_id = ? AND n.status = 'open' AND n.archived = 0
                  AND c.deleted_at IS NOT NULL
                ORDER BY c.name, c.id, n.created_at, n.id
                "#,
                &[Bind::Text(author_id)],
            )
            .await?;
        Ok(group_closed_needs(rows))
    }

    pub async fn open_needs_on_closed_church(
        &self,
        author_id: &str,
        church_id: &str,
    ) -> anyhow::Result<Vec<Need>> {
        let rows = self
            .fetch_all::<NeedRow>(
                r#"
                SELECT n.id, n.church_id, n.author_id, n.title, n.body, n.gift_id, n.scope,
                       n.status, n.created_at, n.closed_at, n.praise, n.archived
                FROM needs n
                JOIN churches c ON c.id = n.church_id
                WHERE n.author_id = ? AND n.church_id = ? AND n.status = 'open' AND n.archived = 0
                  AND c.deleted_at IS NOT NULL
                ORDER BY n.created_at, n.id
                "#,
                &[Bind::Text(author_id), Bind::Text(church_id)],
            )
            .await?;
        Ok(rows.into_iter().map(Need::from).collect())
    }

    pub async fn need_card(&self, id: &str) -> anyhow::Result<Option<NeedCard>> {
        let sql = format!("{NEED_CARD_SELECT} WHERE n.id = ? ORDER BY n.created_at DESC");
        Ok(self
            .fetch_optional::<NeedCardRow>(&sql, &[Bind::Text(id)])
            .await?
            .map(NeedCard::from))
    }

    pub async fn home_need_cards(
        &self,
        viewer: &Viewer,
        after: Option<(&str, &str)>,
    ) -> anyhow::Result<Vec<NeedCard>> {
        let church_ids: Vec<&str> = viewer
            .user
            .memberships
            .iter()
            .filter(|link| link.is_active())
            .map(|link| link.church_id.as_str())
            .collect();
        if church_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sql = String::from(NEED_CARD_SELECT);
        sql.push_str(" WHERE n.archived = 0 AND n.status = 'open' AND (n.church_id IN (");
        sql.push_str(&placeholders(church_ids.len()));
        let distance = haversine_km_sql(
            "mine.latitude",
            "mine.longitude",
            "c.latitude",
            "c.longitude",
        );
        sql.push_str(&format!(
            r#")
            OR (
                n.scope IN ('neighboring', 'body')
                AND EXISTS (
                    SELECT 1 FROM churches_live mine
                    JOIN memberships_live viewer_m ON viewer_m.church_id = mine.id
                    WHERE viewer_m.user_id = ?
                      AND viewer_m.status = 'active'
                      AND mine.id != c.id
                      AND {distance} <= {cutoff}
                )
            )
            OR n.scope = 'body'
        )"#,
            cutoff = nearby_km()
        ));
        if after.is_some() {
            sql.push_str(" AND (n.created_at < ? OR (n.created_at = ? AND n.id < ?))");
        }
        sql.push_str(" ORDER BY n.created_at DESC, n.id DESC LIMIT 40");
        let mut binds: Vec<Bind<'_>> = church_ids.iter().map(|id| Bind::Text(*id)).collect();
        binds.push(Bind::Text(viewer.user.id.as_str()));
        if let Some((created_at, id)) = after {
            binds.push(Bind::Text(created_at));
            binds.push(Bind::Text(created_at));
            binds.push(Bind::Text(id));
        }
        Ok(map_all(self.fetch_all::<NeedCardRow>(&sql, &binds).await?))
    }

    pub async fn church_need_cards_page(
        &self,
        church_id: &str,
        after: Option<(&str, &str)>,
    ) -> anyhow::Result<Vec<NeedCard>> {
        match after {
            None => {
                let sql = format!(
                    "{NEED_CARD_SELECT} WHERE n.church_id = ? AND n.archived = 0 ORDER BY n.created_at DESC, n.id DESC LIMIT 20"
                );
                Ok(map_all(
                    self.fetch_all::<NeedCardRow>(&sql, &[Bind::Text(church_id)])
                        .await?,
                ))
            }
            Some((created_at, id)) => {
                let sql = format!(
                    "{NEED_CARD_SELECT} WHERE n.church_id = ? AND n.archived = 0
                     AND (n.created_at < ? OR (n.created_at = ? AND n.id < ?))
                     ORDER BY n.created_at DESC, n.id DESC
                     LIMIT 20"
                );
                Ok(map_all(
                    self.fetch_all::<NeedCardRow>(
                        &sql,
                        &[
                            Bind::Text(church_id),
                            Bind::Text(created_at),
                            Bind::Text(created_at),
                            Bind::Text(id),
                        ],
                    )
                    .await?,
                ))
            }
        }
    }

    pub async fn application(&self, id: &str) -> anyhow::Result<Option<Application>> {
        Ok(self
            .fetch_optional::<ApplicationRow>(
                "SELECT * FROM applications WHERE id = ?",
                &[Bind::Text(id)],
            )
            .await?
            .map(Application::from))
    }

    pub async fn application_pair(
        &self,
        need_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<Application>> {
        Ok(self
            .fetch_optional::<ApplicationRow>(
                "SELECT * FROM applications WHERE need_id = ? AND user_id = ?",
                &[Bind::Text(need_id), Bind::Text(user_id)],
            )
            .await?
            .map(Application::from))
    }

    pub async fn need_replies(&self, need_id: &str) -> anyhow::Result<Vec<NeedReplyCard>> {
        let rows = self
            .fetch_all::<NeedReplyCardRow>(
                "SELECT r.id, r.need_id, r.author_id, u.first_name AS author_first,
                        u.last_name AS author_last, r.kind, r.body, r.created_at,
                        (
                            SELECT c.name
                            FROM memberships_live m
                            JOIN churches_live c ON c.id = m.church_id
                            WHERE m.user_id = r.author_id AND m.status = 'active'
                            ORDER BY m.created_at ASC, c.name ASC
                            LIMIT 1
                        ) AS author_church
                 FROM need_replies r
                 JOIN users u ON u.id = r.author_id
                 WHERE r.need_id = ?
                 ORDER BY r.created_at ASC, r.id ASC",
                &[Bind::Text(need_id)],
            )
            .await?;
        Ok(map_reply_cards(rows)?)
    }

    pub async fn needs_near(&self, latitude: f64, longitude: f64) -> anyhow::Result<Vec<NeedCard>> {
        let distance = haversine_km_sql(
            &latitude.to_string(),
            &longitude.to_string(),
            "c.latitude",
            "c.longitude",
        );
        let sql = format!(
            "{NEED_CARD_SELECT} WHERE n.archived = 0 AND n.status = 'open' AND {distance} <= {cutoff}
             ORDER BY n.created_at DESC, n.id DESC LIMIT 40",
            cutoff = nearby_km()
        );
        Ok(map_all(self.fetch_all::<NeedCardRow>(&sql, &[]).await?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedNeedGroup {
    pub church_id: String,
    pub church_name: String,
    pub titles: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct ClosedNeedRow {
    church_id: String,
    church_name: String,
    title: String,
}

fn group_closed_needs(rows: Vec<ClosedNeedRow>) -> Vec<ClosedNeedGroup> {
    let mut groups: Vec<ClosedNeedGroup> = Vec::new();
    for row in rows {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.church_id == row.church_id)
        {
            group.titles.push(row.title);
            continue;
        }
        groups.push(ClosedNeedGroup {
            church_id: row.church_id,
            church_name: row.church_name,
            titles: vec![row.title],
        });
    }
    groups
}

#[cfg(test)]
mod tests {
    use crate::db::Db;

    async fn fresh_db() -> Db {
        let path = std::env::temp_dir().join(format!(
            "ecclesia-home-{}-{}.db",
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
    async fn us_read_06_pending_member_gets_an_empty_home_page() {
        let db = fresh_db().await;
        let device = crate::story::DeviceMeta {
            user_agent: String::new(),
            ip: "local".into(),
        };
        let sdk = crate::Sdk::assemble(
            db.clone(),
            crate::judge::JudgeHub::silent(),
            crate::refine::RefineHub::silent(),
            crate::push::PushHub::silent(),
            crate::Cache::memory(),
        );
        db.seed_grace_church().await.expect("seed church");
        crate::story::register(
            &sdk,
            "Peter",
            "Lang",
            "peter@grace.test",
            "Thursday dinners at six oclock",
            &device,
        )
        .await
        .unwrap()
        .unwrap();
        let peter = db
            .user_by_email("peter@grace.test")
            .await
            .unwrap()
            .expect("peter");
        let viewer = sdk.viewer(peter).await.unwrap();
        assert!(!viewer.is_active_anywhere());
        let cards = db.home_need_cards(&viewer, None).await.unwrap();
        assert!(cards.is_empty());
    }
}
