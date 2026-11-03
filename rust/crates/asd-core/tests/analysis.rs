//! owner: WP-X.1
//!
//! PRM-9, RPX-2, RUN-4, PRV-1: the core analysis seams.
//!
//! - the core's profile sections equal the contract's `ScoringProfile` (defaults, JSON both
//!   ways), and the core's run summary decodes as the contract's;
//! - every metrics golden vector is byte-identical through `extract_with` with default params;
//! - `evaluate(Score)` equals today's `rescoreSession` (the asd-wasm blessed output, and the
//!   same primitives on the 1.8 fixture);
//! - `Aggregator` against a reference computation; `suppress`; seeded determinism.

use std::collections::BTreeMap;
use std::path::PathBuf;

use asd_contract::config::ScoringProfile;
use asd_core::analysis::{
    changed_params, dist, evaluate, suppress, Aggregator, AggregatorOptions, AnalysisProfile,
    AoiParams, BandSummary, Dist, DistComparison, FlagCounts, FlagSummary, Level, LevelRequest,
    MetricsParams, QualityStatus, RowFlag, RowSide, RunSummary, SessionInput, SessionRow,
    Unscorable, MIN_N_FOR_DECILES,
};
use asd_core::metrics::{self, AoiRole};
use asd_core::norms::{default_age_brackets, AggregateParams};
use asd_core::types::{GazeFrame, NormativeBaselineSet, PerVideoScoreReport};
use asd_core::{flag, scoring};
use serde_json::{json, Value};

const BASELINES: &str = include_str!("fixtures/normative_baselines.json");
const SESSION_V1_6: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"
));
const SESSION_V1_7: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
));
const SESSION_V1_8: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.8.json"
));
/// The blessed native output of asd-wasm `rescore_session` (today's re-score).
const RESCORE_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../asd-wasm/tests/golden/rescore_v1.json"
));

fn core_profile(p: &ScoringProfile) -> AnalysisProfile {
    serde_json::from_value(serde_json::to_value(p).unwrap()).expect("profile decodes in core")
}

fn baselines() -> NormativeBaselineSet {
    serde_json::from_str(BASELINES).unwrap()
}

// ---- PRM-9: the profile sections ------------------------------------------------------------

#[test]
fn core_profile_defaults_equal_the_contract_scoring_profile() {
    let contract = ScoringProfile::default();
    assert_eq!(core_profile(&contract), AnalysisProfile::default());

    // Same JSON for every section the core reads (the contract adds only schemaVersion).
    let mut c = serde_json::to_value(&contract).unwrap();
    c.as_object_mut().unwrap().remove("schemaVersion");
    assert_eq!(c, serde_json::to_value(AnalysisProfile::default()).unwrap());

    // Sections individually, both ways.
    assert_eq!(
        serde_json::to_value(&contract.metrics).unwrap(),
        serde_json::to_value(MetricsParams::default()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&contract.aoi).unwrap(),
        serde_json::to_value(AoiParams::default()).unwrap()
    );
    assert_eq!(
        AggregateParams::from(
            &AnalysisProfile::default().norms,
            &AnalysisProfile::default().age_bands
        )
        .min_n,
        AggregateParams::default().min_n
    );
    assert_eq!(MIN_N_FOR_DECILES, asd_contract::research::MIN_N_FOR_DECILES);
}

#[test]
fn every_profile_value_reaches_the_core() {
    let mut p = ScoringProfile::default();
    p.flag.green_floor = 0.7;
    p.low_confidence_valid_ratio = 0.55;
    p.quality_gate.min_confidence = 0.4;
    p.tracking_quality.gap_fill.max_ms = 90.0;
    p.metrics.ivt_velocity_units_per_sec = 2.0;
    p.metrics.min_fixation_duration_sec = 0.1;
    p.metrics.fallback_frame_interval_sec = 1.0 / 30.0;
    p.metrics.smile_reactivity_window_sec = 0.25;
    p.metrics.flat_affect_gain = 40.0;
    p.aoi.margin_norm = 0.05;
    p.aoi.dwell_cap_ms = 80;
    p.age_bands.truncate(2);
    p.norms.min_n = 20;
    p.norms.include_imputed = true;
    p.review.low_quality_valid_ratio = 0.75;
    let core = core_profile(&p);
    let mut c = serde_json::to_value(&p).unwrap();
    c.as_object_mut().unwrap().remove("schemaVersion");
    assert_eq!(c, serde_json::to_value(&core).unwrap());

    let agg = AggregateParams::from(&core.norms, &core.age_bands);
    assert_eq!(agg.min_n, 20);
    assert!(agg.include_imputed);
    assert_eq!(agg.brackets, core.age_bands);

    // A 1.1.0 body (no 1.2.0 sections) decodes to the defaults.
    let mut old = serde_json::to_value(ScoringProfile::default()).unwrap();
    for k in ["metrics", "aoi", "ageBands", "norms", "review"] {
        old.as_object_mut().unwrap().remove(k);
    }
    let decoded: AnalysisProfile = serde_json::from_value(old).unwrap();
    assert_eq!(decoded, AnalysisProfile::default());
}

#[test]
fn changed_params_names_the_registry_ids_that_differ() {
    let base = AnalysisProfile::default();
    assert!(changed_params(&base, &base).is_empty());
    let mut p = base.clone();
    p.metrics.ivt_velocity_units_per_sec = 2.0;
    p.flag.green_floor = 0.7;
    p.age_bands.pop();
    assert_eq!(
        changed_params(&p, &base),
        vec![
            "ageBands".to_owned(),
            "flag.greenFloor".to_owned(),
            "metrics.ivtVelocityUnitsPerSec".to_owned(),
        ]
    );
}

// ---- PRM-9: metrics goldens byte-identical with default params ------------------------------

fn load_dir(dir: &str) -> Vec<(String, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(dir);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            (
                name,
                serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn metrics_goldens_are_byte_identical_through_extract_with_defaults() {
    let mut n = 0;
    for (name, case) in load_dir("metrics") {
        let input = &case["input"];
        let frames: Vec<GazeFrame> = serde_json::from_value(input["frames"].clone()).unwrap();
        let (metrics_p, aoi_p) = (MetricsParams::default(), AoiParams::default());
        let actual = match case["function"].as_str() {
            Some("extract") => serde_json::to_string(&metrics::extract_with(
                &frames,
                &AoiRole::builtin(),
                &metrics_p,
                &aoi_p,
            ))
            .unwrap(),
            Some("extract_clip") => serde_json::to_string(&metrics::extract_clip_with(
                input["stimulusId"].as_str().unwrap(),
                &frames,
                input["timeOriginSec"].as_f64().unwrap(),
                &AoiRole::builtin(),
                &metrics_p,
            ))
            .unwrap(),
            other => panic!("{name}: {other:?}"),
        };
        let legacy = match case["function"].as_str() {
            Some("extract") => serde_json::to_string(&metrics::extract(&frames)).unwrap(),
            _ => serde_json::to_string(&metrics::extract_clip(
                input["stimulusId"].as_str().unwrap(),
                &frames,
                input["timeOriginSec"].as_f64().unwrap(),
            ))
            .unwrap(),
        };
        assert_eq!(actual, legacy, "{name}");
        n += 1;
    }
    assert!(n >= 23, "{n}");
}

#[test]
fn extract_with_reads_every_metrics_param() {
    // One second at 60 Hz: still gaze except frames 20..40, which jitter by 0.05 every other
    // frame (3 units/s: below the default 5, above 2); a smile after 0.3 s; low expressivity.
    let mut frames = Vec::new();
    for i in 0..60 {
        let t = i as f64 / 60.0;
        let jitter = if (20..40).contains(&i) && i % 2 == 1 {
            0.05
        } else {
            0.0
        };
        frames.push(json!({
            "gaze": {"valid": true, "x_norm": 0.3 + jitter, "y_norm": 0.5,
                     "video_time_sec": t, "clip_media_time_sec": t},
            "affect": {"socialSmileIntensity": if t > 0.3 { 0.8 } else { 0.1 },
                       "expressivityIndex": 0.01},
            "activeAOINames": [], "qualityVerdict": "optimal", "stimulusId": "c"
        }));
    }
    let frames: Vec<GazeFrame> = serde_json::from_value(Value::Array(frames)).unwrap();
    let base = MetricsParams::default();
    let aoi = AoiParams::default();
    let get = |p: &MetricsParams, key: &str| {
        metrics::extract_with(&frames, &AoiRole::builtin(), p, &aoi)[0].metrics[key]
    };
    // At 2 units/s the jitter splits the one fixation in two shorter ones.
    let mut ivt = base.clone();
    ivt.ivt_velocity_units_per_sec = 2.0;
    assert_eq!(get(&base, "fixation_count"), 1.0);
    assert_eq!(get(&ivt, "fixation_count"), 2.0);
    assert!(get(&ivt, "mean_fixation_duration_ms") < get(&base, "mean_fixation_duration_ms"));

    let mut min_dur = base.clone();
    min_dur.min_fixation_duration_sec = 2.0;
    assert_eq!(get(&min_dur, "fixation_count"), 0.0);

    let mut window = base.clone();
    window.smile_reactivity_window_sec = 0.6; // baseline now includes the smile
    assert!(
        get(&window, "smile_reactivity_index") < get(&base, "smile_reactivity_index"),
        "a longer onset window raises the baseline"
    );

    let mut gain = base.clone();
    gain.flat_affect_gain = 10.0;
    assert!(get(&gain, "flat_affect_score") > get(&base, "flat_affect_score"));

    // Fallback interval: frames without media time and all at time 0.
    let flat: Vec<GazeFrame> = frames
        .iter()
        .cloned()
        .map(|mut f| {
            f.gaze.clip_media_time_sec = None;
            f.gaze.video_time_sec = 0.0;
            f
        })
        .collect();
    let mut slow = base.clone();
    slow.fallback_frame_interval_sec = 1.0 / 30.0;
    let d = |p: &MetricsParams| {
        metrics::extract_with(&flat, &AoiRole::builtin(), p, &aoi)[0].observed_duration_sec
    };
    assert!((d(&base) - 1.0).abs() < 1e-12);
    assert!((d(&slow) - 2.0).abs() < 1e-12);
}

// ---- RPX-2: evaluate(Score) equals today's rescoreSession ---------------------------------

fn reports_of(raw_payload: &Value) -> Vec<PerVideoScoreReport> {
    serde_json::from_value(raw_payload["perVideo"].clone()).expect("perVideo decodes")
}

fn session_input(raw_payload: &Value, age: Option<i64>, scored_at: &str) -> SessionInput {
    SessionInput {
        session_id: raw_payload["sessionId"].as_str().unwrap_or("").to_owned(),
        age_months: age,
        reports: reports_of(raw_payload),
        has_reports: raw_payload.get("perVideo").is_some(),
        valid_frame_ratio: raw_payload["validFrameRatio"].as_f64(),
        tracking: serde_json::from_value(raw_payload["trackingQuality"].clone()).ok(),
        scored_at_utc: scored_at.to_owned(),
        research: None,
    }
}

fn strict_profile() -> ScoringProfile {
    let mut p = ScoringProfile::default();
    p.flag.green_floor = 0.9;
    p.flag.yellow_floor = 0.8;
    p.flag.atypical_share_for_red = 0.2;
    p
}

#[test]
fn evaluate_score_equals_the_blessed_rescore_session_output() {
    let golden: Value = serde_json::from_str(RESCORE_GOLDEN).unwrap();
    let cases = [
        (
            "v1_7_record_default_profile",
            SESSION_V1_7,
            ScoringProfile::default(),
            Some(28),
            "2026-11-01T00:00:00Z",
        ),
        (
            "v1_6_record_strict_profile_younger_bracket",
            SESSION_V1_6,
            strict_profile(),
            Some(14),
            "",
        ),
    ];
    for (name, doc, profile, age, scored_at) in cases {
        let expected = &golden["vectors"][name];
        let raw: Value = serde_json::from_str::<Value>(doc).unwrap()["rawPayload"].clone();
        let e = evaluate(
            &session_input(&raw, age, scored_at),
            &core_profile(&profile),
            &baselines(),
            LevelRequest::Auto,
        );
        assert_eq!(e.level, Level::Score, "{name}");
        assert_eq!(
            serde_json::to_value(&e.score).unwrap(),
            expected["score"],
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(&e.flag).unwrap(),
            expected["flag"],
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(&e.warnings).unwrap(),
            expected["warnings"],
            "{name}"
        );
    }
}

#[test]
fn evaluate_score_on_the_1_8_fixture_equals_the_rescore_path() {
    let doc: Value = serde_json::from_str(SESSION_V1_8).unwrap();
    assert_eq!(doc["schemaVersion"], "1.8");
    let raw = doc["rawPayload"].clone();
    let input = session_input(&raw, Some(28), "2026-11-01T00:00:00Z");
    let base = baselines();
    let e = evaluate(
        &input,
        &AnalysisProfile::default(),
        &base,
        LevelRequest::Score,
    );

    // Today's `rescore_session` without frames, step by step.
    let sets = metrics::from_reports(&input.reports);
    let score = scoring::score_session(&sets, &base, 28, &[], "2026-11-01T00:00:00Z");
    let flag = flag::assess(&score, &flag::FlagThresholds::default());
    assert_eq!(e.score.as_ref(), Some(&score));
    assert_eq!(e.flag.as_ref(), Some(&flag));
    assert_eq!(e.metrics, sets);
    assert_eq!(e.valid_ratio, Some(0.93));
    assert!(e.tracking.is_some(), "1.8 trackingQuality decodes");

    // The profile's threshold, not the constant: everything low-confidence at 0.99.
    let strict = AnalysisProfile {
        low_confidence_valid_ratio: 0.99,
        ..AnalysisProfile::default()
    };
    let e2 = evaluate(&input, &strict, &base, LevelRequest::Score);
    assert!(e2.score.unwrap().videos.iter().all(|v| v.low_confidence));
    assert_eq!(e2.quality_status, QualityStatus::Unreliable);
}

#[test]
fn evaluate_without_age_is_unscorable_and_frames_without_a_log_falls_back() {
    let raw: Value = serde_json::from_str::<Value>(SESSION_V1_7).unwrap()["rawPayload"].clone();
    let e = evaluate(
        &session_input(&raw, None, ""),
        &AnalysisProfile::default(),
        &baselines(),
        LevelRequest::Score,
    );
    assert_eq!(e.unscorable, Some(Unscorable::NoAge));
    assert!(e.score.is_none() && e.flag.is_none());
    assert!(!e.metrics.is_empty());

    let e = evaluate(
        &session_input(&raw, Some(28), ""),
        &AnalysisProfile::default(),
        &baselines(),
        LevelRequest::Frames,
    );
    assert_eq!(e.level, Level::Score);
    assert!(e.warnings.contains(&"level_unavailable: frames".to_owned()));
}

// ---- RUN-4: Aggregator --------------------------------------------------------------------

/// xorshift64*: test data only.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn row(age: i64, published: f64, draft: f64) -> SessionRow {
    SessionRow {
        level: Level::Score,
        age_months: Some(age),
        published: RowSide {
            composite: Some(published),
            flag: Some(if published >= 0.65 {
                RowFlag::Green
            } else {
                RowFlag::Yellow
            }),
            quality_status: Some(QualityStatus::Good),
            metrics: BTreeMap::new(),
        },
        draft: RowSide {
            composite: Some(draft),
            flag: Some(if draft >= 0.65 {
                RowFlag::Green
            } else {
                RowFlag::Red
            }),
            quality_status: Some(QualityStatus::Good),
            metrics: BTreeMap::new(),
        },
        warnings: vec!["param_not_applied: metrics.ivtVelocityUnitsPerSec".to_owned()],
    }
}

fn reference_quantile(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * (1.0 + b.abs())
}

#[test]
fn aggregator_matches_a_reference_computation_on_1000_values() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let values: Vec<f64> = (0..1000).map(|_| rng.next() * 2.0 - 0.5).collect();
    let mut agg = Aggregator::new(&default_age_brackets(), AggregatorOptions::default());
    for (i, v) in values.iter().enumerate() {
        agg.push(&row(12 + (i % 49) as i64, *v, v + 0.1));
    }
    let s = agg.finish(7);
    let d = &s.overall.composite.published;

    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sd = (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    let mut sorted = values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(d.n, Some(1000));
    assert!(close(d.mean.unwrap(), mean), "{:?} {mean}", d.mean);
    assert!(close(d.sd.unwrap(), sd), "{:?} {sd}", d.sd);
    assert!(close(d.median.unwrap(), reference_quantile(&sorted, 0.5)));
    let [q1, q3] = d.iqr.unwrap();
    assert!(close(q1, reference_quantile(&sorted, 0.25)));
    assert!(close(q3, reference_quantile(&sorted, 0.75)));
    assert!(close(d.p10.unwrap(), reference_quantile(&sorted, 0.10)));
    assert!(close(d.p90.unwrap(), reference_quantile(&sorted, 0.90)));
    let [lo, hi] = d.ci95_mean.unwrap();
    assert!(lo < mean && mean < hi, "{lo} {mean} {hi}");
    let se = sd / n.sqrt();
    assert!(
        (hi - lo - 2.0 * 1.96 * se).abs() < 0.3 * 2.0 * 1.96 * se,
        "CI width"
    );
    let hist = d.hist.as_ref().unwrap();
    assert_eq!(hist.counts.iter().sum::<u32>(), 1000);
    assert_eq!(hist.edges.len(), hist.counts.len() + 1);

    // Delta = draft - published = 0.1 for everyone.
    let delta = &s.overall.composite.delta;
    assert!(close(delta.mean.unwrap(), 0.1));
    assert!(delta.sd.unwrap() < 1e-12);

    // Counts, bands and transitions.
    assert_eq!(s.counts.sessions, 1000);
    assert_eq!(s.counts.by_level.score, 1000);
    assert_eq!(
        s.counts.params_not_applied["metrics.ivtVelocityUnitsPerSec"],
        1000
    );
    assert_eq!(s.by_age_band.len(), 5);
    assert_eq!(
        s.by_age_band.iter().map(|b| b.n.unwrap()).sum::<u32>(),
        1000
    );
    let t: u32 = s.flags.transitions.iter().map(|(_, _, n)| n.unwrap()).sum();
    assert_eq!(t, 1000);
    let rates = s.overall.flag_rate.published.unwrap();
    assert!(close(
        rates.green + rates.yellow + rates.red + rates.unreliable,
        1.0
    ));
    // 15 metric keys by default, empty here (rows carry no metrics).
    assert_eq!(s.overall.metrics.len(), 15);
    assert_eq!(s.overall.metrics["fixation_count"].published.n, Some(0));
}

#[test]
fn bootstrap_is_seeded_and_deterministic() {
    let mut rng = Rng(42);
    let values: Vec<f64> = (0..200).map(|_| rng.next()).collect();
    let o = AggregatorOptions::default();
    assert_eq!(dist(&values, &o, 1), dist(&values, &o, 1));
    assert_ne!(
        dist(&values, &o, 1).ci95_mean,
        dist(&values, &o, 2).ci95_mean
    );

    let build = |seed: u64| {
        let mut agg = Aggregator::new(&default_age_brackets(), AggregatorOptions::default());
        for (i, v) in values.iter().enumerate() {
            agg.push(&row(24 + (i % 20) as i64, *v, v * 0.9));
        }
        serde_json::to_string(&agg.finish(seed)).unwrap()
    };
    assert_eq!(build(11), build(11), "same seed, same bytes (RUN-5)");
    assert_ne!(build(11), build(12));

    let no_boot = AggregatorOptions {
        bootstrap: false,
        ..AggregatorOptions::default()
    };
    assert_eq!(dist(&values, &no_boot, 1).ci95_mean, None);
}

#[test]
fn run_summary_json_decodes_as_the_contract_type() {
    let mut agg = Aggregator::new(&default_age_brackets(), AggregatorOptions::default());
    for i in 0..30 {
        agg.push(&row(24 + i % 12, 0.5 + i as f64 / 100.0, 0.6));
    }
    agg.exclude("no_age");
    let mut s = agg.finish(3);
    suppress(&mut s, 12);
    let core = serde_json::to_value(&s).unwrap();
    let contract: asd_contract::research::RunSummary =
        serde_json::from_value(core.clone()).expect("decodes as the contract RunSummary");
    assert_eq!(serde_json::to_value(&contract).unwrap(), core);
    let back: RunSummary =
        serde_json::from_value(serde_json::to_value(&contract).unwrap()).unwrap();
    assert_eq!(back, s);
}

// ---- PRV-1: suppress -----------------------------------------------------------------------

fn full_dist(n: u32) -> Dist {
    let values: Vec<f64> = (0..n).map(|i| i as f64).collect();
    dist(&values, &AggregatorOptions::default(), 1)
}

fn band(id: &str, n: u32) -> BandSummary {
    let c = DistComparison {
        published: full_dist(n),
        draft: full_dist(n),
        delta: full_dist(n),
    };
    let mut b = BandSummary {
        band: id.to_owned(),
        n: Some(n),
        composite: c.clone(),
        ..BandSummary::default()
    };
    b.metrics.insert("eye_dwell_ratio".to_owned(), c);
    b.flag_rate.published = Some(Default::default());
    b.flag_rate.draft = Some(Default::default());
    b
}

#[test]
fn suppress_nulls_small_cells_and_deciles() {
    let mut s = RunSummary {
        by_age_band: vec![band("a", 11), band("b", 12), band("c", 19), band("d", 20)],
        overall: band("overall", 62),
        flags: FlagSummary {
            published: FlagCounts {
                green: Some(0),
                yellow: Some(5),
                red: Some(12),
                unreliable: Some(45),
            },
            transitions: vec![
                ("green".into(), "red".into(), Some(3)),
                ("red".into(), "red".into(), Some(0)),
                ("yellow".into(), "red".into(), Some(13)),
            ],
            ..FlagSummary::default()
        },
        ..RunSummary::default()
    };
    suppress(&mut s, 12);
    assert_eq!(s.small_cell_k, 12);

    // n = 11: every field of every Dist null, band suppressed.
    let a = &s.by_age_band[0];
    assert!(a.suppressed && a.n.is_none());
    assert!(a.flag_rate.published.is_none() && a.flag_rate.draft.is_none());
    for d in [
        &a.composite.published,
        &a.composite.draft,
        &a.composite.delta,
        &a.metrics["eye_dwell_ratio"].draft,
    ] {
        assert_eq!(
            d,
            &Dist {
                suppressed: true,
                ..Dist::default()
            }
        );
    }
    let json = serde_json::to_value(&a.composite.published).unwrap();
    assert_eq!(json["n"], Value::Null);
    assert_eq!(json["suppressed"], true);

    // n = 12: kept; deciles null below 20.
    let b = &s.by_age_band[1];
    assert!(!b.suppressed && b.n == Some(12));
    assert_eq!(b.composite.published.n, Some(12));
    assert!(b.composite.published.mean.is_some() && b.composite.published.ci95_mean.is_some());
    assert!(b.composite.published.p10.is_none() && b.composite.published.p90.is_none());

    // n = 19: p10/p90 null, the rest kept; n = 20: all kept.
    let c = &s.by_age_band[2].composite.published;
    assert!(c.p10.is_none() && c.p90.is_none() && c.median.is_some() && c.iqr.is_some());
    let d = &s.by_age_band[3].composite.published;
    assert!(d.p10.is_some() && d.p90.is_some());
    const { assert!(MIN_N_FOR_DECILES == 20) };

    // Counts and transitions: 0 < n < k null, zero stays zero.
    assert_eq!(s.flags.published.green, Some(0));
    assert_eq!(s.flags.published.yellow, None);
    assert_eq!(s.flags.published.red, Some(12));
    assert_eq!(s.flags.transitions[0].2, None);
    assert_eq!(s.flags.transitions[1].2, Some(0));
    assert_eq!(s.flags.transitions[2].2, Some(13));
    assert_eq!(s.counts.suppressed_cells, 2);

    // Idempotent.
    let once = s.clone();
    suppress(&mut s, 12);
    assert_eq!(s, once);

    // k = 5 keeps the 11-band.
    let mut t = RunSummary {
        by_age_band: vec![band("a", 11)],
        ..RunSummary::default()
    };
    suppress(&mut t, 5);
    assert_eq!(t.by_age_band[0].n, Some(11));
    assert!(t.by_age_band[0].composite.published.mean.is_some());
}
