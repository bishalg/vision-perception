//! owner: WP-B.2
//!
//! AUTH-1: Argon2id with m = 19456 KiB, t = 2, p = 1 and a 16-byte salt (the OWASP
//! minimum), PHC string format in `users.pw_hash`. Login failure costs the same as
//! success: an unknown or disabled user is verified against a dummy hash.
//!
//! Hashing takes tens of milliseconds of CPU; async callers use the `*_blocking` variants,
//! which run on Tokio's blocking pool.

use std::sync::LazyLock;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::AppError;
use crate::problem::InvalidParam;

pub const MEMORY_KIB: u32 = 19_456;
pub const ITERATIONS: u32 = 2;
pub const PARALLELISM: u32 = 1;
pub const SALT_LEN: usize = 16;

/// New passwords: at least 12 characters, at most 256 bytes (bounds hashing cost).
pub const MIN_LEN: usize = 12;
pub const MAX_BYTES: usize = 256;

fn argon2() -> Argon2<'static> {
    let params =
        Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, Some(32)).expect("valid Argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// A hash of a random password with the production parameters; verifying against it costs
/// what a real verification costs.
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    let random = super::random_bytes::<16>();
    hash(&String::from_utf8_lossy(&random)).expect("dummy hash")
});

/// PHC string `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`.
pub fn hash(password: &str) -> Result<String, AppError> {
    let salt = SaltString::encode_b64(&super::random_bytes::<SALT_LEN>())
        .map_err(|e| AppError::Internal(format!("salt: {e}")))?;
    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("argon2: {e}")))
}

/// Verifies `password` against a stored PHC hash (parameters are read from the hash).
/// An unparsable hash never verifies.
pub fn verify(stored: &str, password: &str) -> bool {
    match PasswordHash::new(stored) {
        Ok(parsed) => argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Verifies against `stored`, or against the dummy hash when there is no user, so both
/// paths take the same time. Returns `false` whenever `stored` is `None`.
pub fn verify_or_dummy(stored: Option<&str>, password: &str) -> bool {
    match stored {
        Some(h) => verify(h, password),
        None => {
            let _ = verify(&DUMMY_HASH, password);
            false
        }
    }
}

pub async fn hash_blocking(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash(&password))
        .await
        .map_err(|e| AppError::Internal(format!("hash task: {e}")))?
}

pub async fn verify_or_dummy_blocking(stored: Option<String>, password: String) -> bool {
    tokio::task::spawn_blocking(move || verify_or_dummy(stored.as_deref(), &password))
        .await
        .unwrap_or(false)
}

/// Policy for passwords set through the API.
pub fn validate_new(password: &str) -> Result<(), InvalidParam> {
    let reason = if password.chars().count() < MIN_LEN {
        format!("must be at least {MIN_LEN} characters")
    } else if password.len() > MAX_BYTES {
        format!("must be at most {MAX_BYTES} bytes")
    } else {
        return Ok(());
    };
    Err(InvalidParam {
        name: "password".into(),
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_uses_argon2id_with_the_spec_parameters_and_a_16_byte_salt() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{h}");
        let parsed = PasswordHash::new(&h).unwrap();
        let salt = parsed.salt.unwrap();
        let mut buf = [0u8; 64];
        assert_eq!(salt.decode_b64(&mut buf).unwrap().len(), SALT_LEN);
        assert_eq!(parsed.hash.unwrap().len(), 32);
        assert_ne!(h, hash("correct horse battery staple").unwrap(), "salted");
    }

    #[test]
    fn verify_accepts_the_password_and_rejects_others() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(verify(&h, "correct horse battery staple"));
        assert!(!verify(&h, "Correct horse battery staple"));
        assert!(!verify("not a phc string", "x"));
        assert!(!verify_or_dummy(None, "correct horse battery staple"));
        assert!(verify_or_dummy(Some(&h), "correct horse battery staple"));
    }

    #[test]
    fn dummy_hash_has_production_cost() {
        assert!(DUMMY_HASH.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
    }

    #[test]
    fn new_password_policy() {
        assert!(validate_new("short").is_err());
        assert!(validate_new("twelve chars").is_ok());
        assert!(validate_new(&"x".repeat(257)).is_err());
    }
}
