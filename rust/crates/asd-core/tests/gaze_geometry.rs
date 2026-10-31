//! owner: WP-G.1
//!
//! GEO-1, GEO-2, GEO-3 and the visual-angle definition of technical.md. Every expected value
//! is computed by hand in this file (comments show the arithmetic); nothing is taken from the
//! Swift `ScreenRaycaster`, whose behaviour is defective (G-F1, G-F3, G-F4).

use asd_core::gaze::geometry::{
    angle_between_norm_deg, angle_deg, idiom_default, lookup, rotate_point_mm, table,
};
use asd_core::types::tracking::{
    CameraOffsetSource, DeviceIdiom, GeometrySource, Orientation, ScreenSource,
};

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

// ---------------------------------------------------------------- table and lookup (GEO-1/2)

#[test]
fn gaze_geometry_table_loads_and_every_entry_cites_a_source() {
    let t = table();
    assert!(!t.devices.is_empty(), "compiled-in table must not be empty");
    for d in &t.devices {
        assert!(
            !d.reference.trim().is_empty(),
            "{} has no ref",
            d.display_name
        );
        assert!(
            !d.model_identifiers.is_empty(),
            "{} has no ids",
            d.display_name
        );
        assert!(d.screen_width_mm > 0.0 && d.screen_height_mm > d.screen_width_mm);
        // Spec-derived millimetres must equal published pixels / ppi (GEO-1).
        if d.source == ScreenSource::Spec {
            let px = d.native_pixels.expect("spec entry needs nativePixels");
            let ppi = d.ppi.expect("spec entry needs ppi");
            assert!(
                px[0] < px[1],
                "{}: nativePixels is portrait [w,h]",
                d.display_name
            );
            assert!(close(
                d.screen_width_mm,
                f64::from(px[0]) / ppi * 25.4,
                0.01
            ));
            assert!(close(
                d.screen_height_mm,
                f64::from(px[1]) / ppi * 25.4,
                0.01
            ));
        }
        // No bench yet (BEN-3): every camera offset is unmeasured and follows the legacy
        // idiom rule (horizontally centred, 8 mm above the active area).
        assert_eq!(d.camera_offset_mm.source, CameraOffsetSource::Unmeasured);
        assert!(
            close(d.camera_offset_mm.x, d.screen_width_mm / 2.0, 1e-3),
            "{}",
            d.display_name
        );
        assert_eq!(d.camera_offset_mm.y, -8.0);
    }
    for d in [&t.idiom_defaults.phone, &t.idiom_defaults.pad] {
        assert_eq!(d.source, ScreenSource::LegacyDefault);
        assert!(!d.reference.trim().is_empty());
    }
}

#[test]
fn gaze_geometry_identifiers_are_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for d in &table().devices {
        for id in &d.model_identifiers {
            assert!(seen.insert(id.clone()), "duplicate identifier {id}");
        }
    }
}

#[test]
fn gaze_geometry_lookup_known_identifier() {
    // iPhone 16: 1179 x 2556 px at 460 ppi (Apple compare page).
    let hit = lookup("iPhone17,3");
    assert_eq!(hit.source, GeometrySource::Table);
    assert_eq!(hit.model_identifier, "iPhone17,3");
    assert_eq!(hit.geometry.native_pixels, Some([1179, 2556]));
    // 1179 / 460 * 25.4 = 65.10130 mm; 2556 / 460 * 25.4 = 141.13565 mm.
    assert!(close(hit.geometry.screen_width_mm, 65.1013, 1e-3));
    assert!(close(hit.geometry.screen_height_mm, 141.1357, 1e-3));
}

#[test]
fn gaze_geometry_lookup_unknown_identifier_falls_back_to_idiom_default() {
    let phone = lookup("iPhone99,9");
    assert_eq!(phone.source, GeometrySource::IdiomDefault);
    assert_eq!(phone.model_identifier, "iPhone99,9");
    // Legacy Swift constants (G-F4): 0.071 x 0.154 m, camera centred 8 mm above the glass.
    assert!(close(phone.geometry.screen_width_mm, 71.0, EPS));
    assert!(close(phone.geometry.screen_height_mm, 154.0, EPS));
    assert!(close(phone.geometry.camera_offset_mm.x, 35.5, EPS));
    assert!(close(phone.geometry.camera_offset_mm.y, -8.0, EPS));

    let pad = lookup("iPad99,1");
    assert_eq!(pad.source, GeometrySource::IdiomDefault);
    assert!(close(pad.geometry.screen_width_mm, 178.0, EPS));
    assert!(close(pad.geometry.screen_height_mm, 247.0, EPS));
    assert!(close(pad.geometry.camera_offset_mm.x, 89.0, EPS));

    // Neither prefix (simulator, empty string): phone default, still idiomDefault.
    for id in ["arm64", "", "x86_64"] {
        let g = lookup(id);
        assert_eq!(g.source, GeometrySource::IdiomDefault);
        assert_eq!(g.geometry.idiom, DeviceIdiom::Phone);
    }
    assert_eq!(idiom_default(DeviceIdiom::Pad), pad.geometry);
}

// ---------------------------------------------------------------- orientation (GEO-3)

/// A portrait-native test geometry: 100 x 200 mm, camera at (50, -8).
fn test_geo() -> asd_core::types::tracking::DeviceGeometry {
    let mut g = idiom_default(DeviceIdiom::Phone);
    g.screen_width_mm = 100.0;
    g.screen_height_mm = 200.0;
    g.camera_offset_mm.x = 50.0;
    g.camera_offset_mm.y = -8.0;
    g
}

#[test]
fn gaze_geometry_rotation_swaps_dimensions_and_rotates_camera() {
    let g = test_geo();

    let p = g.rotate(Orientation::Portrait);
    assert_eq!((p.width_mm, p.height_mm), (100.0, 200.0));
    assert_eq!(p.camera_mm, [50.0, -8.0]);

    // Upside down: x' = W - x, y' = H - y -> camera (50, 208), below the glass.
    let u = g.rotate(Orientation::PortraitUpsideDown);
    assert_eq!((u.width_mm, u.height_mm), (100.0, 200.0));
    assert_eq!(u.camera_mm, [50.0, 208.0]);

    // landscapeLeft (UIInterfaceOrientation: home side left, camera on the right):
    // x' = H - y, y' = x -> camera (208, 50), right of the glass.
    let l = g.rotate(Orientation::LandscapeLeft);
    assert_eq!((l.width_mm, l.height_mm), (200.0, 100.0));
    assert_eq!(l.camera_mm, [208.0, 50.0]);

    // landscapeRight (camera on the left): x' = y, y' = W - x -> camera (-8, 50).
    let r = g.rotate(Orientation::LandscapeRight);
    assert_eq!((r.width_mm, r.height_mm), (200.0, 100.0));
    assert_eq!(r.camera_mm, [-8.0, 50.0]);
}

#[test]
fn gaze_geometry_rotation_maps_native_corners_to_oriented_corners() {
    // Native top-left (0,0) of a 100 x 200 panel in every orientation.
    let cases = [
        (Orientation::Portrait, [0.0, 0.0]),
        (Orientation::PortraitUpsideDown, [100.0, 200.0]),
        (Orientation::LandscapeLeft, [200.0, 0.0]),
        (Orientation::LandscapeRight, [0.0, 100.0]),
    ];
    for (o, expected) in cases {
        assert_eq!(
            rotate_point_mm([0.0, 0.0], o, 100.0, 200.0),
            expected,
            "{o:?}"
        );
    }
    // Native centre stays the oriented centre.
    for o in Orientation::ALL {
        let g = test_geo().rotate(o);
        let c = rotate_point_mm([50.0, 100.0], o, 100.0, 200.0);
        assert_eq!(c, [g.width_mm / 2.0, g.height_mm / 2.0], "{o:?}");
    }
}

#[test]
fn gaze_geometry_norm_to_mm_and_back() {
    let l = test_geo().rotate(Orientation::LandscapeLeft); // 200 x 100
    assert_eq!(l.norm_to_mm([0.25, 0.5]), [50.0, 50.0]);
    assert_eq!(l.mm_to_norm([50.0, 50.0]), [0.25, 0.5]);
    // Unclamped both ways.
    assert_eq!(l.norm_to_mm([-0.2, 1.5]), [-40.0, 150.0]);
    assert_eq!(l.mm_to_norm([-40.0, 150.0]), [-0.2, 1.5]);
}

// ---------------------------------------------------------------- visual angle

#[test]
fn gaze_geometry_angle_zero_for_identical_points() {
    let e = [10.0, 20.0, 450.0];
    assert_eq!(angle_deg([3.0, 4.0, 0.0], [3.0, 4.0, 0.0], e), 0.0);
}

#[test]
fn gaze_geometry_angle_one_degree_at_57_3_cm() {
    // Exact: a 10 mm separation subtends 1 degree at D = 10 / tan(1 deg) = 572.899... mm.
    let d = 10.0 / 1.0_f64.to_radians().tan();
    let a = angle_deg([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, d]);
    assert!(close(a, 1.0, 1e-12), "{a}");
    // Rule of thumb: 1 cm at 57.3 cm is 1 degree to within 0.001 degree.
    let a = angle_deg([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, 573.0]);
    assert!(close(a, 1.0, 1e-3), "{a}");
}

#[test]
fn gaze_geometry_angle_ten_degrees_on_axis() {
    // Q at 500 * tan(10 deg) from the foot of the perpendicular.
    let q = 500.0 * 10.0_f64.to_radians().tan();
    let a = angle_deg([0.0, 0.0, 0.0], [0.0, q, 0.0], [0.0, 0.0, 500.0]);
    assert!(close(a, 10.0, 1e-12), "{a}");
}

#[test]
fn gaze_geometry_angle_off_axis_is_not_the_small_angle_difference() {
    // E = (0,0,500), P = (100,0,0), Q = (200,0,0):
    // theta = atan(200/500) - atan(100/500) = 21.80141 - 11.30993 = 10.49148 deg,
    // while the small-angle estimate 100/500 rad would give 11.459 deg.
    let a = angle_deg([100.0, 0.0, 0.0], [200.0, 0.0, 0.0], [0.0, 0.0, 500.0]);
    let expected =
        (200.0_f64 / 500.0).atan().to_degrees() - (100.0_f64 / 500.0).atan().to_degrees();
    assert!(close(a, expected, 1e-12), "{a} vs {expected}");
    assert!(close(a, 10.491_477, 1e-5));
}

#[test]
fn gaze_geometry_angle_matches_acos_form_in_two_dimensions() {
    // a = (100,0,-500), b = (0,100,-500): a.b = 250000, |a||b| = 260000,
    // theta = acos(250000 / 260000) = 15.9424 deg.
    let a = angle_deg([100.0, 0.0, 0.0], [0.0, 100.0, 0.0], [0.0, 0.0, 500.0]);
    let expected = (250_000.0_f64 / 260_000.0).acos().to_degrees();
    assert!(close(a, expected, 1e-9), "{a} vs {expected}");
    assert!(close(a, 15.942_4, 1e-4));
}

#[test]
fn gaze_geometry_angle_between_normalised_points() {
    // Portrait 100 x 200; (0.5,0.5) -> (50,100) mm, (0.6,0.5) -> (60,100) mm.
    // Eye straight in front of (50,100) at 572.899 mm -> exactly 1 degree.
    let g = test_geo().rotate(Orientation::Portrait);
    let d = 10.0 / 1.0_f64.to_radians().tan();
    let a = angle_between_norm_deg(&g, [0.5, 0.5], [0.6, 0.5], [50.0, 100.0, d]);
    assert!(close(a, 1.0, 1e-12), "{a}");
}

#[test]
fn gaze_geometry_angle_is_symmetric_and_non_negative() {
    let e = [33.0, -12.0, 410.0];
    let p = [-20.0, 70.0, 0.0];
    let q = [80.0, 140.0, 0.0];
    let a = angle_deg(p, q, e);
    assert!(a > 0.0);
    assert_eq!(a, angle_deg(q, p, e));
}
