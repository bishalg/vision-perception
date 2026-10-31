//! owner: WP-B.8
//!
//! Report and interop exports of a stored gaze session, shared by every platform (one
//! implementation, CLAUDE.md "Rust is the core language"):
//!
//! - [`fhir`]: the FHIR R4 `Bundle` (`type: collection`), a port of the iOS
//!   `FHIRBundleBuilder` + `FHIRInteroperabilityMapper` (ADR-0003, FHIR-1..4), plus a
//!   `DocumentReference` carrying the rendered PDF;
//! - [`pdf`]: the one-page observation diary for the pediatrician, a port of the iOS
//!   `DoctorReportGenerator` (CLM-5, PLC-2), rendered deterministically;
//! - [`claims`]: the mandatory non-diagnostic copy both sinks print.
//!
//! Why a crate and not a module of `asd-contract`: `asd-contract` is the wire-schema source
//! of truth and is linked into the mobile static library (`asd-ffi`); a PDF writer and
//! presentation copy do not belong there. Later interop exports (CSV, XLSX, HL7 v2) are
//! added here as sibling modules over the same inputs.
//!
//! Refusal (SIM-5): simulated and synthetic captures never become a health record. The
//! bundle omits the gaze `Observation` for them (as iOS does) and [`pdf::render`] refuses
//! them outright; [`export_refusal`] says why.

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_core::types::{CaptureSource, GazeQualityVerdict};

pub mod claims;
pub mod fhir;
mod ids;
pub mod mchat;
pub mod pdf;

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
