//! owner: WP-B.3
//!
//! Derived session columns (ING-2, CON-15), tracking-quality columns (SCH-4, QS-5) and
//! per-clip rows (CON-19), and the SCH-5 scoring columns. Pure functions over the validated
//! request (the 1.8 type; a 1.6 request is carried without a tracking record, 1.6 and 1.7
//! requests as `scored`); nothing is defaulted: unknown stays `None`.

use asd_contract::api_v1::{MetricRow, SessionAttentionFlag};
use asd_contract::config::ScoringProfile;
use asd_contract::session_v1_6;
use asd_contract::session_v1_8::{self, SessionIngestRequest, SessionPerVideoRow};
use asd_core::flag::{assess, FlagThresholds};
use asd_core::types::{
    AttentionFlag, ExpectationStatus, PerVideoScoreReport, PerVideoScoreRow, PerVideoSessionScore,
};
use serde_json::{json, Value};

/// Below this session `validRatio` a missing flag is stored `unreliable` (CON-15): the
/// documented fallback of `lowConfidenceValidRatio` when no profile is published (INV-1).
pub const MIN_VALID_RATIO_FOR_FLAG: f64 = 0.60;

/// The profile's flag thresholds as `asd-core` takes them.
pub fn flag_thresholds(profile: &ScoringProfile) -> FlagThresholds {
    FlagThresholds {
        green_floor: profile.flag.green_floor,
        yellow_floor: profile.flag.yellow_floor,
        low_confidence_share: profile.flag.low_confidence_share,
        atypical_share_for_red: profile.flag.atypical_share_for_red,
    }
}

/// `rawPayload.ageMonths` as a whole number of months in `0..=240` (COH-3,
/// `sessions.age_months`); anything else (absent, fractional, negative, absurd) is `None`, as
/// in the 0026 back-fill.
pub fn age_months(req: &SessionIngestRequest) -> Option<i32> {
    let v = req.raw_payload.extra.get("ageMonths")?;
    let n = v
        .as_i64()
        .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))?;
    (0..=240).contains(&n).then_some(n as i32)
}

fn from_device(flag: AttentionFlag) -> SessionAttentionFlag {
    match flag {
        AttentionFlag::Green => SessionAttentionFlag::Green,
        AttentionFlag::Yellow => SessionAttentionFlag::Yellow,
        AttentionFlag::Red => SessionAttentionFlag::Red,
    }
}

/// Stored `attention_flag` (CON-15). A device flag is kept as sent. Without one (`null` or
/// absent) the server runs `asd-core::flag::assess` over `rawPayload.perVideo` when
/// `validRatio >= profile.lowConfidenceValidRatio` and at least one clip was scored against a
/// baseline; otherwise the session is `unreliable`. Thresholds are the `flag` section of
/// `profile`: the published scoring profile, or the defaults when none is published (INV-1).
/// An `unscorable` 1.8 session (which carries no device flag, SCH-5) is always `unreliable`.
pub fn attention_flag(
    req: &SessionIngestRequest,
    profile: &ScoringProfile,
) -> SessionAttentionFlag {
    if req.is_unscorable() {
        return SessionAttentionFlag::Unreliable;
    }
    if let Some(Some(flag)) = req.attention_flag {
        return from_device(flag);
    }
    let Some(videos) = req.raw_payload.per_video.as_deref() else {
        return SessionAttentionFlag::Unreliable;
    };
    let videos: Vec<PerVideoScoreReport> = videos.iter().map(|v| v.report.clone()).collect();
    if req.valid_ratio < profile.low_confidence_valid_ratio
        || !videos.iter().any(|v| v.has_baseline)
    {
        return SessionAttentionFlag::Unreliable;
    }
    let composite = req
        .raw_payload
        .per_video_composite
        .unwrap_or_else(|| weighted_composite(&videos));
    let score = PerVideoSessionScore {
        age_bracket_id: String::new(),
        age_months: 0,
        composite_score: composite,
        videos,
        baseline_status: String::new(),
        baseline_schema_version: String::new(),
        scored_at_utc: String::new(),
    };
    from_device(assess(&score, &flag_thresholds(profile)).flag)
}

/// `Σ weight·subscore / Σ weight` over clips with a baseline, `0` when none (CORE-3), for
/// a payload that carries `perVideo` without `perVideoComposite`.
fn weighted_composite(videos: &[PerVideoScoreReport]) -> f64 {
    let (sum, weights) = videos
        .iter()
        .filter(|v| v.has_baseline)
        .fold((0.0, 0.0), |(s, w), v| {
            (s + v.weight * v.subscore, w + v.weight)
        });
    if weights > 0.0 {
        sum / weights
    } else {
        0.0
    }
}

/// `composite_score = rawPayload.perVideoComposite` (ING-2).
pub fn composite_score(req: &SessionIngestRequest) -> Option<f64> {
    req.raw_payload.per_video_composite
}

/// `1 - share of lowConfidence clips` over every reported clip; `None` without clips.
/// Reads `rawPayload.perVideo`, else the flattened `perVideo[].details`.
pub fn composite_confidence(req: &SessionIngestRequest) -> Option<f64> {
    let flags: Vec<bool> = match (&req.raw_payload.per_video, &req.per_video) {
        (Some(videos), _) if !videos.is_empty() => {
            videos.iter().map(|v| v.report.low_confidence).collect()
        }
        (_, Some(rows)) if !rows.is_empty() => {
            rows.iter().map(|r| r.details.base.low_confidence).collect()
        }
        _ => return None,
    };
    let low = flags.iter().filter(|l| **l).count();
    Some(1.0 - low as f64 / flags.len() as f64)
}

/// `rawPayload.bundleVersion` when it is a string (ING-2).
pub fn bundle_version(req: &SessionIngestRequest) -> Option<String> {
    req.raw_payload
        .extra
        .get("bundleVersion")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The device's affect means as stored in `sessions.affect` (keys as in `rawPayload`);
/// `None` for an `unscorable` session, which has none (SCH-5).
pub fn affect(req: &SessionIngestRequest) -> Option<Value> {
    let r = &req.raw_payload;
    match (
        r.social_smile_intensity,
        r.smile_reactivity_index,
        r.flat_affect_score,
        r.surprise_score_mean,
    ) {
        (Some(smile), Some(reactivity), Some(flat), Some(surprise)) if !req.is_unscorable() => {
            Some(json!({
                "socialSmileIntensity": smile,
                "smileReactivityIndex": reactivity,
                "flatAffectScore": flat,
                "surpriseScoreMean": surprise,
            }))
        }
        _ => None,
    }
}

/// Stored `social_preference`: the device score, `None` for an `unscorable` session.
pub fn social_preference(req: &SessionIngestRequest) -> Option<f64> {
    if req.is_unscorable() {
        None
    } else {
        req.social_preference_score
    }
}

/// Stored `scoring_status` / `unscorable_reason` (SCH-5): what a 1.8 session sent; both
/// `None` for a 1.6 or 1.7 session, which cannot say.
pub fn scoring_columns(req: &SessionIngestRequest) -> (Option<&'static str>, Option<&'static str>) {
    if req.schema_version != session_v1_8::SCHEMA_VERSION {
        return (None, None);
    }
    (
        Some(req.scoring_status.as_str()),
        req.unscorable_reason.map(|r| r.as_str()),
    )
}

/// `rawPayload.normCollection.consented == true` (ING-3 `norm_candidate`).
pub fn norm_consented(req: &SessionIngestRequest) -> bool {
    req.raw_payload
        .extra
        .get("normCollection")
        .and_then(|n| n.get("consented"))
        .and_then(Value::as_bool)
        == Some(true)
}

/// Stored `calibrated_distance_cm`. 1.6: the value the device sent (unchanged). 1.7 and
/// 1.8 (QS-5): the calibration pose distance when a calibration ran, else `None`, whatever
/// the top-level value says; a 1.6-era app wrote a fixed 35 cm when nothing was measured.
pub fn calibrated_distance_cm(req: &SessionIngestRequest) -> Option<f64> {
    if req.schema_version == session_v1_6::SCHEMA_VERSION {
        req.calibrated_distance_cm
    } else {
        session_v1_8::calibrated_distance_cm(req.raw_payload.tracking_quality.as_ref())
    }
}

/// The SCH-4 columns, all `None` without a tracking-quality record (every 1.6 session).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackingColumns {
    pub verdict: Option<String>,
    pub accuracy_deg: Option<f64>,
    pub calibration_quality: Option<String>,
    pub calibration_model: Option<String>,
    pub pipeline_version: Option<String>,
    pub device_model: Option<String>,
}

/// Record spelling of a unit enum (`good`, `screenAffine`).
fn wire<T: serde::Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => Some(s),
        _ => None,
    }
}

/// SCH-4 columns from `rawPayload.trackingQuality`.
pub fn tracking_columns(req: &SessionIngestRequest) -> TrackingColumns {
    let Some(tq) = req.raw_payload.tracking_quality.as_ref() else {
        return TrackingColumns::default();
    };
    TrackingColumns {
        verdict: wire(&tq.verdict),
        accuracy_deg: tq.estimated_accuracy_deg,
        calibration_quality: wire(&tq.calibration.quality),
        calibration_model: tq.calibration.model.as_ref().and_then(wire),
        pipeline_version: Some(tq.pipeline_version.clone()),
        device_model: Some(tq.device.model_identifier.clone()),
    }
}

fn severity(status: ExpectationStatus) -> u8 {
    match status {
        ExpectationStatus::OnTarget => 0,
        ExpectationStatus::Monitor => 1,
        ExpectationStatus::Atypical => 2,
    }
}

/// `recorded - expected`, `None` without an expected value.
pub fn deviation(row: &PerVideoScoreRow) -> Option<f64> {
    row.expected.map(|e| row.observed - e)
}

/// Index of the worst-status row; ties go to the largest `|deviation|` (a row without an
/// expected value loses a tie), then to the first row (CON-19).
pub fn worst_row(rows: &[PerVideoScoreRow]) -> Option<usize> {
    let key = |r: &PerVideoScoreRow| (severity(r.status), deviation(r).map_or(-1.0, f64::abs));
    let mut best: Option<(usize, (u8, f64))> = None;
    for (i, row) in rows.iter().enumerate() {
        let k = key(row);
        let better = match best {
            None => true,
            Some((_, b)) => k.0 > b.0 || (k.0 == b.0 && k.1 > b.1),
        };
        if better {
            best = Some((i, k));
        }
    }
    best.map(|(i, _)| i)
}

/// One console metric row (CON-19): `metric` is the biomarker key, `recorded` the observed
/// value, `deviation = recorded - expected`.
pub fn metric_row(row: &PerVideoScoreRow) -> MetricRow {
    MetricRow {
        segment_id: row.segment_id.clone(),
        metric: row.biomarker.clone(),
        target_aoi: row.target_aoi.clone(),
        direction: row.direction,
        expected: row.expected,
        acceptable_low: row.acceptable_low,
        acceptable_high: row.acceptable_high,
        recorded: row.observed,
        deviation: deviation(row),
        status: row.status,
        score: row.score,
        imputed: row.imputed,
        source: row.source.clone(),
    }
}

/// A `session_clip_scores` row.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipRow {
    pub clip_id: String,
    pub order: i32,
    pub dwell_social: Option<f64>,
    pub dwell_geometric: Option<f64>,
    pub dwell_eye: Option<f64>,
    pub dwell_mouth: Option<f64>,
    pub orienting_latency_ms: Option<f64>,
    pub fixation_count: Option<i32>,
    pub mean_fixation_ms: Option<f64>,
    pub status: Option<&'static str>,
    pub score: f64,
    pub weight: f64,
    /// Citation of the worst-status row: the reference its expected value comes from.
    pub baseline_ref: Option<String>,
}

pub fn status_str(status: ExpectationStatus) -> &'static str {
    match status {
        ExpectationStatus::OnTarget => "onTarget",
        ExpectationStatus::Monitor => "monitor",
        ExpectationStatus::Atypical => "atypical",
    }
}

/// Rows for `session_clip_scores` from the flattened `perVideo` (validated: unique clip
/// ids, `order`/`fixation_count` fit `i32`). `details` is stored as received, by the caller.
pub fn clip_rows(rows: &[SessionPerVideoRow]) -> Vec<ClipRow> {
    rows.iter()
        .map(|r| {
            let rows = &r.details.base.rows;
            let worst = worst_row(rows).map(|i| &rows[i]);
            ClipRow {
                clip_id: r.clip_id.clone(),
                order: i32::try_from(r.order).unwrap_or(i32::MAX),
                dwell_social: r.dwell_social,
                dwell_geometric: r.dwell_geometric,
                dwell_eye: r.dwell_eye,
                dwell_mouth: r.dwell_mouth,
                orienting_latency_ms: r.orienting_latency_ms,
                fixation_count: r.fixation_count.and_then(|n| i32::try_from(n).ok()),
                mean_fixation_ms: r.mean_fixation_ms,
                status: r.status.map(status_str),
                score: r.score,
                weight: r.weight,
                baseline_ref: worst.map(|w| w.citation.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_core::types::NormativeDirection;

    fn row(status: ExpectationStatus, observed: f64, expected: Option<f64>) -> PerVideoScoreRow {
        PerVideoScoreRow {
            segment_id: "s".into(),
            segment_description: String::new(),
            biomarker: "b".into(),
            target_aoi: None,
            metric_key: "b".into(),
            direction: NormativeDirection::HigherBetter,
            observed,
            expected,
            acceptable_low: None,
            acceptable_high: None,
            status,
            score: 0.0,
            imputed: false,
            citation: String::new(),
            source: String::new(),
        }
    }

    #[test]
    fn worst_row_prefers_status_then_absolute_deviation_then_first() {
        use ExpectationStatus::*;
        assert_eq!(worst_row(&[]), None);
        let rows = [row(OnTarget, 9.0, Some(0.0)), row(Monitor, 1.0, Some(0.9))];
        assert_eq!(worst_row(&rows), Some(1));
        let rows = [
            row(Atypical, 1.0, Some(0.0)),
            row(Atypical, -5.0, Some(0.0)),
            row(Atypical, 2.0, None),
        ];
        assert_eq!(worst_row(&rows), Some(1));
        let rows = [row(Monitor, 1.0, Some(0.0)), row(Monitor, -1.0, Some(0.0))];
        assert_eq!(worst_row(&rows), Some(0));
        let rows = [row(Monitor, 1.0, None), row(Monitor, 1.0, Some(1.0))];
        assert_eq!(worst_row(&rows), Some(1));
    }

    /// The 1.6 golden as the backend carries it (1.6 decoded as 1.7, then 1.8, `scored`).
    fn golden_1_6() -> SessionIngestRequest {
        session_v1_8::upgrade_v1_7(
            serde_json::from_str(include_str!(
                "../../../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"
            ))
            .unwrap(),
        )
    }

    #[test]
    fn weighted_composite_skips_unscored_clips() {
        let golden = golden_1_6();
        let videos: Vec<PerVideoScoreReport> = golden
            .raw_payload
            .per_video
            .clone()
            .unwrap()
            .into_iter()
            .map(|v| v.report)
            .collect();
        assert!((weighted_composite(&videos) - 0.74).abs() < 1e-12);
        assert_eq!(weighted_composite(&videos[1..]), 0.0);
        assert_eq!(composite_confidence(&golden), Some(0.5));
        assert_eq!(composite_score(&golden), Some(0.74));
        assert_eq!(bundle_version(&golden), None);
        assert!(!norm_consented(&golden));
    }

    #[test]
    fn device_flag_is_kept_and_missing_flag_is_derived() {
        let p = ScoringProfile::default();
        let mut req = golden_1_6();
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Yellow);
        req.attention_flag = Some(None);
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Green);
        // Without perVideoComposite the weighted composite (0.74) is used.
        req.raw_payload.per_video_composite = None;
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Green);
        req.attention_flag = None;
        req.valid_ratio = 0.59;
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Unreliable);
        req.valid_ratio = 0.60;
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Green);
    }

    /// INV-1: the floor and the flag thresholds come from the profile; the defaults are
    /// today's constants.
    #[test]
    fn derived_flag_reads_the_profile() {
        let mut p = ScoringProfile::default();
        assert_eq!(p.low_confidence_valid_ratio, MIN_VALID_RATIO_FOR_FLAG);
        assert_eq!(flag_thresholds(&p), FlagThresholds::default());
        let mut req = golden_1_6();
        req.attention_flag = None;
        req.valid_ratio = 0.70;
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Green);
        p.low_confidence_valid_ratio = 0.75;
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Unreliable);
        p.low_confidence_valid_ratio = 0.60;
        // Composite 0.74: green under the default floor 0.65, yellow under 0.80.
        p.flag.green_floor = 0.80;
        assert_eq!(attention_flag(&req, &p), SessionAttentionFlag::Yellow);
    }

    #[test]
    fn age_months_is_a_whole_number_in_range() {
        let mut req = golden_1_6();
        assert_eq!(age_months(&req), None);
        for (v, want) in [
            (json!(30), Some(30)),
            (json!(30.0), Some(30)),
            (json!(0), Some(0)),
            (json!(240), Some(240)),
            (json!(30.5), None),
            (json!(-1), None),
            (json!(241), None),
            (json!("30"), None),
        ] {
            req.raw_payload.extra.insert("ageMonths".into(), v.clone());
            assert_eq!(age_months(&req), want, "{v}");
        }
    }
}

#[cfg(test)]
mod tracking_tests {
    use super::*;

    fn golden_1_7() -> SessionIngestRequest {
        session_v1_8::upgrade_v1_7(
            serde_json::from_str(include_str!(
                "../../../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
            ))
            .unwrap(),
        )
    }

    fn unscorable_1_8() -> SessionIngestRequest {
        serde_json::from_str(include_str!(
            "../../../asd-contract/tests/fixtures/session-v1.8-unscorable.json"
        ))
        .unwrap()
    }

    #[test]
    fn scoring_columns_are_stored_for_1_8_only() {
        let scored = golden_1_7();
        assert_eq!(scoring_columns(&scored), (None, None), "1.7 cannot say");
        assert_eq!(social_preference(&scored), Some(0.62));
        assert!(affect(&scored).is_some());

        let unscorable = unscorable_1_8();
        assert_eq!(
            scoring_columns(&unscorable),
            (Some("unscorable"), Some("aggregation_failed"))
        );
        assert_eq!(social_preference(&unscorable), None);
        assert_eq!(affect(&unscorable), None);
        let p = ScoringProfile::default();
        assert_eq!(
            attention_flag(&unscorable, &p),
            SessionAttentionFlag::Unreliable
        );
        assert_eq!(
            calibrated_distance_cm(&unscorable),
            Some(38.0),
            "QS-5 as 1.7"
        );

        let mut scored_1_8 = scored.clone();
        scored_1_8.schema_version = "1.8".into();
        assert_eq!(scoring_columns(&scored_1_8), (Some("scored"), None));
        assert_eq!(attention_flag(&scored_1_8, &p), attention_flag(&scored, &p));
    }

    #[test]
    fn tracking_columns_come_from_the_record() {
        let req = golden_1_7();
        assert_eq!(
            tracking_columns(&req),
            TrackingColumns {
                verdict: Some("good".into()),
                accuracy_deg: Some(1.63),
                calibration_quality: Some("good".into()),
                calibration_model: Some("screenAffine".into()),
                pipeline_version: Some("asd-gaze/1.0.0".into()),
                device_model: Some("iPad13,18".into()),
            }
        );
        let mut none = req.clone();
        none.raw_payload.tracking_quality = None;
        assert_eq!(tracking_columns(&none), TrackingColumns::default());
    }

    #[test]
    fn distance_is_measured_for_1_7_and_as_sent_for_1_6() {
        let mut req = golden_1_7();
        req.calibrated_distance_cm = Some(35.0);
        assert_eq!(calibrated_distance_cm(&req), Some(38.0));
        req.raw_payload.tracking_quality = None;
        assert_eq!(calibrated_distance_cm(&req), None);
        req.schema_version = "1.6".into();
        assert_eq!(calibrated_distance_cm(&req), Some(35.0));
    }
}
