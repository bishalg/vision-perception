//! Windowed metric traces over clip time and the expected band from the published baselines
//! (ADR-0015 V-2).
//!
//! A trace point at time `t` summarises the frames with media time in `[t - w/2, t + w/2)`
//! (a centred window); points run from the clip's first frame time in steps of `step`
//! (`t_k = t_first + k * step`) up to its last frame time. The dwell values are frame-count
//! ratios over the window's valid frames, like the clip metrics, so one window spanning the
//! whole clip reproduces the clip's `aoi_dwell_ratio::<AOI>` and `valid_sample_ratio`.

use serde::{Deserialize, Serialize};

use super::aoi::{ClipAois, HitParams};
use super::ReplayError;
use crate::baselines::range_for;
use crate::metrics::{is_valid_frame, media_time, PerVideoMetricKey};
use crate::scoring::{composed_key, resolved_bracket_id};
use crate::types::{Area, GazeFrame, NormativeBaselineSet, NormativeDirection};

/// Most points one trace may have (a guard against a tiny step on a long clip).
pub const MAX_TRACE_POINTS: usize = 100_000;

/// What a trace measures; JSON `{"kind": "aoiDwell", "aoi": "Actor_Face"}`, ...
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TraceMetric {
    /// Share of valid frames in the AOI named `aoi` (baseline key `aoi_dwell_ratio::<aoi>`).
    AoiDwell {
        /// The catalog area name.
        aoi: String,
    },
    /// Share of valid frames in any AOI carrying `role` (catalog spelling: `social`,
    /// `geometric`, `eye`, `mouth`, `orienting`). No baseline key.
    RoleDwell {
        /// The role.
        role: String,
    },
    /// Share of valid frames in any AOI. No baseline key.
    AnyAoi,
    /// Share of frames that are valid (baseline key `valid_sample_ratio`).
    ValidRatio,
}

impl TraceMetric {
    /// The baseline expectation this trace is read against, `(biomarker, targetAOI)`.
    pub fn baseline_target(&self) -> Option<(&'static str, Option<&str>)> {
        match self {
            Self::AoiDwell { aoi } => Some((PerVideoMetricKey::AoiDwellRatio.as_str(), Some(aoi))),
            Self::ValidRatio => Some((PerVideoMetricKey::ValidSampleRatio.as_str(), None)),
            Self::RoleDwell { .. } | Self::AnyAoi => None,
        }
    }
}

/// One point of a trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePoint {
    /// Window centre, media time seconds.
    pub t_sec: f64,
    /// The metric in the window; `null` with no (valid) frame to take it from.
    pub value: Option<f64>,
    /// Valid frames / frames in the window (0 for an empty window).
    pub valid_ratio: f64,
    /// Frames in the window.
    pub n_frames: u32,
}

/// A windowed metric trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    /// What was measured.
    pub metric: TraceMetric,
    /// Window length, seconds.
    pub window_sec: f64,
    /// Step between points, seconds.
    pub step_sec: f64,
    /// The points, by time.
    pub points: Vec<TracePoint>,
}

/// The windowed trace of `metric` over one clip's frames (see the module docs).
pub fn windowed_trace(
    frames: &[GazeFrame],
    areas: &[Area],
    params: &HitParams,
    metric: &TraceMetric,
    window_sec: f64,
    step_sec: f64,
) -> Result<Trace, ReplayError> {
    if !(window_sec.is_finite() && window_sec > 0.0) {
        return Err(ReplayError::InvalidArgument(format!(
            "windowSec must be a positive number, got {window_sec}"
        )));
    }
    if !(step_sec.is_finite() && step_sec > 0.0) {
        return Err(ReplayError::InvalidArgument(format!(
            "stepSec must be a positive number, got {step_sec}"
        )));
    }
    let aois = ClipAois::new(areas, params);
    // (time, valid, in-metric) per frame with a finite time, sorted by time (stable).
    let mut samples: Vec<(f64, bool, bool)> = frames
        .iter()
        .filter(|f| media_time(f).is_finite())
        .map(|f| {
            let valid = is_valid_frame(f);
            let hit = valid && {
                let h = aois.hits(f);
                match metric {
                    TraceMetric::AoiDwell { aoi } => aois.hits_name(&h, aoi),
                    TraceMetric::RoleDwell { role } => aois.hits_role(&h, role),
                    TraceMetric::AnyAoi => ClipAois::any(&h),
                    TraceMetric::ValidRatio => true,
                }
            };
            (media_time(f), valid, hit)
        })
        .collect();
    samples.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut points = Vec::new();
    if let (Some(first), Some(last)) = (samples.first(), samples.last()) {
        let span = last.0 - first.0;
        let steps = (span / step_sec).floor();
        if steps >= MAX_TRACE_POINTS as f64 {
            return Err(ReplayError::InvalidArgument(format!(
                "stepSec {step_sec} gives more than {MAX_TRACE_POINTS} points over {span} s"
            )));
        }
        let half = window_sec / 2.0;
        let (mut lo, mut hi) = (0usize, 0usize);
        for k in 0..=(steps as usize) {
            let t = first.0 + k as f64 * step_sec;
            while lo < samples.len() && samples[lo].0 < t - half {
                lo += 1;
            }
            hi = hi.max(lo);
            while hi < samples.len() && samples[hi].0 < t + half {
                hi += 1;
            }
            let window = &samples[lo..hi];
            let n = window.len();
            let n_valid = window.iter().filter(|s| s.1).count();
            let n_hit = window.iter().filter(|s| s.2).count();
            let valid_ratio = if n == 0 {
                0.0
            } else {
                n_valid as f64 / n as f64
            };
            let value = match metric {
                TraceMetric::ValidRatio => (n > 0).then_some(valid_ratio),
                _ => (n_valid > 0).then(|| n_hit as f64 / n_valid as f64),
            };
            points.push(TracePoint {
                t_sec: t,
                value,
                valid_ratio,
                n_frames: n as u32,
            });
        }
    }
    Ok(Trace {
        metric: metric.clone(),
        window_sec,
        step_sec,
        points,
    })
}

/// One labelled horizontal line of an expected band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BandThreshold {
    /// Message key: `replay.band.expected` (the expected value), `replay.band.atypicalBelow`
    /// (a value below is atypical), `replay.band.atypicalAbove` (a value above is atypical).
    pub label_key: String,
    /// The metric value of the line.
    pub value: f64,
}

/// The published expectation for one baseline segment of a clip, for one age bracket, in the
/// metric's own units. The status boundaries are exactly those of `scoring::evaluate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedBand {
    /// Baseline segment id.
    pub segment_id: String,
    /// Segment start, media time seconds.
    pub start_sec: f64,
    /// Segment end, media time seconds (exclusive, as scoring windows it).
    pub end_sec: f64,
    /// `biomarker` or `biomarker::targetAOI`.
    pub metric_key: String,
    /// Direction of "better".
    pub direction: NormativeDirection,
    /// The bracket the range was read for (its own range, else `default`).
    pub bracket_id: String,
    /// The "expected range" to shade: `higherBetter` `[acceptableLow, null)`, `lowerBetter`
    /// `(null, acceptableHigh]`, `band` `[acceptableLow, acceptableHigh]` (on target); bounds
    /// with scoring's fallbacks to `expected`. Both `null` for a degenerate band.
    pub low: Option<f64>,
    /// See `low`.
    pub high: Option<f64>,
    /// The expected value (`band`: `expected`, else the centre of the band).
    pub mean: Option<f64>,
    /// Labelled lines: the expected value and the atypical boundaries.
    pub thresholds: Vec<BandThreshold>,
    /// Baseline citation (data, shown as written).
    pub citation: String,
    /// `literature` or `interpolated`.
    pub source: String,
}

fn line(key: &str, value: f64) -> BandThreshold {
    BandThreshold {
        label_key: format!("replay.band.{key}"),
        value,
    }
}

/// Every expectation of clip `clip_id` in `baselines` for `biomarker` / `target_aoi`, one per
/// segment, read for `bracket_id` with scoring's `default` fallback (`baselines::range_for`).
/// Empty when the clip, the expectation or a range is missing.
pub fn expected_bands(
    baselines: &NormativeBaselineSet,
    clip_id: &str,
    bracket_id: &str,
    biomarker: &str,
    target_aoi: Option<&str>,
) -> Vec<ExpectedBand> {
    let key = composed_key(biomarker, target_aoi);
    let Some(video) = baselines.videos.get(clip_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for segment in &video.segments {
        for e in &segment.expectations {
            if composed_key(&e.biomarker, e.target_aoi.as_deref()) != key {
                continue;
            }
            let Some(r) = range_for(e, bracket_id) else {
                continue;
            };
            let (low, high, mean, thresholds) = match e.direction {
                NormativeDirection::HigherBetter => {
                    let expected = r.expected.or(r.acceptable_low).unwrap_or(0.0);
                    let low = r.acceptable_low.unwrap_or(expected);
                    (
                        Some(low),
                        None,
                        Some(expected),
                        vec![line("expected", expected), line("atypicalBelow", low)],
                    )
                }
                NormativeDirection::LowerBetter => {
                    let expected = r.expected.or(r.acceptable_high).unwrap_or(0.0);
                    let high = r.acceptable_high.unwrap_or(expected);
                    (
                        None,
                        Some(high),
                        Some(expected),
                        vec![line("expected", expected), line("atypicalAbove", high)],
                    )
                }
                NormativeDirection::Band => match (r.acceptable_low, r.acceptable_high) {
                    (Some(low), Some(high)) if high > low => {
                        let half = (high - low) / 2.0;
                        let mean = r.expected.unwrap_or((low + high) / 2.0);
                        (
                            Some(low),
                            Some(high),
                            Some(mean),
                            vec![
                                line("expected", mean),
                                line("atypicalBelow", low - half),
                                line("atypicalAbove", high + half),
                            ],
                        )
                    }
                    _ => {
                        // Scoring: on target at or above `expected`, else monitor; never
                        // atypical.
                        let expected = r.expected.unwrap_or(0.0);
                        (None, None, Some(expected), vec![line("expected", expected)])
                    }
                },
            };
            out.push(ExpectedBand {
                segment_id: segment.id.clone(),
                start_sec: segment.start_sec,
                end_sec: segment.end_sec,
                metric_key: key.clone(),
                direction: e.direction,
                bracket_id: if e.by_age.contains_key(bracket_id) {
                    bracket_id.to_owned()
                } else {
                    crate::baselines::DEFAULT_BRACKET_ID.to_owned()
                },
                low,
                high,
                mean,
                thresholds,
                citation: e.citation.clone(),
                source: e.source.clone(),
            });
        }
    }
    out
}

/// [`expected_bands`] for a child's age: the bracket is `scoring::resolved_bracket_id`, the
/// one scoring uses (nearest bracket for an out-of-range age).
pub fn expected_bands_for_age(
    baselines: &NormativeBaselineSet,
    clip_id: &str,
    age_months: i64,
    biomarker: &str,
    target_aoi: Option<&str>,
) -> Vec<ExpectedBand> {
    let bracket = resolved_bracket_id(baselines, age_months);
    expected_bands(baselines, clip_id, &bracket, biomarker, target_aoi)
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{area, frame, invalid, t};
    use super::*;
    use crate::metrics::extract_clip;
    use crate::scoring::evaluate;
    use crate::types::{ExpectationStatus, NormativeRange};

    fn areas() -> Vec<Area> {
        vec![
            area("Face", (0.0, 10.0), [0.0, 0.0, 0.5, 0.5], &["social"]),
            area("Ball", (0.0, 10.0), [0.5, 0.5, 1.0, 1.0], &["geometric"]),
        ]
    }

    /// 2 s at 60 Hz: first second on the face, second on the ball, frames 90..105 invalid.
    fn frames() -> Vec<GazeFrame> {
        (0..120)
            .map(|i| {
                if (90..105).contains(&i) {
                    invalid(t(i))
                } else if i < 60 {
                    frame(t(i), 0.25, 0.25)
                } else {
                    frame(t(i), 0.75, 0.75)
                }
            })
            .collect()
    }

    fn values(trace: &Trace) -> Vec<Option<f64>> {
        trace.points.iter().map(|p| p.value).collect()
    }

    #[test]
    fn aoi_trace_values_are_known() {
        let tr = windowed_trace(
            &frames(),
            &areas(),
            &HitParams::default(),
            &TraceMetric::AoiDwell { aoi: "Face".into() },
            1.0,
            0.5,
        )
        .unwrap();
        // Points at 0, 0.5, 1.0, 1.5 (last frame 119/60 = 1.983).
        let ts: Vec<f64> = tr.points.iter().map(|p| p.t_sec).collect();
        assert_eq!(ts, vec![0.0, 0.5, 1.0, 1.5]);
        // t=0: [-0.5, 0.5): frames 0..30, all face -> 1.
        // t=0.5: [0, 1): frames 0..60, all face -> 1.
        // t=1.0: [0.5, 1.5): frames 30..90, half face -> 0.5.
        // t=1.5: [1.0, 2.0): frames 60..120, 45 valid, none on the face -> 0.
        assert_eq!(
            values(&tr),
            vec![Some(1.0), Some(1.0), Some(0.5), Some(0.0)]
        );
        assert_eq!(tr.points[3].valid_ratio, 45.0 / 60.0);
        assert_eq!(tr.points[3].n_frames, 60);
    }

    #[test]
    fn role_any_and_valid_traces() {
        let f = frames();
        let p = HitParams::default();
        let role = windowed_trace(
            &f,
            &areas(),
            &p,
            &TraceMetric::RoleDwell {
                role: "geometric".into(),
            },
            1.0,
            0.5,
        )
        .unwrap();
        assert_eq!(
            values(&role),
            vec![Some(0.0), Some(0.0), Some(0.5), Some(1.0)]
        );
        let any = windowed_trace(&f, &areas(), &p, &TraceMetric::AnyAoi, 1.0, 0.5).unwrap();
        assert_eq!(values(&any), vec![Some(1.0); 4]);
        let valid = windowed_trace(&f, &areas(), &p, &TraceMetric::ValidRatio, 1.0, 0.5).unwrap();
        assert_eq!(
            values(&valid),
            vec![Some(1.0), Some(1.0), Some(1.0), Some(0.75)]
        );
    }

    #[test]
    fn a_whole_clip_window_reproduces_the_clip_metric() {
        // Frames whose recorded names agree with the geometry.
        let mut f = frames();
        for fr in &mut f {
            if fr.gaze.is_valid && fr.gaze.x_norm < 0.5 {
                fr.active_aoi_names = vec!["Face".into()];
            } else if fr.gaze.is_valid {
                fr.active_aoi_names = vec!["Ball".into()];
            }
        }
        let set = extract_clip("clip", &f, 0.0);
        let tr = windowed_trace(
            &f,
            &areas(),
            &HitParams::default(),
            &TraceMetric::AoiDwell { aoi: "Face".into() },
            10.0,
            100.0,
        )
        .unwrap();
        assert_eq!(tr.points.len(), 1);
        assert_eq!(
            tr.points[0].value,
            Some(set.metrics["aoi_dwell_ratio::Face"])
        );
        let tr = windowed_trace(
            &f,
            &[],
            &HitParams::default(),
            &TraceMetric::ValidRatio,
            10.0,
            100.0,
        )
        .unwrap();
        assert_eq!(tr.points[0].value, Some(set.valid_sample_ratio));
    }

    #[test]
    fn bad_window_or_step_is_an_error() {
        let p = HitParams::default();
        for (w, s) in [(0.0, 1.0), (1.0, 0.0), (f64::NAN, 1.0), (1.0, -1.0)] {
            assert!(windowed_trace(&frames(), &[], &p, &TraceMetric::AnyAoi, w, s).is_err());
        }
        assert!(windowed_trace(&frames(), &[], &p, &TraceMetric::AnyAoi, 1.0, 1e-9).is_err());
        let empty = windowed_trace(&[], &[], &p, &TraceMetric::AnyAoi, 1.0, 1.0).unwrap();
        assert!(empty.points.is_empty());
    }

    fn baselines() -> NormativeBaselineSet {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/normative_baselines.json"
        ))
        .unwrap()
    }

    #[test]
    fn expected_band_matches_scoring_boundaries() {
        let b = baselines();
        // stim_02 cue_face: aoi_dwell_ratio::Actor_Face, higherBetter.
        let bands = expected_bands_for_age(
            &b,
            "stim_02_gaze_cue_right_8s",
            30,
            "aoi_dwell_ratio",
            Some("Actor_Face"),
        );
        assert_eq!(bands.len(), 1);
        let band = &bands[0];
        assert_eq!(band.segment_id, "cue_face");
        assert_eq!((band.start_sec, band.end_sec), (0.0, 3.5));
        let e = &b.videos["stim_02_gaze_cue_right_8s"].segments[0].expectations[0];
        let r: &NormativeRange = range_for(e, "24-35").unwrap();
        assert_eq!(band.mean, r.expected);
        assert_eq!(band.low, r.acceptable_low);
        // The lines are where scoring's status changes.
        let expected = band.thresholds[0].value;
        let atypical = band.thresholds[1].value;
        assert_eq!(band.thresholds[0].label_key, "replay.band.expected");
        assert_eq!(band.thresholds[1].label_key, "replay.band.atypicalBelow");
        let status = |v: f64| evaluate(v, e.direction, r).0;
        assert_eq!(status(expected), ExpectationStatus::OnTarget);
        assert_eq!(status(expected - 1e-9), ExpectationStatus::Monitor);
        assert_eq!(status(atypical), ExpectationStatus::Monitor);
        assert_eq!(status(atypical - 1e-9), ExpectationStatus::Atypical);

        // A band expectation (stim_01 eye_dwell_ratio 0.25..0.45, expected 0.35).
        let bands = expected_bands(
            &b,
            "stim_01_social_peekaboo_8s",
            "24-35",
            "eye_dwell_ratio",
            None,
        );
        let band = &bands[0];
        assert_eq!(band.bracket_id, "default");
        assert_eq!(
            (band.low, band.high, band.mean),
            (Some(0.25), Some(0.45), Some(0.35))
        );
        let e = &b.videos["stim_01_social_peekaboo_8s"].segments[0].expectations[0];
        let r = range_for(e, "24-35").unwrap();
        let below = band.thresholds[1].value;
        let above = band.thresholds[2].value;
        assert!((below - 0.15).abs() < 1e-12 && (above - 0.55).abs() < 1e-12);
        let status = |v: f64| evaluate(v, e.direction, r).0;
        assert_eq!(status(below + 1e-9), ExpectationStatus::Monitor);
        assert_eq!(status(below - 1e-9), ExpectationStatus::Atypical);
        assert_eq!(status(above + 1e-9), ExpectationStatus::Atypical);

        // lowerBetter with an own bracket range (stim_01 first_entry_latency 12-17).
        let bands = expected_bands_for_age(
            &b,
            "stim_01_social_peekaboo_8s",
            14,
            "first_entry_latency_ms",
            Some("Eye_Region"),
        );
        assert_eq!(bands[0].bracket_id, "12-17");
        assert_eq!((bands[0].low, bands[0].high), (None, Some(750.0)));
        assert_eq!(bands[0].mean, Some(500.0));

        // Two segments, one band each; unknown clip or metric: none.
        assert_eq!(
            expected_bands(
                &b,
                "stim_02_gaze_cue_right_8s",
                "24-35",
                "aoi_dwell_ratio",
                Some("Gaze_Target_Right")
            )
            .len(),
            1
        );
        assert!(expected_bands(&b, "nope", "24-35", "aoi_dwell_ratio", None).is_empty());
        assert!(expected_bands(
            &b,
            "stim_02_gaze_cue_right_8s",
            "24-35",
            "aoi_dwell_ratio",
            None
        )
        .is_empty());
        assert_eq!(
            TraceMetric::AoiDwell { aoi: "X".into() }.baseline_target(),
            Some(("aoi_dwell_ratio", Some("X")))
        );
    }

    #[test]
    fn trace_metric_json() {
        let m: TraceMetric =
            serde_json::from_str(r#"{"kind":"roleDwell","role":"social"}"#).unwrap();
        assert_eq!(
            m,
            TraceMetric::RoleDwell {
                role: "social".into()
            }
        );
        let v: TraceMetric = serde_json::from_str(r#"{"kind":"validRatio"}"#).unwrap();
        assert_eq!(v, TraceMetric::ValidRatio);
    }
}
