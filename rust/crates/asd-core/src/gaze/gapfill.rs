//! owner: WP-G.3
//!
//! Gap fill, fixation segmentation and loss accounting (GAP-1..GAP-3, CNF-3).
//!
//! - A **gap** is a run of non-`tracked` frames between two `tracked` frames that carry a
//!   point and an eye position. Its duration is the **bracket interval**: from the last
//!   tracked frame before it to the first tracked frame after it. It is filled by linear
//!   interpolation in time (point and eye position) when the duration is at most
//!   `gapFill.maxMs` **and** the visual angle between the bracketing points, seen from the
//!   midpoint of their eye positions, is at most `gapFill.maxDeg` (both inclusive). A gap at
//!   the start or the end of the sequence has no bracket and is never filled.
//! - Filled frames are marked `gap_filled` and keep their track state and zero confidence:
//!   they are **not** valid, add no dwell and are never hit-tested (GAP-2, trap 9). Their only
//!   consumer is [`fixations`]; never feed them to `metrics::extract`.
//! - [`loss`] sums frame time by state (GAP-3). Each frame owns the interval to the **next**
//!   frame (the PLC-5 dwell convention of `GazeProcessingActor.finalize`), capped at
//!   `gapFill.lossIntervalCapMs`; the last frame owns nothing. `longestGapSec` is the longest
//!   bracket interval of a non-tracked run, uncapped (a run at the end is measured to the last
//!   frame, a run at the start from its first frame).

use super::geometry::{angle_deg, OrientedGeometry};
use crate::types::tracking::{GapFillParams, LossSummary, ProcessedSample, TrackState};

/// Tolerance on the duration limit, ms (frame clocks accumulate rounding error).
const EPS_MS: f64 = 1e-6;

/// The per-frame view gap filling and fixation segmentation need.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapSample {
    pub t_sec: f64,
    /// `track_state == tracked`.
    pub tracked: bool,
    /// The analysis point (filtered, calibrated, screen-normalised); interpolated when filled.
    pub point_norm: Option<[f64; 2]>,
    /// Eye position, oriented screen frame, mm; interpolated when filled.
    pub eye_pos_mm: Option<[f64; 3]>,
    pub confidence: f64,
    pub gap_filled: bool,
}

impl GapSample {
    /// View of a processed frame: the point is the filtered point (the calibrated one when a
    /// tracked frame has no filtered point), and only for `tracked` frames.
    pub fn from_processed(p: &ProcessedSample) -> Self {
        let tracked = p.track_state == TrackState::Tracked;
        GapSample {
            t_sec: p.t_sec,
            tracked,
            point_norm: if tracked {
                p.filtered_point_norm.or(p.calibrated_point_norm)
            } else {
                None
            },
            eye_pos_mm: p.eye_pos_mm,
            confidence: p.confidence,
            gap_filled: p.gap_filled,
        }
    }

    fn bracket(&self) -> Option<([f64; 2], [f64; 3])> {
        if !self.tracked {
            return None;
        }
        Some((self.point_norm?, self.eye_pos_mm?))
    }
}

fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

/// GAP-1: returns a copy with every fillable gap filled.
pub fn fill(
    samples: &[GapSample],
    geo: &OrientedGeometry,
    params: &GapFillParams,
) -> Vec<GapSample> {
    let mut out = samples.to_vec();
    let mut i = 0;
    while i < samples.len() {
        let Some((pa, ea)) = samples[i].bracket() else {
            i += 1;
            continue;
        };
        // Next sample that can bracket, scanning over non-tracked ones.
        let mut j = i + 1;
        while j < samples.len() && !samples[j].tracked {
            j += 1;
        }
        if j == samples.len() {
            break;
        }
        if j > i + 1 {
            if let Some((pb, eb)) = samples[j].bracket() {
                let (ta, tb) = (samples[i].t_sec, samples[j].t_sec);
                let dur_ms = (tb - ta) * 1000.0;
                let eye = [
                    (ea[0] + eb[0]) / 2.0,
                    (ea[1] + eb[1]) / 2.0,
                    (ea[2] + eb[2]) / 2.0,
                ];
                let a_mm = geo.norm_to_mm(pa);
                let b_mm = geo.norm_to_mm(pb);
                let deg = angle_deg([a_mm[0], a_mm[1], 0.0], [b_mm[0], b_mm[1], 0.0], eye);
                if dur_ms > 0.0 && dur_ms <= params.max_ms + EPS_MS && deg <= params.max_deg {
                    for s in &mut out[i + 1..j] {
                        let f = (s.t_sec - ta) / (tb - ta);
                        s.point_norm = Some([lerp(pa[0], pb[0], f), lerp(pa[1], pb[1], f)]);
                        s.eye_pos_mm = Some([
                            lerp(ea[0], eb[0], f),
                            lerp(ea[1], eb[1], f),
                            lerp(ea[2], eb[2], f),
                        ]);
                        s.gap_filled = true;
                    }
                }
            }
        }
        i = j;
    }
    out
}

/// GAP-1 on processed frames: filled frames get `gap_filled = true`, the interpolated point
/// in `filtered_point_norm` and the interpolated eye position; nothing else changes (state,
/// confidence, `valid = false`, no AOI names).
pub fn fill_processed(
    samples: &[ProcessedSample],
    geo: &OrientedGeometry,
    params: &GapFillParams,
) -> Vec<ProcessedSample> {
    let view: Vec<GapSample> = samples.iter().map(GapSample::from_processed).collect();
    let filled = fill(&view, geo, params);
    samples
        .iter()
        .zip(filled)
        .map(|(p, g)| {
            let mut p = p.clone();
            if g.gap_filled && !p.gap_filled {
                p.gap_filled = true;
                p.filtered_point_norm = g.point_norm;
                p.eye_pos_mm = g.eye_pos_mm;
            }
            p
        })
        .collect()
}

/// One fixation: samples `start..end` (indices into the input).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fixation {
    pub start: usize,
    pub end: usize,
    pub start_sec: f64,
    pub end_sec: f64,
    /// CNF-3: mean confidence of its measured (not filled) samples; `None` if all are filled.
    pub mean_confidence: Option<f64>,
}

/// I-VT fixation segmentation in degrees per second over tracked and gap-filled samples.
///
/// The run logic mirrors `metrics::fixation_stats` (a run starts at the first pair below the
/// threshold, closes at a pair at or above it, or at a repeated or backwards timestamp), with
/// the velocity in degrees of visual angle per second seen from the mean eye position of the
/// pair (SCR-3 units), and a sample without a point (an unfilled gap: GAP-3) closing the run.
/// A run is a fixation when `end_sec - start_sec >= min_duration_sec`.
///
/// Used for the session's `confidence.meanFixation` only; scoring keeps its own I-VT until
/// WP-G.13 changes it.
pub fn fixations(
    samples: &[GapSample],
    geo: &OrientedGeometry,
    velocity_deg_per_sec: f64,
    min_duration_sec: f64,
) -> Vec<Fixation> {
    let mut out = Vec::new();
    let mut run_start: Option<usize> = None;
    let close = |start: Option<usize>, end: usize, out: &mut Vec<Fixation>| {
        if let Some(s) = start {
            if end > s + 1 {
                let (start_sec, end_sec) = (samples[s].t_sec, samples[end - 1].t_sec);
                if end_sec - start_sec >= min_duration_sec {
                    out.push(Fixation {
                        start: s,
                        end,
                        start_sec,
                        end_sec,
                        mean_confidence: super::confidence::fixation(
                            samples[s..end]
                                .iter()
                                .filter(|x| !x.gap_filled)
                                .map(|x| x.confidence),
                        ),
                    });
                }
            }
        }
    };
    let usable = |s: &GapSample| -> Option<([f64; 2], [f64; 3])> {
        if s.tracked || s.gap_filled {
            Some((s.point_norm?, s.eye_pos_mm?))
        } else {
            None
        }
    };
    let mut prev: Option<(usize, [f64; 2], [f64; 3])> = None;
    for (i, s) in samples.iter().enumerate() {
        let Some((p, e)) = usable(s) else {
            close(run_start.take(), i, &mut out);
            prev = None;
            continue;
        };
        if let Some((pi, pp, pe)) = prev {
            let dt = s.t_sec - samples[pi].t_sec;
            if dt > 0.0 {
                let eye = [
                    (pe[0] + e[0]) / 2.0,
                    (pe[1] + e[1]) / 2.0,
                    (pe[2] + e[2]) / 2.0,
                ];
                let a = geo.norm_to_mm(pp);
                let b = geo.norm_to_mm(p);
                let v = angle_deg([a[0], a[1], 0.0], [b[0], b[1], 0.0], eye) / dt;
                if v >= velocity_deg_per_sec || v.is_nan() {
                    close(run_start.take(), i, &mut out);
                } else if run_start.is_none() {
                    run_start = Some(pi);
                }
            } else {
                close(run_start.take(), i, &mut out);
            }
        }
        prev = Some((i, p, e));
    }
    close(run_start.take(), samples.len(), &mut out);
    out
}

/// One frame for the loss sums.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossFrame {
    pub t_sec: f64,
    pub state: TrackState,
    pub gap_filled: bool,
}

/// GAP-3 loss sums and the share of frame time not `tracked` (same capped intervals). No
/// frames, or no measurable time, gives a data loss of 1 when nothing is tracked and the
/// share of non-tracked frames otherwise.
pub fn loss(frames: &[LossFrame], params: &GapFillParams) -> (LossSummary, f64) {
    let cap = params.loss_interval_cap_ms / 1000.0;
    let mut s = LossSummary::default();
    let mut total = 0.0;
    let mut lost = 0.0;
    for (i, f) in frames.iter().enumerate() {
        let dt = frames
            .get(i + 1)
            .map_or(0.0, |n| (n.t_sec - f.t_sec).clamp(0.0, cap));
        total += dt;
        match f.state {
            TrackState::Tracked => {}
            TrackState::Blink => s.blink_sec += dt,
            TrackState::Lost => s.track_loss_sec += dt,
            TrackState::Offscreen => s.offscreen_sec += dt,
            TrackState::Artifact => s.artifact_sec += dt,
        }
        if f.state != TrackState::Tracked {
            lost += dt;
        }
        if f.gap_filled {
            s.gap_filled_sec += dt;
        }
    }

    // Longest non-tracked run, bracket interval, uncapped.
    let mut i = 0;
    while i < frames.len() {
        if frames[i].state == TrackState::Tracked {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < frames.len() && frames[j].state != TrackState::Tracked {
            j += 1;
        }
        let from = if i > 0 {
            frames[i - 1].t_sec
        } else {
            frames[i].t_sec
        };
        let to = if j < frames.len() {
            frames[j].t_sec
        } else {
            frames[j - 1].t_sec
        };
        s.longest_gap_sec = s.longest_gap_sec.max(to - from);
        i = j;
    }

    let data_loss = if total > 0.0 {
        lost / total
    } else if frames.is_empty() {
        1.0
    } else {
        frames
            .iter()
            .filter(|f| f.state != TrackState::Tracked)
            .count() as f64
            / frames.len() as f64
    };
    (s, data_loss)
}
