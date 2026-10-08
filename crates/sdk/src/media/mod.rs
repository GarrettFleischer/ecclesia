//! Private photos. Normalize an upload, stage it for its owner, and serve it
//! only after an audience check.
//!
//! Object keys do not grant a read. [`authorize_media_read`] checks the viewer
//! first. A public host without R2 returns [`MediaError::StorageUnavailable`]
//! from the media call; process boot does not require those variables.
//!
//! # Examples
//! ```
//! use ecclesia_sdk::media::DeclaredFormat;
//! use ecclesia_sdk::story::{StagedGrant, StagedMediaSight};
//!
//! assert_eq!(DeclaredFormat::from_mime("image/jpeg"), DeclaredFormat::Jpeg);
//! assert_eq!(
//!     DeclaredFormat::from_mime("photo.jpg"),
//!     DeclaredFormat::Unsupported
//! );
//!
//! let owner_id = "ada";
//! let ids = vec!["m1".to_string()];
//! let grants: Vec<StagedGrant<'_>> = ids
//!     .iter()
//!     .map(|media_id| StagedGrant { media_id })
//!     .collect();
//! let sight = StagedMediaSight::granted(owner_id, &grants);
//! assert!(sight.allows(owner_id, "m1"));
//! assert!(!sight.allows("bea", "m1"));
//! ```

pub(crate) mod commit;
mod decode;
mod store;

pub use store::{
    ConfiguredStore, LocalStore, MediaCaching, MediaDelivery, ObjectStore, R2Store,
    open_configured_store,
};

use ecclesia_domain::{Viewer, can_view_need};

use crate::clock::{new_id, now_iso};
use crate::db::{Bind, Db};

use commit::{DeleteJob, PromoteJob};
use decode::{normalize_image, read_capped};

/// Longest accepted upload, in bytes. 12 MiB.
pub const MAX_UPLOAD_BYTES: u64 = 12 * 1024 * 1024;

/// Pixel budget for one decoded image. 40 megapixels.
pub const MAX_PIXELS: u64 = 40_000_000;

/// Longest edge of the stored full image.
pub const FULL_LONGEST_EDGE: u32 = 2400;

/// Longest edge of the thumbnail.
pub const THUMB_LONGEST_EDGE: u32 = 640;

/// Presigned object URLs last five minutes.
pub const PRESIGN_SECONDS: u64 = 300;

/// Every stored variant is WebP.
pub const WEBP_CONTENT_TYPE: &str = "image/webp";

/// A media failure. [`MediaError::code`] is for logs. The App maps copy later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    StorageUnavailable,
    Store,
    Unreadable,
    TooManyBytes,
    TooManyPixels,
    Animated,
    TypeMismatch,
    NotFound,
    NotAllowed,
    NotOwned,
    ClosingReplyNotOnNeed,
}

impl MediaError {
    /// A stable code. Not a sentence for the page.
    pub fn code(&self) -> &'static str {
        match self {
            Self::StorageUnavailable => "storage_unavailable",
            Self::Store => "store",
            Self::Unreadable => "unreadable",
            Self::TooManyBytes => "too_many_bytes",
            Self::TooManyPixels => "too_many_pixels",
            Self::Animated => "animated",
            Self::TypeMismatch => "type_mismatch",
            Self::NotFound => "not_found",
            Self::NotAllowed => "not_allowed",
            Self::NotOwned => "not_owned",
            Self::ClosingReplyNotOnNeed => "closing_reply_not_on_need",
        }
    }
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for MediaError {}

/// What the client said the upload was. A file name is not a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredFormat {
    Omitted,
    Jpeg,
    Png,
    WebP,
    Unsupported,
}

impl DeclaredFormat {
    /// Maps a MIME type. `image/jpg` is JPEG. Anything else is unsupported.
    ///
    /// # Examples
    /// ```
    /// use ecclesia_sdk::media::DeclaredFormat;
    ///
    /// assert_eq!(DeclaredFormat::from_mime(" image/png "), DeclaredFormat::Png);
    /// assert_eq!(DeclaredFormat::from_mime(""), DeclaredFormat::Omitted);
    /// ```
    pub fn from_mime(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "" => Self::Omitted,
            "image/jpeg" | "image/jpg" => Self::Jpeg,
            "image/png" => Self::Png,
            "image/webp" => Self::WebP,
            _ => Self::Unsupported,
        }
    }
}

/// How large the client says the body is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadLength {
    Unknown,
    Declared(u64),
}

/// Which stored variant to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaVariant {
    Full,
    Thumb,
}

/// A staged photo the owner may preview or attach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedMedia {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub full_bytes: u64,
    pub thumb_bytes: u64,
}

/// Normalizes `body` and stores both variants for `owner_id`.
///
/// # Parameters
/// - `owner_id`: the signed-in uploader. Only that person can preview or attach the row.
/// - `declared`: the MIME type the client sent. The bytes decide the format.
/// - `length`: `Content-Length` when the client sent one.
/// - `body`: the upload stream.
///
/// # Returns
/// The staged id and the normalized dimensions.
///
/// # Errors
/// Refuses a stream over 12 MiB before decoding. A store or insert failure
/// leaves no staged row. Objects written before a failed insert are deleted.
///
/// # Notes
/// The row is `staged`. Attachment stories promote it in their own transaction.
pub async fn stage_media<S, R>(
    db: &Db,
    store: &S,
    owner_id: &str,
    declared: DeclaredFormat,
    length: UploadLength,
    mut body: R,
) -> Result<StagedMedia, MediaError>
where
    S: ObjectStore,
    R: tokio::io::AsyncRead + Unpin,
{
    if owner_id.is_empty() {
        return Err(MediaError::NotOwned);
    }
    let bytes = read_capped(&mut body, length).await?;
    let image = normalize_image(&bytes, declared)?;
    let id = new_id();
    let (full_key, thumb_key) = commit::staging_keys();
    store.put(&full_key, &image.full).await?;
    if let Err(error) = store.put(&thumb_key, &image.thumb).await {
        best_effort_delete(store, &full_key).await;
        return Err(error);
    }
    if let Err(error) = insert_staged(db, owner_id, &id, &full_key, &thumb_key, &image).await {
        best_effort_delete(store, &full_key).await;
        best_effort_delete(store, &thumb_key).await;
        return Err(error);
    }
    Ok(StagedMedia {
        id,
        width: image.width,
        height: image.height,
        full_bytes: image.full.len() as u64,
        thumb_bytes: image.thumb.len() as u64,
    })
}

/// Staged asset ids owned by `owner_id`, oldest first.
///
/// Pair them with [`ecclesia_sdk::story::StagedGrant`] and
/// [`ecclesia_sdk::story::StagedMediaSight::granted`]. An id staged by someone
/// else is not in the list, so the attachment story refuses it.
pub async fn staged_ids(db: &Db, owner_id: &str) -> Result<Vec<String>, MediaError> {
    let rows = db
        .fetch_all::<IdRow>(
            "SELECT id FROM media_assets
             WHERE owner_id = ? AND state = 'staged' AND deleted_at IS NULL
             ORDER BY created_at, id",
            &[Bind::Text(owner_id)],
        )
        .await
        .map_err(|_| MediaError::Store)?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

/// Marks an owned staged asset deleting and enqueues object deletion.
///
/// # Errors
/// [`MediaError::NotAllowed`] when the row is missing, belongs to someone else,
/// or is already attached. Attached photos are removed through the need, reply,
/// or avatar stories.
pub async fn discard_staged(db: &Db, owner_id: &str, media_id: &str) -> Result<(), MediaError> {
    let asset = load_asset(db, media_id)
        .await?
        .ok_or(MediaError::NotFound)?;
    if asset.owner_id != owner_id {
        return Err(MediaError::NotAllowed);
    }
    if asset.state != "staged" || asset.deleted_at.is_some() {
        return Err(MediaError::NotAllowed);
    }
    db.release_unreferenced_media(media_id)
        .await
        .map_err(|_| MediaError::Store)
}

/// Checks `viewer`, then returns bytes or a five-minute presigned URL.
///
/// # Parameters
/// - `variant`: full image or thumbnail.
///
/// # Returns
/// A stream for the local store, or a presigned URL for R2. Staged reads use
/// [`MediaCaching::NoStore`]. Attached reads may be cached privately for the
/// URL lifetime.
///
/// # Errors
/// Staged media is visible only to its owner. Need and reply media follow the
/// need audience. A profile photo follows profile visibility: any signed-in
/// person may open `/members/{id}`. A deleted asset is [`MediaError::NotFound`].
/// The object key is not an input and does not grant access.
pub async fn authorize_media_read<S: ObjectStore>(
    db: &Db,
    store: &S,
    viewer: &Viewer,
    media_id: &str,
    variant: MediaVariant,
) -> Result<MediaDelivery, MediaError> {
    let asset = load_asset(db, media_id)
        .await?
        .ok_or(MediaError::NotFound)?;
    let caching = access_caching(db, viewer, &asset).await?;
    let key = variant_key(&asset, variant);
    store.deliver(key, caching).await
}

/// Marks staged rows older than 24 hours for deletion.
///
/// The worker poll calls this before it claims outbox rows. Each row is safe
/// to release again.
pub async fn sweep_stale_staged(db: &Db, now: &str) -> anyhow::Result<()> {
    let deadline = stale_before(now)?;
    let rows = db
        .fetch_all::<IdRow>(
            "SELECT id FROM media_assets
             WHERE state = 'staged' AND deleted_at IS NULL AND created_at < ?",
            &[Bind::Text(&deadline)],
        )
        .await?;
    release_each(db, &rows).await
}

/// Runs one promotion or delete outbox payload.
///
/// # Errors
/// A store failure is returned so the outbox row retries. A missing asset is
/// success. Copying and deleting the same keys again is success.
pub async fn perform_media_job<S: ObjectStore>(
    db: &Db,
    store: &S,
    kind: &str,
    payload: &str,
) -> Result<(), MediaError> {
    match kind {
        commit::PROMOTE_KIND => promote_job(db, store, payload).await,
        commit::DELETE_KIND => delete_job(db, store, payload).await,
        _ => Err(MediaError::Store),
    }
}

/// The timestamp 24 hours before `now`, in the same UTC text form as `created_at`.
pub fn stale_before(now: &str) -> Result<String, MediaError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(now).map_err(|_| MediaError::Store)?;
    let deadline = parsed
        .checked_sub_signed(chrono::Duration::hours(24))
        .ok_or(MediaError::Store)?;
    Ok(deadline.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

#[derive(Debug, sqlx::FromRow)]
struct IdRow {
    id: String,
}

#[derive(Debug, sqlx::FromRow)]
struct AssetRow {
    id: String,
    owner_id: String,
    state: String,
    staging_full_key: String,
    staging_thumb_key: String,
    full_key: Option<String>,
    thumb_key: Option<String>,
    deleted_at: Option<String>,
}

async fn insert_staged(
    db: &Db,
    owner_id: &str,
    id: &str,
    full_key: &str,
    thumb_key: &str,
    image: &decode::NormalizedImage,
) -> Result<(), MediaError> {
    let created_at = now_iso();
    db.execute(
        "INSERT INTO media_assets (
            id, owner_id, state, staging_full_key, staging_thumb_key,
            width, height, full_bytes, thumb_bytes, created_at
         ) VALUES (?, ?, 'staged', ?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(id),
            Bind::Text(owner_id),
            Bind::Text(full_key),
            Bind::Text(thumb_key),
            Bind::I64(i64::from(image.width)),
            Bind::I64(i64::from(image.height)),
            Bind::I64(i64::try_from(image.full.len()).unwrap_or(i64::MAX)),
            Bind::I64(i64::try_from(image.thumb.len()).unwrap_or(i64::MAX)),
            Bind::Text(&created_at),
        ],
    )
    .await
    .map_err(|error| {
        tracing::warn!("staged media insert failed: {error:#}");
        MediaError::Store
    })
}

async fn load_asset(db: &Db, media_id: &str) -> Result<Option<AssetRow>, MediaError> {
    db.fetch_optional(
        "SELECT id, owner_id, state, staging_full_key, staging_thumb_key,
                full_key, thumb_key, deleted_at
         FROM media_assets WHERE id = ?",
        &[Bind::Text(media_id)],
    )
    .await
    .map_err(|_| MediaError::Store)
}

async fn access_caching(
    db: &Db,
    viewer: &Viewer,
    asset: &AssetRow,
) -> Result<MediaCaching, MediaError> {
    if asset.deleted_at.is_some() || asset.state == "deleting" {
        return Err(MediaError::NotFound);
    }
    if asset.state == "staged" {
        return staged_access(viewer, asset);
    }
    if asset.state != "attached" {
        return Err(MediaError::NotFound);
    }
    if need_audience_allows(db, viewer, &asset.id).await? {
        return Ok(MediaCaching::Private {
            seconds: PRESIGN_SECONDS,
        });
    }
    if avatar_is_current(db, &asset.id).await? && profile_visible(viewer) {
        return Ok(MediaCaching::Private {
            seconds: PRESIGN_SECONDS,
        });
    }
    Err(MediaError::NotAllowed)
}

fn staged_access(viewer: &Viewer, asset: &AssetRow) -> Result<MediaCaching, MediaError> {
    if viewer.user.id == asset.owner_id {
        Ok(MediaCaching::NoStore)
    } else {
        Err(MediaError::NotAllowed)
    }
}

/// Member pages are open to every signed-in person.
fn profile_visible(viewer: &Viewer) -> bool {
    !viewer.user.id.is_empty()
}

async fn need_audience_allows(
    db: &Db,
    viewer: &Viewer,
    media_id: &str,
) -> Result<bool, MediaError> {
    let rows = db
        .fetch_all::<IdRow>(
            "SELECT need_id AS id FROM need_media WHERE media_id = ?
             UNION
             SELECT r.need_id AS id FROM reply_media m
             JOIN need_replies r ON r.id = m.reply_id
             WHERE m.media_id = ?",
            &[Bind::Text(media_id), Bind::Text(media_id)],
        )
        .await
        .map_err(|_| MediaError::Store)?;
    audience_allows(db, viewer, &rows).await
}

async fn audience_allows(db: &Db, viewer: &Viewer, rows: &[IdRow]) -> Result<bool, MediaError> {
    for row in rows {
        if need_visible(db, viewer, &row.id).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn need_visible(db: &Db, viewer: &Viewer, need_id: &str) -> Result<bool, MediaError> {
    let Some(need) = db.need(need_id).await.map_err(|_| MediaError::Store)? else {
        return Ok(false);
    };
    let Some(church) = db
        .church(&need.church_id)
        .await
        .map_err(|_| MediaError::Store)?
    else {
        return Ok(false);
    };
    Ok(can_view_need(viewer, need.sight(), &church))
}

async fn avatar_is_current(db: &Db, media_id: &str) -> Result<bool, MediaError> {
    let row = db
        .fetch_optional::<IdRow>(
            "SELECT id FROM users WHERE avatar_media_id = ?",
            &[Bind::Text(media_id)],
        )
        .await
        .map_err(|_| MediaError::Store)?;
    Ok(row.is_some())
}

fn variant_key(asset: &AssetRow, variant: MediaVariant) -> &str {
    match variant {
        MediaVariant::Full => asset.full_key.as_deref().unwrap_or(&asset.staging_full_key),
        MediaVariant::Thumb => asset
            .thumb_key
            .as_deref()
            .unwrap_or(&asset.staging_thumb_key),
    }
}

async fn promote_job<S: ObjectStore>(db: &Db, store: &S, payload: &str) -> Result<(), MediaError> {
    let job: PromoteJob = serde_json::from_str(payload).map_err(|_| MediaError::Store)?;
    let Some(asset) = load_asset(db, &job.media_id).await? else {
        return Ok(());
    };
    if asset.deleted_at.is_some() || asset.state == "deleting" {
        return Ok(());
    }
    if promoted_keys_match(&asset, &job) {
        return delete_staging(store, &asset).await;
    }
    store.copy(&asset.staging_full_key, &job.full_key).await?;
    store.copy(&asset.staging_thumb_key, &job.thumb_key).await?;
    record_promoted(db, &asset.id, &job.full_key, &job.thumb_key).await?;
    delete_staging(store, &asset).await
}

fn promoted_keys_match(asset: &AssetRow, job: &PromoteJob) -> bool {
    asset.full_key.as_deref() == Some(job.full_key.as_str())
        && asset.thumb_key.as_deref() == Some(job.thumb_key.as_str())
}

async fn record_promoted(
    db: &Db,
    media_id: &str,
    full_key: &str,
    thumb_key: &str,
) -> Result<(), MediaError> {
    db.execute(
        "UPDATE media_assets SET full_key = ?, thumb_key = ? WHERE id = ? AND state = 'attached'",
        &[
            Bind::Text(full_key),
            Bind::Text(thumb_key),
            Bind::Text(media_id),
        ],
    )
    .await
    .map_err(|_| MediaError::Store)
}

async fn delete_staging<S: ObjectStore>(store: &S, asset: &AssetRow) -> Result<(), MediaError> {
    store.delete(&asset.staging_full_key).await?;
    store.delete(&asset.staging_thumb_key).await
}

async fn delete_job<S: ObjectStore>(db: &Db, store: &S, payload: &str) -> Result<(), MediaError> {
    let job: DeleteJob = serde_json::from_str(payload).map_err(|_| MediaError::Store)?;
    let Some(asset) = load_asset(db, &job.media_id).await? else {
        return Ok(());
    };
    if media_is_referenced(db, &asset.id).await? {
        restore_referenced(db, &asset.id).await?;
        return Ok(());
    }
    delete_asset_objects(store, &asset).await
}

async fn media_is_referenced(db: &Db, media_id: &str) -> Result<bool, MediaError> {
    let row = db
        .fetch_optional::<IdRow>(
            commit::REFERENCE_SQL,
            &[
                Bind::Text(media_id),
                Bind::Text(media_id),
                Bind::Text(media_id),
            ],
        )
        .await
        .map_err(|_| MediaError::Store)?;
    Ok(row.is_some())
}

async fn restore_referenced(db: &Db, media_id: &str) -> Result<(), MediaError> {
    db.execute(commit::RESTORE_ATTACHED_SQL, &[Bind::Text(media_id)])
        .await
        .map_err(|_| MediaError::Store)
}

async fn delete_asset_objects<S: ObjectStore>(
    store: &S,
    asset: &AssetRow,
) -> Result<(), MediaError> {
    let mut keys = vec![
        asset.staging_full_key.as_str(),
        asset.staging_thumb_key.as_str(),
    ];
    if let Some(key) = asset.full_key.as_deref() {
        keys.push(key);
    }
    if let Some(key) = asset.thumb_key.as_deref() {
        keys.push(key);
    }
    delete_keys(store, &keys).await
}

async fn delete_keys<S: ObjectStore>(store: &S, keys: &[&str]) -> Result<(), MediaError> {
    for key in keys {
        if key.is_empty() {
            continue;
        }
        store.delete(key).await?;
    }
    Ok(())
}

async fn release_each(db: &Db, rows: &[IdRow]) -> anyhow::Result<()> {
    for row in rows {
        db.release_unreferenced_media(&row.id).await?;
    }
    Ok(())
}

async fn best_effort_delete<S: ObjectStore>(store: &S, key: &str) {
    match store.delete(key).await {
        Ok(()) => {}
        Err(error) => tracing::warn!("media cleanup failed: {error}"),
    }
}

#[cfg(test)]
mod tests;
