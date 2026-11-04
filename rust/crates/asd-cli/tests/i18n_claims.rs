//! owner: WP-L.0
//!
//! I18N-3 rule 7: `asd_contract::claims` is a copy of the Python claims scanner's lists.
//! The Python file stays untouched; this test reads it as text and fails when
//! `GLOBAL_DENY` (ids and patterns, in order) or `NEGATION` / `DIAGNOS` differ.

use asd_contract::claims::{DENY_TERMS, DIAGNOS, NEGATION};

const SCANNER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../scripts/check_asd_claims.py"
));

/// The text of the first `r"..."` after `from`.
fn raw_string(from: &str) -> &str {
    let start = from.find("r\"").expect("a raw string") + 2;
    let end = from[start..].find('"').expect("closing quote");
    &from[start..start + end]
}

fn python_global_deny() -> Vec<(String, String)> {
    let start = SCANNER
        .find("GLOBAL_DENY = [")
        .expect("GLOBAL_DENY in the scanner");
    let body = &SCANNER[start..];
    let end = body.find("\n]").expect("end of GLOBAL_DENY");
    body[..end]
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("(\""))
        .map(|l| {
            let id = l[2..].split('"').next().unwrap().to_owned();
            // The pattern is the first raw string after the id's closing quote.
            let pattern = raw_string(&l[2 + id.len() + 1..]).to_owned();
            (id, pattern)
        })
        .collect()
}

#[test]
fn deny_terms_equal_the_python_list() {
    let python = python_global_deny();
    assert!(python.len() >= 20, "parsed {} entries", python.len());
    let rust: Vec<(String, String)> = DENY_TERMS
        .iter()
        .map(|(id, p, _)| ((*id).to_owned(), (*p).to_owned()))
        .collect();
    assert_eq!(
        rust, python,
        "update asd_contract::claims::DENY_TERMS from scripts/check_asd_claims.py"
    );
}

#[test]
fn negation_and_diagnos_equal_the_python_patterns() {
    let negation = SCANNER.find("\nNEGATION = re.compile(").expect("NEGATION");
    assert_eq!(raw_string(&SCANNER[negation..]), NEGATION);
    let diagnos = SCANNER.find("\nDIAGNOS = re.compile(").expect("DIAGNOS");
    assert_eq!(raw_string(&SCANNER[diagnos..]), DIAGNOS);
}
