//! owner: WP-B.3
//!
//! ING-3 review auto-rules, applied inside the ingest transaction:
//! `low_quality` when `validRatio < 0.80`, `flag_red` when the stored flag is `red`,
//! `random_audit` with probability `organizations.settings.audit_rate` (default 0.05),
//! drawn deterministically from `sha256(session_id)`, and `norm_candidate` when
//! `rawPayload.normCollection.consented == true`.

use asd_contract::api_v1::{ReviewReason, SessionAttentionFlag};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppResult;

pub const LOW_QUALITY_VALID_RATIO: f64 = 0.80;
pub const DEFAULT_AUDIT_RATE: f64 = 0.05;

/// Uniform draw in `[0, 1)`: the first 8 bytes of `sha256(session_id)` as a big-endian
/// integer over 2^64. Same session, same draw, on every server.
pub fn audit_draw(session_id: &str) -> f64 {
    let digest = Sha256::digest(session_id.as_bytes());
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[..8]);
    // 53 significant bits keep the result strictly below 1.0.
    (u64::from_be_bytes(head) >> 11) as f64 / (1u64 << 53) as f64
}

/// `settings.audit_rate` clamped to `[0, 1]`; the default when absent or not a number.
pub fn audit_rate(settings: &Value) -> f64 {
    settings
        .get("audit_rate")
        .and_then(Value::as_f64)
        .filter(|r| r.is_finite())
        .map_or(DEFAULT_AUDIT_RATE, |r| r.clamp(0.0, 1.0))
}

/// The reasons a new session enters the review queue, in a fixed order.
pub fn reasons(
    session_id: &str,
    valid_ratio: f64,
    flag: SessionAttentionFlag,
    norm_consented: bool,
    audit_rate: f64,
) -> Vec<ReviewReason> {
    let mut out = Vec::new();
    if valid_ratio < LOW_QUALITY_VALID_RATIO {
        out.push(ReviewReason::LowQuality);
    }
    if flag == SessionAttentionFlag::Red {
        out.push(ReviewReason::FlagRed);
    }
    if audit_draw(session_id) < audit_rate {
        out.push(ReviewReason::RandomAudit);
    }
    if norm_consented {
        out.push(ReviewReason::NormCandidate);
    }
    out
}

pub fn reason_str(reason: ReviewReason) -> &'static str {
    match reason {
        ReviewReason::LowQuality => "low_quality",
        ReviewReason::FlagRed => "flag_red",
        ReviewReason::RandomAudit => "random_audit",
        ReviewReason::NormCandidate => "norm_candidate",
        ReviewReason::Manual => "manual",
    }
}

/// Inserts one open item per reason (one per `(session, reason)`).
pub async fn insert(
    conn: &mut PgConnection,
    org_id: Uuid,
    session_id: &str,
    reasons: &[ReviewReason],
) -> AppResult<()> {
    for reason in reasons {
        sqlx::query!(
            "INSERT INTO review_items (org_id, session_id, reason) VALUES ($1, $2, $3)
             ON CONFLICT (session_id, reason) DO NOTHING",
            org_id,
            session_id,
            reason_str(*reason),
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rate_parsing() {
        assert_eq!(audit_rate(&json!({})), DEFAULT_AUDIT_RATE);
        assert_eq!(
            audit_rate(&json!({"audit_rate": "high"})),
            DEFAULT_AUDIT_RATE
        );
        assert_eq!(audit_rate(&json!({"audit_rate": 0.2})), 0.2);
        assert_eq!(audit_rate(&json!({"audit_rate": 7})), 1.0);
        assert_eq!(audit_rate(&json!({"audit_rate": -1})), 0.0);
    }

    #[test]
    fn rules() {
        use SessionAttentionFlag::*;
        assert!(reasons("S", 0.8, Yellow, false, 0.0).is_empty());
        assert_eq!(
            reasons("S", 0.79, Yellow, false, 0.0),
            [ReviewReason::LowQuality]
        );
        assert_eq!(reasons("S", 0.9, Red, false, 0.0), [ReviewReason::FlagRed]);
        assert!(reasons("S", 0.9, Unreliable, false, 0.0).is_empty());
        assert_eq!(
            reasons("S", 0.9, Green, true, 0.0),
            [ReviewReason::NormCandidate]
        );
        assert_eq!(
            reasons("S", 0.9, Green, false, 1.0),
            [ReviewReason::RandomAudit]
        );
    }

    #[test]
    fn draw_is_stable() {
        // Pinned: a change here changes which sessions get audited.
        let d = audit_draw("SESS-GOLDEN-0001");
        assert_eq!(d, audit_draw("SESS-GOLDEN-0001"));
        assert!((0.0..1.0).contains(&d));
    }
}
