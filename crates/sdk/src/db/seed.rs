use super::Db;
use super::bind::Bind;
use super::dialect::Driver;
use super::seed_data::GIFTS;
use crate::host::is_public_host;

/// Dev church and seed accounts: local SQLite only, never Postgres or a public host.
pub(crate) fn dev_church_seed_enabled(driver: Driver) -> bool {
    driver == Driver::Sqlite && !is_public_host()
}

const SEED_CHURCH_ID: &str = "seed_grace";
const SEED_OWNER_ID: &str = "seed_owner";
const SEED_ADMIN_ID: &str = "seed_admin";
/// Dev governor for Grace Fellowship.
pub const SEED_ADMIN_EMAIL: &str = "admin@seed.test";
/// Dev login for the seeded Grace Fellowship accounts. Reset on each boot.
pub const SEED_OWNER_PASSWORD: &str = "password";

impl Db {
    pub(crate) async fn seed_if_empty(&self) -> anyhow::Result<()> {
        if self.gift_count().await? == 0 {
            insert_gift_rows(self, GIFTS).await?;
            tracing::info!("seeded gift catalog");
        }
        if !dev_church_seed_enabled(self.driver()) {
            return Ok(());
        }
        self.ensure_grace_fellowship().await
    }

    async fn ensure_grace_fellowship(&self) -> anyhow::Result<()> {
        if self.church(SEED_CHURCH_ID).await?.is_none() {
            self.seed_grace_church().await?;
            tracing::info!("seeded Grace Fellowship for registration lookup");
        } else {
            self.ensure_seed_owner_password().await?;
        }
        self.ensure_seed_admin().await?;
        Ok(())
    }

    pub async fn seed_grace_church(&self) -> anyhow::Result<&'static str> {
        if !dev_church_seed_enabled(self.driver()) {
            anyhow::bail!("Grace Fellowship seed is only for local SQLite");
        }
        if self.church(SEED_CHURCH_ID).await?.is_some() {
            self.ensure_seed_owner_password().await?;
            self.ensure_seed_admin().await?;
            return Ok(SEED_CHURCH_ID);
        }
        let hash = seed_password_hash()?;
        let now = "2020-01-01T00:00:00Z";
        self.execute(
            "INSERT INTO users (id, first_name, last_name, email, bio, password_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(SEED_OWNER_ID),
                Bind::Text("Seed"),
                Bind::Text("Owner"),
                Bind::Text("owner@seed.test"),
                Bind::Text(""),
                Bind::Text(&hash),
                Bind::Text(now),
            ],
        )
        .await?;
        self.execute(
            "INSERT INTO memberships (user_id, church_id, role, status, created_at) VALUES (?, ?, 'owner', 'active', ?)",
            &[
                Bind::Text(SEED_OWNER_ID),
                Bind::Text(SEED_CHURCH_ID),
                Bind::Text(now),
            ],
        )
        .await?;
        self.execute(
            "INSERT INTO churches (id, name, address, latitude, longitude, country, description, gathering, ein, registry_state, registry_number, owner_id, invite_code, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                Bind::Text(SEED_CHURCH_ID),
                Bind::Text("Grace Fellowship"),
                Bind::Text("100 Main Street\nCedar Falls, IA 50613"),
                Bind::F64(42.5349),
                Bind::F64(-92.4453),
                Bind::Text("US"),
                Bind::Text("Seed church for registration."),
                Bind::Text("Sunday at 10."),
                Bind::Text("12-3456789"),
                Bind::Text("IA"),
                Bind::Text("123456"),
                Bind::Text(SEED_OWNER_ID),
                Bind::Text("GRACESEED"),
                Bind::Text(now),
            ],
        )
        .await?;
        self.ensure_seed_admin().await?;
        Ok(SEED_CHURCH_ID)
    }

    async fn ensure_seed_admin(&self) -> anyhow::Result<()> {
        if self.church(SEED_CHURCH_ID).await?.is_none() {
            return Ok(());
        }
        let now = "2020-01-01T00:00:00Z";
        if self.user(SEED_ADMIN_ID).await?.is_none() {
            let hash = seed_password_hash()?;
            self.execute(
                "INSERT INTO users (id, first_name, last_name, email, bio, password_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
                &[
                    Bind::Text(SEED_ADMIN_ID),
                    Bind::Text("Grace"),
                    Bind::Text("Admin"),
                    Bind::Text(SEED_ADMIN_EMAIL),
                    Bind::Text(""),
                    Bind::Text(&hash),
                    Bind::Text(now),
                ],
            )
            .await?;
            self.ensure_seed_membership(SEED_ADMIN_ID, now).await?;
        } else {
            self.set_seed_password(SEED_ADMIN_ID).await?;
            self.ensure_seed_membership(SEED_ADMIN_ID, now).await?;
        }
        Ok(())
    }

    async fn ensure_seed_membership(&self, user_id: &str, now: &str) -> anyhow::Result<()> {
        self.execute(
            "INSERT INTO memberships (user_id, church_id, role, status, created_at) VALUES (?, ?, 'owner', 'active', ?)
             ON CONFLICT (user_id, church_id) DO UPDATE SET role = 'owner', status = 'active'",
            &[
                Bind::Text(user_id),
                Bind::Text(SEED_CHURCH_ID),
                Bind::Text(now),
            ],
        )
        .await?;
        Ok(())
    }

    async fn ensure_seed_owner_password(&self) -> anyhow::Result<()> {
        self.set_seed_password(SEED_OWNER_ID).await
    }

    async fn set_seed_password(&self, user_id: &str) -> anyhow::Result<()> {
        let hash = seed_password_hash()?;
        self.execute(
            "UPDATE users SET password_hash = ? WHERE id = ?",
            &[Bind::Text(&hash), Bind::Text(user_id)],
        )
        .await?;
        Ok(())
    }
    async fn gift_count(&self) -> anyhow::Result<i64> {
        self.fetch_scalar_i64("SELECT COUNT(*) FROM gifts", &[])
            .await
    }
}
fn seed_password_hash() -> anyhow::Result<String> {
    crate::password::hash_password(SEED_OWNER_PASSWORD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_seed_01_dev_church_seed_never_on_postgres() {
        assert!(!dev_church_seed_enabled(Driver::Postgres));
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
