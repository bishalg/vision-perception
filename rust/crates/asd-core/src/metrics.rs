//! owner: WP-R.2
//!
//! CORE-5: per-clip metric extraction, a 1:1 port of `PerVideoMetricsExtractor`,
//! `PerVideoMetricKey` and `AOIRole` (`PerVideoMetrics.swift`, `SessionBiomarkerAggregator.swift`
//! at 9655fcc). The Swift bodies are the oracle; the golden vectors in
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

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::types::{GazeFrame, GazeQualityVerdict, PerVideoMetricSet, StimulusCatalog};

/// Fallback clip id for frames captured without a playlist (`untaggedClipId`).
pub const UNTAGGED_CLIP_ID: &str = "session_untagged";
/// I-VT velocity threshold in normalised viewport units per second.
pub const VELOCITY_THRESHOLD_UNITS_PER_SEC: f64 = 5.0;
/// Minimum fixation duration in seconds (core `OculomotorClassifier`: 80 ms).
pub const MIN_FIXATION_DURATION_SEC: f64 = 0.08;
/// Nominal 60 Hz frame interval.
pub const FRAME_INTERVAL_SEC: f64 = 1.0 / 60.0;

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
    observed_duration_refs(&frames.iter().collect::<Vec<_>>())
}

fn observed_duration_refs(frames: &[&GazeFrame]) -> f64 {
    let max_media = frames
        .iter()
        .filter_map(|f| f.gaze.clip_media_time_sec)
        .reduce(sequence_max);
    match max_media {
        Some(m) if m > 0.0 => m,
        _ => frames.len() as f64 * FRAME_INTERVAL_SEC,
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
    fixation_stats_refs(&valid_frames.iter().collect::<Vec<_>>())
}

fn fixation_stats_refs(frames: &[&GazeFrame]) -> FixationStats {
    if frames.len() <= 1 {
        let empty = frames.is_empty();
        return FixationStats {
            count: if empty { 0 } else { 1 },
            mean_duration_ms: if empty {
                0.0
            } else {
                MIN_FIXATION_DURATION_SEC * 1000.0
            },
        };
    }

    let mut durations: Vec<f64> = Vec::new();
    let mut run_start: Option<usize> = None;

    let close_run = |run_start: &mut Option<usize>, durations: &mut Vec<f64>, end: usize| {
        if let Some(start) = *run_start {
            if end > start + 1 {
                let duration_sec = media_time(frames[end - 1]) - media_time(frames[start]);
                let fallback = (end - start) as f64 * FRAME_INTERVAL_SEC;
                let duration = if duration_sec > 0.0 {
                    duration_sec
                } else {
                    fallback
                };
                if duration >= MIN_FIXATION_DURATION_SEC {
                    durations.push(duration);
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
                close_run(&mut run_start, &mut durations, idx);
                previous = Some((x, y, t));
                continue;
            }
            let distance = ((x - px) * (x - px) + (y - py) * (y - py)).sqrt();
            let velocity = distance / dt;
            if velocity >= VELOCITY_THRESHOLD_UNITS_PER_SEC {
                close_run(&mut run_start, &mut durations, idx);
            } else if run_start.is_none() {
                run_start = Some(idx - 1);
            }
        } else {
            run_start = Some(idx);
        }
        previous = Some((x, y, t));
    }
    close_run(&mut run_start, &mut durations, frames.len());

    let mean_duration_ms = if durations.is_empty() {
        0.0
    } else {
        sum(durations.iter().copied()) / durations.len() as f64 * 1000.0
    };
    FixationStats {
        count: durations.len() as i64,
        mean_duration_ms,
    }
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
    let mut groups: Vec<(&str, Vec<&GazeFrame>)> = Vec::new();
    let mut index_by_id: HashMap<&str, usize> = HashMap::new();
    for frame in frames {
        let id = frame.stimulus_id.as_deref().unwrap_or(UNTAGGED_CLIP_ID);
        match index_by_id.get(id) {
            Some(&idx) => groups[idx].1.push(frame),
            None => {
                index_by_id.insert(id, groups.len());
                groups.push((id, vec![frame]));
            }
        }
    }
    groups
        .into_iter()
        .map(|(id, clip_frames)| extract_clip_refs(id, &clip_frames, 0.0, roles))
        .collect()
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
    extract_clip_refs(
        stimulus_id,
        &frames.iter().collect::<Vec<_>>(),
        time_origin_sec,
        roles,
    )
}

fn extract_clip_refs(
    stimulus_id: &str,
    frames: &[&GazeFrame],
    time_origin_sec: f64,
    roles: &AoiRole,
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
    let observed_duration = observed_duration_refs(frames);
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
    let fixation = fixation_stats_refs(&valid);
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

        // Clip-level reactivity: peak smile minus the first 500 ms (local to the origin).
        let baseline: Vec<f64> = valid
            .iter()
            .filter_map(|f| {
                let local = media_time(f) - time_origin_sec;
                (0.0..=0.5).contains(&local).then(|| smile_of(f))
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
            clamp01(1.0 - (mean_expressivity * 50.0)),
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
