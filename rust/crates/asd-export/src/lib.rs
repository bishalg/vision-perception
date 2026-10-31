//! owner: WP-I.1 (crate skeleton, `model`, `deid`, `policy`, `codes`, `watermark`);
//! WP-B.8 (session FHIR bundle, observation diary PDF and refusals, moved here from the
//! former `asd-report` crate by WP-I.1, XC-2)
//!
//! `asd-export` owns every outbound format of the ASD product, one implementation each
//! (ADR-0008 D6, XC-1, XC-2; CLAUDE.md "Rust is the core language"):
//!
//! - [`fhir`]: the session FHIR R4 `Bundle` (iOS parity, WP-B.8) and the clinical report
//!   bundle (WP-I.3);
//! - `pdf` (feature `pdf`): the observation diary (WP-B.8), the clinical summary and the
//!   letter (WP-I.5);
//! - [`tabular`]: CSV in three dialects and XLSX (feature `xlsx`) (WP-I.2);
//! - [`hl7v2`]: HL7 v2.5.1 ORU^R01 (WP-I.4);
//! - [`model`]: the canonical input [`model::ExportRecord`] (XC-3). The backend builds it;
//!   the crate never queries anything;
//! - [`deid`], [`policy`], [`watermark`], [`codes`]: de-identification (XC-4), consent
//!   eligibility (XC-5), the watermark and mandatory sentence (XC-6) and the external code
//!   table with its `verified` flags (XC-7). These rules live here, not in callers
//!   (ADR-0008 D7), so every format gets the same ones;
//! - [`claims`]: the mandatory non-diagnostic copy every human-readable artifact prints.
//!
//! Why a crate and not a module of `asd-contract`: `asd-contract` is the wire-schema source
//! of truth and is linked into the mobile static library (`asd-ffi`); format writers and
//! presentation copy do not belong there.
//!
//! The crate is pure: no I/O, no async, no clock and no randomness (the de-identification
//! salt and `generated_at` are inputs). Encoders are `fn(&Input, &Options) -> Result<Vec<u8>>`
//! and give identical bytes for identical input (XC-8). It builds for `aarch64-apple-ios`
//! with `--no-default-features` (future FFI use).
//!
//! Refusal (SIM-5): simulated and synthetic captures never become a health record. The
//! session bundle omits the gaze `Observation` for them (as iOS does) and the diary PDF
//! refuses them outright; [`export_refusal`] says why.

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_core::types::{CaptureSource, GazeQualityVerdict};

pub mod claims;
pub mod codes;
pub mod deid;
pub mod fhir;
pub mod hl7v2;
mod ids;
pub mod mchat;
pub mod model;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod policy;
pub mod tabular;
pub mod watermark;

/// Why a session cannot be exported as a health record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `captureSource: simulated` (SIM-5).
    SimulatedCapture,
    /// `captureSource: synthetic` (CON-13, generated test data).
    SyntheticCapture,
    /// Quality verdict `rejected`: too few usable samples (FHIR-2).
    InsufficientSamples,
}

impl Refusal {
    /// Parent- and clinician-facing reason (the iOS `refusalReason` copy).
    pub fn message(self) -> &'static str {
        match self {
            Self::SimulatedCapture => "Simulated data cannot be exported as a health record.",
            Self::SyntheticCapture => "Synthetic data cannot be exported as a health record.",
            Self::InsufficientSamples => {
                "This observation did not have enough usable samples to export."
            }
        }
    }

    /// `true` for the capture-source refusals (the session is not a real observation).
    pub fn is_capture_source(self) -> bool {
        matches!(self, Self::SimulatedCapture | Self::SyntheticCapture)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// Errors of the report builders.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("refused: {0}")]
    Refused(Refusal),
    /// A stored date that is not RFC 3339; nothing is guessed.
    #[error("{field} is not an RFC 3339 timestamp")]
    InvalidTimestamp { field: &'static str },
}

/// Errors of the ADR-0008 encoders and helpers. The WP-B.8 builders keep [`ReportError`]
/// (the backend maps it), which converts into this.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Report(#[from] ReportError),
    /// A timestamp in the input that is not RFC 3339; nothing is guessed.
    #[error("{field} is not an RFC 3339 timestamp")]
    InvalidTimestamp { field: &'static str },
    /// `OrgFacts.timezone` is not an IANA time-zone name.
    #[error("unknown time zone {0:?}")]
    InvalidTimezone(String),
    /// An external code with `verified = false` and no opt-in in the profile (XC-7).
    #[error(transparent)]
    UnverifiedCode(#[from] codes::UnverifiedCode),
    /// The purpose does not allow the mode (XC-5; `400 consent-purpose-mismatch`).
    #[error(transparent)]
    Policy(#[from] policy::PolicyError),
    /// HL7 v2 refusals (no MRN, de-identified, missing PDF; WP-I.4).
    #[error(transparent)]
    Hl7(#[from] hl7v2::Hl7Error),
    /// More rows than one artifact may hold (XLSX-3, EXJ-9; `400 export-too-large`).
    #[error("{rows} rows exceed the limit of {limit}")]
    ExportTooLarge { rows: usize, limit: usize },
}

/// Why `record`'s gaze Observation cannot be exported, or `None` when it can.
pub fn export_refusal(record: &GazeSessionRecord) -> Option<Refusal> {
    match record.capture_source {
        CaptureSource::Simulated => Some(Refusal::SimulatedCapture),
        CaptureSource::Synthetic => Some(Refusal::SyntheticCapture),
        CaptureSource::TrueDepth => observation_status(record)
            .is_none()
            .then_some(Refusal::InsufficientSamples),
    }
}

/// `Observation.status` (FHIR-2): `final` for optimal/acceptable live captures,
/// `preliminary` for degraded ones, `None` (no Observation) otherwise.
pub fn observation_status(record: &GazeSessionRecord) -> Option<&'static str> {
    if record.capture_source != CaptureSource::TrueDepth {
        return None;
    }
    match record.quality_verdict {
        GazeQualityVerdict::Optimal | GazeQualityVerdict::Acceptable => Some("final"),
        GazeQualityVerdict::Degraded => Some("preliminary"),
        GazeQualityVerdict::Rejected => None,
    }
}
