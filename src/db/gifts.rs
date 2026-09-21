use super::Db;
use crate::leaf::{Endorsement, EndorsementCard, Gift, MemberGift};

impl Db {
    pub async fn gifts(&self) -> anyhow::Result<Vec<Gift>> {
        Ok(
            sqlx::query_as::<_, Gift>("SELECT * FROM gifts ORDER BY category, name")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn gift(&self, id: &str) -> anyhow::Result<Option<Gift>> {
        Ok(
            sqlx::query_as::<_, Gift>("SELECT * FROM gifts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn member_gifts(&self, user_id: &str) -> anyhow::Result<Vec<MemberGift>> {
        Ok(sqlx::query_as::<_, MemberGift>(
            r#"
            SELECT mg.user_id, mg.gift_id, mg.note, g.name AS gift_name, g.category
            FROM member_gifts mg
            JOIN gifts g ON g.id = mg.gift_id
            WHERE mg.user_id = ?
            ORDER BY g.category, g.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn gift_ids_for(&self, user_id: &str) -> anyhow::Result<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT gift_id FROM member_gifts WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn add_member_gift(
        &self,
        user_id: &str,
        gift_id: &str,
        note: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO member_gifts (user_id, gift_id, note) VALUES (?, ?, ?)
             ON CONFLICT(user_id, gift_id) DO UPDATE SET note = excluded.note",
        )
        .bind(user_id)
        .bind(gift_id)
        .bind(note.trim())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_member_gift(&self, user_id: &str, gift_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM member_gifts WHERE user_id = ? AND gift_id = ?")
            .bind(user_id)
            .bind(gift_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn pending_endorsement(
        &self,
        from: &str,
        to: &str,
        skill: &str,
    ) -> anyhow::Result<Option<Endorsement>> {
        Ok(sqlx::query_as::<_, Endorsement>(
            "SELECT * FROM endorsements
             WHERE from_user_id = ? AND to_user_id = ? AND status = 'pending'
               AND lower(trim(skill)) = lower(trim(?))",
        )
        .bind(from)
        .bind(to)
        .bind(skill)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn endorsement(&self, id: &str) -> anyhow::Result<Option<Endorsement>> {
        Ok(
            sqlx::query_as::<_, Endorsement>("SELECT * FROM endorsements WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn set_endorsement_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE endorsements SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn accepted_endorsements_for(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<EndorsementCard>> {
        endorsement_cards(self, user_id, "accepted").await
    }

    pub async fn pending_endorsements_for(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<EndorsementCard>> {
        endorsement_cards(self, user_id, "pending").await
    }
}

async fn endorsement_cards(
    db: &Db,
    user_id: &str,
    status: &str,
) -> anyhow::Result<Vec<EndorsementCard>> {
    Ok(sqlx::query_as::<_, EndorsementCard>(
        r#"
            SELECT e.id, e.from_user_id, f.name AS from_user_name, e.to_user_id, t.name AS to_user_name,
                   e.gift_id, COALESCE(NULLIF(e.skill, ''), g.name, 'Skill') AS gift_name,
                   e.note, e.status, e.created_at
            FROM endorsements e
            JOIN users f ON f.id = e.from_user_id
            JOIN users t ON t.id = e.to_user_id
            LEFT JOIN gifts g ON g.id = e.gift_id AND e.gift_id != ''
            WHERE e.to_user_id = ? AND e.status = ?
            ORDER BY e.created_at DESC
            "#,
    )
    .bind(user_id)
    .bind(status)
    .fetch_all(&db.pool)
    .await?)
}
