//! owner: WP-G.3
//!
//! GEO-7: AOI boxes from video-normalised to screen-normalised space for `aspectFill` and
//! `aspectFit`, dropping what falls outside the visible area (G-F16). Expected values are
//! computed by hand below from the video and screen sizes.
//!
//! 1080 x 1920 video (aspect 0.5625):
//! - iPhone 393 x 852 pt, aspect-fill: the video is wider than the screen, so it fills the
//!   height and overflows horizontally by k = (1080 / 1920) / (393 / 852)
//!   = 1080·852 / (1920·393) = 920160 / 754560 = 1.2194656488549618;
//!   `x_s = 0.5 + (x_v - 0.5)·k`, `y_s = y_v` (crop of the video per side = (1 - 1/k)/2 ≈ 9 %).
//! - iPad 820 x 1180 pt, aspect-fill: the video is narrower, so it fills the width and
//!   overflows vertically by k = (820 / 1180) / (1080 / 1920) = 1574400 / 1274400
//!   = 1.2354048964218456; `x_s = x_v`, `y_s = 0.5 + (y_v - 0.5)·k`.
//! - iPhone aspect-fit: letterboxed vertically, `y_s = 0.5 + (y_v - 0.5) / k`.

use asd_core::gaze::aoi_space::{
    active_aoi_names, clip_aois, video_to_screen, AoiSpace, NormBox, ScreenAoi, VideoGravity,
};
use asd_core::types::catalog::Area;

const VIDEO: f64 = 1080.0 / 1920.0;
const PHONE: f64 = 393.0 / 852.0;
const PAD: f64 = 820.0 / 1180.0;
const EPS: f64 = 1e-12;

fn b(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> NormBox {
    NormBox {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}

fn assert_box(got: Option<NormBox>, want: NormBox, what: &str) {
    let g = got.unwrap_or_else(|| panic!("{what}: dropped"));
    for (a, e) in [
        (g.x_min, want.x_min),
        (g.y_min, want.y_min),
        (g.x_max, want.x_max),
        (g.y_max, want.y_max),
    ] {
        assert!((a - e).abs() < EPS, "{what}: {g:?} != {want:?}");
    }
}

#[test]
fn phone_aspect_fill_crops_horizontally() {
    let fill = VideoGravity::AspectFill;
    // Eye_Region of the catalog: 0.35..0.65 x 0.30..0.45.
    assert_box(
        video_to_screen(b(0.35, 0.30, 0.65, 0.45), VIDEO, PHONE, fill),
        b(0.3170801526717557, 0.30, 0.6829198473282443, 0.45),
        "eye region",
    );
    // Left half: x_s(0) = 0.5 - 0.5k = -0.1097... → clipped to 0.
    assert_box(
        video_to_screen(b(0.0, 0.0, 0.5, 1.0), VIDEO, PHONE, fill),
        b(0.0, 0.0, 0.5, 1.0),
        "left half",
    );
    // The full frame covers the full screen.
    assert_box(
        video_to_screen(b(0.0, 0.0, 1.0, 1.0), VIDEO, PHONE, fill),
        b(0.0, 0.0, 1.0, 1.0),
        "full",
    );
    // A box entirely inside the cropped margin (x_s max = 0.5 - 0.45k < 0) is dropped.
    assert_eq!(
        video_to_screen(b(0.0, 0.2, 0.05, 0.4), VIDEO, PHONE, fill),
        None
    );
}

#[test]
fn pad_aspect_fill_crops_vertically() {
    let fill = VideoGravity::AspectFill;
    assert_box(
        video_to_screen(b(0.35, 0.30, 0.65, 0.45), VIDEO, PAD, fill),
        b(0.35, 0.2529190207156309, 0.65, 0.4382297551789077),
        "eye region",
    );
    // Mouth_Region 0.40..0.60 x 0.58..0.72.
    assert_box(
        video_to_screen(b(0.40, 0.58, 0.60, 0.72), VIDEO, PAD, fill),
        b(0.40, 0.5988323917137477, 0.60, 0.771789077212806),
        "mouth region",
    );
    assert_eq!(
        video_to_screen(b(0.1, 0.0, 0.9, 0.05), VIDEO, PAD, fill),
        None,
        "top margin is cropped"
    );
}

#[test]
fn phone_aspect_fit_letterboxes() {
    assert_box(
        video_to_screen(b(0.0, 0.0, 1.0, 1.0), VIDEO, PHONE, VideoGravity::AspectFit),
        b(0.0, 0.0899843505477308, 1.0, 0.9100156494522692),
        "full frame letterboxed",
    );
}

#[test]
fn equal_aspects_are_the_identity() {
    let a = b(0.1, 0.2, 0.3, 0.4);
    for g in [VideoGravity::AspectFill, VideoGravity::AspectFit] {
        assert_box(video_to_screen(a, VIDEO, VIDEO, g), a, "identity");
    }
}

#[test]
fn invalid_input_is_dropped() {
    let fill = VideoGravity::AspectFill;
    assert_eq!(
        video_to_screen(b(0.5, 0.5, 0.4, 0.6), VIDEO, PHONE, fill),
        None
    );
    assert_eq!(
        video_to_screen(b(0.1, 0.1, 0.2, 0.2), 0.0, PHONE, fill),
        None
    );
    assert_eq!(
        video_to_screen(b(0.1, 0.1, 0.2, 0.2), VIDEO, f64::NAN, fill),
        None
    );
}

fn area(name: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> Area {
    Area {
        name: name.into(),
        start_sec: 0.0,
        end_sec: 10.0,
        x_min: x0,
        y_min: y0,
        x_max: x1,
        y_max: y1,
        roles: vec![],
    }
}

/// Trap 11: until the catalog declares `aoiSpace`, boxes keep today's screen interpretation.
#[test]
fn screen_space_is_the_default_and_passes_boxes_through() {
    assert_eq!(AoiSpace::default(), AoiSpace::Screen);
    let areas = vec![area("Eye_Region", 0.35, 0.30, 0.65, 0.45)];
    let aois = clip_aois(
        &areas,
        AoiSpace::Screen,
        VIDEO,
        PHONE,
        VideoGravity::AspectFill,
    );
    assert_eq!(aois.len(), 1);
    assert_eq!(aois[0].rect, b(0.35, 0.30, 0.65, 0.45));
    let mapped = clip_aois(
        &areas,
        AoiSpace::Video,
        VIDEO,
        PHONE,
        VideoGravity::AspectFill,
    );
    assert_box(
        Some(mapped[0].rect),
        b(0.3170801526717557, 0.30, 0.6829198473282443, 0.45),
        "video space",
    );
    // A box cropped away entirely is left out.
    let gone = clip_aois(
        &[area("Edge", 0.0, 0.2, 0.05, 0.4)],
        AoiSpace::Video,
        VIDEO,
        PHONE,
        VideoGravity::AspectFill,
    );
    assert!(gone.is_empty());
}

/// Hit test as the Swift `StimulusAOI.contains`: inclusive bounds and time window.
#[test]
fn hit_test_is_inclusive_in_space_and_time() {
    let aois = vec![
        ScreenAoi {
            name: "Social_Actor".into(),
            start_sec: 0.0,
            end_sec: 30.0,
            rect: b(0.0, 0.0, 0.5, 1.0),
        },
        ScreenAoi {
            name: "Geometric_Pattern".into(),
            start_sec: 0.0,
            end_sec: 30.0,
            rect: b(0.5, 0.0, 1.0, 1.0),
        },
    ];
    assert_eq!(
        active_aoi_names(&aois, [0.5, 0.5], 30.0),
        vec!["Social_Actor".to_string(), "Geometric_Pattern".to_string()]
    );
    assert_eq!(
        active_aoi_names(&aois, [0.2, 0.5], 1.0),
        vec!["Social_Actor"]
    );
    assert!(active_aoi_names(&aois, [0.2, 0.5], 30.01).is_empty());
    // Points are never clamped: off-screen gaze hits nothing.
    assert!(active_aoi_names(&aois, [-0.01, 0.5], 1.0).is_empty());
}
