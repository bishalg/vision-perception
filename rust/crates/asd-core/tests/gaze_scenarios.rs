//! owner: WP-G.5
//!
//! TV-1 / TV-2 / REP-1 end to end: the synthetic tracking generator
//! (`asd_core::synth_tracking`), replay of its logs through `GazePipeline`
//! (`asd_core::gaze::replay`), and one golden per scenario.
#![cfg(feature = "synth")]

use std::path::PathBuf;

use asd_core::gaze::raw_log;
use asd_core::gaze::replay::{replay, ReplayOptions, ReplayReport};
use asd_core::synth_tracking::{generate, generate_jsonl, Scenario, SCENARIOS};
use asd_core::types::tracking::{
    CalModelKind, CalibrationQuality, Orientation, RecalAction, RecalReason, TargetKind,
    TargetOutcome, TrackingVerdict,
};
use serde_json::{json, Value};

const SEED: u64 = 7;

fn run(scenario: Scenario) -> ReplayReport {
    let log = generate(scenario, SEED);
    replay(&log, &ReplayOptions::default()).expect("replay")
}

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn opt3(x: Option<f64>) -> Value {
    x.map_or(Value::Null, |v| json!(r3(v)))
}

/// The values a scenario golden pins (rounded to 3 decimals).
fn observed(rep: &ReplayReport) -> Value {
    let q = &rep.summary;
    let val = q.validation.as_ref();
    let outcomes: Vec<Value> = rep
        .targets
        .iter()
        .map(|t| json!(format!("{:?}:{:?}", t.kind, t.outcome)))
        .collect();
    json!({
        "frames": rep.frames,
        "recordedFrames": rep.recorded_frames,
        "targetOutcomes": outcomes,
        "calibration": {
            "quality": q.calibration.quality,
            "model": q.calibration.model,
            "usablePoints": q.calibration.usable_points,
            "residualDeg": opt3(q.calibration.residual_deg),
        },
        "validation": {
            "complete": val.map(|v| v.complete),
            "accuracyDeg": opt3(val.and_then(|v| v.accuracy_deg)),
            "precisionRmsDeg": opt3(val.and_then(|v| v.precision_rms_deg)),
            "dataLoss": val.map(|v| r3(v.data_loss)),
        },
        "driftChecksDeg": q.drift_checks.iter().map(|c| opt3(c.drift_deg)).collect::<Vec<_>>(),
        "maxDriftDeg": opt3(q.max_drift_deg),
        "advice": {"action": rep.advice.action, "reasons": rep.advice.reasons},
        "loss": {
            "trackLossSec": r3(q.loss.track_loss_sec),
            "blinkSec": r3(q.loss.blink_sec),
            "offscreenSec": r3(q.loss.offscreen_sec),
            "artifactSec": r3(q.loss.artifact_sec),
            "gapFilledSec": r3(q.loss.gap_filled_sec),
            "longestGapSec": r3(q.loss.longest_gap_sec),
        },
        "meanSampleConfidence": r3(q.confidence.mean_sample),
        "estimatedAccuracyDeg": opt3(q.estimated_accuracy_deg),
        "verdict": q.verdict,
    })
}

fn accuracy(rep: &ReplayReport) -> f64 {
    rep.summary
        .validation
        .as_ref()
        .and_then(|v| v.accuracy_deg)
        .expect("validation accuracy")
}

fn precision(rep: &ReplayReport) -> f64 {
    rep.summary
        .validation
        .as_ref()
        .and_then(|v| v.precision_rms_deg)
        .expect("validation precision")
}

fn assert_clean_advice(rep: &ReplayReport) {
    assert_eq!(rep.advice.action, RecalAction::None, "{:?}", rep.advice);
    assert!(rep.advice.reasons.is_empty());
}

fn count_outcomes(rep: &ReplayReport, kind: TargetKind, outcome: TargetOutcome) -> usize {
    rep.targets
        .iter()
        .filter(|t| t.kind == kind && t.outcome == outcome)
        .count()
}

// ------------------------------------------------------------------------------------------
// TV-1: the generator
// ------------------------------------------------------------------------------------------

#[test]
fn same_seed_gives_identical_bytes_and_other_seeds_differ() {
    for s in SCENARIOS {
        let (a, b) = (generate_jsonl(s, 11), generate_jsonl(s, 11));
        assert_eq!(a, b, "{s}");
        assert_ne!(a, generate_jsonl(s, 12), "{s}: the seed must matter");
    }
}

#[test]
fn generated_logs_are_valid_rep1_logs_with_ground_truth() {
    for s in SCENARIOS {
        let text = generate_jsonl(s, SEED);
        let parsed = raw_log::parse(&text).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(raw_log::to_jsonl(&parsed).unwrap(), text, "{s}: canonical");
        let gt = parsed.header.ground_truth.as_ref().expect("ground truth");
        assert_eq!(gt["seed"], SEED);
        assert_eq!(gt["label"], s.name());
        assert_eq!(parsed.header.condition.as_deref(), Some(s.name()));
        let t = &parsed.header.targets;
        assert!(
            t.windows(2).all(|w| w[0].start_sec <= w[1].start_sec),
            "{s}"
        );
        assert_eq!(
            t.iter()
                .filter(|x| x.kind == TargetKind::Calibration)
                .count(),
            5
        );
    }
}

#[test]
fn scenario_names_round_trip() {
    for s in SCENARIOS {
        assert_eq!(s.name().parse::<Scenario>().unwrap(), s);
    }
    assert!("nope".parse::<Scenario>().unwrap_err().contains("ideal"));
}

#[test]
fn frame_clock_follows_the_scenario_rate() {
    let l60 = generate(Scenario::Ideal, 1);
    let l30 = generate(Scenario::Fps30, 1);
    let dt = |l: &asd_core::gaze::raw_log::RawLog| l.samples[1].t_sec - l.samples[0].t_sec;
    assert!((dt(&l60) - 1.0 / 60.0).abs() < 1e-5);
    assert!((dt(&l30) - 1.0 / 30.0).abs() < 1e-5);
    let rep = run(Scenario::Fps30);
    let hz = rep.summary.device.sample_rate_hz.unwrap();
    assert!((hz - 30.0).abs() < 0.5, "{hz}");
}

/// TV-1 "every orientation": the generator's camera maths agrees with the raycast in all four
/// interface orientations, on a phone and a tablet.
#[test]
fn every_orientation_replays_to_a_good_calibration() {
    use asd_core::synth_tracking::Spec;
    for model in ["iPhone17,3", "iPad16,3"] {
        for o in Orientation::ALL {
            let mut spec = Spec::base();
            spec.model_identifier = model.into();
            spec.orientation = o;
            spec.bias_deg = [1.0, -1.0];
            let log = asd_core::synth_tracking::generate_with(&spec, "orientation", 3);
            assert!(log.samples.iter().all(|s| s.orientation == o));
            let rep = replay(&log, &ReplayOptions::default()).unwrap();
            assert_eq!(rep.summary.calibration.quality, CalibrationQuality::Good);
            assert!(accuracy(&rep) < 0.3, "{model} {o:?}: {}", accuracy(&rep));
            assert_eq!(rep.summary.device.orientation, o);
        }
    }
}

// ------------------------------------------------------------------------------------------
// Replay
// ------------------------------------------------------------------------------------------

#[test]
fn replay_is_deterministic() {
    let log = generate(Scenario::BlinksAndLoss, SEED);
    let a = replay(&log, &ReplayOptions::default()).unwrap();
    let b = replay(&log, &ReplayOptions::default()).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

#[test]
fn only_clip_frames_are_recorded() {
    let rep = run(Scenario::Ideal);
    // Two 6 s clips at 60 Hz.
    assert_eq!(rep.recorded_frames, 720);
    assert!(rep.frames > rep.recorded_frames);
}

#[test]
fn replay_rejects_a_bad_schedule() {
    let mut log = generate(Scenario::Ideal, 1);
    log.header.targets[1].end_sec = log.header.targets[1].start_sec - 1.0;
    let err = replay(&log, &ReplayOptions::default()).unwrap_err();
    assert!(err.to_string().contains("target 1"), "{err}");
    let mut log = generate(Scenario::Ideal, 1);
    log.header.targets.swap(0, 3);
    assert!(replay(&log, &ReplayOptions::default()).is_err());
}

#[test]
fn replay_can_force_the_calibration_model() {
    let log = generate(Scenario::BiasGain, SEED);
    for kind in [CalModelKind::ScreenAffine, CalModelKind::AngularAffine] {
        let rep = replay(
            &log,
            &ReplayOptions {
                model: Some(kind),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rep.summary.calibration.model, Some(kind));
        assert!(accuracy(&rep) < 0.5, "{kind:?}");
    }
    // Models the 5-point layout cannot support still replay without error.
    for kind in [CalModelKind::ScreenHomography, CalModelKind::ScreenPoly2] {
        replay(
            &log,
            &ReplayOptions {
                model: Some(kind),
                ..Default::default()
            },
        )
        .unwrap();
    }
}

// ------------------------------------------------------------------------------------------
// TV-2: the scenarios, with their stated tolerances
// ------------------------------------------------------------------------------------------

#[test]
fn ideal() {
    let rep = run(Scenario::Ideal);
    assert!(accuracy(&rep) <= 0.1);
    assert!(precision(&rep) <= 0.2);
    assert_eq!(rep.summary.calibration.quality, CalibrationQuality::Good);
    assert_eq!(rep.summary.verdict, TrackingVerdict::Good);
    assert_clean_advice(&rep);
    assert_eq!(rep.summary.loss.track_loss_sec, 0.0);
    assert_eq!(
        count_outcomes(&rep, TargetKind::Calibration, TargetOutcome::Hit),
        5
    );
}

/// A constant eye-model bias of (2, -1.5) degrees is calibrated away: accuracy <= 0.2 deg.
#[test]
fn bias_only() {
    let rep = run(Scenario::BiasOnly);
    assert!(accuracy(&rep) <= 0.2, "{}", accuracy(&rep));
    assert_clean_advice(&rep);
    for c in &rep.summary.drift_checks {
        assert!(c.drift_deg.unwrap() <= 0.2);
    }
}

#[test]
fn bias_gain() {
    let rep = run(Scenario::BiasGain);
    assert!(accuracy(&rep) <= 0.3, "{}", accuracy(&rep));
    assert_clean_advice(&rep);
}

/// 45 -> 52 cm after calibration: parallax adds a small error, confidence drops (CNF-1
/// pose-shift factor) but stays above the gate, and no re-calibration is needed.
#[test]
fn distance_shift_after_cal() {
    let rep = run(Scenario::DistanceShiftAfterCal);
    assert!(accuracy(&rep) <= 0.5, "{}", accuracy(&rep));
    let c = rep.summary.confidence.mean_sample;
    assert!(c > 0.5 && c < 0.95, "{c}");
    assert_clean_advice(&rep);
}

/// 45 -> 60 cm (33 %, beyond the 20 % limit): every validation frame is low confidence and
/// re-calibration is advised with `poseShift`.
#[test]
fn distance_shift_large() {
    let rep = run(Scenario::DistanceShiftLarge);
    assert_eq!(rep.advice.action, RecalAction::Recalibrate);
    assert!(rep.advice.reasons.contains(&RecalReason::PoseShift));
    assert_eq!(
        count_outcomes(&rep, TargetKind::Validation, TargetOutcome::Missed),
        4
    );
    assert!(!rep.summary.validation.as_ref().unwrap().complete);
}

#[test]
fn head_yaw_15() {
    let rep = run(Scenario::HeadYaw15);
    assert!(accuracy(&rep) <= 0.5, "{}", accuracy(&rep));
    assert_clean_advice(&rep);
    assert_eq!(rep.summary.verdict, TrackingVerdict::Good);
}

#[test]
fn head_sway() {
    let rep = run(Scenario::HeadSway);
    assert!(accuracy(&rep) <= 0.5, "{}", accuracy(&rep));
    assert!(rep.summary.conditions.head_speed_deg_per_sec_p90.unwrap() > 5.0);
}

/// Linear drift of 0.09 deg/s after the validation (about 3 deg over the session): the check
/// drifts grow monotonically and DRF-3 (two consecutive checks >= 1.5 deg) fires at the
/// second check after the drift exceeds 1.5 deg, not before.
#[test]
fn drift_3deg_linear() {
    let log = generate(Scenario::Drift3DegLinear, SEED);
    let rep = replay(&log, &ReplayOptions::default()).unwrap();
    let drifts: Vec<f64> = rep
        .summary
        .drift_checks
        .iter()
        .map(|c| c.drift_deg.expect("every check is a hit"))
        .collect();
    assert_eq!(drifts.len(), 4);
    assert!(drifts.windows(2).all(|w| w[1] > w[0]), "{drifts:?}");
    let last = *drifts.last().unwrap();
    assert!((2.5..=3.2).contains(&last), "{drifts:?}");
    assert!(accuracy(&rep) <= 0.2, "validation precedes the drift");

    let first = drifts.iter().position(|&d| d > 1.5).expect("exceeds 1.5");
    let checks: Vec<&asd_core::types::tracking::ScheduledTarget> = log
        .header
        .targets
        .iter()
        .filter(|t| t.kind == TargetKind::DriftCheck)
        .collect();
    let prefix = |n_checks: usize| {
        let cut = checks[n_checks - 1].end_sec + 0.1;
        let mut l = log.clone();
        l.samples.retain(|s| s.t_sec <= cut);
        replay(&l, &ReplayOptions::default()).unwrap()
    };
    let at_first = prefix(first + 1);
    assert!(
        !at_first.advice.reasons.contains(&RecalReason::Drift),
        "one check above 1.5 deg only warns after a second: {:?}",
        at_first.advice
    );
    let at_second = prefix(first + 2);
    assert_eq!(at_second.advice.action, RecalAction::Recalibrate);
    assert!(at_second.advice.reasons.contains(&RecalReason::Drift));
    assert_eq!(rep.advice, at_second.advice);
}

#[test]
fn blinks_and_loss() {
    let rep = run(Scenario::BlinksAndLoss);
    let l = &rep.summary.loss;
    assert!((l.track_loss_sec - 1.3).abs() < 0.05, "{l:?}");
    assert!(l.blink_sec > 1.0, "{l:?}");
    assert!(l.longest_gap_sec >= 0.8, "{l:?}");
    // Loss happens in the clips: calibration and validation are untouched.
    assert!(accuracy(&rep) <= 0.2);
    assert_eq!(rep.summary.calibration.quality, CalibrationQuality::Good);
    assert_eq!(
        rep.summary.verdict,
        TrackingVerdict::Fair,
        "{:?}",
        rep.summary
    );
}

/// GAP-1: the first gap (3 frames, 66.7 ms bracket) is filled, the second (12 frames,
/// 216.7 ms bracket) is not.
#[test]
fn gap_60ms_vs_200ms() {
    let rep = run(Scenario::Gap60msVs200ms);
    let l = &rep.summary.loss;
    assert!((l.gap_filled_sec - 3.0 / 60.0).abs() < 1e-6, "{l:?}");
    assert!((l.track_loss_sec - 15.0 / 60.0).abs() < 1e-6, "{l:?}");
    assert!((l.longest_gap_sec - 13.0 / 60.0).abs() < 1e-6, "{l:?}");
}

/// Three single-frame 40 deg spikes: each makes two frames `artifact` (the spike and the
/// return), none of the validation is affected.
#[test]
fn artifact_spike() {
    let rep = run(Scenario::ArtifactSpike);
    assert!((rep.summary.loss.artifact_sec - 6.0 / 60.0).abs() < 2e-3);
    assert!(accuracy(&rep) <= 0.2);
}

#[test]
fn orientation_landscape_ipad() {
    let rep = run(Scenario::OrientationLandscapeIpad);
    assert_eq!(rep.summary.device.orientation, Orientation::LandscapeLeft);
    assert!(rep.summary.device.screen_width_mm > rep.summary.device.screen_height_mm);
    assert!(accuracy(&rep) <= 0.3, "{}", accuracy(&rep));
    assert_clean_advice(&rep);
}

#[test]
fn fps_30() {
    let rep = run(Scenario::Fps30);
    assert!(accuracy(&rep) <= 0.3, "{}", accuracy(&rep));
    assert_clean_advice(&rep);
}

/// One validation target has no frames in the log: it is `missed`, three of four still make
/// a complete validation (VAL-3).
#[test]
fn missing_target() {
    let rep = run(Scenario::MissingTarget);
    assert_eq!(
        count_outcomes(&rep, TargetKind::Validation, TargetOutcome::Missed),
        1
    );
    assert_eq!(
        count_outcomes(&rep, TargetKind::Validation, TargetOutcome::Hit),
        3
    );
    let v = rep.summary.validation.as_ref().unwrap();
    assert!(v.complete);
    assert!(v.targets.iter().filter(|t| t.missed).count() == 1);
    assert!(accuracy(&rep) <= 0.2);
}

/// Glasses-like noise: precision is several times worse than `ideal`, accuracy stays within
/// 0.5 deg, and the log is still usable.
#[test]
fn glasses_noise() {
    let rep = run(Scenario::GlassesNoise);
    let ideal = run(Scenario::Ideal);
    assert!(precision(&rep) >= 3.0 * precision(&ideal));
    assert!(accuracy(&rep) <= 0.5, "{}", accuracy(&rep));
    assert_ne!(rep.summary.verdict, TrackingVerdict::Unusable);
}

// ------------------------------------------------------------------------------------------
// Goldens: one file per scenario (immutable: change = new file)
// ------------------------------------------------------------------------------------------

fn golden_path(s: Scenario) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/gaze/scenarios")
        .join(format!("{}_v1.json", s.name()))
}

#[test]
fn scenario_goldens() {
    let write = std::env::var("ASD_WRITE_GOLDEN").as_deref() == Ok("1");
    for s in SCENARIOS {
        let actual = observed(&run(s));
        let path = golden_path(s);
        if write {
            let doc = json!({
                "schema": "asd-golden/1",
                "function": "synth_tracking::generate + gaze::replay::replay",
                "case": format!("{}_v1", s.name()),
                "derivation": "Generated by tests/gaze_scenarios.rs::scenario_goldens (WP-G.5) with ASD_WRITE_GOLDEN=1: the synthetic tracking log of the scenario (asd_core::synth_tracking, GENERATOR_VERSION 1.0) is replayed through GazePipeline with default params; values are rounded to 3 decimals. The stated tolerances are asserted independently by the scenario tests in that file. Golden files are immutable: change = new file.",
                "input": {"scenario": s.name(), "seed": SEED, "generator": "1.0"},
                "expected": actual,
            });
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        }
        let doc: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("golden file (run with ASD_WRITE_GOLDEN=1)"),
        )
        .unwrap();
        assert_eq!(doc["schema"], "asd-golden/1");
        assert_eq!(doc["input"]["scenario"], s.name());
        assert_eq!(doc["input"]["seed"], SEED);
        assert_json_close(&doc["expected"], &actual, s.name());
    }
}

/// Structural equality; floats within 2e-3 (the goldens are rounded to 3 decimals, so a
/// last-digit flip from libm differences between platforms is absorbed).
fn assert_json_close(want: &Value, got: &Value, at: &str) {
    match (want, got) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!((a - b).abs() <= 2e-3, "{at}: {a} != {b}");
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{at}: length");
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                assert_json_close(x, y, &format!("{at}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            let ka: Vec<_> = a.keys().collect();
            let kb: Vec<_> = b.keys().collect();
            assert_eq!(ka, kb, "{at}: keys");
            for (k, x) in a {
                assert_json_close(x, &b[k], &format!("{at}.{k}"));
            }
        }
        _ => assert_eq!(want, got, "{at}"),
    }
}
