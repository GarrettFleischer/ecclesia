//! One short code for each shareable thing.
//! Open needs keep their code. A met need loses it after 30 days.
//! Sharing an archived need issues a new code for another 30 days.

use ecclesia_domain::{MET_NEED_DAYS, Share, ShareKind, share_code_for, share_material};

use super::Db;
use super::bind::Bind;
use super::rows::ShareRow;

const SHARE_SALT_LIMIT: usize = 8;

impl Db {
    pub async fn share_by_code(&self, code: &str) -> anyhow::Result<Option<Share>> {
        let key = code.trim().to_ascii_lowercase();
        let now = crate::clock::now_iso();
        Ok(self
            .fetch_optional::<ShareRow>(
                "SELECT code, kind, target_id, expires_at FROM shares_live
                 WHERE code = ? AND (expires_at IS NULL OR expires_at > ?)",
                &[Bind::Text(&key), Bind::Text(&now)],
            )
            .await?
            .map(Share::from))
    }

    pub async fn share_for_target(
        &self,
        kind: ShareKind,
        target_id: &str,
    ) -> anyhow::Result<Option<Share>> {
        let now = crate::clock::now_iso();
        Ok(self
            .fetch_optional::<ShareRow>(
                "SELECT code, kind, target_id, expires_at FROM shares_live
                 WHERE kind = ? AND target_id = ? AND (expires_at IS NULL OR expires_at > ?)",
                &[
                    Bind::Text(kind.as_str()),
                    Bind::Text(target_id),
                    Bind::Text(&now),
                ],
            )
            .await?
            .map(Share::from))
    }

    pub(crate) async fn fresh_share_code(
        &self,
        kind: ShareKind,
        target_id: &str,
    ) -> anyhow::Result<String> {
        self.pick_share_code(kind, target_id).await
    }

    pub(crate) async fn fresh_rotated_share_code(
        &self,
        kind: ShareKind,
        target_id: &str,
    ) -> anyhow::Result<String> {
        let material = format!("{target_id}{}", crate::clock::nonce());
        self.pick_share_code(kind, &material).await
    }

    pub(crate) async fn archive_met_needs(&self) -> anyhow::Result<()> {
        let now = crate::clock::now_iso();
        let cutoff = crate::clock::shift_days(&now, -MET_NEED_DAYS);
        self.execute(
            "UPDATE needs SET archived = 1
             WHERE status = 'closed' AND archived = 0 AND closed_at IS NOT NULL AND closed_at <= ?",
            &[Bind::Text(&cutoff)],
        )
        .await?;
        self.execute(
            "UPDATE shares SET deleted_at = ?
             WHERE kind = 'need'
               AND deleted_at IS NULL
               AND (
                    (expires_at IS NOT NULL AND expires_at <= ?)
                    OR (
                        expires_at IS NULL
                        AND target_id IN (SELECT id FROM needs WHERE archived = 1)
                    )
               )",
            &[Bind::Text(&now), Bind::Text(&now)],
        )
        .await
    }

    pub(crate) async fn ensure_need_shares(&self) -> anyhow::Result<()> {
        let missing = self
            .fetch_strings(
                "SELECT id FROM needs n
                 WHERE n.archived = 0
                   AND NOT EXISTS (
                     SELECT 1 FROM shares_live s WHERE s.kind = ? AND s.target_id = n.id
                 )",
                &[Bind::Text(ShareKind::Need.as_str())],
            )
            .await?;
        self.insert_missing_need_shares(&missing).await
    }

    async fn pick_share_code(&self, kind: ShareKind, target_id: &str) -> anyhow::Result<String> {
        let mut chosen = share_code_for(kind, &share_material(target_id, 0));
        for extra in 0..SHARE_SALT_LIMIT {
            chosen = share_code_for(kind, &share_material(target_id, extra));
            if !self.share_code_taken(&chosen).await? {
                return Ok(chosen);
            }
        }
        Ok(chosen)
    }

    async fn share_code_taken(&self, code: &str) -> anyhow::Result<bool> {
        let found = self
            .fetch_strings(
                "SELECT code FROM shares WHERE code = ?",
                &[Bind::Text(code)],
            )
            .await?;
        Ok(!found.is_empty())
    }

    async fn insert_missing_need_shares(&self, missing: &[String]) -> anyhow::Result<()> {
        for id in missing {
            self.insert_need_share(id).await?;
        }
        Ok(())
    }

    async fn insert_need_share(&self, id: &str) -> anyhow::Result<()> {
        let code = self.fresh_share_code(ShareKind::Need, id).await?;
        self.execute(
            self.share_insert_sql(),
            &[
                Bind::Text(&code),
                Bind::Text(ShareKind::Need.as_str()),
                Bind::Text(id),
            ],
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn stamp_need_closed_at(
        &self,
        id: &str,
        closed_at: &str,
    ) -> anyhow::Result<()> {
        self.execute(
            "UPDATE needs SET closed_at = ? WHERE id = ?",
            &[Bind::Text(closed_at), Bind::Text(id)],
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn stamp_share_expiry(
        &self,
        code: &str,
        expires_at: &str,
    ) -> anyhow::Result<()> {
        self.execute(
            "UPDATE shares SET expires_at = ? WHERE code = ?",
            &[Bind::Text(expires_at), Bind::Text(code)],
        )
        .await
    }

    fn share_insert_sql(&self) -> &'static str {
        if self.driver().is_postgres() {
            "INSERT INTO shares (code, kind, target_id) VALUES (?, ?, ?) ON CONFLICT (kind, target_id) DO UPDATE SET code = excluded.code, expires_at = NULL, deleted_at = NULL"
        } else {
            "INSERT INTO shares (code, kind, target_id) VALUES (?, ?, ?) ON CONFLICT (kind, target_id) DO UPDATE SET code = excluded.code, expires_at = NULL, deleted_at = NULL"
        }
    }
}
