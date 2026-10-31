//! owner: WP-R.5
//!
//! UniFFI surface over asd-core, asd-contract and asd-sync (FFI-1, ADR-0006 §4).
//!
//! Documents (frames, baselines, scores, profiles, bundles, session payloads) cross the
//! boundary as UTF-8 JSON in the Swift `Codable` wire format; Swift and Kotlin keep their
//! own types and never mirror the Rust structs. Scalars cross as scalars. The sync entry
//! points return small records (`AsdSyncRequest`, `AsdSyncOutcome`, ...) because headers,
//! body bytes and outcome variants are protocol values, not documents.
//!
//! Every fallible entry point returns `Result<_, AsdError>`: malformed or wrong-shaped JSON
//! is an `AsdError::InvalidJson`, never a panic (no reliance on `catch_unwind`).
//! This file holds no logic beyond decoding, delegating and encoding.

use asd_contract::config::ScoringProfile;
use asd_contract::signing::{self, SigningError};
use asd_core::types::{
    GazeFrame, GazeTelemetrySample, NormativeBaselineSet, PerVideoMetricSet, PerVideoSessionScore,
};
use asd_core::{aggregate, baselines, flag, metrics, quality, scoring};
use asd_sync::{RegisterOutcome, SyncError, SyncOutcome, UrlError};
use serde::de::DeserializeOwned;
use serde::Serialize;

uniffi::setup_scaffolding!("asd_core");

/// The single error type of the boundary. The variant is the kind; `message` is for logs
/// and never contains document contents beyond a serde position hint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum AsdError {
    /// Not JSON, or JSON that does not match the expected document.
    #[error("invalid JSON: {message}")]
    InvalidJson { message: String },
    /// A scalar argument is unusable (empty enrol code, malformed public key).
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
    /// No device token: the device needs enrolment before uploading.
    #[error("not registered: {message}")]
    NotRegistered { message: String },
    /// The session must never be uploaded (simulated capture, legacy provenance,
    /// unsupported schema version). Quarantine it; retrying cannot help.
    #[error("upload refused: {message}")]
    UploadRefused { message: String },
    /// A URL breaks the device-plane rules (F7: https, DNS host, no credentials).
    #[error("invalid URL: {message}")]
    InvalidUrl { message: String },
}

fn invalid_json(what: &str, e: impl std::fmt::Display) -> AsdError {
    AsdError::InvalidJson {
        message: format!("{what}: {e}"),
    }
}

fn decode<T: DeserializeOwned>(json: &str, what: &str) -> Result<T, AsdError> {
    serde_json::from_str(json).map_err(|e| invalid_json(what, e))
}

fn encode<T: Serialize>(value: &T, what: &str) -> Result<String, AsdError> {
    serde_json::to_string(value).map_err(|e| invalid_json(what, e))
}

impl From<SyncError> for AsdError {
    fn from(e: SyncError) -> Self {
        let message = e.to_string();
        match e {
            SyncError::NotRegistered => Self::NotRegistered { message },
            SyncError::SimulatedCapture
            | SyncError::LegacyProvenance
            | SyncError::UnsupportedSchemaVersion(_) => Self::UploadRefused { message },
            SyncError::InvalidDate { .. } | SyncError::InvalidPayload(_) => {
                Self::InvalidJson { message }
            }
            SyncError::EmptyEnrolCode => Self::InvalidArgument { message },
        }
    }
}

impl From<UrlError> for AsdError {
    fn from(e: UrlError) -> Self {
        Self::InvalidUrl {
            message: e.to_string(),
        }
    }
}

// --- version -------------------------------------------------------------------------

/// Version of the Rust core linked into the app (the workspace version).
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

// --- scoring, flag, metrics, quality, baselines (CORE-1..7) ---------------------------

/// `PerVideoScoringEngine.score`: `[PerVideoMetricSet]` + `NormativeBaselineSet` + all
/// session `[GazeFrame]` -> `PerVideoSessionScore` JSON. `scored_at_rfc3339` is written
/// verbatim (use [`format_scored_at`] for the Swift format).
#[uniffi::export]
pub fn score_session_json(
    metric_sets_json: String,
    baseline_json: String,
    age_months: i64,
    frames_json: String,
    scored_at_rfc3339: String,
) -> Result<String, AsdError> {
    let metric_sets: Vec<PerVideoMetricSet> = decode(&metric_sets_json, "metric sets")?;
    let baseline: NormativeBaselineSet = decode(&baseline_json, "baseline set")?;
    let frames: Vec<GazeFrame> = decode(&frames_json, "frames")?;
    let score = scoring::score_session(
        &metric_sets,
        &baseline,
        age_months,
        &frames,
        &scored_at_rfc3339,
    );
    encode(&score, "session score")
}

/// `ISO8601DateFormatter().string(from:)` of a Unix time: `YYYY-MM-DDTHH:MM:SSZ`.
#[uniffi::export]
pub fn format_scored_at(unix_seconds: f64) -> String {
    scoring::format_scored_at(unix_seconds)
}

/// `AttentionFlagAlgorithm.assess`: `PerVideoSessionScore` + `ScoringProfile` (the bundle
/// document; only `flag` is read) -> `AttentionFlagAssessment` JSON.
#[uniffi::export]
pub fn assess_flag_json(score_json: String, profile_json: String) -> Result<String, AsdError> {
    let score: PerVideoSessionScore = decode(&score_json, "session score")?;
    let profile: ScoringProfile = decode(&profile_json, "scoring profile")?;
    let t = &profile.flag;
    let thresholds = flag::FlagThresholds {
        green_floor: t.green_floor,
        yellow_floor: t.yellow_floor,
        low_confidence_share: t.low_confidence_share,
        atypical_share_for_red: t.atypical_share_for_red,
    };
    encode(&flag::assess(&score, &thresholds), "flag assessment")
}

/// The bundled default `ScoringProfile` (today's Swift constants, CON-5), for callers that
/// have no delivered config bundle yet.
#[uniffi::export]
pub fn default_scoring_profile_json() -> String {
    // A plain struct of strings and finite floats always serialises.
    serde_json::to_string(&ScoringProfile::default()).unwrap_or_default()
}

/// `PerVideoMetricsExtractor.extract(from:)`: `[GazeFrame]` -> `[PerVideoMetricSet]` JSON.
#[uniffi::export]
pub fn extract_metrics_json(frames_json: String) -> Result<String, AsdError> {
    let frames: Vec<GazeFrame> = decode(&frames_json, "frames")?;
    encode(&metrics::extract(&frames), "metric sets")
}

/// `SessionBiomarkerAggregator.aggregate(validFrames:durationSec:)`: `[GazeFrame]` ->
/// aggregates JSON (camelCase; unmeasured ratios and latency are absent keys).
#[uniffi::export]
pub fn aggregate_session_json(frames_json: String, duration_sec: f64) -> Result<String, AsdError> {
    let frames: Vec<GazeFrame> = decode(&frames_json, "frames")?;
    encode(&aggregate::session(&frames, duration_sec), "aggregates")
}

/// `GazeQualityGate.evaluateSample`: `GazeTelemetrySample` + gate parameters (camelCase,
/// the `ScoringProfile.qualityGate` object) -> `{isValid, verdict, failureReasons, isBlink}`.
#[uniffi::export]
pub fn evaluate_quality_json(sample_json: String, gate_json: String) -> Result<String, AsdError> {
    let sample: GazeTelemetrySample = decode(&sample_json, "telemetry sample")?;
    let gate: quality::QualityGate = decode(&gate_json, "quality gate")?;
    encode(&quality::evaluate_sample(&sample, &gate), "evaluation")
}

/// `NormativeBaselineLibrary.validate`: coverage problems as human-readable lines, empty
/// when the set is complete.
#[uniffi::export]
pub fn validate_baselines_json(
    set_json: String,
    required_ids: Vec<String>,
) -> Result<Vec<String>, AsdError> {
    let set: NormativeBaselineSet = decode(&set_json, "baseline set")?;
    Ok(baselines::validate(&set, &required_ids))
}

// --- config bundle (CON-6) ------------------------------------------------------------

/// Verifies a signed `ConfigBundle` against one trusted Ed25519 public key (base64url or
/// standard base64, 32 bytes).
///
/// `Ok(true)`: the signature is valid and the bundle matches the contract; use it.
/// `Ok(false)`: unsigned, malformed signature, wrong key or tampered; discard it.
/// `Err(InvalidJson)`: not a JSON object, or signed but not a `ConfigBundle`.
/// `Err(InvalidArgument)`: the public key itself is malformed (an app configuration bug).
#[uniffi::export]
pub fn verify_bundle(bundle_json: String, public_key_b64: String) -> Result<bool, AsdError> {
    match signing::verify_bundle_b64(&bundle_json, &public_key_b64) {
        Ok(_) => Ok(true),
        Err(
            SigningError::MissingSignature
            | SigningError::MalformedSignature
            | SigningError::BadSignature
            | SigningError::MissingKeyId
            | SigningError::UnknownKeyId(_),
        ) => Ok(false),
        Err(e @ SigningError::InvalidKey(_)) => Err(AsdError::InvalidArgument {
            message: e.to_string(),
        }),
        Err(
            e @ (SigningError::InvalidJson(_)
            | SigningError::NotAnObject
            | SigningError::InvalidBundle(_)
            | SigningError::Canonicalisation(_)),
        ) => Err(invalid_json("bundle", e)),
    }
}

// --- sync protocol (CON-7) ------------------------------------------------------------

/// One HTTP header, name as sent.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsdHttpHeader {
    pub name: String,
    pub value: String,
}

/// App identity for `User-Agent`: `<product>/<app_version> (<build>; <os_name> <os_version>)`.
/// iOS uses product `EyeGaze-iOS`, os name `iOS`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsdClientInfo {
    pub product: String,
    pub app_version: String,
    pub build: String,
    pub os_name: String,
    pub os_version: String,
}

impl From<AsdClientInfo> for asd_sync::ClientInfo {
    fn from(c: AsdClientInfo) -> Self {
        Self {
            product: c.product,
            app_version: c.app_version,
            build: c.build,
            os_name: c.os_name,
            os_version: c.os_version,
        }
    }
}

/// A request for the app's transport to send unchanged. `path` is relative to the API base
/// (join with [`sync_endpoint_url`]); `body` are the exact bytes (identical on retry).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsdSyncRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<AsdHttpHeader>,
    pub body: Vec<u8>,
    pub timeout_secs: u32,
}

impl From<asd_sync::SyncRequest> for AsdSyncRequest {
    fn from(r: asd_sync::SyncRequest) -> Self {
        Self {
            method: r.method,
            path: r.path,
            headers: r
                .headers
                .into_iter()
                .map(|(name, value)| AsdHttpHeader { name, value })
                .collect(),
            body: r.body,
            timeout_secs: r.timeout_secs,
        }
    }
}

/// The useful part of a `2xx` session response; every field optional.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsdIngestAck {
    pub session_id: Option<String>,
    pub status: Option<String>,
    pub duplicate: Option<bool>,
    pub frames_upload_url: Option<String>,
    pub report_upload_url: Option<String>,
}

/// What the answer to `POST /v1/sessions` means.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum AsdSyncOutcome {
    /// `2xx` (a replay answered `200 duplicate:true` included): delivered.
    Accepted { ack: AsdIngestAck },
    /// `409`: delivered earlier; count as delivered.
    Duplicate,
    /// `401`/`403`: re-register once with the stored enrol code, then retry.
    Reauthenticate,
    /// `408`, `429`, `5xx`: retry later; `seconds` is a usable `Retry-After` (<= 3600),
    /// combine with [`sync_next_retry_delay`].
    RetryAfter { seconds: Option<f64> },
    /// Anything else: will never be accepted; quarantine.
    Fatal { status: u16 },
}

/// What the answer to `POST /v1/devices/register` means.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AsdRegisterOutcome {
    /// Store `device_token` bound to the API host; keep `device_id` for re-registration.
    Registered {
        status: String,
        device_id: String,
        device_token: String,
        org_id: String,
        config_bundle_version: Option<String>,
    },
    /// The server refused the code; `410` = exhausted or expired.
    Rejected { status: u16 },
    /// `2xx` without a usable token or org id.
    Invalid,
}

/// Where to PUT a blob, and whether the device bearer may be attached (API host only).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsdUploadTarget {
    pub url: String,
    pub attach_bearer: bool,
}

/// `POST /v1/sessions` from a `SessionIngestRequest` (session v1.6) JSON payload: headers
/// (bearer, `X-Idempotency-Key = sessionId`, schema version, user agent) and canonical body.
#[uniffi::export]
pub fn sync_build_session_request_json(
    payload_json: String,
    device_token: String,
    client: AsdClientInfo,
) -> Result<AsdSyncRequest, AsdError> {
    let request = asd_sync::build_session_request_json(
        payload_json.as_bytes(),
        &device_token,
        &client.into(),
    )?;
    Ok(request.into())
}

/// `POST /v1/devices/register`; pass the stored `device_id` on re-registration so the
/// server rotates that device's token (CON-11). `idempotency_key` is a fresh UUID.
#[uniffi::export]
pub fn sync_build_register_request(
    enrol_code: String,
    device_id: Option<String>,
    idempotency_key: String,
    client: AsdClientInfo,
) -> Result<AsdSyncRequest, AsdError> {
    let request = asd_sync::build_register_request(
        &enrol_code,
        device_id.as_deref(),
        &idempotency_key,
        &client.into(),
    )?;
    Ok(request.into())
}

/// Classifies the answer to `POST /v1/sessions`.
#[uniffi::export]
pub fn sync_classify_response(
    status: u16,
    headers: Vec<AsdHttpHeader>,
    body: Vec<u8>,
) -> AsdSyncOutcome {
    let headers: Vec<(String, String)> = headers.into_iter().map(|h| (h.name, h.value)).collect();
    match asd_sync::classify_response(status, &headers, &body) {
        SyncOutcome::Accepted(a) => AsdSyncOutcome::Accepted {
            ack: AsdIngestAck {
                session_id: a.session_id,
                status: a.status,
                duplicate: a.duplicate,
                frames_upload_url: a.frames_upload_url,
                report_upload_url: a.report_upload_url,
            },
        },
        SyncOutcome::Duplicate => AsdSyncOutcome::Duplicate,
        SyncOutcome::Reauthenticate => AsdSyncOutcome::Reauthenticate,
        SyncOutcome::RetryAfter(seconds) => AsdSyncOutcome::RetryAfter { seconds },
        SyncOutcome::Fatal { status } => AsdSyncOutcome::Fatal { status },
    }
}

/// Classifies the answer to `POST /v1/devices/register`.
#[uniffi::export]
pub fn sync_classify_register_response(status: u16, body: Vec<u8>) -> AsdRegisterOutcome {
    match asd_sync::classify_register_response(status, &body) {
        RegisterOutcome::Registered(r) => AsdRegisterOutcome::Registered {
            status: serde_json::to_value(r.status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            device_id: r.device_id,
            device_token: r.device_token,
            org_id: r.org_id,
            config_bundle_version: r.config_bundle_version,
        },
        RegisterOutcome::Rejected { status } => AsdRegisterOutcome::Rejected { status },
        RegisterOutcome::Invalid => AsdRegisterOutcome::Invalid,
    }
}

/// F7 rules for a blob upload URL (absolute, or relative to `api_base`).
#[uniffi::export]
pub fn sync_validate_upload_url(
    url: String,
    api_base: String,
) -> Result<AsdUploadTarget, AsdError> {
    let t = asd_sync::validate_upload_url(&url, &api_base)?;
    Ok(AsdUploadTarget {
        url: t.url,
        attach_bearer: t.attach_bearer,
    })
}

/// F7 rules for the configured ingest (API base) URL.
#[uniffi::export]
pub fn sync_validate_ingest_url(url: String) -> Result<(), AsdError> {
    Ok(asd_sync::validate_ingest_url(&url)?)
}

/// Absolute URL of `path` (e.g. `v1/sessions`) under the API base.
#[uniffi::export]
pub fn sync_endpoint_url(api_base: String, path: String) -> Result<String, AsdError> {
    Ok(asd_sync::endpoint_url(&api_base, &path)?)
}

/// Client backoff in seconds before retry `attempt` (0-based).
#[uniffi::export]
pub fn sync_backoff_schedule(attempt: u32) -> u64 {
    asd_sync::backoff_schedule(attempt)
}

/// Delay before the next retry: the server's `Retry-After` (capped) wins over the schedule.
#[uniffi::export]
pub fn sync_next_retry_delay(attempt: u32, retry_after_secs: Option<f64>) -> f64 {
    asd_sync::next_retry_delay(attempt, retry_after_secs)
}

/// Total attempts (first try plus retries) before a session is parked.
#[uniffi::export]
pub fn sync_max_attempts() -> u32 {
    asd_sync::MAX_ATTEMPTS
}
