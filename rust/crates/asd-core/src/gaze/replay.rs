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
//!   first of them with their positions as the layout (a position repeated right after itself
//!   is a CAL-3 re-show of the same target, not a new one) and is fitted when the next
//!   non-calibration target (or the end of the log) comes. The pipeline decides retries
//!   (`calibration_status`), so a scheduled calibration target is matched against the
//!   pipeline's pending target: when the pipeline wants a retry that the log does not show
//!   (the next scheduled position is another target), the retry is skipped and the target
//!   ends `missed` (`GazePipeline::skip_calibration_target`; the log has no frames for it);
//!   when the log re-shows a target the pipeline already accepted, that window is not opened.
//!   A log without retries replays exactly as before.
//! - **Recording.** A frame counts in the session summary exactly when it carries
//!   `clip_media_time_sec` (it belongs to a stimulus clip), so positioning, calibration,
//!   validation and drift-check frames never count (pipeline docs, `set_recording`).
//! - **Config.** Built from the header (model, geometry, orientation) with default gate and
//!   params unless [`ReplayOptions`] overrides them. No wall clock, no randomness (PIPE-5).

use serde::Serialize;
use thiserror::Error;

use super::calibration::TargetStatus;
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
    /// One result per target window played, in schedule order (a calibration re-show the
    /// pipeline did not need plays no window; a skipped retry adds no result).
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

    /// Opens the window of scheduled target `index` (or, for a calibration re-show the
    /// pipeline did not need, opens nothing).
    fn begin(&mut self, index: usize) -> Result<(), ReplayError> {
        let t = &self.targets[index];
        let at = [t.x_norm, t.y_norm];
        if t.kind == TargetKind::Calibration {
            if !self.calibrating {
                let mut layout: Vec<[f64; 2]> = Vec::new();
                for x in self.targets[index..]
                    .iter()
                    .take_while(|x| x.kind == TargetKind::Calibration)
                {
                    let p = [x.x_norm, x.y_norm];
                    // A re-show (CAL-3 retry) is the same target again, not a new one.
                    if layout.last() != Some(&p) {
                        layout.push(p);
                    }
                }
                self.pipeline
                    .start_calibration(Some(layout), None)
                    .map_err(|e| self.err(index, e))?;
                self.calibrating = true;
            }
            if !self.drive_calibration(index, at)? {
                return Ok(());
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

    /// Brings the running calibration to the scheduled position `at` (module docs,
    /// "Calibration"): a retry the log never shows is skipped (the target ends `missed`); a
    /// re-show the pipeline does not need opens no window. Returns whether to open `at`.
    fn drive_calibration(&mut self, index: usize, at: [f64; 2]) -> Result<bool, ReplayError> {
        loop {
            let Some(TargetStatus::Pending {
                target_norm,
                attempt,
                ..
            }) = self.pipeline.calibration_status()
            else {
                // Complete (or no session): begin_target reports the error.
                return Ok(true);
            };
            if same(target_norm, at) {
                return Ok(true);
            }
            if attempt > 1 {
                self.pipeline
                    .skip_calibration_target()
                    .map_err(|e| self.err(index, e))?;
                continue;
            }
            let reshow = index > 0 && {
                let prev = &self.targets[index - 1];
                prev.kind == TargetKind::Calibration && same([prev.x_norm, prev.y_norm], at)
            };
            // A genuine schedule mismatch is reported by begin_target.
            return Ok(!reshow);
        }
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

/// Two scheduled positions are the same target.
fn same(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= 1e-9 && (a[1] - b[1]).abs() <= 1e-9
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
