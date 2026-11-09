//! Fixation, saccade, blink and track-loss events of one clip (ADR-0015 V-1, R-6).
//!
//! Rules:
//! - **Fixations** are found among the clip's valid frames (in order). I-VT is
//!   [`crate::metrics`]'s own detector, so with I-VT the replay draws exactly the fixations
//!   `fixation_count` counts ([`crate::metrics::fixations_with`]); like the metric, a run may
//!   bridge an invalid stretch whose ends are close in place. I-DT (Salvucci and Goldberg 2000)
//!   grows a window of at least `minFixationDurationSec` while its dispersion
//!   `(max x - min x) + (max y - min y)` stays at or below `idtDispersionNorm`; it never spans a
//!   backwards or repeated timestamp. Times are the first and last member frame; the point is
//!   the members' mean; `dispersion` is the members' dispersion (same formula, both
//!   algorithms).
//! - **Saccades** join two consecutive fixations when every frame between them is valid and
//!   time runs forward: from the last frame of the first to the first frame of the second;
//!   `amplitudeNorm` is the distance of the two fixation points and `peakVelocity` the largest
//!   sample-to-sample speed on that stretch (normalised units per second).
//! - **Blinks and loss**: each maximal run of invalid frames spans from its first frame to the
//!   next valid frame (or, at the clip end or a backwards clock, to its last frame plus
//!   `fallbackFrameIntervalSec`). A run lasting `blinkMinSec..=blinkMaxSec` is a blink, any
//!   other a loss.
//! - Events are sorted by start time (stable: fixations, saccades, then invalid runs at a tie).

use serde::{Deserialize, Serialize};

use crate::analysis::{AnalysisProfile, FixationAlgorithm, MetricsParams};
use crate::metrics::{self, group_indices, is_valid_frame, media_time, sum};
use crate::types::GazeFrame;

/// The event detector's parameters, all read from the scoring profile's `metrics` section
/// (registry ids `metrics.fixationAlgorithm`, `metrics.ivtVelocityUnitsPerSec`,
/// `metrics.minFixationDurationSec`, `metrics.idtDispersionNorm`,
/// `metrics.fallbackFrameIntervalSec`, `metrics.blinkMinSec`, `metrics.blinkMaxSec`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventParams {
    /// Fixation detector.
    pub algorithm: FixationAlgorithm,
    /// I-VT threshold, screen-normalised units per second.
    pub ivt_velocity_units_per_sec: f64,
    /// Shortest fixation (both algorithms), seconds.
    pub min_fixation_duration_sec: f64,
    /// I-DT dispersion threshold, screen-normalised.
    pub idt_dispersion_norm: f64,
    /// Frame interval assumed where timestamps give none, seconds.
    pub fallback_frame_interval_sec: f64,
    /// Shortest invalid run drawn as a blink, seconds.
    pub blink_min_sec: f64,
    /// Longest invalid run drawn as a blink, seconds.
    pub blink_max_sec: f64,
}

impl Default for EventParams {
    fn default() -> Self {
        Self::from(&MetricsParams::default())
    }
}

impl From<&MetricsParams> for EventParams {
    fn from(m: &MetricsParams) -> Self {
        Self {
            algorithm: m.fixation_algorithm,
            ivt_velocity_units_per_sec: m.ivt_velocity_units_per_sec,
            min_fixation_duration_sec: m.min_fixation_duration_sec,
            idt_dispersion_norm: m.idt_dispersion_norm,
            fallback_frame_interval_sec: m.fallback_frame_interval_sec,
            blink_min_sec: m.blink_min_sec,
            blink_max_sec: m.blink_max_sec,
        }
    }
}

impl From<&AnalysisProfile> for EventParams {
    fn from(p: &AnalysisProfile) -> Self {
        Self::from(&p.metrics)
    }
}

impl EventParams {
    /// The `metrics` section the I-VT of [`crate::metrics`] reads.
    fn ivt_metrics(&self) -> MetricsParams {
        MetricsParams {
            ivt_velocity_units_per_sec: self.ivt_velocity_units_per_sec,
            min_fixation_duration_sec: self.min_fixation_duration_sec,
            fallback_frame_interval_sec: self.fallback_frame_interval_sec,
            ..MetricsParams::default()
        }
    }
}

/// A screen-normalised point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormPoint {
    /// Horizontal, 0 = left edge.
    pub x_norm: f64,
    /// Vertical, 0 = top edge.
    pub y_norm: f64,
}

/// One replay event; JSON `{"kind": "fixation" | "saccade" | "blink" | "lost", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GazeEvent {
    /// A fixation.
    #[serde(rename_all = "camelCase")]
    Fixation {
        /// Media time of the first member frame, seconds.
        start_sec: f64,
        /// Media time of the last member frame, seconds.
        end_sec: f64,
        /// Mean x of the members.
        x_norm: f64,
        /// Mean y of the members.
        y_norm: f64,
        /// `(max x - min x) + (max y - min y)` of the members.
        dispersion: f64,
    },
    /// A saccade between two fixations.
    #[serde(rename_all = "camelCase")]
    Saccade {
        /// Last frame of the fixation it leaves, seconds.
        start_sec: f64,
        /// First frame of the fixation it reaches, seconds.
        end_sec: f64,
        /// Point of the fixation it leaves.
        from: NormPoint,
        /// Point of the fixation it reaches.
        to: NormPoint,
        /// Distance from `from` to `to`, screen-normalised.
        amplitude_norm: f64,
        /// Largest sample-to-sample speed, normalised units per second.
        peak_velocity: f64,
    },
    /// An invalid run of blink length.
    #[serde(rename_all = "camelCase")]
    Blink {
        /// First invalid frame, seconds.
        start_sec: f64,
        /// Next valid frame (or the fallback end), seconds.
        end_sec: f64,
    },
    /// Any other invalid run (track loss, off-screen, rejected samples).
    #[serde(rename_all = "camelCase")]
    Lost {
        /// First invalid frame, seconds.
        start_sec: f64,
        /// Next valid frame (or the fallback end), seconds.
        end_sec: f64,
    },
}

impl GazeEvent {
    /// Start of the event, seconds.
    pub fn start_sec(&self) -> f64 {
        match self {
            Self::Fixation { start_sec, .. }
            | Self::Saccade { start_sec, .. }
            | Self::Blink { start_sec, .. }
            | Self::Lost { start_sec, .. } => *start_sec,
        }
    }
}

/// The events of one clip (`stimulusId ?? "session_untagged"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipEvents {
    /// The clip id.
    pub stimulus_id: String,
    /// Its events, by start time.
    pub events: Vec<GazeEvent>,
}

/// [`events`] per clip, clips in first-appearance order (the grouping of the metrics).
pub fn events_by_clip(frames: &[GazeFrame], params: &EventParams) -> Vec<ClipEvents> {
    group_indices(frames)
        .into_iter()
        .map(|(id, indices)| {
            let clip: Vec<GazeFrame> = indices.into_iter().map(|i| frames[i].clone()).collect();
            ClipEvents {
                stimulus_id: id.to_owned(),
                events: events(&clip, params),
            }
        })
        .collect()
}

/// The events of one clip's frames (see the module rules).
pub fn events(frames: &[GazeFrame], params: &EventParams) -> Vec<GazeEvent> {
    let valid_idx: Vec<usize> = (0..frames.len())
        .filter(|&i| is_valid_frame(&frames[i]))
        .collect();
    let refs: Vec<&GazeFrame> = valid_idx.iter().map(|&i| &frames[i]).collect();
    let runs: Vec<(usize, usize)> = match params.algorithm {
        FixationAlgorithm::Ivt if refs.len() > 1 => metrics::ivt_runs(&refs, &params.ivt_metrics())
            .into_iter()
            .map(|r| (r.start, r.end))
            .collect(),
        FixationAlgorithm::Ivt => Vec::new(),
        FixationAlgorithm::Idt => idt_runs(&refs, params),
    };

    let mut out = Vec::new();
    let centers: Vec<NormPoint> = runs
        .iter()
        .map(|&(s, e)| {
            let members = &refs[s..e];
            let n = members.len() as f64;
            let center = NormPoint {
                x_norm: sum(members.iter().map(|f| f.gaze.x_norm)) / n,
                y_norm: sum(members.iter().map(|f| f.gaze.y_norm)) / n,
            };
            out.push(GazeEvent::Fixation {
                start_sec: media_time(members[0]),
                end_sec: media_time(members[members.len() - 1]),
                x_norm: center.x_norm,
                y_norm: center.y_norm,
                dispersion: Extent::of(members).dispersion(),
            });
            center
        })
        .collect();

    for k in 1..runs.len() {
        let last = runs[k - 1].1 - 1;
        let first = runs[k].0;
        // Every frame between the two fixations is valid.
        if valid_idx[first] - valid_idx[last] != first - last {
            continue;
        }
        let Some(peak) = peak_velocity(&refs[last..=first]) else {
            continue;
        };
        let (from, to) = (centers[k - 1], centers[k]);
        out.push(GazeEvent::Saccade {
            start_sec: media_time(refs[last]),
            end_sec: media_time(refs[first]),
            from,
            to,
            amplitude_norm: distance(from.x_norm, from.y_norm, to.x_norm, to.y_norm),
            peak_velocity: peak,
        });
    }

    invalid_runs(frames, params, &mut out);
    out.sort_by(|a, b| a.start_sec().total_cmp(&b.start_sec()));
    out
}

fn distance(x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt()
}

/// Largest sample-to-sample speed; `None` when time does not run forward throughout.
fn peak_velocity(frames: &[&GazeFrame]) -> Option<f64> {
    let mut peak = 0.0_f64;
    for pair in frames.windows(2) {
        let dt = media_time(pair[1]) - media_time(pair[0]);
        if dt.is_nan() || dt <= 0.0 {
            return None;
        }
        let v = distance(
            pair[0].gaze.x_norm,
            pair[0].gaze.y_norm,
            pair[1].gaze.x_norm,
            pair[1].gaze.y_norm,
        ) / dt;
        peak = peak.max(v);
    }
    Some(peak)
}

/// Blink and loss events of the maximal invalid runs of `frames`.
fn invalid_runs(frames: &[GazeFrame], params: &EventParams, out: &mut Vec<GazeEvent>) {
    let mut i = 0;
    while i < frames.len() {
        if is_valid_frame(&frames[i]) {
            i += 1;
            continue;
        }
        let a = i;
        while i < frames.len() && !is_valid_frame(&frames[i]) {
            i += 1;
        }
        let start_sec = media_time(&frames[a]);
        let fallback_end = media_time(&frames[i - 1]) + params.fallback_frame_interval_sec;
        let end_sec = match frames.get(i).map(media_time) {
            Some(next) if next > start_sec => next,
            _ => fallback_end,
        };
        let duration = end_sec - start_sec;
        out.push(
            if duration >= params.blink_min_sec && duration <= params.blink_max_sec {
                GazeEvent::Blink { start_sec, end_sec }
            } else {
                GazeEvent::Lost { start_sec, end_sec }
            },
        );
    }
}

/// Bounding extent of a set of gaze points.
#[derive(Debug, Clone, Copy)]
struct Extent {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl Extent {
    fn of(frames: &[&GazeFrame]) -> Self {
        let mut e = Extent {
            min_x: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            min_y: f64::INFINITY,
            max_y: f64::NEG_INFINITY,
        };
        for f in frames {
            e = e.with(f);
        }
        e
    }

    fn with(self, f: &GazeFrame) -> Self {
        let (x, y) = (f.gaze.x_norm, f.gaze.y_norm);
        Extent {
            min_x: self.min_x.min(x),
            max_x: self.max_x.max(x),
            min_y: self.min_y.min(y),
            max_y: self.max_y.max(y),
        }
    }

    fn dispersion(&self) -> f64 {
        (self.max_x - self.min_x) + (self.max_y - self.min_y)
    }
}

/// I-DT runs `(start, end_exclusive)` over `frames`, split where time does not run forward.
fn idt_runs(frames: &[&GazeFrame], params: &EventParams) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut seg_start = 0;
    for k in 1..=frames.len() {
        let breaks = k == frames.len() || {
            let dt = media_time(frames[k]) - media_time(frames[k - 1]);
            dt.is_nan() || dt <= 0.0
        };
        if breaks {
            idt_segment(frames, seg_start, k, params, &mut out);
            seg_start = k;
        }
    }
    out
}

fn idt_segment(
    frames: &[&GazeFrame],
    lo: usize,
    hi: usize,
    params: &EventParams,
    out: &mut Vec<(usize, usize)>,
) {
    let t = |i: usize| media_time(frames[i]);
    let max_dispersion = params.idt_dispersion_norm;
    let mut i = lo;
    while i < hi {
        // The shortest window from `i` of at least two frames and the minimum duration.
        let mut j = i + 1;
        while j < hi && t(j) - t(i) < params.min_fixation_duration_sec {
            j += 1;
        }
        if j >= hi {
            break;
        }
        let mut extent = Extent::of(&frames[i..=j]);
        if extent.dispersion() <= max_dispersion {
            while j + 1 < hi {
                let grown = extent.with(frames[j + 1]);
                if grown.dispersion() > max_dispersion {
                    break;
                }
                extent = grown;
                j += 1;
            }
            out.push((i, j + 1));
            i = j + 1;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{frame, invalid, t};
    use super::*;
    use crate::analysis::MetricsParams;
    use crate::metrics::fixations_with;

    /// 0.5 s at (0.2, 0.3), a 3-frame jump to (0.8, 0.3), 0.5 s there, then 12 invalid
    /// frames (200 ms: a blink), 0.5 s at (0.8, 0.3) again, 60 invalid frames (1 s: loss).
    fn fixture() -> Vec<GazeFrame> {
        let mut f = Vec::new();
        let mut i = 0;
        for _ in 0..30 {
            f.push(frame(t(i), 0.2, 0.3));
            i += 1;
        }
        for k in 1..=3 {
            f.push(frame(t(i), 0.2 + 0.15 * k as f64, 0.3));
            i += 1;
        }
        for _ in 0..30 {
            f.push(frame(t(i), 0.8, 0.3));
            i += 1;
        }
        for _ in 0..12 {
            f.push(invalid(t(i)));
            i += 1;
        }
        for _ in 0..30 {
            f.push(frame(t(i), 0.8, 0.3));
            i += 1;
        }
        for _ in 0..60 {
            f.push(invalid(t(i)));
            i += 1;
        }
        f
    }

    fn kinds(ev: &[GazeEvent]) -> Vec<&'static str> {
        ev.iter()
            .map(|e| match e {
                GazeEvent::Fixation { .. } => "fixation",
                GazeEvent::Saccade { .. } => "saccade",
                GazeEvent::Blink { .. } => "blink",
                GazeEvent::Lost { .. } => "lost",
            })
            .collect()
    }

    #[test]
    fn ivt_events_are_known_and_match_the_metric_fixations() {
        let frames = fixture();
        let ev = events(&frames, &EventParams::default());
        // I-VT bridges the blink (same place on both sides), like fixation_count.
        assert_eq!(
            kinds(&ev),
            ["fixation", "saccade", "fixation", "blink", "lost"]
        );

        let GazeEvent::Fixation {
            start_sec,
            end_sec,
            x_norm,
            y_norm,
            dispersion,
        } = ev[0]
        else {
            panic!()
        };
        assert_eq!((start_sec, end_sec), (t(0), t(29)));
        assert!((x_norm - 0.2).abs() < 1e-12 && (y_norm - 0.3).abs() < 1e-12);
        assert_eq!(dispersion, 0.0);

        // Jump: 0.2 -> 0.35 -> 0.5 -> 0.65 -> 0.8, each step 0.15 in 1/60 s = 9 units/s.
        let GazeEvent::Saccade {
            start_sec,
            end_sec,
            from,
            to,
            amplitude_norm,
            peak_velocity,
        } = ev[1]
        else {
            panic!()
        };
        assert_eq!((start_sec, end_sec), (t(29), t(33)));
        assert!((from.x_norm - 0.2).abs() < 1e-12 && (to.x_norm - 0.8).abs() < 1e-12);
        assert!((amplitude_norm - 0.6).abs() < 1e-12);
        assert!((peak_velocity - 9.0).abs() < 1e-9);

        let GazeEvent::Fixation {
            start_sec, end_sec, ..
        } = ev[2]
        else {
            panic!()
        };
        assert_eq!((start_sec, end_sec), (t(33), t(104)));

        assert_eq!(
            ev[3],
            GazeEvent::Blink {
                start_sec: t(63),
                end_sec: t(75)
            }
        );
        let GazeEvent::Lost { start_sec, end_sec } = ev[4] else {
            panic!()
        };
        assert_eq!(start_sec, t(105));
        assert!((end_sec - (t(164) + 1.0 / 60.0)).abs() < 1e-12);

        // Same runs as the metrics' plotting fixations.
        let fx = fixations_with(&frames, &MetricsParams::default());
        let ours: Vec<(f64, f64, f64, f64)> = ev
            .iter()
            .filter_map(|e| match *e {
                GazeEvent::Fixation {
                    start_sec,
                    end_sec,
                    x_norm,
                    y_norm,
                    ..
                } => Some((start_sec, end_sec, x_norm, y_norm)),
                _ => None,
            })
            .collect();
        let theirs: Vec<(f64, f64, f64, f64)> = fx
            .iter()
            .map(|f| (f.start_sec, f.end_sec, f.x_norm, f.y_norm))
            .collect();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn idt_on_the_same_fixture_finds_the_same_events() {
        let frames = fixture();
        let params = EventParams {
            algorithm: FixationAlgorithm::Idt,
            ..EventParams::default()
        };
        let ev = events(&frames, &params);
        // Windows starting at the jump frames 30, 31, 32 are wider than 0.05; from 33 the
        // window stays at (0.8, 0.3) and, time running forward over the blink, grows to 104.
        assert_eq!(
            kinds(&ev),
            ["fixation", "saccade", "fixation", "blink", "lost"]
        );
        let GazeEvent::Fixation {
            start_sec, end_sec, ..
        } = ev[0]
        else {
            panic!()
        };
        // I-DT keeps 0..=29 (the next point, 0.35, would make the dispersion 0.15).
        assert_eq!((start_sec, end_sec), (t(0), t(29)));
        let GazeEvent::Fixation {
            start_sec, end_sec, ..
        } = ev[2]
        else {
            panic!()
        };
        assert_eq!((start_sec, end_sec), (t(33), t(104)));
    }

    #[test]
    fn ivt_and_idt_differ_on_slow_drift() {
        // A slow drift at 1 unit/s (below the 5 u/s I-VT threshold) for 0.5 s: one I-VT
        // fixation spanning 0.5 units. For I-DT the shortest 80 ms window (5 intervals)
        // already spans 0.083 units > 0.05, so no fixation at all.
        let frames: Vec<GazeFrame> = (0..31)
            .map(|i| frame(t(i), 0.1 + i as f64 / 60.0, 0.5))
            .collect();
        let ivt = events(&frames, &EventParams::default());
        assert_eq!(kinds(&ivt), ["fixation"]);
        let GazeEvent::Fixation { dispersion, .. } = ivt[0] else {
            panic!()
        };
        assert!((dispersion - 0.5).abs() < 1e-12);

        let idt = events(
            &frames,
            &EventParams {
                algorithm: FixationAlgorithm::Idt,
                ..EventParams::default()
            },
        );
        assert!(idt.is_empty(), "{idt:?}");

        // A wider dispersion threshold (0.2) gives windows of 13 frames (0.2 units, 200 ms):
        // frames 0..=12, 13..=25, and 26..=30 is too short (66 ms).
        let wide = events(
            &frames,
            &EventParams {
                algorithm: FixationAlgorithm::Idt,
                idt_dispersion_norm: 0.2 + 1e-9,
                ..EventParams::default()
            },
        );
        assert_eq!(kinds(&wide), ["fixation", "saccade", "fixation"]);
        let GazeEvent::Fixation {
            start_sec, end_sec, ..
        } = wide[2]
        else {
            panic!()
        };
        assert_eq!((start_sec, end_sec), (t(13), t(25)));
    }

    #[test]
    fn blink_bounds_come_from_the_params() {
        let mut frames: Vec<GazeFrame> = (0..10).map(|i| frame(t(i), 0.5, 0.5)).collect();
        frames.extend((10..14).map(|i| invalid(t(i)))); // 67 ms
        frames.extend((14..20).map(|i| frame(t(i), 0.5, 0.5)));
        let ev = events(&frames, &EventParams::default());
        assert!(matches!(ev.last(), Some(GazeEvent::Blink { .. })), "{ev:?}");
        let strict = EventParams {
            blink_min_sec: 0.07,
            ..EventParams::default()
        };
        let ev = events(&frames, &strict);
        assert!(matches!(ev.last(), Some(GazeEvent::Lost { .. })), "{ev:?}");
    }

    #[test]
    fn params_come_from_the_profile_metrics() {
        let mut p = AnalysisProfile::default();
        p.metrics.ivt_velocity_units_per_sec = 2.0;
        p.metrics.fixation_algorithm = FixationAlgorithm::Idt;
        p.metrics.idt_dispersion_norm = 0.1;
        let e = EventParams::from(&p);
        assert_eq!(e.algorithm, FixationAlgorithm::Idt);
        assert_eq!(e.ivt_velocity_units_per_sec, 2.0);
        assert_eq!(e.idt_dispersion_norm, 0.1);
        assert_eq!(e.min_fixation_duration_sec, 0.08);
        // Defaults reproduce today's profile JSON exactly (no new keys).
        let json = serde_json::to_value(MetricsParams::default()).unwrap();
        assert!(json.get("fixationAlgorithm").is_none());
        let decoded: MetricsParams =
            serde_json::from_value(serde_json::json!({"fixationAlgorithm": "idt"})).unwrap();
        assert_eq!(decoded.fixation_algorithm, FixationAlgorithm::Idt);
    }

    #[test]
    fn events_json_is_tagged_camel_case() {
        let ev = GazeEvent::Saccade {
            start_sec: 1.0,
            end_sec: 1.5,
            from: NormPoint {
                x_norm: 0.0,
                y_norm: 0.0,
            },
            to: NormPoint {
                x_norm: 1.0,
                y_norm: 0.0,
            },
            amplitude_norm: 1.0,
            peak_velocity: 3.0,
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "saccade");
        assert_eq!(v["amplitudeNorm"], 1.0);
        assert_eq!(v["from"]["xNorm"], 0.0);
        assert_eq!(serde_json::from_value::<GazeEvent>(v).unwrap(), ev);
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        assert!(events(&[], &EventParams::default()).is_empty());
        let one = [frame(0.0, 0.5, 0.5)];
        assert!(events(&one, &EventParams::default()).is_empty());
        let backwards = [
            frame(1.0, 0.5, 0.5),
            frame(0.5, 0.5, 0.5),
            invalid(0.4),
            frame(f64::NAN, f64::NAN, 0.5),
        ];
        for algorithm in [FixationAlgorithm::Ivt, FixationAlgorithm::Idt] {
            let _ = events(
                &backwards,
                &EventParams {
                    algorithm,
                    ..EventParams::default()
                },
            );
        }
        let by_clip = events_by_clip(&fixture(), &EventParams::default());
        assert_eq!(by_clip.len(), 1);
        assert_eq!(by_clip[0].stimulus_id, "clip");
    }
}
