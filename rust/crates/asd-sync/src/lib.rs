//! owner: WP-R.3
//!
//! Device-sync protocol logic without I/O (CON-7). The apps own the transport (URLSession,
//! OkHttp); everything they decide about a request or a response comes from here:
//! - [`build_session_request`]: method, path, headers and body bytes of `POST /v1/sessions`;
//! - [`build_register_request`]: `POST /v1/devices/register` (CON-10, CON-11);
//! - [`classify_response`] / [`classify_register_response`]: what an HTTP answer means;
//! - [`validate_upload_url`] / [`validate_ingest_url`] / [`endpoint_url`]: the https and
//!   DNS-host rules of F7 and whether a bearer may be attached;
//! - [`backoff_schedule`] / [`next_retry_delay`]: the retry schedule.
//!
//! Ported from `SessionSyncClient.swift`, `DeviceRegistrationService.swift`,
//! `SyncTransport.swift` (`retryAfterSeconds`), `ResearchStudyConfiguration.validateIngestURL`
//! and `ResearchSyncRetryPolicy`; their tests are the oracle (`tests/protocol.rs`).

mod request;
mod response;
mod retry;
mod url_rules;

pub use request::{
    build_register_request, build_session_request, build_session_request_json, ClientInfo,
    SyncRequest, REGISTER_PATH, REGISTER_TIMEOUT_SECS, SESSIONS_PATH, SESSION_TIMEOUT_SECS,
};
pub use response::{
    classify_register_response, classify_response, retry_after_seconds, IngestAck, RegisterOutcome,
    SyncOutcome, MAX_RETRY_AFTER_SECS,
};
pub use retry::{backoff_schedule, next_retry_delay, BACKOFF_DELAYS_SECS, MAX_ATTEMPTS};
pub use url_rules::{
    endpoint_url, validate_ingest_url, validate_upload_url, UploadTarget, UrlError,
};

/// Why a request could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    /// No device token: the device needs enrolment.
    #[error("device is not registered")]
    NotRegistered,
    /// Simulated sessions are refused by every sink (SIM-5).
    #[error("simulated sessions are never uploaded")]
    SimulatedCapture,
    /// Pre-v1.6 records with inferred provenance are not uploaded (SIM-4).
    #[error("records with legacy provenance are not uploaded")]
    LegacyProvenance,
    #[error("unsupported schema version `{0}`")]
    UnsupportedSchemaVersion(String),
    /// A `rawPayload` date is not RFC 3339 UTC with milliseconds.
    #[error("{field} is not an RFC 3339 UTC date with milliseconds: `{value}`")]
    InvalidDate { field: &'static str, value: String },
    #[error("enrol code is empty")]
    EmptyEnrolCode,
    /// The payload JSON does not match the session contract.
    #[error("payload does not match the session contract: {0}")]
    InvalidPayload(String),
}
