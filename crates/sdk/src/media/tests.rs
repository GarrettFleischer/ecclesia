//! Storage behavior for staged photos, promotion, and authorized reads.

use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, atomic::AtomicU64};
use std::task::{Context, Poll};

use image::ImageEncoder;
use tokio::io::{AsyncRead, ReadBuf};

use ecclesia_domain::sample::{church, user_in_church, viewer_of};
use ecclesia_domain::{
    Attachment, AttachmentRef, CatalogPresence, Effect, Posture, Write, post_need_with_attachments,
};

use super::decode::{self, normalize_image, read_capped};
use super::store::{MediaCaching, MediaDelivery, ObjectStore, R2Store};
use super::{
    DeclaredFormat, MAX_UPLOAD_BYTES, MediaError, MediaVariant, UploadLength, authorize_media_read,
    discard_staged, perform_media_job, stage_media, staged_ids, sweep_stale_staged,
};
use crate::db::{Bind, Db};
use crate::host::R2Config;
use crate::story::{self, StagedGrant, StagedMediaSight};

const MARKER: &[u8] = b"ECCLESIA-EXIF-MARKER-9f3a";

struct MemStore {
    objects: Mutex<std::collections::HashMap<String, Vec<u8>>>,
    fail: Mutex<Fail>,
    delivers: AtomicUsize,
}

#[derive(Clone, Copy)]
enum Fail {
    None,
    Puts,
    CopiesLeft(u8),
    Deletes,
}

impl MemStore {
    fn new() -> Self {
        Self {
            objects: Mutex::new(std::collections::HashMap::new()),
            fail: Mutex::new(Fail::None),
            delivers: AtomicUsize::new(0),
        }
    }

    fn fail_puts(&self) {
        *self.fail.lock().expect("fail") = Fail::Puts;
    }

    fn fail_next_copy(&self) {
        *self.fail.lock().expect("fail") = Fail::CopiesLeft(1);
    }

    fn fail_deletes(&self) {
        *self.fail.lock().expect("fail") = Fail::Deletes;
    }

    fn allow(&self) {
        *self.fail.lock().expect("fail") = Fail::None;
    }

    fn contains(&self, key: &str) -> bool {
        self.objects.lock().expect("objects").contains_key(key)
    }
}

impl ObjectStore for MemStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), MediaError> {
        if matches!(*self.fail.lock().expect("fail"), Fail::Puts) {
            return Err(MediaError::Store);
        }
        self.objects
            .lock()
            .expect("objects")
            .insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        self.objects
            .lock()
            .expect("objects")
            .get(key)
            .cloned()
            .ok_or(MediaError::NotFound)
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), MediaError> {
        let fail_copy = {
            let mut fail = self.fail.lock().expect("fail");
            match *fail {
                Fail::CopiesLeft(left) if left > 0 => {
                    *fail = if left == 1 {
                        Fail::None
                    } else {
                        Fail::CopiesLeft(left - 1)
                    };
                    true
                }
                _ => false,
            }
        };
        if fail_copy {
            return Err(MediaError::Store);
        }
        let bytes = self.get(from).await.map_err(|error| match error {
            MediaError::NotFound => MediaError::Store,
            other => other,
        })?;
        self.put(to, &bytes).await
    }

    async fn delete(&self, key: &str) -> Result<(), MediaError> {
        if matches!(*self.fail.lock().expect("fail"), Fail::Deletes) {
            return Err(MediaError::Store);
        }
        self.objects.lock().expect("objects").remove(key);
        Ok(())
    }

    async fn deliver(&self, key: &str, caching: MediaCaching) -> Result<MediaDelivery, MediaError> {
        self.delivers.fetch_add(1, Ordering::SeqCst);
        let bytes = self.get(key).await?;
        Ok(MediaDelivery::Stream {
            bytes,
            content_type: super::WEBP_CONTENT_TYPE,
            caching,
        })
    }
}

struct CountingLocal {
    inner: super::LocalStore,
    delivers: AtomicUsize,
}

impl ObjectStore for CountingLocal {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), MediaError> {
        self.inner.put(key, bytes).await
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        self.inner.get(key).await
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), MediaError> {
        self.inner.copy(from, to).await
    }

    async fn delete(&self, key: &str) -> Result<(), MediaError> {
        self.inner.delete(key).await
    }

    async fn deliver(&self, key: &str, caching: MediaCaching) -> Result<MediaDelivery, MediaError> {
        self.delivers.fetch_add(1, Ordering::SeqCst);
        self.inner.deliver(key, caching).await
    }
}

struct Exploding;

impl AsyncRead for Exploding {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        panic!("read a stream that was already over the byte cap");
    }
}

struct Zeros {
    left: usize,
}

impl AsyncRead for Zeros {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.left == 0 {
            return Poll::Ready(Ok(()));
        }
        let dest = buf.initialize_unfilled();
        let count = dest.len().min(self.left);
        dest[..count].fill(0);
        buf.advance(count);
        self.left -= count;
        Poll::Ready(Ok(()))
    }
}

#[derive(sqlx::FromRow)]
struct AssetProbe {
    state: String,
    staging_full_key: String,
    staging_thumb_key: String,
    full_key: Option<String>,
    attached_at: Option<String>,
    deleted_at: Option<String>,
}

fn open_path() -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "ecclesia-media-{}-{}-{}.db",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

async fn open_db() -> Db {
    Db::connect(&format!("sqlite://{}", open_path().display()))
        .await
        .expect("test database")
}

fn ada() -> ecclesia_domain::Viewer {
    viewer_of(
        user_in_church("ada", "grace", "member", "active"),
        Some(church("grace")),
    )
}

fn bea() -> ecclesia_domain::Viewer {
    viewer_of(
        user_in_church("bea", "other", "member", "active"),
        Some(church("other")),
    )
}

async fn save_user(db: &Db, id: &str) {
    let person = user_in_church(id, "grace", "member", "active");
    db.apply(&Effect::write(Write::InsertUser(person)))
        .await
        .unwrap();
}

async fn save_church(db: &Db) {
    db.apply(&Effect::write(Write::InsertChurch(church("grace"))))
        .await
        .unwrap();
}

async fn probe(db: &Db, id: &str) -> AssetProbe {
    db.fetch_optional(
        "SELECT state, staging_full_key, staging_thumb_key, full_key, attached_at, deleted_at
         FROM media_assets WHERE id = ?",
        &[Bind::Text(id)],
    )
    .await
    .unwrap()
    .expect("asset")
}

async fn count(db: &Db, sql: &str, id: &str) -> i64 {
    db.fetch_scalar_i64(sql, &[Bind::Text(id)]).await.unwrap()
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn jpeg_with_exif(width: u32, height: u32, orientation: u16, marker: &[u8]) -> Vec<u8> {
    let mut pixels = image::RgbImage::new(width, height);
    for pixel in pixels.pixels_mut() {
        *pixel = image::Rgb([40, 80, 120]);
    }
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 90)
        .write_image(
            pixels.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    let mut out = Vec::with_capacity(encoded.len() + marker.len() + 80);
    out.extend_from_slice(&encoded[..2]);
    out.extend(app1(orientation, marker));
    out.extend_from_slice(&encoded[2..]);
    out
}

fn app1(orientation: u16, marker: &[u8]) -> Vec<u8> {
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&0u16.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes());
    tiff.extend_from_slice(marker);
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend(tiff);
    let mut segment = vec![0xFF, 0xE1];
    let len = u16::try_from(payload.len() + 2).unwrap();
    segment.extend_from_slice(&len.to_be_bytes());
    segment.extend(payload);
    segment
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&(u32::try_from(data.len()).unwrap()).to_be_bytes());
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&0u32.to_be_bytes());
    chunk
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut file = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    file.extend(png_chunk(b"IHDR", &ihdr));
    file.extend(png_chunk(b"IEND", &[]));
    file
}

fn animated_png() -> Vec<u8> {
    let mut file = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    let ihdr = [0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0];
    file.extend(png_chunk(b"IHDR", &ihdr));
    file.extend(png_chunk(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]));
    file.extend(png_chunk(b"IEND", &[]));
    file
}

fn animated_webp() -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(b"WEBP");
    body.extend_from_slice(b"VP8X");
    body.extend_from_slice(&10u32.to_le_bytes());
    body.push(0x02);
    body.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(u32::try_from(body.len()).unwrap()).to_le_bytes());
    file.extend(body);
    file
}

fn jpeg_sof(width: u16, height: u16) -> Vec<u8> {
    let mut body = vec![8];
    body.extend_from_slice(&height.to_be_bytes());
    body.extend_from_slice(&width.to_be_bytes());
    body.extend_from_slice(&[1, 1, 0x11, 0]);
    let len = u16::try_from(body.len() + 2).unwrap();
    let mut file = vec![0xFF, 0xD8, 0xFF, 0xC0];
    file.extend_from_slice(&len.to_be_bytes());
    file.extend(body);
    file.extend_from_slice(&[0xFF, 0xD9]);
    file
}

fn tiny_jpeg() -> Vec<u8> {
    jpeg_with_exif(2, 2, 1, b"")
}

async fn stage_jpeg(db: &Db, store: &MemStore, owner: &str) -> super::StagedMedia {
    stage_media(
        db,
        store,
        owner,
        DeclaredFormat::Omitted,
        UploadLength::Unknown,
        Cursor::new(tiny_jpeg()),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn exif_payload_is_absent_from_the_webp_and_orientation_is_applied() {
    let jpeg = jpeg_with_exif(4, 2, 6, MARKER);
    assert!(contains_bytes(&jpeg, MARKER));
    let normalized = normalize_image(&jpeg, DeclaredFormat::Omitted).unwrap();
    assert!(!contains_bytes(&normalized.full, MARKER));
    assert!(!contains_bytes(&normalized.thumb, MARKER));
    assert_eq!(&normalized.full[..4], b"RIFF");
    assert_eq!(&normalized.full[8..12], b"WEBP");
    let decoded = image::load_from_memory(&normalized.full).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (2, 4));
    assert_eq!((normalized.width, normalized.height), (2, 4));
}

#[tokio::test]
async fn byte_cap_rejects_before_the_stream_is_read_or_decoded() {
    let error = read_capped(&mut Exploding, UploadLength::Declared(MAX_UPLOAD_BYTES + 1))
        .await
        .unwrap_err();
    assert_eq!(error, MediaError::TooManyBytes);

    let error = read_capped(
        &mut Zeros {
            left: usize::try_from(MAX_UPLOAD_BYTES).unwrap() + 1,
        },
        UploadLength::Unknown,
    )
    .await
    .unwrap_err();
    assert_eq!(error, MediaError::TooManyBytes);
}

#[test]
fn pixel_headers_malformed_animation_and_mismatched_types_are_refused() {
    assert!(!decode::exceeds_pixel_budget(10_000, 4_000));
    assert!(decode::exceeds_pixel_budget(10_000, 4_001));
    assert_eq!(
        normalize_image(&png_header(10_000, 4_001), DeclaredFormat::Png).unwrap_err(),
        MediaError::TooManyPixels
    );
    assert_eq!(
        normalize_image(&jpeg_sof(20_000, 2_001), DeclaredFormat::Jpeg).unwrap_err(),
        MediaError::TooManyPixels
    );
    assert_eq!(
        normalize_image(b"not a photo", DeclaredFormat::Omitted).unwrap_err(),
        MediaError::Unreadable
    );
    assert_eq!(
        normalize_image(&[0xFF, 0xD8, 0xFF, 0x00], DeclaredFormat::Omitted).unwrap_err(),
        MediaError::Unreadable
    );
    assert_eq!(
        normalize_image(&animated_png(), DeclaredFormat::Png).unwrap_err(),
        MediaError::Animated
    );
    assert_eq!(
        normalize_image(&animated_webp(), DeclaredFormat::WebP).unwrap_err(),
        MediaError::Animated
    );
    let jpeg = tiny_jpeg();
    assert_eq!(
        normalize_image(&jpeg, DeclaredFormat::Png).unwrap_err(),
        MediaError::TypeMismatch
    );
    assert_eq!(
        normalize_image(&jpeg, DeclaredFormat::Unsupported).unwrap_err(),
        MediaError::TypeMismatch
    );
}

#[test]
fn full_and_thumb_edges_shrink_independently() {
    let jpeg = jpeg_with_exif(2401, 1, 1, b"");
    let normalized = normalize_image(&jpeg, DeclaredFormat::Jpeg).unwrap();
    assert_eq!((normalized.width, normalized.height), (2400, 1));
    let thumb = image::load_from_memory(&normalized.thumb).unwrap();
    assert_eq!((thumb.width(), thumb.height()), (640, 1));
    let full = image::load_from_memory(&normalized.full).unwrap();
    assert_eq!((full.width(), full.height()), (2400, 1));
}

#[tokio::test]
async fn foreign_staged_ids_are_refused_and_a_failed_attach_rolls_back() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    save_user(&db, "bea").await;
    save_church(&db).await;
    let store = MemStore::new();
    let ada_photo = stage_jpeg(&db, &store, "ada").await;
    let bea_photo = stage_jpeg(&db, &store, "bea").await;
    let ids = staged_ids(&db, "ada").await.unwrap();
    assert_eq!(ids, vec![ada_photo.id.clone()]);
    assert!(!ids.iter().any(|id| id == &bea_photo.id));

    let sdk = crate::Sdk::assemble(
        db.clone(),
        crate::judge::JudgeHub::silent(),
        crate::refine::RefineHub::silent(),
        crate::push::PushHub::silent(),
        crate::Cache::memory(),
    );
    let grants = [StagedGrant {
        media_id: ada_photo.id.as_str(),
    }];
    let sight = StagedMediaSight::granted("ada", &grants);
    let refused = story::post_need_with_attachments(
        &sdk,
        &ada(),
        "grace",
        "Roof",
        "It leaked.",
        None,
        "church",
        &[AttachmentRef {
            media_id: &bea_photo.id,
            description: None,
        }],
        &sight,
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(matches!(
        refused,
        story::ConversationError::UngrantedMedia { .. }
    ));
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM needs WHERE author_id = ?", "ada").await,
        0
    );

    let stolen = post_need_with_attachments(
        &ada(),
        "grace",
        "Roof",
        "It leaked.",
        None,
        CatalogPresence::Listed,
        "church",
        Posture::Lifts,
        &[AttachmentRef {
            media_id: &bea_photo.id,
            description: None,
        }],
        "need-foreign".into(),
        "t0".into(),
    )
    .unwrap();
    let error = db.apply(&stolen).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<MediaError>(),
        Some(&MediaError::NotOwned)
    );
    assert_eq!(probe(&db, &bea_photo.id).await.state, "staged");
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM needs WHERE id = ?",
            "need-foreign"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM outbox WHERE id = ?",
            &super::commit::promote_job_id(&bea_photo.id),
        )
        .await,
        0
    );

    let posted = post_need_with_attachments(
        &ada(),
        "grace",
        "Roof",
        "It leaked.",
        None,
        CatalogPresence::Listed,
        "church",
        Posture::Lifts,
        &[],
        "need-open".into(),
        "t0".into(),
    )
    .unwrap();
    db.apply(&posted).await.unwrap();
    let mut effect = Effect::write(Write::AttachNeedMedia {
        need_id: "need-open".into(),
        attachments: vec![Attachment {
            media_id: ada_photo.id.clone(),
            position: 0,
            description: None,
        }],
    });
    effect.push(Write::SetClosingReply {
        need_id: "need-open".into(),
        reply_id: Some("missing-reply".into()),
    });
    assert!(db.apply(&effect).await.is_err());
    assert_eq!(probe(&db, &ada_photo.id).await.state, "staged");
    assert!(probe(&db, &ada_photo.id).await.attached_at.is_none());
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM need_media WHERE media_id = ?",
            &ada_photo.id,
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM outbox WHERE id = ?",
            &super::commit::promote_job_id(&ada_photo.id),
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM needs WHERE id = ? AND status = 'open'",
            "need-open",
        )
        .await,
        1
    );
}

#[tokio::test]
async fn avatar_replacement_keeps_the_new_photo_and_cleans_the_old_one() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    let store = MemStore::new();
    let first = stage_jpeg(&db, &store, "ada").await;
    let second = stage_jpeg(&db, &store, "ada").await;
    db.apply(&Effect::write(Write::SetAvatar {
        user_id: "ada".into(),
        media_id: first.id.clone(),
    }))
    .await
    .unwrap();
    db.apply(&Effect::write(Write::SetAvatar {
        user_id: "ada".into(),
        media_id: second.id.clone(),
    }))
    .await
    .unwrap();
    assert_eq!(probe(&db, &second.id).await.state, "attached");
    assert!(probe(&db, &second.id).await.attached_at.is_some());
    assert_eq!(probe(&db, &first.id).await.state, "deleting");
    assert!(probe(&db, &first.id).await.deleted_at.is_some());
    let old = probe(&db, &first.id).await;
    assert!(store.contains(&old.staging_full_key));

    let row = db
        .outbox_row(&super::commit::delete_job_id(&first.id))
        .await
        .unwrap()
        .unwrap();
    store.fail_deletes();
    assert!(
        perform_media_job(&db, &store, &row.kind, &row.payload)
            .await
            .is_err()
    );
    assert!(store.contains(&old.staging_full_key));
    store.allow();
    perform_media_job(&db, &store, &row.kind, &row.payload)
        .await
        .unwrap();
    assert!(!store.contains(&old.staging_full_key));
    assert!(!store.contains(&old.staging_thumb_key));
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media_assets WHERE id = ?",
            &first.id
        )
        .await,
        1
    );

    let kept = authorize_media_read(&db, &store, &ada(), &second.id, MediaVariant::Full)
        .await
        .unwrap();
    let MediaDelivery::Stream { bytes, caching, .. } = kept else {
        panic!("local read should stream");
    };
    assert!(matches!(caching, MediaCaching::Private { seconds: 300 }));
    assert_eq!(&bytes[..4], b"RIFF");
    assert!(store.contains(&probe(&db, &second.id).await.staging_full_key));
}

#[tokio::test]
async fn a_referenced_avatar_is_not_deleted_when_it_is_replaced() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    save_church(&db).await;
    let store = MemStore::new();
    let shared = stage_jpeg(&db, &store, "ada").await;
    let next = stage_jpeg(&db, &store, "ada").await;
    let posted = post_need_with_attachments(
        &ada(),
        "grace",
        "Roof",
        "It leaked.",
        None,
        CatalogPresence::Listed,
        "church",
        Posture::Lifts,
        &[AttachmentRef {
            media_id: &shared.id,
            description: None,
        }],
        "need-shared".into(),
        "t0".into(),
    )
    .unwrap();
    db.apply(&posted).await.unwrap();
    db.apply(&Effect::write(Write::SetAvatar {
        user_id: "ada".into(),
        media_id: shared.id.clone(),
    }))
    .await
    .unwrap();
    db.apply(&Effect::write(Write::SetAvatar {
        user_id: "ada".into(),
        media_id: next.id.clone(),
    }))
    .await
    .unwrap();
    assert_eq!(probe(&db, &shared.id).await.state, "attached");
    assert!(
        db.outbox_row(&super::commit::delete_job_id(&shared.id))
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.contains(&probe(&db, &shared.id).await.staging_full_key));
}

#[tokio::test]
async fn stale_staged_rows_are_swept_and_retries_leave_no_broken_attachment() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    let store = MemStore::new();
    let fresh = stage_jpeg(&db, &store, "ada").await;
    let stale_id = "stale-photo";
    db.execute(
        "INSERT INTO media_assets (
            id, owner_id, state, staging_full_key, staging_thumb_key,
            width, height, full_bytes, thumb_bytes, created_at
         ) VALUES (?, 'ada', 'staged', 'stage/stale/full.webp', 'stage/stale/thumb.webp',
                   2, 2, 10, 8, '2026-10-06T11:59:59Z')",
        &[Bind::Text(stale_id)],
    )
    .await
    .unwrap();
    store.put("stage/stale/full.webp", b"full").await.unwrap();
    store.put("stage/stale/thumb.webp", b"thumb").await.unwrap();
    sweep_stale_staged(&db, "2026-10-07T12:00:00Z")
        .await
        .unwrap();
    assert_eq!(probe(&db, stale_id).await.state, "deleting");
    assert_eq!(probe(&db, &fresh.id).await.state, "staged");
    let row = db
        .outbox_row(&super::commit::delete_job_id(stale_id))
        .await
        .unwrap()
        .unwrap();
    perform_media_job(&db, &store, &row.kind, &row.payload)
        .await
        .unwrap();
    assert!(!store.contains("stage/stale/full.webp"));
    perform_media_job(&db, &store, &row.kind, &row.payload)
        .await
        .unwrap();

    store.fail_puts();
    let error = stage_media(
        &db,
        &store,
        "ada",
        DeclaredFormat::Omitted,
        UploadLength::Unknown,
        Cursor::new(tiny_jpeg()),
    )
    .await
    .unwrap_err();
    assert_eq!(error, MediaError::Store);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media_assets WHERE owner_id = ?",
            "ada"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn promotion_retries_and_local_reads_follow_the_audience() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    save_church(&db).await;
    let store = MemStore::new();
    let photo = stage_jpeg(&db, &store, "ada").await;
    let staged = authorize_media_read(&db, &store, &ada(), &photo.id, MediaVariant::Thumb)
        .await
        .unwrap();
    let MediaDelivery::Stream { caching, .. } = staged else {
        panic!("staged read streams");
    };
    assert_eq!(caching, MediaCaching::NoStore);
    let delivers_before = store.delivers.load(Ordering::SeqCst);
    let denied = authorize_media_read(&db, &store, &bea(), &photo.id, MediaVariant::Full)
        .await
        .unwrap_err();
    assert_eq!(denied, MediaError::NotAllowed);
    assert_eq!(store.delivers.load(Ordering::SeqCst), delivers_before);

    let posted = post_need_with_attachments(
        &ada(),
        "grace",
        "Roof",
        "It leaked.",
        None,
        CatalogPresence::Listed,
        "church",
        Posture::Lifts,
        &[AttachmentRef {
            media_id: &photo.id,
            description: Some("west slope"),
        }],
        "need-roof".into(),
        "t0".into(),
    )
    .unwrap();
    db.apply(&posted).await.unwrap();
    let attached = probe(&db, &photo.id).await;
    assert_eq!(attached.state, "attached");
    assert!(attached.full_key.is_none());
    assert!(attached.attached_at.is_some());
    let job = db
        .outbox_row(&super::commit::promote_job_id(&photo.id))
        .await
        .unwrap()
        .unwrap();
    store.fail_next_copy();
    assert!(
        perform_media_job(&db, &store, &job.kind, &job.payload)
            .await
            .is_err()
    );
    assert!(probe(&db, &photo.id).await.full_key.is_none());
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM need_media WHERE need_id = ?",
            "need-roof",
        )
        .await,
        1
    );
    assert!(store.contains(&attached.staging_full_key));
    perform_media_job(&db, &store, &job.kind, &job.payload)
        .await
        .unwrap();
    let promoted = probe(&db, &photo.id).await;
    assert!(promoted.full_key.is_some());
    assert!(!store.contains(&promoted.staging_full_key));
    assert!(!store.contains(&promoted.staging_thumb_key));
    assert!(store.contains(promoted.full_key.as_deref().unwrap()));
    let readable = authorize_media_read(&db, &store, &ada(), &photo.id, MediaVariant::Full)
        .await
        .unwrap();
    let MediaDelivery::Stream { bytes, .. } = readable else {
        panic!("promoted local read streams");
    };
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(
        authorize_media_read(&db, &store, &bea(), &photo.id, MediaVariant::Full)
            .await
            .unwrap_err(),
        MediaError::NotAllowed
    );

    let directory = tempfile::tempdir().unwrap();
    let local = CountingLocal {
        inner: super::LocalStore::open(directory.path().to_path_buf()),
        delivers: AtomicUsize::new(0),
    };
    let local_photo = stage_media(
        &db,
        &local,
        "ada",
        DeclaredFormat::Omitted,
        UploadLength::Unknown,
        Cursor::new(tiny_jpeg()),
    )
    .await
    .unwrap();
    let streamed = authorize_media_read(&db, &local, &ada(), &local_photo.id, MediaVariant::Full)
        .await
        .unwrap();
    let MediaDelivery::Stream { bytes, .. } = streamed else {
        panic!("local store streams");
    };
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(local.delivers.load(Ordering::SeqCst), 1);
    assert_eq!(
        authorize_media_read(&db, &local, &bea(), &local_photo.id, MediaVariant::Thumb)
            .await
            .unwrap_err(),
        MediaError::NotAllowed
    );
    assert_eq!(local.delivers.load(Ordering::SeqCst), 1);

    let file_root = directory.path().join("not-a-directory");
    std::fs::write(&file_root, b"x").unwrap();
    let blocked = super::LocalStore::open(file_root);
    let error = stage_media(
        &db,
        &blocked,
        "ada",
        DeclaredFormat::Omitted,
        UploadLength::Unknown,
        Cursor::new(tiny_jpeg()),
    )
    .await
    .unwrap_err();
    assert_eq!(error, MediaError::Store);
}

#[tokio::test]
async fn profile_photos_follow_member_visibility_and_discard_is_owner_only() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    let store = MemStore::new();
    let photo = stage_jpeg(&db, &store, "ada").await;
    db.apply(&Effect::write(Write::SetAvatar {
        user_id: "ada".into(),
        media_id: photo.id.clone(),
    }))
    .await
    .unwrap();
    let seen = authorize_media_read(&db, &store, &bea(), &photo.id, MediaVariant::Thumb)
        .await
        .unwrap();
    assert!(matches!(seen, MediaDelivery::Stream { .. }));
    assert_eq!(
        discard_staged(&db, "bea", &photo.id).await.unwrap_err(),
        MediaError::NotAllowed
    );
    assert_eq!(
        discard_staged(&db, "ada", &photo.id).await.unwrap_err(),
        MediaError::NotAllowed
    );

    let loose = stage_jpeg(&db, &store, "ada").await;
    discard_staged(&db, "ada", &loose.id).await.unwrap();
    assert_eq!(probe(&db, &loose.id).await.state, "deleting");
    assert_eq!(
        authorize_media_read(&db, &store, &ada(), &loose.id, MediaVariant::Full)
            .await
            .unwrap_err(),
        MediaError::NotFound
    );
}

#[test]
fn r2_presign_uses_the_private_endpoint_for_five_minutes() {
    let store = R2Store::open(R2Config {
        account_id: "account-id".into(),
        access_key_id: "key".into(),
        secret_access_key: "secret".into(),
        bucket: "ecclesia-media".into(),
    })
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let delivery = runtime
        .block_on(store.deliver(
            "media/token/full.webp",
            MediaCaching::Private { seconds: 300 },
        ))
        .unwrap();
    let MediaDelivery::Presigned {
        url,
        expires_in_secs,
        content_type,
        ..
    } = delivery
    else {
        panic!("r2 returns a presigned url");
    };
    assert_eq!(expires_in_secs, 300);
    assert_eq!(content_type, "image/webp");
    assert!(url.contains("https://account-id.r2.cloudflarestorage.com/ecclesia-media/"));
    assert!(url.contains("X-Amz-Expires=300"));
    assert!(!url.contains("r2.dev"));
    assert!(!url.contains("secret"));
}

#[tokio::test]
async fn text_only_need_does_not_write_a_media_row() {
    let db = open_db().await;
    save_user(&db, "ada").await;
    let posted = post_need_with_attachments(
        &ada(),
        "grace",
        "Roof",
        "It leaked.",
        None,
        CatalogPresence::Listed,
        "church",
        Posture::Lifts,
        &[],
        "need-text".into(),
        "t0".into(),
    )
    .unwrap();
    db.apply(&posted).await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media_assets WHERE owner_id = ?",
            "ada"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM outbox WHERE kind = ?",
            "media_promote"
        )
        .await,
        0
    );
}
