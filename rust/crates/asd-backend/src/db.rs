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

/// Applies every pending migration (no data conversion; see [`migrate_with_backfill`]).
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

/// The migration that added `users.totp_secret_ct` (plaintext seeds become convertible).
const SEALED_SEED_MIGRATION: i64 = 16;

/// What `asd-backend migrate` runs (KEY-12), in this order:
///
/// 1. the seed pre-step ([`backfill`]): when 0016 is applied and plaintext
///    `users.totp_secret` seeds remain, they are sealed **before** any pending migration
///    is applied, because 0022 refuses to drop the column while a seed is in clear.
///    Without `ASD_DATA_KEK` this fails and applies nothing;
/// 2. the pending migrations. A database still below 0016 is first migrated to 0016 and
///    sealed (step 1 again), so one run takes it to the end;
/// 3. the post-migration [`backfill`] (a no-op once 0022 dropped the column).
///
/// Re-runnable: a failed migration (such as the 0022 guard on a plaintext review note)
/// is rolled back by its own transaction, so running this again after fixing the data
/// continues where it stopped, without manual SQL. Returns the versions applied by this
/// run, in order.
pub async fn migrate_with_backfill(pool: &PgPool, config: &Config) -> Result<Vec<i64>, String> {
    let before = pending_migrations(pool)
        .await
        .map_err(|e| format!("read migration state: {e}"))?;
    if before.contains(&SEALED_SEED_MIGRATION) {
        run_migrator(pool, Some(SEALED_SEED_MIGRATION)).await?;
    }
    backfill(pool, config).await?;
    run_migrator(pool, None).await?;
    backfill(pool, config).await?;
    let after = pending_migrations(pool)
        .await
        .map_err(|e| format!("read migration state: {e}"))?;
    Ok(before.into_iter().filter(|v| !after.contains(v)).collect())
}

/// Runs the migrator (up to `target`, or every pending migration) on one connection. The
/// migrator holds a session-level advisory lock and does not release it when a migration
/// fails, so on error that connection is closed instead of going back to the pool, where
/// the lock would block the next run in the same process.
async fn run_migrator(pool: &PgPool, target: Option<i64>) -> Result<(), String> {
    let mut conn = pool.acquire().await.map_err(|e| format!("connect: {e}"))?;
    let result = match target {
        Some(version) => MIGRATOR.run_to(version, &mut *conn).await,
        None => MIGRATOR.run(&mut *conn).await,
    };
    if result.is_err() {
        // Closing ends the session and with it the advisory lock.
        let _ = conn.close().await;
    }
    result.map_err(|e| format!("migrate: {e}"))
}

/// Data conversions that SQL cannot do because they need `ASD_DATA_KEK`:
///
/// - 0016: plaintext `users.totp_secret` seeds are sealed into `totp_secret_ct`
///   ([`crate::auth::totp::encrypt_legacy_secrets`]). Without the KEK this fails while a
///   plaintext seed remains, so a deployment never runs with secrets left in clear
///   unnoticed; with none left, before 0016 or after 0022 dropped the column, it is a
///   no-op.
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
