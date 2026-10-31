//! owner: WP-G.1
//!
//! Device screen geometry (GEO-1..GEO-3) and visual angles (technical.md "Definitions").
//!
//! The table is compiled in from `resources/device_geometry.json`. Every entry cites its
//! sources in `ref`; millimetres are derived from Apple's published pixels and ppi
//! (`source = spec`) and camera offsets stay `unmeasured` until the bench (BEN-3).
//!
//! Orientation names follow `UIInterfaceOrientation`. Rotation maps a point of the portrait
//! native frame (W x H, origin top-left, y down) to the frame the user sees:
//!
//! | orientation          | x'      | y'      | size  |
//! | -------------------- | ------- | ------- | ----- |
//! | `portrait`           | x       | y       | W x H |
//! | `portraitUpsideDown` | W - x   | H - y   | W x H |
//! | `landscapeLeft`      | H - y   | x       | H x W |
//! | `landscapeRight`     | y       | W - x   | H x W |
//!
//! (`landscapeLeft`: the device's top edge, with the camera, is on the user's right.)
//! Trap 3 of the work-package plan applies: this table is the documented convention, and the
//! WP-G.8 bench corners check in every orientation is what confirms it on hardware.

use std::sync::OnceLock;

use crate::types::tracking::{
    DeviceGeometry, DeviceGeometryTable, DeviceIdiom, GeometrySource, Orientation,
};

const TABLE_JSON: &str = include_str!("../../resources/device_geometry.json");

/// Result of [`lookup`].
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryLookup {
    /// The identifier that was looked up (also for an unknown model).
    pub model_identifier: String,
    pub source: GeometrySource,
    pub geometry: DeviceGeometry,
}

/// A geometry rotated to an interface orientation: every value is in the oriented screen
/// frame (millimetres, origin at the top-left the user sees, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrientedGeometry {
    pub orientation: Orientation,
    pub width_mm: f64,
    pub height_mm: f64,
    /// Camera aperture centre in the oriented frame (may lie outside the glass).
    pub camera_mm: [f64; 2],
    /// Camera aperture centre in the portrait native frame.
    pub camera_native_mm: [f64; 2],
    /// Portrait native size, kept for native-to-oriented conversions.
    pub native_width_mm: f64,
    pub native_height_mm: f64,
}

/// The compiled-in table. The JSON is validated by the `gaze_geometry` tests; should it ever
/// fail to parse at runtime, the table degrades to the two legacy idiom defaults with no
/// model entries (every lookup then reports `idiomDefault`) instead of panicking.
pub fn table() -> &'static DeviceGeometryTable {
    static TABLE: OnceLock<DeviceGeometryTable> = OnceLock::new();
    TABLE.get_or_init(|| serde_json::from_str(TABLE_JSON).unwrap_or_else(|_| fallback_table()))
}

fn fallback_table() -> DeviceGeometryTable {
    use crate::types::tracking::{CameraOffsetMm, CameraOffsetSource, IdiomDefaults, ScreenSource};
    let legacy = |name: &str, idiom, w: f64, h: f64| DeviceGeometry {
        model_identifiers: Vec::new(),
        display_name: name.to_string(),
        idiom,
        native_pixels: None,
        ppi: None,
        screen_width_mm: w,
        screen_height_mm: h,
        camera_offset_mm: CameraOffsetMm {
            x: w / 2.0,
            y: -8.0,
            source: CameraOffsetSource::Unmeasured,
        },
        source: ScreenSource::LegacyDefault,
        reference: "Legacy Swift constants (technical.md G-F4); built-in fallback".to_string(),
        note: None,
    };
    DeviceGeometryTable {
        schema_version: "1.0.0".to_string(),
        idiom_defaults: IdiomDefaults {
            phone: legacy("iPhone (idiom default)", DeviceIdiom::Phone, 71.0, 154.0),
            pad: legacy("iPad (idiom default)", DeviceIdiom::Pad, 178.0, 247.0),
        },
        devices: Vec::new(),
    }
}

/// The legacy idiom geometry (GEO-2 fallback).
pub fn idiom_default(idiom: DeviceIdiom) -> DeviceGeometry {
    let d = &table().idiom_defaults;
    match idiom {
        DeviceIdiom::Phone => d.phone.clone(),
        DeviceIdiom::Pad => d.pad.clone(),
    }
}

/// Idiom implied by a model identifier: `iPad…` is a pad, anything else (including
/// simulator identifiers) a phone.
pub fn idiom_of(model_id: &str) -> DeviceIdiom {
    if model_id.starts_with("iPad") {
        DeviceIdiom::Pad
    } else {
        DeviceIdiom::Phone
    }
}

/// Looks a hardware model identifier up (GEO-2). Unknown models get the idiom default and
/// `source = idiomDefault` (which caps the session verdict at `fair`, QS-4).
pub fn lookup(model_id: &str) -> GeometryLookup {
    let found = table()
        .devices
        .iter()
        .find(|d| d.model_identifiers.iter().any(|id| id == model_id));
    match found {
        Some(g) => GeometryLookup {
            model_identifier: model_id.to_string(),
            source: GeometrySource::Table,
            geometry: g.clone(),
        },
        None => GeometryLookup {
            model_identifier: model_id.to_string(),
            source: GeometrySource::IdiomDefault,
            geometry: idiom_default(idiom_of(model_id)),
        },
    }
}

/// Maps a portrait-native point (mm) of a `w` x `h` panel to the oriented frame (table in the
/// module docs). Works for points off the glass too (camera, off-screen hits).
pub fn rotate_point_mm(p: [f64; 2], o: Orientation, w: f64, h: f64) -> [f64; 2] {
    let [x, y] = p;
    match o {
        Orientation::Portrait => [x, y],
        Orientation::PortraitUpsideDown => [w - x, h - y],
        Orientation::LandscapeLeft => [h - y, x],
        Orientation::LandscapeRight => [y, w - x],
    }
}

impl DeviceGeometry {
    /// Rotates screen size and camera offset to the interface orientation (GEO-3).
    pub fn rotate(&self, orientation: Orientation) -> OrientedGeometry {
        let (w, h) = (self.screen_width_mm, self.screen_height_mm);
        let (width_mm, height_mm) = match orientation {
            Orientation::Portrait | Orientation::PortraitUpsideDown => (w, h),
            Orientation::LandscapeLeft | Orientation::LandscapeRight => (h, w),
        };
        OrientedGeometry {
            orientation,
            width_mm,
            height_mm,
            camera_mm: rotate_point_mm(
                [self.camera_offset_mm.x, self.camera_offset_mm.y],
                orientation,
                w,
                h,
            ),
            camera_native_mm: [self.camera_offset_mm.x, self.camera_offset_mm.y],
            native_width_mm: w,
            native_height_mm: h,
        }
    }
}

impl OrientedGeometry {
    /// Screen-normalised point to oriented millimetres (unclamped).
    pub fn norm_to_mm(&self, p: [f64; 2]) -> [f64; 2] {
        [p[0] * self.width_mm, p[1] * self.height_mm]
    }

    /// Oriented millimetres to screen-normalised (unclamped).
    pub fn mm_to_norm(&self, p: [f64; 2]) -> [f64; 2] {
        [p[0] / self.width_mm, p[1] / self.height_mm]
    }

    /// Portrait-native millimetres to oriented millimetres.
    pub fn native_to_oriented_mm(&self, p: [f64; 2]) -> [f64; 2] {
        rotate_point_mm(
            p,
            self.orientation,
            self.native_width_mm,
            self.native_height_mm,
        )
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Visual angle in degrees between points `p` and `q` seen from `eye` (all in mm, one frame):
/// `atan2(|(P-E) x (Q-E)|, (P-E) . (Q-E))`. Exact, never the small-angle approximation;
/// well conditioned for tiny and large angles alike.
pub fn angle_deg(p: [f64; 3], q: [f64; 3], eye: [f64; 3]) -> f64 {
    let a = sub(p, eye);
    let b = sub(q, eye);
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let cross_norm = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    cross_norm.atan2(dot).to_degrees()
}

/// Visual angle between two screen-normalised points for an eye at `eye_mm` (oriented screen
/// frame, z = distance in front of the glass).
pub fn angle_between_norm_deg(
    geo: &OrientedGeometry,
    p_norm: [f64; 2],
    q_norm: [f64; 2],
    eye_mm: [f64; 3],
) -> f64 {
    let p = geo.norm_to_mm(p_norm);
    let q = geo.norm_to_mm(q_norm);
    angle_deg([p[0], p[1], 0.0], [q[0], q[1], 0.0], eye_mm)
}
