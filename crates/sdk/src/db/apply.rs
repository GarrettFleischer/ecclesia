use sqlx::{PgPool, SqlitePool};

use ecclesia_domain::{
    Application, Attachment, Church, Effect, Endorsement, Need, NeedReply, NoticeDraft, Prayer,
    Share, User, Write,
};

use crate::cache::keys_for_write;
use crate::clock::{new_id, now_iso};

use super::bind::Bind;
use super::dialect::Driver;
use super::extras::StoryExtras;
use super::{Db, Inner};

pub async fn apply_with(
    db: &Db,
    effect: &Effect,
    extras: &StoryExtras,
) -> anyhow::Result<Vec<String>> {
    match &db.inner {
        Inner::Sqlite(pool) => apply_sqlite(pool, effect, extras).await,
        Inner::Postgres(pool) => apply_postgres(pool, effect, extras).await,
    }
}

async fn apply_sqlite(
    pool: &SqlitePool,
    effect: &Effect,
    extras: &StoryExtras,
) -> anyhow::Result<Vec<String>> {
    let mut tx = pool.begin().await?;
    let keys = apply_effect(&mut SqliteExec(&mut tx), effect, extras).await?;
    tx.commit().await?;
    Ok(keys)
}

async fn apply_postgres(
    pool: &PgPool,
    effect: &Effect,
    extras: &StoryExtras,
) -> anyhow::Result<Vec<String>> {
    let mut tx = pool.begin().await?;
    let keys = apply_effect(&mut PostgresExec(&mut tx), effect, extras).await?;
    tx.commit().await?;
    Ok(keys)
}

trait Exec {
    async fn exec(&mut self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<()>;
    async fn fetch_text(&mut self, sql: &str, binds: &[Bind<'_>])
    -> anyhow::Result<Option<String>>;
}

struct SqliteExec<'a, 'c>(&'a mut sqlx::Transaction<'c, sqlx::Sqlite>);
struct PostgresExec<'a, 'c>(&'a mut sqlx::Transaction<'c, sqlx::Postgres>);

impl Exec for SqliteExec<'_, '_> {
    async fn exec(&mut self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<()> {
        let mut query = sqlx::query(sql);
        for bind in binds {
            query = match *bind {
                Bind::Text(value) => query.bind(value),
                Bind::OptText(value) => query.bind(value),
                Bind::I64(value) => query.bind(value),
                Bind::F64(value) => query.bind(value),
            };
        }
        query.execute(&mut **self.0).await?;
        Ok(())
    }

    async fn fetch_text(
        &mut self,
        sql: &str,
        binds: &[Bind<'_>],
    ) -> anyhow::Result<Option<String>> {
        let mut query = sqlx::query_scalar::<sqlx::Sqlite, String>(sql);
        for bind in binds {
            query = match *bind {
                Bind::Text(value) => query.bind(value),
                Bind::OptText(value) => query.bind(value),
                Bind::I64(value) => query.bind(value),
                Bind::F64(value) => query.bind(value),
            };
        }
        Ok(query.fetch_optional(&mut **self.0).await?)
    }
}

impl Exec for PostgresExec<'_, '_> {
    async fn exec(&mut self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<()> {
        let sql = Driver::Postgres.sql(sql);
        let mut query = sqlx::query(sql.as_ref());
        for bind in binds {
            query = match *bind {
                Bind::Text(value) => query.bind(value),
                Bind::OptText(value) => query.bind(value),
                Bind::I64(value) => query.bind(value),
                Bind::F64(value) => query.bind(value),
            };
        }
        query.execute(&mut **self.0).await?;
        Ok(())
    }

    async fn fetch_text(
        &mut self,
        sql: &str,
        binds: &[Bind<'_>],
    ) -> anyhow::Result<Option<String>> {
        let sql = Driver::Postgres.sql(sql);
        let mut query = sqlx::query_scalar::<sqlx::Postgres, String>(sql.as_ref());
        for bind in binds {
            query = match *bind {
                Bind::Text(value) => query.bind(value),
                Bind::OptText(value) => query.bind(value),
                Bind::I64(value) => query.bind(value),
                Bind::F64(value) => query.bind(value),
            };
        }
        Ok(query.fetch_optional(&mut **self.0).await?)
    }
}

async fn apply_effect(
    exec: &mut impl Exec,
    effect: &Effect,
    extras: &StoryExtras,
) -> anyhow::Result<Vec<String>> {
    let mut keys = Vec::new();
    for write in &effect.writes {
        let church_id = church_id_for(exec, write).await?;
        push_unique_keys(&mut keys, keys_for_write(write, church_id.as_deref()));
        apply_write(exec, write).await?;
    }
    apply_notices(exec, &effect.notices).await?;
    apply_extras(exec, extras).await?;
    Ok(keys)
}

async fn apply_extras(exec: &mut impl Exec, extras: &StoryExtras) -> anyhow::Result<()> {
    if let Some(user_id) = extras.delete_sessions_user.as_deref() {
        let deleted_at = now_iso();
        exec.exec(
            "UPDATE sessions SET deleted_at = ? WHERE user_id = ? AND deleted_at IS NULL",
            &[Bind::Text(&deleted_at), Bind::Text(user_id)],
        )
        .await?;
    }
    if let Some(id) = extras.delete_session_id.as_deref() {
        let deleted_at = now_iso();
        exec.exec(
            "UPDATE sessions SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
            &[Bind::Text(&deleted_at), Bind::Text(id)],
        )
        .await?;
    }
    if let Some(write) = extras.password_hash.as_ref() {
        exec.exec(
            "UPDATE users SET password_hash = ? WHERE id = ?",
            &[Bind::Text(&write.hash), Bind::Text(&write.user_id)],
        )
        .await?;
    }
    if let Some(session) = extras.session.as_ref() {
        exec.exec(
            "INSERT INTO sessions (id, user_id, csrf, created_at, last_seen_at, user_agent, ip, transport, refresh_token_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(&session.id),
                Bind::Text(&session.user_id),
                Bind::Text(&session.csrf),
                Bind::Text(&session.created_at),
                Bind::Text(&session.last_seen_at),
                Bind::Text(&session.user_agent),
                Bind::Text(&session.ip),
                Bind::Text(session.transport.as_str()),
                Bind::OptText(session.refresh_token_hash.as_deref()),
            ],
        )
        .await?;
    }
    if let Some(user_id) = extras.retire_magic_user.as_deref() {
        retire_tokens(exec, "magic_links", user_id).await?;
    }
    if let Some(user_id) = extras.retire_reset_user.as_deref() {
        retire_tokens(exec, "password_resets", user_id).await?;
    }
    if let Some(token) = extras.insert_magic.as_ref() {
        insert_token(exec, "magic_links", token).await?;
    }
    if let Some(token) = extras.insert_reset.as_ref() {
        insert_token(exec, "password_resets", token).await?;
    }
    if let Some(id) = extras.consume_magic_id.as_deref() {
        consume_token_row(exec, "magic_links", id).await?;
    }
    if let Some(id) = extras.consume_reset_id.as_deref() {
        consume_token_row(exec, "password_resets", id).await?;
    }
    insert_mails(exec, &extras.mail).await?;
    Ok(())
}

async fn consume_token_row(exec: &mut impl Exec, table: &str, id: &str) -> anyhow::Result<()> {
    let now = now_iso();
    let sql = format!("UPDATE {table} SET consumed_at = ? WHERE id = ?");
    exec.exec(&sql, &[Bind::Text(&now), Bind::Text(id)]).await
}

async fn retire_tokens(exec: &mut impl Exec, table: &str, user_id: &str) -> anyhow::Result<()> {
    let now = now_iso();
    let sql =
        format!("UPDATE {table} SET consumed_at = ? WHERE user_id = ? AND consumed_at IS NULL");
    exec.exec(&sql, &[Bind::Text(&now), Bind::Text(user_id)])
        .await
}

async fn insert_token(
    exec: &mut impl Exec,
    table: &str,
    token: &super::extras::TokenWrite,
) -> anyhow::Result<()> {
    let sql = format!(
        "INSERT INTO {table} (id, user_id, token_hash, expires_at, consumed_at, created_at)
         VALUES (?, ?, ?, ?, NULL, ?)"
    );
    exec.exec(
        &sql,
        &[
            Bind::Text(&token.id),
            Bind::Text(&token.user_id),
            Bind::Text(&token.token_hash),
            Bind::Text(&token.expires_at),
            Bind::Text(&token.created_at),
        ],
    )
    .await
}

async fn insert_mails(
    exec: &mut impl Exec,
    mails: &[super::extras::MailWrite],
) -> anyhow::Result<()> {
    for mail in mails {
        insert_mail(exec, mail).await?;
    }
    Ok(())
}

async fn insert_mail(exec: &mut impl Exec, mail: &super::extras::MailWrite) -> anyhow::Result<()> {
    let created = now_iso();
    let payload = serde_json::json!({
        "to": mail.to,
        "subject": mail.subject,
        "text": mail.text,
    })
    .to_string();
    let outbox_id = new_id();
    exec.exec(
        "INSERT INTO outbox (id, kind, payload, attempts, available_at, status, dead_at)
         VALUES (?, 'mail', ?, 0, ?, 'pending', NULL)",
        &[
            Bind::Text(&outbox_id),
            Bind::Text(&payload),
            Bind::Text(&created),
        ],
    )
    .await
}

async fn church_id_for(exec: &mut impl Exec, write: &Write) -> anyhow::Result<Option<String>> {
    match write {
        Write::UpsertMembership { church_id, .. } | Write::DeleteMembership { church_id, .. } => {
            Ok(Some(church_id.clone()))
        }
        Write::MoveNeed { id, .. }
        | Write::SetNeedStatus { id, .. }
        | Write::AttachNeedMedia { need_id: id, .. }
        | Write::DetachNeedMedia { need_id: id, .. }
        | Write::SetClosingReply { need_id: id, .. } => church_of_need(exec, id).await,
        Write::InsertNeedReply(reply) => church_of_need(exec, &reply.need_id).await,
        Write::AttachReplyMedia { reply_id, .. } | Write::DetachReplyMedia { reply_id, .. } => {
            church_of_reply(exec, reply_id).await
        }
        _ => Ok(None),
    }
}

async fn church_of_need(exec: &mut impl Exec, need_id: &str) -> anyhow::Result<Option<String>> {
    exec.fetch_text(
        "SELECT church_id FROM needs WHERE id = ?",
        &[Bind::Text(need_id)],
    )
    .await
}

async fn church_of_reply(exec: &mut impl Exec, reply_id: &str) -> anyhow::Result<Option<String>> {
    exec.fetch_text(
        "SELECT n.church_id FROM need_replies r
         JOIN needs n ON n.id = r.need_id
         WHERE r.id = ?",
        &[Bind::Text(reply_id)],
    )
    .await
}

fn push_unique_keys(keys: &mut Vec<String>, next: Vec<String>) {
    for key in next {
        if !keys.iter().any(|have| have == &key) {
            keys.push(key);
        }
    }
}

async fn apply_notices(exec: &mut impl Exec, notices: &[NoticeDraft]) -> anyhow::Result<()> {
    for notice in notices {
        apply_notice(exec, notice).await?;
    }
    Ok(())
}

async fn apply_write(exec: &mut impl Exec, write: &Write) -> anyhow::Result<()> {
    match write {
        Write::InsertUser(user) => insert_user(exec, user).await,
        Write::UpdateUser {
            id,
            first_name,
            last_name,
            bio,
        } => update_user(exec, id, first_name, last_name, bio).await,
        Write::SetAvatar { user_id, media_id } => set_avatar(exec, user_id, media_id).await,
        Write::ClearAvatar { user_id } => clear_avatar(exec, user_id).await,
        Write::InsertChurch(church) => insert_church(exec, church).await,
        Write::UpsertMembership {
            user_id,
            church_id,
            status,
            role,
        } => upsert_membership(exec, user_id, church_id, status, role).await,
        Write::DeleteMembership { user_id, church_id } => {
            delete_membership(exec, user_id, church_id).await
        }
        Write::SetChurchOwner {
            church_id,
            owner_id,
        } => set_church_owner(exec, church_id, owner_id).await,
        Write::CloseChurch { id, deleted_at } => close_church(exec, id, deleted_at).await,
        Write::InsertNeed(need) => insert_need(exec, need).await,
        Write::InsertShare(share) => insert_share(exec, share).await,
        Write::DeleteNeedShare { target_id } => delete_need_share(exec, target_id).await,
        Write::SetNeedStatus {
            id,
            status,
            closed_at,
            praise,
        } => set_need_status(exec, id, status, closed_at.as_deref(), praise.as_deref()).await,
        Write::SetClosingReply { need_id, reply_id } => {
            set_closing_reply(exec, need_id, reply_id.as_deref()).await
        }
        Write::AttachNeedMedia {
            need_id,
            attachments,
        } => attach_media(exec, MediaLink::Need, need_id, attachments).await,
        Write::DetachNeedMedia { need_id, media_id } => {
            detach_media(exec, MediaLink::Need, need_id, media_id).await
        }
        Write::MoveNeed { id, church_id } => move_need(exec, id, church_id).await,
        Write::InsertApplication(application) => insert_application(exec, application).await,
        Write::SetApplicationStatus { id, status } => {
            set_status(exec, StatusTable::Applications, id, status).await
        }
        Write::InsertEndorsement(endorsement) => insert_endorsement(exec, endorsement).await,
        Write::SetEndorsementStatus { id, status } => {
            set_status(exec, StatusTable::Endorsements, id, status).await
        }
        Write::UpsertMemberGift {
            user_id,
            gift_id,
            note,
        } => upsert_member_gift(exec, user_id, gift_id, note).await,
        Write::RemoveMemberGift { user_id, gift_id } => {
            remove_member_gift(exec, user_id, gift_id).await
        }
        Write::InsertNeedReply(reply) => insert_need_reply(exec, reply).await,
        Write::AttachReplyMedia {
            reply_id,
            attachments,
        } => attach_media(exec, MediaLink::Reply, reply_id, attachments).await,
        Write::DetachReplyMedia { reply_id, media_id } => {
            detach_media(exec, MediaLink::Reply, reply_id, media_id).await
        }
        Write::InsertPrayer(prayer) => insert_prayer(exec, prayer).await,
        Write::SetPrayerAnswered {
            id,
            praise,
            answered_at,
        } => set_prayer_answered(exec, id, praise, answered_at).await,
        Write::UpsertPrayerMark {
            user_id,
            prayer_id,
            day,
            kind,
        } => upsert_prayer_mark(exec, user_id, prayer_id, day, kind).await,
    }
}

enum StatusTable {
    Applications,
    Endorsements,
}

impl StatusTable {
    fn update_sql(self) -> &'static str {
        match self {
            Self::Applications => "UPDATE applications SET status = ? WHERE id = ?",
            Self::Endorsements => "UPDATE endorsements SET status = ? WHERE id = ?",
        }
    }
}

async fn apply_notice(exec: &mut impl Exec, notice: &NoticeDraft) -> anyhow::Result<()> {
    let notice_id = new_id();
    let created = now_iso();
    exec.exec(
        "INSERT INTO notifications (id, user_id, kind, title, body, href, read, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        &[
            Bind::Text(&notice_id),
            Bind::Text(&notice.user_id),
            Bind::Text(notice.kind),
            Bind::Text(notice.title.as_ref()),
            Bind::Text(notice.body),
            Bind::Text(&notice.href),
            Bind::Text(&created),
        ],
    )
    .await?;
    let payload = serde_json::json!({
        "user_id": notice.user_id,
        "title": notice.title.as_ref(),
        "body": notice.body,
        "href": notice.href,
        "notice_id": notice_id,
    })
    .to_string();
    let outbox_id = new_id();
    exec.exec(
        "INSERT INTO outbox (id, kind, payload, attempts, available_at, status, dead_at)
         VALUES (?, 'push', ?, 0, ?, 'pending', NULL)",
        &[
            Bind::Text(&outbox_id),
            Bind::Text(&payload),
            Bind::Text(&created),
        ],
    )
    .await?;
    Ok(())
}

async fn insert_user(exec: &mut impl Exec, user: &User) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO users (id, first_name, last_name, email, bio, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&user.id),
            Bind::Text(&user.first_name),
            Bind::Text(&user.last_name),
            Bind::Text(&user.email),
            Bind::Text(&user.bio),
            Bind::Text(&user.created_at),
        ],
    )
    .await
}

async fn update_user(
    exec: &mut impl Exec,
    id: &str,
    first_name: &str,
    last_name: &str,
    bio: &str,
) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE users SET first_name = ?, last_name = ?, bio = ? WHERE id = ?",
        &[
            Bind::Text(first_name.trim()),
            Bind::Text(last_name.trim()),
            Bind::Text(bio.trim()),
            Bind::Text(id),
        ],
    )
    .await
}

async fn upsert_membership(
    exec: &mut impl Exec,
    user_id: &str,
    church_id: &str,
    status: &str,
    role: &str,
) -> anyhow::Result<()> {
    let now = crate::clock::now_iso();
    exec.exec(
        "INSERT INTO memberships (user_id, church_id, role, status, created_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (user_id, church_id) DO UPDATE SET role = excluded.role, status = excluded.status, deleted_at = NULL",
        &[
            Bind::Text(user_id),
            Bind::Text(church_id),
            Bind::Text(role),
            Bind::Text(status),
            Bind::Text(&now),
        ],
    )
    .await
}

async fn delete_membership(
    exec: &mut impl Exec,
    user_id: &str,
    church_id: &str,
) -> anyhow::Result<()> {
    let now = crate::clock::now_iso();
    exec.exec(
        "UPDATE memberships SET deleted_at = ? WHERE user_id = ? AND church_id = ? AND deleted_at IS NULL",
        &[
            Bind::Text(&now),
            Bind::Text(user_id),
            Bind::Text(church_id),
        ],
    )
    .await
}

async fn set_church_owner(
    exec: &mut impl Exec,
    church_id: &str,
    owner_id: &str,
) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE churches SET owner_id = ? WHERE id = ? AND deleted_at IS NULL",
        &[Bind::Text(owner_id), Bind::Text(church_id)],
    )
    .await
}

async fn close_church(exec: &mut impl Exec, id: &str, deleted_at: &str) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE churches SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
        &[Bind::Text(deleted_at), Bind::Text(id)],
    )
    .await
}

async fn insert_church(exec: &mut impl Exec, church: &Church) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO churches (id, name, address, latitude, longitude, country, description, gathering, ein, registry_state, registry_number, owner_id, invite_code, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&church.id),
            Bind::Text(&church.name),
            Bind::Text(&church.address),
            Bind::F64(church.latitude),
            Bind::F64(church.longitude),
            Bind::Text(&church.country),
            Bind::Text(&church.description),
            Bind::Text(&church.gathering),
            Bind::Text(&church.ein),
            Bind::Text(&church.registry_state),
            Bind::Text(&church.registry_number),
            Bind::Text(&church.owner_id),
            Bind::Text(&church.invite_code),
            Bind::Text(&church.created_at),
        ],
    )
    .await
}

async fn insert_share(exec: &mut impl Exec, share: &Share) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO shares (code, kind, target_id, expires_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (kind, target_id) DO UPDATE SET code = excluded.code, expires_at = excluded.expires_at, deleted_at = NULL",
        &[
            Bind::Text(&share.code),
            Bind::Text(&share.kind),
            Bind::Text(&share.target_id),
            Bind::OptText(share.expires_at.as_deref()),
        ],
    )
    .await
}

async fn delete_need_share(exec: &mut impl Exec, target_id: &str) -> anyhow::Result<()> {
    let now = crate::clock::now_iso();
    exec.exec(
        "UPDATE shares SET deleted_at = ? WHERE kind = 'need' AND target_id = ? AND deleted_at IS NULL",
        &[Bind::Text(&now), Bind::Text(target_id)],
    )
    .await
}

/// Which conversation row an attachment belongs to.
///
/// # Notes
/// Detach deletes the link only. The asset row stays. When nothing else
/// references it, the same transaction marks it deleting and enqueues deletion.
#[derive(Clone, Copy)]
enum MediaLink {
    Need,
    Reply,
}

impl MediaLink {
    fn insert_sql(self) -> &'static str {
        match self {
            Self::Need => {
                "INSERT INTO need_media (need_id, media_id, position, description) VALUES (?, ?, ?, ?)"
            }
            Self::Reply => {
                "INSERT INTO reply_media (reply_id, media_id, position, description) VALUES (?, ?, ?, ?)"
            }
        }
    }

    fn delete_sql(self) -> &'static str {
        match self {
            Self::Need => "DELETE FROM need_media WHERE need_id = ? AND media_id = ?",
            Self::Reply => "DELETE FROM reply_media WHERE reply_id = ? AND media_id = ?",
        }
    }
}

async fn set_avatar(exec: &mut impl Exec, user_id: &str, media_id: &str) -> anyhow::Result<()> {
    let previous = exec
        .fetch_text(
            "SELECT avatar_media_id FROM users WHERE id = ?",
            &[Bind::Text(user_id)],
        )
        .await?;
    let user = exec
        .fetch_text("SELECT id FROM users WHERE id = ?", &[Bind::Text(user_id)])
        .await?;
    if user.is_none() {
        anyhow::bail!(crate::media::MediaError::NotOwned);
    }
    exec.exec(
        "UPDATE users SET avatar_media_id = ? WHERE id = ?",
        &[Bind::Text(media_id), Bind::Text(user_id)],
    )
    .await?;
    attach_owned_asset(exec, media_id, user_id).await?;
    if let Some(previous) = previous {
        if previous != media_id {
            release_unreferenced(exec, &previous).await?;
        }
    }
    Ok(())
}

async fn clear_avatar(exec: &mut impl Exec, user_id: &str) -> anyhow::Result<()> {
    let previous = exec
        .fetch_text(
            "SELECT avatar_media_id FROM users WHERE id = ?",
            &[Bind::Text(user_id)],
        )
        .await?;
    exec.exec(
        "UPDATE users SET avatar_media_id = NULL WHERE id = ?",
        &[Bind::Text(user_id)],
    )
    .await?;
    if let Some(previous) = previous {
        release_unreferenced(exec, &previous).await?;
    }
    Ok(())
}

async fn set_closing_reply(
    exec: &mut impl Exec,
    need_id: &str,
    reply_id: Option<&str>,
) -> anyhow::Result<()> {
    let Some(reply_id) = reply_id else {
        return exec
            .exec(
                "UPDATE needs SET closing_reply_id = NULL WHERE id = ?",
                &[Bind::Text(need_id)],
            )
            .await;
    };
    let on_need = exec
        .fetch_text(
            "SELECT need_id FROM need_replies WHERE id = ?",
            &[Bind::Text(reply_id)],
        )
        .await?;
    if on_need.as_deref() != Some(need_id) {
        anyhow::bail!(crate::media::MediaError::ClosingReplyNotOnNeed);
    }
    exec.exec(
        "UPDATE needs SET closing_reply_id = ? WHERE id = ?",
        &[Bind::Text(reply_id), Bind::Text(need_id)],
    )
    .await
}

/// Inserts attachment rows in `position` order.
///
/// # Parameters
/// - `attachments`: accepted photos. An empty slice inserts nothing.
///
/// # Returns
/// `Ok` after every row in the slice is inserted, or the first store error.
///
/// # Notes
/// Each photo link and that asset's `attached` state, `attached_at`, and
/// promotion outbox row commit together. An empty slice inserts nothing.
async fn attach_media(
    exec: &mut impl Exec,
    link: MediaLink,
    parent_id: &str,
    attachments: &[Attachment],
) -> anyhow::Result<()> {
    for attachment in attachments {
        insert_attachment(exec, link, parent_id, attachment).await?;
    }
    Ok(())
}

async fn insert_attachment(
    exec: &mut impl Exec,
    link: MediaLink,
    parent_id: &str,
    attachment: &Attachment,
) -> anyhow::Result<()> {
    exec.exec(
        link.insert_sql(),
        &[
            Bind::Text(parent_id),
            Bind::Text(&attachment.media_id),
            Bind::I64(attachment.position),
            Bind::OptText(attachment.description.as_deref()),
        ],
    )
    .await?;
    let author = match link {
        MediaLink::Need => {
            exec.fetch_text(
                "SELECT author_id FROM needs WHERE id = ?",
                &[Bind::Text(parent_id)],
            )
            .await?
        }
        MediaLink::Reply => {
            exec.fetch_text(
                "SELECT author_id FROM need_replies WHERE id = ?",
                &[Bind::Text(parent_id)],
            )
            .await?
        }
    };
    let Some(author) = author else {
        anyhow::bail!(crate::media::MediaError::NotOwned);
    };
    attach_owned_asset(exec, &attachment.media_id, &author).await
}

async fn detach_media(
    exec: &mut impl Exec,
    link: MediaLink,
    parent_id: &str,
    media_id: &str,
) -> anyhow::Result<()> {
    exec.exec(
        link.delete_sql(),
        &[Bind::Text(parent_id), Bind::Text(media_id)],
    )
    .await?;
    release_unreferenced(exec, media_id).await
}

async fn attach_owned_asset(
    exec: &mut impl Exec,
    media_id: &str,
    owner_id: &str,
) -> anyhow::Result<()> {
    let stored = exec
        .fetch_text(
            "SELECT owner_id FROM media_assets
             WHERE id = ? AND deleted_at IS NULL AND state IN ('staged', 'attached')",
            &[Bind::Text(media_id)],
        )
        .await?;
    if stored.as_deref() != Some(owner_id) {
        anyhow::bail!(crate::media::MediaError::NotOwned);
    }
    let now = now_iso();
    exec.exec(
        crate::media::commit::MARK_ATTACHED_SQL,
        &[Bind::Text(&now), Bind::Text(media_id)],
    )
    .await?;
    let (full_key, thumb_key) = crate::media::commit::stable_keys();
    let payload = crate::media::commit::promote_payload(media_id, &full_key, &thumb_key);
    let job_id = crate::media::commit::promote_job_id(media_id);
    exec.exec(
        crate::media::commit::INSERT_PROMOTE_SQL,
        &[Bind::Text(&job_id), Bind::Text(&payload), Bind::Text(&now)],
    )
    .await
}

async fn release_unreferenced(exec: &mut impl Exec, media_id: &str) -> anyhow::Result<()> {
    let referenced = exec
        .fetch_text(
            crate::media::commit::REFERENCE_SQL,
            &[
                Bind::Text(media_id),
                Bind::Text(media_id),
                Bind::Text(media_id),
            ],
        )
        .await?;
    if referenced.is_some() {
        return Ok(());
    }
    let exists = exec
        .fetch_text(
            "SELECT id FROM media_assets WHERE id = ?",
            &[Bind::Text(media_id)],
        )
        .await?;
    if exists.is_none() {
        return Ok(());
    }
    let now = now_iso();
    exec.exec(
        crate::media::commit::MARK_DELETING_SQL,
        &[Bind::Text(&now), Bind::Text(media_id)],
    )
    .await?;
    let payload = crate::media::commit::delete_payload(media_id);
    let job_id = crate::media::commit::delete_job_id(media_id);
    exec.exec(
        crate::media::commit::INSERT_DELETE_SQL,
        &[Bind::Text(&job_id), Bind::Text(&payload), Bind::Text(&now)],
    )
    .await
}

impl Db {
    /// Marks `media_id` deleting when no need, reply, or avatar points at it,
    /// and enqueues one idempotent delete job. A remaining reference is a no-op.
    pub(crate) async fn release_unreferenced_media(&self, media_id: &str) -> anyhow::Result<()> {
        match &self.inner {
            Inner::Sqlite(pool) => {
                let mut tx = pool.begin().await?;
                release_unreferenced(&mut SqliteExec(&mut tx), media_id).await?;
                tx.commit().await?;
                Ok(())
            }
            Inner::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                release_unreferenced(&mut PostgresExec(&mut tx), media_id).await?;
                tx.commit().await?;
                Ok(())
            }
        }
    }
}

async fn set_need_status(
    exec: &mut impl Exec,
    id: &str,
    status: &str,
    closed_at: Option<&str>,
    praise: Option<&str>,
) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE needs SET status = ?, closed_at = ?, praise = ? WHERE id = ?",
        &[
            Bind::Text(status),
            Bind::OptText(closed_at),
            Bind::OptText(praise),
            Bind::Text(id),
        ],
    )
    .await
}

async fn move_need(exec: &mut impl Exec, id: &str, church_id: &str) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE needs SET church_id = ? WHERE id = ? AND status = 'open' AND archived = 0",
        &[Bind::Text(church_id), Bind::Text(id)],
    )
    .await
}

async fn insert_need(exec: &mut impl Exec, need: &Need) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO needs (id, church_id, author_id, title, body, gift_id, scope, status, created_at, closed_at, praise, archived)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&need.id),
            Bind::Text(&need.church_id),
            Bind::Text(&need.author_id),
            Bind::Text(&need.title),
            Bind::Text(&need.body),
            Bind::OptText(need.gift_id.as_deref()),
            Bind::Text(&need.scope),
            Bind::Text(&need.status),
            Bind::Text(&need.created_at),
            Bind::OptText(need.closed_at.as_deref()),
            Bind::OptText(need.praise.as_deref()),
            Bind::I64(need.shelf.flag()),
        ],
    )
    .await
}

async fn insert_application(exec: &mut impl Exec, application: &Application) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&application.id),
            Bind::Text(&application.need_id),
            Bind::Text(&application.user_id),
            Bind::Text(&application.message),
            Bind::Text(&application.status),
            Bind::Text(&application.created_at),
        ],
    )
    .await
}

async fn insert_need_reply(exec: &mut impl Exec, reply: &NeedReply) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO need_replies (id, need_id, author_id, kind, body, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&reply.id),
            Bind::Text(&reply.need_id),
            Bind::Text(&reply.author_id),
            Bind::Text(reply.kind.as_str()),
            Bind::Text(&reply.body),
            Bind::Text(&reply.created_at),
        ],
    )
    .await
}

async fn insert_prayer(exec: &mut impl Exec, prayer: &Prayer) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO prayers (id, church_id, author_id, body, status, praise, manage_hash, created_at, answered_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&prayer.id),
            Bind::Text(&prayer.church_id),
            Bind::OptText(prayer.author_id.as_deref()),
            Bind::Text(&prayer.body),
            Bind::Text(&prayer.status),
            Bind::OptText(prayer.praise.as_deref()),
            Bind::OptText(prayer.manage_hash.as_deref()),
            Bind::Text(&prayer.created_at),
            Bind::OptText(prayer.answered_at.as_deref()),
        ],
    )
    .await
}

async fn set_prayer_answered(
    exec: &mut impl Exec,
    id: &str,
    praise: &str,
    answered_at: &str,
) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE prayers SET status = 'answered', praise = ?, answered_at = ? WHERE id = ?",
        &[Bind::Text(praise), Bind::Text(answered_at), Bind::Text(id)],
    )
    .await
}

async fn upsert_prayer_mark(
    exec: &mut impl Exec,
    user_id: &str,
    prayer_id: &str,
    day: &str,
    kind: &str,
) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO prayer_marks (user_id, prayer_id, day, kind) VALUES (?, ?, ?, ?)
         ON CONFLICT (user_id, prayer_id, day) DO UPDATE SET kind = excluded.kind",
        &[
            Bind::Text(user_id),
            Bind::Text(prayer_id),
            Bind::Text(day),
            Bind::Text(kind),
        ],
    )
    .await
}

async fn insert_endorsement(exec: &mut impl Exec, endorsement: &Endorsement) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, skill, note, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&endorsement.id),
            Bind::Text(&endorsement.from_user_id),
            Bind::Text(&endorsement.to_user_id),
            Bind::Text(&endorsement.gift_id),
            Bind::Text(&endorsement.skill),
            Bind::Text(&endorsement.note),
            Bind::Text(&endorsement.status),
            Bind::Text(&endorsement.created_at),
        ],
    )
    .await
}

async fn set_status(
    exec: &mut impl Exec,
    table: StatusTable,
    id: &str,
    status: &str,
) -> anyhow::Result<()> {
    exec.exec(table.update_sql(), &[Bind::Text(status), Bind::Text(id)])
        .await
}

async fn upsert_member_gift(
    exec: &mut impl Exec,
    user_id: &str,
    gift_id: &str,
    note: &str,
) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO member_gifts (user_id, gift_id, note) VALUES (?, ?, ?)
         ON CONFLICT(user_id, gift_id) DO UPDATE SET note = excluded.note, deleted_at = NULL",
        &[
            Bind::Text(user_id),
            Bind::Text(gift_id),
            Bind::Text(note.trim()),
        ],
    )
    .await
}

async fn remove_member_gift(
    exec: &mut impl Exec,
    user_id: &str,
    gift_id: &str,
) -> anyhow::Result<()> {
    let deleted_at = now_iso();
    exec.exec(
        "UPDATE member_gifts SET deleted_at = ? WHERE user_id = ? AND gift_id = ? AND deleted_at IS NULL",
        &[
            Bind::Text(&deleted_at),
            Bind::Text(user_id),
            Bind::Text(gift_id),
        ],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{AvatarReference, ClosingReplyReference, Db};
    use ecclesia_domain::sample::{church, user_in_church, viewer_of};
    use ecclesia_domain::{
        AttachmentRef, CatalogPresence, NeedReply, NeedStatus, Posture, ReplyKind, Viewer,
        complete_need, post_need_with_attachments,
    };

    async fn open_db() -> Db {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ecclesia-apply-{}-{}-{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Db::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("test database")
    }

    fn author() -> Viewer {
        viewer_of(
            user_in_church("ada", "grace", "member", "active"),
            Some(church("grace")),
        )
    }

    async fn save_author(db: &Db, viewer: &Viewer) {
        db.apply(&Effect::write(Write::InsertUser(viewer.user.clone())))
            .await
            .unwrap();
    }

    async fn stage(db: &Db, id: &str, owner: &str) {
        let full = format!("stage/full/{id}");
        let thumb = format!("stage/thumb/{id}");
        db.execute(
            "INSERT INTO media_assets (
                id, owner_id, state, staging_full_key, staging_thumb_key,
                width, height, full_bytes, thumb_bytes, created_at
             ) VALUES (?, ?, 'staged', ?, ?, 800, 600, 1000, 400, '2026-10-07T12:00:00Z')",
            &[
                Bind::Text(id),
                Bind::Text(owner),
                Bind::Text(&full),
                Bind::Text(&thumb),
            ],
        )
        .await
        .unwrap();
    }

    async fn count(db: &Db, sql: &str, id: &str) -> i64 {
        db.fetch_scalar_i64(sql, &[Bind::Text(id)]).await.unwrap()
    }

    #[tokio::test]
    async fn completion_persists_kind_photos_and_closing_reply_together() {
        let db = open_db().await;
        let viewer = author();
        save_author(&db, &viewer).await;
        stage(&db, "m0", "ada").await;
        stage(&db, "m1", "ada").await;
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[
                AttachmentRef {
                    media_id: "m0",
                    description: Some("  west slope  "),
                },
                AttachmentRef {
                    media_id: "m1",
                    description: None,
                },
            ],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        db.apply(&posted).await.unwrap();
        let photos = db.need_attachments("need-1").await.unwrap();
        assert_eq!(photos.len(), 2);
        assert_eq!(photos[0].media_id, "m0");
        assert_eq!(photos[0].position, 0);
        assert_eq!(photos[0].description.as_deref(), Some("west slope"));
        assert_eq!(photos[1].description, None);
        let need = ecclesia_domain::Need {
            id: "need-1".into(),
            church_id: "grace".into(),
            author_id: "ada".into(),
            title: "Roof".into(),
            body: "It leaked.".into(),
            gift_id: None,
            scope: "church".into(),
            status: "open".into(),
            created_at: "t0".into(),
            closed_at: None,
            praise: None,
            shelf: ecclesia_domain::NeedShelf::Listed,
        };
        let completed = complete_need(
            &viewer,
            &need,
            "The roof is dry.",
            Posture::Lifts,
            &[AttachmentRef {
                media_id: "m1",
                description: Some("after"),
            }],
            "reply-1".into(),
            "t1".into(),
        )
        .unwrap();
        let keys = db.apply(&completed).await.unwrap();
        assert_eq!(
            keys,
            vec!["church:grace".to_string(), "directory".to_string()]
        );
        let reply = db.need_reply("reply-1").await.unwrap().unwrap();
        assert_eq!(reply.kind, ReplyKind::Completion);
        let attached = db.reply_attachments("reply-1").await.unwrap();
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0].description.as_deref(), Some("after"));
        assert_eq!(
            db.closing_reply_reference("need-1").await.unwrap(),
            Some(ClosingReplyReference::Reply("reply-1".into()))
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM needs WHERE id = ? AND status = 'closed' AND praise IS NULL",
                "need-1",
            )
            .await,
            1
        );
        let message = db
            .apply(&Effect::write(Write::InsertNeedReply(NeedReply {
                id: "reply-msg".into(),
                need_id: "need-1".into(),
                author_id: "ada".into(),
                kind: ReplyKind::Message,
                body: "I can come Saturday.".into(),
                created_at: "t2".into(),
            })))
            .await
            .unwrap();
        assert_eq!(
            message,
            vec!["church:grace".to_string(), "directory".to_string()]
        );
        assert_eq!(
            db.need_reply("reply-msg").await.unwrap().unwrap().kind,
            ReplyKind::Message
        );
    }

    #[tokio::test]
    async fn a_failed_closing_reply_leaves_the_need_unchanged() {
        let db = open_db().await;
        let viewer = author();
        save_author(&db, &viewer).await;
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        db.apply(&posted).await.unwrap();
        let mut effect = Effect::write(Write::SetClosingReply {
            need_id: "need-1".into(),
            reply_id: Some("missing".into()),
        });
        effect.push(Write::InsertNeedReply(NeedReply {
            id: "reply-bad".into(),
            need_id: "need-1".into(),
            author_id: "ada".into(),
            kind: ReplyKind::Completion,
            body: "This should roll back.".into(),
            created_at: "t1".into(),
        }));
        effect.push(Write::SetNeedStatus {
            id: "need-1".into(),
            status: NeedStatus::Closed.as_str(),
            closed_at: Some("t1".into()),
            praise: None,
        });
        assert!(db.apply(&effect).await.is_err());
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM needs WHERE id = ? AND status = 'open'",
                "need-1",
            )
            .await,
            1
        );
        assert!(db.need_reply("reply-bad").await.unwrap().is_none());
        assert_eq!(
            db.closing_reply_reference("need-1").await.unwrap(),
            Some(ClosingReplyReference::Unset)
        );
    }

    #[tokio::test]
    async fn detach_and_empty_attach_leave_assets_and_insert_no_rows() {
        let db = open_db().await;
        let viewer = author();
        save_author(&db, &viewer).await;
        stage(&db, "m1", "ada").await;
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[AttachmentRef {
                media_id: "m1",
                description: None,
            }],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        db.apply(&posted).await.unwrap();
        db.apply(&Effect::write(Write::AttachNeedMedia {
            need_id: "need-1".into(),
            attachments: vec![],
        }))
        .await
        .unwrap();
        db.apply(&Effect::write(Write::AttachReplyMedia {
            reply_id: "missing-reply".into(),
            attachments: vec![],
        }))
        .await
        .unwrap();
        assert_eq!(db.need_attachments("need-1").await.unwrap().len(), 1);
        db.apply(&Effect::write(Write::DetachNeedMedia {
            need_id: "need-1".into(),
            media_id: "m1".into(),
        }))
        .await
        .unwrap();
        assert!(db.need_attachments("need-1").await.unwrap().is_empty());
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM media_assets WHERE id = ?", "m1").await,
            1
        );
    }

    #[tokio::test]
    async fn avatar_set_replaces_and_clear_returns_to_initials() {
        let db = open_db().await;
        let viewer = author();
        save_author(&db, &viewer).await;
        stage(&db, "m1", "ada").await;
        stage(&db, "m2", "ada").await;
        let first = db
            .apply(&Effect::write(Write::SetAvatar {
                user_id: "ada".into(),
                media_id: "m1".into(),
            }))
            .await
            .unwrap();
        assert!(first.is_empty());
        assert_eq!(
            db.avatar_reference("ada").await.unwrap(),
            Some(AvatarReference::Photo("m1".into()))
        );
        db.apply(&Effect::write(Write::SetAvatar {
            user_id: "ada".into(),
            media_id: "m2".into(),
        }))
        .await
        .unwrap();
        assert_eq!(
            db.avatar_reference("ada").await.unwrap(),
            Some(AvatarReference::Photo("m2".into()))
        );
        db.apply(&Effect::write(Write::ClearAvatar {
            user_id: "ada".into(),
        }))
        .await
        .unwrap();
        assert_eq!(
            db.avatar_reference("ada").await.unwrap(),
            Some(AvatarReference::Initials)
        );
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
    async fn a_reply_inserted_without_kind_loads_as_message() {
        let db = open_db().await;
        let viewer = author();
        save_author(&db, &viewer).await;
        let posted = post_need_with_attachments(
            &viewer,
            "grace",
            "Roof",
            "It leaked.",
            None,
            CatalogPresence::Listed,
            "church",
            Posture::Lifts,
            &[],
            "need-1".into(),
            "t0".into(),
        )
        .unwrap();
        db.apply(&posted).await.unwrap();
        db.execute(
            "INSERT INTO need_replies (id, need_id, author_id, body, created_at)
             VALUES ('reply-old', 'need-1', 'ada', 'I can help Saturday.', 't1')",
            &[],
        )
        .await
        .unwrap();
        let reply = db.need_reply("reply-old").await.unwrap().unwrap();
        assert_eq!(reply.kind, ReplyKind::Message);
        let cards = db.need_replies("need-1").await.unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].kind, ReplyKind::Message);
    }
}
