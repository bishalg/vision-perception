//! owner: WP-G.3
//!
//! QS-3 / CNF-3 per-clip values (`asd_core::gaze::clip`) as pure functions: mean sample
//! confidence over tracked frames, mean fixation confidence within the clip (gap filled,
//! I-VT in degrees per second), the smallest role-bearing AOI dimension in degrees, and the
//! wire shape of `ClipTracking` / `ClipTrackingEntry`. Edge cases: an empty clip, a clip with
//! every frame lost, short (filled) and long (unfilled) gaps.

use asd_core::gaze::aoi_space::{NormBox, ScreenAoi};
use asd_core::gaze::clip::{
    aoi_min_size_deg, is_role_bearing, mean_fixation_confidence, mean_sample_confidence,
};
use asd_core::gaze::gapfill::GapSample;
use asd_core::gaze::geometry::{idiom_default, OrientedGeometry};
use asd_core::metrics::MIN_FIXATION_DURATION_SEC;
use asd_core::types::tracking::{
    ClipTracking, ClipTrackingEntry, DeviceIdiom, GapFillParams, Orientation,
};
use serde_json::json;

const HZ: f64 = 60.0;
const IVT: f64 = 30.0;
/// Eye 400 mm in front of the centre of the 100 x 200 mm panel.
const EYE: [f64; 3] = [50.0, 100.0, 400.0];

fn geo(orientation: Orientation) -> OrientedGeometry {
    let mut g = idiom_default(DeviceIdiom::Phone);
    g.screen_width_mm = 100.0;
    g.screen_height_mm = 200.0;
    g.rotate(orientation)
}

fn tracked(i: usize, p: [f64; 2], confidence: f64) -> GapSample {
    GapSample {
        t_sec: i as f64 / HZ,
        tracked: true,
        point_norm: Some(p),
        eye_pos_mm: Some(EYE),
        confidence,
        gap_filled: false,
    }
}

fn lost(i: usize) -> GapSample {
    GapSample {
        t_sec: i as f64 / HZ,
        tracked: false,
        point_norm: None,
        eye_pos_mm: None,
        confidence: 0.0,
        gap_filled: false,
    }
}

fn fix_conf(frames: &[GapSample]) -> Option<f64> {
    mean_fixation_confidence(
        frames,
        &geo(Orientation::Portrait),
        &GapFillParams::default(),
        IVT,
        MIN_FIXATION_DURATION_SEC,
    )
}

fn close(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() <= 1e-12, "{what}: {a} != {b}");
}

// ------------------------------------------------------------------------------------------
// meanSampleConfidence
// ------------------------------------------------------------------------------------------

#[test]
fn empty_clip_has_no_values() {
    assert_eq!(mean_sample_confidence(&[]), None);
    assert_eq!(fix_conf(&[]), None);
}

#[test]
fn all_lost_clip_has_zero_sample_confidence_and_no_fixation() {
    let frames: Vec<GapSample> = (0..60).map(lost).collect();
    assert_eq!(mean_sample_confidence(&frames), Some(0.0));
    assert_eq!(fix_conf(&frames), None);
}

#[test]
fn sample_confidence_is_over_tracked_frames_only() {
    let mut frames: Vec<GapSample> = (0..10).map(|i| tracked(i, [0.5, 0.5], 0.8)).collect();
    frames.extend((10..20).map(|i| tracked(i, [0.5, 0.5], 0.6)));
    // Loss is accounted by state, not as zero confidence (QS-1 `meanSample` rule).
    frames.extend((20..40).map(lost));
    close(mean_sample_confidence(&frames).unwrap(), 0.7, "mean");
}

// ------------------------------------------------------------------------------------------
// meanFixationConfidence
// ------------------------------------------------------------------------------------------

#[test]
fn fixation_confidence_is_the_mean_of_fixation_means() {
    // 30 frames at 0.6 on one point, a jump, 12 frames at 1.0 on another point.
    let mut frames: Vec<GapSample> = (0..30).map(|i| tracked(i, [0.3, 0.3], 0.6)).collect();
    frames.extend((30..42).map(|i| tracked(i, [0.7, 0.7], 1.0)));
    // Two fixations (500 ms and 183 ms); per fixation, not per sample: (0.6 + 1.0) / 2.
    close(fix_conf(&frames).unwrap(), 0.8, "two fixations");
    // Sample-weighted would differ.
    let weighted = mean_sample_confidence(&frames).unwrap();
    assert!((weighted - 0.8).abs() > 0.05, "{weighted}");
}

#[test]
fn a_short_gap_is_filled_and_its_frames_carry_no_confidence() {
    // 20 tracked, a 3-frame loss (bracket 66.7 ms <= 75 ms, same point: filled), 20 tracked.
    let mut frames: Vec<GapSample> = (0..20).map(|i| tracked(i, [0.5, 0.5], 0.9)).collect();
    frames.extend((20..23).map(lost));
    frames.extend((23..43).map(|i| tracked(i, [0.5, 0.5], 0.7)));
    // One fixation over the filled gap; the filled frames (confidence 0) are not averaged.
    let mean = (20.0 * 0.9 + 20.0 * 0.7) / 40.0;
    close(
        fix_conf(&frames).unwrap(),
        mean,
        "one fixation across the gap",
    );
}

#[test]
fn a_long_gap_ends_the_fixation() {
    // 20 tracked at 0.9, a 12-frame loss (216 ms: never filled, GAP-3), 10 tracked at 0.5.
    let mut frames: Vec<GapSample> = (0..20).map(|i| tracked(i, [0.5, 0.5], 0.9)).collect();
    frames.extend((20..32).map(lost));
    frames.extend((32..42).map(|i| tracked(i, [0.5, 0.5], 0.5)));
    // Two fixations (317 ms, 150 ms): (0.9 + 0.5) / 2; one fixation would be sample-weighted.
    close(fix_conf(&frames).unwrap(), 0.7, "split by the long gap");
}

#[test]
fn a_drifting_gaze_within_the_ivt_threshold_stays_one_fixation() {
    // Slow drift: 0.0005 per frame of 100 mm = 0.05 mm/frame = 3 mm/s ≈ 0.43°/s at 40 cm.
    let frames: Vec<GapSample> = (0..60)
        .map(|i| tracked(i, [0.4 + 0.0005 * i as f64, 0.5], 0.5 + i as f64 / 200.0))
        .collect();
    let mean = frames.iter().map(|f| f.confidence).sum::<f64>() / 60.0;
    close(fix_conf(&frames).unwrap(), mean, "one drifting fixation");
}

#[test]
fn a_clip_too_short_for_a_fixation_has_none() {
    // 4 frames = 50 ms < 80 ms.
    let frames: Vec<GapSample> = (0..4).map(|i| tracked(i, [0.5, 0.5], 1.0)).collect();
    assert_eq!(fix_conf(&frames), None);
    close(mean_sample_confidence(&frames).unwrap(), 1.0, "sample mean");
}

// ------------------------------------------------------------------------------------------
// aoiMinSizeDeg
// ------------------------------------------------------------------------------------------

fn aoi(name: &str, roles: &[&str], x: [f64; 2], y: [f64; 2]) -> ScreenAoi {
    ScreenAoi {
        name: name.into(),
        start_sec: 0.0,
        end_sec: 10.0,
        rect: NormBox {
            x_min: x[0],
            y_min: y[0],
            x_max: x[1],
            y_max: y[1],
        },
        roles: roles.iter().map(|r| r.to_string()).collect(),
    }
}

fn deg(size_mm: f64, distance_mm: f64) -> f64 {
    2.0 * (size_mm / 2.0 / distance_mm).atan().to_degrees()
}

#[test]
fn role_bearing_is_a_known_role_or_a_builtin_name() {
    assert!(is_role_bearing(&aoi(
        "x",
        &["social"],
        [0.0, 1.0],
        [0.0, 1.0]
    )));
    assert!(is_role_bearing(&aoi(
        "x",
        &["mouth"],
        [0.0, 1.0],
        [0.0, 1.0]
    )));
    assert!(is_role_bearing(&aoi(
        "x",
        &["orienting"],
        [0.0, 1.0],
        [0.0, 1.0]
    )));
    assert!(is_role_bearing(&aoi(
        "Eye_Region",
        &[],
        [0.0, 1.0],
        [0.0, 1.0]
    )));
    assert!(!is_role_bearing(&aoi(
        "Background",
        &[],
        [0.0, 1.0],
        [0.0, 1.0]
    )));
    assert!(!is_role_bearing(&aoi(
        "toy",
        &["nonSocial"],
        [0.0, 1.0],
        [0.0, 1.0]
    )));
}

#[test]
fn smallest_dimension_of_role_bearing_aois_in_degrees() {
    let g = geo(Orientation::Portrait);
    let aois = vec![
        // 40 x 60 mm.
        aoi("face", &["social"], [0.3, 0.7], [0.2, 0.5]),
        // 10 x 20 mm.
        aoi("eyes", &["social", "eye"], [0.45, 0.55], [0.25, 0.35]),
        // 2 x 2 mm, but not role-bearing: ignored.
        aoi("speck", &[], [0.5, 0.52], [0.5, 0.51]),
    ];
    close(
        aoi_min_size_deg(&aois, &g, Some(40.0)).unwrap(),
        deg(10.0, 400.0),
        "eyes width at 40 cm",
    );
    // Farther away, smaller angle.
    assert!(
        aoi_min_size_deg(&aois, &g, Some(60.0)).unwrap()
            < aoi_min_size_deg(&aois, &g, Some(40.0)).unwrap()
    );
}

#[test]
fn landscape_uses_the_oriented_screen_size() {
    // 0.1 x 0.1 of the screen: 10 x 20 mm portrait, 20 x 10 mm landscape; min 10 mm both.
    let aois = vec![aoi("face", &["social"], [0.45, 0.55], [0.45, 0.55])];
    let p = aoi_min_size_deg(&aois, &geo(Orientation::Portrait), Some(40.0)).unwrap();
    let l = aoi_min_size_deg(&aois, &geo(Orientation::LandscapeLeft), Some(40.0)).unwrap();
    close(p, deg(10.0, 400.0), "portrait");
    close(l, deg(10.0, 400.0), "landscape");
    // 0.1 wide x 0.3 high: 10 mm wide portrait, 20 x 30 mm landscape.
    let aois = vec![aoi("face", &["social"], [0.45, 0.55], [0.3, 0.6])];
    let l = aoi_min_size_deg(&aois, &geo(Orientation::LandscapeRight), Some(40.0)).unwrap();
    close(l, deg(20.0, 400.0), "landscape width");
}

#[test]
fn aoi_size_is_absent_without_distance_or_role_bearing_aoi() {
    let g = geo(Orientation::Portrait);
    let role = vec![aoi("face", &["social"], [0.3, 0.7], [0.2, 0.5])];
    assert_eq!(aoi_min_size_deg(&role, &g, None), None);
    assert_eq!(aoi_min_size_deg(&role, &g, Some(0.0)), None);
    assert_eq!(aoi_min_size_deg(&role, &g, Some(f64::NAN)), None);
    assert_eq!(aoi_min_size_deg(&[], &g, Some(40.0)), None);
    let none = vec![aoi("Background", &[], [0.0, 1.0], [0.0, 1.0])];
    assert_eq!(aoi_min_size_deg(&none, &g, Some(40.0)), None);
}

// ------------------------------------------------------------------------------------------
// Wire shape (1.7 `ClipTracking` keys)
// ------------------------------------------------------------------------------------------

#[test]
fn clip_tracking_serialises_the_1_7_keys_and_omits_unmeasured_values() {
    let full = ClipTrackingEntry {
        clip_id: "c1".into(),
        order: 1,
        tracking: ClipTracking {
            mean_sample_confidence: Some(0.9),
            mean_fixation_confidence: Some(0.8),
            nearest_check_offset_deg: Some(1.1),
            aoi_min_size_deg: Some(4.6),
        },
    };
    assert_eq!(
        serde_json::to_value(&full).unwrap(),
        json!({
            "clipId": "c1", "order": 1,
            "meanSampleConfidence": 0.9, "meanFixationConfidence": 0.8,
            "nearestCheckOffsetDeg": 1.1, "aoiMinSizeDeg": 4.6
        })
    );
    let back: ClipTrackingEntry =
        serde_json::from_value(serde_json::to_value(&full).unwrap()).unwrap();
    assert_eq!(back, full);
    assert_eq!(
        serde_json::to_value(ClipTracking::default()).unwrap(),
        json!({})
    );
    // ScreenAoi without roles keeps its old JSON (no `roles` key) and reads it back empty.
    let a = aoi("face", &[], [0.0, 1.0], [0.0, 1.0]);
    let v = serde_json::to_value(&a).unwrap();
    assert!(v.get("roles").is_none());
    assert_eq!(serde_json::from_value::<ScreenAoi>(v).unwrap(), a);
}
