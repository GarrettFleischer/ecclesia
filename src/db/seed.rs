use super::seed_data::*;
use super::Db;
use crate::domain::now_iso;

impl Db {
    pub(crate) async fn seed_if_empty(&self) -> anyhow::Result<()> {
        if self.user_count().await? > 0 {
            return Ok(());
        }
        let now = now_iso();
        insert_gift_rows(self, GIFTS).await?;
        insert_user_rows(self, USERS, &now).await?;
        insert_church_rows(self, CHURCHES, &now).await?;
        insert_membership_rows(self, MEMBERSHIPS, &now).await?;
        insert_member_gift_rows(self, MEMBER_GIFTS).await?;
        insert_need_rows(self, NEEDS, &now).await?;
        insert_opening_stories(self, &now).await?;
        tracing::info!("seeded Cedar Falls / Waterloo demo body");
        Ok(())
    }

    async fn user_count(&self) -> anyhow::Result<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?)
    }
}

async fn insert_gift_rows(db: &Db, rows: &[(&str, &str, &str)]) -> anyhow::Result<()> {
    for row in rows {
        insert_gift_row(db, row).await?;
    }
    Ok(())
}

async fn insert_gift_row(db: &Db, row: &(&str, &str, &str)) -> anyhow::Result<()> {
    let (id, name, category) = *row;
    sqlx::query("INSERT INTO gifts (id, name, category) VALUES (?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(category)
        .execute(&db.pool)
        .await?;
    Ok(())
}

async fn insert_user_rows(
    db: &Db,
    rows: &[(&str, &str, &str, &str, &str, &str)],
    now: &str,
) -> anyhow::Result<()> {
    for row in rows {
        insert_user_row(db, row, now).await?;
    }
    Ok(())
}

async fn insert_user_row(
    db: &Db,
    row: &(&str, &str, &str, &str, &str, &str),
    now: &str,
) -> anyhow::Result<()> {
    let (id, name, email, city, region, bio) = *row;
    sqlx::query(
        "INSERT INTO users (id, name, email, city, region, bio, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(email)
    .bind(city)
    .bind(region)
    .bind(bio)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

async fn insert_church_rows(
    db: &Db,
    rows: &[(&str, &str, &str, &str, &str, &str, &str, &str)],
    now: &str,
) -> anyhow::Result<()> {
    for row in rows {
        insert_church_row(db, row, now).await?;
    }
    Ok(())
}

async fn insert_church_row(
    db: &Db,
    row: &(&str, &str, &str, &str, &str, &str, &str, &str),
    now: &str,
) -> anyhow::Result<()> {
    let (id, name, city, region, description, gathering, owner_id, invite_code) = *row;
    sqlx::query(
        "INSERT INTO churches (id, name, city, region, country, description, gathering, owner_id, invite_code, created_at)
         VALUES (?, ?, ?, ?, 'US', ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(city)
    .bind(region)
    .bind(description)
    .bind(gathering)
    .bind(owner_id)
    .bind(invite_code)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

async fn insert_membership_rows(
    db: &Db,
    rows: &[(&str, &str, &str, &str, &str)],
    now: &str,
) -> anyhow::Result<()> {
    for row in rows {
        insert_membership_row(db, row, now).await?;
    }
    Ok(())
}

async fn insert_membership_row(
    db: &Db,
    row: &(&str, &str, &str, &str, &str),
    now: &str,
) -> anyhow::Result<()> {
    let (id, church_id, user_id, role, status) = *row;
    sqlx::query(
        "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(church_id)
    .bind(user_id)
    .bind(role)
    .bind(status)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

async fn insert_member_gift_rows(db: &Db, rows: &[(&str, &str, &str)]) -> anyhow::Result<()> {
    for row in rows {
        insert_member_gift_row(db, row).await?;
    }
    Ok(())
}

async fn insert_member_gift_row(db: &Db, row: &(&str, &str, &str)) -> anyhow::Result<()> {
    let (user_id, gift_id, note) = *row;
    sqlx::query("INSERT INTO member_gifts (user_id, gift_id, note) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(gift_id)
        .bind(note)
        .execute(&db.pool)
        .await?;
    Ok(())
}

async fn insert_need_rows(
    db: &Db,
    rows: &[(&str, &str, &str, &str, &str, Option<&str>, &str, &str)],
    now: &str,
) -> anyhow::Result<()> {
    for row in rows {
        insert_need_row(db, row, now).await?;
    }
    Ok(())
}

async fn insert_need_row(
    db: &Db,
    row: &(&str, &str, &str, &str, &str, Option<&str>, &str, &str),
    now: &str,
) -> anyhow::Result<()> {
    let (id, church_id, author_id, title, body, gift_id, scope, status) = *row;
    sqlx::query(
        "INSERT INTO needs (id, church_id, author_id, title, body, gift_id, scope, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(church_id)
    .bind(author_id)
    .bind(title)
    .bind(body)
    .bind(gift_id)
    .bind(scope)
    .bind(status)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

async fn insert_opening_stories(db: &Db, now: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("app_ruth_meals")
    .bind("need_meals")
    .bind("user_ruth")
    .bind("I can do Tuesday and Friday. Chicken and rice, and I'll bring bread.")
    .bind("accepted")
    .bind(now)
    .execute(&db.pool)
    .await?;

    insert_endorsement_row(
        db,
        "end_james_ruth",
        "user_james",
        "user_ruth",
        "gift_hospitality",
        "Hospitality",
        "Ruth fed 40 of our teenagers after the flood cleanup in May and stayed until the last parent showed up.",
        "pending",
        now,
    )
    .await?;
    insert_endorsement_row(
        db,
        "end_miriam_elena",
        "user_miriam",
        "user_elena",
        "gift_counseling",
        "Counseling",
        "Elena met with a family from our neighborhood every week for two months when nobody else could get through to them.",
        "accepted",
        now,
    )
    .await?;
    insert_endorsement_row(
        db,
        "end_elena_daniel",
        "user_elena",
        "user_daniel",
        "gift_counseling",
        "Counseling",
        "Daniel sat with my cousin after surgery and didn't try to fill the silence.",
        "pending",
        now,
    )
    .await?;

    db.notify(
        "user_miriam",
        "join_request",
        "Peter Lang asked to join Grace Covenant",
        "Approve or decline from the church page.",
        "/churches/church_grace",
    )
    .await?;
    db.notify(
        "user_ruth",
        "endorsement",
        "James Whitaker endorsed you for Hospitality",
        "Accept it from your inbox, or decline.",
        "/inbox",
    )
    .await?;
    db.notify(
        "user_elena",
        "endorsement",
        "Miriam Cole endorsed you for Counseling",
        "It's on your profile.",
        "/members/user_elena",
    )
    .await?;
    db.notify(
        "user_daniel",
        "endorsement",
        "Elena Vasquez endorsed you for Counseling",
        "Accept it from your inbox, or decline.",
        "/inbox",
    )
    .await?;
    db.notify(
        "user_miriam",
        "application",
        "Ruth Alvarez offered to help with Dinners for the Okonkwo family",
        "You accepted. She's bringing Tuesday and Friday.",
        "/needs/need_meals",
    )
    .await?;
    Ok(())
}

async fn insert_endorsement_row(
    db: &Db,
    id: &str,
    from: &str,
    to: &str,
    gift_id: &str,
    skill: &str,
    note: &str,
    status: &str,
    now: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, skill, note, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(gift_id)
    .bind(skill)
    .bind(note)
    .bind(status)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}
