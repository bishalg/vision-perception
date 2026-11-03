//! owner: WP-X.1
//!
//! RPX-1: reprocessing a research frame log (`asd-raw-tracking/1` with the RFL-1 additions)
//! from raw samples to metric sets with a profile's gaze sections ([`GazeProcessingView`]).
//!
//! # Rules
//! - **Replay.** [`replay_with`] with the view's quality gate and tracking params (and the
//!   options' model / glasses), so calibration, validation, drift and the session summary are
//!   exactly what `gaze replay` gives for the same settings.
//! - **AOIs.** Each clip's AOIs are the header's `clips[].aois` (the boxes the device used,
//!   screen space); a clip the header lists without AOIs, or does not list, takes the
//!   catalog clip of the same id through [`clip_aois_with_margin`] (catalog boxes are screen
//!   space, trap 11). Both are grown by `aoi.marginNorm` and handed to `begin_clip`, so the
//!   pipeline hit-tests the filtered point at the clip media time like on the device. The
//!   caller passes the catalog version the header names (`clips[].catalogVersion`). A clip
//!   with no AOIs from either source gets a warning and no hits.
//! - **Frames.** The device recipe (`GazeProcessingActor.processFaceAnchor`): every pushed
//!   frame goes through one [`TelemetryWriter`] (session origin 0, the view's gate), so the
//!   last-known pose is carried exactly as on the device; recorded frames (those with
//!   `clip_media_time_sec`) become [`GazeFrame`]s with the written sample, the pipeline's
//!   `active_aoi_names`, the affect carried on the raw sample, and the verdict
//!   `rejected` for a `lost` frame, else the gate's.
//! - **Metrics.** [`metrics::extract_with`] over those frames with catalog AOI roles and the
//!   view's `metrics` / `aoi` sections; [`metrics::fixations_with`] for the scanpath.
//! - Pure: no clock, no randomness, no I/O (RPX-6).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::aoi_space::{self, clip_aois_with_margin, AoiSpace, ScreenAoi, VideoGravity};
use super::pipeline::TelemetryWriter;
use super::raw_log::RawLog;
use super::replay::{replay_with, ReplayError, ReplayObserver, ReplayOptions};
use crate::analysis::profile::{GazeProcessingView, IvtUnit};
use crate::metrics::{self, AoiRole, Fixation};
use crate::quality;
use crate::types::tracking::{
    CalModelKind, ProcessedSample, RawTrackingSample, TrackState, TrackingQuality,
};
use crate::types::{
    GazeFrame, GazeFrameSample, GazeQualityVerdict, PerVideoMetricSet, StimulusCatalog,
};

/// Options of [`reprocess`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReprocessOptions {
    /// Forces the calibration model (as `gaze replay --model`).
    #[serde(default)]
    pub model: Option<CalModelKind>,
    /// Operator answer at positioning (CNF-2).
    #[serde(default)]
    pub glasses: Option<bool>,
    /// Rate of the down-sampled plotting trace; no trace when absent or 0.
    #[serde(default)]
    pub trace_hz: Option<u32>,
}

/// Why a log could not be reprocessed.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReprocessError {
    #[error(transparent)]
    Replay(#[from] ReplayError),
}

/// Recorded frames per quality verdict.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerdictCounts {
    pub optimal: u32,
    pub acceptable: u32,
    pub degraded: u32,
    pub rejected: u32,
}

impl VerdictCounts {
    fn add(&mut self, v: GazeQualityVerdict) {
        let slot = match v {
            GazeQualityVerdict::Optimal => &mut self.optimal,
            GazeQualityVerdict::Acceptable => &mut self.acceptable,
            GazeQualityVerdict::Degraded => &mut self.degraded,
            GazeQualityVerdict::Rejected => &mut self.rejected,
        };
        *slot += 1;
    }
}

/// One point of the down-sampled plotting trace (recorded frames only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePoint {
    /// Session time of the frame (`t_sec` of the log).
    pub t_sec: f64,
    /// The written point, screen-normalised (held on a non-tracked frame, never clamped).
    pub x: f64,
    pub y: f64,
    pub state: TrackState,
    /// First AOI hit, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aoi: Option<String>,
    /// Index into [`Reprocessed::fixations`] when the frame lies in a fixation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixation_id: Option<u32>,
}

/// Everything [`reprocess`] produces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reprocessed {
    /// One set per clip, in play order ([`metrics::extract_with`]).
    pub metric_sets: Vec<PerVideoMetricSet>,
    /// Recorded frames.
    pub frames_n: usize,
    /// Share of recorded frames that are valid for the metrics (valid and not `rejected`).
    pub valid_ratio: f64,
    pub quality_verdicts: VerdictCounts,
    /// The replay's session `trackingQuality` (QS-1).
    pub tracking: TrackingQuality,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<TracePoint>>,
    /// The I-VT fixations the fixation counts are made of ([`metrics::fixations_with`]).
    pub fixations: Vec<Fixation>,
    /// What could not be applied or found (`param_not_applied: <id>`, clips without AOIs).
    pub warnings: Vec<String>,
    /// The recorded frames the metrics were extracted from (not serialised: large).
    #[serde(skip)]
    pub frames: Vec<GazeFrame>,
}

/// The [`ReplayObserver`] that builds the device's frames.
struct FrameBuilder<'a> {
    log: &'a RawLog,
    catalog: &'a StimulusCatalog,
    margin_norm: f64,
    screen_aspect: f64,
    writer: TelemetryWriter,
    gate: quality::QualityGate,
    /// AOIs per clip id, resolved once.
    aois: BTreeMap<String, Vec<ScreenAoi>>,
    frames: Vec<GazeFrame>,
    states: Vec<(f64, TrackState)>,
    warnings: Vec<String>,
}

impl FrameBuilder<'_> {
    fn resolve_aois(&mut self, clip_id: &str) -> Vec<ScreenAoi> {
        let header = self.log.header.clips.iter().find(|c| c.clip_id == clip_id);
        if let Some(h) = header.filter(|h| !h.aois.is_empty()) {
            return aoi_space::with_margin(&h.aois, self.margin_norm);
        }
        match self.catalog.clips.iter().find(|c| c.id == clip_id) {
            Some(clip) => {
                // Catalog boxes are screen space (trap 11), so the aspects do not move them.
                let video_aspect = header
                    .and_then(|h| h.video_aspect)
                    .unwrap_or(self.screen_aspect);
                clip_aois_with_margin(
                    &clip.areas,
                    AoiSpace::Screen,
                    video_aspect,
                    self.screen_aspect,
                    VideoGravity::AspectFill,
                    self.margin_norm,
                )
            }
            None => {
                self.warnings.push(format!(
                    "clip {clip_id:?}: no AOIs in the log header or the catalog; no AOI hits"
                ));
                Vec::new()
            }
        }
    }
}

impl ReplayObserver for FrameBuilder<'_> {
    fn clip_aois(&mut self, clip_id: &str) -> Vec<ScreenAoi> {
        if let Some(a) = self.aois.get(clip_id) {
            return a.clone();
        }
        let a = self.resolve_aois(clip_id);
        self.aois.insert(clip_id.to_owned(), a.clone());
        a
    }

    fn frame(&mut self, raw: &RawTrackingSample, p: &ProcessedSample, recording: bool) {
        let written = self.writer.sample(p);
        if !recording {
            return;
        }
        let verdict = if p.track_state == TrackState::Lost {
            GazeQualityVerdict::Rejected
        } else {
            quality::evaluate_sample(&written, &self.gate).verdict
        };
        self.frames.push(GazeFrame {
            gaze: GazeFrameSample {
                is_valid: written.is_valid,
                x_norm: written.x_norm,
                y_norm: written.y_norm,
                video_time_sec: written.video_time_sec,
                clip_media_time_sec: written.clip_media_time_sec,
            },
            affect: raw.affect.clone().unwrap_or_default(),
            active_aoi_names: p.active_aoi_names.clone(),
            quality_verdict: verdict,
            stimulus_id: raw.stimulus_id.clone(),
        });
        self.states.push((raw.t_sec, p.track_state));
    }
}

/// RPX-1: reprocesses `log` with the view's gate, tracking params, metrics and AOI sections
/// (module docs).
pub fn reprocess(
    log: &RawLog,
    catalog: &StimulusCatalog,
    view: &GazeProcessingView,
    options: &ReprocessOptions,
) -> Result<Reprocessed, ReprocessError> {
    let geo = log.header.geometry.rotate(log.header.orientation);
    let gate = *view.quality_gate;
    let mut builder = FrameBuilder {
        log,
        catalog,
        margin_norm: view.aoi.margin_norm,
        screen_aspect: geo.width_mm / geo.height_mm,
        writer: TelemetryWriter::new(0.0, gate),
        gate,
        aois: BTreeMap::new(),
        frames: Vec::new(),
        states: Vec::new(),
        warnings: Vec::new(),
    };
    let replay_options = ReplayOptions {
        params: Some(view.tracking_quality.clone()),
        model: options.model,
        glasses: options.glasses,
        quality_gate: Some(gate),
    };
    let report = replay_with(log, &replay_options, &mut builder)?;
    let FrameBuilder {
        frames,
        states,
        mut warnings,
        ..
    } = builder;

    if view.metrics.ivt_unit == IvtUnit::Deg {
        warnings.push("param_not_applied: metrics.ivtUnit".to_owned());
    }
    let roles = AoiRole::from_catalog(catalog);
    let metric_sets = metrics::extract_with(&frames, &roles, view.metrics, view.aoi);
    let fixations = metrics::fixations_with(&frames, view.metrics);

    let mut quality_verdicts = VerdictCounts::default();
    for f in &frames {
        quality_verdicts.add(f.quality_verdict);
    }
    let valid = frames.iter().filter(|f| metrics::is_valid_frame(f)).count();
    let valid_ratio = if frames.is_empty() {
        0.0
    } else {
        valid as f64 / frames.len() as f64
    };

    let trace = options
        .trace_hz
        .filter(|&hz| hz > 0)
        .map(|hz| trace(&frames, &states, &fixations, hz));

    Ok(Reprocessed {
        metric_sets,
        frames_n: frames.len(),
        valid_ratio,
        quality_verdicts,
        tracking: report.summary,
        trace,
        fixations,
        warnings,
        frames,
    })
}

/// Recorded frames down-sampled to `hz`: a frame is kept when its time reaches the next slot
/// (`first + k / hz`).
fn trace(
    frames: &[GazeFrame],
    states: &[(f64, TrackState)],
    fixations: &[Fixation],
    hz: u32,
) -> Vec<TracePoint> {
    let mut fixation_of: Vec<Option<u32>> = vec![None; frames.len()];
    for (id, f) in fixations.iter().enumerate() {
        for slot in &mut fixation_of[f.first_frame..f.end_frame] {
            *slot = Some(id as u32);
        }
    }
    let step = 1.0 / f64::from(hz);
    let mut out = Vec::new();
    let mut next: Option<f64> = None;
    let mut slot = 0u64;
    for (i, frame) in frames.iter().enumerate() {
        let (t, state) = states[i];
        let first = *next.get_or_insert(t);
        if t + 1e-9 < first + slot as f64 * step {
            continue;
        }
        while first + (slot as f64) * step <= t + 1e-9 {
            slot += 1;
        }
        out.push(TracePoint {
            t_sec: t,
            x: frame.gaze.x_norm,
            y: frame.gaze.y_norm,
            state,
            aoi: frame.active_aoi_names.first().cloned(),
            fixation_id: fixation_of[i],
        });
    }
    out
}
