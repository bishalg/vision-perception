//! owner: WP-B.3
//!
//! Session ingest (ING-1..3): validate, claim the idempotency key, upsert the participant,
//! insert `sessions` + `session_clip_scores` + review items + one audit row, store the
//! response, all in one transaction. A replay (same key, same body) answers `200` with the
//! stored body, `duplicate: true` and fresh upload URLs (CON-17, CON-18).
//!
//! ## Auth and consent
//!
//! The caller is the WP-B.2 device-plane [`DevicePrincipal`] (`Authorization: Bearer dvc_...`,
//! AUTH-3); its org scopes everything written. The ingest audit row is an
//! [`AuditEvent::device`] in the same transaction (AUTH-5). A participant whose consent was
//! withdrawn (`participants.withdrawn_at` set, ADR-B8 erasure flow) gets no new data: the
//! post, replays included, is refused with `409 participant-withdrawn` and nothing is stored.
//!
//! Schema 1.7 (WP-G.11): the SCH-4 tracking columns and the QS-5 calibrated distance are
//! derived in [`derive`], and `low_tracking_quality` is opened in the same transaction.
//!
//! Schema 1.8 (WP-G.12, SCH-5): `scoring_status` / `unscorable_reason` are stored as sent; an
//! `unscorable` session stores no social preference and no affect, its flag is
//! `unreliable`, and the `unscorable` review item is opened in the same transaction.
//!
//! Researcher toolkit (WP-X.2): `sessions.age_months` is set from `rawPayload.ageMonths`
//! (COH-3); the flag floor, flag thresholds, tracking re-check and review threshold are read
//! from the published scoring profile (INV-1, defaults when none is published); the ack of a
//! research-consented session carries `research_frames_upload_url` in proxy blob mode only
//! (RFL-3).

pub mod auto_review;
pub mod derive;
pub mod idempotency;
pub mod validate;

use asd_contract::api_v1::SessionAttentionFlag;
use asd_contract::problem as slugs;
use asd_contract::session_v1_6::{SessionIngestResponse, SessionIngestStatus};
use http::StatusCode;
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{DevicePrincipal, RequestMeta};
use crate::error::{AppError, AppResult};
use crate::routes::blobs;
use idempotency::Claim;
use validate::ValidatedSession;

pub fn flag_str(flag: SessionAttentionFlag) -> &'static str {
    match flag {
        SessionAttentionFlag::Green => "green",
        SessionAttentionFlag::Yellow => "yellow",
        SessionAttentionFlag::Red => "red",
        SessionAttentionFlag::Unreliable => "unreliable",
    }
}

/// Outcome of [`ingest`]: `201` for a new session, `200` for a replay. The response carries
/// `research_frames_upload_url` (RFL-3); the idempotency store keeps the body without it, so
/// a replay recomputes it like the other upload URLs (CON-17).
#[derive(Debug)]
pub struct Ingested {
    pub status: StatusCode,
    pub response: SessionIngestResponse,
}

/// RFL-3: the relative proxy URL of the sealed research-log upload, offered only in proxy
/// blob mode (a pre-signed PUT would bypass the server's encryption) and only for a
/// research-consented session (`rawPayload.normCollection.consented`), the only kind the
/// route accepts.
pub fn research_frames_upload_url(
    state: &AppState,
    req: &asd_contract::session_v1_8::SessionIngestRequest,
) -> Option<String> {
    (state.config.blob_mode == crate::config::BlobMode::Proxy && derive::norm_consented(req))
        .then(|| format!("/v1/sessions/{}/blobs/research_frames", req.session_id))
}

/// Runs ING-1..3 for a request that already passed [`validate::validate`].
pub async fn ingest(
    state: &AppState,
    device: DevicePrincipal,
    meta: &RequestMeta,
    v: ValidatedSession,
) -> AppResult<Ingested> {
    let req = &v.request;
    let org = device.org_id;
    let scope = idempotency::scope(org);
    let mut tx = state.db.begin().await?;

    // Checked before the idempotency claim, so a refused post leaves no key behind and a
    // replay of an earlier accepted session hands out no fresh upload URLs either.
    ensure_not_withdrawn(&mut tx, org, &req.participant_token).await?;

    match idempotency::claim(&mut tx, &scope, &v.idempotency_key, &v.request_hash).await? {
        Claim::Mismatch => {
            return Err(AppError::Conflict {
                slug: slugs::IDEMPOTENCY_KEY_REUSED,
                detail: "this X-Idempotency-Key was used with a different body".into(),
            })
        }
        Claim::Replay { response, .. } => {
            tx.rollback().await?;
            let mut response: SessionIngestResponse = serde_json::from_value(response)
                .map_err(|e| AppError::Internal(format!("stored response: {e}")))?;
            response.duplicate = true;
            let (frames, report) =
                blobs::upload_urls(state, org, &req.participant_token, &req.session_id).await?;
            response.frames_upload_url = frames;
            response.report_upload_url = report;
            response.research_frames_upload_url = research_frames_upload_url(state, req);
            return Ok(Ingested {
                status: StatusCode::OK,
                response,
            });
        }
        Claim::New => {}
    }
    // INV-1: every number the rules below read comes from the published scoring profile
    // (the defaults when none is published).
    let profile = auto_review::published_profile(&mut tx).await?;

    let participant_id = sqlx::query_scalar!(
        "INSERT INTO participants (org_id, token) VALUES ($1, $2)
         ON CONFLICT (org_id, token) DO UPDATE SET token = EXCLUDED.token
         RETURNING id",
        org,
        req.participant_token,
    )
    .fetch_one(&mut *tx)
    .await?;

    let flag = derive::attention_flag(req, &profile);
    let battery = req.battery.clone().flatten();
    let tracking = derive::tracking_columns(req);
    let (scoring_status, unscorable_reason) = derive::scoring_columns(req);
    let inserted = sqlx::query_scalar!(
        r#"INSERT INTO sessions (
               id, org_id, participant_id, participant_token, device_id, capture_source,
               started_at, ended_at, duration_sec, battery, bundle_version, schema_version,
               quality_verdict, valid_frame_ratio, calibrated_distance_cm, social_preference,
               eye_dwell, mouth_dwell, first_eye_latency_ms, affect, composite_score,
               composite_confidence, attention_flag, summary, idempotency_key,
               tracking_verdict, tracking_accuracy_deg, calibration_quality,
               calibration_model, tracking_pipeline_version, device_model, scoring_status,
               unscorable_reason, request_hash, age_months)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                   $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                   $31, $32, $33, $34, $35)
           ON CONFLICT DO NOTHING
           RETURNING 1 AS "one!: i32""#,
        req.session_id,
        org,
        participant_id,
        req.participant_token,
        device.device_id,
        enum_str(&req.capture_source)?,
        v.started_at,
        v.ended_at,
        req.duration_sec,
        battery,
        derive::bundle_version(req),
        req.schema_version,
        enum_str(&req.quality_verdict)?,
        req.valid_ratio,
        derive::calibrated_distance_cm(req),
        derive::social_preference(req),
        req.eye_dwell_ratio,
        req.mouth_dwell_ratio,
        req.first_eye_latency_ms,
        derive::affect(req),
        derive::composite_score(req),
        derive::composite_confidence(req),
        flag_str(flag),
        v.raw_payload,
        v.idempotency_key,
        tracking.verdict,
        tracking.accuracy_deg,
        tracking.calibration_quality,
        tracking.calibration_model,
        tracking.pipeline_version,
        tracking.device_model,
        scoring_status,
        unscorable_reason,
        &v.request_hash[..],
        derive::age_months(req),
    )
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_none() {
        // WP-B.12 (HK-4): the housekeeping sweep deletes idempotency keys after 7 days, so a
        // late post claims the key afresh and meets a `sessions` unique constraint (the
        // primary key, or the globally unique `idempotency_key`). The untargeted
        // `ON CONFLICT DO NOTHING` turns either into "no row"; the stored session then
        // decides: this org's session under this key with the same body is a replay
        // (`200`), anything else is `409 idempotency-key-reused`, never a `500`.
        if stored_replay(&mut tx, org, &v).await? {
            tx.rollback().await?;
            let (frames, report) =
                blobs::upload_urls(state, org, &req.participant_token, &req.session_id).await?;
            return Ok(Ingested {
                status: StatusCode::OK,
                response: SessionIngestResponse {
                    session_id: req.session_id.clone(),
                    status: SessionIngestStatus::Accepted,
                    duplicate: true,
                    frames_upload_url: frames,
                    report_upload_url: report,
                    research_frames_upload_url: research_frames_upload_url(state, req),
                },
            });
        }
        return Err(AppError::Conflict {
            slug: slugs::IDEMPOTENCY_KEY_REUSED,
            detail: "this sessionId or X-Idempotency-Key is already taken by another request"
                .into(),
        });
    }

    let clips = derive::clip_rows(req.per_video.as_deref().unwrap_or_default());
    for (clip, details) in clips.into_iter().zip(&v.per_video_details) {
        sqlx::query!(
            "INSERT INTO session_clip_scores (
                 session_id, clip_id, clip_order, dwell_social, dwell_geometric, dwell_eye,
                 dwell_mouth, orienting_latency_ms, fixation_count, mean_fixation_ms, status,
                 score, weight, baseline_ref, details)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
            req.session_id,
            clip.clip_id,
            clip.order,
            clip.dwell_social,
            clip.dwell_geometric,
            clip.dwell_eye,
            clip.dwell_mouth,
            clip.orienting_latency_ms,
            clip.fixation_count,
            clip.mean_fixation_ms,
            clip.status,
            clip.score,
            clip.weight,
            clip.baseline_ref,
            details,
        )
        .execute(&mut *tx)
        .await?;
    }

    let settings = sqlx::query_scalar!("SELECT settings FROM organizations WHERE id = $1", org)
        .fetch_one(&mut *tx)
        .await?;
    // A 1.6 session has no tracking record, so it never gets `low_tracking_quality`.
    let low_tracking = auto_review::low_tracking_quality(
        req.raw_payload.tracking_quality.as_ref(),
        &profile.tracking_quality,
    );
    let reasons = auto_review::reasons(
        &req.session_id,
        req.valid_ratio,
        low_tracking,
        req.is_unscorable(),
        flag,
        derive::norm_consented(req),
        &auto_review::Rules::new(&profile, &settings),
    );
    auto_review::insert(&mut tx, org, &req.session_id, &reasons).await?;

    AuditEvent::device(&device, "session.ingest", "session", &req.session_id)
        .after(serde_json::json!({
            "attention_flag": flag_str(flag),
            "review_reasons": reasons.iter().map(|r| auto_review::reason_str(*r)).collect::<Vec<_>>(),
        }))
        .meta(meta)
        .record(&mut tx)
        .await?;

    let (frames, report) =
        blobs::upload_urls(state, org, &req.participant_token, &req.session_id).await?;
    let mut response = SessionIngestResponse {
        session_id: req.session_id.clone(),
        status: SessionIngestStatus::Accepted,
        duplicate: false,
        frames_upload_url: frames,
        report_upload_url: report,
        research_frames_upload_url: None,
    };
    // Stored without the research URL: a replay recomputes it (CON-17).
    let body = serde_json::to_value(&response)
        .map_err(|e| AppError::Internal(format!("response: {e}")))?;
    idempotency::complete(&mut tx, &scope, &v.idempotency_key, 201, &body).await?;
    tx.commit().await?;
    response.research_frames_upload_url = research_frames_upload_url(state, req);

    Ok(Ingested {
        status: StatusCode::CREATED,
        response,
    })
}

/// Whether the session row that blocked the insert is this very request stored earlier:
/// same id, same org, same idempotency key and the same body (`request_hash`; for rows
/// stored before migration 0021, `summary` equal to the posted `rawPayload` and the same
/// `participant_token`, so a repost cannot mint upload URLs for another participant).
async fn stored_replay(
    conn: &mut PgConnection,
    org: Uuid,
    v: &ValidatedSession,
) -> AppResult<bool> {
    let same = sqlx::query_scalar!(
        r#"SELECT COALESCE(request_hash = $4, summary = $5 AND participant_token = $6) AS "same"
           FROM sessions
           WHERE id = $1 AND org_id = $2 AND idempotency_key = $3"#,
        v.request.session_id,
        org,
        v.idempotency_key,
        &v.request_hash[..],
        v.raw_payload,
        v.request.participant_token,
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(same.flatten() == Some(true))
}

/// `409 participant-withdrawn` when the org's participant with this token has withdrawn consent
/// (`participants.withdrawn_at`). An unknown token is fine: ingest creates it.
pub(crate) async fn ensure_not_withdrawn(
    conn: &mut PgConnection,
    org: Uuid,
    participant_token: &str,
) -> AppResult<()> {
    let withdrawn = sqlx::query_scalar!(
        r#"SELECT withdrawn_at IS NOT NULL AS "withdrawn!" FROM participants
           WHERE org_id = $1 AND token = $2"#,
        org,
        participant_token,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if withdrawn == Some(true) {
        return Err(participant_withdrawn());
    }
    Ok(())
}

/// The refusal for data about a participant who withdrew consent.
pub(crate) fn participant_withdrawn() -> AppError {
    AppError::Conflict {
        slug: slugs::PARTICIPANT_WITHDRAWN,
        detail: "this participant has withdrawn consent; no new data is accepted".into(),
    }
}

/// The wire spelling of a unit enum (`truedepth`, `optimal`).
fn enum_str<T: serde::Serialize>(value: &T) -> AppResult<String> {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => Ok(s),
        _ => Err(AppError::Internal(
            "enum does not serialise to a string".into(),
        )),
    }
}
