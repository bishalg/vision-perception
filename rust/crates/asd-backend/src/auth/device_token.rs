//! owner: WP-B.2
//!
//! AUTH-3 device tokens: `dvc_` + 32 random bytes base64url (no padding), shown once at
//! registration, stored as SHA-256 in `devices.token_hash`, bound to `(device_id, org_id)`
//! and dead once `devices.revoked_at` is set or the token is rotated (CON-11).

use sqlx::PgPool;

pub use super::extractors::DevicePrincipal;
use crate::error::AppResult;

pub const PREFIX: &str = "dvc_";

/// A new token and the hash to store.
pub fn new_token() -> (String, [u8; 32]) {
    super::opaque_token(PREFIX)
}

pub fn hash_token(token: &str) -> [u8; 32] {
    super::sha256(token.as_bytes())
}

/// Looks up a live device by its token. Anything that is not a well-formed `dvc_` token
/// is rejected without a query.
pub async fn authenticate(pool: &PgPool, token: &str) -> AppResult<Option<DevicePrincipal>> {
    if !super::is_opaque_token(token, PREFIX) {
        return Ok(None);
    }
    let hash = hash_token(token);
    let row = sqlx::query!(
        "SELECT id, org_id FROM devices WHERE token_hash = $1 AND revoked_at IS NULL",
        hash.as_slice()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| DevicePrincipal {
        device_id: r.id,
        org_id: r.org_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_have_the_prefix_and_hash_to_the_stored_value() {
        let (token, hash) = new_token();
        assert!(token.starts_with("dvc_") && token.len() == 47, "{token}");
        assert_eq!(hash, hash_token(&token));
    }
}
