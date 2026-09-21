use sqlx::SqlitePool;

use crate::leaf::{
    Application, Church, Effect, Endorsement, Membership, Need, NoticeDraft, User, Write,
};
use crate::sdk::clock::{new_id, now_iso};

pub async fn apply(pool: &SqlitePool, effect: &Effect) -> anyhow::Result<()> {
    apply_writes(pool, &effect.writes).await?;
    apply_notices(pool, &effect.notices).await?;
    Ok(())
}

async fn apply_writes(pool: &SqlitePool, writes: &[Write]) -> anyhow::Result<()> {
    for write in writes {
        apply_write(pool, write).await?;
    }
    Ok(())
}

async fn apply_notices(pool: &SqlitePool, notices: &[NoticeDraft]) -> anyhow::Result<()> {
    for notice in notices {
        apply_notice(pool, notice).await?;
    }
    Ok(())
}

async fn apply_write(pool: &SqlitePool, write: &Write) -> anyhow::Result<()> {
    match write {
        Write::InsertUser(user) => insert_user(pool, user).await,
        Write::UpdateUser {
            id,
            name,
            city,
            region,
            bio,
        } => update_user(pool, id, name, city, region, bio).await,
        Write::InsertChurch(church) => insert_church(pool, church).await,
        Write::InsertMembership(membership) => insert_membership(pool, membership).await,
        Write::SetMembershipStatus { id, status } => {
            set_status(pool, "memberships", id, status).await
        }
        Write::InsertNeed(need) => insert_need(pool, need).await,
        Write::SetNeedStatus { id, status } => set_status(pool, "needs", id, status).await,
        Write::InsertApplication(application) => insert_application(pool, application).await,
        Write::SetApplicationStatus { id, status } => {
            set_status(pool, "applications", id, status).await
        }
        Write::InsertEndorsement(endorsement) => insert_endorsement(pool, endorsement).await,
        Write::SetEndorsementStatus { id, status } => {
            set_status(pool, "endorsements", id, status).await
        }
        Write::UpsertMemberGift {
            user_id,
            gift_id,
            note,
        } => upsert_member_gift(pool, user_id, gift_id, note).await,
        Write::RemoveMemberGift { user_id, gift_id } => {
            remove_member_gift(pool, user_id, gift_id).await
        }
    }
}

async fn apply_notice(pool: &SqlitePool, notice: &NoticeDraft) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, body, href, read, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(new_id())
    .bind(&notice.user_id)
    .bind(notice.kind)
    .bind(&notice.title)
    .bind(notice.body)
    .bind(&notice.href)
    .bind(now_iso())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_user(pool: &SqlitePool, user: &User) -> anyhow::Result<()> {
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
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_user(
    pool: &SqlitePool,
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
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_church(pool: &SqlitePool, church: &Church) -> anyhow::Result<()> {
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
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_membership(pool: &SqlitePool, membership: &Membership) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&membership.id)
    .bind(&membership.church_id)
    .bind(&membership.user_id)
    .bind(&membership.role)
    .bind(&membership.status)
    .bind(&membership.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_need(pool: &SqlitePool, need: &Need) -> anyhow::Result<()> {
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
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_application(pool: &SqlitePool, application: &Application) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&application.id)
    .bind(&application.need_id)
    .bind(&application.user_id)
    .bind(&application.message)
    .bind(&application.status)
    .bind(&application.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_endorsement(pool: &SqlitePool, endorsement: &Endorsement) -> anyhow::Result<()> {
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
    .execute(pool)
    .await?;
    Ok(())
}

async fn set_status(pool: &SqlitePool, table: &str, id: &str, status: &str) -> anyhow::Result<()> {
    let sql = match table {
        "memberships" => "UPDATE memberships SET status = ? WHERE id = ?",
        "needs" => "UPDATE needs SET status = ? WHERE id = ?",
        "applications" => "UPDATE applications SET status = ? WHERE id = ?",
        "endorsements" => "UPDATE endorsements SET status = ? WHERE id = ?",
        _ => return Ok(()),
    };
    sqlx::query(sql).bind(status).bind(id).execute(pool).await?;
    Ok(())
}

async fn upsert_member_gift(
    pool: &SqlitePool,
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
    .execute(pool)
    .await?;
    Ok(())
}

async fn remove_member_gift(pool: &SqlitePool, user_id: &str, gift_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM member_gifts WHERE user_id = ? AND gift_id = ?")
        .bind(user_id)
        .bind(gift_id)
        .execute(pool)
        .await?;
    Ok(())
}
