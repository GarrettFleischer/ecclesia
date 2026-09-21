use sqlx::{SqliteConnection, SqlitePool};

use crate::leaf::{
    Application, Church, Effect, Endorsement, Membership, Need, NoticeDraft, User, Write,
};
use crate::sdk::clock::{new_id, now_iso};

pub async fn apply(pool: &SqlitePool, effect: &Effect) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    apply_writes(&mut *tx, &effect.writes).await?;
    apply_notices(&mut *tx, &effect.notices).await?;
    tx.commit().await?;
    Ok(())
}

async fn apply_writes(conn: &mut SqliteConnection, writes: &[Write]) -> anyhow::Result<()> {
    for write in writes {
        apply_write(conn, write).await?;
    }
    Ok(())
}

async fn apply_notices(conn: &mut SqliteConnection, notices: &[NoticeDraft]) -> anyhow::Result<()> {
    for notice in notices {
        apply_notice(conn, notice).await?;
    }
    Ok(())
}

async fn apply_write(conn: &mut SqliteConnection, write: &Write) -> anyhow::Result<()> {
    match write {
        Write::InsertUser(user) => insert_user(conn, user).await,
        Write::UpdateUser {
            id,
            name,
            city,
            region,
            bio,
        } => update_user(conn, id, name, city, region, bio).await,
        Write::InsertChurch(church) => insert_church(conn, church).await,
        Write::InsertMembership(membership) => insert_membership(conn, membership).await,
        Write::SetMembershipStatus { id, status } => {
            set_status(conn, StatusTable::Memberships, id, status).await
        }
        Write::InsertNeed(need) => insert_need(conn, need).await,
        Write::SetNeedStatus { id, status } => {
            set_status(conn, StatusTable::Needs, id, status).await
        }
        Write::InsertApplication(application) => insert_application(conn, application).await,
        Write::SetApplicationStatus { id, status } => {
            set_status(conn, StatusTable::Applications, id, status).await
        }
        Write::InsertEndorsement(endorsement) => insert_endorsement(conn, endorsement).await,
        Write::SetEndorsementStatus { id, status } => {
            set_status(conn, StatusTable::Endorsements, id, status).await
        }
        Write::UpsertMemberGift {
            user_id,
            gift_id,
            note,
        } => upsert_member_gift(conn, user_id, gift_id, note).await,
        Write::RemoveMemberGift { user_id, gift_id } => {
            remove_member_gift(conn, user_id, gift_id).await
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

async fn apply_notice(conn: &mut SqliteConnection, notice: &NoticeDraft) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, body, href, read, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(new_id())
    .bind(&notice.user_id)
    .bind(notice.kind)
    .bind(notice.title.as_ref())
    .bind(notice.body)
    .bind(&notice.href)
    .bind(now_iso())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_user(conn: &mut SqliteConnection, user: &User) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO users (id, name, email, city, region, bio, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&user.id)
    .bind(&user.name)
    .bind(&user.email)
    .bind(&user.city)
    .bind(&user.region)
    .bind(&user.bio)
    .bind(&user.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn update_user(
    conn: &mut SqliteConnection,
    id: &str,
    name: &str,
    city: &str,
    region: &str,
    bio: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET name = ?, city = ?, region = ?, bio = ? WHERE id = ?")
        .bind(name.trim())
        .bind(city.trim())
        .bind(region.trim())
        .bind(bio.trim())
        .bind(id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn insert_church(conn: &mut SqliteConnection, church: &Church) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO churches (id, name, city, region, country, description, gathering, owner_id, invite_code, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&church.id)
    .bind(&church.name)
    .bind(&church.city)
    .bind(&church.region)
    .bind(&church.country)
    .bind(&church.description)
    .bind(&church.gathering)
    .bind(&church.owner_id)
    .bind(&church.invite_code)
    .bind(&church.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_membership(
    conn: &mut SqliteConnection,
    membership: &Membership,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&membership.id)
    .bind(&membership.church_id)
    .bind(&membership.user_id)
    .bind(&membership.role)
    .bind(&membership.status)
    .bind(&membership.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_need(conn: &mut SqliteConnection, need: &Need) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO needs (id, church_id, author_id, title, body, gift_id, scope, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&need.id)
    .bind(&need.church_id)
    .bind(&need.author_id)
    .bind(&need.title)
    .bind(&need.body)
    .bind(&need.gift_id)
    .bind(&need.scope)
    .bind(&need.status)
    .bind(&need.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_application(
    conn: &mut SqliteConnection,
    application: &Application,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&application.id)
    .bind(&application.need_id)
    .bind(&application.user_id)
    .bind(&application.message)
    .bind(&application.status)
    .bind(&application.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_endorsement(
    conn: &mut SqliteConnection,
    endorsement: &Endorsement,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, skill, note, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&endorsement.id)
    .bind(&endorsement.from_user_id)
    .bind(&endorsement.to_user_id)
    .bind(&endorsement.gift_id)
    .bind(&endorsement.skill)
    .bind(&endorsement.note)
    .bind(&endorsement.status)
    .bind(&endorsement.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn set_status(
    conn: &mut SqliteConnection,
    table: StatusTable,
    id: &str,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query(table.update_sql())
        .bind(status)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn upsert_member_gift(
    conn: &mut SqliteConnection,
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
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn remove_member_gift(
    conn: &mut SqliteConnection,
    user_id: &str,
    gift_id: &str,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM member_gifts WHERE user_id = ? AND gift_id = ?")
        .bind(user_id)
        .bind(gift_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
