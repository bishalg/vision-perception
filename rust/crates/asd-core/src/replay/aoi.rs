//! AOI hits per frame and the AOI-hit ribbon (ADR-0015 V-1).

use serde::{Deserialize, Serialize};

use crate::analysis::AnalysisProfile;
use crate::gaze::aoi_space::{clip_aois_with_margin, AoiSpace, ScreenAoi, VideoGravity};
use crate::metrics::{is_valid_frame, media_time, AoiRole, AoiRoleKind};
use crate::types::{Area, GazeFrame};

/// Hit-test and frame-span parameters, from the profile (`aoi.marginNorm`, `aoi.dwellCapMs`,
/// `metrics.fallbackFrameIntervalSec`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HitParams {
    /// Grows (positive) or shrinks (negative) every AOI box, screen-normalised.
    pub margin_norm: f64,
    /// Longest interval one frame covers, seconds (`aoi.dwellCapMs / 1000`).
    pub frame_cap_sec: f64,
    /// Span of a frame with no usable next timestamp, seconds (capped by `frame_cap_sec`).
    pub fallback_frame_interval_sec: f64,
}

impl Default for HitParams {
    fn default() -> Self {
        Self::from(&AnalysisProfile::default())
    }
}

impl From<&AnalysisProfile> for HitParams {
    fn from(p: &AnalysisProfile) -> Self {
        Self {
            margin_norm: p.aoi.margin_norm,
            frame_cap_sec: f64::from(p.aoi.dwell_cap_ms) / 1000.0,
            fallback_frame_interval_sec: p.metrics.fallback_frame_interval_sec,
        }
    }
}

/// The AOIs of one clip, ready to hit-test.
pub(crate) struct ClipAois {
    aois: Vec<ScreenAoi>,
    builtin: AoiRole,
}

/// The AOIs one frame is in: indices into [`ClipAois`], or recorded names when the clip has
/// no areas.
pub(crate) enum Hits<'a> {
    Areas(Vec<usize>),
    Recorded(&'a [String]),
}

impl ClipAois {
    pub(crate) fn new(areas: &[Area], params: &HitParams) -> Self {
        Self {
            aois: clip_aois_with_margin(
                areas,
                AoiSpace::Screen,
                1.0,
                1.0,
                VideoGravity::AspectFill,
                params.margin_norm,
            ),
            builtin: AoiRole::builtin(),
        }
    }

    /// The AOIs containing `frame`'s gaze at its media time (callers check validity).
    pub(crate) fn hits<'a>(&self, frame: &'a GazeFrame) -> Hits<'a> {
        if self.aois.is_empty() {
            return Hits::Recorded(&frame.active_aoi_names);
        }
        let p = [frame.gaze.x_norm, frame.gaze.y_norm];
        let t = media_time(frame);
        Hits::Areas(
            (0..self.aois.len())
                .filter(|&i| self.aois[i].contains(p, t))
                .collect(),
        )
    }

    /// Names of the hit AOIs, in AOI order.
    pub(crate) fn names(&self, hits: &Hits<'_>) -> Vec<String> {
        match hits {
            Hits::Areas(ix) => ix.iter().map(|&i| self.aois[i].name.clone()).collect(),
            Hits::Recorded(names) => names.to_vec(),
        }
    }

    /// Whether any hit AOI is named `name`.
    pub(crate) fn hits_name(&self, hits: &Hits<'_>, name: &str) -> bool {
        match hits {
            Hits::Areas(ix) => ix.iter().any(|&i| self.aois[i].name == name),
            Hits::Recorded(names) => names.iter().any(|n| n == name),
        }
    }

    /// Whether any hit AOI carries `role` (catalog roles; built-in name sets without areas).
    pub(crate) fn hits_role(&self, hits: &Hits<'_>, role: &str) -> bool {
        match hits {
            Hits::Areas(ix) => ix
                .iter()
                .any(|&i| self.aois[i].roles.iter().any(|r| r == role)),
            Hits::Recorded(names) => role_kind(role)
                .is_some_and(|kind| names.iter().any(|n| self.builtin.matches(n, kind))),
        }
    }

    /// Whether the frame is in any AOI.
    pub(crate) fn any(hits: &Hits<'_>) -> bool {
        match hits {
            Hits::Areas(ix) => !ix.is_empty(),
            Hits::Recorded(names) => !names.is_empty(),
        }
    }

    /// The innermost hit (smallest box; the first in AOI order at a tie) and its roles.
    fn primary(&self, hits: &Hits<'_>) -> Option<(String, Vec<String>)> {
        match hits {
            Hits::Areas(ix) => ix
                .iter()
                .copied()
                .min_by(|&a, &b| box_area(&self.aois[a]).total_cmp(&box_area(&self.aois[b])))
                .map(|i| (self.aois[i].name.clone(), self.aois[i].roles.clone())),
            Hits::Recorded(names) => names.first().map(|n| {
                let roles = ALL_ROLES
                    .iter()
                    .filter(|k| self.builtin.matches(n, **k))
                    .map(|k| k.as_str().to_owned())
                    .collect();
                (n.clone(), roles)
            }),
        }
    }
}

const ALL_ROLES: [AoiRoleKind; 5] = [
    AoiRoleKind::Social,
    AoiRoleKind::Geometric,
    AoiRoleKind::Eye,
    AoiRoleKind::Mouth,
    AoiRoleKind::Orienting,
];

fn role_kind(role: &str) -> Option<AoiRoleKind> {
    ALL_ROLES.into_iter().find(|k| k.as_str() == role)
}

fn box_area(a: &ScreenAoi) -> f64 {
    (a.rect.x_max - a.rect.x_min) * (a.rect.y_max - a.rect.y_min)
}

/// `(start, end)` media time each frame covers (see the module docs: frame span).
pub(crate) fn frame_spans(frames: &[GazeFrame], params: &HitParams) -> Vec<(f64, f64)> {
    let fallback = params.fallback_frame_interval_sec.min(params.frame_cap_sec);
    (0..frames.len())
        .map(|i| {
            let t = media_time(&frames[i]);
            let end = match frames.get(i + 1).map(media_time) {
                Some(next) if next > t && next - t <= params.frame_cap_sec => next,
                _ => t + fallback,
            };
            (t, end)
        })
        .collect()
}

/// One stretch of the AOI-hit ribbon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RibbonSegment {
    /// Media time, seconds.
    pub start_sec: f64,
    /// Media time, seconds (exclusive).
    pub end_sec: f64,
    /// `false`: invalid samples (blink, loss, rejected); `aoi` is then `null`.
    pub valid: bool,
    /// The innermost AOI holding the gaze, `null` off every AOI or when invalid.
    pub aoi: Option<String>,
    /// That AOI's roles (catalog spelling), for the role colour.
    pub roles: Vec<String>,
    /// Every AOI holding the gaze, in AOI order (overlapping boxes).
    pub hits: Vec<String>,
}

/// The AOI-hit ribbon of one clip's frames: one segment per run of frames with the same
/// validity and the same hits whose spans touch; time no frame covers is left out (a gap).
pub fn aoi_ribbon(frames: &[GazeFrame], areas: &[Area], params: &HitParams) -> Vec<RibbonSegment> {
    let aois = ClipAois::new(areas, params);
    let spans = frame_spans(frames, params);
    let mut out: Vec<RibbonSegment> = Vec::new();
    for (frame, &(start, end)) in frames.iter().zip(&spans) {
        if !(start.is_finite() && end.is_finite()) {
            continue;
        }
        let valid = is_valid_frame(frame);
        let (aoi, roles, hits) = if valid {
            let h = aois.hits(frame);
            let (aoi, roles) = aois
                .primary(&h)
                .map_or((None, Vec::new()), |(a, r)| (Some(a), r));
            (aoi, roles, aois.names(&h))
        } else {
            (None, Vec::new(), Vec::new())
        };
        if let Some(last) = out.last_mut() {
            if last.end_sec == start && last.valid == valid && last.hits == hits && last.aoi == aoi
            {
                last.end_sec = end;
                continue;
            }
        }
        out.push(RibbonSegment {
            start_sec: start,
            end_sec: end,
            valid,
            aoi,
            roles,
            hits,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{area, frame, invalid, t};
    use super::*;

    fn areas() -> Vec<Area> {
        vec![
            // A face over the whole clip and its eyes inside it, and a target from 1 s.
            area("Face", (0.0, 10.0), [0.0, 0.0, 0.5, 0.5], &["social"]),
            area(
                "Eyes",
                (0.0, 10.0),
                [0.1, 0.1, 0.4, 0.2],
                &["social", "eye"],
            ),
            area("Target", (1.0, 10.0), [0.6, 0.6, 1.0, 1.0], &[]),
        ]
    }

    #[test]
    fn ribbon_segments_are_known() {
        let mut f = Vec::new();
        for i in 0..30 {
            f.push(frame(t(i), 0.3, 0.3)); // face only
        }
        for i in 30..48 {
            f.push(frame(t(i), 0.2, 0.15)); // eyes inside face
        }
        for i in 48..54 {
            f.push(invalid(t(i)));
        }
        for i in 54..66 {
            f.push(frame(t(i), 0.8, 0.8)); // target, not yet shown (< 1 s)
        }
        for i in 66..72 {
            f.push(frame(t(i), 0.8, 0.8)); // target from 1.1 s
        }
        let r = aoi_ribbon(&f, &areas(), &HitParams::default());
        let summary: Vec<(f64, f64, bool, Option<&str>, usize)> = r
            .iter()
            .map(|s| {
                (
                    s.start_sec,
                    s.end_sec,
                    s.valid,
                    s.aoi.as_deref(),
                    s.hits.len(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (t(0), t(30), true, Some("Face"), 1),
                (t(30), t(48), true, Some("Eyes"), 2),
                (t(48), t(54), false, None, 0),
                (t(54), t(60), true, None, 0),
                (t(60), t(71) + 1.0 / 60.0, true, Some("Target"), 1),
            ]
        );
        assert_eq!(r[1].roles, vec!["social", "eye"]);
        assert_eq!(r[1].hits, vec!["Face", "Eyes"]);
    }

    #[test]
    fn long_gaps_stay_uncovered_and_margin_applies() {
        // 200 ms between two frames exceeds the 100 ms dwell cap.
        let f = vec![frame(0.0, 0.55, 0.3), frame(0.2, 0.55, 0.3)];
        let r = aoi_ribbon(&f, &areas(), &HitParams::default());
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].end_sec, 1.0 / 60.0);
        assert_eq!(r[0].aoi, None);
        let wide = HitParams {
            margin_norm: 0.1,
            ..HitParams::default()
        };
        let r = aoi_ribbon(&f, &areas(), &wide);
        assert_eq!(r[0].aoi.as_deref(), Some("Face"));
    }

    #[test]
    fn without_areas_the_recorded_names_are_used() {
        let mut a = frame(0.0, 0.5, 0.5);
        a.active_aoi_names = vec!["Eye_Region".into()];
        let r = aoi_ribbon(&[a], &[], &HitParams::default());
        assert_eq!(r[0].aoi.as_deref(), Some("Eye_Region"));
        assert_eq!(r[0].roles, vec!["social", "eye", "orienting"]);
    }
}
