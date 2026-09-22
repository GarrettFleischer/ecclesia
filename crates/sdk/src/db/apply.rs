use sqlx::{PgPool, SqlitePool};

use ecclesia_domain::{
    Application, Church, Effect, Endorsement, Membership, Need, NoticeDraft, User, Write,
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
    async fn fetch_text(&mut self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<Option<String>>;
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
            };
        }
        query.execute(&mut **self.0).await?;
        Ok(())
    }

    async fn fetch_text(&mut self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<Option<String>> {
        let mut query = sqlx::query_scalar::<sqlx::Sqlite, String>(sql);
        for bind in binds {
            query = match *bind {
                Bind::Text(value) => query.bind(value),
                Bind::OptText(value) => query.bind(value),
                Bind::I64(value) => query.bind(value),
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
            };
        }
        query.execute(&mut **self.0).await?;
        Ok(())
    }

    async fn fetch_text(&mut self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<Option<String>> {
        let sql = Driver::Postgres.sql(sql);
        let mut query = sqlx::query_scalar::<sqlx::Postgres, String>(sql.as_ref());
        for bind in binds {
            query = match *bind {
                Bind::Text(value) => query.bind(value),
                Bind::OptText(value) => query.bind(value),
                Bind::I64(value) => query.bind(value),
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
        exec.exec("DELETE FROM sessions WHERE user_id = ?", &[Bind::Text(user_id)])
            .await?;
    }
    if let Some(id) = extras.delete_session_id.as_deref() {
        exec.exec("DELETE FROM sessions WHERE id = ?", &[Bind::Text(id)])
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
            "INSERT INTO sessions (id, user_id, csrf, created_at, last_seen_at, user_agent, ip)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(&session.id),
                Bind::Text(&session.user_id),
                Bind::Text(&session.csrf),
                Bind::Text(&session.created_at),
                Bind::Text(&session.last_seen_at),
                Bind::Text(&session.user_agent),
                Bind::Text(&session.ip),
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
    if let Some(mail) = extras.mail.as_ref() {
        insert_mail(exec, mail).await?;
    }
    Ok(())
}

async fn consume_token_row(exec: &mut impl Exec, table: &str, id: &str) -> anyhow::Result<()> {
    let now = now_iso();
    let sql = format!("UPDATE {table} SET consumed_at = ? WHERE id = ?");
    exec.exec(&sql, &[Bind::Text(&now), Bind::Text(id)]).await
}

async fn retire_tokens(exec: &mut impl Exec, table: &str, user_id: &str) -> anyhow::Result<()> {
    let now = now_iso();
    let sql = format!(
        "UPDATE {table} SET consumed_at = ? WHERE user_id = ? AND consumed_at IS NULL"
    );
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
        Write::SetMembershipStatus { id, .. } => exec
            .fetch_text("SELECT church_id FROM memberships WHERE id = ?", &[Bind::Text(id)])
            .await,
        Write::SetNeedStatus { id, .. } => exec
            .fetch_text("SELECT church_id FROM needs WHERE id = ?", &[Bind::Text(id)])
            .await,
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
            name,
            city,
            region,
            bio,
        } => update_user(exec, id, name, city, region, bio).await,
        Write::InsertChurch(church) => insert_church(exec, church).await,
        Write::InsertMembership(membership) => insert_membership(exec, membership).await,
        Write::SetMembershipStatus { id, status } => {
            set_status(exec, StatusTable::Memberships, id, status).await
        }
        Write::InsertNeed(need) => insert_need(exec, need).await,
        Write::SetNeedStatus { id, status } => set_status(exec, StatusTable::Needs, id, status).await,
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
    }
}

enum StatusTable {
    Memberships,
    Needs,
    Applications,
    Endorsements,
}

impl StatusTable {
    fn update_sql(self) -> &'static str {
        match self {
            Self::Memberships => "UPDATE memberships SET status = ? WHERE id = ?",
            Self::Needs => "UPDATE needs SET status = ? WHERE id = ?",
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
        "INSERT INTO users (id, name, email, city, region, bio, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&user.id),
            Bind::Text(&user.name),
            Bind::Text(&user.email),
            Bind::Text(&user.city),
            Bind::Text(&user.region),
            Bind::Text(&user.bio),
            Bind::Text(&user.created_at),
        ],
    )
    .await
}

async fn update_user(
    exec: &mut impl Exec,
    id: &str,
    name: &str,
    city: &str,
    region: &str,
    bio: &str,
) -> anyhow::Result<()> {
    exec.exec(
        "UPDATE users SET name = ?, city = ?, region = ?, bio = ? WHERE id = ?",
        &[
            Bind::Text(name.trim()),
            Bind::Text(city.trim()),
            Bind::Text(region.trim()),
            Bind::Text(bio.trim()),
            Bind::Text(id),
        ],
    )
    .await
}

async fn insert_church(exec: &mut impl Exec, church: &Church) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO churches (id, name, city, region, country, description, gathering, owner_id, invite_code, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&church.id),
            Bind::Text(&church.name),
            Bind::Text(&church.city),
            Bind::Text(&church.region),
            Bind::Text(&church.country),
            Bind::Text(&church.description),
            Bind::Text(&church.gathering),
            Bind::Text(&church.owner_id),
            Bind::Text(&church.invite_code),
            Bind::Text(&church.created_at),
        ],
    )
    .await
}

async fn insert_membership(exec: &mut impl Exec, membership: &Membership) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        &[
            Bind::Text(&membership.id),
            Bind::Text(&membership.church_id),
            Bind::Text(&membership.user_id),
            Bind::Text(&membership.role),
            Bind::Text(&membership.status),
            Bind::Text(&membership.created_at),
        ],
    )
    .await
}

async fn insert_need(exec: &mut impl Exec, need: &Need) -> anyhow::Result<()> {
    exec.exec(
        "INSERT INTO needs (id, church_id, author_id, title, body, gift_id, scope, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
         ON CONFLICT(user_id, gift_id) DO UPDATE SET note = excluded.note",
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
    exec.exec(
        "DELETE FROM member_gifts WHERE user_id = ? AND gift_id = ?",
        &[Bind::Text(user_id), Bind::Text(gift_id)],
    )
    .await
}
