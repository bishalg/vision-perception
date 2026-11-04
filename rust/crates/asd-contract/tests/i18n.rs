//! owner: WP-L.0
//!
//! The domain catalogue (I18N-1..5), its checks (I18N-3), the flag texts (I18N-4) and the
//! locale negotiation (I18N-12). Acceptance 1, 2 and 5 of
//! `docs/spec/modules/asd-localisation/acceptance.md`.

use std::collections::BTreeMap;

use asd_contract::i18n::{
    self, format_file, keys_of, negotiate, parse_file, problems_for, required_groups, Catalogue,
    CatalogueLocale, CatalogueProblem, KeyGroup, Locale, Sources,
};
use asd_core::flag::FlagCase;
use schemars::JsonSchema;
use serde::Serialize;

fn map(text: &str) -> BTreeMap<String, String> {
    parse_file(text).expect("catalogue file parses")
}

/// The compiled-in sources with one edit to a file's map.
fn edited(
    locale: CatalogueLocale,
    edit: impl FnOnce(&mut BTreeMap<String, String>),
) -> (String, String, String) {
    let src = i18n::sources();
    let (mut en, mut ne, mut hi) = (map(src.en), map(src.ne), map(src.hi.unwrap()));
    match locale {
        CatalogueLocale::En => edit(&mut en),
        CatalogueLocale::Ne => edit(&mut ne),
        CatalogueLocale::Hi => edit(&mut hi),
    }
    (format_file(&en), format_file(&ne), format_file(&hi))
}

fn check(files: &(String, String, String)) -> Vec<CatalogueProblem> {
    let src = Sources {
        en: &files.0,
        ne: &files.1,
        hi: Some(&files.2),
    };
    problems_for(&src, &i18n::required_keys())
}

fn render(problems: &[CatalogueProblem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn catalogue_is_complete_and_clean() {
    let problems = i18n::problems();
    assert!(problems.is_empty(), "{}", render(&problems));
    let cat = i18n::catalogue();
    assert_eq!(cat.entries.len(), i18n::required_keys().len());
    assert!(
        cat.entries.values().all(|e| e.draft),
        "every ne text is a draft until WP-L.3"
    );
}

#[test]
fn deleting_a_ne_key_is_one_named_problem() {
    let files = edited(CatalogueLocale::Ne, |m| {
        m.remove("param.flag.greenFloor.label");
    });
    let problems = check(&files);
    assert_eq!(problems.len(), 1, "{}", render(&problems));
    assert_eq!(problems[0].key, "param.flag.greenFloor.label");
    assert_eq!(problems[0].locale, Some(CatalogueLocale::Ne));
    assert_eq!(problems[0].rule, "missing");
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum TestOnlyKind {
    TeleVisit,
}

#[test]
fn a_new_enum_value_without_text_is_named_in_both_locales() {
    // A test-only enum registered under a test group, and a new value in a real group
    // (acceptance 1: `TeleVisit => "tele_visit"` in `EncounterKind`).
    let mut groups = required_groups();
    groups.push(KeyGroup::of_enum::<TestOnlyKind>("test_group"));
    let encounter = groups
        .iter_mut()
        .find(|g| g.group == "encounter_kind")
        .unwrap();
    encounter.keys.push("encounter_kind.tele_visit".into());
    let src = i18n::sources();
    let problems = problems_for(&src, &keys_of(&groups));
    let named: Vec<(String, Option<CatalogueLocale>, &str)> = problems
        .iter()
        .map(|p| (p.key.clone(), p.locale, p.rule))
        .collect();
    assert_eq!(
        named,
        [
            (
                "encounter_kind.tele_visit".to_string(),
                Some(CatalogueLocale::En),
                "missing"
            ),
            (
                "encounter_kind.tele_visit".to_string(),
                Some(CatalogueLocale::Ne),
                "missing"
            ),
            (
                "test_group.tele_visit".to_string(),
                Some(CatalogueLocale::En),
                "missing"
            ),
            (
                "test_group.tele_visit".to_string(),
                Some(CatalogueLocale::Ne),
                "missing"
            ),
        ]
    );
}

#[test]
fn an_unrequired_key_is_an_orphan() {
    let files = edited(CatalogueLocale::En, |m| {
        m.insert("encounter_kind.tele_visit".into(), "Tele visit".into());
    });
    let problems = check(&files);
    assert_eq!(problems.len(), 1, "{}", render(&problems));
    assert_eq!(problems[0].rule, "orphan");
}

fn single_problem(locale: CatalogueLocale, key: &str, value: &str, rule: &str) -> CatalogueProblem {
    let files = edited(locale, |m| {
        m.insert(key.into(), value.into());
    });
    let problems = check(&files);
    assert_eq!(problems.len(), 1, "{rule}: {}", render(&problems));
    assert_eq!(problems[0].rule, rule);
    assert_eq!(problems[0].key, key);
    problems.into_iter().next().unwrap()
}

#[test]
fn placeholder_mismatch_is_one_problem() {
    single_problem(
        CatalogueLocale::Ne,
        "age_band.range",
        "{min} महिना",
        "placeholder",
    );
}

#[test]
fn missing_devanagari_is_one_problem() {
    single_problem(CatalogueLocale::Ne, "role.owner", "Owner", "devanagari");
}

#[test]
fn a_claims_term_is_one_problem() {
    // Acceptance 1: "diagnosis" in an `en` value.
    let p = single_problem(
        CatalogueLocale::En,
        "role.owner",
        "Owner of the diagnosis",
        "claims",
    );
    assert!(p.message.contains("diagnos"), "{p}");
    single_problem(
        CatalogueLocale::En,
        "role.viewer",
        "Screening viewer",
        "claims",
    );
}

#[test]
fn whitespace_and_straight_quotes_are_problems() {
    single_problem(CatalogueLocale::En, "role.owner", "Owner ", "whitespace");
    single_problem(CatalogueLocale::En, "role.owner", "Owner's", "quote");
}

#[test]
fn an_extra_ne_key_is_one_problem() {
    single_problem(CatalogueLocale::Ne, "role.nobody", "कोही", "extra");
}

#[test]
fn an_unsorted_file_is_one_problem() {
    let src = i18n::sources();
    // The same entries with the first two swapped: valid JSON, wrong order.
    let m = map(src.en);
    let mut pairs: Vec<(&String, &String)> = m.iter().collect();
    pairs.swap(0, 1);
    let body = pairs
        .iter()
        .map(|(k, v)| {
            format!(
                "  {}: {}",
                serde_json::to_string(k).unwrap(),
                serde_json::to_string(v).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let swapped = format!("{{\n{body}\n}}\n");
    let problems = problems_for(
        &Sources {
            en: &swapped,
            ne: src.ne,
            hi: src.hi,
        },
        &i18n::required_keys(),
    );
    assert_eq!(problems.len(), 1, "{}", render(&problems));
    assert_eq!(problems[0].rule, "format");
    assert_eq!(problems[0].key, "domain.en.json");
}

#[test]
fn flag_case_texts_are_the_catalogue_en_values() {
    // I18N-4 / acceptance 5: the English devices write exists once in practice.
    for case in FlagCase::ALL {
        let headline = format!("{}.headline", case.key());
        let detail = format!("{}.detail", case.key());
        assert_eq!(
            i18n::text(CatalogueLocale::En, &headline),
            Some(case.headline()),
            "{headline}"
        );
        assert_eq!(
            i18n::text(CatalogueLocale::En, &detail),
            Some(case.detail()),
            "{detail}"
        );
        assert_ne!(
            i18n::text(CatalogueLocale::Ne, &headline),
            Some(case.headline())
        );
    }
}

#[test]
fn text_render_and_fallbacks() {
    assert_eq!(
        i18n::render(
            CatalogueLocale::Ne,
            "age_band.range",
            &[("min", "24"), ("max", "35")]
        )
        .as_deref(),
        Some("24–35 महिना")
    );
    assert_eq!(
        i18n::render(CatalogueLocale::En, "age_band.range_open", &[("min", "48")]).as_deref(),
        Some("48+ mo")
    );
    // hi is partial: a key without hi text falls back to en.
    assert_eq!(
        i18n::text(CatalogueLocale::Hi, "role.owner"),
        i18n::text(CatalogueLocale::En, "role.owner")
    );
    assert_eq!(
        i18n::text(CatalogueLocale::Hi, "attention_flag.green"),
        Some("हरा")
    );
    assert_eq!(i18n::text(CatalogueLocale::En, "no.such.key"), None);
    let cat = Catalogue::from_sources(&i18n::sources()).unwrap();
    assert_eq!(cat.entries["age_band.range"].params, ["min", "max"]);
}

#[test]
fn negotiation_fixture_passes() {
    let cases = i18n::negotiation_cases();
    assert!(cases.len() >= 20);
    for c in &cases {
        assert_eq!(
            negotiate(c.user, c.org, c.accept_language.as_deref()),
            c.expected,
            "{c:?}"
        );
    }
    // The fixture covers the cases the spec names.
    for header in ["ne;q=0", "ne-NP;q=0.8,en;q=0.9", "*", "", "garbage;;;==,,"] {
        assert!(
            cases
                .iter()
                .any(|c| c.accept_language.as_deref() == Some(header)),
            "{header}"
        );
    }
    assert!(cases.iter().any(|c| c.accept_language.is_none()));
}

#[test]
fn negotiation_order_is_user_org_header_en() {
    assert_eq!(
        negotiate(Some(Locale::Ne), Some(Locale::En), Some("en")),
        Locale::Ne
    );
    assert_eq!(negotiate(None, Some(Locale::Ne), Some("en")), Locale::Ne);
    assert_eq!(negotiate(None, None, Some("ne-NP,en;q=0.5")), Locale::Ne);
    assert_eq!(negotiate(None, None, None), Locale::En);
}

#[test]
fn i18n_document_shape() {
    let doc = i18n::i18n_document();
    assert_eq!(doc["$id"], i18n::I18N_ID);
    assert_eq!(doc["catalogueVersion"], i18n::CATALOGUE_VERSION);
    let entries = doc["entries"].as_object().unwrap();
    assert_eq!(entries.len(), i18n::required_keys().len());
    for (key, e) in entries {
        assert!(e["en"].as_str().is_some_and(|s| !s.is_empty()), "{key}");
        assert!(e["ne"].as_str().is_some_and(|s| !s.is_empty()), "{key}");
        assert_eq!(e["draft"], true, "{key}");
    }
    assert_eq!(
        entries["age_band.range"]["params"],
        serde_json::json!(["min", "max"])
    );
    assert_eq!(entries["attention_flag.green"]["hi"], "हरा");
}

#[test]
fn locale_wire_form() {
    assert_eq!(serde_json::to_string(&Locale::Ne).unwrap(), "\"ne\"");
    let p: asd_contract::clinical::UpdateProfileRequest =
        serde_json::from_str(r#"{"locale": null}"#).unwrap();
    assert_eq!(p.locale, Some(None));
    let p: asd_contract::clinical::UpdateProfileRequest = serde_json::from_str("{}").unwrap();
    assert_eq!(p.locale, None);
    let p: asd_contract::clinical::UpdateProfileRequest =
        serde_json::from_str(r#"{"locale": "ne"}"#).unwrap();
    assert_eq!(p.locale, Some(Some(Locale::Ne)));
    assert!(
        serde_json::from_str::<asd_contract::clinical::UpdateProfileRequest>(r#"{"locale": "fr"}"#)
            .is_err()
    );
}
