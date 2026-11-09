//! owner: WP-L.0
//!
//! The shared domain vocabulary catalogue (ADR-0013 principle 7; spec
//! `docs/spec/modules/asd-localisation/technical.md` I18N-1..5, I18N-11, I18N-12).
//!
//! - `i18n/domain.en.json`, `domain.ne.json` and the partial `domain.hi.json` are flat JSON
//!   objects `key -> text`, sorted, compiled in with `include_str!` and parsed once. Keys are
//!   `group.value` (`encounter_kind.screening_visit`, `problem.case-consent-missing`,
//!   `param.flag.greenFloor.label`); placeholders are `{name}`.
//! - [`required_keys`] is computed from Rust (enum values through their JSON Schema, the
//!   parameter registry, the problem slugs, [`asd_core::flag::FlagCase`]), so a new enum
//!   value or registry row without text fails `cargo test -p asd-contract`.
//! - [`problems`] runs the I18N-3 checks; `asd-cli i18n check` runs them too.
//! - Surfaces never call Rust to translate UI text at runtime: they read the generated
//!   artefacts (`asd-cli i18n export`, the `i18n-v1.json` export of [`i18n_document`]).
//! - [`negotiate`] picks the locale: user > org > `Accept-Language` > `en` (I18N-12), with
//!   the shared fixture [`negotiation_cases`].
//!
//! Every `ne` (and `hi`) text written by an agent is a machine draft: `draft: true` in
//! `i18n-v1.json` until WP-L.3's reviewer signs the key off by adding it to [`REVIEWED_NE`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    api_v1, care, claims, clinical, datasets, emr, export, mirror, params, problem, research,
};

/// Bumped when a key is removed or changes meaning (adding keys keeps it).
pub const CATALOGUE_VERSION: &str = "1.0.0";
/// File name of the catalogue export.
pub const I18N_FILE: &str = "i18n-v1.json";
/// `$id` of the catalogue export.
pub const I18N_ID: &str = "https://schemas.curv.org/asd/i18n-v1.json";
/// File name of the negotiation fixture (I18N-12), read by the Rust and console tests.
pub const NEGOTIATION_FILE: &str = "i18n-negotiation-cases.json";

/// The compiled-in catalogue files.
pub const EN_SOURCE: &str = include_str!("../i18n/domain.en.json");
pub const NE_SOURCE: &str = include_str!("../i18n/domain.ne.json");
pub const HI_SOURCE: &str = include_str!("../i18n/domain.hi.json");

/// Groups whose keys are not derived from Rust (I18N-3 rule 2). Each entry needs a comment
/// saying why. Empty in WP-L.0.
pub const FREE_GROUPS: &[&str] = &[];

/// `ne` values that may be Latin only (I18N-3 rule 5): unit symbols and format names.
pub const LATIN_OK: &[&str] = &[
    "export_format.csv",
    "export_format.fhir_bundle",
    "export_format.fhir_ndjson",
    "export_format.hl7v2",
    "export_format.xlsx",
    "dataset_format.jsonl",
    "dataset_format.parquet",
    "unit.cm",
    "unit.deg",
    "unit.deg_per_sec",
    "unit.hz",
    "unit.millis",
    "unit.mm",
    "unit.percent",
    "unit.rad",
    "unit.seconds",
];

/// Keys exempt from the claims rule (I18N-3 rule 7), with the reason. Kept short: a domain
/// label that can avoid a denied term does.
pub const CLAIMS_OK: &[(&str, &str)] = &[
    (
        "flag_assessment.differs.detail",
        "ADR-0004 mandatory caregiver sentence written by asd-core (I18N-4)",
    ),
    (
        "flag_assessment.matches.detail",
        "ADR-0004 mandatory caregiver sentence written by asd-core (I18N-4)",
    ),
    (
        "flag_assessment.no_scored_clips.detail",
        "ADR-0004 mandatory caregiver sentence written by asd-core (I18N-4)",
    ),
    (
        "flag_assessment.some_differ.detail",
        "ADR-0004 mandatory caregiver sentence written by asd-core (I18N-4)",
    ),
    (
        "flag_assessment.unreliable.detail",
        "ADR-0004 mandatory caregiver sentence written by asd-core (I18N-4)",
    ),
    (
        "problem.clinical-crypto-unavailable",
        "clinician-only console error naming the clinical record store",
    ),
    (
        "problem.clinical-role-not-assignable",
        "clinician-only console error naming the clinical roles",
    ),
    (
        "export_purpose.clinical_care",
        "clinician-only export purpose (XC-5), the term of the consent purpose",
    ),
    (
        "timeline_entry_kind.clinical_review",
        "clinician-only timeline entry; the console label since WP-C.9",
    ),
];

/// `ne` keys reviewed by a native-speaking clinician (WP-L.3). Every other `ne` value is a
/// machine draft (`draft: true` in `i18n-v1.json`).
pub const REVIEWED_NE: &[&str] = &[];

// ---- locales ------------------------------------------------------------------------------

/// A console and backend locale (I18N-14): stored in `users.locale` and
/// `organizations.default_locale` (migration 0028).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    En,
    Ne,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Ne];

    /// Wire and column spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ne => "ne",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "en" => Some(Self::En),
            "ne" => Some(Self::Ne),
            _ => None,
        }
    }
}

/// A catalogue file locale. `hi` is partial and falls back to `en`; it is exported to the
/// native apps only (owner question 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogueLocale {
    En,
    Ne,
    Hi,
}

impl CatalogueLocale {
    pub const ALL: [CatalogueLocale; 3] = [
        CatalogueLocale::En,
        CatalogueLocale::Ne,
        CatalogueLocale::Hi,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ne => "ne",
            Self::Hi => "hi",
        }
    }
}

impl From<Locale> for CatalogueLocale {
    fn from(l: Locale) -> Self {
        match l {
            Locale::En => Self::En,
            Locale::Ne => Self::Ne,
        }
    }
}

// ---- the catalogue ------------------------------------------------------------------------

/// The three catalogue files as text (the compiled-in ones, or a fixture's).
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    pub en: &'a str,
    pub ne: &'a str,
    pub hi: Option<&'a str>,
}

/// The compiled-in files.
pub const fn sources() -> Sources<'static> {
    Sources {
        en: EN_SOURCE,
        ne: NE_SOURCE,
        hi: Some(HI_SOURCE),
    }
}

/// One key in every locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub en: String,
    /// Empty when the `ne` file lacks the key ([`problems`] reports it).
    pub ne: String,
    pub hi: Option<String>,
    /// Placeholder names of the `en` value, in order of first appearance.
    pub params: Vec<String>,
    /// The `ne` value is a machine draft (not in [`REVIEWED_NE`]).
    pub draft: bool,
}

/// The parsed catalogue: every key of the `en` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogue {
    pub version: &'static str,
    pub entries: BTreeMap<String, Entry>,
}

/// Parses one catalogue file (a flat JSON object of strings).
pub fn parse_file(text: &str) -> Result<BTreeMap<String, String>, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// The text form `i18n fmt` writes and rule 8 expects: two-space pretty JSON, keys sorted,
/// UTF-8 unescaped, trailing newline.
pub fn format_file(map: &BTreeMap<String, String>) -> String {
    let mut text = serde_json::to_string_pretty(map).expect("a string map always serialises");
    text.push('\n');
    text
}

impl Catalogue {
    /// Builds a catalogue from file texts; `Err` when a file is not a JSON string map.
    pub fn from_sources(src: &Sources<'_>) -> Result<Self, String> {
        let en = parse_file(src.en).map_err(|e| format!("domain.en.json: {e}"))?;
        let ne = parse_file(src.ne).map_err(|e| format!("domain.ne.json: {e}"))?;
        let hi = match src.hi {
            Some(t) => parse_file(t).map_err(|e| format!("domain.hi.json: {e}"))?,
            None => BTreeMap::new(),
        };
        let entries = en
            .into_iter()
            .map(|(key, en_text)| {
                let entry = Entry {
                    params: placeholders(&en_text),
                    ne: ne.get(&key).cloned().unwrap_or_default(),
                    hi: hi.get(&key).cloned(),
                    draft: !REVIEWED_NE.contains(&key.as_str()),
                    en: en_text,
                };
                (key, entry)
            })
            .collect();
        Ok(Self {
            version: CATALOGUE_VERSION,
            entries,
        })
    }

    /// The text of `key` in `locale`: `hi` (and a missing `ne`) fall back to `en`.
    pub fn text(&self, locale: CatalogueLocale, key: &str) -> Option<&str> {
        let e = self.entries.get(key)?;
        let localised = match locale {
            CatalogueLocale::En => None,
            CatalogueLocale::Ne => Some(e.ne.as_str()).filter(|s| !s.is_empty()),
            CatalogueLocale::Hi => e.hi.as_deref(),
        };
        Some(localised.unwrap_or(&e.en))
    }
}

/// The compiled-in catalogue, parsed once. The files are checked by `cargo test`; a file
/// that does not parse yields an empty catalogue rather than a panic on a device.
pub fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        Catalogue::from_sources(&sources()).unwrap_or(Catalogue {
            version: CATALOGUE_VERSION,
            entries: BTreeMap::new(),
        })
    })
}

/// The text of `key` in `locale` from the compiled-in catalogue.
pub fn text(locale: CatalogueLocale, key: &str) -> Option<&'static str> {
    catalogue().text(locale, key)
}

/// [`text`] with every `{name}` replaced by its argument; an unknown placeholder stays as
/// written. For Rust code that must produce final text (WP-L.4); no plural or number logic.
pub fn render(locale: CatalogueLocale, key: &str, args: &[(&str, &str)]) -> Option<String> {
    Some(substitute(text(locale, key)?, args))
}

/// Replaces `{name}` placeholders in `template`.
pub fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if is_placeholder(&after[..close]) => {
                let name = &after[..close];
                match args.iter().find(|(n, _)| *n == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_placeholder(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Placeholder names of `value` (`{name}`, ASCII identifier), in order of first appearance.
pub fn placeholders(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if is_placeholder(&after[..close]) => {
                let name = after[..close].to_owned();
                if !out.contains(&name) {
                    out.push(name);
                }
                rest = &after[close + 1..];
            }
            _ => rest = after,
        }
    }
    out
}

// ---- required keys (I18N-2) ---------------------------------------------------------------

/// One group of required keys and where they come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGroup {
    /// The first key segment (`encounter_kind`).
    pub group: String,
    /// Full keys.
    pub keys: Vec<String>,
}

impl KeyGroup {
    /// `group.<value>` for every value.
    pub fn of_values<S: AsRef<str>>(group: &str, values: impl IntoIterator<Item = S>) -> Self {
        let mut keys: Vec<String> = values
            .into_iter()
            .map(|v| format!("{group}.{}", v.as_ref()))
            .collect();
        keys.dedup();
        Self {
            group: group.to_owned(),
            keys,
        }
    }

    /// `group.<value>` for every wire value of a serde enum (read from its JSON Schema, so
    /// a new variant is a new required key without a hand-kept list).
    pub fn of_enum<T: JsonSchema>(group: &str) -> Self {
        Self::of_values(group, enum_values::<T>())
    }
}

/// The wire values of a unit-variant enum, from its JSON Schema (`enum`, or `oneOf` /
/// `anyOf` of `const` / `enum` when variants carry doc comments).
pub fn enum_values<T: JsonSchema>() -> Vec<String> {
    let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap_or(Value::Null);
    let mut out = Vec::new();
    collect_enum_values(&schema, &mut out);
    out
}

fn collect_enum_values(schema: &Value, out: &mut Vec<String>) {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        out.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    if let Some(v) = schema.get("const").and_then(Value::as_str) {
        out.push(v.to_owned());
    }
    for k in ["oneOf", "anyOf"] {
        if let Some(items) = schema.get(k).and_then(Value::as_array) {
            for item in items {
                collect_enum_values(item, out);
            }
        }
    }
}

/// Every required group (the I18N-2 table), in a fixed order.
pub fn required_groups() -> Vec<KeyGroup> {
    let mut attention: Vec<String> = enum_values::<mirror::AttentionFlag>();
    for v in enum_values::<api_v1::SessionAttentionFlag>() {
        if !attention.contains(&v) {
            attention.push(v);
        }
    }
    let flag_keys = asd_core::flag::FlagCase::ALL
        .iter()
        .flat_map(|c| {
            [
                format!("{}.headline", c.key()),
                format!("{}.detail", c.key()),
            ]
        })
        .collect();
    let registry = params::registry();
    let param_keys = registry
        .iter()
        .flat_map(|p| [p.label_key.clone(), p.help_key.clone()])
        .collect();
    let mut param_groups: Vec<&str> = Vec::new();
    for p in registry {
        let g = p.id.split('.').next().unwrap_or(&p.id);
        if !param_groups.contains(&g) {
            param_groups.push(g);
        }
    }

    vec![
        KeyGroup::of_enum::<emr::EncounterKind>("encounter_kind"),
        KeyGroup::of_enum::<emr::EncounterStatus>("encounter_status"),
        KeyGroup::of_enum::<emr::AttachmentKind>("attachment_kind"),
        KeyGroup::of_enum::<care::CareEventKind>("care_event_kind"),
        KeyGroup::of_enum::<api_v1::Role>("role"),
        KeyGroup::of_enum::<api_v1::UserStatus>("user_status"),
        KeyGroup::of_enum::<api_v1::OrgKind>("org_kind"),
        KeyGroup::of_enum::<api_v1::ReviewReason>("review_reason"),
        KeyGroup::of_enum::<api_v1::ReviewStatus>("review_status"),
        KeyGroup::of_enum::<clinical::ClinicalReviewState>("clinical_review_state"),
        KeyGroup::of_enum::<clinical::Disposition>("disposition"),
        KeyGroup::of_enum::<clinical::CosignPolicy>("cosign_policy"),
        KeyGroup::of_enum::<clinical::PatientRefMode>("patient_ref_mode"),
        KeyGroup::of_enum::<clinical::ClinicalNoteKind>("clinical_note_kind"),
        KeyGroup::of_enum::<clinical::LetterKind>("letter_kind"),
        KeyGroup::of_enum::<clinical::LetterStatus>("letter_status"),
        KeyGroup::of_enum::<clinical::TimelineEntryKind>("timeline_entry_kind"),
        KeyGroup::of_values("attention_flag", attention),
        KeyGroup {
            group: "flag_assessment".into(),
            keys: flag_keys,
        },
        KeyGroup::of_enum::<mirror::ExpectationStatus>("expectation_status"),
        KeyGroup::of_enum::<mirror::GazeQualityVerdict>("quality_verdict"),
        KeyGroup::of_enum::<mirror::TrackingVerdict>("tracking_verdict"),
        KeyGroup::of_enum::<mirror::CalibrationQuality>("calibration_quality"),
        KeyGroup::of_enum::<api_v1::ScoringStatus>("scoring_status"),
        KeyGroup::of_enum::<mirror::CaptureSource>("capture_source"),
        KeyGroup::of_enum::<export::ExportFormat>("export_format"),
        KeyGroup::of_enum::<export::ExportMode>("export_mode"),
        KeyGroup::of_enum::<export::ExportPurpose>("export_purpose"),
        KeyGroup::of_enum::<export::ExportStatus>("export_status"),
        KeyGroup::of_enum::<export::DatasetFormat>("dataset_format"),
        KeyGroup::of_enum::<research::ExperimentStatus>("experiment_status"),
        KeyGroup::of_enum::<research::RunKind>("run_kind"),
        KeyGroup::of_enum::<research::RunStatus>("run_status"),
        KeyGroup::of_enum::<research::Level>("level"),
        // WP-X.11 (ADR-0015 R-7..R-9): datasets.
        KeyGroup::of_enum::<datasets::DatasetStatus>("dataset_status"),
        KeyGroup::of_enum::<datasets::SplitName>("dataset_split"),
        KeyGroup::of_enum::<datasets::LabelSourceKind>("label_source_kind"),
        KeyGroup::of_enum::<datasets::ExclusionRuleKind>("exclusion_rule_kind"),
        KeyGroup::of_values(
            "dataset_column",
            datasets::DICTIONARY_COLUMNS.iter().map(|c| c.0),
        ),
        KeyGroup::of_enum::<api_v1::ConfigDocumentKind>("config_document_kind"),
        KeyGroup::of_enum::<api_v1::ConfigDocumentStatus>("config_document_status"),
        KeyGroup {
            group: "param".into(),
            keys: param_keys,
        },
        KeyGroup::of_values("param_group", param_groups),
        KeyGroup::of_enum::<params::Unit>("unit"),
        KeyGroup::of_values("age_band", ["range", "range_open"]),
        // DSP-3/DSP-7 (WP-X.0c): the result-screen bands and the metrics they colour.
        KeyGroup::of_values(
            "display_band",
            asd_core::display::BandStatus::ALL.map(|s| s.as_str()),
        ),
        KeyGroup::of_values(
            "display_metric",
            asd_core::display::DisplayMetric::ALL.map(|m| m.as_str()),
        ),
        KeyGroup::of_values("problem", problem::ALL_SLUGS.iter().copied()),
    ]
}

/// The keys of `groups`.
pub fn keys_of(groups: &[KeyGroup]) -> BTreeSet<String> {
    groups.iter().flat_map(|g| g.keys.iter().cloned()).collect()
}

/// Every key the catalogue must hold (I18N-2).
pub fn required_keys() -> BTreeSet<String> {
    keys_of(&required_groups())
}

// ---- checks (I18N-3) ----------------------------------------------------------------------

/// One catalogue finding. `key` is a catalogue key, or the file name for file-level rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueProblem {
    pub key: String,
    pub locale: Option<CatalogueLocale>,
    /// `json`, `key`, `missing`, `orphan`, `extra`, `placeholder`, `devanagari`,
    /// `whitespace`, `quote`, `claims`, `format`.
    pub rule: &'static str,
    pub message: String,
}

impl fmt::Display for CatalogueProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.locale {
            Some(l) => write!(
                f,
                "{} [{}] {}: {}",
                self.key,
                l.as_str(),
                self.rule,
                self.message
            ),
            None => write!(f, "{} {}: {}", self.key, self.rule, self.message),
        }
    }
}

/// The I18N-3 checks on the compiled-in catalogue against [`required_keys`].
pub fn problems() -> Vec<CatalogueProblem> {
    problems_for(&sources(), &required_keys())
}

fn key_grammar_ok(key: &str) -> bool {
    let mut parts = key.split('.');
    let first = parts.next().unwrap_or("");
    let mut fc = first.chars();
    let first_ok = fc.next().is_some_and(|c| c.is_ascii_lowercase())
        && fc.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    let rest: Vec<&str> = parts.collect();
    first_ok
        && !rest.is_empty()
        && rest.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

fn has_devanagari(s: &str) -> bool {
    s.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

/// The I18N-3 checks on `src` against `required`.
pub fn problems_for(src: &Sources<'_>, required: &BTreeSet<String>) -> Vec<CatalogueProblem> {
    let mut out = Vec::new();
    let mut push =
        |key: &str, locale: Option<CatalogueLocale>, rule: &'static str, message: String| {
            out.push(CatalogueProblem {
                key: key.to_owned(),
                locale,
                rule,
                message,
            })
        };

    let mut files: Vec<(CatalogueLocale, &str, BTreeMap<String, String>)> = Vec::new();
    let named = [
        (CatalogueLocale::En, Some(src.en)),
        (CatalogueLocale::Ne, Some(src.ne)),
        (CatalogueLocale::Hi, src.hi),
    ];
    for (locale, text) in named {
        let Some(text) = text else { continue };
        let file = format!("domain.{}.json", locale.as_str());
        match parse_file(text) {
            Err(e) => push(
                &file,
                Some(locale),
                "json",
                format!("not a JSON object of strings: {e}"),
            ),
            Ok(map) => {
                // Rule 8: sorted and formatted exactly as `i18n fmt` writes it.
                if format_file(&map) != text {
                    push(
                        &file,
                        Some(locale),
                        "format",
                        "not sorted or not formatted as `asd-cli i18n fmt` writes it".into(),
                    );
                }
                files.push((locale, text, map));
            }
        }
    }
    let get = |l: CatalogueLocale| files.iter().find(|(x, _, _)| *x == l).map(|(_, _, m)| m);
    let (Some(en), Some(ne)) = (get(CatalogueLocale::En), get(CatalogueLocale::Ne)) else {
        return out;
    };
    let hi = get(CatalogueLocale::Hi);
    let empty = BTreeMap::new();

    for key in en.keys() {
        if !key_grammar_ok(key) {
            push(
                key,
                Some(CatalogueLocale::En),
                "key",
                "key does not match ^[a-z][a-z0-9_]*(\\.[A-Za-z0-9_-]+)+$".into(),
            );
        }
    }

    // Rule 1: required keys present and non-empty in en and ne.
    for key in required {
        for (locale, map) in [(CatalogueLocale::En, en), (CatalogueLocale::Ne, ne)] {
            match map.get(key) {
                None => push(
                    key,
                    Some(locale),
                    "missing",
                    "required key has no text".into(),
                ),
                Some(v) if v.trim().is_empty() => push(
                    key,
                    Some(locale),
                    "missing",
                    "required key has an empty text".into(),
                ),
                Some(_) => {}
            }
        }
    }
    // Rule 2: no orphan in en.
    for key in en.keys() {
        let group = key.split('.').next().unwrap_or("");
        if !required.contains(key) && !FREE_GROUPS.contains(&group) {
            push(
                key,
                Some(CatalogueLocale::En),
                "orphan",
                "key is not required by any Rust source and its group is not in FREE_GROUPS".into(),
            );
        }
    }
    // Rule 3: ne and hi have no key en lacks.
    for (locale, map) in [
        (CatalogueLocale::Ne, ne),
        (CatalogueLocale::Hi, hi.unwrap_or(&empty)),
    ] {
        for key in map.keys().filter(|k| !en.contains_key(*k)) {
            push(
                key,
                Some(locale),
                "extra",
                "key is not in domain.en.json".into(),
            );
        }
    }

    for (key, en_value) in en {
        let en_params: BTreeSet<String> = placeholders(en_value).into_iter().collect();
        let localised = [
            (CatalogueLocale::Ne, ne.get(key)),
            (CatalogueLocale::Hi, hi.and_then(|h| h.get(key))),
        ];
        // Rule 4: same placeholder set.
        for (locale, value) in localised {
            if let Some(v) = value {
                let set: BTreeSet<String> = placeholders(v).into_iter().collect();
                if set != en_params {
                    push(
                        key,
                        Some(locale),
                        "placeholder",
                        format!("placeholders {set:?} differ from en {en_params:?}"),
                    );
                }
            }
        }
        // Rule 5: ne has Devanagari.
        if let Some(v) = ne.get(key) {
            if !v.trim().is_empty() && !has_devanagari(v) && !LATIN_OK.contains(&key.as_str()) {
                push(
                    key,
                    Some(CatalogueLocale::Ne),
                    "devanagari",
                    "ne text has no Devanagari letter (add the key to LATIN_OK if Latin is right)"
                        .into(),
                );
            }
        }
        // Rule 6: whitespace and straight quotes.
        let all = [
            (CatalogueLocale::En, Some(en_value)),
            (CatalogueLocale::Ne, ne.get(key)),
            (CatalogueLocale::Hi, hi.and_then(|h| h.get(key))),
        ];
        for (locale, value) in all {
            let Some(v) = value else { continue };
            if v.trim() != v || v.contains("  ") {
                push(
                    key,
                    Some(locale),
                    "whitespace",
                    "leading/trailing whitespace or a double space".into(),
                );
            }
            if v.contains('\'') || v.contains('"') {
                push(
                    key,
                    Some(locale),
                    "quote",
                    "straight quote: use ’ “ ”".into(),
                );
            }
        }
        // Rule 7: claims terms (en and ne).
        if !CLAIMS_OK.iter().any(|(k, _)| k == key) {
            for (locale, value) in [
                (CatalogueLocale::En, Some(en_value)),
                (CatalogueLocale::Ne, ne.get(key)),
            ] {
                let Some(v) = value else { continue };
                for hit in claims::check(v) {
                    push(
                        key,
                        Some(locale),
                        "claims",
                        format!("ADR-0004 term [{}] \"{}\"", hit.rule, hit.matched),
                    );
                }
            }
        }
    }
    out
}

// ---- negotiation (I18N-12) ----------------------------------------------------------------

/// The locale for a request: the user's override, else the org default, else the best
/// `Accept-Language` match, else `en`.
pub fn negotiate(
    user: Option<Locale>,
    org: Option<Locale>,
    accept_language: Option<&str>,
) -> Locale {
    user.or(org)
        .or_else(|| accept_language.and_then(match_accept_language))
        .unwrap_or(Locale::En)
}

/// The best `Accept-Language` match (RFC 9110 weighted list): `q=0` and malformed weights
/// are excluded; the primary subtag decides (`ne`, `ne-NP` -> `ne`; `en-*` -> `en`); any
/// other range (also `*`) is ignored; ties keep header order.
pub fn match_accept_language(header: &str) -> Option<Locale> {
    let mut best: Option<(f64, Locale)> = None;
    for item in header.split(',') {
        let mut parts = item.split(';');
        let range = parts.next().unwrap_or("").trim();
        let mut q = Some(1.0);
        for p in parts {
            let p = p.trim();
            if let Some(v) = p.strip_prefix("q=").or_else(|| p.strip_prefix("Q=")) {
                q = v
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|q| (0.0..=1.0).contains(q));
            }
        }
        let Some(q) = q.filter(|q| *q > 0.0) else {
            continue;
        };
        let primary = range.split('-').next().unwrap_or("").to_ascii_lowercase();
        let Some(locale) = Locale::parse(&primary) else {
            continue;
        };
        if best.is_none_or(|(b, _)| q > b) {
            best = Some((q, locale));
        }
    }
    best.map(|(_, l)| l)
}

/// One row of the negotiation fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiationCase {
    pub user: Option<Locale>,
    pub org: Option<Locale>,
    pub accept_language: Option<String>,
    pub expected: Locale,
}

/// The shared negotiation fixture (`i18n-negotiation-cases.json`): the Rust test and the
/// console's TypeScript test both run every row.
pub fn negotiation_cases() -> Vec<NegotiationCase> {
    use Locale::{En, Ne};
    /// `(user, org, Accept-Language, expected)`.
    type Row = (Option<Locale>, Option<Locale>, Option<&'static str>, Locale);
    let rows: &[Row] = &[
        (Some(Ne), Some(En), Some("en"), Ne),
        (Some(En), Some(Ne), Some("ne"), En),
        (Some(Ne), None, Some("en-US"), Ne),
        (None, Some(Ne), Some("en"), Ne),
        (None, Some(En), Some("ne-NP"), En),
        (None, Some(Ne), None, Ne),
        (None, None, Some("ne-NP,en;q=0.5"), Ne),
        (None, None, Some("ne-NP;q=0.8,en;q=0.9"), En),
        (None, None, Some("en-US,en;q=0.9"), En),
        (None, None, Some("ne"), Ne),
        (None, None, Some("NE-np"), Ne),
        (None, None, Some("ne-IN"), Ne),
        (None, None, Some("fr"), En),
        (None, None, Some("fr,ne;q=0.5"), Ne),
        (None, None, Some("ne;q=0"), En),
        (None, None, Some("ne;q=0,en;q=0"), En),
        (None, None, Some("*"), En),
        (None, None, Some("*,ne;q=0.1"), Ne),
        (None, None, Some(""), En),
        (None, None, None, En),
        (None, None, Some("garbage;;;==,,"), En),
        (None, None, Some("ne;q=abc"), En),
        (None, None, Some("ne;q=1.5"), En),
        (None, None, Some("en;q=0.5,ne;q=0.5"), En),
        (None, None, Some("ne;q=0.5,en;q=0.5"), Ne),
        (None, None, Some(" ne-NP ; q=0.7 , hi;q=0.9"), Ne),
        (None, None, Some("en-GB;q=0.2, ne;q=0.3"), Ne),
        (None, None, Some("nepali"), En),
        (None, None, Some("hi-IN,hi;q=0.9"), En),
    ];
    rows.iter()
        .map(|(user, org, al, expected)| NegotiationCase {
            user: *user,
            org: *org,
            accept_language: al.map(str::to_owned),
            expected: *expected,
        })
        .collect()
}

// ---- exports (I18N-5) ---------------------------------------------------------------------

/// `i18n-v1.json`: the catalogue as the contract other surfaces and translators read.
pub fn i18n_document() -> Value {
    i18n_document_of(catalogue())
}

/// [`i18n_document`] of any catalogue.
pub fn i18n_document_of(cat: &Catalogue) -> Value {
    let entries: serde_json::Map<String, Value> = cat
        .entries
        .iter()
        .map(|(key, e)| {
            let mut v = json!({ "en": e.en, "ne": e.ne, "params": e.params });
            if let Some(hi) = &e.hi {
                v["hi"] = json!(hi);
            }
            if e.draft {
                v["draft"] = json!(true);
            }
            (key.clone(), v)
        })
        .collect();
    json!({
        "$id": I18N_ID,
        "catalogueVersion": cat.version,
        "locales": ["en", "ne"],
        "partialLocales": ["hi"],
        "entries": entries,
    })
}

/// `i18n-negotiation-cases.json`.
pub fn negotiation_cases_json() -> Value {
    serde_json::to_value(negotiation_cases()).expect("cases serialise")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_and_placeholders() {
        assert_eq!(placeholders("{min}–{max} mo {min}"), ["min", "max"]);
        assert_eq!(placeholders("a {1x} {} {ok}"), ["ok"]);
        assert_eq!(
            substitute("{min}–{max} mo", &[("min", "24"), ("max", "35")]),
            "24–35 mo"
        );
        assert_eq!(substitute("{a} {b}", &[("a", "1")]), "1 {b}");
        assert_eq!(substitute("x { y }", &[]), "x { y }");
    }

    #[test]
    fn key_grammar() {
        assert!(key_grammar_ok("encounter_kind.screening_visit"));
        assert!(key_grammar_ok("param.flag.greenFloor.label"));
        assert!(key_grammar_ok("problem.case-consent-missing"));
        assert!(!key_grammar_ok("Encounter.x"));
        assert!(!key_grammar_ok("nogroup"));
        assert!(!key_grammar_ok("a..b"));
    }

    #[test]
    fn enum_values_read_doc_commented_enums_too() {
        assert_eq!(enum_values::<Locale>(), ["en", "ne"]);
        assert!(enum_values::<emr::EncounterKind>().contains(&"screening_visit".to_string()));
        assert_eq!(
            enum_values::<mirror::CaptureSource>(),
            ["truedepth", "simulated", "synthetic"]
        );
    }
}
