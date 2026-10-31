//! owner: WP-I.1
//!
//! The canonical export input (XC-3): one [`ExportRecord`] per session. The backend builds
//! it from the database (WP-C.12 `export_build/record.rs`); the crate never queries
//! anything. Encoders read only this, after [`crate::policy`] chose the records and, for
//! de-identified exports, [`crate::deid::apply`] transformed them.
//!
//! Session facts reuse the `/v1` types (`SessionDetail`, `ClipScore` with its
//! `MetricRow`s), so the export shows exactly what the console shows. Clinical review
//! state and disposition mirror SIGN-1 / SIGN-2 with the same snake_case names as the
//! clinical DTOs in `asd-contract::clinical` (WP-C.9); this crate keeps its own copy so it
//! does not depend on the wire DTOs' lifecycle.
//!
//! Free text (impression, recommendations, notes, letters) and the MRN are present only
//! when the backend put them in, i.e. when the caller may read them (EXJ-2, MRN-6).
//!
//! Serialisation is deterministic (struct field order, `BTreeMap` keys, `ryu` floats),
//! which [`ExportRecord::to_json_bytes`] relies on. `record_version` changes with any
//! change to the wire shape (`tests/golden/export-record.json`).

use std::fmt;

use asd_contract::api_v1::{ClipScore, SessionDetail};
use serde::{Deserialize, Deserializer, Serialize};

use crate::policy::ConsentFacts;
use crate::ExportError;

/// Current [`ExportRecord::record_version`].
pub const RECORD_VERSION: &str = "1";

/// Everything every encoder needs about one session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportRecord {
    /// Always [`RECORD_VERSION`]; another value is refused on decode.
    #[serde(deserialize_with = "record_version")]
    pub record_version: String,
    pub org: OrgFacts,
    pub patient: PatientRef,
    /// Inputs of [`crate::policy::eligibility`].
    pub consent: ConsentFacts,
    /// The session as `GET /v1/sessions/{id}` returns it.
    pub session: SessionDetail,
    /// The clips as `GET /v1/sessions/{id}/clips` returns them, in order.
    pub clips: Vec<ClipScore>,
    /// The clinical review, when one exists.
    pub review: Option<ClinicalReviewSummary>,
    /// Clinical notes about the session (NOTE-1), only when the caller may read them.
    #[serde(default)]
    pub notes: Vec<NoteText>,
    /// Letters linked to the session (NOTE-3), only when the caller may read them.
    #[serde(default)]
    pub letters: Vec<LetterText>,
    /// Whole days since the participant's first session in the export. Set only by
    /// [`crate::deid::apply`], which reduces timestamps to `YYYY-MM` (XC-4).
    pub days_since_first_session: Option<i64>,
}

fn record_version<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = String::deserialize(d)?;
    if v == RECORD_VERSION {
        Ok(v)
    } else {
        Err(serde::de::Error::custom(format!(
            "unsupported record_version {v:?} (expected {RECORD_VERSION:?})"
        )))
    }
}

impl ExportRecord {
    /// A record with no review, notes or letters.
    pub fn new(
        org: OrgFacts,
        patient: PatientRef,
        consent: ConsentFacts,
        session: SessionDetail,
        clips: Vec<ClipScore>,
    ) -> Self {
        Self {
            record_version: RECORD_VERSION.to_owned(),
            org,
            patient,
            consent,
            session,
            clips,
            review: None,
            notes: Vec::new(),
            letters: Vec::new(),
            days_since_first_session: None,
        }
    }

    /// Compact JSON, identical bytes for identical records (XC-8).
    pub fn to_json_bytes(&self) -> Vec<u8> {
        // Plain data with string keys only: serialisation cannot fail.
        serde_json::to_vec(self).expect("ExportRecord serialises")
    }

    /// Status of the report about this session (FHX-4, HL7-4, PDF-4 banners).
    pub fn report_status(&self) -> ReportStatus {
        self.review.as_ref().map_or(
            ReportStatus::Preliminary,
            ClinicalReviewSummary::report_status,
        )
    }
}

/// Org facts printed on or used by every format (NOTE-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgFacts {
    pub name: String,
    /// IANA zone name (org setting `timezone`, default `UTC`; G11).
    pub timezone: String,
    pub paper: Paper,
    /// `settings.letterhead.address_lines` (at most 4).
    #[serde(default)]
    pub letterhead: Vec<String>,
}

impl OrgFacts {
    /// The org time zone; an unknown name is an error, never a silent UTC (trap 11).
    pub fn tz(&self) -> Result<chrono_tz::Tz, ExportError> {
        self.timezone
            .parse()
            .map_err(|_| ExportError::InvalidTimezone(self.timezone.clone()))
    }
}

/// Paper size (org setting `paper`, default A4; G10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

/// Who the record is about. Never a name, birth date or contact detail (MRN-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatientRef {
    /// `participants.id`; the de-identification pseudonym is derived from it (XC-4).
    pub participant_id: String,
    /// The pseudonymous token the device sent (CON-20).
    pub participant_token: String,
    /// The hospital MRN, only for identified exports by callers with `PatientRefRead`.
    pub mrn: Option<Mrn>,
    pub age_bracket_at_enrol: Option<String>,
}

/// A hospital medical record number (MRN-3). `Debug` never prints the value.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mrn {
    pub value: String,
    /// Short code of the assigning hospital, e.g. `TUTH` (not patient data).
    pub assigning_authority: String,
}

impl fmt::Debug for Mrn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mrn")
            .field("value", &"<redacted>")
            .field("assigning_authority", &self.assigning_authority)
            .finish()
    }
}

/// `clinical_reviews.state` (SIGN-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Open,
    InProgress,
    AwaitingCosign,
    Final,
    Amending,
}

/// The clinician's disposition (SIGN-2). The clinician's statement, never derived from the
/// attention flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    NoConcernNoted,
    Monitor,
    ReferForEvaluation,
    RecaptureRequested,
    NotInterpretable,
}

impl Disposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoConcernNoted => "no_concern_noted",
            Self::Monitor => "monitor",
            Self::ReferForEvaluation => "refer_for_evaluation",
            Self::RecaptureRequested => "recapture_requested",
            Self::NotInterpretable => "not_interpretable",
        }
    }
}

/// Report status shared by FHIR (`DiagnosticReport.status`, `docStatus`), HL7
/// (`OBR-25`/`OBX-11` `P`/`F`/`C`) and the PDF banners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Preliminary,
    Final,
    Amended,
}

impl ReportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preliminary => "preliminary",
            Self::Final => "final",
            Self::Amended => "amended",
        }
    }
}

/// The clinical review as exported (XC-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClinicalReviewSummary {
    pub state: ReviewState,
    /// Version the summary describes (`clinical_review_versions.version_no`).
    pub version_no: u32,
    /// The described version is a signed amendment of an earlier signed version (SIGN-8).
    pub amended: bool,
    pub disposition: Option<Disposition>,
    /// RFC 3339 time the review reached its current signed state.
    pub signed_at: Option<String>,
    /// Hex SHA-256 of the signed snapshot (SIGN-4).
    pub content_hash: Option<String>,
    /// Signatures of the described version, in signing order.
    pub signatures: Vec<Signature>,
    /// Only when the caller has `ClinicalContentRead` (EXJ-2).
    pub impression: Option<String>,
    pub recommendations: Option<String>,
}

impl ClinicalReviewSummary {
    /// At least one signature.
    pub fn is_signed(&self) -> bool {
        !self.signatures.is_empty()
    }

    /// FHX-4: only a `final` review is final (or amended, for a signed amendment); open,
    /// in-progress, awaiting-cosign and amending reviews are preliminary.
    pub fn report_status(&self) -> ReportStatus {
        match (self.state, self.amended) {
            (ReviewState::Final, false) => ReportStatus::Final,
            (ReviewState::Final, true) => ReportStatus::Amended,
            _ => ReportStatus::Preliminary,
        }
    }
}

/// First signature or second signature (SIGN-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureRole {
    Sign,
    Cosign,
}

/// One signature. Identity fields are `None` after de-identification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub role: SignatureRole,
    /// Console user id (FHX-6 `Practitioner.identifier`).
    pub user_id: Option<String>,
    /// NOTE-6.
    pub display_name: Option<String>,
    /// NOTE-6, e.g. `NMC 12345`.
    pub professional_registration: Option<String>,
    /// RFC 3339.
    pub signed_at: String,
}

/// A clinical note body (NOTE-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteText {
    /// RFC 3339.
    pub created_at: String,
    pub author_display_name: Option<String>,
    pub body: String,
}

/// Letter kind (NOTE-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LetterKind {
    Referral,
    Summary,
}

/// A letter's text (NOTE-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LetterText {
    pub kind: LetterKind,
    pub recipient: String,
    pub body: String,
    /// RFC 3339; `None` while draft.
    pub finalized_at: Option<String>,
}
