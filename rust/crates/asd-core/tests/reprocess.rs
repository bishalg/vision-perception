//! owner: WP-X.1
//!
//! RPX-1 / RPX-6: `gaze::reprocess` on synthetic research logs (`synth_tracking`, scenario
//! `ideal`, seed 7) with clips and AOIs added to the header the way the device writes them
//! (RFL-1), and `analysis::evaluate` at `frames` level.
#![cfg(feature = "synth")]

use asd_core::analysis::{evaluate, AnalysisProfile, Level, LevelRequest, ResearchInput};
use asd_core::analysis::{SessionInput, Unscorable};
use asd_core::gaze::aoi_space::{NormBox, ScreenAoi};
use asd_core::gaze::pipeline::TelemetryWriter;
use asd_core::gaze::raw_log::RawLog;
use asd_core::gaze::replay::{replay, replay_with, ReplayObserver, ReplayOptions};
use asd_core::gaze::reprocess::{reprocess, ReprocessOptions, Reprocessed};
use asd_core::metrics::{self, AoiRole};
use asd_core::quality;
use asd_core::synth_tracking::{generate, Scenario};
use asd_core::types::tracking::{ProcessedSample, RawLogClip, RawTrackingSample, TrackState};
use asd_core::types::{
    GazeFrame, GazeFrameAffect, GazeFrameSample, GazeQualityVerdict, NormativeBaselineSet,
    StimulusCatalog,
};

const CATALOG: &str = include_str!("fixtures/stimulus_catalog.json");
const BASELINES: &str = include_str!("fixtures/normative_baselines.json");
const SEED: u64 = 7;

fn catalog() -> StimulusCatalog {
    serde_json::from_str(CATALOG).unwrap()
}

fn aoi(name: &str, rect: NormBox) -> ScreenAoi {
    ScreenAoi {
        name: name.to_owned(),
        start_sec: 0.0,
        end_sec: 60.0,
        rect,
        roles: Vec::new(),
    }
}

fn halves() -> Vec<ScreenAoi> {
    vec![
        aoi(
            "Social_Actor",
            NormBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.5,
                y_max: 1.0,
            },
        ),
        aoi(
            "Geometric_Pattern",
            NormBox {
                x_min: 0.5,
                y_min: 0.0,
                x_max: 1.0,
                y_max: 1.0,
            },
        ),
    ]
}

/// The synth log with a header `clips` entry per clip, each with `aois`, and an affect value
/// on every recorded sample (as the device writes a research log).
fn research_log(aois: Vec<ScreenAoi>) -> RawLog {
    let mut log = generate(Scenario::Ideal, SEED);
    let mut ids: Vec<String> = Vec::new();
    for s in &mut log.samples {
        if let Some(id) = &s.stimulus_id {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
            let t = s.clip_media_time_sec.unwrap_or(0.0);
            s.affect = Some(GazeFrameAffect {
                social_smile_intensity: Some(if t > 1.0 { 0.6 } else { 0.1 }),
                expressivity_index: Some(0.02),
                surprise_score: Some(0.1),
            });
        }
    }
    log.header.clips = ids
        .iter()
        .enumerate()
        .map(|(order, id)| RawLogClip {
            clip_id: id.clone(),
            order: order as u32,
            catalog_version: None,
            aois: aois.clone(),
            video_aspect: None,
        })
        .collect();
    log
}

fn run(log: &RawLog, profile: &AnalysisProfile) -> Reprocessed {
    reprocess(
        log,
        &catalog(),
        &profile.gaze_processing(),
        &ReprocessOptions::default(),
    )
    .expect("reprocess")
}

fn sum_metric(r: &Reprocessed, key: &str) -> f64 {
    r.metric_sets
        .iter()
        .map(|s| s.metrics.get(key).copied().unwrap_or(0.0))
        .sum()
}

#[test]
fn ideal_with_header_aois_hits_and_extracts_through_the_same_code_path() {
    let log = research_log(halves());
    let profile = AnalysisProfile::default();
    let r = run(&log, &profile);
    let plain = replay(&log, &ReplayOptions::default()).unwrap();

    assert_eq!(r.frames_n, plain.recorded_frames);
    assert_eq!(r.frames.len(), r.frames_n);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    // Every valid recorded frame lands in one half (the halves tile the screen).
    let valid: Vec<&GazeFrame> = r
        .frames
        .iter()
        .filter(|f| metrics::is_valid_frame(f))
        .collect();
    assert!(valid.len() * 10 >= r.frames_n * 9, "ideal is mostly valid");
    let inside = |f: &GazeFrame| {
        (0.0..=1.0).contains(&f.gaze.x_norm) && (0.0..=1.0).contains(&f.gaze.y_norm)
    };
    for f in valid.iter().filter(|f| inside(f)) {
        assert!(!f.active_aoi_names.is_empty(), "{f:?}");
        let expect = if f.gaze.x_norm <= 0.5 {
            "Social_Actor"
        } else {
            "Geometric_Pattern"
        };
        assert!(f.active_aoi_names.iter().any(|n| n == expect), "{f:?}");
    }
    // Affect carried from the raw samples.
    assert!(r
        .frames
        .iter()
        .all(|f| f.affect.expressivity_index == Some(0.02)));

    // Metrics are extract_with over the built frames (same code path).
    let expected = metrics::extract_with(
        &r.frames,
        &AoiRole::from_catalog(&catalog()),
        &profile.metrics,
        &profile.aoi,
    );
    assert_eq!(r.metric_sets, expected);
    assert_eq!(r.metric_sets.len(), 2);
    let social = sum_metric(&r, "social_dwell_ratio") + sum_metric(&r, "geometric_dwell_ratio");
    assert!(
        social > 1.5,
        "both clips mostly on one half or the other: {social}"
    );
    assert!(
        sum_metric(&r, "fixation_count") >= 2.0,
        "at least one per clip"
    );
    assert!(!r.fixations.is_empty());
    let counted: f64 = sum_metric(&r, "fixation_count");
    assert_eq!(r.fixations.len() as f64, counted);

    let total: u32 = [
        r.quality_verdicts.optimal,
        r.quality_verdicts.acceptable,
        r.quality_verdicts.degraded,
        r.quality_verdicts.rejected,
    ]
    .iter()
    .sum();
    assert_eq!(total as usize, r.frames_n);
}

/// The device recipe, written from `GazeProcessingActor.processFaceAnchor` (the WP-G.7 kit
/// path): one telemetry writer at session origin 0 over every pushed frame; recorded frames
/// buffered with the written sample, the pipeline's AOI hits, the raw affect, and `rejected`
/// for a lost frame.
struct DeviceTwin {
    aois: Vec<ScreenAoi>,
    writer: TelemetryWriter,
    gate: quality::QualityGate,
    frames: Vec<GazeFrame>,
    processed: Vec<ProcessedSample>,
}

impl ReplayObserver for DeviceTwin {
    fn clip_aois(&mut self, _clip_id: &str) -> Vec<ScreenAoi> {
        self.aois.clone()
    }

    fn frame(&mut self, raw: &RawTrackingSample, p: &ProcessedSample, recording: bool) {
        let written = self.writer.sample(p);
        let eval = self.writer.evaluate(&written);
        if recording {
            let verdict = if p.track_state == TrackState::Lost {
                GazeQualityVerdict::Rejected
            } else {
                eval.verdict
            };
            self.frames.push(GazeFrame {
                gaze: GazeFrameSample {
                    is_valid: written.is_valid,
                    x_norm: written.x_norm,
                    y_norm: written.y_norm,
                    video_time_sec: written.video_time_sec,
                    clip_media_time_sec: written.clip_media_time_sec,
                },
                affect: raw.affect.clone().unwrap_or_default(),
                active_aoi_names: p.active_aoi_names.clone(),
                quality_verdict: verdict,
                stimulus_id: raw.stimulus_id.clone(),
            });
            self.processed.push(p.clone());
        }
    }
}

#[test]
fn reprocess_equals_the_device_recipe_for_the_recorded_range() {
    let log = research_log(halves());
    let r = run(&log, &AnalysisProfile::default());
    let mut twin = DeviceTwin {
        aois: halves(),
        writer: TelemetryWriter::new(0.0, quality::QualityGate::default()),
        gate: quality::QualityGate::default(),
        frames: Vec::new(),
        processed: Vec::new(),
    };
    let report = replay_with(&log, &ReplayOptions::default(), &mut twin).unwrap();
    assert_eq!(twin.gate, quality::QualityGate::default());
    assert_eq!(r.frames, twin.frames);
    assert_eq!(r.tracking, report.summary);
    // The recorded range is the clip frames, every one with a media time.
    assert!(twin
        .processed
        .iter()
        .all(|p| p.clip_media_time_sec.is_some()));
}

#[test]
fn replay_with_an_idle_observer_is_replay() {
    struct Idle;
    impl ReplayObserver for Idle {}
    let log = generate(Scenario::Ideal, SEED);
    let a = replay(&log, &ReplayOptions::default()).unwrap();
    let b = replay_with(&log, &ReplayOptions::default(), &mut Idle).unwrap();
    assert_eq!(a, b);
    // Explicit default gate: same report.
    let c = replay(
        &log,
        &ReplayOptions {
            quality_gate: Some(quality::QualityGate::default()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(a, c);
}

#[test]
fn ivt_threshold_2_changes_the_fixation_counts_on_the_synth_frames() {
    let log = research_log(halves());
    let base = run(&log, &AnalysisProfile::default());
    let mut p = AnalysisProfile::default();
    p.metrics.ivt_velocity_units_per_sec = 2.0;
    let low = run(&log, &p);
    // Same frames: the threshold acts after the hit test.
    assert_eq!(base.frames, low.frames);
    let (n0, n1) = (
        sum_metric(&base, "fixation_count"),
        sum_metric(&low, "fixation_count"),
    );
    let (d0, d1) = (
        sum_metric(&base, "mean_fixation_duration_ms"),
        sum_metric(&low, "mean_fixation_duration_ms"),
    );
    assert_ne!(n0, n1, "fixation counts change");
    // A lower velocity threshold cuts runs on smaller movements: fixations get shorter, and
    // the saccade ramps that used to sit inside a run split it.
    assert!(n1 > n0, "{n0} -> {n1}");
    assert!(d1 < d0, "{d0} -> {d1}");
    assert_eq!(low.fixations.len() as f64, n1);
}

#[test]
fn margin_grows_dwell_on_a_frame_at_an_aoi_edge() {
    // A box whose left edge sits just right of the first fixation of clip 1.
    let first = run(&research_log(halves()), &AnalysisProfile::default()).fixations[0].clone();
    let edge = NormBox {
        x_min: first.x_norm + 0.02,
        y_min: first.y_norm - 0.1,
        x_max: first.x_norm + 0.3,
        y_max: first.y_norm + 0.1,
    };
    let log = research_log(vec![aoi("Edge", edge)]);
    let key = "aoi_dwell_ratio::Edge";
    let without = run(&log, &AnalysisProfile::default());
    let mut p = AnalysisProfile::default();
    p.aoi.margin_norm = 0.05;
    let with = run(&log, &p);
    let dwell = |r: &Reprocessed| sum_metric(r, key);
    assert!(
        dwell(&with) > dwell(&without),
        "{} -> {}",
        dwell(&without),
        dwell(&with)
    );
    // A frame at the fixation point is outside the box and inside the grown one.
    let at = [first.x_norm, first.y_norm];
    assert!(!edge.contains(at));
    assert!(edge.contains_with_margin(at, 0.05));
    // A negative margin shrinks: never more dwell than without.
    p.aoi.margin_norm = -0.05;
    assert!(dwell(&run(&log, &p)) <= dwell(&without));
}

#[test]
fn catalog_fallback_and_missing_aois() {
    // No header clips: the synth clip ids are not in the catalog, so no hits and a warning.
    let mut log = research_log(halves());
    log.header.clips.clear();
    let r = run(&log, &AnalysisProfile::default());
    assert_eq!(r.warnings.len(), 2, "{:?}", r.warnings);
    assert!(r.frames.iter().all(|f| f.active_aoi_names.is_empty()));
    // Without AOIs the tracking summary is the plain replay's.
    assert_eq!(
        r.tracking,
        replay(&log, &ReplayOptions::default()).unwrap().summary
    );

    // A catalog clip of the same id supplies the boxes.
    let mut cat = catalog();
    let mut clip = cat.clips[0].clone();
    clip.id = "synth_clip_1".to_owned();
    for a in &mut clip.areas {
        a.start_sec = 0.0;
        a.end_sec = 60.0;
    }
    cat.clips.push(clip);
    let r = reprocess(
        &log,
        &cat,
        &AnalysisProfile::default().gaze_processing(),
        &ReprocessOptions::default(),
    )
    .unwrap();
    assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
    assert!(r.frames.iter().any(
        |f| f.stimulus_id.as_deref() == Some("synth_clip_1") && !f.active_aoi_names.is_empty()
    ));
}

#[test]
fn reprocess_is_deterministic_and_reads_the_gate_and_trace_options() {
    let log = research_log(halves());
    let p = AnalysisProfile::default();
    let opts = ReprocessOptions {
        trace_hz: Some(10),
        ..Default::default()
    };
    let a = reprocess(&log, &catalog(), &p.gaze_processing(), &opts).unwrap();
    let b = reprocess(&log, &catalog(), &p.gaze_processing(), &opts).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let trace = a.trace.as_ref().unwrap();
    // 60 Hz recorded frames down to 10 Hz.
    let expected = a.frames_n as f64 / 6.0;
    assert!(
        (trace.len() as f64 - expected).abs() <= 3.0,
        "{} vs {expected}",
        trace.len()
    );
    assert!(trace.iter().any(|t| t.fixation_id.is_some()));
    assert!(trace.windows(2).all(|w| w[1].t_sec > w[0].t_sec));

    // A stricter gate makes fewer frames valid.
    let mut strict = AnalysisProfile::default();
    strict.quality_gate.min_confidence = 0.999;
    strict.quality_gate.max_distance_cm = 40.0;
    let s = run(&log, &strict);
    assert!(
        s.valid_ratio < a.valid_ratio,
        "{} vs {}",
        s.valid_ratio,
        a.valid_ratio
    );
}

#[test]
fn evaluate_at_frames_level_scores_the_reprocessed_metrics() {
    let log = research_log(halves());
    let profile = AnalysisProfile::default();
    let baselines: NormativeBaselineSet = serde_json::from_str(BASELINES).unwrap();
    let session = SessionInput {
        session_id: "s".into(),
        age_months: Some(30),
        research: Some(ResearchInput {
            log: log.clone(),
            catalog: catalog(),
            options: ReprocessOptions::default(),
        }),
        ..SessionInput::default()
    };
    let e = evaluate(&session, &profile, &baselines, LevelRequest::Auto);
    assert_eq!(e.level, Level::Frames);
    let r = run(&log, &profile);
    assert_eq!(e.metrics, r.metric_sets);
    assert_eq!(e.valid_ratio, Some(r.valid_ratio));
    assert_eq!(e.tracking.as_ref(), Some(&r.tracking));
    assert!(e.score.is_some() && e.flag.is_some());

    // Score explicitly: the (empty) reports.
    let e2 = evaluate(&session, &profile, &baselines, LevelRequest::Score);
    assert_eq!(e2.level, Level::Score);
    assert!(e2.metrics.is_empty());

    // An unreadable log: unscorable, level frames.
    let mut bad = session.clone();
    bad.research.as_mut().unwrap().log.header.targets[0].end_sec = -1.0;
    let e3 = evaluate(&bad, &profile, &baselines, LevelRequest::Frames);
    assert_eq!(e3.unscorable, Some(Unscorable::ReprocessFailed));

    // Deterministic bytes.
    let again = evaluate(&session, &profile, &baselines, LevelRequest::Auto);
    assert_eq!(
        serde_json::to_string(&e).unwrap(),
        serde_json::to_string(&again).unwrap()
    );
}
