//! owner: security audit follow-ups (2026-10-31)
//!
//! The TOTP second factor as stored and checked, shared by login (`routes::auth`, AUTH-1)
//! and step-up (`clinical::reauth`, SIGN-5). The RFC 6238 algorithm itself is
//! [`reauth::matched_totp_step`]; this module adds:
//!
//! - the seed at rest (migration 0016): `users.totp_secret_ct`, sealed with
//!   [`Keyring::seal_user_secret`] under AAD [`secret_aad`], plus `totp_secret_kek_id`.
//!   Reading needs `ASD_DATA_KEK`; without it an enrolled user's check answers
//!   `503 clinical-crypto-unavailable` (fail closed, never "not enrolled");
//! - spending an accepted code ([`verify_and_spend`]): its time step goes into
//!   `users.last_totp_step` with a compare-and-set (0015), so a code of the same or an
//!   earlier step never passes again, not even in two concurrent requests;
//! - the 0016 back-fill of plaintext seeds ([`encrypt_legacy_secrets`]), run by
//!   `asd-backend migrate` before pending migrations (KEY-12). Migration 0022 drops
//!   `users.totp_secret`, so these helpers use runtime queries guarded by an
//!   `information_schema` check (never a `query!` macro naming the column) and do nothing
//!   once it is gone;
//! - self-service enrolment (WP-B.11, AUTH-6..10, migration 0020): [`start_enrolment`]
//!   stores a *pending* seed (`users.totp_pending_ct`, AAD [`pending_aad`], 10 minutes),
//!   [`confirm_enrolment`] moves it to `totp_secret_ct` once a code from it matches,
//!   [`disable`] and [`reset`] clear everything. [`load_secret`] never reads the pending
//!   columns, so a pending seed affects neither login nor step-up.
//!
//! Seeds and codes are never logged or put into an error.

use std::fmt;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::audit::AuditEvent;
use crate::clinical::reauth;
use crate::crypto::{Aad, CryptoError, Keyring};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

/// Rows per transaction in [`encrypt_legacy_secrets`].
const BACKFILL_BATCH: i64 = 500;

/// The issuer shown by authenticator apps (label prefix and `issuer=` of the otpauth URL).
pub const TOTP_ISSUER: &str = "Eye Gaze Console";

/// How long a pending seed can be confirmed (AUTH-6).
pub const PENDING_TTL_SECS: i64 = 600;

/// Seed length in bytes (RFC 4226 recommends 160 bits).
const SEED_BYTES: usize = 20;

/// `"users:totp_secret_ct:<nil>:{user_id}"`: users are not org data (a superadmin has no
/// org), so the org part is the nil UUID.
pub fn secret_aad(user_id: Uuid) -> Aad {
    Aad::new("users", "totp_secret_ct", Uuid::nil(), user_id)
}

/// `"users:totp_pending_ct:<nil>:{user_id}"`: the pending seed of an enrolment (0020).
/// Different from [`secret_aad`], so a pending ciphertext can never be read as a confirmed
/// seed.
pub fn pending_aad(user_id: Uuid) -> Aad {
    Aad::new("users", "totp_pending_ct", Uuid::nil(), user_id)
}

/// A decrypted base32 seed. Wiped on drop; `Debug` never prints it.
pub struct TotpSecret(Zeroizing<String>);

impl TotpSecret {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for TotpSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TotpSecret(..)")
    }
}

/// The user's TOTP seed, or `None` when the user has not enrolled.
///
/// Fails closed: a sealed seed without a keyring is `503 clinical-crypto-unavailable`,
/// never "not enrolled". Reads only `totp_secret_ct` (AUTH-10; plaintext seeds are sealed
/// by `asd-backend migrate` before 0022 drops their column).
pub async fn load_secret(
    db: &PgPool,
    keyring: Option<&Keyring>,
    user_id: Uuid,
) -> AppResult<Option<TotpSecret>> {
    let Some(row) = sqlx::query!(
        "SELECT totp_secret_ct, totp_secret_kek_id FROM users WHERE id = $1",
        user_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    match (row.totp_secret_ct, row.totp_secret_kek_id) {
        (Some(ct), Some(kek_id)) => {
            let keyring = keyring.ok_or(CryptoError::Unavailable)?;
            let plain = keyring.open_user_secret(&secret_aad(user_id), &kek_id, &ct)?;
            let text = String::from_utf8(plain.to_vec()).map_err(|_| CryptoError::NotUtf8)?;
            Ok(Some(TotpSecret(Zeroizing::new(text))))
        }
        _ => Ok(None),
    }
}

/// Stores (`Some`) or removes (`None`) the user's seed, sealed; resets the spent step.
/// The seed must be RFC 4648 base32 (`400 validation-failed` otherwise). For the
/// enrolment flow and operators; nothing else writes `totp_secret_ct`.
pub async fn set_secret(
    db: &PgPool,
    keyring: &Keyring,
    user_id: Uuid,
    secret: Option<&str>,
) -> AppResult<()> {
    let sealed = match secret {
        None => None,
        Some(s) => {
            if reauth::totp_code(s, 0).is_none() {
                return Err(AppError::Validation(vec![InvalidParam {
                    name: "totp_secret".into(),
                    reason: "must be RFC 4648 base32".into(),
                }]));
            }
            Some(keyring.seal_user_secret(&secret_aad(user_id), s.trim().as_bytes())?)
        }
    };
    let kek_id = sealed.as_ref().map(|_| keyring.kek_id().to_owned());
    sqlx::query!(
        "UPDATE users SET totp_secret_ct = $2, totp_secret_kek_id = $3, last_totp_step = NULL
         WHERE id = $1",
        user_id,
        sealed,
        kek_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Checks `code` against `secret` now (one step of skew either way) and spends it. `false`
/// for a wrong code, a malformed one, or one of a step already spent.
pub async fn verify_and_spend(
    db: &PgPool,
    user_id: Uuid,
    secret: &TotpSecret,
    code: &str,
) -> AppResult<bool> {
    let Some(step) = reauth::matched_totp_step(secret.as_str(), code, unix_now()) else {
        return Ok(false);
    };
    let Ok(step) = i64::try_from(step) else {
        return Ok(false);
    };
    let spent = sqlx::query_scalar!(
        r#"UPDATE users SET last_totp_step = $2
           WHERE id = $1 AND (last_totp_step IS NULL OR last_totp_step < $2)
           RETURNING 1 AS "one!""#,
        user_id,
        step
    )
    .fetch_optional(db)
    .await?;
    Ok(spent.is_some())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Whether plaintext seeds can exist and be converted: `users.totp_secret` (dropped by
/// 0022) and `users.totp_secret_ct` (added by 0016) both exist.
async fn legacy_column_present(db: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) = 2 FROM information_schema.columns
         WHERE table_schema = current_schema() AND table_name = 'users'
           AND column_name IN ('totp_secret', 'totp_secret_ct')",
    )
    .fetch_one(db)
    .await
}

/// Seeds still stored in clear (0016 back-fill pending); 0 before 0016 and after 0022.
pub async fn legacy_secret_count(db: &PgPool) -> Result<i64, sqlx::Error> {
    if !legacy_column_present(db).await? {
        return Ok(0);
    }
    // Runtime query: the column does not exist in the schema `query!` is checked against.
    sqlx::query_scalar("SELECT count(*) FROM users WHERE totp_secret IS NOT NULL")
        .fetch_one(db)
        .await
}

/// 0016 back-fill: seals every plaintext `users.totp_secret` into `totp_secret_ct` and
/// sets `totp_secret = NULL`, in batches (one transaction each, `FOR UPDATE SKIP LOCKED`),
/// with one `user.totp_secret_encrypt` audit row per user (no secret in it). Idempotent:
/// a second run finds nothing; before 0016 and after 0022 it does nothing. Returns the
/// rows converted. A seed that is not valid base32 is sealed as it is: it never verified
/// before and still does not (fail closed).
pub async fn encrypt_legacy_secrets(db: &PgPool, keyring: &Keyring) -> AppResult<u64> {
    use sqlx::Row as _;

    if !legacy_column_present(db).await? {
        return Ok(0);
    }
    let mut total = 0u64;
    loop {
        let mut tx = db.begin().await?;
        // Runtime queries: the column does not exist in the schema `query!` is checked
        // against (0022).
        let rows = sqlx::query(
            "SELECT id, org_id, totp_secret
             FROM users
             WHERE totp_secret IS NOT NULL
             ORDER BY id
             LIMIT $1
             FOR UPDATE SKIP LOCKED",
        )
        .bind(BACKFILL_BATCH)
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            tx.rollback().await?;
            break;
        }
        for r in &rows {
            let id: Uuid = r.try_get("id")?;
            let org_id: Option<Uuid> = r.try_get("org_id")?;
            let plain = Zeroizing::new(r.try_get::<String, _>("totp_secret")?.trim().to_owned());
            let sealed = keyring.seal_user_secret(&secret_aad(id), plain.as_bytes())?;
            sqlx::query(
                "UPDATE users SET totp_secret_ct = $2, totp_secret_kek_id = $3, totp_secret = NULL
                 WHERE id = $1",
            )
            .bind(id)
            .bind(sealed)
            .bind(keyring.kek_id())
            .execute(&mut *tx)
            .await?;
            let mut event = AuditEvent::system("user.totp_secret_encrypt", "user", id)
                .after(json!({"kek_id": keyring.kek_id(), "via": "migrate"}));
            if let Some(org) = org_id {
                event = event.org(org);
            }
            event.record(&mut tx).await?;
        }
        tx.commit().await?;
        total += rows.len() as u64;
    }
    Ok(total)
}

// ---- self-service enrolment (WP-B.11, AUTH-6..10) ---------------------------------------

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// A new seed: 20 bytes from the OS CSPRNG as RFC 4648 base32 without padding (32
/// characters).
pub fn new_secret() -> Zeroizing<String> {
    let bytes = Zeroizing::new(crate::auth::random_bytes::<SEED_BYTES>());
    let mut out = Zeroizing::new(String::with_capacity(SEED_BYTES * 8 / 5));
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes.iter() {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(BASE32_ALPHABET[((buf >> bits) & 31) as usize]));
        }
    }
    if bits > 0 {
        out.push(char::from(
            BASE32_ALPHABET[((buf << (5 - bits)) & 31) as usize],
        ));
    }
    out
}

/// RFC 3986 percent-encoding of everything but unreserved characters.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The Key Uri Format URL an authenticator app scans (AUTH-10):
/// `otpauth://totp/{issuer}:{email}?secret=..&issuer={issuer}&algorithm=SHA1&digits=6&period=30`,
/// label and issuer percent-encoded (`+` in an e-mail becomes `%2B`).
pub fn otpauth_url(email: &str, secret: &str) -> String {
    let issuer = percent_encode(TOTP_ISSUER);
    format!(
        "otpauth://totp/{issuer}:{}?secret={}&issuer={issuer}&algorithm=SHA1&digits=6&period={}",
        percent_encode(email),
        percent_encode(secret),
        reauth::TOTP_STEP_SECS
    )
}

/// A started enrolment: the seed in clear (for the response only), its URL and expiry.
pub struct PendingEnrolment {
    pub secret: Zeroizing<String>,
    pub otpauth_url: String,
    pub expires_at: DateTime<Utc>,
}

impl fmt::Debug for PendingEnrolment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingEnrolment")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

/// AUTH-6 after the password check: generates a seed, stores it sealed as the pending
/// seed (replacing any earlier one) for [`PENDING_TTL_SECS`] and records `audit` in the
/// same transaction. `409 totp-already-enrolled` when the user has a confirmed seed.
pub async fn start_enrolment(
    db: &PgPool,
    keyring: &Keyring,
    user_id: Uuid,
    audit: AuditEvent,
) -> AppResult<PendingEnrolment> {
    // Fails closed on a legacy plaintext seed (503) like login does.
    if load_secret(db, Some(keyring), user_id).await?.is_some() {
        return Err(already_enrolled());
    }
    let secret = new_secret();
    let sealed = keyring.seal_user_secret(&pending_aad(user_id), secret.as_bytes())?;
    let mut tx = db.begin().await?;
    let row = sqlx::query!(
        r#"UPDATE users
           SET totp_pending_ct = $2, totp_pending_kek_id = $3,
               totp_pending_expires_at = now() + make_interval(secs => $4)
           WHERE id = $1 AND totp_secret_ct IS NULL
           RETURNING email, totp_pending_expires_at AS "expires_at!""#,
        user_id,
        sealed,
        keyring.kek_id(),
        PENDING_TTL_SECS as f64
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(already_enrolled)?;
    audit
        .after(json!({"expires_at": row.expires_at, "kek_id": keyring.kek_id()}))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    let otpauth_url = otpauth_url(&row.email, &secret);
    Ok(PendingEnrolment {
        secret,
        otpauth_url,
        expires_at: row.expires_at,
    })
}

fn already_enrolled() -> AppError {
    AppError::Conflict {
        slug: asd_contract::problem::TOTP_ALREADY_ENROLLED,
        detail: "TOTP is already enabled; disable it before enrolling again".into(),
    }
}

/// What [`confirm_enrolment`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    /// The pending seed is now the user's seed; the confirming step is spent.
    Enrolled,
    /// The code does not match the pending seed (nothing changed).
    InvalidCode,
    /// No pending seed, or past its expiry (nothing changed).
    Expired,
}

/// AUTH-7: checks `code` against the pending seed (one step of skew) and, when it matches,
/// in one `UPDATE` moves the seed to `totp_secret_ct` (re-sealed under [`secret_aad`]),
/// clears the pending columns and spends the matched step; records `audit` in the same
/// transaction. The row is locked for the check, so two confirms cannot both succeed: the
/// second finds the user enrolled with nothing pending and gets `409 totp-already-enrolled`.
/// An expired pending seed is cleared before answering [`ConfirmOutcome::Expired`].
pub async fn confirm_enrolment(
    db: &PgPool,
    keyring: &Keyring,
    user_id: Uuid,
    code: &str,
    audit: AuditEvent,
) -> AppResult<ConfirmOutcome> {
    let mut tx = db.begin().await?;
    let row = sqlx::query!(
        r#"SELECT totp_pending_ct, totp_pending_kek_id,
                  COALESCE(totp_pending_expires_at > now(), false) AS "live!",
                  totp_secret_ct IS NOT NULL AS "enrolled!"
           FROM users WHERE id = $1 FOR UPDATE"#,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Ok(ConfirmOutcome::Expired);
    };
    if row.enrolled && row.totp_pending_ct.is_none() {
        // Confirmed already (a second confirm, or the loser of two concurrent ones).
        return Err(already_enrolled());
    }
    if !row.live && row.totp_pending_ct.is_some() {
        // Expired: drop the stale seed now rather than wait for the housekeeping sweep.
        sqlx::query!(
            "UPDATE users
             SET totp_pending_ct = NULL, totp_pending_kek_id = NULL, totp_pending_expires_at = NULL
             WHERE id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(ConfirmOutcome::Expired);
    }
    let (true, Some(ct), Some(kek_id)) = (row.live, row.totp_pending_ct, row.totp_pending_kek_id)
    else {
        return Ok(ConfirmOutcome::Expired);
    };
    let plain = keyring.open_user_secret(&pending_aad(user_id), &kek_id, &ct)?;
    let seed = Zeroizing::new(String::from_utf8(plain.to_vec()).map_err(|_| CryptoError::NotUtf8)?);
    let Some(step) =
        reauth::matched_totp_step(&seed, code, unix_now()).and_then(|s| i64::try_from(s).ok())
    else {
        return Ok(ConfirmOutcome::InvalidCode);
    };
    let sealed = keyring.seal_user_secret(&secret_aad(user_id), seed.as_bytes())?;
    let moved = sqlx::query!(
        "UPDATE users
         SET totp_secret_ct = $2, totp_secret_kek_id = $3, last_totp_step = $4,
             totp_pending_ct = NULL, totp_pending_kek_id = NULL, totp_pending_expires_at = NULL
         WHERE id = $1 AND totp_secret_ct IS NULL",
        user_id,
        sealed,
        keyring.kek_id(),
        step
    )
    .execute(&mut *tx)
    .await?;
    if moved.rows_affected() == 0 {
        return Err(already_enrolled());
    }
    audit
        .after(json!({"totp_enrolled": true, "kek_id": keyring.kek_id()}))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(ConfirmOutcome::Enrolled)
}

/// What [`disable`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisableOutcome {
    Disabled,
    NotEnrolled,
    /// Enrolled, but no code was given.
    CodeRequired,
    /// Wrong, malformed or already spent code.
    InvalidCode,
}

/// AUTH-8 after the password check: verifies and spends `code` ([`verify_and_spend`]),
/// then clears the seed, the spent step and any pending seed, recording `audit` in the
/// same transaction.
pub async fn disable(
    db: &PgPool,
    keyring: Option<&Keyring>,
    user_id: Uuid,
    code: Option<&str>,
    audit: AuditEvent,
) -> AppResult<DisableOutcome> {
    let Some(secret) = load_secret(db, keyring, user_id).await? else {
        return Ok(DisableOutcome::NotEnrolled);
    };
    let Some(code) = code.filter(|c| !c.trim().is_empty()) else {
        return Ok(DisableOutcome::CodeRequired);
    };
    if !verify_and_spend(db, user_id, &secret, code).await? {
        return Ok(DisableOutcome::InvalidCode);
    }
    let mut tx = db.begin().await?;
    clear_all(&mut tx, user_id).await?;
    audit
        .after(json!({"totp_enrolled": false}))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(DisableOutcome::Disabled)
}

/// AUTH-9 (operator `admin reset-totp`, wired by WP-B.14): clears the seed, the spent step
/// and any pending seed, with audit `user.totp_reset` (`actor: operator`). `false` when no
/// such user exists.
pub async fn reset(db: &PgPool, user_id: Uuid) -> AppResult<bool> {
    let mut tx = db.begin().await?;
    let Some(org_id) = clear_all(&mut tx, user_id).await? else {
        tx.rollback().await?;
        return Ok(false);
    };
    let mut event = AuditEvent::system("user.totp_reset", "user", user_id)
        .after(json!({"actor": "operator", "totp_enrolled": false}));
    if let Some(org) = org_id {
        event = event.org(org);
    }
    event.record(&mut tx).await?;
    tx.commit().await?;
    Ok(true)
}

/// Clears every TOTP column of the user; `Some(org_id)` when the user exists.
async fn clear_all(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
) -> AppResult<Option<Option<Uuid>>> {
    let row = sqlx::query_scalar!(
        "UPDATE users
         SET totp_secret_ct = NULL, totp_secret_kek_id = NULL,
             last_totp_step = NULL, totp_pending_ct = NULL, totp_pending_kek_id = NULL, totp_pending_expires_at = NULL
         WHERE id = $1
         RETURNING org_id",
        user_id
    )
    .fetch_optional(conn)
    .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_keeps_only_unreserved() {
        assert_eq!(percent_encode("a+b@x.org"), "a%2Bb%40x.org");
        assert_eq!(percent_encode("Eye Gaze Console"), "Eye%20Gaze%20Console");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    #[test]
    fn new_secret_round_trips_through_the_verifier() {
        let s = new_secret();
        assert_eq!(s.len(), 32);
        assert!(reauth::totp_code(&s, 59).is_some());
    }
}
