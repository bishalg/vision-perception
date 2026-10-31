//! owner: WP-C.9
//!
//! Clinician workspace contracts (CTR-1): the signed-content snapshot and its hash (SIGN-4),
//! and the request/response bodies of the worklist, clinical review and sign-off, MRN
//! patient references, clinical notes, letters and timeline (WP-C.10..C.12 build the routes;
//! the types live here so those packages never edit this file in parallel). Spec ids refer
//! to `docs/spec/modules/asd-clinician-and-interop/technical.md`.
//!
//! Conventions as `api_v1`: snake_case, UUID strings, RFC 3339 UTC timestamps. One
//! exception (RBAC-4): clinical content (impression, recommendations, note and letter
//! text, MRN) is **omitted**, not `null`, for a caller without `ClinicalContentRead` or
//! `PatientRefRead`; those fields are marked "absent without ...".

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api_v1::{MetricRow, Role, SessionAttentionFlag, SessionSummary};

// ---- limits and fixed text ---------------------------------------------------------------

/// SIGN-2: impression and recommendations, in characters.
pub const MAX_REVIEW_TEXT_CHARS: usize = 8_000;
/// NOTE-2: clinical note body, in characters.
pub const MAX_NOTE_CHARS: usize = 20_000;
/// SIGN-8: amendment reason, in characters.
pub const MIN_AMEND_REASON_CHARS: usize = 10;
pub const MAX_AMEND_REASON_CHARS: usize = 500;
/// NOTE-6: signer profile, in characters.
pub const MAX_DISPLAY_NAME_CHARS: usize = 120;
pub const MAX_REGISTRATION_CHARS: usize = 60;
/// NOTE-7: `settings.letterhead.address_lines`.
pub const MAX_LETTERHEAD_LINES: usize = 4;
pub const MAX_LETTERHEAD_LINE_CHARS: usize = 80;
/// NOTE-3: letter body and recipient, in characters.
pub const MAX_LETTER_BODY_CHARS: usize = 20_000;
pub const MAX_LETTER_RECIPIENT_CHARS: usize = 500;
/// MRN-3: assigning authority code and MRN, in characters (before normalisation).
pub const MAX_ASSIGNING_AUTHORITY_CHARS: usize = 40;
pub const MAX_MRN_CHARS: usize = 64;

/// SIGN-5: the meaning statement shown in the sign dialog and stored with each signature.
pub const SIGNATURE_MEANING: &str =
    "I have reviewed this observation record and approve this review.";

// ---- org setting keys (NOTE-7, MRN-2, CW-2, SIGN-6) --------------------------------------

/// `organizations.settings` keys owned by the clinician workspace. Nested keys are objects:
/// `{"clinical": {"worklist_scope": "all", "cosign_policy": "none"}}`.
pub mod settings {
    /// IANA time zone, default `UTC` (G11).
    pub const TIMEZONE: &str = "timezone";
    /// `a4 | letter`, default `a4` (G10).
    pub const PAPER: &str = "paper";
    /// Object `{address_lines: [..]}`.
    pub const LETTERHEAD: &str = "letterhead";
    pub const LETTERHEAD_ADDRESS_LINES: &str = "address_lines";
    /// Object `{patient_ref_mode}`.
    pub const IDENTITY: &str = "identity";
    pub const PATIENT_REF_MODE: &str = "patient_ref_mode";
    /// Object `{worklist_scope, cosign_policy}`.
    pub const CLINICAL: &str = "clinical";
    pub const WORKLIST_SCOPE: &str = "worklist_scope";
    pub const COSIGN_POLICY: &str = "cosign_policy";
    /// Default when unset.
    pub const DEFAULT_TIMEZONE: &str = "UTC";
}

// ---- enumerations ------------------------------------------------------------------------

macro_rules! str_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $s:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            /// The wire and database spelling.
            pub const fn as_str(self) -> &'static str {
                match self { $($name::$variant => $s),* }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some($name::$variant),)* _ => None }
            }
        }
    };
}

str_enum!(
    /// SIGN-1 `clinical_reviews.state`.
    ClinicalReviewState {
        Open => "open",
        InProgress => "in_progress",
        AwaitingCosign => "awaiting_cosign",
        Final => "final",
        Amending => "amending",
    }
);

str_enum!(
    /// SIGN-2: the clinician's statement. Never pre-filled from the attention flag.
    Disposition {
        NoConcernNoted => "no_concern_noted",
        Monitor => "monitor",
        ReferForEvaluation => "refer_for_evaluation",
        RecaptureRequested => "recapture_requested",
        NotInterpretable => "not_interpretable",
    }
);

str_enum!(
    /// `clinical_review_versions.status` (SIGN-7).
    ReviewVersionStatus {
        Draft => "draft",
        Signed => "signed",
        Superseded => "superseded",
    }
);

str_enum!(
    /// `clinical_signatures.role` (SIGN-6).
    SignatureRole {
        Sign => "sign",
        Cosign => "cosign",
    }
);

str_enum!(
    /// SIGN-6 `settings.clinical.cosign_policy`, default `none`.
    CosignPolicy {
        None => "none",
        RedFlag => "red_flag",
        Always => "always",
    }
);

str_enum!(
    /// CW-2 `settings.clinical.worklist_scope`; default `all` for clinic orgs, `flagged`
    /// otherwise.
    WorklistScope {
        All => "all",
        Flagged => "flagged",
    }
);

str_enum!(
    /// MRN-2 `settings.identity.patient_ref_mode`, default `none`.
    PatientRefMode {
        None => "none",
        Mrn => "mrn",
    }
);

str_enum!(
    /// NOTE-7 `settings.paper`, default `a4`.
    Paper {
        A4 => "a4",
        Letter => "letter",
    }
);

str_enum!(
    /// CW-1 `assigned` filter.
    WorklistAssigned {
        Me => "me",
        Unassigned => "unassigned",
        Any => "any",
    }
);

str_enum!(
    /// NOTE-1: a correction is a new note pointing at the corrected one.
    ClinicalNoteKind {
        General => "general",
        Correction => "correction",
    }
);

str_enum!(
    /// NOTE-3.
    LetterKind {
        Referral => "referral",
        Summary => "summary",
    }
);

str_enum!(
    /// NOTE-3: finalized letters are immutable.
    LetterStatus {
        Draft => "draft",
        Final => "final",
    }
);

str_enum!(
    /// CW-3 timeline entry kinds.
    TimelineEntryKind {
        Session => "session",
        ClinicalReview => "clinical_review",
        Note => "note",
        Letter => "letter",
        CareEvent => "care_event",
    }
);

// ---- signed content (SIGN-4) -------------------------------------------------------------

/// The stored session measurements shown on the review page (`SessionDetail` fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotMeasurements {
    #[schemars(extend("format" = "date-time"))]
    pub started_at: String,
    pub duration_sec: f64,
    pub quality_verdict: String,
    pub attention_flag: SessionAttentionFlag,
    pub composite_score: Option<f64>,
    pub composite_confidence: Option<f64>,
    pub valid_frame_ratio: Option<f64>,
    pub calibrated_distance_cm: Option<f64>,
    pub social_preference: Option<f64>,
    pub eye_dwell: Option<f64>,
    pub mouth_dwell: Option<f64>,
    pub first_eye_latency_ms: Option<f64>,
}

/// One clip of the snapshot: every stored `MetricRow`, in display order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotClip {
    pub clip_id: String,
    pub order: i64,
    pub rows: Vec<MetricRow>,
}

/// SIGN-4: exactly what the signer saw. `content_hash = SHA-256(JCS(snapshot))`. Every field
/// is serialised (absent values as `null`), so adding a field to this type changes every
/// hash: version it rather than editing it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClinicalReviewSnapshot {
    pub session_id: String,
    pub org_id: String,
    pub bundle_version: Option<String>,
    pub schema_version: Option<String>,
    pub measurements: SnapshotMeasurements,
    pub clips: Vec<SnapshotClip>,
    pub disposition: Option<Disposition>,
    pub impression: Option<String>,
    pub recommendations: Option<String>,
    pub version_no: u32,
}

/// NOTE-3: what a letter's finalizer signs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LetterSnapshot {
    pub letter_id: String,
    pub org_id: String,
    pub participant_id: String,
    pub kind: LetterKind,
    pub session_ids: Vec<String>,
    pub recipient: String,
    pub body: String,
    pub row_version: u32,
}

fn sha256_jcs<T: Serialize>(value: &T) -> [u8; 32] {
    // A struct of strings, numbers and enums always serialises.
    let value = serde_json::to_value(value).expect("snapshot serialises");
    Sha256::digest(crate::jcs::to_vec(&value)).into()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// SIGN-4 `SHA-256(JCS(snapshot))` (RFC 8785 via `jcs.rs`, never `serde_json::to_vec`).
pub fn content_hash(snapshot: &ClinicalReviewSnapshot) -> [u8; 32] {
    sha256_jcs(snapshot)
}

/// [`content_hash`] as 64 lower-case hex characters (the wire and audit form).
pub fn content_hash_hex(snapshot: &ClinicalReviewSnapshot) -> String {
    hex(&content_hash(snapshot))
}

/// NOTE-3 letter hash, same construction as [`content_hash`].
pub fn letter_content_hash(snapshot: &LetterSnapshot) -> [u8; 32] {
    sha256_jcs(snapshot)
}

/// [`letter_content_hash`] as lower-case hex.
pub fn letter_content_hash_hex(snapshot: &LetterSnapshot) -> String {
    hex(&letter_content_hash(snapshot))
}

// ---- signer profile (NOTE-6) -------------------------------------------------------------

/// `GET /v1/users/me`: the caller's account and signer profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UserProfile {
    pub id: String,
    /// Same value as `id`, named for the console (sign / cosign checks).
    #[serde(default)]
    pub user_id: Option<String>,
    /// True when the user has a TOTP secret (the secret is never returned): a sign or
    /// cosign then needs a `totp_code`.
    #[serde(default)]
    pub totp_enrolled: Option<bool>,
    pub email: String,
    pub role: Role,
    /// `null` only for `superadmin`.
    pub org_id: Option<String>,
    pub display_name: Option<String>,
    pub professional_registration: Option<String>,
}

/// `PATCH /v1/users/me`. Absent = unchanged, `""` = clear. Unknown fields are rejected
/// (role and status are never self-service).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateProfileRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub professional_registration: Option<String>,
}

// ---- worklist (CW-1, CW-2) ---------------------------------------------------------------

/// `GET /v1/orgs/{id}/worklist` item: the session plus its clinical review state. Never
/// carries impression or MRN (RBAC-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_worklist_item())]
pub struct WorklistItem {
    #[serde(flatten)]
    pub session: SessionSummary,
    /// `null` when no clinician has opened the review yet.
    pub review_id: Option<String>,
    pub clinical_state: Option<ClinicalReviewState>,
    pub assigned_to: Option<String>,
    /// Hours since the session was received.
    pub age_hours: f64,
    pub cosign_required: bool,
    /// Whether a hospital MRN is on file (MRN-6). Absent for a caller without
    /// `PatientRefRead` (superadmin); the MRN itself is never in this type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_patient_ref: Option<bool>,
}

fn example_worklist_item() -> WorklistItem {
    WorklistItem {
        session: crate::api_v1::example_session_summary(),
        review_id: None,
        clinical_state: None,
        assigned_to: None,
        age_hours: 5.5,
        cosign_required: false,
        has_patient_ref: Some(false),
    }
}

/// `GET /v1/orgs/{id}/worklist`, cursor over `(received_at, id)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorklistPage {
    pub items: Vec<WorklistItem>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

// ---- clinical review and sign-off (SIGN-1..10) -------------------------------------------

/// One signature (SIGN-5, SIGN-6). The name and registration are a snapshot taken at
/// signing (signature manifestation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClinicalSignature {
    pub id: String,
    pub version_no: u32,
    pub role: SignatureRole,
    pub signer_id: String,
    pub signer_display_name: String,
    pub signer_registration: Option<String>,
    pub meaning: String,
    /// Hex SHA-256 of the signed snapshot.
    pub content_hash: String,
    #[schemars(extend("format" = "date-time"))]
    pub signed_at: String,
}

/// One version of a review's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClinicalReviewVersion {
    pub version_no: u32,
    pub status: ReviewVersionStatus,
    pub disposition: Option<Disposition>,
    /// Absent without `ClinicalContentRead`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impression: Option<String>,
    /// Absent without `ClinicalContentRead`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendations: Option<String>,
    /// SIGN-8 reason of an amendment version; absent without `ClinicalContentRead`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amend_reason: Option<String>,
    pub created_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    pub signed_at: Option<String>,
}

/// `GET /v1/clinical-reviews/{id}` and the result of every review mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClinicalReview {
    pub id: String,
    pub org_id: String,
    pub session_id: String,
    pub state: ClinicalReviewState,
    pub assigned_to: Option<String>,
    pub cosign_required: bool,
    /// Send as `If-Match: "<row_version>"` on `PATCH` (SIGN-3).
    pub row_version: u32,
    /// The version being edited or the latest signed one.
    pub current: ClinicalReviewVersion,
    /// Hex SHA-256 of the current version's snapshot; sign and cosign must echo it
    /// (SIGN-4).
    pub content_hash: String,
    pub signatures: Vec<ClinicalSignature>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
}

/// `POST /v1/sessions/{id}/clinical-review` (claim) body; empty object allowed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimClinicalReviewRequest {
    /// Assign to this user of the org instead of the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
}

/// `PATCH /v1/clinical-reviews/{id}` (draft save, SIGN-2, SIGN-3). Absent = unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateClinicalReviewRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<Disposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendations: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
}

/// Step-up signing body (SIGN-5), used by sign, cosign and letter finalize. The password
/// is verified and dropped; it is never stored or logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignRequest {
    /// The `content_hash` the client displayed.
    pub content_hash: String,
    pub password: String,
    /// Required when the user has TOTP enrolled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totp_code: Option<String>,
    /// Must be `true`: the user accepted [`SIGNATURE_MEANING`].
    pub meaning_ack: bool,
}

/// `POST /v1/clinical-reviews/{id}/amend` (SIGN-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AmendClinicalReviewRequest {
    /// 10..500 characters.
    pub reason: String,
}

// ---- patient reference (MRN-1..6) --------------------------------------------------------

/// `GET /v1/participants/{id}/patient-ref` (`PatientRefRead`; every read is audited).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatientRef {
    pub participant_id: String,
    pub assigning_authority: String,
    pub mrn: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
}

/// `PUT /v1/participants/{id}/patient-ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PutPatientRefRequest {
    pub assigning_authority: String,
    pub mrn: String,
}

/// `POST /v1/orgs/{id}/patient-refs/search`: the MRN travels in the body only (MRN-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatientRefSearchRequest {
    pub assigning_authority: String,
    pub mrn: String,
}

/// Search result: the matching participant, or `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatientRefSearchResponse {
    pub participant_id: Option<String>,
}

// ---- clinical notes (NOTE-1, NOTE-2) -----------------------------------------------------

/// A clinical note. Append-only: a correction is a new note with `corrects_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClinicalNote {
    pub id: String,
    pub participant_id: String,
    pub session_id: Option<String>,
    pub kind: ClinicalNoteKind,
    /// Absent without `ClinicalContentRead`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub corrects_id: Option<String>,
    /// The id of the note that corrects this one, if any (shown struck through).
    pub corrected_by: Option<String>,
    pub author_id: String,
    pub author_display_name: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
}

/// `POST /v1/participants/{id}/notes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateClinicalNoteRequest {
    /// 1..20 000 characters.
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrects_id: Option<String>,
}

/// `GET /v1/participants/{id}/notes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClinicalNotePage {
    pub items: Vec<ClinicalNote>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

// ---- letters (NOTE-3, NOTE-4) ------------------------------------------------------------

/// A referral or summary letter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReferralLetter {
    pub id: String,
    pub org_id: String,
    pub participant_id: String,
    pub kind: LetterKind,
    pub status: LetterStatus,
    pub session_ids: Vec<String>,
    /// Absent without `ClinicalContentRead`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Absent without `ClinicalContentRead`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// `If-Match` value for `PATCH`.
    pub row_version: u32,
    /// Hex SHA-256 of the letter snapshot; finalize must echo it.
    pub content_hash: String,
    pub author_id: String,
    pub finalized_by: Option<String>,
    pub finalized_at: Option<String>,
    pub signer_display_name: Option<String>,
    pub signer_registration: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
}

/// `POST /v1/participants/{id}/letters`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateLetterRequest {
    pub kind: LetterKind,
    #[serde(default)]
    pub session_ids: Vec<String>,
    pub recipient: String,
    pub body: String,
}

/// `PATCH /v1/letters/{id}` while draft (`If-Match`). Absent = unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateLetterRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<LetterKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// `GET /v1/participants/{id}/letters`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LetterPage {
    pub items: Vec<ReferralLetter>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

// ---- timeline (CW-3) ---------------------------------------------------------------------

/// One timeline entry: headers only, never a note or letter body (CW-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEntry {
    pub kind: TimelineEntryKind,
    /// Id of the underlying row (session id, review id, note id, letter id, care event id).
    pub id: String,
    #[schemars(extend("format" = "date-time"))]
    pub at: String,
    pub session_id: Option<String>,
    pub actor_id: Option<String>,
    pub actor_display_name: Option<String>,
    /// Kind-specific header: e.g. `{"attention_flag": "red"}`, `{"state": "final",
    /// "version_no": 2}`, `{"note_kind": "general"}`, `{"letter_kind": "referral",
    /// "status": "final"}`, `{"care_kind": "diary"}`. Never free text.
    pub summary: serde_json::Map<String, serde_json::Value>,
}

/// `GET /v1/participants/{id}/timeline`, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TimelinePage {
    pub items: Vec<TimelineEntry>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_spellings_match_serde() {
        fn check<T: Serialize + Copy>(all: &[T], as_str: fn(T) -> &'static str) {
            for v in all {
                assert_eq!(
                    serde_json::to_value(v).unwrap(),
                    serde_json::Value::from(as_str(*v))
                );
            }
        }
        check(ClinicalReviewState::ALL, ClinicalReviewState::as_str);
        check(Disposition::ALL, Disposition::as_str);
        check(ReviewVersionStatus::ALL, ReviewVersionStatus::as_str);
        check(SignatureRole::ALL, SignatureRole::as_str);
        check(CosignPolicy::ALL, CosignPolicy::as_str);
        check(WorklistScope::ALL, WorklistScope::as_str);
        check(PatientRefMode::ALL, PatientRefMode::as_str);
        check(Paper::ALL, Paper::as_str);
        check(WorklistAssigned::ALL, WorklistAssigned::as_str);
        check(ClinicalNoteKind::ALL, ClinicalNoteKind::as_str);
        check(LetterKind::ALL, LetterKind::as_str);
        check(LetterStatus::ALL, LetterStatus::as_str);
        check(TimelineEntryKind::ALL, TimelineEntryKind::as_str);
        assert_eq!(Disposition::parse("monitor"), Some(Disposition::Monitor));
        assert_eq!(Disposition::parse("Monitor"), None);
    }

    #[test]
    fn content_absent_not_null() {
        let v = ClinicalReviewVersion {
            version_no: 1,
            status: ReviewVersionStatus::Draft,
            disposition: None,
            impression: None,
            recommendations: None,
            amend_reason: None,
            created_by: None,
            created_at: "2026-10-31T00:00:00.000Z".into(),
            signed_at: None,
        };
        let json = serde_json::to_value(v).unwrap();
        assert!(json.get("impression").is_none());
        assert!(json.get("recommendations").is_none());
        assert!(json["disposition"].is_null());
    }

    #[test]
    fn profile_patch_refuses_role() {
        assert!(serde_json::from_str::<UpdateProfileRequest>(r#"{"role":"owner"}"#).is_err());
        assert!(serde_json::from_str::<UpdateProfileRequest>(r#"{"display_name":"A"}"#).is_ok());
    }
}
