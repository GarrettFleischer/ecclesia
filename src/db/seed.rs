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
    .bind("I can cover Tuesday and Friday. Chicken and rice, nothing fancy.")
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
        "Ruth fed our youth after the flood cleanup and stayed until the last parent came. That is the gift.",
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
        "Elena sat with a family we could not reach. She did not make them become us first.",
        "accepted",
        now,
    )
    .await?;

    db.notify(
        "user_miriam",
        "join_request",
        "Peter Lang asked to join Grace Covenant",
        "He is new in Cedar Falls. He named transportation and childcare as how he can serve.",
        "/churches/church_grace",
    )
    .await?;
    db.notify(
        "user_ruth",
        "endorsement",
        "James Whitaker endorsed you for Hospitality",
        "You can accept this onto your profile, or let it go.",
        "/inbox",
    )
    .await?;
    db.notify(
        "user_elena",
        "endorsement",
        "Miriam Cole endorsed you for Counseling",
        "You accepted this. It now lives on your profile.",
        "/members/user_elena",
    )
    .await?;
    db.notify(
        "user_miriam",
        "application",
        "Ruth Alvarez offered meals",
        "Tuesday and Friday. You already received her into this need.",
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
    note: &str,
    status: &str,
    now: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, note, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(gift_id)
    .bind(note)
    .bind(status)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

const GIFTS: &[(&str, &str, &str)] = &[
    ("gift_teaching", "Teaching", "spiritual"),
    ("gift_encouragement", "Encouragement", "spiritual"),
    ("gift_hospitality", "Hospitality", "spiritual"),
    ("gift_intercession", "Intercession", "spiritual"),
    ("gift_wisdom", "Wisdom", "spiritual"),
    ("gift_mercy", "Mercy", "spiritual"),
    ("gift_discernment", "Discernment", "spiritual"),
    ("gift_shepherding", "Shepherding", "pastoral"),
    ("gift_counseling", "Counseling", "pastoral"),
    ("gift_visitation", "Visitation", "pastoral"),
    ("gift_mentoring", "Mentoring", "pastoral"),
    ("gift_meals", "Meals", "practical"),
    ("gift_carpentry", "Carpentry & repairs", "practical"),
    ("gift_childcare", "Childcare", "practical"),
    ("gift_transport", "Transportation", "practical"),
    ("gift_medical", "Medical care", "practical"),
    ("gift_accounting", "Accounting", "practical"),
    ("gift_music", "Music", "creative"),
    ("gift_worship", "Worship leading", "creative"),
    ("gift_art", "Art", "creative"),
    ("gift_writing", "Writing", "creative"),
    ("gift_admin", "Administration", "logistical"),
    ("gift_translation", "Translation", "logistical"),
    ("gift_tech", "Technology", "logistical"),
];

const USERS: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "user_miriam",
        "Miriam Cole",
        "miriam@gracecovenant.test",
        "Cedar Falls",
        "Iowa",
        "Pastor of Grace Covenant. I keep watch so the flock can actually live as a body, not a Sunday audience.",
    ),
    (
        "user_daniel",
        "Daniel Okonkwo",
        "daniel@gracecovenant.test",
        "Cedar Falls",
        "Iowa",
        "Recovering from surgery and learning how to receive help without apology.",
    ),
    (
        "user_ruth",
        "Ruth Alvarez",
        "ruth@gracecovenant.test",
        "Cedar Falls",
        "Iowa",
        "I cook when people are tired. Tables are how I pray.",
    ),
    (
        "user_samuel",
        "Samuel Wright",
        "samuel@stlukes.test",
        "Cedar Falls",
        "Iowa",
        "Rector at St. Luke's. We would rather share a need than pretend we are sufficient.",
    ),
    (
        "user_james",
        "James Whitaker",
        "james@stlukes.test",
        "Cedar Falls",
        "Iowa",
        "Carpenter. If it is broken and a person has to live with it, call me.",
    ),
    (
        "user_keisha",
        "Keisha Brooks",
        "keisha@newmercy.test",
        "Waterloo",
        "Iowa",
        "Pastor at New Mercy. Waterloo and Cedar Falls are one valley. The body should act like it.",
    ),
    (
        "user_elena",
        "Elena Vasquez",
        "elena@newmercy.test",
        "Waterloo",
        "Iowa",
        "Bilingual counselor. I can sit with someone in Spanish or English and not rush them.",
    ),
    (
        "user_peter",
        "Peter Lang",
        "peter@cedarfalls.test",
        "Cedar Falls",
        "Iowa",
        "New in town. I have a van and Saturday mornings. Still looking for a church that will have me.",
    ),
];

const CHURCHES: &[(&str, &str, &str, &str, &str, &str, &str, &str)] = &[
    (
        "church_grace",
        "Grace Covenant Church",
        "Cedar Falls",
        "Iowa",
        "A neighborhood church that wants its gifts in motion during the week, not only listed in a bulletin.",
        "Sundays 10:00 a.m.",
        "user_miriam",
        "grace-k2m9",
    ),
    (
        "church_luke",
        "St. Luke's Fellowship",
        "Cedar Falls",
        "Iowa",
        "An older parish learning to ask neighboring churches for help instead of quietly running out of hands.",
        "Sundays 9:00 a.m.",
        "user_samuel",
        "luke-p4r1",
    ),
    (
        "church_mercy",
        "New Mercy Community",
        "Waterloo",
        "Iowa",
        "A storefront church across the river. We plant, cook, translate, and refuse to be an island.",
        "Sundays 11:00 a.m.",
        "user_keisha",
        "mercy-n8q2",
    ),
];

const MEMBERSHIPS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "mem_grace_miriam",
        "church_grace",
        "user_miriam",
        "owner",
        "active",
    ),
    (
        "mem_grace_daniel",
        "church_grace",
        "user_daniel",
        "member",
        "active",
    ),
    (
        "mem_grace_ruth",
        "church_grace",
        "user_ruth",
        "member",
        "active",
    ),
    (
        "mem_grace_peter",
        "church_grace",
        "user_peter",
        "member",
        "pending_request",
    ),
    (
        "mem_luke_samuel",
        "church_luke",
        "user_samuel",
        "owner",
        "active",
    ),
    (
        "mem_luke_james",
        "church_luke",
        "user_james",
        "steward",
        "active",
    ),
    (
        "mem_mercy_keisha",
        "church_mercy",
        "user_keisha",
        "owner",
        "active",
    ),
    (
        "mem_mercy_elena",
        "church_mercy",
        "user_elena",
        "member",
        "active",
    ),
];

const MEMBER_GIFTS: &[(&str, &str, &str)] = &[
    (
        "user_miriam",
        "gift_shepherding",
        "Walking with people who are tired of performing church.",
    ),
    (
        "user_miriam",
        "gift_wisdom",
        "I listen a long time before I speak.",
    ),
    (
        "user_daniel",
        "gift_encouragement",
        "I write notes. I remember names.",
    ),
    (
        "user_daniel",
        "gift_intercession",
        "Especially in the night when I cannot sleep anyway.",
    ),
    (
        "user_ruth",
        "gift_meals",
        "Casseroles, caldo, and a clean kitchen afterward.",
    ),
    (
        "user_ruth",
        "gift_hospitality",
        "My table has two extra chairs on purpose.",
    ),
    (
        "user_samuel",
        "gift_teaching",
        "Lectionary preaching and quiet midweek study.",
    ),
    (
        "user_samuel",
        "gift_visitation",
        "Hospitals and kitchen tables.",
    ),
    (
        "user_james",
        "gift_carpentry",
        "Ramps, doors, leaky sinks, pews that wobble.",
    ),
    ("user_james", "gift_transport", "Truck and a free Thursday."),
    (
        "user_keisha",
        "gift_shepherding",
        "I will not let a family disappear after a hard month.",
    ),
    (
        "user_keisha",
        "gift_admin",
        "I can stand up a meal train before noon.",
    ),
    (
        "user_elena",
        "gift_translation",
        "Spanish and English, including medical words.",
    ),
    (
        "user_elena",
        "gift_counseling",
        "Grief, marriage, and the quiet panic of new immigrants.",
    ),
    (
        "user_peter",
        "gift_transport",
        "A van that fits a family plus boxes.",
    ),
    (
        "user_peter",
        "gift_childcare",
        "Two of my own. I am calm in a noisy room.",
    ),
];

const NEEDS: &[(&str, &str, &str, &str, &str, Option<&str>, &str, &str)] = &[
    (
        "need_meals",
        "church_grace",
        "user_miriam",
        "Meal train for the Okonkwo family",
        "Daniel is two weeks out of surgery. Ruth has already cooked twice. We need five more dinners this week — drop-off at the side door, no lingering required unless they ask.",
        Some("gift_meals"),
        "church",
        "open",
    ),
    (
        "need_ramp",
        "church_grace",
        "user_daniel",
        "Temporary ramp and a bathroom grab bar",
        "I can almost get into the house. Almost is how people fall. If you have a saw and an hour on Saturday, I will stay out of your way.",
        Some("gift_carpentry"),
        "neighboring",
        "open",
    ),
    (
        "need_spanish",
        "church_grace",
        "user_miriam",
        "Spanish interpreter for Thursday counseling",
        "A couple from our neighborhood asked for prayer and counsel. I need someone who can hold both languages without turning it into a spectacle.",
        Some("gift_translation"),
        "neighboring",
        "open",
    ),
    (
        "need_prayer",
        "church_mercy",
        "user_keisha",
        "Prayer covering for two church-planting families",
        "They are tired and far from their sending churches. This is open to the whole body — a name, a time, and you keep it.",
        Some("gift_intercession"),
        "body",
        "open",
    ),
    (
        "need_worship",
        "church_luke",
        "user_samuel",
        "Someone to lead sung prayer on the 29th",
        "Our musicians are at a retreat. We would rather borrow a neighbor than press play on a laptop.",
        Some("gift_worship"),
        "neighboring",
        "open",
    ),
];
