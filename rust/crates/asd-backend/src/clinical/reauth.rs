//! owner: WP-C.10
//!
//! SIGN-5 step-up re-authentication for sign and cosign (and later letter finalize): the
//! caller's password (Argon2id, AUTH-1) and, when the user has TOTP enrolled
//! (`users.totp_secret`), a current RFC 6238 code, verified in the same request.
//! Attempts are limited per user (AUTH-1: 10 per minute). The password and code are
//! verified and dropped; they are never stored, logged or put into an error.
//!
//! TOTP parameters are the authenticator-app defaults (RFC 6238, Key Uri Format): secret
//! in RFC 4648 base32, HMAC-SHA-1, 6 digits, 30-second steps, one step of clock skew either
//! way. No enrolment flow exists yet; this verifies what an enrolment would store.

use std::sync::Arc;

use aws_lc_rs::hmac;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{password, RateLimiter};
use crate::error::{AppError, AppResult};

/// AUTH-1 applied to step-up: attempts per user per minute.
pub const STEP_UP_ATTEMPTS_PER_MINUTE: u32 = 10;
/// RFC 6238 time step, seconds.
pub const TOTP_STEP_SECS: u64 = 30;
/// Accepted clock skew, in steps either side.
pub const TOTP_SKEW_STEPS: u64 = 1;
const TOTP_DIGITS: u32 = 6;

/// Per-user step-up limiter (one per router, shared by its clones).
#[derive(Debug)]
pub struct StepUpLimiter(pub RateLimiter<Uuid>);

impl StepUpLimiter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(RateLimiter::per_minute(STEP_UP_ATTEMPTS_PER_MINUTE)))
    }
}

/// Why a step-up failed. Only [`StepUpError::Failed`] is audited as a failed signature.
#[derive(Debug)]
pub enum StepUpError {
    /// Wrong password, missing or wrong TOTP code: `401 reauthentication-failed`.
    Failed,
    /// Over the attempt limit: `429`.
    RateLimited { retry_after_secs: u64 },
    /// Anything else (database, user vanished).
    Other(AppError),
}

impl From<StepUpError> for AppError {
    fn from(e: StepUpError) -> Self {
        match e {
            StepUpError::Failed => AppError::UnauthorizedWith {
                slug: asd_contract::problem::REAUTHENTICATION_FAILED,
                detail: "password or authenticator code is not correct".into(),
            },
            StepUpError::RateLimited { retry_after_secs } => {
                AppError::RateLimited { retry_after_secs }
            }
            StepUpError::Other(e) => e,
        }
    }
}

/// Verifies `password` (and `totp_code` when the user has TOTP) for `user_id`.
pub async fn step_up(
    db: &PgPool,
    limiter: &StepUpLimiter,
    user_id: Uuid,
    password_plain: &str,
    totp_code_given: Option<&str>,
) -> Result<(), StepUpError> {
    limiter
        .0
        .check(&user_id)
        .map_err(|retry_after_secs| StepUpError::RateLimited { retry_after_secs })?;
    let row = sqlx::query!(
        "SELECT pw_hash, totp_secret FROM users WHERE id = $1 AND status = 'active'",
        user_id
    )
    .fetch_optional(db)
    .await
    .map_err(|e| StepUpError::Other(e.into()))?;
    let (pw_hash, totp_secret) = match row {
        Some(r) => (Some(r.pw_hash), r.totp_secret),
        None => (None, None),
    };
    let password_ok = password::verify_or_dummy_blocking(pw_hash, password_plain.to_owned()).await;
    if !password_ok {
        return Err(StepUpError::Failed);
    }
    if let Some(secret) = totp_secret.filter(|s| !s.trim().is_empty()) {
        let Some(code) = totp_code_given else {
            return Err(StepUpError::Failed);
        };
        if !verify_totp(&secret, code, unix_now()) {
            return Err(StepUpError::Failed);
        }
    }
    Ok(())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// RFC 4648 base32 (case-insensitive; spaces, hyphens and `=` padding ignored).
fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 5 / 8);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.chars().filter(|c| !matches!(c, ' ' | '-' | '=')) {
        let v = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u32 - 'A' as u32,
            c @ '2'..='7' => c as u32 - '2' as u32 + 26,
            _ => return None,
        };
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buf >> bits) & 0xFF).ok()?);
        }
    }
    (!out.is_empty()).then_some(out)
}

fn hotp(key: &[u8], counter: u64) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    let mac = hmac::sign(&key, &counter.to_be_bytes());
    let mac = mac.as_ref();
    let offset = usize::from(mac[mac.len() - 1] & 0x0F);
    let bin = u32::from_be_bytes([
        mac[offset],
        mac[offset + 1],
        mac[offset + 2],
        mac[offset + 3],
    ]) & 0x7FFF_FFFF;
    format!("{:06}", bin % 10u32.pow(TOTP_DIGITS))
}

/// The RFC 6238 code for a base32 `secret` at `unix_time`; `None` for an unusable secret.
pub fn totp_code(secret: &str, unix_time: u64) -> Option<String> {
    let key = base32_decode(secret)?;
    Some(hotp(&key, unix_time / TOTP_STEP_SECS))
}

/// True when `code` matches the step of `unix_time` or one step either side. An unusable
/// stored secret never verifies (fail closed).
pub fn verify_totp(secret: &str, code: &str, unix_time: u64) -> bool {
    let Some(key) = base32_decode(secret) else {
        tracing::error!("stored TOTP secret is not base32; step-up refused");
        return false;
    };
    let code = code.trim();
    if code.len() != TOTP_DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let step = unix_time / TOTP_STEP_SECS;
    let mut ok = false;
    for counter in step.saturating_sub(TOTP_SKEW_STEPS)..=step + TOTP_SKEW_STEPS {
        // Constant-time comparison; every window step is checked.
        let expected = hotp(&key, counter);
        let diff = expected
            .bytes()
            .zip(code.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        ok |= diff == 0;
    }
    ok
}

/// Checks `meaning_ack` and the hash format of a sign body before any work.
pub fn check_sign_body(content_hash: &str, meaning_ack: bool) -> AppResult<()> {
    let mut errors = Vec::new();
    if !meaning_ack {
        errors.push(crate::problem::InvalidParam {
            name: "meaning_ack".into(),
            reason: "must be true: the signer accepts the meaning statement".into(),
        });
    }
    if content_hash.len() != 64 || !content_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        errors.push(crate::problem::InvalidParam {
            name: "content_hash".into(),
            reason: "must be 64 hex characters (SHA-256)".into(),
        });
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Validation(errors))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_round_trip() {
        assert_eq!(base32_decode("GEZDGNBVGY3TQOJQ").unwrap(), b"1234567890");
        assert_eq!(
            base32_decode("gezd gnbv-gy3t qojq====").unwrap(),
            b"1234567890"
        );
        assert!(base32_decode("1189").is_none());
        assert!(base32_decode("").is_none());
    }

    #[test]
    fn verify_accepts_one_step_of_skew() {
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let t = 1_234_567_890;
        let code = totp_code(secret, t).unwrap();
        assert!(verify_totp(secret, &code, t));
        assert!(verify_totp(secret, &code, t + TOTP_STEP_SECS));
        assert!(verify_totp(secret, &code, t - TOTP_STEP_SECS));
        assert!(!verify_totp(secret, &code, t + 3 * TOTP_STEP_SECS));
        assert!(!verify_totp(secret, "12345", t));
        assert!(!verify_totp(secret, "abcdef", t));
        assert!(!verify_totp("!!", &code, t));
    }

    #[test]
    fn sign_body_checks() {
        assert!(check_sign_body(&"a".repeat(64), true).is_ok());
        assert!(check_sign_body(&"a".repeat(64), false).is_err());
        assert!(check_sign_body("abc", true).is_err());
        assert!(check_sign_body(&"g".repeat(64), true).is_err());
    }
}
