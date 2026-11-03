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
//! [`complete`] (WP-I.13b) adds the participant's encounters (REC-3) and attachments
//! (REC-6) once the sessions are mapped:
//!
//! - encounters travel in both modes (the crate strips everything but kind, status and
//!   months when de-identified); in a de-identified package the backend already leaves
//!   out the clinician, reason, summary and session links, and never decrypts a summary;
//! - an encounter's summary is decrypted only for an identified package whose requester
//!   held `ClinicalContentRead` (`exports.include_clinical_content`), like session notes;
//! - attachments are read only for an identified package that asked for them
//!   (`exports.case_include_attachments`) and whose requester held `ClinicalContentRead`
//!   (reading an attachment is that right, REC-7), through WP-B.19's decrypting reader
//!   [`crate::routes::attachments::load_content`]; rows without uploaded content and
//!   deleted rows are skipped. The request is audited by `export.create` /
//!   `export.complete`; no per-attachment read row is written (trap 11).

use std::collections::HashMap;

use asd_contract::export::ExportMode;
use asd_export::case_package::{
    self, AttachmentKind, CaseAttachment, CaseConsent, CaseEncounter, CasePackageInput,
    CaseSession, EncounterKind, EncounterStatus,
};
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
use crate::blob::BlobStore;
use crate::crypto::{Aad, Keyring};
use crate::error::{AppError, AppResult};
use crate::jobs::runner::JobError;
use crate::jobs::JobContext;
use crate::read::cursor;
use crate::routes::attachments;

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

/// Rows → [`CasePackageInput`]. Encounters and attachments are added by [`complete`].
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

/// What [`complete`] reads, given the export row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseScope {
    /// Identified package: encounter headers keep clinician, reason and session links.
    pub identified: bool,
    /// Encounter summaries are decrypted (identified and `ClinicalContentRead`).
    pub summaries: bool,
    /// Attachments are decrypted and included.
    pub attachments: bool,
}

impl CaseScope {
    pub fn of(row: &ExportRow) -> Self {
        let identified = row.mode == ExportMode::Identified;
        let clinical = identified && row.include_clinical_content;
        Self {
            identified,
            summaries: clinical,
            attachments: clinical && row.case_include_attachments,
        }
    }
}

/// One encounter row; the summary still sealed (and only read when in scope).
struct EncounterRow {
    id: Uuid,
    kind: String,
    occurred_at: chrono::DateTime<chrono::Utc>,
    status: String,
    clinician_name: Option<String>,
    reason: Option<String>,
    summary: Option<Sealed>,
    follow_up_due_at: Option<chrono::DateTime<chrono::Utc>>,
    follow_up_done_at: Option<chrono::DateTime<chrono::Utc>>,
    session_ids: Vec<String>,
}

async fn load_encounters(
    pool: &PgPool,
    org: Uuid,
    participant: Uuid,
    scope: CaseScope,
) -> AppResult<Vec<EncounterRow>> {
    let rows = sqlx::query!(
        r#"SELECT e.id, e.kind, e.occurred_at, e.status, e.reason, e.summary_ct,
                  e.summary_key_id, e.follow_up_due_at, e.follow_up_done_at,
                  u.display_name AS "clinician_name?",
                  coalesce((SELECT array_agg(s.id ORDER BY s.id) FROM sessions s
                            WHERE s.encounter_id = e.id), '{}') AS "session_ids!"
           FROM encounters e LEFT JOIN users u ON u.id = e.clinician_id
           WHERE e.org_id = $1 AND e.participant_id = $2
           ORDER BY e.occurred_at, e.id"#,
        org,
        participant,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let summary = match (scope.summaries, r.summary_ct, r.summary_key_id) {
                (true, Some(ciphertext), Some(key_id)) => Some(Sealed {
                    table: "encounters",
                    column: "summary_ct",
                    row_id: r.id,
                    key_id,
                    ciphertext,
                }),
                _ => None,
            };
            EncounterRow {
                id: r.id,
                kind: r.kind,
                occurred_at: r.occurred_at,
                status: r.status,
                clinician_name: r.clinician_name,
                reason: r.reason,
                summary,
                follow_up_due_at: r.follow_up_due_at,
                follow_up_done_at: r.follow_up_done_at,
                session_ids: r.session_ids,
            }
        })
        .collect())
}

/// A database spelling → the crate's enum (same snake_case wire names).
fn wire<T: serde::de::DeserializeOwned>(what: &str, s: &str) -> AppResult<T> {
    serde_json::from_value(Value::String(s.to_owned()))
        .map_err(|_| AppError::Internal(format!("unknown {what} {s:?}")))
}

/// One encounter row → [`CaseEncounter`] with its (already decrypted) summary. In a
/// de-identified package only kind, status and dates are kept (the crate truncates the
/// dates to months), and session links name only sessions that are in the package.
fn map_encounter(
    row: EncounterRow,
    summary: Option<String>,
    scope: CaseScope,
    included: &[String],
) -> AppResult<CaseEncounter> {
    let identified = scope.identified;
    Ok(CaseEncounter {
        id: row.id.to_string(),
        kind: wire::<EncounterKind>("encounter kind", &row.kind)?,
        occurred_at: cursor::ts(row.occurred_at),
        status: wire::<EncounterStatus>("encounter status", &row.status)?,
        clinician_display: row.clinician_name.filter(|_| identified),
        reason: row.reason.filter(|_| identified),
        summary: summary.filter(|_| scope.summaries),
        follow_up_due_at: row.follow_up_due_at.map(cursor::ts),
        follow_up_done_at: row.follow_up_done_at.map(cursor::ts),
        session_ids: if identified {
            row.session_ids
                .into_iter()
                .filter(|s| included.contains(s))
                .collect()
        } else {
            Vec::new()
        },
    })
}

/// The live attachments of `participant` that have content, in id order.
async fn attachment_ids(pool: &PgPool, org: Uuid, participant: Uuid) -> AppResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM attachments
         WHERE org_id = $1 AND participant_id = $2 AND deleted_at IS NULL
           AND blob_key IS NOT NULL
         ORDER BY id",
        org,
        participant,
    )
    .fetch_all(pool)
    .await?)
}

/// Decrypts every attachment through WP-B.19's reader (digest-checked) and maps it.
async fn load_attachments(
    pool: &PgPool,
    blobs: &dyn BlobStore,
    keyring: &Keyring,
    org: Uuid,
    participant: Uuid,
) -> AppResult<Vec<CaseAttachment>> {
    let mut out = Vec::new();
    for id in attachment_ids(pool, org, participant).await? {
        let content = match attachments::load_content(pool, blobs, keyring, org, id).await {
            Ok(c) => c,
            // Deleted between the listing and the read: not part of the package.
            Err(AppError::NotFound) => continue,
            Err(e) => return Err(e),
        };
        let a = content.attachment;
        out.push(CaseAttachment {
            id: a.id,
            kind: wire::<AttachmentKind>("attachment kind", a.kind.as_str())?,
            file_name: a.file_name,
            media_type: a.media_type.as_str().to_owned(),
            bytes: content.bytes,
        });
    }
    Ok(out)
}

fn crypto_error(e: AppError) -> JobError {
    match e {
        AppError::Database(_) | AppError::Unavailable => JobError::Transient(e.to_string()),
        other => JobError::Permanent(format!("case-package: {other}")),
    }
}

/// Adds the participant's encounters and (when asked for) attachments to `input`, as
/// [`CaseScope::of`] the export row allows (see the module docs). Runs in the job's render
/// phase, with no transaction open.
pub async fn complete(
    ctx: &JobContext,
    row: &ExportRow,
    participant: Uuid,
    input: &mut CasePackageInput,
) -> Result<(), JobError> {
    let scope = CaseScope::of(row);
    let org = row.org_id;
    let included: Vec<String> = input
        .sessions
        .iter()
        .map(|s| s.record.session.summary.id.clone())
        .collect();
    let rows = load_encounters(&ctx.pool, org, participant, scope).await?;
    let needs_keys = scope.attachments || rows.iter().any(|r| r.summary.is_some());
    let keyring = if needs_keys {
        Some(ctx.keyring.as_ref().ok_or_else(|| {
            JobError::Permanent(
                "clinical-crypto-unavailable: this worker has no ASD_DATA_KEK".into(),
            )
        })?)
    } else {
        None
    };
    let mut encounters = Vec::with_capacity(rows.len());
    for mut r in rows {
        let summary = match (r.summary.take(), keyring) {
            (Some(sealed), Some(keyring)) => Some(
                keyring
                    .decrypt_string(
                        org,
                        &Aad::new(sealed.table, sealed.column, org, sealed.row_id),
                        sealed.key_id,
                        &sealed.ciphertext,
                    )
                    .await
                    .map_err(|e| crypto_error(e.into()))?,
            ),
            _ => None,
        };
        encounters.push(map_encounter(r, summary, scope, &included)?);
    }
    input.encounters = encounters;
    input.attachments = match keyring.filter(|_| scope.attachments) {
        Some(keyring) => {
            let blobs = ctx
                .blobs
                .as_ref()
                .ok_or_else(|| JobError::Permanent("this worker has no blob store".into()))?;
            load_attachments(&ctx.pool, &**blobs, keyring, org, participant)
                .await
                .map_err(crypto_error)?
        }
        None => Vec::new(),
    };
    Ok(())
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
