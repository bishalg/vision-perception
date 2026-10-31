//! owner: WP-G.3
//!
//! AOI boxes from video-normalised to screen-normalised space (GEO-7, G-F16) and the AOI hit
//! test on screen-space boxes.
//!
//! Aspects are width / height. With `aspectFill` (`AVLayerVideoGravity.resizeAspectFill`, how
//! the stimulus videos are shown) the video is scaled to cover the screen and the overflow is
//! cropped equally on both sides; with `aspectFit` (`resizeAspect`) it is scaled to fit and
//! letterboxed. For a video wider than the screen the horizontal scale relative to the screen
//! is `k = videoAspect / screenAspect` (fill) or the vertical one `1 / k` (fit), and the other
//! way round for a narrower video; the box maps by `s = 0.5 + (v - 0.5)·k` on the scaled axis.
//! Parts outside the visible area are dropped (the box is clipped to [0, 1]); a box with
//! nothing left is dropped entirely.
//!
//! Trap 11: the catalog does not yet say which space its boxes are in. Until it declares
//! `aoiSpace`, the default here is [`AoiSpace::Screen`] (today's interpretation, boxes pass
//! through unchanged) so scores do not move silently; switching to `video` needs a catalog
//! version bump and new golden files.

use serde::{Deserialize, Serialize};

use crate::types::catalog::Area;

/// How the video layer fills the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VideoGravity {
    /// Cover the screen, crop the overflow (`resizeAspectFill`).
    #[default]
    AspectFill,
    /// Fit inside the screen, letterbox (`resizeAspect`).
    AspectFit,
}

/// Which space a clip's AOI boxes are given in (catalog `aoiSpace`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AoiSpace {
    Video,
    /// Today's interpretation (trap 11).
    #[default]
    Screen,
}

/// A normalised box, origin top-left, y down.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormBox {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

impl NormBox {
    /// Inclusive containment (Swift `StimulusAOI.contains`).
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.x_min && p[0] <= self.x_max && p[1] >= self.y_min && p[1] <= self.y_max
    }
}

fn valid_aspect(a: f64) -> bool {
    a.is_finite() && a > 0.0
}

/// GEO-7: maps a video-normalised box to screen-normalised space. `None` for an empty or
/// inverted box, a non-positive or non-finite aspect, or a box that lies entirely in the
/// cropped area.
pub fn video_to_screen(
    aoi: NormBox,
    video_aspect: f64,
    screen_aspect: f64,
    gravity: VideoGravity,
) -> Option<NormBox> {
    if !valid_aspect(video_aspect) || !valid_aspect(screen_aspect) {
        return None;
    }
    let finite = [aoi.x_min, aoi.y_min, aoi.x_max, aoi.y_max]
        .iter()
        .all(|v| v.is_finite());
    if !finite || aoi.x_max <= aoi.x_min || aoi.y_max <= aoi.y_min {
        return None;
    }
    // Scale of the displayed video relative to the screen, per axis.
    let ratio = video_aspect / screen_aspect;
    let (kx, ky) = match gravity {
        VideoGravity::AspectFill if ratio >= 1.0 => (ratio, 1.0),
        VideoGravity::AspectFill => (1.0, 1.0 / ratio),
        VideoGravity::AspectFit if ratio >= 1.0 => (1.0, 1.0 / ratio),
        VideoGravity::AspectFit => (ratio, 1.0),
    };
    let map = |v: f64, k: f64| 0.5 + (v - 0.5) * k;
    let out = NormBox {
        x_min: map(aoi.x_min, kx).max(0.0),
        y_min: map(aoi.y_min, ky).max(0.0),
        x_max: map(aoi.x_max, kx).min(1.0),
        y_max: map(aoi.y_max, ky).min(1.0),
    };
    (out.x_max > out.x_min && out.y_max > out.y_min).then_some(out)
}

/// An AOI ready for hit testing: screen-normalised box and its clip-time window (seconds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenAoi {
    pub name: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub rect: NormBox,
}

impl ScreenAoi {
    /// Inclusive in space and time (Swift `StimulusAOI.contains(x:y:atTime:)`).
    pub fn contains(&self, p: [f64; 2], t_sec: f64) -> bool {
        t_sec >= self.start_sec && t_sec <= self.end_sec && self.rect.contains(p)
    }
}

/// The clip's catalog areas as screen-space AOIs. `Screen` passes boxes through; `Video` maps
/// them with [`video_to_screen`] and leaves out the ones cropped away.
pub fn clip_aois(
    areas: &[Area],
    space: AoiSpace,
    video_aspect: f64,
    screen_aspect: f64,
    gravity: VideoGravity,
) -> Vec<ScreenAoi> {
    areas
        .iter()
        .filter_map(|a| {
            let b = NormBox {
                x_min: a.x_min,
                y_min: a.y_min,
                x_max: a.x_max,
                y_max: a.y_max,
            };
            let rect = match space {
                AoiSpace::Screen => b,
                AoiSpace::Video => video_to_screen(b, video_aspect, screen_aspect, gravity)?,
            };
            Some(ScreenAoi {
                name: a.name.clone(),
                start_sec: a.start_sec,
                end_sec: a.end_sec,
                rect,
            })
        })
        .collect()
}

/// Names of the AOIs containing `p` at clip time `t_sec`, in AOI order.
pub fn active_aoi_names(aois: &[ScreenAoi], p: [f64; 2], t_sec: f64) -> Vec<String> {
    aois.iter()
        .filter(|a| a.contains(p, t_sec))
        .map(|a| a.name.clone())
        .collect()
}
