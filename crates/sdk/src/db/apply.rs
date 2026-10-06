use sqlx::{PgPool, SqlitePool};

use ecclesia_domain::{
    Application, Church, Effect, Endorsement, Need, NeedReply, NoticeDraft, Prayer, Share, User,
    Write,
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
        Write::MoveNeed { id, .. } | Write::SetNeedStatus { id, .. } => {
            exec.fetch_text(
                "SELECT church_id FROM needs WHERE id = ?",
                &[Bind::Text(id)],
            )
            .await
        }
        _ => Ok(None),
    }
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
        "INSERT INTO need_replies (id, need_id, author_id, body, created_at) VALUES (?, ?, ?, ?, ?)",
        &[
            Bind::Text(&reply.id),
            Bind::Text(&reply.need_id),
            Bind::Text(&reply.author_id),
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
