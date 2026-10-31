//! owner: WP-C.10
//!
//! Clinical review: the SIGN-1 state machine, the signed-content snapshot (SIGN-4), claim
//! and draft save (SIGN-2, SIGN-3), and the `ClinicalReview` read model (RBAC-4).
//!
//! Content columns are encrypted per version row (KEY-1, KEY-3): AAD
//! `"clinical_review_versions:{impression_ct|recommendations_ct|amend_reason_ct}:{org}:{version_id}"`.

use std::fmt;

use asd_contract::api_v1::SessionAttentionFlag;
use asd_contract::clinical::{
    content_hash as snapshot_hash, ClaimClinicalReviewRequest, ClinicalReview,
    ClinicalReviewSnapshot, ClinicalReviewState, ClinicalReviewVersion, ClinicalSignature,
    Disposition, ReviewVersionStatus, SignatureRole, SnapshotClip, SnapshotMeasurements,
    UpdateClinicalReviewRequest, MAX_REVIEW_TEXT_CHARS,
};
use asd_core::types::PerVideoScoreRow;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

use super::{cosign_required, ClinicalSettings};
use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{RequestMeta, UserPrincipal};
use crate::auth::rbac::{self, Action};
use crate::crypto::{Aad, ClinicalCrypto, Keyring};
use crate::error::{AppError, AppResult};
use crate::ingest::derive::metric_row;
use crate::problem::InvalidParam;
use crate::read::cursor;

// ---- state machine (SIGN-1) -----------------------------------------------------------------

/// What a request does to a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOp {
    /// `PATCH` (draft save).
    SaveDraft,
    /// First signature; `cosign_required` from the org's policy (SIGN-6).
    Sign { cosign_required: bool },
    /// Second signature (SIGN-6).
    Cosign,
    /// Start an amendment (SIGN-8).
    Amend,
}

impl ReviewOp {
    fn verb(self) -> &'static str {
        match self {
            Self::SaveDraft => "edit",
            Self::Sign { .. } => "sign",
            Self::Cosign => "cosign",
            Self::Amend => "amend",
        }
    }
}

/// A transition the state machine does not allow: `409 conflict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("cannot {} a review that is {}", .op.verb(), .state.as_str())]
pub struct Refused {
    pub state: ClinicalReviewState,
    pub op: ReviewOp,
}

impl From<Refused> for AppError {
    fn from(r: Refused) -> Self {
        AppError::Conflict {
            slug: asd_contract::problem::CONFLICT,
            detail: r.to_string(),
        }
    }
}

/// SIGN-1: `open → in_progress` (first draft save) → `awaiting_cosign` (signed, second
/// signature required) → `final`; `final | awaiting_cosign → amending` → back through
/// signing. Amending while `awaiting_cosign` withdraws the pending cosign (SIGN-8).
pub fn transition(
    state: ClinicalReviewState,
    op: ReviewOp,
) -> Result<ClinicalReviewState, Refused> {
    use ClinicalReviewState as S;
    let signed = |cosign_required: bool| {
        if cosign_required {
            S::AwaitingCosign
        } else {
            S::Final
        }
    };
    let next = match (state, op) {
        (S::Open | S::InProgress, ReviewOp::SaveDraft) => Some(S::InProgress),
        (S::Amending, ReviewOp::SaveDraft) => Some(S::Amending),
        (S::InProgress | S::Amending, ReviewOp::Sign { cosign_required }) => {
            Some(signed(cosign_required))
        }
        (S::AwaitingCosign, ReviewOp::Cosign) => Some(S::Final),
        (S::Final | S::AwaitingCosign, ReviewOp::Amend) => Some(S::Amending),
        _ => None,
    };
    next.ok_or(Refused { state, op })
}

pub(crate) fn parse_state(s: &str) -> AppResult<ClinicalReviewState> {
    ClinicalReviewState::parse(s)
        .ok_or_else(|| AppError::Internal("stored clinical review state is unknown".into()))
}

// ---- signed content (SIGN-4) ----------------------------------------------------------------

/// The stored session data a review shows (and a signature covers).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionFacts {
    pub session_id: String,
    pub org_id: Uuid,
    pub attention_flag: String,
    pub bundle_version: Option<String>,
    pub schema_version: Option<String>,
    pub measurements: SnapshotMeasurements,
    pub clips: Vec<SnapshotClip>,
}

/// `details.rows` of a stored clip; empty when absent or not decodable (as `read::clips`).
fn detail_rows(details: Option<&Value>) -> Vec<PerVideoScoreRow> {
    details
        .and_then(|d| d.get("rows"))
        .and_then(|rows| serde_json::from_value(rows.clone()).ok())
        .unwrap_or_default()
}

/// The session's stored measurements and clip rows; `None` for an unknown session.
pub async fn session_facts(
    conn: &mut PgConnection,
    session_id: &str,
) -> AppResult<Option<SessionFacts>> {
    let Some(s) = sqlx::query!(
        r#"SELECT id, org_id, attention_flag, bundle_version, schema_version, started_at,
                  duration_sec, quality_verdict, composite_score, composite_confidence,
                  valid_frame_ratio, calibrated_distance_cm, social_preference, eye_dwell,
                  mouth_dwell, first_eye_latency_ms
           FROM sessions WHERE id = $1"#,
        session_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let clips = sqlx::query!(
        r#"SELECT clip_id, clip_order, details FROM session_clip_scores
           WHERE session_id = $1 ORDER BY clip_order, clip_id"#,
        session_id,
    )
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|c| SnapshotClip {
        clip_id: c.clip_id,
        order: i64::from(c.clip_order),
        rows: detail_rows(c.details.as_ref())
            .iter()
            .map(metric_row)
            .collect(),
    })
    .collect();
    let flag: SessionAttentionFlag =
        serde_json::from_value(Value::String(s.attention_flag.clone())).map_err(|_| {
            AppError::Internal("stored attention_flag is not in the contract".into())
        })?;
    Ok(Some(SessionFacts {
        session_id: s.id,
        org_id: s.org_id,
        attention_flag: s.attention_flag,
        bundle_version: s.bundle_version,
        schema_version: Some(s.schema_version),
        measurements: SnapshotMeasurements {
            started_at: cursor::ts(s.started_at),
            duration_sec: s.duration_sec,
            quality_verdict: s.quality_verdict,
            attention_flag: flag,
            composite_score: s.composite_score,
            composite_confidence: s.composite_confidence,
            valid_frame_ratio: Some(s.valid_frame_ratio),
            calibrated_distance_cm: s.calibrated_distance_cm,
            social_preference: Some(s.social_preference),
            eye_dwell: s.eye_dwell,
            mouth_dwell: s.mouth_dwell,
            first_eye_latency_ms: s.first_eye_latency_ms,
        },
        clips,
    }))
}

/// The clinician's statements of one version (SIGN-2). `Debug` never prints the text.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ReviewContent {
    pub disposition: Option<Disposition>,
    pub impression: Option<String>,
    pub recommendations: Option<String>,
}

impl fmt::Debug for ReviewContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewContent")
            .field("disposition", &self.disposition)
            .field(
                "impression_chars",
                &self.impression.as_ref().map(|s| s.chars().count()),
            )
            .field(
                "recommendations_chars",
                &self.recommendations.as_ref().map(|s| s.chars().count()),
            )
            .finish()
    }
}

/// SIGN-4 snapshot of `facts` + `content` at `version_no`.
pub fn snapshot(
    facts: &SessionFacts,
    version_no: i32,
    content: &ReviewContent,
) -> ClinicalReviewSnapshot {
    ClinicalReviewSnapshot {
        session_id: facts.session_id.clone(),
        org_id: facts.org_id.to_string(),
        bundle_version: facts.bundle_version.clone(),
        schema_version: facts.schema_version.clone(),
        measurements: facts.measurements.clone(),
        clips: facts.clips.clone(),
        disposition: content.disposition,
        impression: content.impression.clone(),
        recommendations: content.recommendations.clone(),
        version_no: u32::try_from(version_no).unwrap_or(0),
    }
}

/// `SHA-256(JCS(snapshot))` (asd-contract, RFC 8785; trap 7).
pub fn content_hash(facts: &SessionFacts, version_no: i32, content: &ReviewContent) -> [u8; 32] {
    snapshot_hash(&snapshot(facts, version_no, content))
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

// ---- rows -----------------------------------------------------------------------------------

/// A `clinical_reviews` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub session_id: String,
    pub state: ClinicalReviewState,
    pub assigned_to: Option<Uuid>,
    pub cosign_required: bool,
    pub row_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! review_row {
    ($r:expr) => {{
        let r = $r;
        ReviewRow {
            id: r.id,
            org_id: r.org_id,
            session_id: r.session_id,
            state: parse_state(&r.state)?,
            assigned_to: r.assigned_to,
            cosign_required: r.cosign_required,
            row_version: r.row_version,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }};
}

pub async fn find_review<'e>(db: impl PgExecutor<'e>, id: Uuid) -> AppResult<Option<ReviewRow>> {
    let row = sqlx::query!(
        r#"SELECT id, org_id, session_id, state, assigned_to, cosign_required, row_version,
                  created_at, updated_at
           FROM clinical_reviews WHERE id = $1"#,
        id,
    )
    .fetch_optional(db)
    .await?;
    Ok(match row {
        Some(r) => Some(review_row!(r)),
        None => None,
    })
}

pub async fn find_review_by_session<'e>(
    db: impl PgExecutor<'e>,
    session_id: &str,
) -> AppResult<Option<ReviewRow>> {
    let row = sqlx::query!(
        r#"SELECT id, org_id, session_id, state, assigned_to, cosign_required, row_version,
                  created_at, updated_at
           FROM clinical_reviews WHERE session_id = $1"#,
        session_id,
    )
    .fetch_optional(db)
    .await?;
    Ok(match row {
        Some(r) => Some(review_row!(r)),
        None => None,
    })
}

/// The review row under `FOR UPDATE` (SIGN-3, the concurrent-sign guard).
pub async fn lock_review(conn: &mut PgConnection, id: Uuid) -> AppResult<ReviewRow> {
    let r = sqlx::query!(
        r#"SELECT id, org_id, session_id, state, assigned_to, cosign_required, row_version,
                  created_at, updated_at
           FROM clinical_reviews WHERE id = $1 FOR UPDATE"#,
        id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(review_row!(r))
}

/// A `clinical_review_versions` row (ciphertext, not plaintext).
#[derive(Clone, PartialEq, Eq)]
pub struct VersionRow {
    pub id: Uuid,
    pub version_no: i32,
    pub status: ReviewVersionStatus,
    pub disposition: Option<Disposition>,
    pub impression_ct: Option<Vec<u8>>,
    pub impression_key_id: Option<Uuid>,
    pub recommendations_ct: Option<Vec<u8>>,
    pub recommendations_key_id: Option<Uuid>,
    pub amend_reason_ct: Option<Vec<u8>>,
    pub amend_reason_key_id: Option<Uuid>,
    pub content_hash: Option<Vec<u8>>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub signed_at: Option<DateTime<Utc>>,
}

impl fmt::Debug for VersionRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VersionRow")
            .field("id", &self.id)
            .field("version_no", &self.version_no)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

macro_rules! version_row {
    ($r:expr) => {{
        let r = $r;
        VersionRow {
            id: r.id,
            version_no: r.version_no,
            status: ReviewVersionStatus::parse(&r.status)
                .ok_or_else(|| AppError::Internal("stored version status is unknown".into()))?,
            disposition: match r.disposition.as_deref() {
                Some(d) => {
                    Some(Disposition::parse(d).ok_or_else(|| {
                        AppError::Internal("stored disposition is unknown".into())
                    })?)
                }
                None => None,
            },
            impression_ct: r.impression_ct,
            impression_key_id: r.impression_key_id,
            recommendations_ct: r.recommendations_ct,
            recommendations_key_id: r.recommendations_key_id,
            amend_reason_ct: r.amend_reason_ct,
            amend_reason_key_id: r.amend_reason_key_id,
            content_hash: r.content_hash,
            created_by: r.created_by,
            created_at: r.created_at,
            signed_at: r.signed_at,
        }
    }};
}

/// The version being edited, or the latest one (highest `version_no`).
pub async fn current_version<'e>(
    db: impl PgExecutor<'e>,
    review_id: Uuid,
) -> AppResult<VersionRow> {
    let r = sqlx::query!(
        r#"SELECT id, version_no, status, disposition, impression_ct, impression_key_id,
                  recommendations_ct, recommendations_key_id, amend_reason_ct,
                  amend_reason_key_id, content_hash, created_by, created_at, signed_at
           FROM clinical_review_versions WHERE review_id = $1
           ORDER BY version_no DESC LIMIT 1"#,
        review_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::Internal("clinical review has no version".into()))?;
    Ok(version_row!(r))
}

/// The latest `signed` version (the one awaiting cosign, or the one an amendment copies).
pub async fn latest_signed_version<'e>(
    db: impl PgExecutor<'e>,
    review_id: Uuid,
) -> AppResult<Option<VersionRow>> {
    let r = sqlx::query!(
        r#"SELECT id, version_no, status, disposition, impression_ct, impression_key_id,
                  recommendations_ct, recommendations_key_id, amend_reason_ct,
                  amend_reason_key_id, content_hash, created_by, created_at, signed_at
           FROM clinical_review_versions WHERE review_id = $1 AND status = 'signed'
           ORDER BY version_no DESC LIMIT 1"#,
        review_id,
    )
    .fetch_optional(db)
    .await?;
    Ok(match r {
        Some(r) => Some(version_row!(r)),
        None => None,
    })
}

// ---- encryption -----------------------------------------------------------------------------

pub const VERSIONS_TABLE: &str = "clinical_review_versions";

/// AAD of a version's encrypted column (`column` = the `_ct` name).
pub fn aad(org: Uuid, version_id: Uuid, column: &str) -> Aad {
    Aad::new(VERSIONS_TABLE, column, org, version_id)
}

/// A ciphertext column pair (`<col>_ct`, `<col>_key_id`).
pub type Sealed = (Option<Vec<u8>>, Option<Uuid>);

/// Encrypts `text` for the version row; `None` stays `NULL`.
pub async fn seal(
    keyring: &Keyring,
    org: Uuid,
    version_id: Uuid,
    column: &str,
    text: Option<&str>,
) -> AppResult<Sealed> {
    match text {
        Some(t) => {
            let s = keyring
                .encrypt_str(org, &aad(org, version_id, column), t)
                .await?;
            Ok((Some(s.ciphertext), Some(s.key_id)))
        }
        None => Ok((None, None)),
    }
}

async fn open(
    keyring: &Keyring,
    org: Uuid,
    version_id: Uuid,
    column: &str,
    ct: Option<&[u8]>,
    key_id: Option<Uuid>,
) -> AppResult<Option<String>> {
    match (ct, key_id) {
        (Some(ct), Some(key_id)) => Ok(Some(
            keyring
                .decrypt_string(org, &aad(org, version_id, column), key_id, ct)
                .await
                .inspect_err(|e| {
                    tracing::error!(%version_id, column, error = %e, "clinical review decrypt failed");
                })?,
        )),
        _ => Ok(None),
    }
}

/// Decrypts a version's content and amendment reason.
pub async fn decrypt_content(
    keyring: &Keyring,
    org: Uuid,
    v: &VersionRow,
) -> AppResult<(ReviewContent, Option<String>)> {
    let impression = open(
        keyring,
        org,
        v.id,
        "impression_ct",
        v.impression_ct.as_deref(),
        v.impression_key_id,
    )
    .await?;
    let recommendations = open(
        keyring,
        org,
        v.id,
        "recommendations_ct",
        v.recommendations_ct.as_deref(),
        v.recommendations_key_id,
    )
    .await?;
    let reason = open(
        keyring,
        org,
        v.id,
        "amend_reason_ct",
        v.amend_reason_ct.as_deref(),
        v.amend_reason_key_id,
    )
    .await?;
    Ok((
        ReviewContent {
            disposition: v.disposition,
            impression,
            recommendations,
        },
        reason,
    ))
}

fn has_ciphertext(v: &VersionRow) -> bool {
    v.impression_ct.is_some() || v.recommendations_ct.is_some() || v.amend_reason_ct.is_some()
}

// ---- read model (RBAC-4) --------------------------------------------------------------------

async fn signatures<'e>(
    db: impl PgExecutor<'e>,
    review_id: Uuid,
) -> AppResult<Vec<ClinicalSignature>> {
    let rows = sqlx::query!(
        r#"SELECT s.id, v.version_no, s.role, s.signer_id, s.signer_display_name,
                  s.signer_registration, s.meaning, s.content_hash, s.signed_at
           FROM clinical_signatures s JOIN clinical_review_versions v ON v.id = s.version_id
           WHERE s.review_id = $1
           ORDER BY s.signed_at, v.version_no, s.role DESC"#,
        review_id,
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ClinicalSignature {
                id: r.id.to_string(),
                version_no: u32::try_from(r.version_no).unwrap_or(0),
                role: SignatureRole::parse(&r.role)
                    .ok_or_else(|| AppError::Internal("stored signature role is unknown".into()))?,
                signer_id: r.signer_id.to_string(),
                signer_display_name: r.signer_display_name,
                signer_registration: r.signer_registration,
                meaning: r.meaning,
                content_hash: hex(&r.content_hash),
                signed_at: cursor::ts(r.signed_at),
            })
        })
        .collect()
}

/// `GET /v1/clinical-reviews/{id}` body for `principal`. Impression, recommendations and
/// the amendment reason are present only with `ClinicalContentRead` (RBAC-4); reading them
/// needs the keyring (503 without it).
pub async fn read_model(
    state: &AppState,
    principal: &UserPrincipal,
    review_id: Uuid,
) -> AppResult<ClinicalReview> {
    let review = find_review(&state.db, review_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let version = current_version(&state.db, review_id).await?;
    let sigs = signatures(&state.db, review_id).await?;
    let (content, reason) =
        if rbac::allows(principal.role, Action::ClinicalContentRead) && has_ciphertext(&version) {
            let crypto = ClinicalCrypto::from_state(state)?;
            decrypt_content(&crypto, review.org_id, &version).await?
        } else {
            (
                ReviewContent {
                    disposition: version.disposition,
                    ..ReviewContent::default()
                },
                None,
            )
        };
    let content_hash =
        version.content_hash.as_deref().map(hex).ok_or_else(|| {
            AppError::Internal("clinical review version has no content hash".into())
        })?;
    Ok(ClinicalReview {
        id: review.id.to_string(),
        org_id: review.org_id.to_string(),
        session_id: review.session_id,
        state: review.state,
        assigned_to: review.assigned_to.map(|u| u.to_string()),
        cosign_required: review.cosign_required,
        row_version: u32::try_from(review.row_version).unwrap_or(0),
        current: ClinicalReviewVersion {
            version_no: u32::try_from(version.version_no).unwrap_or(0),
            status: version.status,
            disposition: content.disposition,
            impression: content.impression,
            recommendations: content.recommendations,
            amend_reason: reason,
            created_by: version.created_by.map(|u| u.to_string()),
            created_at: cursor::ts(version.created_at),
            signed_at: version.signed_at.map(cursor::ts),
        },
        content_hash,
        signatures: sigs,
        created_at: cursor::ts(review.created_at),
        updated_at: cursor::ts(review.updated_at),
    })
}

// ---- claim and draft save -------------------------------------------------------------------

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

/// An active user of `org`, for `assigned_to`.
async fn org_member(conn: &mut PgConnection, raw: &str, org: Uuid) -> AppResult<Uuid> {
    let reason = "must be an active user of this org";
    let uid = Uuid::parse_str(raw).map_err(|_| invalid("assigned_to", reason))?;
    let member = sqlx::query_scalar!(
        r#"SELECT 1 AS "one!: i32" FROM users WHERE id = $1 AND org_id = $2 AND status = 'active'"#,
        uid,
        org,
    )
    .fetch_optional(&mut *conn)
    .await?;
    member
        .map(|_| uid)
        .ok_or_else(|| invalid("assigned_to", reason))
}

/// `POST /v1/sessions/{id}/clinical-review`: creates the review (`open`) with an empty
/// version-1 draft, assigned to the caller unless `assigned_to` names another org user.
/// Returns `(created, review_id)`; an existing review is returned as is (no audit row).
pub async fn claim(
    state: &AppState,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    session_id: &str,
    req: ClaimClinicalReviewRequest,
) -> AppResult<(bool, Uuid)> {
    let mut tx = state.db.begin().await?;
    let facts = session_facts(&mut tx, session_id)
        .await?
        .ok_or(AppError::NotFound)?;
    principal.ensure_org(facts.org_id)?;
    let org = facts.org_id;
    if let Some(existing) = find_review_by_session(&mut *tx, session_id).await? {
        return Ok((false, existing.id));
    }
    let assigned_to = match req.assigned_to.as_deref() {
        Some(raw) => org_member(&mut tx, raw, org).await?,
        None => principal.user_id,
    };
    let settings = ClinicalSettings::load(&mut tx, org).await?;
    let cosign = cosign_required(settings.cosign_policy, &facts.attention_flag);
    let created = sqlx::query_scalar!(
        r#"INSERT INTO clinical_reviews (org_id, session_id, assigned_to, cosign_required, created_by)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (session_id) DO NOTHING
           RETURNING id"#,
        org,
        session_id,
        assigned_to,
        cosign,
        principal.user_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(review_id) = created else {
        // A concurrent claim won; return its review.
        drop(tx);
        let existing = find_review_by_session(&state.db, session_id)
            .await?
            .ok_or(AppError::NotFound)?;
        return Ok((false, existing.id));
    };
    let hash = content_hash(&facts, 1, &ReviewContent::default());
    sqlx::query!(
        r#"INSERT INTO clinical_review_versions
               (review_id, org_id, version_no, content_hash, created_by, updated_by)
           VALUES ($1, $2, 1, $3, $4, $4)"#,
        review_id,
        org,
        hash.as_slice(),
        principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        principal,
        "clinical_review.claim",
        "clinical_review",
        review_id,
    )
    .org(org)
    .after(json!({
        "session_id": session_id,
        "assigned_to": assigned_to,
        "version_no": 1,
        "content_hash": hex(&hash),
    }))
    .meta(meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok((true, review_id))
}

/// A text field of a draft save: `None` = unchanged, `""` = clear, else the new text.
fn text_change(name: &str, value: Option<&String>) -> AppResult<Option<Option<String>>> {
    match value {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(Some(None)),
        Some(s) if s.chars().count() > MAX_REVIEW_TEXT_CHARS => Err(invalid(
            name,
            &format!("must be at most {MAX_REVIEW_TEXT_CHARS} characters"),
        )),
        Some(s) => Ok(Some(Some(s.clone()))),
    }
}

/// `PATCH /v1/clinical-reviews/{id}` (SIGN-2, SIGN-3). `if_match` is the client's
/// `row_version`; a different current version answers `412 stale-version`.
pub async fn save_draft(
    state: &AppState,
    crypto: &ClinicalCrypto,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    review_id: Uuid,
    if_match: i32,
    req: UpdateClinicalReviewRequest,
) -> AppResult<()> {
    let stale = || AppError::PreconditionFailed {
        slug: asd_contract::problem::STALE_VERSION,
        detail: "the review changed since it was loaded; reload it".into(),
    };
    let review = find_review(&state.db, review_id)
        .await?
        .ok_or(AppError::NotFound)?;
    principal.ensure_org(review.org_id)?;
    let org = review.org_id;
    if review.row_version != if_match {
        return Err(stale());
    }
    let next = transition(review.state, ReviewOp::SaveDraft)?;
    let impression = text_change("impression", req.impression.as_ref())?;
    let recommendations = text_change("recommendations", req.recommendations.as_ref())?;
    let assigned_to = match req.assigned_to.as_deref() {
        Some(raw) => {
            let mut conn = state.db.acquire().await?;
            Some(org_member(&mut conn, raw, org).await?)
        }
        None => None,
    };

    // Before the transaction: decrypt, re-encrypt what changed, hash (module docs).
    let version = current_version(&state.db, review_id).await?;
    if version.status != ReviewVersionStatus::Draft {
        return Err(AppError::Internal(
            "editable review has no draft version".into(),
        ));
    }
    let (old, _) = decrypt_content(crypto, org, &version).await?;
    let mut changed = Vec::new();
    let mut new = old.clone();
    if let Some(d) = req.disposition {
        new.disposition = Some(d);
        changed.push("disposition");
    }
    let mut impression_col: Sealed = (version.impression_ct.clone(), version.impression_key_id);
    if let Some(text) = impression {
        impression_col = seal(crypto, org, version.id, "impression_ct", text.as_deref()).await?;
        new.impression = text;
        changed.push("impression");
    }
    let mut recommendations_col: Sealed = (
        version.recommendations_ct.clone(),
        version.recommendations_key_id,
    );
    if let Some(text) = recommendations {
        recommendations_col = seal(
            crypto,
            org,
            version.id,
            "recommendations_ct",
            text.as_deref(),
        )
        .await?;
        new.recommendations = text;
        changed.push("recommendations");
    }
    if assigned_to.is_some() {
        changed.push("assigned_to");
    }
    let facts = {
        let mut conn = state.db.acquire().await?;
        session_facts(&mut conn, &review.session_id)
            .await?
            .ok_or(AppError::NotFound)?
    };
    let hash = content_hash(&facts, version.version_no, &new);

    let mut tx = state.db.begin().await?;
    let locked = lock_review(&mut tx, review_id).await?;
    if locked.row_version != if_match {
        return Err(stale());
    }
    sqlx::query!(
        r#"UPDATE clinical_review_versions SET
               disposition = $2, impression_ct = $3, impression_key_id = $4,
               recommendations_ct = $5, recommendations_key_id = $6, content_hash = $7,
               updated_by = $8, updated_at = now()
           WHERE id = $1 AND status = 'draft'"#,
        version.id,
        new.disposition.map(Disposition::as_str),
        impression_col.0,
        impression_col.1,
        recommendations_col.0,
        recommendations_col.1,
        hash.as_slice(),
        principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"UPDATE clinical_reviews SET state = $2, assigned_to = COALESCE($3, assigned_to),
               row_version = row_version + 1, updated_at = now()
           WHERE id = $1"#,
        review_id,
        next.as_str(),
        assigned_to,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        principal,
        "clinical_review.update",
        "clinical_review",
        review_id,
    )
    .org(org)
    .before(json!({"state": locked.state.as_str()}))
    .after(json!({
        "state": next.as_str(),
        "version_no": version.version_no,
        "content_hash": hex(&hash),
        "changed": changed,
    }))
    .meta(meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_are_409_conflict_with_a_reason() {
        let r = transition(ClinicalReviewState::Final, ReviewOp::SaveDraft).unwrap_err();
        let p = AppError::from(r).to_problem();
        assert_eq!(p.status, 409);
        assert!(p.type_uri.ends_with("/conflict"));
        assert_eq!(
            p.detail.as_deref(),
            Some("cannot edit a review that is final")
        );
    }

    #[test]
    fn content_debug_hides_text() {
        let c = ReviewContent {
            disposition: Some(Disposition::Monitor),
            impression: Some("secret impression".into()),
            recommendations: None,
        };
        assert!(!format!("{c:?}").contains("secret"));
    }

    #[test]
    fn text_changes() {
        assert_eq!(text_change("x", None).unwrap(), None);
        assert_eq!(text_change("x", Some(&String::new())).unwrap(), Some(None));
        assert_eq!(
            text_change("x", Some(&"a".to_owned())).unwrap(),
            Some(Some("a".to_owned()))
        );
        assert!(text_change("x", Some(&"a".repeat(MAX_REVIEW_TEXT_CHARS + 1))).is_err());
    }
}
