use super::bind::{placeholders, Bind};
use super::distance::haversine_km_sql;
use super::rows::{
    ApplicationCardRow, ApplicationRow, NeedCardRow, NeedReplyCardRow, NeedRow, map_all,
};
use super::Db;
use ecclesia_domain::{
    Application, ApplicationCard, Need, NeedCard, NeedReplyCard, Viewer, nearby_km,
};

const NEED_CARD_SELECT: &str = r#"
        SELECT n.id, n.church_id, c.name AS church_name, c.address AS church_address,
               n.author_id, u.first_name AS author_first, u.last_name AS author_last,
               n.title, n.body, n.gift_id, g.name AS gift_name,
               n.scope, n.status, n.created_at
        FROM needs n
        JOIN churches c ON c.id = n.church_id
        JOIN users u ON u.id = n.author_id
        LEFT JOIN gifts g ON g.id = n.gift_id
"#;

impl Db {
    pub async fn need(&self, id: &str) -> anyhow::Result<Option<Need>> {
        Ok(self
            .fetch_optional::<NeedRow>("SELECT * FROM needs WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(Need::from))
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
            .church_id
            .as_deref()
            .filter(|_| viewer.user.is_active())
            .into_iter()
            .collect();
        if church_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sql = String::from(NEED_CARD_SELECT);
        sql.push_str(" WHERE n.status = 'open' AND (n.church_id IN (");
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
                    SELECT 1 FROM churches mine
                    JOIN users viewer_user ON viewer_user.church_id = mine.id
                    WHERE viewer_user.id = ?
                      AND viewer_user.church_status = 'active'
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
                    "{NEED_CARD_SELECT} WHERE n.church_id = ? ORDER BY n.created_at DESC, n.id DESC LIMIT 20"
                );
                Ok(map_all(
                    self.fetch_all::<NeedCardRow>(&sql, &[Bind::Text(church_id)])
                        .await?,
                ))
            }
            Some((created_at, id)) => {
                let sql = format!(
                    "{NEED_CARD_SELECT} WHERE n.church_id = ?
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

    pub async fn applications_for_need(
        &self,
        need_id: &str,
    ) -> anyhow::Result<Vec<ApplicationCard>> {
        Ok(map_all(
            self.fetch_all::<ApplicationCardRow>(
                r#"
            SELECT a.id, a.need_id, a.user_id, u.first_name AS user_first, u.last_name AS user_last,
                   a.message, a.status, a.created_at
            FROM applications a
            JOIN users u ON u.id = a.user_id
            WHERE a.need_id = ?
            ORDER BY a.created_at
            "#,
                &[Bind::Text(need_id)],
            )
            .await?,
        ))
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
        Ok(map_all(
            self.fetch_all::<NeedReplyCardRow>(
                "SELECT r.id, r.need_id, r.author_id, u.first_name AS author_first,
                        u.last_name AS author_last, r.body, r.created_at
                 FROM need_replies r
                 JOIN users u ON u.id = r.author_id
                 WHERE r.need_id = ?
                 ORDER BY r.created_at ASC, r.id ASC",
                &[Bind::Text(need_id)],
            )
            .await?,
        ))
    }

    pub async fn needs_near(&self, latitude: f64, longitude: f64) -> anyhow::Result<Vec<NeedCard>> {
        let distance = haversine_km_sql(
            &latitude.to_string(),
            &longitude.to_string(),
            "c.latitude",
            "c.longitude",
        );
        let sql = format!(
            "{NEED_CARD_SELECT} WHERE n.status = 'open' AND {distance} <= {cutoff}
             ORDER BY n.created_at DESC, n.id DESC LIMIT 40",
            cutoff = nearby_km()
        );
        Ok(map_all(self.fetch_all::<NeedCardRow>(&sql, &[]).await?))
    }
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
