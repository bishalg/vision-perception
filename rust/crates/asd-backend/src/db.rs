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

/// Data conversions that SQL cannot do because they need `ASD_DATA_KEK`, run by
/// `asd-backend migrate` right after the migrations:
///
/// - 0016: plaintext `users.totp_secret` seeds are sealed into `totp_secret_ct`
///   ([`crate::auth::totp::encrypt_legacy_secrets`]). Without the KEK this fails while a
///   plaintext seed remains, so a deployment never runs with secrets left in clear
///   unnoticed; with none left it is a no-op.
///
/// Idempotent: a second run converts nothing.
pub async fn backfill(pool: &PgPool, config: &Config) -> Result<(), String> {
    let legacy = crate::auth::totp::legacy_secret_count(pool)
        .await
        .map_err(|e| format!("count plaintext TOTP seeds: {e}"))?;
    if legacy == 0 {
        return Ok(());
    }
    let Some(kek) = config.data_kek.clone() else {
        return Err(format!(
            "{legacy} TOTP seed(s) are stored in clear (migration 0016); set ASD_DATA_KEK and \
             run `asd-backend migrate` again to encrypt them"
        ));
    };
    let keyring = crate::crypto::Keyring::new(kek, pool.clone());
    let converted = crate::auth::totp::encrypt_legacy_secrets(pool, &keyring)
        .await
        .map_err(|e| format!("encrypt TOTP seeds: {e}"))?;
    tracing::info!(converted, "TOTP seeds encrypted at rest (0016)");
    Ok(())
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
