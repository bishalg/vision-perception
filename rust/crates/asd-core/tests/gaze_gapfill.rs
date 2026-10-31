//! owner: WP-G.3
//!
//! GAP-1..GAP-3: gaps of at most `gapFill.maxMs` and `gapFill.maxDeg` are filled by linear
//! interpolation, for fixation segmentation only; filled samples never count as valid, never
//! add dwell and are never hit-tested (trap 9). Loss is accounted by state with each frame
//! interval capped at 100 ms (PLC-5).

use asd_core::gaze::gapfill::{self, fill, fill_processed, fixations, GapSample, LossFrame};
use asd_core::gaze::geometry::{angle_between_norm_deg, idiom_default, OrientedGeometry};
use asd_core::metrics;
use asd_core::types::tracking::{
    DeviceIdiom, GapFillParams, Orientation, ProcessedSample, RayStatus, TrackState,
};
use asd_core::types::{GazeFrame, GazeFrameAffect, GazeFrameSample, GazeQualityVerdict};

/// A 100 x 200 mm panel (portrait).
fn geo() -> OrientedGeometry {
    let mut g = idiom_default(DeviceIdiom::Phone);
    g.screen_width_mm = 100.0;
    g.screen_height_mm = 200.0;
    g.camera_offset_mm.x = 50.0;
    g.camera_offset_mm.y = -8.0;
    g.rotate(Orientation::Portrait)
}

const EYE: [f64; 3] = [50.0, 100.0, 400.0];

/// x offset (screen-normalised) from the centre that subtends `deg` at `EYE`.
fn dx_for_deg(deg: f64) -> f64 {
    400.0 * deg.to_radians().tan() / 100.0
}

fn tracked(t: f64, p: [f64; 2]) -> GapSample {
    GapSample {
        t_sec: t,
        tracked: true,
        point_norm: Some(p),
        eye_pos_mm: Some(EYE),
        confidence: 0.9,
        gap_filled: false,
    }
}

fn missing(t: f64) -> GapSample {
    GapSample {
        t_sec: t,
        tracked: false,
        point_norm: None,
        eye_pos_mm: None,
        confidence: 0.0,
        gap_filled: false,
    }
}

/// Tracked at 0 and `gap_ms`, missing frames every 20 ms in between, then tracked again.
fn with_gap(gap_ms: f64, deg: f64) -> Vec<GapSample> {
    let a = [0.5, 0.5];
    let b = [0.5 + dx_for_deg(deg), 0.5];
    let gap = gap_ms / 1000.0;
    let mut v = vec![tracked(-0.02, a), tracked(0.0, a)];
    let mut t = 0.02;
    while t < gap - 1e-9 {
        v.push(missing(t));
        t += 0.02;
    }
    v.push(tracked(gap, b));
    v.push(tracked(gap + 0.02, b));
    v
}

#[test]
fn degree_helper_is_exact() {
    let d = angle_between_norm_deg(&geo(), [0.5, 0.5], [0.5 + dx_for_deg(0.5), 0.5], EYE);
    assert!((d - 0.5).abs() < 1e-12);
}

#[test]
fn gap_of_60_ms_and_half_a_degree_is_filled_linearly() {
    let input = with_gap(60.0, 0.5);
    let out = fill(&input, &geo(), &GapFillParams::default());
    assert_eq!(out.len(), input.len());
    let b = 0.5 + dx_for_deg(0.5);
    let filled: Vec<&GapSample> = out.iter().filter(|s| s.gap_filled).collect();
    assert_eq!(filled.len(), 2);
    for s in &filled {
        let frac = s.t_sec / 0.06;
        let want = 0.5 + frac * (b - 0.5);
        let p = s.point_norm.unwrap();
        assert!((p[0] - want).abs() < 1e-12 && (p[1] - 0.5).abs() < 1e-12);
        assert_eq!(s.eye_pos_mm, Some(EYE));
        // Still not tracked, still zero confidence: only the point is interpolated.
        assert!(!s.tracked);
        assert_eq!(s.confidence, 0.0);
    }
    // Bracketing samples untouched.
    for (a, b) in input.iter().zip(&out) {
        if !b.gap_filled {
            assert_eq!(a, b);
        }
    }
}

#[test]
fn gap_of_60_ms_and_two_degrees_is_not_filled() {
    let input = with_gap(60.0, 2.0);
    let out = fill(&input, &geo(), &GapFillParams::default());
    assert_eq!(out, input);
}

#[test]
fn gap_of_200_ms_is_not_filled() {
    let input = with_gap(200.0, 0.0);
    let out = fill(&input, &geo(), &GapFillParams::default());
    assert_eq!(out, input);
}

#[test]
fn limits_are_inclusive() {
    // Exactly at both limits (bracket interval 60 ms = maxMs, 1° = maxDeg) is filled.
    let mut input = with_gap(60.0, 1.0);
    let p = GapFillParams {
        max_ms: 60.0,
        max_deg: 1.0 + 1e-12,
        ..GapFillParams::default()
    };
    let out = fill(&input, &geo(), &p);
    assert!(out.iter().any(|s| s.gap_filled));
    // A gap without a tracked sample after it (end of the recording) is never filled.
    input.truncate(4);
    let out = fill(&input, &geo(), &GapFillParams::default());
    assert!(!out.iter().any(|s| s.gap_filled));
}

#[test]
fn leading_gap_is_not_filled() {
    let input = vec![missing(0.0), missing(0.02), tracked(0.04, [0.5, 0.5])];
    let out = fill(&input, &geo(), &GapFillParams::default());
    assert_eq!(out, input);
}

// ------------------------------------------------------------------------------------------
// GAP-2: filled samples change nothing but fixation segmentation
// ------------------------------------------------------------------------------------------

fn processed(t: f64, state: TrackState, p: Option<[f64; 2]>, aoi: &[&str]) -> ProcessedSample {
    let tracked = state == TrackState::Tracked;
    ProcessedSample {
        t_sec: t,
        track_state: state,
        ray_status: p.map(|_| RayStatus::Ok),
        raw_point_norm: p,
        calibrated_point_norm: p,
        filtered_point_norm: if tracked { p } else { None },
        eye_pos_mm: p.map(|_| EYE),
        distance_cm: p.map(|_| 40.0),
        head_yaw_rad: None,
        head_pitch_rad: None,
        head_roll_rad: None,
        head_speed_deg_per_sec: None,
        eye_speed_deg_per_sec: None,
        left_eye_openness: None,
        right_eye_openness: None,
        ambient_intensity: None,
        confidence: if tracked { 0.9 } else { 0.0 },
        valid: tracked,
        gap_filled: false,
        active_aoi_names: aoi.iter().map(|s| s.to_string()).collect(),
        stimulus_id: Some("clip".into()),
        clip_media_time_sec: Some(t),
    }
}

fn frame_of(p: &ProcessedSample) -> GazeFrame {
    let pt = p.filtered_point_norm.unwrap_or([0.0, 0.0]);
    GazeFrame {
        gaze: GazeFrameSample {
            is_valid: p.valid,
            x_norm: pt[0],
            y_norm: pt[1],
            video_time_sec: p.t_sec,
            clip_media_time_sec: p.clip_media_time_sec,
        },
        affect: GazeFrameAffect::default(),
        active_aoi_names: p.active_aoi_names.clone(),
        quality_verdict: if p.valid {
            GazeQualityVerdict::Optimal
        } else {
            GazeQualityVerdict::Rejected
        },
        stimulus_id: p.stimulus_id.clone(),
    }
}

fn session() -> Vec<ProcessedSample> {
    let mut v = Vec::new();
    let mut t = 0.0;
    for i in 0..120 {
        // Social AOI on the left; two 50 ms blinks and one 300 ms loss.
        let state = match i {
            30..=32 | 70..=72 => TrackState::Blink,
            90..=107 => TrackState::Lost,
            _ => TrackState::Tracked,
        };
        let p = if state == TrackState::Lost {
            None
        } else {
            Some([0.25, 0.5])
        };
        let aoi: &[&str] = if state == TrackState::Tracked {
            &["Social_Actor"]
        } else {
            &[]
        };
        v.push(processed(t, state, p, aoi));
        t += 1.0 / 60.0;
    }
    v
}

#[test]
fn filled_samples_do_not_change_valid_ratio_or_dwell() {
    let before = session();
    let after = fill_processed(&before, &geo(), &GapFillParams::default());
    assert_eq!(
        after.iter().filter(|s| s.gap_filled).count(),
        6,
        "two short blinks filled"
    );
    let ratio =
        |v: &[ProcessedSample]| v.iter().filter(|s| s.valid).count() as f64 / v.len() as f64;
    assert_eq!(ratio(&before), ratio(&after));
    for s in after.iter().filter(|s| s.gap_filled) {
        assert!(!s.valid, "filled samples never count as valid");
        assert!(
            s.active_aoi_names.is_empty(),
            "filled samples are never hit-tested"
        );
        assert_eq!(s.confidence, 0.0);
        assert!(s.filtered_point_norm.is_some());
    }
    let m_before = metrics::extract(&before.iter().map(frame_of).collect::<Vec<_>>());
    let m_after = metrics::extract(&after.iter().map(frame_of).collect::<Vec<_>>());
    assert_eq!(
        m_before, m_after,
        "dwell, latency and every metric unchanged"
    );
}

// ------------------------------------------------------------------------------------------
// Fixation segmentation (the only consumer of filled samples)
// ------------------------------------------------------------------------------------------

fn fixation_input(gap_ms: f64) -> Vec<GapSample> {
    // 280 ms of steady gaze, a gap of `gap_ms` (bracket interval), 280 ms at the same place.
    let mut v: Vec<GapSample> = (0..15)
        .map(|i| tracked(i as f64 * 0.02, [0.5, 0.5]))
        .collect();
    let last = 14.0 * 0.02;
    let resume = last + gap_ms / 1000.0;
    let mut t = last + 0.02;
    while t < resume - 1e-9 {
        v.push(missing(t));
        t += 0.02;
    }
    v.extend((0..15).map(|i| tracked(resume + i as f64 * 0.02, [0.5, 0.5])));
    v
}

#[test]
fn a_filled_gap_joins_a_fixation_and_a_long_blink_ends_it() {
    let g = geo();
    let p = GapFillParams::default();
    let short = fill(&fixation_input(60.0), &g, &p);
    let fx = fixations(&short, &g, 30.0, metrics::MIN_FIXATION_DURATION_SEC);
    assert_eq!(fx.len(), 1, "{fx:?}");
    // GAP-3: a 200 ms blink is never filled and ends the fixation.
    let long = fill(&fixation_input(200.0), &g, &p);
    let fx = fixations(&long, &g, 30.0, metrics::MIN_FIXATION_DURATION_SEC);
    assert_eq!(fx.len(), 2, "{fx:?}");
}

#[test]
fn fixation_confidence_excludes_filled_samples() {
    let g = geo();
    let filled = fill(&fixation_input(60.0), &g, &GapFillParams::default());
    let fx = fixations(&filled, &g, 30.0, metrics::MIN_FIXATION_DURATION_SEC);
    assert!((fx[0].mean_confidence.unwrap() - 0.9).abs() < 1e-12);
}

#[test]
fn a_saccade_splits_fixations_in_degrees_per_second() {
    let g = geo();
    let mut v = Vec::new();
    for i in 0..10 {
        v.push(tracked(i as f64 * 0.02, [0.5, 0.5]));
    }
    // 5° in 20 ms = 250°/s.
    for i in 10..20 {
        v.push(tracked(i as f64 * 0.02, [0.5 + dx_for_deg(5.0), 0.5]));
    }
    let fx = fixations(&v, &g, 30.0, 0.08);
    assert_eq!(fx.len(), 2);
    assert_eq!((fx[0].start, fx[0].end), (0, 10));
    assert_eq!((fx[1].start, fx[1].end), (10, 20));
    // A slow drift of 0.4° per 20 ms = 20°/s stays one fixation.
    let slow: Vec<GapSample> = (0..20)
        .map(|i| tracked(i as f64 * 0.02, [0.5 + dx_for_deg(0.4 * i as f64), 0.5]))
        .collect();
    assert_eq!(fixations(&slow, &g, 30.0, 0.08).len(), 1);
}

// ------------------------------------------------------------------------------------------
// GAP-3 loss accounting
// ------------------------------------------------------------------------------------------

fn lf(t: f64, state: TrackState, gap_filled: bool) -> LossFrame {
    LossFrame {
        t_sec: t,
        state,
        gap_filled,
    }
}

#[test]
fn loss_sums_by_state_with_capped_intervals_and_uncapped_longest_gap() {
    use TrackState::*;
    let frames = vec![
        lf(0.0, Tracked, false),
        lf(0.02, Blink, true),      // owns 20 ms
        lf(0.04, Tracked, false),   // owns 20 ms
        lf(0.06, Lost, false),      // owns 300 ms, capped at 100
        lf(0.36, Lost, false),      // owns 20 ms
        lf(0.38, Offscreen, false), // owns 20 ms
        lf(0.40, Artifact, false),  // owns 20 ms
        lf(0.42, Tracked, false),   // owns 20 ms
        lf(0.44, Tracked, false),   // last frame owns nothing
    ];
    let (loss, data_loss) = gapfill::loss(&frames, &GapFillParams::default());
    let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
    assert!(close(loss.blink_sec, 0.02), "{loss:?}");
    assert!(close(loss.track_loss_sec, 0.12), "{loss:?}");
    assert!(close(loss.offscreen_sec, 0.02), "{loss:?}");
    assert!(close(loss.artifact_sec, 0.02), "{loss:?}");
    assert!(close(loss.gap_filled_sec, 0.02), "{loss:?}");
    // Longest gap: last tracked before (0.04) to first tracked after (0.42), uncapped.
    assert!(close(loss.longest_gap_sec, 0.38), "{loss:?}");
    // Total time = 0.02*7 + 0.1 = 0.24; not tracked = 0.18.
    assert!(close(data_loss, 0.18 / 0.24), "{data_loss}");
}

#[test]
fn no_frames_is_total_loss() {
    let (loss, data_loss) = gapfill::loss(&[], &GapFillParams::default());
    assert_eq!(loss.longest_gap_sec, 0.0);
    assert_eq!(data_loss, 1.0);
}
