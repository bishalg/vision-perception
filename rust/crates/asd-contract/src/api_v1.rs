//! owner: WP-R.3
//!
//! `/v1` request and response bodies (API-1, API-2, API-3) other than the session ingest
//! body (`session_v1_6`) and the config bundle (`config`). snake_case throughout, as the
//! console (`apps/curv_console/src/lib/api/types.ts`) and the iOS registration client read
//! them. Exported to `docs/spec/schemas/asd/api-v1.json`; types carrying an `examples`
//! entry are checked by the console contract test (WP-C.8).
//!
//! Identifiers are UUID strings, timestamps RFC 3339 UTC strings. Optional response fields
//! are written as `null`, not omitted, unless noted.

use std::collections::BTreeMap;

use asd_core::types::tracking::TrackingVerdict;
use asd_core::types::{CaptureSource, ExpectationStatus, NormativeDirection};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mirror;
pub use crate::tracking_api::SessionTrackingQuality;

// ---- roles and auth (AUTH-1..4) --------------------------------------------------------

/// RBAC role: org-scoped roles plus the global `superadmin` (AUTH-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Clinician,
    Researcher,
    Reviewer,
    Viewer,
    Superadmin,
}

/// The signed-in user (`TokenPair.user`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_user())]
pub struct User {
    pub id: String,
    pub email: String,
    pub role: Role,
    /// `null` only for the global `superadmin`, who belongs to no org (AUTH-4).
    pub org_id: Option<String>,
}

/// `POST /v1/auth/login`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// `POST /v1/auth/refresh` and `POST /v1/auth/logout` (`{refresh_token}` in the body).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Login and refresh response (AUTH-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_token_pair())]
pub struct TokenPair {
    pub access_token: String,
    /// Opaque, rotated on every refresh.
    pub refresh_token: String,
    /// Access-token lifetime in seconds (900).
    #[schemars(range(min = 1))]
    pub expires_in: u32,
    pub user: User,
}

// ---- devices (CON-10, CON-11, AUTH-3) --------------------------------------------------

/// `POST /v1/devices/register` body as the iOS app sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegisterDeviceRequest {
    pub enrol_code: String,
    /// `iOS`, `Android`.
    pub platform: String,
    pub app_version: String,
    /// Sent on re-registration: the server rotates this device's token without consuming
    /// a use of the code (CON-11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
}

/// Whether a registration created a device or rotated an existing device's token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    Registered,
    Rotated,
}

/// `POST /v1/devices/register` response (CON-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_register_response())]
pub struct RegisterDeviceResponse {
    pub status: RegistrationStatus,
    pub device_id: String,
    /// `dvc_` + 32 random bytes base64url; shown once, stored hashed.
    pub device_token: String,
    pub org_id: String,
    /// Latest bundle for the org, `null` when none is published.
    pub config_bundle_version: Option<String>,
}

/// `POST /v1/orgs/{id}/devices/enrol-code`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnrolCodeRequest {
    /// Default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub max_uses: Option<u32>,
    /// Default 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub expires_in_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The new enrol code; the plain code is returned only here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_enrol_code())]
pub struct EnrolCodeResponse {
    pub id: String,
    pub code: String,
    pub max_uses: u32,
    #[schemars(extend("format" = "date-time"))]
    pub expires_at: String,
}

/// `GET /v1/orgs/{id}/devices` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Device {
    pub id: String,
    pub org_id: String,
    pub platform: String,
    pub app_version: String,
    pub label: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub enrolled_at: String,
    pub revoked_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub config_bundle_version: Option<String>,
}

// ---- orgs, users, participants ---------------------------------------------------------

/// Organisation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgKind {
    Home,
    Clinic,
    Research,
}

/// `GET /v1/orgs/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_org())]
pub struct Org {
    pub id: String,
    pub name: String,
    pub kind: OrgKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_months: Option<u32>,
    /// `organizations.settings` (non-secret keys only: secret-looking keys are refused on
    /// write and filtered here). Present on `GET /v1/orgs/{id}`; absent in `GET /v1/orgs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, Value>>,
}

/// `PATCH /v1/orgs/{id}`; absent members are unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OrgPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_months: Option<u32>,
    /// Merged into `organizations.settings` (e.g. `audit_rate`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, Value>>,
}

/// User account status. `invited` accounts exist but cannot sign in yet; only `active`
/// users can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Disabled,
    Invited,
}

/// `GET /v1/orgs/{id}/users` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrgUser {
    pub id: String,
    pub email: String,
    pub role: Role,
    pub org_id: String,
    pub status: UserStatus,
    pub last_login_at: Option<String>,
    /// Signer profile (NOTE-6); staff data, stored in clear.
    #[serde(default)]
    pub display_name: Option<String>,
    /// e.g. `NMC 12345` (NOTE-6).
    #[serde(default)]
    pub professional_registration: Option<String>,
}

/// `POST /v1/orgs/{id}/users`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateUserRequest {
    pub email: String,
    pub role: Role,
    pub password: String,
    /// Up to 120 characters (NOTE-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Up to 60 characters (NOTE-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub professional_registration: Option<String>,
}

/// `PATCH /v1/orgs/{id}/users/{user_id}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateUserRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<UserStatus>,
    /// Absent = unchanged, `""` = clear; up to 120 characters (NOTE-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Absent = unchanged, `""` = clear; up to 60 characters (NOTE-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub professional_registration: Option<String>,
}

/// `GET /v1/orgs/{id}/participants` item. `id` is `participants.id` (CON-20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Participant {
    pub id: String,
    pub org_id: String,
    pub participant_token: String,
    pub age_bracket_at_enrol: Option<String>,
    pub language: Option<String>,
    pub session_count: u32,
    pub last_session_at: Option<String>,
    pub withdrawn_at: Option<String>,
    /// Whether a hospital MRN is on file (MRN-5/6). Absent for a caller without
    /// `PatientRefRead`; the MRN itself is never in this type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_patient_ref: Option<bool>,
}

// ---- sessions (CON-15, CON-16, CON-19, CON-20) -----------------------------------------

/// Backend-derived flag: the device enum plus `unreliable` (CON-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionAttentionFlag {
    Green,
    Yellow,
    Red,
    Unreliable,
}

/// `GET /v1/orgs/{id}/sessions` item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_session_summary(), example = example_session_summary_v1_7())]
pub struct SessionSummary {
    pub id: String,
    /// `participants.id` (UUID, CON-20).
    pub participant_id: String,
    /// The pseudonymous token the device sent (CON-20).
    pub participant_token: String,
    #[schemars(extend("format" = "date-time"))]
    pub started_at: String,
    pub duration_sec: f64,
    /// Catalog protocol id, e.g. `express_30s` (CON-16).
    #[serde(default)]
    pub battery: Option<String>,
    #[serde(default)]
    #[schemars(with = "Option<mirror::CaptureSource>")]
    pub capture_source: Option<CaptureSource>,
    pub quality_verdict: String,
    pub attention_flag: SessionAttentionFlag,
    #[serde(default)]
    pub composite_score: Option<f64>,
    /// `1 - share of low-confidence clips` (ING-2).
    #[serde(default)]
    pub composite_confidence: Option<f64>,
    #[serde(default)]
    pub bundle_version: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub reviewed: Option<bool>,
    /// QS-4 tracking verdict of a schema 1.7 session; `null` for a 1.6 session or when the
    /// device sent no tracking-quality record (SCH-4).
    #[serde(default)]
    #[schemars(with = "Option<mirror::TrackingVerdict>")]
    pub tracking_verdict: Option<TrackingVerdict>,
    /// QS-2 estimated gaze accuracy, degrees; `null` when not measured (SCH-4).
    #[serde(default)]
    pub tracking_accuracy_deg: Option<f64>,
}

/// `GET /v1/sessions/{id}`: the summary plus stored measurements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_session_detail(), example = example_session_detail_v1_7())]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub org_id: String,
    #[serde(default)]
    pub age_pack: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub schema_version: Option<String>,
    #[serde(default)]
    pub valid_frame_ratio: Option<f64>,
    #[serde(default)]
    pub calibrated_distance_cm: Option<f64>,
    #[serde(default)]
    pub social_preference: Option<f64>,
    #[serde(default)]
    pub eye_dwell: Option<f64>,
    #[serde(default)]
    pub mouth_dwell: Option<f64>,
    #[serde(default)]
    pub first_eye_latency_ms: Option<f64>,
    #[serde(default)]
    pub affect: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub report_pdf_available: Option<bool>,
    /// The session's tracking-quality record (QS-1, snake_case); `null` for a 1.6 session
    /// (app version before tracking quality) or without a record (SCH-4).
    #[serde(default)]
    pub tracking_quality: Option<SessionTrackingQuality>,
}

/// One scored expected-vs-recorded cell of a clip (CON-19).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_metric_row())]
pub struct MetricRow {
    pub segment_id: String,
    /// Biomarker key, e.g. `eye_dwell_ratio`.
    pub metric: String,
    pub target_aoi: Option<String>,
    #[schemars(with = "mirror::NormativeDirection")]
    pub direction: NormativeDirection,
    pub expected: Option<f64>,
    pub acceptable_low: Option<f64>,
    pub acceptable_high: Option<f64>,
    pub recorded: f64,
    /// `recorded - expected`; `null` when there is no expected value.
    pub deviation: Option<f64>,
    #[schemars(with = "mirror::ExpectationStatus")]
    pub status: ExpectationStatus,
    pub score: f64,
    pub imputed: bool,
    pub source: String,
}

/// `GET /v1/sessions/{id}/clips` item: the stored per-clip row plus its metric rows. The
/// flat `metric/expected/recorded/deviation/baseline_ref` columns come from the worst-status
/// row (ties: largest |deviation|), all `null` for a clip without rows (CON-19).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_clip_score())]
pub struct ClipScore {
    pub clip_id: String,
    pub order: i64,
    /// Worst row status; `null` for a clip with no scored rows.
    #[serde(default)]
    #[schemars(with = "Option<mirror::ExpectationStatus>")]
    pub status: Option<ExpectationStatus>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub dwell_social: Option<f64>,
    #[serde(default)]
    pub dwell_geometric: Option<f64>,
    #[serde(default)]
    pub dwell_eye: Option<f64>,
    #[serde(default)]
    pub dwell_mouth: Option<f64>,
    #[serde(default)]
    pub orienting_latency_ms: Option<f64>,
    #[serde(default)]
    pub gaze_cue_accuracy: Option<f64>,
    #[serde(default)]
    pub fixation_count: Option<i64>,
    #[serde(default)]
    pub mean_fixation_ms: Option<f64>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub expected: Option<f64>,
    #[serde(default)]
    pub recorded: Option<f64>,
    #[serde(default)]
    pub deviation: Option<f64>,
    #[serde(default)]
    pub baseline_ref: Option<String>,
    #[serde(default)]
    pub rows: Vec<MetricRow>,
}

/// `GET /v1/sessions/{id}/clips` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = ClipList { items: vec![example_clip_score()] })]
pub struct ClipList {
    pub items: Vec<ClipScore>,
}

macro_rules! page {
    ($(#[$doc:meta])* $name:ident, $item:ty) => {
        $(#[$doc])*
        /// Cursor-paginated list envelope: `cursor` is opaque, `limit <= 100`.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
        pub struct $name {
            pub items: Vec<$item>,
            pub next_cursor: Option<String>,
            pub total: Option<u64>,
        }
    };
}

page!(
    /// `GET /v1/orgs/{id}/sessions`.
    SessionPage,
    SessionSummary
);
page!(
    /// `GET /v1/orgs/{id}/devices`.
    DevicePage,
    Device
);
page!(
    /// `GET /v1/orgs/{id}/participants`.
    ParticipantPage,
    Participant
);
page!(
    /// `GET /v1/orgs` (the orgs the caller can open).
    OrgPage,
    Org
);
page!(
    /// `GET /v1/orgs/{id}/users`.
    OrgUserPage,
    OrgUser
);
page!(
    /// `GET /v1/orgs/{id}/review`.
    ReviewPage,
    ReviewItem
);
page!(
    /// `GET /v1/orgs/{id}/audit`.
    AuditPage,
    AuditEntry
);
page!(
    /// `GET /v1/config/{kind}`.
    ConfigDocumentPage,
    ConfigDocument
);

// ---- metrics ---------------------------------------------------------------------------

/// Devices on one bundle version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundleAdoption {
    pub bundle_version: String,
    pub devices: u64,
}

/// `GET /v1/orgs/{id}/metrics` (console dashboard).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_metrics())]
pub struct Metrics {
    pub sessions_7d: u64,
    pub sessions_30d: u64,
    /// Keyed by `SessionAttentionFlag` value.
    pub flag_distribution: BTreeMap<String, u64>,
    /// Keyed by quality verdict.
    pub quality_distribution: BTreeMap<String, u64>,
    pub review_queue_size: u64,
    pub devices_active: u64,
    pub bundle_adoption: Vec<BundleAdoption>,
}

// ---- review (ING-3) --------------------------------------------------------------------

/// Why a session entered the review queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReason {
    LowQuality,
    FlagRed,
    RandomAudit,
    NormCandidate,
    Manual,
    // Tracking verdict `poor` or `unusable` (SCH-4, schema 1.7 sessions). A plain comment:
    // a doc comment would turn the exported enum into a `oneOf`.
    LowTrackingQuality,
}

/// Review item state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Open,
    InReview,
    Done,
    Dismissed,
}

/// `GET /v1/orgs/{id}/review` item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewItem {
    pub id: String,
    pub org_id: String,
    pub session_id: String,
    pub reason: ReviewReason,
    pub status: ReviewStatus,
    pub assigned_to: Option<String>,
    pub decision: Option<Value>,
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    /// Short summary of the session, so the console needs no call per item. Present in
    /// the list; absent from write responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<ReviewSessionSummary>,
}

/// The session fields shown beside a review item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewSessionSummary {
    pub attention_flag: SessionAttentionFlag,
    pub quality_verdict: String,
    pub participant_token: String,
    #[schemars(extend("format" = "date-time"))]
    pub started_at: String,
}

/// `POST /v1/orgs/{id}/review` (manual enqueue).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateReviewRequest {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A human annotation attached by a reviewer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LabelInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_id: Option<String>,
    pub key: String,
    pub value: Value,
}

/// `PATCH /v1/review/{id}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateReviewRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ReviewStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<LabelInput>,
}

// ---- config documents (CFG-1) ----------------------------------------------------------

/// Config document kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDocumentKind {
    StimulusCatalog,
    NormativeBaselines,
    ScoringProfile,
}

/// Lifecycle `draft -> candidate -> published -> retired`; only `published` is bundled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDocumentStatus {
    Draft,
    Candidate,
    Published,
    Retired,
}

/// `GET /v1/config/{kind}/{version}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigDocument {
    pub id: String,
    pub kind: ConfigDocumentKind,
    pub version: String,
    pub status: ConfigDocumentStatus,
    /// The `StimulusCatalog`, `NormativeBaselineSet` or `ScoringProfile` document.
    pub body: Value,
    pub created_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    pub published_at: Option<String>,
    pub notes: Option<String>,
}

/// `POST /v1/config/{kind}` (creates a `draft`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CreateConfigDocumentRequest {
    pub version: String,
    pub body: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// `GET /v1/config/bundles` item (the bundle body itself is `ConfigBundle`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigBundleSummary {
    pub id: String,
    pub bundle_version: String,
    pub catalog_version: String,
    pub baselines_version: String,
    pub scoring_profile_version: String,
    pub key_id: String,
    #[schemars(extend("format" = "date-time"))]
    pub published_at: String,
}

// ---- audit, health ---------------------------------------------------------------------

/// Who performed an audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    User,
    Device,
    System,
}

/// `GET /v1/orgs/{id}/audit` item (AUTH-5). `before`/`after` are redacted snapshots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditEntry {
    pub id: String,
    #[schemars(extend("format" = "date-time"))]
    pub ts: String,
    pub actor_type: ActorType,
    pub actor_id: Option<String>,
    pub org_id: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// `GET /health` (liveness, CON-22).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = Health { status: "ok".into() })]
pub struct Health {
    /// Always `ok` when the process answers.
    pub status: String,
}

/// `GET /health/ready` (DB and blob store, CON-22). `status` is `ok` only when every
/// check is `ok`; the HTTP status is 503 otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_ready())]
pub struct Ready {
    pub status: String,
    pub checks: BTreeMap<String, String>,
}

// ---- examples (published in api-v1.json) -----------------------------------------------

fn example_user() -> User {
    User {
        id: "0b7c9e2a-5d1f-4c3e-9a8b-1f2e3d4c5b6a".into(),
        email: "clinician@example.org".into(),
        role: Role::Clinician,
        org_id: Some("5f0e1d2c-3b4a-4968-8776-655443322110".into()),
    }
}

fn example_token_pair() -> TokenPair {
    TokenPair {
        access_token: "eyJhbGciOiJFZERTQSJ9.e30.c2ln".into(),
        refresh_token: "rt_4s9QpZ0bYq7mN2xK".into(),
        expires_in: 900,
        user: example_user(),
    }
}

fn example_register_response() -> RegisterDeviceResponse {
    RegisterDeviceResponse {
        status: RegistrationStatus::Registered,
        device_id: "9a8b7c6d-5e4f-4321-8fed-cba987654321".into(),
        device_token: "dvc_Qm9ndXNUb2tlbkZvckV4YW1wbGVPbmx5".into(),
        org_id: "5f0e1d2c-3b4a-4968-8776-655443322110".into(),
        config_bundle_version: None,
    }
}

fn example_enrol_code() -> EnrolCodeResponse {
    EnrolCodeResponse {
        id: "1c2d3e4f-5a6b-4c7d-8e9f-0a1b2c3d4e5f".into(),
        code: "K7Q4-9XMP".into(),
        max_uses: 1,
        expires_at: "2026-11-06T12:00:00.000Z".into(),
    }
}

fn example_org() -> Org {
    Org {
        id: "5f0e1d2c-3b4a-4968-8776-655443322110".into(),
        name: "Example Clinic".into(),
        kind: OrgKind::Clinic,
        retention_months: Some(24),
        settings: None,
    }
}

pub(crate) fn example_session_summary() -> SessionSummary {
    SessionSummary {
        id: "SESS-GOLDEN-0001".into(),
        participant_id: "7f1c1c1e-0000-4000-8000-000000000001".into(),
        participant_token: "child-SESS-GOL".into(),
        started_at: "2026-10-26T07:33:20.000Z".into(),
        duration_sec: 32.5,
        battery: Some("express_30s".into()),
        capture_source: Some(CaptureSource::TrueDepth),
        quality_verdict: "optimal".into(),
        attention_flag: SessionAttentionFlag::Yellow,
        composite_score: Some(0.74),
        composite_confidence: Some(0.5),
        bundle_version: None,
        device_id: Some("9a8b7c6d-5e4f-4321-8fed-cba987654321".into()),
        reviewed: Some(false),
        tracking_verdict: None,
        tracking_accuracy_deg: None,
    }
}

fn example_session_detail() -> SessionDetail {
    SessionDetail {
        summary: example_session_summary(),
        org_id: "5f0e1d2c-3b4a-4968-8776-655443322110".into(),
        age_pack: Some("18-30".into()),
        lang: Some("en".into()),
        schema_version: Some("1.6".into()),
        valid_frame_ratio: Some(0.93),
        calibrated_distance_cm: Some(38.0),
        social_preference: Some(0.62),
        eye_dwell: Some(0.33),
        mouth_dwell: None,
        first_eye_latency_ms: None,
        affect: Some(BTreeMap::from([
            ("socialSmileIntensity".to_string(), Value::from(0.41)),
            ("flatAffectScore".to_string(), Value::from(0.12)),
        ])),
        report_pdf_available: Some(false),
        tracking_quality: None,
    }
}

/// A schema 1.7 session: tracking columns set (SCH-4).
fn example_session_summary_v1_7() -> SessionSummary {
    SessionSummary {
        id: "SESS-GOLDEN-0017".into(),
        tracking_verdict: Some(TrackingVerdict::Good),
        tracking_accuracy_deg: Some(1.63),
        ..example_session_summary()
    }
}

fn example_session_detail_v1_7() -> SessionDetail {
    SessionDetail {
        summary: example_session_summary_v1_7(),
        schema_version: Some("1.7".into()),
        tracking_quality: Some(crate::tracking_api::example_tracking_quality()),
        ..example_session_detail()
    }
}

fn example_metric_row() -> MetricRow {
    MetricRow {
        segment_id: "faces".into(),
        metric: "eye_dwell_ratio".into(),
        target_aoi: None,
        direction: NormativeDirection::HigherBetter,
        expected: Some(0.4),
        acceptable_low: Some(0.3),
        acceptable_high: None,
        recorded: 0.33,
        deviation: Some(-0.07),
        status: ExpectationStatus::Monitor,
        score: 0.7,
        imputed: false,
        source: "literature".into(),
    }
}

fn example_clip_score() -> ClipScore {
    ClipScore {
        clip_id: "stim_05_geopref_split_10s".into(),
        order: 1,
        status: Some(ExpectationStatus::Monitor),
        score: Some(0.74),
        weight: Some(0.3),
        dwell_social: Some(0.62),
        dwell_geometric: Some(0.21),
        dwell_eye: Some(0.33),
        dwell_mouth: None,
        orienting_latency_ms: None,
        gaze_cue_accuracy: None,
        fixation_count: Some(14),
        mean_fixation_ms: Some(280.0),
        metric: Some("eye_dwell_ratio".into()),
        expected: Some(0.4),
        recorded: Some(0.33),
        deviation: Some(-0.07),
        baseline_ref: Some("provisional-literature-seeded".into()),
        rows: vec![example_metric_row()],
    }
}

fn example_metrics() -> Metrics {
    Metrics {
        sessions_7d: 12,
        sessions_30d: 40,
        flag_distribution: BTreeMap::from([
            ("green".to_string(), 25),
            ("yellow".to_string(), 10),
            ("red".to_string(), 3),
            ("unreliable".to_string(), 2),
        ]),
        quality_distribution: BTreeMap::from([
            ("optimal".to_string(), 30),
            ("acceptable".to_string(), 8),
            ("degraded".to_string(), 2),
        ]),
        review_queue_size: 4,
        devices_active: 3,
        bundle_adoption: vec![BundleAdoption {
            bundle_version: "2026.10.30-1".into(),
            devices: 3,
        }],
    }
}

fn example_ready() -> Ready {
    Ready {
        status: "ok".into(),
        checks: BTreeMap::from([
            ("blob_store".to_string(), "ok".to_string()),
            ("database".to_string(), "ok".to_string()),
        ]),
    }
}
