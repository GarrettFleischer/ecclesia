use anyhow::Context;
use sqlx::{
    sqlite::SqliteConnectOptions, sqlite::SqliteJournalMode, sqlite::SqlitePoolOptions, Row,
    SqlitePool,
};
use std::str::FromStr;

use crate::domain::{
    new_id, now_iso, Application, ApplicationCard, Church, ChurchMember, Effect, Endorsement,
    EndorsementCard, Gift, MemberGift, Membership, Need, NeedCard, Notification, User, Write,
};

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await
            .with_context(|| format!("connecting to {url}"))?;
        let db = Self { pool };
        db.migrate().await?;
        db.seed_if_empty().await?;
        Ok(db)
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                city TEXT NOT NULL,
                region TEXT NOT NULL,
                bio TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS churches (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                city TEXT NOT NULL,
                region TEXT NOT NULL,
                country TEXT NOT NULL DEFAULT 'US',
                description TEXT NOT NULL,
                gathering TEXT NOT NULL DEFAULT '',
                owner_id TEXT NOT NULL,
                invite_code TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memberships (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(church_id, user_id)
            );
            CREATE TABLE IF NOT EXISTS gifts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                category TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS member_gifts (
                user_id TEXT NOT NULL,
                gift_id TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (user_id, gift_id)
            );
            CREATE TABLE IF NOT EXISTS needs (
                id TEXT PRIMARY KEY,
                church_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                gift_id TEXT,
                scope TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS applications (
                id TEXT PRIMARY KEY,
                need_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                message TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(need_id, user_id)
            );
            CREATE TABLE IF NOT EXISTS endorsements (
                id TEXT PRIMARY KEY,
                from_user_id TEXT NOT NULL,
                to_user_id TEXT NOT NULL,
                gift_id TEXT NOT NULL,
                note TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                href TEXT NOT NULL DEFAULT '/',
                read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn seed_if_empty(&self) -> anyhow::Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            return Ok(());
        }

        let gifts = [
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
        for (id, name, category) in gifts {
            sqlx::query("INSERT INTO gifts (id, name, category) VALUES (?, ?, ?)")
                .bind(id)
                .bind(name)
                .bind(category)
                .execute(&self.pool)
                .await?;
        }

        let now = now_iso();
        let users = [
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
        for (id, name, email, city, region, bio) in users {
            sqlx::query(
                "INSERT INTO users (id, name, email, city, region, bio, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(email)
            .bind(city)
            .bind(region)
            .bind(bio)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let churches = [
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
        for (id, name, city, region, description, gathering, owner_id, invite_code) in churches {
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
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let memberships = [
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
        for (id, church_id, user_id, role, status) in memberships {
            sqlx::query(
                "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(church_id)
            .bind(user_id)
            .bind(role)
            .bind(status)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        let member_gifts = [
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
        for (user_id, gift_id, note) in member_gifts {
            sqlx::query("INSERT INTO member_gifts (user_id, gift_id, note) VALUES (?, ?, ?)")
                .bind(user_id)
                .bind(gift_id)
                .bind(note)
                .execute(&self.pool)
                .await?;
        }

        let needs = [
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
        for (id, church_id, author_id, title, body, gift_id, scope, status) in needs {
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
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("app_ruth_meals")
        .bind("need_meals")
        .bind("user_ruth")
        .bind("I can cover Tuesday and Friday. Chicken and rice, nothing fancy.")
        .bind("accepted")
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, note, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("end_james_ruth")
        .bind("user_james")
        .bind("user_ruth")
        .bind("gift_hospitality")
        .bind("Ruth fed our youth after the flood cleanup and stayed until the last parent came. That is the gift.")
        .bind("pending")
        .bind(&now)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, note, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("end_miriam_elena")
        .bind("user_miriam")
        .bind("user_elena")
        .bind("gift_counseling")
        .bind("Elena sat with a family we could not reach. She did not make them become us first.")
        .bind("accepted")
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.notify(
            "user_miriam",
            "join_request",
            "Peter Lang asked to join Grace Covenant",
            "He is new in Cedar Falls. He named transportation and childcare as how he can serve.",
            "/churches/church_grace",
        )
        .await?;
        self.notify(
            "user_ruth",
            "endorsement",
            "James Whitaker endorsed you for Hospitality",
            "You can accept this onto your profile, or let it go.",
            "/inbox",
        )
        .await?;
        self.notify(
            "user_elena",
            "endorsement",
            "Miriam Cole endorsed you for Counseling",
            "You accepted this. It now lives on your profile.",
            "/members/user_elena",
        )
        .await?;
        self.notify(
            "user_miriam",
            "application",
            "Ruth Alvarez offered meals",
            "Tuesday and Friday. You already received her into this need.",
            "/needs/need_meals",
        )
        .await?;

        tracing::info!("seeded Cedar Falls / Waterloo demo body");
        Ok(())
    }

    pub async fn user(&self, id: &str) -> anyhow::Result<Option<User>> {
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE lower(email) = lower(?)")
                .bind(email)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn demo_users(&self) -> anyhow::Result<Vec<User>> {
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE id LIKE 'user_%' ORDER BY name")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        city: &str,
        region: &str,
        bio: &str,
    ) -> anyhow::Result<User> {
        let user = User {
            id: new_id(),
            name: name.trim().to_string(),
            email: email.trim().to_string(),
            city: city.trim().to_string(),
            region: region.trim().to_string(),
            bio: bio.trim().to_string(),
            created_at: now_iso(),
        };
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
        .execute(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn update_user(
        &self,
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
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn churches(&self) -> anyhow::Result<Vec<Church>> {
        Ok(
            sqlx::query_as::<_, Church>("SELECT * FROM churches ORDER BY city, name")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn church(&self, id: &str) -> anyhow::Result<Option<Church>> {
        Ok(
            sqlx::query_as::<_, Church>("SELECT * FROM churches WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn church_by_invite(&self, code: &str) -> anyhow::Result<Option<Church>> {
        Ok(sqlx::query_as::<_, Church>(
            "SELECT * FROM churches WHERE lower(invite_code) = lower(?)",
        )
        .bind(code.trim())
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_church(
        &self,
        owner: &User,
        name: &str,
        city: &str,
        region: &str,
        description: &str,
        gathering: &str,
        invite_code: &str,
    ) -> anyhow::Result<Church> {
        let church = Church {
            id: new_id(),
            name: name.trim().to_string(),
            city: city.trim().to_string(),
            region: region.trim().to_string(),
            country: "US".into(),
            description: description.trim().to_string(),
            gathering: gathering.trim().to_string(),
            owner_id: owner.id.clone(),
            invite_code: invite_code.to_string(),
            created_at: now_iso(),
        };
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
        .execute(&self.pool)
        .await?;
        self.insert_membership(&church.id, &owner.id, "owner", "active")
            .await?;
        Ok(church)
    }

    pub async fn memberships_for_user(&self, user_id: &str) -> anyhow::Result<Vec<Membership>> {
        Ok(sqlx::query_as::<_, Membership>(
            "SELECT * FROM memberships WHERE user_id = ? ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn churches_for_user(&self, user_id: &str) -> anyhow::Result<Vec<Church>> {
        Ok(sqlx::query_as::<_, Church>(
            r#"
            SELECT c.* FROM churches c
            JOIN memberships m ON m.church_id = c.id
            WHERE m.user_id = ?
            ORDER BY c.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn membership(&self, id: &str) -> anyhow::Result<Option<Membership>> {
        Ok(
            sqlx::query_as::<_, Membership>("SELECT * FROM memberships WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn membership_pair(
        &self,
        church_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<Membership>> {
        Ok(sqlx::query_as::<_, Membership>(
            "SELECT * FROM memberships WHERE church_id = ? AND user_id = ?",
        )
        .bind(church_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn insert_membership(
        &self,
        church_id: &str,
        user_id: &str,
        role: &str,
        status: &str,
    ) -> anyhow::Result<Membership> {
        let membership = Membership {
            id: new_id(),
            church_id: church_id.into(),
            user_id: user_id.into(),
            role: role.into(),
            status: status.into(),
            created_at: now_iso(),
        };
        sqlx::query(
            "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&membership.id)
        .bind(&membership.church_id)
        .bind(&membership.user_id)
        .bind(&membership.role)
        .bind(&membership.status)
        .bind(&membership.created_at)
        .execute(&self.pool)
        .await?;
        Ok(membership)
    }

    pub async fn set_membership_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE memberships SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn church_members(&self, church_id: &str) -> anyhow::Result<Vec<ChurchMember>> {
        Ok(sqlx::query_as::<_, ChurchMember>(
            r#"
            SELECT m.id AS membership_id, u.id AS user_id, u.name, u.city, m.role, m.status
            FROM memberships m
            JOIN users u ON u.id = m.user_id
            WHERE m.church_id = ?
            ORDER BY
                CASE m.status WHEN 'pending_request' THEN 0 WHEN 'pending_invite' THEN 1 WHEN 'active' THEN 2 ELSE 3 END,
                CASE m.role WHEN 'owner' THEN 0 WHEN 'steward' THEN 1 ELSE 2 END,
                u.name
            "#,
        )
        .bind(church_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn governors(&self, church_id: &str) -> anyhow::Result<Vec<User>> {
        Ok(sqlx::query_as::<_, User>(
            r#"
            SELECT u.* FROM users u
            JOIN memberships m ON m.user_id = u.id
            WHERE m.church_id = ? AND m.status = 'active' AND m.role IN ('owner', 'steward')
            "#,
        )
        .bind(church_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn gifts(&self) -> anyhow::Result<Vec<Gift>> {
        Ok(
            sqlx::query_as::<_, Gift>("SELECT * FROM gifts ORDER BY category, name")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn gift(&self, id: &str) -> anyhow::Result<Option<Gift>> {
        Ok(
            sqlx::query_as::<_, Gift>("SELECT * FROM gifts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn member_gifts(&self, user_id: &str) -> anyhow::Result<Vec<MemberGift>> {
        Ok(sqlx::query_as::<_, MemberGift>(
            r#"
            SELECT mg.user_id, mg.gift_id, mg.note, g.name AS gift_name, g.category
            FROM member_gifts mg
            JOIN gifts g ON g.id = mg.gift_id
            WHERE mg.user_id = ?
            ORDER BY g.category, g.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn gift_ids_for(&self, user_id: &str) -> anyhow::Result<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT gift_id FROM member_gifts WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn add_member_gift(
        &self,
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
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_member_gift(&self, user_id: &str, gift_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM member_gifts WHERE user_id = ? AND gift_id = ?")
            .bind(user_id)
            .bind(gift_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn need(&self, id: &str) -> anyhow::Result<Option<Need>> {
        Ok(
            sqlx::query_as::<_, Need>("SELECT * FROM needs WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn need_card(&self, id: &str) -> anyhow::Result<Option<NeedCard>> {
        let sql = need_card_sql("n.id = ?");
        Ok(sqlx::query_as::<_, NeedCard>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn all_need_cards(&self) -> anyhow::Result<Vec<NeedCard>> {
        let sql = need_card_sql("1 = 1");
        Ok(sqlx::query_as::<_, NeedCard>(&sql)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn church_need_cards(&self, church_id: &str) -> anyhow::Result<Vec<NeedCard>> {
        let sql = need_card_sql("n.church_id = ?");
        Ok(sqlx::query_as::<_, NeedCard>(&sql)
            .bind(church_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn create_need(
        &self,
        church_id: &str,
        author_id: &str,
        title: &str,
        body: &str,
        gift_id: Option<&str>,
        scope: &str,
    ) -> anyhow::Result<Need> {
        let need = Need {
            id: new_id(),
            church_id: church_id.into(),
            author_id: author_id.into(),
            title: title.trim().into(),
            body: body.trim().into(),
            gift_id: gift_id.map(ToOwned::to_owned),
            scope: scope.into(),
            status: "open".into(),
            created_at: now_iso(),
        };
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
        .execute(&self.pool)
        .await?;
        Ok(need)
    }

    pub async fn set_need_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE needs SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn applications_for_need(
        &self,
        need_id: &str,
    ) -> anyhow::Result<Vec<ApplicationCard>> {
        Ok(sqlx::query_as::<_, ApplicationCard>(
            r#"
            SELECT a.id, a.need_id, a.user_id, u.name AS user_name, a.message, a.status, a.created_at
            FROM applications a
            JOIN users u ON u.id = a.user_id
            WHERE a.need_id = ?
            ORDER BY a.created_at
            "#,
        )
        .bind(need_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn application(&self, id: &str) -> anyhow::Result<Option<Application>> {
        Ok(
            sqlx::query_as::<_, Application>("SELECT * FROM applications WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn application_pair(
        &self,
        need_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<Application>> {
        Ok(sqlx::query_as::<_, Application>(
            "SELECT * FROM applications WHERE need_id = ? AND user_id = ?",
        )
        .bind(need_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_application(
        &self,
        need_id: &str,
        user_id: &str,
        message: &str,
    ) -> anyhow::Result<Application> {
        let application = Application {
            id: new_id(),
            need_id: need_id.into(),
            user_id: user_id.into(),
            message: message.trim().into(),
            status: "pending".into(),
            created_at: now_iso(),
        };
        sqlx::query(
            "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&application.id)
        .bind(&application.need_id)
        .bind(&application.user_id)
        .bind(&application.message)
        .bind(&application.status)
        .bind(&application.created_at)
        .execute(&self.pool)
        .await?;
        Ok(application)
    }

    pub async fn set_application_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE applications SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn pending_endorsement(
        &self,
        from: &str,
        to: &str,
        gift_id: &str,
    ) -> anyhow::Result<Option<Endorsement>> {
        Ok(sqlx::query_as::<_, Endorsement>(
            "SELECT * FROM endorsements WHERE from_user_id = ? AND to_user_id = ? AND gift_id = ? AND status = 'pending'",
        )
        .bind(from)
        .bind(to)
        .bind(gift_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_endorsement(
        &self,
        from: &str,
        to: &str,
        gift_id: &str,
        note: &str,
    ) -> anyhow::Result<Endorsement> {
        let endorsement = Endorsement {
            id: new_id(),
            from_user_id: from.into(),
            to_user_id: to.into(),
            gift_id: gift_id.into(),
            note: note.trim().into(),
            status: "pending".into(),
            created_at: now_iso(),
        };
        sqlx::query(
            "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, note, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&endorsement.id)
        .bind(&endorsement.from_user_id)
        .bind(&endorsement.to_user_id)
        .bind(&endorsement.gift_id)
        .bind(&endorsement.note)
        .bind(&endorsement.status)
        .bind(&endorsement.created_at)
        .execute(&self.pool)
        .await?;
        Ok(endorsement)
    }

    pub async fn endorsement(&self, id: &str) -> anyhow::Result<Option<Endorsement>> {
        Ok(
            sqlx::query_as::<_, Endorsement>("SELECT * FROM endorsements WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn set_endorsement_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE endorsements SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn accepted_endorsements_for(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<EndorsementCard>> {
        Ok(sqlx::query_as::<_, EndorsementCard>(
            r#"
            SELECT e.id, e.from_user_id, f.name AS from_user_name, e.to_user_id, t.name AS to_user_name,
                   e.gift_id, g.name AS gift_name, e.note, e.status, e.created_at
            FROM endorsements e
            JOIN users f ON f.id = e.from_user_id
            JOIN users t ON t.id = e.to_user_id
            JOIN gifts g ON g.id = e.gift_id
            WHERE e.to_user_id = ? AND e.status = 'accepted'
            ORDER BY e.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn pending_endorsements_for(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<EndorsementCard>> {
        Ok(sqlx::query_as::<_, EndorsementCard>(
            r#"
            SELECT e.id, e.from_user_id, f.name AS from_user_name, e.to_user_id, t.name AS to_user_name,
                   e.gift_id, g.name AS gift_name, e.note, e.status, e.created_at
            FROM endorsements e
            JOIN users f ON f.id = e.from_user_id
            JOIN users t ON t.id = e.to_user_id
            JOIN gifts g ON g.id = e.gift_id
            WHERE e.to_user_id = ? AND e.status = 'pending'
            ORDER BY e.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn notify(
        &self,
        user_id: &str,
        kind: &str,
        title: &str,
        body: &str,
        href: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO notifications (id, user_id, kind, title, body, href, read, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(new_id())
        .bind(user_id)
        .bind(kind)
        .bind(title)
        .bind(body)
        .bind(href)
        .bind(now_iso())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn notifications(&self, user_id: &str) -> anyhow::Result<Vec<Notification>> {
        Ok(sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 50",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn unread_count(&self, user_id: &str) -> anyhow::Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read = 0")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn mark_notifications_read(&self, user_id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE notifications SET read = 1 WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn counts_for_churches(&self) -> anyhow::Result<Vec<(String, i64, i64)>> {
        let rows = sqlx::query(
            r#"
            SELECT c.id,
                   (SELECT COUNT(*) FROM memberships m WHERE m.church_id = c.id AND m.status = 'active') AS members,
                   (SELECT COUNT(*) FROM needs n WHERE n.church_id = c.id AND n.status = 'open') AS needs
            FROM churches c
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("id"),
                    row.get::<i64, _>("members"),
                    row.get::<i64, _>("needs"),
                )
            })
            .collect())
    }

    pub async fn apply(&self, effect: &Effect) -> anyhow::Result<()> {
        for write in &effect.writes {
            match write {
                Write::InsertUser(user) => {
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
                    .execute(&self.pool)
                    .await?;
                }
                Write::UpdateUser {
                    id,
                    name,
                    city,
                    region,
                    bio,
                } => {
                    self.update_user(id, name, city, region, bio).await?;
                }
                Write::InsertChurch(church) => {
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
                    .execute(&self.pool)
                    .await?;
                }
                Write::InsertMembership(membership) => {
                    sqlx::query(
                        "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&membership.id)
                    .bind(&membership.church_id)
                    .bind(&membership.user_id)
                    .bind(&membership.role)
                    .bind(&membership.status)
                    .bind(&membership.created_at)
                    .execute(&self.pool)
                    .await?;
                }
                Write::SetMembershipStatus { id, status } => {
                    self.set_membership_status(id, status).await?;
                }
                Write::InsertNeed(need) => {
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
                    .execute(&self.pool)
                    .await?;
                }
                Write::SetNeedStatus { id, status } => {
                    self.set_need_status(id, status).await?;
                }
                Write::InsertApplication(application) => {
                    sqlx::query(
                        "INSERT INTO applications (id, need_id, user_id, message, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&application.id)
                    .bind(&application.need_id)
                    .bind(&application.user_id)
                    .bind(&application.message)
                    .bind(&application.status)
                    .bind(&application.created_at)
                    .execute(&self.pool)
                    .await?;
                }
                Write::SetApplicationStatus { id, status } => {
                    self.set_application_status(id, status).await?;
                }
                Write::InsertEndorsement(endorsement) => {
                    sqlx::query(
                        "INSERT INTO endorsements (id, from_user_id, to_user_id, gift_id, note, status, created_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&endorsement.id)
                    .bind(&endorsement.from_user_id)
                    .bind(&endorsement.to_user_id)
                    .bind(&endorsement.gift_id)
                    .bind(&endorsement.note)
                    .bind(&endorsement.status)
                    .bind(&endorsement.created_at)
                    .execute(&self.pool)
                    .await?;
                }
                Write::SetEndorsementStatus { id, status } => {
                    self.set_endorsement_status(id, status).await?;
                }
                Write::UpsertMemberGift {
                    user_id,
                    gift_id,
                    note,
                } => {
                    self.add_member_gift(user_id, gift_id, note).await?;
                }
                Write::RemoveMemberGift { user_id, gift_id } => {
                    self.remove_member_gift(user_id, gift_id).await?;
                }
            }
        }
        for notice in &effect.notices {
            self.notify(
                &notice.user_id,
                &notice.kind,
                &notice.title,
                &notice.body,
                &notice.href,
            )
            .await?;
        }
        Ok(())
    }
}

fn need_card_sql(filter: &str) -> String {
    format!(
        r#"
        SELECT n.id, n.church_id, c.name AS church_name, c.city AS church_city, c.region AS church_region,
               n.author_id, u.name AS author_name, n.title, n.body, n.gift_id, g.name AS gift_name,
               n.scope, n.status, n.created_at
        FROM needs n
        JOIN churches c ON c.id = n.church_id
        JOIN users u ON u.id = n.author_id
        LEFT JOIN gifts g ON g.id = n.gift_id
        WHERE {filter}
        ORDER BY n.created_at DESC
        "#
    )
}
