use super::bind::Bind;
use super::rows::{EndorsementCardRow, EndorsementRow, GiftRow, MemberGiftRow, map_all};
use super::Db;
use ecclesia_domain::{Endorsement, EndorsementCard, Gift, MemberGift};

impl Db {
    pub async fn gifts(&self) -> anyhow::Result<Vec<Gift>> {
        Ok(map_all(
            self.fetch_all::<GiftRow>("SELECT * FROM gifts ORDER BY category, name", &[])
                .await?,
        ))
    }

    pub async fn gift(&self, id: &str) -> anyhow::Result<Option<Gift>> {
        Ok(self
            .fetch_optional::<GiftRow>("SELECT * FROM gifts WHERE id = ?", &[Bind::Text(id)])
            .await?
            .map(Gift::from))
    }

    pub async fn member_gifts(&self, user_id: &str) -> anyhow::Result<Vec<MemberGift>> {
        Ok(map_all(
            self.fetch_all::<MemberGiftRow>(
                r#"
            SELECT mg.user_id, mg.gift_id, mg.note, g.name AS gift_name, g.category
            FROM member_gifts mg
            JOIN gifts g ON g.id = mg.gift_id
            WHERE mg.user_id = ?
            ORDER BY g.category, g.name
            "#,
                &[Bind::Text(user_id)],
            )
            .await?,
        ))
    }

    pub async fn gift_ids_for(&self, user_id: &str) -> anyhow::Result<Vec<String>> {
        self.fetch_strings(
            "SELECT gift_id FROM member_gifts WHERE user_id = ?",
            &[Bind::Text(user_id)],
        )
        .await
    }

    pub async fn add_member_gift(
        &self,
        user_id: &str,
        gift_id: &str,
        note: &str,
    ) -> anyhow::Result<()> {
        self.execute(
            "INSERT INTO member_gifts (user_id, gift_id, note) VALUES (?, ?, ?)
             ON CONFLICT(user_id, gift_id) DO UPDATE SET note = excluded.note",
            &[
                Bind::Text(user_id),
                Bind::Text(gift_id),
                Bind::Text(note.trim()),
            ],
        )
        .await
    }

    pub async fn remove_member_gift(&self, user_id: &str, gift_id: &str) -> anyhow::Result<()> {
        self.execute(
            "DELETE FROM member_gifts WHERE user_id = ? AND gift_id = ?",
            &[Bind::Text(user_id), Bind::Text(gift_id)],
        )
        .await
    }

    pub async fn pending_endorsement(
        &self,
        from: &str,
        to: &str,
        skill: &str,
    ) -> anyhow::Result<Option<Endorsement>> {
        Ok(self
            .fetch_optional::<EndorsementRow>(
                "SELECT * FROM endorsements
             WHERE from_user_id = ? AND to_user_id = ? AND status = 'pending'
               AND lower(trim(skill)) = lower(trim(?))",
                &[Bind::Text(from), Bind::Text(to), Bind::Text(skill)],
            )
            .await?
            .map(Endorsement::from))
    }

    pub async fn endorsement(&self, id: &str) -> anyhow::Result<Option<Endorsement>> {
        Ok(self
            .fetch_optional::<EndorsementRow>(
                "SELECT * FROM endorsements WHERE id = ?",
                &[Bind::Text(id)],
            )
            .await?
            .map(Endorsement::from))
    }

    pub async fn set_endorsement_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        self.execute(
            "UPDATE endorsements SET status = ? WHERE id = ?",
            &[Bind::Text(status), Bind::Text(id)],
        )
        .await
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

    pub async fn declined_endorsements_for(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<EndorsementCard>> {
        endorsement_cards(self, user_id, "declined").await
    }
}

async fn endorsement_cards(
    db: &Db,
    user_id: &str,
    status: &str,
) -> anyhow::Result<Vec<EndorsementCard>> {
    Ok(map_all(
        db.fetch_all::<EndorsementCardRow>(
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
            &[Bind::Text(user_id), Bind::Text(status)],
        )
        .await?,
    ))
}
