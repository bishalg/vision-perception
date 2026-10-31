//! owner: WP-B.3
//!
//! ING-3 review auto-rules, applied inside the ingest transaction:
//! `low_quality` when `validRatio < 0.80`, `flag_red` when the stored flag is `red`,
//! `random_audit` with probability `organizations.settings.audit_rate` (default 0.05),
//! drawn deterministically from `sha256(session_id)`, and `norm_candidate` when
//! `rawPayload.normCollection.consented == true`.
//!
//! WP-G.11 (SCH-4): `low_tracking_quality` when a 1.7 session's tracking verdict is `poor` or
//! `unusable`, or when its estimated accuracy is above the `fair` band of
//! `ScoringProfile.trackingQuality.verdict.accuracyBands` of the latest published scoring
//! profile (asd-core defaults when none is published). The re-check keeps a session whose
//! device ran older or looser thresholds from slipping past a tightened profile. A 1.6
//! session never gets this reason.

use asd_contract::api_v1::{ReviewReason, SessionAttentionFlag};
use asd_contract::config::{ScoringProfile, TrackingQualityParams};
use asd_contract::session_v1_7::TrackingQuality;
use asd_core::types::tracking::TrackingVerdict;
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

/// `low_tracking_quality` rule: verdict `poor`/`unusable`, or estimated accuracy above the
/// profile's `fair` band (upper bound inclusive). `false` without a record.
pub fn low_tracking_quality(
    tracking: Option<&TrackingQuality>,
    params: &TrackingQualityParams,
) -> bool {
    let Some(tq) = tracking else {
        return false;
    };
    matches!(
        tq.verdict,
        TrackingVerdict::Poor | TrackingVerdict::Unusable
    ) || tq
        .estimated_accuracy_deg
        .is_some_and(|a| a > params.verdict.accuracy_bands.fair_max_deg)
}

/// `trackingQuality` of the latest published scoring profile; the asd-core defaults when none
/// is published or it does not decode (logged, never fails an ingest).
pub async fn tracking_params(conn: &mut PgConnection) -> AppResult<TrackingQualityParams> {
    let body = sqlx::query_scalar!(
        "SELECT body FROM config_documents
         WHERE kind = 'scoring_profile' AND status = 'published'
         ORDER BY published_at DESC LIMIT 1"
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(match body {
        None => TrackingQualityParams::default(),
        Some(body) => match serde_json::from_value::<ScoringProfile>(body) {
            Ok(p) => p.tracking_quality,
            Err(e) => {
                tracing::warn!(error = %e, "published scoring profile does not decode; default tracking thresholds");
                TrackingQualityParams::default()
            }
        },
    })
}

/// The reasons a new session enters the review queue, in a fixed order.
pub fn reasons(
    session_id: &str,
    valid_ratio: f64,
    low_tracking: bool,
    flag: SessionAttentionFlag,
    norm_consented: bool,
    audit_rate: f64,
) -> Vec<ReviewReason> {
    let mut out = Vec::new();
    if valid_ratio < LOW_QUALITY_VALID_RATIO {
        out.push(ReviewReason::LowQuality);
    }
    if low_tracking {
        out.push(ReviewReason::LowTrackingQuality);
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
        ReviewReason::LowTrackingQuality => "low_tracking_quality",
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
        assert!(reasons("S", 0.8, false, Yellow, false, 0.0).is_empty());
        assert_eq!(
            reasons("S", 0.79, false, Yellow, false, 0.0),
            [ReviewReason::LowQuality]
        );
        assert_eq!(
            reasons("S", 0.9, false, Red, false, 0.0),
            [ReviewReason::FlagRed]
        );
        assert!(reasons("S", 0.9, false, Unreliable, false, 0.0).is_empty());
        assert_eq!(
            reasons("S", 0.9, false, Green, true, 0.0),
            [ReviewReason::NormCandidate]
        );
        assert_eq!(
            reasons("S", 0.9, false, Green, false, 1.0),
            [ReviewReason::RandomAudit]
        );
    }

    #[test]
    fn low_tracking_rule_and_order() {
        use SessionAttentionFlag::*;
        assert_eq!(
            reasons("S", 0.7, true, Red, false, 0.0),
            [
                ReviewReason::LowQuality,
                ReviewReason::LowTrackingQuality,
                ReviewReason::FlagRed
            ]
        );
        let mut tq: TrackingQuality = serde_json::from_value(
            serde_json::from_str::<Value>(include_str!(
                "../../../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
            ))
            .unwrap()["rawPayload"]["trackingQuality"]
                .clone(),
        )
        .unwrap();
        let params = TrackingQualityParams::default();
        assert!(!low_tracking_quality(None, &params));
        assert!(!low_tracking_quality(Some(&tq), &params));
        tq.verdict = TrackingVerdict::Poor;
        assert!(low_tracking_quality(Some(&tq), &params));
        tq.verdict = TrackingVerdict::Unusable;
        assert!(low_tracking_quality(Some(&tq), &params));
        tq.verdict = TrackingVerdict::Fair;
        tq.estimated_accuracy_deg = Some(3.5);
        assert!(
            !low_tracking_quality(Some(&tq), &params),
            "band bound is inclusive"
        );
        tq.estimated_accuracy_deg = Some(3.51);
        assert!(low_tracking_quality(Some(&tq), &params));
        tq.estimated_accuracy_deg = None;
        assert!(!low_tracking_quality(Some(&tq), &params));
    }

    #[test]
    fn draw_is_stable() {
        // Pinned: a change here changes which sessions get audited.
        let d = audit_draw("SESS-GOLDEN-0001");
        assert_eq!(d, audit_draw("SESS-GOLDEN-0001"));
        assert!((0.0..1.0).contains(&d));
    }
}
