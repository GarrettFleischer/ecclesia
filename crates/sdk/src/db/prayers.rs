use super::bind::Bind;
use super::distance::haversine_km_sql;
use super::rows::{MarkRow, PrayerCardRow, PrayerLocatedRow, PrayerRow, map_all};
use super::Db;
use ecclesia_domain::{Prayer, PrayerCard, nearby_km};

const PRAYER_CARD_SELECT: &str = r#"
        SELECT p.id, p.church_id, c.name AS church_name, p.author_id,
               u.first_name AS author_first, u.last_name AS author_last,
               p.body, p.status, p.praise, p.created_at,
               (SELECT COUNT(*) FROM prayer_marks m WHERE m.prayer_id = p.id AND m.kind = 'prayed') AS prayed_count
        FROM prayers p
        JOIN churches c ON c.id = p.church_id
        LEFT JOIN users u ON u.id = p.author_id
"#;

impl Db {
    pub async fn prayer(&self, id: &str) -> anyhow::Result<Option<Prayer>> {
        Ok(self
            .fetch_optional::<PrayerRow>("SELECT * FROM prayers WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(Prayer::from))
    }

    pub async fn prayer_card(&self, id: &str) -> anyhow::Result<Option<PrayerCard>> {
        let sql = format!("{PRAYER_CARD_SELECT} WHERE p.id = ?");
        Ok(self
            .fetch_optional::<PrayerCardRow>(&sql, &[Bind::Text(id)])
            .await?
            .map(PrayerCard::from))
    }

    pub async fn answered_prayers(&self, church_id: &str) -> anyhow::Result<Vec<PrayerCard>> {
        let sql = format!(
            "{PRAYER_CARD_SELECT} WHERE p.church_id = ? AND p.status = 'answered'
             ORDER BY p.answered_at DESC, p.id DESC LIMIT 20"
        );
        Ok(map_all(
            self.fetch_all::<PrayerCardRow>(&sql, &[Bind::Text(church_id)])
                .await?,
        ))
    }

    pub async fn prayers_near(
        &self,
        latitude: f64,
        longitude: f64,
    ) -> anyhow::Result<Vec<PrayerCard>> {
        let distance = haversine_km_sql(
            &latitude.to_string(),
            &longitude.to_string(),
            "c.latitude",
            "c.longitude",
        );
        let sql = format!(
            "{PRAYER_CARD_SELECT} WHERE p.status = 'open' AND {distance} <= {cutoff}
             ORDER BY p.created_at DESC, p.id DESC LIMIT 40",
            cutoff = nearby_km()
        );
        Ok(map_all(self.fetch_all::<PrayerCardRow>(&sql, &[]).await?))
    }

    pub async fn open_prayers_for_deck(
        &self,
        church_id: Option<&str>,
        place: Option<(f64, f64)>,
    ) -> anyhow::Result<Vec<(Prayer, f64, f64)>> {
        let mut sql = String::from(
            "SELECT p.id, p.church_id, p.author_id, p.body, p.status, p.praise, p.manage_hash,
                    p.created_at, p.answered_at, c.latitude, c.longitude
             FROM prayers p
             JOIN churches c ON c.id = p.church_id
             WHERE p.status = 'open' AND (",
        );
        let mut binds = Vec::new();
        if let Some(church_id) = church_id {
            sql.push_str("p.church_id = ?");
            binds.push(Bind::Text(church_id));
        } else {
            sql.push_str("1 = 0");
        }
        if let Some((latitude, longitude)) = place {
            let distance = haversine_km_sql(
                &latitude.to_string(),
                &longitude.to_string(),
                "c.latitude",
                "c.longitude",
            );
            sql.push_str(&format!(" OR {distance} <= {}", nearby_km()));
        }
        sql.push_str(") ORDER BY p.created_at DESC LIMIT 200");
        Ok(map_all(
            self.fetch_all::<PrayerLocatedRow>(&sql, &binds).await?,
        ))
    }

    pub async fn prayer_mark_kind(
        &self,
        user_id: &str,
        prayer_id: &str,
        day: &str,
    ) -> anyhow::Result<Option<String>> {
        Ok(self
            .fetch_optional::<MarkRow>(
                "SELECT prayer_id, kind FROM prayer_marks WHERE user_id = ? AND prayer_id = ? AND day = ?",
                &[Bind::Text(user_id), Bind::Text(prayer_id), Bind::Text(day)],
            )
            .await?
            .map(|row| row.kind))
    }

    pub async fn seen_prayer_ids(&self, user_id: &str, day: &str) -> anyhow::Result<Vec<String>> {
        let rows = self
            .fetch_all::<MarkRow>(
                "SELECT prayer_id, kind FROM prayer_marks WHERE user_id = ? AND day = ?",
                &[Bind::Text(user_id), Bind::Text(day)],
            )
            .await?;
        Ok(rows.into_iter().map(|row| row.prayer_id).collect())
    }
}
