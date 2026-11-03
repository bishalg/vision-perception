//! owner: WP-R.2
//!
//! CORE-5: per-clip metric extraction, a 1:1 port of `PerVideoMetricsExtractor`,
//! `PerVideoMetricKey` and `AOIRole` (`PerVideoMetrics.swift`, `SessionBiomarkerAggregator.swift`
//! at 9655fcc; the aggregator is now `SessionEngagementAggregator`). The Swift bodies are the oracle; the golden vectors in
//! `tests/golden/metrics/` were produced by running them.
//!
//! Parity notes:
//! - Sums are left folds in frame order, like Swift `reduce(0, +)`, so floats match bit for bit.
//! - `swift_max`/`swift_min` reproduce `Swift.max`/`Swift.min` operand selection exactly.
//! - Affect: a frame's `affect.{socialSmileIntensity,expressivityIndex,surpriseScore}` are the
//!   Swift `FacialAffectMetrics` computed values, supplied by the caller in the frame JSON
//!   (CON-1). An absent value reads as `0.0`, which is what Swift's `.neutral` affect yields.
//! - AOI roles: Swift consults the bundled `StimulusCatalog.current`. Pure Rust has no bundle, so
//!   [`extract`]/[`extract_clip`] use the built-in sets and the `*_with_roles` variants take an
//!   [`AoiRole`] built from a catalog. With the bundled catalog both agree (guarded by a test).
//!
//! Parameters (PRM-9, WP-X.1): every number of the extraction comes from
//! [`MetricsParams`] (`ScoringProfile.metrics`). The constants below are only the default
//! source of `MetricsParams::default()` (and so of the contract's `ScoringProfile::default()`);
//! [`extract`], [`extract_with_roles`], [`extract_clip`] and [`fixation_stats`] keep their
//! signatures and run [`extract_with`] with the defaults, so every golden vector is unchanged.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::analysis::profile::{AoiParams, MetricsParams};
use crate::types::{
    GazeFrame, GazeQualityVerdict, PerVideoMetricSet, PerVideoScoreReport, StimulusCatalog,
};

/// Fallback clip id for frames captured without a playlist (`untaggedClipId`).
pub const UNTAGGED_CLIP_ID: &str = "session_untagged";
/// I-VT velocity threshold in normalised viewport units per second.
pub const VELOCITY_THRESHOLD_UNITS_PER_SEC: f64 = 5.0;
/// Minimum fixation duration in seconds (core `OculomotorClassifier`: 80 ms).
pub const MIN_FIXATION_DURATION_SEC: f64 = 0.08;
/// Nominal 60 Hz frame interval.
pub const FRAME_INTERVAL_SEC: f64 = 1.0 / 60.0;
/// Clip-onset window of the smile-reactivity baseline, seconds (`metrics.smileReactivityWindowSec`).
pub const SMILE_REACTIVITY_WINDOW_SEC: f64 = 0.5;
/// Gain of the flat-affect score (`metrics.flatAffectGain`).
pub const FLAT_AFFECT_GAIN: f64 = 50.0;

/// `Swift.max(x, y)`: `y >= x ? y : x`.
#[inline]
pub(crate) fn swift_max(x: f64, y: f64) -> f64 {
    if y >= x {
        y
    } else {
        x
    }
}

/// `Swift.min(x, y)`: `y < x ? y : x`.
#[inline]
pub(crate) fn swift_min(x: f64, y: f64) -> f64 {
    if y < x {
        y
    } else {
        x
    }
}

/// `max(0, min(1, v))` as written in Swift.
#[inline]
pub(crate) fn clamp01(v: f64) -> f64 {
    swift_max(0.0, swift_min(1.0, v))
}

/// Canonical metric keys (`PerVideoMetricKey`), shared with the normative baselines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerVideoMetricKey {
    SocialDwellRatio,
    GeometricDwellRatio,
    SocialPreferenceScore,
    EyeDwellRatio,
    MouthDwellRatio,
    AoiDwellRatio,
    AoiDwellWindowRatio,
    FirstEntryLatencyMs,
    FixationCount,
    MeanFixationDurationMs,
    SocialSmileIntensityMean,
    SmileReactivityIndex,
    FlatAffectScore,
    SurpriseScoreMean,
    ValidSampleRatio,
}

impl PerVideoMetricKey {
    /// Every key in Swift `CaseIterable` order.
    pub const ALL: [PerVideoMetricKey; 15] = [
        Self::SocialDwellRatio,
        Self::GeometricDwellRatio,
        Self::SocialPreferenceScore,
        Self::EyeDwellRatio,
        Self::MouthDwellRatio,
        Self::AoiDwellRatio,
        Self::AoiDwellWindowRatio,
        Self::FirstEntryLatencyMs,
        Self::FixationCount,
        Self::MeanFixationDurationMs,
        Self::SocialSmileIntensityMean,
        Self::SmileReactivityIndex,
        Self::FlatAffectScore,
        Self::SurpriseScoreMean,
        Self::ValidSampleRatio,
    ];

    /// The Swift `rawValue`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SocialDwellRatio => "social_dwell_ratio",
            Self::GeometricDwellRatio => "geometric_dwell_ratio",
            Self::SocialPreferenceScore => "social_preference_score",
            Self::EyeDwellRatio => "eye_dwell_ratio",
            Self::MouthDwellRatio => "mouth_dwell_ratio",
            Self::AoiDwellRatio => "aoi_dwell_ratio",
            Self::AoiDwellWindowRatio => "aoi_dwell_window_ratio",
            Self::FirstEntryLatencyMs => "first_entry_latency_ms",
            Self::FixationCount => "fixation_count",
            Self::MeanFixationDurationMs => "mean_fixation_duration_ms",
            Self::SocialSmileIntensityMean => "social_smile_intensity_mean",
            Self::SmileReactivityIndex => "smile_reactivity_index",
            Self::FlatAffectScore => "flat_affect_score",
            Self::SurpriseScoreMean => "surprise_score_mean",
            Self::ValidSampleRatio => "valid_sample_ratio",
        }
    }

    /// Metrics-dictionary key: `raw` or `raw::AOI` (an empty AOI counts as none).
    pub fn key(self, target_aoi: Option<&str>) -> String {
        match target_aoi {
            Some(aoi) if !aoi.is_empty() => format!("{}::{aoi}", self.as_str()),
            _ => self.as_str().to_string(),
        }
    }
}

/// Clinical AOI role (`AOIRoleKind`); `as_str` is the catalog `roles` spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AoiRoleKind {
    Social,
    Geometric,
    Eye,
    Mouth,
    Orienting,
}

impl AoiRoleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Social => "social",
            Self::Geometric => "geometric",
            Self::Eye => "eye",
            Self::Mouth => "mouth",
            Self::Orienting => "orienting",
        }
    }
}

/// AOI role resolver (`AOIRole`): catalog `roles` data first, then the built-in name sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AoiRole {
    /// Area name to the union of its catalog roles (`StimulusCatalog.roleIndex`).
    catalog_roles: BTreeMap<String, BTreeSet<String>>,
}

impl AoiRole {
    pub const SOCIAL: &'static [&'static str] = &[
        "Social_Actor",
        "Face_Center",
        "Face_Bounding_Box",
        "Eye_Region",
        "Actor_Face",
        "Actor_Eyes",
        "Cartoon_Character",
        "Teacher_Face",
        "Teacher_Mouth",
    ];
    pub const GEOMETRIC: &'static [&'static str] =
        &["Geometric_Pattern", "Mechanical_Motion", "Gift_Box"];
    pub const EYE: &'static [&'static str] = &["Eye_Region", "Actor_Eyes"];
    pub const MOUTH: &'static [&'static str] = &["Mouth_Region", "Teacher_Mouth"];
    /// Person or eye regions used for first-orient latency.
    pub const ORIENTING: &'static [&'static str] = &[
        "Social_Actor",
        "Face_Center",
        "Face_Bounding_Box",
        "Eye_Region",
        "Actor_Face",
        "Actor_Eyes",
        "Teacher_Face",
    ];

    /// Built-in sets only (Swift with `StimulusCatalog.empty`).
    pub fn builtin() -> Self {
        Self::default()
    }

    /// Built-in sets plus every role the catalog assigns to an area name (union over clips).
    pub fn from_catalog(catalog: &StimulusCatalog) -> Self {
        let mut catalog_roles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for clip in &catalog.clips {
            for area in &clip.areas {
                catalog_roles
                    .entry(area.name.clone())
                    .or_default()
                    .extend(area.roles.iter().cloned());
            }
        }
        Self { catalog_roles }
    }

    /// The hard-coded name set for a role.
    pub const fn builtin_names(kind: AoiRoleKind) -> &'static [&'static str] {
        match kind {
            AoiRoleKind::Social => Self::SOCIAL,
            AoiRoleKind::Geometric => Self::GEOMETRIC,
            AoiRoleKind::Eye => Self::EYE,
            AoiRoleKind::Mouth => Self::MOUTH,
            AoiRoleKind::Orienting => Self::ORIENTING,
        }
    }

    /// `AOIRole.matches`: catalog data, then the built-in set (a pure OR, so order is moot).
    pub fn matches(&self, name: &str, kind: AoiRoleKind) -> bool {
        self.catalog_roles
            .get(name)
            .is_some_and(|r| r.contains(kind.as_str()))
            || Self::builtin_names(kind).contains(&name)
    }

    /// `AOIRole.intersects(_:kind:)`.
    pub fn intersects(&self, names: &[String], kind: AoiRoleKind) -> bool {
        names.iter().any(|n| self.matches(n, kind))
    }
}

/// Result of [`fixation_stats`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixationStats {
    pub count: i64,
    pub mean_duration_ms: f64,
}

/// A frame counts as valid when `gaze.isValid && qualityVerdict != rejected`.
pub fn is_valid_frame(frame: &GazeFrame) -> bool {
    frame.gaze.is_valid && frame.quality_verdict != GazeQualityVerdict::Rejected
}

/// Media clock (`clipMediaTimeSec`) when present, else session elapsed (`videoTimeSec`).
pub fn media_time(frame: &GazeFrame) -> f64 {
    frame
        .gaze
        .clip_media_time_sec
        .unwrap_or(frame.gaze.video_time_sec)
}

/// Longest media time when it is `> 0`, else `frames * 1/60`.
pub fn observed_duration_sec(frames: &[GazeFrame]) -> f64 {
    observed_duration_refs(
        &frames.iter().collect::<Vec<_>>(),
        MetricsParams::default().fallback_frame_interval_sec,
    )
}

fn observed_duration_refs(frames: &[&GazeFrame], frame_interval_sec: f64) -> f64 {
    let max_media = frames
        .iter()
        .filter_map(|f| f.gaze.clip_media_time_sec)
        .reduce(sequence_max);
    match max_media {
        Some(m) if m > 0.0 => m,
        _ => frames.len() as f64 * frame_interval_sec,
    }
}

/// `Sequence.max()`: keeps the current value unless `current < next`.
fn sequence_max(current: f64, next: f64) -> f64 {
    if current < next {
        next
    } else {
        current
    }
}

/// Lightweight I-VT over monotonic time runs (`fixationStats(of:)`). Expects valid frames.
/// A backwards or repeated timestamp (`dt <= 0`) closes the open run and restarts.
pub fn fixation_stats(valid_frames: &[GazeFrame]) -> FixationStats {
    fixation_stats_with(valid_frames, &MetricsParams::default())
}

/// [`fixation_stats`] with the profile's I-VT threshold, minimum duration and frame interval.
pub fn fixation_stats_with(valid_frames: &[GazeFrame], params: &MetricsParams) -> FixationStats {
    fixation_stats_refs(&valid_frames.iter().collect::<Vec<_>>(), params)
}

/// One I-VT run kept as a fixation: frames `start..end` of the valid-frame list it was found in.
#[derive(Debug, Clone, Copy, PartialEq)]
struct IvtRun {
    start: usize,
    end: usize,
    duration_sec: f64,
}

/// The I-VT runs of `frames` (valid frames of one clip), in order.
fn ivt_runs(frames: &[&GazeFrame], params: &MetricsParams) -> Vec<IvtRun> {
    let mut runs: Vec<IvtRun> = Vec::new();
    let mut run_start: Option<usize> = None;

    let close_run = |run_start: &mut Option<usize>, runs: &mut Vec<IvtRun>, end: usize| {
        if let Some(start) = *run_start {
            if end > start + 1 {
                let duration_sec = media_time(frames[end - 1]) - media_time(frames[start]);
                let fallback = (end - start) as f64 * params.fallback_frame_interval_sec;
                let duration = if duration_sec > 0.0 {
                    duration_sec
                } else {
                    fallback
                };
                if duration >= params.min_fixation_duration_sec {
                    runs.push(IvtRun {
                        start,
                        end,
                        duration_sec: duration,
                    });
                }
            }
        }
        *run_start = None;
    };

    let mut previous: Option<(f64, f64, f64)> = None; // (x, y, t)
    for (idx, frame) in frames.iter().enumerate() {
        let t = media_time(frame);
        let (x, y) = (frame.gaze.x_norm, frame.gaze.y_norm);
        if let Some((px, py, pt)) = previous {
            let dt = t - pt;
            if dt <= 0.0 {
                close_run(&mut run_start, &mut runs, idx);
                previous = Some((x, y, t));
                continue;
            }
            let distance = ((x - px) * (x - px) + (y - py) * (y - py)).sqrt();
            let velocity = distance / dt;
            if velocity >= params.ivt_velocity_units_per_sec {
                close_run(&mut run_start, &mut runs, idx);
            } else if run_start.is_none() {
                run_start = Some(idx - 1);
            }
        } else {
            run_start = Some(idx);
        }
        previous = Some((x, y, t));
    }
    close_run(&mut run_start, &mut runs, frames.len());
    runs
}

fn fixation_stats_refs(frames: &[&GazeFrame], params: &MetricsParams) -> FixationStats {
    if frames.len() <= 1 {
        let empty = frames.is_empty();
        return FixationStats {
            count: if empty { 0 } else { 1 },
            mean_duration_ms: if empty {
                0.0
            } else {
                params.min_fixation_duration_sec * 1000.0
            },
        };
    }

    let runs = ivt_runs(frames, params);
    let mean_duration_ms = if runs.is_empty() {
        0.0
    } else {
        sum(runs.iter().map(|r| r.duration_sec)) / runs.len() as f64 * 1000.0
    };
    FixationStats {
        count: runs.len() as i64,
        mean_duration_ms,
    }
}

/// One I-VT fixation for plotting (RPX-1 `fixations`): a run the fixation count of its clip
/// counted. `first_frame..end_frame` index the `frames` slice given to [`fixations_with`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fixation {
    pub stimulus_id: String,
    /// Media time of the first and last frame of the run, seconds.
    pub start_sec: f64,
    pub end_sec: f64,
    pub duration_ms: f64,
    /// Mean point of the run, screen-normalised.
    pub x_norm: f64,
    pub y_norm: f64,
    pub first_frame: usize,
    /// Exclusive.
    pub end_frame: usize,
}

/// The fixations [`extract_with`] counts, per clip in first-appearance order (a clip with a
/// single valid frame counts one fixation in the metrics but has no run, so none here).
pub fn fixations_with(frames: &[GazeFrame], params: &MetricsParams) -> Vec<Fixation> {
    let mut out = Vec::new();
    for (id, indices) in group_indices(frames) {
        let valid: Vec<usize> = indices
            .into_iter()
            .filter(|&i| is_valid_frame(&frames[i]))
            .collect();
        let refs: Vec<&GazeFrame> = valid.iter().map(|&i| &frames[i]).collect();
        if refs.len() <= 1 {
            continue;
        }
        for run in ivt_runs(&refs, params) {
            let members = &refs[run.start..run.end];
            let n = members.len() as f64;
            out.push(Fixation {
                stimulus_id: id.to_string(),
                start_sec: media_time(members[0]),
                end_sec: media_time(members[members.len() - 1]),
                duration_ms: run.duration_sec * 1000.0,
                x_norm: sum(members.iter().map(|f| f.gaze.x_norm)) / n,
                y_norm: sum(members.iter().map(|f| f.gaze.y_norm)) / n,
                first_frame: valid[run.start],
                end_frame: valid[run.end - 1] + 1,
            });
        }
    }
    out
}

/// Left fold from `0.0`, like Swift `reduce(0, +)`.
pub(crate) fn sum(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, |acc, v| acc + v)
}

/// Groups frames by `stimulusId ?? "session_untagged"` in first-appearance order and
/// extracts each clip with the built-in AOI roles (`PerVideoMetricsExtractor.extract`).
pub fn extract(frames: &[GazeFrame]) -> Vec<PerVideoMetricSet> {
    extract_with_roles(frames, &AoiRole::builtin())
}

/// [`extract`] with catalog-driven AOI roles.
pub fn extract_with_roles(frames: &[GazeFrame], roles: &AoiRole) -> Vec<PerVideoMetricSet> {
    extract_with(
        frames,
        roles,
        &MetricsParams::default(),
        &AoiParams::default(),
    )
}

/// PRM-9: [`extract_with_roles`] with the profile's `metrics` and `aoi` sections.
///
/// `aoi` is taken so every caller hands over the whole view; the AOI hit test itself happens
/// where the frames are built (`activeAOINames`: on the device, or in `gaze::reprocess` with
/// `aoi.marginNorm`), and the dwell metrics are frame-count ratios (the Swift oracle), so no
/// `aoi` member changes the result of this function today.
pub fn extract_with(
    frames: &[GazeFrame],
    roles: &AoiRole,
    metrics: &MetricsParams,
    aoi: &AoiParams,
) -> Vec<PerVideoMetricSet> {
    let _ = aoi;
    group_indices(frames)
        .into_iter()
        .map(|(id, indices)| {
            let clip_frames: Vec<&GazeFrame> = indices.into_iter().map(|i| &frames[i]).collect();
            extract_clip_refs(id, &clip_frames, 0.0, roles, metrics)
        })
        .collect()
}

/// Frame indices per `stimulusId ?? "session_untagged"`, in first-appearance order.
fn group_indices(frames: &[GazeFrame]) -> Vec<(&str, Vec<usize>)> {
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    let mut index_by_id: HashMap<&str, usize> = HashMap::new();
    for (i, frame) in frames.iter().enumerate() {
        let id = frame.stimulus_id.as_deref().unwrap_or(UNTAGGED_CLIP_ID);
        match index_by_id.get(id) {
            Some(&idx) => groups[idx].1.push(i),
            None => {
                index_by_id.insert(id, groups.len());
                groups.push((id, vec![i]));
            }
        }
    }
    groups
}

/// One clip (or a time section of one, with `time_origin_sec` the section start), built-in
/// AOI roles (`PerVideoMetricsExtractor.extractClip`).
pub fn extract_clip(
    stimulus_id: &str,
    frames: &[GazeFrame],
    time_origin_sec: f64,
) -> PerVideoMetricSet {
    extract_clip_with_roles(stimulus_id, frames, time_origin_sec, &AoiRole::builtin())
}

/// [`extract_clip`] with catalog-driven AOI roles.
pub fn extract_clip_with_roles(
    stimulus_id: &str,
    frames: &[GazeFrame],
    time_origin_sec: f64,
    roles: &AoiRole,
) -> PerVideoMetricSet {
    extract_clip_with(
        stimulus_id,
        frames,
        time_origin_sec,
        roles,
        &MetricsParams::default(),
    )
}

/// [`extract_clip_with_roles`] with the profile's `metrics` section.
pub fn extract_clip_with(
    stimulus_id: &str,
    frames: &[GazeFrame],
    time_origin_sec: f64,
    roles: &AoiRole,
    metrics: &MetricsParams,
) -> PerVideoMetricSet {
    extract_clip_refs(
        stimulus_id,
        &frames.iter().collect::<Vec<_>>(),
        time_origin_sec,
        roles,
        metrics,
    )
}

fn extract_clip_refs(
    stimulus_id: &str,
    frames: &[&GazeFrame],
    time_origin_sec: f64,
    roles: &AoiRole,
    params: &MetricsParams,
) -> PerVideoMetricSet {
    let valid: Vec<&GazeFrame> = frames
        .iter()
        .copied()
        .filter(|f| is_valid_frame(f))
        .collect();
    let valid_sample_ratio = if frames.is_empty() {
        0.0
    } else {
        valid.len() as f64 / frames.len() as f64
    };

    let mut metrics: BTreeMap<String, f64> = BTreeMap::new();
    let mut aoi_dwell_ratios: BTreeMap<String, f64> = BTreeMap::new();
    let mut put = |key: PerVideoMetricKey, value: f64| {
        metrics.insert(key.key(None), value);
    };

    // AOI dwell ratios (over valid frames) and first-entry media times.
    let mut hit_counts: BTreeMap<&str, i64> = BTreeMap::new();
    let mut first_entry: BTreeMap<&str, f64> = BTreeMap::new();
    for frame in &valid {
        let t = media_time(frame);
        for name in &frame.active_aoi_names {
            *hit_counts.entry(name.as_str()).or_insert(0) += 1;
            first_entry
                .entry(name.as_str())
                .and_modify(|existing| *existing = swift_min(*existing, t))
                .or_insert(t);
        }
    }
    let denominator = valid.len().max(1) as f64;
    let mut aoi_metrics: Vec<(String, f64)> = Vec::new();
    for (name, hits) in &hit_counts {
        let ratio = *hits as f64 / denominator;
        aoi_dwell_ratios.insert((*name).to_string(), ratio);
        aoi_metrics.push((PerVideoMetricKey::AoiDwellRatio.key(Some(name)), ratio));
    }
    let observed_duration = observed_duration_refs(frames, params.fallback_frame_interval_sec);
    for (name, entry) in &first_entry {
        let latency_sec = swift_max(0.0, entry - time_origin_sec);
        aoi_metrics.push((
            PerVideoMetricKey::FirstEntryLatencyMs.key(Some(name)),
            latency_sec * 1000.0,
        ));
    }

    // GeoPref-style preference: literal `Social_Actor` / `Geometric_Pattern` names (not roles).
    let social_hits = hit_counts.get("Social_Actor").copied().unwrap_or(0) as f64;
    let geometric_hits = hit_counts.get("Geometric_Pattern").copied().unwrap_or(0) as f64;
    let total_geo = social_hits + geometric_hits;
    put(
        PerVideoMetricKey::SocialDwellRatio,
        social_hits / denominator,
    );
    put(
        PerVideoMetricKey::GeometricDwellRatio,
        geometric_hits / denominator,
    );
    put(
        PerVideoMetricKey::SocialPreferenceScore,
        if total_geo > 0.0 {
            (social_hits - geometric_hits) / total_geo
        } else {
            0.0
        },
    );

    // Face scanning: eye vs mouth share of face-feature hits (role-based).
    let role_hits = |kind: AoiRoleKind| {
        sum(hit_counts.iter().map(|(name, hits)| {
            if roles.matches(name, kind) {
                *hits as f64
            } else {
                0.0
            }
        }))
    };
    let eye_hits = role_hits(AoiRoleKind::Eye);
    let mouth_hits = role_hits(AoiRoleKind::Mouth);
    let face_hits = eye_hits + mouth_hits;
    put(
        PerVideoMetricKey::EyeDwellRatio,
        if face_hits > 0.0 {
            eye_hits / face_hits
        } else {
            0.0
        },
    );
    put(
        PerVideoMetricKey::MouthDwellRatio,
        if face_hits > 0.0 {
            mouth_hits / face_hits
        } else {
            0.0
        },
    );

    // Fixations.
    let fixation = fixation_stats_refs(&valid, params);
    put(PerVideoMetricKey::FixationCount, fixation.count as f64);
    put(
        PerVideoMetricKey::MeanFixationDurationMs,
        fixation.mean_duration_ms,
    );

    // Facial affect.
    if valid.is_empty() {
        put(PerVideoMetricKey::SocialSmileIntensityMean, 0.0);
        put(PerVideoMetricKey::SmileReactivityIndex, 0.0);
        put(PerVideoMetricKey::FlatAffectScore, 1.0);
        put(PerVideoMetricKey::SurpriseScoreMean, 0.0);
    } else {
        let n = valid.len() as f64;
        let smiles: Vec<f64> = valid.iter().map(|f| smile_of(f)).collect();
        let mean_smile = sum(smiles.iter().copied()) / n;
        let mean_expressivity = sum(valid.iter().map(|f| expressivity_of(f))) / n;
        put(PerVideoMetricKey::SocialSmileIntensityMean, mean_smile);

        // Clip-level reactivity: peak smile minus the onset window (500 ms by default), local
        // to the origin.
        let baseline: Vec<f64> = valid
            .iter()
            .filter_map(|f| {
                let local = media_time(f) - time_origin_sec;
                (0.0..=params.smile_reactivity_window_sec)
                    .contains(&local)
                    .then(|| smile_of(f))
            })
            .collect();
        let baseline_mean = if baseline.is_empty() {
            mean_smile
        } else {
            sum(baseline.iter().copied()) / baseline.len() as f64
        };
        let peak = smiles.iter().copied().reduce(sequence_max).unwrap_or(0.0);
        put(
            PerVideoMetricKey::SmileReactivityIndex,
            clamp01(peak - baseline_mean),
        );
        put(
            PerVideoMetricKey::FlatAffectScore,
            clamp01(1.0 - (mean_expressivity * params.flat_affect_gain)),
        );
        put(
            PerVideoMetricKey::SurpriseScoreMean,
            sum(valid.iter().map(|f| surprise_of(f))) / n,
        );
    }

    put(PerVideoMetricKey::ValidSampleRatio, valid_sample_ratio);
    metrics.extend(aoi_metrics);

    PerVideoMetricSet {
        stimulus_id: stimulus_id.to_string(),
        clip_frame_count: frames.len() as i64,
        valid_frame_count: valid.len() as i64,
        valid_sample_ratio,
        observed_duration_sec: observed_duration,
        aoi_dwell_ratios,
        metrics,
    }
}

pub(crate) fn smile_of(frame: &GazeFrame) -> f64 {
    frame.affect.social_smile_intensity.unwrap_or(0.0)
}

pub(crate) fn expressivity_of(frame: &GazeFrame) -> f64 {
    frame.affect.expressivity_index.unwrap_or(0.0)
}

pub(crate) fn surprise_of(frame: &GazeFrame) -> f64 {
    frame.affect.surprise_score.unwrap_or(0.0)
}

/// Metric sets rebuilt from a session's own score reports (`rawPayload.perVideo`), for a
/// re-score without frames (RPX-2 `Level::Score`). Moved here from `asd-wasm::session`
/// (`metric_sets_from_reports`); see [`from_reports_with_warnings`] for what it cannot recover.
pub fn from_reports(per_video: &[PerVideoScoreReport]) -> Vec<PerVideoMetricSet> {
    from_reports_with_warnings(per_video).0
}

/// [`from_reports`] plus a warning for everything a report cannot carry. One set per report,
/// in report order.
///
/// - `metrics`: `metricKey -> observed` of every non-imputed row. Imputed rows are left
///   out so the scorer imputes them again exactly as the device did.
/// - `observedDurationSec`: recovered from an imputed `first_entry_latency_ms` row
///   (`observed / 1000`); `0` when there is none (the scorer's 0.5 s floor then applies).
/// - `validSampleRatio`: as reported. Frame counts and AOI dwell ratios are `0`/empty;
///   the scorer does not read them.
///
/// Limits (each reported as a warning): segment windows need frames, so a metric whose
/// windowed values differ between segments keeps its first value for every segment; a clip
/// that had no baseline at capture has no rows and so no metrics.
pub fn from_reports_with_warnings(
    per_video: &[PerVideoScoreReport],
) -> (Vec<PerVideoMetricSet>, Vec<String>) {
    let latency = PerVideoMetricKey::FirstEntryLatencyMs.as_str();
    let mut warnings = Vec::new();
    let sets = per_video
        .iter()
        .map(|report| {
            let mut metrics: BTreeMap<String, f64> = BTreeMap::new();
            let mut duration: Option<f64> = None;
            for row in &report.rows {
                if row.imputed {
                    if row.biomarker == latency && duration.is_none() {
                        duration = Some(row.observed / 1000.0);
                    }
                    continue;
                }
                match metrics.get(&row.metric_key) {
                    None => {
                        metrics.insert(row.metric_key.clone(), row.observed);
                    }
                    Some(first) if first.to_bits() != row.observed.to_bits() => {
                        warnings.push(format!(
                            "{}: {} differs between segments ({} vs {}); without frames every \
                             segment is scored with the first value",
                            report.stimulus_id, row.metric_key, first, row.observed
                        ));
                    }
                    Some(_) => {}
                }
            }
            if !report.has_baseline {
                warnings.push(format!(
                    "{}: had no baseline at capture, so the session carries no metrics for \
                     it; pass frames to score it",
                    report.stimulus_id
                ));
            }
            PerVideoMetricSet {
                stimulus_id: report.stimulus_id.clone(),
                clip_frame_count: 0,
                valid_frame_count: 0,
                valid_sample_ratio: report.valid_sample_ratio,
                observed_duration_sec: duration.unwrap_or(0.0),
                aoi_dwell_ratios: BTreeMap::new(),
                metrics,
            }
        })
        .collect();
    (sets, warnings)
}

#[cfg(test)]
mod from_reports_tests {
    use crate::types::{ExpectationStatus, NormativeDirection, PerVideoScoreRow};

    use super::*;

    fn row(segment: &str, key: &str, observed: f64, imputed: bool) -> PerVideoScoreRow {
        PerVideoScoreRow {
            segment_id: segment.into(),
            segment_description: String::new(),
            biomarker: key.split("::").next().unwrap_or(key).into(),
            target_aoi: None,
            metric_key: key.into(),
            direction: NormativeDirection::HigherBetter,
            observed,
            expected: None,
            acceptable_low: None,
            acceptable_high: None,
            status: ExpectationStatus::OnTarget,
            score: 1.0,
            imputed,
            citation: String::new(),
            source: String::new(),
        }
    }

    fn report(id: &str, rows: Vec<PerVideoScoreRow>, has_baseline: bool) -> PerVideoScoreReport {
        PerVideoScoreReport {
            stimulus_id: id.into(),
            title: id.into(),
            category: String::new(),
            rows,
            subscore: 0.0,
            valid_sample_ratio: 0.8,
            low_confidence: false,
            has_baseline,
            weight: 1.0,
        }
    }

    #[test]
    fn rows_become_metrics_and_imputed_latency_gives_the_duration() {
        let reports = [report(
            "clip",
            vec![
                row("a", "social_dwell_ratio", 0.7, false),
                row("a", "first_entry_latency_ms", 8000.0, true),
                row("b", "social_dwell_ratio", 0.7, false),
                row("b", "aoi_dwell_ratio::eyes", 0.3, false),
            ],
            true,
        )];
        let (sets, warnings) = from_reports_with_warnings(&reports);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(sets, from_reports(&reports));
        let set = &sets[0];
        assert_eq!(set.stimulus_id, "clip");
        assert_eq!(set.valid_sample_ratio, 0.8);
        assert_eq!(set.observed_duration_sec, 8.0);
        assert_eq!(
            set.metrics,
            BTreeMap::from([
                ("aoi_dwell_ratio::eyes".to_owned(), 0.3),
                ("social_dwell_ratio".to_owned(), 0.7),
            ])
        );
    }

    #[test]
    fn segment_conflicts_and_unscored_clips_are_warned() {
        let reports = [
            report(
                "clip",
                vec![
                    row("a", "social_dwell_ratio", 0.7, false),
                    row("b", "social_dwell_ratio", 0.4, false),
                ],
                true,
            ),
            report("other", Vec::new(), false),
        ];
        let (sets, warnings) = from_reports_with_warnings(&reports);
        assert_eq!(sets[0].metrics["social_dwell_ratio"], 0.7);
        assert_eq!(sets[0].observed_duration_sec, 0.0);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("clip: social_dwell_ratio differs"));
        assert!(warnings[1].starts_with("other: had no baseline"));
    }
}
