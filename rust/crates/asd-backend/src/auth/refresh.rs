//! owner: WP-B.2
//!
//! AUTH-2 refresh tokens: `rt_` + 32 random bytes base64url, stored as SHA-256 with a
//! `family_id` (one family per login), valid 30 days, single use. `/auth/refresh` marks
//! the presented token `rotated_at` and issues the next one of the family. Presenting a
//! token that was already rotated out means it leaked (or a client replayed it): the whole
//! family is revoked, so the attacker's and the victim's newest tokens both stop working.
//!
//! Every function takes the caller's transaction; the row lock (`FOR UPDATE`) makes two
//! concurrent refreshes of one token serialise, and the second one counts as reuse.

use chrono::Utc;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppResult;

pub const PREFIX: &str = "rt_";
pub const TTL_DAYS: i64 = 30;

pub fn new_token() -> (String, [u8; 32]) {
    super::opaque_token(PREFIX)
}

pub fn hash_token(token: &str) -> [u8; 32] {
    super::sha256(token.as_bytes())
}

/// Issues a new token in `family_id` for `user_id`; returns the plain token.
pub async fn issue(conn: &mut PgConnection, user_id: Uuid, family_id: Uuid) -> AppResult<String> {
    let (token, hash) = new_token();
    let expires_at = Utc::now() + chrono::Duration::days(TTL_DAYS);
    sqlx::query!(
        "INSERT INTO refresh_tokens (user_id, family_id, token_hash, expires_at)
         VALUES ($1, $2, $3, $4)",
        user_id,
        family_id,
        hash.as_slice(),
        expires_at
    )
    .execute(&mut *conn)
    .await?;
    Ok(token)
}

/// What presenting a refresh token amounted to.
#[derive(Debug, PartialEq, Eq)]
pub enum Rotation {
    /// Valid: the presented token is now rotated out; `token` is its successor.
    Rotated {
        user_id: Uuid,
        family_id: Uuid,
        token: String,
    },
    /// Already rotated out: the family has just been revoked. The caller must commit.
    Reused { user_id: Uuid, family_id: Uuid },
    /// Unknown, malformed, expired or revoked.
    Invalid,
}

/// Validates and rotates `presented` (see the module docs).
pub async fn rotate(conn: &mut PgConnection, presented: &str) -> AppResult<Rotation> {
    if !super::is_opaque_token(presented, PREFIX) {
        return Ok(Rotation::Invalid);
    }
    let hash = hash_token(presented);
    let Some(row) = sqlx::query!(
        "SELECT id, user_id, family_id, expires_at, rotated_at, revoked_at
         FROM refresh_tokens WHERE token_hash = $1 FOR UPDATE",
        hash.as_slice()
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(Rotation::Invalid);
    };
    if row.revoked_at.is_some() {
        return Ok(Rotation::Invalid);
    }
    if row.rotated_at.is_some() {
        revoke_family(conn, row.family_id).await?;
        return Ok(Rotation::Reused {
            user_id: row.user_id,
            family_id: row.family_id,
        });
    }
    if row.expires_at <= Utc::now() {
        return Ok(Rotation::Invalid);
    }
    sqlx::query!(
        "UPDATE refresh_tokens SET rotated_at = now() WHERE id = $1",
        row.id
    )
    .execute(&mut *conn)
    .await?;
    let token = issue(conn, row.user_id, row.family_id).await?;
    Ok(Rotation::Rotated {
        user_id: row.user_id,
        family_id: row.family_id,
        token,
    })
}

/// Revokes every live token of a family; returns how many.
pub async fn revoke_family(conn: &mut PgConnection, family_id: Uuid) -> AppResult<u64> {
    let done = sqlx::query!(
        "UPDATE refresh_tokens SET revoked_at = now()
         WHERE family_id = $1 AND revoked_at IS NULL",
        family_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(done.rows_affected())
}

/// Logout: revokes the family of `presented`, whatever state the token is in. Returns
/// `(user_id, family_id)` when the token was known and something was revoked.
pub async fn revoke_family_of(
    conn: &mut PgConnection,
    presented: &str,
) -> AppResult<Option<(Uuid, Uuid)>> {
    if !super::is_opaque_token(presented, PREFIX) {
        return Ok(None);
    }
    let hash = hash_token(presented);
    let Some(row) = sqlx::query!(
        "SELECT user_id, family_id FROM refresh_tokens WHERE token_hash = $1",
        hash.as_slice()
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let revoked = revoke_family(conn, row.family_id).await?;
    Ok((revoked > 0).then_some((row.user_id, row.family_id)))
}

/// Revokes every live refresh token of a user (account disabled).
pub async fn revoke_user(conn: &mut PgConnection, user_id: Uuid) -> AppResult<u64> {
    let done = sqlx::query!(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        user_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(done.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_rt_prefixed_32_bytes() {
        let (token, hash) = new_token();
        assert!(super::super::is_opaque_token(&token, PREFIX), "{token}");
        assert_eq!(hash, hash_token(&token));
        assert_ne!(new_token().0, token);
    }
}
