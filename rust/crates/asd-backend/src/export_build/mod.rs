//! owner: WP-I.6
//!
//! The backend side of exports (EXJ-1..9): the stored `exports` row ([`ExportRow`]),
//! request-time rules that are not consent policy (which formats exist, which need one
//! session or an MRN), the format dispatch onto the `asd-export` encoders ([`render`]),
//! artifact naming, the HL7 profile mapping and proxy-mode download tokens.
//!
//! Every format, de-identification and consent rule lives in `asd-export` (ADR-0008 D6/D7,
//! CLAUDE.md): this module only loads inputs ([`record`]), maps enums and calls encoders.
//! Formats plug in as one match arm of [`render`]; `pdf_summary` (WP-I.5) is refused up
//! front with a clear reason until its encoder exists.

pub mod filters;
pub mod record;

use std::collections::BTreeMap;

use asd_contract::export::{
    Export, ExportFilters, ExportFormat, ExportKind, ExportMode, ExportPurpose, ExportStatus,
    DEFAULT_TTL_HOURS, MAX_TTL_HOURS, MIN_TTL_HOURS,
};
use asd_contract::session_v1_6::GazeSessionRecord;
use asd_export::fhir::clinical::{
    self as fhir_clinical, BundleKind, ClinicalBundleInput, FhirProfile,
};
use asd_export::fhir::{FhirDevice, DEFAULT_DEVICE_NAME};
use asd_export::hl7v2::{self, Hl7Options, Hl7Profile, MappedCode, Pid5Mode};
use asd_export::model::ExportRecord;
use asd_export::policy::{Mode, Purpose};
use asd_export::tabular::{self, Dialect, TabularOptions};
use asd_export::watermark::Watermark;
use asd_export::{deid, ExportError};
use aws_lc_rs::hmac;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::read::cursor;

// ---- the stored row ----------------------------------------------------------------------

/// An `exports` row.
#[derive(Clone)]
pub struct ExportRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub kind: ExportKind,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub purpose: ExportPurpose,
    pub status: ExportStatus,
    pub filters: Value,
    pub reason: String,
    pub destination_ids: Vec<Uuid>,
    pub include_clinical_content: bool,
    pub include_mrn: bool,
    pub requested_by: Option<Uuid>,
    pub job_id: Option<Uuid>,
    /// The job gave up (`jobs.status = failed`) while the row still says queued/running.
    pub job_failed: bool,
    pub ttl_hours: i32,
    pub download_secret: Vec<u8>,
    pub blob_key: Option<String>,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub sha256: Option<Vec<u8>>,
    pub byte_size: Option<i64>,
    pub row_count: Option<i32>,
    pub excluded: Value,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for ExportRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportRow")
            .field("id", &self.id)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

fn stored<T>(what: &str, v: Option<T>) -> AppResult<T> {
    v.ok_or_else(|| AppError::Internal(format!("stored export {what} is not in the contract")))
}

/// One export by id (any org; callers check the org).
pub async fn load_export(db: &PgPool, id: Uuid) -> AppResult<Option<ExportRow>> {
    let Some(r) = sqlx::query!(
        r#"SELECT e.id, e.org_id, e.kind, e.format, e.mode, e.purpose, e.status, e.filters,
                  e.reason, e.destination_ids, e.include_clinical_content, e.include_mrn,
                  e.requested_by, e.job_id, e.ttl_hours, e.download_secret, e.blob_key,
                  e.file_name, e.content_type, e.sha256, e.byte_size, e.row_count, e.excluded,
                  e.error, e.created_at, e.completed_at, e.expires_at,
                  coalesce(j.status = 'failed', false) AS "job_failed!"
           FROM exports e LEFT JOIN jobs j ON j.id = e.job_id
           WHERE e.id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(ExportRow {
        id: r.id,
        org_id: r.org_id,
        kind: stored("kind", ExportKind::parse(&r.kind))?,
        format: stored("format", ExportFormat::parse(&r.format))?,
        mode: stored("mode", ExportMode::parse(&r.mode))?,
        purpose: stored("purpose", ExportPurpose::parse(&r.purpose))?,
        status: stored("status", ExportStatus::parse(&r.status))?,
        filters: r.filters,
        reason: r.reason,
        destination_ids: r.destination_ids,
        include_clinical_content: r.include_clinical_content,
        include_mrn: r.include_mrn,
        requested_by: r.requested_by,
        job_id: r.job_id,
        job_failed: r.job_failed,
        ttl_hours: r.ttl_hours,
        download_secret: r.download_secret,
        blob_key: r.blob_key,
        file_name: r.file_name,
        content_type: r.content_type,
        sha256: r.sha256,
        byte_size: r.byte_size,
        row_count: r.row_count,
        excluded: r.excluded,
        error: r.error,
        created_at: r.created_at,
        completed_at: r.completed_at,
        expires_at: r.expires_at,
    }))
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

impl ExportRow {
    /// The status clients see: a job that gave up makes a queued/running export `failed`.
    pub fn effective_status(&self) -> ExportStatus {
        match self.status {
            ExportStatus::Queued | ExportStatus::Running if self.job_failed => ExportStatus::Failed,
            s => s,
        }
    }

    /// The `Export` resource (no secret, no blob key).
    pub fn to_dto(&self) -> Export {
        let excluded = self
            .excluded
            .as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                    .collect()
            })
            .unwrap_or_default();
        Export {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            kind: self.kind,
            format: self.format,
            mode: self.mode,
            purpose: self.purpose,
            status: self.effective_status(),
            filters: serde_json::from_value::<ExportFilters>(self.filters.clone())
                .unwrap_or_default(),
            reason: self.reason.clone(),
            destination_ids: self.destination_ids.iter().map(Uuid::to_string).collect(),
            requested_by: self.requested_by.map(|u| u.to_string()),
            created_at: cursor::ts(self.created_at),
            completed_at: self.completed_at.map(cursor::ts),
            expires_at: self.expires_at.map(cursor::ts),
            row_count: self.row_count.and_then(|n| u64::try_from(n).ok()),
            byte_size: self.byte_size.and_then(|n| u64::try_from(n).ok()),
            sha256: self.sha256.as_deref().map(hex),
            file_name: self.file_name.clone(),
            content_type: self.content_type.clone(),
            excluded,
            error: if self.effective_status() == ExportStatus::Failed {
                Some(
                    self.error
                        .clone()
                        .unwrap_or_else(|| "job-failed".to_owned()),
                )
            } else {
                None
            },
        }
    }
}

// ---- request rules -----------------------------------------------------------------------

pub fn policy_mode(m: ExportMode) -> Mode {
    match m {
        ExportMode::Identified => Mode::Identified,
        ExportMode::Deidentified => Mode::Deidentified,
    }
}

pub fn policy_purpose(p: ExportPurpose) -> Purpose {
    match p {
        ExportPurpose::ClinicalCare => Purpose::ClinicalCare,
        ExportPurpose::Research => Purpose::Research,
        ExportPurpose::QualityImprovement => Purpose::QualityImprovement,
        ExportPurpose::PatientRequest => Purpose::PatientRequest,
    }
}

/// `Err(reason)` for a format whose encoder does not exist yet.
pub fn format_available(format: ExportFormat) -> Result<(), &'static str> {
    match format {
        ExportFormat::PdfSummary => Err(
            "pdf_summary is not available yet: the PDF/A clinical summary encoder ships with WP-I.5",
        ),
        _ => Ok(()),
    }
}

/// Formats that hold exactly one session (`filters.session_ids` must name one).
pub fn single_session(format: ExportFormat) -> bool {
    matches!(format, ExportFormat::FhirBundle | ExportFormat::Hl7v2)
}

/// Formats that cannot exist without the hospital MRN (HL7-3, EXJ-2).
pub fn needs_mrn(format: ExportFormat) -> bool {
    format == ExportFormat::Hl7v2
}

/// Formats built from the stored session record (`sessions.summary`).
pub fn needs_gaze(format: ExportFormat) -> bool {
    matches!(format, ExportFormat::FhirBundle | ExportFormat::FhirNdjson)
}

/// The org's artifact lifetime (`settings.exports.ttl_hours`, EXJ-4), clamped to 1..168.
pub fn ttl_hours(setting: Option<i64>) -> i32 {
    let h = setting.map_or(i64::from(DEFAULT_TTL_HOURS), |h| {
        h.clamp(i64::from(MIN_TTL_HOURS), i64::from(MAX_TTL_HOURS))
    });
    i32::try_from(h).expect("clamped")
}

/// Blob key of an artifact (EXJ-3): deterministic per export, so a re-run overwrites.
pub fn blob_key(org: Uuid, export: Uuid, file_name: &str) -> String {
    format!("exports/{org}/{export}/{file_name}")
}

// ---- HL7 profile (INTG-3 `hl7 {...}`) -----------------------------------------------------

fn text(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Default `MSH-3` when no destination names one.
pub const DEFAULT_SENDING_APP: &str = "CURV-ASD";

/// Maps a destination's `config.hl7` (INTG-3) onto the encoder profile; absent keys take
/// defaults (`MSH-3 = CURV-ASD`, `MSH-4 = org name`). `embed_pdf` stays off until the
/// PDF/A summary exists (WP-I.5). Unverified codes are never opted in from here.
pub fn hl7_profile(config: Option<&Value>, test_destination: bool, org_name: &str) -> Hl7Profile {
    let empty = Value::Null;
    let c = config.unwrap_or(&empty);
    let code_map = c
        .get("code_map")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(local, v)| {
                    let code = match v {
                        Value::String(s) => s.clone(),
                        other => other.get("code")?.as_str()?.to_owned(),
                    };
                    let text = v
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or(&code)
                        .to_owned();
                    let system = v
                        .get("system")
                        .and_then(Value::as_str)
                        .unwrap_or("L")
                        .to_owned();
                    Some((local.clone(), MappedCode { code, text, system }))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    Hl7Profile {
        sending_app: text(c, "sending_app").unwrap_or_else(|| DEFAULT_SENDING_APP.to_owned()),
        sending_facility: text(c, "sending_facility").unwrap_or_else(|| org_name.to_owned()),
        receiving_app: text(c, "receiving_app").unwrap_or_default(),
        receiving_facility: text(c, "receiving_facility").unwrap_or_default(),
        send_pv1: c.get("send_pv1").and_then(Value::as_bool).unwrap_or(false),
        test_destination,
        pid5_mode: match c.get("pid5_mode").and_then(Value::as_str) {
            Some("token") => Pid5Mode::Token,
            _ => Pid5Mode::Empty,
        },
        code_map,
        embed_pdf: false,
        code_policy: asd_export::codes::CodePolicy::default(),
        file_prefix: text(c, "file_prefix").unwrap_or_default(),
    }
}

// ---- format dispatch ---------------------------------------------------------------------

/// A rendered artifact.
pub struct Artifact {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub content_type: &'static str,
}

/// Why rendering failed. Messages carry no patient data.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("unsupported-format: {0}")]
    Unsupported(&'static str),
    #[error("no-eligible-sessions: the format needs at least one exportable session")]
    NoRecords,
    #[error("one-session-format: the format holds exactly one session, {0} matched")]
    NeedsOneSession(usize),
    #[error("stored-record-invalid: a stored session record does not decode")]
    StoredRecord,
    #[error("export-too-large: {rows} rows exceed the limit of {limit}")]
    TooLarge { rows: usize, limit: usize },
    #[error("missing-mrn: {0}")]
    MissingMrn(String),
    #[error("render-failed: {0}")]
    Export(ExportError),
}

impl From<ExportError> for RenderError {
    fn from(e: ExportError) -> Self {
        match e {
            ExportError::ExportTooLarge { rows, limit } => Self::TooLarge { rows, limit },
            ExportError::Hl7(hl7v2::Hl7Error::MissingMrn) => Self::MissingMrn(e.to_string()),
            other => Self::Export(other),
        }
    }
}

/// Inputs of [`render`]: the eligible records in their identified form (the encoders or
/// this dispatch de-identify, never the caller), aligned per index with `gazes` and
/// `app_versions` (FHIR formats only).
pub struct RenderInput<'a> {
    pub format: ExportFormat,
    pub records: &'a [ExportRecord],
    pub gazes: &'a [Option<Value>],
    pub app_versions: &'a [Option<String>],
    pub watermark: &'a Watermark,
    pub deid_salt: [u8; deid::SALT_LEN],
    /// RFC 3339; the export's `created_at`, so a re-run renders the same bytes (XC-8).
    pub generated_at: &'a str,
    pub hl7: &'a Hl7Profile,
}

const CT_ZIP: &str = "application/zip";
const CT_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const CT_FHIR_JSON: &str = "application/fhir+json";
const CT_FHIR_NDJSON: &str = "application/fhir+ndjson";
/// HL7 v2 ER7 (pipe-delimited) message.
const CT_HL7: &str = "application/hl7-v2";

/// Renders the artifact with the `asd-export` encoder of `input.format`.
pub fn render(input: &RenderInput<'_>) -> Result<Artifact, RenderError> {
    let id = &input.watermark.export_id;
    let base = format!("export-{id}-{}", input.format.as_str().replace('_', "-"));
    let tabular_opts = || TabularOptions::new(input.watermark.clone(), input.deid_salt);
    match input.format {
        ExportFormat::Csv | ExportFormat::CsvExcel | ExportFormat::CsvSheets => {
            let dialect = match input.format {
                ExportFormat::CsvExcel => Dialect::Excel,
                ExportFormat::CsvSheets => Dialect::Sheets,
                _ => Dialect::Canonical,
            };
            Ok(Artifact {
                bytes: tabular::csv::encode_zip(input.records, &tabular_opts(), dialect)?,
                file_name: format!("{base}.zip"),
                content_type: CT_ZIP,
            })
        }
        ExportFormat::Xlsx => Ok(Artifact {
            bytes: tabular::xlsx::encode(input.records, &tabular_opts())?,
            file_name: format!("{base}.xlsx"),
            content_type: CT_XLSX,
        }),
        ExportFormat::FhirBundle | ExportFormat::FhirNdjson => {
            if input.records.is_empty() {
                return Err(RenderError::NoRecords);
            }
            if input.format == ExportFormat::FhirBundle && input.records.len() != 1 {
                return Err(RenderError::NeedsOneSession(input.records.len()));
            }
            // The clinical bundle encoder expects de-identified records (XC-4) and refuses
            // ones that skipped `deid::apply`.
            let prepared = match input.watermark.mode {
                Mode::Identified => input.records.to_vec(),
                Mode::Deidentified => deid::apply(input.records, &input.deid_salt)?,
            };
            let profile = FhirProfile::default();
            let mut lines = Vec::with_capacity(prepared.len());
            for (i, record) in prepared.iter().enumerate() {
                let gaze: GazeSessionRecord = input
                    .gazes
                    .get(i)
                    .and_then(Option::as_ref)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .ok_or(RenderError::StoredRecord)?;
                let version = input
                    .app_versions
                    .get(i)
                    .and_then(Option::as_deref)
                    .unwrap_or("0");
                lines.push(fhir_clinical::encode(&ClinicalBundleInput {
                    record,
                    gaze: &gaze,
                    device: FhirDevice::new(DEFAULT_DEVICE_NAME, version),
                    watermark: input.watermark,
                    profile: &profile,
                    document: None,
                    kind: BundleKind::Collection,
                })?);
            }
            if input.format == ExportFormat::FhirBundle {
                Ok(Artifact {
                    bytes: lines.pop().expect("exactly one"),
                    file_name: format!("{base}.json"),
                    content_type: CT_FHIR_JSON,
                })
            } else {
                let mut bytes = Vec::new();
                for line in lines {
                    bytes.extend_from_slice(&line);
                    bytes.push(b'\n');
                }
                Ok(Artifact {
                    bytes,
                    file_name: format!("{base}.ndjson"),
                    content_type: CT_FHIR_NDJSON,
                })
            }
        }
        ExportFormat::Hl7v2 => {
            let [record] = input.records else {
                return Err(if input.records.is_empty() {
                    RenderError::NoRecords
                } else {
                    RenderError::NeedsOneSession(input.records.len())
                });
            };
            let bytes = hl7v2::encode_oru_r01(
                record,
                &Hl7Options {
                    profile: input.hl7,
                    delivery_id: id,
                    generated_at: input.generated_at,
                    watermark: input.watermark,
                    pdf: None,
                },
            )?;
            Ok(Artifact {
                bytes,
                file_name: hl7v2::file_name(&input.hl7.file_prefix, id, &record.session.summary.id),
                content_type: CT_HL7,
            })
        }
        ExportFormat::PdfSummary => Err(RenderError::Unsupported(
            format_available(ExportFormat::PdfSummary).expect_err("unavailable"),
        )),
    }
}

// ---- proxy-mode download tokens (EXJ-5) --------------------------------------------------

/// `t` of `/v1/exports/{id}/file?t=...`: `{exp}.{user}.{base64url(HMAC-SHA256)}` over
/// `"{export_id}|{user_id}|{exp}"` keyed with the export's own `download_secret`, so a
/// token works for one export, one user and five minutes, on any API replica.
pub fn download_token(secret: &[u8], export: Uuid, user: Uuid, exp: i64) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    let tag = hmac::sign(&key, token_message(export, user, exp).as_bytes());
    format!(
        "{exp}.{}.{}",
        user.simple(),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(tag.as_ref())
    )
}

fn token_message(export: Uuid, user: Uuid, exp: i64) -> String {
    format!("{export}|{user}|{exp}")
}

/// The token's user when it is genuine for `export` and not expired at `now`.
pub fn verify_download_token(secret: &[u8], export: Uuid, token: &str, now: i64) -> Option<Uuid> {
    let mut parts = token.splitn(3, '.');
    let exp: i64 = parts.next()?.parse().ok()?;
    let user = Uuid::parse_str(parts.next()?).ok()?;
    let tag = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts.next()?)
        .ok()?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    hmac::verify(&key, token_message(export, user, exp).as_bytes(), &tag).ok()?;
    (exp > now).then_some(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_bind_export_user_and_expiry() {
        let secret = [7u8; 32];
        let (e, u) = (Uuid::new_v4(), Uuid::new_v4());
        let t = download_token(&secret, e, u, 1_000);
        assert_eq!(verify_download_token(&secret, e, &t, 999), Some(u));
        assert_eq!(
            verify_download_token(&secret, e, &t, 1_000),
            None,
            "expired"
        );
        assert_eq!(verify_download_token(&secret, Uuid::new_v4(), &t, 0), None);
        assert_eq!(verify_download_token(&[8u8; 32], e, &t, 0), None);
        // Tampering with any part fails.
        let other = Uuid::new_v4();
        let swapped = t.replacen(&u.simple().to_string(), &other.simple().to_string(), 1);
        assert_eq!(verify_download_token(&secret, e, &swapped, 0), None);
        let later = t.replacen("1000", "9999", 1);
        assert_eq!(verify_download_token(&secret, e, &later, 0), None);
        assert_eq!(verify_download_token(&secret, e, "garbage", 0), None);
    }

    #[test]
    fn ttl_is_clamped() {
        assert_eq!(ttl_hours(None), 72);
        assert_eq!(ttl_hours(Some(0)), 1);
        assert_eq!(ttl_hours(Some(500)), 168);
        assert_eq!(ttl_hours(Some(24)), 24);
    }

    #[test]
    fn hl7_profile_defaults_and_mapping() {
        let p = hl7_profile(None, false, "Clinic A");
        assert_eq!(p.sending_app, DEFAULT_SENDING_APP);
        assert_eq!(p.sending_facility, "Clinic A");
        assert_eq!(p.pid5_mode, Pid5Mode::Empty);
        let cfg = serde_json::json!({
            "sending_app": "CURV", "receiving_facility": "TUTH", "pid5_mode": "token",
            "send_pv1": true, "embed_pdf": true, "file_prefix": "curv_",
            "code_map": {"valid-frame-ratio": {"code": "VFR", "system": "99TUTH"}, "disposition": "DISP"}
        });
        let p = hl7_profile(Some(&cfg), true, "Clinic A");
        assert_eq!(p.sending_app, "CURV");
        assert_eq!(p.receiving_facility, "TUTH");
        assert!(p.send_pv1 && p.test_destination && !p.embed_pdf);
        assert_eq!(p.pid5_mode, Pid5Mode::Token);
        assert_eq!(p.code_map["valid-frame-ratio"].system, "99TUTH");
        assert_eq!(p.code_map["disposition"].code, "DISP");
        assert_eq!(p.file_prefix, "curv_");
    }

    #[test]
    fn pdf_summary_is_refused_with_a_reason() {
        assert!(format_available(ExportFormat::PdfSummary)
            .unwrap_err()
            .contains("WP-I.5"));
        for f in ExportFormat::ALL
            .iter()
            .filter(|f| **f != ExportFormat::PdfSummary)
        {
            assert!(format_available(*f).is_ok());
        }
    }

    #[test]
    fn enums_map_onto_the_policy_names() {
        for m in ExportMode::ALL {
            assert_eq!(
                serde_json::to_value(policy_mode(*m)).unwrap(),
                serde_json::to_value(m).unwrap()
            );
        }
        for p in ExportPurpose::ALL {
            assert_eq!(
                serde_json::to_value(policy_purpose(*p)).unwrap(),
                serde_json::to_value(p).unwrap()
            );
        }
    }
}
