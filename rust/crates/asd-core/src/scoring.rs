//! owner: WP-R.1
//!
//! Expected-vs-recorded scoring: port of `PerVideoScoringEngine` (`PerVideoScoringEngine.swift`).
//! CORE-1..3.
//!
//! Status bands: `onTarget` meets the expected range, `monitor` is in the tolerance zone,
//! `atypical` is beyond it. Continuous scores are piecewise-linear in [0, 1] with 0 at one
//! band-width beyond the tolerance edge. The composite is the weighted mean of per-video
//! subscores over the clips that have a baseline profile.
//!
//! Every arithmetic step keeps the Swift operand order so results are bit-identical, not
//! just within the 1e-9 parity tolerance.

use std::collections::BTreeMap;

use crate::baselines::{bracket_id_for, range_for};
use crate::metrics;
use crate::types::{
    AgeBracket, ExpectationStatus, GazeFrame, NormativeBaselineSet, NormativeDirection,
    NormativeRange, NormativeSegment, NormativeVideo, PerVideoMetricSet, PerVideoScoreReport,
    PerVideoScoreRow, PerVideoSessionScore,
};

/// Clips below this valid-sample ratio are marked low-confidence (the 0.60 vault gate).
pub const LOW_CONFIDENCE_VALID_RATIO: f64 = 0.60;

/// Clip id for frames captured without a playlist (`PerVideoMetricsExtractor.untaggedClipId`).
pub use crate::metrics::UNTAGGED_CLIP_ID;

/// Biomarker whose missing value is imputed at the clip duration.
const FIRST_ENTRY_LATENCY_MS: &str = "first_entry_latency_ms";

/// Bracket used when the baseline declares no brackets at all.
const FALLBACK_BRACKET_ID: &str = "default";

/// Scores every metric set against the baseline and builds the session matrix.
///
/// `frames` are all session frames; they are grouped by `stimulusId` (untagged frames go to
/// [`UNTAGGED_CLIP_ID`]) so each clip can be scored per segment window. `scored_at_utc` is
/// written verbatim; use [`format_scored_at`] to produce it the way Swift does.
pub fn score_session(
    metric_sets: &[PerVideoMetricSet],
    baseline: &NormativeBaselineSet,
    age_months: i64,
    frames: &[GazeFrame],
    scored_at_utc: &str,
) -> PerVideoSessionScore {
    let bracket_id = resolved_bracket_id(baseline, age_months);
    let mut frames_by_clip: BTreeMap<&str, Vec<GazeFrame>> = BTreeMap::new();
    for frame in frames {
        let id = frame.stimulus_id.as_deref().unwrap_or(UNTAGGED_CLIP_ID);
        frames_by_clip.entry(id).or_default().push(frame.clone());
    }

    let mut reports = Vec::with_capacity(metric_sets.len());
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for metric_set in metric_sets {
        let Some(video) = baseline.videos.get(&metric_set.stimulus_id) else {
            reports.push(PerVideoScoreReport {
                stimulus_id: metric_set.stimulus_id.clone(),
                title: metric_set.stimulus_id.clone(),
                category: "Unattributed".to_owned(),
                rows: Vec::new(),
                subscore: 0.0,
                valid_sample_ratio: metric_set.valid_sample_ratio,
                low_confidence: metric_set.valid_sample_ratio < LOW_CONFIDENCE_VALID_RATIO,
                has_baseline: false,
                weight: 0.0,
            });
            continue;
        };

        let clip_frames = frames_by_clip
            .get(metric_set.stimulus_id.as_str())
            .map_or(&[][..], Vec::as_slice);
        let report = score_video(metric_set, video, &bracket_id, clip_frames);
        weighted_sum += video.weight * report.subscore;
        weight_sum += video.weight;
        reports.push(report);
    }

    let composite = if weight_sum > 0.0 {
        weighted_sum / weight_sum
    } else {
        0.0
    };

    PerVideoSessionScore {
        age_bracket_id: bracket_id,
        age_months,
        composite_score: composite,
        videos: reports,
        baseline_status: baseline.status.clone(),
        baseline_schema_version: baseline.schema_version.clone(),
        scored_at_utc: scored_at_utc.to_owned(),
    }
}

/// Bracket id for an age, clamped to the nearest declared bracket for out-of-range ages
/// (first declared wins a tie); `"default"` when the set declares no brackets.
pub fn resolved_bracket_id(baseline: &NormativeBaselineSet, age_months: i64) -> String {
    if let Some(id) = bracket_id_for(baseline, age_months) {
        return id.to_owned();
    }
    let mut nearest: Option<(&AgeBracket, i64)> = None;
    for bracket in &baseline.age_brackets {
        let d = distance(age_months, bracket);
        // Strictly closer replaces: Swift `min(by:)` keeps the first of equal elements.
        if nearest.is_none_or(|(_, best)| d < best) {
            nearest = Some((bracket, d));
        }
    }
    nearest.map_or_else(|| FALLBACK_BRACKET_ID.to_owned(), |(b, _)| b.id.clone())
}

fn distance(age_months: i64, bracket: &AgeBracket) -> i64 {
    if age_months < bracket.min_months {
        bracket.min_months - age_months
    } else if age_months > bracket.max_months {
        age_months - bracket.max_months
    } else {
        0
    }
}

/// Scores one clip that has a baseline profile.
///
/// A missing `first_entry_latency_ms` is imputed at `max(observedDurationSec, 0.5) * 1000`
/// and marked `imputed`; other missing metrics and expectations without a range for the
/// bracket (or `default`) are skipped. `clip_frames` enables per-segment windows.
pub fn score_video(
    metric_set: &PerVideoMetricSet,
    video: &NormativeVideo,
    bracket_id: &str,
    clip_frames: &[GazeFrame],
) -> PerVideoScoreReport {
    let clip_duration_ms = swift_max(metric_set.observed_duration_sec, 0.5) * 1000.0;
    let mut rows = Vec::new();

    for segment in &video.segments {
        let windowed = segment_metrics(segment, clip_frames, metric_set);
        let lookup = windowed.as_ref().unwrap_or(&metric_set.metrics);
        for expectation in &segment.expectations {
            let metric_key =
                composed_key(&expectation.biomarker, expectation.target_aoi.as_deref());
            let (observed, imputed) = match lookup.get(&metric_key) {
                Some(&value) => (value, false),
                None if expectation.biomarker == FIRST_ENTRY_LATENCY_MS => {
                    // Never fixated the target AOI: worst case for lower-is-better latency.
                    (clip_duration_ms, true)
                }
                // Not produced for this clip and not imputable: skip rather than fabricate.
                None => continue,
            };
            let Some(range) = range_for(expectation, bracket_id) else {
                continue;
            };

            let (status, score) = evaluate(observed, expectation.direction, range);
            rows.push(PerVideoScoreRow {
                segment_id: segment.id.clone(),
                segment_description: segment.description.clone(),
                biomarker: expectation.biomarker.clone(),
                target_aoi: expectation.target_aoi.clone(),
                metric_key,
                direction: expectation.direction,
                observed,
                expected: range.expected,
                acceptable_low: range.acceptable_low,
                acceptable_high: range.acceptable_high,
                status,
                score,
                imputed,
                citation: expectation.citation.clone(),
                source: expectation.source.clone(),
            });
        }
    }

    let subscore = if rows.is_empty() {
        0.0
    } else {
        rows.iter().fold(0.0, |acc, r| acc + r.score) / rows.len() as f64
    };

    PerVideoScoreReport {
        stimulus_id: metric_set.stimulus_id.clone(),
        title: video.title.clone(),
        category: video.category.clone(),
        rows,
        subscore,
        valid_sample_ratio: metric_set.valid_sample_ratio,
        low_confidence: metric_set.valid_sample_ratio < LOW_CONFIDENCE_VALID_RATIO,
        has_baseline: true,
        weight: video.weight,
    }
}

/// Metrics for one segment window `[startSec, endSec)` on `clipMediaTimeSec ?? videoTimeSec`.
/// `None` means "use the whole-clip metric set": no frames, or none inside the window.
fn segment_metrics(
    segment: &NormativeSegment,
    clip_frames: &[GazeFrame],
    fallback: &PerVideoMetricSet,
) -> Option<BTreeMap<String, f64>> {
    if clip_frames.is_empty() {
        return None;
    }
    let window: Vec<GazeFrame> = clip_frames
        .iter()
        .filter(|f| {
            let t = f.gaze.clip_media_time_sec.unwrap_or(f.gaze.video_time_sec);
            t >= segment.start_sec && t < segment.end_sec
        })
        .cloned()
        .collect();
    if window.is_empty() {
        return None;
    }
    Some(metrics::extract_clip(&fallback.stimulus_id, &window, segment.start_sec).metrics)
}

/// Metrics lookup key: `biomarker`, or `biomarker::AOI` when the AOI is non-empty.
pub fn composed_key(biomarker: &str, target_aoi: Option<&str>) -> String {
    match target_aoi {
        Some(aoi) if !aoi.is_empty() => format!("{biomarker}::{aoi}"),
        _ => biomarker.to_owned(),
    }
}

/// Status band and continuous score of one observation.
///
/// Ported as is, including two Swift quirks the golden vectors pin: a degenerate
/// `lowerBetter` range (expected == high) scores through [`piecewise`]'s
/// "observed >= goodEnd" rule, so an on-target value can score 0 and an atypical one 1;
/// and `band` without a proper `low < high` pair reports `onTarget`/1.0 when
/// `observed >= expected`, else `monitor`/0.5.
pub fn evaluate(
    observed: f64,
    direction: NormativeDirection,
    range: &NormativeRange,
) -> (ExpectationStatus, f64) {
    match direction {
        NormativeDirection::HigherBetter => {
            let expected = range.expected.or(range.acceptable_low).unwrap_or(0.0);
            let low = range.acceptable_low.unwrap_or(expected);
            let status = if observed >= expected {
                ExpectationStatus::OnTarget
            } else if observed >= low {
                ExpectationStatus::Monitor
            } else {
                ExpectationStatus::Atypical
            };
            (
                status,
                piecewise(observed, expected, low - (expected - low)),
            )
        }
        NormativeDirection::LowerBetter => {
            let expected = range.expected.or(range.acceptable_high).unwrap_or(0.0);
            let high = range.acceptable_high.unwrap_or(expected);
            let status = if observed <= expected {
                ExpectationStatus::OnTarget
            } else if observed <= high {
                ExpectationStatus::Monitor
            } else {
                ExpectationStatus::Atypical
            };
            (
                status,
                piecewise(observed, expected, high + (high - expected)),
            )
        }
        NormativeDirection::Band => {
            let (low, high) = match (range.acceptable_low, range.acceptable_high) {
                (Some(low), Some(high)) if high > low => (low, high),
                _ => {
                    let expected = range.expected.unwrap_or(0.0);
                    return if observed >= expected {
                        (ExpectationStatus::OnTarget, 1.0)
                    } else {
                        (ExpectationStatus::Monitor, 0.5)
                    };
                }
            };
            let center = (low + high) / 2.0;
            let half_width = (high - low) / 2.0;
            let distance = (observed - center).abs();
            let status = if distance <= half_width {
                ExpectationStatus::OnTarget
            } else if distance <= half_width * 2.0 {
                ExpectationStatus::Monitor
            } else {
                ExpectationStatus::Atypical
            };
            let score = swift_clamp01(1.0 - swift_max(0.0, distance - half_width) / half_width);
            (status, score)
        }
    }
}

/// Piecewise-linear goodness: 1 at `good_end`, 0 at `bad_end`, linear between, clamped
/// beyond. Equal ends: 1 when `observed >= good_end`, else 0.
pub fn piecewise(observed: f64, good_end: f64, bad_end: f64) -> f64 {
    if good_end == bad_end {
        return if observed >= good_end { 1.0 } else { 0.0 };
    }
    let t = (observed - bad_end) / (good_end - bad_end);
    swift_clamp01(t)
}

/// `max(0.0, min(1.0, t))` with Swift's NaN behaviour (`min(1.0, nan)` is 1.0 in Swift,
/// then `max(0.0, 1.0)`), which `f64::clamp` would not reproduce (it returns NaN).
fn swift_clamp01(t: f64) -> f64 {
    swift_max(0.0, swift_min(1.0, t))
}

/// Swift `Swift.min(x, y)`: `y < x ? y : x`.
fn swift_min(x: f64, y: f64) -> f64 {
    if y < x {
        y
    } else {
        x
    }
}

/// Swift `Swift.max(x, y)`: `y >= x ? y : x`.
fn swift_max(x: f64, y: f64) -> f64 {
    if y >= x {
        y
    } else {
        x
    }
}

/// Seconds between the Unix epoch and Foundation's reference date (2001-01-01).
const FOUNDATION_REFERENCE_OFFSET_SEC: f64 = 978_307_200.0;

/// Formats an instant like Swift `ISO8601DateFormatter().string(from:)` with default
/// options: UTC, whole seconds, `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Foundation first rounds to milliseconds and then drops the fraction, so a fraction of
/// 0.9995 s or more shows the next second. The arithmetic follows `CFDateFormatter`:
/// `Date(timeIntervalSince1970:)` stores the time since the reference date, which becomes
/// ICU milliseconds as `(t + 978307200) * 1000 + 0.5`, floored by ICU (golden
/// `scored_at_ms_rounding_v1.json`). The round trip through the reference date is kept on
/// purpose: near the epoch it loses low bits, and Swift's output depends on that.
pub fn format_scored_at(unix_seconds: f64) -> String {
    let since_reference = unix_seconds - FOUNDATION_REFERENCE_OFFSET_SEC;
    let icu_ms = ((since_reference + FOUNDATION_REFERENCE_OFFSET_SEC) * 1000.0 + 0.5).floor();
    // `as` saturates out-of-range values and maps NaN to 0: never a panic.
    let secs = (icu_ms as i64).div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (H. Hinnant, `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_score_clamps_like_swift() {
        let r = NormativeRange {
            expected: None,
            acceptable_low: Some(0.25),
            acceptable_high: Some(0.45),
        };
        assert_eq!(
            evaluate(0.35, NormativeDirection::Band, &r),
            (ExpectationStatus::OnTarget, 1.0)
        );
        assert_eq!(evaluate(5.0, NormativeDirection::Band, &r).1, 0.0);
    }

    #[test]
    fn nan_follows_swift_min_max() {
        // Swift `min(1.0, .nan)` is 1.0 and `max(.nan, 0.5)` is NaN; Rust's f64 methods differ.
        assert_eq!(piecewise(f64::NAN, 1.0, 0.0), 1.0);
        assert!(swift_max(f64::NAN, 0.5).is_nan());
        assert_eq!(swift_max(0.0, f64::NAN), 0.0);
    }

    #[test]
    fn civil_dates() {
        assert_eq!(format_scored_at(0.0), "1970-01-01T00:00:00Z");
        assert_eq!(format_scored_at(951_782_400.0), "2000-02-29T00:00:00Z");
        assert_eq!(format_scored_at(-0.5), "1969-12-31T23:59:59Z");
    }
}
