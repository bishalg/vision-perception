//! owner: WP-B.1
//!
//! Postgres pool and embedded migrations (02 ADR-B2). Migrations run only through
//! `asd-backend migrate` (DEP-1), never implicitly when the server boots.
//!
//! Queries use the compile-time checked `sqlx::query!` family where possible. Their
//! offline data lives in `rust/.sqlx/`; after changing a query run
//! `cargo sqlx prepare --workspace` against a migrated database (trap 8).

use std::collections::BTreeSet;

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use crate::config::Config;

/// Every migration under `crates/asd-backend/migrations/`, embedded at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

fn options(config: &Config) -> Result<(PgPoolOptions, PgConnectOptions), sqlx::Error> {
    let connect: PgConnectOptions = config.database_url.parse()?;
    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .acquire_timeout(config.db_acquire_timeout);
    Ok((pool, connect.application_name("asd-backend")))
}

/// Connects eagerly (fails fast when the database is unreachable).
pub async fn connect(config: &Config) -> Result<PgPool, sqlx::Error> {
    let (pool, connect) = options(config)?;
    pool.connect_with(connect).await
}

/// A pool that connects on first use; the server starts even while Postgres is still
/// coming up, and `/health/ready` reports it.
pub fn connect_lazy(config: &Config) -> Result<PgPool, sqlx::Error> {
    let (pool, connect) = options(config)?;
    Ok(pool.connect_lazy_with(connect))
}

/// Applies every pending migration.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

/// Round-trips one query.
pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    let one = sqlx::query_scalar!(r#"SELECT 1 AS "one!: i32""#)
        .fetch_one(pool)
        .await?;
    debug_assert_eq!(one, 1);
    Ok(())
}

/// Versions embedded in the binary that the database has not applied successfully.
pub async fn pending_migrations(pool: &PgPool) -> Result<Vec<i64>, sqlx::Error> {
    // `_sqlx_migrations` does not exist before the first `migrate`, so this cannot be a
    // compile-time checked query.
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(pool)
        .await?;
    let applied: BTreeSet<i64> = if exists {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect()
    } else {
        BTreeSet::new()
    };
    Ok(MIGRATOR
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .map(|m| m.version)
        .filter(|v| !applied.contains(v))
        .collect())
}
