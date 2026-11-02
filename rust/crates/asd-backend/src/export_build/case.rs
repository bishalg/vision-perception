//! owner: WP-I.13 (encounters and attachments: WP-I.13b)
//!
//! The backend side of "share a case" (SHR-1..7): maps the rows of one participant's case
//! onto [`asd_export::case_package::CasePackageInput`] and renders it. Every format,
//! de-identification and consent rule lives in `asd-export` (CLAUDE.md): this module loads
//! rows, maps types and calls `case_package::build`.
//!
//! The sessions come from the same snapshot as every other export
//! ([`super::record::load`], with the stored session records); the job decides which are
//! eligible (consent policy, capture source) and decrypts what the requester could read.
//! Session notes are read here, sealed, for identified packages whose requester held
//! `ClinicalContentRead` when asking (`exports.include_clinical_content`).
//!
//! Wave 2a scope: encounters and attachments (WP-B.19's tables) are not loaded yet, so the
//! input carries empty lists and `CaseShareRequest.include_attachments` is stored but has
//! no effect until WP-I.13b.

use std::collections::HashMap;

use asd_contract::export::ExportMode;
use asd_export::case_package::{self, CaseConsent, CasePackageInput, CaseSession};
use asd_export::deid;
use asd_export::fhir::clinical::FhirProfile;
use asd_export::fhir::{FhirDevice, DEFAULT_DEVICE_NAME};
use asd_export::model::{ExportRecord, NoteText};
use asd_export::watermark::Watermark;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::record::Sealed;
use super::{Artifact, ExportRow, RenderError};
use crate::error::AppResult;
use crate::read::cursor;

/// One clinical note of an included session, not yet decrypted.
#[derive(Debug, Clone)]
pub struct SealedNote {
    pub session_id: String,
    pub body: Sealed,
    /// RFC 3339.
    pub created_at: String,
    pub author_display_name: Option<String>,
}

/// The clinical notes (NOTE-1) of `participant` that belong to one of `session_ids`, in
/// `created_at, id` order. Participant-level notes (no session) have no place in the
/// package's per-session `notes/notes.json` and are not read.
pub async fn load_notes(
    pool: &PgPool,
    org: Uuid,
    participant: Uuid,
    session_ids: &[String],
) -> AppResult<Vec<SealedNote>> {
    if session_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query!(
        r#"SELECT n.id, n.session_id AS "session_id!", n.body_ct, n.body_key_id, n.created_at,
                  u.display_name AS "author_name?"
           FROM clinical_notes n JOIN users u ON u.id = n.author_id
           WHERE n.org_id = $1 AND n.participant_id = $2 AND n.session_id = ANY($3)
           ORDER BY n.created_at, n.id"#,
        org,
        participant,
        session_ids,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| SealedNote {
            session_id: r.session_id,
            body: Sealed {
                table: "clinical_notes",
                column: "body_ct",
                row_id: r.id,
                key_id: r.body_key_id,
                ciphertext: r.body_ct,
            },
            created_at: cursor::ts(r.created_at),
            author_display_name: r.author_name,
        })
        .collect())
}

/// Attaches decrypted notes (`(session id, note)`, in order) to their records.
pub fn attach_notes(records: &mut [ExportRecord], notes: Vec<(String, NoteText)>) {
    let mut by_session: HashMap<String, Vec<NoteText>> = HashMap::new();
    for (session, note) in notes {
        by_session.entry(session).or_default().push(note);
    }
    for record in records {
        if let Some(notes) = by_session.remove(&record.session.summary.id) {
            record.notes = notes;
        }
    }
}

/// The consent an identified package relies on (SHR-7), from the export row;
/// `recorded_by_display` is the requester's display name. `None` when de-identified.
pub fn consent(row: &ExportRow, requested_by_display: &str) -> Option<CaseConsent> {
    if row.mode != ExportMode::Identified {
        return None;
    }
    Some(CaseConsent {
        reference: row.case_consent_reference.clone()?,
        recorded_at: cursor::ts(row.case_consent_recorded_at?),
        recorded_by_display: requested_by_display.to_owned(),
    })
}

/// One included session: its (decrypted) record, `sessions.summary` and app version.
pub struct IncludedSession {
    pub record: ExportRecord,
    pub gaze: Option<Value>,
    pub app_version: Option<String>,
}

/// Everything [`package_input`] maps besides the sessions.
pub struct CaseFacts {
    pub watermark: Watermark,
    /// The export row's `created_at` (RFC 3339), not the render time: a retry renders the
    /// same bytes (XC-8).
    pub created_at: String,
    pub deid_salt: [u8; deid::SALT_LEN],
    pub recipient: String,
    pub consent: Option<CaseConsent>,
}

/// Rows → [`CasePackageInput`]. Encounters and attachments stay empty until WP-I.13b.
pub fn package_input(
    facts: CaseFacts,
    sessions: Vec<IncludedSession>,
) -> Result<CasePackageInput, RenderError> {
    if sessions.is_empty() {
        return Err(RenderError::NoRecords);
    }
    let sessions = sessions
        .into_iter()
        .map(|s| {
            let gaze = s
                .gaze
                .and_then(|v| asd_export::stored_record(v).ok())
                .ok_or(RenderError::StoredRecord)?;
            let version = s.app_version.unwrap_or_else(|| "0".to_owned());
            Ok(CaseSession {
                record: s.record,
                gaze,
                device: FhirDevice::new(DEFAULT_DEVICE_NAME, &version),
            })
        })
        .collect::<Result<Vec<_>, RenderError>>()?;
    Ok(CasePackageInput {
        watermark: facts.watermark,
        created_at: facts.created_at,
        deid_salt: facts.deid_salt,
        recipient: facts.recipient,
        consent: facts.consent,
        sessions,
        encounters: Vec::new(),
        attachments: Vec::new(),
        fhir_profile: FhirProfile::default(),
    })
}

/// Renders the package (SHR-1): `export-{id}-case-package.zip`.
pub fn render(input: &CasePackageInput) -> Result<Artifact, RenderError> {
    let bytes = case_package::build(input)?;
    Ok(Artifact {
        bytes,
        file_name: format!("export-{}-case-package.zip", input.watermark.export_id),
        content_type: case_package::CONTENT_TYPE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_case_is_refused_before_rendering() {
        let facts = CaseFacts {
            watermark: Watermark {
                export_id: Uuid::nil().to_string(),
                requested_by_display: "Dr A".into(),
                requested_at_utc: "2026-11-03T00:00:00Z".into(),
                mode: asd_export::policy::Mode::Identified,
                purpose: asd_export::policy::Purpose::ClinicalCare,
                org_name: "Clinic A".into(),
            },
            created_at: "2026-11-03T00:00:00Z".into(),
            deid_salt: [0; deid::SALT_LEN],
            recipient: "Dr B, City Hospital".into(),
            consent: None,
        };
        assert!(matches!(
            package_input(facts, Vec::new()),
            Err(RenderError::NoRecords)
        ));
    }
}
