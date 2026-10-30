//! owner: WP-R.0
//!
//! Port of `CaptureSource` (`GazeTypes.swift`) plus the `synthetic` value of CON-13.

use serde::{Deserialize, Serialize};

/// Where a session's gaze data came from. `synthetic` is new in the Rust contract; the
/// backend rejects `simulated` always and `synthetic` unless explicitly allowed (CON-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureSource {
    #[serde(rename = "truedepth")]
    TrueDepth,
    #[serde(rename = "simulated")]
    Simulated,
    #[serde(rename = "synthetic")]
    Synthetic,
}
