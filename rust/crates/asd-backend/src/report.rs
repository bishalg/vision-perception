//! owner: WP-B.8
//!
//! Server-side reports of a stored session: the observation diary PDF and the FHIR R4
//! bundle, built by `asd-export` from `sessions.summary` (the full `GazeSessionRecord` as
//! received, ING-2). This module only loads the inputs and maps refusals to problems; the
//! formatting lives in `asd-export`, shared with every platform.
//!
//! Inputs the server has: the record, the participant token (never a name, CNS-7), the
//! enrolment age bracket when one was recorded, and the recording device's app version.
//! It has no M-CHAT-R/F result and no home-routine data: those sections read "pending" and
//! "Not recorded".
//!
//! Refusal (SIM-5): a synthetic session (only stored with `ASD_ALLOW_SYNTHETIC`) gets 409
//! `capture-source-rejected` for both reports.

use asd_contract::problem::CAPTURE_SOURCE_REJECTED;
use asd_contract::session_v1_6::GazeSessionRecord;
use asd_export::fhir::{self, BundleInput, FhirDevice, FhirSubject, ReportDocument};
use asd_export::pdf::{self, DoctorReportInput};
use asd_export::{export_refusal, ReportError};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Everything a report of one session is built from.
#[derive(Debug, Clone)]
pub struct ReportSource {
    pub org_id: Uuid,
    pub record: GazeSessionRecord,
    pub participant_token: String,
    pub age_months: Option<i64>,
    /// `devices.app_version` of the recording device, when known.
    pub app_version: Option<String>,
}

/// Loads a session's report inputs; `None` when the session does not exist. The caller
/// checks org access on [`ReportSource::org_id`].
pub async fn load(db: &PgPool, id: &str) -> AppResult<Option<ReportSource>> {
    let Some(row) = sqlx::query!(
        r#"SELECT s.org_id, s.participant_token, s.summary,
                  p.age_bracket_at_enrol, d.app_version AS "app_version?"
           FROM sessions s
           JOIN participants p ON p.id = s.participant_id
           LEFT JOIN devices d ON d.id = s.device_id
           WHERE s.id = $1"#,
        id,
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    let record: GazeSessionRecord = serde_json::from_value(row.summary)
        .map_err(|e| AppError::Internal(format!("stored summary does not decode: {e}")))?;
    Ok(Some(ReportSource {
        org_id: row.org_id,
        record,
        participant_token: row.participant_token,
        age_months: row
            .age_bracket_at_enrol
            .as_deref()
            .and_then(age_months_from_bracket),
        app_version: row.app_version,
    }))
}

/// Midpoint months of an iOS `PediatricAgeBracket` raw value (`"24–35 mo"`, also with a
/// hyphen), as the app's report uses; `None` for anything else.
pub fn age_months_from_bracket(bracket: &str) -> Option<i64> {
    let normalised = bracket.replace('\u{2013}', "-");
    match normalised.trim() {
        "12-17 mo" => Some(14),
        "18-23 mo" => Some(20),
        "24-35 mo" => Some(28),
        "36-47 mo" => Some(40),
        "48-60 mo" => Some(54),
        _ => None,
    }
}

fn map_error(e: ReportError) -> AppError {
    match e {
        ReportError::Refused(r) if r.is_capture_source() => AppError::Conflict {
            slug: CAPTURE_SOURCE_REJECTED,
            detail: r.message().to_owned(),
        },
        other => AppError::Internal(format!("report: {other}")),
    }
}

/// 409 for a synthetic or simulated session (checked before building either report).
fn ensure_exportable(src: &ReportSource) -> AppResult<()> {
    match export_refusal(&src.record) {
        Some(r) if r.is_capture_source() => Err(map_error(ReportError::Refused(r))),
        _ => Ok(()),
    }
}

/// The observation diary PDF.
pub fn render_pdf(src: &ReportSource) -> AppResult<Vec<u8>> {
    ensure_exportable(src)?;
    pdf::render(&DoctorReportInput {
        participant_label: src.participant_token.clone(),
        child_age_months: src.age_months,
        guardian_name: pdf::DEFAULT_GUARDIAN.to_owned(),
        assessment_date: src.record.started_at.clone(),
        gaze_record: Some(&src.record),
        mchat: None,
        care: None,
    })
    .map_err(map_error)
}

/// The FHIR R4 collection bundle, with the rendered PDF as a `DocumentReference`. A
/// rejected-quality session keeps Patient/RelatedPerson/Device/DocumentReference and has no
/// gaze Observation (FHIR-2, as the iOS builder).
pub fn fhir_bundle(src: &ReportSource) -> AppResult<Value> {
    let pdf = render_pdf(src)?;
    fhir::bundle(&BundleInput {
        record: &src.record,
        subject: FhirSubject::new(&src.participant_token, src.age_months, None),
        device: FhirDevice::new(
            fhir::DEFAULT_DEVICE_NAME,
            src.app_version.as_deref().unwrap_or("0"),
        ),
        mchat: None,
        document: Some(ReportDocument { pdf: &pdf }),
    })
    .map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_brackets_map_to_the_ios_midpoints() {
        assert_eq!(age_months_from_bracket("24\u{2013}35 mo"), Some(28));
        assert_eq!(age_months_from_bracket("12-17 mo"), Some(14));
        assert_eq!(age_months_from_bracket("48–60 mo"), Some(54));
        assert_eq!(age_months_from_bracket("toddler"), None);
    }

    #[test]
    fn synthetic_refusal_is_a_409_with_the_contract_slug() {
        let e = map_error(ReportError::Refused(asd_export::Refusal::SyntheticCapture));
        let p = e.to_problem();
        assert_eq!(p.status, 409);
        assert!(p.type_uri.ends_with("/capture-source-rejected"));
    }
}
