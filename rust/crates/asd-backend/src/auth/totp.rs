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
//!   `asd-backend migrate`.
//!
//! Seeds and codes are never logged or put into an error.

use std::fmt;

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

/// `"users:totp_secret_ct:<nil>:{user_id}"`: users are not org data (a superadmin has no
/// org), so the org part is the nil UUID.
pub fn secret_aad(user_id: Uuid) -> Aad {
    Aad::new("users", "totp_secret_ct", Uuid::nil(), user_id)
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
/// Fails closed: a sealed seed without a keyring is `503 clinical-crypto-unavailable`; a
/// seed still in clear (0016 back-fill not run) is `503` and logged, never "not enrolled".
pub async fn load_secret(
    db: &PgPool,
    keyring: Option<&Keyring>,
    user_id: Uuid,
) -> AppResult<Option<TotpSecret>> {
    let Some(row) = sqlx::query!(
        r#"SELECT totp_secret_ct, totp_secret_kek_id,
                  (totp_secret IS NOT NULL) AS "legacy!"
           FROM users WHERE id = $1"#,
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
        _ if row.legacy => {
            tracing::error!(
                %user_id,
                "TOTP seed still stored in clear; run `asd-backend migrate` with ASD_DATA_KEK"
            );
            Err(AppError::Unavailable)
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
        "UPDATE users SET totp_secret = NULL, totp_secret_ct = $2, totp_secret_kek_id = $3,
                last_totp_step = NULL
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

/// Seeds still stored in clear (0016 back-fill pending).
pub async fn legacy_secret_count(db: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM users WHERE totp_secret IS NOT NULL"#)
        .fetch_one(db)
        .await
}

/// 0016 back-fill: seals every plaintext `users.totp_secret` into `totp_secret_ct` and
/// sets `totp_secret = NULL`, in batches (one transaction each, `FOR UPDATE SKIP LOCKED`),
/// with one `user.totp_secret_encrypt` audit row per user (no secret in it). Idempotent:
/// a second run finds nothing. Returns the rows converted. A seed that is not valid
/// base32 is sealed as it is: it never verified before and still does not (fail closed).
pub async fn encrypt_legacy_secrets(db: &PgPool, keyring: &Keyring) -> AppResult<u64> {
    let mut total = 0u64;
    loop {
        let mut tx = db.begin().await?;
        let rows = sqlx::query!(
            r#"SELECT id, org_id, totp_secret AS "totp_secret!"
               FROM users
               WHERE totp_secret IS NOT NULL
               ORDER BY id
               LIMIT $1
               FOR UPDATE SKIP LOCKED"#,
            BACKFILL_BATCH
        )
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            tx.rollback().await?;
            break;
        }
        for r in &rows {
            let plain = Zeroizing::new(r.totp_secret.trim().to_owned());
            let sealed = keyring.seal_user_secret(&secret_aad(r.id), plain.as_bytes())?;
            sqlx::query!(
                "UPDATE users SET totp_secret_ct = $2, totp_secret_kek_id = $3, totp_secret = NULL
                 WHERE id = $1",
                r.id,
                sealed,
                keyring.kek_id()
            )
            .execute(&mut *tx)
            .await?;
            let mut event = AuditEvent::system("user.totp_secret_encrypt", "user", r.id)
                .after(json!({"kek_id": keyring.kek_id(), "via": "migrate"}));
            if let Some(org) = r.org_id {
                event = event.org(org);
            }
            event.record(&mut tx).await?;
        }
        tx.commit().await?;
        total += rows.len() as u64;
    }
    Ok(total)
}
