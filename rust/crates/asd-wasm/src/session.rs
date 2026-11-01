//! owner: WP-R.6
//!
//! Reading a stored session for a re-score: decode it with the asd-contract type of its
//! schema version and hand back what the scorer needs. No scoring happens here.
//!
//! Accepted documents:
//! - a `POST /v1/sessions` body (`schemaVersion` `1.6`, `1.7` or `1.8`, has `rawPayload`);
//! - a bare `rawPayload` record as the backend stores it in `sessions.summary`
//!   (`schemaVersion` `v1.6`, `v1.7` or `v1.8`).
//!
//! Without frames, the metric sets are rebuilt from the device's own score rows
//! (`rawPayload.perVideo[].rows[]`): every non-imputed row's `observed` value under its
//! `metricKey`. That is the data the device scored, so a re-score against the same baseline
//! reproduces the device rows; see [`metric_sets_from_reports`] for what it cannot recover.

use std::collections::BTreeMap;

use asd_contract::api_v1::ClipScore;
use asd_contract::{session_v1_6, session_v1_7, session_v1_8};
use asd_core::metrics::PerVideoMetricKey;
use asd_core::scoring;
use asd_core::types::{AttentionFlag, PerVideoMetricSet, PerVideoScoreReport, PerVideoScoreRow};
use serde::Deserialize;
use serde_json::Value;

use crate::error::BoundaryError;

/// The parts of a session a re-score reads.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionView {
    pub session_id: String,
    /// Wire form (`1.6`, `1.7`, `1.8`) for both accepted documents.
    pub schema_version: String,
    /// `rawPayload.perVideo` (the device's per-clip score reports), empty when absent.
    pub reports: Vec<PerVideoScoreReport>,
    /// `true` when `rawPayload.perVideo` was present.
    pub has_reports: bool,
    /// `rawPayload.perVideoComposite` as the device sent it.
    pub device_composite: Option<f64>,
    /// The request's `attentionFlag` (`None` for `null`, absent, or a bare record).
    pub device_flag: Option<AttentionFlag>,
}

fn invalid(e: serde_json::Error) -> BoundaryError {
    BoundaryError::invalid_json("session", e)
}

/// Decodes a session document of any supported version (see the module docs).
pub fn decode(json: &str) -> Result<SessionView, BoundaryError> {
    let value: Value = serde_json::from_str(json).map_err(invalid)?;
    let Some(obj) = value.as_object() else {
        return Err(BoundaryError::InvalidJson(
            "session: expected a JSON object".into(),
        ));
    };
    let version = obj
        .get("schemaVersion")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if obj.contains_key("rawPayload") {
        decode_request(value, &version)
    } else {
        decode_record(value, &version)
    }
}

fn unsupported(version: &str, expected: &str) -> BoundaryError {
    BoundaryError::UnsupportedSchemaVersion(format!(
        "session schemaVersion {version:?} is not supported (expected {expected})"
    ))
}

fn decode_request(value: Value, version: &str) -> Result<SessionView, BoundaryError> {
    if version == session_v1_6::SCHEMA_VERSION {
        let req: session_v1_6::SessionIngestRequest =
            serde_json::from_value(value).map_err(invalid)?;
        let r = req.raw_payload;
        Ok(SessionView {
            session_id: req.session_id,
            schema_version: req.schema_version,
            has_reports: r.per_video.is_some(),
            reports: r.per_video.unwrap_or_default(),
            device_composite: r.per_video_composite,
            device_flag: req.attention_flag.flatten(),
        })
    } else if version == session_v1_7::SCHEMA_VERSION {
        let req: session_v1_7::SessionIngestRequest =
            serde_json::from_value(value).map_err(invalid)?;
        let r = req.raw_payload;
        Ok(SessionView {
            session_id: req.session_id,
            schema_version: req.schema_version,
            has_reports: r.per_video.is_some(),
            reports: r
                .per_video
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.report)
                .collect(),
            device_composite: r.per_video_composite,
            device_flag: req.attention_flag.flatten(),
        })
    } else if version == session_v1_8::SCHEMA_VERSION {
        // 1.8 keeps the 1.7 per-clip report; an unscorable session simply has none.
        let req: session_v1_8::SessionIngestRequest =
            serde_json::from_value(value).map_err(invalid)?;
        let r = req.raw_payload;
        Ok(SessionView {
            session_id: req.session_id,
            schema_version: req.schema_version,
            has_reports: r.per_video.is_some(),
            reports: r
                .per_video
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.report)
                .collect(),
            device_composite: r.per_video_composite,
            device_flag: req.attention_flag.flatten(),
        })
    } else {
        Err(unsupported(version, "\"1.6\", \"1.7\" or \"1.8\""))
    }
}

fn decode_record(value: Value, version: &str) -> Result<SessionView, BoundaryError> {
    let wire = version.strip_prefix('v').unwrap_or_default();
    if wire == session_v1_6::SCHEMA_VERSION {
        let r: session_v1_6::GazeSessionRecord = serde_json::from_value(value).map_err(invalid)?;
        Ok(SessionView {
            session_id: r.session_id,
            schema_version: wire.to_owned(),
            has_reports: r.per_video.is_some(),
            reports: r.per_video.unwrap_or_default(),
            device_composite: r.per_video_composite,
            device_flag: None,
        })
    } else if wire == session_v1_7::SCHEMA_VERSION {
        let r: session_v1_7::GazeSessionRecord = serde_json::from_value(value).map_err(invalid)?;
        Ok(SessionView {
            session_id: r.session_id,
            schema_version: wire.to_owned(),
            has_reports: r.per_video.is_some(),
            reports: r
                .per_video
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.report)
                .collect(),
            device_composite: r.per_video_composite,
            device_flag: None,
        })
    } else if wire == session_v1_8::SCHEMA_VERSION {
        let r: session_v1_8::GazeSessionRecord = serde_json::from_value(value).map_err(invalid)?;
        Ok(SessionView {
            session_id: r.session_id,
            schema_version: wire.to_owned(),
            has_reports: r.per_video.is_some(),
            reports: r
                .per_video
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.report)
                .collect(),
            device_composite: r.per_video_composite,
            device_flag: None,
        })
    } else {
        Err(unsupported(
            version,
            "a request with rawPayload (\"1.6\"-\"1.8\") or a record (\"v1.6\"-\"v1.8\")",
        ))
    }
}

/// The members of an API `SessionDetail` a re-score reads; everything else is ignored, so
/// additive changes to the API type do not break the console's sandbox.
#[derive(Debug, Deserialize)]
struct ApiSession {
    id: String,
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    composite_score: Option<f64>,
    #[serde(default)]
    attention_flag: Option<String>,
    #[serde(default)]
    valid_frame_ratio: Option<f64>,
}

/// Decodes a session as the console reads it from `/v1` (`SessionDetail` plus the `items`
/// of `GET /v1/sessions/{id}/clips`) into the same view as a stored record. Each clip's
/// metric rows become the report rows (`metricKey` = `scoring::composed_key(metric,
/// target_aoi)`, `observed` = `recorded`), clips in `order`.
///
/// What the API form loses, each returned as a warning:
/// - the per-clip valid-sample ratio: every clip gets `valid_sample_ratio` if given, else
///   the session's `valid_frame_ratio`, else 1;
/// - the backend-only `unreliable` flag: the device flag is then `None` and the re-score
///   does not reproduce it.
pub fn decode_api(
    session_json: &str,
    clips_json: &str,
    valid_sample_ratio: Option<f64>,
) -> Result<(SessionView, Vec<String>), BoundaryError> {
    let session: ApiSession = serde_json::from_str(session_json).map_err(invalid)?;
    let mut clips: Vec<ClipScore> =
        serde_json::from_str(clips_json).map_err(|e| BoundaryError::invalid_json("clips", e))?;
    clips.sort_by_key(|c| c.order);
    let mut warnings = Vec::new();

    let ratio = match (valid_sample_ratio, session.valid_frame_ratio) {
        (Some(r), _) => {
            warnings.push(format!(
                "the API has no per-clip valid-sample ratio; every clip uses the given {r}"
            ));
            r
        }
        (None, Some(r)) => {
            warnings.push(format!(
                "the API has no per-clip valid-sample ratio; every clip uses the session's \
                 valid frame ratio {r}"
            ));
            r
        }
        (None, None) => {
            warnings.push(
                "the session has no valid frame ratio; every clip is treated as fully valid"
                    .to_owned(),
            );
            1.0
        }
    };

    let device_flag = match session.attention_flag.as_deref() {
        None => None,
        Some("unreliable") => {
            warnings.push(
                "the backend marked this session unreliable (a backend-only rule); the \
                 re-score does not reproduce that flag"
                    .to_owned(),
            );
            None
        }
        Some(other) => Some(
            serde_json::from_value::<AttentionFlag>(Value::String(other.to_owned()))
                .map_err(invalid)?,
        ),
    };

    let reports = clips
        .into_iter()
        .map(|clip| {
            let rows: Vec<PerVideoScoreRow> = clip
                .rows
                .into_iter()
                .map(|r| PerVideoScoreRow {
                    metric_key: scoring::composed_key(&r.metric, r.target_aoi.as_deref()),
                    segment_id: r.segment_id,
                    segment_description: String::new(),
                    biomarker: r.metric,
                    target_aoi: r.target_aoi,
                    direction: r.direction,
                    observed: r.recorded,
                    expected: r.expected,
                    acceptable_low: r.acceptable_low,
                    acceptable_high: r.acceptable_high,
                    status: r.status,
                    score: r.score,
                    imputed: r.imputed,
                    citation: String::new(),
                    source: r.source,
                })
                .collect();
            PerVideoScoreReport {
                has_baseline: !rows.is_empty(),
                title: clip.clip_id.clone(),
                stimulus_id: clip.clip_id,
                category: String::new(),
                rows,
                subscore: clip.score.unwrap_or(0.0),
                valid_sample_ratio: ratio,
                low_confidence: false,
                weight: clip.weight.unwrap_or(0.0),
            }
        })
        .collect();

    Ok((
        SessionView {
            session_id: session.id,
            schema_version: session.schema_version.unwrap_or_default(),
            reports,
            has_reports: true,
            device_composite: session.composite_score,
            device_flag,
        },
        warnings,
    ))
}

/// Metric sets rebuilt from the device's score reports, plus warnings for what a report
/// cannot carry. One set per report, in report order.
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
pub fn metric_sets_from_reports(
    reports: &[PerVideoScoreReport],
) -> (Vec<PerVideoMetricSet>, Vec<String>) {
    let latency = PerVideoMetricKey::FirstEntryLatencyMs.as_str();
    let mut warnings = Vec::new();
    let sets = reports
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
mod tests {
    use asd_core::types::{ExpectationStatus, NormativeDirection, PerVideoScoreRow};

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
        let (sets, warnings) = metric_sets_from_reports(&reports);
        assert!(warnings.is_empty(), "{warnings:?}");
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
        let (sets, warnings) = metric_sets_from_reports(&reports);
        assert_eq!(sets[0].metrics["social_dwell_ratio"], 0.7);
        assert_eq!(sets[0].observed_duration_sec, 0.0);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("clip: social_dwell_ratio differs"));
        assert!(warnings[1].starts_with("other: had no baseline"));
    }

    #[test]
    fn api_session_becomes_reports_with_warnings() {
        let session = r#"{"id":"S1","schema_version":"1.6","composite_score":0.6,
            "attention_flag":"unreliable","valid_frame_ratio":0.7,"extra":1}"#;
        let clips = r#"[
          {"clip_id":"b","order":1,"rows":[]},
          {"clip_id":"a","order":0,"weight":0.5,"rows":[
            {"segment_id":"s","metric":"eye_dwell_ratio","target_aoi":"eyes","direction":"band",
             "expected":0.3,"acceptable_low":0.2,"acceptable_high":0.4,"recorded":0.25,
             "deviation":-0.05,"status":"onTarget","score":1.0,"imputed":false,"source":"lit"}]}
        ]"#;
        let (view, warnings) = decode_api(session, clips, None).unwrap();
        assert_eq!(view.session_id, "S1");
        assert_eq!(view.device_flag, None);
        assert_eq!(view.device_composite, Some(0.6));
        assert_eq!(view.reports.len(), 2);
        assert_eq!(view.reports[0].stimulus_id, "a");
        assert_eq!(view.reports[0].rows[0].metric_key, "eye_dwell_ratio::eyes");
        assert_eq!(view.reports[0].valid_sample_ratio, 0.7);
        assert!(!view.reports[1].has_baseline);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[1].contains("unreliable"));

        let green = r#"{"id":"S2","attention_flag":"green"}"#;
        let (view, warnings) = decode_api(green, "[]", Some(0.9)).unwrap();
        assert_eq!(view.device_flag, Some(AttentionFlag::Green));
        assert!(warnings[0].contains("0.9"));
        assert_eq!(
            decode_api("{}", "[]", None).unwrap_err().kind(),
            "invalidJson"
        );
        assert_eq!(
            decode_api(green, "{}", None).unwrap_err().kind(),
            "invalidJson"
        );
    }

    #[test]
    fn versions_are_read_from_the_document() {
        let err = decode(r#"{"schemaVersion":"1.9","rawPayload":{}}"#).unwrap_err();
        assert_eq!(err.kind(), "unsupportedSchemaVersion");
        let err = decode(r#"{"schemaVersion":"v1.9"}"#).unwrap_err();
        assert_eq!(err.kind(), "unsupportedSchemaVersion");
        assert_eq!(decode("1").unwrap_err().kind(), "invalidJson");
        assert_eq!(decode("{").unwrap_err().kind(), "invalidJson");
    }
}
