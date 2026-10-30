//! owner: WP-R.2
//!
//! CORE-6: session-level biomarker reduction, a 1:1 port of
//! `SessionBiomarkerAggregator.aggregate` (`SessionBiomarkerAggregator.swift` at 9655fcc).
//! The golden vectors in `tests/golden/aggregate/` were produced by running the Swift body.
//!
//! - One frame counts at most once per role, so nested boxes (face + eyes) do not double-count.
//! - Eye/mouth ratios are `None` (unmeasured, PLC-2) when no face-feature box was hit.
//! - First-orient latency is the session time (`videoTimeSec`) of the first person/eye hit,
//!   clamped at 0, and `None` (PLC-1) when the child never oriented; never a default.
//! - Smile reactivity is the session peak (floored at 0) minus the mean smile over frames
//!   with `videoTimeSec <= 0.5`, clamped to `[0, 1]`.
//! - The caller passes frames already filtered for validity; they are not re-filtered.

use serde::{Deserialize, Serialize};

use crate::metrics::{
    clamp01, expressivity_of, smile_of, surprise_of, swift_max, AoiRole, AoiRoleKind,
    FRAME_INTERVAL_SEC,
};
use crate::types::GazeFrame;

/// `SessionBiomarkerAggregator.Aggregates`. Swift does not encode this struct; the JSON shape
/// (camelCase, unmeasured values as absent keys) is defined here for the FFI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAggregates {
    pub social_dwell_sec: f64,
    pub geometric_dwell_sec: f64,
    pub social_preference_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eye_dwell_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouth_dwell_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_orient_latency_ms: Option<f64>,
    pub social_smile_intensity_mean: f64,
    pub smile_reactivity_index: f64,
    pub flat_affect_score: f64,
    pub surprise_score_mean: f64,
}

/// `SessionBiomarkerAggregator.aggregate(validFrames:durationSec:)` with built-in AOI roles.
///
/// `duration_sec` is accepted for signature parity; the Swift body does not read it.
pub fn session(valid_frames: &[GazeFrame], duration_sec: f64) -> SessionAggregates {
    session_with_roles(valid_frames, duration_sec, &AoiRole::builtin())
}

/// [`session`] with catalog-driven AOI roles.
pub fn session_with_roles(
    valid_frames: &[GazeFrame],
    duration_sec: f64,
    roles: &AoiRole,
) -> SessionAggregates {
    let _ = duration_sec;
    if valid_frames.is_empty() {
        return SessionAggregates {
            social_dwell_sec: 0.0,
            geometric_dwell_sec: 0.0,
            social_preference_score: 0.0,
            eye_dwell_ratio: None,
            mouth_dwell_ratio: None,
            first_orient_latency_ms: None,
            social_smile_intensity_mean: 0.0,
            smile_reactivity_index: 0.0,
            flat_affect_score: 1.0,
            surprise_score_mean: 0.0,
        };
    }

    let mut social_frames = 0_i64;
    let mut geometric_frames = 0_i64;
    let mut eye_frames = 0_i64;
    let mut mouth_frames = 0_i64;
    let mut first_orient_sec: Option<f64> = None;
    let mut smile_sum = 0.0;
    let mut surprise_sum = 0.0;
    let mut expressivity_sum = 0.0;
    let mut peak_smile = 0.0;
    let mut baseline_smile_sum = 0.0;
    let mut baseline_count = 0_i64;

    for frame in valid_frames {
        let names = &frame.active_aoi_names;
        if roles.intersects(names, AoiRoleKind::Social) {
            social_frames += 1;
        }
        if roles.intersects(names, AoiRoleKind::Geometric) {
            geometric_frames += 1;
        }
        if roles.intersects(names, AoiRoleKind::Eye) {
            eye_frames += 1;
        }
        if roles.intersects(names, AoiRoleKind::Mouth) {
            mouth_frames += 1;
        }

        let session_sec = frame.gaze.video_time_sec;
        if first_orient_sec.is_none() && roles.intersects(names, AoiRoleKind::Orienting) {
            first_orient_sec = Some(session_sec);
        }

        let smile = smile_of(frame);
        smile_sum += smile;
        peak_smile = swift_max(peak_smile, smile);
        surprise_sum += surprise_of(frame);
        expressivity_sum += expressivity_of(frame);
        if session_sec <= 0.5 {
            baseline_smile_sum += smile;
            baseline_count += 1;
        }
    }

    let social_dwell = social_frames as f64 * FRAME_INTERVAL_SEC;
    let geometric_dwell = geometric_frames as f64 * FRAME_INTERVAL_SEC;
    let total_pref = social_dwell + geometric_dwell;
    let preference = if total_pref > 0.0 {
        (social_dwell - geometric_dwell) / total_pref
    } else {
        0.0
    };

    let face_feature_frames = eye_frames + mouth_frames;
    let (eye_ratio, mouth_ratio) = if face_feature_frames > 0 {
        (
            Some(eye_frames as f64 / face_feature_frames as f64),
            Some(mouth_frames as f64 / face_feature_frames as f64),
        )
    } else {
        (None, None)
    };

    let count = valid_frames.len() as f64;
    let mean_smile = smile_sum / count;
    let baseline_mean = if baseline_count > 0 {
        baseline_smile_sum / baseline_count as f64
    } else {
        mean_smile
    };
    let mean_expressivity = expressivity_sum / count;

    SessionAggregates {
        social_dwell_sec: social_dwell,
        geometric_dwell_sec: geometric_dwell,
        social_preference_score: preference,
        eye_dwell_ratio: eye_ratio,
        mouth_dwell_ratio: mouth_ratio,
        first_orient_latency_ms: first_orient_sec.map(|s| swift_max(0.0, s) * 1000.0),
        social_smile_intensity_mean: mean_smile,
        smile_reactivity_index: clamp01(peak_smile - baseline_mean),
        flat_affect_score: clamp01(1.0 - (mean_expressivity * 50.0)),
        surprise_score_mean: surprise_sum / count,
    }
}
