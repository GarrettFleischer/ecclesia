//! Run one `?` query on SQLite or Postgres.

use sqlx::query::{Query, QueryAs, QueryScalar};
use sqlx::{PgPool, Postgres, Sqlite, SqlitePool};

use super::bind::Bind;
use super::dialect::Driver;
use super::Db;

enum OwnedBind {
    Text(String),
    OptText(Option<String>),
    I64(i64),
}

fn owned_binds(binds: &[Bind<'_>]) -> Vec<OwnedBind> {
    binds
        .iter()
        .map(|bind| match *bind {
            Bind::Text(value) => OwnedBind::Text(value.to_string()),
            Bind::OptText(value) => OwnedBind::OptText(value.map(str::to_string)),
            Bind::I64(value) => OwnedBind::I64(value),
        })
        .collect()
}

impl Db {
    pub(crate) fn driver(&self) -> Driver {
        match &self.inner {
            super::Inner::Sqlite(_) => Driver::Sqlite,
            super::Inner::Postgres(_) => Driver::Postgres,
        }
    }

    pub(crate) async fn execute(&self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<()> {
        match &self.inner {
            super::Inner::Sqlite(pool) => execute_sqlite(pool, sql, binds).await,
            super::Inner::Postgres(pool) => execute_postgres(pool, sql, binds).await,
        }
    }

    pub(crate) async fn fetch_optional<T>(
        &self,
        sql: &str,
        binds: &[Bind<'_>],
    ) -> anyhow::Result<Option<T>>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
            + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
            + Send
            + Unpin,
    {
        match &self.inner {
            super::Inner::Sqlite(pool) => fetch_optional_sqlite(pool, sql, binds).await,
            super::Inner::Postgres(pool) => fetch_optional_postgres(pool, sql, binds).await,
        }
    }

    pub(crate) async fn fetch_all<T>(&self, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<Vec<T>>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>
            + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>
            + Send
            + Unpin,
    {
        match &self.inner {
            super::Inner::Sqlite(pool) => fetch_all_sqlite(pool, sql, binds).await,
            super::Inner::Postgres(pool) => fetch_all_postgres(pool, sql, binds).await,
        }
    }

    pub(crate) async fn fetch_scalar_i64(
        &self,
        sql: &str,
        binds: &[Bind<'_>],
    ) -> anyhow::Result<i64> {
        match &self.inner {
            super::Inner::Sqlite(pool) => fetch_scalar_i64_sqlite(pool, sql, binds).await,
            super::Inner::Postgres(pool) => fetch_scalar_i64_postgres(pool, sql, binds).await,
        }
    }

    pub(crate) async fn fetch_strings(
        &self,
        sql: &str,
        binds: &[Bind<'_>],
    ) -> anyhow::Result<Vec<String>> {
        match &self.inner {
            super::Inner::Sqlite(pool) => fetch_strings_sqlite(pool, sql, binds).await,
            super::Inner::Postgres(pool) => fetch_strings_postgres(pool, sql, binds).await,
        }
    }
}

async fn execute_sqlite(pool: &SqlitePool, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<()> {
    let owned = owned_binds(binds);
    let mut query = sqlx::query(sql);
    for bind in &owned {
        query = bind_sqlite_query(query, bind);
    }
    query.execute(pool).await?;
    Ok(())
}

async fn execute_postgres(pool: &PgPool, sql: &str, binds: &[Bind<'_>]) -> anyhow::Result<()> {
    let sql = Driver::Postgres.sql(sql);
    let owned = owned_binds(binds);
    let mut query = sqlx::query(&sql);
    for bind in &owned {
        query = bind_postgres_query(query, bind);
    }
    query.execute(pool).await?;
    Ok(())
}

async fn fetch_optional_sqlite<T>(
    pool: &SqlitePool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<Option<T>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    let owned = owned_binds(binds);
    let mut query = sqlx::query_as::<Sqlite, T>(sql);
    for bind in &owned {
        query = bind_sqlite_as(query, bind);
    }
    Ok(query.fetch_optional(pool).await?)
}

async fn fetch_optional_postgres<T>(
    pool: &PgPool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<Option<T>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let sql = Driver::Postgres.sql(sql);
    let owned = owned_binds(binds);
    let mut query = sqlx::query_as::<Postgres, T>(&sql);
    for bind in &owned {
        query = bind_postgres_as(query, bind);
    }
    Ok(query.fetch_optional(pool).await?)
}

async fn fetch_all_sqlite<T>(
    pool: &SqlitePool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<Vec<T>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
{
    let owned = owned_binds(binds);
    let mut query = sqlx::query_as::<Sqlite, T>(sql);
    for bind in &owned {
        query = bind_sqlite_as(query, bind);
    }
    Ok(query.fetch_all(pool).await?)
}

async fn fetch_all_postgres<T>(
    pool: &PgPool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<Vec<T>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let sql = Driver::Postgres.sql(sql);
    let owned = owned_binds(binds);
    let mut query = sqlx::query_as::<Postgres, T>(&sql);
    for bind in &owned {
        query = bind_postgres_as(query, bind);
    }
    Ok(query.fetch_all(pool).await?)
}

async fn fetch_scalar_i64_sqlite(
    pool: &SqlitePool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<i64> {
    let owned = owned_binds(binds);
    let mut query = sqlx::query_scalar::<Sqlite, i64>(sql);
    for bind in &owned {
        query = bind_sqlite_scalar(query, bind);
    }
    Ok(query.fetch_one(pool).await?)
}

async fn fetch_scalar_i64_postgres(
    pool: &PgPool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<i64> {
    let sql = Driver::Postgres.sql(sql);
    let owned = owned_binds(binds);
    let mut query = sqlx::query_scalar::<Postgres, i64>(&sql);
    for bind in &owned {
        query = bind_postgres_scalar(query, bind);
    }
    Ok(query.fetch_one(pool).await?)
}

async fn fetch_strings_sqlite(
    pool: &SqlitePool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<Vec<String>> {
    let owned = owned_binds(binds);
    let mut query = sqlx::query_scalar::<Sqlite, String>(sql);
    for bind in &owned {
        query = bind_sqlite_scalar(query, bind);
    }
    Ok(query.fetch_all(pool).await?)
}

async fn fetch_strings_postgres(
    pool: &PgPool,
    sql: &str,
    binds: &[Bind<'_>],
) -> anyhow::Result<Vec<String>> {
    let sql = Driver::Postgres.sql(sql);
    let owned = owned_binds(binds);
    let mut query = sqlx::query_scalar::<Postgres, String>(&sql);
    for bind in &owned {
        query = bind_postgres_scalar(query, bind);
    }
    Ok(query.fetch_all(pool).await?)
}

fn bind_sqlite_query<'q>(
    query: Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    bind: &'q OwnedBind,
) -> Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match bind {
        OwnedBind::Text(value) => query.bind(value),
        OwnedBind::OptText(value) => query.bind(value.as_deref()),
        OwnedBind::I64(value) => query.bind(*value),
    }
}

fn bind_postgres_query<'q>(
    query: Query<'q, Postgres, sqlx::postgres::PgArguments>,
    bind: &'q OwnedBind,
) -> Query<'q, Postgres, sqlx::postgres::PgArguments> {
    match bind {
        OwnedBind::Text(value) => query.bind(value),
        OwnedBind::OptText(value) => query.bind(value.as_deref()),
        OwnedBind::I64(value) => query.bind(*value),
    }
}

fn bind_sqlite_as<'q, T>(
    query: QueryAs<'q, Sqlite, T, sqlx::sqlite::SqliteArguments<'q>>,
    bind: &'q OwnedBind,
) -> QueryAs<'q, Sqlite, T, sqlx::sqlite::SqliteArguments<'q>> {
    match bind {
        OwnedBind::Text(value) => query.bind(value),
        OwnedBind::OptText(value) => query.bind(value.as_deref()),
        OwnedBind::I64(value) => query.bind(*value),
    }
}

fn bind_postgres_as<'q, T>(
    query: QueryAs<'q, Postgres, T, sqlx::postgres::PgArguments>,
    bind: &'q OwnedBind,
) -> QueryAs<'q, Postgres, T, sqlx::postgres::PgArguments> {
    match bind {
        OwnedBind::Text(value) => query.bind(value),
        OwnedBind::OptText(value) => query.bind(value.as_deref()),
        OwnedBind::I64(value) => query.bind(*value),
    }
}

fn bind_sqlite_scalar<'q, T>(
    query: QueryScalar<'q, Sqlite, T, sqlx::sqlite::SqliteArguments<'q>>,
    bind: &'q OwnedBind,
) -> QueryScalar<'q, Sqlite, T, sqlx::sqlite::SqliteArguments<'q>> {
    match bind {
        OwnedBind::Text(value) => query.bind(value),
        OwnedBind::OptText(value) => query.bind(value.as_deref()),
        OwnedBind::I64(value) => query.bind(*value),
    }
}

fn bind_postgres_scalar<'q, T>(
    query: QueryScalar<'q, Postgres, T, sqlx::postgres::PgArguments>,
    bind: &'q OwnedBind,
) -> QueryScalar<'q, Postgres, T, sqlx::postgres::PgArguments> {
    match bind {
        OwnedBind::Text(value) => query.bind(value),
        OwnedBind::OptText(value) => query.bind(value.as_deref()),
        OwnedBind::I64(value) => query.bind(*value),
    }
}
