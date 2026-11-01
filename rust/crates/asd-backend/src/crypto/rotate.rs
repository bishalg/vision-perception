//! owner: WP-B.15
//!
//! KEY-9 KEK rotation: `asd-backend admin rotate-kek [--batch N] [--dry-run]`.
//!
//! Run with the new KEK active (`ASD_DATA_KEK`) and the old one in
//! `ASD_DATA_KEK_PREVIOUS`. For every row whose stored `kek_id` names a *previous* KEK:
//!
//! - `org_data_keys.wrapped_dek` (every status, retired keys too): unwrapped and wrapped
//!   again under the active KEK ([`Keyring::rewrap_dek`]). The DEK bytes, the AAD and so
//!   every clinical ciphertext and the MRN blind-index key stay as they are;
//! - each sealed user secret in [`USER_SECRET_COLUMNS`]: opened and sealed again under the
//!   active KEK with the same AAD ([`Keyring::reseal_user_secret`]).
//!
//! Each batch of `N` rows is one transaction, read with `FOR UPDATE SKIP LOCKED` and
//! written with a compare-and-set on the old `kek_id` ([`rewrap_dek_cas`],
//! [`reseal_user_secret_cas`]), so a row that a concurrent writer already moved to the
//! active KEK is never overwritten. Rows are walked by primary key, so a row that cannot be
//! opened (locked, damaged) is skipped instead of retried forever. Afterwards every row not
//! under the active KEK is counted as `remaining` (rows under a KEK that is not configured
//! at all, rows skipped); the CLI exits 3 while that is non-zero. A second run changes
//! nothing.
//!
//! Not touched: integration-destination secrets (`interop::destination`) are sealed with
//! the org DEK, not the KEK, so re-wrapping the DEK already covers them.
//!
//! Counts and fingerprints only are printed and audited (`crypto.kek_rotated`, org NULL);
//! never key bytes, seeds or ciphertexts.

use serde::Serialize;
use serde_json::json;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use super::{Aad, CryptoError, Keyring};
use crate::audit::AuditEvent;

/// Default rows per transaction.
pub const DEFAULT_BATCH: u32 = 500;

/// Largest accepted `--batch`.
pub const MAX_BATCH: u32 = 10_000;

/// One sealed per-user secret column pair on `users` and the AAD it is sealed under.
#[derive(Debug, Clone, Copy)]
pub struct UserSecretColumn {
    /// The ciphertext column (`bytea`).
    pub ct: &'static str,
    /// The column holding the sealing KEK's fingerprint (`text`).
    pub kek: &'static str,
    /// AAD of the value for one user id.
    pub aad: fn(Uuid) -> Aad,
}

/// Every KEK-sealed column on `users`: the confirmed TOTP seed (0016) and the pending seed
/// of an enrolment in progress (WP-B.11, migration 0020). A pending seed that expires before
/// the run is still re-sealed (it is cleared by the housekeeping sweep or the next confirm).
pub const USER_SECRET_COLUMNS: &[UserSecretColumn] = &[
    UserSecretColumn {
        ct: "totp_secret_ct",
        kek: "totp_secret_kek_id",
        aad: crate::auth::totp::secret_aad,
    },
    UserSecretColumn {
        ct: "totp_pending_ct",
        kek: "totp_pending_kek_id",
        aad: crate::auth::totp::pending_aad,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotateOptions {
    /// Rows per transaction (1..=[`MAX_BATCH`]).
    pub batch: u32,
    /// Count and check (every candidate is opened) without writing anything.
    pub dry_run: bool,
}

impl Default for RotateOptions {
    fn default() -> Self {
        Self {
            batch: DEFAULT_BATCH,
            dry_run: false,
        }
    }
}

/// What a run did (or, with `dry_run`, would do). Printed as JSON by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RotateReport {
    pub rewrapped_deks: u64,
    pub resealed_users: u64,
    /// Rows still not under the active KEK after the run (for a dry run: rows that the
    /// run would leave behind).
    pub remaining: u64,
    pub dry_run: bool,
}

/// Re-wraps and re-seals everything under a previous KEK; see the module docs.
pub async fn rotate_kek(
    pool: &PgPool,
    keyring: &Keyring,
    opts: RotateOptions,
) -> Result<RotateReport, CryptoError> {
    let batch = i64::from(opts.batch.clamp(1, MAX_BATCH));
    let active = keyring.kek_id().to_owned();
    let previous: Vec<String> = keyring
        .previous_kek_ids()
        .into_iter()
        .map(str::to_owned)
        .collect();

    let mut rewrapped_deks = 0u64;
    let mut cursor = Uuid::nil();
    loop {
        let mut tx = pool.begin().await?;
        let rows = sqlx::query!(
            "SELECT key_id, org_id, wrapped_dek, kek_id FROM org_data_keys
             WHERE kek_id = ANY($1) AND key_id > $2
             ORDER BY key_id
             LIMIT $3
             FOR UPDATE SKIP LOCKED",
            &previous[..],
            cursor,
            batch
        )
        .fetch_all(&mut *tx)
        .await?;
        let Some(last) = rows.last() else {
            tx.rollback().await?;
            break;
        };
        cursor = last.key_id;
        for r in &rows {
            let wrapped = match keyring.rewrap_dek(r.org_id, r.key_id, &r.wrapped_dek, &r.kek_id) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!(
                        key_id = %r.key_id,
                        org = %r.org_id,
                        stored_kek = %r.kek_id,
                        error = %e,
                        "org data key cannot be re-wrapped; left as it is"
                    );
                    continue;
                }
            };
            if opts.dry_run
                || rewrap_dek_cas(&mut tx, r.key_id, &r.kek_id, &wrapped, &active).await?
            {
                rewrapped_deks += 1;
            }
        }
        finish(tx, opts.dry_run).await?;
    }

    let mut resealed_users = 0u64;
    for column in USER_SECRET_COLUMNS {
        resealed_users +=
            reseal_column(pool, keyring, column, &previous, batch, opts.dry_run).await?;
    }

    let not_active = count_not_active(pool, &active).await?;
    let remaining = if opts.dry_run {
        not_active.saturating_sub(rewrapped_deks + resealed_users)
    } else {
        not_active
    };
    let report = RotateReport {
        rewrapped_deks,
        resealed_users,
        remaining,
        dry_run: opts.dry_run,
    };
    if !opts.dry_run {
        let mut tx = pool.begin().await?;
        AuditEvent::system("crypto.kek_rotated", "keyring", &active)
            .after(json!({
                "rewrapped_deks": report.rewrapped_deks,
                "resealed_users": report.resealed_users,
                "remaining": report.remaining,
                "active_kek": active,
                "previous_keks": previous,
                "via": "cli",
            }))
            .record(&mut tx)
            .await
            .map_err(|e| CryptoError::Internal(e.to_string()))?;
        tx.commit().await?;
    }
    Ok(report)
}

async fn finish(
    tx: sqlx::Transaction<'_, sqlx::Postgres>,
    dry_run: bool,
) -> Result<(), sqlx::Error> {
    if dry_run {
        tx.rollback().await
    } else {
        tx.commit().await
    }
}

async fn reseal_column(
    pool: &PgPool,
    keyring: &Keyring,
    column: &UserSecretColumn,
    previous: &[String],
    batch: i64,
    dry_run: bool,
) -> Result<u64, CryptoError> {
    let active = keyring.kek_id();
    // Column names come from the constant table above, never from input.
    let select = format!(
        "SELECT id, {ct} AS ct, {kek} AS kek_id FROM users
         WHERE {kek} = ANY($1) AND {ct} IS NOT NULL AND id > $2
         ORDER BY id
         LIMIT $3
         FOR UPDATE SKIP LOCKED",
        ct = column.ct,
        kek = column.kek
    );
    let mut changed = 0u64;
    let mut cursor = Uuid::nil();
    loop {
        let mut tx = pool.begin().await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(select.clone()))
            .bind(previous)
            .bind(cursor)
            .bind(batch)
            .fetch_all(&mut *tx)
            .await?;
        let Some(last) = rows.last() else {
            tx.rollback().await?;
            break;
        };
        cursor = last.try_get("id")?;
        for row in &rows {
            let id: Uuid = row.try_get("id")?;
            let ct: Vec<u8> = row.try_get("ct")?;
            let kek_id: String = row.try_get("kek_id")?;
            let sealed = match keyring.reseal_user_secret(&(column.aad)(id), &kek_id, &ct) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        user_id = %id,
                        column = column.ct,
                        stored_kek = %kek_id,
                        error = %e,
                        "user secret cannot be re-sealed; left as it is"
                    );
                    continue;
                }
            };
            if dry_run
                || reseal_user_secret_cas(&mut tx, column, id, &kek_id, &sealed, active).await?
            {
                changed += 1;
            }
        }
        finish(tx, dry_run).await?;
    }
    Ok(changed)
}

/// Rows (DEKs and sealed user secrets) whose `kek_id` is not the active KEK.
async fn count_not_active(pool: &PgPool, active: &str) -> Result<u64, CryptoError> {
    let mut n = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM org_data_keys WHERE kek_id <> $1"#,
        active
    )
    .fetch_one(pool)
    .await?;
    for column in USER_SECRET_COLUMNS {
        let sql = format!(
            "SELECT count(*) FROM users WHERE {ct} IS NOT NULL AND {kek} IS DISTINCT FROM $1",
            ct = column.ct,
            kek = column.kek
        );
        let users: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(active)
            .fetch_one(pool)
            .await?;
        n += users;
    }
    Ok(u64::try_from(n).unwrap_or(0))
}

/// Stores a re-wrapped DEK only while the row is still under `old_kek_id`; `false` when
/// another writer changed it first (KEY-9 compare-and-set).
pub async fn rewrap_dek_cas(
    conn: &mut PgConnection,
    key_id: Uuid,
    old_kek_id: &str,
    wrapped: &[u8],
    new_kek_id: &str,
) -> Result<bool, sqlx::Error> {
    let n = sqlx::query!(
        "UPDATE org_data_keys SET wrapped_dek = $3, kek_id = $4
         WHERE key_id = $1 AND kek_id = $2",
        key_id,
        old_kek_id,
        wrapped,
        new_kek_id
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(n == 1)
}

/// Stores a re-sealed user secret only while the row is still under `old_kek_id`; `false`
/// when another writer changed (or cleared) it first (KEY-9 compare-and-set).
pub async fn reseal_user_secret_cas(
    conn: &mut PgConnection,
    column: &UserSecretColumn,
    user_id: Uuid,
    old_kek_id: &str,
    ciphertext: &[u8],
    new_kek_id: &str,
) -> Result<bool, sqlx::Error> {
    let sql = format!(
        "UPDATE users SET {ct} = $3, {kek} = $4 WHERE id = $1 AND {kek} = $2",
        ct = column.ct,
        kek = column.kek
    );
    let n = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(old_kek_id)
        .bind(ciphertext)
        .bind(new_kek_id)
        .execute(conn)
        .await?
        .rows_affected();
    Ok(n == 1)
}
