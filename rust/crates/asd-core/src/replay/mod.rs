//! owner: WP-X.19
//!
//! Gaze replay core (ADR-0015 section E, V-1..V-5, V-9; R-6): everything the console's replay,
//! expected-versus-actual, heatmap and outlier views draw is computed here and reaches the
//! console through `asd-wasm` (one session) or the backend job (cohorts). The console only
//! draws.
//!
//! - [`events()`]: fixations, saccades, blinks and track loss from a clip's frames with the
//!   profile's parameters ([`EventParams`], I-VT by default, I-DT selectable);
//! - [`aoi_ribbon()`]: which AOI holds the gaze at every instant (time-coded boxes honoured);
//! - [`windowed_trace()`] and [`expected_bands()`]: a sliding-window metric over clip time and
//!   the published baseline band it is read against;
//! - [`heatmap_grid()`], [`cohort_grid()`], [`difference()`]: Gaussian KDE grids;
//! - [`outliers::screen()`]: dataset outlier and quality rules.
//!
//! Conventions shared by every function:
//! - **Time** is the media clock, `clipMediaTimeSec ?? videoTimeSec` ([`metrics::media_time`]);
//!   the per-clip functions expect the frames of one clip ([`clip_frames`] selects them).
//! - **Valid** is [`metrics::is_valid_frame`] (`gaze.valid && qualityVerdict != rejected`), the
//!   same rule the metrics use.
//! - **AOI hits** are tested on the catalog boxes in screen space (trap 11, as
//!   `gaze::reprocess` does), grown by `aoi.marginNorm`, inclusive in space and time
//!   ([`crate::gaze::aoi_space::ScreenAoi::contains`]). With no areas, a frame's recorded
//!   `activeAOINames` are used and roles come from the built-in name sets.
//! - **Frame span**: a frame covers the time up to the next frame when that interval is
//!   positive and at most `aoi.dwellCapMs` (PLC-5), else `min(fallbackFrameIntervalSec,
//!   dwellCap)`; longer gaps stay uncovered.
//! - Pure and deterministic: no clock, no I/O, no randomness. JSON is camelCase like the rest
//!   of the core; label texts are message keys, never English.
//!
//! [`metrics::media_time`]: crate::metrics::media_time
//! [`metrics::is_valid_frame`]: crate::metrics::is_valid_frame

mod aoi;
mod events;
mod heatmap;
pub mod outliers;
mod trace;

use thiserror::Error;

use crate::types::GazeFrame;

pub use aoi::{aoi_ribbon, HitParams, RibbonSegment};
pub use events::{events, events_by_clip, ClipEvents, EventParams, GazeEvent, NormPoint};
pub use heatmap::{cohort_grid, difference, heatmap_grid, Grid, COHORT_MIN_N, MAX_GRID_SIDE};
pub use outliers::{screen, Exclusion, MetricRow, Rule, RuleOutcome, Screening};
pub use trace::{
    expected_bands, expected_bands_for_age, windowed_trace, BandThreshold, ExpectedBand, Trace,
    TraceMetric, TracePoint, MAX_TRACE_POINTS,
};

/// A replay input the functions cannot work with (never a panic).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReplayError {
    /// A scalar argument or rule is out of range; the text names it.
    #[error("{0}")]
    InvalidArgument(String),
}

/// The frames of clip `clip_id`, in order: those whose `stimulusId` equals it, or every frame
/// when no frame carries a `stimulusId` (an untagged single-clip capture).
pub fn clip_frames(frames: &[GazeFrame], clip_id: &str) -> Vec<GazeFrame> {
    if frames.iter().all(|f| f.stimulus_id.is_none()) {
        return frames.to_vec();
    }
    frames
        .iter()
        .filter(|f| f.stimulus_id.as_deref() == Some(clip_id))
        .cloned()
        .collect()
}

#[cfg(test)]
pub(crate) mod testutil {
    use crate::types::{Area, GazeFrame, GazeFrameAffect, GazeFrameSample, GazeQualityVerdict};

    /// A valid 60 Hz frame at media time `t`.
    pub fn frame(t: f64, x: f64, y: f64) -> GazeFrame {
        GazeFrame {
            gaze: GazeFrameSample {
                is_valid: true,
                x_norm: x,
                y_norm: y,
                video_time_sec: t,
                clip_media_time_sec: Some(t),
            },
            affect: GazeFrameAffect::default(),
            active_aoi_names: Vec::new(),
            quality_verdict: GazeQualityVerdict::Optimal,
            stimulus_id: Some("clip".into()),
        }
    }

    /// An invalid frame at media time `t`.
    pub fn invalid(t: f64) -> GazeFrame {
        let mut f = frame(t, 0.0, 0.0);
        f.gaze.is_valid = false;
        f
    }

    /// Frame `i` of a 60 Hz clock (exact multiples, so spans are reproducible).
    pub fn t(i: usize) -> f64 {
        i as f64 / 60.0
    }

    pub fn area(name: &str, t: (f64, f64), b: [f64; 4], roles: &[&str]) -> Area {
        Area {
            name: name.into(),
            start_sec: t.0,
            end_sec: t.1,
            x_min: b[0],
            y_min: b[1],
            x_max: b[2],
            y_max: b[3],
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::frame;
    use super::*;

    #[test]
    fn clip_frames_selects_by_id_or_takes_untagged_captures_whole() {
        let mut a = frame(0.0, 0.1, 0.1);
        a.stimulus_id = Some("a".into());
        let mut b = frame(0.1, 0.1, 0.1);
        b.stimulus_id = Some("b".into());
        let mut none = frame(0.2, 0.1, 0.1);
        none.stimulus_id = None;
        let all = vec![a.clone(), b, none.clone()];
        assert_eq!(clip_frames(&all, "a"), vec![a]);
        assert!(clip_frames(&all, "zzz").is_empty());
        let untagged = vec![none.clone(), none];
        assert_eq!(clip_frames(&untagged, "anything").len(), 2);
    }
}
