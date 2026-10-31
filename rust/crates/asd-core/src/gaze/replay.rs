//! owner: WP-G.5
//!
//! Replay of a raw tracking log (REP-1) through [`GazePipeline`], driven by the log header's
//! target schedule. The same code path serves `asd-cli gaze replay`, `asd-cli gaze
//! bench-report` (BEN-5) and the scenario goldens (TV-2).
//!
//! # Rules
//! - **Targets.** `header.targets` (in order) are played through the pipeline's target API.
//!   A target opens at the first frame whose `t_sec >= startSec` and closes as soon as the
//!   pipeline reports its window complete (`GazePipeline::target_complete`); a window still
//!   open when the next target is due, or when the log ends, is closed there. A target whose
//!   whole window lies between two frames is opened and closed at once (outcome `missed`).
//! - **Calibration.** Consecutive `calibration` targets form one calibration: it starts at the
//!   first of them with exactly their positions as the layout and is fitted when the next
//!   non-calibration target (or the end of the log) comes.
//! - **Recording.** A frame counts in the session summary exactly when it carries
//!   `clip_media_time_sec` (it belongs to a stimulus clip), so positioning, calibration,
//!   validation and drift-check frames never count (pipeline docs, `set_recording`).
//! - **Config.** Built from the header (model, geometry, orientation) with default gate and
//!   params unless [`ReplayOptions`] overrides them. No wall clock, no randomness (PIPE-5).

use serde::Serialize;
use thiserror::Error;

use super::pipeline::{GazePipeline, PipelineConfig, PipelineError};
use super::raw_log::RawLog;
use crate::types::tracking::{
    CalModelKind, RecalAdvice, ScheduledTarget, TargetKind, TargetResult, TrackingQuality,
    TrackingQualityParams,
};

/// Replay settings.
#[derive(Debug, Clone, Default)]
pub struct ReplayOptions {
    /// Replaces the default [`TrackingQualityParams`].
    pub params: Option<TrackingQualityParams>,
    /// Forces the calibration model (BEN-5): sets `calibration.defaultModel` and turns
    /// `autoSelect` off.
    pub model: Option<CalModelKind>,
    /// Operator answer at positioning (CNF-2).
    pub glasses: Option<bool>,
}

/// Why a replay could not run.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReplayError {
    #[error("target {index} ({kind:?} at {x}, {y}): {source}")]
    Pipeline {
        index: usize,
        kind: TargetKind,
        x: f64,
        y: f64,
        source: PipelineError,
    },
    #[error("target {index}: {message}")]
    Schedule { index: usize, message: String },
}

/// Everything a replay produces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayReport {
    /// Frames pushed.
    pub frames: usize,
    /// Frames that counted in the summary (clip frames).
    pub recorded_frames: usize,
    /// One result per scheduled target, in schedule order.
    pub targets: Vec<TargetResult>,
    /// Advice at the end of the log (CAL-9, VAL-4, DRF-3).
    pub advice: RecalAdvice,
    /// The session `trackingQuality` record (QS-1).
    pub summary: TrackingQuality,
}

/// Checks the schedule of a header: finite positions and times, `endSec >= startSec`,
/// non-decreasing starts. Used by `gaze replay --check` and [`replay`].
pub fn check_schedule(targets: &[ScheduledTarget]) -> Result<(), ReplayError> {
    let mut prev_start = f64::NEG_INFINITY;
    for (index, t) in targets.iter().enumerate() {
        let bad = |message: &str| ReplayError::Schedule {
            index,
            message: message.to_string(),
        };
        if ![t.x_norm, t.y_norm, t.start_sec, t.end_sec]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(bad("position and times must be finite"));
        }
        if t.end_sec < t.start_sec {
            return Err(bad("endSec is before startSec"));
        }
        if t.start_sec < prev_start {
            return Err(bad("targets must be ordered by startSec"));
        }
        prev_start = t.start_sec;
    }
    Ok(())
}

struct Player<'a> {
    pipeline: GazePipeline,
    targets: &'a [ScheduledTarget],
    results: Vec<TargetResult>,
    /// A target window is open.
    open: bool,
    /// A calibration is running (not yet fitted).
    calibrating: bool,
}

impl Player<'_> {
    fn err(&self, index: usize, source: PipelineError) -> ReplayError {
        let t = &self.targets[index];
        ReplayError::Pipeline {
            index,
            kind: t.kind,
            x: t.x_norm,
            y: t.y_norm,
            source,
        }
    }

    fn finish_calibration(&mut self, index: usize) -> Result<(), ReplayError> {
        if self.calibrating {
            self.calibrating = false;
            self.pipeline
                .finish_calibration()
                .map_err(|e| self.err(index, e))?;
        }
        Ok(())
    }

    fn begin(&mut self, index: usize) -> Result<(), ReplayError> {
        let t = &self.targets[index];
        let at = [t.x_norm, t.y_norm];
        if t.kind == TargetKind::Calibration {
            if !self.calibrating {
                let layout: Vec<[f64; 2]> = self.targets[index..]
                    .iter()
                    .take_while(|x| x.kind == TargetKind::Calibration)
                    .map(|x| [x.x_norm, x.y_norm])
                    .collect();
                self.pipeline
                    .start_calibration(Some(layout), None)
                    .map_err(|e| self.err(index, e))?;
                self.calibrating = true;
            }
        } else {
            self.finish_calibration(index)?;
        }
        self.pipeline
            .begin_target(t.kind, at)
            .map_err(|e| self.err(index, e))?;
        self.open = true;
        Ok(())
    }

    fn end(&mut self) {
        if self.open {
            self.open = false;
            // A calibration window that never saw a frame has no result.
            if let Some(r) = self.pipeline.end_target() {
                self.results.push(r);
            }
        }
    }
}

/// Replays `log` through a fresh pipeline.
pub fn replay(log: &RawLog, options: &ReplayOptions) -> Result<ReplayReport, ReplayError> {
    check_schedule(&log.header.targets)?;
    let mut config = PipelineConfig {
        model_identifier: log.header.device_model.clone(),
        geometry_source: log.header.geometry_source,
        geometry: log.header.geometry.clone(),
        orientation: log.header.orientation,
        quality_gate: Default::default(),
        params: options.params.clone().unwrap_or_default(),
        calibration: None,
        glasses: options.glasses,
    };
    if let Some(kind) = options.model {
        config.params.calibration.default_model = kind;
        config.params.calibration.auto_select = false;
    }
    let targets = &log.header.targets[..];
    let mut p = Player {
        pipeline: GazePipeline::new(config),
        targets,
        results: Vec::new(),
        open: false,
        calibrating: false,
    };

    let mut next = 0usize;
    let mut recorded_frames = 0usize;
    for raw in &log.samples {
        let t = raw.t_sec;
        // The next target is due: a window still open is over.
        if p.open && next < targets.len() && t >= targets[next].start_sec {
            p.end();
        }
        while !p.open && next < targets.len() && t >= targets[next].start_sec {
            let index = next;
            next += 1;
            p.begin(index)?;
            if targets[index].end_sec < t {
                // The whole window fell between two frames: nothing was collected.
                p.end();
            }
        }
        let recording = raw.clip_media_time_sec.is_some();
        recorded_frames += usize::from(recording);
        p.pipeline.set_recording(recording);
        p.pipeline.push(raw);
        if p.open && p.pipeline.target_complete() {
            p.end();
        }
    }
    p.end();
    // Targets that never became due are not played; a running calibration is fitted.
    p.finish_calibration(next.saturating_sub(1))?;
    p.pipeline.set_recording(false);

    Ok(ReplayReport {
        frames: log.samples.len(),
        recorded_frames,
        targets: p.results,
        advice: p.pipeline.advice(),
        summary: p.pipeline.summary(),
    })
}
