//! owner: WP-R.7
//!
//! EMR-lite record and share-a-case contracts (REC-9): encounters (REC-3, REC-4), follow-ups,
//! attachments (REC-6, REC-7), participant create and search (REC-1, REC-2), the wave-3
//! identity type (frozen now, stored by no route yet), case sharing (SHR-7) and share links
//! (SHR-8, SHR-9), plus the participant QR payload (REC-1, REC-12). Spec ids refer to
//! `docs/spec/modules/asd-emr-lite-and-sharing/technical.md`.
//!
//! Conventions as `api_v1` and `clinical`: snake_case, UUID strings, RFC 3339 UTC
//! timestamps. Every field an older console or server does not send is `#[serde(default)]`.
//! Clinical text (the encounter summary) is **omitted**, not `null`, for a caller without
//! `ClinicalContentRead` (RBAC-4); `has_summary` says whether one exists.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_v1::Participant;

// ---- limits and fixed text ---------------------------------------------------------------

/// REC-3: `encounters.reason`, in characters (a short category-like text, kept in clear).
pub const MAX_ENCOUNTER_REASON_CHARS: usize = 200;
/// REC-3: `encounters.summary` (the clinician's text, encrypted at rest), in characters.
pub const MAX_ENCOUNTER_SUMMARY_CHARS: usize = 8_000;
/// REC-6: attachment size, in bytes (10 MiB).
pub const MAX_ATTACHMENT_BYTES: u64 = 10_485_760;
/// REC-6: attachment file name, in characters after sanitising.
pub const MAX_ATTACHMENT_FILE_NAME_CHARS: usize = 120;
/// SHR-7: `CaseShareRequest.recipient`, in characters.
pub const MIN_CASE_RECIPIENT_CHARS: usize = 10;
pub const MAX_CASE_RECIPIENT_CHARS: usize = 200;
/// SHR-7: `CaseShareRequest.consent_reference`, in characters.
pub const MAX_CONSENT_REFERENCE_CHARS: usize = 200;
/// SHR-8: a share link expires at most this many days after creation.
pub const SHARE_LINK_MAX_DAYS: u32 = 7;
/// SHR-8: `CreateShareLinkRequest.expires_in_days` when absent (72 h, ADR-0011 owner
/// decision 5's recommended default).
pub const SHARE_LINK_DEFAULT_DAYS: u32 = 3;
/// SHR-8: `max_downloads` range.
pub const SHARE_LINK_MIN_DOWNLOADS: i32 = 1;
pub const SHARE_LINK_MAX_DOWNLOADS: i32 = 5;
/// SHR-8: `CreateShareLinkRequest.max_downloads` when absent (ADR-0011 owner decision 5's
/// recommended default).
pub const SHARE_LINK_DEFAULT_DOWNLOADS: i32 = 1;
/// SHR-8: passcode length and alphabet `[A-HJ-NP-Z2-9]` (no `I`, `O`, `0`, `1`).
pub const SHARE_PASSCODE_LEN: usize = 12;
pub const SHARE_PASSCODE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// REC-1: server-minted participant tokens are `child-` + 20 lowercase hex digits.
pub const PARTICIPANT_TOKEN_PREFIX: &str = "child-";
pub const PARTICIPANT_TOKEN_HEX_LEN: usize = 20;
/// REC-1, REC-12: QR payload `asd://participant/<token>?org=<org_id>`.
pub const QR_SCHEME: &str = "asd";
pub const QR_HOST: &str = "participant";

macro_rules! str_enum {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident => $s:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        pub enum $name { $($(#[$vdoc])* #[serde(rename = $s)] $variant),* }

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

// ---- encounters (REC-3, REC-4, REC-5) ----------------------------------------------------

str_enum!(
    /// REC-3: `encounters.kind`.
    EncounterKind {
        ScreeningVisit => "screening_visit",
        FollowUpVisit => "follow_up_visit",
        PhoneCall => "phone_call",
        HomeVisit => "home_visit",
        Other => "other",
    }
);

str_enum!(
    /// REC-3: `encounters.status`. A `closed` encounter accepts only `follow_up_done_at`;
    /// `owner` or `clinician` may reopen it (REC-4, `409 encounter-closed` otherwise).
    EncounterStatus {
        Open => "open",
        Closed => "closed",
    }
);

/// REC-4: one encounter. `kind`, `reason` and `summary` are staff input, never pre-filled
/// from the attention flag or a score (REC-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Encounter {
    pub id: String,
    pub participant_id: String,
    pub kind: EncounterKind,
    #[schemars(extend("format" = "date-time"))]
    pub occurred_at: String,
    #[serde(default)]
    pub clinician_id: Option<String>,
    /// REC-5: the clinician's display name, for the timeline header.
    #[serde(default)]
    pub clinician_display_name: Option<String>,
    /// ≤ 200 characters, clear text (REC-3).
    #[serde(default)]
    pub reason: Option<String>,
    /// ≤ 8 000 characters. Absent without `ClinicalContentRead` (REC-4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Whether a summary exists, for callers that may not read it (REC-4).
    pub has_summary: bool,
    pub status: EncounterStatus,
    #[serde(default)]
    #[schemars(extend("format" = "date-time"))]
    pub follow_up_due_at: Option<String>,
    #[serde(default)]
    #[schemars(extend("format" = "date-time"))]
    pub follow_up_done_at: Option<String>,
    /// Linked sessions of the same participant (REC-4).
    #[serde(default)]
    pub session_ids: Vec<String>,
    /// `If-Match: "<row_version>"` on `PATCH` (REC-4, SIGN-3).
    pub row_version: i32,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[serde(default)]
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: Option<String>,
}

/// `POST /v1/participants/{id}/encounters` (REC-4). `summary` needs `ClinicalNoteWrite`,
/// else `400 encounter-summary-forbidden`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateEncounterRequest {
    pub kind: EncounterKind,
    #[schemars(extend("format" = "date-time"))]
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clinician_id: Option<String>,
    /// ≤ 200 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// ≤ 8 000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub follow_up_due_at: Option<String>,
    /// Sessions of the same participant to link on create.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_ids: Vec<String>,
}

/// `PATCH /v1/encounters/{id}` with `If-Match: "<row_version>"` (REC-4). Absent fields
/// stay unchanged. A `closed` encounter accepts only `follow_up_done_at`, and `status:
/// open` from `owner` or `clinician`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateEncounterRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EncounterKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub occurred_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clinician_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Needs `ClinicalNoteWrite` (REC-4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<EncounterStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub follow_up_due_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub follow_up_done_at: Option<String>,
}

/// `GET /v1/participants/{id}/encounters` (REC-4), newest `occurred_at` first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EncounterPage {
    pub items: Vec<Encounter>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
}

/// `POST /v1/encounters/{id}/sessions` (REC-4): link a session of the same participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkSessionRequest {
    pub session_id: String,
}

/// One open follow-up (REC-4: `GET /v1/orgs/{id}/follow-ups?due_before=&assigned=`): an
/// encounter with `follow_up_due_at` set and `follow_up_done_at` unset. No summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FollowUp {
    pub encounter_id: String,
    pub participant_id: String,
    pub participant_token: String,
    pub kind: EncounterKind,
    #[schemars(extend("format" = "date-time"))]
    pub occurred_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub follow_up_due_at: String,
    #[serde(default)]
    pub clinician_id: Option<String>,
    #[serde(default)]
    pub clinician_display_name: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// `GET /v1/orgs/{id}/follow-ups`, earliest due first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FollowUpPage {
    pub items: Vec<FollowUp>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
}

// ---- attachments (REC-6, REC-7) ----------------------------------------------------------

str_enum!(
    /// REC-6: `attachments.kind`.
    AttachmentKind {
        PaperForm => "paper_form",
        Referral => "referral",
        Report => "report",
        PhotoOfDocument => "photo_of_document",
        Other => "other",
    }
);

str_enum!(
    /// REC-6: the only accepted media types; anything else is `400
    /// attachment-type-not-allowed`.
    AttachmentMediaType {
        ApplicationPdf => "application/pdf",
        ImageJpeg => "image/jpeg",
        ImagePng => "image/png",
    }
);

/// One attachment's metadata (REC-6). The bytes are read with `GET /v1/attachments/{id}`
/// (`ClinicalContentRead`, REC-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attachment {
    pub id: String,
    pub participant_id: String,
    #[serde(default)]
    pub encounter_id: Option<String>,
    pub kind: AttachmentKind,
    /// Sanitised to `[A-Za-z0-9._-]`, ≤ 120 characters (`sanitize_file_name`).
    pub file_name: String,
    pub media_type: AttachmentMediaType,
    /// 1..10 485 760.
    pub byte_size: u64,
    /// Hex SHA-256 of the plaintext; `null` until the content is uploaded (REC-7).
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub uploaded_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
}

/// `POST /v1/participants/{id}/attachments` (REC-7, `AttachmentWrite`): metadata first,
/// then the bytes with `PUT` to `AttachmentUpload.upload.url`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateAttachmentRequest {
    pub kind: AttachmentKind,
    /// Sanitised by the server; ≤ 120 characters after sanitising.
    pub file_name: String,
    pub media_type: AttachmentMediaType,
    /// 1..10 485 760, else `400 attachment-too-large`.
    pub byte_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter_id: Option<String>,
}

/// Where to send the bytes (REC-7). Proxy mode only: `PUT /v1/attachments/{id}/content`
/// under the user bearer, with an optional `Content-Digest: sha-256=:<base64>:` (RFC 9530;
/// `400 digest-mismatch`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UploadTarget {
    /// Always `PUT`.
    pub method: String,
    pub url: String,
}

/// `201` answer to `POST /v1/participants/{id}/attachments` (REC-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentUpload {
    pub attachment: Attachment,
    pub upload: UploadTarget,
}

// ---- participants: create and search (REC-1, REC-2) --------------------------------------

/// `POST /v1/orgs/{id}/participants/search` (REC-2, `ParticipantRead`). Every filter is
/// optional; `mrn` needs `PatientRefRead` and is sent in the body only (MRN-4). Identity
/// fields are not searchable before wave 3.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParticipantSearchRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mrn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_bracket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub last_seen_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub last_seen_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_open_follow_up: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// One search result (REC-2): the `Participant` keys plus the record counters. REC-2's
/// `sessions_n` and `last_session_at` are `Participant.session_count` and
/// `Participant.last_session_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParticipantSearchRow {
    #[serde(flatten)]
    pub participant: Participant,
    #[serde(default)]
    pub open_encounters_n: u32,
    #[serde(default)]
    #[schemars(extend("format" = "date-time"))]
    pub next_follow_up_at: Option<String>,
}

/// `POST /v1/orgs/{id}/participants/search` answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParticipantSearchPage {
    pub items: Vec<ParticipantSearchRow>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
}

/// `POST /v1/orgs/{id}/participants` (REC-1, `ParticipantCreate`). The token is minted by
/// the server (`participant_token_from_uuid`), never supplied; `mode` is always `clinic`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateParticipantRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_bracket_at_enrol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The consent the clinic recorded on paper or in the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub consent_at: Option<String>,
}

/// `201` answer to `POST /v1/orgs/{id}/participants` (REC-1): the `Participant` keys plus
/// `qr_payload` (`asd://participant/<token>?org=<org_id>`, see `qr_payload`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreatedParticipant {
    #[serde(flatten)]
    pub participant: Participant,
    pub qr_payload: String,
}

/// Wave-3 participant identity (REC-9). Type only: no route stores or returns it before
/// wave 3, and `identity-disabled` answers any attempt. The keys are frozen now; do not
/// rename them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParticipantIdentity {
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
    /// `female | male | other | unknown`.
    #[serde(default)]
    pub sex: Option<String>,
    /// `YYYY-MM`.
    #[serde(default)]
    pub birth_year_month: Option<String>,
    #[serde(default)]
    pub guardian_phone: Option<String>,
}

// ---- share a case (SHR-7, SHR-8, SHR-9) --------------------------------------------------

/// `ExportRequest.case` for `format: case_package` (SHR-7): one participant per package.
/// Identified mode needs `consent_reference` and `consent_recorded_at`, else `400
/// case-consent-missing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CaseShareRequest {
    pub participant_id: String,
    /// Sessions to include; empty means all of the participant's sessions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_ids: Vec<String>,
    /// 10..200 characters; encrypted at rest.
    pub recipient: String,
    /// ≤ 200 characters, clear (an identifier such as a consent form number).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "date-time"))]
    pub consent_recorded_at: Option<String>,
    /// Identified mode only.
    #[serde(default)]
    pub include_attachments: bool,
}

/// One share link (SHR-8), as listed. Never carries the passcode (see `CreatedShareLink`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShareLink {
    pub id: String,
    pub export_id: String,
    /// The public console page `/share/<id>`.
    pub url: String,
    /// ≤ `created_at` + 7 days.
    #[schemars(extend("format" = "date-time"))]
    pub expires_at: String,
    /// 1..5.
    pub max_downloads: i32,
    #[serde(default)]
    pub downloads: i32,
    #[serde(default)]
    pub opens: i32,
    #[serde(default)]
    #[schemars(extend("format" = "date-time"))]
    pub revoked_at: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[serde(default)]
    #[schemars(extend("format" = "date-time"))]
    pub last_opened_at: Option<String>,
}

/// `POST /v1/exports/{id}/share-link` (SHR-8, `ShareLinkManage`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateShareLinkRequest {
    /// 1..7; default 3 (`SHARE_LINK_DEFAULT_DAYS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_days: Option<u32>,
    /// 1..5; default 1 (`SHARE_LINK_DEFAULT_DOWNLOADS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_downloads: Option<i32>,
}

/// `201` answer to `POST /v1/exports/{id}/share-link` (SHR-8). The passcode is returned
/// this once and never stored or returned again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreatedShareLink {
    pub link: ShareLink,
    /// 12 characters from `[A-HJ-NP-Z2-9]`.
    pub passcode: String,
}

/// `GET /v1/exports/{id}/share-links`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShareLinkPage {
    pub items: Vec<ShareLink>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// `POST /v1/share/{link_id}/open` (SHR-9, public). Wrong passcode: `401
/// share-passcode-invalid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShareLinkOpenRequest {
    pub passcode: String,
}

/// `200` answer to `POST /v1/share/{link_id}/open` (SHR-9). `download_token` is used once
/// as `GET /v1/share/{link_id}/file?t=<download_token>` within 5 minutes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShareLinkOpenResponse {
    pub export_id: String,
    pub file_name: String,
    pub bytes: u64,
    #[schemars(extend("format" = "date-time"))]
    pub expires_at: String,
    pub downloads_left: i32,
    pub download_token: String,
}

// ---- participant token and QR payload (REC-1, REC-12) ------------------------------------

/// REC-1: the server-minted token for a fresh v4 UUID: `child-` + its first 20 hex digits
/// (lowercase, no dashes).
pub fn participant_token_from_uuid(id: Uuid) -> String {
    let simple = id.simple().to_string();
    format!(
        "{PARTICIPANT_TOKEN_PREFIX}{}",
        &simple[..PARTICIPANT_TOKEN_HEX_LEN]
    )
}

/// REC-12: whether `token` matches `^child-[0-9a-f]{20}$`.
pub fn is_participant_token(token: &str) -> bool {
    token
        .strip_prefix(PARTICIPANT_TOKEN_PREFIX)
        .is_some_and(|hex| {
            hex.len() == PARTICIPANT_TOKEN_HEX_LEN
                && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

/// REC-1: the QR payload the console prints, `asd://participant/<token>?org=<org_id>`
/// (org as a lowercase hyphenated UUID).
pub fn qr_payload(org: Uuid, token: &str) -> String {
    format!("{QR_SCHEME}://{QR_HOST}/{token}?org={}", org.hyphenated())
}

/// Why a scanned QR payload was refused (REC-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QrParseError {
    #[error("not an asd:// payload")]
    Scheme,
    #[error("not an asd://participant/ payload")]
    Host,
    #[error("participant token does not match ^child-[0-9a-f]{{20}}$")]
    Token,
    #[error("no org parameter")]
    MissingOrg,
    #[error("more than one org parameter")]
    DuplicateOrg,
    #[error("org is not a hyphenated UUID")]
    Org,
    #[error("unexpected fragment")]
    Fragment,
}

/// REC-12: parses a scanned payload into `(org, token)`. Scheme and host compare
/// case-insensitively (RFC 3986); the token must match `^child-[0-9a-f]{20}$`; `org` must
/// appear exactly once as a hyphenated UUID; other query parameters are ignored. Whether
/// `org` is the device's org is the caller's check (the scan is refused on mismatch).
pub fn parse_qr_payload(payload: &str) -> Result<(Uuid, String), QrParseError> {
    let (scheme, rest) = payload.split_once("://").ok_or(QrParseError::Scheme)?;
    if !scheme.eq_ignore_ascii_case(QR_SCHEME) {
        return Err(QrParseError::Scheme);
    }
    if rest.contains('#') {
        return Err(QrParseError::Fragment);
    }
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (host, token) = path.split_once('/').ok_or(QrParseError::Host)?;
    if !host.eq_ignore_ascii_case(QR_HOST) {
        return Err(QrParseError::Host);
    }
    if !is_participant_token(token) {
        return Err(QrParseError::Token);
    }
    let mut org = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key != "org" {
            continue;
        }
        if org.is_some() {
            return Err(QrParseError::DuplicateOrg);
        }
        org = Some(value);
    }
    let org = org.ok_or(QrParseError::MissingOrg)?;
    // Hyphenated form only (36 characters); `Uuid::parse_str` also takes simple and braced.
    if org.len() != 36 {
        return Err(QrParseError::Org);
    }
    let org = Uuid::parse_str(org).map_err(|_| QrParseError::Org)?;
    Ok((org, token.to_string()))
}

/// REC-6: the stored attachment file name: every character outside `[A-Za-z0-9._-]`
/// becomes `_`, leading dots are dropped (no hidden or `..` names), and the result is cut
/// to 120 characters. An empty result becomes `file`.
pub fn sanitize_file_name(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed: String = mapped
        .trim_start_matches('.')
        .chars()
        .take(MAX_ATTACHMENT_FILE_NAME_CHARS)
        .collect();
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed
    }
}
