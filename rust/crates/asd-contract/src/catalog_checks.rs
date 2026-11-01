//! owner: WP-C.4
//!
//! Value rules on a [`StimulusCatalog`]. The type only checks the shape (it decodes); these
//! are the rules a catalog must also pass before it can be created, approved or published.
//! One implementation for every caller: the backend (`config_docs::lifecycle::validate_body`)
//! and the console editor through asd-wasm (`validateCatalog`).
//!
//! Each problem names the member it is about as a JSON path in the wire spelling
//! (`clips[2].areas[0].xMin`, `fallbackLanguage`); the backend prefixes `body.` for
//! `invalid_params[].name`, the console maps the path to a form field.
//!
//! Rules (spec 03 §4.3 and the catalog `comment` of the bundled file):
//! - `schemaVersion` is set; `languages` is non-empty, without blanks or duplicates;
//!   `fallbackLanguage` is one of them.
//! - Age packs: id set and unique; `0 <= minMonths <= maxMonths`; ranges do not overlap.
//! - Clips: not empty; `id`, `title`, `category` set; `id` unique; `durationSec > 0`;
//!   `0 <= minMonths <= maxMonths`.
//! - Batteries (clip `protocols`): `id` set, `order >= 0`, a clip is in a battery once, and
//!   `order` is unique within a battery across clips.
//! - AOIs (clip `areas`): `name` set and unique per clip; each coordinate in `[0, 1]` with
//!   `xMin < xMax` and `yMin < yMax`; `0 <= startSec < endSec <= durationSec`; every role is
//!   one of [`AOI_ROLES`] and listed once (no role is allowed: control targets).

use std::collections::{BTreeMap, BTreeSet};

use asd_core::types::StimulusCatalog;

pub use crate::profile_checks::ProfileProblem as CatalogProblem;

/// The known AOI roles (`asd_core::metrics::AoiRoleKind::as_str`).
pub const AOI_ROLES: [&str; 5] = ["social", "geometric", "eye", "mouth", "orienting"];

struct Out(Vec<CatalogProblem>);

impl Out {
    fn add(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.0.push(CatalogProblem {
            path: path.into(),
            message: message.into(),
        });
    }
}

/// Every problem of `c`, in a fixed order (empty when valid).
pub fn catalog_problems(c: &StimulusCatalog) -> Vec<CatalogProblem> {
    let mut o = Out(Vec::new());
    if c.schema_version.trim().is_empty() {
        o.add("schemaVersion", "schemaVersion is required");
    }
    languages(c, &mut o);
    age_packs(c, &mut o);
    if c.clips.is_empty() {
        o.add("clips", "clips must not be empty");
    }
    let mut clip_ids = BTreeSet::new();
    // battery id -> order -> first clip id holding it
    let mut orders: BTreeMap<&str, BTreeMap<i64, &str>> = BTreeMap::new();
    for (i, clip) in c.clips.iter().enumerate() {
        let p = format!("clips[{i}]");
        required(&mut o, &format!("{p}.id"), &clip.id, "id");
        required(&mut o, &format!("{p}.title"), &clip.title, "title");
        required(&mut o, &format!("{p}.category"), &clip.category, "category");
        if !clip.id.trim().is_empty() && !clip_ids.insert(clip.id.as_str()) {
            o.add(
                format!("{p}.id"),
                format!("clip id `{}` is used more than once", clip.id),
            );
        }
        if !(clip.duration_sec.is_finite() && clip.duration_sec > 0.0) {
            o.add(
                format!("{p}.durationSec"),
                "durationSec must be greater than 0",
            );
        }
        months(
            &mut o,
            &p,
            clip.min_months,
            clip.max_months,
            "minMonths",
            "maxMonths",
        );
        let mut seen_batteries = BTreeSet::new();
        for (j, m) in clip.protocols.iter().enumerate() {
            let q = format!("{p}.protocols[{j}]");
            if m.id.trim().is_empty() {
                o.add(format!("{q}.id"), "battery id is required");
                continue;
            }
            if m.order < 0 {
                o.add(format!("{q}.order"), "order must not be negative");
            } else if let Some(first) = orders
                .entry(m.id.as_str())
                .or_default()
                .insert(m.order, clip.id.as_str())
            {
                o.add(
                    format!("{q}.order"),
                    format!(
                        "order {} in battery `{}` is already used by clip `{first}`",
                        m.order, m.id
                    ),
                );
            }
            if !seen_batteries.insert(m.id.as_str()) {
                o.add(
                    format!("{q}.id"),
                    format!("the clip is in battery `{}` more than once", m.id),
                );
            }
        }
        areas(clip, &p, &mut o);
    }
    o.0
}

/// [`catalog_problems`] as messages only.
pub fn catalog_problem_messages(c: &StimulusCatalog) -> Vec<String> {
    catalog_problems(c).into_iter().map(|p| p.message).collect()
}

fn required(o: &mut Out, path: &str, value: &str, what: &str) {
    if value.trim().is_empty() {
        o.add(path, format!("{what} is required"));
    }
}

fn months(o: &mut Out, prefix: &str, min: i64, max: i64, min_name: &str, max_name: &str) {
    if min < 0 {
        o.add(
            format!("{prefix}.{min_name}"),
            format!("{min_name} must not be negative"),
        );
    }
    if max < 0 {
        o.add(
            format!("{prefix}.{max_name}"),
            format!("{max_name} must not be negative"),
        );
    }
    if min >= 0 && max >= 0 && min > max {
        o.add(
            format!("{prefix}.{max_name}"),
            format!("{max_name} must not be below {min_name}"),
        );
    }
}

fn languages(c: &StimulusCatalog, o: &mut Out) {
    if c.languages.is_empty() {
        o.add("languages", "languages must not be empty");
    }
    let mut seen = BTreeSet::new();
    for (i, l) in c.languages.iter().enumerate() {
        if l.trim().is_empty() {
            o.add(format!("languages[{i}]"), "language code is required");
        } else if !seen.insert(l.as_str()) {
            o.add(
                format!("languages[{i}]"),
                format!("language `{l}` is listed more than once"),
            );
        }
    }
    if c.fallback_language.trim().is_empty() {
        o.add("fallbackLanguage", "fallbackLanguage is required");
    } else if !c.languages.contains(&c.fallback_language) {
        o.add(
            "fallbackLanguage",
            format!(
                "fallbackLanguage `{}` must be one of languages",
                c.fallback_language
            ),
        );
    }
}

fn age_packs(c: &StimulusCatalog, o: &mut Out) {
    let mut ids = BTreeSet::new();
    for (i, p) in c.age_packs.iter().enumerate() {
        let path = format!("agePacks[{i}]");
        if p.id.trim().is_empty() {
            o.add(format!("{path}.id"), "age pack id is required");
        } else if !ids.insert(p.id.as_str()) {
            o.add(
                format!("{path}.id"),
                format!("age pack id `{}` is used more than once", p.id),
            );
        }
        months(
            o,
            &path,
            p.min_months,
            p.max_months,
            "minMonths",
            "maxMonths",
        );
    }
    for (j, b) in c.age_packs.iter().enumerate() {
        let valid =
            |x: &asd_core::types::AgePack| 0 <= x.min_months && x.min_months <= x.max_months;
        if !valid(b) {
            continue;
        }
        if let Some(a) = c.age_packs[..j]
            .iter()
            .find(|a| valid(a) && a.min_months <= b.max_months && b.min_months <= a.max_months)
        {
            o.add(
                format!("agePacks[{j}].minMonths"),
                format!("age pack `{}` overlaps `{}`", b.id, a.id),
            );
        }
    }
}

fn areas(clip: &asd_core::types::Clip, p: &str, o: &mut Out) {
    let mut names = BTreeSet::new();
    for (j, a) in clip.areas.iter().enumerate() {
        let q = format!("{p}.areas[{j}]");
        if a.name.trim().is_empty() {
            o.add(format!("{q}.name"), "area name is required");
        } else if !names.insert(a.name.as_str()) {
            o.add(
                format!("{q}.name"),
                format!("area name `{}` is used more than once in the clip", a.name),
            );
        }
        let coords = [
            ("xMin", a.x_min),
            ("yMin", a.y_min),
            ("xMax", a.x_max),
            ("yMax", a.y_max),
        ];
        for (k, v) in coords {
            if !(0.0..=1.0).contains(&v) {
                o.add(format!("{q}.{k}"), format!("{k} must be in [0, 1]"));
            }
        }
        if a.x_min >= a.x_max {
            o.add(format!("{q}.xMax"), "xMax must be greater than xMin");
        }
        if a.y_min >= a.y_max {
            o.add(format!("{q}.yMax"), "yMax must be greater than yMin");
        }
        if a.start_sec.is_nan() || a.start_sec < 0.0 {
            o.add(format!("{q}.startSec"), "startSec must not be negative");
        }
        if a.start_sec.partial_cmp(&a.end_sec) != Some(std::cmp::Ordering::Less) {
            o.add(
                format!("{q}.endSec"),
                "endSec must be greater than startSec",
            );
        } else if clip.duration_sec > 0.0 && a.end_sec > clip.duration_sec {
            o.add(
                format!("{q}.endSec"),
                format!(
                    "endSec must not exceed the clip duration ({} s)",
                    clip.duration_sec
                ),
            );
        }
        let mut roles = BTreeSet::new();
        for r in &a.roles {
            if !AOI_ROLES.contains(&r.as_str()) {
                o.add(
                    format!("{q}.roles"),
                    format!("unknown role `{r}` (known: {})", AOI_ROLES.join(", ")),
                );
            } else if !roles.insert(r.as_str()) {
                o.add(
                    format!("{q}.roles"),
                    format!("role `{r}` is listed more than once"),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_core::types::{AgePack, Area, Clip, Membership};

    fn area(name: &str) -> Area {
        Area {
            name: name.into(),
            start_sec: 0.0,
            end_sec: 8.0,
            x_min: 0.1,
            y_min: 0.1,
            x_max: 0.5,
            y_max: 0.5,
            roles: vec!["social".into()],
        }
    }

    fn clip(id: &str, order: i64) -> Clip {
        Clip {
            id: id.into(),
            title: "T".into(),
            category: "c".into(),
            duration_sec: 8.0,
            min_months: 12,
            max_months: 60,
            protocols: vec![Membership {
                id: "express".into(),
                order,
            }],
            areas: vec![area("A")],
        }
    }

    fn catalog() -> StimulusCatalog {
        StimulusCatalog {
            schema_version: "1.0.0".into(),
            comment: None,
            languages: vec!["en".into(), "ne".into()],
            fallback_language: "en".into(),
            age_packs: vec![
                AgePack {
                    id: "a".into(),
                    min_months: 0,
                    max_months: 23,
                },
                AgePack {
                    id: "b".into(),
                    min_months: 24,
                    max_months: 60,
                },
            ],
            clips: vec![clip("c1", 0), clip("c2", 1)],
        }
    }

    fn paths(c: &StimulusCatalog) -> Vec<String> {
        catalog_problems(c).into_iter().map(|p| p.path).collect()
    }

    #[test]
    fn a_good_catalog_is_valid() {
        assert!(catalog_problems(&catalog()).is_empty());
    }

    #[test]
    fn bundled_catalog_is_valid() {
        let raw = include_str!("../../asd-backend/seed/stimulus_catalog.json");
        let c: StimulusCatalog = serde_json::from_str(raw).unwrap();
        assert_eq!(catalog_problems(&c), []);
    }

    #[test]
    fn settings_rules() {
        let mut c = catalog();
        c.schema_version = " ".into();
        c.languages = vec!["en".into(), "en".into(), "".into()];
        c.fallback_language = "fr".into();
        assert_eq!(
            paths(&c),
            [
                "schemaVersion",
                "languages[1]",
                "languages[2]",
                "fallbackLanguage"
            ]
        );
        c.languages.clear();
        assert_eq!(
            paths(&c),
            ["schemaVersion", "languages", "fallbackLanguage"]
        );
    }

    #[test]
    fn age_pack_rules() {
        let mut c = catalog();
        c.age_packs[1].min_months = 20; // overlaps a
        c.age_packs.push(AgePack {
            id: "a".into(),
            min_months: 70,
            max_months: 65,
        });
        c.age_packs.push(AgePack {
            id: "".into(),
            min_months: -1,
            max_months: 5,
        });
        assert_eq!(
            paths(&c),
            [
                "agePacks[2].id",
                "agePacks[2].maxMonths",
                "agePacks[3].id",
                "agePacks[3].minMonths",
                "agePacks[1].minMonths",
            ]
        );
    }

    #[test]
    fn clip_and_battery_rules() {
        let mut c = catalog();
        c.clips[1].id = "c1".into();
        c.clips[1].duration_sec = 0.0;
        c.clips[1].min_months = 70;
        c.clips[1].protocols[0].order = 0; // taken by c1 in `express`
        c.clips[1].protocols.push(Membership {
            id: "express".into(),
            order: 5,
        });
        assert_eq!(
            paths(&c),
            [
                "clips[1].id",
                "clips[1].durationSec",
                "clips[1].maxMonths",
                "clips[1].protocols[0].order",
                "clips[1].protocols[1].id",
            ]
        );
        assert_eq!(
            catalog_problem_messages(&c)[3],
            "order 0 in battery `express` is already used by clip `c1`"
        );
        let mut c = catalog();
        c.clips.clear();
        assert_eq!(paths(&c), ["clips"]);
    }

    #[test]
    fn aoi_rules() {
        let mut c = catalog();
        let a = &mut c.clips[0].areas;
        a[0].x_min = 0.6;
        a[0].y_max = 1.5;
        a[0].start_sec = 4.0;
        a[0].end_sec = 9.0;
        a[0].roles = vec!["social".into(), "face".into(), "social".into()];
        a.push(area("A"));
        assert_eq!(
            paths(&c),
            [
                "clips[0].areas[0].yMax",
                "clips[0].areas[0].xMax",
                "clips[0].areas[0].endSec",
                "clips[0].areas[0].roles",
                "clips[0].areas[0].roles",
                "clips[0].areas[1].name",
            ]
        );
        let mut c = catalog();
        c.clips[0].areas[0].start_sec = 5.0;
        c.clips[0].areas[0].end_sec = 5.0;
        assert_eq!(paths(&c), ["clips[0].areas[0].endSec"]);
        c.clips[0].areas[0].roles.clear();
        c.clips[0].areas[0].start_sec = f64::NAN;
        assert_eq!(
            paths(&c),
            ["clips[0].areas[0].startSec", "clips[0].areas[0].endSec"]
        );
    }
}
