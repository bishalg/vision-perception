//! owner: WP-I.12
//!
//! "Share a case" (ADR-0011 D5, SHR-1..6): one participant's sessions, visits and
//! documents as a single deterministic ZIP a clinic can hand to another clinician or a
//! researcher.
//!
//! A case may hold no session at all (a child seen for visits and documents before any gaze
//! session exists): the caller then names the participant and org in
//! [`CasePackageInput::subject`], the FHIR bundle holds the `Patient` and `Organization`
//! only, `data/sessions.jsonl` is its manifest line alone, `data/clips.csv` its header, and
//! `manifest.date_range` spans the encounters (or the package date when there are none).
//!
//! Entries, exactly these and in this order (SHR-1):
//!
//! | path | content |
//! |---|---|
//! | `manifest.json` | [`CasePackageManifest`] (SHR-2) |
//! | `README.txt` | what this is, the mandatory sentences, `sha256sum -c` lines (SHR-6) |
//! | `fhir/bundle.json` | one `collection` Bundle over every session (SHR-4, [`bundle`]) |
//! | `report/case-summary.pdf` | cover, a page per session, encounters (SHR-5; feature `pdf`, otherwise listed in `absent[]`) |
//! | `data/sessions.jsonl` | de-identified: the [`crate::dataset`] JSON Lines; identified: one [`ExportRecord`] per line after a manifest line |
//! | `data/clips.csv` | the canonical CSV clips table ([`crate::tabular::csv`]) |
//! | `notes/notes.json` | identified only: encounters, session notes, letters, review text |
//! | `attachments/<id>-<file_name>` | identified only: one entry per attachment |
//!
//! No directory entries, no extra fields, fixed timestamps (1980-01-01) and a fixed deflate
//! level: identical input gives identical bytes (XC-8, trap 7), so a retried job writes
//! the same `exports.sha256`.
//!
//! De-identified mode (SHR-3) runs [`crate::deid::apply`] with **one** salt for every file
//! (trap 6), so the `P-`/`S-` pseudonyms join across the FHIR bundle, the JSON Lines and
//! the CSV. The salt is never written into the archive. Notes, letters, encounter free
//! text, clinician names and attachments are never written in that mode.
//!
//! The crate renders what it is given: the backend decides which sessions are eligible
//! (one participant, [`crate::policy`]) and leaves out the MRN for callers without
//! `PatientRefRead` (MRN-6). The checks here refuse inconsistent input instead of
//! silently dropping anything.

use std::io::Write as _;

use asd_contract::emr::sanitize_file_name;
use asd_contract::session_v1_6::GazeSessionRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::deid::{self, SALT_LEN};
use crate::fhir::clinical::FhirProfile;
use crate::fhir::FhirDevice;
use crate::model::{ExportRecord, LetterKind, OrgFacts, PatientRef};
use crate::policy::{self, ConsentFacts, Mode};
use crate::tabular::columns::Table;
use crate::tabular::{csv, Dialect, TabularOptions};
use crate::watermark::{Watermark, MANDATORY_SENTENCE};
use crate::{dataset, ExportError};

mod bundle;
pub mod manifest;
mod readme;
#[cfg(feature = "pdf")]
mod summary;

pub use manifest::{
    manifest_schema, CasePackageManifest, DEIDENTIFIED_RECIPIENT, SCHEMA_FILE, SCHEMA_ID,
    SCHEMA_TAG,
};

/// Media type of the package.
pub const CONTENT_TYPE: &str = "application/zip";

/// Entry paths (SHR-1).
pub mod paths {
    pub const MANIFEST: &str = "manifest.json";
    pub const README: &str = "README.txt";
    pub const FHIR_BUNDLE: &str = "fhir/bundle.json";
    pub const CASE_SUMMARY_PDF: &str = "report/case-summary.pdf";
    pub const SESSIONS: &str = "data/sessions.jsonl";
    pub const CLIPS: &str = "data/clips.csv";
    pub const NOTES: &str = "notes/notes.json";
    /// Prefix of `attachments/<attachment_id>-<file_name>`.
    pub const ATTACHMENTS: &str = "attachments/";
}

/// One session of the case.
#[derive(Debug, Clone)]
pub struct CaseSession {
    /// The export record, identified (the package de-identifies it when the mode says so).
    pub record: ExportRecord,
    /// The stored session (`sessions.summary`) the gaze Observation is built from.
    pub gaze: GazeSessionRecord,
    /// The recording app (FHIR `Device`).
    pub device: FhirDevice,
}

/// `encounters.kind` (REC-3); wire names as in `asd-contract::emr` (WP-R.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterKind {
    ScreeningVisit,
    FollowUpVisit,
    PhoneCall,
    HomeVisit,
    Other,
}

impl EncounterKind {
    /// Printed label. The `screening_visit` wire name is fixed by REC-3; printed copy
    /// avoids the word (ADR-0004 claims denylist).
    pub fn label(self) -> &'static str {
        match self {
            Self::ScreeningVisit => "Intake visit",
            Self::FollowUpVisit => "Follow-up visit",
            Self::PhoneCall => "Phone call",
            Self::HomeVisit => "Home visit",
            Self::Other => "Other",
        }
    }
}

/// `encounters.status` (REC-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterStatus {
    Open,
    Closed,
}

/// One encounter of the participant (REC-3), as the backend read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseEncounter {
    pub id: String,
    pub kind: EncounterKind,
    /// RFC 3339.
    pub occurred_at: String,
    pub status: EncounterStatus,
    /// Clinician display name (NOTE-6). Identified mode only.
    pub clinician_display: Option<String>,
    /// Short reason text. Identified mode only.
    pub reason: Option<String>,
    /// The clinician's summary, only when the caller may read it. Identified mode only.
    pub summary: Option<String>,
    /// RFC 3339.
    pub follow_up_due_at: Option<String>,
    /// RFC 3339.
    pub follow_up_done_at: Option<String>,
    /// Linked session ids. Identified mode only.
    #[serde(default)]
    pub session_ids: Vec<String>,
}

/// `attachments.kind` (REC-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    PaperForm,
    Referral,
    Report,
    PhotoOfDocument,
    Other,
}

/// One attachment with its decrypted bytes (identified packages only, REC-6).
#[derive(Clone, PartialEq, Eq)]
pub struct CaseAttachment {
    pub id: String,
    pub kind: AttachmentKind,
    /// Sanitised to `[A-Za-z0-9._-]` by the backend; sanitised again here.
    pub file_name: String,
    /// `application/pdf`, `image/jpeg` or `image/png`.
    pub media_type: String,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for CaseAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaseAttachment")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("media_type", &self.media_type)
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// The consent an identified package relies on (SHR-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseConsent {
    pub reference: String,
    /// RFC 3339.
    pub recorded_at: String,
    pub recorded_by_display: String,
}

/// The participant and org of a case, for a package without sessions (identified: the
/// package de-identifies it when the mode says so).
#[derive(Debug, Clone)]
pub struct CaseSubject {
    /// `organizations.id` (the FHIR `Organization` identifier).
    pub org_id: String,
    pub org: OrgFacts,
    pub patient: PatientRef,
    /// The participant's consent facts; [`crate::policy::eligibility`] must include them.
    pub consent: ConsentFacts,
}

/// Everything one case package is built from. The crate's own input type (as
/// [`ExportRecord`] is); the backend maps `CaseShareRequest` (WP-R.7) onto it.
#[derive(Debug, Clone)]
pub struct CasePackageInput {
    /// Export id, requester, time, mode and purpose (XC-6).
    pub watermark: Watermark,
    /// RFC 3339 UTC render time (`manifest.created_at`); an input, the crate has no clock.
    pub created_at: String,
    /// Per-export pseudonym salt (XC-4); drawn by the caller, never written anywhere.
    pub deid_salt: [u8; SALT_LEN],
    /// Recipient label (identified packages); de-identified packages print
    /// [`DEIDENTIFIED_RECIPIENT`] whatever this holds.
    pub recipient: String,
    /// Required for identified packages (SHR-7); ignored when de-identified.
    pub consent: Option<CaseConsent>,
    /// Every session of one participant, eligible under [`crate::policy`]. Any order: the
    /// package sorts them by start time.
    pub sessions: Vec<CaseSession>,
    /// Encounters of the participant (identified text only in identified mode).
    pub encounters: Vec<CaseEncounter>,
    /// Attachments; written only in identified mode.
    pub attachments: Vec<CaseAttachment>,
    pub fhir_profile: FhirProfile,
    /// The participant and org when `sessions` is empty (required then). `None`: taken
    /// from the sessions. When both are given they must name the same participant.
    pub subject: Option<CaseSubject>,
}

fn input_error(message: impl Into<String>) -> ExportError {
    ExportError::CasePackage(message.into())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn utc(field: &'static str, rfc3339: &str) -> Result<chrono::DateTime<chrono::Utc>, ExportError> {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.with_timezone(&chrono::Utc))
        .map_err(|_| ExportError::InvalidTimestamp { field })
}

/// The case after validation, sorting and (de-identified mode) [`deid::apply`]: what every
/// renderer reads.
pub(crate) struct Case<'a> {
    pub input: &'a CasePackageInput,
    pub mode: Mode,
    /// Sessions in date order (start time, then id), as given.
    pub sessions: Vec<&'a CaseSession>,
    /// The same records, de-identified in that mode.
    pub records: Vec<ExportRecord>,
    /// Encounters in date order (identified text stripped in de-identified mode).
    pub encounters: Vec<CaseEncounter>,
    /// The participant (de-identified in that mode: pseudonym, no MRN).
    pub participant: PatientRef,
    /// The sending org.
    pub org: OrgFacts,
    /// `organizations.id`.
    pub org_id: String,
}

impl<'a> Case<'a> {
    fn new(input: &'a CasePackageInput) -> Result<Self, ExportError> {
        let wm = &input.watermark;
        let mode = wm.mode;
        policy::validate_request(wm.purpose, mode)?;

        let (patient, org, org_id) = match (input.sessions.first(), &input.subject) {
            (_, Some(subject)) => (&subject.patient, &subject.org, subject.org_id.as_str()),
            (Some(first), None) => (
                &first.record.patient,
                &first.record.org,
                first.record.session.org_id.as_str(),
            ),
            (None, None) => {
                return Err(input_error(
                    "a case package without sessions needs its subject (participant and org)",
                ))
            }
        };
        let participant = patient.participant_id.as_str();
        if input
            .sessions
            .iter()
            .any(|s| s.record.patient.participant_id != participant)
        {
            return Err(input_error("a case package holds one participant only"));
        }
        if input.sessions.is_empty() {
            let shared_files = mode == Mode::Identified && !input.attachments.is_empty();
            if input.encounters.is_empty() && !shared_files {
                return Err(input_error(
                    "a case package needs a session, an encounter or an attachment",
                ));
            }
            if let Some(subject) = &input.subject {
                if let policy::Eligibility::Exclude(reason) =
                    policy::eligibility(wm.purpose, mode, &subject.consent)
                {
                    return Err(input_error(format!(
                        "the participant is not eligible for this purpose and mode: {}",
                        reason.as_str()
                    )));
                }
            }
        }
        let originals: Vec<ExportRecord> =
            input.sessions.iter().map(|s| s.record.clone()).collect();
        let selection = policy::select(&originals, wm.purpose, mode)?;
        if selection.included.len() != originals.len() {
            return Err(input_error(
                "a session is not eligible for this purpose and mode (policy.rs)",
            ));
        }
        if mode == Mode::Identified {
            match &input.consent {
                Some(c) if !c.reference.trim().is_empty() => {
                    utc("consent.recorded_at", &c.recorded_at)?;
                }
                _ => {
                    return Err(input_error(
                        "an identified package needs a consent reference (SHR-7)",
                    ))
                }
            }
        }
        utc("created_at", &input.created_at)?;

        let mut sessions: Vec<(chrono::DateTime<chrono::Utc>, &CaseSession)> = input
            .sessions
            .iter()
            .map(|s| {
                Ok((
                    utc("session.started_at", &s.record.session.summary.started_at)?,
                    s,
                ))
            })
            .collect::<Result<_, ExportError>>()?;
        sessions.sort_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| {
                a.1.record
                    .session
                    .summary
                    .id
                    .cmp(&b.1.record.session.summary.id)
            })
        });
        let sessions: Vec<&CaseSession> = sessions.into_iter().map(|(_, s)| s).collect();
        let sorted: Vec<ExportRecord> = sessions.iter().map(|s| s.record.clone()).collect();
        let records = match mode {
            Mode::Identified => sorted,
            Mode::Deidentified => deid::apply(&sorted, &input.deid_salt)?,
        };

        let mut encounters: Vec<(chrono::DateTime<chrono::Utc>, CaseEncounter)> = input
            .encounters
            .iter()
            .map(|e| Ok((utc("encounter.occurred_at", &e.occurred_at)?, e.clone())))
            .collect::<Result<_, ExportError>>()?;
        encounters.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
        let encounters = encounters
            .into_iter()
            .map(|(at, mut e)| {
                if mode == Mode::Deidentified {
                    e.id = String::new();
                    e.occurred_at = at.format("%Y-%m").to_string();
                    e.clinician_display = None;
                    e.reason = None;
                    e.summary = None;
                    e.session_ids.clear();
                    e.follow_up_due_at = e
                        .follow_up_due_at
                        .as_deref()
                        .map(|t| {
                            utc("encounter.follow_up_due_at", t)
                                .map(|t| t.format("%Y-%m").to_string())
                        })
                        .transpose()?;
                    e.follow_up_done_at = e
                        .follow_up_done_at
                        .as_deref()
                        .map(|t| {
                            utc("encounter.follow_up_done_at", t)
                                .map(|t| t.format("%Y-%m").to_string())
                        })
                        .transpose()?;
                }
                Ok(e)
            })
            .collect::<Result<_, ExportError>>()?;

        let participant = match mode {
            Mode::Identified => patient.clone(),
            Mode::Deidentified => {
                let p = deid::pseudonym(&input.deid_salt, "P-", participant);
                PatientRef {
                    participant_id: p.clone(),
                    participant_token: p,
                    mrn: None,
                    age_bracket_at_enrol: patient.age_bracket_at_enrol.clone(),
                }
            }
        };

        Ok(Self {
            input,
            mode,
            sessions,
            records,
            encounters,
            participant,
            org: org.clone(),
            org_id: org_id.to_owned(),
        })
    }

    pub(crate) fn deidentified(&self) -> bool {
        self.mode == Mode::Deidentified
    }

    /// `P-` pseudonym or token.
    pub(crate) fn participant_label(&self) -> &str {
        &self.participant.participant_token
    }

    /// First and last session date ([`Self::session_date`]). Without sessions: the first
    /// and last encounter, else the package date, in the same precision.
    pub(crate) fn date_range(&self) -> Result<(String, String), ExportError> {
        if let (Some(first), Some(last)) = (self.records.first(), self.records.last()) {
            return Ok((self.session_date(first)?, self.session_date(last)?));
        }
        // Identified: the org's local date; de-identified: `YYYY-MM` UTC (encounter dates
        // already are, Case::new).
        let day = |field: &'static str, t: &str| -> Result<String, ExportError> {
            let at = utc(field, t)?;
            Ok(if self.deidentified() {
                at.format("%Y-%m").to_string()
            } else {
                at.with_timezone(&self.org.tz()?)
                    .format("%Y-%m-%d")
                    .to_string()
            })
        };
        let encounter_day = |e: &CaseEncounter| -> Result<String, ExportError> {
            if self.deidentified() {
                Ok(e.occurred_at.clone())
            } else {
                day("encounter.occurred_at", &e.occurred_at)
            }
        };
        match (self.encounters.first(), self.encounters.last()) {
            (Some(first), Some(last)) => Ok((encounter_day(first)?, encounter_day(last)?)),
            _ => {
                let d = day("created_at", &self.input.created_at)?;
                Ok((d.clone(), d))
            }
        }
    }

    pub(crate) fn recipient_label(&self) -> &str {
        if self.deidentified() {
            DEIDENTIFIED_RECIPIENT
        } else {
            &self.input.recipient
        }
    }

    /// `YYYY-MM-DD` in the org zone (identified) or the `YYYY-MM` the de-identified record
    /// carries.
    pub(crate) fn session_date(&self, record: &ExportRecord) -> Result<String, ExportError> {
        if self.deidentified() {
            return Ok(record.session.summary.started_at.clone());
        }
        let tz = record.org.tz()?;
        Ok(
            utc("session.started_at", &record.session.summary.started_at)?
                .with_timezone(&tz)
                .format("%Y-%m-%d")
                .to_string(),
        )
    }
}

/// One rendered entry.
struct Entry {
    path: String,
    media_type: &'static str,
    bytes: Vec<u8>,
}

impl Entry {
    fn new(path: impl Into<String>, media_type: &'static str, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            media_type,
            bytes,
        }
    }

    fn file_entry(&self) -> manifest::FileEntry {
        manifest::FileEntry {
            path: self.path.clone(),
            media_type: self.media_type.to_owned(),
            bytes: self.bytes.len() as u64,
            sha256: hex(&Sha256::digest(&self.bytes)),
        }
    }
}

/// Renders the package (SHR-1). Identical input gives identical bytes.
pub fn build(input: &CasePackageInput) -> Result<Vec<u8>, ExportError> {
    let (_, bytes) = build_with_manifest(input)?;
    Ok(bytes)
}

/// [`build`], also returning the manifest written into the archive (for the caller's
/// export row and tests).
pub fn build_with_manifest(
    input: &CasePackageInput,
) -> Result<(CasePackageManifest, Vec<u8>), ExportError> {
    let case = Case::new(input)?;
    let wm = &input.watermark;

    let mut entries: Vec<Entry> = Vec::new();
    let mut absent: Vec<manifest::AbsentFile> = Vec::new();

    entries.push(Entry::new(
        paths::FHIR_BUNDLE,
        "application/fhir+json",
        bundle::encode(&case)?,
    ));
    pdf_entry(&case, &mut entries, &mut absent)?;
    entries.push(Entry::new(
        paths::SESSIONS,
        "application/jsonl",
        sessions_jsonl(&case)?,
    ));
    // The tabular encoder de-identifies with the same salt (trap 6): same `S-` ids.
    let originals: Vec<ExportRecord> = case.sessions.iter().map(|s| s.record.clone()).collect();
    entries.push(Entry::new(
        paths::CLIPS,
        "text/csv",
        csv::encode(
            &originals,
            &TabularOptions::new(wm.clone(), input.deid_salt),
            Dialect::Canonical,
            Table::Clips,
        )?,
    ));
    if !case.deidentified() {
        entries.push(Entry::new(
            paths::NOTES,
            "application/json",
            notes_json(&case),
        ));
        let mut attachments: Vec<&CaseAttachment> = input.attachments.iter().collect();
        attachments.sort_by(|a, b| a.id.cmp(&b.id));
        for a in attachments {
            let media_type = match a.media_type.as_str() {
                "application/pdf" => "application/pdf",
                "image/jpeg" => "image/jpeg",
                "image/png" => "image/png",
                _ => return Err(input_error("attachment media type not allowed (REC-6)")),
            };
            entries.push(Entry::new(
                format!(
                    "{}{}-{}",
                    paths::ATTACHMENTS,
                    sanitize_file_name(&a.id),
                    sanitize_file_name(&a.file_name)
                ),
                media_type,
                a.bytes.clone(),
            ));
        }
    }

    let files: Vec<manifest::FileEntry> = entries.iter().map(Entry::file_entry).collect();
    let readme = Entry::new(
        paths::README,
        "text/plain; charset=utf-8",
        readme::render(&case, &files, &absent),
    );
    let mut all_files = vec![readme.file_entry()];
    all_files.extend(files);

    let manifest = manifest_of(&case, all_files, absent)?;
    let mut ordered = vec![
        Entry::new(
            paths::MANIFEST,
            "application/json",
            manifest.to_json_bytes(),
        ),
        readme,
    ];
    ordered.extend(entries);
    Ok((manifest, zip(&ordered)?))
}

#[cfg(feature = "pdf")]
fn pdf_entry(
    case: &Case<'_>,
    entries: &mut Vec<Entry>,
    _absent: &mut Vec<manifest::AbsentFile>,
) -> Result<(), ExportError> {
    entries.push(Entry::new(
        paths::CASE_SUMMARY_PDF,
        "application/pdf",
        summary::render(case)?,
    ));
    Ok(())
}

#[cfg(not(feature = "pdf"))]
fn pdf_entry(
    _case: &Case<'_>,
    _entries: &mut Vec<Entry>,
    absent: &mut Vec<manifest::AbsentFile>,
) -> Result<(), ExportError> {
    absent.push(manifest::AbsentFile {
        path: paths::CASE_SUMMARY_PDF.to_owned(),
        media_type: "application/pdf".to_owned(),
        reason: "built without the pdf feature".to_owned(),
    });
    Ok(())
}

fn manifest_of(
    case: &Case<'_>,
    files: Vec<manifest::FileEntry>,
    absent: Vec<manifest::AbsentFile>,
) -> Result<CasePackageManifest, ExportError> {
    let input = case.input;
    let wm = &input.watermark;
    let org = &case.org;
    let participant = if case.deidentified() {
        manifest::ParticipantInfo::Deidentified {
            pseudonym: case.participant_label().to_owned(),
        }
    } else {
        manifest::ParticipantInfo::Identified {
            token: case.participant_label().to_owned(),
            mrn: case.participant.mrn.as_ref().map(|m| manifest::MrnInfo {
                authority: m.assigning_authority.clone(),
                value: m.value.clone(),
            }),
        }
    };
    let consent = match (&input.consent, case.deidentified()) {
        (Some(c), false) => Some(manifest::ConsentInfo {
            reference: c.reference.clone(),
            recorded_at: c.recorded_at.clone(),
            recorded_by_display: c.recorded_by_display.clone(),
        }),
        _ => None,
    };
    let (from, to) = case.date_range()?;
    Ok(CasePackageManifest {
        schema: SCHEMA_TAG.to_owned(),
        package_id: wm.export_id.clone(),
        created_at: input.created_at.clone(),
        generator: manifest::Generator {
            crate_name: env!("CARGO_PKG_NAME").to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        org: manifest::OrgInfo {
            name: org.name.clone(),
            timezone: org.timezone.clone(),
        },
        mode: wm.mode,
        purpose: wm.purpose,
        recipient: manifest::Recipient {
            label: case.recipient_label().to_owned(),
        },
        consent,
        participant,
        sessions_n: case.records.len() as u64,
        date_range: manifest::DateRange { from, to },
        files,
        absent,
        watermark: wm.line(),
        not_diagnostic: true,
    })
}

/// De-identified: the [`dataset`] JSON Lines (manifest line, one `DatasetRow` per session;
/// it de-identifies with the same salt). Identified: a manifest line, then one
/// [`ExportRecord`] per session (`record_type: "session"`).
fn sessions_jsonl(case: &Case<'_>) -> Result<Vec<u8>, ExportError> {
    let input = case.input;
    if case.deidentified() {
        let originals: Vec<ExportRecord> = case.sessions.iter().map(|s| s.record.clone()).collect();
        return dataset::encode_jsonl(
            &originals,
            &[],
            &dataset::DatasetOptions {
                watermark: input.watermark.clone(),
                deid_salt: input.deid_salt,
                include_labels: false,
            },
        );
    }

    #[derive(Serialize)]
    struct Header<'a> {
        record_type: &'static str,
        format: &'static str,
        record_version: &'static str,
        export_id: &'a str,
        watermark: String,
        notice: &'static str,
        mode: Mode,
        purpose: policy::Purpose,
        row_count: usize,
    }
    #[derive(Serialize)]
    struct Line<'a> {
        record_type: &'static str,
        #[serde(flatten)]
        record: &'a ExportRecord,
    }
    let wm = &input.watermark;
    let mut out = serde_json::to_vec(&Header {
        record_type: "manifest",
        format: "export-record",
        record_version: crate::model::RECORD_VERSION,
        export_id: &wm.export_id,
        watermark: wm.line(),
        notice: MANDATORY_SENTENCE,
        mode: wm.mode,
        purpose: wm.purpose,
        row_count: case.records.len(),
    })
    .expect("plain data");
    out.push(b'\n');
    for record in &case.records {
        serde_json::to_writer(
            &mut out,
            &Line {
                record_type: "session",
                record,
            },
        )
        .expect("plain data");
        out.push(b'\n');
    }
    Ok(out)
}

/// `notes/notes.json` (identified only): encounters, then per session its notes, letters
/// and review text.
fn notes_json(case: &Case<'_>) -> Vec<u8> {
    #[derive(Serialize)]
    struct Notes<'a> {
        schema: &'static str,
        package_id: &'a str,
        notice: &'static str,
        encounters: &'a [CaseEncounter],
        sessions: Vec<SessionNotes<'a>>,
    }
    #[derive(Serialize)]
    struct SessionNotes<'a> {
        session_id: &'a str,
        started_at: &'a str,
        impression: Option<&'a str>,
        recommendations: Option<&'a str>,
        notes: &'a [crate::model::NoteText],
        letters: Vec<Letter<'a>>,
    }
    #[derive(Serialize)]
    struct Letter<'a> {
        kind: LetterKind,
        recipient: &'a str,
        body: &'a str,
        finalized_at: Option<&'a str>,
    }
    let notes = Notes {
        schema: "case-notes/1.0",
        package_id: &case.input.watermark.export_id,
        notice: MANDATORY_SENTENCE,
        encounters: &case.encounters,
        sessions: case
            .records
            .iter()
            .map(|r| SessionNotes {
                session_id: &r.session.summary.id,
                started_at: &r.session.summary.started_at,
                impression: r.review.as_ref().and_then(|v| v.impression.as_deref()),
                recommendations: r.review.as_ref().and_then(|v| v.recommendations.as_deref()),
                notes: &r.notes,
                letters: r
                    .letters
                    .iter()
                    .map(|l| Letter {
                        kind: l.kind,
                        recipient: &l.recipient,
                        body: &l.body,
                        finalized_at: l.finalized_at.as_deref(),
                    })
                    .collect(),
            })
            .collect(),
    };
    let mut out = serde_json::to_vec_pretty(&notes).expect("plain data");
    out.push(b'\n');
    out
}

/// Deflate level of every entry (trap 7: fixed, or the bytes change between builds).
const DEFLATE_LEVEL: i64 = 6;

fn zip(entries: &[Entry]) -> Result<Vec<u8>, ExportError> {
    let zip_err = |e: zip::result::ZipError| input_error(format!("zip: {e}"));
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(DEFLATE_LEVEL))
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for e in entries {
        zip.start_file(e.path.as_str(), opts).map_err(zip_err)?;
        zip.write_all(&e.bytes)
            .map_err(|err| input_error(format!("zip: {err}")))?;
    }
    Ok(zip.finish().map_err(zip_err)?.into_inner())
}
