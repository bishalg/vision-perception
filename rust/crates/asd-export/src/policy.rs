//! owner: WP-I.1
//!
//! Consent eligibility (XC-5): which sessions an export may include, decided here for every
//! format (ADR-0008 D7). Pure and table-tested (`tests/policy.rs`).
//!
//! Rules, in order:
//! 1. a withdrawn participant is excluded, always;
//! 2. the purpose must allow the mode: `research` and `quality_improvement` are
//!    de-identified only (the request is refused up front with
//!    `400 consent-purpose-mismatch`, [`validate_request`]; [`eligibility`] also excludes
//!    so a caller that skips validation still leaks nothing);
//! 3. `research` includes a session only when its `normCollection.consented` is `true`
//!    (G15); an absent value is not consent;
//! 4. `clinical_care` and `patient_request` include in either mode, whatever the research
//!    consent says.
//!
//! [`Purpose`] and [`Mode`] carry the wire names of `ExportRequest` (EXJ-1); the contract
//! DTO (WP-I.6, `asd-contract::export`) uses the same snake_case strings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::ExportRecord;

/// Why the export is made (EXJ-1 `purpose`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    ClinicalCare,
    Research,
    QualityImprovement,
    PatientRequest,
}

impl Purpose {
    /// Human-readable label used in watermarks.
    pub fn label(self) -> &'static str {
        match self {
            Self::ClinicalCare => "clinical care",
            Self::Research => "research",
            Self::QualityImprovement => "quality improvement",
            Self::PatientRequest => "patient request",
        }
    }

    /// Whether this purpose allows `mode` (rule 2).
    pub fn allows(self, mode: Mode) -> bool {
        match self {
            Self::ClinicalCare | Self::PatientRequest => true,
            Self::Research | Self::QualityImprovement => mode == Mode::Deidentified,
        }
    }
}

/// Identified (MRN, token, free text where the caller may read it) or de-identified
/// (XC-4, [`crate::deid`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Identified,
    Deidentified,
}

impl Mode {
    /// Human-readable label used in watermarks.
    pub fn label(self) -> &'static str {
        match self {
            Self::Identified => "identified",
            Self::Deidentified => "de-identified",
        }
    }
}

/// The consent facts of one session, supplied by the backend (it never decides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ConsentFacts {
    /// `participants.withdrawn_at` is set.
    pub withdrawn: bool,
    /// The session's `summary.normCollection.consented`; `None` when absent (G15).
    pub research_consented: Option<bool>,
}

/// Why a session is left out of an export. Serialised names are the keys of
/// `exports.excluded` (EXJ-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    /// The participant withdrew.
    Withdrawn,
    /// `research` purpose and no recorded research consent for the session.
    NoResearchConsent,
    /// The purpose does not allow the mode.
    ModeNotAllowed,
}

impl ExclusionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Withdrawn => "withdrawn",
            Self::NoResearchConsent => "no_research_consent",
            Self::ModeNotAllowed => "mode_not_allowed",
        }
    }
}

/// The decision for one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    Include,
    Exclude(ExclusionReason),
}

/// A request the policy refuses as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// `400 consent-purpose-mismatch` (CTR-2).
    #[error("purpose {purpose:?} does not allow {mode:?} exports")]
    ConsentPurposeMismatch { purpose: Purpose, mode: Mode },
}

/// Request-time check (EXJ-1): identified exports for `research` or
/// `quality_improvement` are refused before any job runs.
pub fn validate_request(purpose: Purpose, mode: Mode) -> Result<(), PolicyError> {
    if purpose.allows(mode) {
        Ok(())
    } else {
        Err(PolicyError::ConsentPurposeMismatch { purpose, mode })
    }
}

/// Whether one session may be included (rules 1..4 in the module docs).
pub fn eligibility(purpose: Purpose, mode: Mode, facts: &ConsentFacts) -> Eligibility {
    if facts.withdrawn {
        return Eligibility::Exclude(ExclusionReason::Withdrawn);
    }
    if !purpose.allows(mode) {
        return Eligibility::Exclude(ExclusionReason::ModeNotAllowed);
    }
    if purpose == Purpose::Research && facts.research_consented != Some(true) {
        return Eligibility::Exclude(ExclusionReason::NoResearchConsent);
    }
    Eligibility::Include
}

/// The outcome of [`select`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    /// Indexes into the input of the included records, in input order.
    pub included: Vec<usize>,
    /// Excluded records counted by reason (`exports.excluded`); reasons with no record are
    /// absent.
    pub excluded: BTreeMap<ExclusionReason, usize>,
}

/// Validates the request, then applies [`eligibility`] to every record's
/// [`ExportRecord::consent`].
pub fn select(
    records: &[ExportRecord],
    purpose: Purpose,
    mode: Mode,
) -> Result<Selection, PolicyError> {
    validate_request(purpose, mode)?;
    let mut out = Selection::default();
    for (i, record) in records.iter().enumerate() {
        match eligibility(purpose, mode, &record.consent) {
            Eligibility::Include => out.included.push(i),
            Eligibility::Exclude(reason) => *out.excluded.entry(reason).or_default() += 1,
        }
    }
    Ok(out)
}
