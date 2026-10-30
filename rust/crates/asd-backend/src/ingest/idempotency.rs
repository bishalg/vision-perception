//! owner: WP-B.3
//!
//! Idempotent session ingest (ADR-B5, CON-17, trap 7). Inside the ingest transaction the
//! key row is inserted **first** with `ON CONFLICT (scope, key) DO NOTHING`; zero rows back
//! means the key exists (Postgres waits for a concurrent inserter to commit or roll back
//! before deciding), and only then is the stored row read to choose replay (same body
//! hash, `200`) or `409 idempotency-key-reused`. Never `SELECT` first.
//!
//! Keys are scoped per org (`sessions:{org_id}`), so two orgs cannot see each other's keys.

use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Idempotency scope of `POST /v1/sessions` for one org.
pub fn scope(org_id: Uuid) -> String {
    format!("sessions:{org_id}")
}

/// SHA-256 over the RFC 8785 canonical form of the body.
pub fn request_hash(body: &Value) -> AppResult<[u8; 32]> {
    let canonical = asd_contract::signing::canonical_bytes(body)
        .map_err(|e| AppError::Internal(format!("canonicalise body: {e}")))?;
    Ok(Sha256::digest(&canonical).into())
}

/// Result of claiming a key.
#[derive(Debug, Clone, PartialEq)]
pub enum Claim {
    /// The key is new; this transaction owns it and must call [`complete`].
    New,
    /// Same key, same body: replay the stored response.
    Replay { status_code: u16, response: Value },
    /// Same key, different body.
    Mismatch,
}

/// Claims `(scope, key)` for `hash` in the caller's transaction.
pub async fn claim(
    conn: &mut PgConnection,
    scope: &str,
    key: &str,
    hash: &[u8; 32],
) -> AppResult<Claim> {
    let inserted = sqlx::query_scalar!(
        r#"INSERT INTO idempotency_keys (scope, key, request_hash)
           VALUES ($1, $2, $3)
           ON CONFLICT (scope, key) DO NOTHING
           RETURNING 1 AS "one!: i32""#,
        scope,
        key,
        &hash[..],
    )
    .fetch_optional(&mut *conn)
    .await?;
    if inserted.is_some() {
        return Ok(Claim::New);
    }

    let stored = sqlx::query!(
        r#"SELECT request_hash, status_code, response_body
           FROM idempotency_keys WHERE scope = $1 AND key = $2"#,
        scope,
        key,
    )
    .fetch_one(&mut *conn)
    .await?;
    if stored.request_hash.as_slice() != hash.as_slice() {
        return Ok(Claim::Mismatch);
    }
    match (stored.status_code, stored.response_body) {
        (Some(code), Some(response)) => Ok(Claim::Replay {
            status_code: u16::try_from(code).unwrap_or(200),
            response,
        }),
        // A committed key always has its response (written in the same transaction).
        _ => Err(AppError::Internal(
            "idempotency key without response".into(),
        )),
    }
}

/// Stores the response for a key claimed with [`Claim::New`].
pub async fn complete(
    conn: &mut PgConnection,
    scope: &str,
    key: &str,
    status_code: u16,
    response: &Value,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE idempotency_keys SET status_code = $3, response_body = $4 WHERE scope = $1 AND key = $2",
        scope,
        key,
        i32::from(status_code),
        response,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hash_ignores_key_order_and_whitespace() {
        let a = json!({"b": 1, "a": [1.0, {"y": true, "x": null}]});
        let b: Value =
            serde_json::from_str(r#"{ "a" : [1.0,{"x":null,"y":true}], "b":1 }"#).unwrap();
        assert_eq!(request_hash(&a).unwrap(), request_hash(&b).unwrap());
        let c = json!({"b": 2, "a": [1.0, {"y": true, "x": null}]});
        assert_ne!(request_hash(&a).unwrap(), request_hash(&c).unwrap());
    }

    #[test]
    fn scope_is_per_org() {
        let org = Uuid::nil();
        assert_eq!(scope(org), "sessions:00000000-0000-0000-0000-000000000000");
    }
}
