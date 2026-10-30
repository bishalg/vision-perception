//! owner: WP-R.0
//!
//! Port of `AttentionFlag.swift` model types.

use serde::{Deserialize, Serialize};

/// Caregiver-facing traffic light. `unreliable` exists only in the backend-derived column
/// (CON-15), never in this device-side enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttentionFlag {
    Green,
    Yellow,
    Red,
}

fn default_true() -> bool {
    true
}

/// Flag plus the strings shown to the caregiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionFlagAssessment {
    pub flag: AttentionFlag,
    pub headline: String,
    pub detail: String,
    /// Always true in Swift; tolerated as absent on decode, always written.
    #[serde(default = "default_true")]
    pub is_screening_signal_only: bool,
}
