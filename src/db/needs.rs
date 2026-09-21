use super::Db;
use crate::leaf::{Application, ApplicationCard, Need, NeedCard};

impl Db {
    pub async fn need(&self, id: &str) -> anyhow::Result<Option<Need>> {
        Ok(
            sqlx::query_as::<_, Need>("SELECT * FROM needs WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn need_card(&self, id: &str) -> anyhow::Result<Option<NeedCard>> {
        Ok(sqlx::query_as::<_, NeedCard>(&need_card_sql("n.id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn all_need_cards(&self) -> anyhow::Result<Vec<NeedCard>> {
        Ok(sqlx::query_as::<_, NeedCard>(&need_card_sql("1 = 1"))
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn church_need_cards(&self, church_id: &str) -> anyhow::Result<Vec<NeedCard>> {
        Ok(
            sqlx::query_as::<_, NeedCard>(&need_card_sql("n.church_id = ?"))
                .bind(church_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn set_need_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE needs SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn applications_for_need(
        &self,
        need_id: &str,
    ) -> anyhow::Result<Vec<ApplicationCard>> {
        Ok(sqlx::query_as::<_, ApplicationCard>(
            r#"
            SELECT a.id, a.need_id, a.user_id, u.name AS user_name, a.message, a.status, a.created_at
            FROM applications a
            JOIN users u ON u.id = a.user_id
            WHERE a.need_id = ?
            ORDER BY a.created_at
            "#,
        )
        .bind(need_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn application(&self, id: &str) -> anyhow::Result<Option<Application>> {
        Ok(
            sqlx::query_as::<_, Application>("SELECT * FROM applications WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn application_pair(
        &self,
        need_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<Application>> {
        Ok(sqlx::query_as::<_, Application>(
            "SELECT * FROM applications WHERE need_id = ? AND user_id = ?",
        )
        .bind(need_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn set_application_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE applications SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn need_card_sql(filter: &str) -> String {
    format!(
        r#"
        SELECT n.id, n.church_id, c.name AS church_name, c.city AS church_city, c.region AS church_region,
               n.author_id, u.name AS author_name, n.title, n.body, n.gift_id, g.name AS gift_name,
               n.scope, n.status, n.created_at
        FROM needs n
        JOIN churches c ON c.id = n.church_id
        JOIN users u ON u.id = n.author_id
        LEFT JOIN gifts g ON g.id = n.gift_id
        WHERE {filter}
        ORDER BY n.created_at DESC
        "#
    )
}
