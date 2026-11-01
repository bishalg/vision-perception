//! owner: WP-G.3
//!
//! QS-3 / CNF-3 per-clip tracking values ([`ClipTracking`]) as pure functions over one clip's
//! frames; `GazePipeline` (`begin_clip` / `end_clip` / `clip_tracking`) collects the frames
//! and the drift check that follows the clip.
//!
//! # Definitions
//! - **`meanSampleConfidence`** (CNF-3, QS-3): mean CNF-1 confidence over the clip's `tracked`
//!   frames, the session `confidence.meanSample` rule (QS-1) applied to the clip: loss is
//!   reported by state, not double-counted as zero confidence. A clip with frames but no
//!   `tracked` frame has 0; a clip without frames has no value.
//! - **`meanFixationConfidence`** (CNF-3, QS-3): the clip's frames are gap filled (GAP-1) and
//!   segmented into fixations by [`gapfill::fixations`] (I-VT in degrees per second at
//!   `scoring.ivtVelocityDegPerSec`, 80 ms minimum), exactly as for the session
//!   `confidence.meanFixation`, but within the clip only (a fixation never spans two clips).
//!   Each fixation's confidence is the mean over its measured samples; the clip value is the
//!   mean of those. Absent without a fixation.
//! - **`aoiMinSizeDeg`** (QS-3): over the clip's role-bearing AOIs ([`is_role_bearing`]), the
//!   smallest of each box's width and height, converted to millimetres with the clip's
//!   oriented screen geometry and to degrees seen on axis from the session's median distance
//!   `d` (`conditions.distanceCmMedian`): `2 · atan(size / 2d)`. Absent without a role-bearing
//!   AOI, or without a positive median distance.
//! - **`nearestCheckOffsetDeg`** (QS-3, DRF-1/DRF-2): see `GazePipeline::clip_tracking`.

use super::aoi_space::ScreenAoi;
use super::confidence;
use super::gapfill::{self, GapSample};
use super::geometry::OrientedGeometry;
use crate::metrics::{AoiRole, AoiRoleKind};
use crate::types::tracking::GapFillParams;

const ROLE_KINDS: [AoiRoleKind; 5] = [
    AoiRoleKind::Social,
    AoiRoleKind::Geometric,
    AoiRoleKind::Eye,
    AoiRoleKind::Mouth,
    AoiRoleKind::Orienting,
];

/// An AOI is role-bearing when scoring would give it a role (`AoiRole::matches`): one of its
/// catalog `roles` is a known role (`social`, `geometric`, `eye`, `mouth`, `orienting`), or
/// its name is in a built-in role set. Other AOIs (distractors, background) carry no metric.
pub fn is_role_bearing(aoi: &ScreenAoi) -> bool {
    ROLE_KINDS.iter().any(|k| {
        aoi.roles.iter().any(|r| r == k.as_str())
            || AoiRole::builtin_names(*k).contains(&aoi.name.as_str())
    })
}

/// `meanSampleConfidence` (module docs).
pub fn mean_sample_confidence(frames: &[GapSample]) -> Option<f64> {
    if frames.is_empty() {
        return None;
    }
    let (sum, n) = frames
        .iter()
        .filter(|f| f.tracked)
        .fold((0.0, 0usize), |(s, n), f| (s + f.confidence, n + 1));
    Some(if n == 0 { 0.0 } else { sum / n as f64 })
}

/// `meanFixationConfidence` (module docs): fills the clip's gaps, then segments.
pub fn mean_fixation_confidence(
    frames: &[GapSample],
    geo: &OrientedGeometry,
    gap_fill: &GapFillParams,
    ivt_velocity_deg_per_sec: f64,
    min_duration_sec: f64,
) -> Option<f64> {
    let filled = gapfill::fill(frames, geo, gap_fill);
    let fixations = gapfill::fixations(&filled, geo, ivt_velocity_deg_per_sec, min_duration_sec);
    confidence::fixation(fixations.iter().filter_map(|f| f.mean_confidence))
}

/// Visual angle (degrees) of a length `size_mm` centred on the line of sight at `distance_mm`.
fn on_axis_deg(size_mm: f64, distance_mm: f64) -> f64 {
    2.0 * (size_mm / 2.0).atan2(distance_mm).to_degrees()
}

/// `aoiMinSizeDeg` (module docs). `distance_cm` is the session's median distance.
pub fn aoi_min_size_deg(
    aois: &[ScreenAoi],
    geo: &OrientedGeometry,
    distance_cm: Option<f64>,
) -> Option<f64> {
    let d_mm = distance_cm.filter(|d| d.is_finite() && *d > 0.0)? * 10.0;
    aois.iter()
        .filter(|a| is_role_bearing(a))
        .map(|a| {
            let w = (a.rect.x_max - a.rect.x_min).abs() * geo.width_mm;
            let h = (a.rect.y_max - a.rect.y_min).abs() * geo.height_mm;
            on_axis_deg(w.min(h), d_mm)
        })
        .filter(|v| v.is_finite())
        .reduce(f64::min)
}
