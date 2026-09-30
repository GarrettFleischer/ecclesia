use super::bind::Bind;
use super::seed_data::GIFTS;
use super::Db;

const SEED_CHURCH_ID: &str = "seed_grace";
const SEED_OWNER_ID: &str = "seed_owner";
/// Dev-only pastor for Grace Fellowship. Same string as integration tests use for register.
pub const SEED_OWNER_PASSWORD: &str = "Thursday dinners at six oclock";

impl Db {
    pub(crate) async fn seed_if_empty(&self) -> anyhow::Result<()> {
        if self.gift_count().await? == 0 {
            insert_gift_rows(self, GIFTS).await?;
            tracing::info!("seeded gift catalog");
        }
        self.ensure_grace_fellowship().await
    }

    async fn ensure_grace_fellowship(&self) -> anyhow::Result<()> {
        if self.church(SEED_CHURCH_ID).await?.is_some() {
            self.ensure_seed_owner_password().await?;
            return Ok(());
        }
        self.seed_grace_church().await?;
        tracing::info!("seeded Grace Fellowship for registration lookup");
        Ok(())
    }

    pub async fn seed_grace_church(&self) -> anyhow::Result<&'static str> {
        if self.church(SEED_CHURCH_ID).await?.is_some() {
            self.ensure_seed_owner_password().await?;
            return Ok(SEED_CHURCH_ID);
        }
        let hash = crate::password::hash_password(SEED_OWNER_PASSWORD)?;
        let now = "2020-01-01T00:00:00Z";
        self.execute(
            "INSERT INTO users (id, name, email, city, region, bio, password_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(SEED_OWNER_ID),
                Bind::Text("Seed Owner"),
                Bind::Text("owner@seed.test"),
                Bind::Text("Cedar Falls"),
                Bind::Text("Iowa"),
                Bind::Text(""),
                Bind::Text(&hash),
                Bind::Text(now),
            ],
        )
        .await?;
        self.execute(
            "INSERT INTO churches (id, name, city, region, country, description, gathering, owner_id, invite_code, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(SEED_CHURCH_ID),
                Bind::Text("Grace Fellowship"),
                Bind::Text("Cedar Falls"),
                Bind::Text("Iowa"),
                Bind::Text("US"),
                Bind::Text("Seed church for registration."),
                Bind::Text("Sunday at 10."),
                Bind::Text(SEED_OWNER_ID),
                Bind::Text("GRACESEED"),
                Bind::Text(now),
            ],
        )
        .await?;
        self.execute(
            "INSERT INTO memberships (id, church_id, user_id, role, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text("seed_owner_mem"),
                Bind::Text(SEED_CHURCH_ID),
                Bind::Text(SEED_OWNER_ID),
                Bind::Text("owner"),
                Bind::Text("active"),
                Bind::Text(now),
            ],
        )
        .await?;
        Ok(SEED_CHURCH_ID)
    }

    async fn ensure_seed_owner_password(&self) -> anyhow::Result<()> {
        if self.user_password_hash(SEED_OWNER_ID).await?.is_some() {
            return Ok(());
        }
        let hash = crate::password::hash_password(SEED_OWNER_PASSWORD)?;
        self.execute(
            "UPDATE users SET password_hash = ? WHERE id = ?",
            &[Bind::Text(&hash), Bind::Text(SEED_OWNER_ID)],
        )
        .await?;
        Ok(())
    }
    async fn gift_count(&self) -> anyhow::Result<i64> {
        self.fetch_scalar_i64("SELECT COUNT(*) FROM gifts", &[]).await
    }
}
async fn insert_gift_rows(db: &Db, rows: &[(&str, &str, &str)]) -> anyhow::Result<()> {
    for row in rows {
        let (id, name, category) = *row;
        db.execute(
            "INSERT INTO gifts (id, name, category) VALUES (?, ?, ?)",
            &[Bind::Text(id), Bind::Text(name), Bind::Text(category)],
        )
        .await?;
    }
    Ok(())
}
