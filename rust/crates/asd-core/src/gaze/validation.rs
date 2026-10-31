//! owner: WP-G.4
//!
//! Target-window statistics and the validation summary (VAL-2..VAL-4; technical.md
//! "Definitions"). Pure functions: the caller (`GazePipeline::end_target`, WP-G.3) hands over
//! the processed samples it buffered for the window.
//!
//! Conventions (decided here, used by validation and drift checks alike):
//! - The **window** is `[started_at_sec, ended_at_sec]`; samples before
//!   `started_at_sec + settle_ms` are dropped (settle). Every count, loss and statistic is over
//!   the remaining **collection** part.
//! - A sample is **usable** when it is `valid`, not `gap_filled` (GAP-2) and carries both a
//!   calibrated point and an eye position. Only usable samples count as valid samples.
//! - Angles are measured from the window's **median eye position** (per-axis median over the
//!   usable samples), in the session's oriented geometry (trap 12: the caller passes the one
//!   session geometry).
//! - **Accuracy** is the visual angle between the target and the per-axis median of the
//!   calibrated points; the signed **offset** vector is that offset split into x (right
//!   positive) and y (down positive) components, each an exact visual angle.
//! - **RMS-S2S** uses pairs of samples that are adjacent in the collection sequence and both
//!   usable (a lost frame in between breaks the pair): on the filtered points for
//!   `precision_rms_deg`, on the calibrated points for `precision_rms_raw_deg`. **SD** is
//!   `sqrt(var_x + var_y)` (population variance) of the filtered points expressed as signed
//!   degree offsets from their centroid.
//! - **Data loss** is the share of collection time not `tracked`: each frame owns the time to
//!   the next frame (the last one to the window end); a window without frames has loss 1.
//! - Points are never clamped (TRK-2): an off-screen median stays off-screen.

use crate::gaze::geometry::{angle_between_norm_deg, OrientedGeometry};
use crate::norms::percentile;
use crate::types::tracking::{
    DriftParams, ProcessedSample, RecalAction, TargetKind, TargetOutcome, TargetResult, TrackState,
    ValidationParams, ValidationRecord, ValidationTargetRecord,
};

/// One target window as shown on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetWindow {
    pub kind: TargetKind,
    /// Target position, screen-normalised.
    pub target_norm: [f64; 2],
    pub started_at_sec: f64,
    pub ended_at_sec: f64,
    pub settle_ms: f64,
    /// Fewer usable samples than this → `missed`.
    pub min_samples: u32,
    /// 1 for the first showing.
    pub attempt: u32,
}

impl TargetWindow {
    /// A validation target (VAL-1, VAL-2 timings and minimum).
    pub fn validation(
        target_norm: [f64; 2],
        started_at_sec: f64,
        ended_at_sec: f64,
        params: &ValidationParams,
    ) -> Self {
        Self {
            kind: TargetKind::Validation,
            target_norm,
            started_at_sec,
            ended_at_sec,
            settle_ms: params.settle_ms,
            min_samples: params.min_samples_per_target,
            attempt: 1,
        }
    }

    /// A drift check target (DRF-1 timings and minimum).
    pub fn drift_check(
        target_norm: [f64; 2],
        started_at_sec: f64,
        ended_at_sec: f64,
        params: &DriftParams,
    ) -> Self {
        Self {
            kind: TargetKind::DriftCheck,
            target_norm,
            started_at_sec,
            ended_at_sec,
            settle_ms: params.settle_ms,
            min_samples: params.min_samples,
            attempt: 1,
        }
    }
}

/// Median of the finite values (mean of the middle two for an even count); `None` if empty.
pub(crate) fn median(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    quantile(values, 0.5)
}

/// Linear-interpolated quantile (`q` in 0..=1) of the finite values; `None` if empty.
pub(crate) fn quantile(values: impl IntoIterator<Item = f64>, q: f64) -> Option<f64> {
    let mut v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(percentile(&v, q))
}

/// Signed visual-angle offset from `from_norm` to `to_norm` seen from `eye_mm`, split into
/// x (right positive) and y (down positive). Each component is the exact angle between
/// `from` and `from` moved along that axis only.
pub fn signed_offset_deg(
    geo: &OrientedGeometry,
    from_norm: [f64; 2],
    to_norm: [f64; 2],
    eye_mm: [f64; 3],
) -> [f64; 2] {
    let ax = angle_between_norm_deg(geo, from_norm, [to_norm[0], from_norm[1]], eye_mm);
    let ay = angle_between_norm_deg(geo, from_norm, [from_norm[0], to_norm[1]], eye_mm);
    let sign = |d: f64| if d < 0.0 { -1.0 } else { 1.0 };
    [
        sign(to_norm[0] - from_norm[0]) * ax,
        sign(to_norm[1] - from_norm[1]) * ay,
    ]
}

fn usable(s: &ProcessedSample) -> bool {
    s.valid && !s.gap_filled && s.calibrated_point_norm.is_some() && s.eye_pos_mm.is_some()
}

fn median2(points: &[[f64; 2]]) -> Option<[f64; 2]> {
    Some([
        median(points.iter().map(|p| p[0]))?,
        median(points.iter().map(|p| p[1]))?,
    ])
}

fn median3(points: &[[f64; 3]]) -> Option<[f64; 3]> {
    Some([
        median(points.iter().map(|p| p[0]))?,
        median(points.iter().map(|p| p[1]))?,
        median(points.iter().map(|p| p[2]))?,
    ])
}

/// RMS of the angles between adjacent pairs where both points exist.
fn rms_s2s(geo: &OrientedGeometry, points: &[Option<[f64; 2]>], eye_mm: [f64; 3]) -> Option<f64> {
    let squares: Vec<f64> = points
        .windows(2)
        .filter_map(|w| match (w[0], w[1]) {
            (Some(a), Some(b)) => Some(angle_between_norm_deg(geo, a, b, eye_mm).powi(2)),
            _ => None,
        })
        .collect();
    if squares.is_empty() {
        return None;
    }
    Some((squares.iter().sum::<f64>() / squares.len() as f64).sqrt())
}

/// `sqrt(var_x + var_y)` of the points as signed degree offsets from their centroid.
fn sd_deg(geo: &OrientedGeometry, points: &[[f64; 2]], eye_mm: [f64; 3]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let n = points.len() as f64;
    let centroid = [
        points.iter().map(|p| p[0]).sum::<f64>() / n,
        points.iter().map(|p| p[1]).sum::<f64>() / n,
    ];
    let offsets: Vec<[f64; 2]> = points
        .iter()
        .map(|&p| signed_offset_deg(geo, centroid, p, eye_mm))
        .collect();
    let var = |axis: usize| {
        let mean = offsets.iter().map(|o| o[axis]).sum::<f64>() / n;
        offsets
            .iter()
            .map(|o| (o[axis] - mean).powi(2))
            .sum::<f64>()
            / n
    };
    Some((var(0) + var(1)).sqrt())
}

fn data_loss(collected: &[&ProcessedSample], end_sec: f64) -> f64 {
    let mut total = 0.0;
    let mut lost = 0.0;
    for (i, s) in collected.iter().enumerate() {
        let next = collected
            .get(i + 1)
            .map_or(end_sec, |n| n.t_sec.min(end_sec));
        let dt = (next - s.t_sec).max(0.0);
        total += dt;
        if s.track_state != TrackState::Tracked {
            lost += dt;
        }
    }
    if collected.is_empty() {
        1.0
    } else if total > 0.0 {
        lost / total
    } else {
        // Frames but no measurable time (all at the window end): count frames instead.
        let not_tracked = collected
            .iter()
            .filter(|s| s.track_state != TrackState::Tracked)
            .count();
        not_tracked as f64 / collected.len() as f64
    }
}

/// Statistics of one target window (VAL-2, DRF-2). `samples` may contain frames outside the
/// window; they are ignored. The outcome is `hit` or `missed` only: the calibration re-show
/// (`needsRetry`, CAL-3) is decided by the calibration session, not here.
pub fn target_result(
    window: &TargetWindow,
    samples: &[ProcessedSample],
    geo: &OrientedGeometry,
) -> TargetResult {
    let collect_from = window.started_at_sec + window.settle_ms / 1000.0;
    let collected: Vec<&ProcessedSample> = samples
        .iter()
        .filter(|s| s.t_sec >= collect_from && s.t_sec <= window.ended_at_sec)
        .collect();

    let usable_samples: Vec<&ProcessedSample> =
        collected.iter().copied().filter(|s| usable(s)).collect();
    let valid_samples = usable_samples.len() as u32;
    let hit = valid_samples >= window.min_samples && valid_samples > 0;

    let calibrated: Vec<[f64; 2]> = usable_samples
        .iter()
        .filter_map(|s| s.calibrated_point_norm)
        .collect();
    let raw: Vec<[f64; 2]> = usable_samples
        .iter()
        .filter_map(|s| s.raw_point_norm)
        .collect();
    let eyes: Vec<[f64; 3]> = usable_samples.iter().filter_map(|s| s.eye_pos_mm).collect();

    let calibrated_median_norm = median2(&calibrated);
    let eye_pos_mm_median = median3(&eyes);

    let mut result = TargetResult {
        kind: window.kind,
        x_norm: window.target_norm[0],
        y_norm: window.target_norm[1],
        started_at_sec: window.started_at_sec,
        ended_at_sec: window.ended_at_sec,
        attempt: window.attempt,
        outcome: if hit {
            TargetOutcome::Hit
        } else {
            TargetOutcome::Missed
        },
        valid_samples,
        total_samples: collected.len() as u32,
        data_loss: data_loss(&collected, window.ended_at_sec),
        raw_median_norm: median2(&raw),
        calibrated_median_norm,
        eye_pos_mm_median,
        accuracy_deg: None,
        offset_deg: None,
        precision_rms_deg: None,
        precision_rms_raw_deg: None,
        sd_deg: None,
    };

    // Accuracy and precision are reported for hit targets only (a missed target is
    // unmeasured, CON-14).
    if let (true, Some(gaze), Some(eye)) = (hit, calibrated_median_norm, eye_pos_mm_median) {
        let target = window.target_norm;
        result.accuracy_deg = Some(angle_between_norm_deg(geo, target, gaze, eye));
        result.offset_deg = Some(signed_offset_deg(geo, target, gaze, eye));

        let filtered_seq: Vec<Option<[f64; 2]>> = collected
            .iter()
            .map(|s| usable(s).then_some(s.filtered_point_norm).flatten())
            .collect();
        let calibrated_seq: Vec<Option<[f64; 2]>> = collected
            .iter()
            .map(|s| usable(s).then_some(s.calibrated_point_norm).flatten())
            .collect();
        result.precision_rms_deg = rms_s2s(geo, &filtered_seq, eye);
        result.precision_rms_raw_deg = rms_s2s(geo, &calibrated_seq, eye);
        let filtered: Vec<[f64; 2]> = filtered_seq.iter().flatten().copied().collect();
        result.sd_deg = sd_deg(geo, &filtered, eye);
    }
    result
}

/// Validation record from the target results in display order (VAL-2, VAL-3). `None` when no
/// validation target was shown. Accuracy and precision are medians over hit targets and are
/// absent when no target was hit; `dataLoss` is the mean over all targets.
pub fn summarize(targets: &[TargetResult], params: &ValidationParams) -> Option<ValidationRecord> {
    if targets.is_empty() {
        return None;
    }
    let hits: Vec<&TargetResult> = targets
        .iter()
        .filter(|t| t.outcome == TargetOutcome::Hit)
        .collect();
    let records = targets
        .iter()
        .map(|t| ValidationTargetRecord {
            x_norm: t.x_norm,
            y_norm: t.y_norm,
            accuracy_deg: t.accuracy_deg,
            precision_rms_deg: t.precision_rms_deg,
            sd_deg: t.sd_deg,
            data_loss: t.data_loss,
            valid_samples: t.valid_samples,
            missed: t.outcome != TargetOutcome::Hit,
        })
        .collect();
    Some(ValidationRecord {
        complete: hits.len() as u32 >= params.min_targets_hit,
        targets: records,
        accuracy_deg: median(hits.iter().filter_map(|t| t.accuracy_deg)),
        precision_rms_deg: median(hits.iter().filter_map(|t| t.precision_rms_deg)),
        precision_rms_raw_deg: median(hits.iter().filter_map(|t| t.precision_rms_raw_deg)),
        data_loss: targets.iter().map(|t| t.data_loss).sum::<f64>() / targets.len() as f64,
    })
}

/// VAL-4: above `maxAccuracyDeg` → re-calibrate; above `warnAccuracyDeg` → warn (both
/// strict). An unmeasured accuracy advises nothing here; a non-finite one re-calibrates.
pub fn accuracy_action(accuracy_deg: Option<f64>, params: &ValidationParams) -> RecalAction {
    match accuracy_deg {
        None => RecalAction::None,
        Some(a) if a.is_nan() || a > params.max_accuracy_deg => RecalAction::Recalibrate,
        Some(a) if a > params.warn_accuracy_deg => RecalAction::Warn,
        Some(_) => RecalAction::None,
    }
}
