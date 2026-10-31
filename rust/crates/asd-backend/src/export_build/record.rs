//! owner: WP-I.6 (created here because WP-C.12 had not merged; C.12 rebases onto it)
//!
//! Builds [`asd_export::model::ExportRecord`]s from database rows (XC-3) for one export,
//! in **one consistent snapshot** (`REPEATABLE READ READ ONLY`, EXJ-3, phase 1 of the job).
//! Rows are read in bulk (one query per table, `= ANY($ids)`), never per session.
//!
//! Nothing is decrypted here: encrypted impression, recommendations and MRN columns are
//! carried as [`Sealed`] values and opened after the snapshot transaction ends, and only
//! for the sessions the consent policy keeps (the job's phase 2). Which of them are even
//! read is decided by what the requester could read when they asked
//! (`exports.include_clinical_content`, `include_mrn`, EXJ-2, MRN-6) and the mode.

use std::collections::{BTreeMap, HashMap};

use asd_contract::api_v1::{ClipScore, SessionDetail, SessionSummary};
use asd_contract::clinical::settings as org_settings;
use asd_contract::export::{ExportMode, MAX_EXPORT_ROWS};
use asd_core::types::{ExpectationStatus, PerVideoScoreRow};
use asd_export::model::{
    ClinicalReviewSummary, Disposition, ExportRecord, OrgFacts, Paper, PatientRef, ReviewState,
    Signature, SignatureRole,
};
use asd_export::policy::ConsentFacts;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::filters::ParsedFilters;
use super::ExportRow;
use crate::error::{AppError, AppResult};
use crate::ingest::derive::{metric_row, worst_row};
use crate::read::cursor;

/// An encrypted column value, opened later with `Aad::new(table, column, org, row_id)`.
#[derive(Clone)]
pub struct Sealed {
    pub table: &'static str,
    pub column: &'static str,
    pub row_id: Uuid,
    pub key_id: Uuid,
    pub ciphertext: Vec<u8>,
}

impl std::fmt::Debug for Sealed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sealed({}.{} {})", self.table, self.column, self.row_id)
    }
}

/// One session of the snapshot: its record (no decrypted fields yet) and what the later
/// phases need besides it.
#[derive(Debug, Clone)]
pub struct SnapSession {
    pub record: ExportRecord,
    pub participant_id: Uuid,
    /// `sessions.summary` (FHIR formats only).
    pub gaze: Option<Value>,
    /// Recording app version (FHIR `Device`).
    pub app_version: Option<String>,
    pub impression: Option<Sealed>,
    pub recommendations: Option<Sealed>,
    /// MRN ciphertext and the assigning authority.
    pub mrn: Option<(Sealed, String)>,
}

/// Everything phase 1 read.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub org: OrgFacts,
    /// Requester's display name, else e-mail, else `system` (XC-6).
    pub requested_by_display: String,
    /// `organizations.settings.exports.ttl_hours` when set and valid.
    pub ttl_hours: Option<i32>,
    /// The `hl7` profile and environment of the first named destination that has one.
    pub hl7: Option<(Value, bool)>,
    /// In `started_at, id` order. Longer than [`MAX_EXPORT_ROWS`] means "too many".
    pub sessions: Vec<SnapSession>,
}

fn invalid(what: &str) -> AppError {
    AppError::Internal(format!("stored {what} is not in the contract"))
}

fn org_facts(name: String, settings: &Value) -> OrgFacts {
    let timezone = settings
        .get(org_settings::TIMEZONE)
        .and_then(Value::as_str)
        .unwrap_or(org_settings::DEFAULT_TIMEZONE)
        .to_owned();
    let paper = match settings.get(org_settings::PAPER).and_then(Value::as_str) {
        Some("letter") => Paper::Letter,
        _ => Paper::A4,
    };
    let letterhead = settings
        .get(org_settings::LETTERHEAD)
        .and_then(|l| l.get(org_settings::LETTERHEAD_ADDRESS_LINES))
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    OrgFacts {
        name,
        timezone,
        paper,
        letterhead,
    }
}

/// `details.rows` of a stored clip; empty when absent or not decodable (as `read::clips`).
fn detail_rows(details: Option<&Value>) -> Vec<PerVideoScoreRow> {
    details
        .and_then(|d| d.get("rows"))
        .and_then(|rows| serde_json::from_value(rows.clone()).ok())
        .unwrap_or_default()
}

fn review_state(s: &str) -> AppResult<ReviewState> {
    serde_json::from_value(Value::String(s.to_owned())).map_err(|_| invalid("review state"))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Reads the export's sessions in one snapshot. At most `MAX_EXPORT_ROWS + 1` sessions are
/// read, so the caller can refuse an oversized export without reading it all.
pub async fn load(
    pool: &PgPool,
    export: &ExportRow,
    filters: &ParsedFilters,
    want_gaze: bool,
) -> AppResult<Snapshot> {
    let org = export.org_id;
    let identified = export.mode == ExportMode::Identified;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;

    let org_row = sqlx::query!(
        "SELECT name, settings FROM organizations WHERE id = $1",
        org
    )
    .fetch_one(&mut *tx)
    .await?;
    let ttl_hours = org_row
        .settings
        .get(asd_contract::export::settings::EXPORTS)
        .and_then(|e| e.get(asd_contract::export::settings::TTL_HOURS))
        .and_then(Value::as_i64)
        .and_then(|h| i32::try_from(h).ok());
    let org_facts = org_facts(org_row.name, &org_row.settings);

    let requested_by_display = match export.requested_by {
        Some(user) => sqlx::query_scalar!(
            r#"SELECT coalesce(display_name, email) AS "name!" FROM users WHERE id = $1"#,
            user
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or_else(|| "unknown user".into()),
        None => "system".into(),
    };

    let hl7 = if export.destination_ids.is_empty() {
        None
    } else {
        sqlx::query!(
            r#"SELECT config -> 'hl7' AS "hl7!", environment FROM integration_destinations
               WHERE org_id = $1 AND id = ANY($2) AND config ? 'hl7'
               ORDER BY created_at, id LIMIT 1"#,
            org,
            &export.destination_ids,
        )
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| (r.hl7, r.environment == "test"))
    };

    let limit = i64::try_from(MAX_EXPORT_ROWS + 1).expect("fits");
    let rows = sqlx::query!(
        r#"SELECT s.id, s.participant_id, s.participant_token, s.started_at, s.duration_sec,
                  s.battery, s.capture_source, s.quality_verdict, s.attention_flag,
                  s.composite_score, s.composite_confidence, s.bundle_version, s.device_id,
                  s.age_pack, s.lang, s.schema_version, s.valid_frame_ratio,
                  s.calibrated_distance_cm, s.tracking_verdict, s.tracking_accuracy_deg, s.social_preference, s.eye_dwell, s.mouth_dwell,
                  s.first_eye_latency_ms, s.affect,
                  s.report_pdf_key IS NOT NULL AS "has_report!",
                  EXISTS (SELECT 1 FROM review_items r
                          WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed'))
                      AS "reviewed!",
                  p.withdrawn_at IS NOT NULL AS "withdrawn!",
                  p.age_bracket_at_enrol,
                  s.summary #>> '{normCollection,consented}' AS research_consented,
                  d.app_version AS "app_version?"
           FROM sessions s
           JOIN participants p ON p.id = s.participant_id
           LEFT JOIN clinical_reviews cr ON cr.session_id = s.id
           LEFT JOIN devices d ON d.id = s.device_id
           WHERE s.org_id = $1
             AND ($2::text IS NULL OR s.attention_flag = $2)
             AND ($3::text IS NULL OR s.quality_verdict = $3)
             AND ($4::timestamptz IS NULL OR s.started_at >= $4)
             AND ($5::timestamptz IS NULL OR s.started_at <= $5)
             AND ($6::text IS NULL OR s.bundle_version = $6)
             AND ($7::bool IS NULL OR EXISTS (
                     SELECT 1 FROM review_items r
                     WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed')) = $7)
             AND (cardinality($8::uuid[]) = 0 OR s.participant_id = ANY($8))
             AND (cardinality($9::text[]) = 0 OR s.id = ANY($9))
             AND (cardinality($10::text[]) = 0 OR coalesce(cr.state, 'none') = ANY($10))
           ORDER BY s.started_at, s.id
           LIMIT $11"#,
        org,
        filters.flag,
        filters.quality,
        filters.from,
        filters.to,
        filters.bundle_version,
        filters.reviewed,
        &filters.participant_ids,
        &filters.session_ids,
        &filters.clinical_state,
        limit,
    )
    .fetch_all(&mut *tx)
    .await?;
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let participant_ids: Vec<Uuid> = rows.iter().map(|r| r.participant_id).collect();

    // ---- clips (as GET /v1/sessions/{id}/clips)
    let mut clips: HashMap<String, Vec<ClipScore>> = HashMap::new();
    for c in sqlx::query!(
        r#"SELECT session_id, clip_id, clip_order, status, score, weight, dwell_social,
                  dwell_geometric, dwell_eye, dwell_mouth, orienting_latency_ms,
                  gaze_cue_accuracy, fixation_count, mean_fixation_ms, baseline_ref, details
           FROM session_clip_scores WHERE session_id = ANY($1)
           ORDER BY session_id, clip_order, clip_id"#,
        &ids,
    )
    .fetch_all(&mut *tx)
    .await?
    {
        let rows = detail_rows(c.details.as_ref());
        let worst = worst_row(&rows).map(|i| metric_row(&rows[i]));
        let status = c
            .status
            .and_then(|s| serde_json::from_value::<ExpectationStatus>(Value::String(s)).ok());
        clips.entry(c.session_id).or_default().push(ClipScore {
            clip_id: c.clip_id,
            order: i64::from(c.clip_order),
            status,
            score: c.score,
            weight: c.weight,
            dwell_social: c.dwell_social,
            dwell_geometric: c.dwell_geometric,
            dwell_eye: c.dwell_eye,
            dwell_mouth: c.dwell_mouth,
            orienting_latency_ms: c.orienting_latency_ms,
            gaze_cue_accuracy: c.gaze_cue_accuracy,
            fixation_count: c.fixation_count.map(i64::from),
            mean_fixation_ms: c.mean_fixation_ms,
            metric: worst.as_ref().map(|w| w.metric.clone()),
            expected: worst.as_ref().and_then(|w| w.expected),
            recorded: worst.as_ref().map(|w| w.recorded),
            deviation: worst.as_ref().and_then(|w| w.deviation),
            baseline_ref: c.baseline_ref,
            rows: rows.iter().map(metric_row).collect(),
        });
    }

    // ---- clinical reviews: the current signed version, else the latest draft (SIGN-7/8)
    let want_text = identified && export.include_clinical_content;
    let mut reviews: HashMap<String, (ClinicalReviewSummary, Option<Sealed>, Option<Sealed>)> =
        HashMap::new();
    let mut version_session: HashMap<Uuid, String> = HashMap::new();
    for r in sqlx::query!(
        r#"SELECT cr.session_id, cr.state, v.id AS "version_id!", v.version_no AS "version_no!",
                  v.disposition AS "disposition?", v.content_hash AS "content_hash?",
                  v.signed_at AS "signed_at?",
                  v.impression_ct AS "impression_ct?", v.impression_key_id AS "impression_key_id?",
                  v.recommendations_ct AS "recommendations_ct?",
                  v.recommendations_key_id AS "recommendations_key_id?",
                  EXISTS (SELECT 1 FROM clinical_review_versions e
                          WHERE e.review_id = cr.id AND e.version_no < v.version_no
                            AND e.signed_at IS NOT NULL) AS "amended!"
           FROM clinical_reviews cr
           JOIN LATERAL (
               SELECT x.id, x.version_no, x.disposition, x.content_hash, x.signed_at,
                      x.impression_ct, x.impression_key_id, x.recommendations_ct,
                      x.recommendations_key_id
               FROM clinical_review_versions x WHERE x.review_id = cr.id
               ORDER BY (x.status = 'signed') DESC, x.version_no DESC LIMIT 1) v ON true
           WHERE cr.org_id = $1 AND cr.session_id = ANY($2)"#,
        org,
        &ids,
    )
    .fetch_all(&mut *tx)
    .await?
    {
        let disposition = r
            .disposition
            .map(|d| {
                serde_json::from_value::<Disposition>(Value::String(d))
                    .map_err(|_| invalid("disposition"))
            })
            .transpose()?;
        let signed = r.signed_at.is_some();
        let summary = ClinicalReviewSummary {
            state: review_state(&r.state)?,
            version_no: u32::try_from(r.version_no).map_err(|_| invalid("version_no"))?,
            amended: signed && r.amended,
            disposition,
            signed_at: r.signed_at.map(cursor::ts),
            content_hash: r.content_hash.as_deref().map(hex),
            signatures: Vec::new(),
            impression: None,
            recommendations: None,
        };
        let seal = |column: &'static str, ct: Option<Vec<u8>>, key: Option<Uuid>| match (ct, key) {
            (Some(ciphertext), Some(key_id)) if want_text => Some(Sealed {
                table: "clinical_review_versions",
                column,
                row_id: r.version_id,
                key_id,
                ciphertext,
            }),
            _ => None,
        };
        let impression = seal("impression_ct", r.impression_ct, r.impression_key_id);
        let recommendations = seal(
            "recommendations_ct",
            r.recommendations_ct,
            r.recommendations_key_id,
        );
        version_session.insert(r.version_id, r.session_id.clone());
        reviews.insert(r.session_id, (summary, impression, recommendations));
    }
    let version_ids: Vec<Uuid> = version_session.keys().copied().collect();
    for s in sqlx::query!(
        r#"SELECT version_id, role, signer_id, signer_display_name, signer_registration,
                  signed_at
           FROM clinical_signatures WHERE version_id = ANY($1)
           ORDER BY signed_at, id"#,
        &version_ids,
    )
    .fetch_all(&mut *tx)
    .await?
    {
        let Some(review) = version_session
            .get(&s.version_id)
            .and_then(|sid| reviews.get_mut(sid))
        else {
            continue;
        };
        review.0.signatures.push(Signature {
            role: if s.role == "cosign" {
                SignatureRole::Cosign
            } else {
                SignatureRole::Sign
            },
            user_id: Some(s.signer_id.to_string()),
            display_name: Some(s.signer_display_name),
            professional_registration: s.signer_registration,
            signed_at: cursor::ts(s.signed_at),
        });
    }

    // ---- MRNs (identified exports by a requester with PatientRefRead only, MRN-6)
    let mut mrns: HashMap<Uuid, (Sealed, String)> = HashMap::new();
    if identified && export.include_mrn {
        for m in sqlx::query!(
            r#"SELECT id, participant_id, assigning_authority, mrn_ct, mrn_key_id
               FROM patient_refs WHERE org_id = $1 AND participant_id = ANY($2)"#,
            org,
            &participant_ids,
        )
        .fetch_all(&mut *tx)
        .await?
        {
            mrns.insert(
                m.participant_id,
                (
                    Sealed {
                        table: "patient_refs",
                        column: "mrn_ct",
                        row_id: m.id,
                        key_id: m.mrn_key_id,
                        ciphertext: m.mrn_ct,
                    },
                    m.assigning_authority,
                ),
            );
        }
    }

    // ---- stored session records (FHIR formats only)
    let mut gazes: HashMap<String, Value> = HashMap::new();
    if want_gaze {
        for g in sqlx::query!("SELECT id, summary FROM sessions WHERE id = ANY($1)", &ids)
            .fetch_all(&mut *tx)
            .await?
        {
            gazes.insert(g.id, g.summary);
        }
    }
    tx.commit().await?;

    let mut sessions = Vec::with_capacity(rows.len());
    for r in rows {
        let summary = SessionSummary {
            id: r.id.clone(),
            participant_id: r.participant_id.to_string(),
            participant_token: r.participant_token.clone(),
            started_at: cursor::ts(r.started_at),
            duration_sec: r.duration_sec,
            battery: r.battery,
            capture_source: serde_json::from_value(Value::String(r.capture_source)).ok(),
            quality_verdict: r.quality_verdict,
            attention_flag: serde_json::from_value(Value::String(r.attention_flag))
                .map_err(|_| invalid("attention_flag"))?,
            composite_score: r.composite_score,
            composite_confidence: r.composite_confidence,
            bundle_version: r.bundle_version,
            device_id: r.device_id.map(|d| d.to_string()),
            reviewed: Some(r.reviewed),
            tracking_verdict: crate::read::sessions::parse_verdict(r.tracking_verdict)?,
            tracking_accuracy_deg: r.tracking_accuracy_deg,
        };
        let affect: Option<BTreeMap<String, Value>> =
            r.affect.and_then(|a| serde_json::from_value(a).ok());
        let detail = SessionDetail {
            summary,
            org_id: org.to_string(),
            age_pack: r.age_pack,
            lang: r.lang,
            schema_version: Some(r.schema_version),
            valid_frame_ratio: Some(r.valid_frame_ratio),
            calibrated_distance_cm: r.calibrated_distance_cm,
            social_preference: Some(r.social_preference),
            eye_dwell: r.eye_dwell,
            mouth_dwell: r.mouth_dwell,
            first_eye_latency_ms: r.first_eye_latency_ms,
            affect,
            report_pdf_available: Some(r.has_report),
            // The full tracking record is not exported yet (WP-G.11 exposes it on the detail
            // API only); exports carry the verdict and accuracy on the summary.
            tracking_quality: None,
        };
        let consent = ConsentFacts {
            withdrawn: r.withdrawn,
            research_consented: match r.research_consented.as_deref() {
                Some("true") => Some(true),
                Some("false") => Some(false),
                _ => None,
            },
        };
        let patient = PatientRef {
            participant_id: r.participant_id.to_string(),
            participant_token: r.participant_token,
            mrn: None,
            age_bracket_at_enrol: r.age_bracket_at_enrol,
        };
        let mut record = ExportRecord::new(
            org_facts.clone(),
            patient,
            consent,
            detail,
            clips.remove(&r.id).unwrap_or_default(),
        );
        let (impression, recommendations) = match reviews.remove(&r.id) {
            Some((summary, i, rec)) => {
                record.review = Some(summary);
                (i, rec)
            }
            None => (None, None),
        };
        sessions.push(SnapSession {
            gaze: gazes.remove(&r.id),
            app_version: r.app_version,
            impression,
            recommendations,
            mrn: mrns.get(&r.participant_id).cloned(),
            participant_id: r.participant_id,
            record,
        });
    }
    Ok(Snapshot {
        org: org_facts,
        requested_by_display,
        ttl_hours,
        hl7,
        sessions,
    })
}
