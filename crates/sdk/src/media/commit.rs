//! Outbox ids and statements that ride in the attachment transaction.
//!
//! Domain writes the need, reply, or avatar link. Apply runs these statements
//! in that same transaction so a failure leaves the asset staged.

use crate::clock::new_id;

pub(crate) const PROMOTE_KIND: &str = "media_promote";
pub(crate) const DELETE_KIND: &str = "media_delete";

pub(crate) const MARK_ATTACHED_SQL: &str = "
UPDATE media_assets
SET state = 'attached',
    attached_at = COALESCE(attached_at, ?)
WHERE id = ?
  AND deleted_at IS NULL
  AND state IN ('staged', 'attached')
";

pub(crate) const INSERT_PROMOTE_SQL: &str = "
INSERT INTO outbox (id, kind, payload, attempts, available_at, status, dead_at)
VALUES (?, 'media_promote', ?, 0, ?, 'pending', NULL)
ON CONFLICT (id) DO UPDATE SET
    status = 'pending',
    attempts = 0,
    dead_at = NULL,
    available_at = excluded.available_at
WHERE outbox.status = 'dead'
";

pub(crate) const REFERENCE_SQL: &str = "
SELECT media_id AS id FROM need_media WHERE media_id = ?
UNION ALL
SELECT media_id AS id FROM reply_media WHERE media_id = ?
UNION ALL
SELECT avatar_media_id AS id FROM users WHERE avatar_media_id = ?
LIMIT 1
";

pub(crate) const MARK_DELETING_SQL: &str = "
UPDATE media_assets
SET state = 'deleting',
    deleted_at = COALESCE(deleted_at, ?)
WHERE id = ?
";

pub(crate) const INSERT_DELETE_SQL: &str = "
INSERT INTO outbox (id, kind, payload, attempts, available_at, status, dead_at)
VALUES (?, 'media_delete', ?, 0, ?, 'pending', NULL)
ON CONFLICT (id) DO UPDATE SET
    status = 'pending',
    attempts = 0,
    dead_at = NULL,
    available_at = excluded.available_at
WHERE outbox.status IN ('done', 'dead')
";

pub(crate) const RESTORE_ATTACHED_SQL: &str = "
UPDATE media_assets
SET state = 'attached',
    deleted_at = NULL
WHERE id = ?
  AND state = 'deleting'
";

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct PromoteJob {
    pub media_id: String,
    pub full_key: String,
    pub thumb_key: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct DeleteJob {
    pub media_id: String,
}

pub(crate) fn promote_job_id(media_id: &str) -> String {
    format!("media-promote-{media_id}")
}

pub(crate) fn delete_job_id(media_id: &str) -> String {
    format!("media-delete-{media_id}")
}

pub(crate) fn staging_keys() -> (String, String) {
    let token = new_id();
    (
        format!("stage/{token}/full.webp"),
        format!("stage/{token}/thumb.webp"),
    )
}

pub(crate) fn stable_keys() -> (String, String) {
    let token = new_id();
    (
        format!("media/{token}/full.webp"),
        format!("media/{token}/thumb.webp"),
    )
}

pub(crate) fn promote_payload(media_id: &str, full_key: &str, thumb_key: &str) -> String {
    serde_json::to_string(&PromoteJob {
        media_id: media_id.to_string(),
        full_key: full_key.to_string(),
        thumb_key: thumb_key.to_string(),
    })
    .unwrap_or_default()
}

pub(crate) fn delete_payload(media_id: &str) -> String {
    serde_json::to_string(&DeleteJob {
        media_id: media_id.to_string(),
    })
    .unwrap_or_default()
}
