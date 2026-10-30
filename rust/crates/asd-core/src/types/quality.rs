//! owner: WP-R.0
//!
//! Port of `GazeQualityGate.Verdict`.

use serde::{Deserialize, Serialize};

/// Per-sample quality verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GazeQualityVerdict {
    Optimal,
    Acceptable,
    Degraded,
    Rejected,
}
